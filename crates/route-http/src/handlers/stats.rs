//! Statistics handlers: basic stats, detailed report.

use axum::extract::Query;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct StatsReportQuery {
    pub format: Option<String>,
    pub out: Option<String>,
    pub top: Option<usize>,
    pub hourly: Option<usize>,
    pub daily: Option<usize>,
}

/// `GET /api/stats`
pub async fn stats_handler() -> Result<Json<Value>, ApiError> {
    route_cli::commands::stats()
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `GET /api/stats/report`
pub async fn report_handler(
    Query(query): Query<StatsReportQuery>,
) -> Result<Json<Value>, ApiError> {
    let format = query.format.unwrap_or_else(|| "markdown".to_string());
    route_cli::commands::stats_report(
        format,
        query.out,
        query.top.unwrap_or(20),
        query.hourly.unwrap_or(24),
        query.daily.unwrap_or(30),
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}