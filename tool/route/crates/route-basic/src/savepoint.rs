//! Development Savepoint — unified whole-development-world archive.
//!
//! Savepoints capture the complete state of development:
//! - code snapshot
//! - context (CPR composition)
//! - project memory view
//! - strategy (profile/workflow/agent policy)
//! - active references/workflow/goals/task/session
//!
//! All are references to existing objects — no duplicate large data.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::dot_dir;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A complete development savepoint.
/// References all existing objects — no data duplication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentSavepoint {
    /// Unique savepoint ID.
    pub id: String,
    /// User-provided name.
    pub name: String,
    /// Creation timestamp (unix millis).
    pub created_at: i64,
    /// Code snapshot hash.
    pub code_snapshot_id: String,
    /// Current context hash.
    pub context_hash: String,
    /// Project memory snapshot (if saved).
    pub memory_snapshot_id: Option<String>,
    /// Current active strategy.
    pub strategy_id: Option<String>,
    /// Current protocol revision.
    pub protocol_revision: String,
    /// Workflow revisions for all workflows.
    pub workflow_revisions: Vec<String>,
    /// Active reference IDs.
    pub active_reference_ids: Vec<String>,
    /// Current agent policy hash.
    pub agent_policy_hash: Option<String>,
    /// Active goal IDs (ordered by priority).
    pub active_goal_ids: Vec<String>,
    /// Current task ID (if any).
    pub current_task_id: Option<String>,
    /// Current session ID (if any).
    pub current_session_id: Option<String>,
    /// Extra metadata (custom tags).
    pub metadata: HashMap<String, String>,
}

/// Scope for restore operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RestoreScope {
    /// Only restore code snapshot.
    Code,
    /// Only restore CPR context.
    Context,
    /// Only restore project memory view.
    Memory,
    /// Only restore strategy (profile/workflow/agent policy).
    Strategy,
    /// Restore everything except Constitution (requires explicit approval).
    All,
}

impl RestoreScope {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "code" => Ok(Self::Code),
            "context" => Ok(Self::Context),
            "memory" => Ok(Self::Memory),
            "strategy" => Ok(Self::Strategy),
            "all" => Ok(Self::All),
            _ => anyhow::bail!(
                "Invalid scope: {}. Expected: code|context|memory|strategy|all",
                s
            ),
        }
    }
}

/// Diff between two savepoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavepointDiff {
    pub a_id: String,
    pub b_id: String,
    pub a_name: String,
    pub b_name: String,
    pub created_at_diff: (i64, i64),
    pub code_changed: bool,
    pub context_changed: bool,
    pub memory_changed: bool,
    pub strategy_changed: bool,
    pub protocol_changed: bool,
    pub workflows_added: Vec<String>,
    pub workflows_removed: Vec<String>,
    pub references_added: Vec<String>,
    pub references_removed: Vec<String>,
    pub goals_added: Vec<String>,
    pub goals_removed: Vec<String>,
    pub task_changed: bool,
    pub session_changed: bool,
}

/// Store for all savepoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavepointStore {
    pub savepoints: Vec<DevelopmentSavepoint>,
}

impl SavepointStore {
    /// Get the savepoint directory.
    pub fn dir(project_root: &Path) -> PathBuf {
        dot_dir(project_root).join("savepoint")
    }

    /// Get the store file path.
    fn store_path(project_root: &Path) -> PathBuf {
        Self::dir(project_root).join("store.json")
    }
}

/// Get the savepoint directory path.
pub fn savepoint_dir(project_root: &Path) -> PathBuf {
    SavepointStore::dir(project_root)
}

/// Get a specific savepoint file path (if needed).
pub fn savepoint_path(project_root: &Path, id: &str) -> PathBuf {
    SavepointStore::dir(project_root).join(format!("{}.json", id))
}

impl SavepointStore {
    /// Load the savepoint store from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = Self::store_path(project_root);
        if !path.exists() {
            return Ok(Self {
                savepoints: Vec::new(),
            });
        }
        let content = fs::read_to_string(&path)?;
        let store: Self = serde_json::from_str(&content)?;
        Ok(store)
    }

    /// Save the store to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = Self::dir(project_root);
        fs::create_dir_all(&dir)?;
        let path = Self::store_path(project_root);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Add a new savepoint.
    pub fn add(&mut self, savepoint: DevelopmentSavepoint) {
        self.savepoints.push(savepoint);
    }

    /// Get a savepoint by ID.
    pub fn get(&self, id: &str) -> Option<&DevelopmentSavepoint> {
        self.savepoints.iter().find(|sp| sp.id == id)
    }

    /// Get a mutable reference to a savepoint by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut DevelopmentSavepoint> {
        self.savepoints.iter_mut().find(|sp| sp.id == id)
    }

    /// List savepoints sorted by creation time (newest first).
    pub fn list(&self) -> Vec<&DevelopmentSavepoint> {
        let mut list: Vec<_> = self.savepoints.iter().collect();
        list.sort_by_key(|sp| std::cmp::Reverse(sp.created_at));
        list
    }
}

// ---------------------------------------------------------------------------
// Core API
// ---------------------------------------------------------------------------

/// Create a new development savepoint.
pub fn create_savepoint(
    project_root: &Path,
    name: String,
    code_snapshot_id: String,
    context_hash: String,
    memory_snapshot_id: Option<String>,
    strategy_id: Option<String>,
    protocol_revision: String,
    workflow_revisions: Vec<String>,
    active_reference_ids: Vec<String>,
    agent_policy_hash: Option<String>,
    active_goal_ids: Vec<String>,
    current_task_id: Option<String>,
    current_session_id: Option<String>,
) -> Result<DevelopmentSavepoint> {
    let id = format!("sp_{}", ulid::Ulid::new().to_string());
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as i64;

    let savepoint = DevelopmentSavepoint {
        id,
        name,
        created_at,
        code_snapshot_id,
        context_hash,
        memory_snapshot_id,
        strategy_id,
        protocol_revision,
        workflow_revisions,
        active_reference_ids,
        agent_policy_hash,
        active_goal_ids,
        current_task_id,
        current_session_id,
        metadata: HashMap::new(),
    };

    let mut store = SavepointStore::load(project_root)?;
    store.add(savepoint.clone());
    store.save(project_root)?;

    Ok(savepoint)
}

/// Compute diff between two savepoints.
pub fn diff_savepoints(a: &DevelopmentSavepoint, b: &DevelopmentSavepoint) -> SavepointDiff {
    use std::collections::HashSet;

    let a_workflows: HashSet<_> = a.workflow_revisions.iter().collect();
    let b_workflows: HashSet<_> = b.workflow_revisions.iter().collect();
    let a_references: HashSet<_> = a.active_reference_ids.iter().collect();
    let b_references: HashSet<_> = b.active_reference_ids.iter().collect();
    let a_goals: HashSet<_> = a.active_goal_ids.iter().collect();
    let b_goals: HashSet<_> = b.active_goal_ids.iter().collect();

    let workflows_added: Vec<String> = b_workflows
        .difference(&a_workflows)
        .map(|s| s.to_string())
        .collect();
    let workflows_removed: Vec<String> = a_workflows
        .difference(&b_workflows)
        .map(|s| s.to_string())
        .collect();

    let references_added: Vec<String> = b_references
        .difference(&a_references)
        .map(|s| s.to_string())
        .collect();
    let references_removed: Vec<String> = a_references
        .difference(&b_references)
        .map(|s| s.to_string())
        .collect();

    let goals_added: Vec<String> = b_goals
        .difference(&a_goals)
        .map(|s| s.to_string())
        .collect();
    let goals_removed: Vec<String> = a_goals
        .difference(&b_goals)
        .map(|s| s.to_string())
        .collect();

    SavepointDiff {
        a_id: a.id.clone(),
        b_id: b.id.clone(),
        a_name: a.name.clone(),
        b_name: b.name.clone(),
        created_at_diff: (a.created_at, b.created_at),
        code_changed: a.code_snapshot_id != b.code_snapshot_id,
        context_changed: a.context_hash != b.context_hash,
        memory_changed: a.memory_snapshot_id != b.memory_snapshot_id,
        strategy_changed: a.strategy_id != b.strategy_id
            || a.agent_policy_hash != b.agent_policy_hash,
        protocol_changed: a.protocol_revision != b.protocol_revision,
        workflows_added,
        workflows_removed,
        references_added,
        references_removed,
        goals_added,
        goals_removed,
        task_changed: a.current_task_id != b.current_task_id,
        session_changed: a.current_session_id != b.current_session_id,
    }
}

/// Format a savepoint for display.
pub fn format_savepoint(sp: &DevelopmentSavepoint, verbose: bool) -> String {
    let dt = chrono::DateTime::from_timestamp_millis(sp.created_at)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| sp.created_at.to_string());

    let mut output = format!(
        "Savepoint [{}] {}\n  Created: {}\n  Code Snapshot: {}\n  Context Hash: {}\n",
        sp.id, sp.name, dt, sp.code_snapshot_id, sp.context_hash
    );

    if verbose {
        if let Some(mid) = &sp.memory_snapshot_id {
            output.push_str(&format!("  Memory Snapshot: {}\n", mid));
        }
        if let Some(sid) = &sp.strategy_id {
            output.push_str(&format!("  Strategy: {}\n", sid));
        }
        output.push_str(&format!("  Protocol Revision: {}\n", sp.protocol_revision));
        output.push_str(&format!(
            "  Workflows: {} items\n",
            sp.workflow_revisions.len()
        ));
        output.push_str(&format!(
            "  Active References: {} items\n",
            sp.active_reference_ids.len()
        ));
        output.push_str(&format!(
            "  Active Goals: {} items\n",
            sp.active_goal_ids.len()
        ));
        if let Some(tid) = &sp.current_task_id {
            output.push_str(&format!("  Current Task: {}\n", tid));
        }
        if let Some(sid) = &sp.current_session_id {
            output.push_str(&format!("  Current Session: {}\n", sid));
        }
    }

    output
}

/// Format a diff for display.
pub fn format_diff(diff: &SavepointDiff) -> String {
    let a_dt = chrono::DateTime::from_timestamp_millis(diff.created_at_diff.0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| diff.created_at_diff.0.to_string());
    let b_dt = chrono::DateTime::from_timestamp_millis(diff.created_at_diff.1)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| diff.created_at_diff.1.to_string());

    let mut output = format!(
        "Diff: {} ({}) → {} ({})\n\n",
        diff.a_name, a_dt, diff.b_name, b_dt
    );

    let mut changed = Vec::new();
    if diff.code_changed {
        changed.push("Code");
    }
    if diff.context_changed {
        changed.push("Context");
    }
    if diff.memory_changed {
        changed.push("Memory");
    }
    if diff.strategy_changed {
        changed.push("Strategy");
    }
    if diff.protocol_changed {
        changed.push("Protocol");
    }
    if diff.task_changed {
        changed.push("Task");
    }
    if diff.session_changed {
        changed.push("Session");
    }

    if changed.is_empty() {
        output.push_str("No changes detected.\n");
    } else {
        output.push_str(&format!("Changed sections: {}\n\n", changed.join(", ")));
    }

    if !diff.workflows_added.is_empty() {
        output.push_str(&format!(
            "Workflows added ({}):\n",
            diff.workflows_added.len()
        ));
        for w in &diff.workflows_added {
            output.push_str(&format!("  + {}\n", w));
        }
        output.push_str("\n");
    }
    if !diff.workflows_removed.is_empty() {
        output.push_str(&format!(
            "Workflows removed ({}):\n",
            diff.workflows_removed.len()
        ));
        for w in &diff.workflows_removed {
            output.push_str(&format!("  - {}\n", w));
        }
        output.push_str("\n");
    }

    if !diff.references_added.is_empty() {
        output.push_str(&format!(
            "References added ({}):\n",
            diff.references_added.len()
        ));
        for r in &diff.references_added {
            output.push_str(&format!("  + {}\n", r));
        }
        output.push_str("\n");
    }
    if !diff.references_removed.is_empty() {
        output.push_str(&format!(
            "References removed ({}):\n",
            diff.references_removed.len()
        ));
        for r in &diff.references_removed {
            output.push_str(&format!("  - {}\n", r));
        }
        output.push_str("\n");
    }

    if !diff.goals_added.is_empty() {
        output.push_str(&format!("Goals added ({}):\n", diff.goals_added.len()));
        for g in &diff.goals_added {
            output.push_str(&format!("  + {}\n", g));
        }
    }
    if !diff.goals_removed.is_empty() {
        output.push_str(&format!("Goals removed ({}):\n", diff.goals_removed.len()));
        for g in &diff.goals_removed {
            output.push_str(&format!("  - {}\n", g));
        }
    }

    output
}

/// Preview restore and check what would be changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePreview {
    pub savepoint_id: String,
    pub scope: RestoreScope,
    pub changes: Vec<String>,
    /// Whether Constitution would be affected and needs explicit approval.
    pub constitution_requires_approval: bool,
    /// Current Constitution hash vs restore target hash.
    pub constitution_diff: Option<(String, String)>,
}

impl RestorePreview {
    pub fn new(savepoint_id: String, scope: RestoreScope) -> Self {
        Self {
            savepoint_id,
            scope,
            changes: Vec::new(),
            constitution_requires_approval: false,
            constitution_diff: None,
        }
    }
}

/// Preview restore for a savepoint with a given scope.
pub fn preview_restore(project_root: &Path, id: &str, scope: &str) -> Result<RestorePreview> {
    let store = SavepointStore::load(project_root)?;
    let sp = store
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("Savepoint '{}' not found", id))?;
    let scope = RestoreScope::from_str(scope)?;

    let mut preview = RestorePreview::new(id.to_string(), scope.clone());

    match &scope {
        RestoreScope::Code => {
            // Check code snapshot exists
            preview
                .changes
                .push(format!("Restore code to snapshot: {}", sp.code_snapshot_id));
        }
        RestoreScope::Context => {
            preview
                .changes
                .push(format!("Restore context to hash: {}", sp.context_hash));
        }
        RestoreScope::Memory => {
            if let Some(mid) = &sp.memory_snapshot_id {
                preview
                    .changes
                    .push(format!("Restore memory view to snapshot: {}", mid));
            } else {
                preview
                    .changes
                    .push("No memory snapshot in this savepoint.".to_string());
            }
        }
        RestoreScope::Strategy => {
            if let Some(sid) = &sp.strategy_id {
                preview
                    .changes
                    .push(format!("Restore strategy to: {}", sid));
            }
            preview.changes.push(format!(
                "Restore protocol revision: {}",
                sp.protocol_revision
            ));
            if !sp.workflow_revisions.is_empty() {
                preview.changes.push(format!(
                    "Restore {} workflow revisions",
                    sp.workflow_revisions.len()
                ));
            }
            if let Some(aph) = &sp.agent_policy_hash {
                preview
                    .changes
                    .push(format!("Restore agent policy hash: {}", aph));
            }
        }
        RestoreScope::All => {
            preview
                .changes
                .push(format!("Restore code to snapshot: {}", sp.code_snapshot_id));
            preview
                .changes
                .push(format!("Restore context to hash: {}", sp.context_hash));
            if let Some(mid) = &sp.memory_snapshot_id {
                preview
                    .changes
                    .push(format!("Restore memory view to snapshot: {}", mid));
            }
            if let Some(sid) = &sp.strategy_id {
                preview
                    .changes
                    .push(format!("Restore strategy to: {}", sid));
            }
            preview.changes.push(format!(
                "Restore protocol revision: {}",
                sp.protocol_revision
            ));
            if !sp.workflow_revisions.is_empty() {
                preview.changes.push(format!(
                    "Restore {} workflow revisions",
                    sp.workflow_revisions.len()
                ));
            }
            if let Some(aph) = &sp.agent_policy_hash {
                preview
                    .changes
                    .push(format!("Restore agent policy hash: {}", aph));
            }
            if !sp.active_goal_ids.is_empty() {
                preview
                    .changes
                    .push(format!("Restore {} active goals", sp.active_goal_ids.len()));
            }
            // Constitution is not modified by default, flag for approval
            preview.constitution_requires_approval = false;
            preview.changes.push(
                "Constitution NOT modified by default (requires explicit --force).".to_string(),
            );
        }
    }

    Ok(preview)
}

/// Execute a restore operation.
pub fn execute_restore(project_root: &Path, id: &str, scope: &str, force: bool) -> Result<()> {
    let store = SavepointStore::load(project_root)?;
    let sp = store
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("Savepoint '{}' not found", id))?;
    let scope = RestoreScope::from_str(scope)?;

    // For All scope, Constitution requires explicit --force
    if scope == RestoreScope::All && !force {
        anyhow::bail!(
            "Restoring 'all' scope does not modify Constitution by default. \
             Use --force to confirm no Constitution changes, or use --scope code|strategy|context|memory for targeted restore."
        );
    }

    match &scope {
        RestoreScope::Code => {
            // Use repository rollback to restore code snapshot
            let repo = crate::repository::BasicRepository::open(project_root)?;
            repo.rollback_to(
                &sp.code_snapshot_id,
                Some("restore: savepoint code restore"),
            )?;
            println!("  Code restored to snapshot: {}", sp.code_snapshot_id);
        }
        RestoreScope::Context => {
            // Rebuild context archive — this is a reference point
            println!("  Context hash reference: {}", sp.context_hash);
            println!("  Use `route task start` to re-establish context.");
        }
        RestoreScope::Memory => {
            if let Some(mid) = &sp.memory_snapshot_id {
                // Restore memory view — load from archive
                let mem_dir = crate::memory::memory_dir(project_root);
                let archive_path = mem_dir.join("archive").join(format!("{}.json", mid));
                if archive_path.exists() {
                    let content = std::fs::read_to_string(&archive_path)?;
                    let mem: crate::memory::ProjectMemory = serde_json::from_str(&content)?;
                    let mut store = crate::memory::MemoryStore::load(project_root)?;
                    // Add the restored memory to the store and mark as current
                    store.memories.push(mem);
                    store.current = store.memories.len().checked_sub(1).map(|i| {
                        let m = &store.memories[i];
                        format!("{}", m.updated_at)
                    });
                    store.save(project_root)?;
                    println!("  Memory view restored to snapshot: {}", mid);
                } else {
                    anyhow::bail!("Memory snapshot '{}' not found in archive.", mid);
                }
            } else {
                anyhow::bail!("No memory snapshot in this savepoint.");
            }
        }
        RestoreScope::Strategy => {
            // Restore strategy via strategy store restore
            if let Some(sid) = &sp.strategy_id {
                let mut strat_store = crate::strategy::StrategyStore::load(project_root)?;
                strat_store.restore(project_root, sid)?;
                println!("  Strategy restored to: {}", sid);
            }
            // Protocol revision is informational
            println!("  Protocol revision reference: {}", sp.protocol_revision);
            if !sp.workflow_revisions.is_empty() {
                println!(
                    "  Workflow revisions: {} items",
                    sp.workflow_revisions.len()
                );
            }
        }
        RestoreScope::All => {
            // 1. Restore code
            let repo = crate::repository::BasicRepository::open(project_root)?;
            repo.rollback_to(&sp.code_snapshot_id, Some("restore: savepoint all restore"))?;
            println!("  Code restored to snapshot: {}", sp.code_snapshot_id);

            // 2. Restore strategy
            if let Some(sid) = &sp.strategy_id {
                let mut strat_store = crate::strategy::StrategyStore::load(project_root)?;
                strat_store.restore(project_root, sid)?;
                println!("  Strategy restored to: {}", sid);
            }

            // 3. Restore memory if available
            if let Some(mid) = &sp.memory_snapshot_id {
                let mem_dir = crate::memory::memory_dir(project_root);
                let archive_path = mem_dir.join("archive").join(format!("{}.json", mid));
                if archive_path.exists() {
                    let content = std::fs::read_to_string(&archive_path)?;
                    let mem: crate::memory::ProjectMemory = serde_json::from_str(&content)?;
                    let mut store = crate::memory::MemoryStore::load(project_root)?;
                    store.memories.push(mem);
                    store.current = store.memories.len().checked_sub(1).map(|i| {
                        let m = &store.memories[i];
                        format!("{}", m.updated_at)
                    });
                    store.save(project_root)?;
                    println!("  Memory view restored to snapshot: {}", mid);
                }
            }

            println!("  Context hash reference: {}", sp.context_hash);
            println!("  Protocol revision reference: {}", sp.protocol_revision);
            println!("  Constitution NOT modified.");
        }
    }

    Ok(())
}

/// Delete a savepoint by ID.
pub fn delete_savepoint(project_root: &Path, id: &str) -> Result<()> {
    let mut store = SavepointStore::load(project_root)?;
    let len_before = store.savepoints.len();
    store.savepoints.retain(|sp| sp.id != id);
    if store.savepoints.len() == len_before {
        anyhow::bail!("Savepoint '{}' not found", id);
    }
    store.save(project_root)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// P5 — Snapshot Memory
// ---------------------------------------------------------------------------

/// Create a memory snapshot manifest from the current ProjectMemory.
pub fn create_memory_snapshot(project_root: &Path, _savepoint_id: &str) -> Result<String> {
    let store = crate::memory::MemoryStore::load(project_root)?;
    let snapshot_id = format!("ms_{}", ulid::Ulid::new().to_string());

    let archive_dir = crate::memory::memory_dir(project_root).join("archive");
    std::fs::create_dir_all(&archive_dir)?;

    let mem = store.current_memory();
    let content = serde_json::to_string_pretty(&mem)?;
    let snapshot_path = archive_dir.join(format!("{}.json", snapshot_id));
    std::fs::write(&snapshot_path, content)?;

    Ok(snapshot_id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let dot = tmp.path().join(".route-basic");
        fs::create_dir_all(&dot).unwrap();
        tmp
    }

    #[test]
    fn test_create_and_load_savepoint() {
        let tmp = setup_dir();
        let root = tmp.path();

        let sp = create_savepoint(
            root,
            "test-save".to_string(),
            "snap123".to_string(),
            "ctx456".to_string(),
            None,
            Some("strat789".to_string()),
            "rev1".to_string(),
            vec!["wf1".to_string()],
            vec!["ref1".to_string(), "ref2".to_string()],
            None,
            vec![],
            None,
            None,
        )
        .unwrap();

        let store = SavepointStore::load(root).unwrap();
        assert_eq!(store.savepoints.len(), 1);
        assert_eq!(store.savepoints[0].name, "test-save");
        assert_eq!(store.savepoints[0].code_snapshot_id, "snap123");
        assert_eq!(store.savepoints[0].id, sp.id);
    }

    #[test]
    fn test_list_order() {
        let tmp = setup_dir();
        let root = tmp.path();

        let sp1 = create_savepoint(
            root,
            "first".to_string(),
            "s1".to_string(),
            "c1".to_string(),
            None,
            None,
            "r1".to_string(),
            vec![],
            vec![],
            None,
            vec![],
            None,
            None,
        )
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(2));

        let sp2 = create_savepoint(
            root,
            "second".to_string(),
            "s2".to_string(),
            "c2".to_string(),
            None,
            None,
            "r2".to_string(),
            vec![],
            vec![],
            None,
            vec![],
            None,
            None,
        )
        .unwrap();

        let store = SavepointStore::load(root).unwrap();
        let list = store.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, sp2.id); // newest first
        assert_eq!(list[1].id, sp1.id);
    }

    #[test]
    fn test_diff_savepoints() {
        let tmp = setup_dir();
        let root = tmp.path();

        let sp1 = create_savepoint(
            root,
            "before".to_string(),
            "snapA".to_string(),
            "ctxA".to_string(),
            None,
            None,
            "r1".to_string(),
            vec!["wf1".to_string()],
            vec!["ref1".to_string()],
            None,
            vec!["g1".to_string()],
            None,
            None,
        )
        .unwrap();

        let sp2 = create_savepoint(
            root,
            "after".to_string(),
            "snapB".to_string(),
            "ctxB".to_string(),
            None,
            None,
            "r2".to_string(),
            vec!["wf1".to_string(), "wf2".to_string()],
            vec!["ref1".to_string(), "ref2".to_string()],
            None,
            vec!["g1".to_string(), "g2".to_string()],
            None,
            None,
        )
        .unwrap();

        let diff = diff_savepoints(&sp1, &sp2);
        assert!(diff.code_changed);
        assert!(diff.context_changed);
        assert!(diff.protocol_changed);
        assert_eq!(diff.workflows_added, vec!["wf2".to_string()]);
        assert_eq!(diff.goals_added, vec!["g2".to_string()]);
        assert!(diff.workflows_removed.is_empty());
    }

    #[test]
    fn test_restore_scope_parsing() {
        assert_eq!(RestoreScope::from_str("code").unwrap(), RestoreScope::Code);
        assert_eq!(
            RestoreScope::from_str("context").unwrap(),
            RestoreScope::Context
        );
        assert_eq!(
            RestoreScope::from_str("memory").unwrap(),
            RestoreScope::Memory
        );
        assert_eq!(
            RestoreScope::from_str("strategy").unwrap(),
            RestoreScope::Strategy
        );
        assert_eq!(RestoreScope::from_str("all").unwrap(), RestoreScope::All);
        assert!(RestoreScope::from_str("invalid").is_err());
    }

    #[test]
    fn test_delete_savepoint() {
        let tmp = setup_dir();
        let root = tmp.path();

        let sp = create_savepoint(
            root,
            "to-delete".to_string(),
            "s1".to_string(),
            "c1".to_string(),
            None,
            None,
            "r1".to_string(),
            vec![],
            vec![],
            None,
            vec![],
            None,
            None,
        )
        .unwrap();

        let store = SavepointStore::load(root).unwrap();
        assert_eq!(store.savepoints.len(), 1);

        delete_savepoint(root, &sp.id).unwrap();
        let store = SavepointStore::load(root).unwrap();
        assert_eq!(store.savepoints.len(), 0);
    }

    #[test]
    fn test_memory_snapshot_creation() {
        let tmp = setup_dir();
        let root = tmp.path();

        // Create a memory store first
        let mem_store = crate::memory::MemoryStore::load(root).unwrap();
        let mem = mem_store.current_memory();
        let snapshot_id = create_memory_snapshot(root, "sp1").unwrap();
        assert!(!snapshot_id.is_empty());

        let archive_dir = crate::memory::memory_dir(root).join("archive");
        let snapshot_path = archive_dir.join(format!("{}.json", snapshot_id));
        assert!(snapshot_path.exists());
    }
}
