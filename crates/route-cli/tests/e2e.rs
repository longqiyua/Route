//! End-to-end integration tests for basic mode through CLI.

use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use tempfile::TempDir;

fn route_binary() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("route.exe")
}

#[test]
fn full_workflow_init_commit_log_export() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        // Try without .exe
        let alt = route.with_extension("");
        if !alt.exists() {
            return Ok(()); // skip if binary not built
        }
    }
    let route = if route.exists() {
        route
    } else {
        route.with_extension("")
    };

    // init
    let output = Command::new(&route)
        .arg("init")
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "init failed: {}", String::from_utf8_lossy(&output.stderr));
    assert!(tmp.path().join(".route-basic").exists());

    // create a file and commit
    std::fs::write(tmp.path().join("hello.txt"), b"hello world")?;
    let output = Command::new(&route)
        .args(["commit", "-m", "initial commit"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "commit failed: {}", String::from_utf8_lossy(&output.stderr));

    // log
    let output = Command::new(&route)
        .args(["log"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("initial commit"));

    // export mermaid
    let output = Command::new(&route)
        .args(["export", "-f", "mermaid"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "mermaid export failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("mindmap"));
    assert!(stdout.contains("::commit"));

    // export emacs org
    let output = Command::new(&route)
        .args(["export", "-f", "emacs"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("* Route 项目"));

    // stats
    let output = Command::new(&route)
        .args(["stats"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Route 统计"));

    Ok(())
}

#[test]
fn branch_workflow() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    // init + commit
    Command::new(&route).arg("init").current_dir(tmp.path()).output()?;
    std::fs::write(tmp.path().join("a.txt"), b"a")?;
    Command::new(&route)
        .args(["commit", "-m", "main commit"])
        .current_dir(tmp.path())
        .output()?;

    // create inherited branch
    let output = Command::new(&route)
        .args(["branch", "create", "feat", "-k", "inherited"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());

    // create sandbox branch
    let output = Command::new(&route)
        .args(["branch", "create", "sandbox1", "-k", "sandbox"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());

    // list branches
    let output = Command::new(&route)
        .args(["branch", "list"])
        .current_dir(tmp.path())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("main"));
    assert!(stdout.contains("feat"));
    assert!(stdout.contains("sandbox1"));

    Ok(())
}

#[test]
fn sync_workflow_add_list_run_remove() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    // init repo first (sync requires .route-basic to exist)
    Command::new(&route).arg("init").current_dir(tmp.path()).output()?;
    assert!(tmp.path().join(".route-basic").exists());

    // prepare a source dir with a file
    let src = tmp.path().join("src");
    let dst = tmp.path().join("dst");
    std::fs::create_dir_all(&src)?;
    std::fs::write(src.join("a.txt"), b"hello sync")?;

    // add a sync target (backup mode)
    let output = Command::new(&route)
        .args([
            "sync", "add",
            "--name", "t1",
            "--source", src.to_str().unwrap(),
            "--dest", dst.to_str().unwrap(),
            "--mode", "backup",
        ])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "sync add failed: {}", String::from_utf8_lossy(&output.stderr));

    // sync.json should exist
    assert!(tmp.path().join(".route-basic").join("sync.json").exists());

    // list
    let output = Command::new(&route)
        .args(["sync", "list"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("t1"), "sync list should contain t1: {stdout}");
    assert!(stdout.contains("backup"));

    // run
    let output = Command::new(&route)
        .args(["sync", "run", "t1"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "sync run failed: {}", String::from_utf8_lossy(&output.stderr));
    // dst should have a.txt
    assert!(dst.join("a.txt").exists());

    // show
    let output = Command::new(&route)
        .args(["sync", "show", "t1"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Name:        t1"));
    assert!(stdout.contains("Mode:        backup"));

    // disable then re-enable
    let output = Command::new(&route)
        .args(["sync", "disable", "t1"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());

    let output = Command::new(&route)
        .args(["sync", "enable", "t1"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());

    // remove
    let output = Command::new(&route)
        .args(["sync", "remove", "t1"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());

    // list should be empty
    let output = Command::new(&route)
        .args(["sync", "list"])
        .current_dir(tmp.path())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no sync targets"), "expected empty list, got: {stdout}");

    Ok(())
}
