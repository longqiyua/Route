//! Task-local narrowing of existing Worker bindings, checked under append lock.
use crate::{
    development::{DevelopmentEvent, DevelopmentEventDraft, DevelopmentEventPayload as P},
    general_work::GeneralChange,
    team::{self, Role, RunState, TeamAction},
    work::WorkAction,
};
use anyhow::{ensure, Result};
pub(crate) fn validate(
    events: &[DevelopmentEvent],
    worker: &str,
    draft: &DevelopmentEventDraft,
) -> Result<()> {
    let goals: std::collections::BTreeSet<_> = events
        .iter()
        .filter_map(|e| match &e.payload {
            P::Team {
                action: TeamAction::Configure { goal_id, .. },
            } => Some(goal_id),
            _ => None,
        })
        .collect();
    let view = goals
        .iter()
        .filter_map(|g| team::project(events, g))
        .find(|v| v.runs.values().any(|r| r.reservation.worker_id == worker));
    let Some(v) = view else { return Ok(()) };
    let run = v
        .runs
        .values()
        .find(|r| r.reservation.worker_id == worker && r.state == RunState::Running);
    ensure!(!v.cancelled && run.is_some(), "TEAM_WORKER_NOT_RUNNING");
    let run = run.unwrap();
    let work_for = |id: &str| {
        events.iter().find_map(|e| match &e.payload {
            P::Work {
                action: WorkAction::ChildCreated { work },
            } if work.work_id == id => Some(work),
            _ => None,
        })
    };
    let mut work_id: Option<&str> = None;
    let target: Option<&str> = match &draft.payload {
        P::Team {
            action: TeamAction::Decide { decision },
        } => Some(&decision.goal_id),
        P::Work { action } => match action {
            WorkAction::ChildCreated { work } => Some(&work.intent_ref),
            WorkAction::ClaimOpened { claim } => {
                work_id = Some(&claim.work_id);
                work_for(&claim.work_id).map(|w| w.intent_ref.as_str())
            }
            WorkAction::ClaimChanged { claim_id, .. } => {
                work_id = events.iter().find_map(|e| match &e.payload {
                    P::Work {
                        action: WorkAction::ClaimOpened { claim },
                    } if claim.claim_id == *claim_id => Some(claim.work_id.as_str()),
                    _ => None,
                });
                work_id.and_then(work_for).map(|w| w.intent_ref.as_str())
            }
            WorkAction::Integrated { .. } => None,
        },
        P::GeneralWork { action } => match &action.change {
            GeneralChange::ArtifactRegistered { artifact } => {
                if run.reservation.role != Role::Planner {
                    let scope = run.reservation.work_id.as_deref().and_then(work_for);
                    ensure!(
                        scope.is_some_and(|w| w
                            .scope_paths
                            .iter()
                            .any(|p| artifact.path == *p
                                || artifact.path.starts_with(&format!("{p}/")))),
                        "TEAM_WORK_SCOPE_DENIED"
                    );
                }
                Some(&artifact.goal_id)
            }
            GeneralChange::ObservationRecorded { observation } => Some(&observation.goal_id),
            GeneralChange::DecisionRequested { decision } => Some(&decision.goal_id),
            GeneralChange::OutcomeRecorded { outcome } => Some(&outcome.goal_id),
            _ => None,
        },
        P::WorkerMessage { message } => message.intent_ref.as_deref(),
        P::Finding { .. } => draft.intent_ref.as_deref(),
        P::CooperationKnowledgeRecorded { record } => {
            record.lesson.as_ref().map(|l| l.goal_id.as_str())
        }
        P::WorkerPresenceUpdated { presence } => presence.current_intent_ref.as_deref(),
        _ => None,
    };
    ensure!(target == Some(v.goal_id.as_str()), "TEAM_GOAL_SCOPE_DENIED");
    if let Some(id) = work_id {
        ensure!(
            run.reservation.role == Role::Planner || run.reservation.work_id.as_deref() == Some(id),
            "TEAM_WORK_SCOPE_DENIED"
        );
    }
    Ok(())
}
