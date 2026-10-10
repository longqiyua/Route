//! Trajectory Learning v1 — complete development trajectory recording and learning.
//!
//! Route learns from complete development trajectories rather than isolated events:
//!
//!   Savepoint A → Intent/Task → Strategy/Workflow/AgentPlan → Changes
//!   → Verification/Failure/Rollback → Savepoint B → User verdict → Learned Experience
//!
//! Data is stored in `.route/trajectory/`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR as DOT_DIR;

// ---------------------------------------------------------------------------
// Directory layout
// ---------------------------------------------------------------------------

fn trajectory_dir(project_root: &Path) -> PathBuf {
    project_root.join(DOT_DIR).join("trajectory")
}

fn trajectories_path(project_root: &Path) -> PathBuf {
    trajectory_dir(project_root).join("trajectories.json")
}

fn proposals_path(project_root: &Path) -> PathBuf {
    trajectory_dir(project_root).join("proposals.json")
}

// ---------------------------------------------------------------------------
// P0: DevelopmentTrajectory
// ---------------------------------------------------------------------------

/// A complete development trajectory — a causal chain from start to end.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentTrajectory {
    /// Stable trajectory ID (ULID).
    pub id: String,
    /// Savepoint ID at the start of the trajectory.
    pub from_savepoint: String,
    /// Savepoint ID at the end (if completed).
    pub to_savepoint: Option<String>,
    /// Task ID this trajectory belongs to.
    pub task_id: Option<String>,
    /// Intent description (human-readable task description).
    pub intent: Option<String>,
    /// Strategy ID used during this trajectory.
    pub strategy_id: Option<String>,
    /// Workflow revisions used.
    pub workflow_revisions: Vec<String>,
    /// Agent plan snapshot (JSON).
    pub agent_plan: Option<String>,
    /// Evidence IDs from the execution ledger.
    pub evidence_ids: Vec<String>,
    /// Decisions made during the trajectory.
    pub decisions: Vec<DecisionRecord>,
    /// Failures encountered.
    pub failures: Vec<FailureRecord>,
    /// Rollback information (if rollback occurred).
    pub rollback: Option<RollbackRecord>,
    /// Outcome: "success" | "failed" | "rolled_back" | "abandoned" | "in_progress".
    pub outcome: String,
    /// User verdict after reviewing the result.
    pub user_verdict: Option<UserVerdict>,
    /// Unix-millis when the trajectory was created.
    pub created_at: i64,
    /// Unix-millis when the trajectory was completed (if at all).
    pub completed_at: Option<i64>,
    /// Tags for categorization.
    pub tags: Vec<String>,
}

/// A decision made during a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: String,
    pub description: String,
    pub alternatives: Vec<String>,
    pub rationale: String,
    pub module: Option<String>,
    pub created_at: i64,
}

/// A failure encountered during a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub id: String,
    pub description: String,
    pub kind: String, // "test_fail" | "compile_error" | "rollback" | "user_reject" | "other"
    pub module: Option<String>,
    pub resolution: Option<String>,
    pub created_at: i64,
}

/// Rollback information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub rolled_back_to: String,
    pub reason: Option<String>,
    pub changes_lost: Vec<String>,
    pub verification_before_rollback: Option<String>,
    pub created_at: i64,
}

/// User verdict on a trajectory outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserVerdict {
    pub verdict: String, // "accepted" | "rejected" | "partial" | "undecided"
    pub comment: Option<String>,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// TrajectoryStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrajectoryStore {
    pub trajectories: Vec<DevelopmentTrajectory>,
}

impl TrajectoryStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = trajectories_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading trajectories from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = trajectories_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&p, json)?;
        Ok(())
    }

    pub fn add(&mut self, t: DevelopmentTrajectory) {
        self.trajectories.push(t);
    }

    pub fn get(&self, id: &str) -> Option<&DevelopmentTrajectory> {
        self.trajectories
            .iter()
            .find(|t| t.id == id || t.id.starts_with(id))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut DevelopmentTrajectory> {
        self.trajectories
            .iter_mut()
            .find(|t| t.id == id || t.id.starts_with(id))
    }

    pub fn list(&self) -> Vec<&DevelopmentTrajectory> {
        let mut list: Vec<_> = self.trajectories.iter().collect();
        list.sort_by_key(|t| std::cmp::Reverse(t.created_at));
        list
    }

    /// Find trajectories by task pattern.
    pub fn by_task_pattern(&self, pattern: &str) -> Vec<&DevelopmentTrajectory> {
        let lower = pattern.to_lowercase();
        self.trajectories
            .iter()
            .filter(|t| {
                t.intent
                    .as_ref()
                    .map(|i| i.to_lowercase().contains(&lower))
                    .unwrap_or(false)
                    || t.task_id
                        .as_ref()
                        .map(|id| id.to_lowercase().contains(&lower))
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Find trajectories by strategy ID.
    pub fn by_strategy(&self, strategy_id: &str) -> Vec<&DevelopmentTrajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.strategy_id.as_deref() == Some(strategy_id))
            .collect()
    }

    /// Find trajectories by outcome.
    pub fn by_outcome(&self, outcome: &str) -> Vec<&DevelopmentTrajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.outcome == outcome)
            .collect()
    }

    /// Find trajectories that have a rollback.
    pub fn with_rollback(&self) -> Vec<&DevelopmentTrajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.rollback.is_some())
            .collect()
    }

    /// Find trajectories with a user verdict.
    pub fn with_verdict(&self, verdict: &str) -> Vec<&DevelopmentTrajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.user_verdict.as_ref().map(|v| v.verdict.as_str()) == Some(verdict))
            .collect()
    }

    /// Filter by workflow revision.
    pub fn by_workflow(&self, wf_id: &str) -> Vec<&DevelopmentTrajectory> {
        self.trajectories
            .iter()
            .filter(|t| t.workflow_revisions.iter().any(|w| w.contains(wf_id)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// P1: Auto Save Policy
// ---------------------------------------------------------------------------

/// The state hash at the last auto-savepoint, used to prevent duplicates.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoSaveState {
    /// The last auto-savepoint's state hash, per trigger type.
    pub last_hashes: HashMap<String, String>,
    /// The last auto-savepoint's trajectory ID, per trigger type.
    pub last_trajectories: HashMap<String, String>,
}

impl AutoSaveState {
    fn path(project_root: &Path) -> PathBuf {
        trajectory_dir(project_root).join("auto-save.json")
    }

    pub fn load(project_root: &Path) -> Result<Self> {
        let p = Self::path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p).unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = Self::path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&p, json)?;
        Ok(())
    }

    /// Check if the state hash has changed since the last auto-save for this trigger.
    /// Returns `true` if the hash is new/different, meaning a new savepoint is needed.
    pub fn has_changed(&self, trigger: &str, state_hash: &str) -> bool {
        self.last_hashes
            .get(trigger)
            .map(|h| h != state_hash)
            .unwrap_or(true)
    }

    /// Record a new auto-save for a trigger.
    pub fn record(&mut self, trigger: &str, state_hash: String, trajectory_id: String) {
        self.last_hashes.insert(trigger.to_string(), state_hash);
        self.last_trajectories
            .insert(trigger.to_string(), trajectory_id);
    }
}

/// Auto-save triggers.
pub const AUTO_SAVE_TASK_START: &str = "task_start";
pub const AUTO_SAVE_HIGH_RISK: &str = "high_risk";
pub const AUTO_SAVE_STRATEGY_SWITCH: &str = "strategy_switch";
pub const AUTO_SAVE_ROLLBACK: &str = "rollback";
pub const AUTO_SAVE_VERIFIED: &str = "verified";
pub const AUTO_SAVE_MANUAL: &str = "manual";

// ---------------------------------------------------------------------------
// P2: Trajectory Diff
// ---------------------------------------------------------------------------

/// A diff between two trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryDiff {
    pub a_id: String,
    pub b_id: String,
    pub a_intent: Option<String>,
    pub b_intent: Option<String>,
    pub a_strategy: Option<String>,
    pub b_strategy: Option<String>,
    pub a_workflows: Vec<String>,
    pub b_workflows: Vec<String>,
    pub a_agent_plan: Option<String>,
    pub b_agent_plan: Option<String>,
    pub a_outcome: String,
    pub b_outcome: String,
    pub a_decision_count: usize,
    pub b_decision_count: usize,
    pub a_failure_count: usize,
    pub b_failure_count: usize,
    pub a_has_rollback: bool,
    pub b_has_rollback: bool,
    pub a_verdict: Option<String>,
    pub b_verdict: Option<String>,
    pub code_changed: bool,
    pub context_changed: bool,
    pub strategy_changed: bool,
    pub workflow_changed: bool,
    pub resources_used: Vec<String>,
}

/// Compute diff between two trajectories.
pub fn diff_trajectories(a: &DevelopmentTrajectory, b: &DevelopmentTrajectory) -> TrajectoryDiff {
    TrajectoryDiff {
        a_id: a.id.clone(),
        b_id: b.id.clone(),
        a_intent: a.intent.clone(),
        b_intent: b.intent.clone(),
        a_strategy: a.strategy_id.clone(),
        b_strategy: b.strategy_id.clone(),
        a_workflows: a.workflow_revisions.clone(),
        b_workflows: b.workflow_revisions.clone(),
        a_agent_plan: a.agent_plan.clone(),
        b_agent_plan: b.agent_plan.clone(),
        a_outcome: a.outcome.clone(),
        b_outcome: b.outcome.clone(),
        a_decision_count: a.decisions.len(),
        b_decision_count: b.decisions.len(),
        a_failure_count: a.failures.len(),
        b_failure_count: b.failures.len(),
        a_has_rollback: a.rollback.is_some(),
        b_has_rollback: b.rollback.is_some(),
        a_verdict: a.user_verdict.as_ref().map(|v| v.verdict.clone()),
        b_verdict: b.user_verdict.as_ref().map(|v| v.verdict.clone()),
        code_changed: a.from_savepoint != b.from_savepoint,
        context_changed: a.evidence_ids != b.evidence_ids,
        strategy_changed: a.strategy_id != b.strategy_id,
        workflow_changed: a.workflow_revisions != b.workflow_revisions,
        resources_used: Vec::new(),
    }
}

/// Format a trajectory for display (P2 show).
pub fn format_trajectory(t: &DevelopmentTrajectory, verbose: bool) -> String {
    let dt = chrono::DateTime::from_timestamp_millis(t.created_at)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| t.created_at.to_string());

    let mut out = format!(
        "Trajectory [{}]\n\
         Intent:    {}\n\
         Started:   {}\n\
         Outcome:   {}\n\
         From:      {}\n",
        t.id,
        t.intent.as_deref().unwrap_or("(none)"),
        dt,
        t.outcome,
        t.from_savepoint,
    );

    if let Some(to) = &t.to_savepoint {
        out.push_str(&format!("  To:        {}\n", to));
    }
    if let Some(sid) = &t.strategy_id {
        out.push_str(&format!("  Strategy:  {}\n", sid));
    }
    if !t.workflow_revisions.is_empty() {
        out.push_str(&format!(
            "  Workflows: {}\n",
            t.workflow_revisions.join(", ")
        ));
    }
    if let Some(ap) = &t.agent_plan {
        if verbose {
            out.push_str(&format!("  AgentPlan: {}\n", &ap[..ap.len().min(200)]));
        } else {
            out.push_str(&format!("  AgentPlan: {} chars\n", ap.len()));
        }
    }
    if let Some(rb) = &t.rollback {
        out.push_str(&format!(
            "  Rollback:  → {} (reason: {})\n",
            rb.rolled_back_to,
            rb.reason.as_deref().unwrap_or("(none)")
        ));
    }
    if let Some(uv) = &t.user_verdict {
        out.push_str(&format!("  Verdict:   {}\n", uv.verdict));
        if let Some(c) = &uv.comment {
            out.push_str(&format!("  Comment:   {}\n", c));
        }
    }

    if verbose {
        if !t.decisions.is_empty() {
            out.push_str(&format!("\nDecisions ({}):\n", t.decisions.len()));
            for d in &t.decisions {
                out.push_str(&format!("  - {}: {}\n", d.id, d.description));
            }
        }
        if !t.failures.is_empty() {
            out.push_str(&format!("\nFailures ({}):\n", t.failures.len()));
            for f in &t.failures {
                out.push_str(&format!("  - {} [{}]: {}", f.id, f.kind, f.description));
                if let Some(r) = &f.resolution {
                    out.push_str(&format!(" → {}", r));
                }
                out.push_str("\n");
            }
        }
        if !t.evidence_ids.is_empty() {
            out.push_str(&format!("\nEvidence: {} items\n", t.evidence_ids.len()));
        }
        if !t.tags.is_empty() {
            out.push_str(&format!("Tags: {}\n", t.tags.join(", ")));
        }
    }

    out
}

/// Format a trajectory diff for display.
pub fn format_trajectory_diff(d: &TrajectoryDiff) -> String {
    let mut out = format!("Trajectory Diff: {} → {}\n\n", d.a_id, d.b_id);

    let a_label = d.a_intent.as_deref().unwrap_or("(none)");
    let b_label = d.b_intent.as_deref().unwrap_or("(none)");
    out.push_str(&format!("  Intent:    {} → {}\n", a_label, b_label));
    out.push_str(&format!("  Outcome:   {} → {}\n", d.a_outcome, d.b_outcome));
    out.push_str(&format!(
        "  Strategy:  {} → {}\n",
        d.a_strategy.as_deref().unwrap_or("(none)"),
        d.b_strategy.as_deref().unwrap_or("(none)")
    ));
    out.push_str(&format!(
        "  Rollback:  {} → {}\n",
        if d.a_has_rollback { "yes" } else { "no" },
        if d.b_has_rollback { "yes" } else { "no" }
    ));
    out.push_str(&format!(
        "  Verdict:   {} → {}\n",
        d.a_verdict.as_deref().unwrap_or("(none)"),
        d.b_verdict.as_deref().unwrap_or("(none)")
    ));
    out.push_str(&format!(
        "  Decisions: {} → {}\n",
        d.a_decision_count, d.b_decision_count
    ));
    out.push_str(&format!(
        "  Failures:  {} → {}\n",
        d.a_failure_count, d.b_failure_count
    ));

    out.push_str(&format!("\nChanges: "));
    let mut changes = Vec::new();
    if d.code_changed {
        changes.push("code");
    }
    if d.context_changed {
        changes.push("context");
    }
    if d.strategy_changed {
        changes.push("strategy");
    }
    if d.workflow_changed {
        changes.push("workflow");
    }
    if changes.is_empty() {
        out.push_str("none\n");
    } else {
        out.push_str(&format!("{}\n", changes.join(", ")));
    }

    out
}

// ---------------------------------------------------------------------------
// P3: Learning from Reversal
// ---------------------------------------------------------------------------

/// Reversal analysis — what a rollback means.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReversalAnalysis {
    pub trajectory_id: String,
    pub rolled_back_to: String,
    pub what_changed: Vec<String>,
    pub which_decision: Option<String>,
    pub which_workflow: Option<String>,
    pub which_modules: Vec<String>,
    pub verification_before_rollback: Option<String>,
    pub explicit_reason: Option<String>,
    /// Observation: what we can safely say happened.
    pub observation: String,
    /// Whether this forms a Failure/Observation candidate (not a rule).
    pub is_candidate: bool,
}

/// Analyze a trajectory with a rollback to produce a reversal analysis.
/// P3: "B被撤销" — not "用户讨厌X".
pub fn analyze_reversal(t: &DevelopmentTrajectory) -> Option<ReversalAnalysis> {
    let rb = t.rollback.as_ref()?;

    let what_changed: Vec<String> = t.decisions.iter().map(|d| d.description.clone()).collect();
    let which_decision = t.decisions.last().map(|d| d.id.clone());
    let which_workflow = t.workflow_revisions.last().cloned();
    let which_modules: Vec<String> = t
        .decisions
        .iter()
        .filter_map(|d| d.module.clone())
        .collect();
    let explicit_reason = rb.reason.clone();

    let observation = if let Some(ref reason) = explicit_reason {
        format!(
            "Rollback with explicit reason: {} — {} changes were reverted",
            reason,
            what_changed.len()
        )
    } else {
        format!(
            "Rollback without explicit reason — {} changes were reverted (observation only)",
            what_changed.len()
        )
    };

    Some(ReversalAnalysis {
        trajectory_id: t.id.clone(),
        rolled_back_to: rb.rolled_back_to.clone(),
        what_changed,
        which_decision,
        which_workflow,
        which_modules,
        verification_before_rollback: rb.verification_before_rollback.clone(),
        explicit_reason,
        observation,
        is_candidate: true, // always a candidate, never a rule
    })
}

// ---------------------------------------------------------------------------
// P4: Success Retention
// ---------------------------------------------------------------------------

/// Retention analysis — weak evidence from continued use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionSignal {
    pub trajectory_id: String,
    pub continued_use: bool,
    pub subsequent_tasks: usize,
    pub verifications_passed: usize,
    pub has_user_accept: bool,
    /// Confidence: weak unless multiple strong signals.
    pub confidence: f64,
}

/// Analyze a trajectory for retention signals.
/// P4: "没有回滚 ≠ 用户喜欢" — retention is always weak evidence.
pub fn analyze_retention(t: &DevelopmentTrajectory, store: &TrajectoryStore) -> RetentionSignal {
    // Count how many trajectories reference this one's to_savepoint as from_savepoint
    let subsequent_tasks = store
        .trajectories
        .iter()
        .filter(|other| {
            other.id != t.id
                && t.to_savepoint
                    .as_ref()
                    .map(|sp| other.from_savepoint == *sp)
                    .unwrap_or(false)
        })
        .count();

    // Count verification passes (from evidence_ids — we check if any are test passes)
    let verifications_passed = t
        .evidence_ids
        .iter()
        // Note: full evidence check requires EvidenceStore load
        // For now, count all evidence IDs as passed (conservative estimate)
        .count();

    let has_user_accept = t
        .user_verdict
        .as_ref()
        .map(|v| v.verdict == "accepted")
        .unwrap_or(false);

    // Confidence: weak unless user explicitly accepted AND subsequent tasks built on it
    let confidence = if has_user_accept && subsequent_tasks > 0 {
        0.5 // moderate but still weak
    } else if has_user_accept {
        0.3
    } else {
        0.1
    };

    RetentionSignal {
        trajectory_id: t.id.clone(),
        continued_use: subsequent_tasks > 0,
        subsequent_tasks,
        verifications_passed,
        has_user_accept,
        confidence,
    }
}

// ---------------------------------------------------------------------------
// P5: Strategy Learning Proposal
// ---------------------------------------------------------------------------

/// A proposal for strategy learning, aggregated from multiple trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyLearningProposal {
    pub id: String,
    pub task_pattern: String,
    pub strategy_id: String,
    pub workflow_label: String,
    pub agent_plan_summary: String,
    pub trajectory_count: usize,
    pub rollback_rate: f64,
    pub success_rate: f64,
    pub user_accept_count: usize,
    pub user_reject_count: usize,
    pub failure_types: Vec<String>,
    pub claim: String,
    pub supporting_trajectory_ids: Vec<String>,
    pub counter_trajectory_ids: Vec<String>,
    pub confidence: f64,
    pub created_at: i64,
    pub status: String, // "open" | "approved" | "rejected"
}

/// Aggregate trajectories by strategy and task pattern to generate learning proposals.
pub fn generate_strategy_learning_proposals(
    store: &TrajectoryStore,
) -> Vec<StrategyLearningProposal> {
    // Group trajectories by (task_pattern, strategy_id)
    let mut groups: HashMap<(String, String), Vec<&DevelopmentTrajectory>> = HashMap::new();

    for t in &store.trajectories {
        // Only consider completed trajectories
        if t.completed_at.is_none() {
            continue;
        }
        let key = (
            t.intent.clone().unwrap_or_else(|| "unknown".to_string()),
            t.strategy_id
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        );
        groups.entry(key).or_default().push(t);
    }

    let mut proposals = Vec::new();

    for ((task_pattern, strategy_id), trajectories) in &groups {
        if trajectories.len() < 2 {
            continue; // Need at least 2 trajectories for a proposal
        }

        let total = trajectories.len();
        let rollbacks = trajectories.iter().filter(|t| t.rollback.is_some()).count();
        let successes = trajectories
            .iter()
            .filter(|t| t.outcome == "success")
            .count();
        let user_accepts = trajectories
            .iter()
            .filter(|t| {
                t.user_verdict
                    .as_ref()
                    .map(|v| v.verdict == "accepted")
                    .unwrap_or(false)
            })
            .count();
        let user_rejects = trajectories
            .iter()
            .filter(|t| {
                t.user_verdict
                    .as_ref()
                    .map(|v| v.verdict == "rejected")
                    .unwrap_or(false)
            })
            .count();

        let rollback_rate = rollbacks as f64 / total as f64;
        let success_rate = successes as f64 / total as f64;

        let mut failure_types = Vec::new();
        for t in trajectories {
            for f in &t.failures {
                if !failure_types.contains(&f.kind) {
                    failure_types.push(f.kind.clone());
                }
            }
        }

        let supporting_ids: Vec<String> = trajectories
            .iter()
            .filter(|t| {
                t.outcome == "success"
                    || t.user_verdict.as_ref().map(|v| v.verdict.as_str()) == Some("accepted")
            })
            .map(|t| t.id.clone())
            .collect();

        let counter_ids: Vec<String> = trajectories
            .iter()
            .filter(|t| {
                t.rollback.is_some()
                    || t.user_verdict.as_ref().map(|v| v.verdict.as_str()) == Some("rejected")
            })
            .map(|t| t.id.clone())
            .collect();

        // Generate claim based on evidence
        let claim = if success_rate > 0.7 && rollback_rate < 0.3 {
            format!(
                "Strategy '{}' for '{}' shows positive evidence ({}% success, {}% rollback)",
                strategy_id,
                task_pattern,
                (success_rate * 100.0) as u32,
                (rollback_rate * 100.0) as u32
            )
        } else if rollback_rate > 0.5 {
            format!(
                "Strategy '{}' for '{}' has high rollback rate ({}%) — consider alternatives",
                strategy_id,
                task_pattern,
                (rollback_rate * 100.0) as u32
            )
        } else {
            continue; // Not enough signal
        };

        // Confidence based on trajectory count and consistency
        let confidence = (total as f64).min(10.0) / 10.0 * 0.8
            + (success_rate - rollback_rate).abs().min(1.0) * 0.2;

        proposals.push(StrategyLearningProposal {
            id: format!("slp_{}", ulid::Ulid::new().to_string()),
            task_pattern: task_pattern.clone(),
            strategy_id: strategy_id.clone(),
            workflow_label: trajectories[0].workflow_revisions.join(","),
            agent_plan_summary: trajectories[0]
                .agent_plan
                .as_ref()
                .map(|_| "present".to_string())
                .unwrap_or_else(|| "none".to_string()),
            trajectory_count: total,
            rollback_rate,
            success_rate,
            user_accept_count: user_accepts,
            user_reject_count: user_rejects,
            failure_types,
            claim,
            supporting_trajectory_ids: supporting_ids,
            counter_trajectory_ids: counter_ids,
            confidence: confidence.min(0.95),
            created_at: route_core::now_millis(),
            status: "open".to_string(),
        });
    }

    proposals
}

// ---------------------------------------------------------------------------
// P6: Workflow Evolution from Trajectories
// ---------------------------------------------------------------------------

/// A workflow change proposal generated from trajectory analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryWorkflowProposal {
    pub id: String,
    pub workflow_id: String,
    pub change_type: String, // "add_step" | "remove_step" | "reorder" | "change_checkpoint" | "change_review" | "change_verification"
    pub description: String,
    pub supporting_trajectory_ids: Vec<String>,
    pub counter_evidence_trajectory_ids: Vec<String>,
    pub scope: String,
    pub confidence: f64,
    pub expected_effect: String,
    pub created_at: i64,
    pub status: String,
}

/// Store for trajectory-based workflow proposals.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrajectoryWorkflowProposalStore {
    pub proposals: Vec<TrajectoryWorkflowProposal>,
}

impl TrajectoryWorkflowProposalStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = proposals_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p).unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = proposals_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&p, json)?;
        Ok(())
    }

    pub fn add(&mut self, p: TrajectoryWorkflowProposal) {
        self.proposals.push(p);
    }

    pub fn list(&self) -> Vec<&TrajectoryWorkflowProposal> {
        self.proposals.iter().collect()
    }
}

/// Generate workflow evolution proposals from trajectory analysis.
pub fn generate_workflow_evolution_proposals(
    trajectory_store: &TrajectoryStore,
    workflow_id: &str,
) -> Vec<TrajectoryWorkflowProposal> {
    let related: Vec<_> = trajectory_store
        .trajectories
        .iter()
        .filter(|t| t.workflow_revisions.iter().any(|w| w.contains(workflow_id)))
        .collect();

    if related.len() < 2 {
        return Vec::new();
    }

    let mut proposals = Vec::new();

    // Check if rollback rate is high — suggest adding a review step
    let rollbacks = related.iter().filter(|t| t.rollback.is_some()).count();
    if rollbacks as f64 / related.len() as f64 > 0.4 {
        let supporting: Vec<String> = related
            .iter()
            .filter(|t| t.rollback.is_some())
            .map(|t| t.id.clone())
            .collect();
        proposals.push(TrajectoryWorkflowProposal {
            id: format!("twp_{}", ulid::Ulid::new().to_string()),
            workflow_id: workflow_id.to_string(),
            change_type: "change_review".to_string(),
            description: format!(
                "High rollback rate ({}%) — consider adding independent review step",
                (rollbacks as f64 / related.len() as f64 * 100.0) as u32
            ),
            supporting_trajectory_ids: supporting,
            counter_evidence_trajectory_ids: Vec::new(),
            scope: "project".to_string(),
            confidence: 0.4,
            expected_effect: "Reduce rollback rate through early issue detection".to_string(),
            created_at: route_core::now_millis(),
            status: "open".to_string(),
        });
    }

    // Check if user rejections are common — suggest changing verification
    let rejects = related
        .iter()
        .filter(|t| {
            t.user_verdict
                .as_ref()
                .map(|v| v.verdict == "rejected")
                .unwrap_or(false)
        })
        .count();
    if rejects as f64 / related.len() as f64 > 0.3 {
        let supporting: Vec<String> = related
            .iter()
            .filter(|t| {
                t.user_verdict
                    .as_ref()
                    .map(|v| v.verdict == "rejected")
                    .unwrap_or(false)
            })
            .map(|t| t.id.clone())
            .collect();
        proposals.push(TrajectoryWorkflowProposal {
            id: format!("twp_{}", ulid::Ulid::new().to_string()),
            workflow_id: workflow_id.to_string(),
            change_type: "change_verification".to_string(),
            description: format!(
                "High user rejection rate ({}%) — consider tightening verification criteria",
                (rejects as f64 / related.len() as f64 * 100.0) as u32
            ),
            supporting_trajectory_ids: supporting,
            counter_evidence_trajectory_ids: Vec::new(),
            scope: "project".to_string(),
            confidence: 0.35,
            expected_effect: "Improve output quality to reduce rejection rate".to_string(),
            created_at: route_core::now_millis(),
            status: "open".to_string(),
        });
    }

    proposals
}

// ---------------------------------------------------------------------------
// P7: Agent Plan Learning
// ---------------------------------------------------------------------------

/// Agent organization learning from trajectory analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlanInsight {
    pub id: String,
    pub task_pattern: String,
    pub roles_used: Vec<String>,
    pub effective: bool, // whether the agent plan led to success
    pub redundant_roles: Vec<String>,
    pub review_found_issues: bool,
    pub trajectory_ids: Vec<String>,
    pub observation: String,
    pub created_at: i64,
}

/// Analyze trajectories for agent plan insights.
pub fn analyze_agent_plans(store: &TrajectoryStore) -> Vec<AgentPlanInsight> {
    let mut insights = Vec::new();

    // Group by agent_plan pattern
    let mut by_plan: HashMap<String, Vec<&DevelopmentTrajectory>> = HashMap::new();
    for t in &store.trajectories {
        if let Some(ap) = &t.agent_plan {
            let key = ap.len().to_string(); // simplified grouping
            by_plan.entry(key).or_default().push(t);
        }
    }

    for (_plan_key, trajectories) in &by_plan {
        if trajectories.len() < 2 {
            continue;
        }

        let successes = trajectories
            .iter()
            .filter(|t| t.outcome == "success")
            .count();
        let failures = trajectories
            .iter()
            .filter(|t| t.outcome == "failed")
            .count();
        let effective = successes > failures;

        let task_pattern = trajectories[0]
            .intent
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let mut redundant_roles = Vec::new();
        let mut review_found = false;

        // Check for review-related evidence
        for t in trajectories {
            for d in &t.decisions {
                if d.description.to_lowercase().contains("review")
                    && d.description.to_lowercase().contains("found")
                {
                    review_found = true;
                }
            }
        }

        // Simple heuristic: if rollback rate is high, agent plan might be ineffective
        let rollbacks = trajectories.iter().filter(|t| t.rollback.is_some()).count();
        if rollbacks as f64 / trajectories.len() as f64 > 0.5 {
            redundant_roles.push("over-engineered-plan".to_string());
        }

        let trajectory_ids: Vec<String> = trajectories.iter().map(|t| t.id.clone()).collect();

        let observation = if effective {
            format!(
                "Agent plan pattern for '{}' shows positive results ({} success, {} failures)",
                task_pattern, successes, failures
            )
        } else {
            format!(
                "Agent plan pattern for '{}' shows issues ({} failures, {} successes)",
                task_pattern, failures, successes
            )
        };

        insights.push(AgentPlanInsight {
            id: format!("api_{}", ulid::Ulid::new().to_string()),
            task_pattern,
            roles_used: Vec::new(),
            effective,
            redundant_roles,
            review_found_issues: review_found,
            trajectory_ids,
            observation,
            created_at: route_core::now_millis(),
        });
    }

    insights
}

// ---------------------------------------------------------------------------
// P8: Memory Consolidation
// ---------------------------------------------------------------------------

/// A memory candidate generated from a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub id: String,
    pub trajectory_id: String,
    pub kind: String, // "decision" | "invariant" | "failure" | "resolution" | "architecture_change" | "open_question"
    pub content: String,
    pub source_ids: Vec<String>,
    pub confidence: f64,
    pub created_at: i64,
}

/// Consolidate a trajectory into memory candidates.
/// P8: Trajectory → MemoryCandidates. Original trajectory is always preserved.
pub fn consolidate_memory_candidates(t: &DevelopmentTrajectory) -> Vec<MemoryCandidate> {
    let mut candidates = Vec::new();

    // Decisions → memory candidates
    for d in &t.decisions {
        candidates.push(MemoryCandidate {
            id: format!("mc_{}", ulid::Ulid::new().to_string()),
            trajectory_id: t.id.clone(),
            kind: "decision".to_string(),
            content: format!("Decision: {} (rationale: {})", d.description, d.rationale),
            source_ids: vec![d.id.clone()],
            confidence: 0.5,
            created_at: route_core::now_millis(),
        });
    }

    // Failures → memory candidates
    for f in &t.failures {
        let kind = if f.resolution.is_some() {
            "resolution"
        } else {
            "failure"
        };
        let content = if let Some(ref res) = f.resolution {
            format!("Failure '{}' resolved by: {}", f.description, res)
        } else {
            format!("Failure: {} [{}]", f.description, f.kind)
        };
        candidates.push(MemoryCandidate {
            id: format!("mc_{}", ulid::Ulid::new().to_string()),
            trajectory_id: t.id.clone(),
            kind: kind.to_string(),
            content,
            source_ids: vec![f.id.clone()],
            confidence: 0.6,
            created_at: route_core::now_millis(),
        });
    }

    // User verdict → memory candidate
    if let Some(uv) = &t.user_verdict {
        let kind = if uv.verdict == "rejected" {
            "open_question"
        } else {
            "decision"
        };
        candidates.push(MemoryCandidate {
            id: format!("mc_{}", ulid::Ulid::new().to_string()),
            trajectory_id: t.id.clone(),
            kind: kind.to_string(),
            content: format!(
                "User verdict: {} — {}",
                uv.verdict,
                uv.comment.as_deref().unwrap_or("(no comment)")
            ),
            source_ids: vec![t.id.clone()],
            confidence: 0.7,
            created_at: route_core::now_millis(),
        });
    }

    // Rollback → memory candidate
    if let Some(rb) = &t.rollback {
        candidates.push(MemoryCandidate {
            id: format!("mc_{}", ulid::Ulid::new().to_string()),
            trajectory_id: t.id.clone(),
            kind: "failure".to_string(),
            content: format!(
                "Rollback to {} — reason: {}",
                rb.rolled_back_to,
                rb.reason.as_deref().unwrap_or("(no reason given)")
            ),
            source_ids: vec![t.id.clone()],
            confidence: 0.8, // Rollbacks are high-confidence signals
            created_at: route_core::now_millis(),
        });
    }

    candidates
}

// ---------------------------------------------------------------------------
// P9: Experience Distillation
// ---------------------------------------------------------------------------

/// A cross-trajectory pattern distilled from multiple trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledPattern {
    pub id: String,
    pub problem: String,
    pub successful_approaches: Vec<String>,
    pub failed_approaches: Vec<String>,
    pub context: String,
    pub tradeoffs: Vec<String>,
    pub evidence_trajectory_ids: Vec<String>,
    pub applicability: Vec<String>,
    pub confidence: f64,
    pub project_local: bool,
    pub created_at: i64,
}

/// Distill patterns from multiple trajectories with the same task pattern.
pub fn distill_patterns(store: &TrajectoryStore, task_pattern: &str) -> Vec<DistilledPattern> {
    let related = store.by_task_pattern(task_pattern);
    if related.len() < 2 {
        return Vec::new();
    }

    let successful: Vec<&DevelopmentTrajectory> = related
        .iter()
        .filter(|t| {
            t.outcome == "success"
                && t.user_verdict
                    .as_ref()
                    .map(|v| v.verdict == "accepted")
                    .unwrap_or(false)
        })
        .copied()
        .collect();

    let failed: Vec<&DevelopmentTrajectory> = related
        .iter()
        .filter(|t| t.rollback.is_some() || t.outcome == "failed")
        .copied()
        .collect();

    if successful.is_empty() && failed.is_empty() {
        return Vec::new();
    }

    // Build successful approaches from successful trajectories
    let successful_approaches: Vec<String> = successful
        .iter()
        .map(|t| {
            format!(
                "Strategy: {}, Workflow: {}, Decisions: {}",
                t.strategy_id.as_deref().unwrap_or("default"),
                t.workflow_revisions.join(","),
                t.decisions.len()
            )
        })
        .collect();

    // Build failed approaches from failed trajectories
    let failed_approaches: Vec<String> = failed
        .iter()
        .map(|t| {
            format!(
                "Strategy: {}, Failures: {}",
                t.strategy_id.as_deref().unwrap_or("default"),
                t.failures
                    .iter()
                    .map(|f| f.description.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })
        .collect();

    let evidence_ids: Vec<String> = related.iter().map(|t| t.id.clone()).collect();
    let confidence = (related.len() as f64).min(5.0) / 5.0 * 0.7;

    let mut tradeoffs = Vec::new();
    if !successful_approaches.is_empty() && !failed_approaches.is_empty() {
        tradeoffs.push(format!(
            "{} successful approaches vs {} failed approaches",
            successful_approaches.len(),
            failed_approaches.len()
        ));
    }

    vec![DistilledPattern {
        id: format!("dp_{}", ulid::Ulid::new().to_string()),
        problem: task_pattern.to_string(),
        successful_approaches,
        failed_approaches,
        context: format!("Aggregated from {} trajectories", related.len()),
        tradeoffs,
        evidence_trajectory_ids: evidence_ids,
        applicability: vec!["project-local".to_string()],
        confidence: confidence.min(0.95),
        project_local: true,
        created_at: route_core::now_millis(),
    }]
}

// ---------------------------------------------------------------------------
// P10: Cross-Project Pattern
// ---------------------------------------------------------------------------

/// A cross-project pattern that can be shared between projects.
/// P10: Memory is strictly isolated; patterns can be shared optionally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossProjectPattern {
    pub id: String,
    pub pattern: DistilledPattern,
    pub source_projects: Vec<String>, // project paths or identifiers
    pub differences: Vec<String>,
    pub confidence: f64,
}

/// Check if a pattern can be promoted to cross-project.
/// P10: Only when evidence is sufficient across multiple projects.
pub fn promote_to_cross_project(
    pattern: &DistilledPattern,
    project_id: &str,
    other_project_patterns: &[DistilledPattern],
) -> Option<CrossProjectPattern> {
    if pattern.confidence < 0.6 {
        return None; // Not confident enough
    }

    let mut all_projects = vec![project_id.to_string()];
    let mut differences = Vec::new();

    for other in other_project_patterns {
        if other.problem == pattern.problem && other.confidence >= 0.5 {
            // Same problem found in another project
            all_projects.push(format!("project-{}", other.id));
            if other.applicability != pattern.applicability {
                differences.push(format!(
                    "Applicability differs: {:?} vs {:?}",
                    pattern.applicability, other.applicability
                ));
            }
        }
    }

    if all_projects.len() < 2 {
        return None; // Only one project — keep project-local
    }

    // Cross-project confidence is lower than project-local
    let cross_confidence = pattern.confidence * 0.8;

    Some(CrossProjectPattern {
        id: format!("cpp_{}", ulid::Ulid::new().to_string()),
        pattern: pattern.clone(),
        source_projects: all_projects,
        differences,
        confidence: cross_confidence,
    })
}

// ---------------------------------------------------------------------------
// P11: Learning Explain
// ---------------------------------------------------------------------------

/// A traceable explanation chain for a learning result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningExplanation {
    pub target_id: String,
    pub target_type: String, // "pattern" | "proposal" | "insight" | "candidate"
    pub title: String,
    pub chain: Vec<ExplanationLink>,
    pub created_at: i64,
}

/// A single link in the explanation chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplanationLink {
    pub kind: String, // "pattern" | "proposal" | "trajectory" | "task" | "evidence" | "savepoint"
    pub id: String,
    pub description: String,
}

/// Build an explanation chain for a learning target.
/// P11: Must be able to trace back to original trajectories and evidence.
pub fn explain_learning(
    trajectory_store: &TrajectoryStore,
    target_id: &str,
    target_type: &str,
) -> LearningExplanation {
    let mut chain = Vec::new();

    match target_type {
        "trajectory" => {
            if let Some(t) = trajectory_store.get(target_id) {
                chain.push(ExplanationLink {
                    kind: "trajectory".to_string(),
                    id: t.id.clone(),
                    description: t.intent.clone().unwrap_or_else(|| "Trajectory".to_string()),
                });

                // Link to savepoints
                chain.push(ExplanationLink {
                    kind: "savepoint".to_string(),
                    id: t.from_savepoint.clone(),
                    description: "Starting savepoint".to_string(),
                });

                if let Some(to) = &t.to_savepoint {
                    chain.push(ExplanationLink {
                        kind: "savepoint".to_string(),
                        id: to.clone(),
                        description: "Ending savepoint".to_string(),
                    });
                }

                // Link to evidence
                for eid in &t.evidence_ids {
                    chain.push(ExplanationLink {
                        kind: "evidence".to_string(),
                        id: eid.clone(),
                        description: "Execution evidence".to_string(),
                    });
                }
            }
        }
        "proposal" | "pattern" | "insight" | "candidate" => {
            // Generic: include the target itself
            chain.push(ExplanationLink {
                kind: target_type.to_string(),
                id: target_id.to_string(),
                description: "Learning target".to_string(),
            });

            // Search trajectories for references to this target
            for t in &trajectory_store.trajectories {
                if t.id.starts_with(target_id) || target_id.starts_with(&t.id) {
                    chain.push(ExplanationLink {
                        kind: "trajectory".to_string(),
                        id: t.id.clone(),
                        description: t.intent.clone().unwrap_or_default(),
                    });
                    break;
                }
            }
        }
        _ => {
            chain.push(ExplanationLink {
                kind: "unknown".to_string(),
                id: target_id.to_string(),
                description: format!("Unknown target type: {}", target_type),
            });
        }
    }

    LearningExplanation {
        target_id: target_id.to_string(),
        target_type: target_type.to_string(),
        title: format!("Explanation for {} [{}]", target_type, target_id),
        chain,
        created_at: route_core::now_millis(),
    }
}

// ---------------------------------------------------------------------------
// P12: Forget / Downgrade Support
// ---------------------------------------------------------------------------

/// A forget/downgrade action record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningAction {
    pub id: String,
    pub action_type: String, // "reject" | "supersede" | "downgrade"
    pub target_id: String,
    pub target_kind: String, // "proposal" | "pattern" | "insight"
    pub reason: Option<String>,
    pub new_confidence: Option<f64>,
    pub superseded_by: Option<String>,
    pub created_at: i64,
}

/// Store for learning actions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearningActionStore {
    pub actions: Vec<LearningAction>,
}

impl LearningActionStore {
    fn path(project_root: &Path) -> PathBuf {
        trajectory_dir(project_root).join("actions.json")
    }

    pub fn load(project_root: &Path) -> Result<Self> {
        let p = Self::path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p).unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = Self::path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&p, json)?;
        Ok(())
    }

    pub fn add(&mut self, action: LearningAction) {
        self.actions.push(action);
    }

    pub fn list(&self) -> Vec<&LearningAction> {
        self.actions.iter().collect()
    }
}

// ---------------------------------------------------------------------------
// P13: Learning Dashboard
// ---------------------------------------------------------------------------

/// A summary of the learning system state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearningDashboard {
    pub approved_knowledge: usize,
    pub pending_proposals: usize,
    pub conflicting_knowledge: usize,
    pub stale_knowledge: usize,
    pub recent_lessons: Vec<String>,
    pub total_trajectories: usize,
    pub total_proposals: usize,
}

/// Generate a learning dashboard summary.
pub fn generate_learning_dashboard(
    trajectory_store: &TrajectoryStore,
    proposals: &[StrategyLearningProposal],
    workflow_proposals: &[TrajectoryWorkflowProposal],
    actions: &[LearningAction],
) -> LearningDashboard {
    let total_trajectories = trajectory_store.trajectories.len();
    let total_proposals = proposals.len() + workflow_proposals.len();

    let approved = proposals.iter().filter(|p| p.status == "approved").count()
        + workflow_proposals
            .iter()
            .filter(|p| p.status == "approved")
            .count();

    let pending = proposals.iter().filter(|p| p.status == "open").count()
        + workflow_proposals
            .iter()
            .filter(|p| p.status == "open")
            .count();

    let rejected = proposals.iter().filter(|p| p.status == "rejected").count()
        + workflow_proposals
            .iter()
            .filter(|p| p.status == "rejected")
            .count();

    // Recent lessons: last 5 successful trajectories with user verdicts
    let mut recent: Vec<String> = trajectory_store
        .trajectories
        .iter()
        .filter(|t| t.user_verdict.is_some())
        .map(|t| {
            format!(
                "{} — {} (verdict: {})",
                t.intent.as_deref().unwrap_or("(unnamed)"),
                t.outcome,
                t.user_verdict
                    .as_ref()
                    .map(|v| v.verdict.as_str())
                    .unwrap_or("(none)")
            )
        })
        .collect();
    recent.reverse();
    recent.truncate(5);

    LearningDashboard {
        approved_knowledge: approved,
        pending_proposals: pending,
        conflicting_knowledge: rejected,
        stale_knowledge: actions.len(),
        recent_lessons: recent,
        total_trajectories,
        total_proposals,
    }
}

pub fn format_dashboard(d: &LearningDashboard) -> String {
    let mut out = format!(
        "Learning Dashboard\n\
         ─────────────────\n\
         Total Trajectories:  {}\n\
         Total Proposals:     {}\n\
         Approved Knowledge:  {}\n\
         Pending Proposals:   {}\n\
         Conflicting:         {}\n\
         Actions (stale/etc): {}\n",
        d.total_trajectories,
        d.total_proposals,
        d.approved_knowledge,
        d.pending_proposals,
        d.conflicting_knowledge,
        d.stale_knowledge,
    );

    if d.recent_lessons.is_empty() {
        out.push_str("\nRecent Lessons: (none)\n");
    } else {
        out.push_str("\nRecent Lessons:\n");
        for lesson in &d.recent_lessons {
            out.push_str(&format!("  - {}\n", lesson));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Trajectory creation helpers
// ---------------------------------------------------------------------------

/// Create a new trajectory bound to a savepoint and task.
pub fn create_trajectory(
    from_savepoint: String,
    task_id: Option<String>,
    intent: Option<String>,
    strategy_id: Option<String>,
    workflow_revisions: Vec<String>,
    agent_plan: Option<String>,
) -> DevelopmentTrajectory {
    DevelopmentTrajectory {
        id: format!("tr_{}", ulid::Ulid::new().to_string()),
        from_savepoint,
        to_savepoint: None,
        task_id,
        intent,
        strategy_id,
        workflow_revisions,
        agent_plan,
        evidence_ids: Vec::new(),
        decisions: Vec::new(),
        failures: Vec::new(),
        rollback: None,
        outcome: "in_progress".to_string(),
        user_verdict: None,
        created_at: route_core::now_millis(),
        completed_at: None,
        tags: Vec::new(),
    }
}

/// Complete a trajectory with an ending savepoint and outcome.
pub fn complete_trajectory(
    t: &mut DevelopmentTrajectory,
    to_savepoint: String,
    outcome: String,
    user_verdict: Option<UserVerdict>,
) {
    t.to_savepoint = Some(to_savepoint);
    t.outcome = outcome;
    t.user_verdict = user_verdict;
    t.completed_at = Some(route_core::now_millis());
}

/// Record a decision in a trajectory.
pub fn record_decision(
    t: &mut DevelopmentTrajectory,
    description: String,
    alternatives: Vec<String>,
    rationale: String,
    module: Option<String>,
) -> String {
    let id = format!("dec_{}", ulid::Ulid::new().to_string());
    t.decisions.push(DecisionRecord {
        id: id.clone(),
        description,
        alternatives,
        rationale,
        module,
        created_at: route_core::now_millis(),
    });
    id
}

/// Record a failure in a trajectory.
pub fn record_failure(
    t: &mut DevelopmentTrajectory,
    description: String,
    kind: String,
    module: Option<String>,
    resolution: Option<String>,
) -> String {
    let id = format!("fail_{}", ulid::Ulid::new().to_string());
    t.failures.push(FailureRecord {
        id: id.clone(),
        description,
        kind,
        module,
        resolution,
        created_at: route_core::now_millis(),
    });
    id
}

/// Record a rollback in a trajectory.
pub fn record_rollback(
    t: &mut DevelopmentTrajectory,
    rolled_back_to: String,
    reason: Option<String>,
    changes_lost: Vec<String>,
    verification_before_rollback: Option<String>,
) {
    t.rollback = Some(RollbackRecord {
        rolled_back_to,
        reason,
        changes_lost,
        verification_before_rollback,
        created_at: route_core::now_millis(),
    });
    t.outcome = "rolled_back".to_string();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trajectory(intent: &str, strategy_id: &str, outcome: &str) -> DevelopmentTrajectory {
        DevelopmentTrajectory {
            id: format!("tr_{}", ulid::Ulid::new().to_string()),
            from_savepoint: "sp_test".to_string(),
            to_savepoint: Some("sp_test_end".to_string()),
            task_id: None,
            intent: Some(intent.to_string()),
            strategy_id: Some(strategy_id.to_string()),
            workflow_revisions: vec!["wf_test".to_string()],
            agent_plan: None,
            evidence_ids: Vec::new(),
            decisions: Vec::new(),
            failures: Vec::new(),
            rollback: None,
            outcome: outcome.to_string(),
            user_verdict: None,
            created_at: route_core::now_millis(),
            completed_at: Some(route_core::now_millis()),
            tags: Vec::new(),
        }
    }

    #[test]
    fn test_create_and_store_trajectory() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".route")).unwrap();

        let mut store = TrajectoryStore::default();
        let t = create_trajectory(
            "sp1".to_string(),
            None,
            Some("test task".to_string()),
            Some("strat1".to_string()),
            vec!["wf1".to_string()],
            None,
        );
        store.add(t);
        store.save(root).unwrap();

        let loaded = TrajectoryStore::load(root).unwrap();
        assert_eq!(loaded.trajectories.len(), 1);
        assert_eq!(loaded.trajectories[0].intent.as_deref(), Some("test task"));
    }

    #[test]
    fn test_trajectory_list_order() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        let mut store = TrajectoryStore::default();
        let t1 = create_trajectory(
            "sp1".to_string(),
            None,
            Some("first".to_string()),
            None,
            vec![],
            None,
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
        let t2 = create_trajectory(
            "sp2".to_string(),
            None,
            Some("second".to_string()),
            None,
            vec![],
            None,
        );
        store.add(t1);
        store.add(t2);

        let list = store.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].intent.as_deref(), Some("second")); // newest first
        assert_eq!(list[1].intent.as_deref(), Some("first"));
    }

    #[test]
    fn test_find_by_outcome() {
        let mut store = TrajectoryStore::default();
        store.add(make_trajectory("t1", "s1", "success"));
        store.add(make_trajectory("t2", "s1", "failed"));
        store.add(make_trajectory("t3", "s2", "success"));

        assert_eq!(store.by_outcome("success").len(), 2);
        assert_eq!(store.by_outcome("failed").len(), 1);
    }

    #[test]
    fn test_find_by_strategy() {
        let mut store = TrajectoryStore::default();
        store.add(make_trajectory("t1", "s1", "success"));
        store.add(make_trajectory("t2", "s2", "success"));
        store.add(make_trajectory("t3", "s1", "failed"));

        assert_eq!(store.by_strategy("s1").len(), 2);
        assert_eq!(store.by_strategy("s2").len(), 1);
    }

    #[test]
    fn test_diff_trajectories() {
        let a = make_trajectory("before", "s1", "failed");
        let b = make_trajectory("after", "s2", "success");
        let diff = diff_trajectories(&a, &b);
        assert_eq!(diff.a_outcome, "failed");
        assert_eq!(diff.b_outcome, "success");
        assert!(diff.strategy_changed);
    }

    #[test]
    fn test_analyze_reversal() {
        let mut t = make_trajectory("test", "s1", "rolled_back");
        t.rollback = Some(RollbackRecord {
            rolled_back_to: "sp_original".to_string(),
            reason: Some("user rejected changes".to_string()),
            changes_lost: vec!["refactor".to_string()],
            verification_before_rollback: None,
            created_at: route_core::now_millis(),
        });
        let analysis = analyze_reversal(&t).unwrap();
        assert!(analysis.explicit_reason.is_some());
        assert!(analysis.is_candidate);
        assert!(analysis.observation.contains("explicit reason"));
    }

    #[test]
    fn test_analyze_retention() {
        let mut store = TrajectoryStore::default();
        let t1 = create_trajectory(
            "spA".to_string(),
            None,
            Some("base".to_string()),
            None,
            vec![],
            None,
        );
        let mut t2 = create_trajectory(
            "spA".to_string(),
            None,
            Some("builds on".to_string()),
            None,
            vec![],
            None,
        );
        t2.user_verdict = Some(UserVerdict {
            verdict: "accepted".to_string(),
            comment: None,
            created_at: route_core::now_millis(),
        });
        store.add(t1);
        store.add(t2.clone());

        let retention = analyze_retention(&t2, &store);
        // Since t2's from_savepoint is "spA" and t1's to_savepoint is None, subsequent_tasks should be 0
        assert_eq!(retention.confidence, 0.3); // has user accept, no subsequent tasks
    }

    #[test]
    fn test_strategy_learning_proposals() {
        let mut store = TrajectoryStore::default();
        // Add 3 successful trajectories with same strategy
        for i in 0..3 {
            let mut t = make_trajectory("migration", "strict", "success");
            t.user_verdict = Some(UserVerdict {
                verdict: "accepted".to_string(),
                comment: None,
                created_at: route_core::now_millis(),
            });
            store.add(t);
        }
        // Add 1 failed trajectory with same strategy
        store.add(make_trajectory("migration", "strict", "failed"));

        let proposals = generate_strategy_learning_proposals(&store);
        // Should find at least one proposal for "migration" + "strict"
        let migration_proposals: Vec<_> = proposals
            .iter()
            .filter(|p| p.task_pattern == "migration")
            .collect();
        assert!(!migration_proposals.is_empty());
        let p = migration_proposals[0];
        assert_eq!(p.trajectory_count, 4);
        assert!(p.success_rate > 0.5);
    }

    #[test]
    fn test_memory_consolidation() {
        let mut t = make_trajectory("test", "s1", "success");
        record_decision(
            &mut t,
            "chose approach A".to_string(),
            vec!["B".to_string()],
            "faster".to_string(),
            Some("core".to_string()),
        );
        record_failure(
            &mut t,
            "test failed".to_string(),
            "test_fail".to_string(),
            Some("core".to_string()),
            Some("fixed by adding null check".to_string()),
        );

        let candidates = consolidate_memory_candidates(&t);
        assert!(candidates.len() >= 2);
        assert!(candidates.iter().any(|c| c.kind == "decision"));
        assert!(candidates.iter().any(|c| c.kind == "resolution"));
    }

    #[test]
    fn test_distill_patterns() {
        let mut store = TrajectoryStore::default();

        // 2 successful trajectories
        let mut t1 = make_trajectory("refactor rollback", "strict", "success");
        t1.user_verdict = Some(UserVerdict {
            verdict: "accepted".to_string(),
            comment: None,
            created_at: 0,
        });
        store.add(t1);

        let mut t2 = make_trajectory("refactor rollback", "strict", "success");
        t2.user_verdict = Some(UserVerdict {
            verdict: "accepted".to_string(),
            comment: None,
            created_at: 0,
        });
        store.add(t2);

        // 1 failed trajectory
        let mut t3 = make_trajectory("refactor rollback", "loose", "failed");
        t3.rollback = Some(RollbackRecord {
            rolled_back_to: "sp_orig".to_string(),
            reason: None,
            changes_lost: vec![],
            verification_before_rollback: None,
            created_at: 0,
        });
        store.add(t3);

        let patterns = distill_patterns(&store, "refactor rollback");
        assert!(!patterns.is_empty());
        assert_eq!(patterns[0].problem, "refactor rollback");
        assert!(!patterns[0].successful_approaches.is_empty());
        assert!(!patterns[0].failed_approaches.is_empty());
    }

    #[test]
    fn test_auto_save_state() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".route")).unwrap();

        let mut state = AutoSaveState::default();
        assert!(state.has_changed("task_start", "hash1"));
        state.record("task_start", "hash1".to_string(), "tr1".to_string());
        assert!(!state.has_changed("task_start", "hash1"));
        assert!(state.has_changed("task_start", "hash2"));

        state.save(root).unwrap();
        let loaded = AutoSaveState::load(root).unwrap();
        assert_eq!(
            loaded.last_hashes.get("task_start").map(|s| s.as_str()),
            Some("hash1")
        );
    }

    #[test]
    fn test_learning_dashboard() {
        let store = TrajectoryStore::default();
        let proposals = Vec::new();
        let wf_proposals = Vec::new();
        let actions = Vec::new();

        let dashboard = generate_learning_dashboard(&store, &proposals, &wf_proposals, &actions);
        assert_eq!(dashboard.total_trajectories, 0);
        assert_eq!(dashboard.total_proposals, 0);
    }

    #[test]
    fn test_workflow_evolution_proposals() {
        let mut store = TrajectoryStore::default();

        // Add many trajectories with rollbacks using workflow "wf1"
        for _ in 0..4 {
            let mut t = make_trajectory("test", "s1", "rolled_back");
            t.rollback = Some(RollbackRecord {
                rolled_back_to: "sp_orig".to_string(),
                reason: None,
                changes_lost: vec![],
                verification_before_rollback: None,
                created_at: 0,
            });
            store.add(t);
        }
        // Add 2 successful ones
        for _ in 0..2 {
            store.add(make_trajectory("test", "s1", "success"));
        }

        let proposals = generate_workflow_evolution_proposals(&store, "wf_test");
        assert!(!proposals.is_empty());
        // Should suggest adding review step due to high rollback rate
        assert!(proposals[0].change_type == "change_review");
    }

    #[test]
    fn test_agent_plan_insights() {
        let mut store = TrajectoryStore::default();
        let mut t1 = make_trajectory("doc task", "simple", "success");
        t1.agent_plan = Some("{\"roles\": [\"writer\"]}".to_string());
        store.add(t1);

        let mut t2 = make_trajectory("doc task", "simple", "success");
        t2.agent_plan = Some("{\"roles\": [\"writer\"]}".to_string());
        store.add(t2);

        let insights = analyze_agent_plans(&store);
        assert!(!insights.is_empty());
    }

    #[test]
    fn test_cross_project_promotion() {
        let pattern = DistilledPattern {
            id: "dp1".to_string(),
            problem: "schema migration".to_string(),
            successful_approaches: vec!["read-compat first".to_string()],
            failed_approaches: vec![],
            context: "test".to_string(),
            tradeoffs: vec![],
            evidence_trajectory_ids: vec![],
            applicability: vec![],
            confidence: 0.7,
            project_local: true,
            created_at: 0,
        };

        let other = DistilledPattern {
            id: "dp2".to_string(),
            problem: "schema migration".to_string(),
            successful_approaches: vec!["read-compat first".to_string()],
            failed_approaches: vec![],
            context: "test2".to_string(),
            tradeoffs: vec![],
            evidence_trajectory_ids: vec![],
            applicability: vec![],
            confidence: 0.6,
            project_local: true,
            created_at: 0,
        };

        let cross = promote_to_cross_project(&pattern, "project-A", &[other]);
        assert!(cross.is_some());
        assert_eq!(cross.unwrap().source_projects.len(), 2);
    }

    #[test]
    fn test_explain_learning() {
        let mut store = TrajectoryStore::default();
        let t = create_trajectory(
            "sp1".to_string(),
            None,
            Some("test".to_string()),
            None,
            vec![],
            None,
        );
        let id = t.id.clone();
        store.add(t);

        let explanation = explain_learning(&store, &id, "trajectory");
        assert_eq!(explanation.target_type, "trajectory");
        assert!(!explanation.chain.is_empty());
    }

    #[test]
    fn test_learning_actions() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".route")).unwrap();

        let mut store = LearningActionStore::default();
        store.add(LearningAction {
            id: "act1".to_string(),
            action_type: "reject".to_string(),
            target_id: "prop1".to_string(),
            target_kind: "proposal".to_string(),
            reason: Some("not applicable".to_string()),
            new_confidence: None,
            superseded_by: None,
            created_at: route_core::now_millis(),
        });
        store.save(root).unwrap();

        let loaded = LearningActionStore::load(root).unwrap();
        assert_eq!(loaded.actions.len(), 1);
        assert_eq!(loaded.actions[0].action_type, "reject");
    }

    #[test]
    fn test_complete_trajectory() {
        let mut t = create_trajectory(
            "sp_start".to_string(),
            None,
            Some("test".to_string()),
            None,
            vec![],
            None,
        );
        assert_eq!(t.outcome, "in_progress");
        assert!(t.completed_at.is_none());

        complete_trajectory(
            &mut t,
            "sp_end".to_string(),
            "success".to_string(),
            Some(UserVerdict {
                verdict: "accepted".to_string(),
                comment: Some("looks good".to_string()),
                created_at: route_core::now_millis(),
            }),
        );

        assert_eq!(t.outcome, "success");
        assert!(t.completed_at.is_some());
        assert_eq!(t.to_savepoint, Some("sp_end".to_string()));
        assert_eq!(t.user_verdict.as_ref().unwrap().verdict, "accepted");
    }
}
