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

// ---------------------------------------------------------------------------
// Conversation e2e tests
// ---------------------------------------------------------------------------

/// Helper: run `route conversation new <title>` and return the session ID.
fn create_conversation_session(route: &PathBuf, cwd: &std::path::Path, title: &str) -> Result<String> {
    let output = Command::new(route)
        .args(["conversation", "new", title])
        .current_dir(cwd)
        .output()?;
    assert!(output.status.success(), "conversation new failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse: "✓ Created session '...' (id: conv-xxx)"
    let id_part = stdout.split("id: ").nth(1).unwrap_or("").trim();
    let id = id_part.trim_end_matches(')').trim();
    assert!(!id.is_empty(), "could not parse session id from: {stdout}");
    Ok(id.to_string())
}

#[test]
fn conversation_workflow() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    // Initialize a route repo (required for some operations)
    let output = Command::new(&route)
        .arg("init")
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "init failed: {}", String::from_utf8_lossy(&output.stderr));

    // --- Create a session ---
    let session_id = create_conversation_session(&route, tmp.path(), "test session")?;

    // --- List sessions ---
    let output = Command::new(&route)
        .args(["conversation", "list"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&session_id), "session id not in list: {stdout}");
    assert!(stdout.contains("test session"), "session title not in list: {stdout}");

    // --- Record user message ---
    let output = Command::new(&route)
        .args(["conversation", "record", &session_id, "user", "hello"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "record failed: {}", String::from_utf8_lossy(&output.stderr));

    // --- Record AI response ---
    let output = Command::new(&route)
        .args(["conversation", "record", &session_id, "ai", "Hi there!"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());

    // --- Show session with messages ---
    let output = Command::new(&route)
        .args(["conversation", "show", &session_id])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello"), "user message not shown: {stdout}");
    assert!(stdout.contains("Hi there!"), "ai message not shown: {stdout}");

    // --- Show with limit ---
    let output = Command::new(&route)
        .args(["conversation", "show", &session_id, "--limit", "1"])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success());
    // With limit=1, only the last message should be shown
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Hi there!"), "limited show missing ai message: {stdout}");

    // --- Archive session ---
    let output = Command::new(&route)
        .args(["conversation", "archive", &session_id])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "archive failed: {}", String::from_utf8_lossy(&output.stderr));

    // --- List should be empty (archived hidden) ---
    let output = Command::new(&route)
        .args(["conversation", "list"])
        .current_dir(tmp.path())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no conversations"), "expected empty list after archive: {stdout}");

    // --- Delete session ---
    let output = Command::new(&route)
        .args(["conversation", "delete", &session_id])
        .current_dir(tmp.path())
        .output()?;
    assert!(output.status.success(), "delete failed: {}", String::from_utf8_lossy(&output.stderr));

    Ok(())
}

#[test]
fn conversation_show_nonexistent() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    // Show a non-existent session should fail
    let output = Command::new(&route)
        .args(["conversation", "show", "nonexistent-session"])
        .current_dir(tmp.path())
        .output()?;
    assert!(!output.status.success(), "expected failure for nonexistent session");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"), "expected 'not found' error: {stderr}");

    Ok(())
}

#[test]
fn conversation_create_after_init() -> Result<()> {
    let tmp = TempDir::new()?;
    let route = route_binary();
    if !route.exists() {
        return Ok(());
    }

    // Create a conversation in a directory without route init
    // (conversation store creates .route/ automatically)
    let session_id = create_conversation_session(&route, tmp.path(), "standalone session")?;
    assert!(!session_id.is_empty(), "session id should not be empty");

    // Verify the .route/conversations.json file was created
    let conv_path = tmp.path().join(".route").join("conversations.json");
    assert!(conv_path.exists(), "conversations.json not created at: {}", conv_path.display());

    Ok(())
}
