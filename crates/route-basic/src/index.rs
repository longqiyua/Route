//! Hidden index file (`<project>/.route/index.json`) for AI agents.
//!
//! AI agents operating the software need a quick way to discover:
//!   - which files are currently tracked and what their state is
//!   - the project's branch layout
//!   - the current `TrackConfig` so the agent honours the user's
//!     filter (suffix / prefix / SHA-256 / OS)
//!   - the canonical system prompt the agent should follow
//!   - the build-in memory buffer so the agent does not commit on
//!     every keystroke
//!
//! All of this is serialised into a single `index.json` file. The
//! file is regenerated on every `commit` and on demand. The agent can
//! read it without having to walk the SQLite database or the blob
//! store.
//!
//! The file is hidden by virtue of living inside `.route/` — the
//! standard convention on Windows / macOS / Linux for "AI-readable
//! metadata for the project".

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use route_core::{content_hash, sha256_hex, RoutePaths};

use crate::models::{AiPrompt, BranchKind, Commit, CommitKind, TrackConfig};
use crate::BasicRepository;

/// One entry in the index's `files` map.
#[derive(Debug, serde::Serialize)]
pub struct IndexFile {
    /// Fast content hash (xxh3-style, 64 hex chars).
    pub content_hash: String,
    /// SHA-256 hash (only present when `verify_sha256` is on; `None`
    /// otherwise). The agent uses this to verify the file bytes
    /// against an external system.
    pub sha256: Option<String>,
    /// Size in bytes.
    pub size: u64,
}

/// Branch summary in the index.
#[derive(Debug, serde::Serialize)]
pub struct IndexBranch {
    pub name: String,
    pub kind: String,
    pub parent: Option<String>,
    pub head_snapshot: Option<String>,
    /// Unix-ms timestamp of the most recent commit on this branch.
    pub last_commit_at: Option<i64>,
}

/// Top-level index file schema. Stable across versions; the `version`
/// field lets the agent detect a breaking change.
#[derive(Debug, serde::Serialize)]
pub struct RouteIndex {
    /// Schema version (1).
    pub version: u32,
    /// Unix-ms when this file was last regenerated.
    pub generated_at: i64,
    /// Absolute path to the project root.
    pub project_path: String,
    /// Currently active branch name.
    pub current_branch: String,
    /// Track configuration (suffixes / prefixes / SHA-256 / memory
    /// buffer / OS enablement).
    pub track: TrackConfig,
    /// The canonical system prompt the agent must follow.
    pub ai_prompt: &'static str,
    /// Branches known to the repo, with their kinds and head snapshot.
    pub branches: Vec<IndexBranch>,
    /// File path → hash / sha256 / size. Sorted by path for stable
    /// diffs across regenerations.
    pub files: BTreeMap<String, IndexFile>,
    /// Most recent N commits (across all branches). N defaults to 50.
    pub recent_commits: Vec<IndexCommit>,
}

/// One entry in `recent_commits`.
#[derive(Debug, serde::Serialize)]
pub struct IndexCommit {
    pub id: String,
    pub short_id: String,
    pub branch: String,
    pub message: String,
    pub author: Option<String>,
    pub operator: Option<String>,
    pub kind: String,
    pub is_checkpoint: bool,
    pub is_ai: bool,
    pub created_at: i64,
    /// Optional body — only included for the most recent 5 commits so
    /// the file does not grow unbounded.
    pub body: Option<String>,
}

/// Build the index for `repo` and return the serialised JSON string.
/// The caller decides whether to write it to disk; we don't touch
/// the file system in this function so tests can inspect the result.
pub fn build_index(repo: &BasicRepository) -> Result<RouteIndex> {
    let project_path = repo.project_path().to_string_lossy().to_string();
    let current_branch = repo
        .get_current_branch_name()
        .unwrap_or_else(|_| "main".into());

    // Branches
    let branches: Vec<IndexBranch> = repo
        .list_branches()
        .unwrap_or_default()
        .into_iter()
        .map(|b| {
            let last = repo
                .list_commits(Some(&b.name), 1)
                .ok()
                .and_then(|mut v| {
                    if v.is_empty() {
                        None
                    } else {
                        v.sort_by(|a, c| c.created_at.cmp(&a.created_at));
                        Some(v[0].created_at)
                    }
                });
            IndexBranch {
                name: b.name,
                kind: b.kind.as_str().to_string(),
                parent: b.parent_branch,
                head_snapshot: b.head_snapshot,
                last_commit_at: last,
            }
        })
        .collect();

    // Files — re-scan the project (honours TrackConfig), then hash.
    let scan = repo.make_scanner().scan()?;
    let mut files: BTreeMap<String, IndexFile> = BTreeMap::new();
    let verify = repo.track_config().verify_sha256;
    for (rel, hash) in &scan.files {
        let abs = scan
            .absolute_paths
            .get(rel)
            .cloned()
            .unwrap_or_else(|| repo.project_path().join(rel));
        let size = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
        let sha = if verify {
            std::fs::read(&abs).ok().map(|b| sha256_hex(&b))
        } else {
            None
        };
        files.insert(
            rel.clone(),
            IndexFile {
                content_hash: hash.clone(),
                sha256: sha,
                size,
            },
        );
    }

    // Recent commits — at most 50, with body only for the most recent 5.
    let mut all = repo
        .list_commits(None, 50)
        .unwrap_or_default();
    all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    // Build a branch_id → branch_name lookup so we can stamp each
    // commit with a friendly name. The Commit struct only carries the
    // branch id; the name lives in the `branches` table.
    let branch_name_for: std::collections::HashMap<String, String> = repo
        .list_branches()
        .unwrap_or_default()
        .into_iter()
        .map(|b| (b.id, b.name))
        .collect();
    let recent: Vec<IndexCommit> = all
        .iter()
        .take(50)
        .enumerate()
        .map(|(i, c): (usize, &Commit)| IndexCommit {
            id: c.id.clone(),
            short_id: route_core::short_id(&c.id),
            branch: branch_name_for
                .get(&c.branch_id)
                .cloned()
                .unwrap_or_else(|| c.branch_id.clone()),
            message: c.message.clone(),
            author: c.author.clone(),
            operator: c.operator.clone(),
            kind: c.kind.as_str().to_string(),
            is_checkpoint: c.is_checkpoint,
            is_ai: c.is_ai,
            created_at: c.created_at,
            body: if i < 5 { c.body.clone() } else { None },
        })
        .collect();

    Ok(RouteIndex {
        version: 1,
        generated_at: now_ms(),
        project_path,
        current_branch,
        track: repo.track_config().clone(),
        ai_prompt: AiPrompt::summary_prompt(),
        branches,
        files,
        recent_commits: recent,
    })
}

/// Serialise the index to a pretty-printed JSON string.
pub fn render_index(idx: &RouteIndex) -> Result<String> {
    Ok(serde_json::to_string_pretty(idx)?)
}

/// Write the index to `<project>/.route/index.json`. Returns the path
/// the file was written to. The route directory is created if it
/// does not exist. The file is overwritten atomically (write to
/// `<file>.tmp`, rename) so an AI agent that reads the file mid-
/// write never sees a partial document.
pub fn write_index(repo: &BasicRepository) -> Result<std::path::PathBuf> {
    let paths = RoutePaths::new(repo.project_path());
    paths.ensure_dirs().ok();
    let idx = build_index(repo)?;
    let json = render_index(&idx)?;

    let final_path = paths.route_dir.join("index.json");
    let tmp = final_path.with_extension("json.tmp");
    if let Some(parent) = tmp.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &final_path)?;
    Ok(final_path)
}

/// Same as `write_index` but writes the file at a caller-supplied
/// path. Used by tests.
pub fn write_index_to(repo: &BasicRepository, dest: &Path) -> Result<()> {
    let idx = build_index(repo)?;
    let json = render_index(&idx)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, json)?;
    Ok(())
}

/// Lightweight helper: ms since epoch. Duplicated here so this module
/// does not need to depend on a specific storage helper.
fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[allow(dead_code)]
fn _kind_str(k: BranchKind) -> &'static str {
    k.as_str()
}

#[allow(dead_code)]
fn _commit_str(k: CommitKind) -> &'static str {
    k.as_str()
}

// Compile-time guarantee that the public types are stable across
// route-core upgrades — content_hash is referenced so a missing
// export is caught at build time.
#[allow(dead_code)]
fn _exports_check() {
    let _ = content_hash(b"");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::TempDir;

    /// Tests in this module touch the DB and Tauri command stack,
    /// which on Windows can blow the 1 MB default test thread stack.
    fn on_big_stack<F: FnOnce() + Send + 'static>(f: F) {
        thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(f)
            .expect("spawn big-stack thread")
            .join()
            .expect("big-stack thread should not panic")
    }

    #[test]
    fn build_and_write_index_roundtrips() {
        on_big_stack(|| {
            let tmp = TempDir::new().unwrap();
            let project = tmp.path().join("p");
            let r = BasicRepository::init(&project)
                .expect("init")
                .with_serial_scanner();
            std::fs::write(project.join("hello.py"), b"print('hi')").unwrap();
            r.commit(crate::CommitOptions {
                message: "add hello".into(),
                ..Default::default()
            })
            .unwrap();
            // The repository is consumed by the above borrows. Reopen
            // and use a fresh handle for the index writer.
            drop(r);
            let r2 = BasicRepository::open(&project)
                .expect("reopen")
                .with_serial_scanner();
            let dest = project.join(".route").join("index.json");
            write_index_to(&r2, &dest).unwrap();
            let raw = std::fs::read_to_string(&dest).unwrap();
            let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(v["version"].as_u64().unwrap(), 1);
            assert!(v["track"]["track_all"].as_bool().unwrap_or(false)
                || v["track"]["track_suffixes"].is_array());
            assert!(v["ai_prompt"].as_str().unwrap().contains("INTENT"));
            assert!(v["files"].get("hello.py").is_some());
        });
    }
}
