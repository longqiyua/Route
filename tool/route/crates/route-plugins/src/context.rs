//! Plugin context — read-only view of repo state passed to plugins.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Read-only context passed to every plugin invocation.
///
/// Cheap to clone (uses `Arc` internally) — plugins can stash a copy if they
/// need to defer work to a background thread, though they should not hold
/// references across `handle_event` calls.
#[derive(Debug, Clone)]
pub struct PluginContext {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    project_path: PathBuf,
    /// Free-form metadata bag (e.g. `repo_id`, `route_version`). Plugins
    /// should treat unknown keys gracefully.
    metadata: serde_json::Value,
}

impl PluginContext {
    pub fn new(project_path: impl Into<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Inner {
                project_path: project_path.into(),
                metadata: serde_json::Value::Null,
            }),
        }
    }

    pub fn with_metadata(project_path: impl Into<PathBuf>, metadata: serde_json::Value) -> Self {
        Self {
            inner: Arc::new(Inner {
                project_path: project_path.into(),
                metadata,
            }),
        }
    }

    /// Path to the project root (the directory containing `.route-basic/`).
    pub fn project_path(&self) -> &Path {
        &self.inner.project_path
    }

    /// Free-form metadata (may be `Null`).
    pub fn metadata(&self) -> &serde_json::Value {
        &self.inner.metadata
    }

    /// Convenience: look up a string-typed metadata field.
    pub fn meta_str(&self, key: &str) -> Option<&str> {
        self.inner.metadata.get(key).and_then(|v| v.as_str())
    }
}

impl Default for PluginContext {
    fn default() -> Self {
        Self::new(PathBuf::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn context_clones_cheaply() {
        let ctx = PluginContext::with_metadata(
            "/tmp/proj",
            json!({"repo_id": "abc", "version": "0.2.0"}),
        );
        let ctx2 = ctx.clone();
        // Both contexts share the same Arc — same pointer.
        assert_eq!(ctx.project_path(), ctx2.project_path());
        assert_eq!(ctx.meta_str("repo_id"), Some("abc"));
        assert_eq!(ctx.meta_str("missing"), None);
    }
}
