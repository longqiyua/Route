//! Permission handlers: status, set.

use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct SetPermissionRequest {
    pub level: String,
}

/// `GET /api/permission`
pub async fn status_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::permission_status().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/permission`
pub async fn set_handler(Json(req): Json<SetPermissionRequest>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::permission_set(req.level)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
