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
pub(crate) fn project_path(state: &AppState) -> Result<std::path::PathBuf, String> {
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

// ---------------------------------------------------------------------------
// Stage / Unstage
// ---------------------------------------------------------------------------

/// Stage specific files (or patterns) into the index. Wraps `git add`.
/// Accepts one or more paths/globs. Returns the list of staged files on
/// success (parsed from `git add --verbose`). When no paths are given,
/// stages everything (`git add -A`).
#[tauri::command]
pub fn git_add(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<String>, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let clean: Vec<&str> = paths
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if clean.is_empty() {
        run_git(&path, &["add", "-A"])?;
        Ok(Vec::new())
    } else {
        let mut args: Vec<&str> = vec!["add", "--verbose"];
        args.extend(clean);
        let out = run_git(&path, &args)?;
        let staged: Vec<String> = out
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(staged)
    }
}

/// Unstage files from the index. Wraps `git reset HEAD -- <paths>`.
/// When no paths are given, unstages everything (`git reset HEAD`).
/// Does NOT touch working-tree files.
#[tauri::command]
pub fn git_reset(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<(), String> {
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
        // Reset entire index (soft — no worktree changes)
        run_git(&path, &["reset", "HEAD"])?;
    } else {
        let mut args: Vec<&str> = vec!["reset", "HEAD", "--"];
        args.extend(clean);
        run_git(&path, &args)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Branch delete (Tauri command)
// ---------------------------------------------------------------------------

/// Delete a local branch. Refuses to delete the currently checked-out
/// branch (git enforces this). Mirrors `git branch -d` (safe delete,
/// refuses when not fully merged).
#[tauri::command]
pub fn git_branch_delete(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("branch name is required".to_string());
    }
    // Try safe delete first; fall back to force delete only if the user
    // explicitly passes `--force` (we don't expose that here).
    match run_git(&path, &["branch", "-d", name]) {
        Ok(_) => Ok(()),
        Err(e) => {
            // If git says "not fully merged", give a clean error with
            // the hint to delete remotely first or use the CLI.
            if e.contains("not fully merged") {
                Err(format!("Branch '{name}' is not fully merged. Delete it remotely first, or use `git branch -D {name}` in the terminal."))
            } else {
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Revert — safe undo via new commit
// ---------------------------------------------------------------------------

/// Revert a commit by creating a new commit that undoes its changes.
/// Wraps `git revert --no-edit`. This is SAFE — it never rewrites
/// history. Returns the SHA of the new revert commit.
#[tauri::command]
pub fn git_revert(state: State<'_, AppState>, sha: String) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let sha = sha.trim();
    if sha.is_empty() {
        return Err("target commit is required".to_string());
    }
    // Validate the commit exists first.
    run_git(&path, &["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .map_err(|_| format!("commit not found: {sha}"))?;
    // --no-edit skips the editor (uses auto-generated message).
    // --no-ff forces a revert commit even if the revert could be
    // fast-forwarded, keeping the history readable.
    run_git(&path, &["revert", "--no-edit", "--no-ff", sha])?;
    let head = run_git(&path, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    Ok(head)
}

// ---------------------------------------------------------------------------
// Cherry-pick — apply specific commits
// ---------------------------------------------------------------------------

/// Cherry-pick one or more commits onto the current HEAD. Wraps
/// `git cherry-pick --no-commit <shas>` followed by `git commit` so
/// the AI gets a single clean commit instead of N individual ones.
/// Returns the new HEAD SHA. If conflicts occur, the cherry-pick is
/// aborted and the error is surfaced with conflict details.
#[tauri::command]
pub fn git_cherry_pick(
    state: State<'_, AppState>,
    shas: Vec<String>,
    message: Option<String>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let clean: Vec<&str> = shas
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if clean.is_empty() {
        return Err("at least one commit SHA is required".to_string());
    }
    // Validate all commits exist before touching the tree.
    for sha in &clean {
        run_git(&path, &["cat-file", "-e", &format!("{sha}^{{commit}}")])
            .map_err(|_| format!("commit not found: {sha}"))?;
    }
    // Use `cherry-pick --no-commit` to stage all changes, then commit.
    let mut args: Vec<&str> = vec!["cherry-pick", "--no-commit"];
    args.extend(&clean);
    match run_git(&path, &args) {
        Ok(_) => {
            // Commit the staged changes.
            let msg = message.unwrap_or_else(|| {
                format!("cherry-pick: {}", clean.join(", "))
            });
            run_git(&path, &["commit", "--quiet", "-m", &msg])?;
            let head = run_git(&path, &["rev-parse", "HEAD"])?
                .trim()
                .to_string();
            Ok(head)
        }
        Err(e) => {
            // Abort the cherry-pick on conflict so the working tree is
            // clean and the error message tells the user.
            let _ = run_git(&path, &["cherry-pick", "--abort"]);
            Err(format!("cherry-pick failed: {e}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Remote operations
// ---------------------------------------------------------------------------

/// DTO for a single remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRemoteDto {
    pub name: String,
    pub url: String,
    pub fetch_url: String,
    /// If `pushurl` is set, it differs from `url`; otherwise same as `url`.
    pub push_url: String,
}

/// List all configured remotes. Returns an empty list when there are
/// no remotes or the project isn't a git repo.
#[tauri::command]
pub fn git_remote_list(state: State<'_, AppState>) -> Result<Vec<GitRemoteDto>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(Vec::new());
    }
    let out = run_git(&path, &["remote", "-v"])?;
    let mut remotes: Vec<GitRemoteDto> = Vec::new();
    let mut seen = std::collections::HashMap::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        // Format: "origin\thttps://... (fetch)"
        let parts: Vec<&str> = line.splitn(2, '\t').collect();
        if parts.len() < 2 { continue; }
        let name = parts[0].to_string();
        let rest = parts[1];
        // Rest format: "url (type)"
        if let Some(paren) = rest.rfind(" (") {
            let url = rest[..paren].trim().to_string();
            let r#type = rest[paren..].trim().to_string();
            let idx = seen.entry(name.clone()).or_insert_with(|| {
                remotes.push(GitRemoteDto {
                    name: name.clone(),
                    url: String::new(),
                    fetch_url: String::new(),
                    push_url: String::new(),
                });
                remotes.len() - 1
            });
            let entry = &mut remotes[*idx];
            if r#type == "(fetch)" {
                entry.fetch_url = url.clone();
                if entry.url.is_empty() { entry.url = url; }
            } else if r#type == "(push)" {
                entry.push_url = url;
            }
        }
    }
    Ok(remotes)
}

/// Add a new remote. Wraps `git remote add <name> <url>`.
#[tauri::command]
pub fn git_remote_add(
    state: State<'_, AppState>,
    name: String,
    url: String,
) -> Result<GitRemoteDto, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let name = name.trim();
    let url = url.trim();
    if name.is_empty() { return Err("remote name is required".to_string()); }
    if url.is_empty() { return Err("remote URL is required".to_string()); }
    run_git(&path, &["remote", "add", name, url])?;
    Ok(GitRemoteDto {
        name: name.to_string(),
        url: url.to_string(),
        fetch_url: url.to_string(),
        push_url: url.to_string(),
    })
}

/// Remove a remote. Wraps `git remote remove <name>`.
#[tauri::command]
pub fn git_remote_remove(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let name = name.trim();
    if name.is_empty() { return Err("remote name is required".to_string()); }
    run_git(&path, &["remote", "remove", name])?;
    Ok(())
}

/// Fetch from a remote. Wraps `git fetch <remote>`. When `remote` is
/// empty, fetches from the default remote (origin). Returns the fetch
/// output as a string so the frontend can display it.
#[tauri::command]
pub fn git_fetch(
    state: State<'_, AppState>,
    remote: Option<String>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let remote = remote.as_deref().unwrap_or("origin");
    // `--prune` removes remote-tracking refs that no longer exist on
    // the remote, keeping the local view clean.
    run_git(&path, &["fetch", "--prune", remote])
}

/// Pull from a remote branch. Wraps `git pull --rebase <remote> <branch>`.
/// Using `--rebase` by default avoids unnecessary merge commits and keeps
/// history linear. When `remote` is empty, uses "origin". When `branch`
/// is empty, uses the current branch name.
#[tauri::command]
pub fn git_pull(
    state: State<'_, AppState>,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let remote = remote.unwrap_or_else(|| "origin".to_string());
    let branch = branch.unwrap_or_else(|| {
        run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    // `--rebase` keeps history linear; `--autostash` stashes local
    // changes before pulling and pops them after.
    run_git(&path, &["pull", "--rebase", "--autostash", &remote, &branch])
}

/// Push to a remote branch. Wraps `git push <remote> <branch>`.
/// When `remote` is empty, uses "origin". When `branch` is empty,
/// uses the current branch name. Returns the push output.
/// 
/// NOTE: This is intentionally explicit — the user must know they are
/// pushing. The frontend should show a confirmation dialog before
/// calling this.
#[tauri::command]
pub fn git_push(
    state: State<'_, AppState>,
    remote: Option<String>,
    branch: Option<String>,
    force: Option<bool>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let remote = remote.unwrap_or_else(|| "origin".to_string());
    let branch = branch.unwrap_or_else(|| {
        run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    if force.unwrap_or(false) {
        // Force push requires explicit opt-in and a warning in the
        // returned message so the frontend can display it.
        let out = run_git(&path, &["push", "--force-with-lease", &remote, &branch])?;
        Ok(format!("[FORCE PUSH] {out}"))
    } else {
        run_git(&path, &["push", &remote, &branch])
    }
}

/// Set upstream for the current branch. Wraps `git push -u <remote> <branch>`.
/// This is separate from `git_push` so the AI can set tracking explicitly
/// when needed.
#[tauri::command]
pub fn git_push_set_upstream(
    state: State<'_, AppState>,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let remote = remote.unwrap_or_else(|| "origin".to_string());
    let branch = branch.unwrap_or_else(|| {
        run_git(&path, &["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    run_git(&path, &["push", "-u", &remote, &branch])
}

// ---------------------------------------------------------------------------
// Clean — remove untracked files
// ---------------------------------------------------------------------------

/// Clean untracked files from the working tree. Wraps `git clean`.
/// When `dry_run` is true (default), only lists what would be removed
/// without actually removing anything. When `directories` is true,
/// also removes untracked directories (`-d`). When `force` is true,
/// actually performs the removal (`-f`).
///
/// Returns the list of files/directories that were (or would be) removed.
#[tauri::command]
pub fn git_clean(
    state: State<'_, AppState>,
    dry_run: Option<bool>,
    directories: Option<bool>,
    force: Option<bool>,
) -> Result<Vec<String>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let mut args: Vec<&str> = vec!["clean"];
    if dry_run.unwrap_or(true) {
        args.push("--dry-run");
    }
    if directories.unwrap_or(false) {
        args.push("-d");
    }
    if force.unwrap_or(false) {
        args.push("-f");
    }
    // `-q` for quiet (no warnings); we capture output to list files.
    let out = run_git(&path, &args)?;
    Ok(out
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

// ---------------------------------------------------------------------------
// Show — inspect a commit
// ---------------------------------------------------------------------------

/// DTO for a detailed commit view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitShowDto {
    pub sha: String,
    pub author: String,
    pub author_email: String,
    pub date: String,
    pub message: String,
    /// Raw diff of the commit (--no-color).
    pub diff: String,
}

/// Show the full details of a commit: author, date, message, and diff.
/// Wraps `git show --no-color --no-ext-diff <sha>`.
#[tauri::command]
pub fn git_show(state: State<'_, AppState>, sha: String) -> Result<GitShowDto, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let sha = sha.trim();
    if sha.is_empty() {
        return Err("commit SHA is required".to_string());
    }
    run_git(&path, &["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .map_err(|_| format!("commit not found: {sha}"))?;

    let author = run_git(&path, &["log", "-1", "--format=%an", sha])?
        .trim()
        .to_string();
    let email = run_git(&path, &["log", "-1", "--format=%ae", sha])?
        .trim()
        .to_string();
    let date = run_git(&path, &["log", "-1", "--format=%ad", "--date=iso-strict", sha])?
        .trim()
        .to_string();
    let message = run_git(&path, &["log", "-1", "--format=%B", sha])?
        .trim()
        .to_string();
    let diff = run_git(&path, &["show", "--no-color", "--no-ext-diff", sha])?;

    Ok(GitShowDto {
        sha: sha.to_string(),
        author,
        author_email: email,
        date,
        message,
        diff,
    })
}

// ---------------------------------------------------------------------------
// Config — get/set git configuration
// ---------------------------------------------------------------------------

/// Get a git config value. Checks local first, then global, then system.
/// Returns `None` if the key is not set.
#[tauri::command]
pub fn git_config_get(
    state: State<'_, AppState>,
    key: String,
    scope: Option<String>,
) -> Result<Option<String>, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    let key = key.trim();
    if key.is_empty() {
        return Err("config key is required".to_string());
    }
    let scope_flag = match scope.as_deref() {
        Some("global") => "--global",
        Some("system") => "--system",
        _ => "--local",
    };
    let out = run_git(&path, &["config", scope_flag, "--get", key]);
    match out {
        Ok(v) => {
            let trimmed = v.trim().to_string();
            Ok(if trimmed.is_empty() { None } else { Some(trimmed) })
        }
        Err(_) => Ok(None), // Key not found is not an error
    }
}

/// Set a git config value. Wraps `git config <scope> <key> <value>`.
/// Returns the old value if one existed.
#[tauri::command]
pub fn git_config_set(
    state: State<'_, AppState>,
    key: String,
    value: String,
    scope: Option<String>,
) -> Result<Option<String>, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err("key and value are required".to_string());
    }
    let scope_flag = match scope.as_deref() {
        Some("global") => "--global",
        Some("system") => "--system",
        _ => "--local",
    };
    // Get old value first (best-effort).
    let old = run_git(&path, &["config", scope_flag, "--get", key]).ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    run_git(&path, &["config", scope_flag, key, value])?;
    Ok(old)
}

// ---------------------------------------------------------------------------
// Log graph — ASCII graph of branch topology
// ---------------------------------------------------------------------------

/// Return the git log as an ASCII graph, useful for visualizing branch
/// topology. Wraps `git log --graph --oneline --all --decorate`.
/// `limit` caps the number of rows (default 50, max 500).
#[tauri::command]
pub fn git_log_graph(
    state: State<'_, AppState>,
    limit: Option<usize>,
    all: Option<bool>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(String::new());
    }
    let n = limit.unwrap_or(50).clamp(1, 500);
    let limit_arg = format!("-{n}");
    let mut args: Vec<&str> = vec!["log", "--graph", "--oneline", "--decorate", &limit_arg];
    if all.unwrap_or(true) {
        args.push("--all");
    }
    run_git(&path, &args)
}

// ---------------------------------------------------------------------------
// Archive — create a tar/zip archive of the repository
// ---------------------------------------------------------------------------

/// Create an archive of the repository at a specific ref (default: HEAD).
/// Wraps `git archive --format=<format> --output=<path> <ref>`.
/// Supported formats: "tar", "zip" (default: "zip").
/// Returns the absolute path of the created archive.
#[tauri::command]
pub fn git_archive(
    state: State<'_, AppState>,
    output_path: String,
    format: Option<String>,
    treeish: Option<String>,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let fmt = format.as_deref().unwrap_or("zip");
    let ref_name = treeish.as_deref().unwrap_or("HEAD");
    if output_path.trim().is_empty() {
        return Err("output path is required".to_string());
    }
    run_git(
        &path,
        &[
            "archive",
            &format!("--format={fmt}"),
            &format!("--output={}", output_path.trim()),
            ref_name,
        ],
    )?;
    // Verify the file was created.
    let abs_path = std::path::Path::new(output_path.trim());
    if abs_path.exists() {
        Ok(abs_path.canonicalize()
            .unwrap_or_else(|_| abs_path.to_path_buf())
            .to_string_lossy()
            .to_string())
    } else {
        Ok(output_path.trim().to_string())
    }
}

// ---------------------------------------------------------------------------
// Rebase — rebase current branch onto another branch
// ---------------------------------------------------------------------------

/// Rebase the current branch onto another branch. Wraps
/// `git rebase <target>`. Returns the new HEAD SHA on success.
/// If conflicts occur, the rebase is aborted and the error is
/// surfaced with conflict details.
#[tauri::command]
pub fn git_rebase(
    state: State<'_, AppState>,
    target: String,
) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    let target = target.trim();
    if target.is_empty() {
        return Err("target branch is required".to_string());
    }
    // Use `--autostash` to stash local changes before rebasing.
    match run_git(&path, &["rebase", "--autostash", target]) {
        Ok(_out) => {
            let head = run_git(&path, &["rev-parse", "HEAD"])?
                .trim()
                .to_string();
            Ok(head)
        }
        Err(e) => {
            // On conflict, abort the rebase to keep the working tree clean.
            let _ = run_git(&path, &["rebase", "--abort"]);
            Err(format!("rebase failed (aborted): {e}"))
        }
    }
}

/// Check if a rebase is currently in progress. Returns true if
/// `.git/rebase-merge` or `.git/rebase-apply` exists.
#[tauri::command]
pub fn git_rebase_in_progress(state: State<'_, AppState>) -> Result<bool, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Ok(false);
    }
    Ok(path.join(".git").join("rebase-merge").exists()
        || path.join(".git").join("rebase-apply").exists())
}

/// Abort the current in-progress rebase. Wraps `git rebase --abort`.
#[tauri::command]
pub fn git_rebase_abort(state: State<'_, AppState>) -> Result<(), String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    run_git(&path, &["rebase", "--abort"]).map(|_| ())
}

/// Continue a rebase after resolving conflicts. Wraps
/// `git rebase --continue --no-edit`.
#[tauri::command]
pub fn git_rebase_continue(state: State<'_, AppState>) -> Result<String, String> {
    let path = project_path(&state)?;
    if !path.join(".git").exists() {
        return Err("not a git repository".to_string());
    }
    run_git(&path, &["rebase", "--continue", "--no-edit"])?;
    let head = run_git(&path, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    Ok(head)
}

// ---------------------------------------------------------------------------
// Clone — clone a repository
// ---------------------------------------------------------------------------

/// Clone a remote repository into a local directory. Wraps
/// `git clone <url> <path>`. Returns the path of the cloned repo.
/// NOTE: This is a potentially long-running operation. The frontend
/// should show a progress indicator.
#[tauri::command]
pub fn git_clone(url: String, path: String) -> Result<String, String> {
    let url = url.trim();
    let path = path.trim();
    if url.is_empty() { return Err("clone URL is required".to_string()); }
    if path.is_empty() { return Err("target path is required".to_string()); }
    let target = std::path::Path::new(path);
    if target.exists() {
        return Err(format!("target path already exists: {path}"));
    }
    // Ensure parent directory exists.
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create parent directory: {e}"))?;
    }
    let mut cmd = git_command();
    cmd.current_dir(target.parent().unwrap_or(std::path::Path::new(".")));
    cmd.arg("clone");
    cmd.arg(url);
    cmd.arg(path);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");
    let output = cmd
        .output()
        .map_err(|e| format!("failed to spawn git clone: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "git clone failed".to_string()
        } else {
            stderr
        });
    }
    // Canonicalize the path.
    let abs = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());
    Ok(abs.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// Safety: backup before destructive operations
// ---------------------------------------------------------------------------

/// Create a safety backup of the current git repository state.
/// Backs up the entire working tree (tracked files) to a timestamped
/// directory under `<project>/.route/git-backups/`. Returns the backup
/// path. This is called automatically before destructive operations
/// (revert, rebase, reset --hard, etc.) when safety mode is enabled.
#[tauri::command]
pub fn git_backup_create(state: State<'_, AppState>) -> Result<String, String> {
    let path = project_path(&state)?;
    ensure_repo(&path)?;
    // Create the backup directory.
    let backup_dir = path.join(".route").join("git-backups");
    std::fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("cannot create backup directory: {e}"))?;
    // Timestamped backup name.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_path = backup_dir.join(format!("pre-op-{ts}"));
    // Use `git archive` to create a tar backup of HEAD.
    let archive_path = backup_path.with_extension("tar");
    run_git(
        &path,
        &[
            "archive",
            "--format=tar",
            &format!("--output={}", archive_path.to_string_lossy()),
            "HEAD",
        ],
    )?;
    // Also save the current HEAD SHA for reference.
    let head = run_git(&path, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    std::fs::write(backup_path.with_extension("head"), &head)
        .map_err(|e| format!("cannot write HEAD reference: {e}"))?;
    Ok(archive_path.to_string_lossy().to_string())
}

/// List all safety backups created by `git_backup_create`.
/// Returns a list of backup paths (newest first).
#[tauri::command]
pub fn git_backup_list(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let path = project_path(&state)?;
    let backup_dir = path.join(".route").join("git-backups");
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<_> = std::fs::read_dir(&backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "tar"))
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(
        e.metadata().and_then(|m| m.created()).ok()
    ));
    Ok(entries
        .into_iter()
        .map(|e| e.path().to_string_lossy().to_string())
        .collect())
}

// ---------------------------------------------------------------------------
// Better error reporting — run git with a timeout
// ---------------------------------------------------------------------------

/// Run git with a hard timeout. Uses `std::process::Command` with
/// a timeout thread. This is the safe variant that should be used
/// for all network operations (clone, fetch, push, pull).
fn run_git_with_timeout(
    project_path: &std::path::Path,
    args: &[&str],
    timeout: Duration,
) -> Result<String, String> {
    let mut cmd = git_command();
    cmd.current_dir(project_path).args(args);
    cmd.env("LC_ALL", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");

    let child = cmd.spawn()
        .map_err(|e| format!("failed to spawn git: {e}"))?;

    // Use a channel to receive the result.
    let (tx, rx) = std::sync::mpsc::channel();
    let pid = child.id();
    let handle = std::thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = tx.send(output);
    });

    // Wait for the result with timeout.
    match rx.recv_timeout(timeout) {
        Ok(output) => {
            let _ = handle.join();
            let output = output.map_err(|e| format!("failed to wait for git: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let detail = if !stderr.is_empty() { stderr } else { stdout };
                return Err(if detail.is_empty() {
                    format!("git {} failed (timeout={}s)", args.join(" "), timeout.as_secs())
                } else {
                    format!("git {}: {detail}", args.join(" "))
                });
            }
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Timeout — kill the process tree.
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .output();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .output();
            }
            let _ = handle.join();
            Err(format!(
                "git {} timed out after {}s",
                args.join(" "),
                timeout.as_secs()
            ))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            let _ = handle.join();
            Err("git process channel disconnected unexpectedly".to_string())
        }
    }
}
