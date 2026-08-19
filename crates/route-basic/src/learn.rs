//! Adaptive Learning v0 — Outcome → Evidence → Learned Reference → Future Context.
//!
//! Route accumulates "what works for this project/user" without letting
//! any single event become a permanent rule. The chain is:
//!
//!   ExperienceEvent → LearningProposal → ReferenceEntry (type=Experience)
//!                                       → Context injection
//!
//! Key boundaries:
//!   - No LLM calls / vector DB / RAG
//!   - No automatic Constitution or Protocol modification
//!   - Events are deterministic system records, never AI-inferred "user likes X"
//!   - Single events never become rules; only multi-evidence aggregation
//!   - Scope is always narrow by default

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{
    write_atomic, ContextHistoryManifest, ContextSnapshot, LearnedMeta, Origin, ReferenceEntry,
    ReferenceRegistry, ReferenceType,
};

// ---------------------------------------------------------------------------
// Directory layout
// ---------------------------------------------------------------------------

fn learn_dir(project_root: &Path) -> PathBuf {
    project_root.join(".route").join("learn")
}

fn events_path(project_root: &Path) -> PathBuf {
    learn_dir(project_root).join("events.json")
}

fn proposals_path(project_root: &Path) -> PathBuf {
    learn_dir(project_root).join("proposals.json")
}

// ---------------------------------------------------------------------------
// P0: ExperienceEvent
// ---------------------------------------------------------------------------

/// The kind of an experience event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// User explicitly accepted a proposal or result.
    UserAccept,
    /// User explicitly rejected a proposal or result.
    UserReject,
    /// A rollback occurred (user undid their work).
    Rollback,
    /// Tests passed.
    TestPass,
    /// Tests failed.
    TestFail,
    /// An agent (AI) reported a result.
    AgentResult,
    /// Manual note recorded by the user via `route learn record --note`.
    ManualNote,
    /// A reference proposal was approved.
    ProposalApproved,
    /// A reference proposal was rejected.
    ProposalRejected,
    /// A learning proposal was approved.
    LearnApproved,
    /// A learning proposal was rejected.
    LearnRejected,
    /// A repair attempt was started.
    RepairAttempt,
    /// A repair attempt succeeded (verification passed).
    RepairSucceeded,
    /// A repair attempt failed (verification failed).
    RepairFailed,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::UserAccept => "user_accept",
            EventKind::UserReject => "user_reject",
            EventKind::Rollback => "rollback",
            EventKind::TestPass => "test_pass",
            EventKind::TestFail => "test_fail",
            EventKind::AgentResult => "agent_result",
            EventKind::ManualNote => "manual_note",
            EventKind::ProposalApproved => "proposal_approved",
            EventKind::ProposalRejected => "proposal_rejected",
            EventKind::LearnApproved => "learn_approved",
            EventKind::LearnRejected => "learn_rejected",
            EventKind::RepairAttempt => "repair_attempt",
            EventKind::RepairSucceeded => "repair_succeeded",
            EventKind::RepairFailed => "repair_failed",
        }
    }
}

/// The scope of an experience event or learned reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LearnScope {
    /// Applies to the entire project.
    #[default]
    Project,
    /// Applies to a specific task pattern (e.g. "rollback", "testing").
    TaskPattern(String),
    /// Applies to a specific tool or host (e.g. "claude", "codex", "cargo").
    ToolOrHost(String),
}

impl LearnScope {
    pub fn as_str(&self) -> &str {
        match self {
            LearnScope::Project => "project",
            LearnScope::TaskPattern(_) => "task-pattern",
            LearnScope::ToolOrHost(_) => "tool-or-host",
        }
    }
}

/// A single recorded experience event.
///
/// Design: `outcome` is never AI-inferred. It records only explicit
/// system events or user feedback. The `evidence` field is a free-text
/// description of what happened (e.g. "user accepted rollback fix",
/// "tests passed after applying patch").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceEvent {
    /// Stable event id (ULID).
    pub id: String,
    /// Optional task description this event relates to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Context hash at the time of the event (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
    /// Optional savepoint ID this event is linked to.
    ///
    /// Used to connect learning evidence to a specific development state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub savepoint_id: Option<String>,
    /// What kind of event occurred.
    pub kind: EventKind,
    /// Human-readable description of the outcome.
    pub outcome: String,
    /// Free-text supporting evidence.
    #[serde(default)]
    pub evidence: String,
    /// Scope of this event.
    #[serde(default)]
    pub scope: LearnScope,
    /// Optional tags for grouping/filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unix-millis when the event was recorded.
    pub created_at: i64,
    /// Optional provenance: who/what triggered this event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

// ---------------------------------------------------------------------------
// ExperienceStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExperienceStore {
    #[serde(default)]
    pub events: Vec<ExperienceEvent>,
}

impl ExperienceStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = events_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading experience store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = events_path(project_root);
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing experience store to {}", p.display()))
    }

    /// Record a new event. Returns the event id.
    pub fn record(
        &mut self,
        _project_root: &Path,
        kind: EventKind,
        outcome: &str,
        evidence: &str,
        scope: LearnScope,
        task: Option<&str>,
        tags: Vec<String>,
        context_hash: Option<String>,
        provenance: Option<String>,
    ) -> Result<String> {
        let id = route_core::new_id();
        let event = ExperienceEvent {
            id: id.clone(),
            task: task.map(|s| s.to_string()),
            context_hash,
            savepoint_id: None,
            kind,
            outcome: outcome.to_string(),
            evidence: evidence.to_string(),
            scope,
            tags,
            created_at: route_core::now_millis(),
            provenance,
        };
        self.events.push(event);
        Ok(id)
    }

    /// Find events matching a predicate.
    pub fn filter<F>(&self, f: F) -> Vec<&ExperienceEvent>
    where
        F: Fn(&ExperienceEvent) -> bool,
    {
        self.events.iter().filter(|e| f(e)).collect()
    }

    /// Find events by kind.
    pub fn by_kind(&self, kind: EventKind) -> Vec<&ExperienceEvent> {
        self.filter(|e| e.kind == kind)
    }

    /// Find events by scope.
    pub fn by_scope(&self, scope: &LearnScope) -> Vec<&ExperienceEvent> {
        self.filter(|e| e.scope == *scope)
    }

    /// Find events matching a task pattern (substring match).
    pub fn by_task(&self, task: &str) -> Vec<&ExperienceEvent> {
        let lower = task.to_lowercase();
        self.filter(|e| {
            e.task
                .as_ref()
                .map(|t| t.to_lowercase().contains(&lower))
                .unwrap_or(false)
        })
    }
}

// ---------------------------------------------------------------------------
// P2: LearningProposal
// ---------------------------------------------------------------------------

/// A candidate learning, generated by deterministic aggregation of
/// similar experience events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningProposal {
    /// Stable proposal id (ULID).
    pub id: String,
    /// The claim / observation (e.g. "rollback fix is preferred for undo").
    pub claim: String,
    /// Scope of the proposed learning.
    pub scope: LearnScope,
    /// IDs of supporting events.
    pub supporting_event_ids: Vec<String>,
    /// Number of positive evidence events.
    pub positive: u32,
    /// Number of negative evidence events.
    pub negative: u32,
    /// Confidence in [0.0, 1.0).
    pub confidence: f32,
    /// Human-readable reason.
    pub reason: String,
    /// Unix-millis when this proposal was generated.
    pub created_at: i64,
    /// Status of this proposal.
    #[serde(default)]
    pub status: ProposalStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    #[default]
    Open,
    Approved,
    Rejected,
}

// ---------------------------------------------------------------------------
// LearningProposalStore
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LearningProposalStore {
    #[serde(default)]
    pub proposals: Vec<LearningProposal>,
}

impl LearningProposalStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = proposals_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading learning proposals from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = proposals_path(project_root);
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing learning proposals to {}", p.display()))
    }

    pub fn open_proposals(&self) -> Vec<&LearningProposal> {
        self.proposals
            .iter()
            .filter(|p| p.status == ProposalStatus::Open)
            .collect()
    }

    pub fn find_open_by_id(&self, id: &str) -> Option<&LearningProposal> {
        self.proposals
            .iter()
            .find(|p| p.id == id && p.status == ProposalStatus::Open)
    }

    pub fn take_by_id(&mut self, id: &str) -> Option<LearningProposal> {
        let i = self.proposals.iter().position(|p| p.id == id)?;
        let p = self.proposals.remove(i);
        Some(p)
    }

    pub fn upsert(&mut self, proposal: LearningProposal) {
        if let Some(existing) = self.proposals.iter_mut().find(|p| p.id == proposal.id) {
            *existing = proposal;
        } else {
            self.proposals.push(proposal);
        }
    }

    /// Check if there's already an open proposal with a similar claim
    /// (exact match on claim text). Prevents duplicate proposals.
    pub fn has_open_with_claim(&self, claim: &str) -> bool {
        self.proposals
            .iter()
            .any(|p| p.claim == claim && p.status == ProposalStatus::Open)
    }

    /// Check if there's a rejected proposal with this claim.
    /// Prevents re-proposing the same pattern after user rejection.
    pub fn has_rejected_with_claim(&self, claim: &str) -> bool {
        self.proposals
            .iter()
            .any(|p| p.claim == claim && p.status == ProposalStatus::Rejected)
    }
}

// ---------------------------------------------------------------------------
// P2: Analysis — deterministic aggregation
// ---------------------------------------------------------------------------

/// Analyze events and generate learning proposals.
///
/// Rules:
///   - Single event → never a proposal
///   - Must have at least 2 events with the same task pattern or scope
///   - Positive and negative evidence both considered
///   - Conflicting evidence lowers confidence
///   - No LLM calls
pub fn analyze_events(project_root: &Path) -> Result<Vec<LearningProposal>> {
    let store = ExperienceStore::load(project_root)?;
    let mut proposals: Vec<LearningProposal> = Vec::new();

    if store.events.len() < 2 {
        return Ok(proposals);
    }

    // Group events by task pattern (primary grouping key).
    // Events without a task are grouped by scope.
    let mut groups: Vec<(String, Vec<&ExperienceEvent>)> = Vec::new();

    for event in &store.events {
        let key = event
            .task
            .as_deref()
            .map(|t| t.to_lowercase())
            .unwrap_or_else(|| format!("scope:{}", event.scope.as_str()));

        if let Some((_, group)) = groups.iter_mut().find(|(k, _)| k == &key) {
            group.push(event);
        } else {
            groups.push((key, vec![event]));
        }
    }

    let now = route_core::now_millis();

    for (key, group) in &groups {
        if group.len() < 2 {
            continue;
        }

        // Count positive and negative evidence.
        let positive = group
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    EventKind::UserAccept
                        | EventKind::TestPass
                        | EventKind::ProposalApproved
                        | EventKind::LearnApproved
                        | EventKind::AgentResult
                )
            })
            .count() as u32;

        let negative = group
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    EventKind::UserReject
                        | EventKind::TestFail
                        | EventKind::Rollback
                        | EventKind::ProposalRejected
                        | EventKind::LearnRejected
                )
            })
            .count() as u32;

        // Must have at least some positive evidence for a proposal.
        if positive == 0 {
            continue;
        }

        // Compute confidence: proportional to the positive ratio, then
        // penalized by each negative event. With mostly-negative evidence
        // the confidence drops below 0.3, suppressing the proposal.
        let total = positive + negative;
        let base = (positive as f32 / total as f32) * 0.8;
        let negative_penalty = negative as f32 * 0.1;
        let confidence = (base - negative_penalty).clamp(0.0, 0.99);

        if confidence < 0.3 {
            continue; // Too low confidence → no proposal.
        }

        // Build claim from the task/scope.
        let claim = if let Some(stripped) = key.strip_prefix("scope:") {
            format!("Preference pattern for {}", stripped)
        } else {
            format!("Preference pattern for task '{}'", key)
        };

        let event_ids: Vec<String> = group.iter().map(|e| e.id.clone()).collect();

        let reason = format!(
            "{} events analyzed: {} positive, {} negative. Confidence: {:.2}",
            total, positive, negative, confidence
        );

        // Determine scope: use the narrowest common scope.
        let scope = determine_scope(group);

        // Only create a proposal if one doesn't already exist for this claim.
        let mut existing_store = LearningProposalStore::load(project_root)?;
        if existing_store.has_open_with_claim(&claim) {
            // Update the existing proposal instead of creating a duplicate.
            if let Some(existing) = existing_store
                .proposals
                .iter_mut()
                .find(|p| p.claim == claim && p.status == ProposalStatus::Open)
            {
                existing.supporting_event_ids = event_ids;
                existing.positive = positive;
                existing.negative = negative;
                existing.confidence = confidence;
                existing.reason = reason.clone();
                existing.created_at = now;
            }
            existing_store.save(project_root)?;
            continue;
        }

        // Skip if this claim was already rejected — prevents repeated
        // proposals for the same pattern after user rejection.
        if existing_store.has_rejected_with_claim(&claim) {
            continue;
        }

        let proposal = LearningProposal {
            id: route_core::new_id(),
            claim,
            scope,
            supporting_event_ids: event_ids,
            positive,
            negative,
            confidence,
            reason,
            created_at: now,
            status: ProposalStatus::Open,
        };

        proposals.push(proposal);
    }

    // Persist new proposals.
    if !proposals.is_empty() {
        let mut store = LearningProposalStore::load(project_root)?;
        for p in proposals.iter() {
            store.upsert(p.clone());
        }
        store.save(project_root)?;
    }

    // Reload to get the full list including updated ones.
    let store = LearningProposalStore::load(project_root)?;
    Ok(store.open_proposals().into_iter().cloned().collect())
}

/// Determine the narrowest common scope from a group of events.
fn determine_scope(events: &[&ExperienceEvent]) -> LearnScope {
    // If any event has a specific task scope, prefer that.
    for event in events {
        if let LearnScope::TaskPattern(t) = &event.scope {
            if !t.is_empty() {
                return LearnScope::TaskPattern(t.clone());
            }
        }
        if let LearnScope::ToolOrHost(t) = &event.scope {
            if !t.is_empty() {
                return LearnScope::ToolOrHost(t.clone());
            }
        }
    }
    LearnScope::Project
}

// ---------------------------------------------------------------------------
// P3: Promotion — apply a learning proposal
// ---------------------------------------------------------------------------

/// Apply a learning proposal: promote it to a ReferenceEntry with
/// type=Experience, origin=Generated.
pub fn apply_learning_proposal(project_root: &Path, proposal_id: &str) -> Result<LearningProposal> {
    let mut store = LearningProposalStore::load(project_root)?;
    let mut proposal = store
        .take_by_id(proposal_id)
        .ok_or_else(|| anyhow::anyhow!("learning proposal '{}' not found", proposal_id))?;

    if proposal.status != ProposalStatus::Open {
        return Err(anyhow::anyhow!(
            "proposal '{}' is not open (status: {:?})",
            proposal_id,
            proposal.status
        ));
    }

    // Create a ReferenceEntry with type=Experience.
    let now = route_core::now_millis();
    let description = format!(
        "Learned: {} (confidence: {:.2}, scope: {})",
        proposal.claim,
        proposal.confidence,
        proposal.scope.as_str()
    );

    let capabilities = format!(
        "Learned preference from {} events ({} positive, {} negative). Claim: {}",
        proposal.positive + proposal.negative,
        proposal.positive,
        proposal.negative,
        proposal.claim
    );

    let constraints = format!(
        "Scope: {}. Confidence: {:.2}. This is a learned pattern, not a hard rule. \
         It may be superseded by future evidence.",
        proposal.scope.as_str(),
        proposal.confidence
    );

    let tags: Vec<String> = vec!["learned".to_string(), proposal.scope.as_str().to_string()];

    let learned_meta = LearnedMeta {
        confidence: proposal.confidence,
        scope: proposal.scope.as_str().to_string(),
        last_confirmed: now,
        stale: false,
        superseded_by: None,
        evidence_ids: proposal.supporting_event_ids.clone(),
    };

    let entry = ReferenceEntry::builder(
        format!("learn-{}", &proposal.id[..12]),
        ReferenceType::Experience,
        "learned",
        &description,
    )
    .with_capabilities(&capabilities)
    .with_constraints(&constraints)
    .with_tags(tags)
    .with_created_at(now)
    .with_origin(Origin::Generated)
    .with_learned_meta(learned_meta)
    .build();

    // Register in the Reference registry.
    let mut registry = ReferenceRegistry::read(project_root)?;
    registry.upsert(entry);
    registry.write(project_root)?;

    // Mark proposal as approved.
    proposal.status = ProposalStatus::Approved;
    store.upsert(proposal.clone());
    store.save(project_root)?;

    // Record a learning-approved event.
    let mut event_store = ExperienceStore::load(project_root)?;
    event_store.record(
        project_root,
        EventKind::LearnApproved,
        &format!(
            "Learning proposal '{}' approved and promoted to Experience Reference",
            proposal.id
        ),
        &proposal.reason,
        proposal.scope.clone(),
        Some(&proposal.claim),
        vec![],
        None,
        Some("system:learn_apply".to_string()),
    )?;

    // Record in experience history manifest.
    let trigger = format!("learn_apply:{}", proposal.id);
    if let Ok(snap) = ContextSnapshot::collect(project_root) {
        let _ =
            ContextHistoryManifest::record_if_new_triggered(project_root, &snap, Some(&trigger));
    }

    event_store.save(project_root)?;

    Ok(proposal)
}

/// Reject a learning proposal. Records negative evidence so the same
/// proposal is not regenerated.
pub fn reject_learning_proposal(
    project_root: &Path,
    proposal_id: &str,
    reason: &str,
) -> Result<LearningProposal> {
    let mut store = LearningProposalStore::load(project_root)?;
    let mut proposal = store
        .take_by_id(proposal_id)
        .ok_or_else(|| anyhow::anyhow!("learning proposal '{}' not found", proposal_id))?;

    if proposal.status != ProposalStatus::Open {
        return Err(anyhow::anyhow!(
            "proposal '{}' is not open (status: {:?})",
            proposal_id,
            proposal.status
        ));
    }

    proposal.status = ProposalStatus::Rejected;
    store.upsert(proposal.clone());
    store.save(project_root)?;

    // Record a rejection event so the claim won't be re-proposed.
    let mut event_store = ExperienceStore::load(project_root)?;
    event_store.record(
        project_root,
        EventKind::LearnRejected,
        &format!("Learning proposal '{}' rejected: {}", proposal.id, reason),
        reason,
        proposal.scope.clone(),
        Some(&proposal.claim),
        vec![],
        None,
        Some("system:learn_reject".to_string()),
    )?;
    event_store.save(project_root)?;

    Ok(proposal)
}

// ---------------------------------------------------------------------------
// P5: Decay / Conflict
// ---------------------------------------------------------------------------

/// Recompute confidence for a learned reference based on new evidence.
/// This is called implicitly when new events are recorded.
pub fn recompute_confidence(project_root: &Path, reference_id: &str) -> Result<f32> {
    let mut registry = ReferenceRegistry::read(project_root)?;

    // Find the entry
    let entry_idx = registry
        .entries
        .iter()
        .position(|e| e.id == reference_id && e.type_ == ReferenceType::Experience);
    let entry_idx = match entry_idx {
        Some(i) => i,
        None => {
            return Err(anyhow::anyhow!(
                "no Experience reference '{}' found",
                reference_id
            ));
        }
    };

    // Extract claim from capabilities
    let claim = registry.entries[entry_idx]
        .capabilities
        .lines()
        .find(|l| l.starts_with("Claim: "))
        .map(|l| l.trim_start_matches("Claim: "))
        .unwrap_or_default()
        .to_string();

    // Find matching events
    let event_store = ExperienceStore::load(project_root)?;
    let matching: Vec<&ExperienceEvent> = event_store
        .events
        .iter()
        .filter(|e| {
            e.task
                .as_ref()
                .map(|t| t.to_lowercase().contains(&claim.to_lowercase()))
                .unwrap_or(false)
        })
        .collect();

    let positive = matching
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::UserAccept
                    | EventKind::TestPass
                    | EventKind::ProposalApproved
                    | EventKind::LearnApproved
                    | EventKind::AgentResult
            )
        })
        .count() as u32;

    let negative = matching
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::UserReject
                    | EventKind::TestFail
                    | EventKind::Rollback
                    | EventKind::ProposalRejected
                    | EventKind::LearnRejected
            )
        })
        .count() as u32;

    let total = positive + negative;
    let confidence = if total == 0 {
        // Return existing confidence from LearnedMeta
        registry.entries[entry_idx]
            .learned_meta
            .as_ref()
            .map(|m| m.confidence)
            .unwrap_or(0.5)
    } else {
        let base = (positive as f32 / total as f32) * 0.8;
        let negative_penalty = negative as f32 * 0.15;
        (base - negative_penalty).clamp(0.0, 0.99)
    };

    let now = route_core::now_millis();

    // Extract scope from tags before any mutable borrow
    let scope_str = registry.entries[entry_idx]
        .tags
        .iter()
        .find(|t| *t == "project" || *t == "task-pattern" || *t == "tool-or-host")
        .map(|s| s.as_str())
        .unwrap_or("project")
        .to_string();

    // Update LearnedMeta
    let meta = registry.entries[entry_idx]
        .learned_meta
        .get_or_insert_with(|| LearnedMeta {
            confidence,
            scope: scope_str.clone(),
            last_confirmed: now,
            stale: false,
            superseded_by: None,
            evidence_ids: Vec::new(),
        });
    meta.confidence = confidence;
    meta.last_confirmed = now;
    meta.stale = confidence < 0.3;
    meta.scope = scope_str.clone();

    // Update constraints for backward compatibility
    registry.entries[entry_idx].constraints = format!(
        "Scope: {}. Confidence: {:.2}. This is a learned pattern, not a hard rule. \
         It may be superseded by future evidence. last_confirmed={}",
        scope_str, confidence, now
    );

    registry.write(project_root)?;

    // Record context history
    let trigger = format!("learn_recompute:{}", reference_id);
    if let Ok(snap) = ContextSnapshot::collect(project_root) {
        let _ =
            ContextHistoryManifest::record_if_new_triggered(project_root, &snap, Some(&trigger));
    }

    Ok(confidence)
}

/// Check if a learned reference is stale.
pub fn is_stale(entry: &ReferenceEntry) -> bool {
    // Check structured LearnedMeta first
    if let Some(ref meta) = entry.learned_meta {
        return meta.stale || meta.superseded_by.is_some() || meta.confidence < 0.3;
    }
    // Legacy fallback: check tags and constraints
    entry.tags.contains(&"stale".to_string())
        || entry.tags.contains(&"superseded".to_string())
        || extract_legacy_confidence(&entry.constraints) < 0.3
}

/// Extract confidence from legacy constraints text (e.g., "Confidence: 0.80").
fn extract_legacy_confidence(constraints: &str) -> f32 {
    constraints
        .lines()
        .find(|l| l.contains("Confidence:"))
        .and_then(|l| {
            l.split("Confidence:").nth(1).and_then(|s| {
                s.split_whitespace()
                    .next()
                    .and_then(|tok| tok.trim_end_matches('.').parse::<f32>().ok())
            })
        })
        .unwrap_or(0.0)
}

/// A scored learned experience for context injection.
#[derive(Debug, Clone, Serialize)]
pub struct ScoredLearnedExperience {
    pub id: String,
    pub description: String,
    pub capabilities: String,
    pub claim: String,
    pub confidence: f32,
    pub score: f32,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// P8: Audit — full history of a learning lifecycle
// ---------------------------------------------------------------------------

/// A single audit entry in the learning lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: i64,
    pub kind: AuditKind,
    pub detail: String,
    pub event_id: Option<String>,
    pub proposal_id: Option<String>,
    pub reference_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    EventRecorded,
    ProposalGenerated,
    ProposalApproved,
    ProposalRejected,
    ReferenceCreated,
    ConfidenceRecomputed,
    MarkedStale,
    Superseded,
}

/// The full audit trail for a learning lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningAudit {
    /// The reference id (if promoted).
    pub reference_id: Option<String>,
    /// The proposal id (if any).
    pub proposal_id: Option<String>,
    /// The claim / observation.
    pub claim: String,
    /// All audit entries in chronological order.
    pub entries: Vec<AuditEntry>,
}

/// Build an audit trail for a learning claim or proposal id.
pub fn build_audit(project_root: &Path, claim_or_id: &str) -> Result<LearningAudit> {
    let event_store = ExperienceStore::load(project_root)?;
    let proposal_store = LearningProposalStore::load(project_root)?;
    let registry = ReferenceRegistry::read(project_root)?;

    let mut audit = LearningAudit {
        reference_id: None,
        proposal_id: None,
        claim: String::new(),
        entries: Vec::new(),
    };

    // Find matching events.
    let matching_events: Vec<&ExperienceEvent> = event_store
        .events
        .iter()
        .filter(|e| {
            e.id == claim_or_id
                || e.task
                    .as_ref()
                    .map(|t| t.contains(claim_or_id))
                    .unwrap_or(false)
                || e.outcome.contains(claim_or_id)
        })
        .collect();

    if matching_events.is_empty() {
        // Try as proposal id.
        if let Some(proposal) = proposal_store
            .proposals
            .iter()
            .find(|p| p.id == claim_or_id)
        {
            audit.claim = proposal.claim.clone();
            audit.proposal_id = Some(proposal.id.clone());
            audit.entries.push(AuditEntry {
                timestamp: proposal.created_at,
                kind: AuditKind::ProposalGenerated,
                detail: format!("Claim: {}. Reason: {}", proposal.claim, proposal.reason),
                event_id: None,
                proposal_id: Some(proposal.id.clone()),
                reference_id: None,
            });
        }
        return Ok(audit);
    }

    // Set claim from the first matching event.
    if let Some(event) = matching_events.first() {
        audit.claim = event.task.clone().unwrap_or_else(|| event.outcome.clone());
    }

    for event in &matching_events {
        audit.entries.push(AuditEntry {
            timestamp: event.created_at,
            kind: match event.kind {
                EventKind::LearnApproved => AuditKind::ProposalApproved,
                EventKind::LearnRejected => AuditKind::ProposalRejected,
                EventKind::ProposalApproved => AuditKind::ProposalApproved,
                EventKind::ProposalRejected => AuditKind::ProposalRejected,
                _ => AuditKind::EventRecorded,
            },
            detail: format!(
                "[{}] {} — {}",
                event.kind.as_str(),
                event.outcome,
                event.evidence
            ),
            event_id: Some(event.id.clone()),
            proposal_id: None,
            reference_id: None,
        });
    }

    // Find matching proposals.
    let matching_proposals: Vec<&LearningProposal> = proposal_store
        .proposals
        .iter()
        .filter(|p| {
            p.claim.contains(&audit.claim)
                || p.supporting_event_ids
                    .iter()
                    .any(|eid| matching_events.iter().any(|me| me.id == *eid))
        })
        .collect();

    for proposal in &matching_proposals {
        if audit.proposal_id.is_none() {
            audit.proposal_id = Some(proposal.id.clone());
        }
        audit.entries.push(AuditEntry {
            timestamp: proposal.created_at,
            kind: AuditKind::ProposalGenerated,
            detail: format!(
                "Proposal: {} (confidence: {:.2}, positive: {}, negative: {})",
                proposal.claim, proposal.confidence, proposal.positive, proposal.negative
            ),
            event_id: None,
            proposal_id: Some(proposal.id.clone()),
            reference_id: None,
        });

        if proposal.status == ProposalStatus::Approved {
            // Find the corresponding reference.
            let ref_id = format!("learn-{}", &proposal.id[..12]);
            if let Some(entry) = registry.entries.iter().find(|e| e.id == ref_id) {
                audit.reference_id = Some(entry.id.clone());
                audit.entries.push(AuditEntry {
                    timestamp: entry.created_at,
                    kind: AuditKind::ReferenceCreated,
                    detail: format!("Reference created: {} — {}", entry.id, entry.description),
                    event_id: None,
                    proposal_id: Some(proposal.id.clone()),
                    reference_id: Some(entry.id.clone()),
                });
            }
        }
    }

    // Sort entries by timestamp.
    audit.entries.sort_by_key(|a| a.timestamp);

    Ok(audit)
}

// ---------------------------------------------------------------------------
// P7: Host feedback contract — rendered in managed blocks
// ---------------------------------------------------------------------------

/// Render the vendor-neutral feedback contract section for managed blocks.
pub fn render_feedback_contract() -> &'static str {
    "## Route Feedback Contract\n\n\
     You (the AI) may report structured observations about what worked \
     or did not work during this session. Use the following format:\n\n\
     ```\n\
     route_feedback_candidate:\n\
       task: \"<brief description of the task>\"\n\
       result: \"<what happened — accept/reject/error/success>\"\n\
       observation: \"<what you observed about the outcome>\"\n\
       evidence: \"<supporting detail: file changed, test passed, etc.>\"\n\
       context_hash: \"<the context fingerprint for this session>\"\n\
       selected_reference_ids: \"<comma-separated ids of selected refs>\"\n\
       agent_roles_used: \"<roles used, if any>\"\n\
       verification_result: \"<pass/fail/unknown>\"\n\
     ```\n\n\
     Constraints:\n\
     - You MUST NOT generate \"user preference facts\" or claim user intent.\n\
     - You MUST NOT self-apply LearningProposals.\n\
     - Each candidate is just a suggestion. Route Engine validates the\n\
       source and records it as an ExperienceEvent after verification.\n\
     - Single observations never become rules. Only multi-evidence\n\
       aggregation (via `route learn analyze`) can produce a proposal.\n\n\
     ---\n\n"
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn init_project() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        // Create .route directory
        std::fs::create_dir_all(root.join(".route")).unwrap();
        // Init constitution + protocol — write defaults
        let constitution = crate::constitutive::Constitution::default();
        constitution.write(&root).unwrap();
        let mut protocol = crate::constitutive::Protocol::default();
        protocol.write(&root).unwrap();
        // Ensure reference registry exists
        crate::constitutive::ReferenceRegistry::ensure_exists(&root).unwrap();
        (tmp, root)
    }

    #[test]
    fn single_event_no_promotion() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        store
            .record(
                &root,
                EventKind::UserAccept,
                "user accepted rollback fix",
                "rollback worked correctly",
                LearnScope::TaskPattern("rollback".into()),
                Some("fix rollback corruption"),
                vec![],
                None,
                None,
            )
            .unwrap();
        store.save(&root).unwrap();

        // Analyze should produce no proposals (only 1 event).
        let proposals = analyze_events(&root).unwrap();
        assert!(
            proposals.is_empty(),
            "single event must not produce a proposal"
        );
    }

    #[test]
    fn multi_evidence_proposal() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // Record 3 user_accept events for the same task.
        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("user accepted proposal #{}", i + 1),
                    "evidence of acceptance",
                    LearnScope::TaskPattern("testing".into()),
                    Some("run tests"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        assert!(!proposals.is_empty(), "3 events must produce a proposal");
        assert_eq!(proposals[0].positive, 3);
        assert_eq!(proposals[0].negative, 0);
        assert!(proposals[0].confidence >= 0.5);
    }

    #[test]
    fn conflicting_evidence_lowers_confidence() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // 2 accept + 1 reject.
        for i in 0..2 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("accept #{}", i + 1),
                    "evidence",
                    LearnScope::TaskPattern("conflict-test".into()),
                    Some("test conflicting evidence"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store
            .record(
                &root,
                EventKind::UserReject,
                "user rejected",
                "user explicitly said no",
                LearnScope::TaskPattern("conflict-test".into()),
                Some("test conflicting evidence"),
                vec![],
                None,
                None,
            )
            .unwrap();
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        assert!(!proposals.is_empty());
        assert_eq!(proposals[0].positive, 2);
        assert_eq!(proposals[0].negative, 1);
        // Confidence should be lower than a pure-positive case.
        assert!(
            proposals[0].confidence < 0.8,
            "conflicting evidence should lower confidence"
        );
    }

    #[test]
    fn reject_prevents_repeated_proposal() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // 3 events to trigger a proposal.
        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("accept #{}", i + 1),
                    "evidence",
                    LearnScope::TaskPattern("reject-test".into()),
                    Some("test reject"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        assert_eq!(proposals.len(), 1);

        // Reject the proposal.
        let rejected = reject_learning_proposal(&root, &proposals[0].id, "not useful").unwrap();
        assert_eq!(rejected.status, ProposalStatus::Rejected);

        // Analyze again — should not produce the same proposal.
        let proposals2 = analyze_events(&root).unwrap();
        let same_claim = proposals2.iter().find(|p| p.claim == proposals[0].claim);
        assert!(
            same_claim.is_none(),
            "rejected proposal must not be re-generated"
        );
    }

    #[test]
    fn scope_isolation() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // Events in different scopes.
        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("accept #{}", i + 1),
                    "evidence",
                    LearnScope::ToolOrHost("claude".into()),
                    Some("scope-a"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("accept #{}", i + 1),
                    "evidence",
                    LearnScope::ToolOrHost("codex".into()),
                    Some("scope-b"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        // Should produce 2 proposals (one per scope).
        assert_eq!(
            proposals.len(),
            2,
            "events in different scopes must produce separate proposals"
        );
    }

    #[test]
    fn rollback_negative_event() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // 2 positive + 1 rollback (negative).
        store
            .record(
                &root,
                EventKind::UserAccept,
                "accepted",
                "good",
                LearnScope::TaskPattern("rollback-neg".into()),
                Some("test rollback negative"),
                vec![],
                None,
                None,
            )
            .unwrap();
        store
            .record(
                &root,
                EventKind::UserAccept,
                "accepted",
                "good",
                LearnScope::TaskPattern("rollback-neg".into()),
                Some("test rollback negative"),
                vec![],
                None,
                None,
            )
            .unwrap();
        store
            .record(
                &root,
                EventKind::Rollback,
                "user rolled back",
                "user undid the change",
                LearnScope::TaskPattern("rollback-neg".into()),
                Some("test rollback negative"),
                vec![],
                None,
                None,
            )
            .unwrap();
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        assert!(!proposals.is_empty());
        assert_eq!(proposals[0].positive, 2);
        assert_eq!(proposals[0].negative, 1);
        assert!(
            proposals[0].confidence < 0.8,
            "negative evidence should reduce confidence"
        );
    }

    #[test]
    fn apply_creates_context_history_node() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("accept #{}", i + 1),
                    "evidence",
                    LearnScope::TaskPattern("history-test".into()),
                    Some("test history"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        assert_eq!(proposals.len(), 1);

        // Apply the proposal.
        let applied = apply_learning_proposal(&root, &proposals[0].id).unwrap();
        assert_eq!(applied.status, ProposalStatus::Approved);

        // Check the registry has the new Experience entry.
        let registry = ReferenceRegistry::read(&root).unwrap();
        let experience_entry = registry
            .entries
            .iter()
            .find(|e| e.type_ == ReferenceType::Experience);
        assert!(
            experience_entry.is_some(),
            "Experience reference must exist after apply"
        );

        // Check context history has the learn_apply trigger.
        let manifests = ContextHistoryManifest::list_all(&root).unwrap();
        let has_trigger = manifests
            .iter()
            .any(|m| m.trigger.as_deref().unwrap_or("").contains("learn_apply:"));
        assert!(
            has_trigger,
            "context history must record learn_apply trigger"
        );
    }

    #[test]
    fn low_confidence_stale_excluded() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // 1 positive + 3 reject → low confidence.
        store
            .record(
                &root,
                EventKind::UserAccept,
                "accepted once",
                "positive",
                LearnScope::TaskPattern("stale-test".into()),
                Some("test stale"),
                vec![],
                None,
                None,
            )
            .unwrap();
        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserReject,
                    &format!("rejected #{}", i + 1),
                    "negative",
                    LearnScope::TaskPattern("stale-test".into()),
                    Some("test stale"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store.save(&root).unwrap();

        let proposals = analyze_events(&root).unwrap();
        // Should be empty because confidence < 0.3.
        assert!(
            proposals.is_empty(),
            "low confidence must not produce a proposal"
        );
    }

    #[test]
    fn ai_feedback_cannot_bypass_proposal() {
        // AI feedback is just a candidate — it must go through
        // the normal event → analyze → proposal → apply pipeline.
        // This test verifies that recording a single feedback event
        // does NOT create a LearningProposal.

        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // Record a single AI feedback candidate.
        store
            .record(
                &root,
                EventKind::AgentResult,
                "agent reported test pass",
                "all tests passed after refactoring",
                LearnScope::TaskPattern("refactoring".into()),
                Some("refactor module"),
                vec![],
                None,
                Some("ai:claude".to_string()),
            )
            .unwrap();
        store.save(&root).unwrap();

        // Analyze should produce nothing (only 1 event).
        let proposals = analyze_events(&root).unwrap();
        assert!(
            proposals.is_empty(),
            "single AI feedback event must not produce a proposal"
        );
    }

    #[test]
    fn audit_trail_full_lifecycle() {
        let (_tmp, root) = init_project();
        let mut store = ExperienceStore::load(&root).unwrap();

        // Record events.
        for i in 0..3 {
            store
                .record(
                    &root,
                    EventKind::UserAccept,
                    &format!("accept #{}", i + 1),
                    "evidence",
                    LearnScope::TaskPattern("audit-test".into()),
                    Some("test audit trail"),
                    vec![],
                    None,
                    None,
                )
                .unwrap();
        }
        store.save(&root).unwrap();

        // Analyze → proposal.
        let proposals = analyze_events(&root).unwrap();
        assert_eq!(proposals.len(), 1);
        let proposal_id = proposals[0].id.clone();

        // Apply.
        apply_learning_proposal(&root, &proposal_id).unwrap();

        // Build audit.
        let audit = build_audit(&root, &proposals[0].claim).unwrap();
        assert!(!audit.entries.is_empty(), "audit must have entries");

        // Should have at least: event records + proposal generated + reference created.
        let has_proposal = audit
            .entries
            .iter()
            .any(|e| matches!(e.kind, AuditKind::ProposalGenerated));
        assert!(has_proposal, "audit must include proposal generation");

        let has_reference = audit
            .entries
            .iter()
            .any(|e| matches!(e.kind, AuditKind::ReferenceCreated));
        assert!(has_reference, "audit must include reference creation");

        // Verify the audit can answer "why does Route think this is valid?"
        assert!(
            audit.reference_id.is_some(),
            "audit must have a reference_id"
        );
    }
}
