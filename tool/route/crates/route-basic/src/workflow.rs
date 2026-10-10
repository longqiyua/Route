//! Workflow definition — a high-level executable description stored as a
//! Reference subtype. Workflows describe multi-step processes that can be
//! bound to Skills, References, and Profiles.
//!
//! Route never executes workflows. It discovers, describes, selects, and
//! exposes them to the host AI context pipeline. Execution is always the
//! responsibility of the host environment.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{write_atomic, Origin, ReferenceType};
use crate::study::StudyCandidate;

/// Directory name under `.route/workflows/`.
pub const WORKFLOWS_DIR: &str = "workflows";

/// Canonical workflows directory path.
pub fn workflow_dir(project_root: &Path) -> PathBuf {
    let dot = project_root.join(crate::constitutive::ROUTE_DOT_DIR);
    dot.join(WORKFLOWS_DIR)
}

/// Path to a single workflow definition file.
pub fn workflow_path(project_root: &Path, id: &str) -> PathBuf {
    workflow_dir(project_root).join(format!("{}.json", id))
}

/// A single workflow step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// Step name / label.
    pub name: String,
    /// Optional description of what this step does.
    #[serde(default)]
    pub description: String,
    /// Optional reference IDs that this step needs.
    #[serde(default)]
    pub references: Vec<String>,
    /// Optional skill IDs that this step uses.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Optional command or tool to invoke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Optional expected outcome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_outcome: Option<String>,
}

/// A high-level workflow definition.
///
/// Workflows are **not** a new top-level world model. They are a Reference
/// subtype that describes a multi-step process. The actual execution is
/// delegated to the host AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// Stable id, e.g. `wf-taiyi-forge` or `wf-build-release`.
    pub id: String,
    /// Human-readable display name.
    #[serde(default)]
    pub name: String,
    /// Where the workflow was sourced from (local path, URL, etc.).
    #[serde(default)]
    pub source: String,
    /// One-sentence description.
    #[serde(default)]
    pub description: String,
    /// Optional list of steps. Steps may be empty for external workflows
    /// that only provide source + usage instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<Vec<WorkflowStep>>,
    /// Reference IDs of skills this workflow uses.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Reference IDs this workflow depends on.
    #[serde(default)]
    pub references: Vec<String>,
    /// Optional agent policy override (JSON or inline text).
    ///
    /// When set, the host AI should use this policy to guide its behaviour
    /// while executing this workflow. The policy can reference a named
    /// agent policy from the project's Reference registry or be an inline
    /// instruction string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_policy: Option<String>,
    /// Optional verification policy override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_policy: Option<String>,
    /// Whether this workflow is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Unix-millis when this workflow was created.
    pub created_at: i64,
    /// Provenance.
    #[serde(default)]
    pub origin: Origin,
    /// Optional tags.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Original upstream source (if imported)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,

    /// Base version of the workflow (0 = original)
    #[serde(default)]
    pub base_version: u32,

    /// Current revision number (starts at 1, incremented on evolve)
    #[serde(default)]
    pub current_revision: u32,
}

fn default_enabled() -> bool {
    true
}

impl WorkflowDefinition {
    /// Create a new workflow with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            source: String::new(),
            description: String::new(),
            steps: None,
            skills: Vec::new(),
            references: Vec::new(),
            agent_policy: None,
            verification_policy: None,
            enabled: true,
            created_at: route_core::now_millis(),
            origin: Origin::UserCreated,
            tags: Vec::new(),
            upstream: None,
            base_version: 0,
            current_revision: 0,
        }
    }

    /// Build a ReferenceEntry from this workflow for context injection.
    pub fn to_reference_entry(&self) -> crate::constitutive::ReferenceEntry {
        let cap = format!("Workflow: {}. {}", self.name, self.description);
        let mut tags = self.tags.clone();
        if !tags.contains(&"workflow".to_string()) {
            tags.push("workflow".to_string());
        }
        crate::constitutive::ReferenceEntry::builder(
            &self.id,
            ReferenceType::Workflow,
            &self.source,
            &cap,
        )
        .with_name(&self.name)
        .with_enabled(self.enabled)
        .with_tags(tags)
        .with_created_at(self.created_at)
        .with_origin(self.origin)
        .build()
    }
}

/// Store for workflow definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowStore {
    pub workflows: Vec<WorkflowDefinition>,
}

impl WorkflowStore {
    /// Load workflows from the workflows directory.
    pub fn load(project_root: &Path) -> Result<Self> {
        let dir = workflow_dir(project_root);
        if !dir.exists() {
            return Ok(Self::default());
        }
        let mut workflows = Vec::new();
        let mut _read_errs = 0usize;
        for entry in std::fs::read_dir(&dir).context("reading workflows directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "json") {
                continue;
            }
            let raw = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => {
                    _read_errs += 1;
                    continue;
                }
            };
            if let Ok(wf) = serde_json::from_str::<WorkflowDefinition>(&raw) {
                workflows.push(wf);
            }
        }
        Ok(Self { workflows })
    }

    /// Save a single workflow definition to disk.
    pub fn save(project_root: &Path, wf: &WorkflowDefinition) -> Result<()> {
        let dir = workflow_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let path = workflow_path(project_root, &wf.id);
        let json = serde_json::to_vec_pretty(wf)?;
        write_atomic(&path, &json)
            .with_context(|| format!("writing workflow '{}' to {}", wf.id, path.display()))?;
        Ok(())
    }

    /// Find a workflow by id.
    pub fn get(&self, id: &str) -> Option<&WorkflowDefinition> {
        self.workflows.iter().find(|w| w.id == id)
    }

    /// Get a mutable reference to a workflow by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkflowDefinition> {
        self.workflows.iter_mut().find(|w| w.id == id)
    }

    /// Index of a workflow by id.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.workflows.iter().position(|w| w.id == id)
    }

    /// Upsert a workflow. Returns true if inserted, false if updated.
    pub fn upsert(&mut self, wf: WorkflowDefinition) -> bool {
        if let Some(existing) = self.get_mut(&wf.id) {
            *existing = wf;
            false
        } else {
            self.workflows.push(wf);
            true
        }
    }

    /// Remove a workflow by id. Returns true if removed.
    pub fn remove(&mut self, id: &str) -> bool {
        let idx = self.index_of(id);
        if let Some(i) = idx {
            self.workflows.remove(i);
            // Also delete the file on disk
            true
        } else {
            false
        }
    }

    /// Delete a workflow file from disk.
    pub fn delete_file(project_root: &Path, id: &str) -> Result<()> {
        let path = workflow_path(project_root, id);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("deleting workflow file {}", path.display()))?;
        }
        Ok(())
    }

    /// Return only enabled workflows.
    pub fn enabled(&self) -> Vec<&WorkflowDefinition> {
        self.workflows.iter().filter(|w| w.enabled).collect()
    }

    /// Enable a workflow by id. Returns an error if not found.
    pub fn enable(&mut self, id: &str) -> Result<bool> {
        let wf = self
            .get_mut(id)
            .ok_or_else(|| anyhow!("Workflow '{}' not found", id))?;
        wf.enabled = true;
        Ok(true)
    }

    /// Disable a workflow by id. Returns an error if not found.
    pub fn disable(&mut self, id: &str) -> Result<bool> {
        let wf = self
            .get_mut(id)
            .ok_or_else(|| anyhow!("Workflow '{}' not found", id))?;
        wf.enabled = false;
        Ok(true)
    }

    /// Apply a change proposal to the workflow.
    ///
    /// Increments the workflow's `current_revision` to match the proposal's
    /// `to_revision`. Does NOT save to disk — the caller must call `save`.
    /// Does NOT modify the proposal store — the caller must update the
    /// proposal status separately.
    pub fn evolve(&mut self, id: &str, proposal: &WorkflowChangeProposal) -> Result<()> {
        let wf = self
            .get_mut(id)
            .ok_or_else(|| anyhow!("Workflow '{}' not found", id))?;
        wf.current_revision = proposal.to_revision;
        Ok(())
    }

    /// Reset a workflow to the original upstream version.
    ///
    /// Resets `base_version` and `current_revision` to 0, and clears the
    /// `upstream` field. Does NOT save to disk.
    pub fn reset_to_upstream(&mut self, id: &str) -> Result<()> {
        let wf = self
            .get_mut(id)
            .ok_or_else(|| anyhow!("Workflow '{}' not found", id))?;
        wf.base_version = 0;
        wf.current_revision = 0;
        wf.upstream = None;
        Ok(())
    }

    /// Reset a workflow to a specific revision number.
    ///
    /// Sets `current_revision` to the given value. Does NOT save to disk.
    pub fn reset_to_revision(&mut self, id: &str, revision: u32) -> Result<()> {
        let wf = self
            .get_mut(id)
            .ok_or_else(|| anyhow!("Workflow '{}' not found", id))?;
        wf.current_revision = revision;
        Ok(())
    }
}

/// A proposal to evolve a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowChangeProposal {
    pub id: String,
    pub workflow_id: String,
    pub before: String,
    pub after: String,
    pub reason: String,
    pub evidence: Vec<String>,
    pub expected_effect: String,
    pub risk: String,
    pub from_revision: u32,
    pub to_revision: u32,
    pub status: String,
    pub created_at: i64,
}

/// Store for workflow change proposals.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowProposalStore {
    pub proposals: Vec<WorkflowChangeProposal>,
}

/// Canonical workflow proposals directory path.
pub fn workflow_proposal_dir(project_root: &Path) -> PathBuf {
    workflow_dir(project_root)
}

/// Path to the workflow proposals file.
pub fn workflow_proposal_path(project_root: &Path) -> PathBuf {
    workflow_dir(project_root).join("proposals.json")
}

impl WorkflowProposalStore {
    /// Load proposals from the proposals file.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = workflow_proposal_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading workflow proposals from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        let s: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing workflow proposals JSON at {}", p.display()))?;
        Ok(s)
    }

    /// Save proposals to the proposals file.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = workflow_proposal_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing workflow proposals to {}", p.display()))?;
        Ok(())
    }

    /// Add a new proposal.
    pub fn add(&mut self, proposal: WorkflowChangeProposal) {
        self.proposals.push(proposal);
    }

    /// List all proposals.
    pub fn list(&self) -> Vec<&WorkflowChangeProposal> {
        self.proposals.iter().collect()
    }

    /// Get a proposal by id (supports prefix matching).
    pub fn get(&self, id: &str) -> Option<&WorkflowChangeProposal> {
        self.proposals
            .iter()
            .find(|p| p.id == id || p.id.starts_with(id))
    }

    /// Get a mutable proposal by id (supports prefix matching).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WorkflowChangeProposal> {
        self.proposals
            .iter_mut()
            .find(|p| p.id == id || p.id.starts_with(id))
    }
}

/// Generate a workflow evolve proposal from project context.
///
/// Creates a `WorkflowChangeProposal` with the workflow's current state
/// as the "before" description and a generic "after" placeholder. The
/// caller should provide a meaningful reason, expected effect, and risk
/// assessment. The proposal is NOT automatically saved to the store.
pub fn generate_evolve_proposal(
    project_root: &Path,
    workflow_id: &str,
    reason: &str,
    expected_effect: &str,
    risk: &str,
) -> WorkflowChangeProposal {
    let store = WorkflowStore::load(project_root).unwrap_or_default();
    let before = store
        .get(workflow_id)
        .map(|wf| {
            format!(
                "Workflow '{}' (v{}.{})",
                wf.id, wf.base_version, wf.current_revision
            )
        })
        .unwrap_or_else(|| format!("Workflow '{}' (unknown)", workflow_id));

    let current_revision = store
        .get(workflow_id)
        .map(|wf| wf.current_revision)
        .unwrap_or(0);

    let after = format!(
        "Evolved workflow '{}' (v{} -> v{})",
        workflow_id,
        current_revision,
        current_revision + 1
    );

    WorkflowChangeProposal {
        id: route_core::new_id(),
        workflow_id: workflow_id.to_string(),
        before,
        after,
        reason: reason.to_string(),
        evidence: Vec::new(),
        expected_effect: expected_effect.to_string(),
        risk: risk.to_string(),
        from_revision: current_revision,
        to_revision: current_revision + 1,
        status: "pending".to_string(),
        created_at: route_core::now_millis(),
    }
}

/// Create a `WorkflowDefinition` from a `StudyCandidate`.
///
/// This is useful for importing a workflow discovered during project study.
/// If `id_override` is provided, it will be used as the workflow id;
/// otherwise the candidate's name is used.
pub fn workflow_from_candidate(
    candidate: &StudyCandidate,
    id_override: Option<&str>,
) -> WorkflowDefinition {
    let wf_id = id_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| candidate.name.clone());
    let description = format!("{} (evidence: {})", candidate.reason, candidate.evidence);

    WorkflowDefinition {
        id: wf_id,
        name: candidate.name.clone(),
        source: candidate.source.clone(),
        description,
        steps: None,
        skills: Vec::new(),
        references: Vec::new(),
        agent_policy: None,
        verification_policy: None,
        enabled: true,
        created_at: route_core::now_millis(),
        origin: Origin::Imported,
        tags: vec!["workflow".to_string(), "study".to_string()],
        upstream: None,
        base_version: 0,
        current_revision: 0,
    }
}
