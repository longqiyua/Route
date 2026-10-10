//! Project Goals — lightweight goal tracking for Route projects.
//!
//! Goals enter ProjectMemory but are NOT part of the Constitution.
//! Guardian must be aware of active goals to avoid maintenance AI
//! going off on its own priorities.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Goal directory under `.route/`.
pub fn goal_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("goal")
}

/// Path to the goal store file.
pub fn goal_path(project_root: &Path) -> PathBuf {
    goal_dir(project_root).join("goals.json")
}

/// The status of a goal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    /// Currently being worked on
    Active,
    /// Temporarily paused
    Paused,
    /// Completed successfully
    Done,
    /// Abandoned / no longer relevant
    Abandoned,
}

impl Default for GoalStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// A project goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique goal ID
    pub id: String,
    /// Short title
    pub title: String,
    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Current status
    #[serde(default)]
    pub status: GoalStatus,
    /// Priority (1-5, 5=highest)
    #[serde(default)]
    pub priority: u8,
    /// Parent goal ID (for sub-goals)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Success criteria
    #[serde(default)]
    pub success_criteria: Vec<String>,
    /// Related idea IDs
    #[serde(default)]
    pub related_ideas: Vec<String>,
    /// Related task session IDs
    #[serde(default)]
    pub related_tasks: Vec<String>,
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// When this goal was created
    pub created_at: i64,
    /// When this goal was last updated
    pub updated_at: i64,
}

/// The goal store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalStore {
    pub goals: Vec<Goal>,
}

impl GoalStore {
    /// Load goals from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = goal_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save goals to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = goal_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(goal_path(project_root), json)?;
        Ok(())
    }

    /// Add a goal.
    pub fn add(&mut self, goal: Goal) {
        self.goals.push(goal);
    }

    /// List all goals, optionally filtered by status.
    pub fn list(&self, status_filter: Option<&str>) -> Vec<&Goal> {
        self.goals
            .iter()
            .filter(|g| {
                status_filter.map_or(true, |s| {
                    format!("{:?}", g.status).to_lowercase() == s.to_lowercase()
                })
            })
            .collect()
    }

    /// Get a goal by ID.
    pub fn get(&self, id: &str) -> Option<&Goal> {
        self.goals.iter().find(|g| g.id == id)
    }

    /// Get a mutable goal by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Goal> {
        self.goals.iter_mut().find(|g| g.id == id)
    }

    /// Update a goal's status.
    pub fn set_status(&mut self, id: &str, status: GoalStatus) -> Result<()> {
        let goal = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Goal '{}' not found", id))?;
        goal.status = status;
        goal.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Update a goal's priority.
    pub fn set_priority(&mut self, id: &str, priority: u8) -> Result<()> {
        let goal = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Goal '{}' not found", id))?;
        goal.priority = priority;
        goal.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Remove a goal by ID.
    pub fn remove(&mut self, id: &str) {
        self.goals.retain(|g| g.id != id);
    }

    /// Get active goals, sorted by priority descending.
    pub fn active_goals(&self) -> Vec<&Goal> {
        let mut goals: Vec<&Goal> = self
            .goals
            .iter()
            .filter(|g| g.status == GoalStatus::Active)
            .collect();
        goals.sort_by(|a, b| b.priority.cmp(&a.priority));
        goals
    }

    /// Create a new goal.
    pub fn create_goal(
        &mut self,
        title: &str,
        description: Option<String>,
        priority: u8,
        parent: Option<String>,
        success_criteria: Vec<String>,
        tags: Vec<String>,
    ) -> Result<Goal> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let id = format!("goal-{}", slugify(title));

        let goal = Goal {
            id: id.clone(),
            title: title.to_string(),
            description,
            status: GoalStatus::Active,
            priority,
            parent,
            success_criteria,
            related_ideas: vec![],
            related_tasks: vec![],
            tags,
            created_at: now,
            updated_at: now,
        };

        self.add(goal.clone());
        Ok(goal)
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

/// Format a goal for display.
pub fn format_goal(goal: &Goal) -> String {
    format!(
        r#"[{id}]
  Title:    {title}
  Status:   {status:?}
  Priority: {prio}/5
  Parent:   {parent}
  Criteria: {criteria}
  Ideas:    {ideas}
  Tasks:    {tasks}
  Tags:     {tags}
  Created:  {created}
  Updated:  {updated}"#,
        id = goal.id,
        title = goal.title,
        status = goal.status,
        prio = goal.priority,
        parent = goal.parent.as_deref().unwrap_or("(none)"),
        criteria = goal.success_criteria.join(", "),
        ideas = goal.related_ideas.join(", "),
        tasks = goal.related_tasks.join(", "),
        tags = goal.tags.join(", "),
        created = goal.created_at,
        updated = goal.updated_at,
    )
}
