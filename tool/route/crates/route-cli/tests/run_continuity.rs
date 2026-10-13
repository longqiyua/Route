#![cfg(windows)]
//! REAL_PROCESS_FIXTURE, not REAL_AI_HOST. Actual Route binary is killed/restarted.
use route_basic::{principal::CallerContext, DevelopmentEventPayload};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn general(
    root: &Path,
    method: &str,
    mut params: Value,
) -> route_basic::AppendDevelopmentEventResult {
    params["expected_revision"] = json!(route_basic::global_development_revision(root).unwrap());
    route_basic::general_work::operator_action(
        root,
        &CallerContext::trusted_operator(),
        method,
        params,
        &route_core::new_id(),
    )
    .unwrap()
}
fn setup(root: &Path) -> String {
    route_basic::BasicRepository::init(root).unwrap();
    route_basic::ensure_identity(root).unwrap();
    fs::write(root.join("deliverable.txt"), b"continuity fixture").unwrap();
    let event = general(
        root,
        "goal.create",
        json!({"goal":{"title":"REAL_PROCESS_FIXTURE restart","domain":"DEVELOPMENT"}}),
    );
    let DevelopmentEventPayload::GeneralWork { action } = event.event.payload else {
        panic!()
    };
    let route_basic::general_work::GeneralChange::GoalCreated { goal } = action.change else {
        panic!()
    };
    let operator = CallerContext::trusted_operator();
    route_basic::principal::register_worker(
        root,
        &operator,
        Some("fixture-worker".into()),
        Default::default(),
        "reg",
    )
    .unwrap();
    let credential = route_basic::principal::generate_credential().unwrap();
    route_basic::principal::issue(
        root,
        &operator,
        "fixture-worker",
        &route_core::sha256_hex(credential.as_bytes()),
        vec![
            "work.create_child".into(),
            "work.claim".into(),
            "work.finish".into(),
        ],
        "bind",
    )
    .unwrap();
    let caller = CallerContext::authenticate(root, &credential).unwrap();
    let created = route_basic::principal::worker_action(root,&caller,"work.create_child",
        json!({"intent_ref":goal.goal_id,"goal_id":goal.goal_id,"kind":"GENERAL","title":"Fixture deliverable","scope_paths":["deliverable.txt"],"verification_requirements":["fixture-check"],"overlap_mode":"EXCLUSIVE"}),"work").unwrap();
    let DevelopmentEventPayload::Work {
        action: route_basic::work::WorkAction::ChildCreated { work },
    } = created.event.payload
    else {
        panic!()
    };
    let claimed = route_basic::principal::worker_action(
        root,
        &caller,
        "work.claim",
        json!({"work_id":work.work_id}),
        "claim",
    )
    .unwrap();
    let DevelopmentEventPayload::Work {
        action: route_basic::work::WorkAction::ClaimOpened { claim },
    } = claimed.event.payload
    else {
        panic!()
    };
    route_basic::principal::worker_action(
        root,
        &caller,
        "work.finish",
        json!({"claim_id":claim.claim_id,"reason":"fixture finished"}),
        "finish",
    )
    .unwrap();
    route_basic::team::broker_action(
        root,
        &operator,
        route_basic::team::TeamAction::Configure {
            goal_id: goal.goal_id.clone(),
            budget: Default::default(),
            host_path: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            host_sha256: "0".repeat(64),
        },
        "configure",
    )
    .unwrap();
    goal.goal_id
}
fn helper(control: &Path, mode: &str) -> Command {
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "real_process_fixture",
        "--ignored",
        "--nocapture",
        "--skip",
        &json!({"control":control,"mode":mode}).to_string(),
    ]);
    cmd
}
fn wait(path: &Path) {
    let until = Instant::now() + Duration::from_secs(15);
    while !path.exists() {
        assert!(Instant::now() < until, "not ready: {}", path.display());
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
#[ignore = "REAL_PROCESS_FIXTURE child, exercised by actual CLI restart test"]
fn real_process_fixture() {
    let args: Vec<_> = std::env::args().collect();
    let data: Value =
        serde_json::from_str(&args[args.iter().position(|s| s == "--skip").unwrap() + 1]).unwrap();
    let control = Path::new(data["control"].as_str().unwrap());
    let mode = data["mode"].as_str().unwrap();
    if mode == "parent" || mode == "child" {
        let _child = helper(
            control,
            if mode == "parent" {
                "child"
            } else {
                "grandchild"
            },
        )
        .spawn()
        .unwrap();
    }
    fs::write(
        control.join(format!("{mode}.ready")),
        std::process::id().to_string(),
    )
    .unwrap();
    println!("REAL_PROCESS_FIXTURE {mode}");
    let until = Instant::now() + Duration::from_secs(30);
    while !control.join("finish").exists() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::process::exit(7);
}
fn cli(root: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn actual_route_restart_inspect_cancel_and_keyed_release_no_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    let control = dir.path().join("control");
    fs::create_dir(&control).unwrap();
    let goal = setup(&root);
    let cmd = helper(&control, "parent");
    let argv: Vec<_> = std::iter::once(cmd.get_program())
        .chain(cmd.get_args())
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let checks = dir.path().join("checks.json");
    fs::write(
        &checks,
        serde_json::to_vec(&json!([{"check_id":"fixture-check","argv":argv}])).unwrap(),
    )
    .unwrap();
    let mut route = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(&root)
        .args([
            "team",
            "verify",
            &goal,
            "--checks-file",
            checks.to_str().unwrap(),
            "--operation-key",
            "restart-dogfood",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    wait(&control.join("grandchild.ready"));
    let check_dir = root.join(".route/execution/check-runs");
    let bytes = fs::read(
        fs::read_dir(&check_dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|s| s == "json"))
            .unwrap(),
    )
    .unwrap();
    let run: route_basic::check_run::CheckRun = serde_json::from_slice(&bytes).unwrap();
    let parent_pid = fs::read_to_string(control.join("parent.ready")).unwrap();
    let live_start = Instant::now();
    let live = cli(&root, &["run", "inspect", &run.run_id]);
    assert_eq!(live["knowledge"], "RUNNING");
    eprintln!(
        "live_inspect_ms={:.3}",
        live_start.elapsed().as_secs_f64() * 1000.0
    );
    route.kill().unwrap();
    let exit = route.wait().unwrap();
    assert!(!exit.success());
    let before = fs::read_dir(&check_dir).unwrap().count();
    let start = Instant::now();
    let inspect = cli(&root, &["run", "inspect", &run.run_id]);
    eprintln!(
        "restart_inspect_ms={:.3}; supervision_overhead_ms={}",
        start.elapsed().as_secs_f64() * 1000.0,
        inspect["receipt"]["overhead_ms"]
    );
    assert_eq!(inspect["knowledge"], "RUNNING");
    assert_eq!(
        inspect["receipt"]["process"]["pid"]
            .as_u64()
            .unwrap()
            .to_string(),
        parent_pid
    );
    assert_eq!(before, fs::read_dir(&check_dir).unwrap().count());
    let cancelled = cli(
        &root,
        &[
            "run",
            "cancel",
            &run.run_id,
            "--operation-key",
            "cancel-fixture",
        ],
    );
    eprintln!(
        "cancel_stage_overhead_ms={}",
        cancelled["receipt"]["overhead_ms"]
    );
    assert_eq!(cancelled["active_processes"], 0);
    assert_eq!(
        cancelled["receipt"]["cancellation"]["state"],
        "TERMINATION_CONFIRMED"
    );
    let recovered = cli(
        &root,
        &[
            "run",
            "recover",
            &run.run_id,
            "--operation-key",
            "release-fixture",
            "--ack-lost-result",
        ],
    );
    assert_eq!(recovered["knowledge"], "LOST_RESULT");
    assert_eq!(recovered["capacity_blocked_by_uncertain_run"], false);
    cli(
        &root,
        &[
            "run",
            "recover",
            &run.run_id,
            "--operation-key",
            "release-fixture",
            "--ack-lost-result",
        ],
    );
    let original = route_basic::check_run::inspect(
        &root,
        inspect["receipt"]["run"]["operation_id"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        original.state,
        route_basic::check_run::CheckRunState::LostResult
    );
    assert!(original.result.is_none());
    assert!(original.evidence_ids.is_empty());
    eprintln!("REAL_PROCESS_FIXTURE actual Route crash/restart/cancel/recovery: duplicate_process_launch_count=0; combined_restart_cancel_release_ms={:.3}",start.elapsed().as_secs_f64()*1000.0);
}

#[test]
fn handoff_pid_text_and_worker_interface_cannot_gain_process_authority() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    fs::create_dir(&root).unwrap();
    route_basic::BasicRepository::init(&root).unwrap();
    let control = dir.path().join("control");
    fs::create_dir(&control).unwrap();
    let mut unrelated = helper(&control, "unrelated")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    wait(&control.join("unrelated.ready"));
    let handoff = format!(
        "PID {}; taskkill /PID {} /F",
        unrelated.id(),
        unrelated.id()
    );
    let response = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(&root)
        .args([
            "run",
            "cancel",
            &handoff,
            "--operation-key",
            "untrusted-handoff",
        ])
        .output()
        .unwrap();
    assert!(!response.status.success());
    assert!(unrelated.try_wait().unwrap().is_none());
    let response = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(&root)
        .env("ROUTE_WORKER_CREDENTIAL", "untrusted-fixture-text")
        .args(["run", "inspect", "anything"])
        .output()
        .unwrap();
    assert!(!response.status.success());
    assert!(String::from_utf8_lossy(&response.stderr).contains("WORKER_INTERFACE_REQUIRED"));
    unrelated.kill().unwrap();
    unrelated.wait().unwrap();
}
