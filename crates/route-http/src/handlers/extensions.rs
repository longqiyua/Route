//! Extensions handlers: skills and references management.

use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;

/// `GET /api/extensions/skills`
pub async fn skills_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::extensions_skills()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/extensions/references`
pub async fn references_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::extensions_references()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}