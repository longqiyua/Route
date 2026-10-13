use super::*;
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn fixture() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    crate::BasicRepository::init(dir.path()).unwrap();
    crate::ensure_identity(dir.path()).unwrap();
    std::fs::write(dir.path().join("source.txt"), "one").unwrap();
    let result = crate::general_work::operator_action(
        dir.path(),
        &crate::principal::CallerContext::trusted_operator(),
        "goal.create",
        json!({"goal":{"title":"Execution trust fixture","domain":"DEVELOPMENT"},
            "expected_revision":crate::global_development_revision(dir.path()).unwrap()}),
        "goal",
    )
    .unwrap();
    let crate::DevelopmentEventPayload::GeneralWork { action } = result.event.payload else {
        panic!()
    };
    let crate::general_work::GeneralChange::GoalCreated { goal } = action.change else {
        panic!()
    };
    (dir, goal.goal_id)
}
fn helper(data: &Value) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "check_run::tests::process_fixture",
        "--ignored",
        "--nocapture",
        "--skip",
        &data.to_string(),
    ]);
    command.stdout(Stdio::null()).stderr(Stdio::null());
    command
}
fn contract(root: &Path, counter: &Path, mode: &str) -> CheckContract {
    let data = json!({"mode":mode,"counter":counter});
    CheckContract::approved(
        root,
        vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--exact".into(),
            "check_run::tests::process_fixture".into(),
            "--ignored".into(),
            "--nocapture".into(),
            "--skip".into(),
            data.to_string(),
        ],
        vec!["a".into(), "b".into()],
        "approved-policy-v1".into(),
    )
    .unwrap()
}
fn count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .count()
}

// Other legacy tests temporarily mutate the process environment for archive
// isolation. A full inherited-environment contract must not run beside them.
// Keep production environment checks strict; isolate these tests, not the hash.
fn isolated(name: &str) -> bool {
    const MARKER: &str = "execution-trust-isolated-test";
    if std::env::args().any(|argument| argument == MARKER) {
        return false;
    }
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("check_run::tests::{name}"),
            "--nocapture",
            "--skip",
            MARKER,
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "isolated execution-trust test failed: {name}"
    );
    true
}

#[test]
#[ignore = "owned subprocess fixture, invoked with JSON in libtest skip argument"]
fn process_fixture() {
    let args: Vec<_> = std::env::args().collect();
    let data: Value =
        serde_json::from_str(&args[args.iter().position(|s| s == "--skip").unwrap() + 1]).unwrap();
    match data["mode"].as_str().unwrap() {
        "count" | "fail" => {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(data["counter"].as_str().unwrap())
                .unwrap();
            writeln!(file, "run").unwrap();
            file.sync_all().unwrap();
            if data["mode"] == "fail" {
                std::process::exit(7);
            }
        }
        "sleep" => std::thread::sleep(std::time::Duration::from_secs(10)),
        "run" => {
            let root = Path::new(data["root"].as_str().unwrap());
            let contract = serde_json::from_value(data["contract"].clone()).unwrap();
            let crash = data["crash"].as_str().unwrap_or("");
            crate::bounded_runner::CRASH_AFTER_SPAWN.with(|flag| flag.set(crash == "after_spawn"));
            execute_inner(
                root,
                data["goal"].as_str().unwrap(),
                data["key"].as_str().unwrap(),
                contract,
                |point| {
                    if point == crash {
                        std::process::exit(23);
                    }
                    Ok(())
                },
            )
            .unwrap();
        }
        _ => panic!("unknown fixture mode"),
    }
}

#[test]
fn exact_replay_keeps_distinct_obligations_and_changed_world_requires_new_run() {
    if isolated("exact_replay_keeps_distinct_obligations_and_changed_world_requires_new_run") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let counter = external.path().join("count");
    let approved = contract(dir.path(), &counter, "count");
    let first = execute(dir.path(), &goal, "one", approved.clone()).unwrap();
    let replay = execute(dir.path(), &goal, "one", approved.clone()).unwrap();
    assert_eq!(count(&counter), 1);
    assert_eq!(first.run_id, replay.run_id);
    assert_ne!(first.evidence_ids["a"], first.evidence_ids["b"]);
    let store = EvidenceStore::load(dir.path()).unwrap();
    for label in ["a", "b"] {
        let evidence = store
            .evidence
            .iter()
            .find(|e| e.id == first.evidence_ids[label])
            .unwrap();
        assert_eq!(evidence.metadata["check_id"], label);
        assert_eq!(evidence.metadata["check_run_id"], first.run_id);
        assert_eq!(evidence.kind, execution::EvidenceKind::CheckPass);
    }
    let mut changed = approved.clone();
    changed.policy_hash = "new-policy".into();
    assert!(execute(dir.path(), &goal, "one", changed.clone())
        .unwrap_err()
        .to_string()
        .contains("IDEMPOTENCY_CONFLICT"));
    execute(dir.path(), &goal, "new-policy", changed).unwrap();
    assert_eq!(count(&counter), 2);
    std::fs::write(dir.path().join("source.txt"), "two").unwrap();
    assert!(execute(dir.path(), &goal, "one", approved.clone())
        .unwrap_err()
        .to_string()
        .contains("STALE"));
    let fresh = execute(dir.path(), &goal, "changed-world", approved).unwrap();
    assert_eq!(count(&counter), 3);
    assert_ne!(first.world_before, fresh.world_before);
}

#[test]
fn failed_observation_never_becomes_pass_and_fresh_key_really_reruns() {
    if isolated("failed_observation_never_becomes_pass_and_fresh_key_really_reruns") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let counter = external.path().join("count");
    let approved = contract(dir.path(), &counter, "fail");
    for _ in 0..2 {
        let result = execute(dir.path(), &goal, "failed", approved.clone()).unwrap();
        assert_eq!(result.result.unwrap().exit_code, 7);
        let store = EvidenceStore::load(dir.path()).unwrap();
        assert!(result.evidence_ids.values().all(|id| store
            .evidence
            .iter()
            .any(|e| &e.id == id && e.kind == execution::EvidenceKind::CheckFail)));
    }
    assert_eq!(count(&counter), 1);
    execute(dir.path(), &goal, "retry-failure", approved).unwrap();
    assert_eq!(count(&counter), 2);
    assert_eq!(
        crate::general_work::status(dir.path(), &goal)
            .unwrap()
            .goal_state,
        crate::general_work::GoalState::Active
    );
}

#[test]
fn later_failed_check_invalidates_earlier_pass_on_exact_replay() {
    if isolated("later_failed_check_invalidates_earlier_pass_on_exact_replay") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let counter = external.path().join("count");
    let approved = contract(dir.path(), &counter, "count");
    execute(dir.path(), &goal, "pass", approved.clone()).unwrap();
    let failed = contract(dir.path(), &counter, "fail");
    execute(dir.path(), &goal, "fail", failed).unwrap();
    let error = execute(dir.path(), &goal, "pass", approved).unwrap_err();
    assert!(error.to_string().contains("PROOF_INVALIDATED"));
    assert_eq!(count(&counter), 2);
}

#[test]
fn real_process_crashes_preserve_uncertainty_or_recover_without_execution() {
    if isolated("real_process_crashes_preserve_uncertainty_or_recover_without_execution") {
        return;
    }
    for point in [
        "reserved",
        "before_spawn",
        "after_spawn",
        "after_exit",
        "before_evidence",
        "after_evidence",
    ] {
        let (dir, goal) = fixture();
        let external = tempfile::tempdir().unwrap();
        let counter = external.path().join("count");
        let approved = contract(dir.path(), &counter, "count");
        let data = json!({"mode":"run","root":dir.path(),"goal":goal,"key":"crash","contract":approved,"crash":point});
        assert_eq!(helper(&data).status().unwrap().code(), Some(23), "{point}");
        let saved = inspect(dir.path(), "crash").unwrap();
        let before = count(&counter);
        if matches!(point, "before_spawn" | "after_spawn" | "after_exit") {
            assert_eq!(saved.state, CheckRunState::PossiblyRunning);
            assert!(execute(dir.path(), &goal, "crash", approved.clone())
                .unwrap_err()
                .to_string()
                .contains("UNCERTAIN"));
            assert!(execute(dir.path(), &goal, "bypass", approved)
                .unwrap_err()
                .to_string()
                .contains("RECOVERY_REQUIRED"));
            if point == "after_spawn" {
                assert!(count(&counter) <= 1);
            } else {
                assert_eq!(count(&counter), before);
            }
        } else {
            let mut resume = data.clone();
            resume["crash"] = json!("");
            assert!(
                helper(&resume).status().unwrap().success(),
                "recover {point}"
            );
            let recovered = inspect(dir.path(), "crash").unwrap();
            assert_eq!(recovered.state, CheckRunState::EvidenceCommitted);
            assert_eq!(recovered.run_id, saved.run_id);
            assert!(recovered.attempt > saved.attempt);
            assert_eq!(count(&counter), 1);
            assert_eq!(
                EvidenceStore::load(dir.path()).unwrap().evidence.len(),
                2,
                "no duplicate Evidence after {point}"
            );
        }
    }
}

#[test]
fn cross_process_same_key_converges_and_distinct_keys_do_not_collapse() {
    if isolated("cross_process_same_key_converges_and_distinct_keys_do_not_collapse") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let counter = external.path().join("count");
    let approved = contract(dir.path(), &counter, "count");
    let data = json!({"mode":"run","root":dir.path(),"goal":goal,"key":"race","contract":approved});
    let mut a = helper(&data).spawn().unwrap();
    let mut b = helper(&data).spawn().unwrap();
    assert!(a.wait().unwrap().success());
    assert!(b.wait().unwrap().success());
    assert_eq!(count(&counter), 1);
    let mut second = data.clone();
    second["key"] = json!("distinct");
    assert!(helper(&second).status().unwrap().success());
    assert_eq!(count(&counter), 2);
}

#[test]
fn stale_attempt_is_fenced_before_evidence_commit() {
    if isolated("stale_attempt_is_fenced_before_evidence_commit") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let approved = contract(dir.path(), &external.path().join("count"), "count");
    let error = execute_inner(dir.path(), &goal, "fence", approved, |point| {
        if point == "before_evidence" {
            let mut newer = inspect(dir.path(), "fence")?;
            newer.owner = "newer-owner".into();
            newer.attempt += 1;
            persist(&location(dir.path(), "fence"), &newer)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("FENCED"));
    assert!(EvidenceStore::load(dir.path()).unwrap().evidence.is_empty());
    assert_eq!(inspect(dir.path(), "fence").unwrap().owner, "newer-owner");
}

#[test]
fn deadline_persists_recovery_diagnostic_not_fake_check_failure() {
    if isolated("deadline_persists_recovery_diagnostic_not_fake_check_failure") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let mut approved = contract(dir.path(), &external.path().join("count"), "sleep");
    approved.deadline_ms = 100;
    let error = execute(dir.path(), &goal, "deadline", approved).unwrap_err();
    assert!(format!("{error:#}").contains("RUN_DEADLINE_EXCEEDED"));
    let run = inspect(dir.path(), "deadline").unwrap();
    assert_eq!(run.state, CheckRunState::RecoveryRequired);
    let diagnostic = run.diagnostic.unwrap();
    assert_eq!(diagnostic.code, "RUN_DEADLINE_EXCEEDED");
    assert_eq!(diagnostic.parent_termination_confirmed, Some(true));
    assert!(run.result.is_none());
    assert!(run.evidence_ids.is_empty());
    assert!(EvidenceStore::load(dir.path()).unwrap().evidence.is_empty());
}

#[test]
fn evidence_write_rejection_preserves_result_and_retry_does_not_spawn() {
    if isolated("evidence_write_rejection_preserves_result_and_retry_does_not_spawn") {
        return;
    }
    let (dir, goal) = fixture();
    let external = tempfile::tempdir().unwrap();
    let counter = external.path().join("count");
    let approved = contract(dir.path(), &counter, "count");
    let blocked = dir.path().join(".route/execution/evidence.json");
    let error = execute_inner(dir.path(), &goal, "reject", approved.clone(), |point| {
        if point == "before_evidence" {
            std::fs::create_dir(&blocked)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("EVIDENCE_COMMIT_FAILED"));
    let saved = inspect(dir.path(), "reject").unwrap();
    assert_eq!(saved.state, CheckRunState::ResultPersisted);
    assert!(saved.evidence_ids.is_empty());
    assert_eq!(count(&counter), 1);
    std::fs::remove_dir(&blocked).unwrap(); // Only this fixture's empty rejection directory.
    let recovered = execute(dir.path(), &goal, "reject", approved).unwrap();
    assert_eq!(recovered.run_id, saved.run_id);
    assert_eq!(recovered.state, CheckRunState::EvidenceCommitted);
    assert_eq!(count(&counter), 1);
}

#[test]
#[ignore = "explicit execution-trust measurement; disposable fixtures, no performance gate"]
fn measure_execution_trust_costs() {
    use std::time::Instant;
    for (label, history) in [
        ("small", 0),
        ("medium-synthetic", 128),
        ("current-source-snapshot", 512),
    ] {
        let (dir, goal) = fixture();
        if label == "current-source-snapshot" {
            let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(4)
                .unwrap();
            let files = Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["ls-files", "-z", "--", "tool/route/crates"])
                .output()
                .unwrap();
            assert!(files.status.success());
            let mut copied = 0;
            for name in files.stdout.split(|b| *b == 0).filter(|s| !s.is_empty()) {
                let path = Path::new(std::str::from_utf8(name).unwrap());
                if matches!(
                    path.extension().and_then(|e| e.to_str()),
                    Some("rs" | "toml")
                ) && repo.join(path).is_file()
                {
                    let destination = dir.path().join("source-snapshot").join(path);
                    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
                    std::fs::copy(repo.join(path), destination).unwrap();
                    copied += 1;
                }
            }
            assert!(copied > 100);
            println!("snapshot_source_files={copied}");
        } else {
            for i in 0..if history == 0 { 8 } else { 256 } {
                std::fs::write(
                    dir.path().join(format!("source-{i}.txt")),
                    "fixture\n".repeat(128),
                )
                .unwrap();
            }
        }
        let external = tempfile::tempdir().unwrap();
        let approved = contract(dir.path(), &external.path().join("count"), "count");
        let start = Instant::now();
        let mut evidence_start = None;
        let run = execute_inner(dir.path(), &goal, "measure", approved.clone(), |point| {
            if point == "before_evidence" {
                evidence_start = Some(Instant::now());
            }
            Ok(())
        })
        .unwrap();
        let creation = start.elapsed().as_secs_f64() * 1000.0;
        let evidence = evidence_start.unwrap().elapsed().as_secs_f64() * 1000.0;
        // Synthetic progress records for OTHER Goals exercise the conservative
        // recovery scan; these are only in this disposable measurement fixture.
        for i in 0..history {
            let mut unrelated = run.clone();
            unrelated.intent_ref = format!("unrelated-fixture-{i}");
            persist(&location(dir.path(), &format!("history-{i}")), &unrelated).unwrap();
        }
        let start = Instant::now();
        inspect(dir.path(), "measure").unwrap();
        let lookup = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        execute(dir.path(), &goal, "measure", approved.clone()).unwrap();
        let replay = start.elapsed().as_secs_f64() * 1000.0;
        let error = execute_inner(
            dir.path(),
            &goal,
            "recover-measure",
            approved.clone(),
            |point| {
                if point == "before_evidence" {
                    anyhow::bail!("MEASUREMENT_INTERRUPTION");
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("MEASUREMENT_INTERRUPTION"));
        let start = Instant::now();
        execute(dir.path(), &goal, "recover-measure", approved).unwrap();
        println!("EXECUTION_TRUST_COST label={label} history={history} create_including_process_ms={creation:.3} evidence_commit_ms={evidence:.3} direct_lookup_ms={lookup:.3} dedup_with_scan_ms={replay:.3} recovery_with_scan_ms={:.3}",start.elapsed().as_secs_f64()*1000.0);
    }
}
