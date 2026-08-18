//! End-to-end tests for Task ↔ EvolutionCampaign ↔ Harness integration (P0–P12).
//!
//! These are real E2E tests: they invoke the built `route` binary. Each CLI
//! invocation is a fresh process, so "reopen"/"crash" recovery is naturally
//! exercised by reading persisted state in a later process.
//!
//! Pure-logic scenarios that the CLI cannot drive (capability degradation,
//! trust advancement, abort semantics) are covered by unit tests in
//! `crates/route-basic/src/campaign.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde_json::Value;
use tempfile::TempDir;

fn route_binary() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let base = PathBuf::from(manifest_dir)
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("route");
    let exe = base.with_extension("exe");
    if exe.exists() {
        exe
    } else {
        base
    }
}

fn run(route: &Path, cwd: &Path, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(route)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run route binary");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn must_run(route: &Path, cwd: &Path, args: &[&str], what: &str) -> String {
    let (ok, stdout, stderr) = run(route, cwd, args);
    assert!(ok, "{what} failed\nstdout: {stdout}\nstderr: {stderr}");
    stdout
}

fn json(run: (bool, String, String), what: &str) -> Value {
    let (ok, stdout, stderr) = run;
    assert!(ok, "{what} failed\nstdout: {stdout}\nstderr: {stderr}");
    serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{what} invalid JSON: {e}\n{stdout}"))
}

/// Start a task and return its full session id.
fn start_task(route: &Path, cwd: &Path) -> String {
    let stdout = must_run(
        route,
        cwd,
        &["task", "start", "campaign e2e task", "--target", "generic"],
        "task start",
    );
    stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("id:"))
        .map(|s| s.trim().to_string())
        .expect("task start stdout should contain session id")
}

/// Start a task bound to a campaign id and return its full session id.
fn start_task_with_campaign(route: &Path, cwd: &Path, campaign_id: &str) -> String {
    let stdout = must_run(
        route,
        cwd,
        &[
            "task",
            "start",
            "campaign bound task",
            "--target",
            "generic",
            "--campaign-id",
            campaign_id,
        ],
        "task start with campaign",
    );
    stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("id:"))
        .map(|s| s.trim().to_string())
        .expect("task start stdout should contain session id")
}

fn create_campaign(route: &Path, cwd: &Path, task_id: Option<&str>) -> String {
    let mut args = vec!["evolve", "campaign", "create", "--goal", "g", "--scope", "s", "--strategy", "st"];
    if let Some(t) = task_id {
        args.push("--task-id");
        args.push(t);
    }
    let stdout = must_run(route, cwd, &args, "campaign create");
    stdout
        .trim()
        .strip_prefix("created campaign ")
        .map(|s| s.trim().to_string())
        .expect("campaign create stdout should contain id")
}

fn report_json(campaign_id: &str, experiment_id: &str, status: &str, harness: &str) -> String {
    format!(
        r#"{{"campaign_id":"{campaign_id}","experiment_id":"{experiment_id}","execution_status":"{status}","harness":"{harness}","evidence_refs":["ev-{experiment_id}"]}}"#
    )
}

// ---------------------------------------------------------------------------
// 1. Normal Task without a Campaign is unchanged.
// ---------------------------------------------------------------------------
#[test]
fn normal_task_without_campaign_unchanged() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(()); // skip if binary not built
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let session = start_task(&route, tmp.path());
    assert!(!session.is_empty());
    // A task works fine with no campaign at all.
    let (ok, _, _) = run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "status", "does-not-exist", "--json"],
    );
    // No campaign exists; looking one up is a clean error, not a panic.
    assert!(!ok, "campaign status for a missing id must fail cleanly");
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. Task → Campaign association persists bidirectionally.
// ---------------------------------------------------------------------------
#[test]
fn task_campaign_association_persists() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let session = start_task(&route, tmp.path());
    let campaign = create_campaign(&route, tmp.path(), Some(&session));

    // Campaign side: status JSON carries the parent task id.
    let status = json(
        run(&route, tmp.path(), &["evolve", "campaign", "status", &campaign, "--json"]),
        "campaign status",
    );
    assert_eq!(status["task_id"], Value::String(session.clone()));

    // End the first session so a new one can be started while bound to a campaign.
    must_run(
        &route,
        tmp.path(),
        &["task", "end", &session, "--result", "success"],
        "end first session",
    );

    // Task side: a task started with --campaign-id exposes the link on the
    // session, so the association is bidirectional.
    let bound_session = start_task_with_campaign(&route, tmp.path(), &campaign);
    let stdout = must_run(
        &route,
        tmp.path(),
        &["task", "show", &bound_session],
        "task show",
    );
    assert!(
        stdout.contains(&campaign),
        "task show should expose the campaign id: {stdout}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. NextAction carries a complete, machine-readable contract.
// ---------------------------------------------------------------------------
#[test]
fn nextaction_contains_executable_contract() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let campaign = create_campaign(&route, tmp.path(), None);

    let next = json(
        run(&route, tmp.path(), &["evolve", "campaign", "next", &campaign, "--json"]),
        "campaign next",
    );
    assert_eq!(next["campaign_id"], Value::String(campaign.clone()));
    assert!(next.get("next_action").is_some());
    assert!(next.get("required_capabilities").is_some());
    assert!(next.get("context_required").is_some());
    assert!(next.get("checkpoint_required").is_some());
    assert!(next.get("candidate_isolation_required").is_some());
    assert!(next.get("expected_evidence").is_some());
    assert!(next.get("budget_remaining_experiments").is_some());
    assert!(next.get("report_contract").is_some());
    // The contract always states satisfiability rather than silently assuming.
    assert!(next.get("satisfiable").is_some());
    assert!(next.get("missing_capabilities").is_some());
    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Insufficient host capabilities → explicit degradation (machine surface).
//    (The exact degrade-to-NEEDS_HUMAN logic is unit-tested in campaign.rs.)
// ---------------------------------------------------------------------------
#[test]
fn nextaction_contract_surface_reports_satisfiability() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let campaign = create_campaign(&route, tmp.path(), None);

    // Request with a capability set that cannot satisfy the action.
    let next = json(
        run(
            &route,
            tmp.path(),
            &["evolve", "campaign", "next", &campaign, "--json", "--cap", "shell"],
        ),
        "campaign next with single cap",
    );
    // The contract must explicitly report satisfiability + any missing caps.
    assert!(next["satisfiable"].is_boolean());
    assert!(next["missing_capabilities"].is_array());
    Ok(())
}

// ---------------------------------------------------------------------------
// 5. A harness "success" report is UNTRUSTED and cannot promote.
// ---------------------------------------------------------------------------
#[test]
fn report_success_without_trusted_evidence_cannot_promote() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let campaign = create_campaign(&route, tmp.path(), None);

    let outcome = report_json(&campaign, "e1", "success", "dsh-pic");
    let ingest = json(
        run(
            &route,
            tmp.path(),
            &["evolve", "campaign", "report", &campaign, "--json", "--outcome", &outcome],
        ),
        "ingest success report",
    );
    assert_eq!(ingest["ingest"], Value::String("recorded".to_string()));
    // Harness-claimed success is never trusted by itself.
    assert_eq!(ingest["trusted"], Value::Bool(false));
    Ok(())
}

// ---------------------------------------------------------------------------
// 6. Campaign success does not bypass Task verification.
// ---------------------------------------------------------------------------
#[test]
fn campaign_success_does_not_bypass_task_verification() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let session = start_task(&route, tmp.path());
    let campaign = create_campaign(&route, tmp.path(), Some(&session));

    // Ingest a "success" report for the linked campaign.
    let outcome = report_json(&campaign, "e1", "success", "dsh-pic");
    must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--outcome", &outcome],
        "ingest success report",
    );

    // The parent task is NOT silently marked succeeded.
    let stdout = must_run(&route, tmp.path(), &["task", "show", &session], "task show");
    assert!(
        !stdout.contains("Succeeded") && !stdout.contains("success"),
        "task must not be auto-marked succeeded by a campaign report: {stdout}"
    );
    // Explain answers what a campaign outcome means for the parent task.
    let explain = must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--explain"],
        "campaign explain",
    );
    assert!(
        explain.contains("never silently marks it Succeeded"),
        "explain must state the no-silent-success rule: {explain}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 7. Crash / reopen restores the exact Task/Campaign/Experiment chain.
// ---------------------------------------------------------------------------
#[test]
fn crash_reopen_restores_task_campaign_chain() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let session = start_task(&route, tmp.path());
    let campaign = create_campaign(&route, tmp.path(), Some(&session));
    let outcome = report_json(&campaign, "e1", "success", "dsh-pic");
    must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--outcome", &outcome],
        "ingest report",
    );

    // A fresh process (each invocation is new) re-reads the persisted chain.
    let status = json(
        run(&route, tmp.path(), &["evolve", "campaign", "status", &campaign, "--json"]),
        "campaign status (reopen)",
    );
    assert_eq!(status["task_id"], Value::String(session.clone()));
    // The report recorded before the "crash" is still there.
    let report = json(
        run(&route, tmp.path(), &["evolve", "campaign", "report", &campaign, "--json"]),
        "campaign report (reopen)",
    );
    assert_eq!(report["task_id"], Value::String(session.clone()));
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Duplicate report submission is idempotent.
// ---------------------------------------------------------------------------
#[test]
fn duplicate_report_idempotent() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let campaign = create_campaign(&route, tmp.path(), None);
    let outcome = report_json(&campaign, "e1", "success", "dsh-pic");

    let first = json(
        run(
            &route,
            tmp.path(),
            &["evolve", "campaign", "report", &campaign, "--json", "--outcome", &outcome],
        ),
        "first ingest",
    );
    assert_eq!(first["ingest"], Value::String("recorded".to_string()));

    // Retry after a timeout: same report → duplicate, nothing double-counted.
    let second = json(
        run(
            &route,
            tmp.path(),
            &["evolve", "campaign", "report", &campaign, "--json", "--outcome", &outcome],
        ),
        "duplicate ingest",
    );
    assert_eq!(second["ingest"], Value::String("duplicate".to_string()));
    assert_eq!(second["report_count"], first["report_count"]);
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. Conflicting duplicate is explicitly detected.
// ---------------------------------------------------------------------------
#[test]
fn conflicting_duplicate_detected() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let campaign = create_campaign(&route, tmp.path(), None);
    let ok_outcome = report_json(&campaign, "e1", "success", "dsh-pic");
    must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--outcome", &ok_outcome],
        "ingest success",
    );

    // Same identity, disagreeing status → conflict evidence recorded.
    let bad_outcome = report_json(&campaign, "e1", "failed", "dsh-pic");
    let conflict = json(
        run(
            &route,
            tmp.path(),
            &["evolve", "campaign", "report", &campaign, "--json", "--outcome", &bad_outcome],
        ),
        "conflicting ingest",
    );
    assert_eq!(conflict["ingest"], Value::String("conflict".to_string()));
    assert_eq!(conflict["conflict_evidence_count"], Value::from(1));
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. Harness switch keeps the same Task/Campaign/lineage (provenance only).
// ---------------------------------------------------------------------------
#[test]
fn cross_harness_keeps_campaign_lineage() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let session = start_task(&route, tmp.path());
    let campaign = create_campaign(&route, tmp.path(), Some(&session));

    // E1 executed by DSH, E2 by a generic host — different experiments.
    must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--outcome", &report_json(&campaign, "e1", "success", "dsh-pic")],
        "ingest dsh report",
    );
    must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--outcome", &report_json(&campaign, "e2", "success", "generic-claude")],
        "ingest generic report",
    );

    // Campaign identity + parent task lineage are continuous across hosts.
    let status = json(
        run(&route, tmp.path(), &["evolve", "campaign", "status", &campaign, "--json"]),
        "campaign status",
    );
    assert_eq!(status["task_id"], Value::String(session.clone()));
    assert_eq!(status["id"], Value::String(campaign.clone()));
    Ok(())
}

// ---------------------------------------------------------------------------
// 11. Campaign explain (P11) derives answers from persisted data.
// ---------------------------------------------------------------------------
#[test]
fn campaign_explain_answers_from_persisted_data() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }
    must_run(&route, tmp.path(), &["init"], "init");
    let session = start_task(&route, tmp.path());
    let campaign = create_campaign(&route, tmp.path(), Some(&session));

    let explain = must_run(
        &route,
        tmp.path(),
        &["evolve", "campaign", "report", &campaign, "--explain"],
        "campaign explain",
    );
    assert!(explain.contains(&session), "explain should name the parent task: {explain}");
    assert!(explain.contains("What was attempted"));
    assert!(explain.contains("Task recommendation"));
    assert!(!explain.trim().is_empty());
    Ok(())
}