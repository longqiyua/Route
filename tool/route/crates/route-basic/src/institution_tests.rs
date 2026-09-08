use crate::{
    development::{self, DevelopmentEventDraft},
    institution::*,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(root: &Path, id: &str, version: &str) -> String {
    let path = root.join(format!("{id}-{version}.json"));
    fs::write(&path,serde_json::to_vec(&json!({"institution_id":id,"name":id,"version":version,"provenance":"fixture","implementation_kind":"DECLARATIVE_V1","supported_hooks":["ON_EVENT"],"requested_capabilities":["OBSERVE","COMMUNICATE"],"compatibility":["route/1"],"rules":[{"hook":"ON_EVENT","effects":[{"family":"MESSAGE","summary":"fixture observation"}]}]})).unwrap()).unwrap();
    path.to_string_lossy().into()
}
fn command(
    root: &Path,
    value: Value,
    key: &str,
) -> crate::development::AppendDevelopmentEventResult {
    execute(root, serde_json::from_value(value).unwrap(), key).unwrap()
}
fn init() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    crate::ensure_identity(d.path()).unwrap();
    d
}
fn activate(root: &Path, id: &str, version: &str, grants: Value, key: &str) -> u64 {
    let pid = crate::load_identity(root).unwrap().unwrap().project_id;
    let previous = bindings(root)
        .unwrap()
        .into_iter()
        .find(|b| b.institution_id == id)
        .map(|b| b.activation_revision)
        .unwrap_or(0);
    command(root,json!({"operation":"activate","institution_id":id,"version":version,"project_id":pid,"grants":grants,"activated_by":"operator","expected_binding_revision":previous}),key).global_revision
}
fn event(root: &Path) -> crate::development::AppendDevelopmentEventResult {
    development::append_development_event(
        root,
        serde_json::from_value::<DevelopmentEventDraft>(
            json!({"payload":{"kind":"FINDING","data":{"summary":"real finding"}}}),
        )
        .unwrap(),
    )
    .unwrap()
}
fn invoke(root: &Path) -> Value {
    let trigger = event(root);
    serde_json::to_value(command(root,json!({"operation":"invoke","hook":"ON_EVENT","triggering_event_ref":trigger.event.event_id,"expected_revision":trigger.global_revision,"correlation_id":"fixture"}),&format!("invoke-{}",trigger.global_revision))).unwrap()["event"]["payload"]["data"]["transaction"]["change"]["report"].clone()
}

#[test]
fn immutable_registration_restart_idempotency_and_fingerprint() {
    let d = init();
    let root = d.path();
    let source = fixture(root, "custom", "1");
    let params = json!({"operation":"register","source_locator":source});
    let first = command(root, params.clone(), "reg");
    let repeated = command(root, params.clone(), "reg");
    assert!(repeated.replay);
    assert_eq!(first.event, repeated.event);
    let before = versions(root).unwrap();
    assert_eq!(before.len(), 1);
    assert!(bindings(root).unwrap().is_empty());
    let new_source = fixture(root, "custom", "2");
    assert!(execute(
        root,
        serde_json::from_value(json!({"operation":"register","source_locator":new_source}))
            .unwrap(),
        "reg"
    )
    .unwrap_err()
    .to_string()
    .contains("IDEMPOTENCY_CONFLICT"));
    assert!(execute(
        root,
        serde_json::from_value(params).unwrap(),
        "different-key"
    )
    .is_err());
    fs::write(&source, b"{}").unwrap();
    assert_eq!(versions(root).unwrap(), before);
    let pid = crate::load_identity(root).unwrap().unwrap().project_id;
    assert!(execute(root,serde_json::from_value(json!({"operation":"activate","institution_id":"custom","version":"1","project_id":pid,"grants":[],"activated_by":"operator","expected_binding_revision":0})).unwrap(),"bad-source").is_err());
}

#[test]
fn requested_capability_is_not_authority_and_effects_are_not_evidence() {
    let d = init();
    let root = d.path();
    let source = fixture(root, "custom", "1");
    command(
        root,
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    activate(root, "custom", "1", json!(["OBSERVE"]), "act");
    let denied = invoke(root);
    assert_eq!(denied["invocations"][0]["effects"][0]["status"], "DENIED");
    activate(
        root,
        "custom",
        "1",
        json!(["OBSERVE", "COMMUNICATE"]),
        "grant",
    );
    let accepted = invoke(root);
    assert_eq!(
        accepted["invocations"][0]["effects"][0]["status"],
        "ACCEPTED"
    );
    assert!(crate::execution::EvidenceStore::load(root)
        .unwrap()
        .evidence
        .is_empty());
    assert!(crate::worker_descriptors(root).unwrap().is_empty());
}

#[test]
fn generic_event_cannot_forge_institution_or_worker_authority() {
    let d = init();
    let root = d.path();
    let source = fixture(root, "custom", "1");
    let real = command(
        root,
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    let draft: DevelopmentEventDraft =
        serde_json::from_value(json!({"payload":real.event.payload})).unwrap();
    assert!(development::append_development_event(root, draft)
        .unwrap_err()
        .to_string()
        .contains("AUTHORITY_DENIED"));
    let pid = crate::load_identity(root).unwrap().unwrap().project_id;
    for grants in [
        json!(["CHANGE_INSTITUTION_BINDING"]),
        json!(["FUTURE_PRIVILEGED"]),
    ] {
        assert!(execute(root,serde_json::from_value(json!({"operation":"activate","institution_id":"custom","version":"1","project_id":pid,"grants":grants,"activated_by":"operator","expected_binding_revision":0})).unwrap(),"bad").is_err());
    }
    assert!(bindings(root).unwrap().is_empty());
}

#[test]
fn raw_reasoning_and_unknown_fields_are_rejected_without_copying_source() {
    let d = init();
    let external = tempfile::tempdir().unwrap();
    let source = fixture(external.path(), "custom", "1");
    let mut value: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    value["rules"][0]["effects"][0]["from_worker"] = json!("forged-worker");
    fs::write(&source, value.to_string()).unwrap();
    assert!(inspect_package(d.path(), &source).is_err());
    value["rules"][0]["effects"][0]
        .as_object_mut()
        .unwrap()
        .remove("from_worker");
    value["configuration"] = json!({"raw_reasoning":"not permitted"});
    fs::write(&source, value.to_string()).unwrap();
    assert!(inspect_package(d.path(), &source).is_err());
    value["configuration"] = json!({});
    fs::write(&source, value.to_string()).unwrap();
    command(
        d.path(),
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    assert!(!d.path().join(".route/institutions").exists());
    assert!(versions(d.path()).unwrap()[0]
        .source_locator
        .starts_with(external.path().to_str().unwrap()));
}

#[test]
fn package_future_kind_remains_representable_but_not_executable() {
    let d = init();
    let source = fixture(d.path(), "future", "1");
    let mut v: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    v["implementation_kind"] = json!("FUTURE_STDIO");
    fs::write(&source, v.to_string()).unwrap();
    command(
        d.path(),
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    assert_eq!(
        versions(d.path()).unwrap()[0].status,
        "UNSUPPORTED_IMPLEMENTATION"
    );
    assert!(get(d.path(), "forged", "1").is_err());
}

#[test]
fn replay_is_deterministic_and_does_not_mutate_ledger() {
    let d = init();
    let root = d.path();
    let source = fixture(root, "custom", "1");
    command(
        root,
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    activate(
        root,
        "custom",
        "1",
        json!(["OBSERVE", "COMMUNICATE"]),
        "act",
    );
    event(root);
    let path = root.join(".route/development/ledger.json");
    let before = fs::read(&path).unwrap();
    let first = replay(root, "custom", "1", 0, 1000).unwrap();
    assert_eq!(first, replay(root, "custom", "1", 0, 1000).unwrap());
    assert_eq!(before, fs::read(path).unwrap());
    assert!(first.dry_run);
    assert!(replay(root, "custom", "1", 0, 1001).is_err());
}

#[test]
fn stale_context_and_cross_project_activation_fail_closed() {
    let d = init();
    let root = d.path();
    let source = fixture(root, "custom", "1");
    command(
        root,
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    assert!(execute(root,serde_json::from_value(json!({"operation":"activate","institution_id":"custom","version":"1","project_id":"foreign","grants":[],"activated_by":"operator","expected_binding_revision":0})).unwrap(),"foreign").is_err());
    let trigger = event(root);
    event(root);
    assert!(execute(root,serde_json::from_value(json!({"operation":"invoke","hook":"ON_EVENT","triggering_event_ref":trigger.event.event_id,"expected_revision":trigger.global_revision,"correlation_id":"stale"})).unwrap(),"stale").unwrap_err().to_string().contains("STALE_CONTEXT"));
}

#[test]
fn no_effect_missing_observe_size_traversal_and_corrupt_ledger() {
    let d = init();
    let root = d.path();
    let source = fixture(root, "custom", "1");
    command(
        root,
        json!({"operation":"register","source_locator":source}),
        "reg",
    );
    activate(root, "custom", "1", json!([]), "no-observe");
    assert_eq!(invoke(root)["invocations"][0]["status"], "DENIED");
    activate(root, "custom", "1", json!(["OBSERVE"]), "observe");
    let trigger = event(root);
    let result = command(
        root,
        json!({"operation":"invoke","hook":"ON_REQUEST","triggering_event_ref":trigger.event.event_id,"expected_revision":trigger.global_revision,"correlation_id":"no-effect"}),
        "no-effect",
    );
    let value = serde_json::to_value(result).unwrap();
    assert_eq!(
        value["event"]["payload"]["data"]["transaction"]["change"]["report"]["invocations"][0]
            ["status"],
        "NO_EFFECT"
    );
    assert!(inspect_package(root, "../outside.json").is_err());
    fs::write(&source, vec![b' '; 65537]).unwrap();
    assert!(inspect_package(root, &source).is_err());
    let ledger = root.join(".route/development/ledger.json");
    let mut corrupted: Value = serde_json::from_slice(&fs::read(&ledger).unwrap()).unwrap();
    corrupted["events"][0]["hash"] = json!("forged");
    let bytes = serde_json::to_vec(&corrupted).unwrap();
    fs::write(&ledger, &bytes).unwrap();
    assert!(versions(root).is_err());
    assert!(bindings(root).is_err());
    assert_eq!(fs::read(ledger).unwrap(), bytes);
}
