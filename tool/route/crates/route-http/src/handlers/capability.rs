//! Capability handlers: promote learned/discovered skills to `.route/skills/`.

use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app_state::SharedState;
use crate::error::ApiError;
use axum::extract::State;

#[derive(Deserialize)]
pub struct PromoteSkillRequest {
    pub id: Option<String>,
    #[serde(default)]
    pub force: bool,
}

/// `POST /api/capabilities/promote-skill`
///
/// Promotes Skill-kind capabilities in `.route/capability/registry.json` to
/// reusable `.route/skills/*.md` files. Idempotent: existing files are left
/// untouched unless `force` is true.
pub async fn promote_skill_handler(
    State(state): State<SharedState>,
    Json(req): Json<PromoteSkillRequest>,
) -> Result<Json<Value>, ApiError> {
    let root = &state.project_path;
    let registry = route_basic::capability::CapabilityRegistry::load(root)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let report = registry
        .promote_skills(root, req.id.as_deref(), req.force)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(json!({
        "ok": true,
        "promoted": report.promoted,
        "skipped_existing": report.skipped_existing,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::build_router;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use route_basic::capability::{Capability, CapabilityKind, CapabilityLevel};
    use std::sync::Arc;
    use tower::ServiceExt;

    fn make_state(tmp: &tempfile::TempDir) -> Arc<AppState> {
        Arc::new(AppState::new(tmp.path().to_path_buf()))
    }

    async fn post_promote(state: Arc<AppState>, body: Value) -> (StatusCode, Value) {
        let app = build_router(state);
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/capabilities/promote-skill")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        let status = response.status();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&body_bytes).unwrap())
    }

    #[tokio::test]
    async fn test_promote_skill_handler() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let state = make_state(&tmp);

        // Seed a Skill-kind capability.
        let mut registry = route_basic::capability::CapabilityRegistry::load(root).unwrap();
        registry.set(Capability {
            id: "cap-demo".into(),
            reference_id: "ref-demo".into(),
            name: "Demo Skill".into(),
            kind: CapabilityKind::Skill,
            level: CapabilityLevel::L2,
            entrypoint: Some("use".into()),
            usage: "How to use the demo skill.".into(),
            inputs: vec!["prompt".into()],
            outputs: vec!["answer".into()],
            permissions: vec![],
            constraints: vec![],
            availability: true,
        });
        registry.save(root).unwrap();

        // Promote via HTTP endpoint.
        let (status, body) = post_promote(state.clone(), json!({ "id": "cap-demo" })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["ok"], true);
        assert!(!body["promoted"].as_array().unwrap().is_empty());
        assert!(root.join(".route/skills/demo-skill.md").exists());

        // Idempotent: second run skips the existing file.
        let (status2, body2) = post_promote(state, json!({ "id": "cap-demo" })).await;
        assert_eq!(status2, StatusCode::OK);
        assert!(body2["promoted"].as_array().unwrap().is_empty());
        assert_eq!(body2["skipped_existing"].as_array().unwrap().len(), 1);
    }
}
