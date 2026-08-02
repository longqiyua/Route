//! Git mode — let `route-tui` drive the user's own `git` binary so that
//! checkpoints become real git commits, branches become real git branches,
//! and the user keeps full control of push / remote operations.
//!
//! This mirrors `crates/route-tauri/src/git_commands.rs` but takes a
//! `&Path` project folder directly instead of Tauri's `AppState`, since
//! the REPL already holds the resolved project path from `--path` / cwd.
//!
//! ## Two-tier permission model
//!
//! Route's positioning is "Git wrapper with a low floor, high ceiling,
//! easy to start". That maps to two tiers:
//!
//!   - **Default (protected) mode** — the `git <cmd>` namespace. Route runs
//!     only *local* git operations: add, commit, branch, log, status, diff,
//!     stash, tag, reset, revert, restore, merge, config. Network/remote
//!     operations (push, pull, fetch, remote, clone) are REFUSED here —
//!     the user runs those themselves in their own terminal. This is the
//!     "maximum protection" default: Route never touches the network on
//!     the user's behalf unless they explicitly opt in.
//!
//!   - **High-privilege mode** — the `root git <cmd>` prefix. Prepending
//!     `root` lifts the restriction: Route will run push/pull/fetch and
//!     configure remotes itself, as software + AI collaboration. The REPL
//!     prints a warning before executing so the user is reminded to check
//!     their remote URL and credentials. `GIT_TERMINAL_PROMPT=0` stays on,
//!     so a missing credential fails cleanly instead of hanging the REPL —
//!     the user should configure a credential helper / SSH key themselves.
//!
//! All commands run with the project folder as CWD, inherited env, and a
//! stable locale so output parses the same everywhere.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Result of `detect` — whether `git` is usable.
#[derive(Debug, Clone)]
pub struct GitDetect {
    pub available: bool,
    /// `git version` output, e.g. "git version 2.43.0". Empty when not available.
    pub version: String,
    /// Human-readable reason when not available.
    pub error: String,
}

/// One row of `log` — mirrors just the fields the REPL timeline needs.
#[derive(Debug, Clone)]
pub struct GitLogEntry {
    /// Full 40-char SHA. Not currently printed (the timeline shows the
    /// short form) but kept so future `git diff <sha>` / rollback-by-sha
    /// commands have the unambiguous id without re-querying git.
    #[allow(dead_code)]
    pub sha: String,
    /// Short 7-char SHA for display.
    pub short_sha: String,
    /// First-line summary (the `--format=%s` subject).
    pub message: String,
    /// Author commit timestamp in milliseconds since epoch.
    pub timestamp_ms: i64,
    /// True if this commit was created by Route's checkpoint flow
    /// (carries a `Route-Checkpoint:` trailer).
    pub is_checkpoint: bool,
    /// True if the commit looks AI-driven (carries `Route-Operator: ai`).
    pub is_ai: bool,
}

/// One local branch.
#[derive(Debug, Clone)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    /// Short HEAD SHA, empty for an unborn branch.
    pub head: String,
}

/// Working-tree status, grouped by change kind. Mirrors the route_basic
/// `working_dir_status` shape so the `changes` command prints the same way
/// in either backend.
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
    pub untracked: Vec<String>,
}

impl GitStatus {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.modified.is_empty()
            && self.removed.is_empty()
            && self.untracked.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Run `git` with the given args in the project folder. Returns stdout as
/// a UTF-8 string on success. Errors are plain `String` so callers can show
/// them verbatim, then converted to `anyhow` at the command boundary.
fn run_git(project_path: &Path, args: &[&str]) -> std::result::Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(project_path).args(args);
    // Force a stable, parseable output regardless of the user's locale.
    cmd.env("LC_ALL", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0"); // never prompt for credentials
    cmd.env("GIT_ASKPASS", ""); // no askpass helper → fail instead of hang

    let output = cmd
        .output()
        .map_err(|e| format!("failed to spawn git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            format!("git {} failed", args.join(" "))
        } else {
            format!("git {}: {detail}", args.join(" "))
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Best-effort variant of `run_git` that swallows the error and returns an
/// empty string. Used for probes where "is this a repo?" / "is there a
/// user.email?" should never throw.
fn try_git(project_path: &Path, args: &[&str]) -> String {
    run_git(project_path, args).unwrap_or_default()
}

fn map_err(e: String) -> anyhow::Error {
    anyhow!(e)
}

/// Split a `%x1f` (ASCII unit separator)-delimited record into fields. We
/// use unit separators instead of newlines so commit messages with embedded
/// newlines don't break parsing.
fn split_record(record: &str) -> Vec<String> {
    record
        .split('\u{1f}')
        .map(|s| s.to_string())
        .collect()
}

/// True when the project folder contains a `.git` entry.
pub fn is_repo(project_path: &Path) -> bool {
    project_path.join(".git").exists()
}

/// Ensure the project folder is a git repo. If `.git` already exists we
/// leave it alone (the user may have configured it themselves). If not, we
/// run `git init` and — best-effort — set a local user.name / user.email
/// when none is inherited, so the first commit doesn't fail with
/// "Author identity unknown".
pub fn ensure_repo(project_path: &Path) -> Result<()> {
    if is_repo(project_path) {
        return Ok(());
    }
    run_git(project_path, &["init", "--quiet"]).map_err(map_err)?;

    // Bootstrap a local identity ONLY if the user hasn't set one globally
    // or system-wide. We never overwrite an existing identity.
    let name = try_git(project_path, &["config", "user.name"]).trim().to_string();
    if name.is_empty() {
        let _ = run_git(project_path, &["config", "user.name", "Route"]);
    }
    let email = try_git(project_path, &["config", "user.email"])
        .trim()
        .to_string();
    if email.is_empty() {
        let _ = run_git(project_path, &["config", "user.email", "route@local"]);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Detect whether `git` is installed and runnable. Does NOT require a
/// project to be open.
pub fn detect() -> GitDetect {
    let output = Command::new("git").arg("--version").output();

    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            GitDetect {
                available: true,
                version: v,
                error: String::new(),
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            GitDetect {
                available: false,
                version: String::new(),
                error: if stderr.is_empty() {
                    "git exited with a non-zero status".to_string()
                } else {
                    stderr
                },
            }
        }
        Err(e) => GitDetect {
            available: false,
            version: String::new(),
            error: format!("git not found on PATH: {e}"),
        },
    }
}

/// Initialize git in the project (idempotent). Returns the resolved path.
pub fn init(project_path: &Path) -> Result<String> {
    ensure_repo(project_path)?;
    Ok(project_path.to_string_lossy().to_string())
}

/// Current branch name. Empty string when not a repo / unborn branch.
pub fn current_branch(project_path: &Path) -> Result<String> {
    if !is_repo(project_path) {
        return Ok(String::new());
    }
    Ok(try_git(project_path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string())
}

/// Stage every change and create a git commit. This is the "打点"
/// (checkpoint) action when Git Mode is on.
///
/// The commit message is `"{title}"` followed by an optional body and two
/// trailers:
///   - `Route-Checkpoint: <title>` — so the timeline can flag it
///   - `Route-Operator: user` (or `ai:<name>` when an operator is given)
///
/// `--allow-empty` so a checkpoint with no file changes still produces a
/// commit (mirrors the built-in route_basic behavior). `--no-verify` is
/// intentionally NOT passed: the user's hooks are theirs.
pub fn commit(
    project_path: &Path,
    title: &str,
    body: Option<&str>,
    operator: Option<&str>,
) -> Result<GitLogEntry> {
    ensure_repo(project_path)?;

    let title = title.trim();
    if title.is_empty() {
        return Err(anyhow!("checkpoint title is required"));
    }

    // Stage everything. `git add -A` matches route_basic semantics.
    run_git(project_path, &["add", "-A"]).map_err(map_err)?;

    let operator = operator
        .map(|o| {
            if o.trim().is_empty() {
                "user".to_string()
            } else {
                format!("ai:{o}")
            }
        })
        .unwrap_or_else(|| "user".to_string());

    let mut message = String::new();
    message.push_str(title);
    message.push_str("\n\n");
    if let Some(b) = body.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        message.push_str(b);
        message.push_str("\n\n");
    }
    message.push_str(&format!("Route-Checkpoint: {title}\n"));
    message.push_str(&format!("Route-Operator: {operator}\n"));

    run_git(
        project_path,
        &["commit", "--quiet", "--allow-empty", "-m", &message],
    )
    .map_err(map_err)?;

    // Return the just-created commit as a log entry.
    parse_log_row(project_path, "-1")
}

/// Read the git log as a list of timeline entries. `limit` caps the number
/// of rows. Output is sorted newest-first by git itself. When `branch` is
/// `Some`, only that branch's history is read.
pub fn log(
    project_path: &Path,
    limit: Option<usize>,
    branch: Option<&str>,
) -> Result<Vec<GitLogEntry>> {
    if !is_repo(project_path) {
        return Ok(Vec::new());
    }

    let n = limit.unwrap_or(200).clamp(1, 1000);
    let limit_arg = format!("-{n}");
    let mut args: Vec<String> = vec!["log".into(), limit_arg];
    if let Some(b) = branch {
        args.push(b.to_string());
    }
    args.push("--no-patch".into());
    args.push(
        "--format=%H%x1f%h%x1f%s%x1f%ct%x1f%(trailers:key=Route-Checkpoint,valueonly)%x1f%(trailers:key=Route-Operator,valueonly)"
            .into(),
    );
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = run_git(project_path, &arg_refs).map_err(map_err)?;

    let mut entries = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts = split_record(line);
        if parts.len() < 4 {
            continue;
        }
        let sha = parts[0].clone();
        let short_sha = parts[1].clone();
        let message = parts[2].clone();
        let ts = parts[3].trim().parse::<i64>().unwrap_or(0) * 1000;
        let ckpt_flag = parts.get(4).cloned().unwrap_or_default();
        let op_flag = parts.get(5).cloned().unwrap_or_default();
        entries.push(GitLogEntry {
            sha,
            short_sha,
            message,
            timestamp_ms: ts,
            is_checkpoint: !ckpt_flag.trim().is_empty(),
            is_ai: op_flag.trim().starts_with("ai"),
        });
    }
    Ok(entries)
}

/// Parse a single log row. `spec` is either `-1` (HEAD) or a revision.
fn parse_log_row(project_path: &Path, spec: &str) -> Result<GitLogEntry> {
    let args = [
        "log",
        spec,
        "--no-patch",
        "--format=%H%x1f%h%x1f%s%x1f%ct%x1f%(trailers:key=Route-Checkpoint,valueonly)%x1f%(trailers:key=Route-Operator,valueonly)",
    ];
    let row = run_git(project_path, &args).map_err(map_err)?;
    let line = row.trim();
    let parts = split_record(line);
    let sha = parts.first().cloned().unwrap_or_default();
    let short_sha = parts.get(1).cloned().unwrap_or_default();
    let message = parts.get(2).cloned().unwrap_or_default();
    let ts = parts
        .get(3)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0)
        * 1000;
    let ckpt_flag = parts.get(4).cloned().unwrap_or_default();
    let op_flag = parts.get(5).cloned().unwrap_or_default();
    Ok(GitLogEntry {
        sha,
        short_sha,
        message,
        timestamp_ms: ts,
        is_checkpoint: !ckpt_flag.trim().is_empty(),
        is_ai: op_flag.trim().starts_with("ai"),
    })
}

/// List local branches. Remote-tracking refs are deliberately excluded —
/// Route doesn't manage remotes, so showing them would be noise.
///
/// Note on formatting: `git branch --format` uses the for-each-ref placeholder
/// syntax (`%(refname:short)` etc.), which — unlike `git log --format` — does
/// NOT expand the `%x1f` hex escape. We therefore separate the name and head
/// SHA with a single space. That's safe because git ref names cannot contain
/// spaces (git rejects them at branch creation), so `splitn(2, ' ')` recovers
/// the two fields unambiguously.
pub fn branch_list(project_path: &Path) -> Result<Vec<GitBranch>> {
    if !is_repo(project_path) {
        return Ok(Vec::new());
    }
    let out = run_git(
        project_path,
        &["branch", "--list", "--format=%(refname:short) %(objectname:short)"],
    )
    .map_err(map_err)?;
    // `branch --format` doesn't mark HEAD reliably across git versions, so
    // ask separately.
    let current = current_branch(project_path).unwrap_or_default();

    let mut branches = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let name = parts.next().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let head = parts.next().unwrap_or("").to_string();
        branches.push(GitBranch {
            current: name == current,
            name,
            head,
        });
    }
    Ok(branches)
}

/// Create a new branch at HEAD. Does NOT switch to it — the user can
/// switch separately via `branch_switch`. Matches route_basic's
/// "create then maybe switch" pattern.
pub fn branch_create(project_path: &Path, name: &str) -> Result<GitBranch> {
    ensure_repo(project_path)?;
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("branch name is required"));
    }
    // Validate the name format to give a clean error before git does.
    if name.contains(' ') || name.contains("..") {
        return Err(anyhow!("invalid branch name: {name}"));
    }
    run_git(project_path, &["branch", name]).map_err(map_err)?;
    let head = try_git(project_path, &["rev-parse", "--short", name])
        .trim()
        .to_string();
    Ok(GitBranch {
        name: name.to_string(),
        current: false,
        head,
    })
}

/// Switch the working tree to a different local branch. A real
/// `git switch`/`checkout` — uncommitted changes are carried over or
/// blocked by git itself; we don't try to be smarter than git here.
pub fn branch_switch(project_path: &Path, name: &str) -> Result<()> {
    if !is_repo(project_path) {
        return Err(anyhow!("not a git repository"));
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("branch name is required"));
    }
    // `switch` is the modern, safer command (won't detach HEAD on a
    // missing branch). Fall back to `checkout` for git < 2.23.
    match run_git(project_path, &["switch", name]) {
        Ok(_) => Ok(()),
        Err(e) => {
            if e.contains("unknown switch") || e.contains("usage: git") {
                run_git(project_path, &["checkout", name]).map_err(map_err)?;
                Ok(())
            } else {
                Err(map_err(e))
            }
        }
    }
}

/// Merge `source` into the current branch. Runs `git merge --no-edit --no-ff`
/// — a real local merge, no push, no remote. `--no-ff` forces a merge commit
/// so branch topology stays visible. Returns the new HEAD SHA.
pub fn merge(project_path: &Path, source: &str) -> Result<String> {
    if !is_repo(project_path) {
        return Err(anyhow!("not a git repository"));
    }
    let source = source.trim();
    if source.is_empty() {
        return Err(anyhow!("source branch is required"));
    }
    run_git(project_path, &["merge", "--no-edit", "--no-ff", source]).map_err(map_err)?;
    let head = run_git(project_path, &["rev-parse", "HEAD"])
        .map_err(map_err)?
        .trim()
        .to_string();
    Ok(head)
}

/// Working-tree status from `git status --porcelain`. Each path is
/// classified by its XY status pair. Untracked files (rows starting with
/// `??`) land in their own bucket so the `changes` command can show them
/// distinctly.
pub fn status(project_path: &Path) -> Result<GitStatus> {
    if !is_repo(project_path) {
        return Ok(GitStatus::default());
    }
    let out = run_git(project_path, &["status", "--porcelain=v1", "-z"]).map_err(map_err)?;
    let mut st = GitStatus::default();
    // `-z` separates entries with NULs (no quoting/escaping), so embedded
    // spaces / unicode in paths survive intact. Each record is `XY<sp>path`
    // where X = staged (index) status and Y = working-tree status.
    for entry in out.split('\0') {
        let entry = entry.trim_end_matches('\n');
        if entry.len() < 3 {
            continue;
        }
        let bytes = entry.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let path = entry[3..].to_string();
        if x == '?' && y == '?' {
            st.untracked.push(path);
        } else if x == 'A' && y == 'D' {
            // Added to index, then deleted from worktree.
            st.removed.push(path);
        } else if x == 'A' || y == 'A' {
            st.added.push(path);
        } else if x == 'D' || y == 'D' {
            st.removed.push(path);
        } else {
            // M (modified), R (renamed), C (copied), T (type-changed)...
            st.modified.push(path);
        }
    }
    Ok(st)
}

/// Format a `timestamp_ms` (git commit time, seconds→ms) as a readable UTC
/// string. Kept here so the git command handlers render dates consistently
/// with the route_basic path (which uses `fmt::format_ts`).
pub fn format_ts(millis: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(millis)
        .unwrap_or_default()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string()
}

// ---------------------------------------------------------------------------
// Local commands (default / protected mode)
// ---------------------------------------------------------------------------

/// Stage paths, or all changes when `paths` is empty (`git add -A`). This
/// is the explicit-staging counterpart to the auto-tracking watcher.
pub fn add(project_path: &Path, paths: &[String]) -> Result<()> {
    ensure_repo(project_path)?;
    if paths.is_empty() {
        run_git(project_path, &["add", "-A"]).map_err(map_err)?;
    } else {
        let mut args: Vec<&str> = vec!["add"];
        for p in paths {
            args.push(p.as_str());
        }
        run_git(project_path, &args).map_err(map_err)?;
    }
    Ok(())
}

/// Unstaged diff — working tree vs index (`git diff`). Returns the raw
/// patch text; the command handler prints it.
pub fn diff_workdir(project_path: &Path) -> Result<String> {
    if !is_repo(project_path) {
        return Ok(String::new());
    }
    run_git(project_path, &["diff"]).map_err(map_err)
}

/// Staged diff — index vs HEAD (`git diff --staged`).
pub fn diff_staged(project_path: &Path) -> Result<String> {
    if !is_repo(project_path) {
        return Ok(String::new());
    }
    run_git(project_path, &["diff", "--staged"]).map_err(map_err)
}

/// Diff between two commits / refs / trees (`git diff <a> <b>`).
pub fn diff_commits(project_path: &Path, a: &str, b: &str) -> Result<String> {
    if !is_repo(project_path) {
        return Err(anyhow!("not a git repository"));
    }
    run_git(project_path, &["diff", a, b]).map_err(map_err)
}

/// Stash operations. `action` is one of `push`/`pop`/`list`/`drop`.
pub fn stash(project_path: &Path, action: &str, args: &[String]) -> Result<String> {
    ensure_repo(project_path)?;
    let mut cmd: Vec<String> = vec!["stash".into(), action.into()];
    for a in args {
        cmd.push(a.clone());
    }
    let refs: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
    run_git(project_path, &refs).map_err(map_err)
}

/// List all tags.
pub fn tag_list(project_path: &Path) -> Result<Vec<String>> {
    if !is_repo(project_path) {
        return Ok(Vec::new());
    }
    let out = run_git(project_path, &["tag", "--list"]).map_err(map_err)?;
    Ok(out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Create a tag. Annotated (`-a`) when a message is given, lightweight
/// otherwise.
pub fn tag_create(project_path: &Path, name: &str, message: Option<&str>) -> Result<()> {
    ensure_repo(project_path)?;
    match message {
        Some(m) => {
            run_git(project_path, &["tag", "-a", name, "-m", m]).map_err(map_err)?;
        }
        None => {
            run_git(project_path, &["tag", name]).map_err(map_err)?;
        }
    }
    Ok(())
}

/// Delete a tag.
pub fn tag_delete(project_path: &Path, name: &str) -> Result<()> {
    run_git(project_path, &["tag", "-d", name])
        .map_err(map_err)
        .map(|_| ())
}

/// Reset mode for `reset`.
#[derive(Debug, Clone, Copy)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

/// Reset HEAD to `commit` with the given mode. `Hard` discards working-tree
/// changes — the handler should confirm intent before calling.
pub fn reset(project_path: &Path, mode: ResetMode, commit: &str) -> Result<()> {
    if !is_repo(project_path) {
        return Err(anyhow!("not a git repository"));
    }
    let flag = match mode {
        ResetMode::Soft => "--soft",
        ResetMode::Mixed => "--mixed",
        ResetMode::Hard => "--hard",
    };
    run_git(project_path, &["reset", flag, commit])
        .map_err(map_err)
        .map(|_| ())
}

/// Revert a commit, creating an inverse commit (`git revert --no-edit`).
pub fn revert(project_path: &Path, commit: &str) -> Result<()> {
    ensure_repo(project_path)?;
    run_git(project_path, &["revert", "--no-edit", commit])
        .map_err(map_err)
        .map(|_| ())
}

/// Restore working-tree files (`git restore <paths>`).
pub fn restore(project_path: &Path, paths: &[String]) -> Result<()> {
    if !is_repo(project_path) {
        return Err(anyhow!("not a git repository"));
    }
    let mut args: Vec<&str> = vec!["restore"];
    for p in paths {
        args.push(p.as_str());
    }
    run_git(project_path, &args).map_err(map_err).map(|_| ())
}

/// Read a config value (best-effort — empty string when unset).
pub fn config_get(project_path: &Path, key: &str) -> Result<String> {
    Ok(try_git(project_path, &["config", key]).trim().to_string())
}

/// Set a config value (local by default, since we run without `--global`).
pub fn config_set(project_path: &Path, key: &str, value: &str) -> Result<()> {
    run_git(project_path, &["config", key, value])
        .map_err(map_err)
        .map(|_| ())
}

// ---------------------------------------------------------------------------
// Network commands (high-privilege / `root` mode only)
//
// These talk to a remote. They are only reachable through the `root git ...`
// prefix in the REPL (see commands.rs), which prints a warning first. We
// keep GIT_TERMINAL_PROMPT=0 so a missing credential fails cleanly instead
// of hanging the REPL — the user should set up a credential helper or SSH
// key themselves. Progress messages git writes to stderr are captured and
// shown so the user can see "main -> main" etc.
// ---------------------------------------------------------------------------

/// Run `git` capturing BOTH stdout and stderr on success. Network commands
/// (push/pull/fetch) write their progress to stderr, so returning only
/// stdout would hide the result from the user. Errors still come from
/// stderr (or stdout) as a single readable message.
fn run_git_capture(project_path: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(project_path).args(args);
    cmd.env("LC_ALL", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd
        .output()
        .map_err(|e| anyhow!("failed to spawn git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            anyhow!("git {} failed", args.join(" "))
        } else {
            anyhow!("git {}: {detail}", args.join(" "))
        });
    }

    // Success — combine stdout + stderr so progress is visible.
    let mut combined = String::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        combined.push_str(&stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    Ok(combined)
}

/// Fetch from a remote (defaults to `origin`).
pub fn fetch(project_path: &Path, remote: Option<&str>) -> Result<String> {
    ensure_repo(project_path)?;
    let remote = remote.unwrap_or("origin");
    run_git_capture(project_path, &["fetch", remote])
}

/// Pull from a remote (defaults to `origin`), optionally into `branch`.
pub fn pull(project_path: &Path, remote: Option<&str>, branch: Option<&str>) -> Result<String> {
    ensure_repo(project_path)?;
    let remote = remote.unwrap_or("origin");
    let mut args: Vec<String> = vec!["pull".into(), remote.into()];
    if let Some(b) = branch {
        args.push(b.into());
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_git_capture(project_path, &refs)
}

/// Push to a remote (defaults to `origin`), optionally the given branch.
/// `force` adds `--force` — the handler should warn before allowing it.
pub fn push(
    project_path: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
    force: bool,
) -> Result<String> {
    ensure_repo(project_path)?;
    let remote = remote.unwrap_or("origin");
    let mut args: Vec<String> = vec!["push".into()];
    if force {
        args.push("--force".into());
    }
    args.push(remote.into());
    if let Some(b) = branch {
        args.push(b.into());
    }
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_git_capture(project_path, &refs)
}

/// One remote entry: name + first URL.
#[derive(Debug, Clone)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
}

/// List configured remotes (`git remote -v`). Each name appears once
/// (fetch/push URLs are usually identical; we take the first).
pub fn remote_list(project_path: &Path) -> Result<Vec<GitRemote>> {
    if !is_repo(project_path) {
        return Ok(Vec::new());
    }
    let out = run_git(project_path, &["remote", "-v"]).map_err(map_err)?;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut remotes = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let name = parts.next().unwrap_or("");
        let url = parts.next().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.to_string()) {
            remotes.push(GitRemote {
                name: name.to_string(),
                url: url.to_string(),
            });
        }
    }
    Ok(remotes)
}

/// Add a remote (`git remote add <name> <url>`).
pub fn remote_add(project_path: &Path, name: &str, url: &str) -> Result<()> {
    ensure_repo(project_path)?;
    run_git(project_path, &["remote", "add", name, url])
        .map_err(map_err)
        .map(|_| ())
}

/// Remove a remote.
pub fn remote_remove(project_path: &Path, name: &str) -> Result<()> {
    run_git(project_path, &["remote", "remove", name])
        .map_err(map_err)
        .map(|_| ())
}

/// Change a remote's URL (`git remote set-url`).
pub fn remote_set_url(project_path: &Path, name: &str, url: &str) -> Result<()> {
    run_git(project_path, &["remote", "set-url", name, url])
        .map_err(map_err)
        .map(|_| ())
}

// Network ops are gated behind the `root` prefix in the REPL, not absent.
// The default `git` namespace refuses them so Route never touches the
// network without an explicit opt-in — that is the "maximum protection"
// default. `root git push|pull|fetch|remote ...` lifts it.
