//! Real-process integration checks shared across timing policy choices.
use serde_json::{json, Value};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
#[ignore = "subprocess fixture launched by integration test"]
fn held_check_subprocess() {
    let marker = std::env::var("ROUTE_TEST_GATE_MARKER").unwrap();
    let release = std::env::var("ROUTE_TEST_GATE_RELEASE").unwrap();
    std::fs::write(marker, b"running").unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !Path::new(&release).exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(Path::new(&release).exists(), "test gate was not released");
}

fn rpc(
    root: &Path,
    method: &str,
    params: Value,
    key: Option<&str>,
    credential: Option<&str>,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_route"));
    command
        .arg("rpc")
        .current_dir(root)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if credential.is_none() && key.is_some() {
        command.arg("--operator");
    }
    if let Some(secret) = credential {
        command.env("ROUTE_WORKER_CREDENTIAL", secret);
    }
    let mut child = command.spawn().unwrap();
    let request = json!({
        "protocol": "route/1",
        "request_id": format!("integration-{method}"),
        "method": method,
        "context": {},
        "params": params,
        "idempotency_key": key,
    });
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn ok(response: Value) -> Value {
    assert_eq!(response["ok"], true, "{response}");
    response["result"].clone()
}

fn work_id(result: &Value) -> String {
    result["event"]["payload"]["data"]["action"]["work"]["work_id"]
        .as_str()
        .unwrap()
        .into()
}

fn claim_id(result: &Value) -> String {
    result["event"]["payload"]["data"]["action"]["claim"]["claim_id"]
        .as_str()
        .unwrap()
        .into()
}

fn system_check(root: &Path, intent: &str, check_id: &str) -> String {
    let binary = env!("CARGO_BIN_EXE_route");
    let output = Command::new(binary)
        .args([
            "task",
            "exec",
            intent,
            "--check-id",
            check_id,
            "--",
            binary,
            "--version",
        ])
        .current_dir(root)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    route_basic::EvidenceStore::load(root)
        .unwrap()
        .evidence
        .iter()
        .rev()
        .find(|e| {
            e.session_id == intent
                && e.source == route_basic::EvidenceSource::System
                && e.kind == route_basic::EvidenceKind::CheckPass
                && e.metadata.get("check_id").is_some_and(|id| id == check_id)
        })
        .unwrap()
        .id
        .clone()
}

#[test]
fn integration_checks_all_claims_content_and_keyed_retry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let init = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .current_dir(root)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let mut workers = Vec::new();
    for id in ["a", "b"] {
        ok(rpc(
            root,
            "worker.register",
            json!({"worker_id": id}),
            Some(&format!("register-{id}")),
            None,
        ));
        let secret = route_basic::principal::generate_credential().unwrap();
        ok(rpc(
            root,
            "worker.binding.issue",
            json!({"worker_id": id, "credential_hash": route_core::sha256_hex(secret.as_bytes())}),
            Some(&format!("bind-{id}")),
            None,
        ));
        workers.push(secret);
    }
    let intent = ok(rpc(
        root,
        "intent.create",
        json!({"objective": "Verify concurrent bounded integration"}),
        Some("intent"),
        None,
    ))["intent_ref"]
        .as_str()
        .unwrap()
        .to_string();

    let mut claims = Vec::new();
    for (index, worker) in workers.iter().enumerate() {
        let requirement = format!("check-{index}");
        let work = work_id(&ok(rpc(
            root,
            "work.create_child",
            json!({"intent_ref": intent, "title": format!("Independent claim {index}"), "kind": "TEST_GAP", "scope_paths": [format!("probe-{index}.txt")], "verification_requirements": [requirement], "overlap_mode": "EXCLUSIVE"}),
            Some(&format!("work-{index}")),
            Some(worker),
        )));
        claims.push(claim_id(&ok(rpc(
            root,
            "work.claim",
            json!({"work_id": work}),
            Some(&format!("claim-{index}")),
            Some(worker),
        ))));
    }

    let gate = tempfile::tempdir().unwrap();
    let marker = gate.path().join("started");
    let release = gate.path().join("release");
    let test_binary = std::env::current_exe().unwrap();
    let held_check = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_route"));
        command
            .args(["task", "exec", &intent, "--check-id", "check-0", "--"])
            .arg(&test_binary)
            .args(["--exact", "held_check_subprocess", "--ignored"])
            .env("ROUTE_TEST_GATE_MARKER", &marker)
            .env("ROUTE_TEST_GATE_RELEASE", &release)
            .env_remove("ROUTE_WORKER_CREDENTIAL")
            .current_dir(root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    };
    let running = held_check().spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(marker.exists(), "held check never started");
    let first_finish = ok(rpc(
        root,
        "work.finish",
        json!({"claim_id": claims[0], "reason": "first candidate"}),
        Some("finish-a"),
        Some(&workers[0]),
    ));
    let first_completion_revision = first_finish["event"]["sequence"].as_u64().unwrap();
    std::fs::write(&release, b"continue").unwrap();
    let held_output = running.wait_with_output().unwrap();
    assert!(
        held_output.status.success(),
        "{}",
        String::from_utf8_lossy(&held_output.stderr)
    );
    let rerun = held_check().output().unwrap();
    assert!(
        rerun.status.success(),
        "{}",
        String::from_utf8_lossy(&rerun.stderr)
    );
    let held_evidence: Vec<_> = route_basic::EvidenceStore::load(root)
        .unwrap()
        .evidence
        .into_iter()
        .filter(|e| e.metadata.get("check_id").is_some_and(|id| id == "check-0"))
        .collect();
    assert_eq!(held_evidence.len(), 2, "rerun needs separate evidence");
    let start_revisions: Vec<_> = held_evidence
        .iter()
        .map(|e| e.metadata["check_started_revision"].parse::<u64>().unwrap())
        .collect();
    assert!(start_revisions[0] < first_completion_revision);
    assert!(start_revisions[1] >= first_completion_revision);

    let first_evidence = system_check(root, &intent, "check-0");
    let peer_evidence = system_check(root, &intent, "check-1");
    let state = route_basic::compute_state_hash(root).unwrap();
    let revision = ok(rpc(
        root,
        "work.available",
        json!({}),
        None,
        Some(&workers[0]),
    ))["global_revision"]
        .as_u64()
        .unwrap();
    let active_peer = rpc(
        root,
        "work.integrate",
        json!({"intent_ref": intent, "accepted_claim_ids": claims, "evidence_refs": [first_evidence, peer_evidence], "state_hash": state, "expected_revision": revision}),
        Some("active-peer"),
        None,
    );
    assert_eq!(
        active_peer["ok"], false,
        "active peer claim must block integration"
    );
    assert!(active_peer["error"]["message"]
        .as_str()
        .unwrap()
        .contains("UNVERIFIED_CLAIM"));

    let second_finish = ok(rpc(
        root,
        "work.finish",
        json!({"claim_id": claims[1], "reason": "second candidate"}),
        Some("finish-b"),
        Some(&workers[1]),
    ));
    let completion_timestamp = second_finish["event"]["timestamp"].as_i64().unwrap();
    std::fs::write(root.join("probe-0.txt"), b"first version").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let outdated_evidence = [
        system_check(root, &intent, "check-0"),
        system_check(root, &intent, "check-1"),
    ];
    std::fs::write(root.join("probe-0.txt"), b"second version").unwrap();
    let current_state = route_basic::compute_state_hash(root).unwrap();
    let outdated_state = route_basic::EvidenceStore::load(root)
        .unwrap()
        .evidence
        .iter()
        .find(|e| e.id == outdated_evidence[0])
        .unwrap()
        .state_hash
        .clone()
        .unwrap();
    assert_ne!(current_state, outdated_state);
    let current_revision = ok(rpc(
        root,
        "work.available",
        json!({}),
        None,
        Some(&workers[0]),
    ))["global_revision"]
        .as_u64()
        .unwrap();
    let stale = rpc(
        root,
        "work.integrate",
        json!({"intent_ref": intent, "accepted_claim_ids": claims, "evidence_refs": outdated_evidence, "state_hash": current_state, "expected_revision": current_revision}),
        Some("stale-content"),
        None,
    );
    assert_eq!(
        stale["ok"], false,
        "newer bytes require new System evidence"
    );
    assert!(stale["error"]["message"]
        .as_str()
        .unwrap()
        .contains("INTEGRATION_EVIDENCE_REQUIRED"));

    std::thread::sleep(std::time::Duration::from_millis(2));
    let checks = [
        system_check(root, &intent, "check-0"),
        system_check(root, &intent, "check-1"),
    ];
    // The commands actually ran after completion. Simulate millisecond clock
    // coalescing in this disposable fixture to ensure causal order is enough.
    let mut evidence = route_basic::EvidenceStore::load(root).unwrap();
    for item in &mut evidence.evidence {
        if checks.contains(&item.id) {
            item.created_at = completion_timestamp;
        }
    }
    evidence.save(root).unwrap();
    let params = json!({"intent_ref": intent, "accepted_claim_ids": claims, "evidence_refs": checks, "state_hash": current_state, "expected_revision": current_revision});
    let integrated = ok(rpc(
        root,
        "work.integrate",
        params.clone(),
        Some("integrate"),
        None,
    ));
    assert_eq!(integrated["global_revision"], current_revision + 1);
    let replay = ok(rpc(root, "work.integrate", params, Some("integrate"), None));
    assert_eq!(replay["event"]["event_id"], integrated["event"]["event_id"]);
    assert_eq!(
        ok(rpc(
            root,
            "work.available",
            json!({}),
            None,
            Some(&workers[0])
        ))["global_revision"],
        current_revision + 1
    );
}
