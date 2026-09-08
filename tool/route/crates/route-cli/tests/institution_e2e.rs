//! Custom packages and all production mutations use independent Route processes.

#[cfg(all(windows, feature = "test-utils"))]
#[test]
#[ignore = "bounded real-process resource measurement"]
fn institution_resource_budget() {
    use std::{io::Read, os::windows::io::AsRawHandle, time::Instant};
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak: usize,
        working: usize,
        pool_peak: usize,
        pool: usize,
        nonpaged_peak: usize,
        nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn K32GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut Counters,
            size: u32,
        ) -> i32;
    }
    fn bytes(path: &Path) -> u64 {
        fs::read_dir(path)
            .unwrap()
            .map(|entry| {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    bytes(&p)
                } else {
                    fs::metadata(p).unwrap().len()
                }
            })
            .sum()
    }
    let mut rows = vec![];
    for count in [0, 1, 5] {
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        route_basic::ensure_identity(root).unwrap();
        route_basic::development::seed_benchmark_events(root, 1000).unwrap();
        let external = tempfile::tempdir().unwrap();
        for i in 0..count {
            let id = format!("bench-{i}");
            let source = manifest(external.path(), &id, "1", "review");
            ok(
                root,
                "institution.register",
                json!({"source_locator":source}),
                Some(&format!("register-{i}")),
            );
            activate(root, &id, "1", i, &format!("activate-{i}"));
        }
        for method in ["institution.invoke", "institution.replay"] {
            if count == 0 && method == "institution.replay" {
                continue;
            }
            let before = snapshot(root);
            let durable_before = bytes(root);
            let mut elapsed = vec![];
            let mut peak = 0;
            let mut result_bytes = 0;
            for run in 0..5 {
                let params = if method == "institution.invoke" {
                    json!({"hook":"ON_EVENT","triggering_event_ref":"benchmark-0","expected_revision":state(root)["global_revision"],"correlation_id":format!("budget-{run}")})
                } else {
                    json!({"institution_id":"bench-0","version":"1","after_revision":0,"limit":1000})
                };
                let key = format!("budget-{count}-{run}");
                let start = Instant::now();
                let mut child = spawn(
                    root,
                    method,
                    params,
                    if method == "institution.invoke" {
                        Some(&key)
                    } else {
                        None
                    },
                );
                let mut stdout = child.stdout.take().unwrap();
                let reader = std::thread::spawn(move || {
                    let mut v = vec![];
                    stdout.read_to_end(&mut v).unwrap();
                    v
                });
                assert!(child.wait().unwrap().success());
                let mut counters = Counters {
                    cb: std::mem::size_of::<Counters>() as u32,
                    ..Default::default()
                };
                assert_ne!(
                    unsafe {
                        K32GetProcessMemoryInfo(
                            child.as_raw_handle(),
                            &mut counters,
                            std::mem::size_of::<Counters>() as u32,
                        )
                    },
                    0
                );
                peak = peak.max(counters.peak);
                let output = reader.join().unwrap();
                result_bytes = output.len();
                elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
                let response: Value = serde_json::from_slice(&output).unwrap();
                assert_eq!(response["ok"], true, "{response}");
                if method == "institution.replay" {
                    assert_eq!(
                        response["result"]["invocations"].as_array().unwrap().len(),
                        1000
                    );
                } else {
                    assert_eq!(
                        report(&response["result"])["invocations"]
                            .as_array()
                            .unwrap()
                            .len(),
                        count as usize
                    );
                }
            }
            elapsed.sort_by(f64::total_cmp);
            let growth = bytes(root) - durable_before;
            if method == "institution.replay" {
                assert_eq!(growth, 0);
                assert!(before == snapshot(root));
            }
            rows.push(json!({"active_institutions":count,"fixture_events":1000,"method":method,"runs":5,"p50_ms":elapsed[2],"p95_ms":elapsed[4],"peak_working_set_bytes":peak,"durable_before_bytes":durable_before,"durable_growth_bytes":growth,"result_bytes":result_bytes,"correctness":"PASS"}));
        }
    }
    println!(
        "INSTITUTION_BUDGET={}",
        json!({"binary_sha256":route_core::sha256_hex(&fs::read(env!("CARGO_BIN_EXE_route")).unwrap()),"binary_bytes":fs::metadata(env!("CARGO_BIN_EXE_route")).unwrap().len(),"profile":"debug test-utils","latency_scope":"fresh process + RPC + ledger validation + evaluation + response; invoke also includes durable ledger and receipt","rows":rows})
    );
}

use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
fn cli(root: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn spawn(root: &Path, method: &str, params: Value, key: Option<&str>) -> std::process::Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("rpc")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(json!({"protocol":"route/1","request_id":"institution-test","method":method,"context":{},"params":params,"idempotency_key":key}).to_string().as_bytes()).unwrap();
    child
}
fn finish(child: std::process::Child) -> Value {
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn call(root: &Path, method: &str, params: Value, key: Option<&str>) -> Value {
    finish(spawn(root, method, params, key))
}
fn ok(root: &Path, method: &str, params: Value, key: Option<&str>) -> Value {
    let v = call(root, method, params, key);
    assert_eq!(v["ok"], true, "{method}: {v}");
    v["result"].clone()
}
fn state(root: &Path) -> Value {
    ok(root, "development.state", json!({}), None)["state"].clone()
}
fn snapshot(root: &Path) -> BTreeMap<String, (String, std::time::SystemTime)> {
    fn visit(
        root: &Path,
        path: &Path,
        out: &mut BTreeMap<String, (String, std::time::SystemTime)>,
    ) {
        for e in fs::read_dir(path).unwrap() {
            let p = e.unwrap().path();
            let m = fs::symlink_metadata(&p).unwrap();
            out.insert(
                p.strip_prefix(root).unwrap().to_string_lossy().into(),
                (
                    if m.is_file() {
                        route_core::sha256_hex(&fs::read(&p).unwrap())
                    } else {
                        "DIR".into()
                    },
                    m.modified().unwrap(),
                ),
            );
            if m.is_dir() {
                visit(root, &p, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn manifest(root: &Path, id: &str, version: &str, value: &str) -> String {
    let path = root.join(format!("{id}-{version}.json"));
    fs::write(&path,json!({"institution_id":id,"name":"User written review suggestion","version":version,"provenance":"disposable user-authored package","implementation_kind":"DECLARATIVE_V1","supported_hooks":["ON_EVENT"],"compatibility":["route/1"],"requested_capabilities":["OBSERVE","PROPOSE"],"configuration":{"review_policy":value},"rules":[{"hook":"ON_EVENT","event_type":"FINDING","effects":[{"family":"RECOMMEND","summary":"Consider independent review","conflict_key":"coordination","value":value}]}]}).to_string()).unwrap();
    path.to_string_lossy().into()
}
fn binding(root: &Path, id: &str) -> Value {
    ok(root, "institution.bindings", json!({}), None)
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["institution_id"] == id)
        .cloned()
        .unwrap_or(Value::Null)
}
fn activate(root: &Path, id: &str, version: &str, order: i32, key: &str) -> Value {
    let revision = binding(root, id)["activation_revision"]
        .as_u64()
        .unwrap_or(0);
    ok(
        root,
        "institution.activate",
        json!({"institution_id":id,"version":version,"project_id":state(root)["project_id"],"grants":["OBSERVE","COMMUNICATE","PROPOSE"],"order":order,"activated_by":"operator","expected_binding_revision":revision}),
        Some(key),
    )
}
fn finding(root: &Path, key: &str) -> Value {
    ok(
        root,
        "development.event.record",
        json!({"actor_worker_id":"worker-a","payload":{"kind":"FINDING","data":{"summary":"A real shared finding"}}}),
        Some(key),
    )
}
fn invoke(root: &Path, event: &Value, key: &str) -> Value {
    ok(
        root,
        "institution.invoke",
        json!({"hook":"ON_EVENT","triggering_event_ref":event["event"]["event_id"],"expected_revision":state(root)["global_revision"],"requesting_actor":"worker-b","correlation_id":key}),
        Some(key),
    )
}
fn report(result: &Value) -> Value {
    result["event"]["payload"]["data"]["transaction"]["change"]["report"].clone()
}

#[test]
fn institution_register_resolves_cooperation_reference_without_an_extra_locator() {
    let project = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    cli(project.path(), &["init"]);
    let source = manifest(external.path(), "cooperation-package", "1", "review");
    let before = snapshot(external.path());
    ok(
        project.path(),
        "cooperation.register",
        json!({"cooperation_id":"package-source","locator":source,"kind":"DOCUMENT","provenance":"author-owned fixture"}),
        Some("source"),
    );
    let params = json!({"cooperation_ref":"package-source"});
    let first = ok(
        project.path(),
        "institution.register",
        params.clone(),
        Some("register-ref"),
    );
    assert_eq!(
        first,
        ok(
            project.path(),
            "institution.register",
            params,
            Some("register-ref")
        )
    );
    let invalid = call(
        project.path(),
        "institution.register",
        json!({"cooperation_ref":"package-source","source_locator":source}),
        Some("ambiguous-source"),
    );
    assert_eq!(invalid["ok"], false);
    assert_eq!(invalid["error"]["code"], "INVALID_PACKAGE");
    assert!(before == snapshot(external.path()));
}

#[cfg(windows)]
#[test]
fn institution_source_rejects_junction_ancestor_without_copying_or_writing_target() {
    let project = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    manifest(external.path(), "linked", "1", "review");
    let before = snapshot(external.path());
    let link = project.path().join("linked");
    let output = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            link.to_str().unwrap(),
            external.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result = route_basic::institution::inspect_package(project.path(), "linked/linked-1.json");
    // Only remove the test-owned junction itself, never recursively its target.
    fs::remove_dir(&link).unwrap();
    assert!(result.is_err());
    assert!(before == snapshot(external.path()));
    assert!(!project.path().join(".route").exists());
}

#[test]
fn custom_package_restart_composition_workers_and_version_rollback() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    cli(root, &["init"]);
    for id in ["worker-a", "worker-b"] {
        ok(root, "worker.register", json!({"worker_id":id}), Some(id));
        ok(
            root,
            "worker.presence.update",
            json!({"worker_id":id,"status":"IDLE","observed_global_revision":0}),
            Some(&format!("presence-{id}")),
        );
    }
    let external = tempfile::tempdir().unwrap();
    let v1 = manifest(external.path(), "user-review", "1", "paired-review");
    let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../examples/institutions/free-autonomy-minimal/institution.json")
        .canonicalize()
        .unwrap();
    let source_before = snapshot(external.path());
    cli(
        root,
        &[
            "institution",
            "register",
            sample.to_str().unwrap(),
            "--operation-key",
            "sample",
        ],
    );
    cli(
        root,
        &["institution", "register", &v1, "--operation-key", "custom"],
    );
    let inspected: Value =
        serde_json::from_str(&cli(root, &["institution", "inspect", &v1])).unwrap();
    assert_eq!(inspected["definition"]["institution_id"], "user-review");
    assert!(ok(root, "institution.bindings", json!({}), None)
        .as_array()
        .unwrap()
        .is_empty());
    activate(root, "free-autonomy-minimal", "1.0.0", 0, "activate-sample");
    activate(root, "user-review", "1", 1, "activate-custom");
    let original = get_version(root, "user-review", "1");
    let trigger = finding(root, "finding");
    let result = invoke(root, &trigger, "invoke");
    let evaluated = report(&result);
    assert!(result["event"]["actor_worker_id"].is_null());
    assert_eq!(evaluated["invocations"].as_array().unwrap().len(), 2);
    assert_eq!(
        evaluated["invocations"][0]["context"]["institution_id"],
        "free-autonomy-minimal"
    );
    assert_eq!(
        evaluated["invocations"][1]["context"]["institution_id"],
        "user-review"
    );
    assert_eq!(
        evaluated["invocations"][0]["context"]["global_revision"],
        evaluated["invocations"][1]["context"]["global_revision"]
    );
    assert_eq!(
        evaluated["invocations"][0]["context"]["worker_presence"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!evaluated["invocations"][1]["effects"][0]["conflict_with"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        evaluated["invocations"][1]["effects"][0]["status"],
        "ACCEPTED"
    );
    for worker in ["worker-a", "worker-b"] {
        let message = ok(
            root,
            "worker.message.send",
            json!({"worker_id":worker,"message_type":"NOTICE","content":"Independently reviewing shared suggestions","source_refs":[evaluated["invocations"][1]["effects"][0]["effect_id"]]}),
            Some(&format!("message-{worker}")),
        );
        assert_eq!(message["event"]["actor_worker_id"], worker);
    }
    for _worker in ["worker-a", "worker-b"] {
        let delta = ok(
            root,
            "development.events.query",
            json!({"after_revision":trigger["global_revision"]}),
            None,
        );
        assert_eq!(delta["events"][0]["payload"]["kind"], "INSTITUTION");
        assert!(delta["events"][0]["actor_worker_id"].is_null());
    }
    let workers = ok(root, "worker.list", json!({}), None);
    assert_eq!(workers["workers"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot(external.path()), source_before);
    assert_eq!(binding(root, "user-review")["version"], "1");
    let v2 = manifest(external.path(), "user-review", "2", "optional-review");
    ok(
        root,
        "institution.register",
        json!({"source_locator":v2}),
        Some("v2"),
    );
    let before = snapshot(root);
    let dry: Value = serde_json::from_str(&cli(
        root,
        &[
            "institution",
            "replay",
            "user-review",
            "2",
            "--after",
            "0",
            "--limit",
            "100",
        ],
    ))
    .unwrap();
    assert_eq!(dry["dry_run"], true);
    assert!(!dry["invocations"].as_array().unwrap().is_empty());
    assert_eq!(before, snapshot(root));
    activate(root, "user-review", "2", 1, "activate-v2");
    assert_eq!(binding(root, "user-review")["version"], "2");
    activate(root, "user-review", "1", 1, "rollback-v1");
    assert_eq!(get_version(root, "user-review", "1"), original);
    let b = binding(root, "user-review");
    cli(
        root,
        &[
            "institution",
            "deactivate",
            "user-review",
            "--actor",
            "operator",
            "--expected-binding-revision",
            &b["activation_revision"].to_string(),
            "--operation-key",
            "deactivate",
        ],
    );
    assert_eq!(binding(root, "user-review")["active"], false);
    let history = ok(
        root,
        "development.events.query",
        json!({"after_revision":0,"limit":1000}),
        None,
    )
    .to_string();
    assert!(history.contains("INSTITUTION_VERSION_CHANGED"));
    assert!(history.contains("INSTITUTION_EFFECT_ACCEPTED"));
    assert!(history.contains("INSTITUTION_DEACTIVATED"));
    assert!(state(root)["latest_evidence"]
        .as_array()
        .unwrap()
        .is_empty());
}
fn get_version(root: &Path, id: &str, version: &str) -> Value {
    ok(
        root,
        "institution.get",
        json!({"institution_id":id,"version":version}),
        None,
    )
}

#[test]
fn mutation_replay_conflicts_concurrent_activation_and_read_only_jsonl() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    cli(root, &["init"]);
    let external = tempfile::tempdir().unwrap();
    let source = manifest(external.path(), "custom", "1", "review");
    let register = json!({"source_locator":source});
    let first = ok(root, "institution.register", register.clone(), Some("reg"));
    assert_eq!(
        ok(root, "institution.register", register.clone(), Some("reg"))["event"],
        first["event"]
    );
    let mut changed = register;
    changed["source_locator"] = json!("different");
    assert_eq!(
        call(root, "institution.register", changed, Some("reg"))["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    let params = json!({"institution_id":"custom","version":"1","project_id":state(root)["project_id"],"grants":["OBSERVE","PROPOSE"],"activated_by":"operator","expected_binding_revision":0});
    let a = spawn(root, "institution.activate", params.clone(), Some("a"));
    let b = spawn(root, "institution.activate", params.clone(), Some("b"));
    let responses = [finish(a), finish(b)];
    assert_eq!(responses.iter().filter(|r| r["ok"] == true).count(), 1);
    let winner = if responses[0]["ok"] == true { "a" } else { "b" };
    ok(root, "institution.activate", params.clone(), Some(winner));
    let mut changed = params;
    changed["order"] = json!(99);
    assert_eq!(
        call(root, "institution.activate", changed, Some(winner))["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    ok(
        root,
        "worker.register",
        json!({"worker_id":"worker-a"}),
        Some("worker-a"),
    );
    ok(
        root,
        "worker.register",
        json!({"worker_id":"worker-b"}),
        Some("worker-b"),
    );
    let event = finding(root, "finding");
    let params = json!({"hook":"ON_EVENT","triggering_event_ref":event["event"]["event_id"],"expected_revision":event["global_revision"],"requesting_actor":"worker-a","correlation_id":"invoke"});
    let inv = ok(root, "institution.invoke", params.clone(), Some("invoke"));
    assert_eq!(
        ok(root, "institution.invoke", params.clone(), Some("invoke"))["event"],
        inv["event"]
    );
    let mut changed = params;
    changed["correlation_id"] = json!("different");
    assert_eq!(
        call(root, "institution.invoke", changed, Some("invoke"))["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    let params = json!({"institution_id":"custom","project_id":state(root)["project_id"],"actor":"operator","expected_binding_revision":binding(root,"custom")["activation_revision"]});
    let deactivated = ok(root, "institution.deactivate", params.clone(), Some("off"));
    assert_eq!(
        ok(root, "institution.deactivate", params.clone(), Some("off"))["event"],
        deactivated["event"]
    );
    let mut changed = params;
    changed["actor"] = json!("other");
    assert_eq!(
        call(root, "institution.deactivate", changed, Some("off"))["error"]["code"],
        "IDEMPOTENCY_CONFLICT"
    );
    let before = snapshot(root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(["rpc", "--jsonl"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    for method in [
        "institution.list",
        "institution.get",
        "institution.bindings",
        "institution.inspect",
        "institution.replay",
    ] {
        writeln!(input,"{}",json!({"protocol":"route/1","request_id":method,"method":method,"context":{},"params":{"institution_id":"custom","version":"1","source_locator":source,"limit":2}})).unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        assert_eq!(serde_json::from_str::<Value>(line).unwrap()["ok"], true);
    }
    assert_eq!(before, snapshot(root));
    let capabilities = ok(root, "system.capabilities", json!({}), None).to_string();
    for method in [
        "list",
        "get",
        "bindings",
        "inspect",
        "register",
        "activate",
        "deactivate",
        "invoke",
        "replay",
    ] {
        assert!(capabilities.contains(&format!("institution.{method}")));
    }
    assert!(!capabilities.contains("institution.shell"));
}

#[test]
fn security_denial_failure_and_source_drift_do_not_mutate_other_domains() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    cli(root, &["init"]);
    let external = tempfile::tempdir().unwrap();
    let source = manifest(external.path(), "custom", "1", "review");
    let mut package: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    package["rules"][0]["effects"] = json!([
        {"family":"EVIDENCE","summary":"forged proof"},
        {"family":"MESSAGE","summary":"forged identity","target_ref":"worker:worker-a"},
        {"family":"REQUEST_ACTION","summary":"not an execution command","target_ref":"shell:echo forbidden"},
        {"family":"RECOMMEND","summary":"no grant"}]);
    fs::write(&source, package.to_string()).unwrap();
    ok(
        root,
        "institution.register",
        json!({"source_locator":source}),
        Some("reg"),
    );
    let pid = state(root)["project_id"].clone();
    let params = json!({"institution_id":"custom","version":"1","project_id":pid,"grants":["OBSERVE"],"activated_by":"operator","expected_binding_revision":0});
    let mut foreign = params.clone();
    foreign["project_id"] = json!("foreign-project");
    assert_eq!(
        call(root, "institution.activate", foreign, Some("foreign"))["ok"],
        false
    );
    ok(root, "institution.activate", params, Some("active"));
    for worker in ["worker-a", "worker-b"] {
        ok(
            root,
            "worker.register",
            json!({"worker_id":worker}),
            Some(worker),
        );
    }
    let trigger = finding(root, "f");
    let result = invoke(root, &trigger, "invoke");
    let outcomes = report(&result);
    assert_eq!(outcomes["invocations"][0]["status"], "INVALID_EFFECT");
    assert!(outcomes["invocations"][0]["effects"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["status"] != "ACCEPTED"));
    let forged = call(
        root,
        "development.event.record",
        json!({"payload":result["event"]["payload"],"actor_worker_id":"worker-a"}),
        Some("forged"),
    );
    assert_eq!(forged["ok"], false);
    package["configuration"] = json!({"changed":"new bytes same version"});
    fs::write(&source, package.to_string()).unwrap();
    let mismatch = report(&invoke(root, &trigger, "mismatch"));
    assert_eq!(mismatch["invocations"][0]["status"], "VERSION_MISMATCH");
    fs::write(&source, b"{broken").unwrap();
    let error = report(&invoke(root, &trigger, "error"));
    assert_eq!(error["invocations"][0]["status"], "INSTITUTION_ERROR");
    fs::remove_file(&source).unwrap();
    let missing = report(&invoke(root, &trigger, "missing"));
    assert_eq!(
        missing["invocations"][0]["status"],
        "INSTITUTION_UNAVAILABLE"
    );
    assert!(state(root)["latest_evidence"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(state(root)["active_execution_sessions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        ok(root, "worker.list", json!({}), None)["workers"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
