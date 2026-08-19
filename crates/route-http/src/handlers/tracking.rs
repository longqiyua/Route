//! Tracking handlers: list, add, remove, sync, history.

use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct AddTrackingRequest {
    pub local: String,
    pub remote: String,
    pub branch: Option<String>,
    pub interval: Option<u64>,
}

#[derive(Deserialize)]
pub struct RemoveTrackingRequest {
    pub local: String,
}

#[derive(Deserialize)]
pub struct SyncTrackingRequest {
    pub local: Option<String>,
}

/// `GET /api/tracking`
pub async fn list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::tracking_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/tracking`
pub async fn add_handler(Json(req): Json<AddTrackingRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::tracking_add(
        req.local,
        req.remote,
        req.branch.unwrap_or_else(|| "main".to_string()),
        req.interval.unwrap_or(600),
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/tracking`
pub async fn remove_handler(
    Json(req): Json<RemoveTrackingRequest>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::tracking_remove(req.local)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/tracking/sync`
pub async fn sync_handler(Json(req): Json<SyncTrackingRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::tracking_sync(req.local).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/tracking/history`
pub async fn history_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::tracking_history().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
