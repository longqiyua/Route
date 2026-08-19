//! Transaction journal for crash-recoverable destructive operations.
//!
//! ## Goal
//!
//! Route cannot make cross-file rollback truly atomic without a heavy
//! database. What it *can* do is make every destructive operation
//! **detectable** and **recoverable**: after a crash at any point, the
//! next `BasicRepository::open` knows an operation was in flight, what
//! it intended, and how to converge the repository back to a consistent
//! state.
//!
//! This is deliberately not ACID. It is "crash-recoverable with a
//! single defined commit point". The principle is:
//!
//! > Prefer recovery over impossible global atomicity.
//!
//! ## Layout
//!
//! ```text
//! .route-basic/transactions/<tx_id>/
//!     intent.json     # what the operation intended to do
//!     state           # one line: PREPARED | APPLYING | COMMITTED | ABORTED
//! ```
//!
//! `intent.json` is written first (fsynced). `state` is updated by
//! atomic temp-write-rename and is the **commit point**.
//!
//! ## State machine
//!
//! ```text
//! PREPARED ──► APPLYING ──► COMMITTED ──► (removed)
//!                 │
//!                 └──► ABORTED ──► (removed, only allowed before any file write)
//! ```
//!
//! Recovery on `open`:
//!
//! | state found | meaning | action |
//! |-------------|---------|--------|
//! | PREPARED | intent written, no files touched | safe discard |
//! | APPLYING | files / metadata being mutated | re-run apply (idempotent) + metadata commit, then mark COMMITTED |
//! | COMMITTED | code + metadata done; maybe conv not aligned | if conv intent present, hand to conv reconciler; then remove |
//! | ABORTED | explicitly abandoned before writes | safe discard |
//! | corrupt / unknown | — | surface `CorruptedJournal`, refuse mutations |
//!
//! ## Idempotency
//!
//! `apply_files_to_project` is idempotent (per-file write-to-temp-rename
//! + `INSERT OR IGNORE` commit + `UPDATE HEAD = to_snapshot`). Therefore
//! re-running APPLYING recovery never makes things worse. `recover()`
//! can be called repeatedly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use route_core::RouteError;

/// One relative path → blob hash, for the apply phase.
pub type TargetFiles = HashMap<String, String>;

/// Conversation-side intent. `None` for a pure code rollback.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConvIntent {
    /// Filesystem path to the conversation store JSON.
    pub storage_path: String,
    pub session_id: String,
    /// The message id to roll back to (messages after it are removed).
    pub target_message_id: String,
    /// The snapshot id the conversation should align to.
    pub target_snapshot_id: String,
}

/// What a transaction intends to do. Persisted to `intent.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionIntent {
    pub tx_id: String,
    /// Deterministic commit-edge id for the rollback commit. Reused
    /// across recovery so that `INSERT OR IGNORE` is truly idempotent
    /// — a recovery re-run never creates a second rollback edge.
    pub commit_id: String,
    /// "rollback" | "rollback_to_message"
    pub kind: String,
    pub branch: String,
    /// HEAD before the operation.
    pub from_snapshot: String,
    /// Snapshot the operation restores.
    pub to_snapshot: String,
    /// Files to apply (rel → blob hash). Captured at PREPARED time so
    /// recovery does not need to re-resolve (which could differ if the
    /// DB changed). Resolution at PREPARED is part of the snapshot
    /// contract.
    pub target_files: TargetFiles,
    /// Attribution fields for the rollback commit edge.
    pub operator: Option<String>,
    pub body: Option<String>,
    pub is_ai: bool,
    pub message: String,
    pub started_at: i64,
    /// Conversation linkage, if this is a `rollback_to_message`.
    #[serde(default)]
    pub conv: Option<ConvIntent>,
}

/// Transaction state. Persisted as a single line in `state`.
///
/// ## State machine
///
/// ```text
/// PREPARED ──► APPLYING ──► COMMITTED ──► COMPLETED ──► (removed)
///                 │                            │
///                 └──► ABORTED              COMMITTED ──► COMMITTED_PENDING_CLEANUP ──► (retry)
///                                                │
///                                                └──► COMPLETED (normal cleanup)
/// ```
///
/// The key distinction:
/// - `COMMITTED` = data is consistent. Cleanup may still be needed.
/// - `COMMITTED_PENDING_CLEANUP` = data is consistent, but journal cleanup
///   failed (e.g. permission denied on directory removal). The transaction
///   is fully applied; only stale metadata remains.
/// - `COMPLETED` = fully done, ready for removal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TxState {
    /// Intent written, no project files touched yet. Safe to discard.
    Prepared,
    /// Apply in progress (files being written / deleted, metadata not
    /// yet committed). On recovery: re-run apply + metadata commit.
    Applying,
    /// Code + metadata committed. Conv may still need reconcile.
    /// From this state, the transaction can advance to either
    /// `COMPLETED` (normal cleanup) or `COMMITTED_PENDING_CLEANUP`
    /// (cleanup failed, retry on next open).
    Committed,
    /// Code + metadata committed, but cleanup (journal removal) failed.
    /// The repository is healthy — only stale metadata remains.
    /// On next open, recovery retries the cleanup.
    CommittedPendingCleanup,
    /// Fully done; journal removal was attempted (may or may not have
    /// succeeded — this is a terminal record). Safe to discard.
    Completed,
    /// Explicitly abandoned before any write. Safe to discard.
    Aborted,
}

impl TxState {
    fn as_str(self) -> &'static str {
        match self {
            TxState::Prepared => "PREPARED",
            TxState::Applying => "APPLYING",
            TxState::Committed => "COMMITTED",
            TxState::CommittedPendingCleanup => "COMMITTED_PENDING_CLEANUP",
            TxState::Completed => "COMPLETED",
            TxState::Aborted => "ABORTED",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "PREPARED" => Some(TxState::Prepared),
            "APPLYING" => Some(TxState::Applying),
            "COMMITTED" => Some(TxState::Committed),
            "COMMITTED_PENDING_CLEANUP" => Some(TxState::CommittedPendingCleanup),
            "COMPLETED" => Some(TxState::Completed),
            "ABORTED" => Some(TxState::Aborted),
            _ => None,
        }
    }
}

/// Handle to the transactions directory of a repository.
#[derive(Debug, Clone)]
pub struct TransactionJournal {
    root: PathBuf,
}

impl TransactionJournal {
    /// Create a journal handle rooted at `<route_dir>/transactions/`.
    /// Does NOT create the directory; call [`Self::ensure_dir`].
    pub fn new(route_dir: &Path) -> Self {
        Self {
            root: route_dir.join("transactions"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Ensure the transactions directory exists.
    pub fn ensure_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)
    }

    fn tx_dir(&self, tx_id: &str) -> PathBuf {
        self.root.join(tx_id)
    }

    fn intent_path(&self, tx_id: &str) -> PathBuf {
        self.tx_dir(tx_id).join("intent.json")
    }

    fn state_path(&self, tx_id: &str) -> PathBuf {
        self.tx_dir(tx_id).join("state")
    }

    /// Atomically write bytes to a target path (temp → fsync → rename).
    /// Used for `state` and `intent.json`.
    fn atomic_write(target: &Path, bytes: &[u8]) -> Result<()> {
        if let Some(parent) = target.parent() {
            let p = parent.to_path_buf();
            std::fs::create_dir_all(&p).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::Error::new(RouteError::PermissionDenied(format!(
                        "cannot create journal directory {}: {e}",
                        p.display()
                    )))
                } else {
                    anyhow::Error::from(e)
                }
            })?;
        }
        let tmp = target.with_extension(format!(
            "route-jtmp-{}-{}",
            std::process::id(),
            route_core::new_id()
        ));
        let _ = std::fs::remove_file(&tmp);
        std::fs::write(&tmp, bytes).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(RouteError::PermissionDenied(format!(
                    "cannot write journal temp file {}: {e}",
                    tmp.display()
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        if let Ok(f) = std::fs::File::open(&tmp) {
            let _ = f.sync_all();
        }
        std::fs::rename(&tmp, target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(RouteError::PermissionDenied(format!(
                    "cannot rename journal temp file to {}: {e}",
                    target.display()
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Begin a new transaction: write `intent.json` and set state to
    /// PREPARED. Fills `intent.tx_id` and `intent.commit_id` in place
    /// (so the caller can reuse the populated intent for later metadata
    /// writes — the commit edge id MUST match what was persisted, so
    /// recovery re-runs are idempotent). Returns the tx id.
    pub fn begin(&self, intent: &mut TransactionIntent) -> Result<String> {
        self.ensure_dir()?;
        if intent.tx_id.is_empty() {
            intent.tx_id = route_core::new_id();
        }
        if intent.commit_id.is_empty() {
            intent.commit_id = route_core::new_id();
        }
        let tx_id = intent.tx_id.clone();
        let dir = self.tx_dir(&tx_id);
        std::fs::create_dir_all(&dir).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(RouteError::PermissionDenied(format!(
                    "cannot create transaction directory {}: {e}",
                    dir.display()
                )))
            } else {
                anyhow::Error::from(e).context(format!("create tx dir {}", dir.display()))
            }
        })?;
        // Write intent first (fsynced via atomic_write).
        let intent_json = serde_json::to_vec_pretty(intent)?;
        Self::atomic_write(&self.intent_path(&tx_id), &intent_json)?;
        // Then state = PREPARED.
        Self::atomic_write(&self.state_path(&tx_id), b"PREPARED\n")?;
        Ok(tx_id)
    }

    /// Transition the transaction to a new state. States may only
    /// advance forward or to specific fallback states. Allowed
    /// transitions:
    ///
    /// ```text
    /// PREPARED → APPLYING
    /// PREPARED → ABORTED
    /// APPLYING → COMMITTED
    /// COMMITTED → COMPLETED          (normal cleanup)
    /// COMMITTED → COMMITTED_PENDING_CLEANUP  (cleanup failed)
    /// COMMITTED_PENDING_CLEANUP → COMPLETED   (retry cleanup)
    /// ```
    ///
    /// Idempotent re-write of the same state is always allowed.
    /// Other transitions return `CorruptedJournal`.
    pub fn set_state(&self, tx_id: &str, new_state: TxState) -> Result<()> {
        let current = self
            .read_state(tx_id)?
            .ok_or_else(|| RouteError::CorruptedJournal {
                path: self.tx_dir(tx_id),
                detail: "state file missing".into(),
            })?;
        // Enforce forward progression.
        let ok = match (current, new_state) {
            (TxState::Prepared, TxState::Applying)
            | (TxState::Prepared, TxState::Aborted)
            | (TxState::Applying, TxState::Committed)
            | (TxState::Committed, TxState::Completed)
            | (TxState::Committed, TxState::CommittedPendingCleanup)
            | (TxState::CommittedPendingCleanup, TxState::Completed) => true,
            // Idempotent re-write of the same state is fine (recovery).
            (a, b) if a == b => true,
            _ => false,
        };
        if !ok {
            return Err(RouteError::CorruptedJournal {
                path: self.tx_dir(tx_id),
                detail: format!("illegal state transition {:?} -> {:?}", current, new_state),
            }
            .into());
        }
        Self::atomic_write(
            &self.state_path(tx_id),
            format!("{}\n", new_state.as_str()).as_bytes(),
        )?;
        Ok(())
    }

    /// Read the current state of a transaction, if any.
    pub fn read_state(&self, tx_id: &str) -> Result<Option<TxState>> {
        let p = self.state_path(tx_id);
        match std::fs::read_to_string(&p) {
            Ok(s) => match TxState::parse(&s) {
                Some(st) => Ok(Some(st)),
                None => Err(RouteError::CorruptedJournal {
                    path: p,
                    detail: format!("unrecognised state content: {s:?}"),
                }
                .into()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Read the intent of a transaction.
    pub fn read_intent(&self, tx_id: &str) -> Result<TransactionIntent> {
        let p = self.intent_path(tx_id);
        let content = std::fs::read_to_string(&p).map_err(|e| RouteError::CorruptedJournal {
            path: p.clone(),
            detail: format!("cannot read intent: {e}"),
        })?;
        serde_json::from_str(&content).map_err(|e| {
            RouteError::CorruptedJournal {
                path: p,
                detail: format!("cannot parse intent: {e}"),
            }
            .into()
        })
    }

    /// Remove a completed transaction directory. Safe only after the
    /// operation is fully committed and (for conv-linked tx) reconciled.
    pub fn remove(&self, tx_id: &str) -> Result<()> {
        let dir = self.tx_dir(tx_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::Error::new(RouteError::PermissionDenied(format!(
                        "cannot remove journal directory {}: {e}",
                        dir.display()
                    )))
                } else {
                    anyhow::Error::from(e).context(format!("remove tx dir {}", dir.display()))
                }
            })?;
        }
        Ok(())
    }

    /// List all pending transaction ids (any tx directory that exists).
    pub fn list_pending(&self) -> Result<Vec<String>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    out.push(name.to_string());
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Read (tx_id, intent, state) for every pending transaction.
    /// Entries that cannot be parsed are returned as a `CorruptedJournal`
    /// error — the caller decides whether to abort all mutations.
    pub fn list_pending_full(&self) -> Result<Vec<(String, TransactionIntent, TxState)>> {
        let mut out = Vec::new();
        for tx_id in self.list_pending()? {
            let intent = self.read_intent(&tx_id)?;
            let state = self
                .read_state(&tx_id)?
                .ok_or_else(|| RouteError::CorruptedJournal {
                    path: self.tx_dir(&tx_id),
                    detail: "state file missing".into(),
                })?;
            out.push((tx_id, intent, state));
        }
        Ok(out)
    }

    /// Remove stale `.route-jtmp-*` temp files left by a crashed
    /// atomic_write inside the journal directory. Housekeeping; safe
    /// to call anytime.
    pub fn cleanup_stale_temps(&self) {
        let Ok(rd) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in rd.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.contains(".route-jtmp-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

impl TransactionIntent {
    /// Construct a rollback intent with a fresh tx id and commit id.
    pub fn new_rollback(
        branch: String,
        from_snapshot: String,
        to_snapshot: String,
        target_files: TargetFiles,
        message: String,
        operator: Option<String>,
        body: Option<String>,
        is_ai: bool,
    ) -> Self {
        Self {
            tx_id: String::new(),
            commit_id: String::new(),
            kind: "rollback".into(),
            branch,
            from_snapshot,
            to_snapshot,
            target_files,
            operator,
            body,
            is_ai,
            message,
            started_at: route_core::now_millis(),
            conv: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn journal() -> (TempDir, TransactionJournal) {
        let tmp = TempDir::new().unwrap();
        let j = TransactionJournal::new(tmp.path());
        j.ensure_dir().unwrap();
        (tmp, j)
    }

    fn sample_intent() -> TransactionIntent {
        TransactionIntent::new_rollback(
            "main".into(),
            "snap-from".into(),
            "snap-to".into(),
            HashMap::from([("a.txt".into(), "hash-a".into())]),
            "back".into(),
            Some("user".into()),
            None,
            false,
        )
    }

    #[test]
    fn begin_writes_prepared_state() {
        let (_tmp, j) = journal();
        let mut intent = sample_intent();
        let id = j.begin(&mut intent).unwrap();
        assert_eq!(j.read_state(&id).unwrap(), Some(TxState::Prepared));
        let read_back = j.read_intent(&id).unwrap();
        assert_eq!(read_back.to_snapshot, "snap-to");
        assert_eq!(read_back.tx_id, id);
        assert!(!read_back.commit_id.is_empty());
    }

    #[test]
    fn state_transitions_forward_only() {
        let (_tmp, j) = journal();
        let mut intent = sample_intent();
        let id = j.begin(&mut intent).unwrap();
        j.set_state(&id, TxState::Applying).unwrap();
        // Going back to Prepared is illegal.
        assert!(j.set_state(&id, TxState::Prepared).is_err());
        j.set_state(&id, TxState::Committed).unwrap();
        // Idempotent re-commit is fine.
        j.set_state(&id, TxState::Committed).unwrap();
        // Advance to Completed (normal cleanup).
        j.set_state(&id, TxState::Completed).unwrap();
        // Idempotent re-write of Completed.
        j.set_state(&id, TxState::Completed).unwrap();
    }

    #[test]
    fn committed_pending_cleanup_retry() {
        let (_tmp, j) = journal();
        let mut intent = sample_intent();
        let id = j.begin(&mut intent).unwrap();
        j.set_state(&id, TxState::Applying).unwrap();
        j.set_state(&id, TxState::Committed).unwrap();
        // Cleanup failed → CommittedPendingCleanup.
        j.set_state(&id, TxState::CommittedPendingCleanup).unwrap();
        // On next open, retry cleanup → Completed.
        j.set_state(&id, TxState::Completed).unwrap();
        // Cannot go back from Completed.
        assert!(j.set_state(&id, TxState::Committed).is_err());
    }

    #[test]
    fn abort_only_from_prepared() {
        let (_tmp, j) = journal();
        let mut intent = sample_intent();
        let id = j.begin(&mut intent).unwrap();
        j.set_state(&id, TxState::Aborted).unwrap();
        // Aborting after Applying is illegal.
        let mut intent2 = sample_intent();
        let id2 = j.begin(&mut intent2).unwrap();
        j.set_state(&id2, TxState::Applying).unwrap();
        assert!(j.set_state(&id2, TxState::Aborted).is_err());
    }

    #[test]
    fn list_pending_returns_all() {
        let (_tmp, j) = journal();
        let mut a = sample_intent();
        let mut b = sample_intent();
        let _ = j.begin(&mut a).unwrap();
        let _ = j.begin(&mut b).unwrap();
        let pending = j.list_pending().unwrap();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn remove_deletes_dir() {
        let (_tmp, j) = journal();
        let mut intent = sample_intent();
        let id = j.begin(&mut intent).unwrap();
        j.remove(&id).unwrap();
        assert!(j.list_pending().unwrap().is_empty());
    }

    #[test]
    fn corrupted_state_is_detected() {
        let (_tmp, j) = journal();
        let mut intent = sample_intent();
        let id = j.begin(&mut intent).unwrap();
        std::fs::write(j.state_path(&id), b"GARBAGE\n").unwrap();
        assert!(j.read_state(&id).is_err());
    }
}
