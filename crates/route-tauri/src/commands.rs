//! Tauri IPC commands wrapping BasicRepository.

use std::path::PathBuf;

use anyhow::Result;
use route_basic::{
    BasicRepository, BranchKind, CommitOptions, CreateBranchOptions, DiffSummary, ExportFormat,
};
use route_core::short_id;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// DTOs for IPC
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct StatusDto {
    pub project_path: String,
    pub mode: String,
    pub current_branch: String,
    pub branches: Vec<BranchDto>,
    pub latest_snapshot: Option<SnapshotDto>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BranchDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub parent_branch: Option<String>,
    pub baseline_snapshot: Option<String>,
    pub head_snapshot: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SnapshotDto {
    pub id: String,
    pub short_id: String,
    pub manifest_hash: String,
    pub created_at: i64,
    pub branch_id: Option<String>,
    pub branch_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitDto {
    pub id: String,
    pub short_id: String,
    pub from_snapshot: String,
    pub to_snapshot: String,
    pub message: String,
    pub author: Option<String>,
    pub created_at: i64,
    pub branch_id: String,
    pub branch_name: String,
    pub kind: String,
    pub diff_summary: Option<DiffSummary>,
    /// Who performed the change. "user" by default; "ai:<name>" for AI-driven commits.
    pub operator: Option<String>,
    /// Optional long-form note from the operator (used for checkpoint body / AI prompt).
    pub body: Option<String>,
    /// True if this commit is a user-marked checkpoint.
    pub is_checkpoint: bool,
    /// True if the change came from the AI control channel (CLI / MCP).
    pub is_ai: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnnotationDto {
    pub id: String,
    pub commit_id: String,
    pub text: String,
    pub created_at: i64,
}

/// Tree node for mindmap rendering.
#[derive(Debug, Serialize, Clone)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub kind: String, // "root" | "branch" | "snapshot" | "commit"
    pub branch_kind: Option<String>,
    pub snapshot_id: Option<String>,
    pub commit_id: Option<String>,
    pub message: Option<String>,
    pub annotations: Vec<String>,
    pub diff: Option<String>,
    pub children: Vec<TreeNode>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn branch_to_dto(b: &route_basic::Branch) -> BranchDto {
    BranchDto {
        id: b.id.clone(),
        name: b.name.clone(),
        kind: b.kind.as_str().to_string(),
        parent_branch: b.parent_branch.clone(),
        baseline_snapshot: b.baseline_snapshot.clone(),
        head_snapshot: b.head_snapshot.clone(),
        created_at: b.created_at,
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    Ok(folder.map(|p| p.to_string()))
}

/// Return a platform-appropriate directory where route can drop a
/// freshly-seeded example project. Used by the "Try example project"
/// button on the welcome page so the user doesn't have to pick a folder
/// just to see the demo. The directory is created if missing.
#[tauri::command]
pub fn default_seed_dir() -> Result<String, String> {
    use std::fs;
    let base = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .ok()
            .map(|p| std::path::PathBuf::from(p).join("Documents").join("Route Examples"))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .ok()
            .map(|p| std::path::PathBuf::from(p).join("Documents").join("Route Examples"))
    } else {
        // Linux / BSD / other Unix
        std::env::var("HOME")
            .ok()
            .map(|p| std::path::PathBuf::from(p).join("Documents").join("Route Examples"))
            .or_else(|| {
                std::env::var("XDG_DOCUMENTS_DIR")
                    .ok()
                    .map(std::path::PathBuf::from)
            })
    };
    let base = base.unwrap_or_else(|| std::env::temp_dir().join("route-examples"));
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    Ok(base.to_string_lossy().to_string())
}

#[tauri::command]
pub fn open_repo(path: String, state: State<'_, AppState>) -> Result<StatusDto, String> {
    let repo = BasicRepository::open(&path).map_err(|e| e.to_string())?;
    let dto = build_status(&repo).map_err(|e| e.to_string())?;
    state.set_repo(repo);
    Ok(dto)
}

#[tauri::command]
pub fn init_repo(path: String, state: State<'_, AppState>) -> Result<StatusDto, String> {
    let repo = BasicRepository::init(&path).map_err(|e| e.to_string())?;
    let dto = build_status(&repo).map_err(|e| e.to_string())?;
    state.set_repo(repo);
    Ok(dto)
}

/// Seed a fully-populated example project under `target_dir` and open
/// it as the active repo. Mirrors `init_repo` from the frontend's
/// perspective — the caller can drop the response into its existing
/// state without a refresh.
#[tauri::command]
pub fn seed_example(
    target_dir: String,
    state: State<'_, AppState>,
) -> Result<StatusDto, String> {
    crate::example_seed::seed_example_command(target_dir, state.inner())
}

/// Create a brand-new, completely empty Route workspace in a freshly
/// minted subfolder under the platform's "Route demos" location and
/// open it as the active repo. Used by the "Enter demo mode" button on
/// the welcome page — gives the user a private sandbox where they can
/// poke at the software without touching any real project.
///
/// The folder is named `demo-<timestamp>` (e.g. `demo-20260729-223045`)
/// so two clicks in the same second never collide. The user can also
/// pass an explicit `parent_dir` to override the default location
/// (the browser preview sends `null` to fall back to `default_seed_dir`).
#[tauri::command]
pub fn create_blank_demo(
    parent_dir: Option<String>,
    state: State<'_, AppState>,
) -> Result<StatusDto, String> {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let base = match parent_dir {
        Some(p) if !p.trim().is_empty() => std::path::PathBuf::from(p),
        _ => {
            // Fall back to the same default location as `default_seed_dir`.
            let dir = default_seed_dir()?;
            std::path::PathBuf::from(dir)
        }
    };
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;

    // Suffix the folder so two "Enter demo mode" clicks in the same
    // second don't try to reuse the same path.
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let folder_name = format!("demo-{}", secs);
    let project_path = base.join(&folder_name);
    // If somehow this exact second already exists, pick a different
    // suffix so the init doesn't fail.
    let project_path = if project_path.exists() {
        let mut i: u32 = 1;
        loop {
            let candidate = base.join(format!("{}-{}", folder_name, i));
            if !candidate.exists() {
                break candidate;
            }
            i += 1;
            if i > 1000 {
                return Err("could not allocate a unique demo folder".to_string());
            }
        }
    } else {
        project_path
    };

    let repo = BasicRepository::init(&project_path).map_err(|e| e.to_string())?;
    let dto = build_status(&repo).map_err(|e| e.to_string())?;
    state.set_repo(repo);
    Ok(dto)
}

#[tauri::command]
pub fn status(state: State<'_, AppState>) -> Result<StatusDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    build_status(repo).map_err(|e| e.to_string())
}

pub fn build_status(repo: &BasicRepository) -> Result<StatusDto> {
    let branches = repo.list_branches()?;
    let current = repo.get_current_branch_name()?;
    let history = repo.history(1)?;
    let latest = history.first().map(|s| SnapshotDto {
        id: s.snapshot.id.clone(),
        short_id: short_id(&s.snapshot.id),
        manifest_hash: s.snapshot.manifest_hash.clone(),
        created_at: s.snapshot.created_at,
        branch_id: s.branch_id.clone(),
        branch_name: s.branch_name.clone(),
    });
    Ok(StatusDto {
        project_path: repo.project_path().to_string_lossy().to_string(),
        mode: repo.config.mode.clone(),
        current_branch: current,
        branches: branches.iter().map(branch_to_dto).collect(),
        latest_snapshot: latest,
    })
}

#[tauri::command]
pub fn commit(
    message: String,
    author: Option<String>,
    full: bool,
    branch: Option<String>,
    operator: Option<String>,
    body: Option<String>,
    is_checkpoint: Option<bool>,
    is_ai: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CommitDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commit = repo
        .commit(CommitOptions {
            message,
            author,
            force_full: full,
            branch,
            operator,
            body,
            is_checkpoint: is_checkpoint.unwrap_or(false),
            is_ai: is_ai.unwrap_or(false),
        })
        .map_err(|e| e.to_string())?;
    commit_to_dto(repo, &commit).map_err(|e| e.to_string())
}

fn commit_to_dto(repo: &BasicRepository, c: &route_basic::Commit) -> Result<CommitDto> {
    let branch = repo.get_branch_by_id(&c.branch_id)?;
    let diff = c
        .diff_summary
        .as_deref()
        .map(|s| serde_json::from_str::<DiffSummary>(s).unwrap_or_default());
    Ok(CommitDto {
        id: c.id.clone(),
        short_id: short_id(&c.id),
        from_snapshot: c.from_snapshot.clone(),
        to_snapshot: c.to_snapshot.clone(),
        message: c.message.clone(),
        author: c.author.clone(),
        created_at: c.created_at,
        branch_id: c.branch_id.clone(),
        branch_name: branch.name,
        kind: c.kind.as_str().to_string(),
        diff_summary: diff,
        operator: c.operator.clone(),
        body: c.body.clone(),
        is_checkpoint: c.is_checkpoint,
        is_ai: c.is_ai,
    })
}

#[tauri::command]
pub fn log(
    limit: Option<usize>,
    branch: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<CommitDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commits = repo
        .list_commits(branch.as_deref(), limit.unwrap_or(50))
        .map_err(|e| e.to_string())?;
    commits
        .iter()
        .map(|c| commit_to_dto(repo, c))
        .collect::<Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rollback(
    snapshot_id: String,
    reason: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommitDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commit = repo
        .rollback_to(&snapshot_id, reason.as_deref())
        .map_err(|e| e.to_string())?;
    commit_to_dto(repo, &commit).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Checkpoint / Undo / Redo (Standard Mode core operations)
// ---------------------------------------------------------------------------

/// Mark a user checkpoint at the current head. The checkpoint has a
/// `title` (required, used as commit message) and an optional `body`
/// (free-form note).
#[tauri::command]
pub fn checkpoint_create(
    title: String,
    body: Option<String>,
    operator: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommitDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commit = repo
        .checkpoint_create(&title, body.as_deref(), operator.as_deref())
        .map_err(|e| e.to_string())?;
    commit_to_dto(repo, &commit).map_err(|e| e.to_string())
}

/// Undo the most recent non-rollback commit on the current branch.
/// Returns the new rollback commit that was created.
#[tauri::command]
pub fn undo_last(state: State<'_, AppState>) -> Result<CommitDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commit = repo.undo_last().map_err(|e| e.to_string())?;
    commit_to_dto(repo, &commit).map_err(|e| e.to_string())
}

/// Redo the most recently undone commit. Returns the new rollback
/// commit (kind=rollback, message="Redo") that was created.
#[tauri::command]
pub fn redo_last(state: State<'_, AppState>) -> Result<CommitDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commit = repo.redo_last().map_err(|e| e.to_string())?;
    commit_to_dto(repo, &commit).map_err(|e| e.to_string())
}

/// Whether the user can currently redo (the in-memory redo stack is non-empty).
#[tauri::command]
pub fn can_redo(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    Ok(repo.can_redo())
}

// ---------------------------------------------------------------------------
// File watcher (auto-tracking)
// ---------------------------------------------------------------------------

/// Start (or restart) the file-system watcher. If a watcher is already
/// running, it is stopped first. `interval_ms` is the polling cadence
/// (clamped to a 100ms floor).
#[tauri::command]
pub fn watch_start(
    interval_ms: u64,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::watcher::WatchStatusDto, String> {
    // Stop any existing watcher first.
    if let Ok(mut guard) = state.watcher.lock() {
        if let Some(mut h) = guard.take() {
            h.stop();
        }
    }

    let handle = crate::watcher::start(app.clone(), interval_ms);
    let project_path = state.project_path.lock().ok()
        .and_then(|p| p.clone())
        .map(|p| p.to_string_lossy().to_string());

    if let Ok(mut guard) = state.watcher.lock() {
        *guard = Some(handle);
    }

    Ok(crate::watcher::WatchStatusDto {
        running: true,
        interval_ms,
        project_path,
        last_commit_at: None,
        last_error: None,
        pending: false,
    })
}

/// Stop the running watcher. Returns the new (idle) status.
#[tauri::command]
pub fn watch_stop(state: State<'_, AppState>) -> Result<crate::watcher::WatchStatusDto, String> {
    if let Ok(mut guard) = state.watcher.lock() {
        if let Some(mut h) = guard.take() {
            h.stop();
        }
    }
    let project_path = state.project_path.lock().ok()
        .and_then(|p| p.clone())
        .map(|p| p.to_string_lossy().to_string());
    Ok(crate::watcher::WatchStatusDto {
        running: false,
        interval_ms: 0,
        project_path,
        last_commit_at: None,
        last_error: None,
        pending: false,
    })
}

/// Force the running watcher to commit any pending changes immediately.
/// Used by the workspace "Run" button so the user can persist their
/// in-flight edits without waiting for the memory-buffer window to
/// expire. No-op when no watcher is running.
#[tauri::command]
pub fn watch_flush(state: State<'_, AppState>) -> Result<crate::watcher::WatchStatusDto, String> {
    if let Ok(guard) = state.watcher.lock() {
        if let Some(h) = guard.as_ref() {
            h.request_flush();
        }
    }
    watch_status(state)
}

/// Query the current watcher state.
#[tauri::command]
pub fn watch_status(state: State<'_, AppState>) -> Result<crate::watcher::WatchStatusDto, String> {
    let guard = state.watcher.lock().unwrap_or_else(|p| p.into_inner());
    let project_path = state.project_path.lock().ok()
        .and_then(|p| p.clone())
        .map(|p| p.to_string_lossy().to_string());

    match guard.as_ref() {
        Some(h) => Ok(crate::watcher::WatchStatusDto {
            running: !h.stop.load(std::sync::atomic::Ordering::Relaxed),
            interval_ms: h.interval_ms,
            project_path,
            last_commit_at: None,
            last_error: None,
            pending: false,
        }),
        None => Ok(crate::watcher::WatchStatusDto {
            running: false,
            interval_ms: 0,
            project_path,
            last_commit_at: None,
            last_error: None,
            pending: false,
        }),
    }
}

#[tauri::command]
pub fn backup_to_dir(target: String, state: State<'_, AppState>) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let path = repo
        .full_backup_to_dir(&PathBuf::from(&target))
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn branch_list(state: State<'_, AppState>) -> Result<Vec<BranchDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let branches = repo.list_branches().map_err(|e| e.to_string())?;
    Ok(branches.iter().map(branch_to_dto).collect())
}

#[tauri::command]
pub fn branch_create(
    name: String,
    kind: String,
    from: Option<String>,
    state: State<'_, AppState>,
) -> Result<BranchDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let branch_kind = match kind.as_str() {
        "main" => BranchKind::Main,
        "inherited" => BranchKind::Inherited,
        "sandbox" => BranchKind::Sandbox,
        other => return Err(format!("Unknown kind: {other}")),
    };
    let current = repo
        .get_current_branch_name()
        .map_err(|e| e.to_string())?;
    let branch = repo
        .create_branch(
            &name,
            CreateBranchOptions {
                kind: branch_kind,
                from_branch: from,
            },
            &current,
        )
        .map_err(|e| e.to_string())?;
    Ok(branch_to_dto(&branch))
}

#[tauri::command]
pub fn branch_delete(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    repo.delete_branch(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn branch_switch(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    repo.set_current_branch(&name).map_err(|e| e.to_string())
}

/// Merge `source` branch into `target` (defaults to current). File-level
/// 3-way merge for inherited branches, 2-way for sandbox. Sandbox cannot
/// be a target. Conflicts resolve to "theirs" and are listed in the
/// commit message.
#[tauri::command]
pub fn branch_merge(
    source: String,
    target: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommitDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let commit = repo
        .merge(&source, target.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(commit_to_dto(repo, &commit).map_err(|e| e.to_string())?)
}

#[tauri::command]
pub fn annotate(
    commit_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<AnnotationDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let a = repo
        .add_path_annotation(&commit_id, &text)
        .map_err(|e| e.to_string())?;
    Ok(AnnotationDto {
        id: a.id,
        commit_id: a.commit_id,
        text: a.text,
        created_at: a.created_at,
    })
}

#[tauri::command]
pub fn list_annotations(
    commit_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AnnotationDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let list = repo
        .list_path_annotations(&commit_id)
        .map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|a| AnnotationDto {
            id: a.id,
            commit_id: a.commit_id,
            text: a.text,
            created_at: a.created_at,
        })
        .collect())
}

#[tauri::command]
pub fn export_data(
    format: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;

    let fmt = ExportFormat::from_str(&format)
        .ok_or_else(|| format!("Unknown format: {format}"))?;

    // File-based formats can't return a string — use export_file command instead.
    if fmt.is_file_based() {
        return Err(format!("Format {format} is file-based — use export_file command with a path"));
    }

    let ctx = repo.build_export_context().map_err(|e| e.to_string())?;

    let exporter = route_basic::DefaultExporters::for_format(fmt)
        .ok_or_else(|| "No exporter".to_string())?;
    let mut buf = Vec::new();
    exporter.export(&ctx, &mut buf).map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Export to a file/directory on disk (for ZIP and Folder formats).
#[tauri::command]
pub fn export_file(
    format: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;

    let fmt = ExportFormat::from_str(&format)
        .ok_or_else(|| format!("Unknown format: {format}"))?;

    let result = match fmt {
        ExportFormat::Zip => repo.export_zip(std::path::Path::new(&path)),
        ExportFormat::Folder => repo.export_folder(std::path::Path::new(&path)),
        _ => return Err(format!("Format {format} is not file-based — use export_data instead")),
    };
    result
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let branches = repo.list_branches().map_err(|e| e.to_string())?;
    let snapshots = repo.all_snapshots().map_err(|e| e.to_string())?;
    let commits = repo
        .list_commits(None, 10000)
        .map_err(|e| e.to_string())?;

    let main = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Main)
        .count();
    let inherited = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Inherited)
        .count();
    let sandbox = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Sandbox)
        .count();

    let inc = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Incremental)
        .count();
    let full = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Full)
        .count();
    let rollback = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Rollback)
        .count();
    let merge = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Merge)
        .count();

    Ok(serde_json::json!({
        "branches": {
            "total": branches.len(),
            "main": main,
            "inherited": inherited,
            "sandbox": sandbox,
        },
        "snapshots": snapshots.len(),
        "commits": {
            "total": commits.len(),
            "incremental": inc,
            "full": full,
            "rollback": rollback,
            "merge": merge,
        },
    }))
}

/// Build a tree for mindmap rendering: root → branch → snapshot chain (with commit edges as annotations).
#[tauri::command]
pub fn history_tree(state: State<'_, AppState>) -> Result<TreeNode, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;

    let branches = repo.list_branches().map_err(|e| e.to_string())?;
    let project_name = repo
        .project_path()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let mut branch_nodes = Vec::new();
    for b in &branches {
        // Get commits on this branch (ascending by time)
        let mut commits = repo
            .list_commits(Some(&b.name), 1000)
            .map_err(|e| e.to_string())?;
        commits.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        let mut snapshot_children: Vec<TreeNode> = Vec::new();
        let mut last_snapshot_id: Option<String> = None;

        for c in &commits {
            // Skip self-loop full backups in tree (they don't advance state)
            if c.kind == route_basic::CommitKind::Full && c.from_snapshot == c.to_snapshot {
                continue;
            }

            // If from != last_snapshot_id, this is a fork point — start new chain
            let annotations = repo
                .list_path_annotations(&c.id)
                .unwrap_or_default()
                .into_iter()
                .map(|a| a.text)
                .collect::<Vec<_>>();

            let diff = c
                .diff_summary
                .as_deref()
                .and_then(|s| {
                    serde_json::from_str::<DiffSummary>(s)
                        .ok()
                        .map(|d| d.short())
                });

            let node = TreeNode {
                id: format!("snap-{}", c.to_snapshot),
                label: short_id(&c.to_snapshot),
                kind: "snapshot".to_string(),
                branch_kind: None,
                snapshot_id: Some(c.to_snapshot.clone()),
                commit_id: Some(c.id.clone()),
                message: Some(c.message.clone()),
                annotations,
                diff,
                children: vec![],
            };
            snapshot_children.push(node);
            last_snapshot_id = Some(c.to_snapshot.clone());
        }
        let _ = last_snapshot_id;

        let branch_node = TreeNode {
            id: format!("branch-{}", b.id),
            label: b.name.clone(),
            kind: "branch".to_string(),
            branch_kind: Some(b.kind.as_str().to_string()),
            snapshot_id: b.head_snapshot.clone(),
            commit_id: None,
            message: None,
            annotations: vec![],
            diff: None,
            children: snapshot_children,
        };
        branch_nodes.push(branch_node);
    }

    Ok(TreeNode {
        id: "root".to_string(),
        label: project_name,
        kind: "root".to_string(),
        branch_kind: None,
        snapshot_id: None,
        commit_id: None,
        message: None,
        annotations: vec![],
        diff: None,
        children: branch_nodes,
    })
}

// ---------------------------------------------------------------------------
// File history & commit diff detail (Phase 5 capability extension)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct FileRevisionDto {
    pub snapshot_id: String,
    pub snapshot_short_id: String,
    pub commit_id: Option<String>,
    pub commit_message: Option<String>,
    pub blob_hash: Option<String>,
    pub created_at: i64,
    pub branch_name: Option<String>,
}

#[tauri::command]
pub fn file_history(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<FileRevisionDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let revs = repo.file_history(&path).map_err(|e| e.to_string())?;
    Ok(revs
        .into_iter()
        .map(|r| FileRevisionDto {
            snapshot_short_id: route_core::short_id(&r.snapshot_id),
            snapshot_id: r.snapshot_id,
            commit_id: r.commit_id,
            commit_message: r.commit_message,
            blob_hash: r.blob_hash,
            created_at: r.created_at,
            branch_name: r.branch_name,
        })
        .collect())
}

#[tauri::command]
pub fn restore_file(
    snapshot_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let target = repo
        .restore_file_from_snapshot(&snapshot_id, &path)
        .map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().to_string())
}

#[derive(Debug, Serialize, Clone)]
pub struct CommitDiffEntryDto {
    pub path: String,
    pub change: String,
    pub blob_hash: Option<String>,
    pub size_bytes: Option<u64>,
}

#[tauri::command]
pub fn commit_diff_detail(
    commit_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CommitDiffEntryDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let entries = repo
        .commit_diff_detail(&commit_id)
        .map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| CommitDiffEntryDto {
            path: e.path,
            change: e.change,
            blob_hash: e.blob_hash,
            size_bytes: e.size_bytes,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Working directory status, snapshot diff, tags (Phase 5 capability extension)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct WorkingFileStatusDto {
    pub path: String,
    pub change: String,
    pub current_hash: Option<String>,
    pub previous_hash: Option<String>,
    pub size_bytes: Option<u64>,
}

#[tauri::command]
pub fn working_dir_status(state: State<'_, AppState>) -> Result<Vec<WorkingFileStatusDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let entries = repo.working_dir_status().map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| WorkingFileStatusDto {
            path: e.path,
            change: e.change,
            current_hash: e.current_hash,
            previous_hash: e.previous_hash,
            size_bytes: e.size_bytes,
        })
        .collect())
}

#[derive(Debug, Serialize, Clone)]
pub struct SnapshotDiffEntryDto {
    pub path: String,
    pub change: String,
    pub from_hash: Option<String>,
    pub to_hash: Option<String>,
}

#[tauri::command]
pub fn diff_snapshots(
    from_snapshot: String,
    to_snapshot: String,
    state: State<'_, AppState>,
) -> Result<Vec<SnapshotDiffEntryDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let entries = repo
        .diff_snapshots(&from_snapshot, &to_snapshot)
        .map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| SnapshotDiffEntryDto {
            path: e.path,
            change: e.change,
            from_hash: e.from_hash,
            to_hash: e.to_hash,
        })
        .collect())
}

#[derive(Debug, Serialize, Clone)]
pub struct TagDto {
    pub id: String,
    pub name: String,
    pub snapshot_id: String,
    pub snapshot_short_id: String,
    pub message: Option<String>,
    pub created_at: i64,
}

#[tauri::command]
pub fn tag_list(state: State<'_, AppState>) -> Result<Vec<TagDto>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let tags = repo.tag_list().map_err(|e| e.to_string())?;
    Ok(tags
        .into_iter()
        .map(|t| TagDto {
            snapshot_short_id: short_id(&t.snapshot_id),
            id: t.id,
            name: t.name,
            snapshot_id: t.snapshot_id,
            message: t.message,
            created_at: t.created_at,
        })
        .collect())
}

#[tauri::command]
pub fn tag_create(
    name: String,
    snapshot_id: String,
    message: Option<String>,
    state: State<'_, AppState>,
) -> Result<TagDto, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let tag = repo
        .tag_create(&name, &snapshot_id, message.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(TagDto {
        snapshot_short_id: short_id(&tag.snapshot_id),
        id: tag.id,
        name: tag.name,
        snapshot_id: tag.snapshot_id,
        message: tag.message,
        created_at: tag.created_at,
    })
}

#[tauri::command]
pub fn tag_delete(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    repo.tag_delete(&name).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// AI operator state (CLI / MCP control channel)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct AiOperatorDto {
    pub name: String,
    pub prompt: String,
    pub since_ms: i64,
}

fn now_millis_local() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Take control of the software as an AI agent. Subsequent commits
/// and rollbacks will be stamped with `operator = "ai:<name>"` and the
/// `body` field will hold the prompt that triggered each change.
#[tauri::command]
pub fn set_ai_operator(
    name: String,
    prompt: String,
    state: State<'_, AppState>,
) -> Result<AiOperatorDto, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("AI operator name cannot be empty".to_string());
    }
    let op = crate::state::AiOperatorState {
        name: trimmed.to_string(),
        prompt,
        since_ms: now_millis_local(),
    };
    let dto = AiOperatorDto {
        name: op.name.clone(),
        prompt: op.prompt.clone(),
        since_ms: op.since_ms,
    };
    *state.ai_operator.lock().unwrap_or_else(|p| p.into_inner()) = Some(op);
    Ok(dto)
}

/// Release control back to the human user. After this, new commits
/// will be stamped with `operator = "user"` (the default).
#[tauri::command]
pub fn clear_ai_operator(state: State<'_, AppState>) -> Result<(), String> {
    *state.ai_operator.lock().unwrap_or_else(|p| p.into_inner()) = None;
    Ok(())
}

/// Returns the active AI operator, or `None` if no AI is currently
/// driving the software.
#[tauri::command]
pub fn get_ai_operator(
    state: State<'_, AppState>,
) -> Result<Option<AiOperatorDto>, String> {
    let guard = state.ai_operator.lock().unwrap_or_else(|p| p.into_inner());
    Ok(guard.as_ref().map(|o| AiOperatorDto {
        name: o.name.clone(),
        prompt: o.prompt.clone(),
        since_ms: o.since_ms,
    }))
}

// ---------------------------------------------------------------------------
// Track configuration + AI summary prompt + .route/index.json
// ---------------------------------------------------------------------------

/// Mirrors `route_basic::TrackConfig` over the IPC boundary. The
/// frontend edits this struct directly.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrackConfigDto {
    pub track_all: bool,
    pub track_suffixes: Vec<String>,
    pub track_prefixes: Vec<String>,
    pub verify_sha256: bool,
    pub memory_buffer_ms: u64,
    pub track_on: TrackOsDto,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
pub struct TrackOsDto {
    pub windows: bool,
    pub macos: bool,
    pub linux: bool,
}

impl From<&route_basic::TrackConfig> for TrackConfigDto {
    fn from(c: &route_basic::TrackConfig) -> Self {
        Self {
            track_all: c.track_all,
            track_suffixes: c.track_suffixes.clone(),
            track_prefixes: c.track_prefixes.clone(),
            verify_sha256: c.verify_sha256,
            memory_buffer_ms: c.memory_buffer_ms,
            track_on: TrackOsDto {
                windows: c.track_on.windows,
                macos: c.track_on.macos,
                linux: c.track_on.linux,
            },
        }
    }
}

impl From<TrackConfigDto> for route_basic::TrackConfig {
    fn from(d: TrackConfigDto) -> Self {
        Self {
            track_all: d.track_all,
            track_suffixes: d.track_suffixes,
            track_prefixes: d.track_prefixes,
            verify_sha256: d.verify_sha256,
            memory_buffer_ms: d.memory_buffer_ms,
            track_on: route_basic::TrackOs {
                windows: d.track_on.windows,
                macos: d.track_on.macos,
                linux: d.track_on.linux,
            },
        }
    }
}

/// Return the current `TrackConfig` for the active project.
#[tauri::command]
pub fn track_get(state: State<'_, AppState>) -> Result<TrackConfigDto, String> {
    let mut guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_mut().ok_or("No repository open")?;
    Ok(TrackConfigDto::from(repo.track_config()))
}

/// Replace the current `TrackConfig` and persist to disk.
#[tauri::command]
pub fn track_set(
    cfg: TrackConfigDto,
    state: State<'_, AppState>,
) -> Result<TrackConfigDto, String> {
    let mut guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_mut().ok_or("No repository open")?;
    let new_cfg: route_basic::TrackConfig = cfg.into();
    let saved = repo.set_track_config(new_cfg).map_err(|e| e.to_string())?;
    // Best-effort: regenerate the AI index so the agent immediately
    // sees the new rules.
    let _ = route_basic::write_index(repo);
    Ok(TrackConfigDto::from(&saved))
}

/// Convenience: toggle "track all files" without touching the suffix
/// or prefix lists.
#[tauri::command]
pub fn track_set_all(
    on: bool,
    state: State<'_, AppState>,
) -> Result<TrackConfigDto, String> {
    let mut guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_mut().ok_or("No repository open")?;
    repo.set_track_all(on).map_err(|e| e.to_string())?;
    let _ = route_basic::write_index(repo);
    Ok(TrackConfigDto::from(repo.track_config()))
}

/// Convenience: toggle SHA-256 verification.
#[tauri::command]
pub fn track_set_verify_sha256(
    on: bool,
    state: State<'_, AppState>,
) -> Result<TrackConfigDto, String> {
    let mut guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_mut().ok_or("No repository open")?;
    repo.set_verify_sha256(on).map_err(|e| e.to_string())?;
    let _ = route_basic::write_index(repo);
    Ok(TrackConfigDto::from(repo.track_config()))
}

/// Convenience: set the memory-buffer debounce. `ms = 0` disables
/// the buffer.
#[tauri::command]
pub fn track_set_memory_buffer_ms(
    ms: u64,
    state: State<'_, AppState>,
) -> Result<TrackConfigDto, String> {
    let mut guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_mut().ok_or("No repository open")?;
    repo.set_memory_buffer_ms(ms).map_err(|e| e.to_string())?;
    let _ = route_basic::write_index(repo);
    Ok(TrackConfigDto::from(repo.track_config()))
}

/// Convenience: update the OS enablement. The current OS is always
/// forced on (so the user can't lock themselves out); the others can
/// be toggled.
#[tauri::command]
pub fn track_set_on(
    on: TrackOsDto,
    state: State<'_, AppState>,
) -> Result<TrackConfigDto, String> {
    let mut guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_mut().ok_or("No repository open")?;
    let mut keep = route_basic::TrackOs {
        windows: on.windows,
        macos: on.macos,
        linux: on.linux,
    };
    if cfg!(target_os = "windows") { keep.windows = true; }
    if cfg!(target_os = "macos") { keep.macos = true; }
    if !cfg!(target_os = "windows") && !cfg!(target_os = "macos") { keep.linux = true; }
    repo.set_track_on(keep).map_err(|e| e.to_string())?;
    let _ = route_basic::write_index(repo);
    Ok(TrackConfigDto::from(repo.track_config()))
}

/// Return the canonical AI summary prompt. AI agents read this from
/// the index file (`.route/index.json`) at the start of every session
/// and follow its rules when producing commit bodies.
#[tauri::command]
pub fn ai_summary_prompt() -> String {
    route_basic::BasicRepository::ai_summary_prompt().to_string()
}

/// Regenerate the hidden `.route/index.json` from the current repo
/// state. Returns the absolute path of the file. The file is also
/// regenerated automatically at the end of every commit; this command
/// exists for the manual "AI wants a fresh view" use case.
#[tauri::command]
pub fn route_index_refresh(state: State<'_, AppState>) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let path = route_basic::write_index(repo).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Return the absolute path of the hidden `.route/index.json` for
/// the current repo, or `None` if the file hasn't been generated
/// yet. Used by the frontend to display the path to the user.
#[tauri::command]
pub fn route_index_path(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let path = repo.project_path().join(".route").join("index.json");
    if path.exists() {
        Ok(Some(path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// AI conflict resolution (智能取舍)
// ---------------------------------------------------------------------------

/// Parse the most recent AI commit on the current branch and return
/// any conflicts it surfaced. The frontend uses this to populate the
/// 智能取舍 dialog ("保留旧逻辑还是用 AI 新写的？").
#[tauri::command]
pub fn ai_conflict_report(state: State<'_, AppState>) -> Result<route_basic::AiConflictReport, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    repo.latest_ai_conflict_report().map_err(|e| e.to_string())
}

/// Payload accepted by `ai_conflict_resolve`.
#[derive(Debug, Deserialize)]
pub struct AiConflictResolvePayload {
    pub commit_id: String,
    pub path: String,
    /// "keep_old" | "keep_ai" | "keep_both"
    pub verdict: String,
    pub note: Option<String>,
}

/// Persist a verdict for one conflict. The verdicts are kept in the
/// timeline so the user can revisit what they decided months later.
#[tauri::command]
pub fn ai_conflict_resolve(
    payload: AiConflictResolvePayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    repo.record_conflict_verdict(
        &payload.commit_id,
        &payload.path,
        &payload.verdict,
        payload.note.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Return all verdicts recorded for the given commit.
#[tauri::command]
pub fn ai_conflict_list(
    commit_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<route_basic::AiConflictVerdict>, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    repo.list_conflict_verdicts(&commit_id)
        .map_err(|e| e.to_string())
}
