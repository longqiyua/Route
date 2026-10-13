//! Native CLI verification and completion share the same acceptance boundary.
use std::process::Command;

#[test]
#[ignore = "subprocess fixture invoked by unreadable_post_command_state_is_failure_not_panic"]
fn corrupt_metadata_command_fixture() {
    // This runs only as the approved command in the parent's temporary project.
    std::fs::write(".route-basic/config.json", b"invalid-json").unwrap();
}

#[test]
fn unreadable_post_command_state_is_failure_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    route_basic::BasicRepository::init(root).unwrap();
    let session = route_basic::begin_session(
        root,
        "Post-command state failure",
        route_basic::ApplyTarget::Claude,
        false,
        None,
    )
    .unwrap();
    let executable = std::env::current_exe().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_route"))
        .current_dir(root)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .args([
            "task",
            "exec",
            &session.id,
            "--check-id",
            "post-state-fault",
            "--",
        ])
        .arg(executable)
        .args([
            "--exact",
            "corrupt_metadata_command_fixture",
            "--ignored",
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("UNKNOWN"), "{stdout}");
    assert!(stderr.contains("could not be read"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    let store = route_basic::EvidenceStore::load(root).unwrap();
    let evidence: Vec<_> = store
        .evidence
        .iter()
        .filter(|e| e.metadata.get("check_id").map(String::as_str) == Some("post-state-fault"))
        .collect();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].kind, route_basic::EvidenceKind::CheckFail);
    assert_eq!(
        evidence[0].metadata.get("exit_code").map(String::as_str),
        Some("0")
    );
    let error = evidence[0].metadata.get("state_hash_error").unwrap();
    assert!(!error.is_empty() && error.chars().count() <= 512);
    assert!(evidence[0].state_hash.is_none());
}

#[test]
fn real_cli_failure_cannot_reuse_old_pass_or_end_successfully() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    route_basic::BasicRepository::init(root).unwrap();
    let session = route_basic::begin_session(
        root,
        "Session CLI regression",
        route_basic::ApplyTarget::Claude,
        false,
        None,
    )
    .unwrap();
    let mut protocol = route_basic::constitutive::Protocol::read(root).unwrap();
    protocol.body.push_str(
        "\n<!-- route-verification-policy:\n  required_checks: [\"native-check\"]\n-->\n",
    );
    protocol.write(root).unwrap();
    let cli = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_route"))
            .current_dir(root)
            .env_remove("ROUTE_WORKER_CREDENTIAL")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let verify = || -> serde_json::Value {
        serde_json::from_slice(&cli(&["task", "verify", &session.id, "--json"])).unwrap()
    };
    assert_eq!(verify()["result"], "INCOMPLETE");
    for (code, expected) in [("0", "VERIFIED"), ("7", "FAILED")] {
        let script = format!("exit {code}");
        let mut args = vec![
            "task",
            "exec",
            &session.id,
            "--check-id",
            "native-check",
            "--",
        ];
        if cfg!(windows) {
            args.extend(["cmd.exe", "/d", "/c", &script]);
        } else {
            args.extend(["sh", "-c", &script]);
        }
        cli(&args);
        assert_eq!(verify()["result"], expected);
    }
    cli(&["task", "end", &session.id, "--result", "success"]);
    let sessions = route_basic::SessionStore::load(root).unwrap();
    assert_eq!(
        sessions.get(&session.id).unwrap().status,
        route_basic::SessionStatus::Failed
    );
}
