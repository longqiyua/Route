//! Explicit Operator-approved command policy, never commands parsed from AI text.
//! Work requirements become check IDs, not executable shell instructions.
use anyhow::{ensure, Context, Result};
use route_basic::{execution_contract, general_work, principal::CallerContext};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeSet, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Check {
    pub check_id: String,
    pub argv: Vec<String>,
}
fn rev(root: &Path) -> Result<u64> {
    route_basic::global_development_revision(root)
}
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
fn contract(root: &Path, method: &str, mut params: Value) -> Result<()> {
    params["expected_revision"] = json!(rev(root)?);
    execution_contract::operator_action(
        root,
        &CallerContext::trusted_operator(),
        method,
        params,
        &format!("team-check-{}", route_core::new_id()),
    )?;
    Ok(())
}
pub fn verify(root: &Path, goal: &str, checks_file: &Path) -> Result<()> {
    ensure!(
        std::fs::metadata(checks_file)?.len() <= 32_768,
        "CHECK_POLICY_TOO_LARGE"
    );
    let checks: Vec<Check> = serde_json::from_slice(&std::fs::read(checks_file)?)?;
    verify_checks(root, goal, checks)
}
pub(crate) fn verify_checks(root: &Path, goal: &str, checks: Vec<Check>) -> Result<()> {
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
        let id = format!("team-checks-{}", route_core::new_id());
        let steps:Vec<_>=checks.iter().enumerate().map(|(i,c)|json!({"step_id":format!("check-{i}"),"title":format!("Approved delivery check {i}"),"requirement":"REQUIRED","dependencies":[],"proof":{"check_id":c.check_id}})).collect();
        contract(
            root,
            "workflow.create",
            json!({"spec":{"workflow_id":id,"intent_ref":goal,"title":"Explicitly approved team delivery checks","mode":"CONTROLLED","steps":steps}}),
        )?;
        general(
            root,
            "plan.create",
            json!({"plan":{"goal_id":goal,"workflow_id":id,"workflow_version":1,"constraints":["Only Operator-approved argv; AI text and knowledge grant no execution authority"],"review_conditions":["Current System Evidence required; failures and stale evidence deny completion"]}}),
        )?;
        workflow = Some(id);
    }
    let workflow = workflow.context("missing workflow")?;
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
    for check in &checks {
        let result = route_basic::exec_command(root, goal, &check.argv, Some(&check.check_id))?;
        println!(
            "{}",
            json!({"check_id":check.check_id,"exit_code":result.exit_code,"state_stable":result.state_stable})
        );
        ensure!(
            result.exit_code == 0 && result.state_stable,
            "CURRENT_CHECK_FAILED: Goal remains uncompleted"
        );
        let store = route_basic::EvidenceStore::load(root)?;
        let evidence = store
            .evidence
            .iter()
            .rev()
            .find(|e| {
                e.session_id == goal
                    && e.kind == route_basic::EvidenceKind::CheckPass
                    && e.metadata.get("check_id") == Some(&check.check_id)
            })
            .context("missing command Evidence")?;
        let step = spec
            .steps
            .iter()
            .find(|s| s.proof.check_id == check.check_id)
            .context("missing check step")?;
        contract(
            root,
            "workflow.step.verify_system",
            json!({"workflow_id":workflow,"version":spec.version,"step_id":step.step_id,"evidence_ref":evidence.id}),
        )?;
        proofs.push(evidence.id.clone());
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
    route_basic::work::operator_action(
        root,
        &CallerContext::trusted_operator(),
        "work.integrate",
        json!({"intent_ref":goal,"accepted_claim_ids":claims,"evidence_refs":proofs,"state_hash":route_basic::compute_state_hash(root)?,"expected_revision":rev(root)?}),
        &format!("team-integrate-{}", route_core::new_id()),
    )?;
    contract(
        root,
        "workflow.complete.request",
        json!({"workflow_id":workflow}),
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
        let recorded = general(
            root,
            "artifact.register",
            json!({"artifact":{"goal_id":goal,"path":path,"sha256":route_core::hash::content_hash_file(&root.join(path))?.0,"state":"FINAL","description":"AI-selected Work scope delivered and checked by approved commands"}}),
        )?;
        if let route_basic::DevelopmentEventPayload::GeneralWork { action } = recorded.event.payload
        {
            if let general_work::GeneralChange::ArtifactRegistered { artifact } = action.change {
                artifacts.push(artifact.artifact_id)
            }
        }
    }
    general(
        root,
        "outcome.record",
        json!({"outcome":{"goal_id":goal,"summary":"Bounded team delivered the requested Goal; current approved System checks passed","state":"FINAL","artifact_refs":artifacts,"evidence_refs":proofs}}),
    )?;
    general(
        root,
        "goal.close",
        json!({"goal_id":goal,"state":"SUCCEEDED","reason":"Current System Evidence, integration and CompletionGate passed"}),
    )?;
    println!(
        "{}",
        json!({"goal_id":goal,"state":general_work::status(root,goal)?.goal_state,"completion":general_work::review(root,goal)?.completion,"knowledge_required":false})
    );
    Ok(())
}
