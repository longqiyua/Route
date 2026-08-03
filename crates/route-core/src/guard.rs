//! Route Guard — protection mechanism for Route data files.
//!
//! # Purpose
//!
//! The guard prevents AI from accidentally deleting or modifying Route's
//! internal data files (`.route/`, `.route-basic/`) when context changes
//! or when the AI's context window grows too long.
//!
//! # How it works
//!
//! 1. **Protected paths** — certain directories and files are registered as
//!    protected (Route's own data).
//! 2. **Permission check** — before any write/delete operation, call
//!    `check_protected(path)` to verify the operation is allowed.
//! 3. **Context anchor** — a `.route-guard` file is placed in the project
//!    root as a persistent anchor that AI can use to locate Route even
//!    after context changes.
//!
//! # Usage
//!
//! ```rust
//! use route_core::guard::{check_protected, GuardResult};
//! use std::path::Path;
//!
//! // Check if a path is protected
//! let root = Path::new("/project");
//! let target = Path::new("/project/.route/config.json");
//! match check_protected(root, target) {
//!     GuardResult::Protected => println!("Operation blocked: path is protected"),
//!     GuardResult::Allowed => println!("Operation allowed"),
//! }
//! ```

use std::path::{Path, PathBuf};

/// Result of a guard check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardResult {
    /// The operation is allowed.
    Allowed,
    /// The path is protected — operation should be blocked.
    Protected,
}

/// Guard configuration and state.
#[derive(Debug, Clone)]
pub struct RouteGuard {
    /// List of protected directory names (relative to project root).
    protected_dirs: Vec<String>,
    /// List of protected file names (relative to project root).
    protected_files: Vec<String>,
}

impl Default for RouteGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteGuard {
    /// Create a new guard with default protected paths.
    ///
    /// Default protected directories:
    /// - `.route/` — Route base configuration
    /// - `.route-basic/` — Route basic mode data
    /// - `.route/storage/` — Route storage
    pub fn new() -> Self {
        Self {
            protected_dirs: vec![
                ".route".to_string(),
                ".route-basic".to_string(),
            ],
            protected_files: vec![
                ".route-guard".to_string(),
            ],
        }
    }

    /// Register an additional protected directory.
    pub fn add_protected_dir(&mut self, dir: &str) {
        if !self.protected_dirs.contains(&dir.to_string()) {
            self.protected_dirs.push(dir.to_string());
        }
    }

    /// Register an additional protected file.
    pub fn add_protected_file(&mut self, file: &str) {
        if !self.protected_files.contains(&file.to_string()) {
            self.protected_files.push(file.to_string());
        }
    }

    /// Check if a path is protected.
    ///
    /// Returns `GuardResult::Protected` if the path is inside a protected
    /// directory or matches a protected file name.
    pub fn check(&self, project_root: &Path, target: &Path) -> GuardResult {
        // First, check if target is a protected file by name
        if let Some(file_name) = target.file_name() {
            let name = file_name.to_string_lossy();
            if self.protected_files.contains(&name.to_string()) {
                return GuardResult::Protected;
            }
        }

        // Check if target is inside a protected directory
        for dir in &self.protected_dirs {
            let protected_path = project_root.join(dir);
            if target.starts_with(&protected_path) {
                return GuardResult::Protected;
            }
        }

        GuardResult::Allowed
    }

    /// Check if a path is protected, using the current working directory as project root.
    pub fn check_cwd(&self, target: &Path) -> GuardResult {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.check(&cwd, target)
    }
}

// ---------------------------------------------------------------------------
// Standalone functions for convenience
// ---------------------------------------------------------------------------

/// Global default guard instance.
static GUARD: std::sync::LazyLock<RouteGuard> = std::sync::LazyLock::new(RouteGuard::new);

/// Check if a path is protected (uses global guard).
pub fn check_protected(project_root: &Path, target: &Path) -> GuardResult {
    GUARD.check(project_root, target)
}

/// Convenience: check if a path is protected using the current directory.
pub fn check_protected_cwd(target: &Path) -> GuardResult {
    GUARD.check_cwd(target)
}

/// Ensure the `.route-guard` anchor file exists in the project root.
///
/// This serves as a persistent marker that AI can use to find Route
/// even after context changes or when the context window is refreshed.
pub fn ensure_guard_anchor(project_root: &Path) -> std::io::Result<()> {
    let anchor_path = project_root.join(".route-guard");
    if !anchor_path.exists() {
        std::fs::write(
            &anchor_path,
            concat!(
                "# Route Guard — Do not delete or modify this file.\n",
                "#\n",
                "# This file marks that Route is managing this project.\n",
                "# If you are an AI assistant, keep this file intact.\n",
                "# Route's data is in `.route/` and `.route-basic/` directories.\n",
                "#\n",
                "# Route: From Route to Routine.\n",
            ),
        )?;

        // Make it read-only on Unix systems (best-effort)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&anchor_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o444); // read-only for all
                let _ = std::fs::set_permissions(&anchor_path, perms);
            }
        }
    }
    Ok(())
}

/// Check if a project has Route active (detects `.route-guard` anchor).
pub fn is_route_active(project_root: &Path) -> bool {
    project_root.join(".route-guard").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protected_route_dir() {
        let guard = RouteGuard::new();
        let root = Path::new("/project");
        let target = root.join(".route").join("config.json");
        assert_eq!(guard.check(root, &target), GuardResult::Protected);
    }

    #[test]
    fn test_protected_route_basic_dir() {
        let guard = RouteGuard::new();
        let root = Path::new("/project");
        let target = root.join(".route-basic").join("db.sqlite");
        assert_eq!(guard.check(root, &target), GuardResult::Protected);
    }

    #[test]
    fn test_protected_guard_file() {
        let guard = RouteGuard::new();
        let root = Path::new("/project");
        let target = root.join(".route-guard");
        assert_eq!(guard.check(root, &target), GuardResult::Protected);
    }

    #[test]
    fn test_allowed_path() {
        let guard = RouteGuard::new();
        let root = Path::new("/project");
        let target = root.join("src").join("main.rs");
        assert_eq!(guard.check(root, &target), GuardResult::Allowed);
    }

    #[test]
    fn test_ensure_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let result = ensure_guard_anchor(dir.path());
        assert!(result.is_ok());
        assert!(dir.path().join(".route-guard").exists());
    }
}