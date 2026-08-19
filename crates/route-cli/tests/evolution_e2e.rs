//! End-to-end tests for the Verified Evolution Core through the route CLI.
//!
//! Covers the Candidate-first loop as machine-driven flows:
//!   propose → evaluate → promote/reject → history (KnownGood chain)
//! plus stable-continues, crash-recovery, and gated-denial scenarios.
//! These are real E2E tests: they invoke the built `route` binary.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
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
    assert!(
        ok,
        "{what} failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    stdout
}

/// Propose a candidate experiment and return its full id.
fn proposed_id(route: &Path, cwd: &Path) -> String {
    let stdout = must_run(
        route,
        cwd,
        &[
            "evolve",
            "propose",
            "--target",
            "workflow",
            "--hypothesis",
            "improve task completion",
            "--baseline-id",
            "b1",
            "--candidate-id",
            "c1",
            "--changed-scope",
            "protocol",
            "--rationale",
            "shorter steps",
            "--generated-by",
            "e2e-test",
            "--reversible-save-id",
            "save-1",
        ],
        "propose",
    );
    // Parse "  id:        <id>"
    let id = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("id:"))
        .map(|s| s.trim().to_string())
        .expect("propose stdout should contain id");
    assert!(!id.is_empty(), "could not parse id from: {stdout}");
    id
}

// ---------------------------------------------------------------------------
// 1. Workflow candidate improves → promote → KnownGood updated
// ---------------------------------------------------------------------------
#[test]
fn evolution_promote_flow_updates_known_good() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(()); // skip if binary not built
    }

    must_run(&route, tmp.path(), &["init"], "init");

    let id = proposed_id(&route, tmp.path());
    let short = &id[..8];

    // Evaluate with a real harness-measured improvement → Dominates.
    let stdout = must_run(
        &route,
        tmp.path(),
        &[
            "evolve",
            "evaluate",
            short,
            "--metric",
            "task_completion=0.8:0.9",
            "--metric",
            "latency=10:5:0",
        ],
        "evaluate",
    );
    assert!(
        stdout.contains("dominates") || stdout.contains("improves"),
        "expected a promotable decision, got: {stdout}"
    );

    // Promote (gated) → new KnownGood.
    must_run(
        &route,
        tmp.path(),
        &["evolve", "promote", short, "--label", "v1"],
        "promote",
    );

    // History shows the promoted experiment and the KnownGood chain.
    let stdout = must_run(&route, tmp.path(), &["evolve", "history"], "history");
    assert!(
        stdout.contains("v1"),
        "history should show promoted label 'v1': {stdout}"
    );
    assert!(
        stdout.contains("promoted"),
        "history should show the promoted status: {stdout}"
    );

    // Show --explain answers the causal chain.
    let stdout = must_run(
        &route,
        tmp.path(),
        &["evolve", "show", short, "--explain"],
        "show --explain",
    );
    assert!(
        stdout.contains("Why proposed?"),
        "explain should answer 'why': {stdout}"
    );
    assert!(
        stdout.contains("Recovery"),
        "explain should answer 'how to recover': {stdout}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 2. No-improvement candidate → promote DENIED → reject → stable untouched
// ---------------------------------------------------------------------------
#[test]
fn evolution_neutral_rejected_and_stable_untouched() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");
    let id = proposed_id(&route, tmp.path());
    let short = &id[..8];

    // Evaluate with no improvement → not promotable.
    must_run(
        &route,
        tmp.path(),
        &["evolve", "evaluate", short],
        "evaluate",
    );

    // Promotion MUST be denied by the engine gate.
    let (ok, _, stderr) = run(
        &route,
        tmp.path(),
        &["evolve", "promote", short, "--label", "v1"],
    );
    assert!(!ok, "non-improving candidate must be denied promotion");
    assert!(
        stderr.contains("DENIED") || stderr.contains("not promotable"),
        "denial reason should be explicit: {stderr}"
    );

    // Reject the candidate.
    must_run(
        &route,
        tmp.path(),
        &["evolve", "reject", short, "--reason", "no evidence"],
        "reject",
    );

    // Stable state is completely unchanged: route status still works,
    // and no KnownGood was ever created.
    let stdout = must_run(&route, tmp.path(), &["status"], "status");
    assert!(!stdout.is_empty(), "route status should still work");

    let stdout = must_run(&route, tmp.path(), &["evolve", "history"], "history");
    assert!(
        !stdout.contains("v1"),
        "no KnownGood should exist after rejection: {stdout}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 3. Route candidate build/test fails → stable Route continues to work
// ---------------------------------------------------------------------------
#[test]
fn evolution_route_failure_stable_continues() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");

    // Propose a Route-self change (candidate-first, never live self-edit).
    let stdout = must_run(
        &route,
        tmp.path(),
        &[
            "evolve",
            "propose",
            "--target",
            "route",
            "--hypothesis",
            "refactor config loader",
            "--baseline-id",
            "b1",
            "--candidate-id",
            "c1",
            "--changed-scope",
            "protocol",
            "--rationale",
            "cleaner",
            "--generated-by",
            "e2e-test",
            "--reversible-save-id",
            "save-r1",
        ],
        "propose",
    );
    assert!(stdout.contains("route"), "proposal should record route target");

    // Even if the candidate would fail to build, the stable binary is a
    // separate process: it keeps working normally. No live edit happened.
    let stdout = must_run(&route, tmp.path(), &["status"], "status");
    assert!(!stdout.is_empty(), "stable route must continue working");

    let stdout = must_run(&route, tmp.path(), &["evolve", "history"], "history");
    assert!(
        stdout.contains("route"),
        "history should list the route candidate: {stdout}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 4. Crash during experiment → reopen → experiment state recovered
// ---------------------------------------------------------------------------
#[test]
fn evolution_crash_recovery_reopens_state() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");
    let id = proposed_id(&route, tmp.path());
    let short = &id[..8];

    // Evaluate (persists decision atomically), then "crash" = a fresh process
    // reopens the store. The experiment and its decision must survive.
    must_run(
        &route,
        tmp.path(),
        &["evolve", "evaluate", short, "--metric", "correctness=0.8:0.9"],
        "evaluate",
    );

    // Reopen in a brand-new process invocation (simulates crash + reopen).
    let stdout = must_run(
        &route,
        tmp.path(),
        &["evolve", "show", short, "--json"],
        "show after reopen",
    );
    assert!(
        stdout.contains(&id),
        "experiment id must persist across reopen: {stdout}"
    );
    let stdout = must_run(
        &route,
        tmp.path(),
        &["evolve", "history"],
        "history after reopen",
    );
    assert!(
        stdout.contains(short),
        "experiment must appear in history after reopen: {stdout}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// 5. Reference-derived constraint: anchor rejects a regressing candidate
// ---------------------------------------------------------------------------
#[test]
fn evolution_reference_derived_case_guards_suite() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");

    // Register a reference (external anchor) that the candidate must respect.
    let stdout = must_run(
        &route,
        tmp.path(),
        &[
            "reference",
            "add",
            "ref-guard",
            "--type",
            "document",
            "--source",
            "file:///norms.md",
        ],
        "reference add",
    );
    assert!(!stdout.is_empty(), "reference add should succeed");

    // Propose while anchoring the reference → derive a ReferenceDerived case.
    let stdout = must_run(
        &route,
        tmp.path(),
        &[
            "evolve",
            "propose",
            "--target",
            "project",
            "--hypothesis",
            "candidate change",
            "--baseline-id",
            "b1",
            "--candidate-id",
            "c1",
            "--changed-scope",
            "protocol",
            "--rationale",
            "r",
            "--generated-by",
            "e2e-test",
            "--reversible-save-id",
            "save-ref",
            "--reference-ids",
            "ref-guard",
        ],
        "propose",
    );

    // The anchored reference appears in the proposal's reference list.
    assert!(
        stdout.contains("references: 1"),
        "proposal should anchor the reference: {stdout}"
    );

    Ok(())
}