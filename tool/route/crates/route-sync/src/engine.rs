//! Sync engine — orchestrates file synchronization.

use std::io::{Read, Result as IoResult};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use chrono::Utc;

use route_plugins::{Event, EventBus, PluginContext};

use crate::config::SyncTarget;
use crate::modes::{ConflictResolution, SyncMode};
use crate::transports::{create_transport, FileEntry, Transport};

/// Statistics from a sync run.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SyncStats {
    pub files_scanned: usize,
    pub files_copied: usize,
    pub files_skipped: usize,
    pub files_deleted: usize,
    pub conflicts_resolved: usize,
    pub errors: usize,
    pub bytes_copied: u64,
}

/// Result of a sync run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResult {
    pub target_name: String,
    pub mode: String,
    pub stats: SyncStats,
    pub timestamp: i64,
    pub error: Option<String>,
}

/// Main sync engine.
pub struct SyncEngine {
    /// Optional event bus. Set once via `set_event_bus`; subsequent reads
    /// go through `emit_event` which no-ops when the bus is unset.
    event_bus: OnceLock<Arc<EventBus>>,
}

impl SyncEngine {
    pub fn new() -> Self {
        Self {
            event_bus: OnceLock::new(),
        }
    }

    /// Attach an event bus. Once set, the bus cannot be replaced (matches
    /// the typical lifecycle: CLI/GUI constructs the bus once at startup).
    /// Returns `Err` if a bus is already attached.
    pub fn set_event_bus(&self, bus: Arc<EventBus>) -> anyhow::Result<()> {
        self.event_bus
            .set(bus)
            .map_err(|_| anyhow::anyhow!("event bus already attached"))
    }

    /// Whether an event bus is attached.
    pub fn has_event_bus(&self) -> bool {
        self.event_bus.get().is_some()
    }

    fn emit_event(&self, event: Event) {
        if let Some(bus) = self.event_bus.get() {
            // SyncEngine has no natural project_path — pass empty context.
            let ctx = PluginContext::new(Path::new(""));
            let result = bus.publish(&event, &ctx);
            if !result.is_ok() {
                tracing::warn!(
                    target: "route::sync::events",
                    delivered = result.delivered,
                    failed = result.failed,
                    "plugin dispatch had failures: {:?}",
                    result.errors
                );
            }
        }
    }

    /// Run a sync for the given target.
    pub fn run(&self, target: &SyncTarget) -> SyncResult {
        let timestamp = Utc::now().timestamp_millis();
        let mode_str = target.mode.as_str().to_string();
        let transport_str = format!("{:?}", target.transport).to_lowercase();

        // Emit SyncStarted before doing any work.
        self.emit_event(Event::SyncStarted {
            target_name: target.name.clone(),
            mode: mode_str.clone(),
            transport: transport_str.clone(),
            timestamp,
        });

        let result = Self::sync_inner(target);

        let (stats, error) = match result {
            Ok(stats) => (stats, None),
            Err(e) => (SyncStats::default(), Some(e.to_string())),
        };

        // Emit SyncCompleted after the run, regardless of success/failure.
        self.emit_event(Event::SyncCompleted {
            target_name: target.name.clone(),
            mode: mode_str.clone(),
            transport: transport_str.clone(),
            files_scanned: stats.files_scanned,
            files_copied: stats.files_copied,
            files_skipped: stats.files_skipped,
            files_deleted: stats.files_deleted,
            conflicts_resolved: stats.conflicts_resolved,
            errors: stats.errors,
            bytes_copied: stats.bytes_copied,
            error_message: error.clone(),
            timestamp: Utc::now().timestamp_millis(),
        });

        SyncResult {
            target_name: target.name.clone(),
            mode: mode_str,
            stats,
            timestamp,
            error,
        }
    }

    fn sync_inner(target: &SyncTarget) -> Result<SyncStats> {
        if !target.enabled {
            return Ok(SyncStats::default());
        }

        let transport = create_transport(target.transport, target.credentials.as_ref());
        let stats = match target.mode {
            SyncMode::Mirror => Self::sync_mirror(target, transport.as_ref())?,
            SyncMode::Backup => Self::sync_backup(target, transport.as_ref())?,
            SyncMode::Archive => Self::sync_archive(target, transport.as_ref())?,
        };
        Ok(stats)
    }

    // -----------------------------------------------------------------------
    // Mirror mode: source is truth, target is overwritten completely.
    // -----------------------------------------------------------------------

    fn sync_mirror(target: &SyncTarget, transport: &dyn Transport) -> Result<SyncStats> {
        let mut stats = SyncStats::default();
        let mut source_files = std::collections::HashSet::new();

        // Copy source files as they are discovered instead of retaining every
        // absolute path and metadata record for the duration of the sync.
        visit_source(&target.source, &target.ignore_patterns, |entry| {
            stats.files_scanned += 1;
            match transport.send_file(&entry, &target.destination) {
                Ok(()) => {
                    stats.files_copied += 1;
                    stats.bytes_copied += entry.size;
                }
                Err(_) => {
                    stats.errors += 1;
                }
            }
            source_files.insert(entry.rel_path);
        })?;

        // Delete files in dest that aren't in source
        let dest_files = transport.list_dest(&target.destination)?;
        for dest_rel in &dest_files {
            if !source_files.contains(dest_rel) {
                if transport.delete_file(dest_rel, &target.destination).is_ok() {
                    stats.files_deleted += 1;
                }
            }
        }

        Ok(stats)
    }

    // -----------------------------------------------------------------------
    // Backup mode: append-only, never delete, handle conflicts.
    // -----------------------------------------------------------------------

    fn sync_backup(target: &SyncTarget, transport: &dyn Transport) -> Result<SyncStats> {
        let mut stats = SyncStats::default();
        let dest_files: std::collections::HashSet<String> = transport
            .list_dest(&target.destination)?
            .into_iter()
            .collect();

        visit_source(&target.source, &target.ignore_patterns, |entry| {
            stats.files_scanned += 1;
            let exists = dest_files.contains(&entry.rel_path);

            if !exists {
                // New file — copy directly
                match transport.send_file(&entry, &target.destination) {
                    Ok(()) => {
                        stats.files_copied += 1;
                        stats.bytes_copied += entry.size;
                    }
                    Err(_) => stats.errors += 1,
                }
            } else {
                // File exists — check if different
                let dest_path = target.destination.join(&entry.rel_path);
                let is_different = files_differ(&entry.source_abs, &dest_path);

                if is_different {
                    match target.conflict {
                        ConflictResolution::KeepBoth => {
                            // Copy with a suffix
                            let suffixed =
                                format!("{}.conflict.{}", entry.rel_path, timestamp_short());
                            let suffixed_entry = FileEntry {
                                rel_path: suffixed,
                                source_abs: entry.source_abs.clone(),
                                size: entry.size,
                            };
                            match transport.send_file(&suffixed_entry, &target.destination) {
                                Ok(()) => {
                                    stats.files_copied += 1;
                                    stats.conflicts_resolved += 1;
                                    stats.bytes_copied += entry.size;
                                }
                                Err(_) => stats.errors += 1,
                            }
                        }
                        ConflictResolution::SkipExisting => {
                            stats.files_skipped += 1;
                        }
                        ConflictResolution::Overwrite => {
                            match transport.send_file(&entry, &target.destination) {
                                Ok(()) => {
                                    stats.files_copied += 1;
                                    stats.conflicts_resolved += 1;
                                    stats.bytes_copied += entry.size;
                                }
                                Err(_) => stats.errors += 1,
                            }
                        }
                    }
                } else {
                    stats.files_skipped += 1;
                }
            }
        })?;

        Ok(stats)
    }

    // -----------------------------------------------------------------------
    // Archive mode: create timestamped snapshot, apply retention.
    // -----------------------------------------------------------------------

    fn sync_archive(target: &SyncTarget, transport: &dyn Transport) -> Result<SyncStats> {
        let mut stats = SyncStats::default();

        // Create timestamped snapshot folder
        let snapshot_name = chrono::Utc::now()
            .format(&target.archive_rule.folder_format)
            .to_string();
        let snapshot_dest = target.destination.join(&snapshot_name);

        visit_source(&target.source, &target.ignore_patterns, |entry| {
            stats.files_scanned += 1;
            match transport.send_file(&entry, &snapshot_dest) {
                Ok(()) => {
                    stats.files_copied += 1;
                    stats.bytes_copied += entry.size;
                }
                Err(_) => stats.errors += 1,
            }
        })?;

        // Apply retention: remove old snapshots
        Self::apply_retention(target, &snapshot_name)?;

        Ok(stats)
    }

    /// Remove old archive snapshots based on retention rules.
    fn apply_retention(target: &SyncTarget, _current: &str) -> Result<()> {
        let dest = &target.destination;
        if !dest.exists() {
            return Ok(());
        }

        let mut snapshots: Vec<(String, i64)> = Vec::new();
        for entry in std::fs::read_dir(dest)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let modified = entry.metadata()?.modified()?;
                let ts = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                snapshots.push((name, ts));
            }
        }

        // Sort by timestamp descending (newest first)
        snapshots.sort_by(|a, b| b.1.cmp(&a.1));

        // Apply max_snapshots
        if let Some(max) = target.archive_rule.max_snapshots {
            for (_, ts) in snapshots.iter().skip(max) {
                // Find the folder name by ts
                if let Ok(entries) = std::fs::read_dir(dest) {
                    for e in entries.flatten() {
                        if e.metadata()
                            .ok()
                            .map(|m| m.modified().ok())
                            .flatten()
                            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as i64)
                            == Some(*ts)
                        {
                            let _ = std::fs::remove_dir_all(e.path());
                        }
                    }
                }
            }
        }

        // Apply max_age_days
        if let Some(max_days) = target.archive_rule.max_age_days {
            let cutoff = Utc::now().timestamp_millis() - (max_days as i64 * 86400 * 1000);
            for (name, ts) in &snapshots {
                if *ts < cutoff {
                    let _ = std::fs::remove_dir_all(dest.join(name));
                }
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Visit source files one at a time so large trees do not require an in-memory
/// `FileEntry` for every path before any work can begin.
fn visit_source<F>(source: &Path, ignore_patterns: &[String], mut visitor: F) -> Result<()>
where
    F: FnMut(FileEntry),
{
    if !source.exists() {
        return Ok(());
    }
    visit_dir(source, source, ignore_patterns, &mut visitor)
}

fn visit_dir<F>(
    root: &Path,
    current: &Path,
    ignore_patterns: &[String],
    visitor: &mut F,
) -> Result<()>
where
    F: FnMut(FileEntry),
{
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Always ignore these
        if name_str.starts_with('.') || name_str == "node_modules" || name_str == "target" {
            continue;
        }

        // Check ignore patterns
        if ignore_patterns.iter().any(|p| name_str == *p) {
            continue;
        }

        if path.is_dir() {
            visit_dir(root, &path, ignore_patterns, visitor)?;
        } else {
            let rel = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            visitor(FileEntry {
                rel_path: rel,
                source_abs: path,
                size,
            });
        }
    }
    Ok(())
}

/// Check if two files differ (by size or content hash).
fn files_differ(a: &Path, b: &Path) -> bool {
    let a_meta = std::fs::metadata(a);
    let b_meta = std::fs::metadata(b);
    match (a_meta, b_meta) {
        (Ok(am), Ok(bm)) => {
            if am.len() != bm.len() {
                return true;
            }
            compare_file_contents(a, b, am.len()).unwrap_or(true)
        }
        _ => true,
    }
}

/// Compare equal-length files with bounded memory. The previous implementation
/// loaded both files completely, making peak memory roughly twice the largest
/// file size during backup conflict checks.
fn compare_file_contents(a: &Path, b: &Path, len: u64) -> IoResult<bool> {
    const COMPARE_BUFFER_SIZE: usize = 64 * 1024;

    let mut a_file = std::fs::File::open(a)?;
    let mut b_file = std::fs::File::open(b)?;
    let mut a_buffer = [0_u8; COMPARE_BUFFER_SIZE];
    let mut b_buffer = [0_u8; COMPARE_BUFFER_SIZE];
    let mut remaining = len;

    while remaining > 0 {
        let chunk_len = remaining.min(COMPARE_BUFFER_SIZE as u64) as usize;
        a_file.read_exact(&mut a_buffer[..chunk_len])?;
        b_file.read_exact(&mut b_buffer[..chunk_len])?;
        if a_buffer[..chunk_len] != b_buffer[..chunk_len] {
            return Ok(true);
        }
        remaining -= chunk_len as u64;
    }

    Ok(false)
}

fn timestamp_short() -> String {
    chrono::Utc::now().format("%Y%m%d%H%M%S").to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SyncTarget;
    use tempfile::TempDir;

    fn make_target(src: &Path, dst: &Path, mode: SyncMode) -> SyncTarget {
        SyncTarget {
            name: "test".into(),
            source: src.to_path_buf(),
            destination: dst.to_path_buf(),
            mode,
            transport: crate::transports::TransportType::Local,
            conflict: Default::default(),
            archive_rule: Default::default(),
            ignore_patterns: vec![],
            enabled: true,
            credentials: None,
        }
    }

    fn run_target(target: &SyncTarget) -> SyncResult {
        SyncEngine::new().run(target)
    }

    #[test]
    fn mirror_copies_and_deletes() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        // Source has a.txt, b.txt
        std::fs::write(src.join("a.txt"), b"hello").unwrap();
        std::fs::write(src.join("b.txt"), b"world").unwrap();

        // Dest has c.txt (should be deleted)
        std::fs::write(dst.join("c.txt"), b"old").unwrap();

        let target = make_target(&src, &dst, SyncMode::Mirror);
        let result = run_target(&target);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.stats.files_copied, 2);
        assert_eq!(result.stats.files_deleted, 1);
        assert!(dst.join("a.txt").exists());
        assert!(dst.join("b.txt").exists());
        assert!(!dst.join("c.txt").exists());
    }

    #[test]
    fn backup_appends_without_deleting() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("a.txt"), b"hello").unwrap();

        // Dest has old.txt (should NOT be deleted in backup mode)
        std::fs::write(dst.join("old.txt"), b"old").unwrap();

        let target = make_target(&src, &dst, SyncMode::Backup);
        let result = run_target(&target);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(dst.join("a.txt").exists());
        assert!(
            dst.join("old.txt").exists(),
            "backup mode must not delete files"
        );
    }

    #[test]
    fn backup_conflict_keep_both() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();

        std::fs::write(src.join("a.txt"), b"new content").unwrap();
        std::fs::write(dst.join("a.txt"), b"old content").unwrap();

        let mut target = make_target(&src, &dst, SyncMode::Backup);
        target.conflict = ConflictResolution::KeepBoth;

        let result = run_target(&target);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.stats.conflicts_resolved, 1);
        // Original file kept
        assert_eq!(
            std::fs::read_to_string(dst.join("a.txt")).unwrap(),
            "old content"
        );
        // Conflict copy created
        let entries: Vec<_> = std::fs::read_dir(&dst).unwrap().collect();
        let has_conflict = entries.iter().any(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("a.txt.conflict.")
        });
        assert!(has_conflict, "should have a conflict copy");
    }

    #[test]
    fn file_comparison_is_chunked_and_exact() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        let mut content = vec![0x5a; 3 * 64 * 1024 + 17];

        std::fs::write(&a, &content).unwrap();
        std::fs::write(&b, &content).unwrap();
        assert!(!files_differ(&a, &b));

        // Change a byte after multiple comparison buffers while preserving
        // the file length, exercising the bounded-memory content path.
        let last = content.len() - 1;
        content[last] ^= 0xff;
        std::fs::write(&b, &content).unwrap();
        assert!(files_differ(&a, &b));
    }

    #[test]
    fn archive_creates_snapshot_folder() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();

        std::fs::write(src.join("a.txt"), b"hello").unwrap();
        std::fs::write(src.join("b.txt"), b"world").unwrap();

        let target = make_target(&src, &dst, SyncMode::Archive);
        let result = run_target(&target);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.stats.files_copied, 2);

        // Should have one timestamped subfolder with the files
        let snapshots: Vec<_> = std::fs::read_dir(&dst).unwrap().collect();
        assert_eq!(snapshots.len(), 1, "should have 1 snapshot folder");
        let snap_dir = snapshots[0].as_ref().unwrap().path();
        assert!(snap_dir.join("a.txt").exists());
        assert!(snap_dir.join("b.txt").exists());
    }

    #[test]
    fn archive_retention_removes_old() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), b"x").unwrap();

        let mut target = make_target(&src, &dst, SyncMode::Archive);
        target.archive_rule.max_snapshots = Some(2);

        // Run 3 times — should keep only 2 newest
        run_target(&target);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        run_target(&target);
        std::thread::sleep(std::time::Duration::from_millis(1100));
        run_target(&target);

        let snapshots: Vec<_> = std::fs::read_dir(&dst).unwrap().collect();
        assert_eq!(
            snapshots.len(),
            2,
            "should retain only 2 snapshots, got {}",
            snapshots.len()
        );
    }
}
