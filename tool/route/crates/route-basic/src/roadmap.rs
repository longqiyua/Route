//! Roadmap Graph — lightweight view of project trajectory.
//!
//! Composed from Goals, Ideas, Tasks, Failures, and Decisions.
//! Not a complex project manager — just answers: "where is the
//! project going and why?"

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A roadmap node — a goal, milestone, task, failure, or decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapNode {
    /// Unique node ID
    pub id: String,
    /// Kind: goal | milestone | action | task | failure | decision
    pub kind: String,
    /// Title
    pub title: String,
    /// Status text
    pub status: String,
    /// Priority (if applicable)
    #[serde(default)]
    pub priority: u8,
    /// What blocks this node
    #[serde(default)]
    pub blocked_by: Vec<String>,
    /// Related node IDs
    #[serde(default)]
    pub related: Vec<String>,
    /// Evidence (session IDs, commit IDs)
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// The roadmap — a collection of nodes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Roadmap {
    pub nodes: Vec<RoadmapNode>,
}

impl Roadmap {
    /// Build a roadmap from available project data.
    pub fn build(project_root: &Path) -> Result<Self> {
        let mut nodes = Vec::new();

        // Add goals
        if let Ok(store) = crate::goal::GoalStore::load(project_root) {
            for goal in &store.goals {
                nodes.push(RoadmapNode {
                    id: goal.id.clone(),
                    kind: "goal".to_string(),
                    title: goal.title.clone(),
                    status: format!("{:?}", goal.status),
                    priority: goal.priority,
                    blocked_by: vec![],
                    related: goal.related_tasks.clone(),
                    evidence: vec![],
                });
            }
        }

        // Add failure cases
        if let Ok(lib) = crate::failure::FailureLibrary::load(project_root) {
            for case in &lib.cases {
                let status = if case.resolved { "resolved" } else { "open" }.to_string();
                nodes.push(RoadmapNode {
                    id: case.id.clone(),
                    kind: "failure".to_string(),
                    title: case.problem.clone(),
                    status,
                    priority: 0,
                    blocked_by: vec![],
                    related: case.evidence.clone(),
                    evidence: case.evidence.clone(),
                });
            }
        }

        // Add accepted ideas
        if let Ok(store) = crate::idea::IdeaStore::load(project_root) {
            for idea in &store.ideas {
                if matches!(idea.status, crate::idea::IdeaStatus::Accepted) {
                    nodes.push(RoadmapNode {
                        id: idea.id.clone(),
                        kind: "action".to_string(),
                        title: idea.text.clone(),
                        status: "accepted".to_string(),
                        priority: 0,
                        blocked_by: vec![],
                        related: idea.links.clone(),
                        evidence: vec![],
                    });
                }
            }
        }

        Ok(Roadmap { nodes })
    }

    /// Render the roadmap as text.
    pub fn render(&self) -> String {
        let mut out = String::from("Roadmap\n=======\n\n");

        // Group by kind
        let goals: Vec<&RoadmapNode> = self.nodes.iter().filter(|n| n.kind == "goal").collect();
        let actions: Vec<&RoadmapNode> = self.nodes.iter().filter(|n| n.kind == "action").collect();
        let failures: Vec<&RoadmapNode> =
            self.nodes.iter().filter(|n| n.kind == "failure").collect();

        if !goals.is_empty() {
            out.push_str("Goals:\n");
            for g in &goals {
                out.push_str(&format!(
                    "  ◉ [{}] {} (priority: {}/5, status: {})\n",
                    g.id, g.title, g.priority, g.status
                ));
                for blocked in &g.blocked_by {
                    out.push_str(&format!("     ⊘ blocked by: {}\n", blocked));
                }
            }
            out.push('\n');
        }

        if !actions.is_empty() {
            out.push_str("Pending Actions:\n");
            for a in &actions {
                out.push_str(&format!(
                    "  ▶ [{}] {} (status: {})\n",
                    a.id, a.title, a.status
                ));
            }
            out.push('\n');
        }

        if !failures.is_empty() {
            out.push_str("Failures:\n");
            for f in &failures {
                out.push_str(&format!(
                    "  ✗ [{}] {} (status: {})\n",
                    f.id, f.title, f.status
                ));
            }
            out.push('\n');
        }

        if self.nodes.is_empty() {
            out.push_str("(no roadmap data available)\n");
        }

        out
    }
}
