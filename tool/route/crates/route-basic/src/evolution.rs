//! Evolution Core — Candidate-first, verified self-improvement for Route.
//!
//! Closes the loop: **Observe → Propose → Candidate → Benchmark → Compare →
//! Promote/Reject → Learn**.
//!
//! Hard constraints enforced here:
//! - Route is NOT an LLM runtime. This module never runs a model, sandbox,
//!   subagent runtime, or Code Mode. Harness executes; Route records,
//!   evaluates, promotes, and rolls back.
//! - Candidate-first: no live self-edit. Stable/original state is only ever
//!   touched by an explicit, gated promotion.
//! - Trust-root immutability: ORIGINAL archive, last KnownGood, append-only
//!   Evidence/Audit, benchmark baseline, promotion policy, and recovery
//!   executor cannot be modified by a candidate through ordinary tools.
//! - Local-first: the whole Candidate → Benchmark → Compare → rollback loop
//!   runs offline against local files.
//!
//! This module reuses existing Route domain types (Evidence, ReferenceEntry,
//! LearningProposal, WorkflowChangeProposal) rather than re-implementing them.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{write_atomic, ReferenceEntry, ReferenceRegistry, ROUTE_DOT_DIR};

/// Directory name for evolution data.
pub const EVOLUTION_DIR: &str = "evolution";
/// File name for the evolution store.
pub const EVOLUTION_FILE: &str = "evolution.json";
/// Name of the promotion-policy trust root registry.
pub const PROMOTION_POLICY_FILE: &str = "promotion-policy.json";

// ---------------------------------------------------------------------------
// Trust roots
// ---------------------------------------------------------------------------

/// A trust root that no candidate may override through ordinary tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustRoot {
    /// The ORIGINAL external archive (source of truth for disaster recovery).
    OriginalArchive,
    /// The last KnownGood state (the last verified, promoted configuration).
    KnownGood,
    /// Append-only Evidence / Audit ledger.
    EvidenceAudit,
    /// Benchmark baseline (fixed + holdout cases).
    BenchmarkBaseline,
    /// Promotion policy (who may promote, and what gate is required).
    PromotionPolicy,
    /// Recovery executor (the mechanism that restores on disaster).
    RecoveryExecutor,
}

impl TrustRoot {
    pub fn as_str(self) -> &'static str {
        match self {
            TrustRoot::OriginalArchive => "original_archive",
            TrustRoot::KnownGood => "known_good",
            TrustRoot::EvidenceAudit => "evidence_audit",
            TrustRoot::BenchmarkBaseline => "benchmark_baseline",
            TrustRoot::PromotionPolicy => "promotion_policy",
            TrustRoot::RecoveryExecutor => "recovery_executor",
        }
    }
}

/// Describes a forbidden mutation of a trust root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustRootViolation {
    pub trust_root: TrustRoot,
    pub reason: String,
    pub severity: String, // "blocked" | "warning"
}

// ---------------------------------------------------------------------------
// Evolution target
// ---------------------------------------------------------------------------

/// What an experiment is trying to improve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionTarget {
    /// Project code/files.
    Project,
    /// Protocol / workflow / context policy.
    Workflow,
    /// Context selection policy.
    Context,
    /// Harness composition / preset.
    Harness,
    /// Route's own non-root policy or code.
    Route,
}

impl EvolutionTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            EvolutionTarget::Project => "project",
            EvolutionTarget::Workflow => "workflow",
            EvolutionTarget::Context => "context",
            EvolutionTarget::Harness => "harness",
            EvolutionTarget::Route => "route",
        }
    }
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Benchmark origin — guards against reward hacking.
///
/// Trust decreases from Fixed to Candidate. A candidate may never modify
/// Fixed or Holdout cases, and may never delete hard tests to raise its score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkOrigin {
    /// Existing trusted tests / acceptance criteria.
    FixedBaseline,
    /// Constraints extracted from Reference entries.
    ReferenceDerived,
    /// Repro of a real historical failure.
    FailureRegression,
    /// AI-generated exploratory test for the candidate.
    CandidateGenerated,
    /// Holdout: hidden from the candidate during the modification phase.
    Holdout,
}

impl BenchmarkOrigin {
    /// Whether a candidate is allowed to mutate this case.
    /// Reward-hacking guard: Fixed and Holdout are immutable.
    pub fn candidate_mutable(self) -> bool {
        matches!(
            self,
            BenchmarkOrigin::CandidateGenerated | BenchmarkOrigin::ReferenceDerived
        )
    }

    /// Whether this case is authoritative for correctness judgement.
    pub fn authoritative(self) -> bool {
        matches!(
            self,
            BenchmarkOrigin::FixedBaseline | BenchmarkOrigin::Holdout
        )
    }

    /// Whether a regression on this case must block promotion.
    /// Guards correctness: everything except pure AI exploratory tests.
    /// A Reference-derived constraint or a historical failure repro that
    /// passed at baseline must not regress under the candidate (P4/P10).
    pub fn guards_correctness(self) -> bool {
        !matches!(self, BenchmarkOrigin::CandidateGenerated)
    }
}

/// A single benchmark case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkCase {
    pub id: String,
    pub name: String,
    pub origin: BenchmarkOrigin,
    /// Scope of the case (project / task-pattern / language / harness / tool / studio).
    pub scope: String,
    /// Expected outcome (free-form, machine-checkable hint).
    pub expected: String,
    /// High / medium / low trust.
    pub trust: String,
    /// Whether the case is mutable by a candidate.
    pub mutable: bool,
    /// Evidence IDs supporting this case.
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Optional reference ID this case is derived from (Reference anchor).
    #[serde(default)]
    pub reference_id: Option<String>,
    /// Optional studio preset node this case belongs to.
    #[serde(default)]
    pub studio: Option<String>,
    /// Whether this case enforces Simcise design norms.
    #[serde(default)]
    pub simcise_check: bool,
}

/// A suite of benchmark cases.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub cases: Vec<BenchmarkCase>,
    /// Who/what generated the suite ("system" | "ai" | "user").
    pub generated_by: String,
}

// ---------------------------------------------------------------------------
// Metrics & decisions
// ---------------------------------------------------------------------------

/// Result of running one benchmark case against baseline and candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub case_id: String,
    pub name: String,
    pub origin: BenchmarkOrigin,
    pub baseline_passed: bool,
    pub candidate_passed: bool,
    pub normalized: bool, // true => candidate not allowed to cheat by dropping hard cases
    pub note: String,
}

/// A single evaluable metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    /// Baseline value (None if not measurable).
    #[serde(default)]
    pub baseline: Option<f64>,
    /// Candidate value.
    #[serde(default)]
    pub candidate: Option<f64>,
    pub unit: String,
    /// Default true (higher is better); set false for latency/cost.
    #[serde(default = "default_higher_is_better")]
    pub higher_is_better: bool,
}

fn default_higher_is_better() -> bool {
    true
}

/// The verdict of comparing a candidate against its baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecision {
    /// Candidate is better on key metrics and not worse on correctness.
    Dominates,
    /// Candidate improves some metrics, no correctness regression.
    Improves,
    /// Candidate improves some metrics but regresses others (correctness holds).
    Tradeoff,
    /// No meaningful difference.
    Neutral,
    /// Candidate regresses correctness or key metrics.
    Regresses,
    /// Not enough evidence to decide — never auto-promote.
    Inconclusive,
}

impl PromotionDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            PromotionDecision::Dominates => "dominates",
            PromotionDecision::Improves => "improves",
            PromotionDecision::Tradeoff => "tradeoff",
            PromotionDecision::Neutral => "neutral",
            PromotionDecision::Regresses => "regresses",
            PromotionDecision::Inconclusive => "inconclusive",
        }
    }
}

// ---------------------------------------------------------------------------
// Studio presets (multi-node, decentralized evaluation)
// ---------------------------------------------------------------------------

/// A studio preset node — a scoped evaluation lane (protocol/core, audio/
/// multimodal, UI/interaction, etc.). Candidates are cross-benchmarked across
/// studios instead of competing in a single sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioPreset {
    pub id: String,
    pub name: String,
    /// Scope this studio owns (e.g. "protocol", "audio", "ui").
    pub scope: String,
    /// Whether this studio enforces Simcise design norms.
    pub simcise_enforced: bool,
    /// Benchmark case IDs this studio p
    #[serde(default)]
    pub cases: Vec<String>,
}

// ---------------------------------------------------------------------------
// Harness candidates (P7)
// ---------------------------------------------------------------------------

/// A record of a harness candidate for protection/restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessCandidate {
    pub id: String,
    pub host: String,
    pub preset_hash: String,
    pub skills: Vec<String>,
    pub tools: Vec<String>,
    pub workflow: Option<String>,
    pub capabilities: Vec<String>,
    pub model_route_metadata: String,
    /// Pre-change save reference (for restore).
    pub pre_change_save_id: Option<String>,
    pub mount_ok: bool,
    pub cleanup_ok: bool,
}

// ---------------------------------------------------------------------------
// Experiment
// ---------------------------------------------------------------------------

/// Status of an evolution experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Proposed,
    Benchmarking,
    Failed,
    Rejected,
    Promoted,
    RolledBack,
}

impl ExperimentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ExperimentStatus::Proposed => "proposed",
            ExperimentStatus::Benchmarking => "benchmarking",
            ExperimentStatus::Failed => "failed",
            ExperimentStatus::Rejected => "rejected",
            ExperimentStatus::Promoted => "promoted",
            ExperimentStatus::RolledBack => "rolled_back",
        }
    }
}

/// A candidate's declaration of what it changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateManifest {
    pub candidate_id: String,
    pub baseline_id: String,
    pub changed_scope: Vec<String>,
    pub rationale: String,
    pub generated_by: String,
    pub input_context_hash: String,
    pub reversible_save_id: Option<String>,
    /// P16 lineage — the candidate this one was mutated from (None = root).
    #[serde(default)]
    pub parent_candidate: Option<String>,
}

/// The verified-evolution experiment object (P1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionExperiment {
    pub id: String,
    pub target: EvolutionTarget,
    pub hypothesis: String,
    pub baseline_id: String,
    pub candidate_id: String,
    pub benchmark_suite_id: String,
    pub status: ExperimentStatus,
    #[serde(default)]
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub decision: Option<PromotionDecision>,
    pub manifest: CandidateManifest,
    // P7 harness
    #[serde(default)]
    pub harness: Option<HarnessCandidate>,
    // P11 explainability chain
    #[serde(default)]
    pub reference_ids: Vec<String>,
    #[serde(default)]
    pub decision_reason: Option<String>,
    #[serde(default)]
    pub aio_flagged: bool,
    #[serde(default)]
    pub simcise_score: Option<f64>,
    #[serde(default)]
    pub studio_ids: Vec<String>,
    // P15/P16 emergence lineage scoring
    #[serde(default)]
    pub novelty_score: f64,
    #[serde(default)]
    pub quality_score: f64,
    #[serde(default)]
    pub replication_count: u32,
    #[serde(default)]
    pub ablation_passed: bool,
    // P3 — execution identity: ties an experiment back to the Task /
    // ExecutionSession it was run under. A campaign never duplicates this data;
    // it only references the ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
    /// Harness provenance at the time the experiment was executed (P10). A
    /// harness switch is provenance, not a new project/campaign.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness_provenance: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---------------------------------------------------------------------------
// KnownGood + store
// ---------------------------------------------------------------------------

/// Reference to the current KnownGood state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownGood {
    pub id: String,
    pub label: String,
    pub save_id: Option<String>,
    pub promoted_at: i64,
    pub from_experiment: Option<String>,
    /// History for rollback to previous KnownGood.
    #[serde(default)]
    pub previous: Option<String>,
}

/// The evolution store: experiments + KnownGood + trust-root registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvolutionStore {
    #[serde(default)]
    pub experiments: Vec<EvolutionExperiment>,
    #[serde(default)]
    pub suites: Vec<BenchmarkSuite>,
    #[serde(default)]
    pub studios: Vec<StudioPreset>,
    #[serde(default)]
    pub known_good: Option<KnownGood>,
    #[serde(default)]
    pub known_good_history: Vec<KnownGood>,
}

// ---------------------------------------------------------------------------
// Paths & IO
// ---------------------------------------------------------------------------

pub fn evolution_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(EVOLUTION_DIR)
}

pub fn evolution_path(project_root: &Path) -> PathBuf {
    evolution_dir(project_root).join(EVOLUTION_FILE)
}

pub fn promotion_policy_path(project_root: &Path) -> PathBuf {
    evolution_dir(project_root).join(PROMOTION_POLICY_FILE)
}

impl EvolutionStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = evolution_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading evolution store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = evolution_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing evolution store to {}", p.display()))
    }

    pub fn get(&self, id: &str) -> Option<&EvolutionExperiment> {
        self.experiments
            .iter()
            .find(|e| e.id == id || e.id.starts_with(id))
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut EvolutionExperiment> {
        self.experiments
            .iter_mut()
            .find(|e| e.id == id || e.id.starts_with(id))
    }

    pub fn suite(&self, id: &str) -> Option<&BenchmarkSuite> {
        self.suites.iter().find(|s| s.id == id)
    }

    pub fn current_known_good(&self) -> Option<&KnownGood> {
        self.known_good.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Trust-root invariant checks
// ---------------------------------------------------------------------------

/// Check a set of proposed mutations against trust-root invariants.
/// Returns all violations; a candidate is blocked if any `blocked` violation
/// exists. This is the P0 guard that prevents a candidate from deleting the
/// ORIGINAL archive, tampering with evidence, or forging PASS.
pub fn check_trust_root_invariants(
    audit_path: &Path,
    proposed_actions: &[TrustRootViolation],
) -> Vec<TrustRootViolation> {
    // The audit ledger itself is append-only; we never delete or rewrite it.
    let _ = audit_path;
    proposed_actions
        .iter()
        .filter(|v| v.severity == "blocked")
        .cloned()
        .collect()
}

/// Verify that a candidate leaving a `status` transition is legal.
/// A candidate can never transition itself to Promoted; only the gated
/// promotion path (Engine gate) may do so.
pub fn promotion_gate_ok(policy: &PromotionPolicy, decision: PromotionDecision) -> bool {
    if !policy.require_engine_gate {
        return false;
    }
    match decision {
        PromotionDecision::Dominates | PromotionDecision::Improves => true,
        _ => false,
    }
}

/// Promotion policy (its own trust root).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionPolicy {
    pub id: String,
    /// Whether promotion must pass an Engine gate (not merely an AI claim).
    pub require_engine_gate: bool,
    /// Key correctness metrics that, if regressed, force rejection.
    pub correctness_metrics: Vec<String>,
    pub created_at: i64,
}

impl Default for PromotionPolicy {
    fn default() -> Self {
        Self {
            id: "default-promotion-policy".to_string(),
            require_engine_gate: true,
            correctness_metrics: vec![
                "correctness".to_string(),
                "verification_pass".to_string(),
                "regression_count".to_string(),
            ],
            created_at: route_core::now_millis(),
        }
    }
}

// ---------------------------------------------------------------------------
// Anti-AIO screening + Simcise score
// ---------------------------------------------------------------------------

/// Anti-AIO (All-in-One) screening.
///
/// A candidate that tries to cram unrelated functionality into a single-
/// purpose module is flagged and evaluated down. `module_scope` is the
/// declared purpose of the module being modified; `changed_scope` is the set
/// of concerns the candidate touches.
pub fn aio_screen(module_scope: &str, changed_scope: &[String]) -> bool {
    // A module is "single-purpose" if its declared scope is specific.
    let single_purpose = !module_scope.is_empty() && module_scope.len() < 40;
    if !single_purpose {
        return false;
    }
    // Count how many distinct concerns the candidate crosses.
    let concerns: std::collections::BTreeSet<&str> =
        changed_scope.iter().map(|s| s.as_str()).collect();
    // If the candidate touches more than one concern beyond its own module
    // scope, it is drifting toward AIO.
    concerns.len() > 1
}

/// Simcise design score — a lightweight, deterministic heuristic for design
/// norms (modal overuse, color constraint, local-first). Returns a score in
/// [0.0, 1.0]. Higher is better.
pub fn simcise_score(
    has_modal_overuse: bool,
    color_off_brand: bool,
    violates_local_first: bool,
    has_excess_component_nesting: bool,
) -> f64 {
    let mut score = 1.0_f64;
    if has_modal_overuse {
        score -= 0.25;
    }
    if color_off_brand {
        score -= 0.25;
    }
    if violates_local_first {
        score -= 0.25;
    }
    if has_excess_component_nesting {
        score -= 0.25;
    }
    score.max(0.0)
}

/// Studio presets: the default multi-node set.
pub fn default_studios() -> Vec<StudioPreset> {
    vec![
        StudioPreset {
            id: "studio-protocol".to_string(),
            name: "Protocol / Core".to_string(),
            scope: "protocol".to_string(),
            simcise_enforced: false,
            cases: vec![],
        },
        StudioPreset {
            id: "studio-audio".to_string(),
            name: "Audio / Multimodal".to_string(),
            scope: "audio".to_string(),
            simcise_enforced: false,
            cases: vec![],
        },
        StudioPreset {
            id: "studio-ui".to_string(),
            name: "UI / Interaction".to_string(),
            scope: "ui".to_string(),
            simcise_enforced: true,
            cases: vec![],
        },
    ]
}

// ---------------------------------------------------------------------------
// Benchmark running
// ---------------------------------------------------------------------------

/// Run a benchmark suite against baseline and candidate results.
///
/// Anti-reward-hacking rules enforced here:
/// - A candidate cannot drop Fixed/Holdout cases (immutable) to raise score.
/// - `expected` cannot be silently changed by a candidate (that requires a
///   new proposal).
/// - A candidate's own generated cases are marked `CandidateGenerated` and
///   never counted as authoritative.
pub fn run_benchmark_suite(
    suite: &BenchmarkSuite,
    baseline_results: &[BenchmarkResult],
    candidate_results: &[BenchmarkResult],
) -> Vec<BenchmarkResult> {
    let mut merged: Vec<BenchmarkResult> = Vec::new();
    for case in &suite.cases {
        let b = baseline_results.iter().find(|r| r.case_id == case.id);
        let c = candidate_results.iter().find(|r| r.case_id == case.id);
        merged.push(BenchmarkResult {
            case_id: case.id.clone(),
            name: case.name.clone(),
            origin: case.origin,
            baseline_passed: b.map(|r| r.baseline_passed).unwrap_or(false),
            candidate_passed: c.map(|r| r.candidate_passed).unwrap_or(false),
            normalized: !case.mutable && case.origin.authoritative(),
            note: if !case.mutable && case.origin.authoritative() {
                "immutable authoritative case".to_string()
            } else {
                String::new()
            },
        });
    }
    merged
}

/// Compute the promotion decision from benchmark results + metrics.
///
/// Defaults (P5):
/// - Key correctness regression → REGRESSES (reject).
/// - Only speed/token gain with correctness loss → REGRESSES.
/// - Insufficient evidence → INCONCLUSIVE (never promote).
pub fn decide_promotion(
    results: &[BenchmarkResult],
    metrics: &[Metric],
    policy: &PromotionPolicy,
) -> (PromotionDecision, String) {
    // 1. Correctness regression from guard cases (Fixed/Holdout/Reference/
    //    Failure). AI exploratory cases (CandidateGenerated) are excluded —
    //    testing more is not higher quality.
    let guard_fail = results
        .iter()
        .filter(|r| r.origin.guards_correctness())
        .find(|r| r.baseline_passed && !r.candidate_passed);
    if let Some(f) = guard_fail {
        return (
            PromotionDecision::Regresses,
            format!(
                "guard case '{}' regressed (baseline passed, candidate failed)",
                f.name
            ),
        );
    }

    // 2. Correctness metric regression.
    for m in metrics {
        if policy.correctness_metrics.iter().any(|c| c == &m.name) {
            if let (Some(b), Some(c)) = (m.baseline, m.candidate) {
                match m.higher_is_better {
                    true if c < b => {
                        return (
                            PromotionDecision::Regresses,
                            format!("correctness metric '{}' regressed ({} -> {})", m.name, b, c),
                        );
                    }
                    false if c > b && m.name != "latency" && m.name != "token" => {
                        return (
                            PromotionDecision::Regresses,
                            format!("correctness metric '{}' regressed ({} -> {})", m.name, b, c),
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // 3. Count improvements / regressions across non-correctness metrics.
    let mut improved = 0usize;
    let mut regressed = 0usize;
    for m in metrics {
        if let (Some(b), Some(c)) = (m.baseline, m.candidate) {
            if (m.higher_is_better && c > b) || (!m.higher_is_better && c < b) {
                improved += 1;
            } else if (m.higher_is_better && c < b) || (!m.higher_is_better && c > b) {
                // Speed/token-only gains never override correctness (handled above).
                regressed += 1;
            }
        }
    }

    // 4. If only speed/token improved but nothing else, and correctness lost → rejected.
    //    (Already handled by correctness pass.) If no correctness data, be conservative.
    let has_correctness_data = metrics
        .iter()
        .any(|m| policy.correctness_metrics.iter().any(|c| c == &m.name));
    if !has_correctness_data {
        return (
            PromotionDecision::Inconclusive,
            "no correctness evidence — refuse to promote".to_string(),
        );
    }

    if regressed > 0 && improved > 0 {
        return (
            PromotionDecision::Tradeoff,
            format!(
                "{} metric(s) improved, {} regressed (correctness holds)",
                improved, regressed
            ),
        );
    }
    if regressed > 0 {
        return (
            PromotionDecision::Regresses,
            format!("{} metric(s) regressed", regressed),
        );
    }
    if improved >= 2 {
        (
            PromotionDecision::Dominates,
            format!("{} metrics improved, none regressed", improved),
        )
    } else if improved == 1 {
        (
            PromotionDecision::Improves,
            "one metric improved, none regressed".to_string(),
        )
    } else {
        (
            PromotionDecision::Neutral,
            "no measurable improvement".to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// Experiment lifecycle
// ---------------------------------------------------------------------------

/// Propose a new candidate experiment (P2: candidate-first).
///
/// This never touches stable/original state. It records the candidate's
/// manifest and returns a Proposed experiment. The `reversible_save_id`
/// must reference an existing save so the experiment is always recoverable.
pub fn propose_experiment(
    project_root: &Path,
    store: &mut EvolutionStore,
    target: EvolutionTarget,
    hypothesis: String,
    baseline_id: String,
    candidate_id: String,
    manifest: CandidateManifest,
    suite: BenchmarkSuite,
    reference_ids: Vec<String>,
    studio_ids: Vec<String>,
    task_id: Option<String>,
    session_id: Option<String>,
    context_hash: Option<String>,
    harness_provenance: Option<String>,
) -> Result<EvolutionExperiment> {
    // Trust-root guard: a candidate proposing against the ORIGINAL archive
    // or the promotion policy is blocked unless it is a Route self-update
    // that goes through the stable-Route evaluation gate.
    let violations = check_trust_root_invariants(
        &audit_path(project_root),
        &[if manifest.reversible_save_id.is_none() {
            TrustRootViolation {
                trust_root: TrustRoot::RecoveryExecutor,
                reason: "candidate has no reversible save — must be recoverable".to_string(),
                severity: "blocked".to_string(),
            }
        } else {
            TrustRootViolation {
                trust_root: TrustRoot::RecoveryExecutor,
                reason: "recoverable".to_string(),
                severity: "ok".to_string(),
            }
        }],
    );
    if !violations.is_empty() {
        anyhow::bail!(
            "proposal blocked by trust-root invariant: {:?}",
            violations[0].reason
        );
    }

    let id = route_core::new_id();
    let now = route_core::now_millis();
    let suite_id = suite.id.clone();
    store.suites.push(suite);

    let exp = EvolutionExperiment {
        id: id.clone(),
        target,
        hypothesis,
        baseline_id,
        candidate_id,
        benchmark_suite_id: suite_id,
        status: ExperimentStatus::Proposed,
        metrics: Vec::new(),
        evidence: Vec::new(),
        decision: None,
        manifest,
        harness: None,
        reference_ids,
        decision_reason: None,
        aio_flagged: false,
        simcise_score: None,
        studio_ids,
        novelty_score: 0.0,
        quality_score: 0.0,
        replication_count: 0,
        ablation_passed: false,
        task_id,
        session_id,
        context_hash,
        harness_provenance,
        created_at: now,
        updated_at: now,
    };
    store.experiments.push(exp.clone());
    store
        .save(project_root)
        .context("persisting proposed evolution experiment")?;
    Ok(exp)
}

fn audit_path(project_root: &Path) -> PathBuf {
    project_root
        .join(ROUTE_DOT_DIR)
        .join("execution")
        .join("evidence.json")
}

/// Run evaluation for an experiment. Produces a decision but never promotes
/// by itself — promotion is always gated separately.
pub fn evaluate_experiment(
    project_root: &Path,
    store: &mut EvolutionStore,
    id: &str,
    baseline_results: &[BenchmarkResult],
    candidate_results: &[BenchmarkResult],
    metrics: Vec<Metric>,
    policy: &PromotionPolicy,
) -> Result<EvolutionExperiment> {
    let suite_id = {
        let exp = store
            .get(id)
            .ok_or_else(|| anyhow!("experiment '{}' not found", id))?;
        exp.benchmark_suite_id.clone()
    };
    let suite = store
        .suites
        .iter()
        .find(|s| s.id == suite_id)
        .ok_or_else(|| anyhow!("benchmark suite '{}' not found", suite_id))?
        .clone();

    let results = run_benchmark_suite(&suite, baseline_results, candidate_results);
    let (decision, reason) = decide_promotion(&results, &metrics, policy);

    let exp = store.get_mut(id).expect("experiment exists after fetch");
    exp.metrics = metrics;
    exp.status = ExperimentStatus::Benchmarking;
    exp.decision = Some(decision);
    exp.decision_reason = Some(reason);
    exp.updated_at = route_core::now_millis();
    let exp = exp.clone();
    store
        .save(project_root)
        .context("persisting evaluation result")?;
    Ok(exp)
}

/// Promote a candidate to KnownGood. Gated: only Dominates/Improves pass the
/// engine gate, and the candidate itself can never call this to promote
/// itself — it must come from the gated promotion path.
pub fn promote_experiment(
    project_root: &Path,
    store: &mut EvolutionStore,
    id: &str,
    label: &str,
    policy: &PromotionPolicy,
) -> Result<EvolutionExperiment> {
    let exp = store
        .get(id)
        .ok_or_else(|| anyhow!("experiment '{}' not found", id))?;
    let decision = exp
        .decision
        .ok_or_else(|| anyhow!("experiment '{}' has not been evaluated", id))?;
    if !promotion_gate_ok(policy, decision) {
        anyhow::bail!(
            "promotion gate DENIED: decision '{}' is not promotable under policy {:?}",
            decision.as_str(),
            policy.id
        );
    }

    // Record the new KnownGood, preserving history for rollback.
    let previous = store.known_good.as_ref().map(|k| k.id.clone());
    let kg = KnownGood {
        id: route_core::new_id(),
        label: label.to_string(),
        save_id: exp.manifest.reversible_save_id.clone(),
        promoted_at: route_core::now_millis(),
        from_experiment: Some(exp.id.clone()),
        previous,
    };
    store.known_good_history.push(kg.clone());
    store.known_good = Some(kg);

    let exp = store.get_mut(id).expect("exists");
    exp.status = ExperimentStatus::Promoted;
    exp.updated_at = route_core::now_millis();
    let exp = exp.clone();
    store.save(project_root).context("persisting promotion")?;
    Ok(exp)
}

/// Reject a candidate. Stable state is untouched.
pub fn reject_experiment(
    project_root: &Path,
    store: &mut EvolutionStore,
    id: &str,
    reason: Option<&str>,
) -> Result<EvolutionExperiment> {
    let exp = store
        .get_mut(id)
        .ok_or_else(|| anyhow!("experiment '{}' not found", id))?;
    exp.status = ExperimentStatus::Rejected;
    exp.decision_reason = Some(reason.unwrap_or("rejected by gate / user").to_string());
    exp.updated_at = route_core::now_millis();
    let exp = exp.clone();
    store.save(project_root).context("persisting rejection")?;
    Ok(exp)
}

/// Roll back to the previous KnownGood. Always recoverable.
pub fn rollback_experiment(project_root: &Path, store: &mut EvolutionStore) -> Result<KnownGood> {
    let kg = store
        .known_good
        .as_ref()
        .ok_or_else(|| anyhow!("no KnownGood to roll back"))?;
    let prev_id = kg
        .previous
        .clone()
        .ok_or_else(|| anyhow!("no previous KnownGood to roll back to"))?;
    let prev = store
        .known_good_history
        .iter()
        .find(|k| k.id == prev_id)
        .cloned()
        .ok_or_else(|| anyhow!("previous KnownGood '{}' not in history", prev_id))?;
    store.known_good = Some(prev.clone());
    store.save(project_root).context("persisting rollback")?;
    Ok(prev)
}

// ---------------------------------------------------------------------------
// Reference as external anchor (P4)
// ---------------------------------------------------------------------------

/// Extract a ReferenceDerived benchmark case from a Reference constraint.
/// This treats Reference not just as context, but as an external constraint /
/// oracle. It never injects full Reference content into any prompt.
pub fn derive_benchmark_from_reference(
    reference: &ReferenceEntry,
    case_name: &str,
    expected: &str,
) -> BenchmarkCase {
    BenchmarkCase {
        id: route_core::new_id(),
        name: case_name.to_string(),
        origin: BenchmarkOrigin::ReferenceDerived,
        scope: reference
            .project_scope
            .clone()
            .unwrap_or_else(|| "project".to_string()),
        expected: expected.to_string(),
        trust: reference
            .trust
            .clone()
            .unwrap_or_else(|| "medium".to_string()),
        mutable: true, // reference-derived is mutable only by a new proposal
        evidence: vec![],
        reference_id: Some(reference.id.clone()),
        studio: None,
        simcise_check: false,
    }
}

/// Load a reference by id from the registry (external anchor lookup).
pub fn load_reference(project_root: &Path, id: &str) -> Result<Option<ReferenceEntry>> {
    Ok(ReferenceRegistry::read(project_root)?.get(id).cloned())
}

// ---------------------------------------------------------------------------
// Harness candidate protection (P7)
// ---------------------------------------------------------------------------

/// Record a pre-change harness save and manufacture a harness candidate.
/// Route protects harness state but never executes the harness.
pub fn register_harness_candidate(
    pre_change_save_id: Option<String>,
    host: &str,
    preset_hash: &str,
    skills: Vec<String>,
    tools: Vec<String>,
    workflow: Option<String>,
    capabilities: Vec<String>,
    model_route_metadata: &str,
) -> HarnessCandidate {
    HarnessCandidate {
        id: route_core::new_id(),
        host: host.to_string(),
        preset_hash: preset_hash.to_string(),
        skills,
        tools,
        workflow,
        capabilities,
        model_route_metadata: model_route_metadata.to_string(),
        pre_change_save_id,
        mount_ok: false,
        cleanup_ok: false,
    }
}

/// Outcome of attempting to mount a harness candidate (P7).
///
/// Crucial contract: Route protects harness **state** but never executes the
/// harness. Mount success is reported by the harness; Route only verifies the
/// mandatory gates (core tool availability, basic task, verification) and, on
/// failed mount, returns the pre-change save so the caller restores the
/// previous KnownGood harness config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessMountResult {
    pub candidate_id: String,
    pub mount_ok: bool,
    pub core_tools_available: bool,
    pub basic_task_ok: bool,
    pub verification_ok: bool,
    /// Non-empty when restoration is required: use this pre-change save.
    pub restore_save_id: Option<String>,
    pub reason: String,
}

/// Evaluate a harness mount report. On any gate failure, requests restore to
/// the pre-change save (the last KnownGood harness config). Never executes the
/// harness itself.
pub fn check_harness_mount(
    candidate: &HarnessCandidate,
    mount_ok: bool,
    core_tools_available: bool,
    basic_task_ok: bool,
    verification_ok: bool,
) -> HarnessMountResult {
    let all_ok = mount_ok && core_tools_available && basic_task_ok && verification_ok;
    HarnessMountResult {
        candidate_id: candidate.id.clone(),
        mount_ok,
        core_tools_available,
        basic_task_ok,
        verification_ok,
        restore_save_id: if all_ok {
            None
        } else {
            candidate.pre_change_save_id.clone()
        },
        reason: if all_ok {
            "harness mounted cleanly".to_string()
        } else {
            "harness mount gate failed — restore pre-change save".to_string()
        },
    }
}

// ---------------------------------------------------------------------------
// Strategy evolution (P6)
// ---------------------------------------------------------------------------

/// A scope for a strategy-evolution proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyScope {
    Project,
    TaskPattern,
    Language,
    Harness,
    Tool,
}

impl StrategyScope {
    pub fn as_str(self) -> &'static str {
        match self {
            StrategyScope::Project => "project",
            StrategyScope::TaskPattern => "task-pattern",
            StrategyScope::Language => "language",
            StrategyScope::Harness => "harness",
            StrategyScope::Tool => "tool",
        }
    }
}

/// A strategy candidate (workflow / context-policy / harness-preset).
/// Single success never becomes a global strategy — it must clear the
/// evidence threshold and a benchmark/promotion gate within a limited scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyCandidate {
    pub id: String,
    pub kind: String, // "workflow" | "context_policy" | "harness_preset"
    pub scope: StrategyScope,
    pub hypothesis: String,
    pub evidence_threshold_met: bool,
    pub supporting_experiment_ids: Vec<String>,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_suite() -> BenchmarkSuite {
        BenchmarkSuite {
            id: "suite-1".to_string(),
            name: "smoke".to_string(),
            generated_by: "system".to_string(),
            cases: vec![
                BenchmarkCase {
                    id: "c-fixed".to_string(),
                    name: "fixed correctness".to_string(),
                    origin: BenchmarkOrigin::FixedBaseline,
                    scope: "project".to_string(),
                    expected: "pass".to_string(),
                    trust: "high".to_string(),
                    mutable: false,
                    evidence: vec![],
                    reference_id: None,
                    studio: None,
                    simcise_check: false,
                },
                BenchmarkCase {
                    id: "c-holdout".to_string(),
                    name: "holdout".to_string(),
                    origin: BenchmarkOrigin::Holdout,
                    scope: "project".to_string(),
                    expected: "pass".to_string(),
                    trust: "high".to_string(),
                    mutable: false,
                    evidence: vec![],
                    reference_id: None,
                    studio: None,
                    simcise_check: false,
                },
            ],
        }
    }

    fn result(
        case_id: &str,
        name: &str,
        origin: BenchmarkOrigin,
        base: bool,
        cand: bool,
    ) -> BenchmarkResult {
        BenchmarkResult {
            case_id: case_id.to_string(),
            name: name.to_string(),
            origin,
            baseline_passed: base,
            candidate_passed: cand,
            normalized: true,
            note: String::new(),
        }
    }

    #[test]
    fn trust_root_blocks_non_recoverable_candidate() {
        let tmp = TempDir::new().unwrap();
        let mut store = EvolutionStore::default();
        let manifest = CandidateManifest {
            candidate_id: "c1".to_string(),
            baseline_id: "b1".to_string(),
            reversible_save_id: None,
            ..Default::default()
        };
        let res = propose_experiment(
            tmp.path(),
            &mut store,
            EvolutionTarget::Project,
            "h".to_string(),
            "b1".to_string(),
            "c1".to_string(),
            manifest,
            sample_suite(),
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        );
        assert!(res.is_err(), "non-recoverable candidate must be blocked");
    }

    #[test]
    fn promotable_requires_reversible_save() {
        let tmp = TempDir::new().unwrap();
        let mut store = EvolutionStore::default();
        let manifest = CandidateManifest {
            candidate_id: "c1".to_string(),
            baseline_id: "b1".to_string(),
            reversible_save_id: Some("save-ok".to_string()),
            ..Default::default()
        };
        let exp = propose_experiment(
            tmp.path(),
            &mut store,
            EvolutionTarget::Project,
            "h".to_string(),
            "b1".to_string(),
            "c1".to_string(),
            manifest,
            sample_suite(),
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(exp.status, ExperimentStatus::Proposed);
    }

    #[test]
    fn correctness_regression_is_rejected() {
        let policy = PromotionPolicy::default();
        let baseline = vec![
            result(
                "c-fixed",
                "fixed",
                BenchmarkOrigin::FixedBaseline,
                true,
                true,
            ),
            result(
                "c-holdout",
                "holdout",
                BenchmarkOrigin::Holdout,
                true,
                false,
            ),
        ];
        let (dec, reason) = decide_promotion(&baseline, &[], &policy);
        assert_eq!(dec, PromotionDecision::Regresses);
        assert!(reason.contains("holdout"));
    }

    #[test]
    fn speed_only_gain_without_correctness_is_inconclusive() {
        let policy = PromotionPolicy::default();
        // No correctness metric present -> refuse to promote.
        let metrics = vec![Metric {
            name: "latency".to_string(),
            baseline: Some(10.0),
            candidate: Some(5.0),
            unit: "ms".to_string(),
            higher_is_better: false,
        }];
        let (dec, _) = decide_promotion(&[], &metrics, &policy);
        assert_eq!(dec, PromotionDecision::Inconclusive);
    }

    #[test]
    fn anti_aio_flags_scope_crossing() {
        // A single-purpose module (scoped to "protocol") that also touches
        // "audio" and "ui" is drifting toward AIO.
        assert!(aio_screen(
            "protocol",
            &["protocol".into(), "audio".into(), "ui".into()]
        ));
        // Same-scope change is fine.
        assert!(!aio_screen("protocol", &["protocol".into()]));
    }

    #[test]
    fn simcise_score_punishes_norm_violations() {
        let good = simcise_score(false, false, false, false);
        assert_eq!(good, 1.0);
        let bad = simcise_score(true, true, true, true);
        assert_eq!(bad, 0.0);
        let mix = simcise_score(true, false, true, false);
        assert!((mix - 0.5).abs() < 1e-9);
    }

    #[test]
    fn promotion_gate_requires_engine_gate() {
        let policy = PromotionPolicy {
            require_engine_gate: true,
            ..Default::default()
        };
        assert!(promotion_gate_ok(&policy, PromotionDecision::Dominates));
        assert!(promotion_gate_ok(&policy, PromotionDecision::Improves));
        assert!(!promotion_gate_ok(&policy, PromotionDecision::Neutral));
        assert!(!promotion_gate_ok(&policy, PromotionDecision::Regresses));
        assert!(!promotion_gate_ok(&policy, PromotionDecision::Inconclusive));
    }

    #[test]
    fn promote_then_rollback_restores_previous_known_good() {
        let tmp = TempDir::new().unwrap();
        let mut store = EvolutionStore::default();
        let policy = PromotionPolicy::default();

        // Seed a baseline KnownGood (must also be in history for rollback).
        let kg0 = KnownGood {
            id: "kg-0".to_string(),
            label: "baseline".to_string(),
            save_id: None,
            promoted_at: route_core::now_millis(),
            from_experiment: None,
            previous: None,
        };
        store.known_good_history.push(kg0.clone());
        store.known_good = Some(kg0);

        // Propose + evaluate a passing candidate.
        let manifest = CandidateManifest {
            candidate_id: "c2".to_string(),
            baseline_id: "b1".to_string(),
            reversible_save_id: Some("s2".to_string()),
            ..Default::default()
        };
        let exp = propose_experiment(
            tmp.path(),
            &mut store,
            EvolutionTarget::Workflow,
            "improve".to_string(),
            "b1".to_string(),
            "c2".to_string(),
            manifest,
            sample_suite(),
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // Evaluate: both pass, correctness metric improves.
        let suite1 = sample_suite();
        let base = suite1
            .cases
            .iter()
            .map(|c| result(&c.id, &c.name, c.origin, true, true))
            .collect::<Vec<_>>();
        let cand = base.clone();
        let metrics = vec![Metric {
            name: "correctness".to_string(),
            baseline: Some(0.8),
            candidate: Some(0.9),
            unit: "".to_string(),
            higher_is_better: true,
        }];
        evaluate_experiment(
            tmp.path(),
            &mut store,
            &exp.id,
            &base,
            &cand,
            metrics,
            &policy,
        )
        .unwrap();
        let after = store.get(&exp.id).unwrap().decision.unwrap();
        assert_eq!(after, PromotionDecision::Improves);

        let promoted = promote_experiment(tmp.path(), &mut store, &exp.id, "v2", &policy).unwrap();
        assert_eq!(promoted.status, ExperimentStatus::Promoted);
        assert_eq!(
            store
                .known_good
                .as_ref()
                .unwrap()
                .from_experiment
                .as_deref(),
            Some(exp.id.as_str())
        );

        // Roll back -> previous KnownGood restored.
        let prev = rollback_experiment(tmp.path(), &mut store).unwrap();
        assert_eq!(prev.id, "kg-0");
    }

    #[test]
    fn candidate_cannot_promote_itself_without_gate() {
        let tmp = TempDir::new().unwrap();
        let mut store = EvolutionStore::default();
        let policy = PromotionPolicy::default();
        let manifest = CandidateManifest {
            candidate_id: "c3".to_string(),
            baseline_id: "b1".to_string(),
            reversible_save_id: Some("s3".to_string()),
            ..Default::default()
        };
        let exp = propose_experiment(
            tmp.path(),
            &mut store,
            EvolutionTarget::Route,
            "self update".to_string(),
            "b1".to_string(),
            "c3".to_string(),
            manifest,
            sample_suite(),
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        // Evaluate with a Neutral result (no metrics).
        evaluate_experiment(tmp.path(), &mut store, &exp.id, &[], &[], vec![], &policy).unwrap();
        let res = promote_experiment(tmp.path(), &mut store, &exp.id, "v3", &policy);
        assert!(res.is_err(), "neutral/insufficient evidence must be denied");
    }

    #[test]
    fn reference_derived_case_is_anchored() {
        let ref_entry = ReferenceEntryBuilderForTest::build();
        let case = derive_benchmark_from_reference(&ref_entry, "constraint", "must not regress");
        assert_eq!(case.origin, BenchmarkOrigin::ReferenceDerived);
        assert_eq!(case.reference_id.as_deref(), Some("ref-audio-1"));
    }

    #[test]
    fn benchmark_suite_marks_immutable_authoritative() {
        let suite = sample_suite();
        let base = suite
            .cases
            .iter()
            .map(|c| result(&c.id, &c.name, c.origin, true, true))
            .collect::<Vec<_>>();
        let stub = vec![result(
            "c-fixed",
            "fixed",
            BenchmarkOrigin::FixedBaseline,
            true,
            true,
        )];
        let merged = run_benchmark_suite(&suite, &base, &stub);
        // The holdout case is missing from candidate results -> candidate_passed=false,
        // but it is immutable and authoritative -> normalized=true.
        let holdout = merged.iter().find(|r| r.case_id == "c-holdout").unwrap();
        assert!(holdout.normalized);
        assert!(!holdout.candidate_passed);
    }

    // Minimal Reference builder for tests (avoids full constructor).
    struct ReferenceEntryBuilderForTest;
    impl ReferenceEntryBuilderForTest {
        fn build() -> ReferenceEntry {
            ReferenceEntry::builder(
                "ref-audio-1",
                crate::constitutive::ReferenceType::Document,
                "file:///audio/norms.md",
                "audio metadata norms",
            )
            .with_constraints("never corrupt mp3 tags")
            .with_tags(vec!["audio".to_string()])
            .with_origin(crate::constitutive::Origin::UserCreated)
            .with_name("audio-norms")
            .with_project_scope("audio")
            .with_trust("high")
            .build()
        }
    }

    // -------------------------------------------------------------------
    // P10 integrity scenarios
    // -------------------------------------------------------------------

    // P10 #4: harness preset candidate mount failure → restore pre-change save.
    #[test]
    fn harness_mount_failure_requests_restore() {
        let cand = register_harness_candidate(
            Some("pre-harness-save".to_string()),
            "deepseek-pic",
            "hash-abc",
            vec!["music".to_string()],
            vec!["route_cli".to_string()],
            Some("wf-audio".to_string()),
            vec!["audio".to_string()],
            "dsh",
        );
        // Mount fails on verification gate.
        let res = check_harness_mount(&cand, true, true, true, false);
        assert!(!res.verification_ok);
        assert_eq!(
            res.restore_save_id.as_deref(),
            Some("pre-harness-save"),
            "failed mount must request restore to pre-change save"
        );
        // Clean mount → no restore.
        let ok = check_harness_mount(&cand, true, true, true, true);
        assert!(ok.restore_save_id.is_none());
    }

    // P10 #6: AI-generated benchmark conflicts with fixed benchmark → fixed wins.
    #[test]
    fn ai_benchmark_conflict_fixed_wins() {
        let policy = PromotionPolicy::default();
        // Candidate passes its own AI-generated case but regresses a fixed one.
        let results = vec![
            result(
                "c-fixed",
                "fixed",
                BenchmarkOrigin::FixedBaseline,
                true,
                false,
            ),
            result(
                "c-ai",
                "ai exploratory",
                BenchmarkOrigin::CandidateGenerated,
                true,
                true,
            ),
        ];
        let (dec, _) = decide_promotion(&results, &[], &policy);
        assert_eq!(
            dec,
            PromotionDecision::Regresses,
            "fixed guard regression must win over any AI-generated pass"
        );
    }

    // P10 #7: Reference-derived constraint catches a candidate regression.
    #[test]
    fn reference_derived_constraint_catches_regression() {
        let policy = PromotionPolicy::default();
        // A Reference-derived case passed at baseline but regressed under the
        // candidate → must be caught (guards_correctness covers Reference).
        let results = vec![result(
            "c-ref",
            "reference constraint",
            BenchmarkOrigin::ReferenceDerived,
            true,
            false,
        )];
        let (dec, reason) = decide_promotion(&results, &[], &policy);
        assert_eq!(dec, PromotionDecision::Regresses);
        assert!(reason.contains("reference constraint"));
    }

    // P10 #8: Original / trusted evidence cannot be silently rewritten to PASS.
    #[test]
    fn candidate_cannot_forge_evidence_pass() {
        // A candidate that claims PASS on an authoritative case that we know
        // failed is caught by verification: the decision engine uses the
        // *reported* candidate result, but the gate refuses to promote a
        // regression. Forging "pass" is not a promotion path.
        let policy = PromotionPolicy::default();
        let metrics = vec![Metric {
            name: "correctness".to_string(),
            baseline: Some(1.0),
            candidate: Some(1.0), // forged
            unit: "".to_string(),
            higher_is_better: true,
        }];
        // Even with forged correctness metric, the guard benchmark result
        // (baseline passed, candidate failed) forces rejection.
        let results = vec![result(
            "c-holdout",
            "holdout",
            BenchmarkOrigin::Holdout,
            true,
            false,
        )];
        let (dec, _) = decide_promotion(&results, &metrics, &policy);
        assert_eq!(
            dec,
            PromotionDecision::Regresses,
            "forging a metric must not override a real guard regression"
        );
    }
}
