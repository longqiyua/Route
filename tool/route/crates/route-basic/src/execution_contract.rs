//! Evidence-bound execution contracts on the existing DevelopmentEvent ledger.
//! Descriptive workflow References remain separate; this module owns only the
//! accepted, versioned execution contract for an active Intent.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::development::{
    self, AppendDevelopmentEventResult, DevelopmentEvent, DevelopmentEventDraft,
    DevelopmentEventPayload,
};
use crate::execution::{
    self, EvidenceKind, EvidenceSource, EvidenceStore, SessionStatus, SessionStore,
};
use crate::principal::CallerContext;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContractMode {
    Controlled,
    FullPower,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Requirement {
    Required,
    Conditional,
    Optional,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum StepCondition {
    PathExists { path: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProofObligation {
    pub check_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContractStep {
    pub step_id: String,
    pub title: String,
    pub requirement: Requirement,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub condition: Option<StepCondition>,
    pub proof: ProofObligation,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSpec {
    pub workflow_id: String,
    pub intent_ref: String,
    #[serde(default)]
    pub version: u32,
    pub title: String,
    pub mode: ContractMode,
    pub steps: Vec<ContractStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanDelta {
    #[serde(default)]
    pub delta_id: String,
    pub workflow_id: String,
    pub from_version: u32,
    pub reason: String,
    pub proposed_steps: Vec<ContractStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContractChange {
    Created {
        spec: WorkflowSpec,
    },
    StepStarted {
        workflow_id: String,
        version: u32,
        step_id: String,
    },
    StepReported {
        workflow_id: String,
        version: u32,
        step_id: String,
        evidence_ref: Option<String>,
    },
    StepFailed {
        workflow_id: String,
        version: u32,
        step_id: String,
        reason: String,
    },
    SkipRequested {
        workflow_id: String,
        version: u32,
        step_id: String,
        reason: String,
        condition_false_observed: bool,
    },
    DeltaProposed {
        delta: PlanDelta,
    },
    DeltaAccepted {
        delta_id: String,
        new_spec: WorkflowSpec,
    },
    DeltaRejected {
        delta_id: String,
        reason: String,
    },
    CompletionEvaluated {
        workflow_id: String,
        version: u32,
        report: CompletionReport,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContractAction {
    pub request_hash: String,
    pub expected_revision: u64,
    pub change: ContractChange,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepState {
    Pending,
    Running,
    Satisfied,
    Failed,
    Blocked,
    SkippedValid,
    Bypassed,
    Stale,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepRun {
    pub step_id: String,
    pub version: u32,
    pub actor_worker_id: Option<String>,
    pub event_revision: u64,
    pub action: String,
    pub evidence_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepView {
    pub step: ContractStep,
    pub state: StepState,
    pub last_run: Option<StepRun>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompletionDecision {
    Pass,
    Denied,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionReport {
    pub completion: CompletionDecision,
    pub workflow_id: String,
    pub version: u32,
    pub state_hash: String,
    pub missing_required_steps: Vec<String>,
    pub stale_steps: Vec<String>,
    pub failed_obligations: Vec<String>,
    pub unauthorized_bypasses: Vec<String>,
    pub blocked_steps: Vec<String>,
    pub required_next_actions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowStatus {
    pub project_id: String,
    pub global_revision: u64,
    pub spec: WorkflowSpec,
    pub versions: Vec<WorkflowSpec>,
    pub plan_deltas: Vec<PlanDelta>,
    pub steps: Vec<StepView>,
    pub completion: CompletionReport,
    pub workflow_status: String,
}

fn nonempty(label: &str, value: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= max,
        "INVALID_{label}"
    );
    ensure!(!value.chars().any(char::is_control), "INVALID_{label}");
    Ok(())
}

fn safe_scope_path(path: &str) -> Result<()> {
    nonempty("PATH", path, 512)?;
    let value = Path::new(path);
    ensure!(
        !value.is_absolute()
            && value
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "INVALID_PATH"
    );
    ensure!(
        !path
            .split(['/', '\\'])
            .any(|c| c == ".route" || c == ".git" || c == ".."),
        "INVALID_PATH"
    );
    Ok(())
}

fn validate_steps(steps: &[ContractStep]) -> Result<()> {
    ensure!(
        !steps.is_empty() && steps.len() <= 100,
        "INVALID_STEP_COUNT"
    );
    let mut ids = BTreeSet::new();
    let mut checks = BTreeSet::new();
    for step in steps {
        nonempty("STEP_ID", &step.step_id, 80)?;
        nonempty("TITLE", &step.title, 240)?;
        nonempty("CHECK_ID", &step.proof.check_id, 240)?;
        ensure!(ids.insert(step.step_id.as_str()), "DUPLICATE_STEP_ID");
        ensure!(
            checks.insert(step.proof.check_id.as_str()),
            "DUPLICATE_CHECK_ID"
        );
        if let Some(StepCondition::PathExists { path }) = &step.condition {
            safe_scope_path(path)?;
        }
        ensure!(
            !matches!(step.requirement, Requirement::Conditional) || step.condition.is_some(),
            "CONDITION_REQUIRED"
        );
    }
    for (index, step) in steps.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for dependency in &step.dependencies {
            ensure!(
                dependency != &step.step_id
                    && ids.contains(dependency.as_str())
                    && seen.insert(dependency),
                "INVALID_DEPENDENCY"
            );
            ensure!(
                steps[..index]
                    .iter()
                    .any(|prior| &prior.step_id == dependency),
                "DEPENDENCY_ORDER_REQUIRED"
            );
        }
    }
    fn visit<'a>(
        id: &'a str,
        steps: &'a [ContractStep],
        visiting: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        ensure!(visiting.insert(id), "CYCLIC_DEPENDENCY");
        let step = steps
            .iter()
            .find(|s| s.step_id == id)
            .ok_or_else(|| anyhow!("UNKNOWN_STEP"))?;
        for dep in &step.dependencies {
            visit(dep, steps, visiting, done)?;
        }
        visiting.remove(id);
        done.insert(id);
        Ok(())
    }
    let mut done = BTreeSet::new();
    for step in steps {
        visit(&step.step_id, steps, &mut BTreeSet::new(), &mut done)?;
    }
    Ok(())
}

fn validate_spec(spec: &WorkflowSpec) -> Result<()> {
    nonempty("WORKFLOW_ID", &spec.workflow_id, 100)?;
    nonempty("INTENT", &spec.intent_ref, 100)?;
    nonempty("TITLE", &spec.title, 240)?;
    ensure!(spec.version > 0, "INVALID_VERSION");
    validate_steps(&spec.steps)
}

pub fn validate_shape(action: &ContractAction) -> Result<()> {
    ensure!(
        action.request_hash.len() == 64
            && action.request_hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "INVALID_REQUEST_HASH"
    );
    match &action.change {
        ContractChange::Created { spec } => validate_spec(spec)?,
        ContractChange::DeltaProposed { delta } => {
            nonempty("DELTA_ID", &delta.delta_id, 100)?;
            nonempty("REASON", &delta.reason, 2000)?;
            validate_steps(&delta.proposed_steps)?;
        }
        ContractChange::DeltaAccepted { new_spec, .. } => validate_spec(new_spec)?,
        ContractChange::StepFailed { reason, .. }
        | ContractChange::SkipRequested { reason, .. }
        | ContractChange::DeltaRejected { reason, .. } => nonempty("REASON", reason, 2000)?,
        _ => {}
    }
    Ok(())
}

fn versions_for(events: &[DevelopmentEvent], workflow_id: &str) -> Vec<(u64, WorkflowSpec)> {
    events
        .iter()
        .filter_map(|event| match &event.payload {
            DevelopmentEventPayload::WorkflowContract {
                action:
                    ContractAction {
                        change:
                            ContractChange::Created { spec }
                            | ContractChange::DeltaAccepted { new_spec: spec, .. },
                        ..
                    },
            } if spec.workflow_id == workflow_id => Some((event.sequence, spec.clone())),
            _ => None,
        })
        .collect()
}

fn latest(events: &[DevelopmentEvent], id: &str) -> Result<(u64, WorkflowSpec)> {
    versions_for(events, id)
        .into_iter()
        .last()
        .ok_or_else(|| anyhow!("UNKNOWN_WORKFLOW"))
}

fn delta_for(events: &[DevelopmentEvent], id: &str) -> Result<PlanDelta> {
    events
        .iter()
        .find_map(|event| match &event.payload {
            DevelopmentEventPayload::WorkflowContract {
                action:
                    ContractAction {
                        change: ContractChange::DeltaProposed { delta },
                        ..
                    },
            } if delta.delta_id == id => Some(delta.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow!("UNKNOWN_PLAN_DELTA"))
}

fn delta_decided(events: &[DevelopmentEvent], id: &str) -> bool {
    events.iter().any(|event| matches!(&event.payload,
        DevelopmentEventPayload::WorkflowContract { action: ContractAction { change: ContractChange::DeltaAccepted { delta_id, .. } | ContractChange::DeltaRejected { delta_id, .. }, .. } } if delta_id == id))
}

fn condition_true(root: &Path, condition: &StepCondition) -> Result<bool> {
    match condition {
        StepCondition::PathExists { path } => {
            safe_scope_path(path)?;
            let mut current = root.to_path_buf();
            for part in Path::new(path).components() {
                current.push(part.as_os_str());
                match std::fs::symlink_metadata(&current) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        return Err(anyhow!("CONDITION_PATH_LINK_UNSUPPORTED"))
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                    Err(e) => return Err(e.into()),
                }
            }
            Ok(true)
        }
    }
}

fn qualifying_proof(
    store: &EvidenceStore,
    spec: &WorkflowSpec,
    step: &ContractStep,
    version_revision: u64,
    evidence_ref: &str,
    current_hash: &str,
) -> Result<(bool, bool)> {
    let Some(e) = store.evidence.iter().find(|e| e.id == evidence_ref) else {
        return Ok((false, false));
    };
    let base = e.session_id == spec.intent_ref
        && e.source == EvidenceSource::System
        && e.kind == EvidenceKind::CheckPass
        && e.metadata.get("check_id") == Some(&step.proof.check_id)
        && e.metadata.get("exit_code").is_some_and(|v| v == "0")
        && e.metadata.get("state_stable").is_some_and(|v| v == "true")
        && e.metadata
            .get("check_started_revision")
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|v| v >= version_revision);
    if !base {
        return Ok((false, false));
    }
    Ok((
        e.state_hash.as_deref() == Some(current_hash),
        e.state_hash.as_deref() != Some(current_hash),
    ))
}

fn step_run(event: &DevelopmentEvent, id: &str, version: u32) -> Option<StepRun> {
    let DevelopmentEventPayload::WorkflowContract { action } = &event.payload else {
        return None;
    };
    let (workflow_id, run_version, step_id, name, evidence_ref) = match &action.change {
        ContractChange::StepStarted {
            workflow_id,
            version,
            step_id,
        } => (workflow_id, *version, step_id, "started", None),
        ContractChange::StepReported {
            workflow_id,
            version,
            step_id,
            evidence_ref,
        } => (
            workflow_id,
            *version,
            step_id,
            "reported",
            evidence_ref.clone(),
        ),
        ContractChange::StepFailed {
            workflow_id,
            version,
            step_id,
            ..
        } => (workflow_id, *version, step_id, "failed", None),
        ContractChange::SkipRequested {
            workflow_id,
            version,
            step_id,
            ..
        } => (workflow_id, *version, step_id, "skip_requested", None),
        _ => return None,
    };
    if workflow_id != id || run_version != version {
        return None;
    }
    Some(StepRun {
        step_id: step_id.clone(),
        version,
        actor_worker_id: event.actor_worker_id.clone(),
        event_revision: event.sequence,
        action: name.into(),
        evidence_ref,
    })
}

fn evaluate(
    root: &Path,
    events: &[DevelopmentEvent],
    spec: &WorkflowSpec,
    version_revision: u64,
) -> Result<(Vec<StepView>, CompletionReport)> {
    let state_hash = execution::compute_state_hash(root)?;
    let evidence_store = EvidenceStore::load(root)?;
    let mut views = Vec::new();
    let mut missing = Vec::new();
    let mut stale = Vec::new();
    let mut failed = Vec::new();
    let mut bypassed = Vec::new();
    let mut blocked = Vec::new();
    let mut actions = Vec::new();
    for step in &spec.steps {
        let latest_run = events.iter().rev().find_map(|e| {
            step_run(e, &spec.workflow_id, spec.version).filter(|r| r.step_id == step.step_id)
        });
        let mut state = StepState::Pending;
        if let Some(run) = &latest_run {
            state = match run.action.as_str() {
                "started" => StepState::Running,
                "failed" => StepState::Failed,
                "reported" => match run.evidence_ref.as_deref() {
                    Some(id) => {
                        match qualifying_proof(
                            &evidence_store,
                            spec,
                            step,
                            version_revision,
                            id,
                            &state_hash,
                        )? {
                            (true, _) => StepState::Satisfied,
                            (_, true) => StepState::Stale,
                            _ => StepState::Failed,
                        }
                    }
                    None => StepState::Failed,
                },
                "skip_requested" => {
                    let event = events
                        .iter()
                        .find(|e| e.sequence == run.event_revision)
                        .ok_or_else(|| anyhow!("MISSING_EVENT"))?;
                    if run.actor_worker_id.is_none() {
                        StepState::SkippedValid
                    } else if let DevelopmentEventPayload::WorkflowContract {
                        action:
                            ContractAction {
                                change:
                                    ContractChange::SkipRequested {
                                        condition_false_observed,
                                        ..
                                    },
                                ..
                            },
                    } = &event.payload
                    {
                        if *condition_false_observed {
                            if step
                                .condition
                                .as_ref()
                                .is_some_and(|c| condition_true(root, c).ok() == Some(false))
                            {
                                StepState::SkippedValid
                            } else {
                                StepState::Stale
                            }
                        } else {
                            StepState::Bypassed
                        }
                    } else {
                        StepState::Bypassed
                    }
                }
                _ => StepState::Pending,
            };
        }
        if !matches!(
            state,
            StepState::Failed | StepState::Bypassed | StepState::Stale
        ) && !step.dependencies.iter().all(|id| {
            views.iter().any(|v: &StepView| {
                &v.step.step_id == id
                    && matches!(v.state, StepState::Satisfied | StepState::SkippedValid)
            })
        }) {
            state = StepState::Blocked;
        }
        let required = matches!(
            step.requirement,
            Requirement::Required | Requirement::Conditional
        );
        if required {
            match state {
                StepState::Satisfied | StepState::SkippedValid => {}
                StepState::Stale => {
                    stale.push(step.step_id.clone());
                    actions.push(format!("revalidate {} against current state", step.step_id));
                }
                StepState::Bypassed => {
                    bypassed.push(step.step_id.clone());
                    actions.push(format!(
                        "obtain authorized waiver or execute {}",
                        step.step_id
                    ));
                }
                StepState::Failed => {
                    failed.push(step.step_id.clone());
                    actions.push(format!(
                        "provide qualifying System evidence for {}",
                        step.step_id
                    ));
                }
                StepState::Blocked => {
                    blocked.push(step.step_id.clone());
                    missing.push(step.step_id.clone());
                    actions.push(format!("satisfy dependencies and run {}", step.step_id));
                }
                _ => {
                    missing.push(step.step_id.clone());
                    actions.push(format!("execute and verify {}", step.step_id));
                }
            }
        }
        views.push(StepView {
            step: step.clone(),
            state,
            last_run: latest_run,
        });
    }
    if state_hash.is_empty() {
        failed.push("workspace_fingerprint_unavailable".into());
        actions.push("initialize a verifiable project state".into());
    }
    if missing.is_empty()
        && stale.is_empty()
        && failed.is_empty()
        && bypassed.is_empty()
        && blocked.is_empty()
    {
        if let Err(_) = crate::work::ensure_integrated_before_success(root, &spec.intent_ref) {
            failed.push("work.integration".into());
            actions.push("integrate completed child work with fresh System evidence".into());
        }
    }
    let completion = if missing.is_empty()
        && stale.is_empty()
        && failed.is_empty()
        && bypassed.is_empty()
        && blocked.is_empty()
    {
        CompletionDecision::Pass
    } else {
        CompletionDecision::Denied
    };
    ensure!(
        execution::compute_state_hash(root)? == state_hash,
        "WORKSPACE_CHANGED_DURING_COMPLETION_GATE"
    );
    Ok((
        views,
        CompletionReport {
            completion,
            workflow_id: spec.workflow_id.clone(),
            version: spec.version,
            state_hash,
            missing_required_steps: missing,
            stale_steps: stale,
            failed_obligations: failed,
            unauthorized_bypasses: bypassed,
            blocked_steps: blocked,
            required_next_actions: actions,
        },
    ))
}

pub fn status(root: &Path, workflow_id: &str) -> Result<WorkflowStatus> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    status_from_events(root, &ledger.events, &identity.project_id, workflow_id)
}

fn status_from_events(
    root: &Path,
    events: &[DevelopmentEvent],
    project_id: &str,
    workflow_id: &str,
) -> Result<WorkflowStatus> {
    let versions = versions_for(events, workflow_id);
    let (version_revision, spec) = versions
        .last()
        .cloned()
        .ok_or_else(|| anyhow!("UNKNOWN_WORKFLOW"))?;
    let (steps, completion) = evaluate(root, events, &spec, version_revision)?;
    let latest_run_revision = steps
        .iter()
        .filter_map(|s| s.last_run.as_ref().map(|r| r.event_revision))
        .max()
        .unwrap_or(version_revision);
    let latest_evaluation = events.iter().rev().find(|e| matches!(&e.payload,
        DevelopmentEventPayload::WorkflowContract { action: ContractAction { change: ContractChange::CompletionEvaluated { workflow_id: id, version, .. }, .. } }
        if id == workflow_id && *version == spec.version));
    let completed = completion.completion == CompletionDecision::Pass
        && latest_evaluation.is_some_and(|e| e.sequence >= latest_run_revision && matches!(&e.payload,
            DevelopmentEventPayload::WorkflowContract { action: ContractAction { change: ContractChange::CompletionEvaluated { report, .. }, .. } }
            if report.completion == CompletionDecision::Pass && report.state_hash == completion.state_hash));
    let workflow_status = if completed {
        "COMPLETE"
    } else if completion.completion == CompletionDecision::Pass {
        "READY"
    } else {
        "INCOMPLETE"
    };
    let plan_deltas = events
        .iter()
        .filter_map(|e| match &e.payload {
            DevelopmentEventPayload::WorkflowContract {
                action:
                    ContractAction {
                        change: ContractChange::DeltaProposed { delta },
                        ..
                    },
            } if delta.workflow_id == workflow_id => Some(delta.clone()),
            _ => None,
        })
        .collect();
    Ok(WorkflowStatus {
        project_id: project_id.into(),
        global_revision: events.last().map(|e| e.sequence).unwrap_or(0),
        spec,
        versions: versions.into_iter().map(|(_, s)| s).collect(),
        plan_deltas,
        steps,
        completion,
        workflow_status: workflow_status.into(),
    })
}

pub fn list(root: &Path) -> Result<Vec<WorkflowStatus>> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    let ids: BTreeSet<String> = ledger
        .events
        .iter()
        .filter_map(|e| match &e.payload {
            DevelopmentEventPayload::WorkflowContract {
                action:
                    ContractAction {
                        change: ContractChange::Created { spec },
                        ..
                    },
            } => Some(spec.workflow_id.clone()),
            _ => None,
        })
        .collect();
    ids.iter()
        .map(|id| status_from_events(root, &ledger.events, &identity.project_id, id))
        .collect()
}

pub fn validate_transition(
    root: &Path,
    events: &[DevelopmentEvent],
    _project_id: &str,
    actor: Option<&str>,
    action: &ContractAction,
) -> Result<()> {
    ensure!(
        action.expected_revision == events.last().map(|e| e.sequence).unwrap_or(0),
        "STALE_CONTEXT"
    );
    match &action.change {
        ContractChange::Created { spec } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            ensure!(
                spec.version == 1 && versions_for(events, &spec.workflow_id).is_empty(),
                "WORKFLOW_ALREADY_EXISTS"
            );
            ensure!(!events.iter().any(|e| matches!(&e.payload, DevelopmentEventPayload::WorkflowContract { action: ContractAction { change: ContractChange::Created { spec: old }, .. } } if old.intent_ref == spec.intent_ref)), "INTENT_CONTRACT_EXISTS");
            let store = SessionStore::load(root)?;
            ensure!(
                store
                    .get(&spec.intent_ref)
                    .is_some_and(|s| s.status == SessionStatus::Active)
                    || crate::general_work::goal_active(events, &spec.intent_ref),
                "ACTIVE_INTENT_OR_GOAL_REQUIRED"
            );
        }
        ContractChange::StepStarted {
            workflow_id,
            version,
            step_id,
        }
        | ContractChange::StepReported {
            workflow_id,
            version,
            step_id,
            ..
        }
        | ContractChange::StepFailed {
            workflow_id,
            version,
            step_id,
            ..
        }
        | ContractChange::SkipRequested {
            workflow_id,
            version,
            step_id,
            ..
        } => {
            let (_, spec) = latest(events, workflow_id)?;
            ensure!(*version == spec.version, "STALE_WORKFLOW_VERSION");
            let step = spec
                .steps
                .iter()
                .find(|s| s.step_id == *step_id)
                .ok_or_else(|| anyhow!("UNKNOWN_STEP"))?;
            if let ContractChange::SkipRequested {
                condition_false_observed,
                ..
            } = &action.change
            {
                if actor.is_some() {
                    ensure!(
                        *condition_false_observed
                            == step
                                .condition
                                .as_ref()
                                .is_some_and(|c| condition_true(root, c).ok() == Some(false)),
                        "CONDITION_OBSERVATION_MISMATCH"
                    );
                } else {
                    ensure!(
                        !condition_false_observed,
                        "OPERATOR_WAIVER_DOES_NOT_NEED_CONDITION"
                    );
                }
            } else {
                ensure!(actor.is_some(), "WORKER_BINDING_REQUIRED");
            }
        }
        ContractChange::DeltaProposed { delta } => {
            let (_, current) = latest(events, &delta.workflow_id)?;
            ensure!(
                delta.from_version == current.version && !delta_decided(events, &delta.delta_id),
                "STALE_WORKFLOW_VERSION"
            );
            ensure!(!events.iter().any(|e| matches!(&e.payload, DevelopmentEventPayload::WorkflowContract { action: ContractAction { change: ContractChange::DeltaProposed { delta: old }, .. } } if old.delta_id == delta.delta_id)), "DELTA_ID_CONFLICT");
        }
        ContractChange::DeltaAccepted { delta_id, new_spec } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            let delta = delta_for(events, delta_id)?;
            ensure!(
                !delta_decided(events, delta_id),
                "PLAN_DELTA_ALREADY_DECIDED"
            );
            let (_, current) = latest(events, &delta.workflow_id)?;
            ensure!(
                delta.from_version == current.version
                    && new_spec.workflow_id == current.workflow_id
                    && new_spec.intent_ref == current.intent_ref
                    && new_spec.title == current.title
                    && new_spec.mode == current.mode
                    && new_spec.version == current.version + 1
                    && new_spec.steps == delta.proposed_steps,
                "INVALID_PLAN_DELTA_ACCEPTANCE"
            );
        }
        ContractChange::DeltaRejected { delta_id, .. } => {
            ensure!(actor.is_none(), "OPERATOR_REQUIRED");
            delta_for(events, delta_id)?;
            ensure!(
                !delta_decided(events, delta_id),
                "PLAN_DELTA_ALREADY_DECIDED"
            );
        }
        ContractChange::CompletionEvaluated {
            workflow_id,
            version,
            report,
        } => {
            let (_, spec) = latest(events, workflow_id)?;
            ensure!(*version == spec.version, "STALE_WORKFLOW_VERSION");
            let computed = status_from_events(
                root,
                events,
                &events.last().map(|e| e.project_id.as_str()).unwrap_or(""),
                workflow_id,
            )?
            .completion;
            ensure!(&computed == report, "COMPLETION_GATE_MISMATCH");
        }
    }
    Ok(())
}

fn action_from_request(
    root: &Path,
    method: &str,
    params: Value,
    key: &str,
    scope: &str,
) -> Result<ContractAction> {
    let request_hash = route_core::sha256_hex(
        format!("{method}:{scope}:{}", serde_json::to_string(&params)?).as_bytes(),
    );
    let expected_revision = params
        .get("expected_revision")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("EXPECTED_REVISION_REQUIRED"))?;
    let mut input = params
        .as_object()
        .ok_or_else(|| anyhow!("INVALID_PARAMS"))?
        .clone();
    input.remove("expected_revision");
    let change = match method {
        "workflow.create" => {
            let mut spec: WorkflowSpec = serde_json::from_value(
                input
                    .get("spec")
                    .cloned()
                    .ok_or_else(|| anyhow!("SPEC_REQUIRED"))?,
            )?;
            ensure!(spec.version <= 1, "INVALID_INITIAL_WORKFLOW_VERSION");
            spec.version = 1;
            ContractChange::Created { spec }
        }
        "workflow.step.start" => serde_json::from_value(
            json!({"operation":"step_started","workflow_id":input.get("workflow_id"),"version":input.get("version"),"step_id":input.get("step_id")}),
        )?,
        "workflow.step.complete" => serde_json::from_value(
            json!({"operation":"step_reported","workflow_id":input.get("workflow_id"),"version":input.get("version"),"step_id":input.get("step_id"),"evidence_ref":input.get("evidence_ref")}),
        )?,
        "workflow.step.fail" => serde_json::from_value(
            json!({"operation":"step_failed","workflow_id":input.get("workflow_id"),"version":input.get("version"),"step_id":input.get("step_id"),"reason":input.get("reason")}),
        )?,
        "workflow.step.skip" => {
            let id = input
                .get("workflow_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("WORKFLOW_ID_REQUIRED"))?;
            let step_id = input
                .get("step_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("STEP_ID_REQUIRED"))?;
            let (_, spec) = latest(&development::load_ledger_readonly(root)?.0.events, id)?;
            let step = spec
                .steps
                .iter()
                .find(|s| s.step_id == step_id)
                .ok_or_else(|| anyhow!("UNKNOWN_STEP"))?;
            let false_observed = scope != "operator"
                && step
                    .condition
                    .as_ref()
                    .is_some_and(|c| condition_true(root, c).ok() == Some(false));
            serde_json::from_value(
                json!({"operation":"skip_requested","workflow_id":id,"version":input.get("version"),"step_id":step_id,"reason":input.get("reason"),"condition_false_observed":false_observed}),
            )?
        }
        "workflow.plan_delta.propose" => {
            let mut delta: PlanDelta = serde_json::from_value(
                input
                    .get("delta")
                    .cloned()
                    .ok_or_else(|| anyhow!("DELTA_REQUIRED"))?,
            )?;
            delta.delta_id = format!("delta-{}", route_core::sha256_hex(key.as_bytes()));
            ContractChange::DeltaProposed { delta }
        }
        "workflow.plan_delta.accept" => {
            let id = input
                .get("delta_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("DELTA_ID_REQUIRED"))?;
            let events = &development::load_ledger_readonly(root)?.0.events;
            let delta = delta_for(events, id)?;
            let (_, mut spec) = latest(events, &delta.workflow_id)?;
            spec.version += 1;
            spec.steps = delta.proposed_steps;
            ContractChange::DeltaAccepted {
                delta_id: id.into(),
                new_spec: spec,
            }
        }
        "workflow.plan_delta.reject" => serde_json::from_value(
            json!({"operation":"delta_rejected","delta_id":input.get("delta_id"),"reason":input.get("reason")}),
        )?,
        "workflow.complete.request" => {
            let id = input
                .get("workflow_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("WORKFLOW_ID_REQUIRED"))?;
            let status = self::status(root, id)?;
            ContractChange::CompletionEvaluated {
                workflow_id: id.into(),
                version: status.spec.version,
                report: status.completion,
            }
        }
        _ => return Err(anyhow!("UNKNOWN_WORKFLOW_METHOD")),
    };
    Ok(ContractAction {
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
    let scope = caller.scope();
    let operator = caller.worker_id().is_none();
    if operator {
        caller.require_operator()?;
    }
    if matches!(
        method,
        "workflow.create" | "workflow.plan_delta.accept" | "workflow.plan_delta.reject"
    ) {
        ensure!(operator, "OPERATOR_REQUIRED");
    }
    if matches!(
        method,
        "workflow.step.start" | "workflow.step.complete" | "workflow.step.fail"
    ) {
        ensure!(!operator, "WORKER_BINDING_REQUIRED");
    }
    let action = action_from_request(root, method, params, key, &scope)?;
    let payload = DevelopmentEventPayload::WorkflowContract { action };
    let draft: DevelopmentEventDraft = serde_json::from_value(
        json!({"actor_worker_id":caller.worker_id(),"payload":payload,"deduplication_key":key,"source_refs":[format!("caller:{scope}")]}),
    )?;
    if operator {
        development::append_operator_contract_event(root, draft)
    } else {
        development::append_authenticated_event(root, draft, caller)
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

pub fn ensure_completed_before_session_success(root: &Path, intent_ref: &str) -> Result<()> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    let ids: Vec<String> = ledger
        .events
        .iter()
        .filter_map(|e| match &e.payload {
            DevelopmentEventPayload::WorkflowContract {
                action:
                    ContractAction {
                        change: ContractChange::Created { spec },
                        ..
                    },
            } if spec.intent_ref == intent_ref => Some(spec.workflow_id.clone()),
            _ => None,
        })
        .collect();
    for id in ids {
        let status = status_from_events(root, &ledger.events, &identity.project_id, &id)?;
        ensure!(
            status.workflow_status == "COMPLETE",
            "WORKFLOW_COMPLETION_REQUIRED: {id}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApplyTarget, ExecutionSession};

    fn fixture() -> (tempfile::TempDir, String, CallerContext, CallerContext) {
        let root = tempfile::tempdir().unwrap();
        crate::repository::BasicRepository::init(root.path()).unwrap();
        let mut sessions = SessionStore::default();
        let session = ExecutionSession::new(
            "Contract test",
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
        let operator = CallerContext::trusted_operator();
        let mut workers = Vec::new();
        for id in ["worker-a", "worker-b"] {
            crate::principal::register_worker(
                root.path(),
                &operator,
                Some(id.into()),
                Default::default(),
                &format!("register-{id}"),
            )
            .unwrap();
            let secret = crate::principal::generate_credential().unwrap();
            crate::principal::issue(
                root.path(),
                &operator,
                id,
                &route_core::sha256_hex(secret.as_bytes()),
                vec![
                    "workflow.step.start".into(),
                    "workflow.step.complete".into(),
                    "workflow.step.fail".into(),
                    "workflow.step.skip".into(),
                    "workflow.plan_delta.propose".into(),
                    "workflow.complete.request".into(),
                ],
                &format!("bind-{id}"),
            )
            .unwrap();
            workers.push(CallerContext::authenticate(root.path(), &secret).unwrap());
        }
        (root, intent, workers.remove(0), workers.remove(0))
    }

    fn revision(root: &Path) -> u64 {
        development::global_development_revision(root).unwrap()
    }
    fn step(id: &str, requirement: Requirement, condition: Option<StepCondition>) -> ContractStep {
        ContractStep {
            step_id: id.into(),
            title: format!("Step {id}"),
            requirement,
            dependencies: vec![],
            condition,
            proof: ProofObligation {
                check_id: format!("check-{id}"),
            },
        }
    }
    fn spec(intent: &str, workflow_id: &str, steps: Vec<ContractStep>) -> WorkflowSpec {
        WorkflowSpec {
            workflow_id: workflow_id.into(),
            intent_ref: intent.into(),
            version: 1,
            title: "Bounded workflow".into(),
            mode: ContractMode::FullPower,
            steps,
        }
    }
    fn create(root: &Path, intent: &str, steps: Vec<ContractStep>) {
        operator_action(
            root,
            &CallerContext::trusted_operator(),
            "workflow.create",
            json!({"expected_revision":revision(root),"spec":spec(intent,"wf-test",steps)}),
            "create",
        )
        .unwrap();
    }
    fn worker(
        root: &Path,
        caller: &CallerContext,
        method: &str,
        params: Value,
        key: &str,
    ) -> Result<AppendDevelopmentEventResult> {
        let mut params = params;
        params["expected_revision"] = json!(revision(root));
        worker_action(root, caller, method, params, key)
    }
    fn proof(root: &Path, intent: &str, id: &str) -> String {
        let argv = vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--list".into(),
        ];
        let check =
            execution::exec_command(root, intent, &argv, Some(&format!("check-{id}"))).unwrap();
        assert_eq!(check.exit_code, 0);
        assert!(check.state_stable);
        EvidenceStore::load(root)
            .unwrap()
            .evidence
            .into_iter()
            .rev()
            .find(|e| e.metadata.get("check_id") == Some(&format!("check-{id}")))
            .unwrap()
            .id
    }
    fn report(
        root: &Path,
        caller: &CallerContext,
        id: &str,
        evidence_ref: Option<&str>,
        key: &str,
    ) {
        worker(
            root,
            caller,
            "workflow.step.complete",
            json!({"workflow_id":"wf-test","version":1,"step_id":id,"evidence_ref":evidence_ref}),
            key,
        )
        .unwrap();
    }

    #[test]
    fn missing_middle_step_claim_only_stale_and_revalidation() {
        let (root, intent, a, _) = fixture();
        let path = root.path();
        create(
            path,
            &intent,
            ["A", "B", "C", "D"]
                .iter()
                .map(|id| step(id, Requirement::Required, None))
                .collect(),
        );
        for id in ["A", "B", "D"] {
            let evidence = proof(path, &intent, id);
            report(path, &a, id, Some(&evidence), &format!("report-{id}"));
        }
        let initial = status(path, "wf-test").unwrap();
        assert_eq!(initial.completion.completion, CompletionDecision::Denied);
        assert_eq!(initial.completion.missing_required_steps, ["C"]);
        assert_eq!(initial.steps[3].state, StepState::Satisfied);
        let denied = worker(
            path,
            &a,
            "workflow.complete.request",
            json!({"workflow_id":"wf-test"}),
            "denied-missing",
        )
        .unwrap();
        assert!(matches!(
            denied.event.payload,
            DevelopmentEventPayload::WorkflowContract {
                action: ContractAction {
                    change: ContractChange::CompletionEvaluated {
                        report: CompletionReport {
                            completion: CompletionDecision::Denied,
                            ..
                        },
                        ..
                    },
                    ..
                }
            }
        ));
        report(path, &a, "C", None, "claim-only");
        let claimed = status(path, "wf-test").unwrap();
        assert_eq!(claimed.steps[2].state, StepState::Failed);
        assert_eq!(claimed.completion.completion, CompletionDecision::Denied);
        let evidence = proof(path, &intent, "C");
        report(path, &a, "C", Some(&evidence), "verified-C");
        assert_eq!(
            status(path, "wf-test").unwrap().completion.completion,
            CompletionDecision::Pass
        );
        worker(
            path,
            &a,
            "workflow.complete.request",
            json!({"workflow_id":"wf-test"}),
            "complete-first",
        )
        .unwrap();
        assert_eq!(status(path, "wf-test").unwrap().workflow_status, "COMPLETE");
        std::fs::write(path.join("candidate.txt"), b"changed after proof").unwrap();
        let stale = status(path, "wf-test").unwrap();
        assert_eq!(stale.steps[2].state, StepState::Stale);
        assert_eq!(stale.workflow_status, "INCOMPLETE");
        assert!(stale.completion.stale_steps.contains(&"C".into()));
        for id in ["A", "B", "C", "D"] {
            let fresh = proof(path, &intent, id);
            report(path, &a, id, Some(&fresh), &format!("fresh-{id}"));
        }
        assert_eq!(status(path, "wf-test").unwrap().workflow_status, "READY");
        worker(
            path,
            &a,
            "workflow.complete.request",
            json!({"workflow_id":"wf-test"}),
            "complete-again",
        )
        .unwrap();
        assert_eq!(status(path, "wf-test").unwrap().workflow_status, "COMPLETE");
    }

    #[test]
    fn plan_delta_immutable_versions_and_skip_authority() {
        let (root, intent, a, b) = fixture();
        let path = root.path();
        create(
            path,
            &intent,
            vec![
                step("A", Requirement::Required, None),
                step("C", Requirement::Required, None),
                step(
                    "X",
                    Requirement::Conditional,
                    Some(StepCondition::PathExists {
                        path: "absent.txt".into(),
                    }),
                ),
            ],
        );
        let attacker = worker(
            path,
            &a,
            "workflow.create",
            json!({"spec":spec(&intent,"wf-attack",vec![step("A",Requirement::Required,None)])}),
            "silent-create",
        );
        assert!(attacker.is_err());
        let forged: DevelopmentEventDraft = serde_json::from_value(json!({"payload":DevelopmentEventPayload::WorkflowContract { action: ContractAction { request_hash: "a".repeat(64), expected_revision: revision(path), change: ContractChange::Created { spec: spec(&intent,"wf-forged",vec![step("A",Requirement::Required,None)]) } } }})).unwrap();
        assert!(development::append_development_event(path, forged).is_err());
        worker(
            path,
            &a,
            "workflow.step.skip",
            json!({"workflow_id":"wf-test","version":1,"step_id":"C","reason":"skip to finish"}),
            "bad-skip",
        )
        .unwrap();
        assert_eq!(
            status(path, "wf-test").unwrap().steps[1].state,
            StepState::Bypassed
        );
        worker(path, &b, "workflow.step.skip", json!({"workflow_id":"wf-test","version":1,"step_id":"X","reason":"condition is false"}), "conditional-skip").unwrap();
        assert_eq!(
            status(path, "wf-test").unwrap().steps[2].state,
            StepState::SkippedValid
        );
        let delta = PlanDelta {
            delta_id: String::new(),
            workflow_id: "wf-test".into(),
            from_version: 1,
            reason: "C replaced by stronger A verification".into(),
            proposed_steps: vec![
                step("A", Requirement::Required, None),
                step(
                    "X",
                    Requirement::Conditional,
                    Some(StepCondition::PathExists {
                        path: "absent.txt".into(),
                    }),
                ),
            ],
        };
        let proposed = worker(
            path,
            &a,
            "workflow.plan_delta.propose",
            json!({"delta":delta}),
            "remove-C-proposal",
        )
        .unwrap();
        let delta_id = match proposed.event.payload {
            DevelopmentEventPayload::WorkflowContract {
                action:
                    ContractAction {
                        change: ContractChange::DeltaProposed { delta },
                        ..
                    },
            } => delta.delta_id,
            _ => panic!("delta"),
        };
        assert_eq!(status(path, "wf-test").unwrap().spec.version, 1);
        assert!(status(path, "wf-test")
            .unwrap()
            .spec
            .steps
            .iter()
            .any(|s| s.step_id == "C"));
        assert!(worker(
            path,
            &a,
            "workflow.plan_delta.accept",
            json!({"delta_id":delta_id}),
            "worker-accept"
        )
        .is_err());
        operator_action(
            path,
            &CallerContext::trusted_operator(),
            "workflow.plan_delta.accept",
            json!({"expected_revision":revision(path),"delta_id":delta_id}),
            "operator-accept",
        )
        .unwrap();
        let current = status(path, "wf-test").unwrap();
        assert_eq!(current.spec.version, 2);
        assert_eq!(current.versions.len(), 2);
        assert!(current.versions[0].steps.iter().any(|s| s.step_id == "C"));
        assert!(!current.versions[1].steps.iter().any(|s| s.step_id == "C"));
        assert!(operator_action(
            path,
            &CallerContext::trusted_operator(),
            "workflow.plan_delta.accept",
            json!({"expected_revision":revision(path),"delta_id":delta_id}),
            "second-accept"
        )
        .is_err());
    }

    #[test]
    fn authorized_waiver_and_concurrent_revision_winner() {
        let (root, intent, a, b) = fixture();
        let path = root.path();
        create(path, &intent, vec![step("C", Requirement::Required, None)]);
        worker(
            path,
            &a,
            "workflow.step.skip",
            json!({"workflow_id":"wf-test","version":1,"step_id":"C","reason":"AI shortcut"}),
            "unauthorized",
        )
        .unwrap();
        assert_eq!(
            status(path, "wf-test")
                .unwrap()
                .completion
                .unauthorized_bypasses,
            ["C"]
        );
        operator_action(path,&CallerContext::trusted_operator(),"workflow.step.skip",json!({"expected_revision":revision(path),"workflow_id":"wf-test","version":1,"step_id":"C","reason":"Human-authorized waiver"}),"authorized").unwrap();
        assert_eq!(
            status(path, "wf-test").unwrap().steps[0].state,
            StepState::SkippedValid
        );
        let expected = revision(path);
        let root_a = path.to_path_buf();
        let root_b = path.to_path_buf();
        let first = std::thread::spawn(move || {
            worker_action(
                &root_a,
                &a,
                "workflow.step.start",
                json!({"expected_revision":expected,"workflow_id":"wf-test","version":1,"step_id":"C"}),
                "race-a",
            )
        });
        let second = std::thread::spawn(move || {
            worker_action(
                &root_b,
                &b,
                "workflow.step.start",
                json!({"expected_revision":expected,"workflow_id":"wf-test","version":1,"step_id":"C"}),
                "race-b",
            )
        });
        assert_eq!(
            [
                first.join().unwrap().is_ok(),
                second.join().unwrap().is_ok()
            ]
            .into_iter()
            .filter(|ok| *ok)
            .count(),
            1
        );
    }

    #[test]
    fn one_proof_cannot_cover_two_steps_and_unmet_dependency_blocks_downstream() {
        let duplicate = vec![
            step("A", Requirement::Required, None),
            ContractStep {
                step_id: "B".into(),
                title: "B".into(),
                requirement: Requirement::Required,
                dependencies: vec![],
                condition: None,
                proof: ProofObligation {
                    check_id: "check-A".into(),
                },
            },
        ];
        assert!(validate_steps(&duplicate).is_err());

        let (root, intent, worker, _) = fixture();
        let path = root.path();
        let mut dependent = step("B", Requirement::Required, None);
        dependent.dependencies.push("A".into());
        create(
            path,
            &intent,
            vec![step("A", Requirement::Optional, None), dependent],
        );
        let proof = proof(path, &intent, "B");
        report(path, &worker, "B", Some(&proof), "reported-B");
        let state = status(path, "wf-test").unwrap();
        assert_eq!(state.steps[1].state, StepState::Blocked);
        assert_eq!(state.completion.blocked_steps, ["B"]);
    }

    #[cfg(windows)]
    #[test]
    fn zero_exit_with_world_mutation_is_not_qualifying_proof() {
        let (root, intent, worker, _) = fixture();
        let path = root.path();
        create(path, &intent, vec![step("C", Requirement::Required, None)]);
        let command = vec![
            "cmd.exe".to_string(),
            "/C".to_string(),
            format!("echo changed>{}", path.join("mutated.txt").display()),
        ];
        let check = execution::exec_command(path, &intent, &command, Some("check-C")).unwrap();
        assert_eq!(check.exit_code, 0);
        assert!(!check.state_stable);
        let evidence = EvidenceStore::load(path).unwrap();
        let last = evidence.evidence.last().unwrap();
        assert_eq!(last.kind, EvidenceKind::CheckFail);
        report(path, &worker, "C", Some(&last.id), "unstable-check");
        let state = status(path, "wf-test").unwrap();
        assert_eq!(state.steps[0].state, StepState::Failed);
        assert_eq!(state.completion.completion, CompletionDecision::Denied);
    }

    #[test]
    #[ignore = "manual bounded performance probe"]
    fn bounded_performance_probe() {
        fn durable_bytes(path: &Path) -> u64 {
            std::fs::read_dir(path)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    let kind = entry.file_type().unwrap();
                    if kind.is_dir() {
                        durable_bytes(&entry.path())
                    } else {
                        entry.metadata().unwrap().len()
                    }
                })
                .sum()
        }
        fn quantile(mut values: Vec<u128>, numerator: usize) -> u128 {
            values.sort_unstable();
            values[(values.len() * numerator / 100).min(values.len() - 1)]
        }
        for count in [10, 100] {
            let (root, intent, worker, _) = fixture();
            let path = root.path();
            let before = durable_bytes(&path.join(".route"));
            create(
                path,
                &intent,
                (0..count)
                    .map(|i| step(&format!("S{i:03}"), Requirement::Required, None))
                    .collect(),
            );
            for i in 0..count {
                let id = format!("S{i:03}");
                report(path, &worker, &id, None, &format!("claim-{id}"));
            }
            let after_write = durable_bytes(&path.join(".route"));
            let mut status_ms = Vec::new();
            let mut history_ms = Vec::new();
            for _ in 0..20 {
                let start = std::time::Instant::now();
                let current = status(path, "wf-test").unwrap();
                assert_eq!(current.completion.completion, CompletionDecision::Denied);
                status_ms.push(start.elapsed().as_millis());
                let start = std::time::Instant::now();
                let (ledger, _, _) = development::load_ledger_readonly(path).unwrap();
                assert!(ledger.events.len() >= count);
                history_ms.push(start.elapsed().as_millis());
            }
            let after_read = durable_bytes(&path.join(".route"));
            let peak_memory = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!("(Get-Process -Id {}).PeakWorkingSet64", std::process::id()),
                ])
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .and_then(|value| value.trim().parse::<u64>().ok());
            println!(
                "RESOURCE count={count} status_gate_p50_ms={} status_gate_p95_ms={} history_p50_ms={} history_p95_ms={} creation_growth_bytes={} read_growth_bytes={} process_peak_working_set_bytes={peak_memory:?}",
                quantile(status_ms.clone(), 50),
                quantile(status_ms, 95),
                quantile(history_ms.clone(), 50),
                quantile(history_ms, 95),
                after_write - before,
                after_read - after_write
            );
        }
    }
}
