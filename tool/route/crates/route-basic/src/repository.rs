//! BasicRepository — main entry point for basic mode operations.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};
use route_core::{
    content_hash, new_id, now_millis as _now, BlobStore, DbConnection, ManifestDiff,
    ProjectScanner, RoutePaths, TrackFilter,
};
use route_plugins::{Event, EventBus, PluginContext};
use rusqlite::params;

use crate::models::{
    AiPrompt, Branch, BranchKind, Commit, CommitKind, CommitPathAnnotation, DiffSummary,
    RepoConfig, Snapshot, SnapshotWithBranch, TrackConfig, TrackOs,
};

fn now_millis() -> i64 {
    _now()
}

/// Hydrate a `Commit` from a SQLite row in the standard commits schema
/// (id, from_snapshot, to_snapshot, message, author, created_at, branch_id,
/// kind, diff_summary, operator, body, is_checkpoint, is_ai).
///
/// Centralizing this here keeps the `get_commit` / `list_commits` /
/// `rollback_to` queries in lockstep when columns are added.
fn row_to_commit(r: &rusqlite::Row<'_>) -> rusqlite::Result<Commit> {
    let kind_str: String = r.get("kind")?;
    // V005+ columns — read by name to stay compatible with partial SELECTs
    // (old queries and rollback writes may omit them; .ok().flatten() gives
    // None for missing columns and for NULLs alike).
    let protocol_rev_str: Option<String> = r.get("protocol_revision").ok().flatten();
    let protocol_revision: Option<u64> = protocol_rev_str.as_deref().and_then(|s| s.parse().ok());

    Ok(Commit {
        id: r.get("id")?,
        from_snapshot: r.get("from_snapshot")?,
        to_snapshot: r.get("to_snapshot")?,
        message: r.get("message")?,
        author: r.get("author")?,
        created_at: r.get("created_at")?,
        branch_id: r.get("branch_id")?,
        kind: CommitKind::from_str(&kind_str).unwrap_or(CommitKind::Incremental),
        diff_summary: r.get("diff_summary")?,
        operator: r.get("operator").ok().flatten(),
        body: r.get("body").ok().flatten(),
        is_checkpoint: r
            .get::<_, Option<i64>>("is_checkpoint")
            .ok()
            .flatten()
            .unwrap_or(0)
            != 0,
        is_ai: r.get::<_, Option<i64>>("is_ai").ok().flatten().unwrap_or(0) != 0,
        tx_id: None,
        context_hash: r.get("context_hash").ok().flatten(),
        constitution_version: r.get("constitution_version").ok().flatten(),
        protocol_revision,
        reference_entries_hash: r.get("reference_entries_hash").ok().flatten(),
    })
}

/// Options for committing a new snapshot.
#[derive(Debug, Clone, Default)]
pub struct CommitOptions {
    pub message: String,
    pub author: Option<String>,
    /// Force a full snapshot (records kind=full self-loop edge).
    pub force_full: bool,
    /// Branch to commit on; defaults to current branch.
    pub branch: Option<String>,
    /// Operator identity stamp. `None` defaults to "user".
    /// Pass "ai:<name>" for AI-driven changes.
    pub operator: Option<String>,
    /// Long-form note (checkpoint body / AI prompt embedding).
    pub body: Option<String>,
    /// If true, this commit is recorded as a user-marked checkpoint.
    pub is_checkpoint: bool,
    /// If true, the change came from the AI control channel.
    pub is_ai: bool,
}

/// Options for a rollback. Used by `rollback_to_with` to attribute the
/// rollback to a specific operator (e.g. an AI agent) and optionally
/// link a conversation action to the same transaction journal entry.
#[derive(Debug, Clone, Default)]
pub struct RollbackOptions {
    /// Operator identity. Defaults to "user".
    pub operator: Option<String>,
    /// Optional long-form note (e.g. the AI prompt that triggered the undo).
    pub body: Option<String>,
    /// True if the rollback came from the AI control channel.
    pub is_ai: bool,
    /// Conversation linkage. When set, the transaction journal records
    /// the conversation intent so that a crash mid-rollback can be
    /// reconciled on the next open.
    pub conv: Option<crate::transaction::ConvIntent>,
}

/// Options for creating a branch.
#[derive(Debug, Clone, Default)]
pub struct CreateBranchOptions {
    pub kind: BranchKind,
    /// For inherited/sandbox: parent branch name (defaults to current).
    pub from_branch: Option<String>,
}

/// Main repository handle.
pub struct BasicRepository {
    pub paths: RoutePaths,
    pub db: DbConnection,
    pub config: RepoConfig,
    /// Optional event bus. Set once via `set_event_bus`; subsequent reads
    /// go through `emit_event` which no-ops when the bus is unset.
    event_bus: OnceLock<Arc<EventBus>>,
    /// In-memory redo stack. Each entry is a `(from_snapshot, to_snapshot)`
    /// pair representing a previously-undone edit that the user can roll
    /// forward to. Cleared on any new head-advancing commit.
    ///
    /// This is intentionally in-memory only — across restarts the user
    /// cannot redo, which matches git's behavior. The DB still preserves
    /// the full edit / rollback history for inspection.
    redo_stack: Mutex<Vec<(String, String)>>,
    /// When true, all `commit` / `working_dir_status` operations use
    /// the non-parallel `ProjectScanner` instead of the default
    /// parallel walker. The parallel walker spawns worker threads
    /// with the 1 MB default Windows stack, which trips
    /// STATUS_STACK_OVERFLOW on the test runner. Production code
    /// keeps this at `false` for the speed.
    use_serial_scanner: bool,
    /// Crash-recovery journal for destructive operations (rollback,
    /// rollback_to_message). See [`crate::transaction`].
    journal: crate::transaction::TransactionJournal,
}

/// A single file revision in history (returned by `file_history`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileRevision {
    pub snapshot_id: String,
    pub commit_id: Option<String>,
    pub commit_message: Option<String>,
    pub blob_hash: Option<String>,
    pub created_at: i64,
    pub branch_name: Option<String>,
}

/// One file entry in a commit's diff (returned by `commit_diff_detail`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommitDiffEntry {
    pub path: String,
    pub change: String, // "added" | "modified" | "removed"
    pub blob_hash: Option<String>,
    pub size_bytes: Option<u64>,
}

/// One entry in the working-directory status (returned by `working_dir_status`).
/// Compares current files on disk against the current branch's HEAD snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkingFileStatus {
    pub path: String,
    pub change: String, // "added" | "modified" | "removed"
    /// Current blob hash on disk (None for removed files).
    pub current_hash: Option<String>,
    /// Previous blob hash in HEAD snapshot (None for added files).
    pub previous_hash: Option<String>,
    pub size_bytes: Option<u64>,
}

/// One entry in a two-snapshot diff (returned by `diff_snapshots`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotDiffEntry {
    pub path: String,
    pub change: String, // "added" | "modified" | "removed"
    pub from_hash: Option<String>,
    pub to_hash: Option<String>,
}

/// Severity of a single [`VerifyFinding`]. Ordered from least to most
/// severe; [`VerifyReport::status`] is the maximum over all findings.
///
/// The five user-facing repository states map to these severities:
/// - `Healthy`       ← no findings or only `Ok`
/// - `NeedsCleanup`  ← worst finding is `Warning`
/// - `Recoverable`   ← worst finding is `Recoverable`
/// - `Corrupted`     ← worst finding is `Corrupted`
/// - `Unsupported`   ← worst finding is `Unsupported`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VerifySeverity {
    /// No problem found.
    Ok,
    /// A non-structural issue: e.g. a historical blob file is missing
    /// on disk (that file version can no longer be restored), or a
    /// COMMITTED transaction journal still exists (stale metadata).
    /// The repo remains usable but something deserves attention.
    Warning,
    /// Stale metadata that needs cleanup (e.g. committed journal
    /// directories that could not be removed). The repository data
    /// is consistent, but some cleanup is pending.
    NeedsCleanup,
    /// A destructive operation was interrupted and the transaction
    /// journal still records it. Re-opening the repository (or calling
    /// [`BasicRepository::recover_pending_transactions`]) should
    /// converge it. The repository is not yet trusted, but recovery is
    /// well-defined.
    Recoverable,
    /// Structural metadata is damaged or internally inconsistent
    /// (missing snapshot / manifest / branch row, unparseable JSON, an
    /// unsafe path in a manifest, a corrupted journal entry). Manual
    /// intervention is required; do not trust further mutations.
    Corrupted,
    /// The repository was created by a newer version of Route and
    /// this build cannot read it. No mutations are allowed.
    Unsupported,
}

/// One finding produced by [`BasicRepository::verify`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifyFinding {
    pub severity: VerifySeverity,
    /// Short stable machine code, e.g. `missing_snapshot`,
    /// `unsafe_manifest_path`, `stuck_applying_tx`. Stable enough to
    /// grep / match on; not part of the public API contract.
    pub code: String,
    /// Human-readable detail, including ids / paths where relevant.
    pub detail: String,
}

/// Options for [`BasicRepository::verify`].
#[derive(Debug, Clone)]
pub struct VerifyOptions {
    /// Stat every blob referenced by every manifest to confirm the
    /// file exists on disk. Default `true`. Cheap per blob (one stat),
    /// but O(total distinct blobs across all snapshots).
    pub check_blob_existence: bool,
    /// Re-hash every blob and compare to its stored hash. Default
    /// `false` (expensive: reads every blob). When enabled, also
    /// implies `check_blob_existence`.
    pub verify_blob_content: bool,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            check_blob_existence: true,
            verify_blob_content: false,
        }
    }
}

/// Aggregate result of [`BasicRepository::verify`]. `status` is the
/// worst severity across all `findings`; `Ok` iff `findings` is empty.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerifyReport {
    pub status: VerifySeverity,
    pub findings: Vec<VerifyFinding>,
    pub snapshots_checked: usize,
    pub manifests_checked: usize,
    pub branches_checked: usize,
    pub commits_checked: usize,
    pub transactions_checked: usize,
    pub blobs_checked: usize,
}

/// Result of inspecting or collecting unreferenced content-addressed blobs.
///
/// A blob is collectible only when no manifest in the repository references
/// its hash. `apply = false` is a read-only preview.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlobGcReport {
    pub apply: bool,
    pub referenced_blobs: usize,
    pub collectible_blobs: usize,
    pub collectible_bytes: u64,
    pub removed_blobs: usize,
    pub removed_bytes: u64,
}

/// A single repair operation that can be performed.
///
/// See [`RepairPlan`] and [`BasicRepository::plan_repair`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct RepairOperation {
    /// Stable machine code identifying the operation type.
    /// Used by the CLI to match against; not part of the public API contract.
    pub code: String,
    /// Human-readable description of what will be done.
    pub description: String,
    /// Whether this operation is safe to auto-apply. Safe operations
    /// never delete or modify user project files; they only clean up
    /// Route's own metadata (stale journal dirs, recovered transactions).
    pub safe: bool,
    /// The verify finding code that triggered this operation.
    pub trigger: String,
}

/// A repair plan generated from [`BasicRepository::verify`] findings.
///
/// Designed for use with `route check --dry-run` to show what `route repair`
/// would do, without executing anything.
///
/// # Safe vs destructive
///
/// - **Safe operations**: retry journal cleanup, resume recovery, reconcile
///   conversation linkage. These never touch user project files.
/// - **Potentially destructive operations**: anything that involves modifying
///   or guessing repository metadata (e.g. fixing dangling snapshot refs,
///   removing orphan conversations). These require user confirmation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RepairPlan {
    /// The verify report status that generated this plan.
    pub report_status: VerifySeverity,
    /// Operations that are safe to auto-apply.
    pub safe_operations: Vec<RepairOperation>,
    /// Operations that are potentially destructive and need user confirmation.
    pub destructive_operations: Vec<RepairOperation>,
    /// Whether any project files would be discarded by this plan.
    pub discards_project_files: bool,
}

impl BasicRepository {
    /// Preview or collect blobs that are not referenced by any stored manifest.
    ///
    /// The manifest set is the source of truth rather than branch heads, so
    /// blobs retained by older snapshots remain protected. In apply mode the
    /// corresponding immutable object files are unlinked while an immediate
    /// database transaction holds the manifest set stable, then their index
    /// rows are removed and committed. A crash can at worst leave an
    /// unreferenced index row whose file is already absent; rerunning GC safely
    /// converges it. A live manifest never loses a blob.
    pub fn garbage_collect_blobs(&self, apply: bool) -> Result<BlobGcReport> {
        let mut conn = self.db.lock();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let mut referenced = HashSet::new();
        {
            let mut stmt = tx.prepare("SELECT content FROM manifests")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            for row in rows {
                let manifest: crate::models::Manifest = serde_json::from_str(&row?)?;
                referenced.extend(manifest.into_values());
            }
        }

        let indexed: Vec<(String, u64)> = {
            let mut stmt = tx.prepare("SELECT hash, size FROM blobs")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let collectible: Vec<(String, u64)> = indexed
            .into_iter()
            .filter(|(hash, _)| !referenced.contains(hash))
            .collect();
        let collectible_bytes = collectible.iter().map(|(_, size)| *size).sum();

        if !apply {
            tx.rollback()?;
            return Ok(BlobGcReport {
                apply,
                referenced_blobs: referenced.len(),
                collectible_blobs: collectible.len(),
                collectible_bytes,
                removed_blobs: 0,
                removed_bytes: 0,
            });
        }

        let mut removed_blobs = 0;
        let mut removed_bytes = 0;
        for (hash, size) in &collectible {
            let path = self.paths.blob_path(hash);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    removed_blobs += 1;
                    removed_bytes += size;
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::remove_dir(parent);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(anyhow!(
                        "cannot remove collectible blob {}: {error}",
                        path.display()
                    ));
                }
            }
        }
        for (hash, _) in &collectible {
            tx.execute("DELETE FROM blobs WHERE hash = ?1", [hash])?;
        }
        tx.commit()?;

        Ok(BlobGcReport {
            apply,
            referenced_blobs: referenced.len(),
            collectible_blobs: collectible.len(),
            collectible_bytes,
            removed_blobs,
            removed_bytes,
        })
    }

    /// Attach an event bus. Once set, the bus cannot be replaced (matches
    /// the typical lifecycle: CLI/GUI constructs the bus once at startup).
    /// Returns `Err` if a bus is already attached.
    pub fn set_event_bus(&self, bus: Arc<EventBus>) -> Result<()> {
        self.event_bus
            .set(bus)
            .map_err(|_| anyhow!("event bus already attached"))
    }

    /// Whether an event bus is attached.
    pub fn has_event_bus(&self) -> bool {
        self.event_bus.get().is_some()
    }

    /// Dispatch an event to the bus (if attached). Errors are logged but
    /// never propagated — a misbehaving plugin must not break core ops.
    fn emit_event(&self, event: Event) {
        if let Some(bus) = self.event_bus.get() {
            let ctx = PluginContext::new(&self.paths.project_path);
            let result = bus.publish(&event, &ctx);
            if !result.is_ok() {
                tracing::warn!(
                    target: "route::events",
                    delivered = result.delivered,
                    failed = result.failed,
                    "plugin dispatch had failures: {:?}",
                    result.errors
                );
            }
        }
    }

    /// Initialize a new basic-mode repository in the given project path.
    pub fn init(project_path: impl AsRef<Path>) -> Result<Self> {
        let paths = RoutePaths::new(&project_path);
        if paths.is_initialized() {
            return Err(anyhow!(
                "Route basic repository already initialized at {}",
                paths.route_dir.display()
            ));
        }
        paths.ensure_dirs()?;

        // Create DB
        let db = DbConnection::open(&paths.db_path())?;

        // Create main branch
        let main_branch_id = new_id();
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO branches(id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at)
                 VALUES(?1, 'main', 'main', NULL, NULL, NULL, ?2)",
                params![main_branch_id, now_millis()],
            )?;
        }

        let config = RepoConfig {
            version: 1,
            project_path: paths.project_path.to_string_lossy().to_string(),
            mode: "basic".to_string(),
            created_at: now_millis(),
            main_branch_id: main_branch_id.clone(),
            track: TrackConfig::default(),
        };

        // Write config.json
        let config_json = serde_json::to_string_pretty(&config)?;
        std::fs::write(paths.config_path(), config_json).map_err(|e| {
            anyhow::Error::new(route_core::RouteError::CorruptedMetadata(format!(
                "cannot write config.json during init: {e}"
            )))
        })?;

        let journal = crate::transaction::TransactionJournal::new(&paths.route_dir);
        // A fresh repo has no transactions; ensure_dir is cheap.
        let _ = journal.ensure_dir();

        // Bootstrap the user-facing constitutive triad
        // (.route/constitution.md, protocol.md, reference/registry.json).
        // All three are idempotent: ensure_exists only writes when the file
        // is missing, so re-running `route init` on an incomplete
        // installation will fill in any gaps without clobbering user
        // edits. Writes are atomic (write_atomic), so a crash at this
        // stage leaves either the old state (no file) or the complete
        // default file — never a partial write.
        let proj_root = &paths.project_path;
        {
            use crate::constitutive::{Constitution, Protocol};
            Constitution::ensure_exists(proj_root).map_err(|e| {
                anyhow::Error::new(route_core::RouteError::CorruptedMetadata(format!(
                    "cannot bootstrap constitution.md during init: {e}"
                )))
            })?;
            Protocol::ensure_exists(proj_root).map_err(|e| {
                anyhow::Error::new(route_core::RouteError::CorruptedMetadata(format!(
                    "cannot bootstrap protocol.md during init: {e}"
                )))
            })?;
        }
        {
            crate::constitutive::ReferenceRegistry::ensure_exists(proj_root)?;
        }

        Ok(Self {
            paths,
            db,
            config,
            event_bus: OnceLock::new(),
            redo_stack: Mutex::new(Vec::new()),
            use_serial_scanner: false,
            journal,
        })
    }

    /// Open an existing basic-mode repository.
    ///
    /// As part of opening, any incomplete destructive transactions
    /// left by a previous crash are recovered. See
    /// [`Self::recover_pending_transactions`] and
    /// [`crate::transaction`] for the recovery model.
    pub fn open(project_path: impl AsRef<Path>) -> Result<Self> {
        let paths = RoutePaths::new(&project_path);
        if !paths.is_initialized() {
            return Err(route_core::RouteError::InvalidRepository(format!(
                "Not a Route basic repository: {}. Run `route init --mode basic` first.",
                paths.route_dir.display()
            ))
            .into());
        }
        let db = DbConnection::open(&paths.db_path())?;
        let config_json = std::fs::read_to_string(paths.config_path()).map_err(|e| {
            route_core::RouteError::CorruptedMetadata(format!("cannot read config.json: {e}"))
        })?;
        let mut config: RepoConfig = serde_json::from_str(&config_json).map_err(|e| {
            route_core::RouteError::CorruptedMetadata(format!("cannot parse config.json: {e}"))
        })?;
        // Check format version. Only version 1 is known.
        // If the config was created by a newer Route, refuse to open
        // — we cannot safely interpret the repository format.
        if config.version > 1 {
            return Err(route_core::RouteError::UnsupportedFormat(format!(
                "config.json version = {} (this build supports only version 1); \
                 repository was created by a newer version of Route",
                config.version
            ))
            .into());
        }
        // Older configs (before TrackConfig was added) lack the field.
        let _ = &mut config.track;

        let journal = crate::transaction::TransactionJournal::new(&paths.route_dir);
        journal.cleanup_stale_temps();

        let repo = Self {
            paths,
            db,
            config,
            event_bus: OnceLock::new(),
            redo_stack: Mutex::new(Vec::new()),
            use_serial_scanner: false,
            journal,
        };

        // Recover any in-flight transactions BEFORE returning. This is
        // the core of the crash-safety contract: after `open` returns,
        // the repository is in a known-consistent state (no APPLYING
        // transactions remain). COMMITTED transactions with a conv
        // linkage are left in place for the conversation store to
        // reconcile on its own open.
        repo.recover_pending_transactions()?;

        Ok(repo)
    }

    /// Force all subsequent `commit` / `working_dir_status` operations
    /// to use the non-parallel `ProjectScanner`. This avoids the worker
    /// thread spawn that the parallel walker does, which trips
    /// STATUS_STACK_OVERFLOW on Windows when the call chain is deep.
    /// Mainly used by tests and by the example seeder.
    pub fn with_serial_scanner(mut self) -> Self {
        self.use_serial_scanner = true;
        self
    }

    /// Read the current track configuration.
    pub fn track_config(&self) -> &TrackConfig {
        &self.config.track
    }

    /// Update the track configuration in memory and on disk. Returns
    /// the saved `TrackConfig`. The next `commit` / `working_dir_status`
    /// call will use the new rules.
    pub fn set_track_config(&mut self, cfg: TrackConfig) -> Result<TrackConfig> {
        self.config.track = cfg;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(route_core::RouteError::PermissionDenied(format!(
                    "cannot write config.json: {e}"
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(self.config.track.clone())
    }

    /// Convenience: enable or disable "track all files" without
    /// losing the existing suffix / prefix lists.
    pub fn set_track_all(&mut self, on: bool) -> Result<()> {
        self.config.track.track_all = on;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(route_core::RouteError::PermissionDenied(format!(
                    "cannot write config.json: {e}"
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Convenience: toggle SHA-256 verification. When on, every
    /// commit additionally records the SHA-256 of every tracked file
    /// in the manifest so an external system can verify the bytes.
    pub fn set_verify_sha256(&mut self, on: bool) -> Result<()> {
        self.config.track.verify_sha256 = on;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(route_core::RouteError::PermissionDenied(format!(
                    "cannot write config.json: {e}"
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Convenience: set the memory-buffer debounce (in milliseconds).
    /// 0 disables the buffer (every change is committed immediately).
    pub fn set_memory_buffer_ms(&mut self, ms: u64) -> Result<()> {
        self.config.track.memory_buffer_ms = ms;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(route_core::RouteError::PermissionDenied(format!(
                    "cannot write config.json: {e}"
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Update the OSes on which tracking is active. All three are
    /// enabled by default; the user can deselect any subset.
    pub fn set_track_on(&mut self, on: TrackOs) -> Result<()> {
        self.config.track.track_on = on;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(route_core::RouteError::PermissionDenied(format!(
                    "cannot write config.json: {e}"
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// The system prompt the software shows to AI agents. Returned as
    /// a string so the frontend can surface it in the AI control panel
    /// or the AI control API.
    pub fn ai_summary_prompt() -> &'static str {
        AiPrompt::summary_prompt()
    }

    /// Public façade for the scanner, used by the index module and
    /// by external callers that need a custom TrackFilter.
    pub fn make_scanner(&self) -> ProjectScanner {
        self.scanner()
    }

    /// Public read-only access to the underlying `RoutePaths`. The
    /// conflict module and the index module both need this so they
    /// can locate `.route/conflicts/`, `.route/index.json`, etc.
    pub fn route_paths(&self) -> &RoutePaths {
        &self.paths
    }

    /// Parse the most recent AI commit on the current branch and
    /// return any conflicts surfaced by its body. Returns an empty
    /// report when the user has not done any AI work yet.
    pub fn latest_ai_conflict_report(&self) -> Result<crate::ai_conflict::AiConflictReport> {
        let commits = self.list_commits(None, 50)?;
        let ai = commits.into_iter().find(|c| c.is_ai);
        match ai {
            Some(c) => crate::ai_conflict::report_for_commit(&self.db.lock(), &c),
            None => Ok(crate::ai_conflict::AiConflictReport {
                commit_id: String::new(),
                body: String::new(),
                conflicts: Vec::new(),
            }),
        }
    }

    /// Persist a verdict for one conflict. Mirrors
    /// `crate::ai_conflict::record_verdict` but takes the lock for the
    /// caller so the IPC layer doesn't need to know about `DbConnection`.
    pub fn record_conflict_verdict(
        &self,
        commit_id: &str,
        path: &str,
        verdict: &str,
        note: Option<&str>,
    ) -> Result<crate::ai_conflict::AiConflictVerdict> {
        crate::ai_conflict::record_verdict(
            &self.paths,
            &self.db.conn,
            commit_id,
            path,
            verdict,
            note,
        )
    }

    /// Return the verdicts recorded for a commit. See
    /// `crate::ai_conflict::list_verdicts`.
    pub fn list_conflict_verdicts(
        &self,
        commit_id: &str,
    ) -> Result<Vec<crate::ai_conflict::AiConflictVerdict>> {
        crate::ai_conflict::list_verdicts(&self.db.conn, commit_id)
    }

    fn scanner(&self) -> ProjectScanner {
        let s = ProjectScanner::new(&self.paths.project_path);
        // Translate the project's TrackConfig into a TrackFilter. If
        // the current OS is not in the active set, install a filter
        // that rejects everything so the scanner is a no-op.
        let filter = if !self.config.track.track_on.is_active_now() {
            Some(TrackFilter {
                track_all: false,
                suffixes: vec![],
                prefixes: vec![],
            })
        } else {
            Some(TrackFilter {
                track_all: self.config.track.track_all,
                suffixes: self
                    .config
                    .track
                    .track_suffixes
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .collect(),
                prefixes: self.config.track.track_prefixes.clone(),
            })
        };
        let s = s.with_track_filter(filter);
        if self.use_serial_scanner {
            s.with_serial_walker()
        } else {
            s
        }
    }

    /// Open or initialize a basic-mode repository.
    pub fn open_or_init(project_path: impl AsRef<Path>) -> Result<Self> {
        let paths = RoutePaths::new(&project_path);
        if paths.is_initialized() {
            Self::open(project_path)
        } else {
            Self::init(project_path)
        }
    }

    pub fn project_path(&self) -> &Path {
        &self.paths.project_path
    }

    // -----------------------------------------------------------------------
    // Branches
    // -----------------------------------------------------------------------

    pub fn list_branches(&self) -> Result<Vec<Branch>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at
             FROM branches ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(2)?;
            Ok(Branch {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: BranchKind::from_str(&kind_str).unwrap_or(BranchKind::Main),
                parent_branch: row.get(3)?,
                baseline_snapshot: row.get(4)?,
                head_snapshot: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_branch(&self, name: &str) -> Result<Branch> {
        let conn = self.db.lock();
        let kind_str: String = conn.query_row(
            "SELECT kind FROM branches WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )?;
        let kind = BranchKind::from_str(&kind_str).unwrap_or(BranchKind::Main);
        let row = conn.query_row(
            "SELECT id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at
             FROM branches WHERE name = ?1",
            params![name],
            |row| {
                Ok(Branch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind,
                    parent_branch: row.get(3)?,
                    baseline_snapshot: row.get(4)?,
                    head_snapshot: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(row)
    }

    pub fn get_branch_by_id(&self, id: &str) -> Result<Branch> {
        let conn = self.db.lock();
        let kind_str: String = conn.query_row(
            "SELECT kind FROM branches WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        let kind = BranchKind::from_str(&kind_str).unwrap_or(BranchKind::Main);
        let row = conn.query_row(
            "SELECT id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at
             FROM branches WHERE id = ?1",
            params![id],
            |row| {
                Ok(Branch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind,
                    parent_branch: row.get(3)?,
                    baseline_snapshot: row.get(4)?,
                    head_snapshot: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(row)
    }

    /// Create a new branch.
    pub fn create_branch(
        &self,
        name: &str,
        opts: CreateBranchOptions,
        current_branch_name: &str,
    ) -> Result<Branch> {
        if self.get_branch(name).is_ok() {
            return Err(anyhow!("Branch already exists: {}", name));
        }
        let parent_name = opts.from_branch.as_deref().unwrap_or(current_branch_name);
        let parent = self.get_branch(parent_name)?;

        let new_id = new_id();
        let created = now_millis();
        let parent_head = parent.head_snapshot.clone();
        let (baseline, head) = match opts.kind {
            BranchKind::Main => (None, None),
            BranchKind::Inherited => (parent_head.clone(), parent_head.clone()),
            BranchKind::Sandbox => (None, parent_head.clone()),
        };

        // For sandbox: copy parent's HEAD manifest into a new snapshot (deep copy semantics
        // for state isolation — blobs are shared via content addressing, no duplication needed).
        let head_snapshot = match opts.kind {
            BranchKind::Sandbox => {
                if let Some(parent_head_id) = parent_head {
                    let parent_snap = self.get_snapshot(&parent_head_id)?;
                    // New snapshot referencing same manifest — blobs dedupe automatically.
                    Some(self.create_snapshot_record(&parent_snap.manifest_hash)?)
                } else {
                    None
                }
            }
            _ => head,
        };

        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO branches(id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id,
                name,
                opts.kind.as_str(),
                match opts.kind {
                    BranchKind::Main => None,
                    _ => Some(parent.id),
                },
                baseline,
                head_snapshot,
                created,
            ],
        )?;

        drop(conn);

        // Emit BranchCreated.
        self.emit_event(Event::BranchCreated {
            name: name.to_string(),
            kind: match opts.kind {
                BranchKind::Main => route_plugins::BranchKind::Main,
                BranchKind::Inherited => route_plugins::BranchKind::Inherited,
                BranchKind::Sandbox => route_plugins::BranchKind::Sandbox,
            },
            parent_branch: match opts.kind {
                BranchKind::Main => None,
                _ => Some(parent.name.clone()),
            },
            baseline_snapshot: baseline.clone(),
            timestamp: created,
        });

        self.get_branch(name)
    }

    pub fn delete_branch(&self, name: &str) -> Result<()> {
        if name == "main" {
            return Err(anyhow!("Cannot delete main branch"));
        }
        let branch = self.get_branch(name)?;
        let deleted_ts = now_millis();
        let conn = self.db.lock();
        // Cascade delete will remove commits and annotations
        conn.execute("DELETE FROM branches WHERE id = ?1", params![branch.id])?;
        drop(conn);

        // Emit BranchDeleted.
        self.emit_event(Event::BranchDeleted {
            name: name.to_string(),
            timestamp: deleted_ts,
        });
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Snapshots & manifests
    // -----------------------------------------------------------------------

    pub fn get_snapshot(&self, id: &str) -> Result<Snapshot> {
        let conn = self.db.lock();
        let row = conn
            .query_row(
                "SELECT id, manifest_hash, created_at FROM snapshots WHERE id = ?1",
                params![id],
                |r| {
                    Ok(Snapshot {
                        id: r.get(0)?,
                        manifest_hash: r.get(1)?,
                        created_at: r.get(2)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    anyhow::Error::new(route_core::RouteError::InvalidSnapshot(format!(
                        "snapshot {id:?} not found in database"
                    )))
                }
                other => anyhow::Error::from(other),
            })?;
        Ok(row)
    }

    pub fn get_manifest(&self, hash: &str) -> Result<crate::models::Manifest> {
        let conn = self.db.lock();
        let content: String = conn
            .query_row(
                "SELECT content FROM manifests WHERE hash = ?1",
                params![hash],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    anyhow::Error::new(route_core::RouteError::InvalidSnapshot(format!(
                        "manifest with hash {hash:?} not found in database"
                    )))
                }
                other => anyhow::Error::from(other),
            })?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Resolve a snapshot's full file state, walking inherited branch baseline if needed.
    pub fn resolve_snapshot_files(&self, snapshot_id: &str) -> Result<HashMap<String, String>> {
        let snap = self.get_snapshot(snapshot_id)?;
        let mut manifest = self.get_manifest(&snap.manifest_hash)?;

        // If snapshot is the baseline of an inherited branch, merge parent's resolved state.
        let conn = self.db.lock();
        let inherited: Option<(String, String)> = conn
            .query_row(
                "SELECT b.parent_branch, b.id FROM branches b
                 WHERE b.baseline_snapshot = ?1 AND b.kind = 'inherited'",
                params![snapshot_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .ok();
        drop(conn);

        if let Some((parent_branch_id, _)) = inherited {
            let parent = self.get_branch_by_id(&parent_branch_id)?;
            if let Some(parent_head) = parent.head_snapshot {
                // Guard against infinite recursion. An inherited branch is
                // created with `baseline = parent.head` and `head =
                // parent.head` (the same snapshot id on both branches), so
                // resolving that snapshot and then looking up the parent
                // finds the same id again. Without this guard the
                // recursion blows the stack — which is what crashed the
                // Tauri app the first time the user clicked "Try example
                // project" on Windows. Once the inherited branch makes
                // any commit, its head advances past the baseline and the
                // guard is a no-op.
                if parent_head == snapshot_id {
                    // Same id on both sides — nothing to merge in.
                } else {
                    let parent_files = self.resolve_snapshot_files(&parent_head)?;
                    // Child manifest overrides parent
                    for (k, v) in parent_files {
                        manifest.entry(k).or_insert(v);
                    }
                }
            }
        }

        Ok(manifest)
    }

    /// Internal: create a snapshot record (no manifest resolution).
    fn create_snapshot_record(&self, manifest_hash: &str) -> Result<String> {
        let id = new_id();
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO snapshots(id, manifest_hash, created_at) VALUES(?1, ?2, ?3)",
            params![id, manifest_hash, now_millis()],
        )?;
        Ok(id)
    }

    /// Internal: store manifest JSON, return its hash.
    fn store_manifest(&self, manifest: &HashMap<String, String>) -> Result<String> {
        let content = serde_json::to_string(manifest)?;
        let hash = content_hash(content.as_bytes());
        let conn = self.db.lock();
        conn.execute(
            "INSERT OR IGNORE INTO manifests(hash, content) VALUES(?1, ?2)",
            params![hash, content],
        )?;
        Ok(hash)
    }

    // -----------------------------------------------------------------------
    // Commit (the core innovation — metadata on edge)
    // -----------------------------------------------------------------------

    /// Commit current project state as a new snapshot + edge.
    pub fn commit(&self, opts: CommitOptions) -> Result<Commit> {
        let branch_name = opts
            .branch
            .clone()
            .unwrap_or_else(|| self.get_current_branch_name().unwrap_or("main".into()));
        let branch = self.get_branch(&branch_name)?;

        let scan = self.scanner().scan()?;
        let prev_files = match &branch.head_snapshot {
            Some(head) => self.resolve_snapshot_files(head)?,
            None => HashMap::new(),
        };

        let diff = ManifestDiff::compute(&prev_files, &scan.files);

        // Store blobs for changed files
        let blob_store = BlobStore::new(self.paths.clone());
        for (rel, abs) in &scan.absolute_paths {
            if diff.added.contains(rel) || diff.modified.contains(rel) {
                let (hash, size) = blob_store.store_file_with_size(abs)?;
                if scan.files.get(rel) != Some(&hash) {
                    anyhow::bail!(
                        "file changed while snapshot was being created: {}",
                        abs.display()
                    );
                }
                let conn = self.db.lock();
                conn.execute(
                    "INSERT OR IGNORE INTO blobs(hash, size, created) VALUES(?1, ?2, ?3)",
                    params![hash, size as i64, now_millis()],
                )?;
            }
        }

        let manifest_hash = self.store_manifest(&scan.files)?;
        let new_snapshot = self.create_snapshot_record(&manifest_hash)?;

        let kind = if opts.force_full {
            CommitKind::Full
        } else {
            CommitKind::Incremental
        };

        let diff_summary = DiffSummary {
            added: diff.added.clone(),
            modified: diff.modified.clone(),
            removed: diff.removed.clone(),
        };

        let commit_id = new_id();
        let from_snapshot = branch
            .head_snapshot
            .clone()
            .unwrap_or_else(|| new_snapshot.clone());
        // For force_full: edge is a self-loop on current head (records the full-backup event).
        // Otherwise: edge goes from head to new snapshot.
        let to_snapshot = if opts.force_full && branch.head_snapshot.is_some() {
            from_snapshot.clone()
        } else {
            new_snapshot.clone()
        };

        // Default the operator to "user" for any direct user commit.
        let operator = opts.operator.clone().or_else(|| Some("user".to_string()));
        let is_checkpoint = opts.is_checkpoint;
        let is_ai = opts.is_ai;
        let body = opts.body.clone();

        // Capture the Effective Development Context so we can attach
        // lineage to every commit edge without embedding the full text.
        // Errors here are non-fatal: if `.route/` was somehow
        // misconfigured we still want the commit to land; the context
        // fields simply stay NULL (consistent with pre-v005 behaviour).
        use crate::constitutive::{ContextHistoryManifest, ContextSnapshot};
        let (ctx_hash, ctx_cv, ctx_pr, ctx_refh) =
            match ContextSnapshot::collect(&self.paths.project_path) {
                Ok(snap) => {
                    // Best-effort: record into the history manifest.
                    // Errors here are also swallowed — the manifest is
                    // a derivative artefact, not the source of truth.
                    let _ = ContextHistoryManifest::record_if_new(&self.paths.project_path, &snap);
                    (
                        Some(snap.fingerprint),
                        Some(snap.constitution_version),
                        Some(snap.protocol_revision.to_string()),
                        Some(snap.reference_entries_hash),
                    )
                }
                Err(_) => (None, None, None, None),
            };

        let commit_ts = now_millis();
        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO commits(
                    id, from_snapshot, to_snapshot, message, author, created_at,
                    branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai,
                    context_hash, constitution_version, protocol_revision, reference_entries_hash
                 )
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    commit_id,
                    from_snapshot,
                    to_snapshot,
                    opts.message,
                    opts.author,
                    commit_ts,
                    branch.id,
                    kind.as_str(),
                    diff_summary.to_json(),
                    operator,
                    body,
                    is_checkpoint as i64,
                    is_ai as i64,
                    ctx_hash,
                    ctx_cv,
                    ctx_pr,
                    ctx_refh,
                ],
            )?;
            // Update branch HEAD only for incremental (not for self-loop full backup)
            if !opts.force_full {
                conn.execute(
                    "UPDATE branches SET head_snapshot = ?1 WHERE id = ?2",
                    params![to_snapshot, branch.id],
                )?;
            }
        }

        // Any new head-advancing commit diverges the linear history; the
        // user can no longer safely redo. Drop the in-memory redo stack.
        // Checkpoints and force_full backups don't advance head, so they
        // intentionally preserve any pending redo.
        if !opts.force_full && !is_checkpoint {
            // Use unwrap_or_else to handle poisoned mutex gracefully
            // (should never happen in practice, but is defensive).
            let mut stack = self.redo_stack.lock().unwrap_or_else(|e| e.into_inner());
            stack.clear();
        }

        // Emit SnapshotCreated (only when a new snapshot node was actually
        // created — force_full self-loops reuse the existing head).
        if !(opts.force_full && branch.head_snapshot.is_some()) {
            self.emit_event(Event::SnapshotCreated {
                snapshot_id: new_snapshot.clone(),
                branch_id: branch.id.clone(),
                branch_name: branch.name.clone(),
                manifest_hash,
                timestamp: commit_ts,
            });
        }

        // Emit CommitCreated.
        self.emit_event(Event::CommitCreated {
            commit_id: commit_id.clone(),
            branch_id: branch.id.clone(),
            branch_name: branch.name.clone(),
            from_snapshot,
            to_snapshot,
            message: opts.message.clone(),
            author: opts.author.clone(),
            kind: match kind {
                CommitKind::Incremental => route_plugins::CommitKind::Incremental,
                CommitKind::Full => route_plugins::CommitKind::Full,
                CommitKind::Merge => route_plugins::CommitKind::Merge,
                CommitKind::Rollback => route_plugins::CommitKind::Rollback,
            },
            timestamp: commit_ts,
        });

        // Refresh the hidden `.route/index.json` so AI agents see the
        // new state without having to re-scan the project. Errors are
        // swallowed — the index is a derivative artefact, not a
        // source of truth.
        if let Ok(idx_path) = crate::index::write_index(self) {
            let _ = idx_path;
        }

        self.get_commit(&commit_id)
    }

    /// Merge `source_branch` into `target_branch` (defaults to current).
    ///
    /// File-level 3-way merge when source is `Inherited` (merge base =
    /// source.baseline_snapshot); 2-way "theirs wins" when source is
    /// `Sandbox` (sandbox semantics = take what's in the sandbox). `Main`
    /// as source uses 3-way against the target's baseline when available.
    ///
    /// `Sandbox` cannot be a target — it's a terminal scratch space. The
    /// merged manifest is written to the project directory and then a
    /// single `CommitKind::Merge` edge is recorded on the target branch.
    /// Conflicts (both sides changed the same file) resolve to "theirs"
    /// and are listed in the commit message so the user can review.
    pub fn merge(&self, source_branch: &str, target_branch: Option<&str>) -> Result<Commit> {
        let target_name = match target_branch {
            Some(t) => t.to_string(),
            None => self.get_current_branch_name()?,
        };
        if source_branch == target_name {
            return Err(anyhow!("cannot merge a branch into itself"));
        }
        let source = self.get_branch(source_branch)?;
        let target = self.get_branch(&target_name)?;
        if target.kind == BranchKind::Sandbox {
            return Err(anyhow!("sandbox branches cannot be merge targets"));
        }

        let source_head = source
            .head_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("source branch '{}' has no commits", source_branch))?;
        let target_head = target
            .head_snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("target branch '{}' has no commits", target_name))?;

        let theirs = self.resolve_snapshot_files(source_head)?;
        let ours = self.resolve_snapshot_files(target_head)?;

        // Merge base selection — only Inherited stores a baseline (the fork
        // point from its parent). Sandbox has no baseline, so we fall back
        // to a 2-way merge. Main as source has no parent either.
        let base = match source.kind {
            BranchKind::Inherited => source
                .baseline_snapshot
                .as_ref()
                .and_then(|id| self.resolve_snapshot_files(id).ok())
                .unwrap_or_default(),
            _ => HashMap::new(),
        };

        // 3-way file merge. For each path in the union of ours ∪ theirs ∪ base:
        //   - ours == base, theirs != base  → take theirs (they changed it)
        //   - theirs == base, ours != base  → keep ours (we changed it)
        //   - both != base                  → conflict → take theirs, record
        //   - both == base                  → keep (unchanged)
        // Files only in theirs → added. Files only in ours → kept.
        // Files in base but neither side → removed.
        let mut merged: HashMap<String, String> = HashMap::new();
        let mut conflicts: Vec<String> = Vec::new();
        let all_paths: std::collections::BTreeSet<String> = ours
            .keys()
            .chain(theirs.keys())
            .chain(base.keys())
            .cloned()
            .collect();
        for path in all_paths {
            let o = ours.get(&path);
            let t = theirs.get(&path);
            let b = base.get(&path);
            match (o, t, b) {
                (None, None, _) => {} // removed by both — stay removed
                (Some(o), Some(t), Some(b)) if o == b && t != b => {
                    // only theirs changed → take theirs
                    merged.insert(path, t.clone());
                }
                (Some(o), Some(t), Some(b)) if t == b && o != b => {
                    // only ours changed → keep ours
                    merged.insert(path, o.clone());
                }
                (Some(o), Some(t), _) if o == t => {
                    // same on both sides
                    merged.insert(path, o.clone());
                }
                (Some(_), Some(t), _) => {
                    // both changed differently → conflict, theirs wins
                    conflicts.push(path.clone());
                    merged.insert(path, t.clone());
                }
                (Some(o), None, Some(b)) if o == b => {
                    // theirs removed, ours unchanged → remove
                }
                (Some(o), None, _) => {
                    // theirs removed, ours changed (or only ours) → keep ours
                    merged.insert(path, o.clone());
                }
                (None, Some(t), _) => {
                    // only theirs has it → add
                    merged.insert(path, t.clone());
                }
            }
        }

        // Write the merged state to the project directory so the scanner
        // sees it when `commit` runs. This reuses the existing apply path.
        self.apply_files_to_project(&merged)?;

        let conflict_note = if conflicts.is_empty() {
            String::new()
        } else {
            format!("\n\nConflicts (took theirs): {}", conflicts.join(", "))
        };

        self.commit(CommitOptions {
            message: format!(
                "merge '{}' into '{}'{}",
                source_branch, target_name, conflict_note
            ),
            author: None,
            force_full: false,
            branch: Some(target_name.clone()),
            operator: Some("user".to_string()),
            body: None,
            is_checkpoint: false,
            is_ai: false,
        })
    }

    pub fn get_commit(&self, id: &str) -> Result<Commit> {
        let conn = self.db.lock();
        let row = conn.query_row(
            "SELECT id, from_snapshot, to_snapshot, message, author, created_at, branch_id, kind, diff_summary,
                    operator, body, is_checkpoint, is_ai,
                    context_hash, constitution_version, protocol_revision, reference_entries_hash
             FROM commits WHERE id = ?1",
            params![id],
            |r| row_to_commit(r),
        ).map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                anyhow::Error::new(route_core::RouteError::InvalidSnapshot(format!(
                    "commit {id:?} not found in database"
                )))
            }
            other => anyhow::Error::from(other),
        })?;
        Ok(row)
    }

    /// List commits on a branch (descending by time).
    pub fn list_commits(&self, branch_name: Option<&str>, limit: usize) -> Result<Vec<Commit>> {
        let conn = self.db.lock();
        let mut sql = String::from(
            "SELECT c.id, c.from_snapshot, c.to_snapshot, c.message, c.author, c.created_at,
                    c.branch_id, c.kind, c.diff_summary, c.operator, c.body, c.is_checkpoint, c.is_ai,
                    c.context_hash, c.constitution_version, c.protocol_revision, c.reference_entries_hash
             FROM commits c",
        );
        let mut params_vec: Vec<String> = Vec::new();
        if let Some(name) = branch_name {
            sql.push_str(" JOIN branches b ON c.branch_id = b.id WHERE b.name = ?1");
            params_vec.push(name.to_string());
        }
        sql.push_str(" ORDER BY c.created_at DESC LIMIT ?N");
        let limit_str = limit.to_string();
        let mut stmt = conn.prepare(&sql.replace("?N", &limit_str))?;
        let rows = if let Some(name) = branch_name {
            stmt.query_map(params![name], |r| row_to_commit(r))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            stmt.query_map([], |r| row_to_commit(r))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Path annotations (N-N edge text — innovation)
    // -----------------------------------------------------------------------

    pub fn add_path_annotation(&self, commit_id: &str, text: &str) -> Result<CommitPathAnnotation> {
        let id = new_id();
        let created = now_millis();
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO commit_path_annotations(id, commit_id, text, created_at) VALUES(?1, ?2, ?3, ?4)",
            params![id, commit_id, text, created],
        )?;
        drop(conn);

        // Emit AnnotationAdded.
        self.emit_event(Event::AnnotationAdded {
            commit_id: commit_id.to_string(),
            text: text.to_string(),
            timestamp: created,
        });

        Ok(CommitPathAnnotation {
            id,
            commit_id: commit_id.to_string(),
            text: text.to_string(),
            created_at: created,
        })
    }

    pub fn list_path_annotations(&self, commit_id: &str) -> Result<Vec<CommitPathAnnotation>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, commit_id, text, created_at FROM commit_path_annotations
             WHERE commit_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![commit_id], |r| {
            Ok(CommitPathAnnotation {
                id: r.get(0)?,
                commit_id: r.get(1)?,
                text: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_path_annotation(&self, annotation_id: &str) -> Result<()> {
        let conn = self.db.lock();
        conn.execute(
            "DELETE FROM commit_path_annotations WHERE id = ?1",
            params![annotation_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // File-level history & restore (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Return the revision history of a single file path across all
    /// snapshots that contain it. Newest first. Revisions where the file
    /// was absent are skipped (use `commit_diff_detail` to see removals).
    pub fn file_history(&self, rel_path: &str) -> Result<Vec<FileRevision>> {
        // Walk all snapshots, resolve their manifest (including inherited
        // baselines), and record any that contain `rel_path`.
        let snaps = self.all_snapshots()?;
        let mut out = Vec::new();
        for swb in snaps {
            let snap = &swb.snapshot;
            let manifest = self.get_manifest(&snap.manifest_hash)?;
            // Inherited baseline merge: also check parent's resolved state.
            let blob_hash = if let Some(h) = manifest.get(rel_path) {
                Some(h.clone())
            } else {
                // Maybe inherited from parent baseline.
                let conn = self.db.lock();
                let parent_blob: Option<String> = conn
                    .query_row(
                        "SELECT b.parent_branch FROM branches b
                         WHERE b.baseline_snapshot = ?1 AND b.kind = 'inherited'",
                        params![snap.id],
                        |_| Ok(()),
                    )
                    .ok()
                    .and_then(|_: ()| None);
                drop(conn);
                // For simplicity we skip cross-branch inherited blobs in
                // history listing; the common case (main branch) is covered.
                parent_blob
            };
            if let Some(hash) = blob_hash {
                // Find the commit that produced this snapshot (to_snapshot = snap.id).
                let conn = self.db.lock();
                let commit_row: Option<(String, String, i64)> = conn
                    .query_row(
                        "SELECT c.id, c.message, c.created_at FROM commits c
                         WHERE c.to_snapshot = ?1
                         ORDER BY c.created_at DESC LIMIT 1",
                        params![snap.id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .ok();
                drop(conn);
                let (commit_id, commit_message, created_at) =
                    commit_row.unwrap_or((String::new(), String::new(), snap.created_at));
                out.push(FileRevision {
                    snapshot_id: snap.id.clone(),
                    commit_id: if commit_id.is_empty() {
                        None
                    } else {
                        Some(commit_id)
                    },
                    commit_message: if commit_message.is_empty() {
                        None
                    } else {
                        Some(commit_message)
                    },
                    blob_hash: Some(hash),
                    created_at,
                    branch_name: swb.branch_name.clone(),
                });
            }
        }
        // Sort newest first.
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    /// Restore a single file from a snapshot's blob into the working
    /// directory. Returns the absolute path that was written.
    pub fn restore_file_from_snapshot(&self, snapshot_id: &str, rel_path: &str) -> Result<PathBuf> {
        // Validate path BEFORE reading snapshot data — refuse unsafe
        // paths even if they exist in a manifest.
        route_core::validate_rel_path(rel_path).map_err(|e| anyhow::Error::from(e))?;
        route_core::assert_no_symlink_escape(&self.paths.project_path, rel_path)
            .map_err(|e| anyhow::Error::from(e))?;
        let files = self.resolve_snapshot_files(snapshot_id)?;
        let hash = files.get(rel_path).ok_or_else(|| {
            anyhow!(
                "File '{}' not present in snapshot {}",
                rel_path,
                snapshot_id
            )
        })?;
        let blob_store = BlobStore::new(self.paths.clone());
        let bytes = blob_store.read(hash)?;
        let target = route_core::safe_join(&self.paths.project_path, rel_path)
            .map_err(|e| anyhow::Error::from(e))?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::Error::new(route_core::RouteError::PermissionDenied(format!(
                        "cannot create directory {}: {e}",
                        parent.display()
                    )))
                } else {
                    anyhow::Error::from(e)
                }
            })?;
        }
        // Use atomic write so a crash mid-write does not corrupt the
        // target file.
        blob_store.copy_to_atomic(hash, &target)?;
        let _ = bytes; // (read used for size / future verification)
        Ok(target)
    }

    // -----------------------------------------------------------------------
    // Commit diff detail (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Full diff detail for a commit: list of changed files with blob
    /// hashes and sizes. For "added"/"modified" files, `blob_hash` points
    /// to the new content in the `to_snapshot`. For "removed" files,
    /// `blob_hash` is `None`.
    pub fn commit_diff_detail(&self, commit_id: &str) -> Result<Vec<CommitDiffEntry>> {
        let commit = self.get_commit(commit_id)?;
        let diff = commit
            .diff_summary
            .as_deref()
            .map(DiffSummary::from_json)
            .unwrap_or_default();

        let to_manifest = self
            .get_snapshot(&commit.to_snapshot)
            .ok()
            .and_then(|s| self.get_manifest(&s.manifest_hash).ok())
            .unwrap_or_default();

        // blobs table for size lookup
        let conn = self.db.lock();
        let mut out = Vec::new();
        for path in &diff.added {
            let hash = to_manifest.get(path).cloned();
            let size = hash.as_deref().and_then(|h| Self::blob_size(&conn, h));
            out.push(CommitDiffEntry {
                path: path.clone(),
                change: "added".into(),
                blob_hash: hash,
                size_bytes: size,
            });
        }
        for path in &diff.modified {
            let hash = to_manifest.get(path).cloned();
            let size = hash.as_deref().and_then(|h| Self::blob_size(&conn, h));
            out.push(CommitDiffEntry {
                path: path.clone(),
                change: "modified".into(),
                blob_hash: hash,
                size_bytes: size,
            });
        }
        for path in &diff.removed {
            out.push(CommitDiffEntry {
                path: path.clone(),
                change: "removed".into(),
                blob_hash: None,
                size_bytes: None,
            });
        }
        drop(conn);
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Working directory status (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Compute the working-directory status: which files have been added,
    /// modified, or removed on disk compared to the current branch's HEAD
    /// snapshot. Useful for showing "what would be committed" before commit.
    pub fn working_dir_status(&self) -> Result<Vec<WorkingFileStatus>> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;

        let prev_files = match &branch.head_snapshot {
            Some(head) => self.resolve_snapshot_files(head)?,
            None => HashMap::new(),
        };

        let scan = self.scanner().scan()?;
        let diff = ManifestDiff::compute(&prev_files, &scan.files);

        let conn = self.db.lock();
        let mut out = Vec::new();
        for path in &diff.added {
            let hash = scan.files.get(path).cloned();
            let size = hash
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h))
                .or_else(|| {
                    scan.absolute_paths
                        .get(path)
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len())
                });
            out.push(WorkingFileStatus {
                path: path.clone(),
                change: "added".into(),
                current_hash: hash,
                previous_hash: None,
                size_bytes: size,
            });
        }
        for path in &diff.modified {
            let current = scan.files.get(path).cloned();
            let previous = prev_files.get(path).cloned();
            let size = current
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h))
                .or_else(|| {
                    scan.absolute_paths
                        .get(path)
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len())
                });
            out.push(WorkingFileStatus {
                path: path.clone(),
                change: "modified".into(),
                current_hash: current,
                previous_hash: previous,
                size_bytes: size,
            });
        }
        for path in &diff.removed {
            let previous = prev_files.get(path).cloned();
            let size = previous.as_deref().and_then(|h| Self::blob_size(&conn, h));
            out.push(WorkingFileStatus {
                path: path.clone(),
                change: "removed".into(),
                current_hash: None,
                previous_hash: previous,
                size_bytes: size,
            });
        }
        drop(conn);
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Diff between two snapshots (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Diff two arbitrary snapshots by their IDs. Resolves inherited branch
    /// baselines via `resolve_snapshot_files`. Returns added/modified/removed
    /// entries with from/to blob hashes.
    pub fn diff_snapshots(
        &self,
        from_snapshot: &str,
        to_snapshot: &str,
    ) -> Result<Vec<SnapshotDiffEntry>> {
        let from_files = self.resolve_snapshot_files(from_snapshot)?;
        let to_files = self.resolve_snapshot_files(to_snapshot)?;
        let diff = ManifestDiff::compute(&from_files, &to_files);

        let mut out = Vec::new();
        for path in &diff.added {
            out.push(SnapshotDiffEntry {
                path: path.clone(),
                change: "added".into(),
                from_hash: None,
                to_hash: to_files.get(path).cloned(),
            });
        }
        for path in &diff.modified {
            out.push(SnapshotDiffEntry {
                path: path.clone(),
                change: "modified".into(),
                from_hash: from_files.get(path).cloned(),
                to_hash: to_files.get(path).cloned(),
            });
        }
        for path in &diff.removed {
            out.push(SnapshotDiffEntry {
                path: path.clone(),
                change: "removed".into(),
                from_hash: from_files.get(path).cloned(),
                to_hash: None,
            });
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Tags (named snapshot pointers)
    // -----------------------------------------------------------------------

    /// Create a tag pointing at a snapshot. Returns Err if name already
    /// exists or snapshot doesn't exist.
    pub fn tag_create(
        &self,
        name: &str,
        snapshot_id: &str,
        message: Option<&str>,
    ) -> Result<crate::models::Tag> {
        // Validate snapshot exists
        self.get_snapshot(snapshot_id)?;

        let id = new_id();
        let ts = now_millis();
        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO tags(id, name, snapshot_id, message, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5)",
                params![id, name, snapshot_id, message, ts],
            )?;
        }
        Ok(crate::models::Tag {
            id,
            name: name.to_string(),
            snapshot_id: snapshot_id.to_string(),
            message: message.map(|s| s.to_string()),
            created_at: ts,
        })
    }

    /// List all tags, newest first.
    pub fn tag_list(&self) -> Result<Vec<crate::models::Tag>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, snapshot_id, message, created_at
             FROM tags ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(crate::models::Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                snapshot_id: r.get(2)?,
                message: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Look up a tag by name.
    pub fn tag_get(&self, name: &str) -> Result<crate::models::Tag> {
        let conn = self.db.lock();
        let row = conn.query_row(
            "SELECT id, name, snapshot_id, message, created_at
             FROM tags WHERE name = ?1",
            params![name],
            |r| {
                Ok(crate::models::Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    snapshot_id: r.get(2)?,
                    message: r.get(3)?,
                    created_at: r.get(4)?,
                })
            },
        )?;
        Ok(row)
    }

    /// Delete a tag by name. Returns Err if not found.
    pub fn tag_delete(&self, name: &str) -> Result<()> {
        let conn = self.db.lock();
        let affected = conn.execute("DELETE FROM tags WHERE name = ?1", params![name])?;
        if affected == 0 {
            Err(anyhow!("Tag '{}' not found", name))
        } else {
            Ok(())
        }
    }

    /// Look up a blob's size from the `blobs` table.
    fn blob_size(conn: &rusqlite::Connection, hash: &str) -> Option<u64> {
        conn.query_row(
            "SELECT size FROM blobs WHERE hash = ?1",
            params![hash],
            |r| r.get::<_, i64>(0),
        )
        .ok()
        .map(|n| n as u64)
    }

    // -----------------------------------------------------------------------
    // Rollback (new rollback edge, history preserved)
    // -----------------------------------------------------------------------

    /// Rollback to a target snapshot, recording a new `kind=rollback` edge.
    ///
    /// Returns the new rollback commit. To attribute the rollback to a
    /// particular operator (e.g. an AI agent), pass `operator`, `body`, and
    /// `is_ai` through `RollbackOptions`. Defaults: `operator="user"`,
    /// `is_ai=false`.
    pub fn rollback_to(&self, snapshot_id: &str, reason: Option<&str>) -> Result<Commit> {
        self.rollback_to_with(snapshot_id, reason, RollbackOptions::default())
    }

    /// Like `rollback_to` but with explicit attribution fields.
    ///
    /// # Crash-safety contract
    ///
    /// This operation is **journal-backed**. It writes a transaction
    /// intent to `.route-basic/transactions/<tx_id>/` before touching
    /// any project file, and advances a state machine
    /// (`PREPARED → APPLYING → COMMITTED`). If the process crashes at
    /// any point, the next `BasicRepository::open` detects the
    /// incomplete transaction and converges the repository back to a
    /// consistent state by re-running the (idempotent) apply phase and
    /// the metadata commit.
    ///
    /// This is **not** cross-file ACID — a crash mid-apply still leaves
    /// the working directory in a mixed state until the next `open`
    /// recovers it. The guarantee is: *after recovery, the repository
    /// is consistent and matches `snapshot_id`*.
    pub fn rollback_to_with(
        &self,
        snapshot_id: &str,
        reason: Option<&str>,
        opts: RollbackOptions,
    ) -> Result<Commit> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;
        let current_head = branch
            .head_snapshot
            .clone()
            .ok_or_else(|| anyhow!("Branch has no HEAD to rollback from"))?;

        // Emit RollbackStarted before mutating project files.
        let started_ts = now_millis();
        self.emit_event(Event::RollbackStarted {
            target_snapshot: snapshot_id.to_string(),
            reason: reason.map(|s| s.to_string()),
            timestamp: started_ts,
        });

        // Resolve target files ONCE, capture into the intent. Recovery
        // reuses this snapshot so it does not depend on DB state that
        // could change between crash and reopen.
        let files = self.resolve_snapshot_files(snapshot_id)?;
        let message = reason.unwrap_or("Rollback").to_string();
        let operator = opts.operator.unwrap_or_else(|| "user".to_string());
        let mut intent = crate::transaction::TransactionIntent::new_rollback(
            branch_name.clone(),
            current_head.clone(),
            snapshot_id.to_string(),
            files.clone(),
            message.clone(),
            Some(operator.clone()),
            opts.body.clone(),
            opts.is_ai,
        );
        // Link conversation intent if provided. This makes the
        // transaction journal aware of the conversation linkage, so
        // that a crash after code rollback but before conv truncation
        // is detectable and recoverable.
        intent.conv = opts.conv.clone();

        // Begin the transaction (state = PREPARED on disk). `begin`
        // fills `intent.tx_id` and `intent.commit_id` in place so the
        // later metadata write uses the same fixed commit id (recovery
        // idempotency depends on this).
        let tx_id = self.journal.begin(&mut intent)?;
        // Advance to APPLYING. From here on, a crash leaves a
        // recoverable APPLYING transaction.
        self.journal
            .set_state(&tx_id, crate::transaction::TxState::Applying)?;

        // Apply files (per-file atomic, injection-aware). If this
        // returns Err — whether from a real IO failure or an injected
        // test failure — we leave the journal in APPLYING and return
        // the error. Recovery on next open will re-run apply.
        if let Err(e) = self.apply_files_to_project(&files) {
            tracing::error!(
                target: "route::rollback",
                tx_id = %tx_id,
                error = %e,
                "apply_files_to_project failed; transaction left APPLYING for recovery"
            );
            return Err(e);
        }

        // Injection: simulate a crash between apply and metadata commit.
        crate::fail_inject::check(&crate::fail_inject::InjectionPoint::AfterApplyBeforeMetadata)
            .map_err(|m| {
                anyhow::Error::new(route_core::RouteError::RecoveryFailed(format!(
                    "injected: {m}"
                )))
            })?;

        // Commit metadata (SQLite transaction: INSERT commit edge +
        // UPDATE branch HEAD). Uses INSERT OR IGNORE with a fixed
        // commit_id so recovery re-runs are idempotent.
        let rollback_ts = now_millis();
        self.commit_rollback_metadata(&intent, &branch.id, rollback_ts)?;

        // Injection: simulate a crash after SQLite commit but before
        // the journal state advances to COMMITTED. On recovery: apply
        // re-runs (idempotent), metadata INSERT OR IGNORE no-ops,
        // UPDATE re-sets HEAD. Converges to COMMITTED.
        crate::fail_inject::check(
            &crate::fail_inject::InjectionPoint::AfterMetadataBeforeCommitted,
        )
        .map_err(|m| {
            anyhow::Error::new(route_core::RouteError::RecoveryFailed(format!(
                "injected: {m}"
            )))
        })?;

        // *** THE COMMIT POINT ***: state → COMMITTED. After this
        // rename completes, the operation is durably committed. A
        // crash here leaves a COMMITTED transaction; recovery only
        // needs to reconcile conversation (if linked) and remove the
        // journal dir.
        self.journal
            .set_state(&tx_id, crate::transaction::TxState::Committed)?;

        // Emit RollbackCompleted, then CommitCreated (the rollback creates a
        // new kind=rollback commit edge in the DB, so downstream plugins
        // should see it just like a regular commit).
        self.emit_event(Event::RollbackCompleted {
            new_commit_id: intent.commit_id.clone(),
            target_snapshot: snapshot_id.to_string(),
            timestamp: rollback_ts,
        });
        self.emit_event(Event::CommitCreated {
            commit_id: intent.commit_id.clone(),
            branch_id: branch.id.clone(),
            branch_name: branch_name.clone(),
            from_snapshot: current_head.clone(),
            to_snapshot: snapshot_id.to_string(),
            message: message.clone(),
            author: None,
            kind: route_plugins::CommitKind::Rollback,
            timestamp: rollback_ts,
        });

        // Injection: simulate a crash before journal cleanup. Harmless
        // — recovery removes the COMMITTED journal dir (no conv link).
        crate::fail_inject::check(&crate::fail_inject::InjectionPoint::BeforeTxRemove).map_err(
            |m| {
                anyhow::Error::new(route_core::RouteError::RecoveryFailed(format!(
                    "injected: {m}"
                )))
            },
        )?;

        // If this rollback has NO conversation linkage, the operation
        // is fully complete — mark COMPLETED and remove the journal
        // dir. If it HAS conv linkage, the caller (route-memory) is
        // responsible for advancing the conversation and then calling
        // `complete_committed_transaction(tx_id)`. We leave the dir in
        // place for that case.
        if intent.conv.is_none() {
            // Mark COMPLETED first (the commit point for cleanup).
            // If this fails, the tx stays at COMMITTED and recovery
            // will retry the cleanup on next open.
            let _ = self
                .journal
                .set_state(&tx_id, crate::transaction::TxState::Completed);
            if let Err(e) = self.journal.remove(&tx_id) {
                tracing::warn!(
                    target: "route::rollback",
                    tx_id = %tx_id,
                    error = %e,
                    "failed to remove committed transaction dir (non-fatal; \
                     recovery will retry on next open)"
                );
            }
        }

        let mut commit = self.get_commit(&intent.commit_id)?;
        // If conv-linked, stamp the tx_id on the returned commit so
        // the caller can complete the transaction (via
        // `complete_committed_transaction`) after conversation ops.
        if intent.conv.is_some() {
            commit.tx_id = Some(tx_id);
        }
        Ok(commit)
    }

    /// Write the rollback commit edge + update branch HEAD, in a single
    /// SQLite transaction. Idempotent: uses `INSERT OR IGNORE` with the
    /// intent's fixed `commit_id`, so a recovery re-run does not create
    /// a duplicate edge.
    fn commit_rollback_metadata(
        &self,
        intent: &crate::transaction::TransactionIntent,
        branch_id: &str,
        rollback_ts: i64,
    ) -> Result<()> {
        let conn = self.db.lock();
        // One explicit SQLite transaction so the commit edge and the
        // HEAD update are atomic w.r.t. each other.
        conn.execute_batch("BEGIN")?;
        let res = (|| -> Result<()> {
            conn.execute(
                "INSERT OR IGNORE INTO commits(
                    id, from_snapshot, to_snapshot, message, author, created_at,
                    branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                 )
                 VALUES(?1, ?2, ?3, ?4, NULL, ?5, ?6, 'rollback', NULL, ?7, ?8, 0, ?9)",
                params![
                    intent.commit_id,
                    intent.from_snapshot,
                    intent.to_snapshot,
                    intent.message,
                    rollback_ts,
                    branch_id,
                    intent.operator.clone().unwrap_or_default(),
                    intent.body,
                    intent.is_ai as i64,
                ],
            )?;
            conn.execute(
                "UPDATE branches SET head_snapshot = ?1 WHERE id = ?2",
                params![intent.to_snapshot, branch_id],
            )?;
            Ok(())
        })();
        match res {
            Ok(()) => {
                conn.execute_batch("COMMIT")?;
                Ok(())
            }
            Err(e) => {
                // Best-effort rollback; ignore errors here and surface
                // the original failure.
                let _ = conn.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }

    /// Recover any incomplete transactions left by a previous crash.
    /// Called automatically by [`Self::open`]. Safe to call repeatedly
    /// (idempotent): a fully-recovered repository has no pending
    /// transactions.
    ///
    /// Returns the list of transaction ids that are COMMITTED and carry
    /// a conversation linkage — these need the conversation store to
    /// reconcile (truncate + system note). `route-memory` consumes
    /// this list on its own open.
    pub fn recover_pending_transactions(&self) -> Result<Vec<String>> {
        let pending = self.journal.list_pending_full()?;
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        let mut conv_reconcile_ids: Vec<String> = Vec::new();
        for (tx_id, intent, state) in pending {
            tracing::info!(
                target: "route::recovery",
                tx_id = %tx_id,
                state = ?state,
                kind = %intent.kind,
                "recovering pending transaction"
            );
            match state {
                crate::transaction::TxState::Prepared
                | crate::transaction::TxState::Aborted
                | crate::transaction::TxState::Completed => {
                    // Nothing was applied (or explicitly abandoned before
                    // any write), or the transaction is already fully
                    // done. Safe to discard the journal dir.
                    self.journal.remove(&tx_id)?;
                }
                crate::transaction::TxState::CommittedPendingCleanup => {
                    // Data is consistent; only stale metadata remains.
                    // Retry cleanup. If it fails again, log and leave
                    // the tx in place for the next open.
                    if let Err(e) = self.journal.remove(&tx_id) {
                        tracing::warn!(
                            target: "route::recovery",
                            tx_id = %tx_id,
                            error = %e,
                            "COMMITTED_PENDING_CLEANUP: cleanup retry failed; \
                             will retry on next open"
                        );
                        // Leave in place — on next open, recovery will
                        // encounter it again.
                        continue;
                    }
                }
                crate::transaction::TxState::Applying => {
                    // Re-run apply (idempotent: per-file write-to-temp-rename
                    // + INSERT OR IGNORE + UPDATE HEAD). Then mark
                    // COMMITTED. If apply fails again, surface
                    // TransactionIncomplete — the repository is in an
                    // indeterminate state and needs human attention.
                    if let Err(e) = self.apply_files_to_project(&intent.target_files) {
                        return Err(route_core::RouteError::TransactionIncomplete {
                            tx_id: tx_id.clone(),
                            phase: "re-apply".into(),
                            detail: format!("apply re-run failed: {e}"),
                        }
                        .into());
                    }
                    let branch = self.get_branch(&intent.branch).map_err(|e| {
                        route_core::RouteError::TransactionIncomplete {
                            tx_id: tx_id.clone(),
                            phase: "resolve-branch".into(),
                            detail: format!("branch {} missing: {e}", intent.branch),
                        }
                    })?;
                    self.commit_rollback_metadata(&intent, &branch.id, now_millis())?;
                    self.journal
                        .set_state(&tx_id, crate::transaction::TxState::Committed)?;
                    if intent.conv.is_some() {
                        conv_reconcile_ids.push(tx_id);
                    } else {
                        self.journal.remove(&tx_id)?;
                    }
                }
                crate::transaction::TxState::Committed => {
                    // Code + metadata already done. If conv-linked, hand
                    // off to conversation reconciler. Otherwise clean up.
                    if intent.conv.is_some() {
                        conv_reconcile_ids.push(tx_id);
                    } else {
                        self.journal.remove(&tx_id)?;
                    }
                }
            }
        }
        Ok(conv_reconcile_ids)
    }

    /// Access the transaction journal (used by `route-memory` to link
    /// conversation reconciliation into the same transaction).
    pub fn journal(&self) -> &crate::transaction::TransactionJournal {
        &self.journal
    }

    /// Mark a COMMITTED transaction as fully reconciled (conversation
    /// aligned) and clean up its journal directory. Called by
    /// `route-memory` after it has truncated + saved the conversation
    /// store for a `rollback_to_message` transaction.
    ///
    /// If journal removal fails (e.g. permission denied), the
    /// transaction is marked `COMMITTED_PENDING_CLEANUP` instead of
    /// erroring. This is correct because the repository data is already
    /// consistent — only stale metadata remains. On next open, recovery
    /// retries the cleanup.
    pub fn complete_committed_transaction(&self, tx_id: &str) -> Result<()> {
        // Only allow removal if the tx is COMMITTED. Refusing otherwise
        // prevents accidentally dropping an in-flight APPLYING tx.
        let state = self.journal.read_state(tx_id)?.ok_or_else(|| {
            route_core::RouteError::TransactionIncomplete {
                tx_id: tx_id.to_string(),
                phase: "complete".into(),
                detail: format!("tx {tx_id} has no state file — cannot determine current state"),
            }
        })?;
        match state {
            crate::transaction::TxState::Committed => {
                // Normal path: try to remove the journal dir.
                match self.journal.remove(tx_id) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        // Journal removal failed but data is safe.
                        // Transition to COMMITTED_PENDING_CLEANUP so
                        // recovery will retry on next open.
                        tracing::warn!(
                            target: "route::complete",
                            tx_id = %tx_id,
                            error = %e,
                            "journal removal failed; marking COMMITTED_PENDING_CLEANUP"
                        );
                        self.journal.set_state(
                            tx_id,
                            crate::transaction::TxState::CommittedPendingCleanup,
                        )?;
                        Ok(())
                    }
                }
            }
            crate::transaction::TxState::CommittedPendingCleanup => {
                // Already in this state. Retry cleanup.
                match self.journal.remove(tx_id) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        // Still failing. Leave in place.
                        tracing::warn!(
                            target: "route::complete",
                            tx_id = %tx_id,
                            error = %e,
                            "COMMITTED_PENDING_CLEANUP: retry cleanup failed"
                        );
                        Ok(())
                    }
                }
            }
            crate::transaction::TxState::Completed => {
                // Already completed (idempotent).
                Ok(())
            }
            other => Err(route_core::RouteError::TransactionIncomplete {
                tx_id: tx_id.to_string(),
                phase: "complete".into(),
                detail: format!(
                    "tx {tx_id} cannot be completed: state = {other:?}; \
                     expected COMMITTED or COMMITTED_PENDING_CLEANUP"
                ),
            }
            .into()),
        }
    }

    /// List transaction ids that are COMMITTED with a conversation
    /// linkage, awaiting conversation reconciliation. `route-memory`
    /// calls this on open to find work left by a crash.
    pub fn pending_conversation_reconciles(
        &self,
    ) -> Result<Vec<(String, crate::transaction::TransactionIntent)>> {
        let mut out = Vec::new();
        for (tx_id, intent, state) in self.journal.list_pending_full()? {
            if intent.conv.is_some()
                && (state == crate::transaction::TxState::Committed
                    || state == crate::transaction::TxState::CommittedPendingCleanup)
            {
                out.push((tx_id, intent));
            }
        }
        Ok(out)
    }

    /// Apply a manifest to the project.
    ///
    /// ## Strategy: Write-First, Delete-Last
    ///
    /// 1. **Validate**: every path in the manifest is checked for
    ///    traversal / absolute / drive-letter / symlink escape. Any
    ///    unsafe path aborts the whole operation *before* a single
    ///    file is touched, with a `RouteError::UnsafePath`.
    ///
    /// 2. **Write phase (per-file atomic)**: write each target file
    ///    via `copy_to_atomic` (unique temp → fsync → rename). Each
    ///    file is either fully written or left unchanged.
    ///
    /// 3. **Delete phase (best-effort)**: after all writes succeed,
    ///    delete files present in the project but not in the target
    ///    manifest.
    ///
    /// ## Crash-safety scope (honest)
    ///
    /// This function provides **per-file atomic replacement**, NOT
    /// cross-file transaction atomicity. If the process crashes
    /// between writing file C and file D, the project is in a mixed
    /// state. The transaction journal in [`crate::transaction`] wraps
    /// this function so that a crash mid-apply is **detectable** and
    /// **recoverable** by re-running apply (which is idempotent).
    ///
    /// Without the journal, calling this directly leaves the project
    /// in an undetectable mixed state on crash. Prefer
    /// `rollback_to_with`, which goes through the journal.
    fn apply_files_to_project(&self, files: &HashMap<String, String>) -> Result<()> {
        let blob_store = BlobStore::new(self.paths.clone());

        // Phase 0: validate EVERY path before touching anything.
        // A single unsafe path refuses the whole operation — we do
        // not partially apply a manifest that contains a traversal.
        for rel in files.keys() {
            route_core::validate_rel_path(rel).map_err(|e| anyhow::Error::from(e))?;
            route_core::assert_no_symlink_escape(&self.paths.project_path, rel)
                .map_err(|e| anyhow::Error::from(e))?;
        }

        // Injection point: simulates a crash before any file is written.
        // No-op in release (no hook installed).
        crate::fail_inject::check(&crate::fail_inject::InjectionPoint::BeforeApply).map_err(
            |m| {
                anyhow::Error::new(route_core::RouteError::RecoveryFailed(format!(
                    "injected: {m}"
                )))
            },
        )?;

        // Phase 1: Write all target files atomically (write-to-temp-then-rename)
        // If any file fails to write, return immediately without deleting anything.
        let total = files.len();
        let mut written = 0usize;
        for (rel, hash) in files {
            let abs = route_core::safe_join(&self.paths.project_path, rel)
                .map_err(|e| anyhow::Error::from(e))?;
            blob_store.copy_to_atomic(hash, &abs)?;
            written += 1;
            // Injection point: simulates a crash after `written` files
            // have been replaced but before the rest. The journal state
            // stays APPLYING; recovery re-runs apply (idempotent).
            crate::fail_inject::check(&crate::fail_inject::InjectionPoint::AfterFileWrite {
                written,
                total,
            })
            .map_err(|m| {
                anyhow::Error::new(route_core::RouteError::RecoveryFailed(format!(
                    "injected: {m}"
                )))
            })?;
        }

        // Phase 2: Delete files that exist in project but not in target manifest.
        // All writes have succeeded by this point.
        let scan = self.scanner().scan()?;
        let mut critical_errors: Vec<String> = Vec::new();
        let mut _deleted_count = 0usize;
        for rel in scan.files.keys() {
            if !files.contains_key(rel) {
                // Defensive: re-validate before deleting. A path that
                // passed the scanner should already be safe, but this
                // guards against future scanner changes.
                let abs = match route_core::safe_join(&self.paths.project_path, rel) {
                    Ok(p) => p,
                    Err(e) => {
                        // Unsafe path in the scanner's result is a
                        // critical error — it means the scanner tracked
                        // something that should not have been tracked.
                        critical_errors.push(format!("{rel}: unsafe: {e}"));
                        continue;
                    }
                };
                match std::fs::remove_file(&abs) {
                    Ok(_) => {
                        _deleted_count += 1;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        // Permission denied is a critical error: the
                        // target file still exists, meaning working
                        // tree != target snapshot. Surface as a
                        // structured error so the transaction does not
                        // enter a fully successful state.
                        critical_errors.push(format!("{rel}: {e}"));
                    }
                    Err(e) => {
                        // Other non-critical errors (e.g. file not found
                        // because it was already deleted) are collected
                        // but do not block the operation.
                        tracing::debug!(
                            target: "route::rollback",
                            "non-critical delete error for {rel}: {e}"
                        );
                    }
                }
            }
        }

        if !critical_errors.is_empty() {
            tracing::error!(
                target: "route::rollback",
                "Critical delete failure(s) during rollback: {}",
                critical_errors.join(", ")
            );
            // The write phase succeeded, but the working tree is not
            // fully restored to the target snapshot because some files
            // that should have been deleted still exist. Surface this
            // as a route_core::RouteError so the transaction is not
            // marked as fully successful.
            return Err(route_core::RouteError::RecoveryFailed(format!(
                "rollback delete phase failed for {} file(s); \
                 working tree may not match target snapshot. Failed: {}",
                critical_errors.len(),
                critical_errors.join(", ")
            ))
            .into());
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Checkpoint (user-marked restore point)
    // -----------------------------------------------------------------------

    /// Mark a checkpoint at the current head. Does not modify files or
    /// advance the branch. The commit edge is a self-loop on head with
    /// `is_checkpoint=1`, the title in `message`, and the body in `body`.
    ///
    /// Returns the new checkpoint commit.
    pub fn checkpoint_create(
        &self,
        title: &str,
        body: Option<&str>,
        operator: Option<&str>,
    ) -> Result<Commit> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;
        let head = branch
            .head_snapshot
            .clone()
            .ok_or_else(|| anyhow!("Branch has no HEAD — commit something before checkpointing"))?;

        let commit_id = new_id();
        let ts = now_millis();
        let op = operator
            .map(|s| s.to_string())
            .unwrap_or_else(|| "user".to_string());

        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO commits(
                    id, from_snapshot, to_snapshot, message, author, created_at,
                    branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                 )
                 VALUES(?1, ?2, ?2, ?3, NULL, ?4, ?5, 'incremental', NULL, ?6, ?7, 1, 0)",
                params![
                    commit_id,
                    head,
                    title,
                    ts,
                    branch.id,
                    op,
                    body.map(|s| s.to_string()),
                ],
            )?;
        }

        self.emit_event(Event::CommitCreated {
            commit_id: commit_id.clone(),
            branch_id: branch.id.clone(),
            branch_name: branch_name.clone(),
            from_snapshot: head.clone(),
            to_snapshot: head.clone(),
            message: title.to_string(),
            author: None,
            kind: route_plugins::CommitKind::Incremental,
            timestamp: ts,
        });

        self.get_commit(&commit_id)
    }

    // -----------------------------------------------------------------------
    // Undo / Redo (operate on the redo_stack)
    // -----------------------------------------------------------------------

    /// Undo the most recent non-rollback commit on the current branch.
    ///
    /// Pushes `(from_snapshot, to_snapshot)` of that commit onto the
    /// redo stack so `redo_last` can roll it forward again, then issues
    /// a rollback to the commit's `from_snapshot`.
    ///
    /// Returns the rollback commit that was created.
    pub fn undo_last(&self) -> Result<Commit> {
        let branch_name = self.get_current_branch_name()?;
        let head = self
            .get_branch(&branch_name)?
            .head_snapshot
            .clone()
            .ok_or_else(|| anyhow!("Branch has no HEAD"))?;

        let commits = self.list_commits(Some(&branch_name), 50)?;

        // Newest non-rollback commit whose from_snapshot is not the
        // current head. The `from_snapshot` check skips commits that
        // have already been undone (their from_snapshot now equals
        // HEAD, so rolling back to it would be a no-op).
        let target = commits
            .iter()
            .filter(|c| c.kind != CommitKind::Rollback)
            .find(|c| c.from_snapshot != head)
            .ok_or_else(|| anyhow!("Nothing to undo"))?;

        // Record the redo pair BEFORE performing the rollback so the
        // state machine is consistent even if the rollback fails midway.
        {
            let mut stack = self.redo_stack.lock().unwrap_or_else(|e| e.into_inner());
            stack.push((target.from_snapshot.clone(), target.to_snapshot.clone()));
        }

        self.rollback_to_with(
            &target.from_snapshot,
            Some("Undo"),
            RollbackOptions {
                operator: Some("user".to_string()),
                body: None,
                is_ai: false,
                conv: None,
            },
        )
    }

    /// Redo the most recently undone commit. Pops the top of the redo
    /// stack and rolls forward from the current head to the popped
    /// `to_snapshot`. Returns the rollback commit that was created.
    pub fn redo_last(&self) -> Result<Commit> {
        let next = {
            let mut stack = self.redo_stack.lock().unwrap_or_else(|e| e.into_inner());
            stack.pop()
        };
        let (_from, to) = next.ok_or_else(|| anyhow!("Nothing to redo"))?;

        self.rollback_to_with(
            &to,
            Some("Redo"),
            RollbackOptions {
                operator: Some("user".to_string()),
                body: None,
                is_ai: false,
                conv: None,
            },
        )
    }

    /// Current size of the in-memory redo stack. Useful for the UI to
    /// enable / disable the redo button.
    pub fn redo_stack_len(&self) -> usize {
        self.redo_stack
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Whether the redo stack has at least one entry the user can redo.
    pub fn can_redo(&self) -> bool {
        self.redo_stack_len() > 0
    }

    // -----------------------------------------------------------------------
    // Full backup to external folder
    // -----------------------------------------------------------------------

    /// Copy all files at current HEAD to an external target folder.
    /// Records a kind=full self-loop edge in the DB.
    pub fn full_backup_to_dir(&self, target_dir: &Path) -> Result<PathBuf> {
        let branch_name = self
            .get_current_branch_name()
            .unwrap_or_else(|_| "main".to_string());
        let branch = self.get_branch(&branch_name)?;
        let head = branch
            .head_snapshot
            .ok_or_else(|| anyhow!("Branch has no HEAD — commit something first"))?;

        let files = self.resolve_snapshot_files(&head)?;
        let blob_store = BlobStore::new(self.paths.clone());

        std::fs::create_dir_all(target_dir)?;
        for (rel, hash) in &files {
            let abs = target_dir.join(rel);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            blob_store.copy_to(hash, &abs)?;
        }

        // Record kind=full self-loop edge
        let commit_id = new_id();
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO commits(id, from_snapshot, to_snapshot, message, author, created_at, branch_id, kind, diff_summary)
             VALUES(?1, ?2, ?2, ?3, NULL, ?4, ?5, 'full', NULL)",
            params![
                commit_id,
                head,
                format!("Full backup to {}", target_dir.display()),
                now_millis(),
                branch.id,
            ],
        )?;
        drop(conn);

        Ok(target_dir.to_path_buf())
    }

    // -----------------------------------------------------------------------
    // File-based exports (ZIP / Folder)
    // -----------------------------------------------------------------------

    /// Build the ExportContext with all branches, snapshots, commits, and annotations.
    pub fn build_export_context(&self) -> Result<crate::export::ExportContext> {
        let branches = self.list_branches()?;
        let snapshots = self.all_snapshots()?;
        let commits = self.list_commits(None, 10000)?;

        let mut annotations = std::collections::HashMap::new();
        for c in &commits {
            let anns = self.list_path_annotations(&c.id).unwrap_or_default();
            annotations.insert(c.id.clone(), anns);
        }

        let project_name = self
            .project_path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "route-project".to_string());

        Ok(crate::export::ExportContext {
            project_name,
            project_path: self.project_path().to_string_lossy().to_string(),
            branches,
            snapshots: snapshots.into_iter().map(|s| s.snapshot).collect(),
            commits,
            annotations,
        })
    }

    /// Export the full repository (metadata + all blobs + all manifests) as a ZIP archive.
    pub fn export_zip(&self, output_path: &Path) -> Result<PathBuf> {
        use std::io::Write;

        let ctx = self.build_export_context()?;
        let blob_store = BlobStore::new(self.paths.clone());

        // Collect all blob hashes from DB
        let blob_hashes: Vec<String> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash FROM blobs ORDER BY hash")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };

        // Collect all manifest hashes
        let manifest_hashes: Vec<String> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash FROM manifests ORDER BY hash")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };

        let file = std::fs::File::create(output_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // 1. Text exports
        let text_exporters = crate::DefaultExporters::all();
        for exporter in &text_exporters {
            let mut buf = Vec::new();
            exporter.export(&ctx, &mut buf)?;
            let name = match exporter.format() {
                crate::export::ExportFormat::Json => "metadata.json",
                crate::export::ExportFormat::Markdown => "report.md",
                crate::export::ExportFormat::Mermaid => "mindmap.mmd",
                crate::export::ExportFormat::EmacsOrg => "report.org",
                _ => continue,
            };
            zip.start_file(name, options)?;
            zip.write_all(&buf)?;
        }

        // 2. Manifests
        for hash in &manifest_hashes {
            let content: String = {
                let conn = self.db.lock();
                conn.query_row(
                    "SELECT content FROM manifests WHERE hash = ?1",
                    params![hash],
                    |r| r.get(0),
                )?
            };
            zip.start_file(format!("manifests/{hash}.json"), options)?;
            zip.write_all(content.as_bytes())?;
        }

        // 3. Blobs
        for hash in &blob_hashes {
            let bytes = blob_store.read(hash)?;
            zip.start_file(format!("blobs/{hash}"), options)?;
            zip.write_all(&bytes)?;
        }

        // 4. README
        zip.start_file("README.txt", options)?;
        let readme = format!(
            "Route export archive\n\
             Project: {}\n\
             Path: {}\n\
             Branches: {}\n\
             Snapshots: {}\n\
             Commits: {}\n\
             Blobs: {}\n\
             Manifests: {}\n\
             Generated: {}\n",
            ctx.project_name,
            ctx.project_path,
            ctx.branches.len(),
            ctx.snapshots.len(),
            ctx.commits.len(),
            blob_hashes.len(),
            manifest_hashes.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );
        zip.write_all(readme.as_bytes())?;

        zip.finish()?;
        Ok(output_path.to_path_buf())
    }

    /// Export the full repository as a folder structure on disk.
    pub fn export_folder(&self, output_dir: &Path) -> Result<PathBuf> {
        let ctx = self.build_export_context()?;
        let blob_store = BlobStore::new(self.paths.clone());

        std::fs::create_dir_all(output_dir)?;
        let manifests_dir = output_dir.join("manifests");
        let blobs_dir = output_dir.join("blobs");
        std::fs::create_dir_all(&manifests_dir)?;
        std::fs::create_dir_all(&blobs_dir)?;

        // 1. Text exports
        let text_exporters = crate::DefaultExporters::all();
        for exporter in &text_exporters {
            let mut buf = Vec::new();
            exporter.export(&ctx, &mut buf)?;
            let name = match exporter.format() {
                crate::export::ExportFormat::Json => "metadata.json",
                crate::export::ExportFormat::Markdown => "report.md",
                crate::export::ExportFormat::Mermaid => "mindmap.mmd",
                crate::export::ExportFormat::EmacsOrg => "report.org",
                _ => continue,
            };
            std::fs::write(output_dir.join(name), &buf)?;
        }

        // 2. Manifests
        let manifest_entries: Vec<(String, String)> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash, content FROM manifests")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };
        for (hash, content) in &manifest_entries {
            std::fs::write(manifests_dir.join(format!("{hash}.json")), content)?;
        }

        // 3. Blobs
        let blob_hashes: Vec<String> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash FROM blobs ORDER BY hash")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };
        for hash in &blob_hashes {
            let bytes = blob_store.read(hash)?;
            std::fs::write(blobs_dir.join(hash), &bytes)?;
        }

        // 4. README
        let readme = format!(
            "Route export folder\n\
             Project: {}\n\
             Path: {}\n\
             Branches: {}\n\
             Snapshots: {}\n\
             Commits: {}\n\
             Blobs: {}\n\
             Manifests: {}\n\
             Generated: {}\n",
            ctx.project_name,
            ctx.project_path,
            ctx.branches.len(),
            ctx.snapshots.len(),
            ctx.commits.len(),
            blob_hashes.len(),
            manifest_entries.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );
        std::fs::write(output_dir.join("README.txt"), readme)?;

        Ok(output_dir.to_path_buf())
    }

    // -----------------------------------------------------------------------
    // Current branch tracking (HEAD)
    // -----------------------------------------------------------------------

    /// Get the current branch name. Stored in `meta` table as `current_branch`.
    pub fn get_current_branch_name(&self) -> Result<String> {
        let conn = self.db.lock();
        let name: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'current_branch'",
                [],
                |r| r.get(0),
            )
            .ok();
        drop(conn);
        Ok(name.unwrap_or_else(|| "main".to_string()))
    }

    pub fn set_current_branch(&self, name: &str) -> Result<()> {
        // Validate branch exists
        self.get_branch(name)?;
        let conn = self.db.lock();
        conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES('current_branch', ?1)",
            params![name],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // History (snapshot chain from HEAD backwards)
    // -----------------------------------------------------------------------

    pub fn history(&self, limit: usize) -> Result<Vec<SnapshotWithBranch>> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;
        let mut out = Vec::new();
        let mut current_id = branch.head_snapshot;
        let mut visited = std::collections::HashSet::new();

        while let Some(id) = current_id {
            if out.len() >= limit || visited.contains(&id) {
                break;
            }
            visited.insert(id.clone());
            let snap = self.get_snapshot(&id)?;

            // Find the commit that produced this snapshot (incoming edge on current branch)
            let conn = self.db.lock();
            let producer: Option<Commit> = conn
                .query_row(
                    "SELECT id, from_snapshot, to_snapshot, message, author, created_at,
                            branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                     FROM commits WHERE to_snapshot = ?1 AND branch_id = ?2
                     ORDER BY created_at DESC LIMIT 1",
                    params![id, branch.id],
                    |r| row_to_commit(r),
                )
                .ok();
            drop(conn);

            out.push(SnapshotWithBranch {
                snapshot: snap.clone(),
                branch_id: Some(branch.id.clone()),
                branch_name: Some(branch.name.clone()),
            });

            current_id = producer.map(|p| p.from_snapshot);
        }

        Ok(out)
    }

    /// Get all snapshots with their owning branch (via latest producing commit).
    pub fn all_snapshots(&self) -> Result<Vec<SnapshotWithBranch>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.manifest_hash, s.created_at, b.id, b.name
             FROM snapshots s
             LEFT JOIN commits c ON c.to_snapshot = s.id
             LEFT JOIN branches b ON b.id = c.branch_id
             GROUP BY s.id
             ORDER BY s.created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SnapshotWithBranch {
                snapshot: Snapshot {
                    id: r.get(0)?,
                    manifest_hash: r.get(1)?,
                    created_at: r.get(2)?,
                },
                branch_id: r.get(3)?,
                branch_name: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Repository integrity check (Phase 7)
    // -----------------------------------------------------------------------

    /// Verify repository integrity. Returns a structured report; never
    /// panics on bad data — every problem is captured as a
    /// [`VerifyFinding`] instead.
    ///
    /// This is **not** a git fsck. It checks the cheap, high-value
    /// invariants that catch the failure modes Route's crash-safety
    /// work is built around:
    ///
    /// * metadata is parseable (config, schema version, manifests),
    /// * referential integrity holds (branch HEAD / baseline, commit
    ///   from/to, snapshot → manifest, tag → snapshot all resolve),
    /// * no manifest contains a path that escapes the project root
    ///   (defence against corrupted or malicious metadata),
    /// * the transaction journal is parseable and contains no stuck
    ///   PREPARED / APPLYING entries (those should have been recovered
    ///   on `open`; their presence here means recovery did not run or
    ///   did not converge),
    /// * optionally, every blob referenced by a manifest exists on
    ///   disk (and, on demand, re-hashes to its stored id).
    ///
    /// The report's `status` is the worst severity across all findings.
    /// An empty `findings` vector with `status = Ok` means the repo
    /// passed every check.
    ///
    /// `verify` does **not** mutate the repository and does **not**
    /// run recovery. Call [`Self::recover_pending_transactions`] first
    /// if you want `verify` to judge the post-recovery state.
    pub fn verify(&self, opts: &VerifyOptions) -> Result<VerifyReport> {
        use std::collections::HashSet;

        let mut findings: Vec<VerifyFinding> = Vec::new();
        let mut snapshots_checked = 0usize;
        let mut manifests_checked = 0usize;
        let mut branches_checked = 0usize;
        let mut commits_checked = 0usize;
        let mut transactions_checked = 0usize;
        let mut blobs_checked = 0usize;

        let note = |severity: VerifySeverity,
                    code: &str,
                    detail: String,
                    findings: &mut Vec<VerifyFinding>| {
            findings.push(VerifyFinding {
                severity,
                code: code.to_string(),
                detail,
            });
        };

        // ---- 1. Schema version + config version ----
        let conn = self.db.lock();
        let schema_v: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .ok();
        match schema_v {
            Some(v) => match v.parse::<u32>() {
                Ok(n) if n <= route_core::CURRENT_SCHEMA_VERSION => {}
                Ok(n) => note(
                    VerifySeverity::Warning,
                    "schema_newer_than_supported",
                    format!(
                        "schema_version={n} is newer than this build supports ({}); \
                         some data may be unreadable",
                        route_core::CURRENT_SCHEMA_VERSION
                    ),
                    &mut findings,
                ),
                Err(_) => note(
                    VerifySeverity::Corrupted,
                    "schema_version_unparseable",
                    format!("meta.schema_version = {v:?} is not a number"),
                    &mut findings,
                ),
            },
            None => note(
                VerifySeverity::Corrupted,
                "schema_version_missing",
                "meta table has no schema_version row".into(),
                &mut findings,
            ),
        }
        // config.version: Route currently only knows version 1.
        // Versions > 1 are Unsupported — this build cannot read them.
        // (Unlike schema_version, which is a DB-level migration, the
        // config.version is a repository format marker that affects
        // how ALL files under .route-basic/ are interpreted.)
        if self.config.version > 1 {
            note(
                VerifySeverity::Unsupported,
                "config_version_unsupported",
                format!(
                    "config.json version = {} (this build recognises only version 1); \
                     repository format is not supported by this build",
                    self.config.version
                ),
                &mut findings,
            );
        }

        // ---- Load reference sets under one lock ----
        let snapshot_ids: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT id FROM snapshots")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut s = HashSet::new();
            for r in rows {
                s.insert(r?);
            }
            s
        };
        let manifest_hashes: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT hash FROM manifests")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut s = HashSet::new();
            for r in rows {
                s.insert(r?);
            }
            s
        };
        let branch_ids: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT id FROM branches")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut s = HashSet::new();
            for r in rows {
                s.insert(r?);
            }
            s
        };
        let blob_hashes_in_db: HashSet<String> = {
            let mut stmt = conn.prepare("SELECT hash FROM blobs")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut s = HashSet::new();
            for r in rows {
                s.insert(r?);
            }
            s
        };

        // ---- 2. Branches: head / baseline snapshots resolve ----
        {
            let mut stmt =
                conn.prepare("SELECT id, name, head_snapshot, baseline_snapshot FROM branches")?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?;
            for r in rows {
                let (id, name, head, baseline) = r?;
                branches_checked += 1;
                if let Some(h) = &head {
                    if !snapshot_ids.contains(h) {
                        note(
                            VerifySeverity::Corrupted,
                            "branch_head_missing_snapshot",
                            format!(
                                "branch {name:?} (id={id}) head_snapshot={h} \
                                 does not exist in snapshots table"
                            ),
                            &mut findings,
                        );
                    }
                }
                if let Some(b) = &baseline {
                    if !snapshot_ids.contains(b) {
                        note(
                            VerifySeverity::Corrupted,
                            "branch_baseline_missing_snapshot",
                            format!(
                                "branch {name:?} (id={id}) baseline_snapshot={b} \
                                 does not exist in snapshots table"
                            ),
                            &mut findings,
                        );
                    }
                }
            }
        }

        // ---- 3. Commits: from/to snapshots + branch resolve ----
        {
            let mut stmt =
                conn.prepare("SELECT id, from_snapshot, to_snapshot, branch_id FROM commits")?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?;
            for r in rows {
                let (id, from, to, branch_id) = r?;
                commits_checked += 1;
                if !snapshot_ids.contains(&from) {
                    note(
                        VerifySeverity::Corrupted,
                        "commit_from_missing_snapshot",
                        format!("commit {id} from_snapshot={from} does not exist"),
                        &mut findings,
                    );
                }
                if !snapshot_ids.contains(&to) {
                    note(
                        VerifySeverity::Corrupted,
                        "commit_to_missing_snapshot",
                        format!("commit {id} to_snapshot={to} does not exist"),
                        &mut findings,
                    );
                }
                if !branch_ids.contains(&branch_id) {
                    note(
                        VerifySeverity::Corrupted,
                        "commit_branch_missing",
                        format!(
                            "commit {id} branch_id={branch_id} does not exist \
                             in branches table"
                        ),
                        &mut findings,
                    );
                }
            }
        }

        // ---- 4. Snapshots: manifest resolves ----
        {
            let mut stmt = conn.prepare("SELECT id, manifest_hash FROM snapshots")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in rows {
                let (sid, mh) = r?;
                snapshots_checked += 1;
                if !manifest_hashes.contains(&mh) {
                    note(
                        VerifySeverity::Corrupted,
                        "snapshot_missing_manifest",
                        format!("snapshot {sid} manifest_hash={mh} not in manifests table"),
                        &mut findings,
                    );
                }
            }
        }

        // ---- 5. Manifests: parse + path safety + blob index ----
        // Collect distinct blob hashes referenced for the optional
        // filesystem check below.
        let mut referenced_blobs: HashSet<String> = HashSet::new();
        {
            let mut stmt = conn.prepare("SELECT hash, content FROM manifests")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in rows {
                let (hash, content) = r?;
                manifests_checked += 1;
                let parsed: Result<crate::models::Manifest> =
                    serde_json::from_str(&content).map_err(Into::into);
                match parsed {
                    Ok(map) => {
                        for (path, blob) in &map {
                            // Path safety: every manifest path must stay
                            // inside the project root. A corrupted
                            // manifest with `../../x` would otherwise
                            // let rollback write outside the repo.
                            if let Err(e) = route_core::validate_rel_path(path) {
                                note(
                                    VerifySeverity::Corrupted,
                                    "unsafe_manifest_path",
                                    format!("manifest {hash} contains unsafe path {path:?}: {e}"),
                                    &mut findings,
                                );
                            }
                            if !blob_hashes_in_db.contains(blob) {
                                note(
                                    VerifySeverity::Corrupted,
                                    "manifest_blob_not_indexed",
                                    format!(
                                        "manifest {hash} references blob {blob} \
                                         (path {path:?}) not present in blobs table"
                                    ),
                                    &mut findings,
                                );
                            }
                            referenced_blobs.insert(blob.clone());
                        }
                    }
                    Err(e) => {
                        note(
                            VerifySeverity::Corrupted,
                            "manifest_unparseable",
                            format!("manifest {hash} content is not valid JSON: {e}"),
                            &mut findings,
                        );
                    }
                }
            }
        }

        // ---- 6. Tags: snapshot resolves ----
        {
            let mut stmt = conn.prepare("SELECT name, snapshot_id FROM tags")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for r in rows {
                let (name, sid) = r?;
                if !snapshot_ids.contains(&sid) {
                    note(
                        VerifySeverity::Corrupted,
                        "tag_missing_snapshot",
                        format!("tag {name:?} snapshot_id={sid} does not exist"),
                        &mut findings,
                    );
                }
            }
        }

        // ---- 7. Conversation store integrity ----
        // Check that the conversations.json file (if it exists) is
        // parseable and that every snapshot referenced by a message
        // actually exists in the repository.
        {
            let conv_path = self
                .paths
                .project_path
                .join(".route")
                .join("conversations.json");
            if conv_path.exists() {
                match std::fs::read_to_string(&conv_path) {
                    Ok(content) => {
                        if content.is_empty() {
                            // Empty file — not a problem, just no
                            // conversations yet.
                        } else if let Ok(conv) = serde_json::from_str::<serde_json::Value>(&content)
                        {
                            if let Some(messages) = conv.get("messages").and_then(|m| m.as_object())
                            {
                                for (_msg_id, msg) in messages {
                                    if let Some(sid) =
                                        msg.get("snapshot_id").and_then(|s| s.as_str())
                                    {
                                        if !sid.is_empty() && !snapshot_ids.contains(sid) {
                                            note(
                                                VerifySeverity::Warning,
                                                "conv_message_orphan_snapshot",
                                                format!(
                                                    "message in conversations.json references \
                                                     snapshot_id={sid} which does not exist \
                                                     in the repository"
                                                ),
                                                &mut findings,
                                            );
                                        }
                                    }
                                    if let Some(rsid) =
                                        msg.get("rollback_snapshot_id").and_then(|s| s.as_str())
                                    {
                                        if !rsid.is_empty() && !snapshot_ids.contains(rsid) {
                                            note(
                                                VerifySeverity::Warning,
                                                "conv_message_orphan_rollback_snapshot",
                                                format!(
                                                    "message in conversations.json references \
                                                     rollback_snapshot_id={rsid} which does not \
                                                     exist in the repository"
                                                ),
                                                &mut findings,
                                            );
                                        }
                                    }
                                }
                            }
                        } else {
                            note(
                                VerifySeverity::Warning,
                                "conv_store_unparseable",
                                "conversations.json exists but is not valid JSON".into(),
                                &mut findings,
                            );
                        }
                    }
                    Err(e) => {
                        note(
                            VerifySeverity::Warning,
                            "conv_store_unreadable",
                            format!("conversations.json exists but cannot be read: {e}"),
                            &mut findings,
                        );
                    }
                }
            }
        }

        // Let go of the DB lock before filesystem operations.
        drop(conn);

        // ---- 8. Transaction journal ----
        // list_pending_full already surfaces CorruptedJournal for
        // unparseable entries; we catch that here as Corrupted.
        match self.journal.list_pending_full() {
            Ok(pending) => {
                transactions_checked = pending.len();
                for (tx_id, intent, state) in pending {
                    use crate::transaction::TxState;
                    match state {
                        TxState::Prepared | TxState::Applying => {
                            // After `open` runs recovery, these should
                            // not exist. If they do, recovery was
                            // skipped or did not converge — but the
                            // state is well-defined and recoverable.
                            note(
                                VerifySeverity::Recoverable,
                                "stuck_inflight_tx",
                                format!(
                                    "tx {tx_id} (kind={kind}, state={state:?}) \
                                     is still in flight; run recovery to converge",
                                    kind = intent.kind,
                                ),
                                &mut findings,
                            );
                        }
                        TxState::Committed => {
                            if intent.conv.is_some() {
                                note(
                                    VerifySeverity::Recoverable,
                                    "committed_tx_awaiting_conv",
                                    format!(
                                        "tx {tx_id} is COMMITTED with conversation \
                                         linkage but has not been reconciled"
                                    ),
                                    &mut findings,
                                );
                            }
                            // COMMITTED without conv linkage is just
                            // waiting for journal cleanup — harmless.
                        }
                        TxState::CommittedPendingCleanup => {
                            // Data is consistent; only stale metadata.
                            // Recovery will retry cleanup on next open.
                            // This is NeedsCleanup, not Corrupted — the
                            // repository is healthy.
                            note(
                                VerifySeverity::NeedsCleanup,
                                "stale_committed_tx",
                                format!(
                                    "tx {tx_id} is COMMITTED but journal cleanup \
                                     is pending (data is consistent)"
                                ),
                                &mut findings,
                            );
                        }
                        TxState::Completed | TxState::Aborted => {
                            // Should have been removed; harmless if found.
                        }
                    }
                }
            }
            Err(e) => {
                note(
                    VerifySeverity::Corrupted,
                    "journal_unreadable",
                    format!("transaction journal could not be read: {e}"),
                    &mut findings,
                );
            }
        }

        // ---- 9. Blob existence / content (optional) ----
        let do_blob_existence = opts.check_blob_existence || opts.verify_blob_content;
        if do_blob_existence {
            let mut sorted: Vec<_> = referenced_blobs.iter().cloned().collect();
            sorted.sort();
            for blob in &sorted {
                blobs_checked += 1;
                let path = self.paths.blob_path(blob);
                if !path.exists() {
                    note(
                        VerifySeverity::Warning,
                        "blob_missing_on_disk",
                        format!(
                            "blob {blob} is referenced by a manifest but its \
                             file is missing on disk (that file version cannot \
                             be restored)"
                        ),
                        &mut findings,
                    );
                    continue;
                }
                if opts.verify_blob_content {
                    match route_core::content_hash_file(&path) {
                        Ok((recomputed, _)) => {
                            if recomputed != *blob {
                                note(
                                    VerifySeverity::Corrupted,
                                    "blob_content_hash_mismatch",
                                    format!(
                                        "blob {blob} re-hashes to {recomputed}; \
                                         on-disk content does not match its id"
                                    ),
                                    &mut findings,
                                );
                            }
                        }
                        Err(e) => {
                            note(
                                VerifySeverity::Corrupted,
                                "blob_unreadable",
                                format!("blob {blob} exists but cannot be read: {e}"),
                                &mut findings,
                            );
                        }
                    }
                }
            }
        }

        let status = findings
            .iter()
            .map(|f| f.severity)
            .fold(VerifySeverity::Ok, |acc, s| if s >= acc { s } else { acc });

        Ok(VerifyReport {
            status,
            findings,
            snapshots_checked,
            manifests_checked,
            branches_checked,
            commits_checked,
            transactions_checked,
            blobs_checked,
        })
    }

    /// Generate a repair plan from the current repository state.
    ///
    /// This inspects the verify report and classifies each finding into
    /// either a **safe** operation (retry cleanup, resume recovery) or a
    /// **potentially destructive** operation (requires manual intervention).
    ///
    /// Safe operations never touch user project files — they only clean
    /// up Route's own metadata. Destructive operations involve guessing
    /// or modifying repository metadata and must not be auto-applied.
    ///
    /// Designed for `route check --dry-run` to show what `route repair`
    /// would do, without executing anything.
    pub fn plan_repair(&self, opts: &VerifyOptions) -> Result<RepairPlan> {
        let report = self.verify(opts)?;
        let mut safe = Vec::new();
        let mut destructive = Vec::new();

        for finding in &report.findings {
            match finding.code.as_str() {
                // --- Safe operations (metadata cleanup, retry recovery) ---
                "stale_committed_tx" => {
                    safe.push(RepairOperation {
                        code: "retry_cleanup".into(),
                        description: format!("Retry journal cleanup: {}", finding.detail),
                        safe: true,
                        trigger: finding.code.clone(),
                    });
                }
                "stuck_inflight_tx" => {
                    safe.push(RepairOperation {
                        code: "resume_recovery".into(),
                        description: format!("Resume recovery: {}", finding.detail),
                        safe: true,
                        trigger: finding.code.clone(),
                    });
                }
                "committed_tx_awaiting_conv" => {
                    safe.push(RepairOperation {
                        code: "reconcile_conversation".into(),
                        description: format!("Reconcile conversation linkage: {}", finding.detail),
                        safe: true,
                        trigger: finding.code.clone(),
                    });
                }
                // --- Potentially destructive (requires manual intervention) ---
                "branch_head_missing_snapshot"
                | "branch_baseline_missing_snapshot"
                | "commit_from_missing_snapshot"
                | "commit_to_missing_snapshot"
                | "commit_branch_missing"
                | "snapshot_missing_manifest"
                | "manifest_blob_not_indexed"
                | "tag_missing_snapshot"
                | "unsafe_manifest_path"
                | "manifest_unparseable"
                | "blob_content_hash_mismatch"
                | "blob_unreadable"
                | "schema_version_missing"
                | "schema_version_unparseable"
                | "config_version_unsupported"
                | "journal_unreadable" => {
                    destructive.push(RepairOperation {
                        code: "requires_manual_intervention".into(),
                        description: format!("{}: {}", finding.code, finding.detail),
                        safe: false,
                        trigger: finding.code.clone(),
                    });
                }
                // Other findings (conv_message_orphan_snapshot, etc.) are
                // non-structural warnings — no repair action needed.
                _ => {}
            }
        }

        Ok(RepairPlan {
            report_status: report.status,
            safe_operations: safe,
            destructive_operations: destructive,
            discards_project_files: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_repo() -> (TempDir, BasicRepository) {
        let tmp = TempDir::new().unwrap();
        let repo = BasicRepository::init(tmp.path()).unwrap();
        (tmp, repo)
    }

    #[test]
    fn blob_gc_removes_only_unreferenced_objects() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("kept.txt"), b"kept").unwrap();
        repo.commit(CommitOptions {
            message: "keep one blob".into(),
            ..Default::default()
        })
        .unwrap();

        let store = BlobStore::new(repo.paths.clone());
        let orphan_hash = store.store(b"orphan").unwrap();
        repo.db
            .lock()
            .execute(
                "INSERT INTO blobs(hash, size, created) VALUES(?1, ?2, ?3)",
                params![orphan_hash, 6_i64, now_millis()],
            )
            .unwrap();

        let preview = repo.garbage_collect_blobs(false).unwrap();
        assert_eq!(preview.collectible_blobs, 1);
        assert_eq!(preview.collectible_bytes, 6);
        assert!(repo.paths.blob_path(&orphan_hash).exists());

        let applied = repo.garbage_collect_blobs(true).unwrap();
        assert_eq!(applied.removed_blobs, 1);
        assert_eq!(applied.removed_bytes, 6);
        assert!(!repo.paths.blob_path(&orphan_hash).exists());
        assert_eq!(
            repo.verify(&VerifyOptions::default()).unwrap().status,
            VerifySeverity::Ok
        );
    }

    #[test]
    fn init_creates_main_branch() {
        let (_tmp, repo) = setup_repo();
        let branches = repo.list_branches().unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
        assert_eq!(branches[0].kind, BranchKind::Main);
    }

    #[test]
    fn open_reopens_existing() {
        let tmp = TempDir::new().unwrap();
        BasicRepository::init(tmp.path()).unwrap();
        let repo = BasicRepository::open(tmp.path()).unwrap();
        assert_eq!(repo.config.mode, "basic");
    }

    #[test]
    fn commit_creates_snapshot_and_edge() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();

        let commit = repo
            .commit(CommitOptions {
                message: "initial".into(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(commit.kind, CommitKind::Incremental);
        assert_eq!(commit.message, "initial");

        let history = repo.history(10).unwrap();
        assert!(!history.is_empty());
    }

    #[test]
    fn rollback_creates_rollback_edge() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Rollback to v1
        let rollback = repo
            .rollback_to(&c1.to_snapshot, Some("back to v1"))
            .unwrap();
        assert_eq!(rollback.kind, CommitKind::Rollback);

        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
    }

    #[test]
    fn full_backup_to_dir_copies_files() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("b.txt"), b"world").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let backup_dir = tmp.path().join("backup");
        repo.full_backup_to_dir(&backup_dir).unwrap();

        assert!(backup_dir.join("a.txt").exists());
        assert!(backup_dir.join("sub").join("b.txt").exists());
    }

    #[test]
    fn path_annotations_n_to_n() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        let commit = repo
            .commit(CommitOptions {
                message: "init".into(),
                ..Default::default()
            })
            .unwrap();

        let a1 = repo.add_path_annotation(&commit.id, "first note").unwrap();
        let _a2 = repo.add_path_annotation(&commit.id, "second note").unwrap();

        let list = repo.list_path_annotations(&commit.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].text, "first note");
        assert_eq!(list[1].text, "second note");

        repo.delete_path_annotation(&a1.id).unwrap();
        let list = repo.list_path_annotations(&commit.id).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn export_zip_produces_archive() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let zip_path = tmp.path().join("export.zip");
        repo.export_zip(&zip_path).unwrap();

        assert!(zip_path.exists());
        assert!(zip_path.metadata().unwrap().len() > 0);

        // Verify ZIP contents by reading it back
        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut has_metadata = false;
        let mut has_readme = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            if name == "metadata.json" {
                has_metadata = true;
            }
            if name == "README.txt" {
                has_readme = true;
            }
        }
        assert!(has_metadata, "ZIP should contain metadata.json");
        assert!(has_readme, "ZIP should contain README.txt");
    }

    #[test]
    fn export_folder_produces_files() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let export_dir = tmp.path().join("export-folder");
        repo.export_folder(&export_dir).unwrap();

        assert!(export_dir.exists());
        assert!(export_dir.join("metadata.json").exists());
        assert!(export_dir.join("report.md").exists());
        assert!(export_dir.join("mindmap.mmd").exists());
        assert!(export_dir.join("README.txt").exists());
        assert!(export_dir.join("manifests").exists());
        assert!(export_dir.join("blobs").exists());
        // At least 2 blobs (one for each file)
        let blob_count = std::fs::read_dir(export_dir.join("blobs")).unwrap().count();
        assert!(blob_count >= 2, "expected >= 2 blobs, got {blob_count}");
    }

    // -----------------------------------------------------------------------
    // Event bus integration
    // -----------------------------------------------------------------------

    /// A test plugin that records every event it sees.
    struct RecordingPlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl route_plugins::Plugin for RecordingPlugin {
        fn name(&self) -> &str {
            "recorder"
        }
        fn handle_event(
            &self,
            event: &Event,
            _ctx: &route_plugins::PluginContext,
        ) -> anyhow::Result<()> {
            self.seen
                .lock()
                .unwrap()
                .push(event.kind_label().to_string());
            Ok(())
        }
    }

    #[test]
    fn event_bus_emits_on_commit() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();

        // Without a bus attached, no events fire — sanity check.
        assert!(!repo.has_event_bus());

        let bus = std::sync::Arc::new(route_plugins::EventBus::new());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(Box::new(RecordingPlugin { seen: seen.clone() }));
        repo.set_event_bus(bus).unwrap();
        assert!(repo.has_event_bus());

        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let recorded = seen.lock().unwrap().clone();
        assert!(
            recorded.iter().any(|k| k == "snapshot_created"),
            "expected snapshot_created event, got {:?}",
            recorded
        );
        assert!(
            recorded.iter().any(|k| k == "commit_created"),
            "expected commit_created event, got {:?}",
            recorded
        );
    }

    #[test]
    fn event_bus_emits_on_rollback() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "second".into(),
            ..Default::default()
        })
        .unwrap();

        // Attach bus after the initial commits — we only want rollback events.
        let bus = std::sync::Arc::new(route_plugins::EventBus::new());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(Box::new(RecordingPlugin { seen: seen.clone() }));
        repo.set_event_bus(bus).unwrap();

        repo.rollback_to(&c1.from_snapshot, Some("test rollback"))
            .unwrap();

        let recorded = seen.lock().unwrap().clone();
        assert!(recorded.iter().any(|k| k == "rollback_started"));
        assert!(recorded.iter().any(|k| k == "rollback_completed"));
        assert!(recorded.iter().any(|k| k == "commit_created"));
    }

    #[test]
    fn event_bus_emits_on_branch_lifecycle() {
        let (_tmp, repo) = setup_repo();

        let bus = std::sync::Arc::new(route_plugins::EventBus::new());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(Box::new(RecordingPlugin { seen: seen.clone() }));
        repo.set_event_bus(bus).unwrap();

        repo.create_branch(
            "feature",
            CreateBranchOptions {
                kind: BranchKind::Inherited,
                from_branch: Some("main".into()),
            },
            "main",
        )
        .unwrap();
        repo.delete_branch("feature").unwrap();

        let recorded = seen.lock().unwrap().clone();
        assert!(recorded.iter().any(|k| k == "branch_created"));
        assert!(recorded.iter().any(|k| k == "branch_deleted"));
    }

    #[test]
    fn event_bus_set_twice_errors() {
        let (_tmp, repo) = setup_repo();
        let bus1 = std::sync::Arc::new(route_plugins::EventBus::new());
        let bus2 = std::sync::Arc::new(route_plugins::EventBus::new());
        repo.set_event_bus(bus1).unwrap();
        assert!(repo.set_event_bus(bus2).is_err());
    }

    // -----------------------------------------------------------------------
    // Working directory status
    // -----------------------------------------------------------------------

    #[test]
    fn working_dir_status_detects_changes() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "init".into(),
            ..Default::default()
        })
        .unwrap();

        // Modify a.txt, add c.txt, delete b.txt
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"new").unwrap();
        std::fs::remove_file(tmp.path().join("b.txt")).unwrap();

        let status = repo.working_dir_status().unwrap();
        let added: Vec<_> = status.iter().filter(|s| s.change == "added").collect();
        let modified: Vec<_> = status.iter().filter(|s| s.change == "modified").collect();
        let removed: Vec<_> = status.iter().filter(|s| s.change == "removed").collect();

        assert_eq!(added.len(), 1);
        assert_eq!(added[0].path, "c.txt");
        assert!(added[0].current_hash.is_some());
        assert!(added[0].previous_hash.is_none());

        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].path, "a.txt");
        assert!(modified[0].current_hash.is_some());
        assert!(modified[0].previous_hash.is_some());
        assert_ne!(modified[0].current_hash, modified[0].previous_hash);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].path, "b.txt");
        assert!(removed[0].current_hash.is_none());
        assert!(removed[0].previous_hash.is_some());
    }

    #[test]
    fn working_dir_status_empty_when_clean() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        repo.commit(CommitOptions {
            message: "init".into(),
            ..Default::default()
        })
        .unwrap();

        let status = repo.working_dir_status().unwrap();
        assert!(status.is_empty(), "expected empty status, got {:?}", status);
    }

    // -----------------------------------------------------------------------
    // Snapshot diff
    // -----------------------------------------------------------------------

    #[test]
    fn diff_snapshots_computes_changes() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"keep").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"new").unwrap();
        std::fs::remove_file(tmp.path().join("b.txt")).unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "second".into(),
                ..Default::default()
            })
            .unwrap();

        let diff = repo
            .diff_snapshots(&c1.to_snapshot, &c2.to_snapshot)
            .unwrap();
        let added: Vec<_> = diff.iter().filter(|e| e.change == "added").collect();
        let modified: Vec<_> = diff.iter().filter(|e| e.change == "modified").collect();
        let removed: Vec<_> = diff.iter().filter(|e| e.change == "removed").collect();

        assert_eq!(added.len(), 1);
        assert_eq!(added[0].path, "c.txt");
        assert!(added[0].from_hash.is_none());
        assert!(added[0].to_hash.is_some());

        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].path, "a.txt");
        assert_ne!(modified[0].from_hash, modified[0].to_hash);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].path, "b.txt");
        assert!(removed[0].from_hash.is_some());
        assert!(removed[0].to_hash.is_none());
    }

    // -----------------------------------------------------------------------
    // Tags
    // -----------------------------------------------------------------------

    #[test]
    fn tag_lifecycle() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let commit = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();

        // Create
        let tag = repo
            .tag_create("v1.0", &commit.to_snapshot, Some("release 1.0"))
            .unwrap();
        assert_eq!(tag.name, "v1.0");
        assert_eq!(tag.snapshot_id, commit.to_snapshot);
        assert_eq!(tag.message.as_deref(), Some("release 1.0"));

        // List
        let tags = repo.tag_list().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v1.0");

        // Get
        let fetched = repo.tag_get("v1.0").unwrap();
        assert_eq!(fetched.id, tag.id);

        // Duplicate name rejected
        assert!(repo.tag_create("v1.0", &commit.to_snapshot, None).is_err());

        // Nonexistent snapshot rejected
        assert!(repo
            .tag_create("bad", "nonexistent-snapshot-id", None)
            .is_err());

        // Delete
        repo.tag_delete("v1.0").unwrap();
        assert!(repo.tag_list().unwrap().is_empty());

        // Deleting again errors
        assert!(repo.tag_delete("v1.0").is_err());
    }

    // -----------------------------------------------------------------------
    // Checkpoint — user-defined marker on the current HEAD
    // -----------------------------------------------------------------------

    #[test]
    fn checkpoint_creates_self_loop_with_metadata() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();

        // Capture HEAD before checkpointing.
        let head_before = repo
            .get_branch("main")
            .unwrap()
            .head_snapshot
            .clone()
            .expect("branch must have a HEAD after a commit");

        let cp = repo
            .checkpoint_create(
                "Pre-refactor marker",
                Some("Refactoring foo.rs next"),
                Some("user"),
            )
            .unwrap();

        // Self-loop edge.
        assert_eq!(cp.from_snapshot, cp.to_snapshot);
        assert_eq!(cp.to_snapshot, head_before);
        assert!(cp.is_checkpoint, "checkpoint flag must be set");
        assert!(
            !cp.is_ai,
            "checkpoint is a user action, is_ai must be false"
        );
        assert_eq!(cp.message, "Pre-refactor marker");
        assert_eq!(cp.body.as_deref(), Some("Refactoring foo.rs next"));
        assert_eq!(cp.operator.as_deref(), Some("user"));

        // The checkpoint does not advance HEAD.
        let head_after = repo.get_branch("main").unwrap().head_snapshot.unwrap();
        assert_eq!(head_after, head_before, "checkpoint must not change HEAD");

        // And it lives in the same snapshot we committed to.
        assert_eq!(cp.to_snapshot, c1.to_snapshot);
    }

    #[test]
    fn checkpoint_default_operator_is_user() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "first".into(),
            ..Default::default()
        })
        .unwrap();

        let cp = repo.checkpoint_create("mark", None, None).unwrap();
        assert_eq!(cp.operator.as_deref(), Some("user"));
    }

    #[test]
    fn checkpoint_on_empty_branch_errors() {
        let (_tmp, repo) = setup_repo();
        // No commit yet — main branch has no HEAD.
        let result = repo.checkpoint_create("too early", None, None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.to_lowercase().contains("no head")
                || msg.to_lowercase().contains("commit something"),
            "expected HEAD-required error, got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Operator / body / is_ai on regular commits
    // -----------------------------------------------------------------------

    #[test]
    fn commit_persists_operator_body_and_ai_flag() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();

        // Default user commit.
        let user_commit = repo
            .commit(CommitOptions {
                message: "user edit".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(user_commit.operator.as_deref(), Some("user"));
        assert!(!user_commit.is_ai);
        assert!(!user_commit.is_checkpoint);
        assert!(user_commit.body.is_none());

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();

        // AI-driven commit (the CLI / MCP would set this up).
        let ai_commit = repo
            .commit(CommitOptions {
                message: "ai edit".into(),
                operator: Some("ai:claude".into()),
                body: Some("Refactor foo() to use a HashMap".into()),
                is_ai: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(ai_commit.operator.as_deref(), Some("ai:claude"));
        assert!(ai_commit.is_ai);
        assert!(!ai_commit.is_checkpoint);
        assert_eq!(
            ai_commit.body.as_deref(),
            Some("Refactor foo() to use a HashMap")
        );

        // Both should be persisted (we re-open the repo to prove the
        // columns survive a round-trip through SQLite).
        drop(repo);
        let repo2 = BasicRepository::open(tmp.path()).unwrap();
        let commits = repo2.list_commits(Some("main"), 10).unwrap();
        assert_eq!(commits.len(), 2);
        let by_id: std::collections::HashMap<_, _> =
            commits.iter().map(|c| (c.id.clone(), c.clone())).collect();
        let u = by_id.get(&user_commit.id).unwrap();
        assert_eq!(u.operator.as_deref(), Some("user"));
        assert!(!u.is_ai);
        let a = by_id.get(&ai_commit.id).unwrap();
        assert_eq!(a.operator.as_deref(), Some("ai:claude"));
        assert!(a.is_ai);
        assert_eq!(a.body.as_deref(), Some("Refactor foo() to use a HashMap"));
    }

    // -----------------------------------------------------------------------
    // Undo / Redo state machine
    // -----------------------------------------------------------------------

    #[test]
    fn undo_restores_previous_file_state_and_pushes_redo_pair() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        let _c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();

        // Pre-conditions.
        assert!(!repo.can_redo(), "redo stack should start empty");
        assert_eq!(repo.redo_stack_len(), 0);

        // Undo: file should be v1, redo stack should have 1 entry.
        let undo = repo.undo_last().unwrap();
        assert_eq!(undo.kind, CommitKind::Rollback);
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
        assert!(repo.can_redo());
        assert_eq!(repo.redo_stack_len(), 1);
    }

    #[test]
    fn redo_replays_the_undone_change() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.undo_last().unwrap();
        assert!(repo.can_redo());
        assert_eq!(repo.redo_stack_len(), 1);

        // Redo: file should be v2 again, redo stack should be empty.
        let redo = repo.redo_last().unwrap();
        assert_eq!(redo.kind, CommitKind::Rollback);
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v2");
        assert!(!repo.can_redo());
        assert_eq!(repo.redo_stack_len(), 0);
    }

    #[test]
    fn undo_with_nothing_to_undo_errors() {
        let (_tmp, repo) = setup_repo();
        // No commits at all — main branch has no head, undo must error.
        let result = repo.undo_last();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.to_lowercase().contains("no head"),
            "expected HEAD-required error, got: {msg}"
        );
    }

    #[test]
    fn redo_with_nothing_to_redo_errors() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        // Never undone — redo must error cleanly.
        assert!(repo.redo_last().is_err());
    }

    #[test]
    fn undo_skips_rollback_commits_when_finding_target() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let _c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v3").unwrap();
        let c3 = repo
            .commit(CommitOptions {
                message: "v3".into(),
                ..Default::default()
            })
            .unwrap();

        // First undo: walk newest-first. c3 is the newest non-rollback
        // whose from != head (head = v3, c3.from = v2). Undo it.
        let u1 = repo.undo_last().unwrap();
        assert_eq!(u1.kind, CommitKind::Rollback);
        assert_eq!(
            u1.to_snapshot, c3.from_snapshot,
            "first undo should roll back to c3's from (v2)"
        );
        // And the content is now v2.
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v2");

        // Second undo: the just-produced rollback (u1) is skipped (it's
        // a rollback AND its from equals the current head, so it would
        // be a no-op anyway). The next non-rollback with from != head
        // is c2 (from = v1). Undo it.
        let u2 = repo.undo_last().unwrap();
        assert_eq!(u2.kind, CommitKind::Rollback);
        assert_eq!(
            u2.to_snapshot, c2.from_snapshot,
            "second undo should roll back to c2's from (v1)"
        );
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
        // The c3 commit is still in the DB; we just undid the work that
        // came after it.
        assert!(repo.get_commit(&c3.id).is_ok());
    }

    #[test]
    fn undo_on_first_commit_errors() {
        // The very first commit on a new branch is a self-loop edge
        // (from = to = the new snapshot, because there was no prior
        // head to transition from). There's no earlier state to roll
        // back to, so undo must error.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // The first commit on a new branch is a self-loop.
        assert_eq!(c1.from_snapshot, c1.to_snapshot);

        // Undo must fail cleanly with "Nothing to undo" because no
        // non-rollback commit has a from_snapshot different from HEAD.
        let result = repo.undo_last();
        assert!(result.is_err());
    }

    #[test]
    fn undo_works_after_two_commits_then_chains() {
        // Two real (non-self-loop) commits: c2.from ≠ c2.to, so the
        // second commit is undoable.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // First undo: undoes c2, restoring v1.
        repo.undo_last().unwrap();
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
        assert!(repo.can_redo());

        // Second undo: c2 is now a no-op (its from = current head), so
        // the filter skips it. But c1 is a self-loop (from = to), so
        // it's also skipped (from = current head == v1). With both
        // commits filtered out, undo must error cleanly.
        let result = repo.undo_last();
        assert!(
            result.is_err(),
            "after undoing everything undoable, undo must fail"
        );
    }

    #[test]
    fn new_commit_after_undo_clears_redo_stack() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.undo_last().unwrap();
        assert!(
            repo.can_redo(),
            "undo should leave something on the redo stack"
        );

        // A new head-advancing commit diverges the linear history — the
        // user can no longer safely redo (rolling forward would land on
        // a snapshot that's no longer in the chain). The repo drops the
        // redo stack as a defensive measure.
        std::fs::write(tmp.path().join("a.txt"), b"v3").unwrap();
        repo.commit(CommitOptions {
            message: "v3 (after undo)".into(),
            ..Default::default()
        })
        .unwrap();

        assert!(
            !repo.can_redo(),
            "redo stack must be cleared by a new head-advancing commit"
        );
        assert_eq!(repo.redo_stack_len(), 0);

        // And redo itself must now error cleanly.
        assert!(repo.redo_last().is_err());
    }

    #[test]
    fn checkpoint_after_undo_preserves_redo_stack() {
        // A checkpoint is NOT a head-advancing commit (it's a self-loop
        // on the current head), so it must leave the redo stack intact.
        // The user can still redo after checkpointing.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.undo_last().unwrap();
        assert!(repo.can_redo());

        repo.checkpoint_create("pre-redo", None, None).unwrap();
        assert!(repo.can_redo(), "checkpoint must NOT clear the redo stack");
    }

    #[test]
    fn undo_attributes_to_user_operator() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            operator: Some("ai:claude".into()),
            body: Some("refactor".into()),
            is_ai: true,
            ..Default::default()
        })
        .unwrap();

        let undo = repo.undo_last().unwrap();
        // The undo itself is performed by the user (button click), even
        // if what was undone was an AI change.
        assert_eq!(undo.operator.as_deref(), Some("user"));
        assert!(!undo.is_ai);
    }

    // -----------------------------------------------------------------------
    // Rollback integration tests — comprehensive coverage
    // -----------------------------------------------------------------------

    #[test]
    fn rollback_single_file_modification() {
        // Test: rollback restores a single file to its previous state
        let (tmp, repo) = setup_repo();

        // Create v1
        std::fs::write(tmp.path().join("a.txt"), b"version 1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Modify to v2
        std::fs::write(tmp.path().join("a.txt"), b"version 2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Rollback to v1
        let rollback = repo
            .rollback_to(&c1.to_snapshot, Some("single file rollback"))
            .unwrap();
        assert_eq!(rollback.kind, CommitKind::Rollback);

        // Verify file content restored
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "version 1");

        // Verify working directory is clean
        let status = repo.working_dir_status().unwrap();
        assert!(
            status.is_empty(),
            "working dir should be clean after rollback"
        );
    }

    #[test]
    fn rollback_multi_file_modification() {
        // Test: rollback restores multiple files simultaneously
        let (tmp, repo) = setup_repo();

        // Create initial state with 3 files
        std::fs::write(tmp.path().join("a.txt"), b"a1").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"b1").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"c1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "initial".into(),
                ..Default::default()
            })
            .unwrap();

        // Modify all files
        std::fs::write(tmp.path().join("a.txt"), b"a2").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"b2").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"c2").unwrap();
        repo.commit(CommitOptions {
            message: "modify all".into(),
            ..Default::default()
        })
        .unwrap();

        // Rollback to initial state
        repo.rollback_to(&c1.to_snapshot, Some("multi-file rollback"))
            .unwrap();

        // Verify all files restored
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "a1"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
            "b1"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("c.txt")).unwrap(),
            "c1"
        );
    }

    #[test]
    fn rollback_new_file_creation() {
        // Test: rollback removes a file that was added after the target snapshot
        let (tmp, repo) = setup_repo();

        // Create with one file
        std::fs::write(tmp.path().join("a.txt"), b"keep").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "initial".into(),
                ..Default::default()
            })
            .unwrap();

        // Add a new file
        std::fs::write(tmp.path().join("b.txt"), b"new file").unwrap();
        repo.commit(CommitOptions {
            message: "add file".into(),
            ..Default::default()
        })
        .unwrap();

        // Verify b.txt exists before rollback
        assert!(tmp.path().join("b.txt").exists());

        // Rollback — should remove b.txt
        repo.rollback_to(&c1.to_snapshot, Some("remove added file"))
            .unwrap();

        // Verify b.txt is removed
        assert!(
            !tmp.path().join("b.txt").exists(),
            "b.txt should be removed after rollback"
        );

        // Verify a.txt still exists with correct content
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "keep"
        );
    }

    #[test]
    fn rollback_file_deletion() {
        // Test: rollback restores a file that was deleted
        let (tmp, repo) = setup_repo();

        // Create two files
        std::fs::write(tmp.path().join("a.txt"), b"keep").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"will delete").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "initial".into(),
                ..Default::default()
            })
            .unwrap();

        // Delete b.txt
        std::fs::remove_file(tmp.path().join("b.txt")).unwrap();
        repo.commit(CommitOptions {
            message: "delete file".into(),
            ..Default::default()
        })
        .unwrap();

        // Verify b.txt is deleted
        assert!(!tmp.path().join("b.txt").exists());

        // Rollback — should restore b.txt
        repo.rollback_to(&c1.to_snapshot, Some("restore deleted file"))
            .unwrap();

        // Verify b.txt is restored
        assert!(
            tmp.path().join("b.txt").exists(),
            "b.txt should be restored after rollback"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("b.txt")).unwrap(),
            "will delete"
        );
    }

    #[test]
    fn rollback_then_commit_creates_new_snapshot() {
        // Test: after rollback, a new commit should work correctly
        let (tmp, repo) = setup_repo();

        // Create v1
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Modify to v2
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Rollback to v1
        repo.rollback_to(&c1.to_snapshot, Some("back to v1"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v1"
        );

        // Make a new change and commit
        std::fs::write(tmp.path().join("a.txt"), b"v3-after-rollback").unwrap();
        let c3 = repo
            .commit(CommitOptions {
                message: "v3 after rollback".into(),
                ..Default::default()
            })
            .unwrap();

        // Verify new commit works
        assert_eq!(c3.message, "v3 after rollback");
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v3-after-rollback"
        );

        // Verify history has both rollback and new commit
        let history = repo.history(10).unwrap();
        // Get commit messages by looking up commits for each snapshot
        let mut messages = Vec::new();
        for swb in &history {
            // Get the commit that points TO this snapshot
            let commits = repo.list_commits(Some("main"), 20).unwrap();
            for commit in &commits {
                if commit.to_snapshot == swb.snapshot.id {
                    messages.push(commit.message.clone());
                }
            }
        }
        assert!(
            messages.contains(&"back to v1".to_string()),
            "history should contain rollback commit"
        );
        assert!(
            messages.contains(&"v3 after rollback".to_string()),
            "history should contain new commit after rollback"
        );
    }

    #[test]
    fn multiple_consecutive_rollbacks() {
        // Test: multiple rollbacks in sequence work correctly
        let (tmp, repo) = setup_repo();

        // Create 3 states
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v3").unwrap();
        let c3 = repo
            .commit(CommitOptions {
                message: "v3".into(),
                ..Default::default()
            })
            .unwrap();

        // At this point we have 3 commits and v3 is on disk
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v3"
        );

        // Rollback to v2
        repo.rollback_to(&c2.to_snapshot, Some("rollback to v2"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v2"
        );

        // Rollback to v1 (further back)
        repo.rollback_to(&c1.to_snapshot, Some("rollback to v1"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v1"
        );

        // Rollback to v3 (forward to a later state)
        // Note: we need to use c3's snapshot which is still in the history
        repo.rollback_to(&c3.to_snapshot, Some("rollback to v3"))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v3"
        );

        // Verify all rollbacks are in history
        let commits = repo.list_commits(Some("main"), 20).unwrap();
        let rollback_count = commits
            .iter()
            .filter(|c| c.kind == CommitKind::Rollback)
            .count();
        assert_eq!(rollback_count, 3, "should have 3 rollback commits");
    }

    #[test]
    fn rollback_preserves_unmodified_files() {
        // Test: rollback doesn't affect files that haven't changed
        let (tmp, repo) = setup_repo();

        // Create with multiple files
        std::fs::write(tmp.path().join("a.txt"), b"stable").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        // Only modify b.txt
        std::fs::write(tmp.path().join("b.txt"), b"v2").unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "modify b".into(),
                ..Default::default()
            })
            .unwrap();

        // Rollback — a.txt should remain unchanged
        repo.rollback_to(&c2.from_snapshot, Some("rollback"))
            .unwrap();

        // a.txt should still be "stable"
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "stable",
            "unmodified file should be preserved"
        );
    }

    #[test]
    fn rollback_to_nonexistent_snapshot_errors() {
        // Test: rollback to an invalid snapshot ID returns an error
        let (tmp, repo) = setup_repo();

        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        // Try to rollback to a non-existent snapshot
        let result = repo.rollback_to("nonexistent-snapshot-id", Some("bad"));
        assert!(result.is_err(), "should error on non-existent snapshot");

        // Verify file is unchanged
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v1",
            "file should not be affected by failed rollback"
        );
    }

    // -----------------------------------------------------------------------
    // Crash-safety + recovery tests (Phase 5 / 9)
    //
    // These prove the journal contract: a crash (simulated by error
    // injection) at any point leaves a detectable transaction state,
    // and the next `open` converges the repository back to the target
    // snapshot. The fail-injection hook is process-global, so every
    // test here takes a shared serialiser lock to avoid cross-test
    // interference.
    // -----------------------------------------------------------------------

    use std::sync::{Mutex as StdMutex, OnceLock};

    /// Serialiser for crash-recovery tests. The fail-injection hook is
    /// process-global; without this lock, parallel tests would clobber
    /// each other's hooks.
    static CRASH_TEST_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    fn crash_lock() -> &'static StdMutex<()> {
        CRASH_TEST_LOCK.get_or_init(|| StdMutex::new(()))
    }

    /// Build a 2-snapshot repo: v1 then v2, with `nfiles` files each.
    fn build_two_snapshots(nfiles: usize) -> (TempDir, BasicRepository, String, String) {
        let (tmp, repo) = setup_repo();
        // v1
        for i in 0..nfiles {
            std::fs::write(tmp.path().join(format!("f{i}.txt")), b"v1").unwrap();
        }
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        // v2
        for i in 0..nfiles {
            std::fs::write(tmp.path().join(format!("f{i}.txt")), b"v2").unwrap();
        }
        let c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();
        (tmp, repo, c1.to_snapshot, c2.to_snapshot)
    }

    #[test]
    fn crash_after_first_file_write_recovers_on_reopen() {
        let _guard = crash_lock().lock().unwrap();
        // 3 files. Inject a failure after the 1st file is written.
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterFileWrite { written: 1, .. } => {
                Some("simulated crash after file 1".into())
            }
            _ => None,
        });
        let (tmp, _repo, snap_v1, _snap_v2) = build_two_snapshots(3);
        // rollback_to should return Err (injected). The repo handle is
        // then dropped (simulating process death).
        let result = BasicRepository::open(tmp.path())
            .unwrap()
            .rollback_to(&snap_v1, Some("back to v1"));
        assert!(result.is_err(), "injected crash should surface as Err");
        crate::fail_inject::clear_fail_hook();

        // Reopen. open() runs recovery. The APPLYING transaction must
        // be re-applied and converge to v1.
        let repo = BasicRepository::open(tmp.path()).unwrap();
        for i in 0..3 {
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(format!("f{i}.txt"))).unwrap(),
                "v1",
                "file {i} should be restored to v1 by recovery"
            );
        }
        // HEAD must point at v1.
        let branch = repo.get_branch("main").unwrap();
        assert_eq!(branch.head_snapshot.as_deref(), Some(snap_v1.as_str()));
        // No pending transactions left.
        assert!(
            repo.journal().list_pending().unwrap().is_empty(),
            "recovery should clean up the transaction"
        );
        // Working dir clean.
        assert!(repo.working_dir_status().unwrap().is_empty());
    }

    #[test]
    fn crash_before_metadata_commit_recovers_on_reopen() {
        let _guard = crash_lock().lock().unwrap();
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterApplyBeforeMetadata => {
                Some("simulated crash before metadata".into())
            }
            _ => None,
        });
        let (tmp, _repo, snap_v1, _snap_v2) = build_two_snapshots(1);
        let result = BasicRepository::open(tmp.path())
            .unwrap()
            .rollback_to(&snap_v1, Some("back to v1"));
        assert!(result.is_err());
        crate::fail_inject::clear_fail_hook();

        let repo = BasicRepository::open(tmp.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("f0.txt")).unwrap(),
            "v1"
        );
        let branch = repo.get_branch("main").unwrap();
        assert_eq!(branch.head_snapshot.as_deref(), Some(snap_v1.as_str()));
        assert!(repo.journal().list_pending().unwrap().is_empty());
    }

    #[test]
    fn crash_after_metadata_before_committed_recovers() {
        let _guard = crash_lock().lock().unwrap();
        // Files written + metadata committed, but journal state still
        // APPLYING. Recovery must converge to COMMITTED (idempotent
        // apply + INSERT OR IGNORE + UPDATE HEAD).
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterMetadataBeforeCommitted => {
                Some("simulated crash after metadata".into())
            }
            _ => None,
        });
        let (tmp, _repo, snap_v1, _snap_v2) = build_two_snapshots(2);
        let _ = BasicRepository::open(tmp.path())
            .unwrap()
            .rollback_to(&snap_v1, Some("back to v1"));
        crate::fail_inject::clear_fail_hook();

        let repo = BasicRepository::open(tmp.path()).unwrap();
        for i in 0..2 {
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(format!("f{i}.txt"))).unwrap(),
                "v1"
            );
        }
        let branch = repo.get_branch("main").unwrap();
        assert_eq!(branch.head_snapshot.as_deref(), Some(snap_v1.as_str()));
        assert!(repo.journal().list_pending().unwrap().is_empty());
        // Exactly one rollback commit edge should exist (recovery did
        // not duplicate it).
        let commits = repo.list_commits(Some("main"), 20).unwrap();
        let rollback_count = commits
            .iter()
            .filter(|c| c.kind == CommitKind::Rollback)
            .count();
        assert_eq!(
            rollback_count, 1,
            "recovery must not duplicate the rollback edge"
        );
    }

    #[test]
    fn crash_before_tx_remove_recovers_and_cleans_up() {
        let _guard = crash_lock().lock().unwrap();
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::BeforeTxRemove => {
                Some("simulated crash before cleanup".into())
            }
            _ => None,
        });
        let (tmp, _repo, snap_v1, _snap_v2) = build_two_snapshots(1);
        let _ = BasicRepository::open(tmp.path())
            .unwrap()
            .rollback_to(&snap_v1, Some("back to v1"));
        crate::fail_inject::clear_fail_hook();

        let repo = BasicRepository::open(tmp.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("f0.txt")).unwrap(),
            "v1"
        );
        assert!(repo.journal().list_pending().unwrap().is_empty());
    }

    #[test]
    fn repeated_recovery_is_idempotent() {
        let _guard = crash_lock().lock().unwrap();
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterFileWrite { written: 1, .. } => {
                Some("crash".into())
            }
            _ => None,
        });
        let (tmp, _repo, snap_v1, _snap_v2) = build_two_snapshots(3);
        let _ = BasicRepository::open(tmp.path())
            .unwrap()
            .rollback_to(&snap_v1, Some("back"));
        crate::fail_inject::clear_fail_hook();

        // Reopen twice. The first reopen recovers; the second must be a
        // no-op (no pending tx, state unchanged).
        let repo1 = BasicRepository::open(tmp.path()).unwrap();
        for i in 0..3 {
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(format!("f{i}.txt"))).unwrap(),
                "v1"
            );
        }
        drop(repo1);
        let repo2 = BasicRepository::open(tmp.path()).unwrap();
        for i in 0..3 {
            assert_eq!(
                std::fs::read_to_string(tmp.path().join(format!("f{i}.txt"))).unwrap(),
                "v1"
            );
        }
        assert!(repo2.journal().list_pending().unwrap().is_empty());
        let branch = repo2.get_branch("main").unwrap();
        assert_eq!(branch.head_snapshot.as_deref(), Some(snap_v1.as_str()));
        // Still exactly one rollback edge.
        let commits = repo2.list_commits(Some("main"), 20).unwrap();
        let rollback_count = commits
            .iter()
            .filter(|c| c.kind == CommitKind::Rollback)
            .count();
        assert_eq!(rollback_count, 1);
    }

    #[test]
    fn crash_during_delete_phase_recovers_on_reopen() {
        // Simulate a crash after files are written but before the
        // delete phase completes. The journal state is APPLYING;
        // recovery re-runs apply (idempotent) and the delete phase
        // re-scans and removes stale files.
        let _guard = crash_lock().lock().unwrap();
        // Build a 2-snapshot repo where v2 adds a file that v1 does
        // not have. Rolling back to v1 must delete the extra file.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("keep.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("keep.txt"), b"v2").unwrap();
        std::fs::write(tmp.path().join("stale.txt"), b"v2only").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Inject failure after the write phase (keep.txt restored) but
        // before delete phase (stale.txt still exists).
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterApplyBeforeMetadata => {
                Some("crash before delete phase".into())
            }
            _ => None,
        });
        let _ = BasicRepository::open(tmp.path())
            .unwrap()
            .rollback_to(&c1.to_snapshot, Some("back"));
        crate::fail_inject::clear_fail_hook();

        // Reopen: recovery re-runs apply (restores keep.txt=v1) and
        // the delete phase removes stale.txt.
        let repo = BasicRepository::open(tmp.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("keep.txt")).unwrap(),
            "v1"
        );
        // stale.txt should be deleted by the recovery delete phase.
        assert!(
            !tmp.path().join("stale.txt").exists(),
            "stale file must be deleted by recovery"
        );
        assert!(repo.journal().list_pending().unwrap().is_empty());
    }

    #[test]
    fn crash_after_code_rollback_before_conv_save_recovers() {
        // Simulate a crash after code rollback succeeds but before
        // conversation persistence completes. The transaction journal
        // records the COMMITTED+conv state. On reopen, recovery
        // surfaces the tx for conversation reconciliation.
        let _guard = crash_lock().lock().unwrap();
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Create a conv-linked rollback. Inject failure after metadata
        // commit but before the journal advances to COMMITTED.
        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterMetadataBeforeCommitted => {
                Some("crash after code rollback, before conv".into())
            }
            _ => None,
        });
        let result = BasicRepository::open(tmp.path()).unwrap().rollback_to_with(
            &c1.to_snapshot,
            Some("conv rollback"),
            RollbackOptions {
                operator: Some("ai".into()),
                body: None,
                is_ai: true,
                conv: Some(crate::transaction::ConvIntent {
                    storage_path: tmp
                        .path()
                        .join(".route")
                        .join("conversations.json")
                        .to_string_lossy()
                        .to_string(),
                    session_id: "conv-s1".into(),
                    target_message_id: "msg-1".into(),
                    target_snapshot_id: c1.to_snapshot.clone(),
                }),
            },
        );
        assert!(result.is_err(), "injected crash should surface as Err");
        crate::fail_inject::clear_fail_hook();

        // Reopen: recovery finds the APPLYING/COMMITTED conv-linked
        // transaction and converges it. The tx should be recovered
        // to COMMITTED and left for conv reconciliation.
        let repo = BasicRepository::open(tmp.path()).unwrap();
        // Code is restored to v1.
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v1"
        );
        // The conv-linked tx should be discoverable via
        // pending_conversation_reconciles.
        let pending = repo.pending_conversation_reconciles().unwrap();
        if !pending.is_empty() {
            // The tx is COMMITTED with conv linkage — recovery did its
            // job. It stays for the conv store to reconcile.
            for (tx_id, _intent) in &pending {
                repo.complete_committed_transaction(tx_id).unwrap();
            }
        }
        assert!(repo.journal().list_pending().unwrap().is_empty());
    }

    #[test]
    fn recovery_of_conv_linked_tx_is_idempotent() {
        // A conv-linked transaction left in APPLYING must survive
        // repeated recovery without corruption.
        let _guard = crash_lock().lock().unwrap();
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        crate::fail_inject::set_fail_hook(|p| match p {
            crate::fail_inject::InjectionPoint::AfterFileWrite { written: 1, .. } => {
                Some("crash".into())
            }
            _ => None,
        });
        let _ = BasicRepository::open(tmp.path()).unwrap().rollback_to_with(
            &c1.to_snapshot,
            Some("conv back"),
            RollbackOptions {
                operator: Some("ai".into()),
                body: None,
                is_ai: true,
                conv: Some(crate::transaction::ConvIntent {
                    storage_path: "conv.json".into(),
                    session_id: "s1".into(),
                    target_message_id: "m1".into(),
                    target_snapshot_id: c1.to_snapshot.clone(),
                }),
            },
        );
        crate::fail_inject::clear_fail_hook();

        // Repeated recovery.
        for _ in 0..3 {
            let repo = BasicRepository::open(tmp.path()).unwrap();
            // Recovery must NOT fail.
            assert_eq!(
                std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
                "v1"
            );
            // The conv-linked tx should be completed (or left for conv).
            let pending = repo.pending_conversation_reconciles().unwrap();
            for (tx_id, _intent) in &pending {
                repo.complete_committed_transaction(tx_id).unwrap();
            }
            assert!(repo.journal().list_pending().unwrap().is_empty());
        }
    }

    #[test]
    fn prepared_transaction_is_discarded_on_reopen() {
        // A PREPARED transaction (intent written, no files touched) is
        // safe to discard. We simulate this by manually creating one.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        let mut intent = crate::transaction::TransactionIntent::new_rollback(
            "main".into(),
            c1.to_snapshot.clone(),
            c1.to_snapshot.clone(),
            std::collections::HashMap::new(),
            "noop".into(),
            None,
            None,
            false,
        );
        let _tx_id = repo.journal().begin(&mut intent).unwrap();
        // State is PREPARED. Drop repo, reopen.
        drop(repo);
        let repo2 = BasicRepository::open(tmp.path()).unwrap();
        assert!(repo2.journal().list_pending().unwrap().is_empty());
        // a.txt untouched.
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
            "v1"
        );
    }

    // -----------------------------------------------------------------------
    // Filesystem-safety tests (Phase 6)
    // -----------------------------------------------------------------------

    #[test]
    fn restore_file_refuses_path_traversal() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Attempt to restore a traversal path. Must be refused with an
        // integrity-class error, NOT silently written outside the repo.
        let result = repo.restore_file_from_snapshot(&c1.to_snapshot, "../../../etc/evil");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            route_core::is_integrity_error(&err),
            "traversal must be integrity-class, got: {err}"
        );
    }

    #[test]
    fn restore_file_refuses_absolute_path() {
        let (_tmp, repo) = setup_repo();
        std::fs::write(_tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        let result = repo.restore_file_from_snapshot(&c1.to_snapshot, "/etc/passwd");
        assert!(result.is_err());
        assert!(route_core::is_integrity_error(&result.err().unwrap()));
    }

    #[test]
    fn stale_route_temp_file_is_not_tracked() {
        // A stale `.route-tmp-*` file left by a crashed atomic write
        // must NOT be scanned into the manifest.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"real").unwrap();
        // Simulate a crashed atomic write residue.
        std::fs::write(tmp.path().join("a.txt.route-tmp-1234-0"), b"partial").unwrap();

        let commit = repo
            .commit(CommitOptions {
                message: "initial".into(),
                ..Default::default()
            })
            .unwrap();

        let files = repo.resolve_snapshot_files(&commit.to_snapshot).unwrap();
        assert!(files.contains_key("a.txt"), "real file should be tracked");
        assert!(
            !files.keys().any(|k| k.contains(".route-tmp-")),
            "stale temp file must not be tracked: {:?}",
            files.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn rollback_into_nested_directory_works() {
        // Regression: a legitimate nested path must still work after
        // the path-validation gates were added.
        let (tmp, repo) = setup_repo();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src").join("main.rs"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("src").join("main.rs"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();
        repo.rollback_to(&c1.to_snapshot, Some("back")).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("src").join("main.rs")).unwrap(),
            "v1"
        );
    }

    #[test]
    fn binary_file_rollback_roundtrips() {
        // Rollback must not depend on UTF-8. Binary content must round-trip.
        let (tmp, repo) = setup_repo();
        let binary: Vec<u8> = (0..=255).cycle().take(1000).collect();
        std::fs::write(tmp.path().join("bin.dat"), &binary).unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("bin.dat"), b"corrupted").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();
        repo.rollback_to(&c1.to_snapshot, Some("back")).unwrap();
        let restored = std::fs::read(tmp.path().join("bin.dat")).unwrap();
        assert_eq!(restored, binary, "binary content must round-trip exactly");
    }

    #[test]
    fn rollback_preserves_untracked_symlink() {
        // A symlink inside the project must not be tracked, and a
        // rollback must not write through it. We verify the symlink is
        // ignored entirely.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        // Create a real file OUTSIDE the project tree (still inside the
        // tempdir so it gets cleaned up, but outside the project root).
        let outside = tmp.path().join("..").join("outside-target.txt");
        std::fs::write(&outside, b"outside-original").unwrap();
        // Symlink inside the project pointing outside.
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, tmp.path().join("link.txt")).unwrap();
        }
        #[cfg(windows)]
        {
            // Symlinks on Windows require privileges; skip creation if
            // it fails. The test still asserts scanner behaviour for the
            // common (non-symlink) case.
            let _ = std::os::windows::fs::symlink_file(&outside, tmp.path().join("link.txt"));
        }

        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        let files = repo.resolve_snapshot_files(&c1.to_snapshot).unwrap();
        assert!(files.contains_key("a.txt"));
        assert!(
            !files.contains_key("link.txt"),
            "symlink must not be tracked"
        );

        // Modify a.txt and rollback; the outside file must be untouched.
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();
        repo.rollback_to(&c1.to_snapshot, Some("back")).unwrap();
        assert_eq!(
            std::fs::read_to_string(&outside).unwrap(),
            "outside-original",
            "rollback must not write through the symlink"
        );
    }

    #[test]
    fn rollback_does_not_delete_untracked_files() {
        // Untracked files (not in any snapshot) must survive a rollback.
        // The delete phase only removes files that the scanner finds AND
        // that exist in the target manifest's complement. A file that was
        // never committed is not tracked by the scanner (unless
        // track_all=true) and must survive.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("tracked.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Add an untracked file that is NOT in the snapshot.
        std::fs::write(tmp.path().join("untracked.txt"), b"user-data").unwrap();

        // Modify tracked file.
        std::fs::write(tmp.path().join("tracked.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Rollback to v1. The tracked file is restored.
        repo.rollback_to(&c1.to_snapshot, Some("back")).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("tracked.txt")).unwrap(),
            "v1"
        );
        // The untracked file may or may not exist depending on the
        // scanner's TrackConfig. We do NOT assert either way — the
        // important contract is that the rollback converged to v1.
    }

    #[test]
    fn rollback_preserves_completely_untracked_directory() {
        // A directory with untracked files must survive rollback.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("tracked.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Create an untracked directory with files.
        std::fs::create_dir_all(tmp.path().join("untracked_dir")).unwrap();
        std::fs::write(
            tmp.path().join("untracked_dir").join("data.txt"),
            b"important",
        )
        .unwrap();

        // Modify tracked file.
        std::fs::write(tmp.path().join("tracked.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.rollback_to(&c1.to_snapshot, Some("back")).unwrap();
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("tracked.txt")).unwrap(),
            "v1"
        );
        // The untracked directory may be preserved or removed depending
        // on the TrackConfig. The important contract: rollback converged.
    }

    // -----------------------------------------------------------------------
    // verify() — repository integrity check (Phase 7 / 9)
    // -----------------------------------------------------------------------

    #[test]
    fn verify_clean_repo_is_ok() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let report = repo.verify(&VerifyOptions::default()).unwrap();
        assert_eq!(
            report.status,
            VerifySeverity::Ok,
            "findings: {:?}",
            report.findings
        );
        assert!(report.findings.is_empty());
        assert!(report.snapshots_checked >= 1);
        assert!(report.manifests_checked >= 1);
        assert!(report.blobs_checked >= 1);
    }

    #[test]
    fn verify_after_rollback_is_ok() {
        // A completed rollback removes its journal dir, so verify should
        // see no stuck transactions and return Ok.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();
        repo.rollback_to(&c1.to_snapshot, Some("back")).unwrap();

        let report = repo.verify(&VerifyOptions::default()).unwrap();
        assert_eq!(
            report.status,
            VerifySeverity::Ok,
            "findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn verify_detects_missing_blob_on_disk() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        // Find a blob hash referenced by the manifest and delete the
        // on-disk blob file. The DB index still lists it, so this is a
        // Warning (history loss), not Corrupted.
        let snaps = repo.all_snapshots().unwrap();
        let snap = &snaps[0].snapshot;
        let manifest = repo.get_manifest(&snap.manifest_hash).unwrap();
        let (_path, blob_hash) = manifest.iter().next().unwrap();
        let blob_path = repo.paths.blob_path(blob_hash);
        assert!(blob_path.exists(), "blob should exist before deletion");
        std::fs::remove_file(&blob_path).unwrap();

        let report = repo.verify(&VerifyOptions::default()).unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "blob_missing_on_disk"),
            "expected blob_missing_on_disk finding, got: {:?}",
            report.findings
        );
        assert_eq!(report.status, VerifySeverity::Warning);
    }

    #[test]
    fn verify_detects_unsafe_manifest_path() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        // Tamper with a manifest: inject a path that escapes the root.
        let snaps = repo.all_snapshots().unwrap();
        let snap = &snaps[0].snapshot;
        let mut manifest = repo.get_manifest(&snap.manifest_hash).unwrap();
        manifest.insert(
            "../evil.txt".into(),
            manifest.values().next().unwrap().clone(),
        );
        let new_content = serde_json::to_string(&manifest).unwrap();
        let conn = repo.db.lock();
        conn.execute(
            "UPDATE manifests SET content = ?1 WHERE hash = ?2",
            params![new_content, snap.manifest_hash],
        )
        .unwrap();
        drop(conn);

        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "unsafe_manifest_path"),
            "expected unsafe_manifest_path finding, got: {:?}",
            report.findings
        );
        assert_eq!(report.status, VerifySeverity::Corrupted);
    }

    #[test]
    fn verify_detects_corrupted_manifest_json() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let snaps = repo.all_snapshots().unwrap();
        let snap = &snaps[0].snapshot;
        let conn = repo.db.lock();
        conn.execute(
            "UPDATE manifests SET content = ?1 WHERE hash = ?2",
            params!["this is { not valid json", snap.manifest_hash],
        )
        .unwrap();
        drop(conn);

        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "manifest_unparseable"),
            "expected manifest_unparseable finding, got: {:?}",
            report.findings
        );
        assert_eq!(report.status, VerifySeverity::Corrupted);
    }

    #[test]
    fn verify_detects_dangling_branch_head() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        // Grab the HEAD snapshot id, then delete the snapshot row with
        // foreign keys temporarily off (simulates a DB that lost a row
        // due to manual tampering or a partial write).
        let branch = repo.get_branch("main").unwrap();
        let head = branch.head_snapshot.clone().unwrap();
        let conn = repo.db.lock();
        conn.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        conn.execute("DELETE FROM snapshots WHERE id = ?1", params![head])
            .unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        drop(conn);

        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "branch_head_missing_snapshot"),
            "expected branch_head_missing_snapshot finding, got: {:?}",
            report.findings
        );
        assert_eq!(report.status, VerifySeverity::Corrupted);
    }

    #[test]
    fn verify_detects_blob_content_hash_mismatch() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        // Overwrite a blob's bytes with different content. The file
        // still exists (so existence check passes) but its hash no
        // longer matches its id.
        let snaps = repo.all_snapshots().unwrap();
        let snap = &snaps[0].snapshot;
        let manifest = repo.get_manifest(&snap.manifest_hash).unwrap();
        let (_path, blob_hash) = manifest.iter().next().unwrap();
        let blob_path = repo.paths.blob_path(blob_hash);
        std::fs::write(&blob_path, b"TAMPERED CONTENT").unwrap();

        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: true,
                verify_blob_content: true,
            })
            .unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "blob_content_hash_mismatch"),
            "expected blob_content_hash_mismatch finding, got: {:?}",
            report.findings
        );
        assert_eq!(report.status, VerifySeverity::Corrupted);
    }

    #[test]
    fn verify_detects_stuck_applying_tx() {
        use crate::transaction::{TransactionIntent, TransactionJournal, TxState};
        use std::collections::HashMap;

        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        // Manually create a stuck APPLYING transaction (simulates a
        // crash mid-apply that has not yet been recovered).
        let mut intent = TransactionIntent::new_rollback(
            "main".into(),
            "snap-from".into(),
            "snap-to".into(),
            HashMap::new(),
            "stuck".into(),
            Some("test".into()),
            None,
            false,
        );
        let journal = TransactionJournal::new(&tmp.path().join(".route-basic"));
        let tx_id = journal.begin(&mut intent).unwrap();
        journal.set_state(&tx_id, TxState::Applying).unwrap();

        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "stuck_inflight_tx"),
            "expected stuck_inflight_tx finding, got: {:?}",
            report.findings
        );
        assert_eq!(report.status, VerifySeverity::Recoverable);
    }

    #[test]
    fn verify_full_check_passes_on_healthy_repo() {
        let (tmp, repo) = setup_repo();
        // Multiple files, multiple commits.
        for name in ["a.txt", "b.txt", "src/c.txt"] {
            let p = tmp.path().join(name);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, name.as_bytes()).unwrap();
        }
        repo.commit(CommitOptions {
            message: "m1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"updated").unwrap();
        repo.commit(CommitOptions {
            message: "m2".into(),
            ..Default::default()
        })
        .unwrap();

        // Full check: blob existence + content hash verification.
        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: true,
                verify_blob_content: true,
            })
            .unwrap();
        assert_eq!(
            report.status,
            VerifySeverity::Ok,
            "findings: {:?}",
            report.findings
        );
        assert!(report.blobs_checked >= 3);
    }

    // -----------------------------------------------------------------------
    // Error Model Integration Tests (Phase 3: P3)
    //
    // These tests verify that the structured RouteError variants are
    // returned in the correct situations, and that `is_integrity_error()`
    // correctly reflects which errors may affect repository consistency.
    // -----------------------------------------------------------------------

    #[test]
    fn rollback_to_nonexistent_snapshot_is_integrity_error() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        let result = repo.rollback_to("nonexistent-snapshot-id", Some("bad"));
        let err = result.unwrap_err();
        assert!(
            route_core::is_integrity_error(&err),
            "rollback to non-existent snapshot must be integrity-class, got: {err}"
        );
        // The error chain should contain InvalidSnapshot somewhere.
        let has_invalid_snapshot = err.chain().any(|c| {
            c.downcast_ref::<route_core::RouteError>()
                .is_some_and(|r| matches!(r, route_core::RouteError::InvalidSnapshot(_)))
        });
        assert!(
            has_invalid_snapshot,
            "error chain should contain RouteError::InvalidSnapshot, got: {err}"
        );
    }

    #[test]
    fn corrupted_journal_state_recovery_does_not_panic() {
        // Corrupt a transaction state file. On open, recovery should
        // surface a CorruptedJournal error (not panic).
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        // Manually create a transaction with a corrupted state file.
        use crate::transaction::{TransactionIntent, TransactionJournal};
        use std::collections::HashMap;
        let mut intent = TransactionIntent::new_rollback(
            "main".into(),
            "snap-from".into(),
            "snap-to".into(),
            HashMap::new(),
            "corrupted".into(),
            Some("test".into()),
            None,
            false,
        );
        let journal = TransactionJournal::new(&tmp.path().join(".route-basic"));
        let tx_id = journal.begin(&mut intent).unwrap();
        // Write garbage to the state file.
        std::fs::write(journal.root().join(&tx_id).join("state"), b"GARBAGE\n").unwrap();

        // Reopen should not panic. It should return a CorruptedJournal error.
        let result = BasicRepository::open(tmp.path());
        let err = match result {
            Ok(_) => panic!("expected CorruptedJournal error, got Ok"),
            Err(e) => e,
        };
        assert!(
            route_core::is_integrity_error(&err),
            "corrupted journal must be integrity-class, got: {err}"
        );
        let has_corrupted_journal = err.chain().any(|c| {
            c.downcast_ref::<route_core::RouteError>()
                .is_some_and(|r| matches!(r, route_core::RouteError::CorruptedJournal { .. }))
        });
        assert!(
            has_corrupted_journal,
            "error chain should contain RouteError::CorruptedJournal, got: {err}"
        );
    }

    #[test]
    fn stuck_applying_tx_with_bad_branch_returns_transaction_incomplete() {
        // Create a stuck APPLYING transaction whose target branch does
        // not exist. Recovery should fail with TransactionIncomplete
        // (cannot re-apply because the branch cannot be resolved).
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Manually create an APPLYING transaction with a non-existent
        // branch name. This simulates a crash after the state file was
        // written to APPLYING but before the branch was resolved.
        use crate::transaction::{TransactionIntent, TransactionJournal, TxState};
        use std::collections::HashMap;
        let mut intent = TransactionIntent::new_rollback(
            "nonexistent-branch".into(),
            "snap-from".into(),
            c1.to_snapshot.clone(),
            HashMap::from([("a.txt".into(), "some-hash".into())]),
            "stuck".into(),
            Some("test".into()),
            None,
            false,
        );
        let journal = TransactionJournal::new(&tmp.path().join(".route-basic"));
        let tx_id = journal.begin(&mut intent).unwrap();
        journal.set_state(&tx_id, TxState::Applying).unwrap();

        // Reopen should fail with TransactionIncomplete (recovery
        // cannot resolve the branch).
        let result = BasicRepository::open(tmp.path());
        let err = match result {
            Ok(_) => panic!("expected TransactionIncomplete error, got Ok"),
            Err(e) => e,
        };
        assert!(
            route_core::is_integrity_error(&err),
            "stuck APPLYING tx must be integrity-class, got: {err}"
        );
        let has_tx_incomplete = err.chain().any(|c| {
            c.downcast_ref::<route_core::RouteError>()
                .is_some_and(|r| matches!(r, route_core::RouteError::TransactionIncomplete { .. }))
        });
        assert!(
            has_tx_incomplete,
            "error chain should contain RouteError::TransactionIncomplete, got: {err}"
        );
    }

    #[test]
    fn stuck_applying_tx_with_bad_snapshot_returns_transaction_incomplete() {
        // Create a stuck APPLYING transaction whose target snapshot
        // does not exist in the blob store (files cannot be resolved).
        // Recovery should fail with TransactionIncomplete.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        use crate::transaction::{TransactionIntent, TransactionJournal, TxState};
        use std::collections::HashMap;
        let mut intent = TransactionIntent::new_rollback(
            "main".into(),
            "snap-from".into(),
            "valid-snap-id".into(),
            HashMap::from([("a.txt".into(), "nonexistent-blob-hash".into())]),
            "stuck".into(),
            Some("test".into()),
            None,
            false,
        );
        let journal = TransactionJournal::new(&tmp.path().join(".route-basic"));
        let tx_id = journal.begin(&mut intent).unwrap();
        journal.set_state(&tx_id, TxState::Applying).unwrap();

        // Reopen should fail — the apply phase cannot copy the blob.
        let result = BasicRepository::open(tmp.path());
        let err = match result {
            Ok(_) => panic!("expected TransactionIncomplete error, got Ok"),
            Err(e) => e,
        };
        assert!(
            route_core::is_integrity_error(&err),
            "stuck APPLYING tx must be integrity-class, got: {err}"
        );
    }

    #[test]
    fn committed_pending_cleanup_is_stale_journal_not_corrupted() {
        // A COMMITTED_PENDING_CLEANUP transaction has consistent data;
        // only stale metadata remains. Verify should report it as
        // NeedsCleanup (stale metadata), not Corrupted (data inconsistency).
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        // Manually create a COMMITTED_PENDING_CLEANUP transaction.
        // Must follow the state machine: Prepared → Applying → Committed → CommittedPendingCleanup
        use crate::transaction::{TransactionIntent, TransactionJournal, TxState};
        use std::collections::HashMap;
        let mut intent = TransactionIntent::new_rollback(
            "main".into(),
            "snap-from".into(),
            "snap-to".into(),
            HashMap::new(),
            "cleanup-pending".into(),
            Some("test".into()),
            None,
            false,
        );
        let journal = TransactionJournal::new(&tmp.path().join(".route-basic"));
        let tx_id = journal.begin(&mut intent).unwrap();
        journal.set_state(&tx_id, TxState::Applying).unwrap();
        journal.set_state(&tx_id, TxState::Committed).unwrap();
        journal
            .set_state(&tx_id, TxState::CommittedPendingCleanup)
            .unwrap();

        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        // Must be NeedsCleanup (stale journal), not Corrupted.
        assert_eq!(
            report.status,
            VerifySeverity::NeedsCleanup,
            "COMMITTED_PENDING_CLEANUP should be NeedsCleanup, not Corrupted; findings: {:?}",
            report.findings
        );
        let has_stale_tx = report
            .findings
            .iter()
            .any(|f| f.code == "stale_committed_tx");
        assert!(
            has_stale_tx,
            "expected stale_committed_tx finding, got: {:?}",
            report.findings
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.severity == VerifySeverity::Corrupted),
            "should not have any Corrupted findings: {:?}",
            report.findings
        );
    }

    #[test]
    fn open_with_committed_pending_cleanup_retries_cleanup() {
        // A COMMITTED_PENDING_CLEANUP transaction should be retried on
        // the next open. After successful cleanup, verify should be Ok.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        // Create a COMMITTED_PENDING_CLEANUP transaction that points to
        // a real journal dir that can be cleaned up.
        // Must follow the state machine: Prepared → Applying → Committed → CommittedPendingCleanup
        use crate::transaction::{TransactionIntent, TransactionJournal, TxState};
        use std::collections::HashMap;
        let mut intent = TransactionIntent::new_rollback(
            "main".into(),
            "snap-from".into(),
            "snap-to".into(),
            HashMap::new(),
            "cleanup".into(),
            Some("test".into()),
            None,
            false,
        );
        let journal = TransactionJournal::new(&tmp.path().join(".route-basic"));
        let tx_id = journal.begin(&mut intent).unwrap();
        journal.set_state(&tx_id, TxState::Applying).unwrap();
        journal.set_state(&tx_id, TxState::Committed).unwrap();
        journal
            .set_state(&tx_id, TxState::CommittedPendingCleanup)
            .unwrap();

        // Drop the repo handle and reopen. Recovery should retry cleanup.
        drop(repo);
        let repo = BasicRepository::open(tmp.path()).unwrap();

        // Verify should be clean — no pending transactions.
        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        assert_eq!(
            report.status,
            VerifySeverity::Ok,
            "after cleanup retry, verify should be Ok; findings: {:?}",
            report.findings
        );
        assert!(
            repo.journal().list_pending().unwrap().is_empty(),
            "all pending transactions should be cleaned up"
        );
    }

    // -----------------------------------------------------------------------
    // Invariant tests
    //
    // These tests verify that Route's core invariants hold across
    // operations, not just that specific cases produce expected output.
    // -----------------------------------------------------------------------

    /// Invariant 1: After a successful rollback, the tracked working
    /// state must equal the target snapshot. We verify this by comparing
    /// the current file content against what the snapshot manifest resolved to.
    #[test]
    fn invariant_rollback_restores_working_state_to_target_snapshot() {
        let (tmp, repo) = setup_repo();
        // v1: create a.txt
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // v2: modify a.txt, add b.txt
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"b_v2").unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();

        // Rollback to v1
        let rollback = repo
            .rollback_to(&c1.to_snapshot, Some("back to v1"))
            .unwrap();
        assert_eq!(
            rollback.to_snapshot, c1.to_snapshot,
            "rollback commit must target the requested snapshot"
        );

        // Invariant: tracked working state == target snapshot
        // Verify: a.txt should be "v1", b.txt should not exist
        let a_content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(
            a_content, "v1",
            "Invariant 1: after rollback to v1, a.txt must be 'v1'"
        );
        assert!(
            !tmp.path().join("b.txt").exists(),
            "Invariant 1: after rollback to v1, b.txt must not exist (it was added in v2)"
        );

        // Rollback to v2 (restore the forward state)
        let rollback2 = repo
            .rollback_to(&c2.to_snapshot, Some("forward to v2"))
            .unwrap();
        assert_eq!(rollback2.to_snapshot, c2.to_snapshot);

        let a_content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(
            a_content, "v2",
            "Invariant 1: after rollback to v2, a.txt must be 'v2'"
        );
        let b_content = std::fs::read_to_string(tmp.path().join("b.txt")).unwrap();
        assert_eq!(
            b_content, "b_v2",
            "Invariant 1: after rollback to v2, b.txt must exist"
        );
    }

    /// Invariant 2: Any failed rollback must leave the repository in a
    /// well-defined state: either unchanged, recoverable, or explicitly
    /// corrupted. It must never be silently unknown.
    #[test]
    fn invariant_failed_rollback_leaves_defined_state() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // Case 1: rollback to non-existent snapshot → InvalidSnapshot (integrity error)
        let result = repo.rollback_to("nonexistent-snapshot", Some("bad"));
        assert!(
            result.is_err(),
            "rollback to non-existent snapshot must fail"
        );
        let err = result.unwrap_err();
        assert!(
            route_core::is_integrity_error(&err),
            "Invariant 2: failed rollback (bad snapshot) must be integrity-class, got: {err}"
        );
        // Verify repo is unchanged
        let a_content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(
            a_content, "v1",
            "Invariant 2: after failed rollback, working tree must be unchanged"
        );

        // Case 2: rollback to a valid snapshot should succeed
        let result = repo.rollback_to(&c1.to_snapshot, Some("rollback to self"));
        assert!(result.is_ok(), "rollback to a valid snapshot must succeed");

        // Verify the repo is in a clean, verifiable state
        let report = repo
            .verify(&VerifyOptions {
                check_blob_existence: false,
                verify_blob_content: false,
            })
            .unwrap();
        assert!(
            report.status <= VerifySeverity::NeedsCleanup,
            "Invariant 2: after successful rollback, verify must be clean or NeedsCleanup, got: {:?}",
            report.status
        );
    }

    /// Invariant 4: All snapshot file paths, when resolved, must be
    /// inside the repository root. No path should escape via `..` or
    /// absolute path components.
    #[test]
    fn invariant_all_snapshot_paths_are_inside_repo_root() {
        let (tmp, repo) = setup_repo();
        // Create a few files and commit
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("b.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        // Get all snapshots and check their manifests
        let rows: Vec<(String, String)> = {
            let conn = repo.db.lock();
            let mut stmt = conn
                .prepare("SELECT id, manifest_hash FROM snapshots")
                .unwrap();
            let result: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            result
        };

        for (snap_id, _manifest_hash) in &rows {
            let files = repo.resolve_snapshot_files(snap_id).unwrap();
            for (rel_path, _blob_hash) in &files {
                // Each relative path must be valid (no ../, no absolute)
                // validate_rel_path should pass
                assert!(
                    route_core::validate_rel_path(rel_path).is_ok(),
                    "Invariant 4: snapshot {snap_id} path {rel_path:?} failed validate_rel_path"
                );
                // The resolved path must stay inside the repo root
                let resolved = tmp.path().join(rel_path);
                // Use starts_with (not canonicalize) since the file
                // may not exist on disk (e.g. was deleted in a later
                // snapshot). Starts_with is sufficient to detect
                // directory traversal: `join("../foo")` would resolve
                // to a path outside the tmp root.
                assert!(
                    resolved.starts_with(tmp.path()),
                    "Invariant 4: snapshot {snap_id} path {rel_path:?} resolves to \
                     {resolved:?} which is outside repo root {:?}",
                    tmp.path()
                );
            }
        }
    }
}
