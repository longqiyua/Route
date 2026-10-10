//! Active Tracking Mode — Cloud Repository Local Auto-Backup
//!
//! Allows the user to specify local folders that Route should actively
//! track and keep in sync with a GitHub remote repository. This is a
//! Route-specific feature that goes beyond standard git by providing
//! automatic, scheduled synchronization.
//!
//! ## Features
//!
//! 1. **Folder tracking**: User specifies folders to watch.
//! 2. **Auto-sync**: Periodically fetches from the remote and merges.
//! 3. **Conflict detection**: Warns when local changes conflict with remote.
//! 4. **Sync history**: Records all sync operations with timestamps.
//! 5. **Configurable frequency**: User sets the update interval.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Tracking configuration
// ---------------------------------------------------------------------------

/// A single tracked folder configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFolder {
    /// Local folder path.
    pub local_path: String,
    /// GitHub remote URL (e.g., https://github.com/user/repo.git).
    pub remote_url: String,
    /// Branch to track (default: main).
    pub branch: String,
    /// Whether auto-sync is enabled for this folder.
    pub enabled: bool,
    /// Sync interval in seconds (default: 600 = 10 minutes).
    pub interval_secs: u64,
    /// Last sync timestamp (ISO 8601).
    pub last_sync: Option<String>,
    /// Last sync status: "ok", "conflict", "error", "pending".
    pub last_status: String,
    /// Whether this folder is currently being synced.
    pub syncing: bool,
}

impl Default for TrackedFolder {
    fn default() -> Self {
        Self {
            local_path: String::new(),
            remote_url: String::new(),
            branch: "main".to_string(),
            enabled: true,
            interval_secs: 600,
            last_sync: None,
            last_status: "pending".to_string(),
            syncing: false,
        }
    }
}

/// Full tracking configuration, stored in `.route/tracking.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackingConfig {
    pub folders: Vec<TrackedFolder>,
}

impl TrackingConfig {
    fn config_path(project_path: &Path) -> PathBuf {
        project_path.join(".route").join("tracking.json")
    }

    pub fn load(project_path: &Path) -> Self {
        let path = Self::config_path(project_path);
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, project_path: &Path) -> Result<(), String> {
        let path = Self::config_path(project_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create tracking dir: {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialization error: {e}"))?;
        std::fs::write(&path, &raw)
            .map_err(|e| format!("cannot write tracking config: {e}"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sync history
// ---------------------------------------------------------------------------

/// A single sync history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHistoryEntry {
    pub folder_path: String,
    pub timestamp: String,
    pub status: String, // "ok", "conflict", "error"
    pub message: String,
    pub changes_count: usize,
}

/// Sync history log, stored in `.route/tracking-history.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncHistory {
    pub entries: Vec<SyncHistoryEntry>,
}

impl SyncHistory {
    fn history_path(project_path: &Path) -> PathBuf {
        project_path.join(".route").join("tracking-history.json")
    }

    pub fn load(project_path: &Path) -> Self {
        let path = Self::history_path(project_path);
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, project_path: &Path) -> Result<(), String> {
        let path = Self::history_path(project_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create history dir: {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialization error: {e}"))?;
        std::fs::write(&path, &raw)
            .map_err(|e| format!("cannot write sync history: {e}"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sync logic
// ---------------------------------------------------------------------------

/// Execute a sync for a single tracked folder.
/// Returns the status and a message.
fn sync_folder(folder: &TrackedFolder) -> Result<(String, String, usize), String> {
    let local_path = Path::new(&folder.local_path);
    if !local_path.exists() {
        return Err(format!("local path does not exist: {}", folder.local_path));
    }

    // Check if it's a git repo. If not, we need to clone.
    let git_dir = local_path.join(".git");
    if !git_dir.exists() {
        // Clone the repo.
        run_git_clone(&folder.remote_url, &folder.local_path, &folder.branch)?;
    }

    // Fetch the latest.
    run_git_cmd(local_path, &["fetch", "--prune", "origin"])?;

    // Check for local changes.
    let status = run_git_cmd_output(local_path, &["status", "--porcelain"])?;
    let has_local_changes = !status.trim().is_empty();

    // Check if we're behind the remote.
    let behind = run_git_cmd_output(local_path, &[
        "rev-list",
        "--count",
        &format!("HEAD..origin/{}", folder.branch),
    ])?;
    let behind_count: usize = behind.trim().parse().unwrap_or(0);

    if has_local_changes && behind_count > 0 {
        // Conflict: local changes + remote updates.
        return Ok((
            "conflict".to_string(),
            format!(
                "Local changes detected and {} remote commit(s) available. \
                 Manual resolution required.",
                behind_count
            ),
            0,
        ));
    }

    if behind_count > 0 {
        // Pull with rebase and autostash.
        run_git_cmd(local_path, &[
            "pull",
            "--rebase",
            "--autostash",
            "origin",
            &folder.branch,
        ])?;
        return Ok((
            "ok".to_string(),
            format!("Synced {} remote commit(s)", behind_count),
            behind_count,
        ));
    }

    if has_local_changes {
        return Ok((
            "ok".to_string(),
            "Local changes present (no remote updates)".to_string(),
            0,
        ));
    }

    Ok(("ok".to_string(), "Already up to date".to_string(), 0))
}

fn run_git_cmd(cwd: &Path, args: &[&str]) -> Result<(), String> {
    let _output = run_git_cmd_output(cwd, args)?;
    Ok(())
}

fn run_git_cmd_output(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = std::process::Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.current_dir(cwd).args(args);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    let output = cmd.output().map_err(|e| format!("git command failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let err = if !stderr.is_empty() { stderr } else { stdout };
        return Err(format!("git {} failed: {err}", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_git_clone(url: &str, path: &str, branch: &str) -> Result<(), String> {
    let parent = Path::new(path).parent().unwrap_or(Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create parent dir: {e}"))?;
    }
    let mut cmd = std::process::Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.args(["clone", "--branch", branch, url, path]);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    let output = cmd.output().map_err(|e| format!("git clone failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git clone failed: {stderr}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

use crate::state::AppState;
use tauri::State;

/// DTO for tracked folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFolderDto {
    pub local_path: String,
    pub remote_url: String,
    pub branch: String,
    pub enabled: bool,
    pub interval_secs: u64,
    pub last_sync: Option<String>,
    pub last_status: String,
    pub syncing: bool,
}

impl From<TrackedFolder> for TrackedFolderDto {
    fn from(f: TrackedFolder) -> Self {
        Self {
            local_path: f.local_path,
            remote_url: f.remote_url,
            branch: f.branch,
            enabled: f.enabled,
            interval_secs: f.interval_secs,
            last_sync: f.last_sync,
            last_status: f.last_status,
            syncing: f.syncing,
        }
    }
}

impl From<TrackedFolderDto> for TrackedFolder {
    fn from(d: TrackedFolderDto) -> Self {
        Self {
            local_path: d.local_path,
            remote_url: d.remote_url,
            branch: d.branch,
            enabled: d.enabled,
            interval_secs: d.interval_secs,
            last_sync: d.last_sync,
            last_status: d.last_status,
            syncing: d.syncing,
        }
    }
}

/// List all tracked folders.
#[tauri::command]
pub fn tracking_list(state: State<'_, AppState>) -> Result<Vec<TrackedFolderDto>, String> {
    let path = crate::git_commands::project_path(&state)?;
    let config = TrackingConfig::load(&path);
    Ok(config.folders.into_iter().map(Into::into).collect())
}

/// Add a tracked folder.
#[tauri::command]
pub fn tracking_add(
    state: State<'_, AppState>,
    folder: TrackedFolderDto,
) -> Result<(), String> {
    let path = crate::git_commands::project_path(&state)?;
    let mut config = TrackingConfig::load(&path);
    config.folders.push(folder.into());
    config.save(&path)
}

/// Remove a tracked folder by local path.
#[tauri::command]
pub fn tracking_remove(
    state: State<'_, AppState>,
    local_path: String,
) -> Result<(), String> {
    let path = crate::git_commands::project_path(&state)?;
    let mut config = TrackingConfig::load(&path);
    config.folders.retain(|f| f.local_path != local_path);
    config.save(&path)
}

/// Update a tracked folder's configuration.
#[tauri::command]
pub fn tracking_update(
    state: State<'_, AppState>,
    local_path: String,
    folder: TrackedFolderDto,
) -> Result<(), String> {
    let path = crate::git_commands::project_path(&state)?;
    let mut config = TrackingConfig::load(&path);
    if let Some(existing) = config.folders.iter_mut().find(|f| f.local_path == local_path) {
        *existing = folder.into();
    }
    config.save(&path)
}

/// Manually trigger a sync for a tracked folder.
#[tauri::command]
pub fn tracking_sync_now(
    state: State<'_, AppState>,
    local_path: String,
) -> Result<SyncResultDto, String> {
    let path = crate::git_commands::project_path(&state)?;
    let mut config = TrackingConfig::load(&path);

    // Check if syncing already.
    let already_syncing = config
        .folders
        .iter()
        .find(|f| f.local_path == local_path)
        .map(|f| f.syncing)
        .unwrap_or(false);
    if already_syncing {
        return Err("sync already in progress".to_string());
    }

    // Clone the folder data for sync, mark syncing, save.
    let folder_clone = {
        let folder = config
            .folders
            .iter_mut()
            .find(|f| f.local_path == local_path)
            .ok_or_else(|| format!("folder not tracked: {local_path}"))?;
        folder.syncing = true;
        folder.clone()
    };
    let _ = config.save(&path);

    let result = sync_folder(&folder_clone);

    let mut config = TrackingConfig::load(&path);
    if let Some(folder) = config.folders.iter_mut().find(|f| f.local_path == local_path) {
        let now = chrono::Utc::now().to_rfc3339();
        folder.last_sync = Some(now.clone());
        folder.syncing = false;

        match &result {
            Ok((status, msg, count)) => {
                folder.last_status = status.clone();
                let mut history = SyncHistory::load(&path);
                history.entries.push(SyncHistoryEntry {
                    folder_path: local_path.clone(),
                    timestamp: now,
                    status: status.clone(),
                    message: msg.clone(),
                    changes_count: *count,
                });
                let _ = history.save(&path);
            }
            Err(e) => {
                folder.last_status = "error".to_string();
                let mut history = SyncHistory::load(&path);
                history.entries.push(SyncHistoryEntry {
                    folder_path: local_path.clone(),
                    timestamp: now,
                    status: "error".to_string(),
                    message: e.clone(),
                    changes_count: 0,
                });
                let _ = history.save(&path);
            }
        }
        let _ = config.save(&path);
    }

    match result {
        Ok((status, message, count)) => Ok(SyncResultDto {
            status,
            message,
            changes_count: count,
        }),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResultDto {
    pub status: String,
    pub message: String,
    pub changes_count: usize,
}

/// Get sync history for all tracked folders.
#[tauri::command]
pub fn tracking_history(state: State<'_, AppState>) -> Result<Vec<SyncHistoryEntry>, String> {
    let path = crate::git_commands::project_path(&state)?;
    let history = SyncHistory::load(&path);
    Ok(history.entries)
}

// ---------------------------------------------------------------------------
// Tracking scheduler — background auto-sync
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Handle to a running tracking scheduler thread.
pub struct TrackingScheduler {
    stop: Arc<AtomicBool>,
}

impl TrackingScheduler {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start the tracking scheduler. It periodically syncs all enabled
/// tracked folders every `interval_secs` seconds.
#[tauri::command]
pub fn tracking_start(
    interval_secs: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Stop any existing scheduler first.
    let mut guard = state.tracking_scheduler.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = guard.take() {
        s.stop();
    }

    let project_path = crate::git_commands::project_path(&state)?;
    let interval = std::cmp::max(interval_secs, 30); // minimum 30s
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();

    thread::spawn(move || {
        loop {
            if stop_clone.load(Ordering::Relaxed) {
                break;
            }

            // Load config and sync enabled folders.
            let config = TrackingConfig::load(&project_path);
            for folder in &config.folders {
                if !folder.enabled || stop_clone.load(Ordering::Relaxed) {
                    continue;
                }
                // Skip if currently syncing.
                if folder.syncing {
                    continue;
                }

                let result = sync_folder(folder);
                let mut config = TrackingConfig::load(&project_path);
                if let Some(f) = config.folders.iter_mut().find(|f| f.local_path == folder.local_path) {
                    let now = chrono::Utc::now().to_rfc3339();
                    f.last_sync = Some(now.clone());
                    match &result {
                        Ok((status, msg, count)) => {
                            f.last_status = status.clone();
                            let mut history = SyncHistory::load(&project_path);
                            history.entries.push(SyncHistoryEntry {
                                folder_path: folder.local_path.clone(),
                                timestamp: now,
                                status: status.clone(),
                                message: msg.clone(),
                                changes_count: *count,
                            });
                            let _ = history.save(&project_path);
                        }
                        Err(e) => {
                            f.last_status = "error".to_string();
                            let mut history = SyncHistory::load(&project_path);
                            history.entries.push(SyncHistoryEntry {
                                folder_path: folder.local_path.clone(),
                                timestamp: now,
                                status: "error".to_string(),
                                message: e.clone(),
                                changes_count: 0,
                            });
                            let _ = history.save(&project_path);
                        }
                    }
                    let _ = config.save(&project_path);
                }
            }

            // Sleep for the interval (checking stop signal every second).
            for _ in 0..interval {
                if stop_clone.load(Ordering::Relaxed) {
                    return;
                }
                thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    });

    *guard = Some(TrackingScheduler { stop });
    Ok(())
}

/// Stop the tracking scheduler.
#[tauri::command]
pub fn tracking_stop(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.tracking_scheduler.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = guard.take() {
        s.stop();
    }
    Ok(())
}

/// Check if the tracking scheduler is running.
#[tauri::command]
pub fn tracking_status(state: State<'_, AppState>) -> Result<bool, String> {
    let guard = state.tracking_scheduler.lock().unwrap_or_else(|p| p.into_inner());
    Ok(guard.is_some())
}