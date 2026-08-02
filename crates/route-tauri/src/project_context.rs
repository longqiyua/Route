//! Project Context API — provides project structure, tracking records, and
//! project memory for AI context injection.
//!
//! When the Active AI mode is enabled, the ChatPage calls this API to get:
//!
//! 1. **Project structure** — file tree (top-level directories and files)
//! 2. **Recent commits** — last 10 commits with diff summaries
//! 3. **References content** — files from `.route/references/`
//! 4. **Tracking history** — recent sync/backup history
//! 5. **Current branch** — active branch info
//!
//! This data is formatted as a structured text block that is injected into
//! the AI system prompt, giving the AI full context about the project.

use std::path::Path;

use route_basic::BasicRepository;
use route_basic::models::DiffSummary;
use route_core::short_id;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Full project context returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectContextDto {
    /// Formatted text block for AI system prompt injection.
    pub formatted_context: String,
    /// Project structure as a tree string.
    pub project_tree: String,
    /// Recent commits formatted as text.
    pub recent_commits: String,
    /// References content (from `.route/references/`).
    pub references_content: String,
    /// Tracking history formatted as text.
    pub tracking_history: String,
    /// Current branch name.
    pub current_branch: String,
    /// Number of entries in each section.
    pub stats: ContextStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextStats {
    pub files_in_tree: usize,
    pub commits: usize,
    pub references: usize,
    pub tracking_entries: usize,
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// Get the full project context for AI injection.
/// Returns structured data about the project structure, history, and memory.
#[tauri::command]
pub fn project_context(state: State<'_, AppState>) -> Result<ProjectContextDto, String> {
    let project_path = crate::git_commands::project_path(&state)?;

    let tree = build_project_tree(&project_path)?;
    let commits = format_recent_commits(&project_path)?;
    let refs = format_references(&project_path);
    let tracking = format_tracking_history(&project_path);
    let branch = get_current_branch(&project_path)?;

    let stats = ContextStats {
        files_in_tree: tree.lines().count(),
        commits: commits.lines().filter(|l| l.starts_with("- ")).count(),
        references: refs.lines().filter(|l| l.starts_with("- ")).count(),
        tracking_entries: tracking.lines().filter(|l| l.starts_with("- ")).count(),
    };

    let formatted_context = format!(
        r#"=== Project Context ===

## Current Branch
{branch}

## Project Structure
{tree}

## Recent Commits
{commits}

## References (Project Memory)
{refs}

## Tracking History
{tracking}
"#,
        branch = branch,
        tree = tree,
        commits = commits,
        refs = refs,
        tracking = tracking,
    );

    Ok(ProjectContextDto {
        formatted_context,
        project_tree: tree,
        recent_commits: commits,
        references_content: refs,
        tracking_history: tracking,
        current_branch: branch,
        stats,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a compact project tree (top 3 levels).
fn build_project_tree(project_path: &Path) -> Result<String, String> {
    let mut lines: Vec<String> = Vec::new();
    let mut count = 0;
    let max_entries = 100;

    // Skip .route, .git, node_modules, target
    let skip_dirs = [".route", ".git", "node_modules", "target", ".next", "dist", "build"];

    if let Ok(entries) = std::fs::read_dir(project_path) {
        let mut dirs: Vec<_> = Vec::new();
        let mut files: Vec<_> = Vec::new();

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || skip_dirs.contains(&name.as_str()) {
                continue;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push((name, entry.path()));
            } else {
                if let Ok(meta) = entry.metadata() {
                    files.push((name, meta.len()));
                }
            }
        }

        dirs.sort_by(|a, b| a.0.cmp(&b.0));
        files.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, path) in &dirs {
            if count >= max_entries {
                lines.push(format!("  ... (more entries truncated)"));
                break;
            }
            lines.push(format!("  📁 {name}/"));
            count += 1;

            // Show one level of sub-items
            if let Ok(sub_entries) = std::fs::read_dir(path) {
                let mut sub_dirs: Vec<String> = Vec::new();
                let mut sub_files: Vec<String> = Vec::new();
                for sub in sub_entries.flatten() {
                    let sub_name = sub.file_name().to_string_lossy().to_string();
                    if sub_name.starts_with('.') {
                        continue;
                    }
                    if sub.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        sub_dirs.push(sub_name);
                    } else {
                        sub_files.push(sub_name);
                    }
                }
                sub_dirs.sort();
                sub_files.sort();
                for s in sub_dirs {
                    if count >= max_entries { break; }
                    lines.push(format!("    📁 {s}/"));
                    count += 1;
                }
                for s in sub_files {
                    if count >= max_entries { break; }
                    lines.push(format!("    📄 {s}"));
                    count += 1;
                }
            }
        }

        for (name, size) in &files {
            if count >= max_entries {
                lines.push(format!("  ... (more entries truncated)"));
                break;
            }
            let size_str = if *size > 1024 * 1024 {
                format!("{:.1} MB", *size as f64 / (1024.0 * 1024.0))
            } else if *size > 1024 {
                format!("{:.1} KB", *size as f64 / 1024.0)
            } else {
                format!("{} B", size)
            };
            lines.push(format!("  📄 {name} ({size_str})"));
            count += 1;
        }
    }

    if lines.is_empty() {
        lines.push("  (empty project)".to_string());
    }

    Ok(lines.join("\n"))
}

/// Format recent commits (last 10).
fn format_recent_commits(project_path: &Path) -> Result<String, String> {
    let repo = BasicRepository::open(project_path)
        .map_err(|e| format!("cannot open repo: {e}"))?;

    let commits = repo
        .list_commits(None, 10)
        .map_err(|e| format!("cannot read commits: {e}"))?;

    if commits.is_empty() {
        return Ok("  (no commits yet)".to_string());
    }

    let mut lines: Vec<String> = Vec::new();
    for c in &commits {
        let short = short_id(&c.id);
        let summary = if let Some(ref ds) = c.diff_summary {
            let parsed: Option<DiffSummary> = serde_json::from_str(ds).ok();
            if let Some(s) = parsed {
                format!(" (+{}/~{}/-{})", s.added.len(), s.modified.len(), s.removed.len())
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        let op = c.operator.as_deref().unwrap_or("user");
        lines.push(format!(
            "  - {short} {op}: {}{summary}",
            c.message.lines().next().unwrap_or("")
        ));
    }

    Ok(lines.join("\n"))
}

/// Format references content from `.route/references/`.
fn format_references(project_path: &Path) -> String {
    let refs_dir = project_path.join(".route").join("references");
    if !refs_dir.exists() {
        return "  (no references configured)".to_string();
    }

    let mut lines: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&refs_dir) {
        let mut files: Vec<_> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                let preview: String = content.chars().take(500).collect();
                let truncated = if content.len() > 500 { "..." } else { "" };
                files.push((name, preview, truncated.to_string()));
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, preview, trunc) in files {
            lines.push(format!("  - {name}:\n    {preview}{trunc}"));
        }
    }

    if lines.is_empty() {
        return "  (no references files)".to_string();
    }

    lines.join("\n")
}

/// Format tracking history.
fn format_tracking_history(project_path: &Path) -> String {
    let history = crate::tracking::SyncHistory::load(project_path);
    if history.entries.is_empty() {
        return "  (no tracking history)".to_string();
    }

    let mut lines: Vec<String> = Vec::new();
    // Show last 10 entries
    let start = if history.entries.len() > 10 {
        history.entries.len() - 10
    } else {
        0
    };
    for entry in &history.entries[start..] {
        let status_icon = match entry.status.as_str() {
            "ok" => "✅",
            "conflict" => "⚠️",
            "error" => "❌",
            _ => "ℹ️",
        };
        lines.push(format!(
            "  {status_icon} {} — {} ({})",
            entry.message, entry.timestamp, entry.status
        ));
    }

    lines.join("\n")
}

/// Get the current branch name from the BasicRepository.
fn get_current_branch(project_path: &Path) -> Result<String, String> {
    let repo = BasicRepository::open(project_path)
        .map_err(|e| format!("cannot open repo: {e}"))?;

    let branch = repo
        .get_current_branch_name()
        .map_err(|e| format!("cannot read current branch: {e}"))?;

    Ok(branch)
}