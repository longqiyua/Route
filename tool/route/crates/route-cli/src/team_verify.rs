//! Explicit Operator-approved command policy, never commands parsed from AI text.
//! Work requirements become check IDs, not executable shell instructions.
use anyhow::{ensure, Context, Result};
use route_basic::{execution_contract, general_work, principal::CallerContext};
use serde::{Deserialize, Serialize};
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Check {
    pub check_id: String,
    pub argv: Vec<String>,
}
#[cfg(test)]
fn rev(root: &Path) -> Result<u64> {
    route_basic::global_development_revision(root)
}
#[cfg(test)]
fn general(
    root: &Path,
    method: &str,
    mut params: Value,
) -> Result<route_basic::AppendDevelopmentEventResult> {
    params["expected_revision"] = json!(rev(root)?);
    general_work::operator_action(
        root,
        &CallerContext::trusted_operator(),
        method,
        params,
        &format!("team-final-{}", route_core::new_id()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("deliverable.txt"), b"local verification fixture").unwrap();
        route_basic::BasicRepository::init(root).unwrap();
        route_basic::ensure_identity(root).unwrap();
        let created = general(
            root,
            "goal.create",
            json!({"goal":{"title":"Shared approved command regression","domain":"DEVELOPMENT"}}),
        )
        .unwrap();
        let route_basic::DevelopmentEventPayload::GeneralWork { action } = created.event.payload
        else {
            panic!()
        };
        let general_work::GeneralChange::GoalCreated { goal } = action.change else {
            panic!()
        };
        let op = CallerContext::trusted_operator();
        route_basic::principal::register_worker(
            root,
            &op,
            Some("fixture-worker".into()),
            Default::default(),
            "reg",
        )
        .unwrap();
        let secret = route_basic::principal::generate_credential().unwrap();
        route_basic::principal::issue(
            root,
            &op,
            "fixture-worker",
            &route_core::sha256_hex(secret.as_bytes()),
            vec![
                "work.create_child".into(),
                "work.claim".into(),
                "work.finish".into(),
            ],
            "bind",
        )
        .unwrap();
        let caller = CallerContext::authenticate(root, &secret).unwrap();
        let created = route_basic::principal::worker_action(root, &caller, "work.create_child",
            json!({"intent_ref":goal.goal_id,"goal_id":goal.goal_id,"kind":"GENERAL","title":"Fixture deliverable",
                "scope_paths":["deliverable.txt"],"verification_requirements":["label-a","label-b"],"overlap_mode":"EXCLUSIVE"}), "work").unwrap();
        let route_basic::DevelopmentEventPayload::Work {
            action: route_basic::work::WorkAction::ChildCreated { work },
        } = created.event.payload
        else {
            panic!()
        };
        let claimed = route_basic::principal::worker_action(
            root,
            &caller,
            "work.claim",
            json!({"work_id":work.work_id}),
            "claim",
        )
        .unwrap();
        let route_basic::DevelopmentEventPayload::Work {
            action: route_basic::work::WorkAction::ClaimOpened { claim },
        } = claimed.event.payload
        else {
            panic!()
        };
        route_basic::principal::worker_action(
            root,
            &caller,
            "work.finish",
            json!({"claim_id":claim.claim_id,"reason":"fixture finished"}),
            "finish",
        )
        .unwrap();
        route_basic::team::broker_action(
            root,
            &op,
            route_basic::team::TeamAction::Configure {
                goal_id: goal.goal_id.clone(),
                budget: Default::default(),
                host_path: std::env::current_exe()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                host_sha256: "0".repeat(64),
            },
            "configure",
        )
        .unwrap();
        (dir, goal.goal_id)
    }

    fn counted_argv(path: &Path) -> Vec<String> {
        vec![
            std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            "--exact".into(),
            "team_verify::tests::counted_process_fixture".into(),
            "--ignored".into(),
            "--nocapture".into(),
            "--skip".into(),
            path.to_string_lossy().into_owned(),
        ]
    }

    #[test]
    #[ignore = "owned subprocess fixture; counter path passed via libtest skip argument"]
    fn counted_process_fixture() {
        use std::io::Write;
        let args: Vec<_> = std::env::args().collect();
        let position = args.iter().position(|s| s == "--skip").unwrap();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&args[position + 1])
            .unwrap();
        writeln!(file, "run").unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn single_approved_command_runs_once_for_two_requirement_labels() {
        let (dir, goal) = fixture();
        let counter_dir = tempfile::tempdir().unwrap();
        let counter = counter_dir.path().join("physical-invocations.txt");
        let argv = counted_argv(&counter);
        let checks = ["label-a", "label-b"]
            .into_iter()
            .map(|id| Check {
                check_id: id.into(),
                argv: argv.clone(),
            })
            .collect();
        verify_checks(dir.path(), &goal, checks).unwrap();
        assert_eq!(
            std::fs::read_to_string(&counter).unwrap().lines().count(),
            1,
            "one approved command was physically executed more than once"
        );
        assert_eq!(
            general_work::status(dir.path(), &goal).unwrap().goal_state,
            general_work::GoalState::Succeeded
        );
        let evidence = route_basic::execution::EvidenceStore::load(dir.path()).unwrap();
        let a = evidence
            .evidence
            .iter()
            .find(|e| e.metadata.get("check_id").is_some_and(|id| id == "label-a"))
            .unwrap();
        let b = evidence
            .evidence
            .iter()
            .find(|e| e.metadata.get("check_id").is_some_and(|id| id == "label-b"))
            .unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.metadata["check_run_id"], b.metadata["check_run_id"]);
        assert_eq!(a.kind, route_basic::execution::EvidenceKind::CheckPass);
        assert_eq!(b.kind, route_basic::execution::EvidenceKind::CheckPass);
    }

    #[test]
    #[ignore = "owned finalizer subprocess; deterministic crash without Drop"]
    fn finalizer_process_fixture() {
        let args: Vec<_> = std::env::args().collect();
        let data: Value =
            serde_json::from_str(&args[args.iter().position(|s| s == "--skip").unwrap() + 1])
                .unwrap();
        crate::team_verify_journal::CRASH_AT
            .with(|point| *point.borrow_mut() = data["crash"].as_str().map(String::from));
        verify_checks_keyed(
            Path::new(data["root"].as_str().unwrap()),
            data["goal"].as_str().unwrap(),
            serde_json::from_value(data["checks"].clone()).unwrap(),
            data["key"].as_str().unwrap(),
        )
        .unwrap();
    }
    fn finalizer(data: &Value) -> std::process::Command {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "team_verify::tests::finalizer_process_fixture",
            "--ignored",
            "--nocapture",
            "--skip",
            &data.to_string(),
        ]);
        command.stdout(std::process::Stdio::null());
        command
    }
    #[test]
    fn finalization_real_crashes_resume_exact_operation_without_reexecution() {
        for point in [
            "before:work.integrate",
            "after:work.integrate",
            "after:workflow.complete.request",
            "after:artifact.register",
            "after:outcome.record",
            "before:goal.close",
            "after:goal.close",
        ] {
            let (dir, goal) = fixture();
            let external = tempfile::tempdir().unwrap();
            let counter = external.path().join("count");
            let checks: Vec<_> = ["label-a", "label-b"]
                .iter()
                .map(|id| Check {
                    check_id: (*id).into(),
                    argv: counted_argv(&counter),
                })
                .collect();
            let data = json!({"root":dir.path(),"goal":goal,"checks":checks,"key":"recover","crash":point});
            assert_eq!(
                finalizer(&data).status().unwrap().code(),
                Some(23),
                "{point}"
            );
            if point != "after:goal.close" {
                assert_ne!(
                    general_work::status(dir.path(), &goal).unwrap().goal_state,
                    general_work::GoalState::Succeeded
                );
            }
            let before_revision = route_basic::global_development_revision(dir.path()).unwrap();
            let mut resume = data.clone();
            resume["crash"] = json!(null);
            let resume_started = std::time::Instant::now();
            assert!(
                finalizer(&resume).status().unwrap().success(),
                "resume {point}"
            );
            println!(
                "FINALIZATION_RESUME point={point} elapsed_ms={:.3}",
                resume_started.elapsed().as_secs_f64() * 1000.0
            );
            assert_eq!(
                std::fs::read_to_string(&counter).unwrap().lines().count(),
                1,
                "{point}"
            );
            assert_eq!(
                general_work::status(dir.path(), &goal).unwrap().goal_state,
                general_work::GoalState::Succeeded
            );
            assert_eq!(
                general_work::review(dir.path(), &goal).unwrap().completion,
                "PASS"
            );
            let revision = route_basic::global_development_revision(dir.path()).unwrap();
            assert!(revision >= before_revision);
            let mut a = finalizer(&resume).spawn().unwrap();
            let mut b = finalizer(&resume).spawn().unwrap();
            assert!(a.wait().unwrap().success());
            assert!(b.wait().unwrap().success());
            assert_eq!(
                route_basic::global_development_revision(dir.path()).unwrap(),
                revision,
                "replay must not append {point}"
            );
        }
    }
}
pub fn verify(root: &Path, goal: &str, checks_file: &Path) -> Result<()> {
    verify_keyed(root, goal, checks_file, &route_core::new_id())
}
pub fn verify_keyed(root: &Path, goal: &str, checks_file: &Path, key: &str) -> Result<()> {
    ensure!(
        std::fs::metadata(checks_file)?.len() <= 32_768,
        "CHECK_POLICY_TOO_LARGE"
    );
    let checks: Vec<Check> = serde_json::from_slice(&std::fs::read(checks_file)?)?;
    verify_checks_keyed(root, goal, checks, key)
}
pub(crate) fn verify_checks(root: &Path, goal: &str, checks: Vec<Check>) -> Result<()> {
    verify_checks_keyed(root, goal, checks, &route_core::new_id())
}
pub(crate) fn verify_checks_keyed(
    root: &Path,
    goal: &str,
    checks: Vec<Check>,
    key: &str,
) -> Result<()> {
    ensure!(
        !checks.is_empty()
            && checks.len() <= 32
            && checks.iter().all(|c| !c.argv.is_empty()
                && c.argv.len() <= 32
                && c.argv.iter().all(|a| a.len() <= 4096)),
        "INVALID_CHECK_POLICY"
    );
    let ids: BTreeSet<_> = checks.iter().map(|c| c.check_id.as_str()).collect();
    ensure!(ids.len() == checks.len(), "DUPLICATE_CHECK_POLICY");
    let mut journal = crate::team_verify_journal::Journal::begin(
        root,
        goal,
        key,
        route_core::sha256_hex(&serde_json::to_vec(&checks)?),
    )?;
    println!("{}", json!({"verification_operation": key}));
    if journal.done() {
        ensure!(
            general_work::status(root, goal)?.goal_state == general_work::GoalState::Succeeded
                && general_work::review(root, goal)?.completion == "PASS",
            "FINALIZATION_NO_LONGER_VALID"
        );
        route_basic::work::ensure_integrated_before_success(root, goal)?;
        return Ok(());
    }
    if general_work::status(root, goal)?.goal_state == general_work::GoalState::Succeeded {
        return journal.recover_closed(root);
    }
    let team = route_basic::team::status(root, goal)?;
    ensure!(
        !team.cancelled && team.runs.values().all(|r| !r.state.active()),
        "QUIESCENT_TEAM_REQUIRED"
    );
    let view = route_basic::work::available(root)?;
    let works: Vec<_> = view
        .available
        .iter()
        .filter(|w| w.work.intent_ref == goal)
        .collect();
    ensure!(
        !works.is_empty() && works.iter().all(|w| w.state == "COMPLETED_CANDIDATE"),
        "WORK_NOT_COMPLETED"
    );
    ensure!(
        works
            .iter()
            .flat_map(|w| &w.work.verification_requirements)
            .all(|id| ids.contains(id.as_str())),
        "EXPLICIT_CHECK_POLICY_REQUIRED: cover every Work verification requirement"
    );
    let mut workflow = general_work::status(root, goal)?
        .workflow
        .map(|w| w.spec.workflow_id);
    if workflow.is_none() {
        let id = format!("team-checks-{}", journal.identity());
        let steps:Vec<_>=checks.iter().enumerate().map(|(i,c)|json!({"step_id":format!("check-{i}"),"title":format!("Approved delivery check {i}"),"requirement":"REQUIRED","dependencies":[],"proof":{"check_id":c.check_id}})).collect();
        journal.action(
            root,
            "contract",
            "workflow.create",
            json!({"spec":{"workflow_id":id,"intent_ref":goal,"title":"Explicitly approved team delivery checks","mode":"CONTROLLED","steps":steps}}),
            "workflow-create",
        )?;
        workflow = Some(id);
    }
    let workflow = workflow.context("missing workflow")?;
    if general_work::status(root, goal)?.current_plan.is_none() {
        journal.action(
            root,
            "general",
            "plan.create",
            json!({"plan":{"goal_id":goal,"workflow_id":workflow,"workflow_version":1,"constraints":["Only Operator-approved argv; AI text and knowledge grant no execution authority"],"review_conditions":["Current System Evidence required; failures and stale evidence deny completion"]}}),
            "plan-create",
        )?;
    }
    let spec = execution_contract::status(root, &workflow)?.spec;
    ensure!(
        spec.steps.len() == checks.len()
            && spec
                .steps
                .iter()
                .all(|s| ids.contains(s.proof.check_id.as_str())),
        "CHECK_POLICY_CONTRACT_MISMATCH"
    );
    let mut proofs = vec![];
    let mut groups: BTreeMap<Vec<String>, Vec<String>> = BTreeMap::new();
    for check in &checks {
        groups
            .entry(check.argv.clone())
            .or_default()
            .push(check.check_id.clone());
    }
    let operation = journal.identity().to_string();
    let policy_hash = route_core::sha256_hex(&serde_json::to_vec(&(&spec, &checks))?);
    let mut results = BTreeMap::new();
    for (argv, labels) in groups {
        let contract = route_basic::check_run::CheckContract::approved(
            root,
            argv.clone(),
            labels,
            policy_hash.clone(),
        )?;
        let key = format!(
            "{operation}:{}",
            route_core::sha256_hex(&serde_json::to_vec(&argv)?)
        );
        let run = route_basic::check_run::execute(root, goal, &key, contract)?;
        let result = run.result.context("CHECKRUN_RESULT_MISSING")?;
        for (label, evidence_id) in run.evidence_ids {
            results.insert(label, (result.clone(), evidence_id));
        }
    }
    for check in &checks {
        let (result, evidence_id) = results
            .remove(&check.check_id)
            .context("MISSING_OBLIGATION_COVERAGE")?;
        println!(
            "{}",
            json!({"check_id":check.check_id,"exit_code":result.exit_code,"state_stable":result.state_stable})
        );
        ensure!(
            result.exit_code == 0 && result.state_stable,
            "CURRENT_CHECK_FAILED: Goal remains uncompleted"
        );
        let step = spec
            .steps
            .iter()
            .find(|s| s.proof.check_id == check.check_id)
            .context("missing check step")?;
        journal.action(
            root,
            "contract",
            "workflow.step.verify_system",
            json!({"workflow_id":workflow,"version":spec.version,"step_id":step.step_id,"evidence_ref":evidence_id}),
            &format!("step:{}", check.check_id),
        )?;
        proofs.push(evidence_id);
    }
    let claims: Vec<_> = works
        .iter()
        .flat_map(|w| {
            w.claims
                .iter()
                .filter(|c| c.state == route_basic::work::ClaimState::Completed)
                .map(|c| c.claim_id.clone())
        })
        .collect();
    journal.action(
        root,
        "work",
        "work.integrate",
        json!({"intent_ref":goal,"accepted_claim_ids":claims,"evidence_refs":proofs,"state_hash":route_basic::compute_state_hash(root)?}),
        "integrate",
    )?;
    journal.action(
        root,
        "contract",
        "workflow.complete.request",
        json!({"workflow_id":workflow}),
        "workflow-complete",
    )?;
    ensure!(
        general_work::review(root, goal)?.completion == "PASS",
        "COMPLETION_DENIED"
    );
    let paths: BTreeSet<_> = works
        .iter()
        .flat_map(|w| &w.work.scope_paths)
        .filter(|p| root.join(p).is_file())
        .collect();
    ensure!(!paths.is_empty(), "PUBLISHED_ARTIFACT_REQUIRED");
    let mut artifacts = vec![];
    for path in paths {
        let recorded = journal.action(
            root,
            "general",
            "artifact.register",
            json!({"artifact":{"goal_id":goal,"path":path,"sha256":route_core::hash::content_hash_file(&root.join(path))?.0,"state":"FINAL","description":"AI-selected Work scope delivered and checked by approved commands"}}),
            &format!("artifact:{path}"),
        )?;
        if let route_basic::DevelopmentEventPayload::GeneralWork { action } = recorded.event.payload
        {
            if let general_work::GeneralChange::ArtifactRegistered { artifact } = action.change {
                artifacts.push(artifact.artifact_id)
            }
        }
    }
    journal.action(
        root,
        "general",
        "outcome.record",
        json!({"outcome":{"goal_id":goal,"summary":"Bounded team delivered the requested Goal; current approved System checks passed","state":"FINAL","artifact_refs":artifacts,"evidence_refs":proofs}}),
        "outcome",
    )?;
    journal.action(
        root,
        "general",
        "goal.close",
        json!({"goal_id":goal,"state":"SUCCEEDED","reason":"Current System Evidence, integration and CompletionGate passed"}),
        "goal-close",
    )?;
    journal.finish()?;
    println!(
        "{}",
        json!({"goal_id":goal,"state":general_work::status(root,goal)?.goal_state,"completion":general_work::review(root,goal)?.completion,"knowledge_required":false})
    );
    Ok(())
}
