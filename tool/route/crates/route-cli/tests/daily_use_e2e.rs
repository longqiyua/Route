//! All workflow mutations cross a real executable boundary. Fixtures never edit Route state.
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
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn call(root: &Path, method: &str, params: Value, key: Option<&str>) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("rpc")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let request = json!({"protocol":"route/1","request_id":"daily","method":method,"context":{},"params":params,"idempotency_key":key});
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
fn ok(root: &Path, method: &str, params: Value, key: Option<&str>) -> Value {
    let response = call(root, method, params, key);
    assert_eq!(response["ok"], true, "{method}: {response}");
    if method == "development.state" {
        response["result"]["state"].clone()
    } else {
        response["result"].clone()
    }
}
fn snapshot(root: &Path) -> BTreeMap<String, (Vec<u8>, std::time::SystemTime)> {
    fn visit(
        root: &Path,
        path: &Path,
        out: &mut BTreeMap<String, (Vec<u8>, std::time::SystemTime)>,
    ) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            out.insert(
                path.strip_prefix(root).unwrap().to_string_lossy().into(),
                (
                    if meta.is_file() {
                        fs::read(&path).unwrap()
                    } else {
                        vec![]
                    },
                    meta.modified().unwrap(),
                ),
            );
            if meta.is_dir() {
                visit(root, &path, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}

#[test]
fn real_process_normal_resume_world_change_workers_and_100_reads() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("resource.txt"), b"version one").unwrap();
    cli(root, &["init"]);
    for worker in ["worker-a", "worker-b"] {
        ok(
            root,
            "worker.register",
            json!({"worker_id":worker}),
            Some(worker),
        );
    }
    let before = ok(root, "development.state", json!({}), None);
    cli(
        root,
        &[
            "reference",
            "register",
            "ref",
            "resource.txt",
            "--operation-key",
            "ref",
        ],
    );
    let registration = json!({"cooperation_id":"resource","locator":"resource.txt","kind":"DOCUMENT","provenance":"worker-a","actor_worker_id":"worker-a"});
    ok(
        root,
        "cooperation.register",
        registration.clone(),
        Some("resource"),
    );
    let resource = ok(
        root,
        "cooperation.get",
        json!({"cooperation_id":"resource"}),
        None,
    );
    let knowledge = json!({"knowledge_id":"knowledge-a","cooperation_id":"resource","statement":"declared text fixture","capability_refs":["fixture-read"],"epistemic_status":"DECLARED","provenance":"worker-a","actor_worker_id":"worker-a","resource_fingerprint":resource["fingerprint"]["value"]});
    ok(
        root,
        "cooperation.knowledge.record",
        knowledge.clone(),
        Some("knowledge"),
    );
    let state_a = ok(root, "development.state", json!({}), None);
    let delta = ok(
        root,
        "development.events.query",
        json!({"after_revision":before["global_revision"]}),
        None,
    );
    assert_eq!(delta["project_id"], state_a["project_id"]);
    assert!(delta["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| event["actor_worker_id"] == "worker-a"));
    let learned = ok(
        root,
        "cooperation.knowledge.query",
        json!({"cooperation_id":"resource"}),
        None,
    );
    assert_eq!(learned[0]["provenance"], "worker-a");
    ok(
        root,
        "worker.presence.update",
        json!({"worker_id":"worker-b","status":"IDLE","observed_global_revision":state_a["global_revision"]}),
        Some("b-reuse"),
    );
    let state_b = ok(root, "development.state", json!({}), None);
    ok(root, "cooperation.register", registration, Some("resource"));
    ok(
        root,
        "cooperation.knowledge.record",
        knowledge.clone(),
        Some("knowledge"),
    );
    assert_eq!(
        ok(root, "development.state", json!({}), None)["global_revision"],
        state_b["global_revision"]
    );
    let mut conflict = knowledge;
    conflict["statement"] = json!("different");
    assert_eq!(
        call(
            root,
            "cooperation.knowledge.record",
            conflict,
            Some("knowledge")
        )["ok"],
        false
    );
    assert_eq!(
        ok(root, "reference.get", json!({"reference_id":"ref"}), None)["id"],
        "ref"
    );
    ok(root, "reference.list", json!({}), None);
    ok(root, "cooperation.list", json!({}), None);
    let snapshot_before = snapshot(root);
    for _ in 0..100 {
        ok(root, "cooperation.knowledge.query", json!({}), None);
    }
    let snapshot_after = snapshot(root);
    let changed: Vec<_> = snapshot_after
        .iter()
        .filter(|(path, value)| snapshot_before.get(*path) != Some(value))
        .map(|(path, value)| (path, snapshot_before.get(path).map(|old| old.0 == value.0)))
        .collect();
    assert!(
        snapshot_before == snapshot_after,
        "100 reads changed paths: {changed:?}"
    );
    fs::write(root.join("resource.txt"), b"version two changed").unwrap();
    cli(
        root,
        &[
            "cooperation",
            "refresh",
            "resource",
            "--operation-key",
            "refresh",
        ],
    );
    cli(
        root,
        &["reference", "observe", "ref", "--operation-key", "observe"],
    );
    let stale = ok(root, "cooperation.knowledge.query", json!({}), None);
    assert_eq!(stale[0]["stale"], true);
    assert_eq!(
        ok(
            root,
            "cooperation.get",
            json!({"cooperation_id":"resource"}),
            None
        )["declared_capabilities"],
        json!([])
    );
    assert_eq!(stale[0]["provenance"], "worker-a");
    cli(
        root,
        &[
            "cooperation",
            "knowledge",
            "record",
            "knowledge-b",
            "resource",
            "replacement declaration",
            "--provenance",
            "worker-b",
            "--supersedes",
            "knowledge-a",
            "--operation-key",
            "replacement",
        ],
    );
    assert_eq!(
        ok(root, "cooperation.knowledge.query", json!({}), None)
            .as_array()
            .unwrap()
            .len(),
        2
    );
    fs::remove_file(root.join("resource.txt")).unwrap();
    ok(
        root,
        "cooperation.refresh",
        json!({"cooperation_id":"resource"}),
        Some("missing"),
    );
    assert_eq!(
        ok(
            root,
            "cooperation.get",
            json!({"cooperation_id":"resource"}),
            None
        )["availability"],
        "MISSING"
    );
}

#[cfg(feature = "test-utils")]
#[test]
fn actual_crash_recovery_and_retry_without_manual_state_edit() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    cli(root, &["init"]);
    fs::write(root.join("external.txt"), b"fixture").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_route"))
        .args([
            "reference",
            "register",
            "interrupted",
            "external.txt",
            "--operation-key",
            "crash-key",
        ])
        .env("ROUTE_REFERENCE_FAIL_AT", "domain")
        .current_dir(root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(23));
    let read = call(root, "reference.list", json!({}), None);
    assert_eq!(read["ok"], false);
    assert!(read["error"]["message"]
        .as_str()
        .unwrap()
        .contains("route reference recover"));
    cli(
        root,
        &["reference", "recover", "--operation-key", "recovery"],
    );
    let before = ok(root, "development.state", json!({}), None);
    cli(
        root,
        &[
            "reference",
            "register",
            "interrupted",
            "external.txt",
            "--operation-key",
            "crash-key",
        ],
    );
    assert_eq!(
        before["global_revision"],
        ok(root, "development.state", json!({}), None)["global_revision"]
    );
    assert_eq!(
        ok(root, "reference.list", json!({}), None)
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["id"] == "interrupted")
            .count(),
        1
    );
    cli(
        root,
        &[
            "reference",
            "register",
            "continued",
            "external.txt",
            "--operation-key",
            "continued",
        ],
    );
}

#[test]
fn foreign_project_inspection_and_fresh_reads_are_non_mutating() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for method in [
        "reference.list",
        "cooperation.list",
        "cooperation.knowledge.query",
        "development.state",
        "development.events.query",
        "worker.list",
    ] {
        call(a.path(), method, json!({}), None);
        assert_eq!(
            fs::read_dir(a.path()).unwrap().count(),
            0,
            "{method} initialized fresh project"
        );
    }
    cli(a.path(), &["init"]);
    cli(b.path(), &["init"]);
    let sa = ok(a.path(), "development.state", json!({}), None);
    let sb = ok(b.path(), "development.state", json!({}), None);
    assert_ne!(sa["project_id"], sb["project_id"]);
    let before = snapshot(b.path());
    ok(
        a.path(),
        "cooperation.register",
        json!({"cooperation_id":"foreign","locator":b.path(),"kind":"PROJECT","provenance":"read-only fixture"}),
        Some("foreign"),
    );
    ok(
        a.path(),
        "cooperation.get",
        json!({"cooperation_id":"foreign"}),
        None,
    );
    assert_eq!(before, snapshot(b.path()));
}

#[test]
fn bounded_adversarial_resources_and_evidence_gate() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    cli(root, &["init"]);
    for (id, locator) in [
        ("secret", "https://user:password@example.invalid/x"),
        ("token=fixture", "missing"),
        ("traversal", "../outside"),
    ] {
        let rejected = call(
            root,
            "cooperation.register",
            json!({"cooperation_id":id,"locator":locator,"kind":"DOCUMENT","provenance":"fixture"}),
            Some(id),
        );
        assert_eq!(rejected["ok"], false, "{rejected}");
    }
    let external = tempfile::tempdir().unwrap();
    let large = external.path().join("large.bin");
    fs::File::create(&large)
        .unwrap()
        .set_len(256 * 1024 * 1024)
        .unwrap();
    cli(
        root,
        &[
            "cooperation",
            "add",
            "large",
            large.to_str().unwrap(),
            "--provenance",
            "fixture",
            "--operation-key",
            "large",
        ],
    );
    let resource = ok(
        root,
        "cooperation.get",
        json!({"cooperation_id":"large"}),
        None,
    );
    assert_eq!(
        resource["fingerprint"]["algorithm"],
        "SHA256_BOUNDED_HEAD_TAIL_MTIME_V2"
    );
    let denied = call(
        root,
        "cooperation.knowledge.record",
        json!({"knowledge_id":"untrusted","cooperation_id":"large","statement":"cannot self-certify","epistemic_status":"OBSERVED","provenance":"fixture","capability_refs":["read"],"resource_fingerprint":resource["fingerprint"]["value"]}),
        Some("untrusted"),
    );
    assert_eq!(denied["ok"], false);
    assert!(denied["error"]["message"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("evidence"));
    let many = external.path().join("many");
    fs::create_dir(&many).unwrap();
    for index in 0..2000 {
        fs::write(many.join(index.to_string()), b"not recursively inspected").unwrap();
    }
    ok(
        root,
        "cooperation.register",
        json!({"cooperation_id":"many","locator":many,"kind":"DIRECTORY","provenance":"fixture"}),
        Some("many"),
    );
    let bytes: usize = snapshot(root).values().map(|v| v.0.len()).sum();
    assert!(
        bytes < 1024 * 1024,
        "external contents were copied into Route state"
    );
    fs::write(root.join("normal.txt"), b"normal").unwrap();
    ok(
        root,
        "cooperation.register",
        json!({"cooperation_id":"normal","locator":"./normal.txt","kind":"DOCUMENT","provenance":"fixture"}),
        Some("normal"),
    );
    assert_eq!(
        ok(
            root,
            "cooperation.get",
            json!({"cooperation_id":"normal"}),
            None
        )["locator"],
        "normal.txt"
    );
}

#[test]
fn each_dedicated_mutation_has_durable_replay_and_changed_parameter_rejection() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    cli(root, &["init"]);
    fs::write(root.join("fixture.txt"), b"safe").unwrap();
    let operations = [
        (
            "reference.register",
            json!({"reference_id":"r","locator":"fixture.txt"}),
        ),
        ("reference.refresh", json!({"reference_id":"r"})),
        ("reference.recover", json!({})),
        (
            "cooperation.register",
            json!({"cooperation_id":"c","locator":"fixture.txt","kind":"DOCUMENT","provenance":"fixture"}),
        ),
        ("cooperation.refresh", json!({"cooperation_id":"c"})),
        (
            "cooperation.knowledge.record",
            json!({"knowledge_id":"k","cooperation_id":"c","statement":"declared fixture","epistemic_status":"DECLARED","provenance":"fixture"}),
        ),
    ];
    let capabilities = ok(root, "system.capabilities", json!({}), None).to_string();
    let logical = || {
        snapshot(root)
            .into_iter()
            .filter(|(path, _)| {
                path.ends_with("registry.json")
                    || path.ends_with("ledger.json")
                    || path.ends_with("rpc-idempotency.json")
            })
            .collect::<BTreeMap<_, _>>()
    };
    for (method, params) in operations {
        assert!(capabilities.contains(method));
        ok(root, method, params.clone(), Some(method));
        let before = logical();
        ok(root, method, params.clone(), Some(method));
        assert!(before == logical(), "{method} replay mutated state");
        let mut changed = params;
        changed["description"] = json!("changed payload");
        let rejected = call(root, method, changed, Some(method));
        assert_eq!(rejected["ok"], false, "{method}: {rejected}");
        assert!(
            before == logical(),
            "{method} conflicting retry mutated state"
        );
    }
    // JSONL uses the same real dispatch and cannot acquire writes for telemetry.
    let before = snapshot(root);
    let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(["rpc", "--jsonl"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for method in [
        "reference.list",
        "reference.get",
        "cooperation.list",
        "cooperation.get",
        "cooperation.knowledge.query",
    ] {
        writeln!(stdin,"{}",json!({"protocol":"route/1","request_id":method,"method":method,"params":{"reference_id":"r","cooperation_id":"c"},"context":{}})).unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 5);
    assert!(responses.iter().all(|response| response["ok"] == true));
    assert!(before == snapshot(root));
}

#[cfg(windows)]
#[test]
fn junction_ancestor_is_unavailable_without_inspecting_target() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    cli(root, &["init"]);
    let external = tempfile::tempdir().unwrap();
    fs::write(external.path().join("secret.txt"), b"not read").unwrap();
    let link = root.join("linked");
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
    assert!(
        output.status.success(),
        "junction fixture creation failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let result = ok(
        root,
        "cooperation.register",
        json!({"cooperation_id":"linked","locator":"linked/secret.txt","kind":"DOCUMENT","provenance":"fixture"}),
        Some("linked"),
    );
    assert!(result.is_object());
    let resource = ok(
        root,
        "cooperation.get",
        json!({"cooperation_id":"linked"}),
        None,
    );
    assert_eq!(resource["availability"], "UNAVAILABLE");
    assert!(resource["fingerprint"].is_null());
    // Remove just our junction; never recursively delete its external target.
    fs::remove_dir(&link).unwrap();
    assert!(external.path().join("secret.txt").exists());
}

#[test]
fn malformed_metadata_is_preserved_and_reports_store_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    cli(root, &["init"]);
    let registry = root.join(".route/reference/registry.json");
    fs::write(&registry, b"{invalid persistent metadata").unwrap();
    let before = snapshot(root);
    let response = call(root, "reference.list", json!({}), None);
    assert_eq!(response["ok"], false);
    let message = response["error"]["message"].as_str().unwrap();
    assert!(message.contains("registry.json"), "{response}");
    assert!(message.contains("No automatic reset"), "{response}");
    assert!(before == snapshot(root));
}

#[cfg(all(windows, feature = "test-utils"))]
#[test]
#[ignore = "explicit resource benchmark, real independent processes; run with --ignored"]
fn resource_budget_real_binary() {
    use std::{io::Read, os::windows::io::AsRawHandle, time::Instant};
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
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
    let mut rows = vec![];
    for count in [0, 1000, 10000, 1] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        route_basic::ensure_identity(root).unwrap();
        route_basic::development::seed_benchmark_events(root, if count == 1 { 0 } else { count })
            .unwrap();
        let external = tempfile::tempdir().unwrap();
        if count == 1 {
            let path = external.path().join("large.bin");
            fs::File::create(&path)
                .unwrap()
                .set_len(256 * 1024 * 1024)
                .unwrap();
            ok(
                root,
                "cooperation.register",
                json!({"cooperation_id":"large","locator":path,"kind":"DOCUMENT","provenance":"benchmark"}),
                Some("large"),
            );
            ok(
                root,
                "reference.register",
                json!({"reference_id":"large","locator":path}),
                Some("large"),
            );
        }
        for method in [
            "reference.list",
            "cooperation.list",
            "development.events.query",
            "development.state",
        ] {
            let before = snapshot(root);
            let durable: usize = before.values().map(|v| v.0.len()).sum();
            let mut elapsed = vec![];
            let mut peak = 0;
            let mut result_size = 0;
            for _ in 0..20 {
                let start = Instant::now();
                let mut child = Command::new(env!("CARGO_BIN_EXE_route"))
                    .arg("rpc")
                    .current_dir(root)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap();
                child.stdin.take().unwrap().write_all(json!({"protocol":"route/1","request_id":"budget","method":method,"params":{"limit":100},"context":{}}).to_string().as_bytes()).unwrap();
                let mut stdout = child.stdout.take().unwrap();
                let reader = std::thread::spawn(move || {
                    let mut bytes = vec![];
                    stdout.read_to_end(&mut bytes).unwrap();
                    bytes
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
                let bytes = reader.join().unwrap();
                elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
                result_size = bytes.len();
                let response: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(response["ok"], true, "{response}");
                if method == "development.events.query" {
                    assert_eq!(
                        response["result"]["global_revision"],
                        if count == 1 { 2 } else { count }
                    );
                }
            }
            elapsed.sort_by(f64::total_cmp);
            assert!(before == snapshot(root), "benchmark read mutated state");
            rows.push(json!({"dataset":if count==1 {"LARGE".to_string()} else {format!("E{count}")},"external_bytes":if count==1 {256*1024*1024} else {0},"events":if count==1 {2} else {count},"method":method,"runs":20,"p50_ms":elapsed[9],"p95_ms":elapsed[18],"peak_working_set_bytes":peak,"durable_bytes":durable,"durable_growth":0,"result_bytes":result_size,"bytes_read_written":"NOT_MEASURED","correctness":"PASS"}));
        }
    }
    let report = json!({"binary_sha256":route_core::sha256_hex(&fs::read(env!("CARGO_BIN_EXE_route")).unwrap()),"binary_bytes":fs::metadata(env!("CARGO_BIN_EXE_route")).unwrap().len(),"rows":rows});
    println!("RESOURCE_BUDGET={report}");
    if let Ok(path) = std::env::var("ROUTE_BUDGET_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
