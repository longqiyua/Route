//! Blob store (content-addressable) and project file scanner.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::hash::{content_hash, content_hash_file};
use crate::paths::RoutePaths;

/// Monotonic counter combined with the PID and a timestamp-y value to
/// produce a unique temp-file suffix per call. Uniqueness only needs to
/// hold within a single directory for the lifetime of a crash window;
/// the counter guarantees that even repeated calls on the same target
/// path do not collide.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Build a unique temp path for `target` in the same directory.
///
/// The temp file is named `<target_file_name>.route-tmp-<pid>-<counter>`.
/// The `.route-tmp-` marker is what `ProjectScanner` keys on to ignore
/// stale temp files left by a crashed atomic write.
fn unique_temp_path(target: &Path) -> PathBuf {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "target".to_string());
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    // Include a per-process id so two parallel processes operating on
    // the same project don't collide. std::process::id is stable.
    let pid = std::process::id();
    let tmp_name = format!("{file_name}.route-tmp-{pid}-{n}");
    dir.join(tmp_name)
}

/// Returns `true` if `file_name` looks like a Route atomic-write temp
/// file (`.route-tmp-...`). Used by the scanner to skip stale temp
/// files left over from a crashed `copy_to_atomic`.
pub fn is_route_temp_file(file_name: &str) -> bool {
    file_name.contains(".route-tmp-")
}

/// Content-addressable blob store. Files stored at `objects/<hash前2>/<hash>`.
pub struct BlobStore {
    paths: RoutePaths,
}

impl BlobStore {
    pub fn new(paths: RoutePaths) -> Self {
        Self { paths }
    }

    /// Store bytes; returns content hash. Idempotent.
    pub fn store(&self, bytes: &[u8]) -> Result<String> {
        let hash = content_hash(bytes);
        let blob_path = self.paths.blob_path(&hash);
        if !blob_path.exists() {
            if let Some(parent) = blob_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::PermissionDenied {
                        anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                            "cannot create blob directory {}: {e}",
                            parent.display()
                        )))
                    } else {
                        anyhow::Error::from(e)
                    }
                })?;
            }
            std::fs::write(&blob_path, bytes).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                        "cannot write blob {}: {e}",
                        blob_path.display()
                    )))
                } else {
                    anyhow::Error::from(e)
                }
            })?;
        }
        Ok(hash)
    }

    /// Store a file by reading it; returns content hash.
    pub fn store_file(&self, abs_path: &Path) -> Result<String> {
        self.store_file_with_size(abs_path).map(|(hash, _)| hash)
    }

    /// Store a file using bounded-memory hashing and filesystem streaming.
    pub fn store_file_with_size(&self, abs_path: &Path) -> Result<(String, u64)> {
        let (hash, size) = content_hash_file(abs_path)?;
        let blob_path = self.paths.blob_path(&hash);
        if !blob_path.exists() {
            if let Some(parent) = blob_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let tmp_path = unique_temp_path(&blob_path);
            std::fs::copy(abs_path, &tmp_path)?;
            if let Ok(file) = std::fs::File::open(&tmp_path) {
                let _ = file.sync_all();
            }
            if let Err(error) = std::fs::rename(&tmp_path, &blob_path) {
                let _ = std::fs::remove_file(&tmp_path);
                // Another process may have stored the same content while we
                // copied. In that case the desired immutable blob exists.
                if !blob_path.exists() {
                    return Err(error.into());
                }
            }
        }
        Ok((hash, size))
    }

    /// Read a blob by hash.
    pub fn read(&self, hash: &str) -> Result<Vec<u8>> {
        std::fs::read(self.paths.blob_path(hash)).context("blob not found")
    }

    /// Copy a blob to a target path (used for full backup to folder).
    pub fn copy_to(&self, hash: &str, target: &Path) -> Result<()> {
        let src = self.paths.blob_path(hash);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                        "cannot create directory {}: {e}",
                        parent.display()
                    )))
                } else {
                    anyhow::Error::from(e)
                }
            })?;
        }
        std::fs::copy(&src, target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                    "cannot copy blob {hash} to {}: {e}",
                    target.display()
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Atomically copy a blob to a target path.
    ///
    /// Writes to a unique temporary file in the same directory, fsyncs
    /// it, then renames it onto the target. This ensures that:
    /// * a crash during the write does not leave a partial or corrupted
    ///   target file (the rename is atomic on the same filesystem on
    ///   both POSIX and Windows),
    /// * two concurrent or interleaved operations on `a` and `a.txt`
    ///   do not collide on the same temp name (each call gets a unique
    ///   temp file via a random suffix),
    /// * a stale temp file from a previous crash is identifiable and
    ///   ignorable by the scanner (temp files use the
    ///   `.route-tmp-<id>` suffix, which `ProjectScanner` ignores).
    ///
    /// **Crash-safety scope:** this is *per-file* atomic. It does NOT
    /// provide cross-file transaction atomicity — see the transaction
    /// journal in `route-basic::transaction` for that.
    pub fn copy_to_atomic(&self, hash: &str, target: &Path) -> Result<()> {
        let src = self.paths.blob_path(hash);
        if let Some(parent) = target.parent() {
            let path = parent.to_path_buf();
            std::fs::create_dir_all(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                        "cannot create directory {}: {e}",
                        path.display()
                    )))
                } else {
                    anyhow::Error::from(e)
                }
            })?;
        }
        // Unique temp name in the same directory. The `.route-tmp-<id>`
        // suffix is ignored by the scanner so a crashed operation does
        // not pollute the next commit's manifest.
        let tmp_target = unique_temp_path(target);
        // Clean up our own stale temp file from a previous crashed call
        // with the same id — extremely unlikely, but cheap.
        let _ = std::fs::remove_file(&tmp_target);

        // Copy blob bytes into the temp file, then fsync so that the
        // rename (the commit point) is backed by durable bytes.
        std::fs::copy(&src, &tmp_target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                    "cannot copy blob {hash} to {}: {e}",
                    tmp_target.display()
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        if let Ok(file) = std::fs::File::open(&tmp_target) {
            // Best-effort fsync. On Windows this calls
            // FlushFileBuffers; errors are non-fatal (the rename is
            // still atomic, just not power-loss-durable).
            let _ = file.sync_all();
        }
        // Atomic rename: target is either fully replaced or unchanged.
        // On Windows, std::fs::rename uses MoveFileExW with
        // MOVEFILE_REPLACE_EXISTING, which is atomic on the same
        // filesystem.
        std::fs::rename(&tmp_target, target).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::Error::new(crate::RouteError::PermissionDenied(format!(
                    "cannot rename temp file to {}: {e}",
                    target.display()
                )))
            } else {
                anyhow::Error::from(e)
            }
        })?;
        Ok(())
    }

    /// Record blob in DB index if not already present.
    pub fn index_in_db(&self, conn: &Connection, hash: &str, size: u64) -> Result<()> {
        conn.execute(
            "INSERT OR IGNORE INTO blobs(hash, size, created) VALUES(?1, ?2, ?3)",
            rusqlite::params![hash, size as i64, now_millis()],
        )?;
        Ok(())
    }
}

/// Result of scanning a project directory.
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Relative path (forward-slash) → blob hash
    pub files: HashMap<String, String>,
    /// Relative path → absolute path
    pub absolute_paths: HashMap<String, PathBuf>,
}

/// Project scanner using `ignore` crate (respects .gitignore + custom patterns).
pub struct ProjectScanner {
    project_path: PathBuf,
    extra_ignores: Vec<String>,
    /// Override the walker's thread count. `None` means "use whatever
    /// the ignore crate picks (typically one per CPU core)". `Some(1)`
    /// forces a single-threaded walk, which dodges the 1 MB default
    /// thread stack on Windows and is also useful in tests.
    thread_override: Option<usize>,
    /// When true, use a simple recursive std::fs walker instead of the
    /// parallel `ignore` walker. The parallel walker spawns worker
    /// threads with the platform default stack (1 MB on Windows), which
    /// trips STATUS_STACK_OVERFLOW on deep call chains. The serial
    /// walker runs entirely on the calling thread.
    serial: bool,
    /// Optional file-tracking filter. When `Some`, only files that
    /// pass `TrackConfig::allows` are emitted by `scan`. When `None`
    /// (the default) every file is emitted, matching the legacy
    /// behaviour.
    track_filter: Option<TrackFilter>,
}

/// Snapshot of `TrackConfig` for use by the scanner. The scanner
/// does not import `route_basic` (to avoid a cycle), so the
/// configuration is mirrored here.
#[derive(Debug, Clone, Default)]
pub struct TrackFilter {
    /// If true, every file is allowed (subject to `extra_ignores`).
    pub track_all: bool,
    /// Lower-case suffixes, each starting with a dot. A file is
    /// allowed when its extension matches one of these. Empty means
    /// "no suffix restriction".
    pub suffixes: Vec<String>,
    /// Lower-case path prefixes. A file is allowed when its
    /// forward-slash relative path starts with one of these. Empty
    /// means "no prefix restriction".
    pub prefixes: Vec<String>,
}

impl TrackFilter {
    /// Decide whether a relative path (forward-slash, relative to
    /// project root) should be tracked under the current config.
    pub fn allows(&self, rel_path: &str) -> bool {
        // Prefix filter takes priority when configured.
        if !self.prefixes.is_empty() {
            let lower = rel_path.to_ascii_lowercase();
            let hit = self
                .prefixes
                .iter()
                .any(|p| lower.starts_with(&p.to_ascii_lowercase()));
            if !hit {
                return false;
            }
        }
        if self.track_all {
            return true;
        }
        // Suffix filter.
        if self.suffixes.is_empty() {
            return false;
        }
        let lower = rel_path.to_ascii_lowercase();
        self.suffixes
            .iter()
            .any(|s| lower.ends_with(&s.to_ascii_lowercase()))
    }
}

/// Compute the SHA-256 hex digest of the given bytes. Returned alongside
/// the fast `content_hash` when `verify_sha256` is enabled on the
/// project. Cheap (a few microseconds for typical source files) so the
/// overhead is negligible.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

impl ProjectScanner {
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        Self {
            project_path: project_path.as_ref().to_path_buf(),
            extra_ignores: vec![
                ".route-basic".to_string(),
                ".route".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
                ".git".to_string(),
            ],
            thread_override: None,
            serial: false,
            track_filter: None,
        }
    }

    pub fn with_extra_ignores(mut self, ignores: Vec<String>) -> Self {
        self.extra_ignores.extend(ignores);
        self
    }

    /// Install a `TrackFilter` so the scanner only emits files that
    /// match the project's `TrackConfig` (suffix / prefix rules).
    /// Pass `None` to disable the filter and emit every file.
    pub fn with_track_filter(mut self, filter: Option<TrackFilter>) -> Self {
        self.track_filter = filter;
        self
    }

    /// Force the underlying `ignore` walker to use `n` threads. Pass
    /// `Some(1)` to run on the calling thread (no worker spawn) — this
    /// is what tests use to dodge the Windows 1 MB stack limit.
    pub fn with_threads(mut self, n: usize) -> Self {
        self.thread_override = Some(n);
        self
    }

    /// Use a non-parallel recursive `std::fs` walker. This is slower on
    /// huge trees but completely avoids the worker-thread spawn that
    /// the parallel `ignore` walker does, which is critical for tests
    /// on Windows where the 1 MB default stack is too tight.
    pub fn with_serial_walker(mut self) -> Self {
        self.serial = true;
        self
    }

    /// Scan and hash all files. Returns ScanResult.
    pub fn scan(&self) -> Result<ScanResult> {
        let mut files = HashMap::new();
        let mut absolute_paths = HashMap::new();

        if self.serial {
            self.scan_serial(&mut files, &mut absolute_paths)?;
        } else {
            self.scan_parallel(&mut files, &mut absolute_paths)?;
        }

        Ok(ScanResult {
            files,
            absolute_paths,
        })
    }

    /// Recursive std::fs scan. Honours the same `extra_ignores` set as
    /// the parallel walker so behaviour is consistent.
    fn scan_serial(
        &self,
        files: &mut HashMap<String, String>,
        absolute_paths: &mut HashMap<String, PathBuf>,
    ) -> Result<()> {
        let root = self.project_path.clone();
        let mut stack: Vec<PathBuf> = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)
                .with_context(|| format!("read_dir failed for {}", dir.display()))?
            {
                let entry = entry?;
                let path = entry.path();
                let file_type = match entry.file_type() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                if file_type.is_dir() {
                    // Honour our ignore list. This mirrors the parallel
                    // walker's behaviour for the patterns we set up in
                    // `new()`.
                    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                        if self.extra_ignores.iter().any(|pat| pat == name) {
                            continue;
                        }
                    }
                    stack.push(path);
                } else if file_type.is_file() {
                    let rel = match path.strip_prefix(&root) {
                        Ok(r) => r,
                        Err(_) => continue,
                    };
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    // Skip Route atomic-write temp files left by a
                    // crashed `copy_to_atomic`. Without this, a stale
                    // `<name>.route-tmp-...` would be scanned as a real
                    // file and pollute the next commit's manifest.
                    if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                        if is_route_temp_file(fname) {
                            continue;
                        }
                    }
                    // Skip symlinks — Route tracks plain files only.
                    // Following a symlink during scan would store the
                    // target's content, and a later rollback could write
                    // through the symlink to outside the project.
                    if let Ok(md) = std::fs::symlink_metadata(&path) {
                        if md.file_type().is_symlink() {
                            continue;
                        }
                    }
                    // Apply the user's track filter (suffix / prefix).
                    if let Some(f) = &self.track_filter {
                        if !f.allows(&rel_str) {
                            continue;
                        }
                    }
                    let hash = match content_hash_file(&path) {
                        Ok((hash, _)) => hash,
                        Err(_) => continue,
                    };
                    files.insert(rel_str.clone(), hash);
                    absolute_paths.insert(rel_str, path);
                }
            }
        }
        Ok(())
    }

    /// Default parallel walker using the `ignore` crate.
    fn scan_parallel(
        &self,
        files: &mut HashMap<String, String>,
        absolute_paths: &mut HashMap<String, PathBuf>,
    ) -> Result<()> {
        let mut builder = ignore::WalkBuilder::new(&self.project_path);
        builder
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .git_global(true)
            .standard_filters(true);
        if let Some(n) = self.thread_override {
            builder.threads(n);
        }

        let mut overrides = ignore::overrides::OverrideBuilder::new(&self.project_path);
        for pat in &self.extra_ignores {
            overrides.add(&format!("!{pat}"))?;
        }
        builder.overrides(overrides.build()?);

        for entry in builder.build() {
            let entry = entry?;
            // Skip symlinks — Route tracks plain files only. The `ignore`
            // crate follows symlinks by default; we must explicitly drop
            // them here so a later rollback can't write through a
            // symlink to outside the project.
            let ftype = match entry.file_type() {
                Some(t) => t,
                None => continue,
            };
            if !ftype.is_file() {
                continue;
            }
            let abs = entry.path();
            let rel = abs.strip_prefix(&self.project_path)?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            // Skip Route atomic-write temp files left by a crashed
            // `copy_to_atomic`.
            if let Some(fname) = abs.file_name().and_then(|s| s.to_str()) {
                if is_route_temp_file(fname) {
                    continue;
                }
            }
            // Apply the user's track filter (suffix / prefix).
            if let Some(f) = &self.track_filter {
                if !f.allows(&rel_str) {
                    continue;
                }
            }
            let (hash, _) = content_hash_file(abs)?;
            files.insert(rel_str.clone(), hash);
            absolute_paths.insert(rel_str, abs.to_path_buf());
        }
        Ok(())
    }
}

/// Compute diff between two manifest maps.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ManifestDiff {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
}

impl ManifestDiff {
    pub fn compute(previous: &HashMap<String, String>, current: &HashMap<String, String>) -> Self {
        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut removed = Vec::new();

        for (path, hash) in current {
            match previous.get(path) {
                None => added.push(path.clone()),
                Some(prev_hash) if prev_hash != hash => modified.push(path.clone()),
                _ => {}
            }
        }
        for path in previous.keys() {
            if !current.contains_key(path) {
                removed.push(path.clone());
            }
        }

        added.sort();
        modified.sort();
        removed.sort();
        Self {
            added,
            modified,
            removed,
        }
    }

    pub fn total(&self) -> usize {
        self.added.len() + self.modified.len() + self.removed.len()
    }
}

/// Connection guard that wraps a rusqlite Connection with a Mutex (for multi-thread access).
pub struct DbConnection {
    pub conn: Mutex<Connection>,
}

impl DbConnection {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        crate::schema::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("DB mutex poisoned")
    }
}

pub fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn blob_store_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let paths = RoutePaths::new(tmp.path());
        paths.ensure_dirs().unwrap();
        let store = BlobStore::new(paths);
        let hash = store.store(b"hello world").unwrap();
        let read_back = store.read(&hash).unwrap();
        assert_eq!(read_back, b"hello world");
    }

    #[test]
    fn scanner_respects_route_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"a").unwrap();
        std::fs::create_dir(tmp.path().join(".route-basic")).unwrap();
        std::fs::write(tmp.path().join(".route-basic").join("db.sqlite"), b"x").unwrap();

        let scanner = ProjectScanner::new(tmp.path());
        let result = scanner.scan().unwrap();
        assert!(result.files.contains_key("a.txt"));
        assert!(!result.files.contains_key(".route-basic/db.sqlite"));
    }

    #[test]
    fn diff_detects_changes() {
        let mut prev = HashMap::new();
        prev.insert("a".to_string(), "h1".to_string());
        prev.insert("b".to_string(), "h2".to_string());
        let mut curr = HashMap::new();
        curr.insert("a".to_string(), "h1".to_string());
        curr.insert("b".to_string(), "h3".to_string());
        curr.insert("c".to_string(), "h4".to_string());

        let diff = ManifestDiff::compute(&prev, &curr);
        assert_eq!(diff.added, vec!["c"]);
        assert_eq!(diff.modified, vec!["b"]);
        assert_eq!(diff.removed, Vec::<String>::new());
    }
}
