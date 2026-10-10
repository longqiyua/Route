//! Read-only Route command history.
//!
//! Route appends one sanitized command outcome per completed CLI invocation.
//! Entries form a SHA-256 chain, while a separate head file detects tail
//! truncation. Both files are marked read-only after every append. The CLI
//! intentionally exposes viewing and verification only; there is no edit,
//! delete, or backfill surface.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const HISTORY_VERSION: u8 = 1;
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteHistoryEvent {
    pub version: u8,
    pub sequence: u64,
    pub timestamp: i64,
    /// Sanitized command path such as `task exec`; arguments are never stored.
    pub operation: String,
    pub success: bool,
    pub previous_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RouteHistoryHead {
    version: u8,
    count: u64,
    hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteHistoryVerification {
    pub valid: bool,
    pub entries: usize,
    pub head_hash: String,
    pub error: Option<String>,
}

#[derive(Serialize)]
struct HashPayload<'a> {
    version: u8,
    sequence: u64,
    timestamp: i64,
    operation: &'a str,
    success: bool,
    previous_hash: &'a str,
}

pub fn route_history_dir(project_root: &Path) -> PathBuf {
    project_root.join(".route").join("history")
}

pub fn route_history_path(project_root: &Path) -> PathBuf {
    route_history_dir(project_root).join("events.jsonl")
}

pub fn route_history_head_path(project_root: &Path) -> PathBuf {
    route_history_dir(project_root).join("head.json")
}

/// Find the nearest Route project without creating or modifying state.
pub fn find_route_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(".route").is_dir() || current.join(".route-basic").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Append a completed Route operation. This is an internal engine surface;
/// there is deliberately no corresponding user-facing append command.
pub fn append_route_history(
    project_root: &Path,
    operation: &str,
    success: bool,
) -> Result<RouteHistoryEvent> {
    let operation = sanitize_operation(operation);
    let dir = route_history_dir(project_root);
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating Route history directory at {}", dir.display()))?;
    // Sequence allocation, event append, and head replacement are one
    // cross-process critical section. Route supports concurrent sessions, so
    // an in-process mutex would not be sufficient here.
    let _append_lock = HistoryAppendLock::acquire(&dir)?;

    let verification = verify_route_history(project_root);
    if !verification.valid {
        anyhow::bail!(
            "Route history integrity check failed; refusing to append: {}",
            verification
                .error
                .unwrap_or_else(|| "unknown integrity error".to_string())
        );
    }

    let previous_hash = if verification.entries == 0 {
        GENESIS_HASH.to_string()
    } else {
        verification.head_hash
    };
    let sequence = verification.entries as u64 + 1;
    let timestamp = route_core::now_millis();
    let hash = event_hash(sequence, timestamp, &operation, success, &previous_hash)?;
    let event = RouteHistoryEvent {
        version: HISTORY_VERSION,
        sequence,
        timestamp,
        operation,
        success,
        previous_hash,
        hash: hash.clone(),
    };

    let events_path = route_history_path(project_root);
    unlock_if_present(&events_path)?;
    let append_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
            .with_context(|| format!("opening Route history at {}", events_path.display()))?;
        serde_json::to_writer(&mut file, &event)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    })();
    lock_if_present(&events_path)?;
    append_result?;

    let head = RouteHistoryHead {
        version: HISTORY_VERSION,
        count: sequence,
        hash,
    };
    let head_path = route_history_head_path(project_root);
    unlock_if_present(&head_path)?;
    let head_result =
        crate::constitutive::write_atomic(&head_path, &serde_json::to_vec_pretty(&head)?);
    lock_if_present(&head_path)?;
    head_result?;

    Ok(event)
}

pub fn load_route_history(project_root: &Path) -> Result<Vec<RouteHistoryEvent>> {
    let path = route_history_path(project_root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("reading Route history at {}", path.display()))?;
    content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line)
                .with_context(|| format!("invalid Route history entry at line {}", index + 1))
        })
        .collect()
}

/// Verify ordering, every hash link, and the separately locked head anchor.
pub fn verify_route_history(project_root: &Path) -> RouteHistoryVerification {
    let events = match load_route_history(project_root) {
        Ok(events) => events,
        Err(error) => return invalid(0, String::new(), error.to_string()),
    };

    let mut previous_hash = GENESIS_HASH.to_string();
    for (index, event) in events.iter().enumerate() {
        let expected_sequence = index as u64 + 1;
        if event.version != HISTORY_VERSION {
            return invalid(
                index,
                previous_hash,
                format!("entry {} has unsupported version", expected_sequence),
            );
        }
        if event.sequence != expected_sequence {
            return invalid(
                index,
                previous_hash,
                format!(
                    "expected sequence {}, found {}",
                    expected_sequence, event.sequence
                ),
            );
        }
        if event.previous_hash != previous_hash {
            return invalid(
                index,
                previous_hash,
                format!("entry {} breaks the previous-hash link", event.sequence),
            );
        }
        let expected_hash = match event_hash(
            event.sequence,
            event.timestamp,
            &event.operation,
            event.success,
            &event.previous_hash,
        ) {
            Ok(hash) => hash,
            Err(error) => return invalid(index, previous_hash, error.to_string()),
        };
        if event.hash != expected_hash {
            return invalid(
                index,
                previous_hash,
                format!("entry {} content hash does not match", event.sequence),
            );
        }
        previous_hash = event.hash.clone();
    }

    let head_path = route_history_head_path(project_root);
    if events.is_empty() && !head_path.exists() {
        return RouteHistoryVerification {
            valid: true,
            entries: 0,
            head_hash: String::new(),
            error: None,
        };
    }
    let head: RouteHistoryHead = match fs::read(&head_path)
        .with_context(|| format!("reading Route history head at {}", head_path.display()))
        .and_then(|bytes| serde_json::from_slice(&bytes).context("parsing Route history head"))
    {
        Ok(head) => head,
        Err(error) => return invalid(events.len(), previous_hash, error.to_string()),
    };
    if head.version != HISTORY_VERSION
        || head.count != events.len() as u64
        || head.hash != previous_hash
    {
        return invalid(
            events.len(),
            previous_hash,
            "history head does not match the event chain (possible truncation)".to_string(),
        );
    }

    RouteHistoryVerification {
        valid: true,
        entries: events.len(),
        head_hash: previous_hash,
        error: None,
    }
}

fn event_hash(
    sequence: u64,
    timestamp: i64,
    operation: &str,
    success: bool,
    previous_hash: &str,
) -> Result<String> {
    let payload = HashPayload {
        version: HISTORY_VERSION,
        sequence,
        timestamp,
        operation,
        success,
        previous_hash,
    };
    Ok(route_core::sha256_hex(&serde_json::to_vec(&payload)?))
}

fn sanitize_operation(operation: &str) -> String {
    let sanitized: String = operation
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .take(120)
        .collect();
    let sanitized = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn unlock_if_present(path: &Path) -> Result<()> {
    if path.exists() {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn lock_if_present(path: &Path) -> Result<()> {
    if path.exists() {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn invalid(entries: usize, head_hash: String, error: String) -> RouteHistoryVerification {
    RouteHistoryVerification {
        valid: false,
        entries,
        head_hash,
        error: Some(error),
    }
}

/// The shared OS file-lock implementation serializes history append on
/// supported local filesystems. Kernel ownership, not age, controls recovery.
struct HistoryAppendLock {
    _guard: crate::ownership_lock::OwnershipLock,
}
impl HistoryAppendLock {
    fn acquire(history_dir: &Path) -> Result<Self> {
        // Never unlink the synchronization path: Windows may retain a
        // delete-pending directory handle while another writer opens it.
        // Existing crash-left legacy directories fail closed, not auto-removed.
        Ok(Self {
            _guard: crate::ownership_lock::OwnershipLock::acquire(
                &history_dir.join(".append-lock"),
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn history_is_hash_chained_locked_and_verifiable() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".route")).unwrap();

        let first = append_route_history(tmp.path(), "status", true).unwrap();
        let second = append_route_history(tmp.path(), "task exec", false).unwrap();
        assert_eq!(first.operation, "status");
        assert_eq!(second.previous_hash, first.hash);

        let verification = verify_route_history(tmp.path());
        assert!(verification.valid, "{:?}", verification.error);
        assert_eq!(verification.entries, 2);
        assert!(fs::metadata(route_history_path(tmp.path()))
            .unwrap()
            .permissions()
            .readonly());
        assert!(fs::metadata(route_history_head_path(tmp.path()))
            .unwrap()
            .permissions()
            .readonly());
    }

    #[test]
    fn history_detects_content_tampering() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".route")).unwrap();
        append_route_history(tmp.path(), "status", true).unwrap();

        let path = route_history_path(tmp.path());
        unlock_if_present(&path).unwrap();
        let content = fs::read_to_string(&path)
            .unwrap()
            .replace("status", "commit");
        fs::write(&path, content).unwrap();
        lock_if_present(&path).unwrap();

        let verification = verify_route_history(tmp.path());
        assert!(!verification.valid);
        assert!(verification.error.unwrap().contains("content hash"));
    }

    #[test]
    fn history_detects_tail_truncation_via_head_anchor() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".route")).unwrap();
        append_route_history(tmp.path(), "status", true).unwrap();
        append_route_history(tmp.path(), "check", true).unwrap();

        let path = route_history_path(tmp.path());
        unlock_if_present(&path).unwrap();
        let first_line = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string();
        fs::write(&path, format!("{first_line}\n")).unwrap();
        lock_if_present(&path).unwrap();

        let verification = verify_route_history(tmp.path());
        assert!(!verification.valid);
        assert!(verification.error.unwrap().contains("head"));
    }

    #[test]
    fn concurrent_process_style_appends_keep_every_entry() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".route")).unwrap();
        let root = std::sync::Arc::new(tmp.path().to_path_buf());

        let handles: Vec<_> = (0..12)
            .map(|index| {
                let root = root.clone();
                std::thread::spawn(move || {
                    append_route_history(&root, &format!("worker {index}"), true).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let verification = verify_route_history(&root);
        assert!(verification.valid, "{:?}", verification.error);
        assert_eq!(verification.entries, 12);
        let events = load_route_history(&root).unwrap();
        assert_eq!(events.len(), 12);
        assert_eq!(events.last().unwrap().sequence, 12);
    }
}
