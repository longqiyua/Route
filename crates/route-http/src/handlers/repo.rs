//! Repository handlers: init, status, commit, log, rollback, changes, undo, redo, checkpoint, diff, backup, annotate.

use axum::extract::{Path, Query};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct InitRequest {
    pub path: Option<String>,
}

#[derive(Deserialize)]
pub struct CommitRequest {
    pub message: String,
    pub author: Option<String>,
    pub full: Option<bool>,
    pub branch: Option<String>,
}

#[derive(Deserialize)]
pub struct LogQuery {
    pub limit: Option<usize>,
    pub branch: Option<String>,
}

#[derive(Deserialize)]
pub struct RollbackRequest {
    pub snapshot_id: String,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct BackupRequest {
    pub target: String,
}

#[derive(Deserialize)]
pub struct DiffQuery {
    pub from: String,
    pub to: String,
}

#[derive(Deserialize)]
pub struct CheckpointRequest {
    pub title: String,
    pub body: Option<String>,
}

#[derive(Deserialize)]
pub struct AnnotateRequest {
    pub text: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/init`
pub async fn init_handler(Json(req): Json<InitRequest>) -> Result<Json<Value>, ApiError> {
    let path = req.path.as_deref().unwrap_or(".");
    route_cli::commands::init(Some(path.to_string()), true)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true, "path": path })))
}

/// `GET /api/status`
pub async fn status_handler() -> Result<Json<Value>, ApiError> {
    // Capture stdout output from the CLI status command
    let output = std::process::Command::new("route")
        .arg("status")
        .output()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(Json(json!({
        "ok": true,
        "status_text": text,
    })))
}

/// `POST /api/commit`
pub async fn commit_handler(Json(req): Json<CommitRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::commit(
        req.message,
        req.author,
        req.full.unwrap_or(false),
        req.branch,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/log`
pub async fn log_handler(Query(query): Query<LogQuery>) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(20);
    let branch = query.branch;

    // We delegate to the CLI function which prints to stdout.
    // For a proper API, we'd call repo.list_commits() directly.
    route_cli::commands::log(limit, branch).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/rollback`
pub async fn rollback_handler(Json(req): Json<RollbackRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::rollback(req.snapshot_id, req.reason)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/backup`
pub async fn backup_handler(Json(req): Json<BackupRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::backup(req.target).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/changes`
pub async fn changes_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::changes().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/undo`
pub async fn undo_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::undo().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/redo`
pub async fn redo_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::redo().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/checkpoint`
pub async fn checkpoint_handler(
    Json(req): Json<CheckpointRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::checkpoint(req.title, req.body, false)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/diff`
pub async fn diff_handler(Query(query): Query<DiffQuery>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::diff_snapshots(query.from, query.to)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/commits/:id/annotations`
pub async fn annotations_list_handler(
    Path(commit_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::annotations(commit_id).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/commits/:id/annotations`
pub async fn annotations_create_handler(
    Path(commit_id): Path<String>,
    Json(req): Json<AnnotateRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::annotate(commit_id, req.text)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
