//! Tool/Resource Binding — associate resources with tasks.
//!
//! A TaskBinding defines which resources a task can access.
//! The AgentCompiler can only use bound/selected resources.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Binding directory under `.route/`.
pub fn binding_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("binding")
}

/// Path to the bindings file.
pub fn binding_path(project_root: &Path) -> PathBuf {
    binding_dir(project_root).join("bindings.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A binding — associates a set of resources with a task or session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBinding {
    /// The session/task ID this binding belongs to
    pub session_id: String,
    /// Task description
    pub task: String,
    /// Memory item IDs bound to this task
    #[serde(default)]
    pub memory_ids: Vec<String>,
    /// Reference entry IDs bound to this task
    #[serde(default)]
    pub reference_ids: Vec<String>,
    /// Workflow IDs bound to this task
    #[serde(default)]
    pub workflow_ids: Vec<String>,
    /// Capability IDs bound to this task
    #[serde(default)]
    pub capability_ids: Vec<String>,
    /// Strategy ID bound to this task
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// Pack IDs bound to this task
    #[serde(default)]
    pub pack_ids: Vec<String>,
    /// When this binding was created
    pub created_at: i64,
    /// Whether this binding is automatically generated
    #[serde(default)]
    pub automatic: bool,
}

/// The binding store — tracks all task bindings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BindingStore {
    pub bindings: Vec<TaskBinding>,
}

impl BindingStore {
    /// Load bindings from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = binding_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save bindings to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = binding_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(binding_path(project_root), json)?;
        Ok(())
    }

    /// Create a binding for a session.
    pub fn bind(
        &mut self,
        session_id: &str,
        task: &str,
        memory_ids: Vec<String>,
        reference_ids: Vec<String>,
        workflow_ids: Vec<String>,
        capability_ids: Vec<String>,
        strategy_id: Option<String>,
        pack_ids: Vec<String>,
        automatic: bool,
    ) -> Result<TaskBinding> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let binding = TaskBinding {
            session_id: session_id.to_string(),
            task: task.to_string(),
            memory_ids,
            reference_ids,
            workflow_ids,
            capability_ids,
            strategy_id,
            pack_ids,
            created_at: now,
            automatic,
        };

        self.bindings.push(binding.clone());
        Ok(binding)
    }

    /// Get binding for a session.
    pub fn get_for_session(&self, session_id: &str) -> Option<&TaskBinding> {
        self.bindings.iter().find(|b| b.session_id == session_id)
    }

    /// List all bindings.
    pub fn list(&self) -> &[TaskBinding] {
        &self.bindings
    }

    /// Remove bindings for a session.
    pub fn remove_for_session(&mut self, session_id: &str) {
        self.bindings.retain(|b| b.session_id != session_id);
    }

    /// Inspect what resources a task can use, with explanations.
    pub fn inspect_resources(&self, session_id: &str) -> Result<String> {
        let binding = self
            .get_for_session(session_id)
            .ok_or_else(|| anyhow::anyhow!("No binding found for session '{}'", session_id))?;

        let mut out = format!(
            "Task: {}\nSession: {}\n\nResources:\n",
            binding.task, binding.session_id
        );

        if !binding.memory_ids.is_empty() {
            out.push_str(&format!(
                "  Memory items ({}): {}\n",
                binding.memory_ids.len(),
                binding.memory_ids.join(", ")
            ));
        }
        if !binding.reference_ids.is_empty() {
            out.push_str(&format!(
                "  References ({}): {}\n",
                binding.reference_ids.len(),
                binding.reference_ids.join(", ")
            ));
        }
        if !binding.workflow_ids.is_empty() {
            out.push_str(&format!(
                "  Workflows ({}): {}\n",
                binding.workflow_ids.len(),
                binding.workflow_ids.join(", ")
            ));
        }
        if !binding.capability_ids.is_empty() {
            out.push_str(&format!(
                "  Capabilities ({}): {}\n",
                binding.capability_ids.len(),
                binding.capability_ids.join(", ")
            ));
        }
        if let Some(sid) = &binding.strategy_id {
            out.push_str(&format!("  Strategy: {}\n", sid));
        }
        if !binding.pack_ids.is_empty() {
            out.push_str(&format!(
                "  Packs ({}): {}\n",
                binding.pack_ids.len(),
                binding.pack_ids.join(", ")
            ));
        }

        out.push_str(&format!(
            "\nBinding type: {}\n",
            if binding.automatic { "auto" } else { "manual" }
        ));
        Ok(out)
    }
}
