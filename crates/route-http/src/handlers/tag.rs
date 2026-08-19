//! Tag handlers: list, add, remove.

use axum::extract::Path;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct AddTagRequest {
    pub name: String,
    pub snapshot_id: String,
    pub message: Option<String>,
}

/// `GET /api/tags`
pub async fn list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::tag_list().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/tags`
pub async fn add_handler(Json(req): Json<AddTagRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::tag_add(req.name, req.snapshot_id, req.message)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/tags/:name`
pub async fn remove_handler(Path(name): Path<String>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::tag_remove(name).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
