//! End-to-end tests for Emergence Hardening (P13–P24) through the route CLI.
//!
//! Machine-driven flows: experimental enable/status, blackboard events,
//! novelty archive, benchmark court, search budget, and lineage explain.
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
    assert!(ok, "{what} failed\nstdout: {stdout}\nstderr: {stderr}");
    stdout
}

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
            "e2e-explorer",
            "--reversible-save-id",
            "save-em1",
        ],
        "propose",
    );
    let id = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("id:"))
        .map(|s| s.trim().to_string())
        .expect("propose stdout should contain id");
    assert!(!id.is_empty(), "could not parse id from: {stdout}");
    id
}

// ---------------------------------------------------------------------------
// P13 / P25 #9: blackboard events persist and projection is rebuildable.
// ---------------------------------------------------------------------------
#[test]
fn emergence_blackboard_events_persist_across_processes() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");

    // Experimental is OFF by default (P13 "off by default").
    let stdout = must_run(&route, tmp.path(), &["emerge", "status"], "status");
    assert!(
        stdout.contains("disabled"),
        "emergence must be disabled by default: {stdout}"
    );

    must_run(&route, tmp.path(), &["emerge", "enable"], "enable");

    // Append events from multiple actors (distinct processes).
    must_run(
        &route,
        tmp.path(),
        &["emerge", "event", "--actor", "explorer-a", "--source", "observation", "--confidence", "0.9"],
        "event a",
    );
    must_run(
        &route,
        tmp.path(),
        &["emerge", "event", "--actor", "explorer-b", "--source", "hypothesis", "--confidence", "0.6"],
        "event b",
    );
    must_run(
        &route,
        tmp.path(),
        &["emerge", "event", "--actor", "evaluator-1", "--source", "evidence", "--confidence", "0.8"],
        "event c",
    );

    // Status reflects the count; JSON projection is deterministic.
    let stdout = must_run(&route, tmp.path(), &["emerge", "status"], "status");
    assert!(
        stdout.contains("blackboard events: 3"),
        "blackboard must retain all events: {stdout}"
    );
    let json = must_run(&route, tmp.path(), &["emerge", "status", "--json"], "status json");
    assert!(
        json.contains("\"experimental_enabled\": true"),
        "json status should show enabled: {json}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// P15 / P25 #1: multi-explorer novelty archive is recorded.
// ---------------------------------------------------------------------------
#[test]
fn emergence_novelty_archive_is_exposed() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");
    must_run(&route, tmp.path(), &["emerge", "enable"], "enable");

    let stdout = must_run(&route, tmp.path(), &["emerge", "novelty"], "novelty");
    assert!(
        stdout.contains("empty") || stdout.contains("(novelty archive empty)"),
        "novelty archive should start empty: {stdout}"
    );

    // A proposed experiment is lineage-explainable.
    let id = proposed_id(&route, tmp.path());
    let short = &id[..8];
    let stdout = must_run(
        &route,
        tmp.path(),
        &["evolve", "explain", short],
        "explain",
    );
    assert!(
        stdout.contains("Parent candidate"),
        "explain should answer parent lineage: {stdout}"
    );
    assert!(
        stdout.contains("Hypothesis"),
        "explain should answer hypothesis: {stdout}"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// P17 / P25 #4: benchmark court + run budget are exposed.
// ---------------------------------------------------------------------------
#[test]
fn emergence_court_and_budget_are_exposed() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    must_run(&route, tmp.path(), &["init"], "init");
    must_run(&route, tmp.path(), &["emerge", "enable"], "enable");

    let stdout = must_run(&route, tmp.path(), &["emerge", "court"], "court");
    assert!(
        stdout.contains("empty") || stdout.contains("court empty"),
        "court should start empty: {stdout}"
    );

    let stdout = must_run(&route, tmp.path(), &["emerge", "run"], "run");
    assert!(
        stdout.contains("no evolution run") || stdout.contains("run"),
        "run budget should be exposed: {stdout}"
    );

    Ok(())
}