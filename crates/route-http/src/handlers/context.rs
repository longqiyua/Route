//! Project context handler.

use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiError;

/// `GET /api/context`
pub async fn context_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::project_context().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
