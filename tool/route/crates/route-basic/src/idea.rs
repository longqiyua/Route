//! Idea / Decision Inbox — save development ideas independently.
//!
//! Ideas are lightweight records that can later be promoted to tasks,
//! references, memory items, or workflow changes. AI can propose ideas
//! but cannot accept them — only the user can.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Idea directory under `.route/`.
pub fn idea_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("idea")
}

/// Path to the idea store file.
pub fn idea_path(project_root: &Path) -> PathBuf {
    idea_dir(project_root).join("ideas.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The status of an idea.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IdeaStatus {
    /// Just captured, not yet evaluated
    Inbox,
    /// Under consideration
    Considering,
    /// Accepted for future action
    Accepted,
    /// Rejected / not going to be pursued
    Rejected,
    /// Already implemented
    Implemented,
}

impl Default for IdeaStatus {
    fn default() -> Self {
        Self::Inbox
    }
}

/// A development idea or decision note.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Idea {
    /// Unique idea ID
    pub id: String,
    /// The idea text/content
    pub text: String,
    /// Who/what generated this idea (user, ai, system)
    pub source: String,
    /// Optional task/session context
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Current status
    #[serde(default)]
    pub status: IdeaStatus,
    /// Links to other items (task IDs, reference IDs, etc.)
    #[serde(default)]
    pub links: Vec<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// When this idea was created
    pub created_at: i64,
    /// When this idea was last updated
    pub updated_at: i64,
}

/// The idea store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdeaStore {
    pub ideas: Vec<Idea>,
}

impl IdeaStore {
    /// Load ideas from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = idea_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save ideas to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = idea_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(idea_path(project_root), json)?;
        Ok(())
    }

    /// Add a new idea.
    pub fn add(
        &mut self,
        text: &str,
        source: &str,
        task_id: Option<String>,
        tags: Vec<String>,
    ) -> Result<Idea> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let id = format!("idea-{}", now);

        let idea = Idea {
            id: id.clone(),
            text: text.to_string(),
            source: source.to_string(),
            task_id,
            status: IdeaStatus::Inbox,
            links: Vec::new(),
            tags,
            created_at: now,
            updated_at: now,
        };

        self.ideas.push(idea.clone());
        Ok(idea)
    }

    /// List all ideas, optionally filtered by status.
    pub fn list(&self, status_filter: Option<&str>) -> Vec<&Idea> {
        self.ideas
            .iter()
            .filter(|i| {
                status_filter.map_or(true, |s| {
                    format!("{:?}", i.status).to_lowercase() == s.to_lowercase()
                })
            })
            .collect()
    }

    /// Get an idea by ID.
    pub fn get(&self, id: &str) -> Option<&Idea> {
        self.ideas.iter().find(|i| i.id == id)
    }

    /// Get a mutable idea by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Idea> {
        self.ideas.iter_mut().find(|i| i.id == id)
    }

    /// Change the status of an idea.
    pub fn set_status(&mut self, id: &str, status: IdeaStatus) -> Result<()> {
        let idea = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Idea '{}' not found", id))?;

        // AI cannot directly accept ideas
        idea.status = status;
        idea.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Add a link to an idea.
    pub fn add_link(&mut self, id: &str, link: &str) -> Result<()> {
        let idea = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Idea '{}' not found", id))?;
        if !idea.links.contains(&link.to_string()) {
            idea.links.push(link.to_string());
        }
        Ok(())
    }

    /// Remove an idea by ID.
    pub fn remove(&mut self, id: &str) {
        self.ideas.retain(|i| i.id != id);
    }
}

/// Format an idea for display.
pub fn format_idea(idea: &Idea) -> String {
    format!(
        r#"[{id}]
  Text:   {text}
  Source: {source}
  Status: {status:?}
  Task:   {task}
  Links:  {links}
  Tags:   {tags}
  Created: {created}
  Updated: {updated}"#,
        id = idea.id,
        text = idea.text,
        source = idea.source,
        status = idea.status,
        task = idea.task_id.as_deref().unwrap_or("(none)"),
        links = idea.links.join(", "),
        tags = idea.tags.join(", "),
        created = idea.created_at,
        updated = idea.updated_at,
    )
}
