//! Native CLI budget inspection, not an AI/provider spending test.
use route_basic::{
    principal::CallerContext,
    team::{self, AgentBudget, TeamAction},
};
use serde_json::{json, Value};
use std::process::Command;

#[test]
fn budget_flag_is_readonly_and_legacy_status_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    route_basic::BasicRepository::init(root).unwrap();
    route_basic::ensure_identity(root).unwrap();
    let operator = CallerContext::trusted_operator();
    let created = route_basic::general_work::operator_action(
        root, &operator, "goal.create",
        json!({"expected_revision":route_basic::global_development_revision(root).unwrap(),"goal":{"title":"Read-only budget fixture","domain":"DEVELOPMENT"}}),
        "budget-goal",
    ).unwrap();
    let route_basic::DevelopmentEventPayload::GeneralWork { action } = created.event.payload else {
        panic!()
    };
    let route_basic::general_work::GeneralChange::GoalCreated { goal } = action.change else {
        panic!()
    };
    team::broker_action(
        root,
        &operator,
        TeamAction::Configure {
            goal_id: goal.goal_id.clone(),
            budget: AgentBudget::default(),
            host_path: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            host_sha256: "0".repeat(64),
        },
        "budget-configure",
    )
    .unwrap();
    let path = route_basic::development::development_ledger_path(root).unwrap();
    let before = std::fs::read(&path).unwrap();
    let cli = |budget: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_route"));
        command
            .current_dir(root)
            .args(["team", "status", &goal.goal_id])
            .env_remove("ROUTE_WORKER_CREDENTIAL");
        if budget {
            command.arg("--budget");
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };
    assert_eq!(
        cli(false),
        serde_json::to_value(team::status(root, &goal.goal_id).unwrap()).unwrap()
    );
    let summary = cli(true);
    assert_eq!(summary["capacity_blocked_by_uncertain_run"], false);
    assert!(summary["process_inspection"]
        .as_str()
        .unwrap()
        .contains("route run inspect"));
    assert_eq!(summary["goal_id"], goal.goal_id);
    assert_eq!(
        summary["remaining"],
        json!({"spawn_attempts":4,"new_distinct_workers":3,"local_concurrent_slots":2})
    );
    assert_eq!(
        summary["usage"],
        json!({"spawn_attempts":0,"distinct_workers":0,"active_workers":0})
    );
    assert_eq!(summary["new_worker_local_limit_reasons"], json!([]));
    assert_eq!(summary["launch_authorized"], false);
    assert_eq!(summary["project_global_capacity"], "SNAPSHOT_EVALUATED");
    assert_eq!(summary["project_concurrency"]["active_workers"], 0);
    assert_eq!(
        summary["project_concurrency"]["effective_concurrent_limit"],
        2
    );
    assert_eq!(
        summary["project_concurrency"]["remaining_concurrent_slots"],
        2
    );
    assert_eq!(std::fs::read(path).unwrap(), before);

    let created = route_basic::general_work::operator_action(root, &operator, "goal.create",
        json!({"expected_revision":route_basic::global_development_revision(root).unwrap(),"goal":{"title":"Other Goal owns project slot","domain":"DEVELOPMENT"}}), "other-budget-goal").unwrap();
    let route_basic::DevelopmentEventPayload::GeneralWork { action } = created.event.payload else {
        panic!()
    };
    let route_basic::general_work::GeneralChange::GoalCreated { goal: other } = action.change
    else {
        panic!()
    };
    let apply =
        |action| team::broker_action(root, &operator, action, &route_core::new_id()).unwrap();
    apply(TeamAction::Configure {
        goal_id: other.goal_id.clone(),
        budget: AgentBudget {
            max_concurrent_workers: 1,
            ..AgentBudget::default()
        },
        host_path: std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        host_sha256: "0".repeat(64),
    });
    apply(TeamAction::Reserve {
        goal_id: other.goal_id.clone(),
        reservation: team::Reservation {
            run_id: "other-run".into(),
            worker_id: "other-worker".into(),
            parent_run_id: None,
            work_id: None,
            role: team::Role::Planner,
            grants: team::role_grants(&team::Role::Planner),
        },
    });
    let path = route_basic::development::development_ledger_path(root).unwrap();
    let before = std::fs::read(&path).unwrap();
    let blocked = cli(true);
    assert_eq!(blocked["remaining"]["local_concurrent_slots"], 2);
    assert_eq!(blocked["new_worker_local_limit_reasons"], json!([]));
    assert_eq!(
        blocked["project_concurrency"]["remaining_concurrent_slots"],
        0
    );
    assert_eq!(
        blocked["new_worker_project_limit_reasons"],
        json!(["global_max_concurrent_workers"])
    );
    assert_eq!(blocked["launch_authorized"], false);
    assert_eq!(blocked["token_budget"], "NOT_TRACKED");
    assert_eq!(blocked["monetary_budget"], "NOT_TRACKED");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    apply(TeamAction::Transition {
        goal_id: other.goal_id,
        run_id: "other-run".into(),
        state: team::RunState::Failed,
    });
    let before = std::fs::read(&path).unwrap();
    let free = cli(true);
    assert_eq!(free["project_concurrency"]["remaining_concurrent_slots"], 2);
    assert_eq!(free["new_worker_project_limit_reasons"], json!([]));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
