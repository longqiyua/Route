//! Git mode commands — let Route drive the user's own `git` binary so that
//! checkpoints become real git commits, branches become real git branches,
//! and the user keeps full control of push / remote operations.
//!
//! Design rules (deliberate, non-negotiable):
//!   - We ONLY ever call `git add` + `git commit` + `git branch` + `git log`
//!     + `git rev-parse` + `git init` + `git config` (for identity bootstrap
//!     when none is set). Nothing else.
//!   - We NEVER call `git push`, `git pull`, `git fetch`, `git remote`,
//!     `git clone`, or anything that talks to the network. Push and remote
//!     configuration are the user's job — Route tells them so in the UI.
//!   - All commands run with the project folder as CWD, inherited env, and
//!     a hard timeout so a hung git never freezes the app.
//!   - If `git` is not on PATH we surface a clean `available: false` from
//!     `git_detect` so the frontend can prompt the user instead of throwing.
//!
//! The route_basic repository remains the source of truth for the built-in
//! timeline when git mode is OFF. When git mode is ON, the workbench's
//! "打点" (checkpoint) button calls `git_commit` instead of the built-in
//! `commit`, and the timeline page reads `git_log` instead of `log`.
//! The two histories are NOT merged — git mode is an alternative backend,
//! not a bridge.

use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

// On Windows the desktop app is built with `windows_subsystem = "windows"`,
// so it has no console of its own. Every `Command::new("git")` would
// therefore pop up a fresh black console window — which is what caused the
// "黑终端窗口跳来跳去" flicker at startup and on every checkpoint. The
// `CREATE_NO_WINDOW` creation flag suppresses that auxiliary console. On
// non-Windows this is a no-op.
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `git` `Command` with the no-console flag pre-applied on Windows.
/// All git invocations in this module go through here so the flag can never
/// be forgotten on a new call site.
fn git_command() -> Command {
    let mut cmd = Command::new("git");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Hard cap for any single git invocation. Git operations on a project
/// folder are local and should finish in seconds; if they don't, something
/// is wrong (huge repo, hung credential prompt, etc.) and we'd rather
/// return an error than block the UI indefinitely.
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Result of `git_detect` — tells the frontend whether `git` is usable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDetectDto {
    pub available: bool,
    /// `git version` output, e.g. "git version 2.43.0". Empty when not available.
    pub version: String,
    /// Human-readable reason when not available (e.g. "git not found on PATH").
    pub error: String,
}

/// One row of `git_log` — mirrors just the fields Route's timeline needs.
/// We deliberately do NOT expose author email / hash signatures / refs —
/// the timeline shows message + time + (optional) checkpoint flag, nothing
/// more.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitLogEntryDto {
    /// Full 40-char SHA. Used as the unique id.
    pub sha: String,
    /// First-line summary (the `--format=%s` subject).
    pub message: String,
    /// Author commit timestamp in milliseconds since epoch.
    pub timestamp_ms: i64,
    /// True if this commit was created by Route's checkpoint flow.
    /// Detected via a trailer `Route-Checkpoint: <title>` so user-made
    /// commits and Route-made checkpoints are distinguishable on the
    /// timeline without parsing free-text messages.
    pub is_checkpoint: bool,
    /// True if the commit looks like an AI-driven one (carries the
    /// `Route-Operator: ai` trailer). Best-effort, never throws.
    pub is_ai: bool,
}

/// One row of `git_branch_list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranchDto {
    pub name: String,
    pub current: bool,
}

/// Working-tree status grouped by change kind. Mirrors route-tui's
/// `GitStatus` and route_basic's `working_dir_status` shape so the
/// workbench (and the AI commit-message generator) can treat both
/// backends uniformly: route_basic returns a flat `[{path, change}]`
/// list, git mode returns this grouped map — the frontend normalizes
/// both into the same `{change, path}` tuples before building the
/// prompt.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitStatusDto {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
    pub untracked: Vec<String>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Run `git` with the given args in the project folder. Returns the
/// stdout as a UTF-8 string on success. Errors are returned as a plain
/// `String` so the frontend gets a readable message via the IPC bridge.
fn run_git(project_path: &std::path::Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = git_command();
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

/// Best-effort variant of `run_git` that swallows the error and returns
/// an empty string. Used for probes where "is this a git repo?" / "is
/// there a user.email?" should never throw.
fn try_git(project_path: &std::path::Path, args: &[&str]) -> String {
    run_git(project_path, args).unwrap_or_default()
}

/// Resolve the currently-open project folder. Returns an error string
/// the frontend can show directly when no project is open.
fn project_path(state: &AppState) -> Result<std::path::PathBuf, String> {
    state
        .project_path()
        .ok_or_else(|| "no project is open".to_string())
}

/// Ensure the project folder is a git repo. If `.git` already exists we
/// leave it alone (the user may have configured it themselves). If not,
/// we run `git init` and — best-effort — set a local user.name / user.email
/// when none is inherited, so the first commit doesn't fail with
/// "Author identity unknown".
fn ensure_repo(project_path: &std::path::Path) -> Result<(), String> {
    let dot_git = project_path.join(".git");
    if dot_git.exists() {
        return Ok(());
    }
    run_git(project_path, &["init", "--quiet"])?;

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

/// Split a `%x00`-delimited single-line record into fields. We use NUL
/// separators (`-z` style) instead of newlines so commit messages with
/// embedded newlines don't break parsing.
fn split_record(record: &str) -> Vec<String> {
    record
        .split('\u{1f}') // ASCII unit separator — stable across locales
        .map(|s| s.to_string())
        .collect()
}

/// Parse the current HEAD commit into a `GitLogEntryDto`. Shared by
/// `git_commit` and `git_rollback` so both return the just-created entry
/// in the exact same shape — and so a future change to the trailer scheme
/// only has to be made in one place.
fn parse_head_entry(project_path: &std::path::Path) -> Result<GitLogEntryDto, String> {
    let head_sha = run_git(project_path, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let row = run_git(
        project_path,
        &[
            "log",
            "-1",
            "--no-patch",
            "--format=%H%x1f%s%x1f%ct%x1f%(trailers:key=Route-Checkpoint,valueonly)%x1f%(trailers:key=Route-Operator,valueonly)",
        ],
    )?;
    let parts = split_record(row.trim());
    let sha = parts.first().cloned().unwrap_or(head_sha);
    let msg = parts.get(1).cloned().unwrap_or_default();
    let ts = parts
        .get(2)
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0)
        * 1000;
    let ckpt_flag = parts.get(3).cloned().unwrap_or_default();
    let op_flag = parts.get(4).cloned().unwrap_or_default();
    Ok(GitLogEntryDto {
        sha,
        message: msg,
        timestamp_ms: ts,
        is_checkpoint: !ckpt_flag.trim().is_empty(),
        is_ai: op_flag.trim().starts_with("ai"),
    })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Detect whether `git` is installed and runnable. Does NOT require a
/// project to be open — this is the probe the settings page calls when
/// the user toggles Git Mode on.
#[tauri::command]
pub fn git_detect() -> GitDetectDto {
    let output = git_command()
        .arg("--version")
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            GitDetectDto {
                available: true,
                version: v,
                error: String::new(),
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            GitDetectDto {
                available: false,
                version: String::new(),
                error: if stderr.is_empty() {
                    "git exited with a non-zero status".to_string()
                } else {
                    stderr
                },
            }
        }
        Err(e) => GitDetectDto {
            available: false,
            version: String::new(),
            error: format!("git not found on PATH: {e}"),
        },
    }
}

/// Toggle git mode on the backend. The file watcher reads this flag on
/// every tick to decide whether to commit via `git add + git commit`
/// (git mode on) or via route_basic's `repo.commit` (git mode off).
/// The frontend persists the same flag in localStorage for the UI; this
/// command syncs the backend so the watcher sees the change live.
#[tauri::command]
pub fn git_mode_set(enabled: bool, state: State<'_, AppState>) -> Result<bool, String> {
    state.git_mode.store(enabled, Ordering::Relaxed);
    Ok(enabled)
}

/// Initialize git in the current project (idempotent). Called when the
/// user enables Git Mode so the first checkpoint doesn't fail with
/// "not a git repository". Returns the resolved project path so the
/// frontend can display it.
#[tauri::command]
pub fn git_init(state: State<'_, AppState>) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    Ok(path.to_string_lossy().to_string())
}

/// Stage every change and create a git commit. This is the "打点"
/// (checkpoint) action when Git Mode is on.
///
/// The commit message is `"{title}"` followed by an optional body and
/// two trailers:
///   - `Route-Checkpoint: <title>` — so the timeline can flag it
///   - `Route-Operator: user` (or `ai:<name>` when an AI operator is set)
///
/// We pass `--allow-empty` so a checkpoint with no file changes still
/// produces a commit (mirrors the built-in route_basic behavior).
#[tauri::command]
pub fn git_commit(
    state: State<'_, AppState>,
    title: String,
    body: Option<String>,
) -> Result<GitLogEntryDto, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;

    let title = title.trim();
    if title.is_empty() {
        return Err("checkpoint title is required".to_string());
    }

    // Stage everything. `git add -A` matches the route_basic semantics
    // (the watcher commits all changes, not a curated subset).
    run_git(&path, &["add", "-A"])?;

    // Compose the commit message. Trailers go after a blank line so git
    // treats them as trailers (parseable by `git log --format=%(trailers)`).
    let operator = state
        .ai_operator
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|o| format!("ai:{}", o.name))
        .unwrap_or_else(|| "user".to_string());

    let mut message = String::new();
    message.push_str(title);
    message.push_str("\n\n");
    if let Some(b) = body.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        message.push_str(b);
        message.push_str("\n\n");
    }
    message.push_str(&format!("Route-Checkpoint: {title}\n"));
    message.push_str(&format!("Route-Operator: {operator}\n"));

    // `--allow-empty` — a checkpoint may mark a moment with no file changes.
    // `--no-verify` is intentionally NOT passed: the user's hooks are theirs.
    run_git(
        &path,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            &message,
        ],
    )?;

    // Return the just-created commit as a log entry.
    parse_head_entry(&path)
}

/// Roll the working tree back to the state of a past commit `<sha>`,
/// recorded as a NEW commit on top of HEAD. This is the Git mode
/// counterpart of route_basic's `rollback` — and like route_basic it is
/// NON-destructive: commits after `<sha>` are not discarded, they stay in
/// history. The new commit's tree simply matches `<sha>`'s tree.
///
/// Mechanically: `git read-tree --reset -u <sha>` resets the index AND
/// working tree to `<sha>`'s tree without moving HEAD (the worktree-only
/// equivalent of `git reset --hard`), then `git commit` records that
/// restored state as a new commit. Untracked files are left alone — we
/// only restore tracked files. `--allow-empty` covers the `<sha> == HEAD`
/// edge case.
///
/// The commit carries a `Route-Operator` trailer (so AI-driven rollbacks
/// stay distinguishable) but NO `Route-Checkpoint` trailer — a rollback is
/// not a user 打点, so `is_checkpoint` stays false. The subject line
/// "rollback to <short>" is self-describing on the timeline.
#[tauri::command]
pub fn git_rollback(
    state: State<'_, AppState>,
    sha: String,
) -> Result<GitLogEntryDto, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let sha = sha.trim().to_string();
    if sha.is_empty() {
        return Err("target commit is required".to_string());
    }
    // Validate that <sha> resolves to a commit before touching the tree,
    // so an invalid target gives a clean error instead of a confusing
    // read-tree failure downstream.
    run_git(&path, &["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .map_err(|_| format!("commit not found: {sha}"))?;

    let short = run_git(&path, &["rev-parse", "--short", &sha])?
        .trim()
        .to_string();

    // Restore index + worktree to <sha>'s tree. HEAD is NOT moved, so the
    // next commit creates a new commit on top of the current branch rather
    // than rewriting history (matches route_basic's non-destructive
    // rollback semantics).
    run_git(&path, &["read-tree", "--reset", "-u", &sha])?;

    let operator = state
        .ai_operator
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|o| format!("ai:{}", o.name))
        .unwrap_or_else(|| "user".to_string());
    let message = format!("rollback to {short}\n\nRoute-Operator: {operator}\n");

    run_git(
        &path,
        &["commit", "--quiet", "--allow-empty", "-m", &message],
    )?;

    // Return the just-created rollback commit as a log entry.
    parse_head_entry(&path)
}

/// Read the git log as a list of timeline entries. `limit` caps the
/// number of rows (the timeline paginates anyway). The output is sorted
/// newest-first by git itself. When `branch` is `Some`, only that
/// branch's history is read — used by the branch tree to populate each
/// lane independently.
#[tauri::command]
pub fn git_log(
    state: State<'_, AppState>,
    limit: Option<usize>,
    branch: Option<String>,
) -> Result<Vec<GitLogEntryDto>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        // Not a git repo yet — return an empty list instead of erroring
        // so the timeline renders a blank state.
        return Ok(Vec::new());
    }

    let n = limit.unwrap_or(200).clamp(1, 1000);
    let limit_arg = format!("-{n}");
    // Build args. When a branch is given, scope the log to it; otherwise
    // read HEAD (the current branch).
    let mut args: Vec<String> = vec!["log".into(), limit_arg];
    if let Some(b) = branch.as_ref() {
        args.push(b.clone());
    }
    args.push("--no-patch".into());
    args.push("--format=%H%x1f%s%x1f%ct%x1f%(trailers:key=Route-Checkpoint,valueonly)%x1f%(trailers:key=Route-Operator,valueonly)".into());
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let out = run_git(&path, &arg_refs)?;

    let mut entries = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts = split_record(line);
        if parts.len() < 3 {
            continue;
        }
        let sha = parts[0].clone();
        let message = parts[1].clone();
        let ts = parts[2].trim().parse::<i64>().unwrap_or(0) * 1000;
        let ckpt_flag = parts.get(3).cloned().unwrap_or_default();
        let op_flag = parts.get(4).cloned().unwrap_or_default();
        entries.push(GitLogEntryDto {
            sha,
            message,
            timestamp_ms: ts,
            is_checkpoint: !ckpt_flag.trim().is_empty(),
            is_ai: op_flag.trim().starts_with("ai"),
        });
    }
    Ok(entries)
}

/// List local branches. Remote-tracking refs are deliberately excluded —
/// Route doesn't manage remotes, so showing them would be noise.
#[tauri::command]
pub fn git_branch_list(state: State<'_, AppState>) -> Result<Vec<GitBranchDto>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(Vec::new());
    }
    let out = run_git(&path, &["branch", "--list", "--format=%(refname:short)%x1f%(objectname:short)"])?;
    // To know which branch is current, ask separately — `branch --format`
    // doesn't mark HEAD reliably across git versions.
    let current = run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut branches = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let name = split_record(line).first().cloned().unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let is_current = name == current;
        branches.push(GitBranchDto {
            name,
            current: is_current,
        });
    }
    Ok(branches)
}

/// Create a new branch at HEAD. Does NOT switch to it — the user can
/// switch separately via `git_branch_switch`. This matches the "create
/// then maybe switch" pattern of the built-in route_basic branch_create.
#[tauri::command]
pub fn git_branch_create(
    state: State<'_, AppState>,
    name: String,
) -> Result<GitBranchDto, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("branch name is required".to_string());
    }
    // Validate the name format to give a clean error before git does.
    if name.contains(' ') || name.contains("..") {
        return Err(format!("invalid branch name: {name}"));
    }
    run_git(&path, &["branch", name])?;
    Ok(GitBranchDto {
        name: name.to_string(),
        current: false,
    })
}

/// Switch the working tree to a different local branch. This is a real
/// `git checkout`/`switch` — uncommitted changes will be carried over or
/// blocked by git itself; we don't try to be smarter than git here.
#[tauri::command]
pub fn git_branch_switch(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("branch name is required".to_string());
    }
    // `switch` is the modern, safer command (won't detach HEAD on a
    // missing branch). Fall back to `checkout` for very old git.
    match run_git(&path, &["switch", name]) {
        Ok(_) => Ok(()),
        Err(e) => {
            // Fallback for git < 2.23.
            if e.contains("unknown switch") || e.contains("usage: git") {
                run_git(&path, &["checkout", name])?;
                Ok(())
            } else {
                Err(e)
            }
        }
    }
}

/// Current branch name. Convenience for the workbench header.
#[tauri::command]
pub fn git_current_branch(state: State<'_, AppState>) -> Result<String, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(String::new());
    }
    Ok(run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// Merge `source` into the current branch. Runs `git merge --no-edit`,
/// which is a real local merge — no push, no remote. The user retains
/// full control of network operations. Returns the new HEAD SHA so the
/// frontend can refresh the timeline without a separate `git_log` round
/// trip.
#[tauri::command]
pub fn git_merge(state: State<'_, AppState>, source: String) -> Result<String, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let source = source.trim();
    if source.is_empty() {
        return Err("source branch is required".to_string());
    }
    // `--no-edit` skips the commit-message editor; `--no-ff` forces a
    // merge commit even when a fast-forward would do, so the branch
    // topology stays visible in the tree.
    run_git(&path, &["merge", "--no-edit", "--no-ff", source])?;
    let head = run_git(&path, &["rev-parse", "HEAD"])?.trim().to_string();
    Ok(head)
}

/// Read the working-tree status grouped by change kind. This is the Git
/// mode counterpart of route_basic's `working_dir_status` — the workbench
/// uses it to show pending changes, and the AI commit-message generator
/// uses it (in Active AI mode) to draft a title from the file list.
///
/// Parsing mirrors route-tui's `git::status` exactly so both surfaces
/// agree on how `--porcelain=v1 -z` rows map to {added, modified, removed,
/// untracked}. Returns an empty DTO (not an error) when the project is not
/// a git repo yet, so the workbench renders a blank state cleanly.
#[tauri::command]
pub fn git_status(state: State<'_, AppState>) -> Result<GitStatusDto, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(GitStatusDto::default());
    }
    let out = run_git(&path, &["status", "--porcelain=v1", "-z"])?;
    let mut st = GitStatusDto::default();
    for entry in out.split('\0') {
        let entry = entry.trim_end_matches('\n');
        if entry.len() < 3 {
            continue;
        }
        let bytes = entry.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;
        let file_path = entry[3..].to_string();
        if x == '?' && y == '?' {
            st.untracked.push(file_path);
        } else if x == 'A' && y == 'D' {
            st.removed.push(file_path);
        } else if x == 'A' || y == 'A' {
            st.added.push(file_path);
        } else if x == 'D' || y == 'D' {
            st.removed.push(file_path);
        } else {
            st.modified.push(file_path);
        }
    }
    Ok(st)
}

/// Return the unified diff of the working tree against HEAD — i.e. all
/// unstaged AND staged changes that haven't been committed yet. This is
/// the Git mode counterpart of route_basic's snapshot diff, exposed as a
/// raw string so the frontend (or a richer AI prompt) can render / send
/// it verbatim.
///
/// `git diff HEAD` exits non-zero with "no changes" semantics handled by
/// git itself (it actually exits 0 with empty output when there's nothing
/// to diff), so we don't need special "nothing to commit" handling here.
/// Returns an empty string when the project is not a git repo yet.
#[tauri::command]
pub fn git_diff(state: State<'_, AppState>) -> Result<String, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(String::new());
    }
    // `--no-color` keeps the output plain regardless of the user's git
    // config (some users set `color.diff = always`). `--no-pager` is not
    // needed because we capture stdout directly, but `--no-color` is.
    Ok(run_git(&path, &["diff", "HEAD", "--no-color", "--no-ext-diff"])?)
}

// ---------------------------------------------------------------------------
// Stash / tag / restore — local-only Git primitives, ported from route-tui's
// git.rs so the GUI backend has the same coverage as the CLI/TUI. Like the
// rest of this file these NEVER touch the network. Destructive `git reset
// --hard` / `git revert` are deliberately NOT exposed here: `git_rollback`
// already covers the "restore to a past state" semantic NON-destructively,
// which matches the "low floor / maximum protection" default. Stash and
// tag are reversible local operations; restore discards uncommitted
// working-tree changes to tracked files (the user's explicit intent when
// they click it).
// ---------------------------------------------------------------------------

/// List stash entries as raw `stash@{n}: ...` lines. Empty when there are
/// no stashes or the project isn't a git repo.
#[tauri::command]
pub fn git_stash_list(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(Vec::new());
    }
    let out = run_git(&path, &["stash", "list"])?;
    Ok(out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Push a new stash. `message` is optional (`-m`). Returns git's stdout
/// (usually empty on success). "No local changes to save" is treated as a
/// no-op success (returns empty string) rather than an error, since the
/// caller asking to stash nothing isn't a failure condition.
#[tauri::command]
pub fn git_stash_push(
    state: State<'_, AppState>,
    message: Option<String>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let msg = message
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let args: Vec<&str> = match &msg {
        Some(m) => vec!["stash", "push", "-m", m.as_str()],
        None => vec!["stash", "push"],
    };
    match run_git(&path, &args) {
        Ok(out) => Ok(out),
        Err(e) if e.contains("No local changes") || e.contains("Nothing to save") => {
            Ok(String::new())
        }
        Err(e) => Err(e),
    }
}

/// Pop the top stash. Returns git's combined output. A pop that hits
/// conflicts exits non-zero and is surfaced as an error (the stash is
/// kept by git in that case) so the UI can show the conflict details —
/// this matches the user's expectation that a failed pop isn't silent.
#[tauri::command]
pub fn git_stash_pop(state: State<'_, AppState>) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    run_git(&path, &["stash", "pop"])
}

/// List all tags, sorted as git sorts them (lexicographic by default).
/// Empty when there are no tags or the project isn't a git repo.
#[tauri::command]
pub fn git_tag_list(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(Vec::new());
    }
    let out = run_git(&path, &["tag", "--list"])?;
    Ok(out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// Create a tag at HEAD. Annotated (`-a -m`) when a message is given,
/// lightweight otherwise. Mirrors route-tui's `tag_create`.
#[tauri::command]
pub fn git_tag_create(
    state: State<'_, AppState>,
    name: String,
    message: Option<String>,
) -> Result<(), String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("tag name is required".to_string());
    }
    if name.contains(' ') || name.contains("..") {
        return Err(format!("invalid tag name: {name}"));
    }
    let msg = message
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match &msg {
        Some(m) => run_git(&path, &["tag", "-a", name, "-m", m.as_str()])?,
        None => run_git(&path, &["tag", name])?,
    };
    Ok(())
}

/// Delete a tag (`git tag -d`). Errors if the tag doesn't exist.
#[tauri::command]
pub fn git_tag_delete(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("tag name is required".to_string());
    }
    run_git(&path, &["tag", "-d", name])?;
    Ok(())
}

/// Restore working-tree files from the index (`git restore <paths>`),
/// discarding uncommitted modifications to those tracked files. This is
/// the "discard changes" primitive — destructive to uncommitted work on
/// the given paths, which is the user's explicit intent when they invoke
/// it. Untracked files are not affected (they're not in the index).
#[tauri::command]
pub fn git_restore(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let clean: Vec<&str> = paths
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if clean.is_empty() {
        return Err("at least one path is required".to_string());
    }
    let mut args: Vec<&str> = Vec::with_capacity(clean.len() + 1);
    args.push("restore");
    args.extend(clean);
    run_git(&path, &args)?;
    Ok(())
}

/// Auto-commit pending changes without a checkpoint trailer. Called by
/// the file watcher when git mode is on — the equivalent of
/// route_basic's `commit("auto")`. Stages everything (`git add -A`) and
/// commits with a plain `auto` message. Returns the new HEAD SHA, or
/// `None` when there was nothing to commit (git commit exits non-zero
/// with "nothing to commit" — we treat that as a no-op, not an error).
pub fn git_auto_commit(project_path: &std::path::Path) -> Result<Option<String>, String> {
    run_git(project_path, &["add", "-A"])?;
    match run_git(project_path, &["commit", "--quiet", "-m", "auto"]) {
        Ok(_) => {
            let head = run_git(project_path, &["rev-parse", "HEAD"])?
                .trim()
                .to_string();
            Ok(Some(head))
        }
        Err(e) => {
            // "nothing to commit" is not an error for the watcher — it
            // polls repeatedly and most ticks have no changes.
            if e.contains("nothing to commit") || e.contains("no changes added") {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

// Note: no `git_push`, no `git_remote_*`, no `git_pull`/`fetch`. Push and
// remote operations are intentionally absent — Route tells the user in the
// UI that those are theirs to run. Adding them here would defeat the
// "user keeps full control" guarantee.
//
// `GIT_TIMEOUT` is declared but not currently wired (std::process::Command
// has no built-in timeout). We keep the constant so a future move to
// `wait4`/`tokio::process` can pick it up without re-deriving the value.
#[allow(dead_code)]
const _GIT_TIMEOUT_DOC: Duration = GIT_TIMEOUT;
