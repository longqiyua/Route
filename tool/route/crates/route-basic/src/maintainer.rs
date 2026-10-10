//! Maintainer — generate limited maintenance plans.
//!
//! Default: max 3 tasks per plan. No "scan everything and fix everything".
//! Tasks are executed sequentially — each task must complete before the
//! next is evaluated.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A maintenance plan — a limited set of tasks to improve project health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerPlan {
    /// Unique plan ID
    pub id: String,
    /// When this plan was created
    pub created_at: i64,
    /// Plan objectives
    #[serde(default)]
    pub objectives: Vec<String>,
    /// Findings this plan addresses
    #[serde(default)]
    pub findings_addressed: Vec<String>,
    /// Proposed tasks (max 3)
    #[serde(default)]
    pub proposed_tasks: Vec<MaintainerTask>,
    /// Order of execution
    #[serde(default)]
    pub order: Vec<usize>,
    /// Stop conditions — when to stop executing
    #[serde(default)]
    pub stop_conditions: Vec<String>,
}

/// A single maintenance task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintainerTask {
    /// Task index in the plan
    pub index: usize,
    /// Title
    pub title: String,
    /// Description
    pub description: String,
    /// Finding IDs this task addresses
    #[serde(default)]
    pub addresses_findings: Vec<String>,
    /// Effort estimate (xs, sm, md, lg, xl)
    pub effort: String,
    /// Whether this task has been started
    #[serde(default)]
    pub started: bool,
    /// Whether this task is complete
    #[serde(default)]
    pub completed: bool,
    /// Session ID (if started)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// The maintainer plan store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintainerStore {
    pub plans: Vec<MaintainerPlan>,
}

impl MaintainerStore {
    /// Load plans from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root
            .join(crate::constitutive::ROUTE_DOT_DIR)
            .join("plan")
            .join("maintainer.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save plans to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = project_root
            .join(crate::constitutive::ROUTE_DOT_DIR)
            .join("plan");
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(dir.join("maintainer.json"), json)?;
        Ok(())
    }

    /// Add a plan.
    pub fn add(&mut self, plan: MaintainerPlan) {
        self.plans.push(plan);
    }

    /// Get a plan by ID.
    pub fn get(&self, id: &str) -> Option<&MaintainerPlan> {
        self.plans.iter().find(|p| p.id == id)
    }

    /// Get a mutable plan by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut MaintainerPlan> {
        self.plans.iter_mut().find(|p| p.id == id)
    }

    /// List all plans (most recent first).
    pub fn list(&self) -> Vec<&MaintainerPlan> {
        let mut plans: Vec<&MaintainerPlan> = self.plans.iter().collect();
        plans.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        plans
    }
}

/// Generate a maintenance plan from guardian findings.
///
/// Default: max 3 tasks. Prioritizes critical findings first.
pub fn generate_maintainer_plan(
    _project_root: &Path,
    findings: &[crate::guardian::GuardianFinding],
    max_tasks: usize,
) -> Result<MaintainerPlan> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let max = max_tasks.min(3); // never more than 3
    let mut tasks = Vec::new();
    let mut addressed = Vec::new();
    let mut objectives = Vec::new();

    // Sort findings by severity
    let mut sorted: Vec<&crate::guardian::GuardianFinding> = findings.iter().collect();
    sorted.sort_by(|a, b| {
        let a_sev = match a.severity {
            crate::guardian::FindingSeverity::Critical => 3,
            crate::guardian::FindingSeverity::Warning => 2,
            crate::guardian::FindingSeverity::Info => 1,
        };
        let b_sev = match b.severity {
            crate::guardian::FindingSeverity::Critical => 3,
            crate::guardian::FindingSeverity::Warning => 2,
            crate::guardian::FindingSeverity::Info => 1,
        };
        b_sev.cmp(&a_sev)
    });

    for finding in sorted {
        if tasks.len() >= max {
            break;
        }
        if finding.status == crate::guardian::FindingStatus::Resolved
            || finding.status == crate::guardian::FindingStatus::Ignored
        {
            continue;
        }

        let task = MaintainerTask {
            index: tasks.len() + 1,
            title: finding.title.clone(),
            description: finding.explanation.clone(),
            addresses_findings: vec![finding.id.clone()],
            effort: "md".to_string(),
            started: false,
            completed: false,
            session_id: None,
        };

        addressed.push(finding.id.clone());
        objectives.push(format!("Address: {}", finding.title));
        tasks.push(task);
    }

    if tasks.is_empty() {
        objectives.push("No critical issues found — routine maintenance".to_string());
    }

    let plan = MaintainerPlan {
        id: format!("maintain-{}", now),
        created_at: now,
        objectives,
        findings_addressed: addressed,
        proposed_tasks: tasks.clone(),
        order: (1..=tasks.len()).collect(),
        stop_conditions: vec![
            "Stop if a task fails verification".to_string(),
            "Stop if user interrupts".to_string(),
            "Re-evaluate before starting each subsequent task".to_string(),
        ],
    };

    Ok(plan)
}

/// Format a maintainer plan for display.
pub fn format_maintainer_plan(plan: &MaintainerPlan) -> String {
    let mut out = format!("Maintainer Plan: {}\nObjectives:\n", plan.id);

    for obj in &plan.objectives {
        out.push_str(&format!("  ◉ {}\n", obj));
    }

    out.push_str(&format!("\nTasks ({}):\n", plan.proposed_tasks.len()));
    for task in &plan.proposed_tasks {
        let status = if task.completed {
            "✓ DONE"
        } else if task.started {
            "▶ IN PROGRESS"
        } else {
            "○ PENDING"
        };
        out.push_str(&format!(
            "  {}. [{}] {} ({})\n     {}\n",
            task.index, status, task.title, task.effort, task.description
        ));
    }

    if !plan.stop_conditions.is_empty() {
        out.push_str("\nStop Conditions:\n");
        for sc in &plan.stop_conditions {
            out.push_str(&format!("  ⊘ {}\n", sc));
        }
    }

    out
}
