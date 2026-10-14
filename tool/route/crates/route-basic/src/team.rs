//! Bounded, goal-local coordination. The Development ledger is the only truth.
//! Reservations include failed launches; only the broker may consume capacity.
use crate::development::{self, DevelopmentEvent, DevelopmentEventPayload as Payload};
use crate::principal::CallerContext;
use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentBudget {
    pub max_concurrent_workers: usize,
    pub max_workers_per_goal: usize,
    pub max_spawn_depth: usize,
    pub max_spawn_attempts: usize,
    pub worker_timeout_seconds: u64,
    pub retry_limit: usize,
}
impl Default for AgentBudget {
    fn default() -> Self {
        Self {
            max_concurrent_workers: 2,
            max_workers_per_goal: 3,
            max_spawn_depth: 1,
            max_spawn_attempts: 4,
            worker_timeout_seconds: 900,
            retry_limit: 1,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    Planner,
    Implementer,
    IndependentReviewer,
    Knowledge,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RunState {
    Reserved,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    /// Operator explicitly released a terminated Run whose result was lost.
    LostResult,
}
impl RunState {
    pub fn active(&self) -> bool {
        matches!(self, Self::Reserved | Self::Running)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoutingDecision {
    pub goal_id: String,
    pub split: bool,
    pub role: Role,
    pub work_ids: Vec<String>,
    pub reason: String,
    pub independence_required: bool,
    pub capability_status: crate::cooperation::EpistemicStatus,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub run_id: String,
    pub worker_id: String,
    pub parent_run_id: Option<String>,
    pub work_id: Option<String>,
    pub role: Role,
    pub grants: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum TeamAction {
    QueueExtraction {
        goal_id: String,
        job_id: String,
        finding_event_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        positive_packet: Option<crate::positive_base::ExtractionPacket>,
    },
    FinishExtraction {
        goal_id: String,
        job_id: String,
        succeeded: bool,
    },
    Configure {
        goal_id: String,
        budget: AgentBudget,
        host_path: String,
        host_sha256: String,
    },
    Decide {
        decision: RoutingDecision,
    },
    Reserve {
        goal_id: String,
        reservation: Reservation,
    },
    Transition {
        goal_id: String,
        run_id: String,
        state: RunState,
    },
    CancelAll {
        goal_id: String,
    },
}
impl TeamAction {
    pub fn goal_id(&self) -> &str {
        match self {
            Self::Decide { decision } => &decision.goal_id,
            Self::Configure { goal_id, .. }
            | Self::Reserve { goal_id, .. }
            | Self::Transition { goal_id, .. }
            | Self::CancelAll { goal_id }
            | Self::QueueExtraction { goal_id, .. }
            | Self::FinishExtraction { goal_id, .. } => goal_id,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct RunView {
    pub reservation: Reservation,
    pub state: RunState,
    pub depth: usize,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct TeamView {
    pub knowledge_jobs: BTreeMap<String, ExtractionJob>,
    pub goal_id: String,
    pub budget: AgentBudget,
    pub host_path: String,
    pub host_sha256: String,
    pub cancelled: bool,
    pub decisions: Vec<RoutingDecision>,
    pub runs: BTreeMap<String, RunView>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ExtractionJob {
    pub finding_event_id: String,
    pub state: String,
}
pub(crate) fn project(events: &[DevelopmentEvent], goal: &str) -> Option<TeamView> {
    let mut view: Option<TeamView> = None;
    for e in events {
        let Payload::Team { action } = &e.payload else {
            continue;
        };
        if action.goal_id() != goal {
            continue;
        }
        match action {
            TeamAction::Configure {
                goal_id,
                budget,
                host_path,
                host_sha256,
            } => {
                view = Some(TeamView {
                    goal_id: goal_id.clone(),
                    budget: budget.clone(),
                    host_path: host_path.clone(),
                    host_sha256: host_sha256.clone(),
                    cancelled: false,
                    decisions: vec![],
                    runs: BTreeMap::new(),
                    knowledge_jobs: BTreeMap::new(),
                })
            }
            TeamAction::QueueExtraction {
                job_id,
                finding_event_id,
                ..
            } => {
                if let Some(v) = &mut view {
                    v.knowledge_jobs.insert(
                        job_id.clone(),
                        ExtractionJob {
                            finding_event_id: finding_event_id.clone(),
                            state: "PENDING".into(),
                        },
                    );
                }
            }
            TeamAction::FinishExtraction {
                job_id, succeeded, ..
            } => {
                if let Some(j) = view.as_mut().and_then(|v| v.knowledge_jobs.get_mut(job_id)) {
                    j.state = if *succeeded { "SUCCEEDED" } else { "FAILED" }.into();
                }
            }
            TeamAction::Decide { decision } => {
                if let Some(v) = &mut view {
                    v.decisions.push(decision.clone())
                }
            }
            TeamAction::Reserve { reservation, .. } => {
                if let Some(v) = &mut view {
                    let depth = reservation
                        .parent_run_id
                        .as_ref()
                        .and_then(|id| v.runs.get(id))
                        .map_or(0, |p| p.depth + 1);
                    v.runs.insert(
                        reservation.run_id.clone(),
                        RunView {
                            reservation: reservation.clone(),
                            state: RunState::Reserved,
                            depth,
                            revision: e.sequence,
                        },
                    );
                }
            }
            TeamAction::Transition { run_id, state, .. } => {
                if let Some(run) = view.as_mut().and_then(|v| v.runs.get_mut(run_id)) {
                    run.state = state.clone();
                    run.revision = e.sequence
                }
            }
            TeamAction::CancelAll { .. } => {
                if let Some(v) = &mut view {
                    v.cancelled = true;
                    // Running processes retain their slot until the owning broker reaps them.
                    for run in v
                        .runs
                        .values_mut()
                        .filter(|r| r.state == RunState::Reserved)
                    {
                        run.state = RunState::Cancelled;
                        run.revision = e.sequence
                    }
                }
            }
        }
    }
    view
}
pub fn status(root: &Path, goal: &str) -> Result<TeamView> {
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    project(&ledger.events, goal).ok_or_else(|| anyhow!("TEAM_NOT_CONFIGURED"))
}

/// Project-wide persisted reservations, not OS process liveness or permission.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ProjectConcurrencySnapshot {
    pub revision: u64,
    pub active_workers: usize,
    pub effective_concurrent_limit: usize,
    pub remaining_concurrent_slots: usize,
}

fn project_concurrency(
    events: &[DevelopmentEvent],
    candidate: &TeamView,
) -> ProjectConcurrencySnapshot {
    // Borrow IDs from this immutable snapshot. Do not clone complete TeamViews,
    // routing decisions, host paths or extraction jobs just to count slots.
    #[derive(Clone, Copy)]
    enum CapacityState {
        Reserved,
        Running,
        Terminal,
    }
    struct CapacityTeam<'a> {
        limit: usize,
        runs: BTreeMap<&'a str, CapacityState>,
    }
    let mut teams: BTreeMap<&str, CapacityTeam<'_>> = BTreeMap::new();
    for event in events {
        let Payload::Team { action } = &event.payload else {
            continue;
        };
        match action {
            TeamAction::Configure {
                goal_id, budget, ..
            } => {
                // Full replay resets the team on Configure, including all runs.
                teams.insert(
                    goal_id,
                    CapacityTeam {
                        limit: budget.max_concurrent_workers,
                        runs: BTreeMap::new(),
                    },
                );
            }
            TeamAction::Reserve {
                goal_id,
                reservation,
            } => {
                if let Some(team) = teams.get_mut(goal_id.as_str()) {
                    team.runs
                        .insert(&reservation.run_id, CapacityState::Reserved);
                }
            }
            TeamAction::Transition {
                goal_id,
                run_id,
                state,
            } => {
                if let Some(run) = teams
                    .get_mut(goal_id.as_str())
                    .and_then(|team| team.runs.get_mut(run_id.as_str()))
                {
                    *run = match state {
                        RunState::Reserved => CapacityState::Reserved,
                        RunState::Running => CapacityState::Running,
                        _ => CapacityState::Terminal,
                    };
                }
            }
            TeamAction::CancelAll { goal_id } => {
                if let Some(team) = teams.get_mut(goal_id.as_str()) {
                    for run in team.runs.values_mut() {
                        if matches!(run, CapacityState::Reserved) {
                            *run = CapacityState::Terminal;
                        }
                    }
                }
            }
            TeamAction::Decide { .. }
            | TeamAction::QueueExtraction { .. }
            | TeamAction::FinishExtraction { .. } => {}
        }
    }
    let mut active_workers = 0;
    let mut effective_concurrent_limit = candidate.budget.max_concurrent_workers;
    for team in teams.values() {
        let active = team
            .runs
            .values()
            .filter(|state| !matches!(state, CapacityState::Terminal))
            .count();
        active_workers += active;
        if active != 0 {
            effective_concurrent_limit = effective_concurrent_limit.min(team.limit);
        }
    }
    ProjectConcurrencySnapshot {
        revision: events.last().map(|e| e.sequence).unwrap_or(0),
        active_workers,
        effective_concurrent_limit,
        remaining_concurrent_slots: effective_concurrent_limit.saturating_sub(active_workers),
    }
}

// Independent legacy full replay oracle; intentionally not used by production.
#[cfg(test)]
fn project_concurrency_full_replay(
    events: &[DevelopmentEvent],
    candidate: &TeamView,
) -> ProjectConcurrencySnapshot {
    let goals: std::collections::BTreeSet<_> = events
        .iter()
        .filter_map(|e| match &e.payload {
            Payload::Team {
                action: TeamAction::Configure { goal_id, .. },
            } => Some(goal_id),
            _ => None,
        })
        .collect();
    let views: Vec<_> = goals.iter().filter_map(|g| project(events, g)).collect();
    let active_workers = views
        .iter()
        .map(|v| v.runs.values().filter(|r| r.state.active()).count())
        .sum();
    let effective_concurrent_limit = views
        .iter()
        .filter(|v| v.runs.values().any(|r| r.state.active()))
        .map(|v| v.budget.max_concurrent_workers)
        .chain(std::iter::once(candidate.budget.max_concurrent_workers))
        .min()
        .unwrap();
    ProjectConcurrencySnapshot {
        revision: events.last().map(|e| e.sequence).unwrap_or(0),
        active_workers,
        effective_concurrent_limit,
        remaining_concurrent_slots: effective_concurrent_limit.saturating_sub(active_workers),
    }
}

/// One verified ledger snapshot supplies both Goal and project capacity.
/// Reservation validation re-evaluates this under its authoritative write boundary.
pub fn status_with_project_concurrency(
    root: &Path,
    goal: &str,
) -> Result<(TeamView, ProjectConcurrencySnapshot)> {
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    let view = project(&ledger.events, goal).ok_or_else(|| anyhow!("TEAM_NOT_CONFIGURED"))?;
    let capacity = project_concurrency(&ledger.events, &view);
    Ok((view, capacity))
}
pub fn role_grants(role: &Role) -> Vec<String> {
    let mut grants = vec![
        "work.claim",
        "work.finish",
        "work.release",
        "work.interrupt",
        "worker.message.send",
        "development.event.record",
    ];
    match role {
        Role::Planner => grants.extend(["work.create_child", "team.decide"]),
        Role::Implementer => {
            grants.extend(["artifact.register", "observation.record", "outcome.record"])
        }
        Role::IndependentReviewer => grants.push("observation.record"),
        Role::Knowledge => {
            grants = vec!["cooperation.knowledge.record"];
        }
    }
    grants.into_iter().map(String::from).collect()
}
/// Validate routing intent, not permission, capacity, capability or model choice.
pub fn validate_routing_mode(decision: &RoutingDecision) -> Result<()> {
    ensure!(
        matches!((decision.split, &decision.role),
            (false, Role::Planner) | (true, Role::Implementer | Role::IndependentReviewer)),
        "INVALID_ROUTING_MODE: split=false requires PLANNER; split=true requires IMPLEMENTER or INDEPENDENT_REVIEWER"
    );
    Ok(())
}

pub(crate) fn validate_shape(action: &TeamAction) -> Result<()> {
    ensure!(
        !action.goal_id().is_empty() && action.goal_id().len() <= 128,
        "INVALID_GOAL_ID"
    );
    ensure!(
        serde_json::to_vec(action)?.len() <= 8192,
        "TEAM_INPUT_TOO_LARGE"
    );
    if let TeamAction::Configure {
        budget: b,
        host_path,
        host_sha256,
        ..
    } = action
    {
        ensure!(
            (1..=8).contains(&b.max_concurrent_workers)
                && (1..=16).contains(&b.max_workers_per_goal)
                && b.max_concurrent_workers <= b.max_workers_per_goal
                && b.max_spawn_depth <= 1
                && (1..=32).contains(&b.max_spawn_attempts)
                && b.retry_limit <= 2
                && (1..=3600).contains(&b.worker_timeout_seconds),
            "INVALID_AGENT_BUDGET"
        );
        ensure!(
            Path::new(host_path).is_absolute()
                && host_sha256.len() == 64
                && host_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "HOST_APPROVAL_REQUIRED"
        );
    }
    if let TeamAction::Decide { decision: d } = action {
        validate_routing_mode(d)?;
        ensure!(
            !d.reason.trim().is_empty() && d.reason.len() <= 1000 && d.work_ids.len() <= 16,
            "INVALID_ROUTING_DECISION"
        );
        ensure!(
            !d.independence_required || d.role == Role::IndependentReviewer,
            "INDEPENDENCE_ROLE_REQUIRED"
        );
    }
    Ok(())
}
/// Called under DevelopmentAppendLock, not a preflight-only check.
pub(crate) fn validate_transition(
    root: &Path,
    events: &[DevelopmentEvent],
    actor: Option<&str>,
    action: &TeamAction,
) -> Result<()> {
    let view = project(events, action.goal_id());
    if matches!(action, TeamAction::Configure { .. }) {
        ensure!(actor.is_none(), "OPERATOR_REQUIRED");
        ensure!(view.is_none(), "TEAM_ALREADY_CONFIGURED");
        ensure!(events.iter().any(|e|matches!(&e.payload,Payload::GeneralWork {action:a} if matches!(&a.change,crate::general_work::GeneralChange::GoalCreated {goal} if goal.goal_id==action.goal_id()))),"UNKNOWN_GOAL");
        return Ok(());
    }
    let v = view.ok_or_else(|| anyhow!("TEAM_NOT_CONFIGURED"))?;
    if let TeamAction::Decide { decision } = action {
        let actor = actor.ok_or_else(|| anyhow!("WORKER_BINDING_REQUIRED"))?;
        ensure!(
            v.runs.values().any(|r| r.reservation.worker_id == actor
                && r.reservation.role == Role::Planner
                && r.state == RunState::Running),
            "PLANNER_REQUIRED"
        );
        ensure!(
            !v.cancelled && v.decisions.len() < v.budget.max_spawn_attempts,
            "ROUTING_BUDGET_DENIED"
        );
        for id in &decision.work_ids {
            ensure!(events.iter().any(|e|matches!(&e.payload,Payload::Work {action:crate::work::WorkAction::ChildCreated {work}} if &work.work_id==id && work.intent_ref==v.goal_id)),"UNKNOWN_GOAL_WORK");
        }
        return Ok(());
    }
    ensure!(
        actor.is_none(),
        "OPERATOR_REQUIRED: only the canonical broker reserves or transitions Workers"
    );
    if let TeamAction::QueueExtraction {
        job_id,
        finding_event_id,
        positive_packet,
        ..
    } = action
    {
        let budget = crate::passive_base::KnowledgeBudget::default();
        ensure!(
            !v.cancelled
                && !v.knowledge_jobs.contains_key(job_id)
                && v.knowledge_jobs.len() < budget.max_jobs_per_goal,
            "KNOWLEDGE_BUDGET_DENIED: jobs_per_goal"
        );
        let goals: std::collections::BTreeSet<_> = events
            .iter()
            .filter_map(|e| match &e.payload {
                Payload::Team {
                    action: TeamAction::Configure { goal_id, .. },
                } => Some(goal_id),
                _ => None,
            })
            .collect();
        let pending: usize = goals
            .iter()
            .filter_map(|g| project(events, g))
            .map(|v| {
                v.knowledge_jobs
                    .values()
                    .filter(|j| j.state == "PENDING")
                    .count()
            })
            .sum();
        ensure!(
            pending < budget.max_pending_jobs,
            "KNOWLEDGE_BUDGET_DENIED: pending_jobs"
        );
        let evidence = crate::execution::EvidenceStore::load(root)?;
        if let Some(packet) = positive_packet {
            ensure!(
                packet.finding == *finding_event_id
                    && crate::positive_base::packets(root, action.goal_id())?.contains(packet),
                "HIGH_SIGNAL_REQUIRED"
            );
        } else {
            ensure!(events.iter().any(|e|e.event_id==*finding_event_id && e.intent_ref.as_deref()==Some(action.goal_id()) && crate::passive_base::supported_source(&evidence,e) && matches!(&e.payload,Payload::Finding {summary,..} if summary.starts_with("REUSABLE:"))),"HIGH_SIGNAL_REQUIRED");
        }
    }
    if let TeamAction::FinishExtraction { job_id, .. } = action {
        ensure!(
            v.knowledge_jobs
                .get(job_id)
                .is_some_and(|j| j.state == "PENDING"),
            "KNOWLEDGE_JOB_NOT_PENDING"
        );
    }
    if let TeamAction::Reserve { reservation: r, .. } = action {
        ensure!(!v.cancelled, "AGENT_BUDGET_DENIED: cancel_all");
        ensure!(
            !r.run_id.is_empty()
                && !r.worker_id.is_empty()
                && r.run_id.len() <= 128
                && r.worker_id.len() <= 128
                && !v.runs.contains_key(&r.run_id),
            "RUN_ID_CONFLICT"
        );
        ensure!(
            v.runs.len() < v.budget.max_spawn_attempts,
            "AGENT_BUDGET_DENIED: max_spawn_attempts"
        );
        ensure!(
            v.runs.values().filter(|r| r.state.active()).count() < v.budget.max_concurrent_workers,
            "AGENT_BUDGET_DENIED: max_concurrent_workers"
        );
        let capacity = project_concurrency(events, &v);
        ensure!(
            capacity.active_workers < capacity.effective_concurrent_limit,
            "AGENT_BUDGET_DENIED: global max_concurrent_workers"
        );
        let same: Vec<_> = v
            .runs
            .values()
            .filter(|p| p.reservation.worker_id == r.worker_id)
            .collect();
        if same.is_empty() {
            let workers: std::collections::BTreeSet<_> =
                v.runs.values().map(|r| &r.reservation.worker_id).collect();
            ensure!(
                workers.len() < v.budget.max_workers_per_goal,
                "AGENT_BUDGET_DENIED: max_workers_per_goal"
            );
        } else {
            ensure!(
                same.len() <= v.budget.retry_limit
                    && same
                        .iter()
                        .all(|p| matches!(p.state, RunState::Failed | RunState::TimedOut))
                    && same.iter().all(|p| p.reservation.role == r.role
                        && p.reservation.work_id == r.work_id
                        && p.reservation.parent_run_id == r.parent_run_id
                        && p.reservation.grants == r.grants),
                "AGENT_BUDGET_DENIED: retry_limit"
            );
        }
        let allowed = role_grants(&r.role);
        ensure!(
            r.grants.iter().all(|g| allowed.contains(g)),
            "AUTHORITY_DENIED: role grants exceeded"
        );
        if let Some(parent) = &r.parent_run_id {
            let p = v
                .runs
                .get(parent)
                .ok_or_else(|| anyhow!("UNKNOWN_PARENT_RUN"))?;
            ensure!(
                p.depth < v.budget.max_spawn_depth,
                "AGENT_BUDGET_DENIED: max_spawn_depth"
            );
            ensure!(
                p.reservation.role == Role::Planner,
                "AUTHORITY_DENIED: only a temporary planner delegates"
            );
            // Policy, not parent's incidental capability, defines child grants.
            if r.role == Role::Knowledge {
                ensure!(
                    r.work_id.as_ref().is_some_and(|id| v
                        .knowledge_jobs
                        .get(id)
                        .is_some_and(|j| j.state == "PENDING")),
                    "KNOWLEDGE_JOB_REQUIRED"
                );
            } else {
                ensure!(
                    v.decisions.iter().any(|d| d.split
                        && validate_routing_mode(d).is_ok()
                        && d.role == r.role
                        && r.work_id.as_ref().is_some_and(|id| d.work_ids.contains(id))),
                    "ROUTING_DECISION_REQUIRED"
                );
            }
        } else {
            ensure!(
                v.runs.is_empty() && r.role == Role::Planner,
                "INITIAL_PLANNER_ONLY"
            );
        }
    }
    if let TeamAction::Transition { run_id, state, .. } = action {
        let r = v.runs.get(run_id).ok_or_else(|| anyhow!("UNKNOWN_RUN"))?;
        ensure!(
            r.state.active()
                && *state != RunState::Reserved
                && !(*state == RunState::Running && r.state == RunState::Running),
            "INVALID_RUN_TRANSITION"
        );
        ensure!(
            !v.cancelled || *state != RunState::Running,
            "AGENT_BUDGET_DENIED: cancel_all"
        );
    }
    Ok(())
}
pub fn broker_action(
    root: &Path,
    caller: &CallerContext,
    action: TeamAction,
    key: &str,
) -> Result<development::AppendDevelopmentEventResult> {
    caller.require_operator()?;
    ensure!(
        !matches!(action, TeamAction::Decide { .. }),
        "WORKER_BINDING_REQUIRED"
    );
    let draft =
        serde_json::from_value(json!({"payload":Payload::Team {action},"deduplication_key":key}))?;
    development::append_operator_team_event(root, draft)
}
pub(crate) fn decide(
    root: &Path,
    caller: &CallerContext,
    params: Value,
    key: &str,
) -> Result<development::AppendDevelopmentEventResult> {
    let decision: RoutingDecision = serde_json::from_value(params)?;
    let draft = serde_json::from_value(
        json!({"payload":Payload::Team {action:TeamAction::Decide {decision}},"actor_worker_id":caller.worker_id(),"deduplication_key":key}),
    )?;
    development::append_authenticated_event(root, draft, caller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{tempdir, TempDir};
    fn setup(budget: AgentBudget) -> (TempDir, String) {
        let tmp = tempdir().unwrap();
        crate::BasicRepository::init(tmp.path()).unwrap();
        crate::ensure_identity(tmp.path()).unwrap();
        let revision = development::global_development_revision(tmp.path()).unwrap();
        let created=crate::general_work::operator_action(tmp.path(),&CallerContext::trusted_operator(),"goal.create",json!({"expected_revision":revision,"goal":{"title":"Fixture only: bounded work","domain":"DEVELOPMENT"}}),"goal-fixture").unwrap();
        let Payload::GeneralWork { action } = created.event.payload else {
            panic!()
        };
        let crate::general_work::GeneralChange::GoalCreated { goal } = action.change else {
            panic!()
        };
        broker_action(
            tmp.path(),
            &CallerContext::trusted_operator(),
            TeamAction::Configure {
                goal_id: goal.goal_id.clone(),
                budget,
                host_path: std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                host_sha256: "0".repeat(64),
            },
            "config",
        )
        .unwrap();
        (tmp, goal.goal_id)
    }

    // Synthetic projection inputs, not authenticated ledger or AI-host evidence.
    fn capacity_replay_fixture(count: usize, goals: usize) -> (Vec<DevelopmentEvent>, TeamView) {
        let (dir, goal) = setup(AgentBudget::default());
        let (ledger, _, _) = development::load_ledger_readonly(dir.path()).unwrap();
        let candidate = project(&ledger.events, &goal).unwrap();
        let template = ledger.events.last().unwrap();
        let unrelated = ledger
            .events
            .iter()
            .find(|e| !matches!(e.payload, Payload::Team { .. }))
            .unwrap()
            .payload
            .clone();
        let mut events = Vec::new();
        for i in 0..count {
            let goal_id = format!("goal-{}", (i * 7 + i / 13) % goals);
            let run_id = format!("run-{}", i / 11 % 8);
            let action = match i % 11 {
                0 => TeamAction::Configure {
                    goal_id,
                    budget: AgentBudget {
                        max_concurrent_workers: 1 + i % 8,
                        ..AgentBudget::default()
                    },
                    host_path: "fixture-host".into(),
                    host_sha256: "0".repeat(64),
                },
                1 | 7 => TeamAction::Reserve {
                    goal_id,
                    reservation: Reservation {
                        run_id,
                        worker_id: "fixture-worker".into(),
                        parent_run_id: None,
                        work_id: None,
                        role: Role::Planner,
                        grants: vec![],
                    },
                },
                2 | 8 | 9 => TeamAction::Transition {
                    goal_id,
                    run_id: if i % 11 == 9 {
                        "unknown-run".into()
                    } else {
                        run_id
                    },
                    state: [
                        RunState::Reserved,
                        RunState::Running,
                        RunState::Succeeded,
                        RunState::Failed,
                        RunState::Cancelled,
                        RunState::TimedOut,
                    ][i / 11 % 6]
                        .clone(),
                },
                3 => TeamAction::Decide {
                    decision: RoutingDecision {
                        goal_id,
                        split: false,
                        role: Role::Planner,
                        work_ids: vec![],
                        reason: "bounded fixture".into(),
                        independence_required: false,
                        capability_status: crate::cooperation::EpistemicStatus::Declared,
                    },
                },
                4 => TeamAction::QueueExtraction {
                    positive_packet: None,
                    goal_id,
                    job_id: "job".into(),
                    finding_event_id: "finding".into(),
                },
                5 => TeamAction::FinishExtraction {
                    goal_id,
                    job_id: "job".into(),
                    succeeded: i % 2 == 0,
                },
                6 => TeamAction::CancelAll { goal_id },
                _ => {
                    let mut event = template.clone();
                    event.sequence = i as u64 + 1;
                    event.payload = unrelated.clone();
                    events.push(event);
                    continue;
                }
            };
            let mut event = template.clone();
            event.sequence = i as u64 + 1;
            event.payload = Payload::Team { action };
            events.push(event);
        }
        (events, candidate)
    }

    #[test]
    fn narrow_capacity_matches_full_replay_at_every_prefix() {
        for goals in [1, 16] {
            let (events, mut candidate) = capacity_replay_fixture(600, goals);
            for limit in [1, 2, 8] {
                candidate.budget.max_concurrent_workers = limit;
                for end in 0..=events.len() {
                    assert_eq!(
                        project_concurrency(&events[..end], &candidate),
                        project_concurrency_full_replay(&events[..end], &candidate),
                        "goals={goals}, prefix={end}, candidate limit={limit}"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "explicit synthetic debug projection timing; not an end-to-end RSS benchmark"]
    fn capacity_projection_timing() {
        let (mut events, candidate) = capacity_replay_fixture(10_000, 16);
        for event in &mut events {
            if let Payload::Team {
                action: TeamAction::Decide { decision },
            } = &mut event.payload
            {
                decision.reason = "x".repeat(16 * 1024);
            }
        }
        assert_eq!(
            project_concurrency(&events, &candidate),
            project_concurrency_full_replay(&events, &candidate)
        );
        for (name, project_fn) in [
            (
                "full_replay",
                project_concurrency_full_replay
                    as fn(&[DevelopmentEvent], &TeamView) -> ProjectConcurrencySnapshot,
            ),
            ("narrow_replay", project_concurrency),
        ] {
            let mut samples = Vec::new();
            for _ in 0..30 {
                let start = std::time::Instant::now();
                std::hint::black_box(project_fn(std::hint::black_box(&events), &candidate));
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "CAPACITY_TIMING {name} events={} goals=16 samples=30 p50_ms={:.3} p95_ms={:.3}",
                events.len(),
                samples[14],
                samples[28]
            );
        }
    }

    #[test]
    fn project_snapshot_matches_reservation_gate_and_ignores_idle_low_limits() {
        let (dir, first) = setup(AgentBudget {
            max_concurrent_workers: 1,
            ..AgentBudget::default()
        });
        let root = dir.path();
        let configure = |key: &str, limit: usize| {
            let created = crate::general_work::operator_action(root, &CallerContext::trusted_operator(), "goal.create",
                json!({"expected_revision":development::global_development_revision(root).unwrap(),"goal":{"title":key,"domain":"DEVELOPMENT"}}), key).unwrap();
            let Payload::GeneralWork { action } = created.event.payload else {
                panic!()
            };
            let crate::general_work::GeneralChange::GoalCreated { goal } = action.change else {
                panic!()
            };
            apply(
                root,
                TeamAction::Configure {
                    goal_id: goal.goal_id.clone(),
                    budget: AgentBudget {
                        max_concurrent_workers: limit,
                        ..AgentBudget::default()
                    },
                    host_path: std::env::current_exe()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    host_sha256: "0".repeat(64),
                },
            )
            .unwrap();
            goal.goal_id
        };
        let second = configure("project-capacity-second", 2);
        let _idle = configure("project-capacity-idle", 1);
        reserve(
            root,
            &first,
            "first-run",
            "first-worker",
            None,
            None,
            Role::Planner,
        )
        .unwrap();
        let path = development::development_ledger_path(root).unwrap();
        let before = std::fs::read(&path).unwrap();
        let (view, full) = status_with_project_concurrency(root, &second).unwrap();
        assert!(view.runs.is_empty());
        assert_eq!(full.active_workers, 1);
        assert_eq!(full.effective_concurrent_limit, 1);
        assert_eq!(full.remaining_concurrent_slots, 0);
        assert_eq!(
            full.revision,
            development::global_development_revision(root).unwrap()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(reserve(
            root,
            &second,
            "denied-run",
            "second-worker",
            None,
            None,
            Role::Planner
        )
        .unwrap_err()
        .to_string()
        .contains("global max_concurrent_workers"));
        transition(root, &first, "first-run", RunState::Failed);
        let (_, free) = status_with_project_concurrency(root, &second).unwrap();
        assert_eq!(free.active_workers, 0);
        assert_eq!(
            free.effective_concurrent_limit, 2,
            "idle low-limit teams must not cap the candidate"
        );
        assert_eq!(free.remaining_concurrent_slots, 2);
        reserve(
            root,
            &second,
            "accepted-run",
            "second-worker",
            None,
            None,
            Role::Planner,
        )
        .unwrap();
        assert_eq!(
            status_with_project_concurrency(root, &second)
                .unwrap()
                .1
                .active_workers,
            1
        );
        std::fs::write(&path, b"invalid-json").unwrap();
        assert!(status_with_project_concurrency(root, &second).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid-json");
    }
    fn apply(root: &Path, action: TeamAction) -> Result<development::AppendDevelopmentEventResult> {
        broker_action(
            root,
            &CallerContext::trusted_operator(),
            action,
            &route_core::new_id(),
        )
    }
    fn reserve(
        root: &Path,
        g: &str,
        id: &str,
        worker: &str,
        parent: Option<&str>,
        work: Option<&str>,
        role: Role,
    ) -> Result<development::AppendDevelopmentEventResult> {
        let grants = role_grants(&role);
        apply(
            root,
            TeamAction::Reserve {
                goal_id: g.into(),
                reservation: Reservation {
                    run_id: id.into(),
                    worker_id: worker.into(),
                    parent_run_id: parent.map(String::from),
                    work_id: work.map(String::from),
                    role,
                    grants,
                },
            },
        )
    }
    fn transition(root: &Path, g: &str, id: &str, state: RunState) {
        apply(
            root,
            TeamAction::Transition {
                goal_id: g.into(),
                run_id: id.into(),
                state,
            },
        )
        .unwrap();
    }
    fn planner(root: &Path, g: &str) -> CallerContext {
        reserve(root, g, "planner", "p", None, None, Role::Planner).unwrap();
        let op = CallerContext::trusted_operator();
        crate::principal::register_worker(root, &op, Some("p".into()), Default::default(), "reg")
            .unwrap();
        let credential = crate::principal::generate_credential().unwrap();
        crate::principal::issue(
            root,
            &op,
            "p",
            &route_core::sha256_hex(credential.as_bytes()),
            role_grants(&Role::Planner),
            "binding",
        )
        .unwrap();
        transition(root, g, "planner", RunState::Running);
        CallerContext::authenticate(root, &credential).unwrap()
    }
    fn work(root: &Path, g: &str, caller: &CallerContext, key: &str, role: Role) -> String {
        let out=crate::principal::worker_action(root,caller,"work.create_child",json!({"intent_ref":g,"goal_id":g,"title":"Fixture Work","kind":"GENERAL","scope_paths":["src/lib.rs"],"verification_requirements":["cargo test"],"overlap_mode":"EXCLUSIVE"}),key).unwrap();
        let Payload::Work {
            action: crate::work::WorkAction::ChildCreated { work },
        } = out.event.payload
        else {
            panic!()
        };
        crate::principal::worker_action(root,caller,"team.decide",json!({"goal_id":g,"split":true,"role":role,"work_ids":[work.work_id],"reason":"Implementation and independent checks warrant separate bounded Workers","independence_required":role==Role::IndependentReviewer,"capability_status":"DECLARED"}),&format!("route-{key}")).unwrap();
        work.work_id
    }
    #[test]
    fn routing_mode_matrix_and_legacy_decision_cannot_authorize_child() {
        let (dir, goal) = setup(AgentBudget::default());
        let root = dir.path();
        let caller = planner(root, &goal);
        let work_id = work(root, &goal, &caller, "legacy-mode-work", Role::Implementer);
        let (mut ledger, _, _) = development::load_ledger_readonly(root).unwrap();
        let decision = status(root, &goal).unwrap().decisions[0].clone();
        for split in [false, true] {
            for role in [
                Role::Planner,
                Role::Implementer,
                Role::IndependentReviewer,
                Role::Knowledge,
            ] {
                let mut d = decision.clone();
                d.split = split;
                d.role = role.clone();
                assert_eq!(
                    validate_routing_mode(&d).is_ok(),
                    matches!(
                        (split, role),
                        (false, Role::Planner)
                            | (true, Role::Implementer | Role::IndependentReviewer)
                    )
                );
            }
        }
        let reservation = TeamAction::Reserve {
            goal_id: goal.clone(),
            reservation: Reservation {
                run_id: "delegated-mode-run".into(),
                worker_id: "delegated-mode-worker".into(),
                parent_run_id: Some("planner".into()),
                work_id: Some(work_id),
                role: Role::Implementer,
                grants: role_grants(&Role::Implementer),
            },
        };
        assert!(validate_transition(root, &ledger.events, None, &reservation).is_ok());
        // Test-only old-record fixture: no history or hash is rewritten on disk.
        for event in &mut ledger.events {
            if let Payload::Team {
                action: TeamAction::Decide { decision },
            } = &mut event.payload
            {
                decision.split = false;
            }
        }
        let legacy = project(&ledger.events, &goal).unwrap();
        assert_eq!(legacy.decisions.len(), 1, "legacy history stays readable");
        assert!(!legacy.decisions[0].split);
        assert!(
            validate_transition(root, &ledger.events, None, &reservation)
                .unwrap_err()
                .to_string()
                .contains("ROUTING_DECISION_REQUIRED")
        );
    }

    #[test]
    fn routing_mode_rejects_contradictory_delegation_before_append() {
        let (dir, goal) = setup(AgentBudget::default());
        let root = dir.path();
        let caller = planner(root, &goal);
        let work_id = work(root, &goal, &caller, "routing-mode-work", Role::Implementer);
        let path = development::development_ledger_path(root).unwrap();
        for (split, role) in [
            (false, Role::Implementer),
            (false, Role::IndependentReviewer),
            (false, Role::Knowledge),
            (true, Role::Planner),
            (true, Role::Knowledge),
        ] {
            let before = std::fs::read(&path).unwrap();
            let result = crate::principal::worker_action(
                root,
                &caller,
                "team.decide",
                json!({"goal_id":goal,"split":split,"role":role,"work_ids":[work_id],
                    "reason":"Routing mode regression fixture","independence_required":false,
                    "capability_status":"DECLARED"}),
                &route_core::new_id(),
            );
            assert!(
                result.is_err(),
                "contradictory mode was accepted: split={split}, role={role:?}"
            );
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("INVALID_ROUTING_MODE"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn budget_retries_depth_authority_and_cancellation() {
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        let p = planner(root, &g);
        let w = work(root, &g, &p, "w", Role::Implementer);
        reserve(
            root,
            &g,
            "child",
            "c",
            Some("planner"),
            Some(&w),
            Role::Implementer,
        )
        .unwrap();
        assert!(reserve(
            root,
            &g,
            "over-concurrent",
            "x",
            Some("planner"),
            Some(&w),
            Role::Implementer
        )
        .unwrap_err()
        .to_string()
        .contains("max_concurrent"));
        transition(root, &g, "planner", RunState::Succeeded);
        let denied = reserve(
            root,
            &g,
            "grandchild",
            "x",
            Some("child"),
            Some(&w),
            Role::Implementer,
        )
        .unwrap_err()
        .to_string();
        assert!(denied.contains("max_spawn_depth"));
        let mut r = status(root, &g).unwrap().runs["child"].reservation.clone();
        r.run_id = "escalation".into();
        r.worker_id = "escalator".into();
        r.grants.push("worker.binding.issue".into());
        assert!(apply(
            root,
            TeamAction::Reserve {
                goal_id: g.clone(),
                reservation: r
            }
        )
        .unwrap_err()
        .to_string()
        .contains("AUTHORITY_DENIED"));
        transition(root, &g, "child", RunState::Failed);
        reserve(
            root,
            &g,
            "retry",
            "c",
            Some("planner"),
            Some(&w),
            Role::Implementer,
        )
        .unwrap();
        transition(root, &g, "retry", RunState::Failed);
        assert!(reserve(
            root,
            &g,
            "retry-2",
            "c",
            Some("planner"),
            Some(&w),
            Role::Implementer
        )
        .unwrap_err()
        .to_string()
        .contains("retry_limit"));
        reserve(
            root,
            &g,
            "third",
            "third-worker",
            Some("planner"),
            Some(&w),
            Role::Implementer,
        )
        .unwrap();
        apply(root, TeamAction::CancelAll { goal_id: g.clone() }).unwrap();
        assert_eq!(
            status(root, &g).unwrap().runs["third"].state,
            RunState::Cancelled
        );
        assert!(reserve(
            root,
            &g,
            "after-cancel",
            "a",
            Some("planner"),
            Some(&w),
            Role::Implementer
        )
        .is_err());
        assert!(broker_action(
            root,
            &p,
            TeamAction::CancelAll { goal_id: g.clone() },
            "worker-admin"
        )
        .is_err());
        assert!(development::append_development_event(
            root,
            serde_json::from_value(
                json!({"payload":Payload::Team {action:TeamAction::CancelAll {goal_id:g.clone()}}})
            )
            .unwrap()
        )
        .is_err());
    }
    #[test]
    fn distinct_workers_limit_even_when_slots_are_free() {
        let (tmp, g) = setup(AgentBudget {
            max_workers_per_goal: 2,
            ..Default::default()
        });
        let root = tmp.path();
        let p = planner(root, &g);
        let w = work(root, &g, &p, "w", Role::Implementer);
        transition(root, &g, "planner", RunState::Succeeded);
        reserve(
            root,
            &g,
            "second",
            "s",
            Some("planner"),
            Some(&w),
            Role::Implementer,
        )
        .unwrap();
        transition(root, &g, "second", RunState::Succeeded);
        assert!(reserve(
            root,
            &g,
            "third",
            "t",
            Some("planner"),
            Some(&w),
            Role::Implementer
        )
        .unwrap_err()
        .to_string()
        .contains("max_workers_per_goal"));
    }
    #[test]
    fn final_slot_process_helper() {
        let Ok(root) = std::env::var("ROUTE_TEST_TEAM_ROOT") else {
            return;
        };
        let g = std::env::var("ROUTE_TEST_TEAM_GOAL").unwrap();
        let id = std::env::var("ROUTE_TEST_TEAM_ID").unwrap();
        let w = std::env::var("ROUTE_TEST_TEAM_WORK").unwrap();
        let result = reserve(
            Path::new(&root),
            &g,
            &id,
            &id,
            Some("planner"),
            Some(&w),
            Role::Implementer,
        );
        std::process::exit(if result.is_ok() { 0 } else { 42 });
    }
    #[test]
    fn concurrent_processes_only_one_final_slot() {
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        let p = planner(root, &g);
        let w = work(root, &g, &p, "w", Role::Implementer);
        let mut children: Vec<_> = (0..2)
            .map(|i| {
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "team::tests::final_slot_process_helper",
                        "--nocapture",
                    ])
                    .env("ROUTE_TEST_TEAM_ROOT", root)
                    .env("ROUTE_TEST_TEAM_GOAL", &g)
                    .env("ROUTE_TEST_TEAM_WORK", &w)
                    .env("ROUTE_TEST_TEAM_ID", format!("race-{i}"))
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let successes = children
            .iter_mut()
            .map(|c| c.wait().unwrap().success())
            .filter(|ok| *ok)
            .count();
        assert_eq!(successes, 1);
        assert_eq!(status(root, &g).unwrap().runs.len(), 2);
    }
    #[test]
    fn blind_context_and_ordinary_success_zero_growth() {
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        let p = planner(root, &g);
        let w = work(root, &g, &p, "w", Role::IndependentReviewer);
        for i in 0..100 {
            development::append_development_event(root,serde_json::from_value(json!({"payload":Payload::Finding {summary:"Ordinary successful task".into(),source_refs:vec![]},"intent_ref":g,"deduplication_key":format!("ordinary-{i}")})).unwrap()).unwrap();
        }
        let path = development::development_ledger_path(root).unwrap();
        let before = std::fs::read(&path).unwrap();
        for _ in 0..10 {
            let context =
                crate::context_pack::build(root, &g, Some(&w), &Role::IndependentReviewer).unwrap();
            assert!(context["findings"].as_array().unwrap().is_empty());
            assert_eq!(context["previous_rationale_withheld"], true);
            assert!(context["base_units"].as_array().unwrap().is_empty());
            assert!(serde_json::to_vec(&context).unwrap().len() <= crate::context_pack::MAX_BYTES);
            assert!(crate::passive_base::detect(root, &g).unwrap().is_empty());
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    #[test]
    fn colliding_display_names_do_not_merge_principals() {
        let (tmp, _) = setup(AgentBudget::default());
        let root = tmp.path();
        let mut ids = vec![];
        for id in ["distinct-a", "distinct-b"] {
            let op = CallerContext::trusted_operator();
            crate::principal::register_worker(
                root,
                &op,
                Some(id.into()),
                crate::WorkerMetadata {
                    display_name: Some("Scout".into()),
                    ..Default::default()
                },
                &format!("reg-{id}"),
            )
            .unwrap();
            let secret = crate::principal::generate_credential().unwrap();
            crate::principal::issue(
                root,
                &op,
                id,
                &route_core::sha256_hex(secret.as_bytes()),
                vec!["worker.message.send".into()],
                &format!("issue-{id}"),
            )
            .unwrap();
            ids.push(
                CallerContext::authenticate(root, &secret)
                    .unwrap()
                    .worker_id()
                    .unwrap()
                    .to_string(),
            );
        }
        assert_ne!(ids[0], ids[1]);
    }
    #[test]
    fn knowledge_queue_bounded_and_crash_does_not_change_goal() {
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        let before = crate::general_work::status(root, &g).unwrap().goal_state;
        let mut evidence = crate::execution::EvidenceStore::load(root).unwrap();
        let proof = evidence.record(
            &g,
            crate::EvidenceKind::CheckFail,
            crate::EvidenceSource::System,
            route_core::sha256_hex(b"synthetic validator fixture only"),
            Default::default(),
        );
        evidence.save(root).unwrap();
        let event=development::append_development_event(root,serde_json::from_value(json!({"payload":Payload::Finding {summary:"REUSABLE: Synthetic queue validator fixture only".into(),source_refs:vec![]},"intent_ref":g,"evidence_refs":[proof]})).unwrap()).unwrap();
        apply(
            root,
            TeamAction::QueueExtraction {
                positive_packet: None,
                goal_id: g.clone(),
                job_id: "job-1".into(),
                finding_event_id: event.event.event_id.clone(),
            },
        )
        .unwrap();
        assert!(apply(
            root,
            TeamAction::QueueExtraction {
                positive_packet: None,
                goal_id: g.clone(),
                job_id: "job-2".into(),
                finding_event_id: event.event.event_id
            }
        )
        .unwrap_err()
        .to_string()
        .contains("jobs_per_goal"));
        apply(
            root,
            TeamAction::FinishExtraction {
                goal_id: g.clone(),
                job_id: "job-1".into(),
                succeeded: false,
            },
        )
        .unwrap();
        assert_eq!(
            status(root, &g).unwrap().knowledge_jobs["job-1"].state,
            "FAILED"
        );
        assert_eq!(
            crate::general_work::status(root, &g).unwrap().goal_state,
            before
        );
        assert!(apply(
            root,
            TeamAction::FinishExtraction {
                goal_id: g.clone(),
                job_id: "job-1".into(),
                succeeded: true
            }
        )
        .is_err());
    }
    #[test]
    fn trivial_goal_can_stay_one_worker() {
        let (tmp, g) = setup(AgentBudget {
            max_concurrent_workers: 1,
            max_workers_per_goal: 1,
            ..Default::default()
        });
        let root = tmp.path();
        let p = planner(root, &g);
        let created=crate::principal::worker_action(root,&p,"work.create_child",json!({"intent_ref":g,"goal_id":g,"title":"One local edit","kind":"GENERAL","scope_paths":["src/lib.rs"],"verification_requirements":["cargo test"],"overlap_mode":"EXCLUSIVE"}),"single").unwrap();
        let Payload::Work {
            action: crate::work::WorkAction::ChildCreated { work },
        } = created.event.payload
        else {
            panic!()
        };
        crate::principal::worker_action(root,&p,"team.decide",json!({"goal_id":g,"split":false,"role":"PLANNER","work_ids":[work.work_id],"reason":"DO_NOT_SPLIT: one local edit; coordination exceeds expected benefit","independence_required":false,"capability_status":"INFERRED"}),"single-route").unwrap();
        let claim = crate::principal::worker_action(
            root,
            &p,
            "work.claim",
            json!({"work_id":work.work_id}),
            "single-claim",
        )
        .unwrap();
        let Payload::Work {
            action: crate::work::WorkAction::ClaimOpened { claim },
        } = claim.event.payload
        else {
            panic!()
        };
        crate::principal::worker_action(root,&p,"work.finish",json!({"claim_id":claim.claim_id,"reason":"Fixture work finished; still not System Evidence"}),"single-finish").unwrap();
        transition(root, &g, "planner", RunState::Succeeded);
        assert_eq!(status(root, &g).unwrap().runs.len(), 1);
        assert!(!status(root, &g).unwrap().decisions[0].split);
    }
    #[test]
    fn team_worker_goal_scope_and_stopped_replay() {
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        let p = planner(root, &g);
        let params = json!({"intent_ref":g,"payload":Payload::Finding {summary:"Synthetic scope fixture".into(),source_refs:vec![]}});
        let first = crate::principal::worker_action(
            root,
            &p,
            "development.event.record",
            params.clone(),
            "scope-first",
        );
        // Use the existing authenticated publication surface, never raw actor labels.
        assert!(first.is_ok(), "{first:?}");
        let mut foreign_params = params.clone();
        foreign_params["intent_ref"] = json!("foreign-goal");
        let foreign = crate::principal::worker_action(
            root,
            &p,
            "development.event.record",
            foreign_params,
            "scope-foreign",
        );
        assert!(foreign.is_err());
        transition(root, &g, "planner", RunState::Succeeded);
        assert!(crate::principal::worker_action(
            root,
            &p,
            "development.event.record",
            params.clone(),
            "scope-first"
        )
        .is_ok());
        assert!(crate::principal::worker_action(
            root,
            &p,
            "development.event.record",
            params,
            "scope-new-after-stop"
        )
        .is_err());
    }
    #[test]
    fn system_proof_rejects_knowledge_ids_stale_and_newer_failures() {
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        planner(root, &g);
        transition(root, &g, "planner", RunState::Succeeded);
        let op = CallerContext::trusted_operator();
        let workflow = json!({"workflow_id":"fixture-workflow","intent_ref":g,"title":"Synthetic command proof fixture","mode":"CONTROLLED","steps":[{"step_id":"check","title":"Actual native command check","requirement":"REQUIRED","dependencies":[],"proof":{"check_id":"fixture-check"}}]});
        crate::execution_contract::operator_action(root,&op,"workflow.create",json!({"expected_revision":development::global_development_revision(root).unwrap(),"spec":workflow}),"workflow").unwrap();
        let verify = |id: &str, key: &str| {
            crate::execution_contract::operator_action(
                root,
                &op,
                "workflow.step.verify_system",
                json!({"expected_revision":development::global_development_revision(root).unwrap(),"workflow_id":"fixture-workflow","version":1,"step_id":"check","evidence_ref":id}),
                key,
            )
        };
        assert!(verify("Base says PASS, not Evidence", "fake-base").is_err());
        let pass: Vec<String> = if cfg!(windows) {
            vec!["cmd", "/C", "exit", "0"]
        } else {
            vec!["sh", "-c", "exit 0"]
        }
        .into_iter()
        .map(String::from)
        .collect();
        crate::exec_command(root, &g, &pass, Some("fixture-check")).unwrap();
        let proof = crate::EvidenceStore::load(root)
            .unwrap()
            .evidence
            .last()
            .unwrap()
            .id
            .clone();
        verify(&proof, "first-pass").unwrap();
        let fail: Vec<String> = if cfg!(windows) {
            vec!["cmd", "/C", "exit", "1"]
        } else {
            vec!["sh", "-c", "exit 1"]
        }
        .into_iter()
        .map(String::from)
        .collect();
        crate::exec_command(root, &g, &fail, Some("fixture-check")).unwrap();
        assert!(verify(&proof, "old-pass-after-fail").is_err());
        assert_ne!(
            crate::execution_contract::status(root, "fixture-workflow")
                .unwrap()
                .completion
                .completion,
            crate::execution_contract::CompletionDecision::Pass
        );
        // A genuine rerun gets new Evidence even if state/command are unchanged.
        crate::exec_command(root, &g, &pass, Some("fixture-check")).unwrap();
        let fresh = crate::EvidenceStore::load(root)
            .unwrap()
            .evidence
            .last()
            .unwrap()
            .id
            .clone();
        assert_ne!(fresh, proof);
        verify(&fresh, "fresh-pass").unwrap();
        std::fs::write(root.join("changed.txt"), "new current bytes").unwrap();
        assert!(verify(&fresh, "stale-proof").is_err());
    }
    #[test]
    fn passive_lesson_scope_dedup_validation_and_stale_retrieval() {
        use crate::cooperation::{self, CooperationKnowledgeRecord, EpistemicStatus};
        use crate::passive_base::{Lesson, LessonState};
        let (tmp, g) = setup(AgentBudget::default());
        let root = tmp.path();
        let p = planner(root, &g);
        let w = work(root, &g, &p, "lesson-work", Role::Implementer);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/lib.rs"), "synthetic validator version one").unwrap();
        cooperation::register_cooperation_resource(
            root,
            "fixture-resource".into(),
            "src/lib.rs".into(),
            cooperation::CooperationKind("FILE".into()),
            "synthetic validator only".into(),
            None,
            vec![],
            vec![],
            vec![],
            None,
            Some("resource".into()),
        )
        .unwrap();
        let fp = cooperation::cooperation_resource(root, "fixture-resource")
            .unwrap()
            .unwrap()
            .fingerprint
            .unwrap()
            .value;
        let identity = crate::load_identity(root).unwrap().unwrap();
        let mut store = crate::EvidenceStore::load(root).unwrap();
        let evidence = store.record(
            &g,
            crate::EvidenceKind::CheckPass,
            crate::EvidenceSource::System,
            route_core::sha256_hex(b"synthetic validator fixture, not real knowledge"),
            std::collections::HashMap::from([
                ("project_id".into(), identity.project_id),
                ("cooperation_id".into(), "fixture-resource".into()),
                ("resource_fingerprint".into(), fp.clone()),
            ]),
        );
        store.save(root).unwrap();
        let finding=development::append_development_event(root,serde_json::from_value(json!({"payload":Payload::Finding {summary:"REUSABLE: Windows restricted sandbox path canonicalization failed, then explicit local boundary retry recovered. Synthetic validator only.".into(),source_refs:vec![]},"intent_ref":g,"task_ref":w,"evidence_refs":[evidence]})).unwrap()).unwrap().event.event_id;
        let record=CooperationKnowledgeRecord {lesson:Some(Lesson {positive:None,state:LessonState::Candidate,goal_id:g.clone(),work_id:w,creator_worker_id:"p".into(),finding_event_id:finding.clone(),source_event_ids:vec![finding],review_source:None,component:"path canonicalization".into(),environment:vec!["Windows".into(),"restricted sandbox".into()],condition:"When Windows restricted sandbox path canonicalization reports access denied".into(),action:"Request an explicitly approved local path boundary before retrying canonicalization".into(),falsification_check:"Repeat the same restricted path test and compare exit code with approved local retry".into()}),knowledge_id:"candidate".into(),cooperation_id:"fixture-resource".into(),statement:"Narrow synthetic validator lesson; not actual project knowledge".into(),capability_refs:vec![],epistemic_status:EpistemicStatus::Inferred,provenance:"synthetic validator fixture only".into(),source_refs:vec![],evidence_refs:vec![evidence],observed_at:0,resource_fingerprint:Some(fp),supersedes:None};
        let mut broad = record.clone();
        broad.lesson.as_mut().unwrap().environment = vec!["all Rust platforms".into()];
        assert!(
            cooperation::record_cooperation_knowledge(root, broad, None, Some("broad".into()))
                .is_err()
        );
        let mut missing = record.clone();
        missing.evidence_refs = vec!["absent".into()];
        assert!(cooperation::record_cooperation_knowledge(
            root,
            missing,
            None,
            Some("missing".into())
        )
        .is_err());
        cooperation::record_cooperation_knowledge(
            root,
            record.clone(),
            None,
            Some("candidate".into()),
        )
        .unwrap();
        let environment = vec!["Windows".into(), "restricted sandbox".into()];
        assert!(
            crate::passive_base::retrieve(root, "path canonicalization", &environment, false)
                .unwrap()
                .is_empty()
        );
        let mut duplicate = record.clone();
        duplicate.knowledge_id = "duplicate".into();
        assert!(cooperation::record_cooperation_knowledge(
            root,
            duplicate,
            None,
            Some("duplicate".into())
        )
        .is_err());
        crate::principal::register_worker(
            root,
            &CallerContext::trusted_operator(),
            Some("review-fixture".into()),
            Default::default(),
            "review-worker",
        )
        .unwrap();
        let review=development::append_development_event(root,serde_json::from_value(json!({"payload":Payload::Finding {summary:"Independent synthetic validator review, not an actual AI finding".into(),source_refs:vec![]},"actor_worker_id":"review-fixture","intent_ref":g})).unwrap()).unwrap().event.event_id;
        let mut validated = record.clone();
        validated.knowledge_id = "validated".into();
        validated.supersedes = Some("candidate".into());
        validated.epistemic_status = EpistemicStatus::Observed;
        validated.lesson.as_mut().unwrap().state = LessonState::ValidatedLocal;
        validated.lesson.as_mut().unwrap().review_source = Some(review);
        cooperation::record_cooperation_knowledge(root, validated, None, Some("validated".into()))
            .unwrap();
        assert_eq!(
            crate::passive_base::retrieve(root, "path canonicalization", &environment, false)
                .unwrap()
                .len(),
            1
        );
        assert!(
            crate::passive_base::retrieve(root, "path canonicalization", &environment, true)
                .unwrap()
                .is_empty()
        );
        std::fs::write(root.join("src/lib.rs"), "synthetic validator version two").unwrap();
        cooperation::refresh_cooperation_resource(
            root,
            "fixture-resource",
            None,
            Some("refresh".into()),
        )
        .unwrap();
        assert!(
            crate::passive_base::retrieve(root, "path canonicalization", &environment, false)
                .unwrap()
                .is_empty()
        );
        assert!(crate::passive_base::detect(root, &g).unwrap().len() <= 1);
    }
}
