//! Extension system — skills and references for AI pre-injection.
//!
//! Route creates two directories inside the project's `.route/` folder:
//!
//! - `.route/skills/`     — user-provided skills (e.g. prompt templates, workflows)
//! - `.route/references/` — reference materials (docs, specs, examples)
//!
//! When the AI assistant performs version management, it reads all files from
//! these directories as "pre-injection" context — the AI must consult these
//! materials before any operation. This is a passive, read-only mechanism:
//! Route does not edit or interpret these files, it only makes them available.
//!
//! ## Important
//!
//! Route is strictly a version management tool. The extension system is
//! purely a data conduit — it does not execute skills, run code, or
//! interpret references. Users should use the CLI or MCP interface for
//! the best experience.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// A single entry in the extensions listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified: String,
}

/// List all files in a subdirectory of `.route/`.
fn list_dir(project_path: &Path, sub: &str) -> Result<Vec<ExtensionEntry>, String> {
    let dir = project_path.join(".route").join(sub);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("cannot read {sub} dir: {e}"))?;
    for entry in read_dir {
        let entry = entry.map_err(|e| format!("entry error: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            let meta = std::fs::metadata(&path).map_err(|e| format!("metadata error: {e}"))?;
            entries.push(ExtensionEntry {
                name: entry
                    .file_name()
                    .to_string_lossy()
                    .to_string(),
                path: path.to_string_lossy().to_string(),
                size: meta.len(),
                modified: meta
                    .modified()
                    .ok()
                    .and_then(|t| {
                        chrono::DateTime::<chrono::Utc>::from(t)
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string()
                            .into()
                    })
                    .unwrap_or_default(),
            });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Ensure the skills and references directories exist.
pub fn ensure_dirs(project_path: &Path) -> Result<(), String> {
    let skills = project_path.join(".route").join("skills");
    let refs = project_path.join(".route").join("references");
    std::fs::create_dir_all(&skills)
        .map_err(|e| format!("cannot create skills dir: {e}"))?;
    std::fs::create_dir_all(&refs)
        .map_err(|e| format!("cannot create references dir: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

use crate::state::AppState;
use tauri::State;

/// List all skills.
#[tauri::command]
pub fn skills_list(state: State<'_, AppState>) -> Result<Vec<ExtensionEntry>, String> {
    let path = crate::git_commands::project_path(&state)?;
    list_dir(&path, "skills")
}

/// List all references.
#[tauri::command]
pub fn references_list(state: State<'_, AppState>) -> Result<Vec<ExtensionEntry>, String> {
    let path = crate::git_commands::project_path(&state)?;
    list_dir(&path, "references")
}