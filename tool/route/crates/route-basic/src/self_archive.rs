//! Route self-archive — a versioned, rollback-able archive of Route's **own**
//! evolving standards.
//!
//! This is the "Route 自存档" domain. It keeps every historical version of
//! Route's canonical spec files (constitution / protocol / reference /
//! goal / workflow) inside the central save root at:
//!
//! ```text
//! Documents/Route/route/
//! ├── index.json          # linear history index (seq -> version)
//! └── versions/
//!     ├── v000001/
//!     │   ├── meta.json   # version metadata + file manifest
//!     │   └── files/      # snapshot of the standard files at that version
//!     │       ├── constitution.md
//!     │       └── ...
//!     └── v000002/ ...
//! ```
//!
//! Snapshots are append-only: a new iteration never overwrites an old one, so
//! every standard can be rolled back to. Deciding to actually adopt a snapshot
//! as the new current standard is a separate, explicit step (`apply`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, time};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const SELF_DIR: &str = "route";
const VERSIONS_DIR: &str = "versions";
const FILES_DIR: &str = "files";
const INDEX_FILE: &str = "index.json";
const META_FILE: &str = "meta.json";

/// Metadata for a single archived self-version (no file contents).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfVersionMeta {
    /// Monotonic sequence number (also the directory key).
    pub seq: u64,
    /// Stable id for this version.
    pub version_id: String,
    /// Route product version this standard belongs to.
    pub route_version: String,
    /// Why this version was archived.
    pub message: String,
    /// Unix millis when archived.
    pub created_at: i64,
    /// Logical file names stored in this snapshot.
    pub files: Vec<String>,
}

/// A full self-version including its archived file contents (loaded on demand).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfVersion {
    pub meta: SelfVersionMeta,
    /// Logical name -> content for every archived standard file.
    pub file_contents: HashMap<String, String>,
}

/// Linear history index of all self-versions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelfArchiveIndex {
    pub versions: Vec<SelfVersionMeta>,
}

/// Root of the Route self-archive: `<archive_root>/route/`
pub fn self_archive_root() -> Result<PathBuf> {
    Ok(crate::game_save::archive_root()?.join(SELF_DIR))
}

fn versions_root() -> Result<PathBuf> {
    Ok(self_archive_root()?.join(VERSIONS_DIR))
}

fn version_dir(seq: u64) -> Result<PathBuf> {
    Ok(versions_root()?.join(format!("v{:06}", seq)))
}

fn index_path() -> Result<PathBuf> {
    Ok(self_archive_root()?.join(INDEX_FILE))
}

fn now_millis_self() -> i64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Assign a new monotonic seq. Sequence is always one past the current latest
/// so versions are never remapped/overwritten (append-only).
fn next_seq(index: &SelfArchiveIndex) -> u64 {
    index.versions.last().map(|v| v.seq + 1).unwrap_or(1)
}

impl SelfArchiveIndex {
    pub fn load() -> Result<Self> {
        let path = index_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = fs::read_to_string(&path)
            .with_context(|| format!("failed to read self-archive index at {}", path.display()))?;
        serde_json::from_str(&json)
            .with_context(|| format!("failed to parse self-archive index at {}", path.display()))
    }

    fn save(&self) -> Result<()> {
        let path = index_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        // Atomic write: a torn `index.json` would make the whole history
        // unreadable. The version dir is written first and its `meta.json` is
        // the commit point; the index update is the last write and must be
        // atomic so readers never observe a half-written index.
        crate::constitutive::write_atomic(&path, json.as_bytes())?;
        Ok(())
    }
}

fn write_version_atomically(
    dir: &Path,
    meta: &SelfVersionMeta,
    files: &HashMap<String, String>,
) -> Result<()> {
    let files_dir = dir.join(FILES_DIR);
    fs::create_dir_all(&files_dir)?;
    for (name, content) in files {
        // Logical name must be a single safe component.
        let safe = crate::game_save::sanitize_dir_name(name);
        fs::write(files_dir.join(safe), content)?;
    }
    let meta_json = serde_json::to_string_pretty(meta)?;
    fs::write(dir.join(META_FILE), meta_json)?;
    Ok(())
}

fn read_version_dir(seq: u64) -> Result<Option<SelfVersion>> {
    let vdir = version_dir(seq)?;
    if !vdir.exists() {
        return Ok(None);
    }
    let meta_json = fs::read_to_string(vdir.join(META_FILE))
        .with_context(|| format!("failed to read meta for self-version {seq}"))?;
    let meta: SelfVersionMeta = serde_json::from_str(&meta_json)?;
    let files_dir = vdir.join(FILES_DIR);
    let mut file_contents: HashMap<String, String> = HashMap::new();
    if files_dir.exists() {
        for name in &meta.files {
            let safe = crate::game_save::sanitize_dir_name(name);
            let p = files_dir.join(&safe);
            if p.exists() {
                if let Ok(c) = fs::read_to_string(&p) {
                    file_contents.insert(name.clone(), c);
                }
            }
        }
    }
    Ok(Some(SelfVersion {
        meta,
        file_contents,
    }))
}

/// Archive the current set of Route standard files as a new self-version.
///
/// `files` maps logical names (e.g. `constitution.md`) to their content. The
/// snapshot is stored under a fresh, monotonically increasing sequence, never
/// overwriting a prior one.
pub fn archive_current(
    files: &HashMap<String, String>,
    route_version: &str,
    message: &str,
) -> Result<SelfVersion> {
    let mut index = SelfArchiveIndex::load()?;
    let seq = next_seq(&index);
    let version_id = format!("self_v{}", seq);
    let mut names: Vec<String> = files.keys().cloned().collect();
    names.sort();

    let meta = SelfVersionMeta {
        seq,
        version_id: version_id.clone(),
        route_version: route_version.to_string(),
        message: message.to_string(),
        created_at: now_millis_self(),
        files: names,
    };
    write_version_atomically(&version_dir(seq)?, &meta, files)?;
    index.versions.push(meta.clone());
    index.save()?;
    Ok(SelfVersion {
        meta,
        file_contents: files.clone(),
    })
}

/// List all archived self-versions, newest last.
pub fn list() -> Result<Vec<SelfVersionMeta>> {
    Ok(SelfArchiveIndex::load()?.versions)
}

/// Get the most recent self-version, if any.
pub fn latest() -> Result<Option<SelfVersion>> {
    let index = SelfArchiveIndex::load()?;
    if let Some(last) = index.versions.last() {
        return read_version_dir(last.seq);
    }
    Ok(None)
}

/// Get a specific self-version by seq (or returned `version_id`).
pub fn get(seq: u64) -> Result<Option<SelfVersion>> {
    read_version_dir(seq)
}

/// Get a specific self-version by its `version_id` (`self_v<seq>`).
pub fn get_by_id(version_id: &str) -> Result<Option<SelfVersion>> {
    let seq_suffix = version_id
        .strip_prefix("self_v")
        .and_then(|s| s.parse::<u64>().ok());
    match seq_suffix {
        Some(seq) => get(seq),
        None => Ok(None),
    }
}

/// Roll the archive back to a requested version.
///
/// This does **not** mutate the repo. It returns the archived files for the
/// orchestrator to decide how to adopt them (e.g. write to the source of truth
/// or stage as a candidate). The archived snapshot itself is untouched.
pub fn apply(seq: u64) -> Result<SelfVersion> {
    match read_version_dir(seq)? {
        Some(v) => Ok(v),
        None => anyhow::bail!("no self-version with seq {seq} (run `route self-archive list`)"),
    }
}

/// Physical location of a version's directory (`versions/v000001/`).
pub fn version_path(seq: u64) -> Result<PathBuf> {
    version_dir(seq)
}

// ---------------------------------------------------------------------------
// Cross-project self-evolution input
// ---------------------------------------------------------------------------

/// Aggregated, cross-project input for one self-evolution iteration.
///
/// Route walks its own central archive (`Documents/Route/`) and summarizes
/// every registered project — its name, known root path, save state, and a
/// preview of its current `.route/constitution.md` — so a harness (the AI) can
/// propose a new standard and verify it. This is **read-only input**; nothing
/// here mutates a project or promotes a new standard (candidate-first policy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfEvolveInput {
    /// Unix millis when the snapshot of project state was taken.
    pub generated_at: i64,
    /// Number of projects read from the central registry.
    pub project_count: usize,
    /// The most recent archived self-version seq (see `self_archive`), if any.
    pub latest_self_version: Option<u64>,
    /// Per-project summaries, sorted by project name.
    pub projects: Vec<SelfEvolveProject>,
}

/// A single project's summary as seen from the central archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfEvolveProject {
    pub project_id: String,
    pub project_name: String,
    /// The most recent known root path of the project (may be gone).
    pub known_path: Option<String>,
    /// Number of saves in the central archive.
    pub save_count: u32,
    /// Latest save id (if archived).
    pub latest_save_id: Option<String>,
    /// First lines of the project's current `.route/constitution.md`, if the
    /// root path still exists on disk.
    pub constitution_preview: Option<String>,
}

/// Collect the cross-project self-evolution input from the central archive.
pub fn collect_self_evolve_input() -> Result<SelfEvolveInput> {
    let registry = crate::game_save::ProjectRegistry::load()?;
    let mut projects = Vec::with_capacity(registry.entries.len());
    for entry in &registry.entries {
        let meta = crate::game_save::ProjectArchiveMeta::load(&entry.project_id)?;
        let known_path = entry.known_paths.first().cloned();
        let constitution_preview = known_path.as_ref().and_then(|p| {
            let dot = std::path::Path::new(p)
                .join(".route")
                .join("constitution.md");
            std::fs::read_to_string(&dot)
                .ok()
                .map(|c| c.lines().take(4).collect::<Vec<_>>().join("\n"))
                .filter(|s| !s.trim().is_empty())
        });
        projects.push(SelfEvolveProject {
            project_id: entry.project_id.clone(),
            project_name: entry.project_name.clone(),
            save_count: meta.as_ref().map(|m| m.save_count).unwrap_or(0),
            latest_save_id: meta.and_then(|m| m.latest_save_id),
            known_path,
            constitution_preview,
        });
    }
    projects.sort_by_key(|a| a.project_name.to_lowercase());
    let latest_self_version = SelfArchiveIndex::load()?.versions.last().map(|v| v.seq);
    Ok(SelfEvolveInput {
        generated_at: now_millis_self(),
        project_count: projects.len(),
        latest_self_version,
        projects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that override the global `ROUTE_ARCHIVE_ROOT` env var must take the
    /// archive-root lock so they never race `game_save`'s disk-level tests,
    /// which resolve the same env var. (See `TEST_ARCHIVE_ROOT_LOCK`.)
    fn with_isolated_root<T>(f: impl FnOnce() -> T) -> T {
        // Single isolation primitive: RAII guard redirects `ROUTE_ARCHIVE_ROOT`
        // to a unique temp root and restores it on Drop (even on panic).
        let _guard = crate::game_save::TestArchiveRootGuard::new();
        f()
    }

    fn sample_files(note: &str) -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert(
            "constitution.md".to_string(),
            format!("principles v1 @ {note}"),
        );
        m.insert("protocol.md".to_string(), format!("protocol v1 @ {note}"));
        m.insert(
            "reference/README.md".to_string(),
            format!("reference @ {note}"),
        );
        m
    }

    #[test]
    fn archive_list_latest_roundtrip() {
        with_isolated_root(|| {
            assert!(list().unwrap().is_empty());
            let v1 = archive_current(&sample_files("A"), "1.0.0", "first").unwrap();
            let v2 = archive_current(&sample_files("B"), "1.0.0", "second").unwrap();
            assert_eq!(v1.meta.seq, 1);
            assert_eq!(v2.meta.seq, 2);
            assert_eq!(v2.meta.files.len(), 3);

            let vers = list().unwrap();
            assert_eq!(vers.len(), 2);
            assert_eq!(vers[0].seq, 1);
            assert_eq!(vers[1].seq, 2);

            // latest returns v2 with its own content.
            let latest = latest().unwrap().unwrap();
            assert_eq!(latest.meta.seq, 2);
            assert_eq!(latest.file_contents["protocol.md"], "protocol v1 @ B");

            // get_by_id works.
            let by_id = get_by_id("self_v1").unwrap().unwrap();
            assert_eq!(by_id.meta.seq, 1);
            assert_eq!(by_id.file_contents["constitution.md"], "principles v1 @ A");
        });
    }

    #[test]
    fn apply_is_append_only_and_idempotent() {
        with_isolated_root(|| {
            archive_current(&sample_files("A"), "1.0.0", "v1").unwrap();
            archive_current(&sample_files("B"), "1.0.0", "v2").unwrap();
            // Applying an old version returns its files without mutating archive.
            let applied = apply(1).unwrap();
            assert_eq!(applied.file_contents["protocol.md"], "protocol v1 @ A");
            // List length is unchanged (append-only; apply never adds).
            assert_eq!(list().unwrap().len(), 2);
            // Applying again is deterministic (idempotent).
            let again = apply(1).unwrap();
            assert_eq!(again.file_contents["constitution.md"], "principles v1 @ A");
        });
    }

    #[test]
    fn apply_unknown_version_errors() {
        with_isolated_root(|| {
            assert!(apply(99).is_err());
        });
    }

    #[test]
    fn collect_input_reads_registered_projects() {
        with_isolated_root(|| {
            // Start empty: no projects registered.
            let empty = collect_self_evolve_input().unwrap();
            assert_eq!(empty.project_count, 0);
            assert!(empty.latest_self_version.is_none());

            // Register one project whose known_path has a .route/constitution.md.
            let root = std::env::temp_dir().join(format!(
                "route_selfevolve_proj_{}",
                route_core::hash::new_id()
            ));
            std::fs::create_dir_all(root.join(".route")).unwrap();
            std::fs::write(
                root.join(".route").join("constitution.md"),
                "## Data safety\nNever delete data.\n## User control\n",
            )
            .unwrap();

            let mut registry = crate::game_save::ProjectRegistry::default();
            registry.upsert(crate::game_save::RegistryEntry {
                project_id: "prj_demo".to_string(),
                project_name: "Demo Project".to_string(),
                known_paths: vec![root.to_string_lossy().replace('\\', "/")],
                created_at: 1,
                last_seen_at: 1,
            });
            registry.save().unwrap();

            let input = collect_self_evolve_input().unwrap();
            assert_eq!(input.project_count, 1);
            assert_eq!(input.projects[0].project_name, "Demo Project");
            assert_eq!(input.projects[0].project_id, "prj_demo");
            assert_eq!(input.projects[0].save_count, 0);
            assert!(input.projects[0].latest_save_id.is_none());
            let preview = input.projects[0].constitution_preview.as_deref().unwrap();
            assert!(preview.contains("## Data safety"));

            let _ = std::fs::remove_dir_all(&root);
        });
    }
}
