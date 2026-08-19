//! Git operation handlers — delegates to `route-cli::git_commands`.

use axum::extract::{Path, Query};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct GitCommitRequest {
    pub message: String,
}

#[derive(Deserialize)]
pub struct GitLogQuery {
    pub limit: Option<usize>,
    pub graph: Option<bool>,
    pub all: Option<bool>,
}

#[derive(Deserialize)]
pub struct GitBranchCreateRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct GitRemoteAddRequest {
    pub name: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct GitPushRequest {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub force: Option<bool>,
}

#[derive(Deserialize)]
pub struct GitFetchQuery {
    pub remote: Option<String>,
}

#[derive(Deserialize)]
pub struct GitPullQuery {
    pub remote: Option<String>,
    pub branch: Option<String>,
}

#[derive(Deserialize)]
pub struct GitStashPushRequest {
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct GitTagCreateRequest {
    pub name: String,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct GitConfigSetRequest {
    pub key: String,
    pub value: String,
    pub scope: Option<String>,
}

#[derive(Deserialize)]
pub struct GitConfigGetQuery {
    pub key: String,
}

#[derive(Deserialize)]
pub struct GitRevertRequest {
    pub sha: String,
}

#[derive(Deserialize)]
pub struct GitCherryPickRequest {
    pub shas: Vec<String>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct GitRebaseRequest {
    pub target: String,
}

#[derive(Deserialize)]
pub struct GitCleanQuery {
    pub dry_run: Option<bool>,
    pub directories: Option<bool>,
    pub force: Option<bool>,
}

#[derive(Deserialize)]
pub struct GitShowQuery {
    pub sha: String,
}

#[derive(Deserialize)]
pub struct GitArchiveRequest {
    pub output: String,
    pub format: Option<String>,
    pub treeish: Option<String>,
}

#[derive(Deserialize)]
pub struct GitCloneRequest {
    pub url: String,
    pub target: String,
}

#[derive(Deserialize)]
pub struct GitMergeRequest {
    pub source: String,
}

#[derive(Deserialize)]
pub struct GitRestoreRequest {
    pub paths: Vec<String>,
}

#[derive(Deserialize)]
pub struct GitModeRequest {
    pub mode: String,
}

#[derive(Deserialize)]
pub struct GitPushUpstreamRequest {
    pub remote: Option<String>,
    pub branch: Option<String>,
}

#[derive(Deserialize)]
pub struct GitAddRequest {
    pub paths: Vec<String>,
}

#[derive(Deserialize)]
pub struct GitResetRequest {
    pub paths: Vec<String>,
}

// ---------------------------------------------------------------------------
// Handlers — Init / Status / Log / Commit
// ---------------------------------------------------------------------------

/// `POST /api/git/init`
pub async fn init_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::init().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/status`
pub async fn status_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::status().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/log`
pub async fn log_handler(Query(query): Query<GitLogQuery>) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(20);
    let graph = query.graph.unwrap_or(false);
    let all = query.all.unwrap_or(false);
    route_cli::git_commands::log(limit, graph, all)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/commit`
pub async fn commit_handler(Json(req): Json<GitCommitRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::commit(req.message).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Branch
// ---------------------------------------------------------------------------

/// `GET /api/git/branches`
pub async fn branch_list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::branch_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/branches`
pub async fn branch_create_handler(
    Json(req): Json<GitBranchCreateRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::branch_create(req.name)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/git/branches/:name`
pub async fn branch_delete_handler(Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::branch_delete(name, false)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/branches/:name/switch`
pub async fn branch_switch_handler(Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::branch_switch(name).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Remote
// ---------------------------------------------------------------------------

/// `GET /api/git/remotes`
pub async fn remote_list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::remote_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/remotes`
pub async fn remote_add_handler(
    Json(req): Json<GitRemoteAddRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::remote_add(req.name, req.url)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/git/remotes/:name`
pub async fn remote_remove_handler(Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::remote_remove(name).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Fetch / Pull / Push / Diff
// ---------------------------------------------------------------------------

/// `POST /api/git/fetch`
pub async fn fetch_handler(Json(req): Json<GitFetchQuery>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::fetch(req.remote).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/pull`
pub async fn pull_handler(Json(req): Json<GitPullQuery>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::pull(req.remote, req.branch)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/push`
pub async fn push_handler(Json(req): Json<GitPushRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::push(req.remote, req.branch, req.force.unwrap_or(false))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/diff`
pub async fn diff_handler(Query(query): Query<GitPullQuery>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::diff(query.branch).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Add / Reset / Stash
// ---------------------------------------------------------------------------

/// `POST /api/git/add`
pub async fn add_handler(Json(req): Json<GitAddRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::add(req.paths).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/reset`
pub async fn reset_handler(Json(req): Json<GitResetRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::reset(req.paths).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/stash`
pub async fn stash_list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::stash_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/stash/push`
pub async fn stash_push_handler(
    Json(req): Json<GitStashPushRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::stash_push(req.message)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/stash/pop`
pub async fn stash_pop_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::stash_pop().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Tag
// ---------------------------------------------------------------------------

/// `GET /api/git/tags`
pub async fn tag_list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::tag_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/tags`
pub async fn tag_create_handler(
    Json(req): Json<GitTagCreateRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::tag_create(req.name, req.message)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/git/tags/:name`
pub async fn tag_delete_handler(Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::tag_delete(name).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// `GET /api/git/config`
pub async fn config_get_handler(
    Query(query): Query<GitConfigGetQuery>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::config_get(query.key)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/config`
pub async fn config_set_handler(
    Json(req): Json<GitConfigSetRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::config_set(req.key, req.value, req.scope)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------
// Advanced operations
// ---------------------------------------------------------------------------

/// `POST /api/git/revert`
pub async fn revert_handler(Json(req): Json<GitRevertRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::revert(req.sha).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/cherry-pick`
pub async fn cherry_pick_handler(
    Json(req): Json<GitCherryPickRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::cherry_pick(req.shas, req.message)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/rebase`
pub async fn rebase_handler(Json(req): Json<GitRebaseRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::rebase(req.target).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/rebase/abort`
pub async fn rebase_abort_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::rebase_abort().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/rebase/continue`
pub async fn rebase_continue_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::rebase_continue().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/merge`
pub async fn merge_handler(Json(req): Json<GitMergeRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::merge(req.source).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/clone`
pub async fn clone_handler(Json(req): Json<GitCloneRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::clone(req.url, req.target)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/archive`
pub async fn archive_handler(Json(req): Json<GitArchiveRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::archive(req.output, req.format, req.treeish)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/backup`
pub async fn backup_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::backup().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/backups`
pub async fn backup_list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::backup_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/restore`
pub async fn restore_handler(Json(req): Json<GitRestoreRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::restore(req.paths).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/mode`
pub async fn mode_handler(Json(req): Json<GitModeRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::set_mode(req.mode).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/git/push-upstream`
pub async fn push_upstream_handler(
    Json(req): Json<GitPushUpstreamRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::push_set_upstream(req.remote, req.branch)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/rebase-in-progress`
pub async fn rebase_in_progress_handler() -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::rebase_in_progress().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/git/show`
pub async fn show_handler(Query(query): Query<GitShowQuery>) -> Result<Json<Value>, ApiError> {
    route_cli::git_commands::show(query.sha).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
