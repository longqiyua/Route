//! Conversation handlers: list, create, show, message, rollback, archive, delete.
//!
//! All handlers use `axum::extract::State` to get the conversation store path
//! from `AppState`, ensuring thread safety and testability.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app_state::SharedState;
use crate::error::ApiError;

/// Open the conversation store from the state's conversation_path.
fn open_store(state: &SharedState) -> route_memory::ConversationStore {
    route_memory::ConversationStore::with_path(state.conversation_path.clone())
}

/// `GET /api/conversations`
pub async fn list_handler(
    State(state): State<SharedState>,
) -> Result<Json<Value>, ApiError> {
    let store = open_store(&state);
    let sessions = store.list_sessions();
    let list: Vec<Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "created_at": s.created_at,
                "updated_at": s.updated_at,
                "message_count": s.message_count,
                "tags": s.tags,
                "archived": s.archived,
            })
        })
        .collect();
    Ok(Json(json!({ "ok": true, "sessions": list })))
}

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub title: String,
}

/// `POST /api/conversations`
pub async fn create_handler(
    State(state): State<SharedState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut store = open_store(&state);
    let session = store.create_session(&req.title)?;
    Ok(Json(json!({
        "ok": true,
        "session": {
            "id": session.id,
            "title": session.title,
            "created_at": session.created_at,
        }
    })))
}

#[derive(Deserialize)]
pub struct ShowQuery {
    pub limit: Option<usize>,
}

/// `GET /api/conversations/{id}`
pub async fn show_handler(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Query(query): Query<ShowQuery>,
) -> Result<Json<Value>, ApiError> {
    let store = open_store(&state);
    let session = store
        .get_session(&id)
        .ok_or_else(|| ApiError::not_found(format!("Session '{}' not found", id)))?;

    let msgs = store.session_messages(&id);
    let msgs: Vec<&route_memory::Message> = if let Some(limit) = query.limit {
        msgs.iter().rev().take(limit).rev().copied().collect()
    } else {
        msgs
    };

    let messages: Vec<Value> = msgs
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "role": m.role,
                "content": m.content,
                "created_at": m.created_at,
                "snapshot_id": m.snapshot_id,
                "parent_id": m.parent_id,
                "rollback_snapshot_id": m.rollback_snapshot_id,
            })
        })
        .collect();

    Ok(Json(json!({
        "ok": true,
        "session": {
            "id": session.id,
            "title": session.title,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
            "message_count": session.message_count,
            "tags": session.tags,
            "archived": session.archived,
        },
        "messages": messages,
    })))
}

#[derive(Deserialize)]
pub struct AddMessageRequest {
    pub role: String,
    pub content: String,
    pub snapshot_id: Option<String>,
}

/// `POST /api/conversations/{id}/messages`
pub async fn add_message_handler(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<AddMessageRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut store = open_store(&state);
    let msg = store
        .add_message(&id, &req.role, &req.content, req.snapshot_id)?
        .ok_or_else(|| ApiError::not_found(format!("Session '{}' not found", id)))?;

    Ok(Json(json!({
        "ok": true,
        "message": {
            "id": msg.id,
            "role": msg.role,
            "content": msg.content,
            "created_at": msg.created_at,
            "snapshot_id": msg.snapshot_id,
        }
    })))
}

#[derive(Deserialize)]
pub struct RollbackRequest {
    pub message_id: String,
    pub reason: Option<String>,
}

/// `POST /api/conversations/{id}/rollback`
pub async fn rollback_handler(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(req): Json<RollbackRequest>,
) -> Result<Json<Value>, ApiError> {
    let mut store = open_store(&state);
    let result = store.rollback_to_message(&id, &req.message_id, req.reason.as_deref());

    if result.success {
        Ok(Json(json!({
            "ok": true,
            "snapshot_id": result.snapshot_id,
            "messages_removed": result.messages_removed,
        })))
    } else {
        Err(ApiError::internal(
            result.error.unwrap_or_else(|| "Unknown rollback error".to_string()),
        ))
    }
}

/// `POST /api/conversations/{id}/archive`
pub async fn archive_handler(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mut store = open_store(&state);
    if store.archive_session(&id)? {
        Ok(Json(json!({ "ok": true })))
    } else {
        Err(ApiError::not_found(format!("Session '{}' not found", id)))
    }
}

/// `DELETE /api/conversations/{id}`
pub async fn delete_handler(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mut store = open_store(&state);
    if store.delete_session(&id)? {
        Ok(Json(json!({ "ok": true })))
    } else {
        Err(ApiError::not_found(format!("Session '{}' not found", id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;
    use crate::app_state::AppState;
    use crate::build_router;

    fn make_state(tmp: &tempfile::TempDir) -> Arc<AppState> {
        Arc::new(AppState::new(tmp.path().to_path_buf()))
    }

    async fn send_request(
        state: Arc<AppState>,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let app = build_router(state);
        let builder = Request::builder()
            .method(method)
            .uri(format!("/api{}", path))
            .header("Content-Type", "application/json");
        let req = if let Some(body) = body {
            builder.body(Body::from(serde_json::to_string(&body).unwrap())).unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        };
        let response = app.oneshot(req).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        (status, value)
    }

    #[tokio::test]
    async fn test_list_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_state(&tmp);
        let (status, body) = send_request(state, Method::GET, "/conversations", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        assert!(body["sessions"].is_array());
        assert!(body["sessions"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_state(&tmp);
        // Create a session
        let (status, body) = send_request(
            state.clone(),
            Method::POST,
            "/conversations",
            Some(json!({"title": "test session"})),
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        let session_id = body["session"]["id"].as_str().unwrap().to_string();
        assert!(!session_id.is_empty());

        // List sessions
        let (status, body) = send_request(state.clone(), Method::GET, "/conversations", None).await;
        assert_eq!(status, StatusCode::OK);
        let sessions = body["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0]["id"], session_id);
        assert_eq!(sessions[0]["title"], "test session");
    }

    #[tokio::test]
    async fn test_show_session() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_state(&tmp);
        // Create a session
        let (_, body) = send_request(
            state.clone(),
            Method::POST,
            "/conversations",
            Some(json!({"title": "show test"})),
        ).await;
        let session_id = body["session"]["id"].as_str().unwrap().to_string();

        // Add a message
        let (status, body) = send_request(
            state.clone(),
            Method::POST,
            &format!("/conversations/{}/messages", session_id),
            Some(json!({"role": "user", "content": "hello"})),
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["message"]["role"], "user");

        // Show session
        let (status, body) = send_request(
            state.clone(),
            Method::GET,
            &format!("/conversations/{}", session_id),
            None,
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["session"]["title"], "show test");
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["content"], "hello");
    }

    #[tokio::test]
    async fn test_show_not_found() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_state(&tmp);
        let (status, body) = send_request(
            state,
            Method::GET,
            "/conversations/nonexistent",
            None,
        ).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], true);
    }

    #[tokio::test]
    async fn test_archive_and_delete() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_state(&tmp);
        // Create a session
        let (_, body) = send_request(
            state.clone(),
            Method::POST,
            "/conversations",
            Some(json!({"title": "to archive"})),
        ).await;
        let session_id = body["session"]["id"].as_str().unwrap().to_string();

        // Archive it
        let (status, body) = send_request(
            state.clone(),
            Method::POST,
            &format!("/conversations/{}/archive", session_id),
            None,
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);

        // List should be empty (archived hidden)
        let (_, body) = send_request(state.clone(), Method::GET, "/conversations", None).await;
        assert!(body["sessions"].as_array().unwrap().is_empty());

        // Delete it
        let (status, body) = send_request(
            state.clone(),
            Method::DELETE,
            &format!("/conversations/{}", session_id),
            None,
        ).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn test_add_message_to_nonexistent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let state = make_state(&tmp);
        let (status, _) = send_request(
            state,
            Method::POST,
            "/conversations/nonexistent/messages",
            Some(json!({"role": "user", "content": "hello"})),
        ).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}