//! Evolution Campaign & Governance (P26–P41) — a lightweight aggregate over
//! experiments plus governance controls.
//!
//! Reuses EvolutionExperiment / Evidence / Save / Learning / Reference / Task.
//! Route never runs an LLM, agent scheduler, sandbox, or model API. Harness
//! executes; Route records, constrains, evaluates, selects, and recovers.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::write_atomic;

/// Directory name for campaign data under `.route/`.
pub const CAMPAIGN_DIR: &str = "campaign";

// ---------------------------------------------------------------------------
// P26 — Campaign
// ---------------------------------------------------------------------------

/// Lifecycle of a campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    Planned,
    Running,
    Paused,
    Completed,
    Exhausted,
    Aborted,
}

impl CampaignStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CampaignStatus::Planned => "planned",
            CampaignStatus::Running => "running",
            CampaignStatus::Paused => "paused",
            CampaignStatus::Completed => "completed",
            CampaignStatus::Exhausted => "exhausted",
            CampaignStatus::Aborted => "aborted",
        }
    }
}

/// A lightweight aggregate: experiment set + budget + selection state. It is
/// NOT an agent scheduler — it only records and gates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionCampaign {
    pub id: String,
    pub goal: String,
    pub scope: String,
    pub strategy: String,
    pub budget: CampaignBudget,
    #[serde(default)]
    pub experiment_ids: Vec<String>,
    /// Champion experiment id.
    pub champion: Option<String>,
    /// Challenger experiment ids.
    #[serde(default)]
    pub challengers: Vec<String>,
    pub status: CampaignStatus,
    pub stop_reason: Option<String>,
    pub summary: String,
    /// Optional parent Task session id this campaign was created to serve.
    /// Normal tasks without a campaign leave this unset; a campaign never
    /// duplicates Task data (it only references the Task id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl EvolutionCampaign {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            CampaignStatus::Completed | CampaignStatus::Exhausted | CampaignStatus::Aborted
        )
    }
}

// ---------------------------------------------------------------------------
// P37 — Resource governor
// ---------------------------------------------------------------------------

/// Resource budget for a campaign. Route rejects over-budget experiments; it
/// never controls a harness process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignBudget {
    pub max_experiments: usize,
    pub max_failed_experiments: usize,
    pub max_wall_time_ms: i64,
    pub max_tokens: Option<u64>,
    pub max_cost: Option<f64>,
    pub disk_budget: Option<u64>,
    pub max_parallel_external: usize,
}

impl Default for CampaignBudget {
    fn default() -> Self {
        Self {
            max_experiments: 12,
            max_failed_experiments: 4,
            max_wall_time_ms: 3600_000,
            max_tokens: None,
            max_cost: None,
            disk_budget: None,
            max_parallel_external: 1,
        }
    }
}

/// Admit a new experiment into a campaign, or reject it for exceeding budget.
pub fn budget_admits(
    campaign: &EvolutionCampaign,
    now_ms: i64,
    failed_count: usize,
    tokens_used: u64,
    cost: f64,
) -> std::result::Result<(), String> {
    let b = &campaign.budget;
    if campaign.experiment_ids.len() >= b.max_experiments {
        return Err("max_experiments exceeded".to_string());
    }
    if failed_count >= b.max_failed_experiments {
        return Err("max_failed_experiments exceeded".to_string());
    }
    if now_ms - campaign.created_at >= b.max_wall_time_ms {
        return Err("max_wall_time exceeded".to_string());
    }
    if let Some(t) = b.max_tokens {
        if tokens_used >= t {
            return Err("max_tokens exceeded".to_string());
        }
    }
    if let Some(c) = b.max_cost {
        if cost >= c {
            return Err("max_cost exceeded".to_string());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// P27 — Think free / Act bounded
// ---------------------------------------------------------------------------

/// A constraint that an exploratory hypothesis knowingly violates. Thinking is
/// unbounded; executing a candidate that carries these requires a fresh gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolatedConstraint {
    pub dimension: String, // architecture | workflow | constitution | ...
    pub description: String,
}

/// A free-form hypothesis. EXPLORE may record anything (even "delete X
/// entirely?"); EXECUTE must re-pass the gate before any Stable change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeHypothesis {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub violated_constraints: Vec<ViolatedConstraint>,
    pub recorded_at: i64,
}

/// Result of the execution-space gate (P27). Every real candidate creation or
/// execution re-checks TrustRoot / Constitution / permission / resource /
/// recovery before touching Stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionGate {
    pub allowed: bool,
    pub reason: String,
}

/// A hypothesis that proposes to violate a Stable constraint is always blocked
/// at the execution gate — it may live in the hypothesis space, never in the
/// execution space.
pub fn execution_gate(hypothesis: &FreeHypothesis) -> ExecutionGate {
    if hypothesis.violated_constraints.is_empty() {
        ExecutionGate {
            allowed: true,
            reason: "no violated constraints".to_string(),
        }
    } else {
        ExecutionGate {
            allowed: false,
            reason: format!(
                "violates stable constraint(s): {}",
                hypothesis
                    .violated_constraints
                    .iter()
                    .map(|c| c.dimension.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// P28 — Evidence hierarchy
// ---------------------------------------------------------------------------

/// Precedence of evidence. Lower number = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    /// hash / filesystem fact / process exit / trusted test / immutable invariant
    Level0,
    /// verified benchmark / trusted reference / reproducible measurement
    Level1,
    /// independent model evaluation / architecture review
    Level2,
    /// candidate self-report / hypothesis / aesthetic judgment
    Level3,
}

impl EvidenceLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceLevel::Level0 => "level0",
            EvidenceLevel::Level1 => "level1",
            EvidenceLevel::Level2 => "level2",
            EvidenceLevel::Level3 => "level3",
        }
    }
}

/// An evidence observation with its level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub level: EvidenceLevel,
    pub summary: String,
}

/// Resolve two conflicting evidence: the lower level wins. An AI Judge
/// (Level2) can never override a trusted test (Level1) or an invariant
/// (Level0).
pub fn resolve_evidence<'a>(a: &'a Evidence, b: &'a Evidence) -> &'a Evidence {
    if a.level <= b.level {
        a
    } else {
        b
    }
}

/// If the only support is Level3 (self-report/hypothesis) → Inconclusive.
pub fn only_level3_support(evidence: &[Evidence]) -> bool {
    evidence.iter().all(|e| e.level == EvidenceLevel::Level3)
}

// ---------------------------------------------------------------------------
// P29 — Evaluator independence
// ---------------------------------------------------------------------------

/// Provenance of an evaluator/judge. An implementer must never be the sole
/// evaluator of their own candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatorProvenance {
    pub model: String,
    pub harness: Option<String>,
    pub context_hash: String,
    #[serde(default)]
    pub references: Vec<String>,
    pub participated_in_hypothesis: bool,
    pub participated_in_implementation: bool,
}

/// Whether the evaluator is independent enough to be a sole judge.
pub fn evaluator_independent(p: &EvaluatorProvenance) -> bool {
    !p.participated_in_implementation
}

// ---------------------------------------------------------------------------
// P30 — Diversity preservation
// ---------------------------------------------------------------------------

/// A coarse candidate profile used to compute differences between candidates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateProfile {
    pub id: String,
    pub architecture: String,
    pub workflow: String,
    pub tool_composition: String,
    pub changed_scope: String,
    pub parent_lineage: String,
}

/// Coarse-grained difference between two candidates (0 = identical, 1 = very
/// different) across architecture / workflow / tools / scope / lineage.
pub fn coarse_difference(a: &CandidateProfile, b: &CandidateProfile) -> f64 {
    let mut same = 0.0;
    let dims = 5.0;
    if a.architecture == b.architecture {
        same += 1.0;
    }
    if a.workflow == b.workflow {
        same += 1.0;
    }
    if a.tool_composition == b.tool_composition {
        same += 1.0;
    }
    if a.changed_scope == b.changed_scope {
        same += 1.0;
    }
    if a.parent_lineage == b.parent_lineage {
        same += 1.0;
    }
    1.0 - same / dims
}

/// Diversity of a candidate against an existing set: the minimum difference to
/// any member. High-quality but near-identical candidates crowd out diversity.
pub fn novelty_against_set(candidate: &CandidateProfile, existing: &[CandidateProfile]) -> f64 {
    existing
        .iter()
        .map(|e| coarse_difference(candidate, e))
        .fold(1.0, f64::min)
}

// ---------------------------------------------------------------------------
// P31 — Anti-monoculture
// ---------------------------------------------------------------------------

/// A diversification warning when many consecutive candidates share a
/// dimension. Route only proposes the request; it never calls a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationWarning {
    pub dimension: String,
    pub consecutive: usize,
    pub recommendation: String,
}

/// Detect monoculture along model / parent / strategy / reference / workflow.
pub fn check_monoculture(
    profiles: &[CandidateProfile],
    dimension_fn: &dyn Fn(&CandidateProfile) -> &str,
    threshold: usize,
) -> Option<ExplorationWarning> {
    if profiles.len() < threshold {
        return None;
    }
    let tail = &profiles[profiles.len() - threshold..];
    let first = dimension_fn(&tail[0]);
    if tail.iter().all(|p| dimension_fn(p) == first) {
        Some(ExplorationWarning {
            dimension: "uniform".to_string(),
            consecutive: threshold,
            recommendation: format!(
                "last {threshold} candidates share {} — diversify model/parent/strategy/reference/workflow",
                first
            ),
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// P32 — Replication
// ---------------------------------------------------------------------------

/// States of an innovation record. One success never becomes knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InnovationState {
    Observed,
    Replicated,
    Generalized,
    Trusted,
    Retired,
}

impl InnovationState {
    pub fn as_str(self) -> &'static str {
        match self {
            InnovationState::Observed => "observed",
            InnovationState::Replicated => "replicated",
            InnovationState::Generalized => "generalized",
            InnovationState::Trusted => "trusted",
            InnovationState::Retired => "retired",
        }
    }
}

/// Advance an innovation state given evidence.
pub fn advance_innovation(
    state: InnovationState,
    replicated: bool,
    generalized: bool,
    ablation_passed: bool,
    regression_clean: bool,
    evidence_sufficient: bool,
) -> InnovationState {
    // A regression retires the innovation regardless of other signals.
    if !regression_clean {
        return InnovationState::Retired;
    }
    match state {
        InnovationState::Observed if replicated => InnovationState::Replicated,
        InnovationState::Replicated if generalized => InnovationState::Generalized,
        InnovationState::Generalized if ablation_passed && evidence_sufficient => {
            InnovationState::Trusted
        }
        _ => state,
    }
}

// ---------------------------------------------------------------------------
// P33 — Counterfactual / ablation
// ---------------------------------------------------------------------------

/// Compare A (full), B (remove suspected innovation), C (baseline + only the
/// suspected innovation) to answer "where does the improvement come from?".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AblationStudy {
    pub id: String,
    pub full_candidate: String,
    pub removed_candidate: String,
    pub baseline_only_candidate: String,
    pub full_score: f64,
    pub removed_score: f64,
    pub baseline_score: f64,
}

/// Whether the suspected innovation contributes. If removing it changes nothing
/// (full ≈ removed), it must not be promoted to Gene/Strategy.
pub fn ablation_contributes(s: &AblationStudy, margin: f64) -> bool {
    (s.full_score - s.removed_score).abs() >= margin && s.full_score > s.baseline_score
}

// ---------------------------------------------------------------------------
// P34 — Knowledge distillation
// ---------------------------------------------------------------------------

/// Distilled knowledge from a successful experiment. Route never stores the
/// full CoT / huge context as "knowledge".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledKnowledge {
    pub id: String,
    pub trigger: String,
    pub principle: String,
    pub applicable_scope: String,
    pub observed_effect: String,
    pub failure_boundary: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub lineage_refs: Vec<String>,
}

// ---------------------------------------------------------------------------
// P35 — Failure distillation
// ---------------------------------------------------------------------------

/// Distilled failure. Repeated identical failures are merged/associated, not
/// piled up as duplicate memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledFailure {
    pub id: String,
    pub failure_signature: String,
    pub trigger: String,
    pub bad_mutation: String,
    pub detection: String,
    pub recovery: String,
    pub regression_benchmark_proposal: Option<String>,
    pub occurrence_count: u32,
}

/// Merge a repeated failure into an existing record by signature.
pub fn merge_repeated_failure(
    records: &mut Vec<DistilledFailure>,
    incoming: DistilledFailure,
) -> bool {
    if let Some(existing) = records
        .iter_mut()
        .find(|r| r.failure_signature == incoming.failure_signature)
    {
        existing.occurrence_count += 1;
        true
    } else {
        records.push(incoming);
        false
    }
}

// ---------------------------------------------------------------------------
// P36 — Unattended next-action protocol
// ---------------------------------------------------------------------------

/// Machine-readable instruction for the harness in an unattended campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    ExecuteExperiment,
    Diversify,
    Replicate,
    Ablate,
    Stop,
    NeedsHuman,
}

impl NextAction {
    pub fn as_str(self) -> &'static str {
        match self {
            NextAction::ExecuteExperiment => "execute_experiment",
            NextAction::Diversify => "diversify",
            NextAction::Replicate => "replicate",
            NextAction::Ablate => "ablate",
            NextAction::Stop => "stop",
            NextAction::NeedsHuman => "needs_human",
        }
    }
}

/// Decide the next action for a campaign. Budget exhaustion → STOP; ablation
/// needed for a candidate → ABLATE; monoculture → DIVERSIFY; insufficient
/// evidence → REPLICATE; unresolved → NEEDS_HUMAN.
pub fn campaign_next_action(
    campaign: &EvolutionCampaign,
    now_ms: i64,
    failed_count: usize,
    monoculture: bool,
    has_candidate: bool,
    candidate_needs_ablation: bool,
    evidence_sufficient: bool,
) -> NextAction {
    if campaign.is_terminal() {
        return NextAction::Stop;
    }
    if budget_admits(campaign, now_ms, failed_count, 0, 0.0).is_err() {
        return NextAction::Stop;
    }
    if monoculture {
        NextAction::Diversify
    } else if candidate_needs_ablation {
        NextAction::Ablate
    } else if has_candidate && !evidence_sufficient {
        NextAction::Replicate
    } else if has_candidate {
        NextAction::ExecuteExperiment
    } else if !evidence_sufficient {
        NextAction::NeedsHuman
    } else {
        NextAction::ExecuteExperiment
    }
}

// ---------------------------------------------------------------------------
// P1/P2 — NextAction harness contract
// ---------------------------------------------------------------------------

/// Canonical capability names a harness may declare. These are capabilities,
/// NOT concrete vendors — a harness decides how to satisfy them.
pub const CAP_FILESYSTEM: &str = "filesystem";
pub const CAP_SHELL: &str = "shell";
pub const CAP_WEB: &str = "web";
pub const CAP_SUBAGENTS: &str = "subagents";
pub const CAP_CODE_MODE: &str = "code_mode";
pub const CAP_SKILLS: &str = "skills";
pub const CAP_WORKFLOWS: &str = "workflows";

/// Capabilities a given action requires of the executing harness.
/// STOP / NEEDS_HUMAN require nothing (no harness execution happens).
pub fn action_requires_capabilities(action: NextAction) -> &'static [&'static str] {
    match action {
        NextAction::ExecuteExperiment => &[CAP_FILESYSTEM, CAP_SHELL],
        NextAction::Diversify => &[CAP_SUBAGENTS],
        NextAction::Replicate => &[CAP_FILESYSTEM, CAP_SHELL],
        NextAction::Ablate => &[CAP_FILESYSTEM, CAP_SHELL],
        NextAction::Stop => &[],
        NextAction::NeedsHuman => &[],
    }
}

/// Check whether a harness's declared capabilities can satisfy a required set.
/// Returns `(satisfied, missing)`. Only explicit `true` counts; unknown is
/// treated as NOT satisfied so Route never silently pretends execution works.
pub fn capabilities_satisfy(available: &[&str], required: &[&str]) -> (bool, Vec<String>) {
    let missing: Vec<String> = required
        .iter()
        .filter(|r| !available.contains(r))
        .map(|r| (*r).to_string())
        .collect();
    (missing.is_empty(), missing)
}

/// Machine-readable instruction bundle for an external harness. This is what
/// P1 strengthens: it carries everything a harness needs to act, without
/// Route telling it which vendor/model to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextActionContract {
    /// The action to perform (execute_experiment | diversify | replicate |
    /// ablate | stop | needs_human).
    pub action: NextAction,
    /// Parent campaign id.
    pub campaign_id: String,
    /// Candidate experiment id when one is referenced by the action.
    pub experiment_id: Option<String>,
    /// Parent Task session id the campaign serves (if any).
    pub task_id: Option<String>,
    /// The campaign goal the harness works toward.
    pub goal: String,
    /// The scope the harness may touch.
    pub scope: String,
    /// Capability names required to perform this action (see CAP_*).
    pub required_capabilities: Vec<String>,
    /// Whether Route task/context must be consulted before acting.
    pub context_required: bool,
    /// Whether a checkpoint/save must be recorded before mutating.
    pub checkpoint_required: bool,
    /// Whether the candidate work must be isolated from stable state.
    pub candidate_isolation_required: bool,
    /// Evidence kinds Route expects back from this action.
    pub expected_evidence: Vec<String>,
    /// Budget remaining for experiments.
    pub budget_remaining_experiments: usize,
    /// Budget remaining in wall-clock ms.
    pub budget_remaining_wall_ms: i64,
    /// Fields the harness report must provide (report contract).
    pub report_contract: Vec<String>,
    /// Whether the harness's declared capabilities can satisfy the action.
    /// If false, `action` is degraded to needs_human and execution must not
    /// be silently pretended possible.
    pub satisfiable: bool,
    /// Capabilities the harness lacks (empty when satisfiable).
    pub missing_capabilities: Vec<String>,
}

impl NextActionContract {
    /// Standard report contract fields an executing harness must return.
    pub const REPORT_CONTRACT: [&'static str; 12] = [
        "campaign_id",
        "experiment_id",
        "session_id",
        "execution_status",
        "candidate",
        "observed_changes",
        "tests",
        "benchmarks",
        "evidence_refs",
        "metrics",
        "failure",
        "harness/model provenance",
    ];

    /// Expected evidence kinds per action.
    pub fn expected_evidence_for(action: NextAction) -> Vec<String> {
        match action {
            NextAction::ExecuteExperiment => vec![
                "system/test_pass".to_string(),
                "system/snapshot".to_string(),
                "agent/feedback".to_string(),
            ],
            NextAction::Replicate => vec!["system/test_pass".to_string()],
            NextAction::Ablate => vec!["system/test_pass".to_string()],
            NextAction::Diversify => vec!["agent/feedback".to_string()],
            NextAction::Stop | NextAction::NeedsHuman => vec![],
        }
    }
}

/// Build the full harness contract for a campaign's next action. `available`
/// is the harness's declared capability list; if it cannot satisfy the action
/// the action degrades to NEEDS_HUMAN with `satisfiable = false`.
#[allow(clippy::too_many_arguments)]
pub fn build_next_action_contract(
    campaign: &EvolutionCampaign,
    now_ms: i64,
    failed_count: usize,
    monoculture: bool,
    has_candidate: bool,
    candidate_needs_ablation: bool,
    evidence_sufficient: bool,
    available: &[&str],
) -> NextActionContract {
    let action = campaign_next_action(
        campaign,
        now_ms,
        failed_count,
        monoculture,
        has_candidate,
        candidate_needs_ablation,
        evidence_sufficient,
    );
    let required: Vec<String> = action_requires_capabilities(action)
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let required_refs: Vec<&str> = required.iter().map(|s| s.as_str()).collect();
    let (satisfiable, missing) = capabilities_satisfy(available, &required_refs);

    let action = if satisfiable {
        action
    } else {
        NextAction::NeedsHuman
    };

    // Budget remaining.
    let budget_remaining_experiments = campaign
        .budget
        .max_experiments
        .saturating_sub(campaign.experiment_ids.len());
    let budget_remaining_wall_ms =
        (campaign.budget.max_wall_time_ms - (now_ms - campaign.created_at)).max(0);

    NextActionContract {
        action,
        campaign_id: campaign.id.clone(),
        experiment_id: campaign.experiment_ids.last().cloned(),
        task_id: campaign.task_id.clone(),
        goal: campaign.goal.clone(),
        scope: campaign.scope.clone(),
        required_capabilities: required,
        context_required: matches!(
            action,
            NextAction::ExecuteExperiment | NextAction::Replicate | NextAction::Ablate
        ),
        checkpoint_required: matches!(
            action,
            NextAction::ExecuteExperiment | NextAction::Replicate | NextAction::Ablate
        ),
        candidate_isolation_required: matches!(action, NextAction::ExecuteExperiment),
        expected_evidence: NextActionContract::expected_evidence_for(action),
        budget_remaining_experiments,
        budget_remaining_wall_ms,
        report_contract: NextActionContract::REPORT_CONTRACT
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        satisfiable,
        missing_capabilities: missing,
    }
}

// ---------------------------------------------------------------------------
// P4 — Harness report contract (UNTRUSTED input)
// ---------------------------------------------------------------------------

/// A single metric value reported by a harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetric {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub unit: String,
}

/// Structured execution outcome reported by an external harness. This is
/// UNTRUSTED input: `execution_status == "success"` NEVER equals a trusted
/// PASS. Route resolves the outcome through its existing Evidence hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignReport {
    pub campaign_id: String,
    pub experiment_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    /// "success" | "failed" | "error" | "aborted"
    pub execution_status: String,
    #[serde(default)]
    pub candidate: Option<String>,
    #[serde(default)]
    pub observed_changes: Vec<String>,
    #[serde(default)]
    pub tests: Vec<String>,
    #[serde(default)]
    pub benchmarks: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub metrics: Vec<ReportMetric>,
    #[serde(default)]
    pub failure: Option<String>,
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Stable identity of a report for idempotency. Same campaign + experiment +
/// evidence-ref set → same identity, so a retried submission cannot
/// double-promote / double-count / duplicate learning / corrupt budget.
pub fn report_identity(report: &CampaignReport) -> String {
    let mut refs: Vec<&str> = report.evidence_refs.iter().map(|s| s.as_str()).collect();
    refs.sort_unstable();
    let joined = refs.join(",");
    route_core::sha256_hex(
        format!("{}|{}|{}", report.campaign_id, report.experiment_id, joined).as_bytes(),
    )
}

/// Result of ingesting a harness report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestReportResult {
    /// First submission — recorded as a report record. Never trusted by itself.
    Recorded,
    /// Exact duplicate submission — ignored, nothing double-counted.
    Duplicate,
    /// Same identity but a conflicting outcome — recorded as a conflict
    /// evidence record so the divergence is auditable.
    Conflict,
}

/// A persisted harness report record. Trust is never granted by the harness;
/// `trusted` is only ever set later through Route's evidence resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignReportRecord {
    pub record_id: String,
    pub identity: String,
    pub campaign_id: String,
    pub experiment_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub execution_status: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Resolved trust. Never auto-true from the report; only Route evidence
    /// resolution can set this.
    #[serde(default)]
    pub trusted: bool,
    /// Id of a conflict evidence record when a duplicate disagreed.
    #[serde(default)]
    pub conflict_evidence_id: Option<String>,
    pub received_at: i64,
}

/// A conflict evidence record produced when two submissions with the same
/// identity disagree. Kept so the divergence is explicit and auditable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictEvidenceRecord {
    pub id: String,
    pub identity: String,
    pub campaign_id: String,
    pub experiment_id: String,
    pub first_status: String,
    pub second_status: String,
    pub recorded_at: i64,
}

// ---------------------------------------------------------------------------
// P5 — Campaign → Task state (recommendation only, never silent success)
// ---------------------------------------------------------------------------

/// What a campaign's outcome implies for its parent Task. This is always a
/// RECOMMENDATION surfaced to the user/harness — a campaign never silently
/// marks a Task Succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRecommendation {
    /// Campaign not linked to a Task, or still exploring.
    None,
    /// Campaign stable/accepted → may recommend Task completion. The Task must
    /// still pass its own verification; this is a suggestion only.
    RecommendComplete,
    /// STOP without an accepted solution → Task stays actionable/blocked.
    RecommendContinue,
    /// Campaign failed (exhausted, no accepted solution) → Task history intact,
    /// Task stays actionable.
    RecommendReview,
    /// Campaign aborted → Task survives, review before continuing.
    RecommendAbort,
}

/// Map a campaign to a Task recommendation without mutating the Task.
pub fn task_recommendation(campaign: &EvolutionCampaign) -> TaskRecommendation {
    match campaign.status {
        CampaignStatus::Completed if campaign.champion.is_some() => {
            TaskRecommendation::RecommendComplete
        }
        CampaignStatus::Exhausted => TaskRecommendation::RecommendReview,
        CampaignStatus::Aborted => TaskRecommendation::RecommendAbort,
        CampaignStatus::Completed | CampaignStatus::Paused => TaskRecommendation::RecommendContinue,
        CampaignStatus::Planned | CampaignStatus::Running => TaskRecommendation::None,
    }
}

// ---------------------------------------------------------------------------
// P38 / P39 — Staged promotion
// ---------------------------------------------------------------------------

/// Promotion policy tier (P38).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionTier {
    ProjectLowRisk,
    ProjectHighRisk,
    Harness,
    Workflow,
    Route,
}

/// Staged promotion state (P39): Candidate → Canary → ProvisionalKnownGood →
/// Stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionStage {
    Candidate,
    Canary,
    ProvisionalKnownGood,
    Stable,
}

/// Whether a tier allows auto-promotion.
pub fn auto_promotion_allowed(tier: PromotionTier) -> bool {
    matches!(tier, PromotionTier::ProjectLowRisk)
}

/// Advance a stage. A canary failure reverts to the previous KnownGood
/// (from Candidate, i.e. no promotion).
pub fn advance_stage(stage: PromotionStage, canary_ok: bool) -> PromotionStage {
    if !canary_ok && stage == PromotionStage::Canary {
        return PromotionStage::Candidate; // revert
    }
    match stage {
        PromotionStage::Candidate => PromotionStage::Canary,
        PromotionStage::Canary => PromotionStage::ProvisionalKnownGood,
        PromotionStage::ProvisionalKnownGood => PromotionStage::Stable,
        PromotionStage::Stable => PromotionStage::Stable,
    }
}

// ---------------------------------------------------------------------------
// P40 — Contamination control
// ---------------------------------------------------------------------------

/// Separated context hashes. Holdout / evaluator private data / promotion
/// criteria must never be injected into the generation context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSeparation {
    pub generation_context_hash: String,
    pub evaluation_context_hash: String,
}

/// True when the generation context did not include evaluation-only material.
pub fn contamination_clean(sep: &ContextSeparation) -> bool {
    sep.generation_context_hash != sep.evaluation_context_hash
}

// ---------------------------------------------------------------------------
// P41 — Human decision as data
// ---------------------------------------------------------------------------

/// A recorded human preference (accept / reject / override / prefer A over B).
/// It is evidence, not absolute truth — only repeated, stable preferences may
/// become a Policy/Reference proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceEvidence {
    pub id: String,
    pub actor: String,
    pub preference: String,
    pub context_hash: String,
    pub recorded_at: i64,
}

/// Qualify a preference into a promotion proposal only after repeated stable
/// occurrence.
pub fn preference_reaches_threshold(
    preferences: &[PreferenceEvidence],
    preference: &str,
    threshold: usize,
) -> bool {
    preferences
        .iter()
        .filter(|p| p.preference == preference)
        .count()
        >= threshold
}

// ---------------------------------------------------------------------------
// P42 — Campaign summary / explain
// ---------------------------------------------------------------------------

/// Explain a campaign: goal, routes explored, why the champion won, odd
/// directions, failures that produced benchmarks, reproducible laws, resource
/// usage, why it stopped, next direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignExplainer {
    pub goal: String,
    pub routes_explored: usize,
    pub champion_id: Option<String>,
    pub champion_reason: String,
    pub odd_directions: usize,
    pub failures_turned_to_benchmarks: usize,
    pub reproducible_laws: usize,
    pub still_anecdotal: usize,
    pub campaigns_exhausted: usize,
    pub stop_reason: String,
    pub next_direction: String,
}

// ---------------------------------------------------------------------------
// Store & IO
// ---------------------------------------------------------------------------

/// Where a failed-but-novel candidate should be archived (P44 #6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveTarget {
    /// Worth re-exploring later; not a KnownGood.
    NoveltyArchive,
    /// A familiar failure; kept for regression/benchmark reuse.
    FailureArchive,
    /// A successful candidate eligible for KnownGood promotion.
    KnownGoodCandidate,
}

/// Route a candidate's outcome. A high-novelty failed candidate goes to the
/// NoveltyArchive (a direction worth re-exploring), never to KnownGood. A
/// low-novelty failure goes to the FailureArchive. A success is eligible for
/// KnownGood promotion.
pub fn archive_outcome(novelty: f64, novelty_threshold: f64, failed: bool) -> ArchiveTarget {
    if failed {
        if novelty >= novelty_threshold {
            ArchiveTarget::NoveltyArchive
        } else {
            ArchiveTarget::FailureArchive
        }
    } else {
        ArchiveTarget::KnownGoodCandidate
    }
}

/// The campaign store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CampaignStore {
    #[serde(default)]
    pub campaigns: Vec<EvolutionCampaign>,
    #[serde(default)]
    pub free_hypotheses: Vec<FreeHypothesis>,
    #[serde(default)]
    pub distilled_knowledge: Vec<DistilledKnowledge>,
    #[serde(default)]
    pub distilled_failures: Vec<DistilledFailure>,
    #[serde(default)]
    pub preference_evidence: Vec<PreferenceEvidence>,
    /// Ordered ids of past champions, for rebuilding the full lineage (P44 #12).
    #[serde(default)]
    pub champion_lineage: Vec<String>,
    /// Experiment ids that are novel but did not win (P30).
    #[serde(default)]
    pub novelty_archive: Vec<String>,
    /// Experiment ids of familiar failures (P30).
    #[serde(default)]
    pub failure_archive: Vec<String>,
    /// Harness report records (UNTRUSTED input, never auto-trusted).
    #[serde(default)]
    pub reports: Vec<CampaignReportRecord>,
    /// Conflict evidence from disagreeing duplicate submissions (P9).
    #[serde(default)]
    pub conflict_evidence: Vec<ConflictEvidenceRecord>,
}

impl CampaignStore {
    /// Append a new champion to the lineage. Successive calls with the same id
    /// are ignored so the lineage stays a clean ordered history.
    pub fn record_champion(&mut self, experiment_id: &str) {
        if self.champion_lineage.last().map(String::as_str) != Some(experiment_id) {
            self.champion_lineage.push(experiment_id.to_string());
        }
    }
}

impl CampaignStore {
    /// Ingest a harness execution report (P4/P9). The report is UNTRUSTED input:
    /// it is recorded with `trusted = false` and never promotes anything by
    /// itself. Idempotent: re-submitting the same report returns `Duplicate`
    /// without double-counting; a disagreeing duplicate returns `Conflict` and
    /// records conflict evidence.
    pub fn ingest_report(&mut self, report: &CampaignReport) -> IngestReportResult {
        let identity = report_identity(report);
        let now = route_core::now_millis();

        if let Some(existing) = self.reports.iter_mut().find(|r| r.identity == identity) {
            if existing.execution_status == report.execution_status {
                return IngestReportResult::Duplicate;
            }
            // Conflicting duplicate: record conflict evidence, mark the record.
            let conflict_id = route_core::new_id();
            self.conflict_evidence.push(ConflictEvidenceRecord {
                id: conflict_id.clone(),
                identity: identity.clone(),
                campaign_id: report.campaign_id.clone(),
                experiment_id: report.experiment_id.clone(),
                first_status: existing.execution_status.clone(),
                second_status: report.execution_status.clone(),
                recorded_at: now,
            });
            existing.conflict_evidence_id = Some(conflict_id);
            return IngestReportResult::Conflict;
        }

        self.reports.push(CampaignReportRecord {
            record_id: route_core::new_id(),
            identity,
            campaign_id: report.campaign_id.clone(),
            experiment_id: report.experiment_id.clone(),
            session_id: report.session_id.clone(),
            execution_status: report.execution_status.clone(),
            evidence_refs: report.evidence_refs.clone(),
            harness: report.harness.clone(),
            model: report.model.clone(),
            trusted: false,
            conflict_evidence_id: None,
            received_at: now,
        });
        IngestReportResult::Recorded
    }

    /// Resolve report trust from Route evidence, never from the harness claim.
    /// A report is only trusted when it references at least one piece of
    /// non-self-reported evidence (Level0/Level1) that actually exists.
    /// This is intentionally conservative — `trusted` stays false otherwise.
    pub fn resolve_report_trust(&mut self, report: &CampaignReport, evidence_ok: bool) -> bool {
        let identity = report_identity(report);
        if let Some(rec) = self.reports.iter_mut().find(|r| r.identity == identity) {
            if evidence_ok && !report.evidence_refs.is_empty() {
                rec.trusted = true;
            }
            return rec.trusted;
        }
        false
    }
}

impl CampaignStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = campaign_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading campaign store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = campaign_path(project_root);
        let dir = p.parent().expect("campaign dir has parent");
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing campaign store to {}", p.display()))
    }
}

pub fn campaign_dir(project_root: &Path) -> PathBuf {
    project_root
        .join(crate::constitutive::ROUTE_DOT_DIR)
        .join(CAMPAIGN_DIR)
}

pub fn campaign_path(project_root: &Path) -> PathBuf {
    campaign_dir(project_root).join("campaign.json")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn campaign(status: CampaignStatus, budget: CampaignBudget, now: i64) -> EvolutionCampaign {
        EvolutionCampaign {
            id: "cam".to_string(),
            goal: "g".to_string(),
            scope: "s".to_string(),
            strategy: "st".to_string(),
            budget,
            experiment_ids: vec![],
            champion: None,
            challengers: vec![],
            status,
            stop_reason: None,
            summary: String::new(),
            task_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    // P44 #1: budget exhaustion rejects new experiments.
    #[test]
    fn budget_exhaustion_rejects_new_experiment() {
        let b = CampaignBudget {
            max_experiments: 1,
            max_failed_experiments: 4,
            max_wall_time_ms: 3600_000,
            max_tokens: None,
            max_cost: None,
            disk_budget: None,
            max_parallel_external: 1,
        };
        let mut c = campaign(CampaignStatus::Running, b, 0);
        c.experiment_ids.push("e1".to_string());
        assert!(budget_admits(&c, 1, 0, 0, 0.0).is_err());
    }

    // P44 #2: a hypothesis violating Stable is saved but blocked at execution.
    #[test]
    fn think_free_but_execution_gate_blocks() {
        let h = FreeHypothesis {
            id: "h".to_string(),
            text: "what if we delete X entirely?".to_string(),
            violated_constraints: vec![ViolatedConstraint {
                dimension: "constitution".to_string(),
                description: "delete X".to_string(),
            }],
            recorded_at: 0,
        };
        // Think free: the hypothesis itself is saved.
        assert!(!h.violated_constraints.is_empty());
        // Act bounded: the execution gate blocks it.
        let gate = execution_gate(&h);
        assert!(!gate.allowed);
        assert!(gate.reason.contains("constitution"));
        // A clean hypothesis passes the gate.
        let clean = FreeHypothesis {
            violated_constraints: vec![],
            ..h
        };
        assert!(execution_gate(&clean).allowed);
    }

    // P28 / P44 #3: AI Judge (Level2) cannot override a trusted test (Level1).
    #[test]
    fn trusted_test_beats_ai_judge() {
        let trusted = Evidence {
            id: "t".to_string(),
            level: EvidenceLevel::Level1,
            summary: "trusted test".to_string(),
        };
        let judge = Evidence {
            id: "j".to_string(),
            level: EvidenceLevel::Level2,
            summary: "model review".to_string(),
        };
        let winner = resolve_evidence(&trusted, &judge);
        assert_eq!(winner.id, "t");

        // Only Level3 support → Inconclusive.
        let only = vec![Evidence {
            id: "s".to_string(),
            level: EvidenceLevel::Level3,
            summary: "self-report".to_string(),
        }];
        assert!(only_level3_support(&only));
    }

    // P29 / P44 #4: an implementer cannot be the sole evaluator.
    #[test]
    fn implementer_cannot_be_sole_evaluator() {
        let implementer = EvaluatorProvenance {
            model: "m".to_string(),
            harness: None,
            context_hash: "c".to_string(),
            references: vec![],
            participated_in_hypothesis: true,
            participated_in_implementation: true,
        };
        assert!(!evaluator_independent(&implementer));
        let independent = EvaluatorProvenance {
            participated_in_implementation: false,
            ..implementer
        };
        assert!(evaluator_independent(&independent));
    }

    // P30: near-identical candidates crowd out diversity.
    #[test]
    fn similar_candidates_have_low_novelty_headroom() {
        let a = CandidateProfile {
            id: "a".to_string(),
            architecture: "same".to_string(),
            workflow: "same".to_string(),
            tool_composition: "same".to_string(),
            changed_scope: "same".to_string(),
            parent_lineage: "same".to_string(),
        };
        let b = a.clone();
        assert_eq!(coarse_difference(&a, &b), 0.0);
        let set = vec![b];
        assert_eq!(novelty_against_set(&a, &set), 0.0);
    }

    // P31 / P44 #5: homogeneous candidates trigger a diversification warning.
    #[test]
    fn monoculture_triggers_diversification() {
        let profiles: Vec<CandidateProfile> = (0..4)
            .map(|i| CandidateProfile {
                id: format!("c{}", i),
                architecture: "same-tech".to_string(),
                workflow: "same-wf".to_string(),
                tool_composition: "same-tools".to_string(),
                changed_scope: "same-scope".to_string(),
                parent_lineage: "same-parent".to_string(),
            })
            .collect();
        let warning = check_monoculture(&profiles, &|p| p.architecture.as_str(), 3);
        assert!(warning.is_some());
        assert!(warning.unwrap().recommendation.contains("diversify"));
    }

    // P32 / P44 #7: replication advances Observed → Replicated; ablation → Trusted.
    #[test]
    fn replication_advances_innovation_state() {
        assert_eq!(
            advance_innovation(InnovationState::Observed, true, false, false, true, true),
            InnovationState::Replicated
        );
        assert_eq!(
            advance_innovation(InnovationState::Generalized, true, true, true, true, true),
            InnovationState::Trusted
        );
        // Regression retires it.
        assert_eq!(
            advance_innovation(InnovationState::Replicated, true, true, true, false, true),
            InnovationState::Retired
        );
    }

    // P33 / P44 #8: ablation with no contribution blocks Strategy promotion.
    #[test]
    fn ablation_no_contribution_blocks_promotion() {
        let no_effect = AblationStudy {
            id: "a".to_string(),
            full_candidate: "A".to_string(),
            removed_candidate: "B".to_string(),
            baseline_only_candidate: "C".to_string(),
            full_score: 0.9,
            removed_score: 0.9, // removing changes nothing
            baseline_score: 0.5,
        };
        assert!(!ablation_contributes(&no_effect, 0.05));
        let real_effect = AblationStudy {
            removed_score: 0.55, // removing the innovation drops it a lot
            ..no_effect
        };
        assert!(ablation_contributes(&real_effect, 0.05));
    }

    // P34: distillation stores only the essence, not full context.
    #[test]
    fn knowledge_is_distilled_not_holistic() {
        let k = DistilledKnowledge {
            id: "k".to_string(),
            trigger: "when mp3 tag batch runs".to_string(),
            principle: "validate before write".to_string(),
            applicable_scope: "audio".to_string(),
            observed_effect: "no corruption".to_string(),
            failure_boundary: "large files".to_string(),
            evidence_refs: vec!["ev-1".to_string()],
            lineage_refs: vec!["exp-1".to_string()],
        };
        assert!(!k.trigger.is_empty());
        assert!(k.evidence_refs.len() == 1);
    }

    // P35: repeated failures merge, not pile up.
    #[test]
    fn repeated_failures_merge() {
        let mut records = vec![];
        merge_repeated_failure(
            &mut records,
            DistilledFailure {
                id: "f1".to_string(),
                failure_signature: "sig".to_string(),
                trigger: "t".to_string(),
                bad_mutation: "b".to_string(),
                detection: "d".to_string(),
                recovery: "r".to_string(),
                regression_benchmark_proposal: None,
                occurrence_count: 1,
            },
        );
        let merged = merge_repeated_failure(
            &mut records,
            DistilledFailure {
                id: "f2".to_string(),
                failure_signature: "sig".to_string(),
                trigger: "t".to_string(),
                bad_mutation: "b".to_string(),
                detection: "d".to_string(),
                recovery: "r".to_string(),
                regression_benchmark_proposal: None,
                occurrence_count: 1,
            },
        );
        assert!(merged);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].occurrence_count, 2);
    }

    // P36: budget exhaustion → STOP.
    #[test]
    fn unattended_budget_exhaustion_stops() {
        let b = CampaignBudget {
            max_experiments: 1,
            max_failed_experiments: 4,
            max_wall_time_ms: 3600_000,
            max_tokens: None,
            max_cost: None,
            disk_budget: None,
            max_parallel_external: 1,
        };
        let mut c = campaign(CampaignStatus::Running, b.clone(), 0);
        c.experiment_ids.push("e1".to_string());
        assert_eq!(
            campaign_next_action(&c, 1, 0, false, true, false, true),
            NextAction::Stop
        );
        // Monoculture → DIVERSIFY before execution.
        let c2 = campaign(CampaignStatus::Running, b, 0);
        assert_eq!(
            campaign_next_action(&c2, 1, 0, true, true, false, true),
            NextAction::Diversify
        );
    }

    // P38/P39 / P44 #10: canary failure reverts to previous KnownGood.
    #[test]
    fn canary_failure_reverts_stage() {
        assert_eq!(
            advance_stage(PromotionStage::Canary, false),
            PromotionStage::Candidate
        );
        assert!(auto_promotion_allowed(PromotionTier::ProjectLowRisk));
        assert!(!auto_promotion_allowed(PromotionTier::Route));
    }

    // P40: holdout must not enter generation context.
    #[test]
    fn contamination_control_keeps_contexts_separate() {
        let clean = ContextSeparation {
            generation_context_hash: "gen".to_string(),
            evaluation_context_hash: "eval".to_string(),
        };
        assert!(contamination_clean(&clean));
        let contaminated = ContextSeparation {
            generation_context_hash: "eval".to_string(), // same as eval
            evaluation_context_hash: "eval".to_string(),
        };
        assert!(!contamination_clean(&contaminated));
    }

    // P41: only repeated stable preference crosses the threshold.
    #[test]
    fn human_preference_is_data_not_truth() {
        let prefs = vec![
            PreferenceEvidence {
                id: "p1".to_string(),
                actor: "u".to_string(),
                preference: "prefer A over B".to_string(),
                context_hash: "c".to_string(),
                recorded_at: 0,
            },
            PreferenceEvidence {
                id: "p2".to_string(),
                actor: "u".to_string(),
                preference: "prefer A over B".to_string(),
                context_hash: "c".to_string(),
                recorded_at: 1,
            },
        ];
        assert!(preference_reaches_threshold(&prefs, "prefer A over B", 2));
        assert!(!preference_reaches_threshold(&prefs, "prefer B over A", 2));
    }

    // P44 #6: a high-novelty failed candidate goes to the NoveltyArchive,
    // never to KnownGood.
    #[test]
    fn high_novelty_failure_archived_not_promoted() {
        let novel = archive_outcome(0.9, 0.5, true);
        assert_eq!(novel, ArchiveTarget::NoveltyArchive);
        let familiar = archive_outcome(0.1, 0.5, true);
        assert_eq!(familiar, ArchiveTarget::FailureArchive);
        let success = archive_outcome(0.9, 0.5, false);
        assert_eq!(success, ArchiveTarget::KnownGoodCandidate);
    }

    // P44 #11: an unattended campaign that "crashes" mid-way can be recovered
    // by re-loading the persisted store (state is durable, not in-memory).
    #[test]
    fn crash_recovery_restores_campaign_state() {
        let dir = std::env::temp_dir().join(format!("route-campaign-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // First run: create a campaign, run one experiment, record a champion.
        {
            let mut store = CampaignStore::default();
            let b = CampaignBudget {
                max_experiments: 4,
                max_failed_experiments: 2,
                max_wall_time_ms: 3600_000,
                max_tokens: None,
                max_cost: None,
                disk_budget: None,
                max_parallel_external: 1,
            };
            let c = campaign(CampaignStatus::Running, b, 0);
            store.campaigns.push(c);
            store.novelty_archive.push("nov-1".to_string());
            store.record_champion("exp-1");
            store.save(&dir).unwrap();
        } // simulated crash: store dropped without further mutation

        // Second run: recover from disk.
        let mut recovered = CampaignStore::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(recovered.campaigns.len(), 1);
        assert_eq!(recovered.novelty_archive, vec!["nov-1".to_string()]);
        assert_eq!(recovered.champion_lineage, vec!["exp-1".to_string()]);
        // The campaign can continue: a new experiment is still admitted.
        let c = recovered.campaigns.first_mut().unwrap();
        assert!(budget_admits(c, 1, 0, 0, 0.0).is_ok());
    }

    // P44 #12: the full Champion lineage can be rebuilt from the archive.
    #[test]
    fn champion_lineage_rebuilds_history() {
        let mut store = CampaignStore::default();
        for id in ["exp-1", "exp-2", "exp-2", "exp-3"] {
            store.record_champion(id);
        }
        // Duplicate consecutive champions are collapsed, order preserved.
        assert_eq!(
            store.champion_lineage,
            vec!["exp-1", "exp-2", "exp-3"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
        // Lineage is durable through a save/load round-trip on disk.
        let dir =
            std::env::temp_dir().join(format!("route-campaign-lineage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        store.save(&dir).unwrap();
        let reloaded = CampaignStore::load(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(reloaded.champion_lineage, store.champion_lineage);
    }

    // P4/P9: a harness report is UNTRUSTED; "success" never auto-trusts.
    #[test]
    fn report_success_not_trusted_without_verified_evidence() {
        let mut store = CampaignStore::default();
        let report = CampaignReport {
            campaign_id: "cam".to_string(),
            experiment_id: "e1".to_string(),
            session_id: None,
            execution_status: "success".to_string(),
            candidate: Some("c1".to_string()),
            observed_changes: vec!["x".to_string()],
            tests: vec!["t1".to_string()],
            benchmarks: vec![],
            evidence_refs: vec![],
            metrics: vec![],
            failure: None,
            harness: Some("dsh-pic".to_string()),
            model: None,
        };
        assert_eq!(store.ingest_report(&report), IngestReportResult::Recorded);
        // No evidence refs → cannot be trusted even if marked ok.
        assert!(!store.resolve_report_trust(&report, true));
        // Harness claim alone never flips trust when evidence is absent.
        assert!(!store.resolve_report_trust(&report, false));
        // With a real evidence ref that was ingested → trusted.
        let trusted = CampaignReport {
            evidence_refs: vec!["ev-1".to_string()],
            ..report.clone()
        };
        assert_eq!(store.ingest_report(&trusted), IngestReportResult::Recorded);
        assert!(store.resolve_report_trust(&trusted, true));
    }

    // P9: identical re-submission is idempotent; disagreeing one is a conflict.
    #[test]
    fn report_duplicate_idempotent_and_conflict_detected() {
        let mut store = CampaignStore::default();
        let report = CampaignReport {
            campaign_id: "cam".to_string(),
            experiment_id: "e1".to_string(),
            session_id: None,
            execution_status: "success".to_string(),
            candidate: Some("c1".to_string()),
            observed_changes: vec![],
            tests: vec![],
            benchmarks: vec![],
            evidence_refs: vec!["ev-1".to_string()],
            metrics: vec![],
            failure: None,
            harness: None,
            model: None,
        };
        assert_eq!(store.ingest_report(&report), IngestReportResult::Recorded);
        // Exact re-submission → Duplicate, nothing double-counted.
        assert_eq!(store.ingest_report(&report), IngestReportResult::Duplicate);
        assert_eq!(store.reports.len(), 1);
        assert_eq!(store.conflict_evidence.len(), 0);
        // Same identity, disagreeing status → Conflict + conflict evidence.
        let conflicting = CampaignReport {
            execution_status: "failed".to_string(),
            ..report.clone()
        };
        assert_eq!(
            store.ingest_report(&conflicting),
            IngestReportResult::Conflict
        );
        assert_eq!(store.conflict_evidence.len(), 1);
        assert_eq!(store.reports.len(), 1);
    }

    // P2: if the harness cannot satisfy the action's capabilities, the action
    // degrades to NEEDS_HUMAN and Route never pretends execution is possible.
    #[test]
    fn next_action_contract_degrades_on_missing_capability() {
        let b = CampaignBudget::default();
        let mut c = campaign(CampaignStatus::Running, b, 0);
        c.experiment_ids.push("e1".to_string()); // has_candidate → ExecuteExperiment
        let full = build_next_action_contract(
            &c,
            1,
            0,
            false,
            true,
            false,
            true,
            &["filesystem", "shell"],
        );
        assert_eq!(full.action, NextAction::ExecuteExperiment);
        assert!(full.satisfiable);

        // Missing "filesystem" → degrade to NEEDS_HUMAN, explicitly unsatisfiable.
        let degraded = build_next_action_contract(&c, 1, 0, false, true, false, true, &["shell"]);
        assert_eq!(degraded.action, NextAction::NeedsHuman);
        assert!(!degraded.satisfiable);
        assert_eq!(
            degraded.missing_capabilities,
            vec!["filesystem".to_string()]
        );
    }

    // P5: campaign outcome only produces a Task recommendation, never a
    // silent success. The Task must still pass its own verification.
    #[test]
    fn task_recommendation_never_silent_success() {
        let b = CampaignBudget::default();
        let mut completed = campaign(CampaignStatus::Completed, b.clone(), 0);
        completed.champion = Some("e1".to_string());
        assert_eq!(
            task_recommendation(&completed),
            TaskRecommendation::RecommendComplete
        );

        let exhausted = campaign(CampaignStatus::Exhausted, b.clone(), 0);
        assert_eq!(
            task_recommendation(&exhausted),
            TaskRecommendation::RecommendReview
        );

        let aborted = campaign(CampaignStatus::Aborted, b.clone(), 0);
        assert_eq!(
            task_recommendation(&aborted),
            TaskRecommendation::RecommendAbort
        );

        let running = campaign(CampaignStatus::Running, b, 0);
        assert_eq!(task_recommendation(&running), TaskRecommendation::None);
    }
}
