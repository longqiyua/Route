//! Next Action Engine — propose the most valuable next actions.
//!
//! Not a general todo sorter. Prioritization is based on Route project state:
//! blocking > integrity/data risk > repeated failure > active goal dependency > accepted idea > cleanup.
//! Must explain why each action is ranked where it is.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// An action proposal — a suggested next step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionProposal {
    /// Unique proposal ID
    pub id: String,
    /// Title of the action
    pub title: String,
    /// Why now — urgency/rationale
    pub why_now: String,
    /// Expected value of doing this
    pub expected_value: String,
    /// Urgency level (1-5, 5=most urgent)
    pub urgency: u8,
    /// Effort hint (xs, sm, md, lg, xl)
    pub effort_hint: String,
    /// Dependencies — what must be true before this action
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Risks of doing this
    #[serde(default)]
    pub risks: Vec<String>,
    /// Relevant memory item IDs
    #[serde(default)]
    pub relevant_memory: Vec<String>,
    /// Relevant failure case IDs
    #[serde(default)]
    pub relevant_failures: Vec<String>,
    /// Relevant reference IDs
    #[serde(default)]
    pub references: Vec<String>,
    /// Suggested strategy ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_strategy: Option<String>,
    /// Suggested workflow ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_workflow: Option<String>,
    /// Explanation of why this proposal is ranked here
    pub ranking_reason: String,
    /// When this proposal was created
    pub created_at: i64,
}

/// The next action store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NextActionStore {
    pub proposals: Vec<ActionProposal>,
}

impl NextActionStore {
    /// Load proposals from disk.
    pub fn load() -> Result<Self> {
        // In-memory only for now — proposals are ephemeral
        Ok(Self::default())
    }

    /// Save proposals (stub — in-memory only).
    pub fn save(&self) -> Result<()> {
        Ok(())
    }

    /// Add a proposal.
    pub fn add(&mut self, proposal: ActionProposal) {
        self.proposals.push(proposal);
    }

    /// Get a proposal by ID.
    pub fn get(&self, id: &str) -> Option<&ActionProposal> {
        self.proposals.iter().find(|p| p.id == id)
    }

    /// Remove a proposal by ID.
    pub fn remove(&mut self, id: &str) {
        self.proposals.retain(|p| p.id != id);
    }

    /// List all proposals, sorted by urgency descending.
    pub fn list(&self) -> Vec<&ActionProposal> {
        let mut proposals: Vec<&ActionProposal> = self.proposals.iter().collect();
        proposals.sort_by(|a, b| b.urgency.cmp(&a.urgency));
        proposals
    }
}

/// Generate the next action proposals based on project state.
///
/// Priority order:
/// 1. Blocking issues
/// 2. Integrity/data risks
/// 3. Repeated failures
/// 4. Active goal dependencies
/// 5. Accepted ideas
/// 6. Cleanup
pub fn generate_next_actions(
    _project_root: &Path,
    findings: &[crate::guardian::GuardianFinding],
    goals: &[crate::goal::Goal],
    limit: usize,
) -> Result<Vec<ActionProposal>> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut proposals = Vec::new();

    // 1. Check for blocking findings (critical severity)
    for finding in findings {
        if finding.severity != crate::guardian::FindingSeverity::Critical {
            continue;
        }
        if finding.status == crate::guardian::FindingStatus::Resolved
            || finding.status == crate::guardian::FindingStatus::Ignored
        {
            continue;
        }

        let urgency = 5u8;
        proposals.push(ActionProposal {
            id: format!("action-{}-{}", finding.id, now),
            title: format!("Resolve critical finding: {}", finding.title),
            why_now: format!("Blocking issue detected: {}", finding.explanation),
            expected_value: "Removes a critical blocker from the project".to_string(),
            urgency,
            effort_hint: "md".to_string(),
            dependencies: vec![],
            risks: vec![],
            relevant_memory: vec![],
            relevant_failures: finding.evidence_ids.clone(),
            references: vec![],
            suggested_strategy: None,
            suggested_workflow: None,
            ranking_reason: format!(
                "Critical severity finding — highest priority (urgency={})",
                urgency
            ),
            created_at: now,
        });
    }

    // 2. Check for active goals without progress
    for goal in goals {
        if goal.status != crate::goal::GoalStatus::Active {
            continue;
        }

        let has_recent_task = false; // simplified
        if !has_recent_task {
            proposals.push(ActionProposal {
                id: format!("action-goal-{}-{}", goal.id, now),
                title: format!("Make progress on goal: {}", goal.title),
                why_now: format!("Active goal with no recent task: {}", goal.title),
                expected_value: "Advances project toward a stated goal".to_string(),
                urgency: 4,
                effort_hint: "md".to_string(),
                dependencies: vec![],
                risks: vec![],
                relevant_memory: vec![],
                relevant_failures: vec![],
                references: vec![],
                suggested_strategy: None,
                suggested_workflow: None,
                ranking_reason: format!(
                    "Active goal '{}' has no recent task progress (urgency=4)",
                    goal.title
                ),
                created_at: now,
            });
        }
    }

    // 3. Check for warning-level findings
    for finding in findings {
        if finding.severity != crate::guardian::FindingSeverity::Warning {
            continue;
        }
        if finding.status == crate::guardian::FindingStatus::Resolved
            || finding.status == crate::guardian::FindingStatus::Ignored
        {
            continue;
        }

        let urgency = 3u8;
        proposals.push(ActionProposal {
            id: format!("action-warn-{}-{}", finding.id, now),
            title: format!("Address warning: {}", finding.title),
            why_now: format!("Warning finding: {}", finding.explanation),
            expected_value: "Reduces project risk".to_string(),
            urgency,
            effort_hint: "sm".to_string(),
            dependencies: vec![],
            risks: vec![],
            relevant_memory: vec![],
            relevant_failures: finding.evidence_ids.clone(),
            references: vec![],
            suggested_strategy: None,
            suggested_workflow: None,
            ranking_reason: format!(
                "Warning-level finding — should be addressed (urgency={})",
                urgency
            ),
            created_at: now,
        });
    }

    // Limit results
    proposals.sort_by(|a, b| b.urgency.cmp(&a.urgency));
    proposals.truncate(limit);

    Ok(proposals)
}

/// Format an action proposal for display.
pub fn format_action_proposal(proposal: &ActionProposal) -> String {
    format!(
        r#"[{id}]
  Title:    {title}
  Why Now:  {why}
  Value:    {value}
  Urgency:  {urgency}/5
  Effort:   {effort}
  Strategy: {strat}
  Workflow: {wf}
  Reason:   {reason}"#,
        id = proposal.id,
        title = proposal.title,
        why = proposal.why_now,
        value = proposal.expected_value,
        urgency = proposal.urgency,
        effort = proposal.effort_hint,
        strat = proposal
            .suggested_strategy
            .as_deref()
            .unwrap_or("(default)"),
        wf = proposal
            .suggested_workflow
            .as_deref()
            .unwrap_or("(default)"),
        reason = proposal.ranking_reason,
    )
}
