//! Capability Pack — bundle workflows, skills, tools, references, and strategy.
//!
//! Packs only store references + metadata — they do NOT copy external projects.
//! A pack can be imported/exported as a portable JSON file.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Pack directory under `.route/`.
pub fn pack_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("pack")
}

/// Path to the pack index file.
pub fn pack_index_path(project_root: &Path) -> PathBuf {
    pack_dir(project_root).join("index.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A capability pack — a named bundle of references to Route resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPack {
    /// Unique pack ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Optional description
    #[serde(default)]
    pub description: String,
    /// Version of this pack
    #[serde(default = "default_version")]
    pub version: String,
    /// Workflow IDs included in this pack
    #[serde(default)]
    pub workflow_ids: Vec<String>,
    /// Skill reference IDs included
    #[serde(default)]
    pub skill_ids: Vec<String>,
    /// Tool/CLI reference IDs included
    #[serde(default)]
    pub tool_ids: Vec<String>,
    /// General reference IDs included
    #[serde(default)]
    pub reference_ids: Vec<String>,
    /// Capability IDs included
    #[serde(default)]
    pub capability_ids: Vec<String>,
    /// Default strategy ID to use when this pack is active
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_strategy_id: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// When this pack was created (unix timestamp)
    pub created_at: i64,
    /// When this pack was last updated (unix timestamp)
    pub updated_at: i64,
    /// Whether this pack is currently enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Source project (if imported from a study)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_true() -> bool {
    true
}

/// The pack index — tracks all packs in the project.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackIndex {
    pub packs: Vec<CapabilityPack>,
    /// Which packs are currently active (by ID)
    #[serde(default)]
    pub active_pack_ids: Vec<String>,
}

impl PackIndex {
    /// Load pack index from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = pack_index_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save pack index to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = pack_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(pack_index_path(project_root), json)?;
        Ok(())
    }

    /// List all packs.
    pub fn list(&self) -> &[CapabilityPack] {
        &self.packs
    }

    /// Get a pack by ID.
    pub fn get(&self, id: &str) -> Option<&CapabilityPack> {
        self.packs.iter().find(|p| p.id == id)
    }

    /// Get a mutable pack by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut CapabilityPack> {
        self.packs.iter_mut().find(|p| p.id == id)
    }

    /// Add a pack.
    pub fn add(&mut self, pack: CapabilityPack) {
        self.packs.push(pack);
    }

    /// Remove a pack by ID.
    pub fn remove(&mut self, id: &str) {
        self.packs.retain(|p| p.id != id);
        self.active_pack_ids.retain(|aid| aid != id);
    }

    /// Activate a pack by ID.
    pub fn activate(&mut self, id: &str) -> Result<()> {
        if self.get(id).is_none() {
            return Err(anyhow::anyhow!("Pack '{}' not found", id));
        }
        if !self.active_pack_ids.contains(&id.to_string()) {
            self.active_pack_ids.push(id.to_string());
        }
        Ok(())
    }

    /// Deactivate a pack by ID.
    pub fn deactivate(&mut self, id: &str) {
        self.active_pack_ids.retain(|aid| aid != id);
    }

    /// Get all active packs.
    pub fn active_packs(&self) -> Vec<&CapabilityPack> {
        self.active_pack_ids
            .iter()
            .filter_map(|id| self.get(id))
            .collect()
    }

    /// Export a pack to a portable JSON string.
    pub fn export_pack(&self, id: &str) -> Result<String> {
        let pack = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Pack '{}' not found", id))?;
        Ok(serde_json::to_string_pretty(pack)?)
    }

    /// Import a pack from a JSON string.
    pub fn import_pack(&mut self, json: &str) -> Result<String> {
        let pack: CapabilityPack = serde_json::from_str(json)?;
        let id = pack.id.clone();
        self.add(pack);
        Ok(id)
    }

    /// Create a new pack with the given resources.
    pub fn create_pack(
        &mut self,
        name: &str,
        description: &str,
        workflow_ids: Vec<String>,
        skill_ids: Vec<String>,
        tool_ids: Vec<String>,
        reference_ids: Vec<String>,
        capability_ids: Vec<String>,
        default_strategy_id: Option<String>,
        tags: Vec<String>,
        source: Option<String>,
    ) -> Result<CapabilityPack> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let id = format!("pack-{}", slugify(name));

        let pack = CapabilityPack {
            id: id.clone(),
            name: name.to_string(),
            description: description.to_string(),
            version: "1.0.0".to_string(),
            workflow_ids,
            skill_ids,
            tool_ids,
            reference_ids,
            capability_ids,
            default_strategy_id,
            tags,
            created_at: now,
            updated_at: now,
            enabled: true,
            source,
        };

        self.add(pack.clone());
        Ok(pack)
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Format a pack for display.
pub fn format_pack(pack: &CapabilityPack) -> String {
    format!(
        r#"Pack: {name} ({id})
  Description: {desc}
  Version:     {ver}
  Workflows:   {wfs}
  Skills:      {skills}
  Tools:       {tools}
  References:  {refs}
  Capabilities: {caps}
  Strategy:    {strat}
  Tags:        {tags}
  Enabled:     {enabled}
  Source:      {src}
  Created:     {created}
  Updated:     {updated}"#,
        name = pack.name,
        id = pack.id,
        desc = pack.description,
        ver = pack.version,
        wfs = pack.workflow_ids.join(", "),
        skills = pack.skill_ids.join(", "),
        tools = pack.tool_ids.join(", "),
        refs = pack.reference_ids.join(", "),
        caps = pack.capability_ids.join(", "),
        strat = pack.default_strategy_id.as_deref().unwrap_or("(none)"),
        tags = pack.tags.join(", "),
        enabled = if pack.enabled { "yes" } else { "no" },
        src = pack.source.as_deref().unwrap_or("(none)"),
        created = pack.created_at,
        updated = pack.updated_at,
    )
}
