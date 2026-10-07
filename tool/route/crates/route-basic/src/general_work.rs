//! General Goal and planning facts on the existing project DevelopmentEvent ledger.
//! Legacy project-memory goals and counterfactual plans remain readable, but
//! are not execution truth. This module adds no independent state store.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::development::{
    self, AppendDevelopmentEventResult, DevelopmentEvent, DevelopmentEventDraft,
    DevelopmentEventPayload,
};
use crate::execution::{self, EvidenceKind, EvidenceSource, EvidenceStore};
use crate::execution_contract::{CompletionDecision, ContractChange, StepState, WorkflowStatus};
use crate::principal::CallerContext;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalDomain {
    Development,
    Planning,
    Research,
    General,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalState {
    Active,
    Blocked,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalSpec {
    #[serde(default)]
    pub goal_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub domain: GoalDomain,
    #[serde(default)]
    pub parent_goal_id: Option<String>,
    #[serde(default)]
    pub constraint_refs: Vec<String>,
    #[serde(default)]
    pub reference_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanRecord {
    pub goal_id: String,
    pub workflow_id: String,
    pub workflow_version: u32,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub unknowns: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub milestones: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub review_conditions: Vec<String>,
    #[serde(default)]
    pub observation_refs: Vec<String>,
    #[serde(default)]
    pub source_delta_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObservationRecord {
    #[serde(default)]
    pub observation_id: String,
    pub goal_id: String,
    pub summary: String,
    #[serde(default)]
    pub affected_step_ids: Vec<String>,
    #[serde(default)]
    pub affected_work_ids: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    #[serde(default)]
    pub decision_id: String,
    pub goal_id: String,
    pub question: String,
    pub reason: String,
    #[serde(default)]
    pub blocked_work_ids: Vec<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub context_refs: Vec<String>,
    #[serde(default)]
    pub step_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionView {
    pub request: DecisionRequest,
    pub answer: Option<String>,
    pub answered_by: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArtifactState {
    Draft,
    Final,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRecord {
    #[serde(default)]
    pub artifact_id: String,
    pub goal_id: String,
    #[serde(default)]
    pub step_id: Option<String>,
    pub path: String,
    pub sha256: String,
    pub state: ArtifactState,
    pub description: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeState {
    Draft,
    Partial,
    Final,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OutcomeRecord {
    #[serde(default)]
    pub outcome_id: String,
    pub goal_id: String,
    pub summary: String,
    pub state: OutcomeState,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewReport {
    pub goal_id: String,
    pub workflow_version: Option<u32>,
    pub completion: String,
    pub missing_steps: Vec<String>,
    pub stale_steps: Vec<String>,
    pub blocked_steps: Vec<String>,
    pub pending_decisions: Vec<String>,
    pub unaddressed_observations: Vec<String>,
    pub affected_steps: Vec<String>,
    pub required_next_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeneralChange {
    GoalCreated {
        goal: GoalSpec,
    },
    GoalClosed {
        goal_id: String,
        state: GoalState,
        reason: String,
    },
    PlanRecorded {
        plan: PlanRecord,
    },
    ObservationRecorded {
        observation: ObservationRecord,
    },
    DecisionRequested {
        decision: DecisionRequest,
    },
    DecisionResponded {
        decision_id: String,
        answer: String,
        reason: String,
    },
    ArtifactRegistered {
        artifact: ArtifactRecord,
    },
    OutcomeRecorded {
        outcome: OutcomeRecord,
    },
    ReviewRecorded {
        report: ReviewReport,
    },
    ProofVerified {
        goal_id: String,
        step_id: String,
        evidence_ref: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GeneralAction {
    pub request_hash: String,
    pub expected_revision: u64,
    pub change: GeneralChange,
}

#[derive(Clone, Debug, Serialize)]
pub struct AssistantStatus {
    pub project_id: String,
    pub global_revision: u64,
    pub creator: String,
    pub created_at: i64,
    pub goal: GoalSpec,
    pub goal_state: GoalState,
    pub plans: Vec<PlanRecord>,
    pub current_plan: Option<PlanRecord>,
    pub workflow: Option<WorkflowStatus>,
    pub work_items: Vec<Value>,
    pub active_workers: Vec<String>,
    pub observations: Vec<ObservationRecord>,
    pub decisions: Vec<DecisionView>,
    pub artifacts: Vec<ArtifactRecord>,
    pub outcomes: Vec<OutcomeRecord>,
    pub review: ReviewReport,
    pub needs_human: bool,
    pub required_satisfied: usize,
    pub required_total: usize,
    pub answer: String,
}

fn bounded(label: &str, value: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control),
        "INVALID_{label}"
    );
    Ok(())
}

fn bounded_vec(label: &str, values: &[String], max_items: usize) -> Result<()> {
    ensure!(values.len() <= max_items, "TOO_MANY_{label}");
    for value in values {
        bounded(label, value, 1000)?;
    }
    Ok(())
}

fn artifact_path(root: &Path, relative: &str) -> Result<PathBuf> {
    bounded("ARTIFACT_PATH", relative, 512)?;
    let path = Path::new(relative);
    ensure!(
        !path.is_absolute()
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_))),
        "INVALID_ARTIFACT_PATH"
    );
    ensure!(
        !relative
            .split(['/', '\\'])
            .any(|part| part.eq_ignore_ascii_case(".route")
                || part.eq_ignore_ascii_case(".git")
                || part == ".."),
        "INVALID_ARTIFACT_PATH"
    );
    let mut result = root.to_path_buf();
    for part in path.components() {
        result.push(part.as_os_str());
        let metadata = std::fs::symlink_metadata(&result)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "ARTIFACT_LINK_UNSUPPORTED"
        );
    }
    ensure!(result.is_file(), "ARTIFACT_NOT_FILE");
    ensure!(
        result.canonicalize()?.starts_with(root.canonicalize()?),
        "ARTIFACT_OUTSIDE_PROJECT"
    );
    Ok(result)
}

fn artifact_current(root: &Path, artifact: &ArtifactRecord) -> bool {
    artifact_path(root, &artifact.path)
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .is_some_and(|bytes| route_core::sha256_hex(&bytes) == artifact.sha256)
}

pub fn validate_shape(action: &GeneralAction) -> Result<()> {
    ensure!(
        action.request_hash.len() == 64
            && action.request_hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "INVALID_REQUEST_HASH"
    );
    match &action.change {
        GeneralChange::GoalCreated { goal } => {
            bounded("GOAL_ID", &goal.goal_id, 100)?;
            bounded("GOAL_TITLE", &goal.title, 240)?;
            ensure!(goal.description.len() <= 4000, "INVALID_GOAL_DESCRIPTION");
            bounded_vec("CONSTRAINT_REF", &goal.constraint_refs, 32)?;
            bounded_vec("REFERENCE_REF", &goal.reference_refs, 32)?;
        }
        GeneralChange::PlanRecorded { plan } => {
            bounded("GOAL_ID", &plan.goal_id, 100)?;
            bounded("WORKFLOW_ID", &plan.workflow_id, 100)?;
            ensure!(plan.workflow_version > 0, "INVALID_PLAN_VERSION");
            for values in [
                &plan.assumptions,
                &plan.unknowns,
                &plan.constraints,
                &plan.milestones,
                &plan.risks,
                &plan.review_conditions,
                &plan.observation_refs,
            ] {
                bounded_vec("PLAN_FIELD", values, 64)?;
            }
        }
        GeneralChange::ObservationRecorded { observation } => {
            bounded("OBSERVATION_ID", &observation.observation_id, 100)?;
            bounded("OBSERVATION", &observation.summary, 2000)?;
            bounded_vec("AFFECTED_STEP", &observation.affected_step_ids, 100)?;
            bounded_vec("AFFECTED_WORK", &observation.affected_work_ids, 100)?;
            bounded_vec("SOURCE_REF", &observation.source_refs, 32)?;
        }
        GeneralChange::DecisionRequested { decision } => {
            bounded("DECISION_ID", &decision.decision_id, 100)?;
            bounded("QUESTION", &decision.question, 2000)?;
            bounded("REASON", &decision.reason, 2000)?;
            bounded_vec("OPTION", &decision.options, 12)?;
            bounded_vec("BLOCKED_WORK", &decision.blocked_work_ids, 32)?;
            bounded_vec("CONTEXT_REF", &decision.context_refs, 32)?;
        }
        GeneralChange::DecisionResponded {
            decision_id,
            answer,
            reason,
        } => {
            bounded("DECISION_ID", decision_id, 100)?;
            bounded("ANSWER", answer, 2000)?;
            bounded("REASON", reason, 2000)?;
        }
        GeneralChange::ArtifactRegistered { artifact } => {
            bounded("ARTIFACT_ID", &artifact.artifact_id, 100)?;
            bounded("ARTIFACT_PATH", &artifact.path, 512)?;
            bounded("ARTIFACT_DESCRIPTION", &artifact.description, 1000)?;
            ensure!(
                artifact.sha256.len() == 64
                    && artifact.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "INVALID_ARTIFACT_HASH"
            );
        }
        GeneralChange::OutcomeRecorded { outcome } => {
            bounded("OUTCOME_ID", &outcome.outcome_id, 100)?;
            bounded("OUTCOME_SUMMARY", &outcome.summary, 2000)?;
            bounded_vec("ARTIFACT_REF", &outcome.artifact_refs, 32)?;
            bounded_vec("EVIDENCE_REF", &outcome.evidence_refs, 32)?;
        }
        GeneralChange::GoalClosed {
            goal_id, reason, ..
        } => {
            bounded("GOAL_ID", goal_id, 100)?;
            bounded("CLOSE_REASON", reason, 2000)?;
        }
        GeneralChange::ReviewRecorded { report } => bounded("GOAL_ID", &report.goal_id, 100)?,
        GeneralChange::ProofVerified {
            goal_id,
            step_id,
            evidence_ref,
        } => {
            bounded("GOAL_ID", goal_id, 100)?;
            bounded("STEP_ID", step_id, 80)?;
            bounded("EVIDENCE_REF", evidence_ref, 100)?;
        }
    }
    Ok(())
}

fn changes<'a>(
    events: &'a [DevelopmentEvent],
) -> impl DoubleEndedIterator<Item = (&'a DevelopmentEvent, &'a GeneralChange)> {
    events.iter().filter_map(|event| match &event.payload {
        DevelopmentEventPayload::GeneralWork { action } => Some((event, &action.change)),
        _ => None,
    })
}

fn goal_from(events: &[DevelopmentEvent], id: &str) -> Option<GoalSpec> {
    changes(events).find_map(|(_, change)| match change {
        GeneralChange::GoalCreated { goal } if goal.goal_id == id => Some(goal.clone()),
        _ => None,
    })
}

pub fn goal_active(events: &[DevelopmentEvent], id: &str) -> bool {
    goal_from(events, id).is_some() && !changes(events).any(|(_, change)| matches!(change, GeneralChange::GoalClosed { goal_id, .. } if goal_id == id))
}

pub fn workflow_contains_step(events: &[DevelopmentEvent], goal_id: &str, step_id: &str) -> bool {
    events
        .iter()
        .rev()
        .find_map(|event| match &event.payload {
            DevelopmentEventPayload::WorkflowContract { action } => match &action.change {
                ContractChange::Created { spec }
                | ContractChange::DeltaAccepted { new_spec: spec, .. }
                    if spec.intent_ref == goal_id =>
                {
                    Some(spec.steps.iter().any(|step| step.step_id == step_id))
                }
                _ => None,
            },
            _ => None,
        })
        .unwrap_or(false)
}

fn plans_for(events: &[DevelopmentEvent], id: &str) -> Vec<PlanRecord> {
    changes(events)
        .filter_map(|(_, change)| match change {
            GeneralChange::PlanRecorded { plan } if plan.goal_id == id => Some(plan.clone()),
            _ => None,
        })
        .collect()
}

fn observations_for(events: &[DevelopmentEvent], id: &str) -> Vec<(u64, ObservationRecord)> {
    changes(events)
        .filter_map(|(event, change)| match change {
            GeneralChange::ObservationRecorded { observation } if observation.goal_id == id => {
                Some((event.sequence, observation.clone()))
            }
            _ => None,
        })
        .collect()
}

fn decisions_for(events: &[DevelopmentEvent], id: &str) -> Vec<DecisionView> {
    let mut decisions: BTreeMap<String, DecisionView> = BTreeMap::new();
    for (_, change) in changes(events) {
        match change {
            GeneralChange::DecisionRequested { decision } if decision.goal_id == id => {
                decisions.insert(
                    decision.decision_id.clone(),
                    DecisionView {
                        request: decision.clone(),
                        answer: None,
                        answered_by: None,
                    },
                );
            }
            GeneralChange::DecisionResponded {
                decision_id,
                answer,
                ..
            } => {
                if let Some(view) = decisions.get_mut(decision_id) {
                    view.answer = Some(answer.clone());
                    view.answered_by = Some("operator".into());
                }
            }
            _ => {}
        }
    }
    decisions.into_values().collect()
}

fn artifacts_for(events: &[DevelopmentEvent], id: &str) -> Vec<ArtifactRecord> {
    changes(events)
        .filter_map(|(_, change)| match change {
            GeneralChange::ArtifactRegistered { artifact } if artifact.goal_id == id => {
                Some(artifact.clone())
            }
            _ => None,
        })
        .collect()
}

fn outcomes_for(events: &[DevelopmentEvent], id: &str) -> Vec<OutcomeRecord> {
    changes(events)
        .filter_map(|(_, change)| match change {
            GeneralChange::OutcomeRecorded { outcome } if outcome.goal_id == id => {
                Some(outcome.clone())
            }
            _ => None,
        })
        .collect()
}

fn workflow_id_for(events: &[DevelopmentEvent], id: &str) -> Option<String> {
    events.iter().find_map(|event| match &event.payload {
        DevelopmentEventPayload::WorkflowContract { action } => match &action.change {
            ContractChange::Created { spec } if spec.intent_ref == id => {
                Some(spec.workflow_id.clone())
            }
            _ => None,
        },
        _ => None,
    })
}

fn review_from_events(root: &Path, events: &[DevelopmentEvent], id: &str) -> Result<ReviewReport> {
    goal_from(events, id).ok_or_else(|| anyhow!("UNKNOWN_GOAL"))?;
    let workflow = workflow_id_for(events, id)
        .map(|workflow_id| crate::execution_contract::status(root, &workflow_id))
        .transpose()?;
    let plans = plans_for(events, id);
    let plan = plans.last();
    let decisions = decisions_for(events, id);
    let pending_decisions: Vec<_> = decisions
        .iter()
        .filter(|item| item.answer.is_none())
        .map(|item| item.request.decision_id.clone())
        .collect();
    let addressed: BTreeSet<_> = plan
        .map(|p| p.observation_refs.iter().cloned().collect())
        .unwrap_or_default();
    let unaddressed: Vec<_> = observations_for(events, id)
        .into_iter()
        .filter(|(_, item)| !addressed.contains(&item.observation_id))
        .map(|(_, item)| item)
        .collect();
    let mut affected = BTreeSet::new();
    for observation in &unaddressed {
        affected.extend(observation.affected_step_ids.iter().cloned());
    }
    let mut actions = Vec::new();
    if plan.is_none() {
        actions.push("create a structured plan tied to an accepted workflow".to_string());
    }
    if let (Some(plan), Some(workflow)) = (plan, &workflow) {
        if plan.workflow_version != workflow.spec.version {
            actions.push(format!(
                "record structured Plan v{} after accepted PlanDelta",
                workflow.spec.version
            ));
        }
    }
    for decision in &pending_decisions {
        actions.push(format!("obtain authorized answer for {decision}"));
    }
    if !unaddressed.is_empty() {
        actions.push("review observations and propose/accept a bounded PlanDelta".into());
    }
    if let Some(workflow) = &workflow {
        actions.extend(workflow.completion.required_next_actions.clone());
        if workflow.workflow_status != "COMPLETE"
            && workflow.completion.completion == CompletionDecision::Pass
        {
            actions.push("request canonical workflow completion".into());
        }
    } else {
        actions.push("attach an accepted execution contract".into());
    }
    let complete = workflow
        .as_ref()
        .is_some_and(|w| w.workflow_status == "COMPLETE")
        && plan.is_some_and(|p| {
            workflow
                .as_ref()
                .is_some_and(|w| p.workflow_version == w.spec.version)
        })
        && pending_decisions.is_empty()
        && unaddressed.is_empty();
    let completion = if complete { "PASS" } else { "DENIED" };
    Ok(ReviewReport {
        goal_id: id.into(),
        workflow_version: workflow.as_ref().map(|w| w.spec.version),
        completion: completion.into(),
        missing_steps: workflow
            .as_ref()
            .map(|w| w.completion.missing_required_steps.clone())
            .unwrap_or_default(),
        stale_steps: workflow
            .as_ref()
            .map(|w| w.completion.stale_steps.clone())
            .unwrap_or_default(),
        blocked_steps: workflow
            .as_ref()
            .map(|w| w.completion.blocked_steps.clone())
            .unwrap_or_default(),
        pending_decisions,
        unaddressed_observations: unaddressed
            .iter()
            .map(|o| o.observation_id.clone())
            .collect(),
        affected_steps: affected.into_iter().collect(),
        required_next_actions: actions,
    })
}

pub fn review(root: &Path, id: &str) -> Result<ReviewReport> {
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    review_from_events(root, &ledger.events, id)
}

pub fn status(root: &Path, id: &str) -> Result<AssistantStatus> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    let goal = goal_from(&ledger.events, id).ok_or_else(|| anyhow!("UNKNOWN_GOAL"))?;
    let created = changes(&ledger.events).find(|(_, change)| matches!(change, GeneralChange::GoalCreated { goal: item } if item.goal_id == id)).ok_or_else(|| anyhow!("UNKNOWN_GOAL"))?.0;
    let plans = plans_for(&ledger.events, id);
    let current_plan = plans.last().cloned();
    let workflow = workflow_id_for(&ledger.events, id)
        .map(|workflow_id| crate::execution_contract::status(root, &workflow_id))
        .transpose()?;
    let work = crate::work::available(root)?;
    let work_items: Vec<Value> = work
        .available
        .into_iter()
        .filter(|item| item.work.intent_ref == id)
        .map(|item| json!(item))
        .collect();
    let mut workers = BTreeSet::new();
    for item in &work_items {
        if let Some(claims) = item["claims"].as_array() {
            for claim in claims {
                if claim["state"] == "ACTIVE" {
                    if let Some(worker) = claim["worker_id"].as_str() {
                        workers.insert(worker.to_string());
                    }
                }
            }
        }
    }
    let decisions = decisions_for(&ledger.events, id);
    let observations = observations_for(&ledger.events, id)
        .into_iter()
        .map(|(_, item)| item)
        .collect();
    let artifacts = artifacts_for(&ledger.events, id);
    let outcomes = outcomes_for(&ledger.events, id);
    let review = review_from_events(root, &ledger.events, id)?;
    let persisted_state = changes(&ledger.events)
        .rev()
        .find_map(|(_, change)| match change {
            GeneralChange::GoalClosed { goal_id, state, .. } if goal_id == id => Some(*state),
            _ => None,
        });
    let goal_state = match persisted_state {
        Some(GoalState::Succeeded) if review.completion != "PASS" => GoalState::Blocked,
        Some(value) => value,
        None if !review.pending_decisions.is_empty() || !review.blocked_steps.is_empty() => {
            GoalState::Blocked
        }
        None => GoalState::Active,
    };
    let (required_satisfied, required_total) = workflow
        .as_ref()
        .map(|w| {
            let relevant: Vec<_> = w
                .steps
                .iter()
                .filter(|s| {
                    matches!(
                        s.step.requirement,
                        crate::execution_contract::Requirement::Required
                            | crate::execution_contract::Requirement::Conditional
                    )
                })
                .collect();
            (
                relevant
                    .iter()
                    .filter(|s| matches!(s.state, StepState::Satisfied | StepState::SkippedValid))
                    .count(),
                relevant.len(),
            )
        })
        .unwrap_or((0, 0));
    let answer = if goal_state == GoalState::Succeeded && review.completion == "PASS" {
        format!("Goal completed: {}", goal.title)
    } else if review.completion == "PASS" {
        format!(
            "Work verified; final Outcome and Goal closure still pending: {}",
            goal.title
        )
    } else {
        format!(
            "Goal incomplete: {}. {} of {} required obligations satisfied.",
            goal.title, required_satisfied, required_total
        )
    };
    Ok(AssistantStatus {
        project_id: identity.project_id,
        global_revision: ledger
            .events
            .last()
            .map(|event| event.sequence)
            .unwrap_or(0),
        creator: created
            .actor_worker_id
            .clone()
            .unwrap_or_else(|| "operator".into()),
        created_at: created.timestamp,
        goal,
        goal_state,
        plans,
        current_plan,
        workflow,
        work_items,
        active_workers: workers.into_iter().collect(),
        observations,
        decisions,
        artifacts,
        outcomes,
        needs_human: !review.pending_decisions.is_empty(),
        review,
        required_satisfied,
        required_total,
        answer,
    })
}

pub fn list(root: &Path) -> Result<Vec<AssistantStatus>> {
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    let ids: Vec<_> = changes(&ledger.events)
        .filter_map(|(_, change)| match change {
            GeneralChange::GoalCreated { goal } => Some(goal.goal_id.clone()),
            _ => None,
        })
        .collect();
    ids.iter().map(|id| status(root, id)).collect()
}

pub fn current(root: &Path) -> Result<AssistantStatus> {
    let all = list(root)?;
    all.iter()
        .rev()
        .find(|item| matches!(item.goal_state, GoalState::Active | GoalState::Blocked))
        .or_else(|| all.last())
        .cloned()
        .ok_or_else(|| anyhow!("NO_GOAL"))
}

pub fn validate_transition(
    root: &Path,
    events: &[DevelopmentEvent],
    actor: Option<&str>,
    action: &GeneralAction,
) -> Result<()> {
    ensure!(
        action.expected_revision == events.last().map(|e| e.sequence).unwrap_or(0),
        "STALE_CONTEXT"
    );
    match &action.change {
        GeneralChange::GoalCreated { goal } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            ensure!(
                goal_from(events, &goal.goal_id).is_none(),
                "GOAL_ID_CONFLICT"
            );
            if let Some(parent) = &goal.parent_goal_id {
                ensure!(goal_active(events, parent), "ACTIVE_PARENT_GOAL_REQUIRED");
            }
        }
        GeneralChange::PlanRecorded { plan } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            ensure!(goal_active(events, &plan.goal_id), "ACTIVE_GOAL_REQUIRED");
            ensure!(
                !plans_for(events, &plan.goal_id)
                    .iter()
                    .any(|p| p.workflow_version == plan.workflow_version),
                "PLAN_VERSION_EXISTS"
            );
            let workflow = crate::execution_contract::status(root, &plan.workflow_id)?;
            ensure!(
                workflow.spec.intent_ref == plan.goal_id
                    && workflow.spec.version == plan.workflow_version,
                "WORKFLOW_VERSION_MISMATCH"
            );
            if plan.workflow_version > 1 {
                let delta_id = plan
                    .source_delta_id
                    .as_deref()
                    .ok_or_else(|| anyhow!("PLAN_DELTA_REQUIRED"))?;
                ensure!(events.iter().any(|event| matches!(&event.payload,
                    DevelopmentEventPayload::WorkflowContract { action: crate::execution_contract::ContractAction { change: ContractChange::DeltaAccepted { delta_id: id, new_spec }, .. } }
                    if id == delta_id && new_spec.workflow_id == plan.workflow_id && new_spec.version == plan.workflow_version)), "ACCEPTED_PLAN_DELTA_REQUIRED");
            } else {
                ensure!(plan.source_delta_id.is_none(), "UNEXPECTED_PLAN_DELTA");
            }
            let observations: BTreeSet<_> = observations_for(events, &plan.goal_id)
                .into_iter()
                .map(|(_, o)| o.observation_id)
                .collect();
            ensure!(
                plan.observation_refs
                    .iter()
                    .all(|id| observations.contains(id)),
                "UNKNOWN_OBSERVATION_REF"
            );
        }
        GeneralChange::ObservationRecorded { observation } => {
            ensure!(
                goal_active(events, &observation.goal_id),
                "ACTIVE_GOAL_REQUIRED"
            );
            ensure!(
                !observations_for(events, &observation.goal_id)
                    .iter()
                    .any(|(_, o)| o.observation_id == observation.observation_id),
                "OBSERVATION_ID_CONFLICT"
            );
            for step in &observation.affected_step_ids {
                ensure!(
                    workflow_contains_step(events, &observation.goal_id, step),
                    "UNKNOWN_AFFECTED_STEP"
                );
            }
            for work_id in &observation.affected_work_ids {
                ensure!(events.iter().any(|event| matches!(&event.payload, DevelopmentEventPayload::Work { action: crate::work::WorkAction::ChildCreated { work } } if work.work_id == *work_id && work.intent_ref == observation.goal_id)), "UNKNOWN_AFFECTED_WORK");
            }
        }
        GeneralChange::DecisionRequested { decision } => {
            ensure!(
                goal_active(events, &decision.goal_id),
                "ACTIVE_GOAL_REQUIRED"
            );
            ensure!(
                !decisions_for(events, &decision.goal_id)
                    .iter()
                    .any(|d| d.request.decision_id == decision.decision_id),
                "DECISION_ID_CONFLICT"
            );
            if let Some(step) = &decision.step_id {
                ensure!(
                    workflow_contains_step(events, &decision.goal_id, step),
                    "UNKNOWN_DECISION_STEP"
                );
            }
            for work_id in &decision.blocked_work_ids {
                ensure!(events.iter().any(|event| matches!(&event.payload, DevelopmentEventPayload::Work { action: crate::work::WorkAction::ChildCreated { work } } if work.work_id == *work_id && work.intent_ref == decision.goal_id)), "UNKNOWN_BLOCKED_WORK");
            }
        }
        GeneralChange::DecisionResponded { decision_id, .. } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            let found = changes(events)
                .find_map(|(_, change)| match change {
                    GeneralChange::DecisionRequested { decision }
                        if decision.decision_id == *decision_id =>
                    {
                        Some(decision.goal_id.clone())
                    }
                    _ => None,
                })
                .ok_or_else(|| anyhow!("UNKNOWN_DECISION"))?;
            ensure!(goal_active(events, &found), "ACTIVE_GOAL_REQUIRED");
            ensure!(
                decisions_for(events, &found)
                    .iter()
                    .any(|d| d.request.decision_id == *decision_id && d.answer.is_none()),
                "DECISION_ALREADY_ANSWERED"
            );
        }
        GeneralChange::ArtifactRegistered { artifact } => {
            ensure!(
                goal_active(events, &artifact.goal_id),
                "ACTIVE_GOAL_REQUIRED"
            );
            ensure!(
                !artifacts_for(events, &artifact.goal_id)
                    .iter()
                    .any(|a| a.artifact_id == artifact.artifact_id),
                "ARTIFACT_ID_CONFLICT"
            );
            ensure!(artifact_current(root, artifact), "ARTIFACT_HASH_MISMATCH");
            if let Some(step) = &artifact.step_id {
                ensure!(
                    workflow_contains_step(events, &artifact.goal_id, step),
                    "UNKNOWN_ARTIFACT_STEP"
                );
            }
            if artifact.state == ArtifactState::Final {
                ensure!(actor.is_none(), "OPERATOR_REQUIRED");
                ensure!(
                    review_from_events(root, events, &artifact.goal_id)?.completion == "PASS",
                    "COMPLETION_GATE_DENIED"
                );
            }
        }
        GeneralChange::OutcomeRecorded { outcome } => {
            ensure!(
                goal_active(events, &outcome.goal_id),
                "ACTIVE_GOAL_REQUIRED"
            );
            ensure!(
                !outcomes_for(events, &outcome.goal_id)
                    .iter()
                    .any(|o| o.outcome_id == outcome.outcome_id),
                "OUTCOME_ID_CONFLICT"
            );
            let artifacts = artifacts_for(events, &outcome.goal_id);
            ensure!(
                outcome.artifact_refs.iter().all(|id| artifacts
                    .iter()
                    .any(|a| a.artifact_id == *id && artifact_current(root, a))),
                "UNKNOWN_OR_STALE_ARTIFACT"
            );
            if outcome.state == OutcomeState::Final {
                ensure!(actor.is_none(), "OPERATOR_REQUIRED");
                ensure!(
                    review_from_events(root, events, &outcome.goal_id)?.completion == "PASS",
                    "COMPLETION_GATE_DENIED"
                );
                ensure!(
                    outcome.artifact_refs.iter().any(|id| artifacts
                        .iter()
                        .any(|a| a.artifact_id == *id && a.state == ArtifactState::Final)),
                    "FINAL_ARTIFACT_REQUIRED"
                );
            }
        }
        GeneralChange::ReviewRecorded { report } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            ensure!(
                &review_from_events(root, events, &report.goal_id)? == report,
                "REVIEW_MISMATCH"
            );
        }
        GeneralChange::ProofVerified {
            goal_id,
            step_id,
            evidence_ref,
        } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            ensure!(
                goal_active(events, goal_id) && workflow_contains_step(events, goal_id, step_id),
                "ACTIVE_GOAL_STEP_REQUIRED"
            );
            ensure!(
                EvidenceStore::load(root)?
                    .evidence
                    .iter()
                    .any(|e| e.id == *evidence_ref
                        && e.session_id == *goal_id
                        && e.source == EvidenceSource::System
                        && e.kind == EvidenceKind::CheckPass),
                "QUALIFYING_EVIDENCE_REQUIRED"
            );
        }
        GeneralChange::GoalClosed { goal_id, state, .. } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            ensure!(goal_active(events, goal_id), "ACTIVE_GOAL_REQUIRED");
            ensure!(
                matches!(
                    state,
                    GoalState::Succeeded | GoalState::Failed | GoalState::Cancelled
                ),
                "INVALID_GOAL_CLOSE_STATE"
            );
            if *state == GoalState::Succeeded {
                ensure!(
                    review_from_events(root, events, goal_id)?.completion == "PASS",
                    "COMPLETION_GATE_DENIED"
                );
                ensure!(
                    outcomes_for(events, goal_id)
                        .iter()
                        .any(|o| o.state == OutcomeState::Final),
                    "FINAL_OUTCOME_REQUIRED"
                );
            }
        }
    }
    Ok(())
}

fn generated_id(prefix: &str, key: &str) -> String {
    format!("{prefix}-{}", route_core::sha256_hex(key.as_bytes()))
}

fn action_from_request(
    root: &Path,
    method: &str,
    params: Value,
    key: &str,
    scope: &str,
) -> Result<GeneralAction> {
    let request_hash = route_core::sha256_hex(
        format!("{method}:{scope}:{}", serde_json::to_string(&params)?).as_bytes(),
    );
    let expected_revision = params
        .get("expected_revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("EXPECTED_REVISION_REQUIRED"))?;
    let input = params
        .as_object()
        .ok_or_else(|| anyhow!("INVALID_PARAMS"))?;
    let change = match method {
        "goal.create" => {
            let mut goal: GoalSpec = serde_json::from_value(
                input
                    .get("goal")
                    .cloned()
                    .ok_or_else(|| anyhow!("GOAL_REQUIRED"))?,
            )?;
            goal.goal_id = generated_id("goal", key);
            GeneralChange::GoalCreated { goal }
        }
        "goal.close" => GeneralChange::GoalClosed {
            goal_id: serde_json::from_value(
                input
                    .get("goal_id")
                    .cloned()
                    .ok_or_else(|| anyhow!("GOAL_ID_REQUIRED"))?,
            )?,
            state: serde_json::from_value(
                input
                    .get("state")
                    .cloned()
                    .ok_or_else(|| anyhow!("STATE_REQUIRED"))?,
            )?,
            reason: serde_json::from_value(
                input
                    .get("reason")
                    .cloned()
                    .ok_or_else(|| anyhow!("REASON_REQUIRED"))?,
            )?,
        },
        "plan.create" => GeneralChange::PlanRecorded {
            plan: serde_json::from_value(
                input
                    .get("plan")
                    .cloned()
                    .ok_or_else(|| anyhow!("PLAN_REQUIRED"))?,
            )?,
        },
        "observation.record" => {
            let mut observation: ObservationRecord = serde_json::from_value(
                input
                    .get("observation")
                    .cloned()
                    .ok_or_else(|| anyhow!("OBSERVATION_REQUIRED"))?,
            )?;
            observation.observation_id = generated_id("observation", key);
            GeneralChange::ObservationRecorded { observation }
        }
        "decision.request" => {
            let mut decision: DecisionRequest = serde_json::from_value(
                input
                    .get("decision")
                    .cloned()
                    .ok_or_else(|| anyhow!("DECISION_REQUIRED"))?,
            )?;
            decision.decision_id = generated_id("decision", key);
            GeneralChange::DecisionRequested { decision }
        }
        "decision.respond" => GeneralChange::DecisionResponded {
            decision_id: serde_json::from_value(
                input
                    .get("decision_id")
                    .cloned()
                    .ok_or_else(|| anyhow!("DECISION_ID_REQUIRED"))?,
            )?,
            answer: serde_json::from_value(
                input
                    .get("answer")
                    .cloned()
                    .ok_or_else(|| anyhow!("ANSWER_REQUIRED"))?,
            )?,
            reason: serde_json::from_value(
                input
                    .get("reason")
                    .cloned()
                    .ok_or_else(|| anyhow!("REASON_REQUIRED"))?,
            )?,
        },
        "artifact.register" => {
            let mut artifact: ArtifactRecord = serde_json::from_value(
                input
                    .get("artifact")
                    .cloned()
                    .ok_or_else(|| anyhow!("ARTIFACT_REQUIRED"))?,
            )?;
            artifact.artifact_id = generated_id("artifact", key);
            GeneralChange::ArtifactRegistered { artifact }
        }
        "outcome.record" => {
            let mut outcome: OutcomeRecord = serde_json::from_value(
                input
                    .get("outcome")
                    .cloned()
                    .ok_or_else(|| anyhow!("OUTCOME_REQUIRED"))?,
            )?;
            outcome.outcome_id = generated_id("outcome", key);
            GeneralChange::OutcomeRecorded { outcome }
        }
        "plan.review.record" => {
            let goal_id = input
                .get("goal_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("GOAL_ID_REQUIRED"))?;
            GeneralChange::ReviewRecorded {
                report: review(root, goal_id)?,
            }
        }
        _ => return Err(anyhow!("UNKNOWN_GENERAL_WORK_METHOD")),
    };
    Ok(GeneralAction {
        request_hash,
        expected_revision,
        change,
    })
}

fn perform(
    root: &Path,
    caller: &CallerContext,
    method: &str,
    mut params: Value,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.authorize(root, method, &mut params, true)?;
    let actor = caller.worker_id();
    if actor.is_none() {
        caller.require_operator()?;
    }
    if matches!(
        method,
        "goal.create" | "goal.close" | "plan.create" | "decision.respond" | "plan.review.record"
    ) {
        ensure!(actor.is_none(), "OPERATOR_REQUIRED");
    }
    let action = action_from_request(root, method, params, key, &caller.scope())?;
    if actor.is_some() {
        match &action.change {
            GeneralChange::ArtifactRegistered { artifact } => {
                ensure!(artifact.state == ArtifactState::Draft, "OPERATOR_REQUIRED")
            }
            GeneralChange::OutcomeRecorded { outcome } => {
                ensure!(outcome.state != OutcomeState::Final, "OPERATOR_REQUIRED")
            }
            GeneralChange::ObservationRecorded { .. } | GeneralChange::DecisionRequested { .. } => {
            }
            _ => return Err(anyhow!("OPERATOR_REQUIRED")),
        }
    }
    let draft: DevelopmentEventDraft = serde_json::from_value(json!({
        "actor_worker_id": actor,
        "payload": DevelopmentEventPayload::GeneralWork { action },
        "deduplication_key": key,
        "source_refs": [format!("caller:{}", caller.scope())]
    }))?;
    if actor.is_some() {
        development::append_authenticated_event(root, draft, caller)
    } else {
        development::append_operator_general_event(root, draft)
    }
}

pub fn worker_action(
    root: &Path,
    caller: &CallerContext,
    method: &str,
    params: Value,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    ensure!(caller.worker_id().is_some(), "WORKER_BINDING_REQUIRED");
    perform(root, caller, method, params, key)
}

pub fn operator_action(
    root: &Path,
    caller: &CallerContext,
    method: &str,
    params: Value,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.require_operator()?;
    perform(root, caller, method, params, key)
}

/// Only the trusted Operator can ask Route to evaluate structured planning
/// facts into canonical System evidence. A Worker claim alone is insufficient.
pub fn verify_step(
    root: &Path,
    caller: &CallerContext,
    goal_id: &str,
    step_id: &str,
    expected_revision: u64,
    key: &str,
) -> Result<Value> {
    caller.require_operator()?;
    let lock_path = root.join(".route").join(".plan-proof-lock");
    let _lock = crate::ownership_lock::OwnershipLock::acquire(&lock_path)?;
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    let current_revision = ledger
        .events
        .last()
        .map(|event| event.sequence)
        .unwrap_or(0);
    let request_hash =
        route_core::sha256_hex(format!("{goal_id}:{step_id}:{expected_revision}").as_bytes());
    let store = EvidenceStore::load(root)?;
    if let Some(existing) = store.evidence.iter().find(|e| {
        e.metadata
            .get("plan_verify_key")
            .is_some_and(|value| value == key)
    }) {
        ensure!(
            existing.metadata.get("plan_verify_request") == Some(&request_hash),
            "IDEMPOTENCY_CONFLICT"
        );
        if ledger.events.iter().any(|event| matches!(&event.payload,
            DevelopmentEventPayload::GeneralWork { action: GeneralAction { change: GeneralChange::ProofVerified { evidence_ref, .. }, .. } }
            if evidence_ref == &existing.id)) {
            return Ok(json!({"evidence_ref":existing.id,"replay":true,"check_id":existing.metadata.get("check_id")}));
        }
        // A process may have stopped after saving System evidence but before
        // recording its ledger event. Reconcile only if the same project state
        // and goal/step are still valid; never create a second proof.
        ensure!(
            existing.session_id == goal_id
                && goal_active(&ledger.events, goal_id)
                && workflow_contains_step(&ledger.events, goal_id, step_id)
                && existing.state_hash.as_deref()
                    == Some(execution::compute_state_hash(root)?.as_str()),
            "ORPHAN_PROOF_STALE"
        );
        let action = GeneralAction {
            request_hash: route_core::sha256_hex(
                format!("plan.verify_step:{key}:{goal_id}:{step_id}:{expected_revision}")
                    .as_bytes(),
            ),
            expected_revision: current_revision,
            change: GeneralChange::ProofVerified {
                goal_id: goal_id.into(),
                step_id: step_id.into(),
                evidence_ref: existing.id.clone(),
            },
        };
        let draft: DevelopmentEventDraft = serde_json::from_value(json!({
            "payload": DevelopmentEventPayload::GeneralWork { action },
            "deduplication_key": key,
            "source_refs": ["caller:operator"],
            "evidence_refs": [existing.id]
        }))?;
        development::append_operator_general_event(root, draft)?;
        return Ok(
            json!({"evidence_ref":existing.id,"replay":true,"check_id":existing.metadata.get("check_id")}),
        );
    }
    ensure!(current_revision == expected_revision, "STALE_CONTEXT");
    ensure!(goal_active(&ledger.events, goal_id), "ACTIVE_GOAL_REQUIRED");
    let workflow_id =
        workflow_id_for(&ledger.events, goal_id).ok_or_else(|| anyhow!("WORKFLOW_REQUIRED"))?;
    let workflow = crate::execution_contract::status(root, &workflow_id)?;
    let step = workflow
        .spec
        .steps
        .iter()
        .find(|step| step.step_id == step_id)
        .ok_or_else(|| anyhow!("UNKNOWN_STEP"))?;
    let plan = plans_for(&ledger.events, goal_id)
        .into_iter()
        .last()
        .ok_or_else(|| anyhow!("PLAN_REQUIRED"))?;
    ensure!(
        plan.workflow_version == workflow.spec.version,
        "PLAN_VERSION_STALE"
    );
    let work = crate::work::available(root)?;
    let associated: Vec<_> = work
        .available
        .iter()
        .filter(|item| {
            item.work.intent_ref == goal_id
                && item.work.workflow_step_id.as_deref() == Some(step_id)
        })
        .collect();
    ensure!(
        !associated.is_empty()
            && associated
                .iter()
                .all(|item| item.state == "COMPLETED_CANDIDATE"),
        "WORK_NOT_COMPLETED"
    );
    let artifacts = artifacts_for(&ledger.events, goal_id);
    let artifact_ok = artifacts.iter().any(|artifact| {
        artifact.step_id.as_deref() == Some(step_id) && artifact_current(root, artifact)
    });
    let decision_ok = decisions_for(&ledger.events, goal_id)
        .iter()
        .any(|decision| {
            decision.request.step_id.as_deref() == Some(step_id) && decision.answer.is_some()
        });
    ensure!(artifact_ok || decision_ok, "STRUCTURED_PROOF_REQUIRED");
    let state_hash = execution::compute_state_hash(root)?;
    ensure!(!state_hash.is_empty(), "STATE_FINGERPRINT_REQUIRED");
    let mut store = store;
    ensure!(
        execution::compute_state_hash(root)? == state_hash
            && development::global_development_revision(root)? == expected_revision,
        "STATE_CHANGED_DURING_VERIFICATION"
    );
    let mut metadata = HashMap::new();
    metadata.insert("check_id".into(), step.proof.check_id.clone());
    metadata.insert("exit_code".into(), "0".into());
    metadata.insert("state_stable".into(), "true".into());
    metadata.insert(
        "check_started_revision".into(),
        expected_revision.to_string(),
    );
    metadata.insert(
        "check_finished_revision".into(),
        expected_revision.to_string(),
    );
    metadata.insert("plan_verify_key".into(), key.into());
    metadata.insert("plan_verify_request".into(), request_hash);
    metadata.insert("verification_class".into(), "structured_plan".into());
    let evidence_ref = store.record_with_state(
        goal_id,
        EvidenceKind::CheckPass,
        EvidenceSource::System,
        route_core::sha256_hex(
            format!("{}:{}:{}", goal_id, step_id, workflow.spec.version).as_bytes(),
        ),
        metadata,
        Some(state_hash),
        None,
        None,
        None,
    );
    store.save(root)?;
    let action = GeneralAction {
        request_hash: route_core::sha256_hex(
            format!("plan.verify_step:{key}:{goal_id}:{step_id}:{expected_revision}").as_bytes(),
        ),
        expected_revision,
        change: GeneralChange::ProofVerified {
            goal_id: goal_id.into(),
            step_id: step_id.into(),
            evidence_ref: evidence_ref.clone(),
        },
    };
    let draft: DevelopmentEventDraft = serde_json::from_value(json!({
        "payload": DevelopmentEventPayload::GeneralWork { action },
        "deduplication_key": key,
        "source_refs": ["caller:operator"],
        "evidence_refs": [evidence_ref]
    }))?;
    development::append_operator_general_event(root, draft)?;
    Ok(json!({"evidence_ref":evidence_ref,"replay":false,"check_id":step.proof.check_id}))
}
