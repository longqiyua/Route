//! Strategy versioning — record and restore development strategy snapshots.
//!
//! A StrategySnapshot captures the combination of:
//! - Profile (active profile + policy overrides)
//! - Protocol (revision, content)
//! - Workflow (selected workflow IDs)
//! - AgentPlan (if generated)
//!
//! This allows "code can rollback, AI working style can also rollback."

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{write_atomic, ROUTE_DOT_DIR};

/// Directory name under `.route/strategy/`.
pub const STRATEGY_DIR: &str = "strategy";

/// Strategy store file name.
pub const STRATEGY_FILE: &str = "strategy.json";

/// Canonical strategy directory path.
pub fn strategy_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(STRATEGY_DIR)
}

/// Canonical strategy store file path.
pub fn strategy_path(project_root: &Path) -> PathBuf {
    strategy_dir(project_root).join(STRATEGY_FILE)
}

/// A snapshot of development strategy: profile + protocol + workflow + agent plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySnapshot {
    pub id: String,
    pub created_at: i64,
    pub profile_id: String,
    pub profile: serde_json::Value,
    pub protocol_revision: u32,
    pub protocol_content: String,
    pub workflow_ids: Vec<String>,
    pub agent_plan: Option<serde_json::Value>,
    pub label: Option<String>,
    pub description: Option<String>,

    /// Tags for categorization (e.g. "taiyi-inspired", "fast-experimental", "strict-v2")
    #[serde(default)]
    pub tags: Vec<String>,

    /// Parent strategy ID if this is a fork
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Memory snapshot reference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_ref: Option<String>,

    /// Enabled reference IDs from the ReferenceRegistry at snapshot time
    #[serde(default)]
    pub enabled_refs: Vec<String>,
}

/// Store for strategy snapshots.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyStore {
    pub snapshots: Vec<StrategySnapshot>,
    pub current: Option<String>, // id of current strategy
}

impl StrategyStore {
    /// Load strategy store from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = strategy_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading strategy store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        let s: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing strategy store JSON at {}", p.display()))?;
        Ok(s)
    }

    /// Save strategy store to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = strategy_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing strategy store to {}", p.display()))?;
        Ok(())
    }

    /// Record a new strategy snapshot capturing the current state.
    ///
    /// Captures current profile, protocol, workflows, references, and any agent plan.
    /// Returns the newly created snapshot.
    pub fn record_snapshot(
        &mut self,
        project_root: &Path,
        label: Option<String>,
        description: Option<String>,
    ) -> Result<StrategySnapshot> {
        use crate::constitutive::{Protocol, ReferenceRegistry};
        use crate::profile::ProfileStore;
        use crate::workflow::WorkflowStore;

        // Capture active profile
        let profile_store = ProfileStore::load(project_root)?;
        let active_id = ProfileStore::active_id(project_root)?.unwrap_or_default();
        let profile_json = serde_json::to_value(&profile_store)?;

        // Capture protocol
        let protocol = Protocol::read(project_root)?;
        let protocol_revision = protocol.revision as u32;
        let protocol_content = protocol.body.clone();

        // Capture workflow IDs (enabled workflows)
        let workflow_store = WorkflowStore::load(project_root)?;
        let workflow_ids: Vec<String> = workflow_store
            .workflows
            .iter()
            .filter(|w| w.enabled)
            .map(|w| w.id.clone())
            .collect();

        // Capture enabled reference IDs
        let registry = ReferenceRegistry::read(project_root)?;
        let enabled_refs: Vec<String> = registry
            .entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.id.clone())
            .collect();

        let snapshot = StrategySnapshot {
            id: route_core::new_id(),
            created_at: route_core::now_millis(),
            profile_id: active_id,
            profile: profile_json,
            protocol_revision,
            protocol_content,
            workflow_ids,
            agent_plan: None,
            label,
            description,
            tags: Vec::new(),
            parent_id: None,
            memory_ref: None,
            enabled_refs,
        };

        self.snapshots.push(snapshot.clone());
        self.current = Some(snapshot.id.clone());
        self.save(project_root)?;

        Ok(snapshot)
    }

    /// Restore a strategy snapshot.
    ///
    /// Restores profile, protocol, and workflow configuration from the
    /// snapshot. Does NOT touch project code or snapshots. Preserves the
    /// current strategy by saving it first (so you can switch back).
    pub fn restore(&mut self, project_root: &Path, snapshot_id: &str) -> Result<()> {
        use crate::constitutive::Protocol;
        use crate::profile::ProfileStore;
        use crate::workflow::WorkflowStore;

        // Find the snapshot (support prefix matching)
        let snapshot = self
            .find_snapshot(snapshot_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", snapshot_id))?;

        // Save current strategy first so we can switch back
        self.record_snapshot(
            project_root,
            Some("auto-save".to_string()),
            Some("Auto-saved before restore".to_string()),
        )?;

        // Restore profile
        let profile_store: ProfileStore = serde_json::from_value(snapshot.profile.clone())
            .context("Failed to deserialize profile from snapshot")?;
        profile_store.save(project_root)?;
        if !snapshot.profile_id.is_empty() {
            ProfileStore::set_active(project_root, &snapshot.profile_id)?;
        }

        // Restore protocol directly (write the exact content from snapshot)
        let protocol_path = crate::constitutive::protocol_path(project_root);
        let rendered = format!(
            "<!-- route-protocol:version={} -->\n\
             <!-- route-protocol:revision={} -->\n\
             <!-- route-protocol:updated_at={} -->\n\
             {}",
            Protocol::CURRENT_VERSION,
            snapshot.protocol_revision,
            route_core::now_millis(),
            snapshot.protocol_content
        );
        write_atomic(&protocol_path, rendered.as_bytes()).context("Failed to restore protocol")?;

        // Restore workflow configuration: enable only those in snapshot
        let mut workflow_store = WorkflowStore::load(project_root)?;
        for wf in &mut workflow_store.workflows {
            wf.enabled = snapshot.workflow_ids.contains(&wf.id);
        }
        for wf in &workflow_store.workflows {
            WorkflowStore::save(project_root, wf)?;
        }

        // Set current to the restored snapshot
        self.current = Some(snapshot.id.clone());
        self.save(project_root)?;

        Ok(())
    }

    /// Find a snapshot by exact id or prefix.
    fn find_snapshot(&self, id: &str) -> Option<&StrategySnapshot> {
        self.snapshots
            .iter()
            .find(|s| s.id == id || s.id.starts_with(id))
    }

    /// List all snapshots (newest first).
    pub fn list(&self) -> Vec<&StrategySnapshot> {
        let mut result: Vec<&StrategySnapshot> = self.snapshots.iter().collect();
        result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        result
    }

    /// Get a snapshot by id or prefix.
    pub fn get(&self, id: &str) -> Option<&StrategySnapshot> {
        self.find_snapshot(id)
    }

    /// Show a formatted snapshot.
    pub fn show(id: &str, store: &Self) -> Result<String> {
        let snapshot = store
            .find_snapshot(id)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", id))?;

        let mut output = String::new();
        output.push_str(&format!("Snapshot:     {}\n", snapshot.id));
        output.push_str(&format!("Created:      {}\n", snapshot.created_at));
        output.push_str(&format!("Profile:      {}\n", snapshot.profile_id));
        output.push_str(&format!("Protocol rev: {}\n", snapshot.protocol_revision));
        output.push_str(&format!(
            "Workflows:    {}\n",
            if snapshot.workflow_ids.is_empty() {
                "(none)".to_string()
            } else {
                snapshot.workflow_ids.join(", ")
            }
        ));
        if let Some(label) = &snapshot.label {
            output.push_str(&format!("Label:        {}\n", label));
        }
        if let Some(desc) = &snapshot.description {
            output.push_str(&format!("Description:  {}\n", desc));
        }
        if snapshot.agent_plan.is_some() {
            output.push_str("Agent Plan:   present\n");
        } else {
            output.push_str("Agent Plan:   absent\n");
        }
        if !snapshot.tags.is_empty() {
            output.push_str(&format!("Tags:         {}\n", snapshot.tags.join(", ")));
        }
        if let Some(parent) = &snapshot.parent_id {
            output.push_str(&format!("Parent:       {}\n", parent));
        }
        if let Some(mref) = &snapshot.memory_ref {
            output.push_str(&format!("Memory Ref:   {}\n", mref));
        }
        if !snapshot.enabled_refs.is_empty() {
            output.push_str(&format!(
                "Refs:         {}\n",
                snapshot.enabled_refs.join(", ")
            ));
        }
        let is_current = store.current.as_ref().map_or(false, |c| c == &snapshot.id);
        if is_current {
            output.push_str("Status:       current\n");
        }
        Ok(output)
    }

    /// Diff two snapshots and return a human-readable text diff.
    pub fn diff(a_id: &str, b_id: &str, store: &Self) -> Result<String> {
        let a = store
            .find_snapshot(a_id)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", a_id))?;
        let b = store
            .find_snapshot(b_id)
            .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", b_id))?;

        let mut output = String::new();
        output.push_str(&format!("--- Snapshot: {} ({})\n", a.id, a.created_at));
        output.push_str(&format!("+++ Snapshot: {} ({})\n", b.id, b.created_at));
        output.push('\n');

        // Profile diff
        if a.profile_id != b.profile_id {
            output.push_str(&format!("- Profile: {}\n", a.profile_id));
            output.push_str(&format!("+ Profile: {}\n", b.profile_id));
        } else {
            output.push_str(&format!("  Profile: {} (unchanged)\n", a.profile_id));
        }

        // Protocol revision diff
        if a.protocol_revision != b.protocol_revision {
            output.push_str(&format!("- Protocol revision: {}\n", a.protocol_revision));
            output.push_str(&format!("+ Protocol revision: {}\n", b.protocol_revision));
        } else {
            output.push_str(&format!(
                "  Protocol revision: {} (unchanged)\n",
                a.protocol_revision
            ));
        }

        // Protocol content diff
        if a.protocol_content != b.protocol_content {
            output.push_str("  Protocol content: changed\n");
        } else {
            output.push_str("  Protocol content: (unchanged)\n");
        }

        // Workflow diff
        let a_wfs: std::collections::BTreeSet<_> = a.workflow_ids.iter().collect();
        let b_wfs: std::collections::BTreeSet<_> = b.workflow_ids.iter().collect();
        let added: Vec<_> = b_wfs.difference(&a_wfs).collect();
        let removed: Vec<_> = a_wfs.difference(&b_wfs).collect();
        if !added.is_empty() {
            for wf in &added {
                output.push_str(&format!("+ Workflow: {}\n", wf));
            }
        }
        if !removed.is_empty() {
            for wf in &removed {
                output.push_str(&format!("- Workflow: {}\n", wf));
            }
        }
        if added.is_empty() && removed.is_empty() {
            output.push_str("  Workflows: (unchanged)\n");
        }

        // Agent plan diff
        match (&a.agent_plan, &b.agent_plan) {
            (None, None) => output.push_str("  Agent Plan: absent (unchanged)\n"),
            (Some(_), None) => output.push_str("- Agent Plan: present\n+ Agent Plan: absent\n"),
            (None, Some(_)) => output.push_str("- Agent Plan: absent\n+ Agent Plan: present\n"),
            (Some(_), Some(_)) => output.push_str("  Agent Plan: present (unchanged)\n"),
        }

        // Label diff
        if a.label != b.label {
            output.push_str(&format!("- Label:       {:?}\n", a.label));
            output.push_str(&format!("+ Label:       {:?}\n", b.label));
        }

        // Description diff
        if a.description != b.description {
            output.push_str(&format!("- Description: {:?}\n", a.description));
            output.push_str(&format!("+ Description: {:?}\n", b.description));
        }

        // Tags diff
        let a_tags: std::collections::BTreeSet<_> = a.tags.iter().collect();
        let b_tags: std::collections::BTreeSet<_> = b.tags.iter().collect();
        let tags_added: Vec<_> = b_tags.difference(&a_tags).collect();
        let tags_removed: Vec<_> = a_tags.difference(&b_tags).collect();
        if !tags_added.is_empty() {
            for t in &tags_added {
                output.push_str(&format!("+ Tag: {}\n", t));
            }
        }
        if !tags_removed.is_empty() {
            for t in &tags_removed {
                output.push_str(&format!("- Tag: {}\n", t));
            }
        }
        if tags_added.is_empty() && tags_removed.is_empty() {
            output.push_str("  Tags: (unchanged)\n");
        }

        // Parent diff
        if a.parent_id != b.parent_id {
            output.push_str(&format!("- Parent: {:?}\n", a.parent_id));
            output.push_str(&format!("+ Parent: {:?}\n", b.parent_id));
        }

        // Memory ref diff
        if a.memory_ref != b.memory_ref {
            output.push_str(&format!("- Memory ref: {:?}\n", a.memory_ref));
            output.push_str(&format!("+ Memory ref: {:?}\n", b.memory_ref));
        }

        // References diff
        let a_refs: std::collections::BTreeSet<_> = a.enabled_refs.iter().collect();
        let b_refs: std::collections::BTreeSet<_> = b.enabled_refs.iter().collect();
        let refs_added: Vec<_> = b_refs.difference(&a_refs).collect();
        let refs_removed: Vec<_> = a_refs.difference(&b_refs).collect();
        if !refs_added.is_empty() {
            for r in &refs_added {
                output.push_str(&format!("+ Reference: {}\n", r));
            }
        }
        if !refs_removed.is_empty() {
            for r in &refs_removed {
                output.push_str(&format!("- Reference: {}\n", r));
            }
        }
        if refs_added.is_empty() && refs_removed.is_empty() {
            output.push_str("  References: (unchanged)\n");
        }

        Ok(output)
    }

    /// Fork a new strategy from an existing snapshot (or current state).
    ///
    /// Captures current profile, protocol, workflows, references, and memory,
    /// then links the new snapshot to the parent via `parent_id`.
    pub fn fork(
        &mut self,
        project_root: &Path,
        name: &str,
        from_id: Option<&str>,
    ) -> Result<StrategySnapshot> {
        // Determine parent snapshot
        let parent_id = match from_id {
            Some(id) => {
                let parent = self
                    .find_snapshot(id)
                    .ok_or_else(|| anyhow::anyhow!("Source strategy '{}' not found", id))?;
                Some(parent.id.clone())
            }
            None => None,
        };

        // Fork always captures current state
        let mut snapshot = self.record_snapshot_with_refs(project_root, name, parent_id.clone())?;

        // Apply the parent's profile/protocol/workflows if from_id is specified
        if let Some(pid) = &parent_id {
            let parent = self
                .find_snapshot(pid)
                .ok_or_else(|| anyhow::anyhow!("Parent strategy '{}' disappeared", pid))?;
            snapshot.profile_id = parent.profile_id.clone();
            snapshot.profile = parent.profile.clone();
            snapshot.protocol_revision = parent.protocol_revision;
            snapshot.protocol_content = parent.protocol_content.clone();
            snapshot.workflow_ids = parent.workflow_ids.clone();
            snapshot.enabled_refs = parent.enabled_refs.clone();
            snapshot.parent_id = Some(pid.clone());
        }

        // Replace the snapshot in the store
        self.snapshots.pop();
        self.snapshots.push(snapshot.clone());
        self.current = Some(snapshot.id.clone());
        self.save(project_root)?;

        Ok(snapshot)
    }

    /// Create a new strategy from current state (no fork).
    ///
    /// Captures the current profile, protocol, workflows, and references
    /// as a named strategy snapshot.
    pub fn create(&mut self, project_root: &Path, name: &str) -> Result<StrategySnapshot> {
        let mut snapshot = self.record_snapshot(project_root, Some(name.to_string()), None)?;
        snapshot.tags = Vec::new();
        // Update in store
        if let Some(s) = self.snapshots.last_mut() {
            if s.id == snapshot.id {
                s.tags = Vec::new();
            }
        }
        self.save(project_root)?;
        Ok(snapshot)
    }

    /// Internal helper: record a snapshot with optional parent_id and name.
    fn record_snapshot_with_refs(
        &mut self,
        project_root: &Path,
        name: &str,
        parent_id: Option<String>,
    ) -> Result<StrategySnapshot> {
        use crate::constitutive::{Protocol, ReferenceRegistry};
        use crate::profile::ProfileStore;
        use crate::workflow::WorkflowStore;

        // Capture active profile
        let profile_store = ProfileStore::load(project_root)?;
        let active_id = ProfileStore::active_id(project_root)?.unwrap_or_default();
        let profile_json = serde_json::to_value(&profile_store)?;

        // Capture protocol
        let protocol = Protocol::read(project_root)?;
        let protocol_revision = protocol.revision as u32;
        let protocol_content = protocol.body.clone();

        // Capture workflow IDs (enabled workflows)
        let workflow_store = WorkflowStore::load(project_root)?;
        let workflow_ids: Vec<String> = workflow_store
            .workflows
            .iter()
            .filter(|w| w.enabled)
            .map(|w| w.id.clone())
            .collect();

        // Capture enabled reference IDs
        let registry = ReferenceRegistry::read(project_root)?;
        let enabled_refs: Vec<String> = registry
            .entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.id.clone())
            .collect();

        // Capture memory ref if available
        let memory_ref = crate::memory::MemoryStore::load(project_root)
            .ok()
            .and_then(|ms| ms.current_memory().map(|m| m.summary.clone()));

        let snapshot = StrategySnapshot {
            id: route_core::new_id(),
            created_at: route_core::now_millis(),
            profile_id: active_id,
            profile: profile_json,
            protocol_revision,
            protocol_content,
            workflow_ids,
            agent_plan: None,
            label: Some(name.to_string()),
            description: None,
            tags: Vec::new(),
            parent_id,
            memory_ref,
            enabled_refs,
        };

        self.snapshots.push(snapshot.clone());
        Ok(snapshot)
    }

    /// Use a specific strategy (set it as active).
    ///
    /// This does the same as `restore` but with a simpler interface:
    /// it restores the strategy's profile, protocol, and workflows
    /// without creating an auto-save first.
    pub fn use_strategy(&mut self, project_root: &Path, id: &str) -> Result<()> {
        use crate::constitutive::{Protocol, ReferenceRegistry};
        use crate::profile::ProfileStore;
        use crate::workflow::WorkflowStore;

        // Find the snapshot (support prefix matching)
        let snapshot = self
            .find_snapshot(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", id))?;

        // Restore profile
        let profile_store: ProfileStore = serde_json::from_value(snapshot.profile.clone())
            .context("Failed to deserialize profile from snapshot")?;
        profile_store.save(project_root)?;
        if !snapshot.profile_id.is_empty() {
            ProfileStore::set_active(project_root, &snapshot.profile_id)?;
        }

        // Restore protocol
        let protocol_path = crate::constitutive::protocol_path(project_root);
        let rendered = format!(
            "<!-- route-protocol:version={} -->\n\
             <!-- route-protocol:revision={} -->\n\
             <!-- route-protocol:updated_at={} -->\n\
             {}",
            Protocol::CURRENT_VERSION,
            snapshot.protocol_revision,
            route_core::now_millis(),
            snapshot.protocol_content
        );
        write_atomic(&protocol_path, rendered.as_bytes()).context("Failed to restore protocol")?;

        // Restore workflow configuration
        let mut workflow_store = WorkflowStore::load(project_root)?;
        for wf in &mut workflow_store.workflows {
            wf.enabled = snapshot.workflow_ids.contains(&wf.id);
        }
        for wf in &workflow_store.workflows {
            WorkflowStore::save(project_root, wf)?;
        }

        // Restore reference registry enabled/disabled state
        if !snapshot.enabled_refs.is_empty() {
            let mut registry = ReferenceRegistry::read(project_root)?;
            for entry in &mut registry.entries {
                entry.enabled = snapshot.enabled_refs.contains(&entry.id);
            }
            registry.write(project_root)?;
        }

        // Set current to the activated snapshot
        self.current = Some(snapshot.id.clone());
        self.save(project_root)?;

        Ok(())
    }

    /// Compare two strategies and return a human-readable diff.
    ///
    /// Shows differences in: profile, protocol version, workflow set,
    /// references, tags, and agent policy.
    pub fn compare(&self, a_id: &str, b_id: &str) -> Result<String> {
        let a = self
            .find_snapshot(a_id)
            .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", a_id))?;
        let b = self
            .find_snapshot(b_id)
            .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", b_id))?;

        let mut output = String::new();
        output.push_str(&format!("=== Compare Strategies ===\n\n"));
        output.push_str(&format!(
            "--- A: {} ({})\n",
            a.id,
            a.label.as_deref().unwrap_or("unnamed")
        ));
        output.push_str(&format!(
            "+++ B: {} ({})\n",
            b.id,
            b.label.as_deref().unwrap_or("unnamed")
        ));
        output.push('\n');

        // Profile
        if a.profile_id != b.profile_id {
            output.push_str(&format!("- Profile: {}\n", a.profile_id));
            output.push_str(&format!("+ Profile: {}\n", b.profile_id));
        } else {
            output.push_str(&format!("  Profile: {} (unchanged)\n", a.profile_id));
        }

        // Protocol revision
        if a.protocol_revision != b.protocol_revision {
            output.push_str(&format!("- Protocol rev: {}\n", a.protocol_revision));
            output.push_str(&format!("+ Protocol rev: {}\n", b.protocol_revision));
        } else {
            output.push_str(&format!(
                "  Protocol rev: {} (unchanged)\n",
                a.protocol_revision
            ));
        }

        // Protocol content
        if a.protocol_content != b.protocol_content {
            output.push_str("  Protocol content: changed\n");
        } else {
            output.push_str("  Protocol content: (unchanged)\n");
        }

        // Workflows
        let a_wfs: std::collections::BTreeSet<_> = a.workflow_ids.iter().collect();
        let b_wfs: std::collections::BTreeSet<_> = b.workflow_ids.iter().collect();
        let added: Vec<_> = b_wfs.difference(&a_wfs).collect();
        let removed: Vec<_> = a_wfs.difference(&b_wfs).collect();
        if !added.is_empty() {
            for wf in &added {
                output.push_str(&format!("+ Workflow: {}\n", wf));
            }
        }
        if !removed.is_empty() {
            for wf in &removed {
                output.push_str(&format!("- Workflow: {}\n", wf));
            }
        }
        if added.is_empty() && removed.is_empty() {
            output.push_str("  Workflows: (unchanged)\n");
        }

        // References
        let a_refs: std::collections::BTreeSet<_> = a.enabled_refs.iter().collect();
        let b_refs: std::collections::BTreeSet<_> = b.enabled_refs.iter().collect();
        let refs_added: Vec<_> = b_refs.difference(&a_refs).collect();
        let refs_removed: Vec<_> = a_refs.difference(&b_refs).collect();
        if !refs_added.is_empty() {
            for r in &refs_added {
                output.push_str(&format!("+ Reference: {}\n", r));
            }
        }
        if !refs_removed.is_empty() {
            for r in &refs_removed {
                output.push_str(&format!("- Reference: {}\n", r));
            }
        }
        if refs_added.is_empty() && refs_removed.is_empty() {
            output.push_str("  References: (unchanged)\n");
        }

        // Tags
        let a_tags: std::collections::BTreeSet<_> = a.tags.iter().collect();
        let b_tags: std::collections::BTreeSet<_> = b.tags.iter().collect();
        let tags_added: Vec<_> = b_tags.difference(&a_tags).collect();
        let tags_removed: Vec<_> = a_tags.difference(&b_tags).collect();
        if !tags_added.is_empty() || !tags_removed.is_empty() {
            for t in &tags_added {
                output.push_str(&format!("+ Tag: {}\n", t));
            }
            for t in &tags_removed {
                output.push_str(&format!("- Tag: {}\n", t));
            }
        } else {
            output.push_str("  Tags: (unchanged)\n");
        }

        // Agent plan
        match (&a.agent_plan, &b.agent_plan) {
            (None, None) => output.push_str("  Agent Plan: absent (unchanged)\n"),
            (Some(_), None) => output.push_str("- Agent Plan: present\n+ Agent Plan: absent\n"),
            (None, Some(_)) => output.push_str("- Agent Plan: absent\n+ Agent Plan: present\n"),
            (Some(_), Some(_)) => output.push_str("  Agent Plan: present (unchanged)\n"),
        }

        // Parent
        if a.parent_id != b.parent_id {
            output.push_str(&format!("- Parent: {:?}\n", a.parent_id));
            output.push_str(&format!("+ Parent: {:?}\n", b.parent_id));
        }

        // Memory ref
        if a.memory_ref != b.memory_ref {
            output.push_str(&format!("- Memory ref: {:?}\n", a.memory_ref));
            output.push_str(&format!("+ Memory ref: {:?}\n", b.memory_ref));
        }

        Ok(output)
    }

    /// Delete a non-current strategy.
    pub fn delete(&mut self, project_root: &Path, id: &str) -> Result<()> {
        let snapshot = self
            .find_snapshot(id)
            .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", id))?;

        // Cannot delete current strategy
        if self.current.as_ref().map_or(false, |c| c == &snapshot.id) {
            anyhow::bail!("Cannot delete the current strategy. Use 'use' to switch to another strategy first.");
        }

        // Find full index (not just by prefix)
        let idx = self
            .snapshots
            .iter()
            .position(|s| s.id == snapshot.id)
            .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found in store", id))?;

        self.snapshots.remove(idx);
        self.save(project_root)?;

        Ok(())
    }

    /// Get the current strategy ID.
    pub fn current_strategy(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Get strategies by tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&StrategySnapshot> {
        self.snapshots
            .iter()
            .filter(|s| s.tags.iter().any(|t| t == tag))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_strategy_dir_and_path() {
        let tmp = TempDir::new().unwrap();
        let dir = strategy_dir(tmp.path());
        assert!(dir.ends_with(".route/strategy"));
        let p = strategy_path(tmp.path());
        assert!(p.ends_with(".route/strategy/strategy.json"));
    }

    #[test]
    fn test_load_empty_store() {
        let tmp = TempDir::new().unwrap();
        let store = StrategyStore::load(tmp.path()).unwrap();
        assert!(store.snapshots.is_empty());
        assert!(store.current.is_none());
    }

    #[test]
    fn test_record_and_list() {
        let tmp = TempDir::new().unwrap();
        // Initialize a minimal environment
        crate::init_profile_store(tmp.path()).unwrap();

        let mut store = StrategyStore::default();
        let snap = store
            .record_snapshot(
                tmp.path(),
                Some("test-label".to_string()),
                Some("test description".to_string()),
            )
            .unwrap();

        assert_eq!(snap.label.as_deref(), Some("test-label"));
        assert_eq!(snap.description.as_deref(), Some("test description"));
        assert_eq!(store.current.as_deref(), Some(snap.id.as_str()));

        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, snap.id);
    }

    #[test]
    fn test_get_by_prefix() {
        let tmp = TempDir::new().unwrap();
        crate::init_profile_store(tmp.path()).unwrap();

        let mut store = StrategyStore::default();
        let snap = store.record_snapshot(tmp.path(), None, None).unwrap();

        let prefix = &snap.id[..8];
        let found = store.get(prefix);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, snap.id);
    }

    #[test]
    fn test_show_and_diff() {
        let tmp = TempDir::new().unwrap();
        crate::init_profile_store(tmp.path()).unwrap();

        let mut store = StrategyStore::default();
        let a = store
            .record_snapshot(tmp.path(), Some("first".to_string()), None)
            .unwrap();
        let b = store
            .record_snapshot(tmp.path(), Some("second".to_string()), None)
            .unwrap();

        let show_a = StrategyStore::show(&a.id, &store).unwrap();
        assert!(show_a.contains(&a.id));

        let diff = StrategyStore::diff(&a.id, &b.id, &store).unwrap();
        assert!(diff.contains(&a.id));
        assert!(diff.contains(&b.id));
    }

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        crate::init_profile_store(tmp.path()).unwrap();

        let mut store = StrategyStore::default();
        store.record_snapshot(tmp.path(), None, None).unwrap();

        // Save and reload
        store.save(tmp.path()).unwrap();
        let loaded = StrategyStore::load(tmp.path()).unwrap();
        assert_eq!(loaded.snapshots.len(), 1);
        assert_eq!(loaded.snapshots[0].id, store.snapshots[0].id);
    }

    #[test]
    fn test_restore() {
        let tmp = TempDir::new().unwrap();
        crate::init_profile_store(tmp.path()).unwrap();

        // Create initial state
        let mut store = StrategyStore::default();
        let snap = store
            .record_snapshot(tmp.path(), Some("original".to_string()), None)
            .unwrap();

        // Restore the snapshot
        let mut store2 = StrategyStore::load(tmp.path()).unwrap();
        store2.restore(tmp.path(), &snap.id).unwrap();

        // Should have auto-saved before restore, so we have original + auto-save
        let loaded = StrategyStore::load(tmp.path()).unwrap();
        assert_eq!(loaded.snapshots.len(), 2);
        assert_eq!(loaded.current.as_deref(), Some(snap.id.as_str()));
    }
}
