//! Project structure tracking.
//!
//! Maintains a snapshot of the project's file tree, automatically updated
//! when changes are detected. Used by AI to understand project layout
//! without scanning the entire filesystem.

use std::collections::BTreeMap;

/// A single entry in the project structure snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructureEntry {
    /// Relative path from project root (e.g., "src/main.rs").
    pub path: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
    /// Last modified timestamp (millis).
    pub modified_at: i64,
}

/// A snapshot of the project's file tree at a point in time.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructureSnapshot {
    /// All entries, keyed by relative path.
    pub entries: BTreeMap<String, StructureEntry>,
    /// Timestamp when this snapshot was taken (millis).
    pub captured_at: i64,
    /// Total number of files.
    pub file_count: usize,
    /// Total number of directories.
    pub dir_count: usize,
}

impl StructureSnapshot {
    /// Create a new empty snapshot.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            captured_at: chrono::Utc::now().timestamp_millis(),
            file_count: 0,
            dir_count: 0,
        }
    }

    /// Get the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Format as a compact tree string for AI context injection.
    ///
    /// Respects max_tokens budget. Returns (formatted, was_truncated).
    pub fn format_tree(&self, max_lines: usize) -> (String, bool) {
        let mut lines: Vec<String> = Vec::new();
        lines.push("=== Project Structure ===\n".to_string());

        // Group entries by directory
        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();

        for (path, entry) in &self.entries {
            if entry.is_dir {
                dirs.push(path.clone());
            } else {
                files.push(path.clone());
            }
        }

        // Show directories first, then files
        for dir in &dirs {
            if lines.len() >= max_lines {
                lines.push(format!(
                    "  ... ({} more entries)",
                    self.entries.len() - lines.len()
                ));
                return (lines.join("\n"), true);
            }
            let depth = dir.matches('/').count();
            let indent = "  ".repeat(depth);
            let name = dir.rsplit('/').next().unwrap_or(dir);
            lines.push(format!("{}{}/", indent, name));
        }

        for file in &files {
            if lines.len() >= max_lines {
                lines.push(format!(
                    "  ... ({} more entries)",
                    self.entries.len() - lines.len()
                ));
                return (lines.join("\n"), true);
            }
            let depth = file.matches('/').count();
            let indent = "  ".repeat(depth);
            let name = file.rsplit('/').next().unwrap_or(file);
            let entry = &self.entries[file];
            let size = if entry.size > 1024 * 1024 {
                format!("{:.1} MB", entry.size as f64 / (1024.0 * 1024.0))
            } else if entry.size > 1024 {
                format!("{:.1} KB", entry.size as f64 / 1024.0)
            } else {
                format!("{} B", entry.size)
            };
            lines.push(format!("{}{}  ({})", indent, name, size));
        }

        (lines.join("\n"), false)
    }
}

/// Builder for creating structure snapshots from a real filesystem.
#[derive(Debug, Clone)]
pub struct ProjectStructure;

impl ProjectStructure {
    /// Scan a project directory and create a structure snapshot.
    ///
    /// Skips common ignored directories (.git, node_modules, target, etc.)
    /// and respects depth limits.
    pub fn scan(
        project_path: &std::path::Path,
        max_depth: usize,
    ) -> std::io::Result<StructureSnapshot> {
        let mut entries = BTreeMap::new();
        let mut file_count = 0;
        let mut dir_count = 0;

        let skip_dirs: &[&str] = &[
            ".git",
            ".route",
            ".route-basic",
            "node_modules",
            "target",
            ".next",
            "dist",
            "build",
        ];

        if project_path.exists() {
            Self::scan_recursive(
                project_path,
                project_path,
                0,
                max_depth,
                skip_dirs,
                &mut entries,
                &mut file_count,
                &mut dir_count,
            )?;
        }

        Ok(StructureSnapshot {
            entries,
            captured_at: chrono::Utc::now().timestamp_millis(),
            file_count,
            dir_count,
        })
    }

    fn scan_recursive(
        root: &std::path::Path,
        dir: &std::path::Path,
        depth: usize,
        max_depth: usize,
        skip_dirs: &[&str],
        entries: &mut BTreeMap<String, StructureEntry>,
        file_count: &mut usize,
        dir_count: &mut usize,
    ) -> std::io::Result<()> {
        if depth > max_depth {
            return Ok(());
        }

        if let Ok(read_dir) = std::fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();

                // Skip hidden files and ignored directories
                if skip_dirs.contains(&name.as_str()) {
                    continue;
                }
                if name.starts_with('.') {
                    continue;
                }

                let rel_path = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string()
                    .replace('\\', "/");

                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let size = if is_dir {
                    0
                } else {
                    entry.metadata().map(|m| m.len()).unwrap_or(0)
                };
                let modified_at = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);

                entries.insert(
                    rel_path.clone(),
                    StructureEntry {
                        path: rel_path,
                        is_dir,
                        size,
                        modified_at,
                    },
                );

                if is_dir {
                    *dir_count += 1;
                    Self::scan_recursive(
                        root,
                        &path,
                        depth + 1,
                        max_depth,
                        skip_dirs,
                        entries,
                        file_count,
                        dir_count,
                    )?;
                } else {
                    *file_count += 1;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_snapshot() {
        let snap = StructureSnapshot::new();
        assert!(snap.is_empty());
    }

    #[test]
    fn test_new_snapshot_has_timestamp() {
        let snap = StructureSnapshot::new();
        assert!(snap.captured_at > 0);
    }
}
