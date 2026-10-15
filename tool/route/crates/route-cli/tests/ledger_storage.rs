//! Real Git/CLI acceptance of legacy cutover and independent ledger restore.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

fn cli(root: &Path, host: &Path, args: &[&str]) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(root)
        .args(args)
        .env("ROUTE_SIDECAR_HOME", host)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}
fn rpc(root: &Path, method: &str, params: Value, key: Option<&str>) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(root)
        .args(["rpc", "--operator"])
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"protocol":"route/1","request_id":method,"method":method,
        "params":params,"context":{},"idempotency_key":key});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["ok"], true, "{value}");
    value["result"].clone()
}
#[test]
fn git_attach_migrate_restart_backup_restore_preserves_goal_and_history() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    let host = temp.path().join("host");
    fs::create_dir(&root).unwrap();
    assert!(Command::new("git")
        .current_dir(&root)
        .args(["init", "-q"])
        .status()
        .unwrap()
        .success());
    fs::write(root.join("source.txt"), "isolated source fixture\n").unwrap();
    let identity = route_basic::ensure_identity(&root).unwrap();
    fs::create_dir_all(root.join(".route/development")).unwrap();
    fs::write(
        root.join(".route/development/ledger.json"),
        serde_json::to_vec(&json!({
            "schema":2,"project_id":identity.project_id,"head_hash":"0".repeat(64),"events":[]
        }))
        .unwrap(),
    )
    .unwrap();
    cli(&root, &host, &["attach"]);
    assert_eq!(cli(&root, &host, &["enter", "--json"])["code"], "READY");
    let revision = route_basic::global_development_revision(&root).unwrap();
    let created = rpc(
        &root,
        "goal.create",
        json!({"expected_revision":revision,
        "goal":{"title":"Storage acceptance","description":"Preserve actual local facts",
        "domain":"PLANNING","constraint_refs":[],"reference_refs":[]}}),
        Some("storage-goal"),
    );
    let goal = created["event"]["payload"]["data"]["action"]["change"]["goal"]["goal_id"]
        .as_str()
        .unwrap();
    cli(
        &root,
        &host,
        &["sidecar", "note", "Storage fact before migration"],
    );
    let events = route_basic::query_development_events(&root, 0, 1000).unwrap();
    let before = rpc(&root, "assistant.status", json!({"goal_id":goal}), None);
    let doorway = fs::read(root.join("ROUTE.md")).unwrap();
    let result = cli(&root, &host, &["storage", "migrate", "--json"]);
    assert_eq!(result["revision"], events.global_revision);
    assert_eq!(
        events,
        route_basic::query_development_events(&root, 0, 1000).unwrap()
    );
    assert_eq!(
        before,
        rpc(&root, "assistant.status", json!({"goal_id":goal}), None)
    );
    assert_eq!(cli(&root, &host, &["enter", "--json"])["code"], "READY");
    cli(
        &root,
        &host,
        &["sidecar", "note", "Storage fact after restart"],
    );
    let final_events = route_basic::query_development_events(&root, 0, 1000).unwrap();
    let backup = temp.path().join("backup");
    cli(
        &root,
        &host,
        &["storage", "backup", backup.to_str().unwrap()],
    );
    let restored = temp.path().join("restored");
    cli(
        &root,
        &host,
        &[
            "storage",
            "restore",
            backup.to_str().unwrap(),
            restored.to_str().unwrap(),
        ],
    );
    assert_eq!(
        final_events,
        route_basic::query_development_events(&restored, 0, 1000).unwrap()
    );
    assert_eq!(
        rpc(&root, "assistant.status", json!({"goal_id":goal}), None),
        rpc(&restored, "assistant.status", json!({"goal_id":goal}), None)
    );
    // Host credentials are intentionally outside the ledger backup. A restored
    // workspace has no implicit Worker authority; explicit attach provisions it.
    let restored_host = temp.path().join("restored-host");
    cli(&restored, &restored_host, &["enter", "--json"]);
    cli(&restored, &restored_host, &["attach"]);
    assert_eq!(
        cli(&restored, &restored_host, &["enter", "--json"])["code"],
        "READY"
    );
    assert_eq!(doorway, fs::read(root.join("ROUTE.md")).unwrap());
}

#[cfg(feature = "test-utils")]
#[test]
fn actual_cli_crash_and_operation_receipt_retry_preserve_original_event() {
    for point in [
        "before_append",
        "half_record",
        "before_durable_commit",
        "after_durable_commit",
    ] {
        let root = tempfile::tempdir().unwrap();
        route_basic::ensure_identity(root.path()).unwrap();
        let params = json!({"payload":{"kind":"FINDING","data":{"summary":"actual CLI crash fixture","source_refs":[]}}});
        let request = json!({"protocol":"route/1","request_id":"cli-crash","method":"development.event.record",
            "params":params,"context":{},"idempotency_key":"cli-crash-key"});
        let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
            .current_dir(root.path())
            .args(["rpc", "--operator"])
            .env_remove("ROUTE_WORKER_CREDENTIAL")
            .env("ROUTE_LEDGER_FAILPOINT", point)
            .env("ROUTE_LEDGER_FAILPOINT_CRASH", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.to_string().as_bytes())
            .unwrap();
        let crashed = child.wait_with_output().unwrap();
        assert_eq!(
            crashed.status.code(),
            Some(86),
            "{point}: {}",
            String::from_utf8_lossy(&crashed.stderr)
        );
        let recovered = rpc(
            root.path(),
            "development.event.record",
            params.clone(),
            Some("cli-crash-key"),
        );
        let retry = rpc(
            root.path(),
            "development.event.record",
            params,
            Some("cli-crash-key"),
        );
        assert_eq!(recovered["event"], retry["event"]);
        let events = route_basic::query_development_events(root.path(), 0, 100).unwrap();
        assert_eq!(events.global_revision, 1, "{point}");
        assert_eq!(events.events.len(), 1, "{point}");
    }
}
