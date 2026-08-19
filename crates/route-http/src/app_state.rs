//! Shared application state.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use route_basic::BasicRepository;
use tokio::sync::Mutex;

/// Shared application state for all handlers.
pub struct AppState {
    /// Base path for the project (default: cwd at startup).
    pub project_path: PathBuf,
    /// Lazily-opened repository (mutex for interior mutability).
    pub repo: Mutex<Option<BasicRepository>>,
    /// Conversation store path (.route/conversations.json).
    pub conversation_path: PathBuf,
}

impl AppState {
    pub fn new(project_path: PathBuf) -> Self {
        let conversation_path = project_path.join(".route").join("conversations.json");
        Self {
            project_path,
            repo: Mutex::new(None),
            conversation_path,
        }
    }

    /// Open or get the cached repository handle.
    pub async fn open_repo(&self) -> Result<tokio::sync::MutexGuard<'_, Option<BasicRepository>>> {
        let mut guard = self.repo.lock().await;
        if guard.is_none() {
            let repo = BasicRepository::open(&self.project_path)?;
            *guard = Some(repo);
        }
        Ok(guard)
    }
}

pub type SharedState = Arc<AppState>;
