//! Real route/1 subprocess coverage. These scripted clients are not AI model dogfood.
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
    let raw = json!({"protocol":"route/1","request_id":format!("test-{method}"),"method":method,"context":{},"params":params,"idempotency_key":key}).to_string();
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
fn child_id(value: &Value) -> String {
    value["event"]["payload"]["data"]["action"]["work"]["work_id"]
        .as_str()
        .unwrap()
        .into()
}
fn claim_id(value: &Value) -> String {
    value["event"]["payload"]["data"]["action"]["claim"]["claim_id"]
        .as_str()
        .unwrap()
        .into()
}

#[test]
fn independent_processes_share_claims_recovery_and_integration_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let init = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let mut secrets = Vec::new();
    for id in ["a", "b", "c"] {
        op(
            root,
            "worker.register",
            json!({"worker_id":id}),
            &format!("register-{id}"),
        );
        let secret = route_basic::principal::generate_credential().unwrap();
        op(
            root,
            "worker.binding.issue",
            json!({"worker_id":id,"credential_hash":route_core::sha256_hex(secret.as_bytes())}),
            &format!("bind-{id}"),
        );
        secrets.push(secret);
    }
    let intent = op(
        root,
        "intent.create",
        json!({"objective":"Bounded Route self-maintenance transport and safety verification"}),
        "intent",
    )["intent_ref"]
        .as_str()
        .unwrap()
        .to_string();
    let a = &secrets[0];
    let b = &secrets[1];
    let c = &secrets[2];
    let child_params = json!({"intent_ref":intent,"title":"Verify Route work transport","kind":"TEST_GAP","scope_paths":["tool/route/crates/route-cli/tests/autonomy_e2e.rs"],"verification_requirements":["route task exec"],"overlap_mode":"EXCLUSIVE"});
    let work = child_id(&worker(
        root,
        "work.create_child",
        child_params.clone(),
        "work-1",
        a,
    ));
    let replay = worker(root, "work.create_child", child_params.clone(), "work-1", a);
    assert_eq!(child_id(&replay), work);
    let mut changed = child_params;
    changed["title"] = json!("Changed operation under same key");
    assert_eq!(
        rpc(
            root,
            "work.create_child",
            changed,
            Some("work-1"),
            Some(a),
            false
        )["ok"],
        false
    );
    let initial = ok(rpc(root, "work.available", json!({}), None, Some(b), false));
    assert_eq!(initial["available"][0]["state"], "READY");
    let a_claim = claim_id(&worker(
        root,
        "work.claim",
        json!({"work_id":work}),
        "claim-a",
        a,
    ));
    let conflict = rpc(
        root,
        "work.claim",
        json!({"work_id":work}),
        Some("claim-b-too-early"),
        Some(b),
        false,
    );
    assert_eq!(conflict["ok"], false);
    let spoof = rpc(
        root,
        "work.release",
        json!({"claim_id":a_claim,"reason":"spoof"}),
        Some("spoof-release"),
        Some(c),
        false,
    );
    assert_eq!(spoof["ok"], false);
    worker(
        root,
        "worker.message.send",
        json!({"target_worker":"b","intent_ref":intent,"message_type":"HELP_REQUEST","content":"Need independent transport review"}),
        "help-request",
        a,
    );
    let delta = ok(rpc(
        root,
        "development.events.query",
        json!({"after_revision":initial["global_revision"],"limit":100}),
        None,
        Some(b),
        false,
    ));
    assert!(delta["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["payload"]["kind"] == "WORKER_MESSAGE"));
    worker(
        root,
        "worker.message.send",
        json!({"target_worker":"a","intent_ref":intent,"message_type":"HELP_OFFER","content":"I can review after recovery"}),
        "help-offer",
        b,
    );
    op(
        root,
        "work.interrupt",
        json!({"claim_id":a_claim,"reason":"host observed worker process exit"}),
        "interrupt-a",
    );
    assert_eq!(
        ok(rpc(root, "work.available", json!({}), None, Some(b), false))["available"][0]["state"],
        "REASSIGNABLE"
    );
    let b_claim = claim_id(&worker(
        root,
        "work.claim",
        json!({"work_id":work,"resumes_claim_id":a_claim}),
        "claim-b",
        b,
    ));
    worker(
        root,
        "worker.message.send",
        json!({"target_worker":"c","intent_ref":intent,"message_type":"REVIEW_REQUEST","content":"Review the continuing claim"}),
        "review-request",
        b,
    );
    worker(
        root,
        "worker.message.send",
        json!({"target_worker":"b","intent_ref":intent,"message_type":"DISAGREEMENT","content":"Prefer process-level check over mock-only check"}),
        "disagreement",
        c,
    );
    worker(
        root,
        "worker.message.send",
        json!({"target_worker":"b","intent_ref":intent,"message_type":"REVIEW_FINDING","content":"Process-level check is independently observable"}),
        "review-finding",
        c,
    );
    worker(
        root,
        "work.finish",
        json!({"claim_id":b_claim,"reason":"verified candidate"}),
        "finish-b",
        b,
    );
    let before = rpc(
        root,
        "intent.close",
        json!({"intent_ref":intent,"result":"success"}),
        Some("close-before-integration"),
        None,
        true,
    );
    assert_eq!(before["ok"], false);
    let view = ok(rpc(root, "work.available", json!({}), None, Some(c), false));
    assert_eq!(view["available"][0]["claims"].as_array().unwrap().len(), 2);
    let claims = view["available"][0]["claims"].as_array().unwrap();
    assert_eq!(
        claims.iter().find(|c| c["claim_id"] == a_claim).unwrap()["state"],
        "INTERRUPTED"
    );
    assert_eq!(
        claims.iter().find(|c| c["claim_id"] == b_claim).unwrap()["state"],
        "COMPLETED"
    );
    let store = route_basic::EvidenceStore::load(root).unwrap();
    assert!(!store
        .evidence
        .iter()
        .any(|e| e.source == route_basic::EvidenceSource::System
            && e.session_id == intent
            && matches!(
                e.kind,
                route_basic::EvidenceKind::CheckPass | route_basic::EvidenceKind::TestPass
            )));
    let page = ok(rpc(
        root,
        "development.events.query",
        json!({"after_revision":0,"limit":100}),
        None,
        Some(a),
        false,
    ));
    assert!(page["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["event_type"] == "WORK"));
    assert!(!page.to_string().contains(a));
    assert!(!page.to_string().contains(b));
    assert!(!page.to_string().contains(c));
    let route_binary = env!("CARGO_BIN_EXE_route");
    let check = Command::new(route_binary)
        .args([
            "task",
            "exec",
            &intent,
            "--check-id",
            "route task exec",
            "--",
            route_binary,
            "--version",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    let evidence = route_basic::EvidenceStore::load(root).unwrap();
    let check_ref = evidence
        .evidence
        .iter()
        .find(|e| {
            e.session_id == intent
                && e.source == route_basic::EvidenceSource::System
                && e.kind == route_basic::EvidenceKind::CheckPass
        })
        .unwrap()
        .id
        .clone();
    let state_hash = route_basic::compute_state_hash(root).unwrap();
    let revision = ok(rpc(root, "work.available", json!({}), None, Some(a), false))
        ["global_revision"]
        .as_u64()
        .unwrap();
    op(
        root,
        "work.integrate",
        json!({"intent_ref":intent,"accepted_claim_ids":[b_claim],"evidence_refs":[check_ref],"state_hash":state_hash,"expected_revision":revision}),
        "integration",
    );
    let closed = op(
        root,
        "intent.close",
        json!({"intent_ref":intent,"result":"success"}),
        "close-after-integration",
    );
    assert_eq!(closed["intent_ref"], intent);
}
