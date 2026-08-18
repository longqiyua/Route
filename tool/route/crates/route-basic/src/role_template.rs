//! AI Role Library — accumulate role templates from successful agent plans.
//!
//! When agent plans are dynamically generated and used successfully, their
//! role configurations can be saved as templates for future reuse. The
//! AgentCompiler may reuse or adapt these templates — it is never required
//! to use a fixed set of agents.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Role template directory under `.route/`.
pub fn role_template_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("role_template")
}

/// Path to the role template store file.
pub fn role_template_path(project_root: &Path) -> PathBuf {
    role_template_dir(project_root).join("templates.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A reusable role template — accumulated from successful agent plans.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleTemplate {
    /// Unique template ID
    pub id: String,
    /// Human-readable name (e.g. "storage-migration-architect")
    pub name: String,
    /// What this role is good for
    pub purpose: String,
    /// Task patterns this role is good for
    #[serde(default)]
    pub good_for: Vec<String>,
    /// Tools this role typically uses
    #[serde(default)]
    pub tools: Vec<String>,
    /// Permissions this role typically needs
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Exit criteria — when this role's work is done
    #[serde(default)]
    pub exit_criteria: Vec<String>,
    /// Evidence — session IDs where this template was validated
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Success rate from sessions using this template
    pub success_rate: f64,
    /// How many times this template has been used
    pub usage_count: u32,
    /// When this template was created
    pub created_at: i64,
    /// When this was last updated
    pub updated_at: i64,
    /// Whether this template is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// The role template store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoleTemplateStore {
    pub templates: Vec<RoleTemplate>,
}

impl RoleTemplateStore {
    /// Load templates from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = role_template_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save templates to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = role_template_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(role_template_path(project_root), json)?;
        Ok(())
    }

    /// List all templates.
    pub fn list(&self) -> &[RoleTemplate] {
        &self.templates
    }

    /// Get a template by ID.
    pub fn get(&self, id: &str) -> Option<&RoleTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// Get a mutable template by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut RoleTemplate> {
        self.templates.iter_mut().find(|t| t.id == id)
    }

    /// Add a template.
    pub fn add(&mut self, template: RoleTemplate) {
        self.templates.push(template);
    }

    /// Remove a template by ID.
    pub fn remove(&mut self, id: &str) {
        self.templates.retain(|t| t.id != id);
    }

    /// Find templates matching a task pattern.
    pub fn find_for_pattern(&self, task_pattern: &str) -> Vec<&RoleTemplate> {
        let tp = task_pattern.to_lowercase();
        self.templates
            .iter()
            .filter(|t| t.enabled && t.good_for.iter().any(|g| tp.contains(&g.to_lowercase())))
            .collect()
    }

    /// Record a successful usage of a template.
    pub fn record_success(&mut self, id: &str, session_id: &str) -> Result<()> {
        let template = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", id))?;
        template.usage_count += 1;
        template.success_rate = (template.success_rate * (template.usage_count - 1) as f64 + 1.0)
            / template.usage_count as f64;
        if !template.evidence.contains(&session_id.to_string()) {
            template.evidence.push(session_id.to_string());
        }
        template.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Record a failed usage of a template.
    pub fn record_failure(&mut self, id: &str, _session_id: &str) -> Result<()> {
        let template = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found", id))?;
        template.usage_count += 1;
        template.success_rate =
            template.success_rate * (template.usage_count - 1) as f64 / template.usage_count as f64;
        template.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Create a template from an agent plan's role specs.
    pub fn create_from_role(
        name: &str,
        purpose: &str,
        tools: Vec<String>,
        permissions: Vec<String>,
        exit_criteria: Vec<String>,
        session_id: &str,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut store = Self::default();
        let template = RoleTemplate {
            id: format!("role-{}", slugify(name)),
            name: name.to_string(),
            purpose: purpose.to_string(),
            good_for: vec![],
            tools,
            permissions,
            exit_criteria,
            evidence: vec![session_id.to_string()],
            success_rate: 1.0,
            usage_count: 1,
            created_at: now,
            updated_at: now,
            enabled: true,
        };
        store.add(template);
        store
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

/// Format a role template for display.
pub fn format_role_template(template: &RoleTemplate) -> String {
    format!(
        r#"[{id}]
  Name:        {name}
  Purpose:     {purpose}
  Good For:    {good}
  Tools:       {tools}
  Permissions: {perms}
  Exit:        {exit}
  Success:     {rate:.0}% ({count} uses)
  Evidence:    {ev}
  Enabled:     {enabled}"#,
        id = template.id,
        name = template.name,
        purpose = template.purpose,
        good = template.good_for.join(", "),
        tools = template.tools.join(", "),
        perms = template.permissions.join(", "),
        exit = template.exit_criteria.join(", "),
        rate = template.success_rate * 100.0,
        count = template.usage_count,
        ev = template.evidence.join(", "),
        enabled = if template.enabled { "yes" } else { "no" },
    )
}
