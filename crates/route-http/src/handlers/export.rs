//! Export handlers: export repository data in various formats.

use axum::extract::Query;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct ExportQuery {
    pub format: String,
    pub out: Option<String>,
}

/// `GET /api/export`
pub async fn export_handler(Query(query): Query<ExportQuery>) -> Result<Json<Value>, ApiError> {
    route_cli::commands::export(query.format, query.out)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}
