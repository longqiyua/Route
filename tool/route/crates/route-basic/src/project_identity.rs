//! Stable local project/workspace identity.
//!
//! Identity is persisted inside `.route`, so moving or renaming a directory
//! does not change the project id. A workspace id is intentionally distinct
//! and is generated per attached checkout.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "project-identity.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectIdentity {
    pub schema: u8,
    pub project_id: String,
    pub workspace_id: String,
    pub created_at: i64,
    #[serde(default)]
    pub attached_from: Option<String>,
    /// Canonical checkout binding used to distinguish a copied checkout from
    /// a moved one. The project id remains stable; a changed binding rotates
    /// only the workspace id.
    #[serde(default)]
    pub bound_root: Option<String>,
}

pub fn identity_path(root: &Path) -> PathBuf {
    root.join(".route").join(FILE_NAME)
}

pub fn discover_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = fs::canonicalize(start).ok()?;
    if current.is_file() {
        current.pop();
    }
    loop {
        if current.join(".route").is_dir() || current.join(".route-basic").is_dir() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    fs::write(&temp, bytes)?;
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(first) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&temp, path).map_err(|_| first.into())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn load_identity(root: &Path) -> Result<Option<ProjectIdentity>> {
    let path = identity_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
        format!("parsing project identity at {}", path.display())
    })?))
}

pub fn ensure_identity(root: &Path) -> Result<ProjectIdentity> {
    let canonical_root = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/");
    if let Some(mut identity) = load_identity(root)? {
        if identity.bound_root.as_deref() != Some(canonical_root.as_str()) {
            identity.workspace_id = format!("ws_{}", route_core::new_id());
            identity.bound_root = Some(canonical_root);
            atomic_write(&identity_path(root), &serde_json::to_vec_pretty(&identity)?)?;
        }
        return Ok(identity);
    }
    fs::create_dir_all(root.join(".route"))?;
    let identity = ProjectIdentity {
        schema: 1,
        project_id: format!("prj_{}", route_core::new_id()),
        workspace_id: format!("ws_{}", route_core::new_id()),
        created_at: route_core::now_millis(),
        attached_from: None,
        bound_root: Some(canonical_root),
    };
    atomic_write(&identity_path(root), &serde_json::to_vec_pretty(&identity)?)?;
    Ok(identity)
}

/// Attach this checkout to another project's identity while retaining a
/// distinct workspace id. Repeated attachment is idempotent.
pub fn attach(root: &Path, source: &Path) -> Result<ProjectIdentity> {
    let source_root = discover_project_root(source)
        .ok_or_else(|| anyhow::anyhow!("source project has no Route state"))?;
    let source_identity = ensure_identity(&source_root)?;
    let mut identity = ensure_identity(root)?;
    identity.project_id = source_identity.project_id;
    identity.attached_from = Some(source_root.to_string_lossy().replace('\\', "/"));
    atomic_write(&identity_path(root), &serde_json::to_vec_pretty(&identity)?)?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn identity_survives_reload_and_attach_is_idempotent() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        let first = ensure_identity(source.path()).unwrap();
        let attached = attach(target.path(), source.path()).unwrap();
        let again = attach(target.path(), source.path()).unwrap();
        assert_eq!(first.project_id, attached.project_id);
        assert_eq!(attached, again);
        assert_ne!(first.workspace_id, attached.workspace_id);
    }

    #[test]
    fn copied_identity_rotates_workspace_without_merging_projects() {
        let source = tempdir().unwrap();
        let copy = tempdir().unwrap();
        let original = ensure_identity(source.path()).unwrap();
        fs::create_dir_all(copy.path().join(".route")).unwrap();
        fs::copy(identity_path(source.path()), identity_path(copy.path())).unwrap();
        let copied = ensure_identity(copy.path()).unwrap();
        assert_eq!(original.project_id, copied.project_id);
        assert_ne!(original.workspace_id, copied.workspace_id);
    }
}
