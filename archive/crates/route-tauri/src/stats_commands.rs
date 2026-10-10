//! Tauri IPC commands for route-stats integration.

use route_stats::{CollectOptions, RepoStats, StatsCollector};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// Optional range filter passed from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsRangeDto {
    pub from: i64,
    pub to: i64,
}

/// Parameters for the `stats_collect` command.
#[derive(Debug, Clone, Deserialize)]
pub struct StatsParams {
    pub range: Option<StatsRangeDto>,
    pub top_files_limit: Option<usize>,
    pub hourly_buckets: Option<usize>,
    pub daily_buckets: Option<usize>,
}

impl Default for StatsParams {
    fn default() -> Self {
        Self {
            range: None,
            top_files_limit: None,
            hourly_buckets: None,
            daily_buckets: None,
        }
    }
}

fn build_options(params: StatsParams) -> CollectOptions {
    let mut opts = CollectOptions::defaults();
    opts.range = params.range.map(|r| route_stats::TimeRange { from: r.from, to: r.to });
    if let Some(t) = params.top_files_limit {
        opts.top_files_limit = t;
    }
    if let Some(h) = params.hourly_buckets {
        opts.hourly_buckets = h;
    }
    if let Some(d) = params.daily_buckets {
        opts.daily_buckets = d;
    }
    opts
}

/// Collect full repo stats (returns the entire RepoStats structure).
#[tauri::command]
pub fn stats_collect(
    params: Option<StatsParams>,
    state: State<'_, AppState>,
) -> Result<RepoStats, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let opts = build_options(params.unwrap_or_default());
    StatsCollector::collect_with(repo, opts).map_err(|e| e.to_string())
}

/// Render stats as Markdown.
#[tauri::command]
pub fn stats_markdown(
    params: Option<StatsParams>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let opts = build_options(params.unwrap_or_default());
    let stats = StatsCollector::collect_with(repo, opts).map_err(|e| e.to_string())?;
    route_stats::render_markdown(&stats).map_err(|e| e.to_string())
}

/// Render stats as JSON (pretty-printed).
#[tauri::command]
pub fn stats_json(
    params: Option<StatsParams>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let guard = state.repo.lock().unwrap_or_else(|p| p.into_inner());
    let repo = guard.as_ref().ok_or("No repository open")?;
    let opts = build_options(params.unwrap_or_default());
    let stats = StatsCollector::collect_with(repo, opts).map_err(|e| e.to_string())?;
    route_stats::render_json(&stats).map_err(|e| e.to_string())
}
