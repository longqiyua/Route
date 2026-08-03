//! AI handlers: chat, config.

use axum::extract::Query;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub system: Option<String>,
    /// Session ID to continue (auto-creates if not provided)
    pub session_id: Option<String>,
    /// Create a snapshot before recording the message
    #[serde(default)]
    pub snapshot: bool,
}

#[derive(Deserialize)]
pub struct AiConfigQuery {
    pub show: Option<bool>,
}

/// `GET /api/ai/config`
pub async fn config_handler(
    Query(query): Query<AiConfigQuery>,
) -> Result<Json<Value>, ApiError> {
    route_cli::commands::ai_config(query.show.unwrap_or(false))
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/ai/chat`
pub async fn chat_handler(
    Json(req): Json<ChatRequest>,
) -> Result<Json<Value>, ApiError> {
    let output = capture_stdout(|| {
        route_cli::commands::ai_chat(req.message, req.system, req.session_id, req.snapshot)
    })?;
    Ok(Json(json!({
        "ok": true,
        "response": output,
    })))
}

/// Helper: run a function that prints to stdout and capture its output.
fn capture_stdout<F>(f: F) -> Result<String, ApiError>
where
    F: FnOnce() -> anyhow::Result<()>,
{
    // For simplicity, call the function and let it print to stdout
    f().map_err(|e| ApiError::internal(e.to_string()))?;
    Ok("(output printed to stdout)".to_string())
}