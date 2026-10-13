use super::*;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn fixture(root: &Path, store: &Path, mode: &str) -> Command {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "process_supervision::tests::real_process_fixture",
        "--ignored",
        "--nocapture",
        "--skip",
        mode,
    ])
    .env("ROUTE_PROCESS_FIXTURE_ROOT", root)
    .env("ROUTE_PROCESS_FIXTURE_STORE", store);
    cmd
}
fn binding() -> RunIdentity {
    RunIdentity {
        run_id: "fixture-run".into(),
        operation_id: "fixture-operation".into(),
        owner: "owner-one".into(),
        attempt: 1,
        team_goal: None,
    }
}
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf, Supervisor) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let store = dir.path().join("private-runtime");
    let s = Supervisor::at(&root, &store).unwrap();
    (dir, root, store, s)
}
fn wait_file(path: &Path) {
    let until = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        assert!(
            Instant::now() < until,
            "fixture readiness missing: {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn isolated(name: &str) -> bool {
    const MARKER: &str = "run-continuity-isolated-test";
    if std::env::args().any(|s| s == MARKER) {
        return false;
    }
    let _guard = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("process_supervision::tests::{name}"),
            "--nocapture",
            "--skip",
            MARKER,
        ])
        .status()
        .unwrap();
    assert!(status.success(), "isolated continuity test failed: {name}");
    true
}
#[test]
#[ignore = "REAL_PROCESS_FIXTURE helper; exercised by nonignored OS integration tests"]
fn real_process_fixture() {
    let root = PathBuf::from(std::env::var_os("ROUTE_PROCESS_FIXTURE_ROOT").unwrap());
    let store = PathBuf::from(std::env::var_os("ROUTE_PROCESS_FIXTURE_STORE").unwrap());
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[args.iter().position(|s| s == "--skip").unwrap() + 1];
    if let Some(point) = mode.strip_prefix("crash-") {
        let s = Supervisor::at(&root, &store).unwrap();
        if point == "cancel_requested" {
            let _child = s
                .spawn_owned(binding(), &fixture(&root, &store, "parent"), false)
                .unwrap();
            wait_file(&root.join("grandchild.ready"));
            windows::CRASH_POINT.with(|p| *p.borrow_mut() = point.into());
            s.cancel("fixture-run", "cancel-once").unwrap();
        } else {
            windows::CRASH_POINT.with(|p| *p.borrow_mut() = point.into());
            let _child = s
                .spawn_owned(binding(), &fixture(&root, &store, "parent"), false)
                .unwrap();
        }
        panic!("crash seam not reached");
    }
    if mode == "supervisor" {
        let s = Supervisor::at(&root, &store).unwrap();
        let child = s
            .spawn_owned(binding(), &fixture(&root, &store, "parent"), false)
            .unwrap();
        wait_file(&root.join("grandchild.ready"));
        std::fs::write(root.join("supervisor.ready"), child.id().to_string()).unwrap();
        std::process::exit(23); // real supervisor crash: no destructors, no child cleanup
    }
    if mode == "parent" || mode == "child" || mode == "parent-exits" {
        let next = if mode != "child" {
            "child"
        } else {
            "grandchild"
        };
        let _child = fixture(&root, &store, next).spawn().unwrap();
        if mode == "parent-exits" {
            wait_file(&root.join("grandchild.ready"));
            std::process::exit(7);
        }
    }
    std::fs::write(
        root.join(format!("{mode}.ready")),
        std::process::id().to_string(),
    )
    .unwrap();
    println!("REAL_PROCESS_FIXTURE {mode} {}", std::process::id());
    let until = Instant::now() + Duration::from_secs(60);
    while !root.join("finish").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::process::exit(7);
}

#[test]
fn windows_spawn_handshake_and_cancel_crashes_are_recoverable_without_relaunch() {
    if isolated("windows_spawn_handshake_and_cancel_crashes_are_recoverable_without_relaunch") {
        return;
    }
    for point in [
        "before_spawn",
        "after_create",
        "after_identity",
        "cancel_requested",
    ] {
        let (_dir, root, store, s) = setup();
        let mut owner = fixture(&root, &store, &format!("crash-{point}"))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        assert_eq!(owner.wait().unwrap().code(), Some(23));
        let inspect = s.inspect("fixture-run").unwrap();
        assert!(s
            .spawn_owned(binding(), &fixture(&root, &store, "parent"), false)
            .is_err());
        if point == "before_spawn" {
            assert_eq!(inspect.receipt.stage, "BEFORE_SPAWN");
            assert_eq!(inspect.active_processes, Some(0));
        } else {
            assert!(
                inspect.active_processes.unwrap() >= 1,
                "{point}: {:?}",
                inspect
            );
            if point == "after_create" {
                assert!(inspect.receipt.process.is_none());
            }
            if point == "after_identity" {
                assert_eq!(inspect.receipt.stage, "OWNED_PROCESS_IDENTIFIED");
            }
            if point == "cancel_requested" {
                assert_eq!(
                    inspect.receipt.cancellation.unwrap().state,
                    "CANCEL_REQUESTED"
                );
            }
            let ended = s.cancel("fixture-run", "cancel-once").unwrap();
            assert_eq!(ended.active_processes, Some(0));
            assert_eq!(
                ended.receipt.cancellation.unwrap().state,
                "TERMINATION_CONFIRMED"
            );
        }
    }
}
#[test]
fn windows_parent_exit_does_not_release_live_descendant_tree() {
    if isolated("windows_parent_exit_does_not_release_live_descendant_tree") {
        return;
    }
    let (_dir, root, store, s) = setup();
    let mut child = s
        .spawn_owned(binding(), &fixture(&root, &store, "parent-exits"), false)
        .unwrap();
    wait_file(&root.join("grandchild.ready"));
    std::thread::sleep(Duration::from_millis(150));
    assert!(
        child.try_wait().unwrap().is_none(),
        "root exit must not release tree capacity"
    );
    assert!(s.inspect("fixture-run").unwrap().active_processes.unwrap() >= 2);
    child.kill().unwrap();
    assert!(child.wait().is_ok());
    assert_eq!(s.inspect("fixture-run").unwrap().active_processes, Some(0));
}
#[test]
fn windows_restart_recognizes_same_tree_and_cancels_no_duplicate() {
    if isolated("windows_restart_recognizes_same_tree_and_cancels_no_duplicate") {
        return;
    }
    let (_dir, root, store, s) = setup();
    let mut owner = fixture(&root, &store, "supervisor")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    wait_file(&root.join("supervisor.ready"));
    assert_eq!(owner.wait().unwrap().code(), Some(23));
    let start = Instant::now();
    let before = std::fs::read(s.path("fixture-run")).unwrap();
    let restarted = Supervisor::at(&root, &store).unwrap();
    let inspected = restarted.inspect("fixture-run").unwrap();
    assert_eq!(inspected.knowledge, "RUNNING");
    assert!(inspected.active_processes.unwrap() >= 3); // Windows may also add a contained console host.
    assert!(inspected.capacity_blocked_by_uncertain_run);
    assert_eq!(
        before,
        std::fs::read(s.path("fixture-run")).unwrap(),
        "inspect must be read-only"
    );
    assert!(restarted
        .spawn_owned(binding(), &fixture(&root, &store, "parent"), false)
        .is_err());
    eprintln!(
        "restart_inspection_ms={:.3}; duplicate_process_launch_count=0",
        start.elapsed().as_secs_f64() * 1000.0
    );
    let start = Instant::now();
    let cancelled = restarted.cancel("fixture-run", "cancel-once").unwrap();
    assert_eq!(cancelled.active_processes, Some(0));
    assert_eq!(
        cancelled.receipt.cancellation.unwrap().state,
        "TERMINATION_CONFIRMED"
    );
    assert_eq!(cancelled.receipt.exit_code, None);
    assert_eq!(cancelled.knowledge, "LOST_RESULT");
    eprintln!(
        "cancel_and_tree_confirmation_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    assert_eq!(
        restarted
            .cancel("fixture-run", "cancel-once")
            .unwrap()
            .active_processes,
        Some(0)
    );
    assert!(restarted
        .cancel("fixture-run", "conflicting-cancel")
        .is_err());
}
#[test]
fn windows_natural_exit_after_supervisor_loss_is_lost_result_not_success() {
    if isolated("windows_natural_exit_after_supervisor_loss_is_lost_result_not_success") {
        return;
    }
    let (_dir, root, store, s) = setup();
    let child = s
        .spawn_owned(binding(), &fixture(&root, &store, "grandchild"), false)
        .unwrap();
    wait_file(&root.join("grandchild.ready"));
    drop(child); // all local process/job handles gone; OS process intentionally survives
    std::fs::write(root.join("finish"), b"finish").unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let inspect = s.inspect("fixture-run").unwrap();
        if inspect.knowledge == "LOST_RESULT" {
            assert_eq!(inspect.knowledge, "LOST_RESULT");
            assert_eq!(inspect.receipt.exit_code, None);
            assert!(inspect.capacity_blocked_by_uncertain_run);
            break;
        }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        s.recover("fixture-run", "observe-lost").unwrap().knowledge,
        "LOST_RESULT"
    );
    // The Job name can disappear after the last inherited lease closes. Never
    // turn that into a durable empty-tree observation or a capacity release.
    let mut receipt = s.load("fixture-run").unwrap();
    receipt.tree_termination_confirmed = false;
    receipt.cancellation = Some(super::Cancellation {
        operation_key: "lost-job-cancel".into(),
        state: "CANCEL_SIGNALLED".into(),
    });
    s.save(&receipt).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while s.inspect("fixture-run").unwrap().active_processes.is_some() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(10));
    }
    let lost = s.cancel("fixture-run", "lost-job-cancel").unwrap();
    assert_eq!(lost.active_processes, None);
    assert_eq!(
        lost.receipt.cancellation.unwrap().state,
        "CANCEL_UNCONFIRMED"
    );
    assert!(s
        .acknowledge_lost_result("fixture-run", "unsafe-release")
        .is_err());
}
#[test]
fn windows_pid_reuse_and_forged_metadata_fail_closed() {
    if isolated("windows_pid_reuse_and_forged_metadata_fail_closed") {
        return;
    }
    let (_dir, root, store, s) = setup();
    let mut child = s
        .spawn_owned(binding(), &fixture(&root, &store, "grandchild"), false)
        .unwrap();
    wait_file(&root.join("grandchild.ready"));
    let mut unrelated = fixture(&root, &store, "unrelated")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait_file(&root.join("unrelated.ready"));
    let original = s.load("fixture-run").unwrap();
    // Simulate a reused PID: old creation identity now points at a different real process.
    let mut forged = original.clone();
    forged.process.as_mut().unwrap().pid = unrelated.id();
    s.save(&forged).unwrap();
    assert_eq!(
        s.inspect("fixture-run").unwrap().knowledge,
        "RECOVERY_REQUIRED"
    );
    assert!(s.cancel("fixture-run", "attack").is_err());
    assert!(unrelated.try_wait().unwrap().is_none());
    s.save(&original).unwrap();
    // Shared-project records cannot replace local authoritative launch provenance.
    let shared = root.join(".route/execution/processes");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(
        shared.join("fixture-run.json"),
        serde_json::to_vec(&forged).unwrap(),
    )
    .unwrap();
    assert_eq!(s.inspect("fixture-run").unwrap().knowledge, "RUNNING");
    let mut changed = original.clone();
    changed.process.as_mut().unwrap().executable = "C:\\unrelated.exe".into();
    s.save(&changed).unwrap();
    assert_eq!(
        s.inspect("fixture-run").unwrap().knowledge,
        "RECOVERY_REQUIRED"
    );
    assert!(s.cancel("fixture-run", "attack").is_err());
    s.save(&original).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    unrelated.kill().unwrap();
    unrelated.wait().unwrap(); // exact Child launched by fixture, never discovered PID
}
#[test]
fn windows_foreign_project_malformed_receipt_and_stale_attempt_are_rejected() {
    if isolated("windows_foreign_project_malformed_receipt_and_stale_attempt_are_rejected") {
        return;
    }
    let (_dir, root, store, s) = setup();
    let mut child = s
        .spawn_owned(binding(), &fixture(&root, &store, "grandchild"), false)
        .unwrap();
    let original = s.load("fixture-run").unwrap();
    let mut newer = original.clone();
    newer.run.attempt += 1;
    newer.containment = job_name(&s.root, &newer.run, &newer.nonce);
    s.save(&newer).unwrap();
    assert!(child.kill().unwrap_err().to_string().contains("FENCED"));
    assert!(child.persist_result(0).is_err());
    s.save(&original).unwrap();
    let other = root.parent().unwrap().join("foreign");
    std::fs::create_dir(&other).unwrap();
    assert!(Supervisor::at(&other, &store)
        .unwrap()
        .inspect("fixture-run")
        .is_err());
    std::fs::write(s.path("fixture-run"), b"invalid json").unwrap();
    assert!(s.inspect("fixture-run").is_err());
    assert!(s.cancel("fixture-run", "attack").is_err());
    s.save(&original).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn windows_late_old_process_callback_cannot_overwrite_after_safe_release_and_new_run() {
    if isolated("windows_late_old_process_callback_cannot_overwrite_after_safe_release_and_new_run")
    {
        return;
    }
    let (_dir, root, store, s) = setup();
    crate::BasicRepository::init(&root).unwrap();
    std::fs::create_dir_all(root.join(".route/execution/check-runs")).unwrap();
    // Canonical reservation fixture; actual process, cancellation and explicit
    // acknowledgement use production APIs, not simulated OS completion.
    let install = |identity: &RunIdentity| {
        let command = fixture(&root, &store, "grandchild");
        let argv = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let run = crate::check_run::CheckRun {
            run_id: identity.run_id.clone(),
            intent_ref: "fixture-logical-obligation".into(),
            contract: crate::check_run::CheckContract::approved(
                &root,
                argv,
                vec!["fixture-check".into()],
                "approved".into(),
            )
            .unwrap(),
            state: crate::check_run::CheckRunState::PossiblyRunning,
            owner: identity.owner.clone(),
            attempt: identity.attempt,
            created_at: route_core::now_millis(),
            finished_at: None,
            world_before: "fixture-world".into(),
            result: None,
            metadata: Default::default(),
            evidence_ids: Default::default(),
            diagnostic: None,
        };
        let path = root.join(".route/execution/check-runs").join(format!(
            "{}.json",
            route_core::sha256_hex(identity.operation_id.as_bytes())
        ));
        crate::ownership_lock::atomic_replace(&path, &serde_json::to_vec(&run).unwrap()).unwrap();
    };
    let first = binding();
    install(&first);
    let mut old = s
        .spawn_owned(first.clone(), &fixture(&root, &store, "grandchild"), false)
        .unwrap();
    wait_file(&root.join("grandchild.ready"));
    old.kill().unwrap();
    let status = old.wait().unwrap();
    let released = s
        .acknowledge_lost_result(&first.run_id, "explicit-release-old")
        .unwrap();
    assert!(!released.capacity_blocked_by_uncertain_run);
    let newer = RunIdentity {
        run_id: "fixture-new-run".into(),
        operation_id: "new-approved-observation".into(),
        owner: "owner-two".into(),
        attempt: 2,
        team_goal: None,
    };
    install(&newer);
    let mut new = s
        .spawn_owned(newer.clone(), &fixture(&root, &store, "grandchild"), false)
        .unwrap();
    // The old process is definitely gone before the new approval. A delayed
    // completion callback holding its original OS handle is non-authoritative.
    assert!(old
        .persist_result(status.code().unwrap() as u32)
        .unwrap_err()
        .to_string()
        .contains("FENCED"));
    assert!(old.kill().is_err());
    let current = s.inspect(&newer.run_id).unwrap();
    assert!(current.capacity_blocked_by_uncertain_run);
    assert!(current.receipt.cancellation.is_none());
    assert!(new.try_wait().unwrap().is_none());
    let old_record = crate::check_run::inspect(&root, &first.operation_id).unwrap();
    assert_eq!(
        old_record.state,
        crate::check_run::CheckRunState::LostResult
    );
    assert!(old_record.result.is_none());
    assert!(old_record.evidence_ids.is_empty());
    new.kill().unwrap();
    new.wait().unwrap();
}

#[test]
fn windows_proven_before_spawn_restores_original_check_reservation_with_history() {
    if isolated("windows_proven_before_spawn_restores_original_check_reservation_with_history") {
        return;
    }
    let (_dir, root, _store, _) = setup();
    crate::BasicRepository::init(&root).unwrap();
    crate::ensure_identity(&root).unwrap();
    let created = crate::general_work::operator_action(&root,&crate::principal::CallerContext::trusted_operator(),"goal.create",
        serde_json::json!({"goal":{"title":"Safe unlaunched recovery fixture","domain":"DEVELOPMENT"},"expected_revision":crate::global_development_revision(&root).unwrap()}),"goal").unwrap();
    let crate::DevelopmentEventPayload::GeneralWork { action } = created.event.payload else {
        panic!()
    };
    let crate::general_work::GeneralChange::GoalCreated { goal } = action.change else {
        panic!()
    };
    let s = Supervisor::open(&root).unwrap();
    let base = s.authority.parent().unwrap();
    let mut owner = fixture(&root, base, "crash-before_spawn")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    assert_eq!(owner.wait().unwrap().code(), Some(23));
    let first = binding();
    let contract = crate::check_run::CheckContract::approved(
        &root,
        vec!["cmd.exe".into(), "/d".into(), "/c".into(), "exit 7".into()],
        vec!["fixture-check".into()],
        "approved-fixture".into(),
    )
    .unwrap();
    let run = crate::check_run::CheckRun {
        run_id: first.run_id.clone(),
        intent_ref: goal.goal_id.clone(),
        contract: contract.clone(),
        state: crate::check_run::CheckRunState::PossiblyRunning,
        owner: first.owner.clone(),
        attempt: first.attempt,
        created_at: route_core::now_millis(),
        finished_at: None,
        world_before: crate::compute_state_hash(&root).unwrap(),
        result: None,
        metadata: Default::default(),
        evidence_ids: Default::default(),
        diagnostic: None,
    };
    let path = root.join(".route/execution/check-runs").join(format!(
        "{}.json",
        route_core::sha256_hex(first.operation_id.as_bytes())
    ));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    crate::ownership_lock::atomic_replace(&path, &serde_json::to_vec(&run).unwrap()).unwrap();
    assert_eq!(
        s.recover(&first.run_id, "recover-never-launched")
            .unwrap()
            .knowledge,
        "BEFORE_SPAWN"
    );
    assert_eq!(
        crate::check_run::inspect(&root, &first.operation_id)
            .unwrap()
            .state,
        crate::check_run::CheckRunState::Reserved
    );
    let observed =
        crate::check_run::execute(&root, &goal.goal_id, &first.operation_id, contract.clone())
            .unwrap();
    assert_eq!(observed.run_id, first.run_id);
    assert_eq!(observed.attempt, 2);
    assert_eq!(observed.result.as_ref().unwrap().exit_code, 7);
    let launched = s.load(&first.run_id).unwrap();
    assert!(launched.process.is_some());
    assert_eq!(
        std::fs::read_dir(&s.authority)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("launch-history"))
            .count(),
        1
    );
    crate::check_run::execute(&root, &goal.goal_id, &first.operation_id, contract).unwrap();
    assert_eq!(
        s.load(&first.run_id).unwrap().process.unwrap().pid,
        launched.process.unwrap().pid
    );
}

#[test]
fn windows_team_capacity_requires_proof_and_explicit_lost_result_release() {
    if isolated("windows_team_capacity_requires_proof_and_explicit_lost_result_release") {
        return;
    }
    let (_dir, root, store, s) = setup();
    crate::BasicRepository::init(&root).unwrap();
    crate::ensure_identity(&root).unwrap();
    let op = crate::principal::CallerContext::trusted_operator();
    let created = crate::general_work::operator_action(&root,&op,"goal.create",serde_json::json!({"goal":{"title":"Team capacity fixture","domain":"DEVELOPMENT"},"expected_revision":crate::global_development_revision(&root).unwrap()}),"goal").unwrap();
    let crate::DevelopmentEventPayload::GeneralWork { action } = created.event.payload else {
        panic!()
    };
    let crate::general_work::GeneralChange::GoalCreated { goal } = action.change else {
        panic!()
    };
    let role = crate::team::Role::Planner;
    for (key, action) in [
        (
            "configure",
            crate::team::TeamAction::Configure {
                goal_id: goal.goal_id.clone(),
                budget: Default::default(),
                host_path: std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                host_sha256: "0".repeat(64),
            },
        ),
        (
            "reserve",
            crate::team::TeamAction::Reserve {
                goal_id: goal.goal_id.clone(),
                reservation: crate::team::Reservation {
                    run_id: "fixture-run".into(),
                    worker_id: "fixture-worker".into(),
                    parent_run_id: None,
                    work_id: None,
                    role: role.clone(),
                    grants: crate::team::role_grants(&role),
                },
            },
        ),
        (
            "running",
            crate::team::TeamAction::Transition {
                goal_id: goal.goal_id.clone(),
                run_id: "fixture-run".into(),
                state: crate::team::RunState::Running,
            },
        ),
    ] {
        crate::team::broker_action(&root, &op, action, key).unwrap();
    }
    let mut identity = binding();
    identity.owner = "fixture-worker".into();
    identity.team_goal = Some(goal.goal_id.clone());
    let child = s
        .spawn_owned(identity, &fixture(&root, &store, "grandchild"), false)
        .unwrap();
    wait_file(&root.join("grandchild.ready"));
    drop(child);
    let (_, capacity) = crate::team::status_with_project_concurrency(&root, &goal.goal_id).unwrap();
    assert_eq!(capacity.active_workers, 1);
    assert!(s
        .acknowledge_lost_result("fixture-run", "premature-release")
        .is_err());
    s.cancel("fixture-run", "cancel-team-fixture").unwrap();
    let (_, capacity) = crate::team::status_with_project_concurrency(&root, &goal.goal_id).unwrap();
    assert_eq!(
        capacity.active_workers, 1,
        "termination alone must not silently release canonical capacity"
    );
    assert!(
        !s.acknowledge_lost_result("fixture-run", "release-team-fixture")
            .unwrap()
            .capacity_blocked_by_uncertain_run
    );
    s.acknowledge_lost_result("fixture-run", "release-team-fixture")
        .unwrap();
    let (view, capacity) =
        crate::team::status_with_project_concurrency(&root, &goal.goal_id).unwrap();
    assert_eq!(capacity.active_workers, 0);
    assert_eq!(
        view.runs["fixture-run"].state,
        crate::team::RunState::LostResult
    );
    assert_eq!(
        crate::general_work::status(&root, &goal.goal_id)
            .unwrap()
            .goal_state,
        crate::general_work::GoalState::Active
    );
}
