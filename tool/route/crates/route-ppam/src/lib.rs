//! Route PPAM Bridge — pluggable document enhancement embedding.
//!
//! # What is PPAM?
//!
//! PPAM (可插拔文档增强组件) is a standalone document enhancement system.
//! This crate bridges Route's core capabilities — version management, project
//! structure memory, and fuzzy search — into PPAM's document pipeline.
//!
//! # Design
//!
//! PPAM documents are tracked through Route's snapshot model:
//! - Each document version = a Route snapshot
//! - Document relationships = Route's project memory
//! - Document retrieval = Route's fuzzy search engine
//!
//! # Usage
//!
//! ```rust,no_run
//! use route_ppam::PpamBridge;
//! use std::path::Path;
//!
//! let bridge = PpamBridge::new(Path::new("/path/to/project"));
//! // Requires an initialized Route repository
//! ```

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use route_basic::repository::{BasicRepository, CommitOptions};
use route_memory::{MemoryStore, MemoryTier, ProjectStructure, StructureSnapshot};

/// A simple wrapper so we can put BasicRepository in a Mutex.
/// BasicRepository is !Clone + !Debug, so we store it behind a lock.
type RepoSlot = Option<BasicRepository>;

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Field selector for batch_context — lets PPAM request only what it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextField {
    RemoteUrl,
    FileCount,
    MemoryEntries,
    RecentCommits,
    CurrentBranch,
    Memory,
    FileTree,
    Conversation,
}

/// A single line in a document diff.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamDiffLine {
    /// "+" / "-" / " "
    pub kind: char,
    /// Line number in the original (0 for inserted lines).
    pub old_lineno: usize,
    /// Line number in the new (0 for deleted lines).
    pub new_lineno: usize,
    /// The line content (without the kind prefix).
    pub content: String,
}

/// A directional relationship between two PPAM documents.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamDocRelation {
    /// Source document path (relative to project root).
    pub from: String,
    /// Target document path (relative to project root).
    pub to: String,
    /// Relationship label, e.g. "references", "implements", "extends".
    pub relation: String,
    /// Unix timestamp when the relation was recorded.
    pub created_at: i64,
}

/// PPAM bridge — connects Route's core to PPAM's document enhancement pipeline.
pub struct PpamBridge {
    /// Project root path.
    project_path: PathBuf,
    /// Cached repository handle (lazily opened, cleared on write).
    repo_cache: Mutex<RepoSlot>,
    /// Cached memory store (lazily loaded, cleared on write).
    memory_cache: Mutex<Option<MemoryStore>>,
}

/// Project context snapshot for PPAM.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamProjectContext {
    pub project_path: String,
    pub ppam_remote_url: Option<String>,
    pub file_count: usize,
    pub memory_entries: usize,
    pub recent_commits: Vec<PpamCommit>,
    pub current_branch: String,
    pub memory: Vec<PpamMemoryEntry>,
    pub file_tree: Vec<String>,
    pub truncated: bool,
    /// Number of conversation sessions.
    pub conversation_count: usize,
}

/// Commit summary for PPAM.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamCommit {
    pub id: String,
    pub message: String,
    pub created_at: i64,
    pub branch: String,
}

/// Memory entry for PPAM.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamMemoryEntry {
    pub key: String,
    pub value: String,
    pub tier: String,
    pub tags: Vec<String>,
}

/// Document version tracked by Route.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamDocVersion {
    pub snapshot_id: String,
    pub commit_id: String,
    pub message: String,
    pub created_at: i64,
    pub file_path: String,
}

/// Search result from Route's fuzzy engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PpamSearchResult {
    pub text: String,
    pub score: f64,
    pub source: String,
}

// -----------------------------------------------------------------------
// Relation storage
// -----------------------------------------------------------------------

fn relations_path(project: &Path) -> PathBuf {
    project.join(".route").join("ppam-relations.json")
}

fn load_relations(project: &Path) -> Vec<PpamDocRelation> {
    let p = relations_path(project);
    if p.exists() {
        std::fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        vec![]
    }
}

fn save_relations(project: &Path, relations: &[PpamDocRelation]) -> anyhow::Result<()> {
    let p = relations_path(project);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, serde_json::to_string_pretty(relations)?)?;
    Ok(())
}

// -----------------------------------------------------------------------
// Simple line-by-line diff
// -----------------------------------------------------------------------

fn simple_diff(old: &str, new: &str) -> Vec<PpamDiffLine> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    // Wagner–Fischer for LCS-based diff (simplified)
    let (m, n) = (old_lines.len(), new_lines.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            if old_lines[i - 1] == new_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] = 1 + dp[i - 1][j].min(dp[i][j - 1]);
            }
        }
    }

    // Backtrack
    let mut result: Vec<PpamDiffLine> = Vec::new();
    let mut i = m;
    let mut j = n;
    let mut stack: Vec<PpamDiffLine> = Vec::new();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            stack.push(PpamDiffLine {
                kind: ' ',
                old_lineno: i,
                new_lineno: j,
                content: old_lines[i - 1].to_string(),
            });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] <= dp[i - 1][j]) {
            stack.push(PpamDiffLine {
                kind: '+',
                old_lineno: 0,
                new_lineno: j,
                content: new_lines[j - 1].to_string(),
            });
            j -= 1;
        } else if i > 0 {
            stack.push(PpamDiffLine {
                kind: '-',
                old_lineno: i,
                new_lineno: 0,
                content: old_lines[i - 1].to_string(),
            });
            i -= 1;
        }
    }
    while let Some(line) = stack.pop() {
        result.push(line);
    }
    result
}

// -----------------------------------------------------------------------
// PpamBridge
// -----------------------------------------------------------------------

impl PpamBridge {
    /// Create a new PPAM bridge for the given project.
    pub fn new(project_path: &Path) -> Self {
        Self {
            project_path: project_path.to_path_buf(),
            repo_cache: Mutex::new(None),
            memory_cache: Mutex::new(None),
        }
    }

    // ---- Caching --------------------------------------------------------

    /// Clear all cached state (repo + memory). Next call will reload from disk.
    pub fn clear_cache(&self) {
        *self.repo_cache.lock().unwrap() = None;
        *self.memory_cache.lock().unwrap() = None;
    }

    fn get_or_open_repo(&self) -> anyhow::Result<BasicRepository> {
        let mut cache = self.repo_cache.lock().unwrap();
        if cache.is_none() {
            *cache = Some(BasicRepository::open(&self.project_path)?);
        }
        // BasicRepository is !Clone, so we take it out and put it back.
        // This works because we're single-threaded in practice (PyO3 GIL).
        Ok(cache.take().unwrap())
    }

    /// Return the repo to the cache after use.
    fn return_repo(&self, repo: BasicRepository) {
        *self.repo_cache.lock().unwrap() = Some(repo);
    }

    fn get_or_load_memory(&self) -> MemoryStore {
        let mut cache = self.memory_cache.lock().unwrap();
        if cache.is_none() {
            *cache = Some(self.load_memory_from_disk());
        }
        cache.as_ref().unwrap().clone()
    }

    fn invalidate_cache_after_write(&self) {
        self.clear_cache();
    }

    // ---- Disk I/O (uncached) -------------------------------------------

    fn load_memory_from_disk(&self) -> MemoryStore {
        let path = self.project_path.join(".route").join("memory.json");
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<MemoryStore>(&s).ok())
                .unwrap_or_else(MemoryStore::new)
        } else {
            MemoryStore::new()
        }
    }

    fn save_memory_to_disk(&self, memory: &MemoryStore) -> anyhow::Result<()> {
        let path = self.project_path.join(".route").join("memory.json");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(memory)?)?;
        Ok(())
    }

    fn scan_structure(&self) -> StructureSnapshot {
        ProjectStructure::scan(&self.project_path, 8).unwrap_or_else(|_| StructureSnapshot::new())
    }

    // ---- Pointer file ---------------------------------------------------

    /// Read the `.ppam-link` pointer file to get the remote PPAM repository URL.
    pub fn remote_url(&self) -> Option<String> {
        let link_path = self.project_path.join(".ppam-link");
        let content = std::fs::read_to_string(link_path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("PPAM_REMOTE_URL=") {
                let url = line.trim_start_matches("PPAM_REMOTE_URL=").trim().to_string();
                if !url.is_empty() {
                    return Some(url);
                }
            }
        }
        None
    }

    /// Read the `.ppam-link` pointer file to get the local PPAM install path.
    ///
    /// Returns `project_path.join("ppam")` by default.
    pub fn ppam_path(&self) -> PathBuf {
        let link_path = self.project_path.join(".ppam-link");
        if let Ok(content) = std::fs::read_to_string(&link_path) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("PPAM_LOCAL_PATH=") {
                    let rel = line.trim_start_matches("PPAM_LOCAL_PATH=").trim().to_string();
                    if !rel.is_empty() {
                        return self.project_path.join(&rel);
                    }
                }
            }
        }
        self.project_path.join("ppam")
    }

    /// Convenience: read a file from the PPAM install directory.
    pub fn ppam_read_file(&self, rel_path: &str) -> anyhow::Result<String> {
        let full = self.ppam_path().join(rel_path);
        let content = std::fs::read_to_string(&full)?;
        Ok(content)
    }

    // ---- A. Document Content Interface ----------------------------------

    /// Read a document's content from disk.
    pub fn read_document(&self, path: &str) -> anyhow::Result<String> {
        let full = self.project_path.join(path);
        let content = std::fs::read_to_string(&full)?;
        Ok(content)
    }

    /// Write content to a document and auto-commit through Route.
    ///
    /// Returns the new document version record.
    pub fn write_document(&self, path: &str, content: &str, message: &str) -> anyhow::Result<PpamDocVersion> {
        // Write to disk first
        let full = self.project_path.join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, content)?;

        // Commit via Route
        let repo = self.get_or_open_repo()?;
        let opts = CommitOptions {
            message: message.to_string(),
            author: Some("PPAM".to_string()),
            force_full: false,
            branch: None,
            operator: Some("ai:ppam".to_string()),
            body: None,
            is_checkpoint: false,
            is_ai: true,
        };
        let commit = repo.commit(opts)?;
        let snapshot = repo.get_snapshot(&commit.to_snapshot)?;
        self.return_repo(repo);
        self.invalidate_cache_after_write();

        Ok(PpamDocVersion {
            snapshot_id: snapshot.id,
            commit_id: commit.id,
            message: commit.message,
            created_at: commit.created_at,
            file_path: path.to_string(),
        })
    }

    /// Compute a line-by-line diff between two versions of a document.
    ///
    /// `from` and `to` are snapshot IDs. If either is empty, the current
    /// working tree version is used for that side.
    pub fn diff_document(&self, path: &str, from: &str, to: &str) -> anyhow::Result<Vec<PpamDiffLine>> {
        let repo = self.get_or_open_repo()?;

        let old_content = if from.is_empty() {
            std::fs::read_to_string(self.project_path.join(path)).unwrap_or_default()
        } else {
            repo.restore_file_from_snapshot(from, path)?;
            std::fs::read_to_string(self.project_path.join(path)).unwrap_or_default()
        };

        let new_content = if to.is_empty() {
            std::fs::read_to_string(self.project_path.join(path)).unwrap_or_default()
        } else {
            repo.restore_file_from_snapshot(to, path)?;
            std::fs::read_to_string(self.project_path.join(path)).unwrap_or_default()
        };

        self.return_repo(repo);
        Ok(simple_diff(&old_content, &new_content))
    }

    // ---- B. Batch Operations -------------------------------------------

    /// Record multiple document versions in a single Route commit.
    ///
    /// Each entry is `(relative_path, message)`. All changes are committed
    /// atomically as one Route commit.
    pub fn batch_track(&self, files: &[(&str, &str)]) -> anyhow::Result<Vec<PpamDocVersion>> {
        if files.is_empty() {
            return Ok(vec![]);
        }

        let repo = self.get_or_open_repo()?;
        let combined_msg = if files.len() == 1 {
            files[0].1.to_string()
        } else {
            format!("PPAM batch: {} documents", files.len())
        };

        let opts = CommitOptions {
            message: combined_msg,
            author: Some("PPAM".to_string()),
            force_full: true,
            branch: None,
            operator: Some("ai:ppam".to_string()),
            body: None,
            is_checkpoint: false,
            is_ai: true,
        };
        let commit = repo.commit(opts)?;
        let snapshot = repo.get_snapshot(&commit.to_snapshot)?;
        self.return_repo(repo);
        self.invalidate_cache_after_write();

        let version = PpamDocVersion {
            snapshot_id: snapshot.id,
            commit_id: commit.id,
            message: commit.message,
            created_at: commit.created_at,
            file_path: files.iter().map(|(p, _)| *p).collect::<Vec<_>>().join(", "),
        };
        Ok(vec![version])
    }

    /// Get a partial project context — only the fields PPAM actually needs.
    ///
    /// Pass a slice of `ContextField` variants to select what to load.
    /// Empty slice returns all fields (same as `project_context()`).
    pub fn batch_context(&self, fields: &[ContextField]) -> anyhow::Result<PpamProjectContext> {
        let all = fields.is_empty();
        let repo = all || fields.contains(&ContextField::RecentCommits) || fields.contains(&ContextField::CurrentBranch);
        let memory = all || fields.contains(&ContextField::MemoryEntries) || fields.contains(&ContextField::Memory);
        let tree = all || fields.contains(&ContextField::FileCount) || fields.contains(&ContextField::FileTree);

        let mut repo_handle = if repo { Some(self.get_or_open_repo()?) } else { None };
        let mem = if memory { Some(self.get_or_load_memory()) } else { None };
        let snapshot = if tree { Some(self.scan_structure()) } else { None };

        let mut ctx = PpamProjectContext {
            project_path: self.project_path.to_string_lossy().to_string(),
            ppam_remote_url: if all || fields.contains(&ContextField::RemoteUrl) { self.remote_url() } else { None },
            file_count: snapshot.as_ref().map_or(0, |s| s.file_count),
            memory_entries: mem.as_ref().map_or(0, |m| m.len()),
            recent_commits: vec![],
            current_branch: String::new(),
            memory: vec![],
            file_tree: vec![],
            truncated: false,
            conversation_count: 0,
        };

        if let Some(ref r) = repo_handle {
            if all || fields.contains(&ContextField::RecentCommits) {
                if let Ok(commits) = r.list_commits(None, 10) {
                    ctx.recent_commits = commits.iter().map(|c| PpamCommit {
                        id: c.id.clone(),
                        message: c.message.clone(),
                        created_at: c.created_at,
                        branch: c.branch_id.clone(),
                    }).collect();
                }
            }
            if all || fields.contains(&ContextField::CurrentBranch) {
                ctx.current_branch = r.get_current_branch_name().unwrap_or_default();
            }
        }

        // Return repo to cache
        if let Some(r) = repo_handle.take() {
            self.return_repo(r);
        }

        if let Some(ref m) = mem {
            if all || fields.contains(&ContextField::Memory) {
                ctx.memory = m.all_sorted().iter().map(|e| PpamMemoryEntry {
                    key: e.key.clone(),
                    value: e.value.clone(),
                    tier: format!("{:?}", e.tier).to_lowercase(),
                    tags: e.tags.clone(),
                }).collect();
            }
        }

        if let Some(ref s) = snapshot {
            if all || fields.contains(&ContextField::FileTree) {
                let (tree_str, truncated) = s.format_tree(100);
                ctx.file_tree = tree_str.lines().map(|l| l.to_string()).collect();
                ctx.truncated = truncated;
            }
        }

        if all || fields.contains(&ContextField::Conversation) {
            let conv_path = self.project_path.join(".route").join("conversations.json");
            if let Ok(json) = std::fs::read_to_string(&conv_path) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
                    ctx.conversation_count = val.get("sessions").and_then(|v| v.as_array()).map_or(0, |a| a.len());
                }
            }
        }

        Ok(ctx)
    }

    // ---- C. Low Threshold ----------------------------------------------

    /// Get the latest version of a document.
    ///
    /// Returns `None` if the document has no Route-tracked history.
    pub fn latest_version(&self, path: &str) -> anyhow::Result<Option<PpamDocVersion>> {
        let repo = self.get_or_open_repo()?;
        let commits = repo.list_commits(None, 50)?;

        for commit in &commits {
            let diff = repo.diff_snapshots(&commit.from_snapshot, &commit.to_snapshot)?;
            if diff.iter().any(|e| e.path == path) {
                let snapshot = repo.get_snapshot(&commit.to_snapshot)?;
                self.return_repo(repo);
                return Ok(Some(PpamDocVersion {
                    snapshot_id: snapshot.id,
                    commit_id: commit.id.clone(),
                    message: commit.message.clone(),
                    created_at: commit.created_at,
                    file_path: path.to_string(),
                }));
            }
        }
        self.return_repo(repo);
        Ok(None)
    }

    /// Rollback a document to a previous version by index.
    ///
    /// `version_index` is 0-based (0 = oldest tracked version).
    /// Returns an error if the index is out of range.
    pub fn rollback_to_version(&self, path: &str, version_index: usize) -> anyhow::Result<()> {
        let repo = self.get_or_open_repo()?;
        let commits = repo.list_commits(None, 100)?;

        let mut versions: Vec<String> = Vec::new();
        for commit in &commits {
            let diff = repo.diff_snapshots(&commit.from_snapshot, &commit.to_snapshot)?;
            if diff.iter().any(|e| e.path == path) {
                versions.push(commit.to_snapshot.clone());
            }
        }

        let snapshot_id = versions.get(version_index)
            .ok_or_else(|| anyhow::anyhow!(
                "version index {} out of range (max {})", version_index, versions.len().saturating_sub(1)
            ))?;

        repo.rollback_to(snapshot_id, Some(&format!("PPAM rollback to version {} of {}", version_index, path)))?;
        self.return_repo(repo);
        self.invalidate_cache_after_write();
        Ok(())
    }

    // ---- D. Document Relations -----------------------------------------

    /// Record a directional relationship between two documents.
    ///
    /// Example: `bridge.link_documents("spec.md", "impl.rs", "implements")`
    pub fn link_documents(&self, from: &str, to: &str, relation: &str) -> anyhow::Result<()> {
        let mut relations = load_relations(&self.project_path);
        // Avoid duplicates
        relations.retain(|r| !(r.from == from && r.to == to && r.relation == relation));
        relations.push(PpamDocRelation {
            from: from.to_string(),
            to: to.to_string(),
            relation: relation.to_string(),
            created_at: chrono::Utc::now().timestamp(),
        });
        save_relations(&self.project_path, &relations)?;
        Ok(())
    }

    /// Remove a directional relationship between two documents.
    pub fn unlink_documents(&self, from: &str, to: &str) -> anyhow::Result<()> {
        let mut relations = load_relations(&self.project_path);
        relations.retain(|r| !(r.from == from && r.to == to));
        save_relations(&self.project_path, &relations)?;
        Ok(())
    }

    /// Get all relationships involving a document (both directions).
    pub fn document_relations(&self, path: &str) -> anyhow::Result<Vec<PpamDocRelation>> {
        let relations = load_relations(&self.project_path);
        let result: Vec<PpamDocRelation> = relations.into_iter()
            .filter(|r| r.from == path || r.to == path)
            .collect();
        Ok(result)
    }

    // ---- Legacy API (unchanged) ----------------------------------------

    /// Get full project context for PPAM document enhancement.
    pub fn project_context(&self) -> anyhow::Result<PpamProjectContext> {
        self.batch_context(&[])
    }

    /// Track a document version through Route.
    pub fn track_document_version(&self, file_path: &str, message: &str) -> anyhow::Result<PpamDocVersion> {
        self.write_document(file_path, "", message)
    }

    /// Search project memory using fuzzy matching.
    pub fn search_memory(&self, query: &str, limit: Option<usize>) -> Vec<PpamSearchResult> {
        let memory = self.get_or_load_memory();
        let limit = limit.unwrap_or(5);

        let mut results: Vec<PpamSearchResult> = Vec::new();

        for (entry, score) in memory.search(query) {
            if results.len() >= limit {
                break;
            }
            results.push(PpamSearchResult {
                text: format!("[{}] {}: {}", entry.key, entry.value, entry.tags.join(", ")),
                score,
                source: "memory".to_string(),
            });
        }

        if results.len() < limit {
            let snapshot = self.scan_structure();
            let files: Vec<&str> = snapshot.entries.keys().map(|s| s.as_str()).collect();
            let matcher = route_engine::FuzzyMatcher::new();
            for r in matcher.fuzzy_search(query, &files) {
                if results.len() >= limit {
                    break;
                }
                results.push(PpamSearchResult {
                    text: r.text,
                    score: r.score,
                    source: "file".to_string(),
                });
            }
        }

        results
    }

    /// Record a memory entry that PPAM discovered during document enhancement.
    pub fn record_memory(&self, key: &str, value: &str, tags: Vec<String>) -> anyhow::Result<()> {
        let mut memory = self.get_or_load_memory();
        memory.set(key, value, MemoryTier::Hot, tags);
        self.save_memory_to_disk(&memory)?;
        self.invalidate_cache_after_write();
        Ok(())
    }

    /// Get the version history of a specific document.
    pub fn document_history(&self, file_path: &str) -> anyhow::Result<Vec<PpamDocVersion>> {
        let repo = self.get_or_open_repo()?;
        let commits = repo.list_commits(None, 50)?;

        let mut versions: Vec<PpamDocVersion> = Vec::new();
        for commit in &commits {
            let diff = repo.diff_snapshots(&commit.from_snapshot, &commit.to_snapshot)?;
            if diff.iter().any(|e| e.path == file_path && (e.change == "added" || e.change == "modified")) {
                if let Ok(snapshot) = repo.get_snapshot(&commit.to_snapshot) {
                    versions.push(PpamDocVersion {
                        snapshot_id: snapshot.id,
                        commit_id: commit.id.clone(),
                        message: commit.message.clone(),
                        created_at: commit.created_at,
                        file_path: file_path.to_string(),
                    });
                }
            }
        }
        self.return_repo(repo);
        Ok(versions)
    }

    /// Rollback a document to a specific version.
    pub fn rollback_document(&self, snapshot_id: &str) -> anyhow::Result<()> {
        let repo = self.get_or_open_repo()?;
        repo.rollback_to(snapshot_id, Some("PPAM rollback"))?;
        self.return_repo(repo);
        self.invalidate_cache_after_write();
        Ok(())
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ---- Basics ----

    #[test]
    fn test_bridge_create() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.project_path.to_string_lossy().contains("tmp"));
    }

    #[test]
    fn test_project_context_no_repo() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.project_context().is_err());
    }

    #[test]
    fn test_search_memory_empty() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.search_memory("test", Some(5)).is_empty());
    }

    #[test]
    fn test_record_memory_missing_dir() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.record_memory("test/key", "test value", vec![]).is_ok());
    }

    #[test]
    fn test_remote_url_no_file() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.remote_url().is_none());
    }

    #[test]
    fn test_remote_url_with_file() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".ppam-link"), "PPAM_REMOTE_URL=https://github.com/example/ppam\n").unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert_eq!(bridge.remote_url(), Some("https://github.com/example/ppam".to_string()));
    }

    // ---- A. Document Content ----

    #[test]
    fn test_read_document() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "hello\nworld").unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert_eq!(bridge.read_document("test.txt").unwrap(), "hello\nworld");
    }

    #[test]
    fn test_read_document_not_found() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.read_document("nonexistent.txt").is_err());
    }

    #[test]
    fn test_simple_diff_identical() {
        let text = "line1\nline2\nline3";
        let diff = simple_diff(text, text);
        assert!(diff.iter().all(|l| l.kind == ' '));
        assert_eq!(diff.len(), 3);
    }

    #[test]
    fn test_simple_diff_insert() {
        let old = "line1\nline3";
        let new = "line1\nline2\nline3";
        let diff = simple_diff(old, new);
        let inserted: Vec<_> = diff.iter().filter(|l| l.kind == '+').collect();
        assert_eq!(inserted.len(), 1);
        assert_eq!(inserted[0].content, "line2");
    }

    #[test]
    fn test_simple_diff_delete() {
        let old = "line1\nline2\nline3";
        let new = "line1\nline3";
        let diff = simple_diff(old, new);
        let deleted: Vec<_> = diff.iter().filter(|l| l.kind == '-').collect();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].content, "line2");
    }

    // ---- B. Batch ----

    #[test]
    fn test_batch_context_select_fields() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        let ctx = bridge.batch_context(&[ContextField::RemoteUrl, ContextField::FileCount]).unwrap();
        assert!(ctx.ppam_remote_url.is_none());
        assert_eq!(ctx.file_count, 0);
        // Not-loaded fields should be empty
        assert!(ctx.recent_commits.is_empty());
        assert!(ctx.memory.is_empty());
    }

    #[test]
    fn test_batch_track_empty() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.batch_track(&[]).unwrap().is_empty());
    }

    // ---- C. Low Threshold ----

    #[test]
    fn test_latest_version_no_repo() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.latest_version("test.txt").is_err());
    }

    #[test]
    fn test_rollback_to_version_out_of_range() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        assert!(bridge.rollback_to_version("test.txt", 0).is_err());
    }

    // ---- D. Document Relations ----

    #[test]
    fn test_link_and_unlink() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        bridge.link_documents("a.md", "b.rs", "references").unwrap();
        bridge.link_documents("a.md", "c.py", "implements").unwrap();
        let rels = bridge.document_relations("a.md").unwrap();
        assert_eq!(rels.len(), 2);
        bridge.unlink_documents("a.md", "b.rs").unwrap();
        let rels = bridge.document_relations("a.md").unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].to, "c.py");
    }

    #[test]
    fn test_link_deduplicates() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        bridge.link_documents("a.md", "b.rs", "references").unwrap();
        bridge.link_documents("a.md", "b.rs", "references").unwrap();
        let rels = bridge.document_relations("a.md").unwrap();
        assert_eq!(rels.len(), 1);
    }

    #[test]
    fn test_unlink_nonexistent() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        // Should not error
        bridge.unlink_documents("a.md", "b.rs").unwrap();
    }

    // ---- E. Cache ----

    #[test]
    fn test_clear_cache() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        bridge.clear_cache();
        // No-op on empty cache, just shouldn't panic
        assert!(bridge.remote_url().is_none());
    }

    #[test]
    fn test_cache_invalidated_after_write() {
        let dir = tempdir().unwrap();
        let bridge = PpamBridge::new(dir.path());
        // Record memory, which should invalidate cache
        bridge.record_memory("k", "v", vec![]).unwrap();
        // Verify memory is still retrievable
        let results = bridge.search_memory("k", Some(5));
        assert!(!results.is_empty());
    }
}