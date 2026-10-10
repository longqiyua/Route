//! Route HTTP REST API — route and server setup.

use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};

pub mod app_state;
pub mod error;
pub mod extract;
pub mod handlers;

pub use app_state::{AppState, SharedState};
pub use error::ApiError;

/// Build the full API router with all routes and shared state.
pub fn build_router(state: SharedState) -> Router {
    // CORS: allow all origins for development
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Nest all API routes under /api
    let api_routes = Router::new()
        // ---- Repository (basic mode) ----
        .route("/init", post(handlers::repo::init_handler))
        .route("/status", get(handlers::repo::status_handler))
        .route("/commit", post(handlers::repo::commit_handler))
        .route("/log", get(handlers::repo::log_handler))
        .route("/rollback", post(handlers::repo::rollback_handler))
        .route("/backup", post(handlers::repo::backup_handler))
        .route("/changes", get(handlers::repo::changes_handler))
        .route("/undo", post(handlers::repo::undo_handler))
        .route("/redo", post(handlers::repo::redo_handler))
        .route("/checkpoint", post(handlers::repo::checkpoint_handler))
        .route("/diff", get(handlers::repo::diff_handler))
        .route(
            "/commits/{id}/annotations",
            get(handlers::repo::annotations_list_handler),
        )
        .route(
            "/commits/{id}/annotations",
            post(handlers::repo::annotations_create_handler),
        )
        // ---- Branches (basic mode) ----
        .route("/branches", get(handlers::branch::list_handler))
        .route("/branches", post(handlers::branch::create_handler))
        .route("/branches/{name}", delete(handlers::branch::delete_handler))
        .route(
            "/branches/{name}/switch",
            post(handlers::branch::switch_handler),
        )
        // ---- Tags (basic mode) ----
        .route("/tags", get(handlers::tag::list_handler))
        .route("/tags", post(handlers::tag::add_handler))
        .route("/tags/{name}", delete(handlers::tag::remove_handler))
        // ---- Tracking ----
        .route("/tracking", get(handlers::tracking::list_handler))
        .route("/tracking", post(handlers::tracking::add_handler))
        .route("/tracking", delete(handlers::tracking::remove_handler))
        .route("/tracking/sync", post(handlers::tracking::sync_handler))
        .route(
            "/tracking/history",
            get(handlers::tracking::history_handler),
        )
        // ---- AI ----
        .route("/ai/config", get(handlers::ai::config_handler))
        .route("/ai/chat", post(handlers::ai::chat_handler))
        // ---- Context ----
        .route("/context", get(handlers::context::context_handler))
        // ---- Export ----
        .route("/export", get(handlers::export::export_handler))
        // ---- Extensions ----
        .route(
            "/extensions/skills",
            get(handlers::extensions::skills_handler),
        )
        .route(
            "/extensions/references",
            get(handlers::extensions::references_handler),
        )
        .route(
            "/capabilities/promote-skill",
            post(handlers::capability::promote_skill_handler),
        )
        // ---- Permission ----
        .route("/permission", get(handlers::permission::status_handler))
        .route("/permission", post(handlers::permission::set_handler))
        // ---- Stats ----
        .route("/stats", get(handlers::stats::stats_handler))
        .route("/stats/report", get(handlers::stats::report_handler))
        // ---- Conversations ----
        .route("/conversations", get(handlers::conversation::list_handler))
        .route(
            "/conversations",
            post(handlers::conversation::create_handler),
        )
        .route(
            "/conversations/{id}",
            get(handlers::conversation::show_handler),
        )
        .route(
            "/conversations/{id}/messages",
            post(handlers::conversation::add_message_handler),
        )
        .route(
            "/conversations/{id}/rollback",
            post(handlers::conversation::rollback_handler),
        )
        .route(
            "/conversations/{id}/archive",
            post(handlers::conversation::archive_handler),
        )
        .route(
            "/conversations/{id}",
            delete(handlers::conversation::delete_handler),
        )
        // ---- Sync ----
        .route("/sync", get(handlers::sync::list_handler))
        .route("/sync", post(handlers::sync::add_handler))
        .route("/sync/{name}", get(handlers::sync::show_handler))
        .route("/sync/{name}", delete(handlers::sync::remove_handler))
        .route("/sync/run", post(handlers::sync::run_handler))
        .route("/sync/enable", post(handlers::sync::enable_handler))
        .route("/sync/start", post(handlers::sync::start_handler))
        // ---- Git operations ----
        .route("/git/init", post(handlers::git::init_handler))
        .route("/git/status", get(handlers::git::status_handler))
        .route("/git/log", get(handlers::git::log_handler))
        .route("/git/commit", post(handlers::git::commit_handler))
        .route("/git/branches", get(handlers::git::branch_list_handler))
        .route("/git/branches", post(handlers::git::branch_create_handler))
        .route(
            "/git/branches/{name}",
            delete(handlers::git::branch_delete_handler),
        )
        .route(
            "/git/branches/{name}/switch",
            post(handlers::git::branch_switch_handler),
        )
        .route("/git/remotes", get(handlers::git::remote_list_handler))
        .route("/git/remotes", post(handlers::git::remote_add_handler))
        .route(
            "/git/remotes/{name}",
            delete(handlers::git::remote_remove_handler),
        )
        .route("/git/fetch", post(handlers::git::fetch_handler))
        .route("/git/pull", post(handlers::git::pull_handler))
        .route("/git/push", post(handlers::git::push_handler))
        .route("/git/diff", get(handlers::git::diff_handler))
        .route("/git/add", post(handlers::git::add_handler))
        .route("/git/reset", post(handlers::git::reset_handler))
        .route("/git/stash", get(handlers::git::stash_list_handler))
        .route("/git/stash/push", post(handlers::git::stash_push_handler))
        .route("/git/stash/pop", post(handlers::git::stash_pop_handler))
        .route("/git/tags", get(handlers::git::tag_list_handler))
        .route("/git/tags", post(handlers::git::tag_create_handler))
        .route(
            "/git/tags/{name}",
            delete(handlers::git::tag_delete_handler),
        )
        .route("/git/config", get(handlers::git::config_get_handler))
        .route("/git/config", post(handlers::git::config_set_handler))
        .route("/git/revert", post(handlers::git::revert_handler))
        .route("/git/cherry-pick", post(handlers::git::cherry_pick_handler))
        .route("/git/rebase", post(handlers::git::rebase_handler))
        .route(
            "/git/rebase/abort",
            post(handlers::git::rebase_abort_handler),
        )
        .route(
            "/git/rebase/continue",
            post(handlers::git::rebase_continue_handler),
        )
        .route("/git/merge", post(handlers::git::merge_handler))
        .route("/git/clone", post(handlers::git::clone_handler))
        .route("/git/archive", post(handlers::git::archive_handler))
        .route("/git/backup", post(handlers::git::backup_handler))
        .route("/git/backups", get(handlers::git::backup_list_handler))
        .route("/git/restore", post(handlers::git::restore_handler))
        .route("/git/mode", post(handlers::git::mode_handler))
        .route(
            "/git/push-upstream",
            post(handlers::git::push_upstream_handler),
        )
        .route(
            "/git/rebase-in-progress",
            get(handlers::git::rebase_in_progress_handler),
        )
        .route("/git/show", get(handlers::git::show_handler));

    // Wrap with CORS layer and shared state
    Router::new()
        .nest("/api", api_routes)
        .layer(cors)
        .with_state(state)
}
