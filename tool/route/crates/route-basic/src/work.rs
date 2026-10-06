//! Child work and claims are projections of the existing DevelopmentEvent ledger.
//! A claim records coordination, never filesystem, Git or project authority.
use crate::development::{
    self, AppendDevelopmentEventResult, DevelopmentEvent, DevelopmentEventDraft,
    DevelopmentEventPayload,
};
use crate::execution::{self, EvidenceKind, EvidenceSource, EvidenceStore, SessionStatus};
use crate::principal::CallerContext;
use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkKind {
    TestGap,
    CompatibilityProbe,
    Implementation,
    Review,
    Investigation,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverlapMode {
    Exclusive,
    Cooperative,
    Alternative,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimState {
    Active,
    Released,
    Interrupted,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChildWork {
    #[serde(default)]
    pub work_id: String,
    pub intent_ref: String,
    pub title: String,
    pub kind: WorkKind,
    pub scope_paths: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub verification_requirements: Vec<String>,
    pub overlap_mode: OverlapMode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkClaim {
    #[serde(default)]
    pub claim_id: String,
    pub work_id: String,
    #[serde(default)]
    pub worker_id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub resumes_claim_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkAction {
    ChildCreated {
        work: ChildWork,
    },
    ClaimOpened {
        claim: WorkClaim,
    },
    ClaimChanged {
        claim_id: String,
        state: ClaimState,
        reason: String,
    },
    Integrated {
        intent_ref: String,
        accepted_claim_ids: Vec<String>,
        evidence_refs: Vec<String>,
        state_hash: String,
        expected_revision: u64,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct ClaimView {
    pub claim_id: String,
    pub work_id: String,
    pub worker_id: String,
    pub project_id: String,
    pub state: ClaimState,
    pub created_revision: u64,
    pub last_revision: u64,
    pub focus: Option<String>,
    pub resumes_claim_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AvailableWork {
    pub work: ChildWork,
    pub state: String,
    pub blockers: Vec<String>,
    pub claimable: bool,
    pub claims: Vec<ClaimView>,
    pub created_revision: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Overlap {
    pub left_work_id: String,
    pub right_work_id: String,
    pub kind: String,
    pub scope_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkView {
    pub project_id: String,
    pub global_revision: u64,
    pub intents: Vec<crate::development::SessionSummary>,
    pub available: Vec<AvailableWork>,
    pub overlaps: Vec<Overlap>,
}

fn bounded(label: &str, value: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= max,
        "INVALID_WORK: {label}"
    );
    Ok(())
}

fn validate_path(path: &str) -> Result<()> {
    bounded("scope path", path, 256)?;
    ensure!(
        !path.contains('\\')
            && !path.contains(':')
            && !path.starts_with('/')
            && !path.contains('*')
            && !path.contains('?'),
        "INVALID_WORK_SCOPE"
    );
    let parts: Vec<_> = path.split('/').collect();
    ensure!(
        parts
            .iter()
            .all(|p| !p.is_empty() && *p != "." && *p != ".."),
        "INVALID_WORK_SCOPE"
    );
    ensure!(
        !parts.iter().any(|p| matches!(
            p.to_ascii_lowercase().as_str(),
            ".git" | ".route" | "release" | "yuich"
        )),
        "INVALID_WORK_SCOPE"
    );
    Ok(())
}

pub(crate) fn validate_shape(action: &WorkAction) -> Result<()> {
    match action {
        WorkAction::ChildCreated { work } => {
            bounded("work id", &work.work_id, 256)?;
            bounded("intent ref", &work.intent_ref, 256)?;
            bounded("title", &work.title, 512)?;
            ensure!(
                !work.scope_paths.is_empty() && work.scope_paths.len() <= 16,
                "INVALID_WORK_SCOPE"
            );
            for path in &work.scope_paths {
                validate_path(path)?;
            }
            ensure!(
                work.dependencies.len() <= 16
                    && work
                        .dependencies
                        .iter()
                        .all(|d| !d.is_empty() && d.len() <= 256),
                "INVALID_WORK_DEPENDENCY"
            );
            ensure!(
                !work.verification_requirements.is_empty()
                    && work.verification_requirements.len() <= 16,
                "INVALID_WORK_VERIFICATION"
            );
            for requirement in &work.verification_requirements {
                bounded("verification requirement", requirement, 256)?;
            }
        }
        WorkAction::ClaimOpened { claim } => {
            bounded("claim id", &claim.claim_id, 256)?;
            bounded("work id", &claim.work_id, 256)?;
            bounded("worker id", &claim.worker_id, 256)?;
            if let Some(focus) = &claim.focus {
                bounded("focus", focus, 512)?;
            }
        }
        WorkAction::ClaimChanged {
            claim_id,
            state,
            reason,
        } => {
            bounded("claim id", claim_id, 256)?;
            bounded("transition reason", reason, 512)?;
            ensure!(
                !matches!(state, ClaimState::Active),
                "INVALID_CLAIM_TRANSITION"
            );
        }
        WorkAction::Integrated {
            intent_ref,
            accepted_claim_ids,
            evidence_refs,
            state_hash,
            ..
        } => {
            bounded("intent ref", intent_ref, 256)?;
            ensure!(
                !accepted_claim_ids.is_empty() && accepted_claim_ids.len() <= 64,
                "INTEGRATION_REQUIRED"
            );
            ensure!(
                !evidence_refs.is_empty() && evidence_refs.len() <= 64,
                "INTEGRATION_EVIDENCE_REQUIRED"
            );
            ensure!(state_hash.len() == 64, "STALE_INTEGRATION_STATE");
        }
    }
    Ok(())
}

fn projected(
    events: &[DevelopmentEvent],
) -> (
    BTreeMap<String, (ChildWork, u64)>,
    BTreeMap<String, ClaimView>,
) {
    let mut work = BTreeMap::new();
    let mut claims = BTreeMap::new();
    for event in events {
        if let DevelopmentEventPayload::Work { action } = &event.payload {
            match action {
                WorkAction::ChildCreated { work: item } => {
                    work.insert(item.work_id.clone(), (item.clone(), event.sequence));
                }
                WorkAction::ClaimOpened { claim } => {
                    claims.insert(
                        claim.claim_id.clone(),
                        ClaimView {
                            claim_id: claim.claim_id.clone(),
                            work_id: claim.work_id.clone(),
                            worker_id: claim.worker_id.clone(),
                            project_id: claim.project_id.clone(),
                            state: ClaimState::Active,
                            created_revision: event.sequence,
                            last_revision: event.sequence,
                            focus: claim.focus.clone(),
                            resumes_claim_id: claim.resumes_claim_id.clone(),
                        },
                    );
                }
                WorkAction::ClaimChanged {
                    claim_id, state, ..
                } => {
                    if let Some(c) = claims.get_mut(claim_id) {
                        c.state = state.clone();
                        c.last_revision = event.sequence;
                    }
                }
                WorkAction::Integrated { .. } => {}
            }
        }
    }
    (work, claims)
}

fn active_intent(root: &Path, intent_ref: &str) -> Result<()> {
    let session = execution::session_status(root, Some(intent_ref))?
        .pop()
        .ok_or_else(|| anyhow!("UNKNOWN_INTENT"))?;
    ensure!(session.id == intent_ref, "INVALID_INTENT_REF");
    ensure!(session.status == SessionStatus::Active, "INTENT_NOT_ACTIVE");
    Ok(())
}

pub(crate) fn validate_transition(
    root: &Path,
    events: &[DevelopmentEvent],
    project_id: &str,
    actor: Option<&str>,
    action: &WorkAction,
) -> Result<()> {
    let (work, claims) = projected(events);
    let integrated = |intent: &str| {
        events.iter().any(|event| {
            matches!(&event.payload,
        DevelopmentEventPayload::Work { action: WorkAction::Integrated { intent_ref, .. } }
        if intent_ref == intent)
        })
    };
    match action {
        WorkAction::ChildCreated { work: item } => {
            ensure!(actor.is_some(), "WORKER_BINDING_REQUIRED");
            active_intent(root, &item.intent_ref)?;
            ensure!(!integrated(&item.intent_ref), "INTEGRATION_FROZEN");
            ensure!(!work.contains_key(&item.work_id), "WORK_ID_CONFLICT");
            let mut seen = BTreeSet::new();
            for dependency in &item.dependencies {
                ensure!(seen.insert(dependency), "DUPLICATE_DEPENDENCY");
                ensure!(
                    work.get(dependency)
                        .is_some_and(|(w, _)| w.intent_ref == item.intent_ref),
                    "INVALID_WORK_DEPENDENCY"
                );
            }
        }
        WorkAction::ClaimOpened { claim } => {
            ensure!(actor == Some(&claim.worker_id), "ACTOR_MISMATCH");
            ensure!(claim.project_id == project_id, "PROJECT_IDENTITY_CONFLICT");
            let (item, _) = work
                .get(&claim.work_id)
                .ok_or_else(|| anyhow!("UNKNOWN_WORK"))?;
            active_intent(root, &item.intent_ref)?;
            ensure!(!integrated(&item.intent_ref), "INTEGRATION_FROZEN");
            ensure!(!claims.contains_key(&claim.claim_id), "CLAIM_ID_CONFLICT");
            for dep in &item.dependencies {
                ensure!(
                    claims
                        .values()
                        .any(|c| c.work_id == *dep && matches!(c.state, ClaimState::Completed)),
                    "WORK_BLOCKED_BY_DEPENDENCY"
                );
            }
            if let Some(previous) = &claim.resumes_claim_id {
                let prior = claims
                    .get(previous)
                    .ok_or_else(|| anyhow!("UNKNOWN_PREDECESSOR_CLAIM"))?;
                ensure!(
                    prior.work_id == claim.work_id
                        && matches!(prior.state, ClaimState::Interrupted),
                    "INVALID_REASSIGNMENT"
                );
            }
            if matches!(item.overlap_mode, OverlapMode::Exclusive) {
                let latest_interrupted = claims
                    .values()
                    .filter(|c| {
                        c.work_id == claim.work_id && matches!(c.state, ClaimState::Interrupted)
                    })
                    .max_by_key(|c| c.last_revision);
                if let Some(previous) = latest_interrupted {
                    ensure!(
                        claim.resumes_claim_id.as_deref() == Some(previous.claim_id.as_str()),
                        "REASSIGNMENT_LINEAGE_REQUIRED"
                    );
                }
            }
            ensure!(
                !claims.values().any(|c| c.work_id == claim.work_id
                    && matches!(c.state, ClaimState::Completed)
                    && matches!(item.overlap_mode, OverlapMode::Exclusive)),
                "WORK_ALREADY_COMPLETED"
            );
            if matches!(item.overlap_mode, OverlapMode::Exclusive) {
                ensure!(
                    !claims.values().any(
                        |c| c.work_id == claim.work_id && matches!(c.state, ClaimState::Active)
                    ),
                    "CLAIM_CONFLICT"
                );
            }
        }
        WorkAction::ClaimChanged {
            claim_id, state, ..
        } => {
            let claim = claims
                .get(claim_id)
                .ok_or_else(|| anyhow!("UNKNOWN_CLAIM"))?;
            let (item, _) = work
                .get(&claim.work_id)
                .ok_or_else(|| anyhow!("UNKNOWN_WORK"))?;
            ensure!(!integrated(&item.intent_ref), "INTEGRATION_FROZEN");
            ensure!(
                matches!(claim.state, ClaimState::Active),
                "CLAIM_NOT_ACTIVE"
            );
            if actor.is_none() {
                ensure!(
                    matches!(state, ClaimState::Interrupted),
                    "OPERATOR_CAN_ONLY_INTERRUPT"
                );
            } else {
                ensure!(actor == Some(claim.worker_id.as_str()), "ACTOR_MISMATCH");
            }
        }
        WorkAction::Integrated {
            intent_ref,
            accepted_claim_ids,
            evidence_refs,
            state_hash,
            expected_revision,
        } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            active_intent(root, intent_ref)?;
            ensure!(!integrated(intent_ref), "INTEGRATION_FROZEN");
            let revision = events.last().map(|e| e.sequence).unwrap_or(0);
            ensure!(*expected_revision == revision, "STALE_CONTEXT");
            let relevant: Vec<_> = work
                .values()
                .filter(|(w, _)| &w.intent_ref == intent_ref)
                .collect();
            ensure!(!relevant.is_empty(), "INTEGRATION_REQUIRED");
            let selected: BTreeSet<_> = accepted_claim_ids.iter().collect();
            ensure!(
                selected.len() == accepted_claim_ids.len(),
                "DUPLICATE_ACCEPTED_CLAIM"
            );
            for id in &selected {
                ensure!(
                    claims
                        .get(*id)
                        .is_some_and(|c| matches!(c.state, ClaimState::Completed)
                            && relevant.iter().any(|(w, _)| w.work_id == c.work_id)),
                    "UNVERIFIED_CLAIM"
                );
            }
            for (item, _) in &relevant {
                ensure!(
                    claims.values().any(|c| c.work_id == item.work_id
                        && matches!(c.state, ClaimState::Completed)
                        && selected.contains(&c.claim_id)),
                    "INCOMPLETE_WORK"
                );
                ensure!(!claims.values().any(|c| c.work_id == item.work_id && matches!(c.state, ClaimState::Active)), "ACTIVE_CLAIM");
            }
            let current = execution::compute_state_hash(root)?;
            ensure!(
                !current.is_empty() && current == *state_hash,
                "STALE_INTEGRATION_STATE"
            );
            let store = EvidenceStore::load(root)?;
            let completions: Vec<_> = events
                .iter()
                .filter(|event| matches!(&event.payload, DevelopmentEventPayload::Work { action: WorkAction::ClaimChanged { claim_id, state: ClaimState::Completed, .. } } if selected.contains(claim_id) ))
                .collect();
            let last_completion_revision = completions
                .iter()
                .map(|e| e.sequence)
                .max()
                .ok_or_else(|| anyhow!("INCOMPLETE_WORK"))?;
            // Preserve the legacy maximum timestamp independently of ledger
            // sequence: wall clocks may regress between two completions.
            let last_completion_timestamp = completions
                .iter()
                .map(|e| e.timestamp)
                .max()
                .ok_or_else(|| anyhow!("INCOMPLETE_WORK"))?;
            for id in evidence_refs {
                ensure!(
                    store.evidence.iter().any(|e| e.id == *id
                        && e.session_id == *intent_ref
                        && e.source == EvidenceSource::System
                        && matches!(e.kind, EvidenceKind::CheckPass | EvidenceKind::TestPass)
                        && match e.metadata.get("check_started_revision") {
                            Some(encoded_revision) => encoded_revision
                                .parse::<u64>()
                                .map(|started| started >= last_completion_revision
                                    && started <= revision)
                                .unwrap_or(false),
                            // Older System evidence has no causal marker. Keep its
                            // conservative time boundary for compatibility.
                            None => e.created_at > last_completion_timestamp,
                        }
                        && e.state_hash.as_deref() == Some(state_hash)),
                    "INTEGRATION_EVIDENCE_REQUIRED"
                );
            }
            for (item, _) in &relevant {
                for requirement in &item.verification_requirements {
                    ensure!(
                        evidence_refs.iter().any(|id| store.evidence.iter().any(
                            |e| e.id == *id && e.metadata.get("check_id") == Some(requirement)
                        )),
                        "WORK_VERIFICATION_REQUIRED: {requirement}"
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn available(root: &Path) -> Result<WorkView> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    let (work, claims) = projected(&ledger.events);
    let sessions = execution::session_status(root, None)?;
    let intents = sessions
        .iter()
        .filter(|s| s.status == SessionStatus::Active)
        .map(crate::development::SessionSummary::from)
        .collect();
    let mut available = Vec::new();
    for (item, created_revision) in work.values() {
        let related: Vec<_> = claims
            .values()
            .filter(|c| c.work_id == item.work_id)
            .cloned()
            .collect();
        let blockers: Vec<_> = item
            .dependencies
            .iter()
            .filter(|d| {
                !claims
                    .values()
                    .any(|c| c.work_id == **d && matches!(c.state, ClaimState::Completed))
            })
            .cloned()
            .collect();
        let parent_active = sessions
            .iter()
            .any(|s| s.id == item.intent_ref && s.status == SessionStatus::Active);
        let active = related
            .iter()
            .any(|c| matches!(c.state, ClaimState::Active));
        let completed = related
            .iter()
            .any(|c| matches!(c.state, ClaimState::Completed));
        let interrupted = related
            .iter()
            .any(|c| matches!(c.state, ClaimState::Interrupted));
        let state = if !parent_active {
            "PARENT_CLOSED"
        } else if !blockers.is_empty() {
            "BLOCKED"
        } else if active {
            "ACTIVE"
        } else if completed {
            "COMPLETED_CANDIDATE"
        } else if interrupted {
            "REASSIGNABLE"
        } else {
            "READY"
        };
        let claimable = parent_active
            && blockers.is_empty()
            && (!active || !matches!(item.overlap_mode, OverlapMode::Exclusive))
            && (!completed || !matches!(item.overlap_mode, OverlapMode::Exclusive));
        available.push(AvailableWork {
            work: item.clone(),
            state: state.into(),
            blockers,
            claimable,
            claims: related,
            created_revision: *created_revision,
        });
    }
    let mut overlaps = Vec::new();
    for (index, left) in available.iter().enumerate() {
        if !left
            .claims
            .iter()
            .any(|c| matches!(c.state, ClaimState::Active))
        {
            continue;
        }
        if left
            .claims
            .iter()
            .filter(|c| matches!(c.state, ClaimState::Active))
            .count()
            > 1
        {
            let kind = match left.work.overlap_mode {
                OverlapMode::Cooperative => "COOPERATIVE_OVERLAP",
                OverlapMode::Alternative => "COMPETITIVE_ALTERNATIVE",
                OverlapMode::Exclusive => "UNINTENDED_CONFLICT",
            };
            overlaps.push(Overlap {
                left_work_id: left.work.work_id.clone(),
                right_work_id: left.work.work_id.clone(),
                kind: kind.into(),
                scope_paths: left.work.scope_paths.clone(),
            });
        }
        for right in available.iter().skip(index + 1) {
            if !right
                .claims
                .iter()
                .any(|c| matches!(c.state, ClaimState::Active))
            {
                continue;
            }
            let shared: Vec<_> = left
                .work
                .scope_paths
                .iter()
                .filter(|p| {
                    right.work.scope_paths.iter().any(|q| {
                        *p == q
                            || p.starts_with(&format!("{q}/"))
                            || q.starts_with(&format!("{p}/"))
                    })
                })
                .cloned()
                .collect();
            if !shared.is_empty() {
                overlaps.push(Overlap {
                    left_work_id: left.work.work_id.clone(),
                    right_work_id: right.work.work_id.clone(),
                    kind: "UNINTENDED_CONFLICT".into(),
                    scope_paths: shared,
                });
            }
        }
    }
    Ok(WorkView {
        project_id: identity.project_id,
        global_revision: ledger.events.last().map(|e| e.sequence).unwrap_or(0),
        intents,
        available,
        overlaps,
    })
}

pub fn worker_action(
    root: &Path,
    caller: &CallerContext,
    method: &str,
    mut params: Value,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.authorize(root, method, &mut params, true)?;
    let worker = caller
        .worker_id()
        .ok_or_else(|| anyhow!("WORKER_BINDING_REQUIRED"))?;
    let object = params
        .as_object_mut()
        .ok_or_else(|| anyhow!("INVALID_PARAMS"))?;
    object.remove("worker_id");
    object.remove("actor_worker_id");
    object.remove("from_worker");
    let action = match method {
        "work.create_child" => {
            let mut work: ChildWork = serde_json::from_value(params)?;
            work.work_id = format!("work-{}", route_core::sha256_hex(key.as_bytes()));
            WorkAction::ChildCreated { work }
        }
        "work.claim" => {
            let mut claim: WorkClaim = serde_json::from_value(params)?;
            claim.claim_id = format!("claim-{}", route_core::sha256_hex(key.as_bytes()));
            claim.worker_id = worker.into();
            claim.project_id = development::load_ledger_readonly(root)?.1.project_id;
            WorkAction::ClaimOpened { claim }
        }
        "work.release" | "work.interrupt" | "work.finish" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                claim_id: String,
                reason: String,
            }
            let input: Input = serde_json::from_value(params)?;
            let state = match method {
                "work.release" => ClaimState::Released,
                "work.interrupt" => ClaimState::Interrupted,
                _ => ClaimState::Completed,
            };
            WorkAction::ClaimChanged {
                claim_id: input.claim_id,
                state,
                reason: input.reason,
            }
        }
        _ => return Err(anyhow!("UNKNOWN_WORK_METHOD")),
    };
    let draft: DevelopmentEventDraft = serde_json::from_value(
        json!({"actor_worker_id":worker,"payload":DevelopmentEventPayload::Work{action},"deduplication_key":key,"source_refs":[format!("caller:{}",caller.scope())]}),
    )?;
    development::append_authenticated_event(root, draft, caller)
}

pub fn operator_action(
    root: &Path,
    caller: &CallerContext,
    method: &str,
    params: Value,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.require_operator()?;
    let action = match method {
        "work.interrupt" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Input {
                claim_id: String,
                reason: String,
            }
            let input: Input = serde_json::from_value(params)?;
            WorkAction::ClaimChanged {
                claim_id: input.claim_id,
                state: ClaimState::Interrupted,
                reason: input.reason,
            }
        }
        "work.integrate" => serde_json::from_value::<WorkAction>(
            json!({"operation":"integrated","intent_ref":params["intent_ref"],"accepted_claim_ids":params["accepted_claim_ids"],"evidence_refs":params["evidence_refs"],"state_hash":params["state_hash"],"expected_revision":params["expected_revision"]}),
        )?,
        _ => return Err(anyhow!("UNKNOWN_WORK_METHOD")),
    };
    let draft: DevelopmentEventDraft = serde_json::from_value(
        json!({"payload":DevelopmentEventPayload::Work{action},"deduplication_key":key,"source_refs":["caller:operator"]}),
    )?;
    development::append_operator_work_event(root, draft)
}

pub fn ensure_integrated_before_success(root: &Path, intent_ref: &str) -> Result<()> {
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    let (work, claims) = projected(&ledger.events);
    let related: Vec<_> = ledger.events.iter().filter(|e| matches!(&e.payload, DevelopmentEventPayload::Work { action: WorkAction::ChildCreated { work } } if work.intent_ref == intent_ref)).collect();
    if related.is_empty() {
        return Ok(());
    }
    let integration = ledger.events.iter().rev().find(|e| matches!(&e.payload, DevelopmentEventPayload::Work { action: WorkAction::Integrated { intent_ref: id, .. } } if id == intent_ref)).ok_or_else(|| anyhow!("INTEGRATION_REQUIRED"))?;
    ensure!(
        !ledger.events.iter().any(|e| {
            if e.sequence <= integration.sequence {
                return false;
            }
            match &e.payload {
                DevelopmentEventPayload::Work {
                    action: WorkAction::ChildCreated { work },
                } => work.intent_ref == intent_ref,
                DevelopmentEventPayload::Work {
                    action: WorkAction::ClaimOpened { claim },
                } => work
                    .get(&claim.work_id)
                    .is_some_and(|(item, _)| item.intent_ref == intent_ref),
                DevelopmentEventPayload::Work {
                    action: WorkAction::ClaimChanged { claim_id, .. },
                } => claims
                    .get(claim_id)
                    .and_then(|claim| work.get(&claim.work_id))
                    .is_some_and(|(item, _)| item.intent_ref == intent_ref),
                _ => false,
            }
        }),
        "INTEGRATION_STALE"
    );
    if let DevelopmentEventPayload::Work {
        action: WorkAction::Integrated { state_hash, .. },
    } = &integration.payload
    {
        ensure!(
            execution::compute_state_hash(root)? == *state_hash,
            "INTEGRATION_STALE"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplyTarget, ExecutionSession, SessionStore};

    fn fixture() -> (tempfile::TempDir, CallerContext, CallerContext, String) {
        let root = tempfile::tempdir().unwrap();
        let operator = CallerContext::trusted_operator();
        let mut sessions = SessionStore::default();
        let session = ExecutionSession::new(
            "Bounded maintenance",
            ApplyTarget::Generic,
            "context".into(),
            vec![],
            None,
            None,
            None,
        );
        let intent = session.id.clone();
        sessions.sessions.push(session);
        sessions.save(root.path()).unwrap();
        let mut callers = Vec::new();
        for worker in ["a", "b"] {
            crate::principal::register_worker(
                root.path(),
                &operator,
                Some(worker.into()),
                Default::default(),
                &format!("register-{worker}"),
            )
            .unwrap();
            let secret = crate::principal::generate_credential().unwrap();
            crate::principal::issue(
                root.path(),
                &operator,
                worker,
                &route_core::sha256_hex(secret.as_bytes()),
                vec![
                    "work.create_child".into(),
                    "work.claim".into(),
                    "work.release".into(),
                    "work.interrupt".into(),
                    "work.finish".into(),
                ],
                &format!("bind-{worker}"),
            )
            .unwrap();
            callers.push(CallerContext::authenticate(root.path(), &secret).unwrap());
        }
        (root, callers.remove(0), callers.remove(0), intent)
    }

    fn create(root: &Path, actor: &CallerContext, intent: &str, key: &str, path: &str) -> String {
        let result = worker_action(root, actor, "work.create_child", json!({"intent_ref":intent,"title":"Bounded test","kind":"TEST_GAP","scope_paths":[path],"dependencies":[],"verification_requirements":["cargo test"],"overlap_mode":"EXCLUSIVE"}), key).unwrap();
        if let DevelopmentEventPayload::Work {
            action: WorkAction::ChildCreated { work },
        } = result.event.payload
        {
            work.work_id
        } else {
            panic!("created work event")
        }
    }

    #[test]
    fn claim_is_atomic_actor_bound_and_reassignable_only_after_explicit_interrupt() {
        let (root, a, b, intent) = fixture();
        let path = root.path();
        let work = create(path, &a, &intent, "create", "tool/route/src/lib.rs");
        let claimed =
            worker_action(path, &a, "work.claim", json!({"work_id":work}), "claim-a").unwrap();
        let claim_id = if let DevelopmentEventPayload::Work {
            action: WorkAction::ClaimOpened { claim },
        } = claimed.event.payload
        {
            claim.claim_id
        } else {
            panic!("claim event")
        };
        assert!(worker_action(
            path,
            &b,
            "work.claim",
            json!({"work_id":work}),
            "claim-b-early"
        )
        .is_err());
        assert!(worker_action(
            path,
            &b,
            "work.release",
            json!({"claim_id":claim_id,"reason":"spoof"}),
            "release-other"
        )
        .is_err());
        assert!(worker_action(
            path,
            &a,
            "work.claim",
            json!({"work_id":work,"worker_id":"b"}),
            "spoof"
        )
        .is_err());
        let before = available(path).unwrap().global_revision;
        let interrupted = operator_action(
            path,
            &CallerContext::trusted_operator(),
            "work.interrupt",
            json!({"claim_id":claim_id,"reason":"host observed process exit"}),
            "interrupt-a",
        )
        .unwrap();
        assert_eq!(interrupted.global_revision, before + 1);
        assert_eq!(available(path).unwrap().available[0].state, "REASSIGNABLE");
        let continued = worker_action(
            path,
            &b,
            "work.claim",
            json!({"work_id":work,"resumes_claim_id":claim_id}),
            "claim-b",
        )
        .unwrap();
        let b_claim = if let DevelopmentEventPayload::Work {
            action: WorkAction::ClaimOpened { claim },
        } = continued.event.payload
        {
            claim.claim_id
        } else {
            panic!("claim event")
        };
        assert_eq!(available(path).unwrap().available[0].claims.len(), 2);
        worker_action(
            path,
            &b,
            "work.finish",
            json!({"claim_id":b_claim,"reason":"tests passed locally"}),
            "finish-b",
        )
        .unwrap();
        assert_eq!(
            available(path).unwrap().available[0].state,
            "COMPLETED_CANDIDATE"
        );
        assert!(ensure_integrated_before_success(path, &intent).is_err());
        assert!(operator_action(path, &CallerContext::trusted_operator(), "work.integrate", json!({"intent_ref":intent,"accepted_claim_ids":[b_claim],"evidence_refs":["fake"],"state_hash":"0".repeat(64),"expected_revision":available(path).unwrap().global_revision}), "fake-integration").is_err());
    }

    #[test]
    fn child_scope_and_dependencies_fail_closed() {
        let (root, a, b, intent) = fixture();
        let path = root.path();
        assert!(worker_action(path, &a, "work.create_child", json!({"intent_ref":intent,"title":"scope escape","kind":"IMPLEMENTATION","scope_paths":["../yuich/secret.py"],"verification_requirements":["test"],"overlap_mode":"EXCLUSIVE"}), "escape").is_err());
        assert!(worker_action(path, &a, "work.create_child", json!({"intent_ref":intent,"title":"unrelated","kind":"PRODUCT_GOAL","scope_paths":["tool/route/src/lib.rs"],"verification_requirements":["test"],"overlap_mode":"EXCLUSIVE"}), "kind").is_err());
        assert!(worker_action(path, &a, "work.create_child", json!({"intent_ref":intent,"title":"unknown dependency","kind":"REVIEW","scope_paths":["tool/route/src/lib.rs"],"dependencies":["nope"],"verification_requirements":["test"],"overlap_mode":"EXCLUSIVE"}), "dependency").is_err());
        let first = create(path, &a, &intent, "first", "tool/route/src/lib.rs");
        let second = worker_action(path, &b, "work.create_child", json!({"intent_ref":intent,"title":"review","kind":"REVIEW","scope_paths":["tool/route/src/lib.rs"],"dependencies":[first],"verification_requirements":["test"],"overlap_mode":"EXCLUSIVE"}), "second").unwrap();
        let second_id = if let DevelopmentEventPayload::Work {
            action: WorkAction::ChildCreated { work },
        } = second.event.payload
        {
            work.work_id
        } else {
            panic!("work event")
        };
        assert!(worker_action(
            path,
            &b,
            "work.claim",
            json!({"work_id":second_id}),
            "blocked"
        )
        .is_err());
        assert_eq!(
            available(path)
                .unwrap()
                .available
                .iter()
                .find(|w| w.work.work_id == second_id)
                .unwrap()
                .state,
            "BLOCKED"
        );
    }

    #[test]
    fn concurrent_exclusive_claim_has_one_winner_and_durable_replay() {
        let (root, a, b, intent) = fixture();
        let work = create(
            root.path(),
            &a,
            &intent,
            "concurrent-work",
            "tool/route/src/lib.rs",
        );
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = [a, b]
            .into_iter()
            .enumerate()
            .map(|(index, actor)| {
                let path = root.path().to_path_buf();
                let barrier = barrier.clone();
                let work = work.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    worker_action(
                        &path,
                        &actor,
                        "work.claim",
                        json!({"work_id":work}),
                        &format!("concurrent-{index}"),
                    )
                })
            })
            .collect();
        let results: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        let view = available(root.path()).unwrap();
        assert_eq!(view.available[0].claims.len(), 1);
        assert_eq!(view.available[0].claims[0].state, ClaimState::Active);
        assert_eq!(
            available(root.path()).unwrap().global_revision,
            view.global_revision
        );
    }

    #[test]
    fn integration_requires_post_completion_system_evidence_and_current_state() {
        let (root, a, _, intent) = fixture();
        let path = root.path();
        let work = create(path, &a, &intent, "integrate-work", "tool/route/src/lib.rs");
        let claimed = worker_action(
            path,
            &a,
            "work.claim",
            json!({"work_id":work}),
            "integrate-claim",
        )
        .unwrap();
        let claim_id = match claimed.event.payload {
            DevelopmentEventPayload::Work {
                action: WorkAction::ClaimOpened { claim },
            } => claim.claim_id,
            _ => panic!("claim event"),
        };
        let state_hash = execution::compute_state_hash(path).unwrap();
        assert_eq!(state_hash.len(), 64);
        let check_meta =
            std::collections::HashMap::from([("check_id".into(), "cargo test".into())]);
        let mut store = EvidenceStore::load(path).unwrap();
        let premature = store.record_with_state(
            &intent,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "pre".into(),
            check_meta.clone(),
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        store.save(path).unwrap();
        worker_action(
            path,
            &a,
            "work.finish",
            json!({"claim_id":claim_id,"reason":"candidate ready"}),
            "integrate-finish",
        )
        .unwrap();
        let revision = available(path).unwrap().global_revision;
        assert!(execution::end_session(path, &intent, "success").is_err());
        assert_eq!(
            execution::session_status(path, Some(&intent)).unwrap()[0].status,
            SessionStatus::Active
        );
        let integrate = |id: &str, key: &str| {
            operator_action(
                path,
                &CallerContext::trusted_operator(),
                "work.integrate",
                json!({"intent_ref":intent,"accepted_claim_ids":[claim_id],"evidence_refs":[id],"state_hash":state_hash,"expected_revision":revision}),
                key,
            )
        };
        assert!(integrate(&premature, "premature").is_err());
        let mut store = EvidenceStore::load(path).unwrap();
        let agent = store.record_with_state(
            &intent,
            EvidenceKind::TestPass,
            EvidenceSource::Agent,
            "agent".into(),
            check_meta.clone(),
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        store.save(path).unwrap();
        assert!(integrate(&agent, "agent-proof").is_err());
        std::thread::sleep(std::time::Duration::from_millis(2));
        let mut store = EvidenceStore::load(path).unwrap();
        let verified = store.record_with_state(
            &intent,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "system".into(),
            check_meta,
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        store.save(path).unwrap();
        assert!(integrate(&verified, "integrated").is_ok());
        ensure_integrated_before_success(path, &intent).unwrap();
        assert!(worker_action(path, &a, "work.create_child", json!({"intent_ref":intent,"title":"late child","kind":"TEST_GAP","scope_paths":["tool/route/late.rs"],"verification_requirements":["cargo test"],"overlap_mode":"EXCLUSIVE"}), "late-child").is_err());
    }

    #[test]
    fn integration_uses_check_start_revision_when_timestamps_are_equal() {
        let (root, a, _, intent) = fixture();
        let path = root.path();
        let work = create(path, &a, &intent, "revision-work", "tool/route/src/lib.rs");
        let claimed = worker_action(
            path,
            &a,
            "work.claim",
            json!({"work_id":work}),
            "revision-claim",
        )
        .unwrap();
        let claim_id = match claimed.event.payload {
            DevelopmentEventPayload::Work {
                action: WorkAction::ClaimOpened { claim },
            } => claim.claim_id,
            _ => panic!("claim event"),
        };
        let before_completion = available(path).unwrap().global_revision;
        worker_action(
            path,
            &a,
            "work.finish",
            json!({"claim_id":claim_id,"reason":"candidate ready"}),
            "revision-finish",
        )
        .unwrap();
        let completion = development::load_ledger_readonly(path)
            .unwrap()
            .0
            .events
            .last()
            .unwrap()
            .clone();
        let state_hash = execution::compute_state_hash(path).unwrap();
        let mut store = EvidenceStore::load(path).unwrap();
        let metadata = |revision: u64| {
            std::collections::HashMap::from([
                ("check_id".into(), "cargo test".into()),
                ("check_started_revision".into(), revision.to_string()),
            ])
        };
        let crossed_completion = store.record_with_state(
            &intent,
            EvidenceKind::CheckPass,
            EvidenceSource::System,
            "crossed".into(),
            metadata(before_completion),
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        store.evidence.last_mut().unwrap().created_at = completion.timestamp + 1;
        let same_millisecond = store.record_with_state(
            &intent,
            EvidenceKind::CheckPass,
            EvidenceSource::System,
            "after".into(),
            metadata(completion.sequence),
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        store.evidence.last_mut().unwrap().created_at = completion.timestamp;
        let future_revision = store.record_with_state(
            &intent,
            EvidenceKind::CheckPass,
            EvidenceSource::System,
            "future".into(),
            metadata(completion.sequence + 1),
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        let mut malformed_metadata = metadata(completion.sequence);
        malformed_metadata.insert("check_started_revision".into(), "invalid".into());
        let malformed_revision = store.record_with_state(
            &intent,
            EvidenceKind::CheckPass,
            EvidenceSource::System,
            "malformed".into(),
            malformed_metadata,
            Some(state_hash.clone()),
            None,
            None,
            None,
        );
        // A malformed marker must not fall back to otherwise valid legacy time.
        store.evidence.last_mut().unwrap().created_at = completion.timestamp + 1;
        store.save(path).unwrap();
        let integrate = |id: &str, key: &str| {
            operator_action(
                path,
                &CallerContext::trusted_operator(),
                "work.integrate",
                json!({"intent_ref":intent,"accepted_claim_ids":[claim_id],"evidence_refs":[id],"state_hash":state_hash,"expected_revision":completion.sequence}),
                key,
            )
        };
        assert!(integrate(&crossed_completion, "crossed-completion").is_err());
        assert!(integrate(&future_revision, "future-revision").is_err());
        assert!(integrate(&malformed_revision, "malformed-revision").is_err());
        assert!(integrate(&same_millisecond, "same-millisecond").is_ok());
    }

    #[test]
    #[ignore = "subprocess fixture for integration_check_rerun_after_crossed_completion"]
    fn integration_check_subprocess_fixture() {
        let root = std::env::current_dir().unwrap();
        assert!(root.join(".route/test-check-fixture").exists());
        std::fs::write(root.join(".route/test-check-ready"), b"ready").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !root.join(".route/test-check-release").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "fixture release timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn integration_check_rerun_after_crossed_completion() {
        let (root, a, _, intent) = fixture();
        let path = root.path();
        let work = create(path, &a, &intent, "rerun-work", "tool/route/src/lib.rs");
        let claimed = worker_action(
            path,
            &a,
            "work.claim",
            json!({"work_id":work}),
            "rerun-claim",
        )
        .unwrap();
        let claim_id = match claimed.event.payload {
            DevelopmentEventPayload::Work {
                action: WorkAction::ClaimOpened { claim },
            } => claim.claim_id,
            _ => panic!("claim event"),
        };
        std::fs::write(path.join(".route/test-check-fixture"), b"fixture").unwrap();
        let argv = vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--ignored".into(),
            "--exact".into(),
            "work::tests::integration_check_subprocess_fixture".into(),
        ];
        let before_completion = available(path).unwrap().global_revision;
        let check_root = path.to_path_buf();
        let check_intent = intent.clone();
        let check_argv = argv.clone();
        let running = std::thread::spawn(move || {
            execution::exec_command(&check_root, &check_intent, &check_argv, Some("cargo test"))
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !path.join(".route/test-check-ready").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "check start timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        worker_action(
            path,
            &a,
            "work.finish",
            json!({"claim_id":claim_id,"reason":"ready during first check"}),
            "rerun-finish",
        )
        .unwrap();
        let completion_revision = available(path).unwrap().global_revision;
        std::fs::write(path.join(".route/test-check-release"), b"release").unwrap();
        let first = running.join().unwrap().unwrap();
        assert_eq!(first.exit_code, 0);
        let second = execution::exec_command(path, &intent, &argv, Some("cargo test")).unwrap();
        assert_eq!(second.exit_code, 0);
        assert_eq!(first.state_hash, second.state_hash);
        let store = EvidenceStore::load(path).unwrap();
        let checks: Vec<_> = store
            .evidence
            .iter()
            .filter(|e| e.session_id == intent && matches!(e.kind, EvidenceKind::CheckPass))
            .collect();
        assert_eq!(
            checks.len(),
            2,
            "valid rerun must not be suppressed by crossed check"
        );
        assert_eq!(
            checks[0].metadata["check_started_revision"],
            before_completion.to_string()
        );
        assert_eq!(
            checks[1].metadata["check_started_revision"],
            completion_revision.to_string()
        );
        let integrate = |evidence: &str, key: &str| {
            operator_action(
                path,
                &CallerContext::trusted_operator(),
                "work.integrate",
                json!({"intent_ref":intent,"accepted_claim_ids":[claim_id],"evidence_refs":[evidence],"state_hash":second.state_hash,"expected_revision":completion_revision}),
                key,
            )
        };
        assert!(integrate(&checks[0].id, "rerun-old").is_err());
        assert!(integrate(&checks[1].id, "rerun-new").is_ok());
    }

    #[test]
    fn check_cannot_certify_bytes_changed_while_it_was_running() {
        let (root, _, _, intent) = fixture();
        let path = root.path();
        crate::repository::BasicRepository::init(path).unwrap();
        std::fs::write(path.join(".route/test-check-fixture"), b"fixture").unwrap();
        let candidate = path.join("changing.txt");
        std::fs::write(&candidate, b"before").unwrap();
        let argv = vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--ignored".into(),
            "--exact".into(),
            "work::tests::integration_check_subprocess_fixture".into(),
        ];
        let check_root = path.to_path_buf();
        let check_intent = intent.clone();
        let check_argv = argv.clone();
        let running = std::thread::spawn(move || {
            execution::exec_command(&check_root, &check_intent, &check_argv, Some("cargo test"))
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while !path.join(".route/test-check-ready").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "check start timed out"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::fs::write(&candidate, b"after").unwrap();
        std::fs::write(path.join(".route/test-check-release"), b"release").unwrap();
        let crossed = running.join().unwrap().unwrap();
        assert_eq!(crossed.exit_code, 0);
        assert!(!crossed.state_stable);
        let stable = execution::exec_command(path, &intent, &argv, Some("cargo test")).unwrap();
        assert!(stable.state_stable);
        let store = EvidenceStore::load(path).unwrap();
        let checks: Vec<_> = store
            .evidence
            .iter()
            .filter(|e| e.session_id == intent)
            .collect();
        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].kind, EvidenceKind::CheckFail);
        assert_eq!(checks[1].kind, EvidenceKind::CheckPass);
    }

    #[test]
    fn failed_command_never_records_passing_evidence() {
        let (root, _, _, intent) = fixture();
        let path = root.path();
        crate::repository::BasicRepository::init(path).unwrap();
        let argv = vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--invalid-test-runner-option".into(),
        ];
        let result = execution::exec_command(path, &intent, &argv, Some("cargo test")).unwrap();
        assert_ne!(result.exit_code, 0);
        assert!(result.state_stable);
        let store = EvidenceStore::load(path).unwrap();
        assert!(store
            .evidence
            .iter()
            .any(|e| { e.session_id == intent && e.kind == EvidenceKind::CheckFail }));
        assert!(!store
            .evidence
            .iter()
            .any(|e| { e.session_id == intent && e.kind == EvidenceKind::CheckPass }));
    }

    #[test]
    fn state_hash_changes_when_bytes_change_on_the_same_dirty_path() {
        let root = tempfile::tempdir().unwrap();
        crate::repository::BasicRepository::init(root.path()).unwrap();
        let file = root.path().join("bounded.txt");
        std::fs::write(&file, b"first").unwrap();
        let first = execution::compute_state_hash(root.path()).unwrap();
        std::fs::write(&file, b"second").unwrap();
        let second = execution::compute_state_hash(root.path()).unwrap();
        assert_ne!(
            first, second,
            "integration evidence must bind changed bytes"
        );
    }
}
