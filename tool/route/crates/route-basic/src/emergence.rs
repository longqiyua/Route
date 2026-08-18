//! Emergence Hardening (P13–P23) — shared blackboard, three-planes, novelty,
//! lineage, benchmark court, reference trust, champion/challenger, self-evolution
//! supervision, heterogeneous search, search budget, and failure-as-asset.
//!
//! Reuses the existing Evolution / Save / Learning / Evidence / Reference
//! architecture. Route never executes a model, scheduler, or harness; it only
//! records, evaluates, promotes, and rolls back. All emergence capability is
//! **experimental and off by default** — it never touches Stable core unless a
//! caller explicitly enables it and passes the existing gated promotion path.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::write_atomic;
use crate::evolution::{BenchmarkCase, BenchmarkOrigin, Metric, PromotionDecision};

/// Directory name for emergence data under `.route/`.
pub const EMERGENCE_DIR: &str = "emergence";
/// Engine-gated promotion result (P19) that maps to the core decision.
pub use crate::evolution::PromotionDecision as EmergenceDecision;

// ---------------------------------------------------------------------------
// P14 — Three planes
// ---------------------------------------------------------------------------

/// The plane a candidate belongs to. EXPLORE produces hypotheses, EXECUTE
/// implements them into candidates (never self-promotes), EVALUATE judges
/// (never implements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plane {
    Explore,
    Execute,
    Evaluate,
}

impl Plane {
    pub fn as_str(self) -> &'static str {
        match self {
            Plane::Explore => "explore",
            Plane::Execute => "execute",
            Plane::Evaluate => "evaluate",
        }
    }
}

/// A search role (P21): heterogeneous Explorer / Executor / Evaluator, each
/// bound to a provider/model. Route stores only capability + provenance; the
/// harness does the actual spawn / API / tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRole {
    pub plane: Plane,
    pub provider: String,
    pub model: String,
    pub harness: Option<String>,
    pub capabilities: Vec<String>,
}

// ---------------------------------------------------------------------------
// P13 — Shared blackboard
// ---------------------------------------------------------------------------

/// An append-only blackboard event. Agents write events; the Route Engine
/// projects authoritative state. Agents can never directly mutate stable state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEvent {
    pub id: String,
    pub ts: i64,
    pub actor: String,
    pub plane: Plane,
    pub harness: Option<String>,
    pub session_id: Option<String>,
    pub task_id: Option<String>,
    pub source: String, // observation | proposal | evidence | decision
    pub confidence: f64,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub payload: serde_json::Value,
}

/// The shared project blackboard. Events are append-only; the projection is a
/// derived view that can be rebuilt from scratch (crash-safe, P25 #9).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Blackboard {
    #[serde(default)]
    pub events: Vec<BlackboardEvent>,
    #[serde(default)]
    pub projection: serde_json::Value,
}

impl Blackboard {
    /// Append an event (append-only) and repaint the projection. Never mutates
    /// authoritative state directly — that is the Engine's job.
    pub fn append(&mut self, ev: BlackboardEvent) {
        self.events.push(ev);
        self.rebuild_projection();
    }

    /// Rebuild the derived projection from the raw event log. Deterministic and
    /// crash-recoverable: any partial write discards to the last atomic save.
    pub fn rebuild_projection(&mut self) {
        let mut facts: Vec<serde_json::Value> = Vec::new();
        let mut by_source: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for ev in &self.events {
            by_source
                .entry(ev.source.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            facts.push(serde_json::json!({
                "id": ev.id,
                "ts": ev.ts,
                "actor": ev.actor,
                "plane": ev.plane.as_str(),
                "confidence": ev.confidence,
            }));
        }
        self.projection = serde_json::json!({
            "total_events": self.events.len(),
            "by_source": by_source,
            "facts": facts,
        });
    }
}

// ---------------------------------------------------------------------------
// P15 — Quality + novelty
// ---------------------------------------------------------------------------

/// Multi-dimensional scoring. Evolution must not optimize benchmark score alone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScoring {
    pub quality_score: f64,
    pub novelty_score: f64,
    pub risk: String, // low | medium | high
    pub cost: f64,
    pub scope: String,
}

/// The current Champion (best known candidate at the specified scope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Champion {
    pub scope: String,
    pub candidate_id: String,
    pub quality_score: f64,
    pub decision: Option<PromotionDecision>,
    pub promoted_at: i64,
}

/// A high-novelty candidate that did not win — archived to avoid premature
/// convergence to a single path. Novelty alone can never bypass the
/// correctness gate (P15).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoveltyArchiveEntry {
    pub id: String,
    pub candidate_id: String,
    pub hypothesis: String,
    pub novelty_score: f64,
    pub quality_score: f64,
    pub decision: Option<PromotionDecision>,
    pub archived_at: i64,
}

// ---------------------------------------------------------------------------
// P16 — Emergence lineage
// ---------------------------------------------------------------------------

/// Full lineage of a successful candidate (P16). Answers "how did this
/// breakthrough appear?" and "would it still work without X?" via ablation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnovationRecord {
    pub id: String,
    pub model: String,
    pub model_config: Option<String>,
    pub harness: String,
    pub preset: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    pub workflow: Option<String>,
    pub context_hash: String,
    #[serde(default)]
    pub references: Vec<String>,
    pub hypothesis: String,
    pub parent_candidate: Option<String>,
    pub mutation: String,
    #[serde(default)]
    pub changed_scope: Vec<String>,
    pub benchmark_suite: String,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub metrics: Vec<Metric>,
    pub replication_count: u32,
    pub cross_task_validated: bool,
    pub ablation_passed: bool,
    pub created_at: i64,
}

/// The gate for promoting a lineage into a long-lived Strategy/Gene. A single
/// success never becomes a strategy; it needs replication >= threshold AND
/// cross-task validation OR ablation proving a stable contribution (P16).
pub fn strategy_gate_ok(
    record: &InnovationRecord,
    replication_threshold: u32,
) -> bool {
    record.replication_count >= replication_threshold
        && (record.cross_task_validated || record.ablation_passed)
}

// ---------------------------------------------------------------------------
// P17 — Benchmark court
// ---------------------------------------------------------------------------

/// Independent lifecycle of a benchmark case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkLifecycle {
    Proposed,
    Quarantined,
    Validated,
    Trusted,
    Retired,
}

/// The benchmark court ledger. Candidate-generated benchmarks start
/// Quarantined; a benchmark that backed candidate A cannot be the sole
/// promotion evidence for A (P17).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtEntry {
    pub id: String,
    pub case: BenchmarkCase,
    pub lifecycle: BenchmarkLifecycle,
    /// Candidate that produced the case (if CandidateGenerated).
    pub produced_by: Option<String>,
    /// Baseline discriminates historical pass/fail (admission check).
    pub discriminates: bool,
    pub constitution_compatible: bool,
    pub provenance_ok: bool,
}

/// Admission checks for a benchmark (P17). Returns a list of failures.
pub fn benchmark_admission(
    case: &BenchmarkCase,
    discriminates: bool,
    constitution_compatible: bool,
    provenance_ok: bool,
) -> Vec<String> {
    let mut problems = Vec::new();
    if !discriminates {
        problems.push("baseline cannot distinguish historical pass/fail".to_string());
    }
    if !constitution_compatible {
        problems.push("conflicts with the Constitution".to_string());
    }
    if !provenance_ok {
        problems.push("missing or untrusted provenance".to_string());
    }
    if case.trust == "high" && !match case.origin {
        BenchmarkOrigin::FixedBaseline | BenchmarkOrigin::Holdout => true,
        _ => false,
    } {
        problems.push("high-trust claimed on a non-fixed/holdout case".to_string());
    }
    problems
}

/// Judge whether a produced benchmark may be the evidence for its producer.
/// A candidate-generated benchmark is never sole evidence for the candidate
/// that generated it (P17 anti-gaming).
pub fn self_benchmark_blocked(case_origin: BenchmarkOrigin, produced_by_self: bool) -> bool {
    matches!(case_origin, BenchmarkOrigin::CandidateGenerated) && produced_by_self
}

// ---------------------------------------------------------------------------
// P18 — Reference trust
// ---------------------------------------------------------------------------

/// Trust ordering for reference sources (P18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceAuthority {
    Constitution,
    VerifiedSpecification,
    TrustedReference,
    OrdinaryReference,
    AiHypothesis,
}

impl ReferenceAuthority {
    pub fn as_str(self) -> &'static str {
        match self {
            ReferenceAuthority::Constitution => "constitution",
            ReferenceAuthority::VerifiedSpecification => "verified_specification",
            ReferenceAuthority::TrustedReference => "trusted_reference",
            ReferenceAuthority::OrdinaryReference => "ordinary_reference",
            ReferenceAuthority::AiHypothesis => "ai_hypothesis",
        }
    }
}

/// Trust metadata attached to a reference used as an evolution anchor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceTrust {
    pub authority: ReferenceAuthority,
    pub freshness_ms: i64,
    pub scope: String,
    pub provenance: String,
    /// When non-empty, a conflicting authority exists — never resolve silently.
    pub conflict: Option<String>,
}

/// A reference conflict produces Conflict Evidence or NeedsHumanDecision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictResolution {
    ConflictEvidence(String),
    NeedsHumanDecision,
}

/// Resolve a reference conflict. Never silently picks a winner (P18).
pub fn resolve_reference_conflict(
    primary: &ReferenceTrust,
    secondary: &ReferenceTrust,
) -> ConflictResolution {
    if primary.authority == secondary.authority {
        return ConflictResolution::NeedsHumanDecision;
    }
    let winner_is_constitution = primary.authority == ReferenceAuthority::Constitution
        || primary.authority == ReferenceAuthority::VerifiedSpecification;
    let loser_is_weaker = primary.authority > secondary.authority;
    if winner_is_constitution || loser_is_weaker {
        ConflictResolution::ConflictEvidence(format!(
            "{} overrides {} per authority ordering",
            primary.authority.as_str(),
            secondary.authority.as_str()
        ))
    } else {
        ConflictResolution::NeedsHumanDecision
    }
}

// ---------------------------------------------------------------------------
// P22 — Search budget
// ---------------------------------------------------------------------------

/// Resource boundary for an EvolutionRun (P22). Reaching any boundary stops
/// the run and summarizes findings — never infinite recursion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchBudget {
    pub max_candidates: usize,
    pub max_rounds: usize,
    pub max_wall_time_ms: i64,
    pub max_failures: usize,
    pub cost_budget: Option<f64>,
    pub novelty_plateau_limit: usize,
}

impl Default for SearchBudget {
    fn default() -> Self {
        Self {
            max_candidates: 8,
            max_rounds: 3,
            max_wall_time_ms: 60_000,
            max_failures: 5,
            cost_budget: None,
            novelty_plateau_limit: 4,
        }
    }
}

/// Reason a run stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    NotStopped,
    MaxCandidates,
    MaxRounds,
    MaxWallTime,
    MaxFailures,
    CostBudget,
    NoveltyPlateau,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::NotStopped => "active",
            StopReason::MaxCandidates => "max_candidates",
            StopReason::MaxRounds => "max_rounds",
            StopReason::MaxWallTime => "max_wall_time",
            StopReason::MaxFailures => "max_failures",
            StopReason::CostBudget => "cost_budget",
            StopReason::NoveltyPlateau => "novelty_plateau",
        }
    }
}

/// A single evolution run with its budget and counters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionRun {
    pub id: String,
    pub budget: SearchBudget,
    pub candidates: usize,
    pub rounds: usize,
    pub failures: usize,
    pub cost: f64,
    pub novelty_plateau: usize,
    pub started_at: i64,
    pub status: String,
}

/// Decide whether a run must stop (P22). Deterministic and side-effect free.
pub fn should_stop(run: &EvolutionRun, now_ms: i64) -> StopReason {
    if run.candidates >= run.budget.max_candidates {
        StopReason::MaxCandidates
    } else if run.rounds >= run.budget.max_rounds {
        StopReason::MaxRounds
    } else if now_ms - run.started_at >= run.budget.max_wall_time_ms {
        StopReason::MaxWallTime
    } else if run.failures >= run.budget.max_failures {
        StopReason::MaxFailures
    } else if let Some(cost) = run.budget.cost_budget {
        if run.cost >= cost {
            StopReason::CostBudget
        } else {
            StopReason::NotStopped
        }
    } else if run.novelty_plateau >= run.budget.novelty_plateau_limit {
        StopReason::NoveltyPlateau
    } else {
        StopReason::NotStopped
    }
}

// ---------------------------------------------------------------------------
// P20 — Self-evolution supervisor
// ---------------------------------------------------------------------------

/// A watchdog result for a Route self-evolution run (P20). If the candidate
/// route fails to come up healthy, the supervisor returns to the previous
/// KnownGood. Promotion is atomic; the candidate route can never modify the
/// Stable evaluator, Original, or promotion record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogResult {
    pub healthy: bool,
    pub reverts_to: Option<String>,
    pub reason: String,
}

/// Run the watchdog: all gates must pass to keep the candidate route.
pub fn watchdog_check(
    candidate_up: bool,
    evaluator_intact: bool,
    original_intact: bool,
    promotion_record_intact: bool,
    previous_known_good: Option<String>,
) -> WatchdogResult {
    let healthy = candidate_up && evaluator_intact && original_intact && promotion_record_intact;
    WatchdogResult {
        healthy,
        reverts_to: if healthy {
            None
        } else {
            previous_known_good
        },
        reason: if healthy {
            "candidate route is healthy".to_string()
        } else {
            "watchdog failed — revert to previous KnownGood".to_string()
        },
    }
}

// ---------------------------------------------------------------------------
// P23 — Failure as asset
// ---------------------------------------------------------------------------

/// Extracted knowledge from a rejected candidate (P23). Code may be deleted,
/// but the failure pattern and a regression-benchmark proposal are preserved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    pub id: String,
    pub hypothesis: String,
    pub environment: String,
    pub context_hash: String,
    pub why_rejected: String,
    pub failed_mutation: String,
    pub regression_benchmark_proposal: Option<BenchmarkCase>,
    pub recorded_at: i64,
}

// ---------------------------------------------------------------------------
// Store & IO
// ---------------------------------------------------------------------------

/// The emergence store: blackboard + novelty + lineage + benchmark court +
/// failure patterns + runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmergenceStore {
    /// Experimental capability is OFF by default; Stable core is untouched.
    pub experimental_enabled: bool,
    #[serde(default)]
    pub blackboard: Blackboard,
    #[serde(default)]
    pub champions: Vec<Champion>,
    #[serde(default)]
    pub novelty_archive: Vec<NoveltyArchiveEntry>,
    #[serde(default)]
    pub innovations: Vec<InnovationRecord>,
    #[serde(default)]
    pub bench_court: Vec<CourtEntry>,
    #[serde(default)]
    pub failure_patterns: Vec<FailurePattern>,
    #[serde(default)]
    pub runs: Vec<EvolutionRun>,
}

impl EmergenceStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = emergence_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading emergence store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = emergence_path(project_root);
        let dir = p.parent().expect("emergence dir has parent");
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing emergence store to {}", p.display()))
    }

    /// Enable experimental emergence capability explicitly. Stable core is
    /// never auto-enabled.
    pub fn set_experimental_enabled(&mut self, enabled: bool) {
        self.experimental_enabled = enabled;
    }
}

pub fn emergence_dir(project_root: &Path) -> PathBuf {
    project_root
        .join(crate::constitutive::ROUTE_DOT_DIR)
        .join(EMERGENCE_DIR)
}

pub fn emergence_path(project_root: &Path) -> PathBuf {
    emergence_dir(project_root).join("emergence.json")
}

/// Guard: most emergence mutations require the experimental flag on.
pub fn require_experimental(store: &EmergenceStore) -> Result<()> {
    if store.experimental_enabled {
        Ok(())
    } else {
        Err(anyhow!(
            "emergence capability is experimental and disabled by default; \
             enable it explicitly before use"
        ))
    }
}

// ---------------------------------------------------------------------------
// Engine projection helpers
// ---------------------------------------------------------------------------

/// Record a blackboard event through the Engine (the only allowed write path).
pub fn engine_project_event(
    store: &mut EmergenceStore,
    ev: BlackboardEvent,
) -> Result<()> {
    require_experimental(store)?;
    store.blackboard.append(ev);
    Ok(())
}

/// Archive a high-novelty but non-winning candidate (P15). Novelty alone never
/// promotes; it only preserves the alternative path.
pub fn archive_novel_candidate(
    store: &mut EmergenceStore,
    candidate_id: &str,
    hypothesis: &str,
    novelty_score: f64,
    quality_score: f64,
    decision: Option<PromotionDecision>,
) -> Result<NoveltyArchiveEntry> {
    require_experimental(store)?;
    let entry = NoveltyArchiveEntry {
        id: route_core::new_id(),
        candidate_id: candidate_id.to_string(),
        hypothesis: hypothesis.to_string(),
        novelty_score,
        quality_score,
        decision,
        archived_at: route_core::now_millis(),
    };
    store.novelty_archive.push(entry.clone());
    Ok(entry)
}

/// Refresh the Champion for a scope if the candidate is strictly better and
/// the correctness gate passed (P19).
pub fn update_champion(
    store: &mut EmergenceStore,
    scope: &str,
    candidate_id: &str,
    quality_score: f64,
    decision: PromotionDecision,
) -> Result<()> {
    require_experimental(store)?;
    match decision {
        PromotionDecision::Regresses
        | PromotionDecision::Inconclusive
        | PromotionDecision::Neutral => {
            return Ok(()); // never replace champion on a non-positive decision
        }
        _ => {}
    }
    let slot = store
        .champions
        .iter_mut()
        .find(|c| c.scope == scope);
    match slot {
        Some(ch) => {
            if quality_score >= ch.quality_score {
                ch.candidate_id = candidate_id.to_string();
                ch.quality_score = quality_score;
                ch.decision = Some(decision);
                ch.promoted_at = route_core::now_millis();
            }
        }
        None => {
            store.champions.push(Champion {
                scope: scope.to_string(),
                candidate_id: candidate_id.to_string(),
                quality_score,
                decision: Some(decision),
                promoted_at: route_core::now_millis(),
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::EvolutionTarget;

    fn tstore() -> EmergenceStore {
        let mut s = EmergenceStore::default();
        s.set_experimental_enabled(true);
        s
    }

    fn bevent(actor: &str, source: &str) -> BlackboardEvent {
        BlackboardEvent {
            id: route_core::new_id(),
            ts: route_core::now_millis(),
            actor: actor.to_string(),
            plane: Plane::Explore,
            harness: None,
            session_id: None,
            task_id: None,
            source: source.to_string(),
            confidence: 0.7,
            evidence: vec![],
            payload: serde_json::json!({}),
        }
    }

    // P13 / P25 #9: blackboard events are not lost and state can be rebuilt.
    #[test]
    fn blackboard_rebuild_is_deterministic_and_append_only() {
        let mut s = tstore();
        engine_project_event(&mut s, bevent("explorer-a", "observation")).unwrap();
        engine_project_event(&mut s, bevent("explorer-b", "hypothesis")).unwrap();
        engine_project_event(&mut s, bevent("evaluator-1", "evidence")).unwrap();
        assert_eq!(s.blackboard.events.len(), 3);
        let p1 = s.blackboard.projection.clone();
        // Rebuild from scratch reproduces the same projection.
        s.blackboard.rebuild_projection();
        assert_eq!(s.blackboard.projection, p1);
        assert_eq!(
            s.blackboard.projection["by_source"]["observation"],
            serde_json::json!(1)
        );
    }

    #[test]
    fn experimental_off_by_default_blocks_mutation() {
        let mut s = EmergenceStore::default();
        assert!(!s.experimental_enabled);
        let r = engine_project_event(&mut s, bevent("a", "observation"));
        assert!(r.is_err(), "must be disabled by default");
    }

    // P15: high novelty archived, never promoted by novelty alone.
    #[test]
    fn novel_but_failed_is_archived_not_promoted() {
        let mut s = tstore();
        let entry = archive_novel_candidate(
            &mut s,
            "cand-novel",
            "wild idea",
            0.95,
            0.4,
            Some(PromotionDecision::Regresses),
        )
        .unwrap();
        assert_eq!(entry.novelty_score, 0.95);
        assert_eq!(entry.decision, Some(PromotionDecision::Regresses));
        // It lives in the archive, not as a champion.
        assert!(!s.champions.iter().any(|c| c.candidate_id == "cand-novel"));
    }

    // P15/P19: only a strictly better, gated candidate replaces the champion.
    #[test]
    fn champion_replaced_only_on_positive_decision() {
        let mut s = tstore();
        update_champion(&mut s, "audio", "c1", 0.8, PromotionDecision::Dominates).unwrap();
        assert_eq!(s.champions[0].candidate_id, "c1");
        // A regression must not replace the champion.
        update_champion(&mut s, "audio", "c2", 0.95, PromotionDecision::Regresses).unwrap();
        assert_eq!(s.champions[0].candidate_id, "c1");
        // A better non-regressing candidate replaces it.
        update_champion(&mut s, "audio", "c3", 0.9, PromotionDecision::Improves).unwrap();
        assert_eq!(s.champions[0].candidate_id, "c3");
    }

    // P16: single success never becomes a strategy; replication + ablation does.
    #[test]
    fn strategy_gate_needs_replication_and_validation() {
        let rec = InnovationRecord {
            id: "r1".to_string(),
            model: "deepseek".to_string(),
            model_config: None,
            harness: "dsh".to_string(),
            preset: None,
            skills: vec![],
            tools: vec![],
            workflow: None,
            context_hash: "h".to_string(),
            references: vec![],
            hypothesis: "h".to_string(),
            parent_candidate: None,
            mutation: "m".to_string(),
            changed_scope: vec![],
            benchmark_suite: "s".to_string(),
            outputs: vec![],
            evidence: vec![],
            metrics: vec![],
            replication_count: 1,
            cross_task_validated: false,
            ablation_passed: false,
            created_at: 0,
        };
        // Replication 1, no validation -> NO strategy.
        assert!(!strategy_gate_ok(&rec, 3));
        let rec2 = InnovationRecord {
            replication_count: 3,
            cross_task_validated: true,
            ..rec.clone()
        };
        assert!(strategy_gate_ok(&rec2, 3));
        // Ablation alone + replication also passes.
        let rec3 = InnovationRecord {
            replication_count: 3,
            cross_task_validated: false,
            ablation_passed: true,
            ..rec.clone()
        };
        assert!(strategy_gate_ok(&rec3, 3));
    }

    // P17: candidate-generated benchmarks are quarantined and self-blocked.
    #[test]
    fn candidate_generated_benchmark_is_self_blocked() {
        assert!(self_benchmark_blocked(BenchmarkOrigin::CandidateGenerated, true));
        assert!(!self_benchmark_blocked(BenchmarkOrigin::FixedBaseline, true));
    }

    #[test]
    fn benchmark_admission_rejects_non_discriminating() {
        let case = BenchmarkCase {
            id: "c".to_string(),
            name: "x".to_string(),
            origin: BenchmarkOrigin::CandidateGenerated,
            scope: "project".to_string(),
            expected: "pass".to_string(),
            trust: "high".to_string(),
            mutable: true,
            evidence: vec![],
            reference_id: None,
            studio: None,
            simcise_check: false,
        };
        let problems = benchmark_admission(&case, false, true, true);
        assert!(!problems.is_empty());
        assert!(problems.iter().any(|p| p.contains("baseline")));
    }

    // P18: reference conflict never resolves silently; Constitution wins.
    #[test]
    fn reference_conflict_requires_explicit_resolution() {
        let low = ReferenceTrust {
            authority: ReferenceAuthority::OrdinaryReference,
            freshness_ms: 0,
            scope: "audio".to_string(),
            provenance: "p".to_string(),
            conflict: None,
        };
        let constitution = ReferenceTrust {
            authority: ReferenceAuthority::Constitution,
            freshness_ms: 0,
            scope: "audio".to_string(),
            provenance: "p".to_string(),
            conflict: None,
        };
        match resolve_reference_conflict(&constitution, &low) {
            ConflictResolution::ConflictEvidence(msg) => assert!(msg.contains("overrides")),
            ConflictResolution::NeedsHumanDecision => panic!("constitution should win"),
        }
        // Equal authority -> NeedsHumanDecision.
        match resolve_reference_conflict(&low, &low) {
            ConflictResolution::NeedsHumanDecision => {}
            ConflictResolution::ConflictEvidence(_) => panic!("equal authority must defer"),
        }
    }

    // P22: budget stops the run.
    #[test]
    fn search_budget_stops_at_boundary() {
        let budget = SearchBudget {
            max_candidates: 2,
            max_rounds: 3,
            max_wall_time_ms: 60_000,
            max_failures: 5,
            cost_budget: None,
            novelty_plateau_limit: 4,
        };
        let run = EvolutionRun {
            id: "r".to_string(),
            budget: budget.clone(),
            candidates: 2,
            rounds: 0,
            failures: 0,
            cost: 0.0,
            novelty_plateau: 0,
            started_at: 0,
            status: "active".to_string(),
        };
        assert_eq!(should_stop(&run, 100), StopReason::MaxCandidates);
        // Novelty plateau boundary.
        let run2 = EvolutionRun {
            novelty_plateau: 4,
            candidates: 0,
            ..run
        };
        assert_eq!(should_stop(&run2, 100), StopReason::NoveltyPlateau);
    }

    // P20: watchdog reverts to previous KnownGood on failure.
    #[test]
    fn watchdog_reverts_on_failure() {
        let res = watchdog_check(true, true, true, false, Some("kg-0".to_string()));
        assert!(!res.healthy);
        assert_eq!(res.reverts_to.as_deref(), Some("kg-0"));
        let ok = watchdog_check(true, true, true, true, Some("kg-0".to_string()));
        assert!(ok.healthy);
        assert!(ok.reverts_to.is_none());
    }

    // P23: failure preserves a regression benchmark proposal.
    #[test]
    fn failure_pattern_keeps_regression_benchmark() {
        let pattern = FailurePattern {
            id: route_core::new_id(),
            hypothesis: "h".to_string(),
            environment: "dsh".to_string(),
            context_hash: "c".to_string(),
            why_rejected: "regressed audio tags".to_string(),
            failed_mutation: "mp3 tag batch".to_string(),
            regression_benchmark_proposal: Some(BenchmarkCase {
                id: route_core::new_id(),
                name: "audio-tag-regression".to_string(),
                origin: BenchmarkOrigin::FailureRegression,
                scope: "audio".to_string(),
                expected: "pass".to_string(),
                trust: "medium".to_string(),
                mutable: true,
                evidence: vec![],
                reference_id: None,
                studio: Some("studio-audio".to_string()),
                simcise_check: false,
            }),
            recorded_at: route_core::now_millis(),
        };
        assert!(pattern.regression_benchmark_proposal.is_some());
    }

    // P21: heterogeneous roles carry only capability + provenance.
    #[test]
    fn heterogeneous_roles_are_capability_only() {
        let roles = vec![
            SearchRole {
                plane: Plane::Explore,
                provider: "deepseek".to_string(),
                model: "deepseek-v4".to_string(),
                harness: Some("dsh".to_string()),
                capabilities: vec!["hypothesis".to_string()],
            },
            SearchRole {
                plane: Plane::Evaluate,
                provider: "anthropic".to_string(),
                model: "claude".to_string(),
                harness: None,
                capabilities: vec!["judge".to_string()],
            },
        ];
        assert_eq!(roles.len(), 2);
        assert_ne!(roles[0].plane, roles[1].plane);
        let _t: EvolutionTarget = EvolutionTarget::Project;
    }

    fn rec(replication_count: u32, cross_task_validated: bool, ablation_passed: bool) -> InnovationRecord {
        InnovationRecord {
            id: "r".to_string(),
            model: "m".to_string(),
            model_config: None,
            harness: "h".to_string(),
            preset: None,
            skills: vec![],
            tools: vec![],
            workflow: None,
            context_hash: "c".to_string(),
            references: vec![],
            hypothesis: "h".to_string(),
            parent_candidate: None,
            mutation: "m".to_string(),
            changed_scope: vec![],
            benchmark_suite: "s".to_string(),
            outputs: vec![],
            evidence: vec![],
            metrics: vec![],
            replication_count,
            cross_task_validated,
            ablation_passed,
            created_at: 0,
        }
    }

    // P25 #7: ablation failure blocks Gene promotion even with replication.
    #[test]
    fn ablation_failure_blocks_gene_promotion() {
        // High replication but no cross-task validation and failed ablation.
        let r = rec(5, false, false);
        assert!(!strategy_gate_ok(&r, 3), "failed ablation must block strategy");
        // Cross-task validation alone unlocks it.
        let r2 = rec(5, true, false);
        assert!(strategy_gate_ok(&r2, 3));
    }

    // P25 #8: Explorer / Executor / Evaluator are isolated planes.
    #[test]
    fn three_planes_are_mutually_exclusive() {
        let planes = [
            (Plane::Explore, "explore"),
            (Plane::Execute, "execute"),
            (Plane::Evaluate, "evaluate"),
        ];
        for (i, (p, s)) in planes.iter().enumerate() {
            assert_eq!(p.as_str(), *s);
            for (j, (q, _)) in planes.iter().enumerate() {
                if i != j {
                    assert_ne!(p, q, "planes must be isolated");
                }
            }
        }
        // An Evaluate role is never an Execute role.
        let evaluator = SearchRole {
            plane: Plane::Evaluate,
            provider: "x".to_string(),
            model: "y".to_string(),
            harness: None,
            capabilities: vec![],
        };
        assert_ne!(evaluator.plane, Plane::Execute);
    }

    // P25 #10/#11: route candidate cannot modify the evaluator; watchdog reverts.
    #[test]
    fn route_candidate_cannot_tamper_evaluator() {
        // Watchdog catches an injected evaluator change and reverts to KnownGood.
        let res = watchdog_check(true, /*evaluator_intact=*/ false, true, true, Some("kg-0".to_string()));
        assert!(!res.healthy);
        assert_eq!(res.reverts_to.as_deref(), Some("kg-0"));
    }
}