//! File-system watcher that auto-commits project changes.
//!
//! When `watch_start` is invoked, a background thread is spawned that
//! polls the project directory at a fixed interval. On every tick it
//! compares file mtimes against the snapshot it saw on the previous
//! tick; if anything changed, it takes the AppState's repo lock and
//! records an incremental commit — but only after waiting for a
//! "quiet period" of `memory_buffer_ms` so a burst of typing does not
//! produce dozens of commits. The thread exits when `watch_stop` flips
//! the stop flag.
//!
//! ## Memory buffer
//!
//! The buffer is configurable per-project (see `TrackConfig::memory_buffer_ms`).
//! When the watcher sees a change, it records the change and waits. If
//! another change arrives within the buffer window, the wait is extended.
//! Only after `memory_buffer_ms` of *no* further changes does the
//! watcher commit. A `0` value disables the buffer entirely (every
//! change is committed on the next tick). A `request_flush` call
//! forces a flush — bound to the "Run" button on the workspace so the
//! user can persist their work without waiting for the timer.
//!
//! We deliberately use a polling loop instead of `notify`'s OS-level
//! event stream because:
//!  - The debounce window is trivial to express (one sleep).
//!  - There's no risk of missed events when the watcher thread is
//!    starved or the OS queue overflows.
//!  - The project path can move between repos, and re-subscribing on
//!    every change is fiddly. Polling is robust against that.
//!
//! Cost: one filesystem scan per `interval_ms` (default 1500ms). The
//! scan is `O(visible files)`, so a few thousand files is well under
//! 100ms on any reasonable machine.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use route_basic::{BasicRepository, CommitOptions};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// DTO returned to the frontend by `watch_start` / `watch_status`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchStatusDto {
    pub running: bool,
    pub interval_ms: u64,
    pub project_path: Option<String>,
    pub last_commit_at: Option<i64>,
    /// Last error encountered by the watcher thread. Cleared by the
    /// next successful tick.
    pub last_error: Option<String>,
    /// True when the watcher has unsaved changes sitting in the memory
    /// buffer, waiting for the quiet period to elapse.
    pub pending: bool,
}

/// Handle to the watcher thread. Dropping without `stop()` will leak
/// the thread (it will keep running until the process exits). Always
/// call `WatcherHandle::stop` to terminate cleanly.
pub struct WatcherHandle {
    pub stop: Arc<AtomicBool>,
    pub thread: Option<JoinHandle<()>>,
    pub interval_ms: u64,
    /// Shared with the worker thread; flipping this true forces a
    /// flush on the next loop iteration.
    pub flush: Arc<AtomicBool>,
}

impl WatcherHandle {
    /// Signal the worker thread to exit. Joins it (blocking). Safe to
    /// call multiple times.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Force a flush. Wakes the worker thread's next iteration and
    /// asks it to commit whatever's pending without waiting for the
    /// buffer to expire. Safe to call from any thread.
    pub fn request_flush(&self) {
        self.flush.store(true, Ordering::Relaxed);
    }
}

/// Build a fresh watcher. Returns a `WatcherHandle` that the caller
/// is responsible for storing (typically inside `AppState::watcher`).
pub fn start(
    app: AppHandle,
    interval_ms: u64,
) -> WatcherHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop.clone();
    let flush = Arc::new(AtomicBool::new(false));
    let flush_for_thread = flush.clone();

    let interval = interval_ms.max(100); // hard floor — never go below 100ms
    let thread = thread::Builder::new()
        .name("route-file-watcher".to_string())
        .spawn(move || {
            run_loop(app, stop_for_thread, flush_for_thread, interval);
        })
        .expect("failed to spawn watcher thread");

    WatcherHandle {
        stop,
        thread: Some(thread),
        interval_ms: interval,
        flush,
    }
}

/// State machine for the memory buffer:
///   - `Idle`     : no pending changes; the next detected change moves
///                  us into `Pending` with the current timestamp.
///   - `Pending`  : we have a diff ready to commit, but we're waiting
///                  for `memory_buffer_ms` of no further changes (or a
///                  `request_flush` signal). More changes during the
///                  wait extend the deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BufferState {
    Idle,
    Pending,
}

fn run_loop(
    app: AppHandle,
    stop: Arc<AtomicBool>,
    flush: Arc<AtomicBool>,
    interval_ms: u64,
) {
    let interval = Duration::from_millis(interval_ms);
    // First scan records the current state without committing.
    let mut last_mtimes: HashMap<String, i64> = HashMap::new();
    let mut primed = false;
    let last_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let last_commit_at: Arc<Mutex<Option<i64>>> = Arc::new(Mutex::new(None));
    let pending_flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

    // Buffer state: when we last saw a change, and what the project
    // wanted the buffer window to be.
    let mut buffer_state = BufferState::Idle;
    let mut last_change_at: Option<Instant> = None;

    // Prime the mtime cache before we start checking for diffs.
    if let Some(path) = current_project_path(&app) {
        if let Ok(scan) = scan_mtimes(&path) {
            last_mtimes = scan;
            primed = true;
        }
    }

    while !stop.load(Ordering::Relaxed) {
        thread::sleep(interval);

        let path = match current_project_path(&app) {
            Some(p) => p,
            None => continue, // no repo open yet — just wait
        };

        // Skip the diff on the very first iteration (already primed).
        if !primed {
            if let Ok(scan) = scan_mtimes(&path) {
                last_mtimes = scan;
                primed = true;
            }
            continue;
        }

        // Read the memory-buffer window from the active project's
        // config. The value is refreshed on every tick so a settings
        // change is picked up without a restart.
        let memory_buffer_ms = read_memory_buffer_ms(&app);

        let scan = match scan_mtimes(&path) {
            Ok(s) => s,
            Err(e) => {
                set_last_error(&last_error, format!("scan failed: {e}"));
                continue;
            }
        };

        let changed = scan != last_mtimes;
        let explicit_flush = flush.swap(false, Ordering::Relaxed);

        if changed {
            // A new change came in. If we were idle, transition to
            // Pending. If we were already Pending, the deadline resets
            // and we wait another full window.
            buffer_state = BufferState::Pending;
            last_change_at = Some(Instant::now());
            pending_flag.store(true, Ordering::Relaxed);
        }

        // Decide whether to commit now. We commit when:
        //   (a) There are pending changes AND
        //   (b) The buffer is disabled (memory_buffer_ms == 0), OR
        //       `memory_buffer_ms` of quiet have passed since the last
        //       change, OR
        //       the user explicitly asked for a flush.
        let should_commit = if buffer_state == BufferState::Pending {
            if memory_buffer_ms == 0 {
                true
            } else if explicit_flush {
                true
            } else if let Some(t) = last_change_at {
                t.elapsed() >= Duration::from_millis(memory_buffer_ms)
            } else {
                false
            }
        } else {
            false
        };

        if !should_commit {
            continue;
        }

        // Try to commit. Bail out cleanly if the repo mutex is held
        // by another command — we'll catch the change on the next tick.
        let state = app.state::<AppState>();
        let guard = match state.repo.lock() {
            Ok(g) => g,
            Err(e) => {
                set_last_error(&last_error, format!("repo lock poisoned: {e}"));
                continue;
            }
        };
        let repo = match guard.as_ref() {
            Some(r) => r,
            None => continue,
        };

        match commit_if_dirty(&app, repo, &path) {
            Ok(Some(ts)) => {
                last_mtimes = scan;
                buffer_state = BufferState::Idle;
                last_change_at = None;
                pending_flag.store(false, Ordering::Relaxed);
                if let Ok(mut g) = last_commit_at.lock() {
                    *g = Some(ts);
                }
                *last_error.lock().unwrap_or_else(|p| p.into_inner()) = None;
            }
            Ok(None) => {
                // Repo thinks nothing changed (e.g. ignore rules filter
                // out the modified file). Refresh the cache to be safe.
                last_mtimes = scan;
                buffer_state = BufferState::Idle;
                last_change_at = None;
                pending_flag.store(false, Ordering::Relaxed);
            }
            Err(e) => {
                set_last_error(&last_error, format!("auto-commit failed: {e}"));
            }
        }
    }
}

fn set_last_error(slot: &Arc<Mutex<Option<String>>>, msg: String) {
    if let Ok(mut g) = slot.lock() {
        *g = Some(msg);
    }
}

/// Read the memory-buffer window from the active project's config. We
/// go through AppState so the value is fresh on every tick — the user
/// can change it on the settings page without restarting the watcher.
fn read_memory_buffer_ms(app: &AppHandle) -> u64 {
    let state = app.state::<AppState>();
    let guard = match state.repo.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let repo = match guard.as_ref() {
        Some(r) => r,
        None => return 0,
    };
    repo.track_config().memory_buffer_ms
}

/// Read the current project path from AppState. Returns `None` if no
/// repo is open.
fn current_project_path(app: &AppHandle) -> Option<PathBuf> {
    let state = app.state::<AppState>();
    let guard = state.project_path.lock().ok()?;
    guard.clone()
}

/// Walk the project directory and return `{rel_path: mtime_ms}`. Uses
/// the same ignore rules as `ProjectScanner` (so `.route`, `node_modules`,
/// etc. are skipped — the user's edits inside those are never tracked
/// anyway).
fn scan_mtimes(root: &PathBuf) -> std::io::Result<HashMap<String, i64>> {
    use ignore::WalkBuilder;
    let mut out = HashMap::new();
    let walker = WalkBuilder::new(root)
        .standard_filters(true)
        .require_git(false)
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Mirror ProjectScanner's hard-coded skips.
            !matches!(
                name.as_ref(),
                ".route" | ".route-basic" | "node_modules" | "target" | "dist" | "build" | ".git"
            )
        })
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let abs = entry.path();
        let rel = match abs.strip_prefix(root) {
            Ok(p) => p.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        out.insert(rel, mtime);
    }
    Ok(out)
}

/// Try to commit. Returns the new commit's `created_at` if a commit
/// was actually created, or `None` if there was no real change (the
/// repo's own diff came back empty).
fn commit_if_dirty(app: &tauri::AppHandle, repo: &BasicRepository, path: &PathBuf) -> anyhow::Result<Option<i64>> {
    // If an AI operator is currently in control, attribute the auto-commit
    // to it. We read the AI state from AppState (not the repo) so the
    // watcher loop sees the same identity the CLI/MCP just registered.
    let (operator, is_ai) = {
        let state = app.state::<AppState>();
        let guard = state.ai_operator.lock().ok();
        match guard.as_ref().and_then(|g| g.as_ref()) {
            Some(op) => (format!("ai:{}", op.name), true),
            None => ("user".to_string(), false),
        }
    };

    // Git mode: bypass route_basic entirely and let the user's own git
    // record the auto-commit. The watcher keeps the same memory-buffer
    // debounce — only the commit sink changes.
    let git_mode = {
        let state = app.state::<AppState>();
        state.git_mode.load(std::sync::atomic::Ordering::Relaxed)
    };
    if git_mode {
        // git_auto_commit returns Ok(None) when there's nothing to commit,
        // which is the normal case on most ticks.
        match crate::git_commands::git_auto_commit(path) {
            Ok(Some(_sha)) => {
                return Ok(Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                ))
            }
            Ok(None) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!(e)),
        }
    }

    let opts = CommitOptions {
        message: "auto".to_string(),
        author: None,
        force_full: false,
        branch: None,
        operator: Some(operator),
        body: None,
        is_checkpoint: false,
        is_ai,
    };
    let head_before = repo
        .get_current_branch_name()
        .ok()
        .and_then(|name| repo.get_branch(&name).ok())
        .and_then(|b| b.head_snapshot);

    let new_commit = repo.commit(opts)?;

    // If head didn't change AND this is an incremental that produced
    // the same to/from, treat it as a no-op.
    if new_commit.from_snapshot == new_commit.to_snapshot
        && new_commit.kind == route_basic::CommitKind::Incremental
    {
        return Ok(None);
    }

    if let Some(prev) = head_before {
        if prev == new_commit.to_snapshot {
            return Ok(None);
        }
    }

    Ok(Some(new_commit.created_at))
}
