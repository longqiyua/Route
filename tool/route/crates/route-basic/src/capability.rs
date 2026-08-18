//! Capability Model — unified view over Reference entries.
//!
//! Each Capability wraps a Reference entry with a standardized kind,
//! maturity level, and usage metadata. Route does not reimplement tools;
//! capabilities describe what the host AI can discover and use.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::{ReferenceEntry, ReferenceRegistry, ROUTE_DOT_DIR};

/// Capability directory under `.route/`.
pub fn capability_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("capability")
}

/// Path to the capability registry file.
pub fn capability_registry_path(project_root: &Path) -> PathBuf {
    capability_dir(project_root).join("registry.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The kind of capability a Reference entry provides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    /// Declarative knowledge about a domain (docs, specs, decisions)
    Knowledge,
    /// How-to instructions for a specific task
    Instruction,
    /// A concrete tool/command/script
    Tool,
    /// A reusable AI skill (like a prompt template or agent capability)
    Skill,
    /// A development workflow (CI/CD, review, release)
    Workflow,
    /// A network service / API endpoint
    Service,
    /// A runtime environment (language, framework, platform)
    Runtime,
}

impl Default for CapabilityKind {
    fn default() -> Self {
        Self::Knowledge
    }
}

/// Maturity level of a capability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CapabilityLevel {
    /// Referenceable — the entry exists and can be cited
    L1,
    /// Understandable — AI knows how to use it
    L2,
    /// Executable — the host environment can actually invoke it
    L3,
}

impl Default for CapabilityLevel {
    fn default() -> Self {
        Self::L1
    }
}

/// A unified capability view wrapping a Reference entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Unique capability ID (usually matches the reference entry id)
    pub id: String,
    /// The reference entry this capability wraps
    pub reference_id: String,
    /// Human-readable name
    pub name: String,
    /// Kind of capability
    pub kind: CapabilityKind,
    /// Maturity level
    pub level: CapabilityLevel,
    /// Optional entrypoint — how to invoke/use this (command, path, URL)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Usage instructions / how to call this capability
    #[serde(default)]
    pub usage: String,
    /// Expected inputs (file paths, arguments, environment)
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Expected outputs (results, files, side effects)
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Required permissions to use this capability
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Constraints on usage
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Whether this capability is currently available
    #[serde(default = "default_availability")]
    pub availability: bool,
}

fn default_availability() -> bool {
    true
}

/// The capability registry — a collection of capabilities mapped from references.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityRegistry {
    pub capabilities: Vec<Capability>,
}

impl CapabilityRegistry {
    /// Load the capability registry from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = capability_registry_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save the capability registry to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = capability_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(capability_registry_path(project_root), json)?;
        Ok(())
    }

    /// List all capabilities, optionally filtered by kind.
    pub fn list(&self, kind: Option<&str>) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| {
                kind.map_or(true, |k| {
                    format!("{:?}", c.kind).to_lowercase() == k.to_lowercase()
                })
            })
            .collect()
    }

    /// Get a capability by ID.
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.capabilities.iter().find(|c| c.id == id)
    }

    /// Add or update a capability.
    pub fn set(&mut self, cap: Capability) {
        if let Some(existing) = self.capabilities.iter_mut().find(|c| c.id == cap.id) {
            *existing = cap;
        } else {
            self.capabilities.push(cap);
        }
    }

    /// Remove a capability by ID.
    pub fn remove(&mut self, id: &str) {
        self.capabilities.retain(|c| c.id != id);
    }

    /// In (scan / auto-discover) capabilities from a `ReferenceRegistry`.
    ///
    /// Maps each entry to a capability based on its type:
    /// - `document` → Knowledge
    /// - `skill` → Skill
    /// - `cli` / `executable` → Tool (L3 if available, L2 otherwise)
    /// - `mcp` → Service
    /// - `workflow` → Workflow
    /// - `repo` / `api` → Knowledge
    ///
    /// Existing capabilities are not overwritten unless `overwrite` is true.
    pub fn discover_from_registry(
        &mut self,
        registry: &ReferenceRegistry,
        overwrite: bool,
    ) -> Result<Vec<String>> {
        let mut discovered = Vec::new();

        for entry in &registry.entries {
            if !overwrite && self.capabilities.iter().any(|c| c.reference_id == entry.id) {
                continue;
            }

            let (kind, level) = Self::infer_from_entry(entry);
            let cap = Capability {
                id: format!("cap-{}", entry.id),
                reference_id: entry.id.clone(),
                name: entry.name.clone(),
                kind,
                level,
                entrypoint: entry.entrypoint.clone(),
                usage: entry.description.clone(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                permissions: Vec::new(),
                constraints: if entry.constraints.is_empty() {
                    Vec::new()
                } else {
                    vec![entry.constraints.clone()]
                },
                availability: entry.enabled,
            };

            self.set(cap);
            discovered.push(entry.id.clone());
        }

        Ok(discovered)
    }

    fn infer_from_entry(entry: &ReferenceEntry) -> (CapabilityKind, CapabilityLevel) {
        use crate::constitutive::ReferenceType;
        match entry.type_ {
            ReferenceType::Document => (CapabilityKind::Knowledge, CapabilityLevel::L1),
            ReferenceType::Skill => (CapabilityKind::Skill, CapabilityLevel::L2),
            ReferenceType::Cli | ReferenceType::Executable => {
                (CapabilityKind::Tool, CapabilityLevel::L3)
            }
            ReferenceType::Mcp => (CapabilityKind::Service, CapabilityLevel::L3),
            ReferenceType::Workflow => (CapabilityKind::Workflow, CapabilityLevel::L2),
            ReferenceType::Repo => (CapabilityKind::Knowledge, CapabilityLevel::L1),
            ReferenceType::Api => (CapabilityKind::Service, CapabilityLevel::L2),
            _ => (CapabilityKind::Knowledge, CapabilityLevel::L1),
        }
    }

    /// Inspect a capability — return a detailed text description.
    pub fn inspect(&self, id: &str) -> Result<String> {
        let cap = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Capability '{}' not found", id))?;

        Ok(format!(
            r#"Capability: {name} ({id})
  Reference:  {ref_id}
  Kind:       {kind:?}
  Level:      {level:?}
  Entrypoint: {entry}
  Usage:      {usage}
  Inputs:     {inputs}
  Outputs:    {outputs}
  Permissions: {perms}
  Constraints: {constraints}
  Available:  {avail}"#,
            name = cap.name,
            id = cap.id,
            ref_id = cap.reference_id,
            kind = cap.kind,
            level = cap.level,
            entry = cap.entrypoint.as_deref().unwrap_or("(none)"),
            usage = cap.usage,
            inputs = cap.inputs.join(", "),
            outputs = cap.outputs.join(", "),
            perms = cap.permissions.join(", "),
            constraints = cap.constraints.join(", "),
            avail = if cap.availability { "yes" } else { "no" },
        ))
    }
}
