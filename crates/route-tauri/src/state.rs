//! Application state — holds the currently open repository.

use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use route_basic::BasicRepository;
use route_sync::Scheduler;

use crate::permissions::PermissionLevel;
use crate::watcher::WatcherHandle;

/// Identity of the AI agent currently driving the software, if any.
///
/// When set, all commits and rollbacks are stamped with `operator =
/// "ai:<name>"` and the `body` field carries the prompt that triggered
/// the change. The frontend can call `get_ai_operator` to display a
/// status badge and `clear_ai_operator` to release control back to the
/// human user.
#[derive(Debug, Clone, Default)]
pub struct AiOperatorState {
    pub name: String,
    pub prompt: String,
    /// ms since epoch when the AI took control. Used so the UI can show
    /// "AI · 14m" badges.
    pub since_ms: i64,
}

#[derive(Default)]
pub struct AppState {
    pub repo: Mutex<Option<BasicRepository>>,
    pub project_path: Mutex<Option<PathBuf>>,
    /// Currently running sync scheduler (if any).
    pub sync_scheduler: Mutex<Option<Scheduler>>,
    /// Currently running tracking scheduler (if any).
    pub tracking_scheduler: Mutex<Option<crate::tracking::TrackingScheduler>>,
    /// Currently running file-system watcher (if any). The watcher
    /// auto-commits changes; only one may run at a time.
    pub watcher: Mutex<Option<WatcherHandle>>,
    /// Currently-active AI operator, if any. None means "user" owns the
    /// next edit. See `AiOperatorState` for the schema.
    pub ai_operator: Mutex<Option<AiOperatorState>>,
    /// Whether git mode is active. When true, the file watcher commits
    /// via `git add + git commit` instead of route_basic's `repo.commit`,
    /// and checkpoints become git commits. Toggled from the frontend
    /// via the `git_mode_set` command so the backend watcher sees the
    /// change without a restart.
    pub git_mode: Arc<AtomicBool>,
    /// Permission level for CLI/MCP. The GUI always runs at High.
    /// Defaults to Normal, which restricts remote operations.
    pub permission_level: Mutex<PermissionLevel>,
}

/// Holds handles to child processes started by the GUI (CLI daemon, MCP server).
pub struct ManagedProcesses {
    pub cli: Option<Child>,
    pub mcp: Option<Child>,
}

/// Wrapper around `ManagedProcesses` for Tauri managed state.
pub struct ProcessManager(pub Mutex<ManagedProcesses>);

impl Default for ProcessManager {
    fn default() -> Self {
        Self(Mutex::new(ManagedProcesses {
            cli: None,
            mcp: None,
        }))
    }
}

impl AppState {
    pub fn set_repo(&self, repo: BasicRepository) {
        let path = repo.project_path().to_path_buf();
        // Auto-attach event bus from plugins.json if any plugins are
        // configured. Errors are logged but never fatal — a misconfigured
        // plugin must not block opening a repo.
        let paths = route_core::RoutePaths::new(&path);
        match route_plugins::build_bus_from_config(&paths.route_dir) {
            Ok(Some(bus)) => {
                if let Err(e) = repo.set_event_bus(bus) {
                    tracing::warn!(error = %e, "could not attach event bus");
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, "failed to build event bus from plugins.json");
            }
        }
        *self.repo.lock().unwrap_or_else(|p| p.into_inner()) = Some(repo);
        *self.project_path.lock().unwrap_or_else(|p| p.into_inner()) = Some(path);
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        *self.repo.lock().unwrap_or_else(|p| p.into_inner()) = None;
        *self.project_path.lock().unwrap_or_else(|p| p.into_inner()) = None;
        // Stop scheduler if running
        let mut guard = self.sync_scheduler.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut scheduler) = guard.take() {
            scheduler.stop_all();
        }
    }

    #[allow(dead_code)]
    pub fn project_path(&self) -> Option<PathBuf> {
        self.project_path.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}
