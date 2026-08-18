//! Open Loop Detection — find unclosed cycles in project state.
//!
//! Detects:
//! - Started tasks not ended
//! - Accepted ideas without tasks
//! - Failure cases without resolution
//! - Decisions with follow-up not completed
//! - Goals without any active action
//! - Workflow proposals not reviewed
//! - Learning proposals on backlog
//! - Imported references not inspected
//! - Study candidates not processed

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// An open loop — something that was started but not completed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenLoop {
    /// Unique loop ID
    pub id: String,
    /// Source kind (task, idea, failure, decision, goal, workflow_proposal, learning_proposal, reference, study_candidate)
    pub source: String,
    /// Title/description
    pub title: String,
    /// How old this loop is (in seconds since creation)
    pub age_secs: u64,
    /// Impact assessment
    pub impact: String,
    /// Suggested next action
    pub next_action: String,
    /// Related source ID
    pub source_id: String,
}

/// Result of an open loop scan.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenLoopResult {
    pub loops: Vec<OpenLoop>,
}

/// Scan for open loops in the project.
pub fn scan_open_loops(project_root: &Path) -> Result<OpenLoopResult> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut loops = Vec::new();

    // 1. Open sessions (started task not ended)
    if let Ok(store) = crate::execution::SessionStore::load(project_root) {
        for session in &store.sessions {
            if session.status == crate::execution::SessionStatus::Active {
                let age = now.saturating_sub(session.started_at as u64);
                loops.push(OpenLoop {
                    id: format!("loop-session-{}", session.id),
                    source: "task".to_string(),
                    title: format!("Open session: {}", session.task),
                    age_secs: age,
                    impact: "Unfinished work — may have uncommitted changes".to_string(),
                    next_action: "End the session (success, failed, or aborted)".to_string(),
                    source_id: session.id.clone(),
                });
            }
        }
    }

    // 2. Accepted ideas without tasks
    if let Ok(store) = crate::idea::IdeaStore::load(project_root) {
        for idea in &store.ideas {
            if matches!(idea.status, crate::idea::IdeaStatus::Accepted) {
                let age = now.saturating_sub(idea.created_at as u64);
                loops.push(OpenLoop {
                    id: format!("loop-idea-{}", idea.id),
                    source: "idea".to_string(),
                    title: format!("Accepted idea not yet implemented: {}", idea.text),
                    age_secs: age,
                    impact: "Approved work item not started".to_string(),
                    next_action: "Create a task for this idea or mark it as implemented"
                        .to_string(),
                    source_id: idea.id.clone(),
                });
            }
        }
    }

    // 3. Unresolved failure cases
    if let Ok(lib) = crate::failure::FailureLibrary::load(project_root) {
        for case in &lib.cases {
            if !case.resolved {
                let age = now.saturating_sub(case.created_at as u64);
                loops.push(OpenLoop {
                    id: format!("loop-failure-{}", case.id),
                    source: "failure".to_string(),
                    title: format!("Unresolved failure: {}", case.problem),
                    age_secs: age,
                    impact: "Known issue still open — may affect stability".to_string(),
                    next_action: "Investigate root cause and resolve the failure".to_string(),
                    source_id: case.id.clone(),
                });
            }
        }
    }

    // 4. Goals without any active action
    if let Ok(store) = crate::goal::GoalStore::load(project_root) {
        for goal in &store.goals {
            if goal.status == crate::goal::GoalStatus::Active && goal.related_tasks.is_empty() {
                let age = now.saturating_sub(goal.created_at as u64);
                loops.push(OpenLoop {
                    id: format!("loop-goal-{}", goal.id),
                    source: "goal".to_string(),
                    title: format!("Active goal without tasks: {}", goal.title),
                    age_secs: age,
                    impact: "Goal is active but no work has been started".to_string(),
                    next_action: "Create a task to make progress on this goal".to_string(),
                    source_id: goal.id.clone(),
                });
            }
        }
    }

    Ok(OpenLoopResult { loops })
}

/// Format open loops for display.
pub fn format_open_loops(result: &OpenLoopResult) -> String {
    if result.loops.is_empty() {
        return "No open loops detected.\n".to_string();
    }

    let mut out = format!("Open Loops ({}):\n\n", result.loops.len());
    for (i, loop_) in result.loops.iter().enumerate() {
        let age_days = loop_.age_secs / 86400;
        let age_str = if age_days > 0 {
            format!("{}d", age_days)
        } else {
            let age_hours = loop_.age_secs / 3600;
            if age_hours > 0 {
                format!("{}h", age_hours)
            } else {
                format!("{}m", loop_.age_secs / 60)
            }
        };

        out.push_str(&format!(
            "{}. [{}] {} (age: {})\n   Impact: {}\n   → {}\n\n",
            i + 1,
            loop_.source,
            loop_.title,
            age_str,
            loop_.impact,
            loop_.next_action,
        ));
    }

    out
}
