//! Document-area local git backup (no remote).
//!
//! The central archive `Documents/Route/` is backed up with a **local-only**
//! git repository. The repository lives *inside* the archive folder itself
//! and is never connected to any remote: history, rollback and archival are
//! preserved purely on disk. No data ever leaves the machine.
//!
//! The flow is deliberately simple and idempotent:
//!
//! 1. `git_init_archive_no_remote()` — `git init` the archive root if not
//!    already a repo (never adds a remote; strips any that exist).
//! 2. `git_commit_archive(message)` — `git add -A` + an atomic commit, so a
//!    snapshot of the whole archive is recorded as history.
//! 3. `git_log_archive(limit)` — inspect the local history.
//!
//! These helpers drive the user's own `git` binary (same policy as the CLI's
//! git commands): no embedded VCS, no network, always local. If `git` is not
//! installed the helpers return a clear error instead of silently skipping.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::game_save::archive_root;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn git_command(repo_root: &std::path::Path) -> Command {
    let mut cmd = Command::new("git");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.current_dir(repo_root);
    cmd
}

/// Run git in `repo_root`, returning stdout. Never prompts for remote auth.
fn run_git(repo_root: &std::path::Path, args: &[&str]) -> Result<String> {
    let mut cmd = git_command(repo_root);
    cmd.args(args);
    cmd.env("LC_ALL", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");
    let output = cmd
        .output()
        .map_err(|e| anyhow!("failed to spawn git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            anyhow!("git {} failed in {}", args.join(" "), repo_root.display())
        } else {
            anyhow!("git {}: {detail}", args.join(" "))
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// The documentation-archive root that becomes the local git repository.
pub fn archive_repo_root() -> Result<PathBuf> {
    archive_root()
}

/// Is `Documents/Route` already a git repository?
pub fn is_git_repo() -> Result<bool> {
    let root = archive_repo_root()?;
    Ok(root.join(".git").exists())
}

/// Ensure the archive root is a **local-only** git repository (idempotent).
///
/// - `git init` if it is not already a repo.
/// - Bootstrap a repo-local identity if none is configured (user must never
///   be asked to configure git for a background backup).
/// - Remove any configured remote, guaranteeing the backup never pushes.
pub fn git_init_archive_no_remote() -> Result<PathBuf> {
    let root = archive_repo_root()?;
    std::fs::create_dir_all(&root)
        .with_context(|| format!("failed to create archive root {}", root.display()))?;
    if !root.join(".git").exists() {
        run_git(&root, &["init", "--quiet"])?;
    }
    // Repo-local identity, so commits work without any global git config.
    if run_git(&root, &["config", "user.name"])
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        let _ = run_git(&root, &["config", "user.name", "Route"]);
    }
    if run_git(&root, &["config", "user.email"])
        .map(|v| v.trim().is_empty())
        .unwrap_or(true)
    {
        let _ = run_git(&root, &["config", "user.email", "route@local"]);
    }
    // Enforce "local only": drain any remote so the archive never pushes.
    let remotes = run_git(&root, &["remote"]).unwrap_or_default();
    for remote in remotes.split_whitespace() {
        let _ = run_git(&root, &["remote", "remove", remote]);
    }
    Ok(root)
}

/// Commit the whole archive as a snapshot in the local git repo.
///
/// Returns the short SHA when a commit was made, or `None` when there was
/// nothing to commit (no changes, no-op — not an error).
pub fn git_commit_archive(message: &str) -> Result<Option<String>> {
    let root = git_init_archive_no_remote()?;
    run_git(&root, &["add", "-A"])?;
    // `git commit` exits non-zero when there is nothing to commit; treat that
    // as "no changes" rather than a failure.
    let output = git_command(&root)
        .args(["commit", "--quiet", "--no-gpg-sign", "-m", message])
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .output()
        .map_err(|e| anyhow!("failed to spawn git commit: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stderr.contains("nothing to commit")
            || stderr.contains("no changes added")
            || stdout.contains("nothing to commit")
            || stdout.contains("no changes added")
        {
            return Ok(None);
        }
        let code = output.status.code().unwrap_or(-1);
        return Err(anyhow!(
            "git commit failed (exit {code}): {}{}",
            stdout.trim(),
            stderr.trim()
        ));
    }
    let sha = run_git(&root, &["rev-parse", "--short", "HEAD"])?
        .trim()
        .to_string();
    Ok(Some(sha))
}

/// Show the local archive git history (newest first).
pub fn git_log_archive(limit: usize) -> Result<String> {
    let root = archive_repo_root()?;
    let n = format!("-{}", limit.clamp(1, 1000));
    run_git(&root, &["log", "--oneline", "--decorate", "--no-color", &n])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_save::TestArchiveRootGuard;

    /// Redirect `ROUTE_ARCHIVE_ROOT` to a temp dir and run the test body.
    /// `root` is the exact canonical archive root git operates on.
    fn with_isolated_root<T>(f: impl FnOnce(&PathBuf) -> T) -> T {
        let guard = TestArchiveRootGuard::new();
        let r = f(&crate::game_save::archive_root().unwrap());
        drop(guard);
        r
    }

    #[test]
    fn init_is_idempotent_and_removeless() {
        with_isolated_root(|_| {
            let first = git_init_archive_no_remote().unwrap();
            assert!(first.join(".git").exists());

            // Second call must not error and must not add a remote.
            git_init_archive_no_remote().unwrap();
            let remotes = run_git(&first, &["remote"]).unwrap_or_default();
            assert!(remotes.trim().is_empty(), "must stay remote-less");
        });
    }

    #[test]
    fn full_flow_init_commit_log() {
        with_isolated_root(|_| {
            // Derive the exact canonical root git operates on, then place files
            // there so `git add -A` sees them (Windows verbatim-path safe).
            let root = git_init_archive_no_remote().unwrap();
            let meta_dir = root.join("route").join("versions").join("v000001");
            std::fs::create_dir_all(&meta_dir).unwrap();
            std::fs::write(meta_dir.join("meta.json"), b"{}").unwrap();
            std::fs::write(root.join("registry.json"), b"{}").unwrap();

            let sha = git_commit_archive("first backup").unwrap();
            assert!(sha.is_some());

            // A second run with no changes yields None (no-op, not error).
            let none = git_commit_archive("no changes").unwrap();
            assert!(none.is_none());

            let log = git_log_archive(10).unwrap();
            assert!(log.contains("first backup"), "log should contain message");
        });
    }
}
