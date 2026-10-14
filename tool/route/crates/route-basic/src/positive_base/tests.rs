use super::*;
use crate::{
    general_work,
    passive_base::Lesson,
    principal::{self, CallerContext},
    team::{self, Role, TeamAction},
};

const COMPONENT: &str = "src/recovery.rs";
fn env() -> Vec<String> {
    vec![if cfg!(windows) {
        "Windows"
    } else {
        std::env::consts::OS
    }
    .into()]
}
fn isolate(name: &str) -> bool {
    if std::env::args().any(|a| a == "positive-chain-isolated") {
        return false;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("positive_base::tests::{name}"),
            "--nocapture",
            "--skip",
            "positive-chain-isolated",
        ])
        .status()
        .unwrap();
    assert!(status.success(), "isolated fixture failed");
    true
}
fn worker(root: &Path, id: &str) -> CallerContext {
    let op = CallerContext::trusted_operator();
    principal::register_worker(
        root,
        &op,
        Some(id.into()),
        Default::default(),
        &format!("register-{id}"),
    )
    .unwrap();
    let secret = principal::generate_credential().unwrap();
    principal::issue(
        root,
        &op,
        id,
        &route_core::sha256_hex(secret.as_bytes()),
        vec![
            "work.create_child".into(),
            "development.event.record".into(),
        ],
        &format!("bind-{id}"),
    )
    .unwrap();
    CallerContext::authenticate(root, &secret).unwrap()
}
fn finding(
    root: &Path,
    caller: &CallerContext,
    goal: &str,
    work: &str,
    text: String,
    key: &str,
) -> String {
    principal::worker_action(root,caller,"development.event.record",json!({"intent_ref":goal,"task_ref":work,"payload":Payload::Finding {summary:text,source_refs:vec![]}}),key).unwrap().event.event_id
}
fn check(root: &Path, goal: &str, nonce: &str) -> (crate::execution::CommandEvidence, String) {
    let argv = vec![
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        "--ignored".into(),
        "--exact".into(),
        "positive_base::tests::source_check_helper".into(),
        "--skip".into(),
        nonce.into(),
    ];
    crate::execution::exec_command_with_evidence(root, goal, &argv, Some("recovery-source-check"))
        .unwrap()
}
#[test]
#[ignore = "controlled subprocess reads actual fixture source; never synthesizes System Evidence"]
fn source_check_helper() {
    assert_eq!(
        std::fs::read_to_string(COMPONENT).unwrap(),
        "recovery = verified\n"
    );
}
struct Fixture {
    tmp: tempfile::TempDir,
    goal: String,
    work: String,
    main: CallerContext,
    reviewer: CallerContext,
    record: Record,
}
fn fixture() -> Fixture {
    fixture_in(None)
}
fn fixture_in(parent: Option<&Path>) -> Fixture {
    let tmp = if let Some(parent) = parent {
        tempfile::Builder::new()
            .prefix("source-backed-")
            .tempdir_in(parent)
            .unwrap()
    } else {
        tempfile::tempdir().unwrap()
    };
    let root = tmp.path();
    crate::repository::BasicRepository::init(root).unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    std::fs::write(root.join(COMPONENT), "recovery = broken\n").unwrap();
    let op = CallerContext::trusted_operator();
    let revision = development::global_development_revision(root).unwrap();
    let created = general_work::operator_action(root,&op,"goal.create",json!({"expected_revision":revision,"goal":{"title":"Controlled Route recovery fixture, not AI synthesis","domain":"DEVELOPMENT"}}),"goal").unwrap();
    let Payload::GeneralWork { action } = created.event.payload else {
        panic!("goal")
    };
    let general_work::GeneralChange::GoalCreated { goal: g } = action.change else {
        panic!("goal")
    };
    let goal = g.goal_id;
    let main = worker(root, "main-fixture");
    let _knowledge = worker(root, "knowledge-fixture");
    let reviewer = worker(root, "review-fixture");
    let created = crate::work::worker_action(root,&main,"work.create_child",json!({"intent_ref":goal,"title":"Fix recovery source fixture","kind":"TEST_GAP","scope_paths":[COMPONENT],"dependencies":[],"verification_requirements":["recovery-source-check"],"overlap_mode":"EXCLUSIVE"}),"work").unwrap();
    let Payload::Work {
        action: crate::work::WorkAction::ChildCreated { work: w },
    } = created.event.payload
    else {
        panic!("work")
    };
    let work = w.work_id;
    let (failed, _) = check(root, &goal, "initial-failure");
    assert_ne!(failed.exit_code, 0);
    finding(root,&main,&goal,&work,format!("RECOVERY: {} {} controlled recovery fixture fails because the source keeps the broken marker; replace only that marker and rerun recovery-source-check",env()[0],COMPONENT),"investigation");
    std::fs::write(root.join(COMPONENT), "recovery = verified\n").unwrap();
    let (passed, _) = check(root, &goal, "verified-fix");
    assert_eq!(passed.exit_code, 0);
    let packet = packets(root, &goal).unwrap().remove(0);
    cooperation::register_cooperation_resource(
        root,
        "recovery-resource".into(),
        COMPONENT.into(),
        cooperation::CooperationKind::new("FILE").unwrap(),
        "Controlled source-backed fixture".into(),
        None,
        vec![],
        vec![],
        vec![],
        None,
        Some("resource".into()),
    )
    .unwrap();
    let resource = cooperation::cooperation_resource(root, "recovery-resource")
        .unwrap()
        .unwrap();
    let record = Record {
        knowledge_id: "KB-fixture-candidate".into(),
        cooperation_id: resource.cooperation_id,
        statement: "Controlled recovery marker replacement passed its exact source check".into(),
        epistemic_status: EpistemicStatus::Inferred,
        provenance: "FIXTURE: real subprocess FAIL/fix/PASS, manual lesson text, no AI synthesis"
            .into(),
        capability_refs: vec![],
        source_refs: packet.source_events.clone(),
        evidence_refs: vec![
            packet.failed_evidence.clone(),
            packet.passed_evidence.clone(),
        ],
        observed_at: 0,
        resource_fingerprint: resource.fingerprint.map(|f| f.value),
        supersedes: None,
        lesson: Some(Lesson {
            state: LessonState::Candidate,
            goal_id: goal.clone(),
            work_id: work.clone(),
            creator_worker_id: "knowledge-fixture".into(),
            finding_event_id: packet.finding.clone(),
            source_event_ids: packet.source_events.clone(),
            review_source: None,
            component: COMPONENT.into(),
            environment: env(),
            condition: format!(
                "{} {} contains the broken recovery marker in the controlled fixture",
                env()[0],
                COMPONENT
            ),
            action:
                "Replace the broken recovery marker with verified only in the controlled fixture"
                    .into(),
            falsification_check:
                "Run recovery-source-check; a broken marker must still exit nonzero".into(),
            positive: Some(PositiveLesson {
                packet,
                synthesis: "FIXTURE".into(),
                kind: "FIX_PATTERN".into(),
                title: "Controlled recovery marker replacement".into(),
                symptoms: vec!["Source check exits nonzero for the broken marker".into()],
                cause_if_established: Some("The fixture source retained the broken marker".into()),
                non_applicability: vec![
                    "Not applicable to real recovery semantics or other components".into(),
                ],
                reuse_evidence: vec![],
            }),
        }),
    };
    Fixture {
        tmp,
        goal,
        work,
        main,
        reviewer,
        record,
    }
}

fn future_work(f: &Fixture) -> String {
    let result=crate::work::worker_action(f.tmp.path(),&f.main,"work.create_child",json!({"intent_ref":f.goal,"title":"Later controlled reuse Work, not original investigation","kind":"TEST_GAP","scope_paths":[COMPONENT],"dependencies":[],"verification_requirements":["recovery-source-check"],"overlap_mode":"EXCLUSIVE"}),"future-reuse-work").unwrap();
    let Payload::Work {
        action: crate::work::WorkAction::ChildCreated { work },
    } = result.event.payload
    else {
        panic!("future work")
    };
    work.work_id
}

#[test]
#[ignore = "explicit audit dogfood only; retains controlled source/Evidence history at the provided destination"]
fn persistent_source_backed_dogfood() {
    let destination = std::env::args()
        .find_map(|a| {
            a.strip_prefix("positive-chain-dogfood-destination=")
                .map(std::path::PathBuf::from)
        })
        .expect("explicit isolated audit destination required");
    std::fs::create_dir_all(&destination).unwrap();
    let f = fixture_in(Some(&destination));
    let root = f.tmp.path();
    let packet = f
        .record
        .lesson
        .as_ref()
        .unwrap()
        .positive
        .as_ref()
        .unwrap()
        .packet
        .clone();
    submit(root, f.record.clone(), "candidate").unwrap();
    let validated = promote(
        &f,
        &f.record,
        LessonState::ValidatedLocal,
        None,
        "validated",
    );
    let future = future_work(&f);
    let context =
        crate::context_pack::build(root, &f.goal, Some(&future), &Role::Implementer).unwrap();
    assert_eq!(context["base_units"].as_array().unwrap().len(), 1);
    let (_, proof) = check(root, &f.goal, "real-reuse-dogfood");
    let reused = promote(
        &f,
        &validated,
        LessonState::ReusedConfirmed,
        Some(proof.clone()),
        "reused",
    );
    let units = query(root, COMPONENT, &env(), false, false).unwrap();
    assert_eq!(units[0]["status"], "REUSED_CONFIRMED");
    let receipt = json!({"scenario":"FRESH_CONTROLLED_SOURCE_FIXTURE_NOT_AI_SYNTHESIS","status":"PASS","goal":f.goal,"work":f.work,"future_work":future,"finding":packet.finding,"failed_evidence":packet.failed_evidence,"passed_evidence":packet.passed_evidence,"reuse_evidence":proof,"source_events":packet.source_events,"source_revision":packet.source_revision,"candidate":f.record.knowledge_id,"validated":validated.knowledge_id,"reused":reused.knowledge_id,"review":reused.lesson.as_ref().unwrap().review_source,"packet_bytes":serde_json::to_vec(&packet).unwrap().len(),"base_context_units":1,"project_path":root,"ledger_path":development::development_ledger_path(root).unwrap(),"knowledge_is_evidence":false,"ai_synthesis":"NOT_RUN"});
    std::fs::write(
        destination.join("dogfood-receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    let kept = f.tmp.keep();
    println!("Retained controlled audit fixture: {}", kept.display());
}
fn promote(
    f: &Fixture,
    old: &Record,
    state: LessonState,
    reuse: Option<String>,
    key: &str,
) -> Record {
    let mut next = old.clone();
    next.knowledge_id = format!("KB-{key}");
    next.supersedes = Some(old.knowledge_id.clone());
    let review=finding(f.tmp.path(),&f.reviewer,&f.goal,&f.work,format!("Independent controlled source review for {}: source and System Evidence inspected; this lesson remains fixture-only",old.knowledge_id),&format!("review-{key}"));
    let l = next.lesson.as_mut().unwrap();
    l.state = state;
    l.review_source = Some(review);
    if let Some(id) = reuse {
        l.positive.as_mut().unwrap().reuse_evidence.push(id)
    }
    submit(f.tmp.path(), next.clone(), key).unwrap();
    next
}
#[test]
fn source_backed_positive_chain_and_bounded_context() {
    if isolate("source_backed_positive_chain_and_bounded_context") {
        return;
    }
    let f = fixture();
    let root = f.tmp.path();
    submit(root, f.record.clone(), "candidate").unwrap();
    assert!(query(root, COMPONENT, &env(), false, false)
        .unwrap()
        .is_empty());
    let validated = promote(
        &f,
        &f.record,
        LessonState::ValidatedLocal,
        None,
        "validated",
    );
    assert_eq!(
        query(root, COMPONENT, &env(), false, false).unwrap()[0]["status"],
        "VALIDATED_LOCAL"
    );
    let future = future_work(&f);
    let context =
        crate::context_pack::build(root, &f.goal, Some(&future), &Role::Implementer).unwrap();
    assert_eq!(context["base_units"].as_array().unwrap().len(), 1);
    let blind =
        crate::context_pack::build(root, &f.goal, Some(&f.work), &Role::IndependentReviewer)
            .unwrap();
    assert!(blind["base_units"].as_array().unwrap().is_empty());
    let (_, proof) = check(root, &f.goal, "actual-reuse-check");
    let reused = promote(
        &f,
        &validated,
        LessonState::ReusedConfirmed,
        Some(proof),
        "reused",
    );
    assert_eq!(
        query(root, COMPONENT, &env(), false, false).unwrap()[0]["successful_reuse_count"],
        1
    );
    let ledger_path = development::development_ledger_path(root).unwrap();
    let before = std::fs::read(&ledger_path).unwrap();
    for _ in 0..10 {
        assert!(query(root, COMPONENT, &env(), true, false)
            .unwrap()
            .is_empty());
        query(root, COMPONENT, &env(), false, false).unwrap();
    }
    assert_eq!(std::fs::read(&ledger_path).unwrap(), before);
    std::fs::write(root.join("unrelated.txt"), "unrelated").unwrap();
    assert_eq!(
        query(root, COMPONENT, &env(), false, false).unwrap().len(),
        1,
        "unrelated source does not stale scoped knowledge"
    );
    std::fs::write(root.join(COMPONENT), "recovery = broken\n").unwrap();
    assert_eq!(
        query(root, COMPONENT, &env(), false, true).unwrap()[0]["status"],
        "STALE"
    );
    check(root, &f.goal, "current-failure-wins");
    assert_eq!(
        query(root, COMPONENT, &env(), false, true).unwrap()[0]["status"],
        "DISPUTED"
    );
    assert!(query(root, COMPONENT, &env(), false, false)
        .unwrap()
        .is_empty());
    let units = cooperation::cooperation_knowledge(root, None).unwrap();
    assert!(units
        .iter()
        .any(|u| u.record.knowledge_id == f.record.knowledge_id));
    assert!(units
        .iter()
        .any(|u| u.record.knowledge_id == reused.knowledge_id));
}

#[test]
fn projection_limits_overlap_and_no_self_validation() {
    if isolate("projection_limits_overlap_and_no_self_validation") {
        return;
    }
    let f = fixture();
    let root = f.tmp.path();
    let mut immediate = f.record.clone();
    immediate.lesson.as_mut().unwrap().state = LessonState::ValidatedLocal;
    assert!(submit(root, immediate, "self-validation").is_err());
    submit(root, f.record.clone(), "candidate").unwrap();
    let mut bad = f.record.clone();
    bad.knowledge_id = "KB-no-independent-review".into();
    bad.supersedes = Some(f.record.knowledge_id.clone());
    bad.lesson.as_mut().unwrap().state = LessonState::ValidatedLocal;
    assert!(submit(root, bad, "no-independent-review").is_err());
    let validated = promote(&f, &f.record, LessonState::ValidatedLocal, None, "reviewed");
    let mut bad = validated.clone();
    bad.knowledge_id = "KB-fake-reuse".into();
    bad.supersedes = Some(validated.knowledge_id.clone());
    bad.lesson.as_mut().unwrap().state = LessonState::ReusedConfirmed;
    assert!(submit(root, bad, "reuse-without-new-evidence").is_err());
    // Projection-only cardinality fixture: never appended as canonical Knowledge.
    let (mut ledger, _, _) = development::load_ledger_readonly(root).unwrap();
    let template = ledger.events.last().unwrap().clone();
    for i in 0..8 {
        let mut event = template.clone();
        event.sequence += i + 1;
        event.event_id = format!("projection-only-{i}");
        let mut record = validated.clone();
        record.knowledge_id = format!("projection-only-{i}");
        record.supersedes = None;
        record
            .lesson
            .as_mut()
            .unwrap()
            .condition
            .push_str(&format!(" overlap variant {i}"));
        event.payload = Payload::CooperationKnowledgeRecorded { record };
        ledger.events.push(event);
    }
    let values = query_events(root, &ledger.events, COMPONENT, &env(), false).unwrap();
    assert!(!values.is_empty() && values.len() <= 3);
    assert!(serde_json::to_vec(&values).unwrap().len() <= 8192);
    assert!(!values[0]["probable_overlap_ids"]
        .as_array()
        .unwrap()
        .is_empty());
    let mut retired = validated.clone();
    retired.knowledge_id = "KB-retired".into();
    retired.supersedes = Some(validated.knowledge_id.clone());
    retired.lesson.as_mut().unwrap().state = LessonState::Retired;
    submit(root, retired.clone(), "retired").unwrap();
    let mut revived = retired.clone();
    revived.knowledge_id = "KB-revived".into();
    revived.supersedes = Some(retired.knowledge_id.clone());
    revived.lesson.as_mut().unwrap().state = LessonState::Candidate;
    assert!(submit(root, revived, "revived").is_err());
    assert_eq!(
        query(root, COMPONENT, &env(), false, true).unwrap()[0]["status"],
        "RETIRED"
    );
}
#[test]
fn attacks_duplicates_contradictions_and_optional_jobs() {
    if isolate("attacks_duplicates_contradictions_and_optional_jobs") {
        return;
    }
    let f = fixture();
    let root = f.tmp.path();
    for (i, text) in [
        "Testing is important.",
        "Understand requirements first.",
        "Review code carefully.",
        "Handle edge cases.",
        "Use good error handling.",
        "Be careful with concurrency.",
        "Use robust error handling.",
        "Always read documentation.",
        "All Rust filesystem errors should be rerun outside sandbox.",
    ]
    .iter()
    .enumerate()
    {
        let mut r = f.record.clone();
        r.statement = (*text).into();
        assert!(submit(root, r, &format!("generic-{i}")).is_err());
    }
    for i in 0..6 {
        let mut r = f.record.clone();
        let l = r.lesson.as_mut().unwrap();
        match i {
            0 => l.condition = "All sandbox problems should retry outside the sandbox".into(),
            1 => l.positive.as_mut().unwrap().packet.source_revision = u64::MAX,
            2 => r.evidence_refs = vec!["unknown".into()],
            3 => l.positive.as_mut().unwrap().packet.finding = "unknown".into(),
            4 => l.creator_worker_id = "main-fixture".into(),
            _ => {
                l.positive.as_mut().unwrap().symptoms =
                    vec!["private fixture secret=do-not-publish".into()]
            }
        }
        assert!(submit(root, r, &format!("attack-{i}")).is_err());
    }
    let status_before = general_work::status(root, &f.goal).unwrap();
    let deferred = defer(root, &f.goal, "unavailable", "no-team").unwrap();
    assert_eq!(deferred["status"], "DEFERRED");
    submit(root, f.record.clone(), "candidate").unwrap();
    let mut duplicate = f.record.clone();
    duplicate.knowledge_id = "KB-duplicate".into();
    assert!(submit(root, duplicate, "duplicate")
        .unwrap_err()
        .to_string()
        .contains("DUPLICATE"));
    let mut conflict = f.record.clone();
    conflict.knowledge_id = "KB-conflict".into();
    conflict.lesson.as_mut().unwrap().action =
        "Reject this recovery marker replacement under the same controlled assumptions".into();
    conflict
        .lesson
        .as_mut()
        .unwrap()
        .positive
        .as_mut()
        .unwrap()
        .kind = "FAILED_APPROACH".into();
    submit(root, conflict, "conflict").unwrap();
    assert!(query(root, COMPONENT, &env(), false, true)
        .unwrap()
        .iter()
        .all(|u| u["status"] == "DISPUTED"));
    let mut excess = f.record.clone();
    excess.knowledge_id = "KB-excess".into();
    excess
        .lesson
        .as_mut()
        .unwrap()
        .condition
        .push_str(" additional distinct condition");
    assert!(submit(root, excess, "excess").is_err());
    let op = CallerContext::trusted_operator();
    team::broker_action(
        root,
        &op,
        TeamAction::Configure {
            goal_id: f.goal.clone(),
            budget: Default::default(),
            host_path: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            host_sha256: route_core::sha256_hex(b"fixture-only"),
        },
        "configure",
    )
    .unwrap();
    assert_eq!(
        defer(root, &f.goal, "optional", "queue").unwrap()["status"],
        "PENDING"
    );
    assert_eq!(
        defer(root, &f.goal, "over-budget", "queue-2").unwrap()["status"],
        "DEFERRED"
    );
    team::broker_action(
        root,
        &op,
        TeamAction::FinishExtraction {
            goal_id: f.goal.clone(),
            job_id: "optional".into(),
            succeeded: false,
        },
        "timeout-or-unavailable",
    )
    .unwrap();
    assert_eq!(
        team::status(root, &f.goal).unwrap().knowledge_jobs["optional"].state,
        "FAILED"
    );
    assert_eq!(
        general_work::status(root, &f.goal)
            .unwrap()
            .review
            .completion,
        status_before.review.completion
    );
}
#[test]
fn hundred_ordinary_successes_zero_knowledge_and_readonly_empty() {
    if isolate("hundred_ordinary_successes_zero_knowledge_and_readonly_empty") {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // Validator fixture only: no source failure exists, even 100 System-marked PASSs cannot trigger.
    crate::ensure_identity(root).unwrap();
    let mut store = EvidenceStore::default();
    for i in 0..100 {
        store.record(
            "normal-goal",
            EvidenceKind::CheckPass,
            EvidenceSource::System,
            route_core::sha256_hex(format!("ordinary-{i}").as_bytes()),
            Default::default(),
        );
    }
    store.save(root).unwrap();
    let before = std::fs::read_dir(root).unwrap().count();
    assert!(packets(root, "normal-goal").unwrap().is_empty());
    assert!(query(root, "", &[], false, true).unwrap().is_empty());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), before);
    assert!(cooperation::cooperation_knowledge(root, None)
        .unwrap()
        .is_empty());
}
