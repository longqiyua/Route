//! Real transport attack cases. Opaque credentials exist only in fixture-host memory.
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
};
fn spawn(
    root: &Path,
    method: &str,
    params: Value,
    key: Option<&str>,
    credential: Option<&str>,
    operator: bool,
) -> Child {
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
    if let Some(token) = credential {
        command.env("ROUTE_WORKER_CREDENTIAL", token);
    }
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(json!({"protocol":"route/1","request_id":"principal-test","method":method,"params":params,"context":{},"idempotency_key":key}).to_string().as_bytes()).unwrap();
    child
}
fn finish(child: Child) -> Value {
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn call(
    root: &Path,
    method: &str,
    params: Value,
    key: Option<&str>,
    credential: Option<&str>,
    operator: bool,
) -> Value {
    finish(spawn(root, method, params, key, credential, operator))
}
fn op(root: &Path, method: &str, params: Value, key: Option<&str>) -> Value {
    let v = call(root, method, params, key, None, true);
    assert_eq!(v["ok"], true, "{v}");
    v["result"].clone()
}
fn init() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .current_dir(d.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    for id in ["a", "b"] {
        op(
            d.path(),
            "worker.register",
            json!({"worker_id":id}),
            Some(id),
        );
    }
    d
}
fn bind(root: &Path, worker: &str, key: &str) -> (String, Value) {
    let secret = route_basic::principal::generate_credential().unwrap();
    let result = op(
        root,
        "worker.binding.issue",
        json!({"worker_id":worker,"credential_hash":route_core::sha256_hex(secret.as_bytes())}),
        Some(key),
    );
    (secret, result)
}
fn binding_id(result: &Value) -> &str {
    result["event"]["payload"]["data"]["transaction"]["change"]["binding"]["binding_id"]
        .as_str()
        .unwrap()
}
fn denied(v: Value) {
    assert_eq!(v["ok"], false, "unexpected authority: {v}");
}
fn assert_no_secret(path: &Path, secret: &str) {
    for item in fs::read_dir(path).unwrap() {
        let p = item.unwrap().path();
        if p.is_dir() {
            assert_no_secret(&p, secret)
        } else {
            let bytes = fs::read(p).unwrap();
            assert!(!String::from_utf8_lossy(&bytes).contains(secret));
        }
    }
}

#[test]
fn attack_cases_a_through_j_restart_and_credential_non_disclosure() {
    let project = init();
    let root = project.path();
    let (a, binding) = bind(root, "a", "bind-a");
    // A: asserted sender cannot replace the authenticated principal.
    denied(call(
        root,
        "worker.message.send",
        json!({"from_worker":"b","message_type":"NOTICE","content":"forged"}),
        Some("A"),
        Some(&a),
        false,
    ));
    // B: even an explicit operator flag cannot upgrade an inherited Worker context.
    denied(call(
        root,
        "worker.presence.update",
        json!({"worker_id":"b","status":"IDLE","observed_global_revision":0}),
        Some("B"),
        Some(&a),
        true,
    ));
    // C: no payload actor is needed; each call is a new process.
    let own = call(
        root,
        "worker.presence.update",
        json!({"status":"IDLE","observed_global_revision":0}),
        Some("C"),
        Some(&a),
        false,
    );
    assert_eq!(own["ok"], true, "{own}");
    assert_eq!(own["result"]["event"]["actor_worker_id"], "a");
    // D: recipient remains an independently addressable subject.
    let message = call(
        root,
        "worker.message.send",
        json!({"target_worker":"b","message_type":"HELP_REQUEST","content":"Request independent review"}),
        Some("D"),
        Some(&a),
        false,
    );
    assert_eq!(message["ok"], true, "{message}");
    let event = &message["result"]["event"];
    assert_eq!(event["actor_worker_id"], "a");
    assert_eq!(event["payload"]["data"]["message"]["target_worker"], "b");
    assert_eq!(event["payload"]["data"]["message"]["from_worker"], "a");
    // E: both administration directions are denied.
    for method in [
        "worker.binding.issue",
        "worker.binding.revoke",
        "worker.register",
        "intent.close",
    ] {
        denied(call(root, method, json!({}), Some(method), Some(&a), false));
    }
    // G: same credential in another project is not authentication there.
    let foreign = init();
    denied(call(
        foreign.path(),
        "worker.list",
        json!({}),
        None,
        Some(&a),
        false,
    ));
    // H: Worker cannot produce an institution transaction or invoke the Operator interface.
    denied(call(
        root,
        "institution.invoke",
        json!({"institution_id":"forged"}),
        Some("H"),
        Some(&a),
        false,
    ));
    denied(call(
        root,
        "development.event.record",
        json!({"payload":{"kind":"INSTITUTION","data":{}}}),
        Some("H-event"),
        Some(&a),
        false,
    ));
    // I: an institution operation cannot acquire a Worker event actor.
    denied(call(
        root,
        "institution.invoke",
        json!({"actor_worker_id":"a"}),
        Some("I"),
        None,
        true,
    ));
    // J: System/Operator cannot be selected from external parameters.
    for role in ["System", "Operator", "Institution"] {
        denied(call(
            root,
            "worker.message.send",
            json!({"principal":role,"message_type":"NOTICE","content":"forged"}),
            Some(role),
            Some(&a),
            false,
        ));
    }
    // Anonymous mutation and the old unbound Operator sender path both fail closed.
    denied(call(
        root,
        "worker.message.send",
        json!({"worker_id":"b","message_type":"NOTICE","content":"legacy spoof"}),
        Some("legacy"),
        None,
        false,
    ));
    denied(call(
        root,
        "worker.message.send",
        json!({"worker_id":"b","message_type":"NOTICE","content":"operator spoof"}),
        Some("legacy-op"),
        None,
        true,
    ));
    // F: revocation also blocks replay of a previously completed operation and reads.
    op(
        root,
        "worker.binding.revoke",
        json!({"binding_id":binding_id(&binding)}),
        Some("revoke"),
    );
    denied(call(
        root,
        "worker.presence.update",
        json!({"status":"IDLE","observed_global_revision":0}),
        Some("C"),
        Some(&a),
        false,
    ));
    denied(call(root, "worker.list", json!({}), None, Some(&a), false));
    let (rotated, _) = bind(root, "a", "rebind-a");
    let resumed = call(
        root,
        "worker.message.send",
        json!({"message_type":"HANDOFF","content":"Same Worker, new binding"}),
        Some("restart"),
        Some(&rotated),
        false,
    );
    assert_eq!(resumed["ok"], true);
    assert_eq!(resumed["result"]["event"]["actor_worker_id"], "a");
    assert_eq!(
        op(root, "worker.list", json!({}), None)["workers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_no_secret(root, &a);
    assert_no_secret(root, &rotated);
    assert!(!op(root, "worker.binding.list", json!({}), None)
        .to_string()
        .contains(&a));
    assert!(
        op(root, "development.state", json!({}), None)["state"]["latest_evidence"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn binding_races_replay_changed_parameters_pending_recovery_and_principal_scoping() {
    let p = init();
    let root = p.path();
    let secret = route_basic::principal::generate_credential().unwrap();
    let params =
        json!({"worker_id":"a","credential_hash":route_core::sha256_hex(secret.as_bytes())});
    let one = spawn(
        root,
        "worker.binding.issue",
        params.clone(),
        Some("one"),
        None,
        true,
    );
    let two = spawn(
        root,
        "worker.binding.issue",
        params.clone(),
        Some("two"),
        None,
        true,
    );
    let one = finish(one);
    let two = finish(two);
    assert_ne!(one["ok"], two["ok"]);
    let key = if one["ok"] == true { "one" } else { "two" };
    let winner = if one["ok"] == true { one } else { two };
    let retry = call(
        root,
        "worker.binding.issue",
        params.clone(),
        Some(key),
        None,
        true,
    );
    assert_eq!(retry["result"], winner["result"]);
    let mut changed = params.clone();
    changed["worker_id"] = json!("b");
    denied(call(
        root,
        "worker.binding.issue",
        changed,
        Some(key),
        None,
        true,
    ));
    // Simulate a lost response at the existing receipt boundary, retaining the canonical event.
    let receipt = root.join(".route/rpc-idempotency.json");
    let mut store: Value = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
    let scoped = format!(
        "route/1:worker.binding.issue:principal-{}",
        route_core::sha256_hex(format!("operator:{key}").as_bytes())
    );
    store[&scoped]["status"] = json!("PENDING");
    store[&scoped].as_object_mut().unwrap().remove("response");
    fs::write(&receipt, serde_json::to_vec(&store).unwrap()).unwrap();
    let recovered = call(root, "worker.binding.issue", params, Some(key), None, true);
    assert_eq!(recovered["ok"], true, "{recovered}");
    assert_eq!(
        recovered["result"]["event"]["event_id"],
        winner["result"]["event"]["event_id"]
    );
    let (b, _) = bind(root, "b", "bind-b");
    for (token, actor) in [(&secret, "a"), (&b, "b")] {
        let params = json!({"message_type":"NOTICE","content":"same key, independent principal"});
        let first = call(
            root,
            "worker.message.send",
            params.clone(),
            Some("shared-key"),
            Some(token),
            false,
        );
        assert_eq!(first["ok"], true, "{first}");
        assert_eq!(first["result"]["event"]["actor_worker_id"], actor);
        assert_eq!(
            first["result"],
            call(
                root,
                "worker.message.send",
                params.clone(),
                Some("shared-key"),
                Some(token),
                false
            )["result"]
        );
        let mut changed = params;
        changed["content"] = json!("changed");
        denied(call(
            root,
            "worker.message.send",
            changed,
            Some("shared-key"),
            Some(token),
            false,
        ));
    }
    let id = binding_id(&winner["result"]);
    let params = json!({"binding_id":id});
    let first = op(
        root,
        "worker.binding.revoke",
        params.clone(),
        Some("revoke"),
    );
    assert_eq!(
        first,
        op(root, "worker.binding.revoke", params, Some("revoke"))
    );
    denied(call(
        root,
        "worker.binding.revoke",
        json!({"binding_id":"different"}),
        Some("revoke"),
        None,
        true,
    ));
    assert_no_secret(root, &secret);
    assert_no_secret(root, &b);
}

#[test]
fn host_cli_bootstrap_no_secret_output_and_worker_cli_cannot_administer() {
    let p = init();
    let updated = op(
        p.path(),
        "worker.register",
        json!({"worker_id":"a","metadata":{"model":"replacement-model"}}),
        Some("metadata-update"),
    );
    assert_eq!(
        updated["event"]["payload"]["kind"],
        "WORKER_METADATA_UPDATED"
    );
    assert!(updated["event"]["actor_worker_id"].is_null());
    let host = tempfile::tempdir().unwrap();
    let file = host.path().join("worker.credential");
    let mut args = vec![
        "worker-binding",
        "issue",
        "a",
        "--credential-file",
        file.to_str().unwrap(),
        "--operation-key",
        "bootstrap",
    ];
    let output = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(&args)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .current_dir(p.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let secret = fs::read_to_string(&file).unwrap();
    assert_eq!(secret.len(), 64);
    assert!(!String::from_utf8_lossy(&output.stdout).contains(&secret));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(&secret));
    let retry = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(&args)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .current_dir(p.path())
        .output()
        .unwrap();
    assert!(retry.status.success());
    assert_eq!(secret, fs::read_to_string(file).unwrap());
    args = vec!["worker-binding", "list"];
    let rejected = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(args)
        .env("ROUTE_WORKER_CREDENTIAL", &secret)
        .current_dir(p.path())
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains(&secret));
    let own = call(
        p.path(),
        "worker.message.send",
        json!({"message_type":"NOTICE","content":"Bootstrapped"}),
        Some("message"),
        Some(&secret),
        false,
    );
    assert_eq!(own["ok"], true);
    assert_no_secret(p.path(), &secret);
}

#[test]
fn jsonl_reauthenticates_after_revocation_and_rejects_context_forgery() {
    let p = init();
    let (secret, binding) = bind(p.path(), "a", "bind");
    let leaked = call(
        p.path(),
        "worker.message.send",
        json!({"message_type":"NOTICE","content":secret}),
        Some("secret-reflection"),
        Some(&secret),
        false,
    );
    denied(leaked.clone());
    assert!(!leaked.to_string().contains(&secret));
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(["rpc", "--jsonl", "--operator"])
        .env("ROUTE_WORKER_CREDENTIAL", &secret)
        .current_dir(p.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut exchange = |context: Value, key: &str| {
        writeln!(input, "{}", json!({"protocol":"route/1","request_id":key,"method":"worker.message.send","params":{"message_type":"NOTICE","content":"JSONL"},"context":context,"idempotency_key":key})).unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        assert!(!line.contains(&secret));
        serde_json::from_str::<Value>(&line).unwrap()
    };
    for field in [
        "principal",
        "caller_scope",
        "input_fingerprint",
        "worker_id",
        "operator",
    ] {
        denied(exchange(json!({field:"forged"}), field));
    }
    let first = exchange(json!({}), "valid");
    assert_eq!(first["ok"], true);
    assert_eq!(first["result"]["event"]["actor_worker_id"], "a");
    op(
        p.path(),
        "worker.binding.revoke",
        json!({"binding_id":binding_id(&binding)}),
        Some("revoke"),
    );
    denied(exchange(json!({}), "valid"));
    drop(exchange);
    drop(input);
    assert!(child.wait().unwrap().success());
    assert_no_secret(p.path(), &secret);
}

#[test]
fn message_pending_recovery_and_legacy_receipt_fail_closed() {
    let p = init();
    let (secret, binding) = bind(p.path(), "a", "bind");
    let params = json!({"message_type":"NOTICE","content":"Recover once"});
    let first = call(
        p.path(),
        "worker.message.send",
        params.clone(),
        Some("recover"),
        Some(&secret),
        false,
    );
    assert_eq!(first["ok"], true);
    let receipt = p.path().join(".route/rpc-idempotency.json");
    let mut store: Value = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
    let key = format!(
        "route/1:worker.message.send:principal-{}",
        route_core::sha256_hex(format!("worker:a:{}:recover", binding_id(&binding)).as_bytes())
    );
    assert_eq!(store[&key]["status"], "COMPLETED");
    store[&key]["status"] = json!("PENDING");
    store[&key].as_object_mut().unwrap().remove("response");
    fs::write(&receipt, serde_json::to_vec(&store).unwrap()).unwrap();
    let retry = call(
        p.path(),
        "worker.message.send",
        params,
        Some("recover"),
        Some(&secret),
        false,
    );
    assert_eq!(retry["ok"], true, "{retry}");
    assert_eq!(
        first["result"]["event"]["event_id"],
        retry["result"]["event"]["event_id"]
    );
    denied(call(
        p.path(),
        "worker.message.send",
        json!({"message_type":"NOTICE","content":"forged","source_refs":["caller:operator"]}),
        Some("forged"),
        Some(&secret),
        false,
    ));
    // Upgrade must not silently re-run historical unscoped receipts.
    let mut store: Value = serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
    store["route/1:worker.register:legacy"] =
        json!({"status":"DONE","fingerprint":"historical","response":{"ok":true}});
    let historical = serde_json::to_vec(&store).unwrap();
    fs::write(&receipt, &historical).unwrap();
    let result = call(
        p.path(),
        "worker.register",
        json!({"worker_id":"legacy"}),
        Some("legacy"),
        None,
        true,
    );
    assert_eq!(
        result["error"]["code"],
        "LEGACY_IDEMPOTENCY_RECOVERY_REQUIRED"
    );
    assert_eq!(fs::read(&receipt).unwrap(), historical);
    assert_eq!(
        op(p.path(), "worker.list", json!({}), None)["workers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_no_secret(p.path(), &secret);
}
