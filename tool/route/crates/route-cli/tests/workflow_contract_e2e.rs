//! Scripted independent Route processes exercise the execution contract surface.
use serde_json::{json, Value};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn rpc(
    root: &Path,
    method: &str,
    params: Value,
    key: Option<&str>,
    credential: Option<&str>,
    operator: bool,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_route"));
    command
        .arg("rpc")
        .current_dir(root)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if operator {
        command.arg("--operator");
    }
    if let Some(secret) = credential {
        command.env("ROUTE_WORKER_CREDENTIAL", secret);
    }
    let mut child = command.spawn().unwrap();
    let raw = json!({"protocol":"route/1","request_id":format!("contract-{method}"),"method":method,"context":{},"params":params,"idempotency_key":key}).to_string();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(raw.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn ok(value: Value) -> Value {
    assert_eq!(value["ok"], true, "{value}");
    value["result"].clone()
}
fn op(root: &Path, method: &str, params: Value, key: &str) -> Value {
    ok(rpc(root, method, params, Some(key), None, true))
}
fn worker(root: &Path, method: &str, params: Value, key: &str, secret: &str) -> Value {
    ok(rpc(root, method, params, Some(key), Some(secret), false))
}
fn rev(root: &Path) -> u64 {
    route_basic::development::global_development_revision(root).unwrap()
}
fn step(id: &str) -> Value {
    json!({"step_id":id,"title":format!("Step {id}"),"requirement":"REQUIRED","dependencies":[],"proof":{"check_id":format!("check-{id}")}})
}
fn evidence(root: &Path, intent: &str, id: &str) -> String {
    let binary = env!("CARGO_BIN_EXE_route");
    let output = Command::new(binary)
        .args([
            "task",
            "exec",
            intent,
            "--check-id",
            &format!("check-{id}"),
            "--",
            binary,
            "--version",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    route_basic::EvidenceStore::load(root)
        .unwrap()
        .evidence
        .into_iter()
        .rev()
        .find(|e| e.metadata.get("check_id") == Some(&format!("check-{id}")))
        .unwrap()
        .id
}
fn create_fixture() -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let output = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    op(
        root,
        "worker.register",
        json!({"worker_id":"test-worker"}),
        "register",
    );
    let secret = route_basic::principal::generate_credential().unwrap();
    op(
        root,
        "worker.binding.issue",
        json!({"worker_id":"test-worker","credential_hash":route_core::sha256_hex(secret.as_bytes())}),
        "bind",
    );
    let intent = op(
        root,
        "intent.create",
        json!({"objective":"Verify A B C D without shortcutting required C"}),
        "intent",
    )["intent_ref"]
        .as_str()
        .unwrap()
        .to_string();
    (dir, intent, secret)
}

#[test]
fn anti_shortcut_restart_stale_and_transport_authority() {
    let (dir, intent, secret) = create_fixture();
    let root = dir.path();
    let hello = ok(rpc(
        root,
        "system.hello",
        json!({"supported_protocols":["route/1"]}),
        None,
        None,
        false,
    ));
    let methods = hello["capabilities"].as_array().unwrap();
    for name in [
        "workflow.list",
        "workflow.get",
        "workflow.create",
        "workflow.status",
        "workflow.step.start",
        "workflow.step.complete",
        "workflow.plan_delta.propose",
        "workflow.plan_delta.accept",
        "workflow.complete.check",
        "workflow.complete.request",
    ] {
        assert!(
            methods.iter().any(|m| m == name),
            "method not advertised: {name}"
        );
    }
    let spec = json!({"workflow_id":"wf-anti-shortcut","intent_ref":intent,"title":"Required steps","mode":"FULL_POWER","steps":[step("A"),step("B"),step("C"),step("D")]});
    assert_eq!(
        rpc(
            root,
            "workflow.create",
            json!({"expected_revision":rev(root),"spec":spec}),
            Some("worker-create"),
            Some(&secret),
            false
        )["ok"],
        false
    );
    op(
        root,
        "workflow.create",
        json!({"expected_revision":rev(root),"spec":spec}),
        "create",
    );
    for id in ["A", "B", "D"] {
        let proof = evidence(root, &intent, id);
        worker(
            root,
            "workflow.step.complete",
            json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut","version":1,"step_id":id,"evidence_ref":proof}),
            &format!("step-{id}"),
            &secret,
        );
    }
    let missing = ok(rpc(
        root,
        "workflow.complete.check",
        json!({"workflow_id":"wf-anti-shortcut"}),
        None,
        Some(&secret),
        false,
    ));
    assert_eq!(missing["completion"], "DENIED");
    assert_eq!(missing["missing_required_steps"], json!(["C"]));
    assert_eq!(
        ok(rpc(
            root,
            "workflow.status",
            json!({"workflow_id":"wf-anti-shortcut"}),
            None,
            None,
            false
        ))["steps"][3]["state"],
        "SATISFIED"
    );
    let denial = worker(
        root,
        "workflow.complete.request",
        json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut"}),
        "denied",
        &secret,
    );
    assert_eq!(
        denial["event"]["payload"]["data"]["action"]["change"]["report"]["completion"],
        "DENIED"
    );
    worker(
        root,
        "workflow.step.complete",
        json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut","version":1,"step_id":"C"}),
        "claim-C-done",
        &secret,
    );
    assert_eq!(
        ok(rpc(
            root,
            "workflow.complete.check",
            json!({"workflow_id":"wf-anti-shortcut"}),
            None,
            None,
            false
        ))["completion"],
        "DENIED"
    );
    let proof = evidence(root, &intent, "C");
    let params = json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut","version":1,"step_id":"C","evidence_ref":proof});
    let finished = worker(
        root,
        "workflow.step.complete",
        params.clone(),
        "verified-C",
        &secret,
    );
    let replay = worker(
        root,
        "workflow.step.complete",
        params.clone(),
        "verified-C",
        &secret,
    );
    assert_eq!(finished["event"]["event_id"], replay["event"]["event_id"]);
    let mut changed = params;
    changed["evidence_ref"] = json!("not-the-same-proof");
    assert_eq!(
        rpc(
            root,
            "workflow.step.complete",
            changed,
            Some("verified-C"),
            Some(&secret),
            false
        )["ok"],
        false
    );
    assert_eq!(
        ok(rpc(
            root,
            "workflow.complete.check",
            json!({"workflow_id":"wf-anti-shortcut"}),
            None,
            None,
            false
        ))["completion"],
        "PASS"
    );
    worker(
        root,
        "workflow.complete.request",
        json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut"}),
        "complete-1",
        &secret,
    );
    assert_eq!(
        ok(rpc(
            root,
            "workflow.status",
            json!({"workflow_id":"wf-anti-shortcut"}),
            None,
            None,
            false
        ))["workflow_status"],
        "COMPLETE"
    );
    std::fs::write(root.join("candidate.txt"), b"changed").unwrap();
    let restarted = ok(rpc(
        root,
        "workflow.status",
        json!({"workflow_id":"wf-anti-shortcut"}),
        None,
        None,
        false,
    ));
    assert_eq!(restarted["workflow_status"], "INCOMPLETE");
    assert_eq!(restarted["steps"][2]["state"], "STALE");
    assert_eq!(restarted["versions"].as_array().unwrap().len(), 1);
    for id in ["A", "B", "C", "D"] {
        let fresh = evidence(root, &intent, id);
        worker(
            root,
            "workflow.step.complete",
            json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut","version":1,"step_id":id,"evidence_ref":fresh}),
            &format!("fresh-{id}"),
            &secret,
        );
    }
    worker(
        root,
        "workflow.complete.request",
        json!({"expected_revision":rev(root),"workflow_id":"wf-anti-shortcut"}),
        "complete-2",
        &secret,
    );
    assert_eq!(
        ok(rpc(
            root,
            "workflow.status",
            json!({"workflow_id":"wf-anti-shortcut"}),
            None,
            None,
            false
        ))["workflow_status"],
        "COMPLETE"
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(["workflow", "status", "wf-anti-shortcut"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(cli.status.success());
    assert!(String::from_utf8_lossy(&cli.stdout).contains("Completion: COMPLETE"));
}

#[test]
fn plan_delta_waiver_principal_replay_and_concurrent_completion() {
    let (dir, intent, secret) = create_fixture();
    let root = dir.path();
    let conditional = json!({"step_id":"X","title":"Optional environment probe","requirement":"CONDITIONAL","dependencies":[],"condition":{"kind":"PATH_EXISTS","path":"not-installed.txt"},"proof":{"check_id":"check-X"}});
    let spec = json!({"workflow_id":"wf-plan","intent_ref":intent,"title":"Plan governance","mode":"CONTROLLED","steps":[step("A"),step("C"),conditional]});
    op(
        root,
        "workflow.create",
        json!({"expected_revision":rev(root),"spec":spec}),
        "create-plan",
    );
    let worker_skip = worker(
        root,
        "workflow.step.skip",
        json!({"expected_revision":rev(root),"workflow_id":"wf-plan","version":1,"step_id":"C","reason":"AI says unnecessary"}),
        "worker-waiver",
        &secret,
    );
    assert_eq!(worker_skip["event"]["actor_worker_id"], "test-worker");
    let before = ok(rpc(
        root,
        "workflow.status",
        json!({"workflow_id":"wf-plan"}),
        None,
        None,
        false,
    ));
    assert_eq!(before["steps"][1]["state"], "BYPASSED");
    assert_eq!(before["completion"]["unauthorized_bypasses"], json!(["C"]));
    worker(
        root,
        "workflow.step.skip",
        json!({"expected_revision":rev(root),"workflow_id":"wf-plan","version":1,"step_id":"X","reason":"path absent"}),
        "conditional",
        &secret,
    );
    assert_eq!(
        ok(rpc(
            root,
            "workflow.status",
            json!({"workflow_id":"wf-plan"}),
            None,
            None,
            false
        ))["steps"][2]["state"],
        "SKIPPED_VALID"
    );
    let candidate = json!({"delta_id":"","workflow_id":"wf-plan","from_version":1,"reason":"Replace C with stronger A verification","proposed_steps":[step("A"),json!({"step_id":"X","title":"Optional environment probe","requirement":"CONDITIONAL","dependencies":[],"condition":{"kind":"PATH_EXISTS","path":"not-installed.txt"},"proof":{"check_id":"check-X"}})]});
    let proposed = worker(
        root,
        "workflow.plan_delta.propose",
        json!({"expected_revision":rev(root),"delta":candidate}),
        "propose",
        &secret,
    );
    let delta_id = proposed["event"]["payload"]["data"]["action"]["change"]["delta"]["delta_id"]
        .as_str()
        .unwrap();
    assert_eq!(
        ok(rpc(
            root,
            "workflow.status",
            json!({"workflow_id":"wf-plan"}),
            None,
            None,
            false
        ))["spec"]["version"],
        1
    );
    assert_eq!(
        rpc(
            root,
            "workflow.plan_delta.accept",
            json!({"expected_revision":rev(root),"delta_id":delta_id}),
            Some("worker-accept"),
            Some(&secret),
            false
        )["ok"],
        false
    );
    op(
        root,
        "workflow.plan_delta.accept",
        json!({"expected_revision":rev(root),"delta_id":delta_id}),
        "accept",
    );
    let after = ok(rpc(
        root,
        "workflow.get",
        json!({"workflow_id":"wf-plan"}),
        None,
        None,
        false,
    ));
    assert_eq!(after["spec"]["version"], 2);
    assert_eq!(after["versions"].as_array().unwrap().len(), 2);
    assert_eq!(after["versions"][0]["steps"][1]["step_id"], "C");
    assert_eq!(after["versions"][1]["steps"][1]["step_id"], "X");
    let fresh = evidence(root, &intent, "A");
    let expected = rev(root);
    let a = root.to_path_buf();
    let b = root.to_path_buf();
    let s = secret.clone();
    let proof = fresh.clone();
    let report = std::thread::spawn(move || {
        rpc(
            &a,
            "workflow.step.complete",
            json!({"expected_revision":expected,"workflow_id":"wf-plan","version":2,"step_id":"A","evidence_ref":proof}),
            Some("race-report"),
            Some(&s),
            false,
        )
    });
    let s = secret.clone();
    let finish = std::thread::spawn(move || {
        rpc(
            &b,
            "workflow.complete.request",
            json!({"expected_revision":expected,"workflow_id":"wf-plan"}),
            Some("race-finish"),
            Some(&s),
            false,
        )
    });
    let result_a = report.join().unwrap();
    let result_b = finish.join().unwrap();
    assert_eq!(
        [result_a["ok"] == true, result_b["ok"] == true]
            .into_iter()
            .filter(|v| *v)
            .count(),
        1
    );
    let coherent = ok(rpc(
        root,
        "workflow.status",
        json!({"workflow_id":"wf-plan"}),
        None,
        None,
        false,
    ));
    assert_ne!(coherent["workflow_status"], "COMPLETE");
    op(
        root,
        "worker.binding.revoke",
        json!({"binding_id":op(root,"worker.binding.list",json!({}),"unused")[0]["binding_id"]}),
        "revoke",
    );
    assert_eq!(
        rpc(
            root,
            "workflow.step.start",
            json!({"expected_revision":rev(root),"workflow_id":"wf-plan","version":2,"step_id":"A"}),
            Some("revoked"),
            Some(&secret),
            false
        )["ok"],
        false
    );
    let (other, _, _) = create_fixture();
    assert_eq!(
        rpc(
            other.path(),
            "workflow.step.start",
            json!({"expected_revision":rev(other.path()),"workflow_id":"wf-plan","version":2,"step_id":"A"}),
            Some("foreign"),
            Some(&secret),
            false
        )["ok"],
        false
    );
}
