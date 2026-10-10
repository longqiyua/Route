//! Non-code general work through independent Route processes and authenticated Workers.
use serde_json::{json, Value};
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};

fn rpc(root: &Path, method: &str, params: Value, key: Option<&str>, secret: Option<&str>) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_route"));
    command
        .args(["rpc"])
        .current_dir(root)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if key.is_some() && secret.is_none() {
        command.arg("--operator");
    }
    if let Some(secret) = secret {
        command.env("ROUTE_WORKER_CREDENTIAL", secret);
    }
    let mut child = command.spawn().unwrap();
    let input = json!({"protocol":"route/1","request_id":format!("general-{method}"),"method":method,"context":{},"params":params,"idempotency_key":key});
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
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
    ok(rpc(root, method, params, Some(key), None))
}
fn worker(root: &Path, method: &str, params: Value, key: &str, secret: &str) -> Value {
    ok(rpc(root, method, params, Some(key), Some(secret)))
}
fn rev(root: &Path) -> u64 {
    route_basic::development::global_development_revision(root).unwrap()
}
fn bytes_under(root: &Path) -> u64 {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                bytes_under(&path)
            } else {
                path.metadata().unwrap().len()
            }
        })
        .sum()
}
fn step(id: &str, requirement: &str, dependencies: Vec<&str>) -> Value {
    json!({"step_id":id,"title":format!("Plan {id}"),"requirement":requirement,"dependencies":dependencies,"proof":{"check_id":format!("plan-{id}")}})
}
#[test]
fn two_workers_world_change_and_truthful_general_goal() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .current_dir(root)
        .output()
        .unwrap()
        .status
        .success());
    let initial_bytes = bytes_under(root);
    let methods = ok(rpc(root, "system.hello", json!({}), None, None))["capabilities"]
        .as_array()
        .unwrap()
        .clone();
    for method in [
        "goal.create",
        "plan.create",
        "assistant.status",
        "plan.review",
        "plan.verify_step",
        "observation.record",
        "decision.request",
        "artifact.register",
        "outcome.record",
    ] {
        assert!(
            methods.iter().any(|item| item == method),
            "not advertised: {method}"
        );
    }
    let mut secrets = Vec::new();
    for id in ["planner-a", "planner-b"] {
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
            &format!("binding-{id}"),
        );
        secrets.push(secret);
    }
    let created = op(
        root,
        "goal.create",
        json!({"expected_revision":rev(root),"goal":{"title":"30-day product launch plan","description":"Non-code go-to-market planning","domain":"PLANNING","constraint_refs":[],"reference_refs":[]}}),
        "goal",
    );
    let goal = created["event"]["payload"]["data"]["action"]["change"]["goal"]["goal_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        rpc(
            root,
            "goal.close",
            json!({"expected_revision":rev(root),"goal_id":goal,"state":"SUCCEEDED","reason":"premature"}),
            Some("premature"),
            None
        )["ok"],
        false
    );
    let steps = vec![
        step("A", "REQUIRED", vec![]),
        step("B", "REQUIRED", vec!["A"]),
        step("C", "REQUIRED", vec!["B"]),
        step("D", "REQUIRED", vec!["C"]),
        step("E", "REQUIRED", vec!["D"]),
        json!({"step_id":"F","title":"Optional venue activation if permit arrives","requirement":"CONDITIONAL","dependencies":[],"condition":{"kind":"PATH_EXISTS","path":"permit-approved.txt"},"proof":{"check_id":"plan-F"}}),
        step("G", "OPTIONAL", vec![]),
    ];
    op(
        root,
        "workflow.create",
        json!({"expected_revision":rev(root),"spec":{"workflow_id":"launch-workflow","intent_ref":goal,"title":"Product launch plan","mode":"CONTROLLED","steps":steps}}),
        "workflow",
    );
    op(
        root,
        "plan.create",
        json!({"expected_revision":rev(root),"plan":{"goal_id":goal,"workflow_id":"launch-workflow","workflow_version":1,"assumptions":["supplier delivery Day 10"],"unknowns":["final budget"],"constraints":["30-day horizon"],"milestones":["research","budget","launch"],"risks":["supplier delay"],"review_conditions":["supplier date changes"],"observation_refs":[]}}),
        "plan-v1",
    );
    assert_eq!(
        ok(rpc(
            root,
            "assistant.status",
            json!({"goal_id":goal}),
            None,
            None
        ))["required_satisfied"],
        0
    );
    // These are real independent Worker processes; neither uses Operator authority.
    let mut works = Vec::new();
    let mut claims = Vec::new();
    for (index, id) in ["A", "B", "C", "D", "E", "G"].iter().enumerate() {
        let secret = &secrets[index % 2];
        let created = worker(
            root,
            "work.create_child",
            json!({"intent_ref":goal,"goal_id":goal,"workflow_step_id":id,"title":format!("Produce {id} planning deliverable"),"kind":"GENERAL","scope_paths":[],"verification_requirements":[],"overlap_mode":"EXCLUSIVE"}),
            &format!("work-{id}"),
            secret,
        );
        let work_id = created["event"]["payload"]["data"]["action"]["work"]["work_id"]
            .as_str()
            .unwrap()
            .to_string();
        let claim = worker(
            root,
            "work.claim",
            json!({"work_id":work_id}),
            &format!("claim-{id}"),
            secret,
        );
        works.push(work_id);
        claims.push(
            claim["event"]["payload"]["data"]["action"]["claim"]["claim_id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    let decision = worker(
        root,
        "decision.request",
        json!({"expected_revision":rev(root),"decision":{"goal_id":goal,"question":"Approve budget ceiling?","reason":"Budget affects channel mix","blocked_work_ids":[works[2]],"options":["low","high"],"context_refs":[],"step_id":"C"}}),
        "budget-decision",
        &secrets[0],
    );
    let decision_id = decision["event"]["payload"]["data"]["action"]["change"]["decision"]
        ["decision_id"]
        .as_str()
        .unwrap()
        .to_string();
    let pending = ok(rpc(
        root,
        "assistant.status",
        json!({"goal_id":goal}),
        None,
        None,
    ));
    assert_eq!(pending["needs_human"], true);
    assert_eq!(
        rpc(
            root,
            "decision.respond",
            json!({"expected_revision":rev(root),"decision_id":decision_id,"answer":"low","reason":"Worker cannot decide"}),
            Some("worker-answer"),
            Some(&secrets[0])
        )["ok"],
        false
    );
    op(
        root,
        "decision.respond",
        json!({"expected_revision":rev(root),"decision_id":decision_id,"answer":"low","reason":"Approved by Operator"}),
        "answer",
    );
    let observation = worker(
        root,
        "observation.record",
        json!({"expected_revision":rev(root),"observation":{"goal_id":goal,"summary":"Supplier delivery moves from Day 10 to Day 17","affected_step_ids":["D"],"affected_work_ids":[works[3]],"source_refs":["supplier-notice"]}}),
        "supplier-delay",
        &secrets[1],
    );
    let observation_id = observation["event"]["payload"]["data"]["action"]["change"]["observation"]
        ["observation_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        ok(rpc(
            root,
            "plan.review",
            json!({"goal_id":goal}),
            None,
            None
        ))["affected_steps"],
        json!(["D"])
    );
    let mut revised = vec![
        step("A", "REQUIRED", vec![]),
        step("B", "REQUIRED", vec!["A"]),
        step("C", "REQUIRED", vec!["B"]),
        step("D", "REQUIRED", vec!["C"]),
        step("E", "REQUIRED", vec!["D"]),
        json!({"step_id":"F","title":"Optional venue activation if permit arrives","requirement":"CONDITIONAL","dependencies":[],"condition":{"kind":"PATH_EXISTS","path":"permit-approved.txt"},"proof":{"check_id":"plan-F"}}),
        step("G", "OPTIONAL", vec![]),
    ];
    revised[3]["title"] = json!("Supplier-dependent activation after Day 17");
    revised[4]["title"] = json!("Launch sequence after delayed activation");
    let delta = worker(
        root,
        "workflow.plan_delta.propose",
        json!({"expected_revision":rev(root),"delta":{"workflow_id":"launch-workflow","from_version":1,"reason":"Supplier Day 17 changes launch sequencing","proposed_steps":revised}}),
        "delta",
        &secrets[1],
    );
    let delta_id = delta["event"]["payload"]["data"]["action"]["change"]["delta"]["delta_id"]
        .as_str()
        .unwrap()
        .to_string();
    op(
        root,
        "workflow.plan_delta.accept",
        json!({"expected_revision":rev(root),"delta_id":delta_id}),
        "delta-accept",
    );
    op(
        root,
        "plan.create",
        json!({"expected_revision":rev(root),"plan":{"goal_id":goal,"workflow_id":"launch-workflow","workflow_version":2,"assumptions":["supplier delivery Day 17"],"unknowns":[],"constraints":["30-day horizon"],"milestones":["research","budget","revised launch"],"risks":["supplier delay"],"review_conditions":["supplier date changes again"],"observation_refs":[observation_id],"source_delta_id":delta_id}}),
        "plan-v2",
    );
    assert_eq!(
        ok(rpc(root, "plan.get", json!({"goal_id":goal}), None, None))["plans"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let versioned = ok(rpc(
        root,
        "workflow.get",
        json!({"workflow_id":"launch-workflow"}),
        None,
        None,
    ));
    assert_eq!(versioned["versions"][0]["steps"][3]["title"], "Plan D");
    assert_eq!(
        versioned["spec"]["steps"][3]["title"],
        "Supplier-dependent activation after Day 17"
    );
    assert_eq!(
        ok(rpc(
            root,
            "plan.review",
            json!({"goal_id":goal}),
            None,
            None
        ))["unaddressed_observations"],
        json!([])
    );
    let mut artifacts = Vec::new();
    for (index, id) in ["A", "B", "C", "D", "E", "G"].iter().enumerate() {
        let path = format!("launch-{id}.md");
        let contents = format!("# Deliverable {id}\nSupplier Day 17; budget low.\n");
        std::fs::write(root.join(&path), contents.as_bytes()).unwrap();
        let artifact = worker(
            root,
            "artifact.register",
            json!({"expected_revision":rev(root),"artifact":{"goal_id":goal,"step_id":id,"path":path,"sha256":route_core::sha256_hex(contents.as_bytes()),"state":"DRAFT","description":format!("Draft {id}")}}),
            &format!("draft-{id}"),
            &secrets[index % 2],
        );
        artifacts.push(
            artifact["event"]["payload"]["data"]["action"]["change"]["artifact"]["artifact_id"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }
    worker(
        root,
        "outcome.record",
        json!({"expected_revision":rev(root),"outcome":{"goal_id":goal,"summary":"Draft plan, pending verification","state":"DRAFT","artifact_refs":artifacts,"evidence_refs":[]}}),
        "draft-outcome",
        &secrets[0],
    );
    assert_eq!(
        rpc(
            root,
            "artifact.register",
            json!({"expected_revision":rev(root),"artifact":{"goal_id":goal,"path":"launch-A.md","sha256":route_core::sha256_hex(std::fs::read(root.join("launch-A.md")).unwrap().as_slice()),"state":"FINAL","description":"premature"}}),
            Some("premature-final"),
            None
        )["ok"],
        false
    );
    for (index, id) in ["A", "B", "C", "D", "E", "G"].iter().enumerate() {
        worker(
            root,
            "work.finish",
            json!({"claim_id":claims[index],"reason":format!("Draft {id} complete")}),
            &format!("finish-{id}"),
            &secrets[index % 2],
        );
    }
    let path = "launch-final.md";
    let contents = b"# 30-day launch plan\nSupplier Day 17; low budget.\n";
    std::fs::write(root.join(path), contents).unwrap();
    let denied = ok(rpc(
        root,
        "workflow.complete.check",
        json!({"workflow_id":"launch-workflow"}),
        None,
        None,
    ));
    assert_eq!(denied["completion"], "DENIED");
    let mut proofs = Vec::new();
    for id in ["A", "B", "C", "D", "E"] {
        let params = json!({"goal_id":goal,"step_id":id,"expected_revision":rev(root)});
        let result = op(
            root,
            "plan.verify_step",
            params.clone(),
            &format!("verify-{id}"),
        );
        let replay = op(root, "plan.verify_step", params, &format!("verify-{id}"));
        assert_eq!(result["evidence_ref"], replay["evidence_ref"]);
        let proof = result["evidence_ref"].as_str().unwrap().to_string();
        worker(
            root,
            "workflow.step.complete",
            json!({"expected_revision":rev(root),"workflow_id":"launch-workflow","version":2,"step_id":id,"evidence_ref":proof}),
            &format!("complete-{id}"),
            &secrets[0],
        );
        proofs.push(proof);
    }
    worker(
        root,
        "workflow.step.skip",
        json!({"expected_revision":rev(root),"workflow_id":"launch-workflow","version":2,"step_id":"F","reason":"Permit absent; conditional step inactive"}),
        "skip-F",
        &secrets[1],
    );
    op(
        root,
        "work.integrate",
        json!({"intent_ref":goal,"accepted_claim_ids":claims,"evidence_refs":proofs,"state_hash":route_basic::execution::compute_state_hash(root).unwrap(),"expected_revision":rev(root)}),
        "integrate",
    );
    assert_eq!(
        ok(rpc(
            root,
            "workflow.complete.check",
            json!({"workflow_id":"launch-workflow"}),
            None,
            None
        ))["completion"],
        "PASS"
    );
    worker(
        root,
        "workflow.complete.request",
        json!({"expected_revision":rev(root),"workflow_id":"launch-workflow"}),
        "workflow-complete",
        &secrets[0],
    );
    assert_eq!(
        ok(rpc(
            root,
            "plan.review",
            json!({"goal_id":goal}),
            None,
            None
        ))["completion"],
        "PASS"
    );
    let final_artifact = op(
        root,
        "artifact.register",
        json!({"expected_revision":rev(root),"artifact":{"goal_id":goal,"path":path,"sha256":route_core::sha256_hex(contents),"state":"FINAL","description":"Verified final plan"}}),
        "final-artifact",
    );
    let final_id = final_artifact["event"]["payload"]["data"]["action"]["change"]["artifact"]
        ["artifact_id"]
        .as_str()
        .unwrap()
        .to_string();
    op(
        root,
        "outcome.record",
        json!({"expected_revision":rev(root),"outcome":{"goal_id":goal,"summary":"Final launch plan produced","state":"FINAL","artifact_refs":[final_id],"evidence_refs":[]}}),
        "final-outcome",
    );
    op(
        root,
        "goal.close",
        json!({"expected_revision":rev(root),"goal_id":goal,"state":"SUCCEEDED","reason":"Verified and accepted"}),
        "goal-close",
    );
    let done = ok(rpc(
        root,
        "assistant.status",
        json!({"goal_id":goal}),
        None,
        None,
    ));
    assert_eq!(done["goal_state"], "SUCCEEDED");
    assert_eq!(done["required_satisfied"], 6);
    assert_eq!(
        ok(rpc(root, "assistant.status", json!({}), None, None))["goal_state"],
        "SUCCEEDED"
    );
    let cli = Command::new(env!("CARGO_BIN_EXE_route"))
        .args(["assistant", "status", &goal])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    assert!(String::from_utf8_lossy(&cli.stdout).contains("Goal completed"));
    assert!(Command::new(env!("CARGO_BIN_EXE_route"))
        .args(["assistant", "status"])
        .current_dir(root)
        .output()
        .unwrap()
        .status
        .success());
    let ledger_path = route_basic::development::development_ledger_path(root).unwrap();
    let before = std::fs::metadata(&ledger_path).unwrap().len();
    let durable_before_reads = bytes_under(root);
    let mut times = Vec::new();
    let mut plan_times = Vec::new();
    let mut delta_times = Vec::new();
    for _ in 0..12 {
        let start = Instant::now();
        assert_eq!(
            ok(rpc(
                root,
                "assistant.status",
                json!({"goal_id":goal}),
                None,
                None
            ))["goal_state"],
            "SUCCEEDED"
        );
        times.push(start.elapsed().as_millis());
        let start = Instant::now();
        assert_eq!(
            ok(rpc(root, "plan.get", json!({"goal_id":goal}), None, None))["plans"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        plan_times.push(start.elapsed().as_millis());
        let start = Instant::now();
        assert!(ok(rpc(
            root,
            "development.events.query",
            json!({"after_revision":0,"limit":100}),
            None,
            None
        ))["events"]
            .as_array()
            .is_some());
        delta_times.push(start.elapsed().as_millis());
    }
    times.sort_unstable();
    plan_times.sort_unstable();
    delta_times.sort_unstable();
    let after = std::fs::metadata(&ledger_path).unwrap().len();
    assert_eq!(
        before, after,
        "read-only status must not grow durable state"
    );
    assert_eq!(
        durable_before_reads,
        bytes_under(root),
        "read-only RPCs grew durable project state"
    );
    eprintln!("GENERAL_WORK_PERF fresh-process status p50/p95={}/{}ms plan.get={}/{}ms events.query={}/{}ms ledger={}bytes durable_growth={}bytes events={} goals=1 plans=2 work=6 workers=2 decisions=1 artifacts=7 evidence={}",times[6],times[11],plan_times[6],plan_times[11],delta_times[6],delta_times[11],after,bytes_under(root)-initial_bytes,rev(root),route_basic::EvidenceStore::load(root).unwrap().evidence.len());
}

#[test]
fn general_work_authority_and_read_only_status() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    assert!(Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .current_dir(root)
        .output()
        .unwrap()
        .status
        .success());
    op(
        root,
        "worker.register",
        json!({"worker_id":"limited"}),
        "register-limited",
    );
    let secret = route_basic::principal::generate_credential().unwrap();
    let bound = op(
        root,
        "worker.binding.issue",
        json!({"worker_id":"limited","credential_hash":route_core::sha256_hex(secret.as_bytes()),"grants":["observation.record","decision.request","artifact.register","outcome.record"]}),
        "bind-limited",
    );
    let binding_id = bound["event"]["payload"]["data"]["transaction"]["change"]["binding"]
        ["binding_id"]
        .as_str()
        .unwrap()
        .to_string();
    let goal_result = op(
        root,
        "goal.create",
        json!({"expected_revision":rev(root),"goal":{"title":"Plan a community event","description":"Non-code planning","domain":"PLANNING"}}),
        "event-goal",
    );
    let goal = goal_result["event"]["payload"]["data"]["action"]["change"]["goal"]["goal_id"]
        .as_str()
        .unwrap();
    let before = rev(root);
    for _ in 0..3 {
        assert_eq!(
            ok(rpc(
                root,
                "assistant.status",
                json!({"goal_id":goal}),
                None,
                None
            ))["goal"]["goal_id"],
            goal
        );
    }
    assert_eq!(
        rev(root),
        before,
        "read-only status wrote to the global ledger"
    );
    for (method, params) in [
        (
            "goal.close",
            json!({"expected_revision":rev(root),"goal_id":goal,"state":"SUCCEEDED","reason":"spoof"}),
        ),
        (
            "plan.create",
            json!({"expected_revision":rev(root),"plan":{"goal_id":goal,"workflow_id":"forged","workflow_version":1}}),
        ),
        (
            "decision.respond",
            json!({"expected_revision":rev(root),"decision_id":"forged","answer":"yes","reason":"spoof"}),
        ),
        (
            "plan.verify_step",
            json!({"expected_revision":rev(root),"goal_id":goal,"step_id":"A"}),
        ),
    ] {
        assert_eq!(
            rpc(
                root,
                method,
                params,
                Some(&format!("spoof-{method}")),
                Some(&secret)
            )["ok"],
            false,
            "{method}"
        );
    }
    assert_eq!(rev(root), before, "rejected writes mutated the ledger");
    op(
        root,
        "worker.register",
        json!({"worker_id":"no-research"}),
        "register-no-research",
    );
    let no_research = route_basic::principal::generate_credential().unwrap();
    op(
        root,
        "worker.binding.issue",
        json!({"worker_id":"no-research","credential_hash":route_core::sha256_hex(no_research.as_bytes())}),
        "bind-no-research",
    );
    let created = worker(
        root,
        "work.create_child",
        json!({"intent_ref":goal,"goal_id":goal,"title":"Research venue options","kind":"GENERAL","scope_paths":[],"verification_requirements":[],"required_capabilities":["research"],"overlap_mode":"EXCLUSIVE"}),
        "research-work",
        &no_research,
    );
    let work_id = created["event"]["payload"]["data"]["action"]["work"]["work_id"]
        .as_str()
        .unwrap();
    assert_eq!(
        rpc(
            root,
            "work.claim",
            json!({"work_id":work_id}),
            Some("claim-no-research"),
            Some(&no_research)
        )["ok"],
        false
    );
    op(
        root,
        "worker.register",
        json!({"worker_id":"researcher","metadata":{"capabilities":{"research":"planning"}}}),
        "register-researcher",
    );
    let researcher = route_basic::principal::generate_credential().unwrap();
    op(
        root,
        "worker.binding.issue",
        json!({"worker_id":"researcher","credential_hash":route_core::sha256_hex(researcher.as_bytes())}),
        "bind-researcher",
    );
    worker(
        root,
        "work.claim",
        json!({"work_id":work_id}),
        "claim-researcher",
        &researcher,
    );
    let other = tempfile::tempdir().unwrap();
    assert!(Command::new(env!("CARGO_BIN_EXE_route"))
        .arg("init")
        .current_dir(other.path())
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(
        rpc(
            other.path(),
            "observation.record",
            json!({"expected_revision":rev(other.path()),"observation":{"goal_id":goal,"summary":"cross-project","affected_step_ids":[],"affected_work_ids":[],"source_refs":[]}}),
            Some("cross-project"),
            Some(&secret)
        )["ok"],
        false
    );
    op(
        root,
        "worker.binding.revoke",
        json!({"binding_id":binding_id}),
        "revoke-limited",
    );
    assert_eq!(
        rpc(
            root,
            "observation.record",
            json!({"expected_revision":rev(root),"observation":{"goal_id":goal,"summary":"after revoke","affected_step_ids":[],"affected_work_ids":[],"source_refs":[]}}),
            Some("revoked"),
            Some(&secret)
        )["ok"],
        false
    );
}
