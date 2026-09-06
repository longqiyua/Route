use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use serde_json::{json, Value};
use tempfile::tempdir;

fn request(id: &str, method: &str, params: Value, key: Option<&str>) -> String {
    json!({
        "protocol": "route/1",
        "request_id": id,
        "method": method,
        "context": {},
        "params": params,
        "idempotency_key": key,
    })
    .to_string()
}

fn spawn_rpc(root: &Path, input: &str) -> Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("rpc")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn route rpc");
    child
        .stdin
        .take()
        .expect("rpc stdin")
        .write_all(input.as_bytes())
        .expect("write rpc request");
    child
}

fn finish_rpc(child: Child) -> Value {
    let output = child.wait_with_output().expect("wait for route rpc");
    assert!(
        output.status.success(),
        "route rpc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid route/1 response")
}

fn rpc(root: &Path, input: String) -> Value {
    finish_rpc(spawn_rpc(root, &input))
}

#[test]
fn cooperation_generic_rpc_evidence_gate_and_pending_replay() {
    use route_basic::cooperation::*;
    use route_basic::execution::{EvidenceKind, EvidenceSource, EvidenceStore};
    let project = tempdir().unwrap();
    let root = project.path();
    std::fs::write(root.join("fixture.txt"), b"safe fixture").unwrap();
    register_cooperation_resource(
        root,
        "resource".into(),
        "fixture.txt".into(),
        CooperationKind::new("DOCUMENT").unwrap(),
        "fixture".into(),
        None,
        vec![],
        vec![],
        vec![],
        None,
        Some("resource".into()),
    )
    .unwrap();
    let resource = cooperation_resource(root, "resource").unwrap().unwrap();
    let fingerprint = resource.fingerprint.unwrap().value;
    let mut record = CooperationKnowledgeRecord {
        knowledge_id: "observed".into(),
        cooperation_id: "resource".into(),
        statement: "Fixture check passed".into(),
        capability_refs: vec!["fixture.read".into()],
        epistemic_status: EpistemicStatus::Observed,
        provenance: "test system".into(),
        source_refs: vec![],
        evidence_refs: vec!["message-is-not-evidence".into()],
        observed_at: 1,
        resource_fingerprint: Some(fingerprint.clone()),
        supersedes: None,
    };
    let rejected = rpc(
        root,
        request(
            "bad",
            "development.event.record",
            json!({"payload": route_basic::DevelopmentEventPayload::CooperationKnowledgeRecorded { record: record.clone() }}),
            Some("bad"),
        ),
    );
    assert_eq!(rejected["error"]["code"], "DEVELOPMENT_EVENT_REJECTED");
    assert!(cooperation_knowledge(root, None).unwrap().is_empty());
    let mut evidence = EvidenceStore::load(root).unwrap();
    record.evidence_refs = vec![evidence.record(
        "fixture",
        EvidenceKind::CheckPass,
        EvidenceSource::System,
        "fixture-hash".into(),
        std::collections::HashMap::from([
            ("project_id".into(), resource.project_id),
            ("cooperation_id".into(), "resource".into()),
            ("resource_fingerprint".into(), fingerprint),
        ]),
    )];
    evidence.save(root).unwrap();
    let params = json!({"payload": route_basic::DevelopmentEventPayload::CooperationKnowledgeRecorded { record }});
    let first = rpc(
        root,
        request(
            "first",
            "development.event.record",
            params.clone(),
            Some("valid"),
        ),
    );
    assert_eq!(first["ok"], true, "{first}");
    let receipts = root.join(".route/rpc-idempotency.json");
    let mut stored: Value = serde_json::from_slice(&std::fs::read(&receipts).unwrap()).unwrap();
    stored["route/1:development.event.record:valid"]["status"] = json!("PENDING");
    stored["route/1:development.event.record:valid"]
        .as_object_mut()
        .unwrap()
        .remove("response");
    std::fs::write(&receipts, serde_json::to_vec(&stored).unwrap()).unwrap();
    let replay = rpc(
        root,
        request("retry", "development.event.record", params, Some("valid")),
    );
    assert_eq!(replay["ok"], true, "{replay}");
    assert_eq!(replay["result"]["replay"], true);
    assert_eq!(
        first["result"]["event"]["event_id"],
        replay["result"]["event"]["event_id"]
    );
    assert_eq!(cooperation_knowledge(root, None).unwrap().len(), 1);
}

#[test]
fn legacy_reference_cli_preserves_corrupt_registry() {
    let project = tempdir().unwrap();
    let path = project.path().join(".route/reference/registry.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    for bytes in ["", "{", "not-json"] {
        std::fs::write(&path, bytes).unwrap();
        for command in ["list", "show"] {
            let output = Command::new(env!("CARGO_BIN_EXE_route"))
                .args(["reference", command])
                .current_dir(project.path())
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("registry"),
                "must reach the registry error, not fail argument parsing"
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        }
    }
}

#[test]
fn legacy_reference_reads_on_fresh_project_create_no_state() {
    let project = tempdir().unwrap();
    for command in ["list", "show"] {
        let output = Command::new(env!("CARGO_BIN_EXE_route"))
            .args(["reference", command])
            .current_dir(project.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(std::fs::read_dir(project.path()).unwrap().count(), 0);
    }
}

#[test]
fn independent_process_supersession_has_exactly_one_winner() {
    use route_basic::cooperation::*;
    let project = tempdir().unwrap();
    let root = project.path();
    register_cooperation_resource(
        root,
        "resource".into(),
        "opaque:fixture".into(),
        CooperationKind::new("UNKNOWN").unwrap(),
        "fixture".into(),
        None,
        vec![],
        vec![],
        vec![],
        None,
        Some("resource".into()),
    )
    .unwrap();
    let original = CooperationKnowledgeRecord {
        knowledge_id: "old".into(),
        cooperation_id: "resource".into(),
        statement: "Declared fixture".into(),
        capability_refs: vec![],
        epistemic_status: EpistemicStatus::Declared,
        provenance: "fixture".into(),
        source_refs: vec![],
        evidence_refs: vec![],
        observed_at: 1,
        resource_fingerprint: None,
        supersedes: None,
    };
    record_cooperation_knowledge(root, original.clone(), None, Some("old".into())).unwrap();
    let children: Vec<_> = (0..2).map(|i| {
        let mut record = original.clone();
        record.knowledge_id = format!("new-{i}");
        record.supersedes = Some("old".into());
        let body = request(&format!("request-{i}"), "development.event.record", json!({"payload":route_basic::DevelopmentEventPayload::CooperationKnowledgeRecorded { record }}), Some(&format!("key-{i}")));
        spawn_rpc(root, &body)
    }).collect();
    let replies: Vec<_> = children.into_iter().map(finish_rpc).collect();
    assert_eq!(
        replies.iter().filter(|reply| reply["ok"] == true).count(),
        1,
        "{replies:?}"
    );
    assert_eq!(
        replies
            .iter()
            .filter(|reply| reply["error"]["code"] == "DEVELOPMENT_EVENT_REJECTED")
            .count(),
        1
    );
    assert_eq!(cooperation_knowledge(root, None).unwrap().len(), 2);
}

#[test]
fn independent_processes_exchange_revisions_messages_and_replay_receipts() {
    let project = tempdir().unwrap();
    route_basic::ensure_identity(project.path()).unwrap();

    let first = rpc(
        project.path(),
        request(
            "register-a",
            "worker.register",
            json!({"worker_id":"worker-a","metadata":{"provider":"host-a","model":"model-a"}}),
            Some("register-a"),
        ),
    );
    assert_eq!(first["ok"], true);
    assert_eq!(first["result"]["global_revision"], 1);

    let second = rpc(
        project.path(),
        request(
            "register-b",
            "worker.register",
            json!({"worker_id":"worker-b","metadata":{"provider":"host-b","model":"model-b"}}),
            Some("register-b"),
        ),
    );
    assert_eq!(second["result"]["global_revision"], 2);

    let message = rpc(
        project.path(),
        request(
            "message-b-a",
            "worker.message.send",
            json!({
                "worker_id":"worker-b",
                "target_worker":"worker-a",
                "message_type":"QUESTION",
                "content":"Can you verify the shared revision?",
                "requires_response":true
            }),
            Some("message-b-a"),
        ),
    );
    assert_eq!(message["result"]["global_revision"], 3);

    let observed_by_a = rpc(
        project.path(),
        request(
            "query-a",
            "development.events.query",
            json!({"after_revision":1,"limit":20}),
            None,
        ),
    );
    assert_eq!(observed_by_a["result"]["global_revision"], 3);
    assert_eq!(
        observed_by_a["result"]["events"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        observed_by_a["result"]["events"][1]["payload"]["kind"],
        "WORKER_MESSAGE"
    );

    let presence = rpc(
        project.path(),
        request(
            "presence-a",
            "worker.presence.update",
            json!({
                "worker_id":"worker-a",
                "status":"ACTIVE",
                "current_activity_summary":"reviewing worker-b message",
                "observed_global_revision":3
            }),
            Some("presence-a"),
        ),
    );
    assert_eq!(presence["result"]["global_revision"], 4);

    let observed_by_b = rpc(
        project.path(),
        request(
            "query-b",
            "development.events.query",
            json!({"after_revision":3,"limit":20}),
            None,
        ),
    );
    assert_eq!(observed_by_b["result"]["global_revision"], 4);
    assert_eq!(
        observed_by_b["result"]["events"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        observed_by_b["result"]["events"][0]["payload"]["kind"],
        "WORKER_PRESENCE_UPDATED"
    );

    let replay = rpc(
        project.path(),
        request(
            "presence-a-retry",
            "worker.presence.update",
            json!({
                "worker_id":"worker-a",
                "status":"ACTIVE",
                "current_activity_summary":"reviewing worker-b message",
                "observed_global_revision":3
            }),
            Some("presence-a"),
        ),
    );
    assert_eq!(replay["ok"], true);
    assert_eq!(replay["receipt"]["replay"], true);

    let state = rpc(
        project.path(),
        request(
            "state-b",
            "development.state",
            json!({"seen_revision":2,"recent_limit":20}),
            None,
        ),
    );
    assert_eq!(state["result"]["state"]["global_revision"], 4);
    assert_eq!(state["result"]["stale"], true);
    assert_eq!(state["result"]["state"]["latest_evidence"], json!([]));
}

#[test]
fn concurrent_processes_append_once_in_one_total_order() {
    let project = tempdir().unwrap();
    route_basic::ensure_identity(project.path()).unwrap();

    let children = (0..8)
        .map(|index| {
            spawn_rpc(
                project.path(),
                &request(
                    &format!("register-{index}"),
                    "worker.register",
                    json!({"worker_id":format!("worker-{index}"),"metadata":{"host":format!("host-{index}")}}),
                    Some(&format!("register-{index}")),
                ),
            )
        })
        .collect::<Vec<_>>();
    for child in children {
        assert_eq!(finish_rpc(child)["ok"], true);
    }

    let events = rpc(
        project.path(),
        request(
            "all-events",
            "development.events.query",
            json!({"after_revision":0,"limit":20}),
            None,
        ),
    );
    assert_eq!(events["result"]["global_revision"], 8);
    for (index, event) in events["result"]["events"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(event["sequence"], (index + 1) as u64);
    }
}

#[test]
fn attached_workspace_observes_owner_ledger_and_corruption_fails_closed() {
    let owner = tempdir().unwrap();
    let attached = tempdir().unwrap();
    route_basic::ensure_identity(owner.path()).unwrap();
    route_basic::attach_project_identity(attached.path(), owner.path()).unwrap();

    let registered = rpc(
        attached.path(),
        request(
            "attached-worker",
            "worker.register",
            json!({"worker_id":"attached-worker","metadata":{}}),
            Some("attached-worker"),
        ),
    );
    assert_eq!(registered["ok"], true);

    let owner_view = rpc(
        owner.path(),
        request(
            "owner-view",
            "development.events.query",
            json!({"after_revision":0,"limit":10}),
            None,
        ),
    );
    assert_eq!(owner_view["result"]["global_revision"], 1);
    assert_ne!(
        route_basic::ensure_identity(owner.path())
            .unwrap()
            .workspace_id,
        route_basic::ensure_identity(attached.path())
            .unwrap()
            .workspace_id
    );

    std::fs::write(
        owner.path().join(".route/development/ledger.json"),
        b"{not-json",
    )
    .unwrap();
    let rejected = rpc(
        owner.path(),
        request(
            "corrupt-view",
            "development.events.query",
            json!({"after_revision":0,"limit":10}),
            None,
        ),
    );
    assert_eq!(rejected["ok"], false);
    assert_eq!(rejected["error"]["code"], "DEVELOPMENT_EVENTS_UNAVAILABLE");
}
