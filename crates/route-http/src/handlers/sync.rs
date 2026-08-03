//! Sync handlers: directory synchronization (mirror / backup / archive).

use axum::extract::Path;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct AddSyncTargetRequest {
    pub name: String,
    pub source: String,
    pub dest: String,
    pub mode: Option<String>,
    pub transport: Option<String>,
    pub conflict: Option<String>,
    pub max_snapshots: Option<usize>,
    pub max_age_days: Option<u32>,
    pub folder_format: Option<String>,
    pub ignore: Option<Vec<String>>,
    pub disable: Option<bool>,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub bucket: Option<String>,
    pub region: Option<String>,
}

#[derive(Deserialize)]
pub struct SyncRunRequest {
    pub name: Option<String>,
    pub verbose: Option<bool>,
}

#[derive(Deserialize)]
pub struct SyncSetEnabledRequest {
    pub name: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct SyncStartRequest {
    pub interval: u64,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /api/sync`
pub async fn add_handler(
    Json(req): Json<AddSyncTargetRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_add(
        req.name,
        req.source,
        req.dest,
        req.mode.unwrap_or_else(|| "backup".to_string()),
        req.transport,
        req.conflict,
        req.max_snapshots,
        req.max_age_days,
        req.folder_format,
        req.ignore.unwrap_or_default(),
        req.disable.unwrap_or(false),
        req.url,
        req.username,
        req.password,
        req.access_key,
        req.secret_key,
        req.bucket,
        req.region,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/sync`
pub async fn list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_list()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/sync/:name`
pub async fn remove_handler(
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_remove(name)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/sync/:name`
pub async fn show_handler(
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_show(name)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/sync/run`
pub async fn run_handler(
    Json(req): Json<SyncRunRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_run(req.name, req.verbose.unwrap_or(false))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/sync/enable`
pub async fn enable_handler(
    Json(req): Json<SyncSetEnabledRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_set_enabled(req.name, req.enabled)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/sync/start`
pub async fn start_handler(
    Json(req): Json<SyncStartRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::sync_commands::sync_start(req.interval)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}