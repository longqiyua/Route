//! Branch handlers: list, create, delete, switch.

use axum::extract::Path;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
    pub kind: Option<String>,
    pub from: Option<String>,
}

#[derive(Deserialize)]
pub struct SwitchBranchRequest {
    pub name: String,
}

/// `GET /api/branches`
pub async fn list_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::branch_list()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/branches`
pub async fn create_handler(
    Json(req): Json<CreateBranchRequest>,
) -> Result<Json<Value>, ApiError> {
    let kind = req.kind.unwrap_or_else(|| "inherited".to_string());
    route_cli::commands::branch_create(req.name, kind, req.from)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/branches/:name`
pub async fn delete_handler(
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::branch_delete(name)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/branches/:name/switch`
pub async fn switch_handler(
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::branch_switch(name)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}