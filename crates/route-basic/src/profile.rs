//! ProjectProfile — a named preset that bundles Protocol, Workflow, and
//! Reference selections into a single active configuration.
//!
//! Profile is the top-level user-facing knob: switching profiles changes
//! what the AI sees and how it behaves. Only one profile is active at a
//! time (v0 simplicity).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{write_atomic, ROUTE_DOT_DIR};

/// Profile file name under `.route/`.
pub const PROFILE_FILE: &str = "profile.json";

/// Active profile pointer file under `.route/`.
pub const ACTIVE_PROFILE_FILE: &str = "active_profile";

/// Canonical profile store path.
pub fn profile_path(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(PROFILE_FILE)
}

/// Active profile pointer path.
pub fn active_profile_path(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(ACTIVE_PROFILE_FILE)
}

/// A named project profile that bundles configuration.
///
/// v0 is deliberately flat: no inheritance, no composition. One profile
/// active at a time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProfile {
    /// Stable id, e.g. "default", "fast", "strict".
    pub id: String,
    /// Human-readable display name.
    #[serde(default)]
    pub name: String,
    /// Optional protocol override (path or inline content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    /// Workflow IDs to include when this profile is active.
    #[serde(default)]
    pub workflow_ids: Vec<String>,
    /// Reference IDs to include when this profile is active.
    #[serde(default)]
    pub reference_ids: Vec<String>,
    /// Optional agent policy override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_policy_override: Option<String>,
    /// Optional verification policy override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_policy_override: Option<String>,
    /// Optional description.
    #[serde(default)]
    pub description: String,
}

impl ProjectProfile {
    /// Create a new profile with the given id.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            protocol: None,
            workflow_ids: Vec::new(),
            reference_ids: Vec::new(),
            agent_policy_override: None,
            verification_policy_override: None,
            description: String::new(),
        }
    }
}

/// Store for all defined profiles.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileStore {
    pub profiles: Vec<ProjectProfile>,
}

impl ProfileStore {
    /// Load profiles from the profile store file.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = profile_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading profile store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        let s: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing profile store JSON at {}", p.display()))?;
        Ok(s)
    }

    /// Save the profile store to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = profile_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing profile store to {}", p.display()))?;
        Ok(())
    }

    /// Find a profile by id.
    pub fn get(&self, id: &str) -> Option<&ProjectProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Get a mutable reference to a profile by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ProjectProfile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    /// Index of a profile by id.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.profiles.iter().position(|p| p.id == id)
    }

    /// Upsert a profile. Returns true if inserted, false if updated.
    pub fn upsert(&mut self, profile: ProjectProfile) -> bool {
        if let Some(existing) = self.get_mut(&profile.id) {
            *existing = profile;
            false
        } else {
            self.profiles.push(profile);
            true
        }
    }

    /// Remove a profile by id. Returns true if removed.
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(i) = self.index_of(id) {
            self.profiles.remove(i);
            true
        } else {
            false
        }
    }

    /// Get the active profile id.
    pub fn active_id(project_root: &Path) -> Result<Option<String>> {
        let p = active_profile_path(project_root);
        if !p.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading active profile from {}", p.display()))?;
        let trimmed = raw.trim().to_string();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    }

    /// Set the active profile id.
    pub fn set_active(project_root: &Path, id: &str) -> Result<()> {
        let p = active_profile_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        std::fs::write(&p, id.as_bytes())
            .with_context(|| format!("writing active profile to {}", p.display()))?;
        Ok(())
    }

    /// Clear the active profile.
    pub fn clear_active(project_root: &Path) -> Result<()> {
        let p = active_profile_path(project_root);
        if p.exists() {
            std::fs::remove_file(&p)?;
        }
        Ok(())
    }

    /// Load the active profile, if any.
    pub fn load_active(project_root: &Path) -> Result<Option<ProjectProfile>> {
        let id = Self::active_id(project_root)?;
        match id {
            None => Ok(None),
            Some(id) => {
                let store = Self::load(project_root)?;
                Ok(store.get(&id).cloned())
            }
        }
    }
}

/// Built-in profiles that are always available.
pub fn builtin_profiles() -> Vec<ProjectProfile> {
    vec![
        ProjectProfile {
            id: "default".to_string(),
            name: "Default".to_string(),
            protocol: None,
            workflow_ids: vec![],
            reference_ids: vec![],
            agent_policy_override: None,
            verification_policy_override: None,
            description: "Standard Route profile — no overrides, all enabled references and workflows are included.".to_string(),
        },
        ProjectProfile {
            id: "fast".to_string(),
            name: "Fast".to_string(),
            protocol: None,
            workflow_ids: vec![],
            reference_ids: vec![],
            agent_policy_override: Some(
                r#"{"mode":"single","max_agents":1}"#.to_string()
            ),
            verification_policy_override: None,
            description: "Minimal profile — single-agent mode, no sub-agents.".to_string(),
        },
        ProjectProfile {
            id: "strict".to_string(),
            name: "Strict".to_string(),
            protocol: None,
            workflow_ids: vec![],
            reference_ids: vec![],
            agent_policy_override: Some(
                r#"{"mode":"single","max_agents":1,"do_not_spawn_when":["unsure"]}"#.to_string()
            ),
            verification_policy_override: None,
            description: "Strict profile — single-agent, conservative behavior.".to_string(),
        },
    ]
}

/// Initialize the profile store with built-in profiles.
pub fn init_profile_store(project_root: &Path) -> Result<()> {
    let p = profile_path(project_root);
    if p.exists() {
        return Ok(());
    }
    let mut store = ProfileStore::default();
    for profile in builtin_profiles() {
        store.upsert(profile);
    }
    store.save(project_root)?;
    // Set default as active if not already set
    if ProfileStore::active_id(project_root)?.is_none() {
        ProfileStore::set_active(project_root, "default")?;
    }
    Ok(())
}
