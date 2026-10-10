//! Filesystem safety helpers.
//!
//! These functions enforce the invariant that **every path Route writes
//! to or removes during a rollback / restore stays inside the project
//! root**. Without them, a malicious or corrupted manifest containing
//! `../../etc/passwd` or an absolute path could make Route overwrite
//! files outside the repository.
//!
//! The model is strict and explicit: unsafe paths are **refused** with
//! a structured `RouteError::UnsafePath`. We do not silently skip them,
//! because a silent skip during rollback would leave the project in a
//! mixed state anyway.

use std::path::{Component, Path, PathBuf};

use crate::error::RouteError;

/// Validate that a relative path is safe to join onto the project root.
///
/// A path is safe iff:
/// * it is not empty,
/// * it contains no `Component::ParentDir` (`..`),
/// * it has no `Component::RootDir` (i.e. is not absolute),
/// * it has no `Component::Prefix` (Windows drive letter like `C:`),
/// * it contains no NUL bytes,
/// * after normalisation it stays inside the root (defence in depth).
///
/// On Windows, backslashes are accepted as separators. The check is
/// purely lexical — it does not touch the filesystem, so it is fast
/// and free of TOCTOU.
pub fn validate_rel_path(rel: &str) -> Result<(), RouteError> {
    if rel.is_empty() {
        return Err(RouteError::UnsafePath("empty path".into()));
    }
    if rel.contains('\0') {
        return Err(RouteError::UnsafePath(format!("nul byte in path: {rel:?}")));
    }
    let p = Path::new(rel);
    for c in p.components() {
        match c {
            Component::Normal(_) => {}
            Component::CurDir => {} // "." is harmless
            Component::ParentDir => {
                return Err(RouteError::UnsafePath(format!(
                    "path escapes project root via '..': {rel:?}"
                )));
            }
            Component::RootDir => {
                return Err(RouteError::UnsafePath(format!(
                    "absolute path refused: {rel:?}"
                )));
            }
            Component::Prefix(_) => {
                return Err(RouteError::UnsafePath(format!(
                    "windows drive prefix refused: {rel:?}"
                )));
            }
        }
    }
    Ok(())
}

/// Resolve `root.join(rel)` and verify the result is still inside `root`.
/// Returns the absolute path on success.
///
/// This is the runtime defence used at apply time: even if
/// `validate_rel_path` passed, we re-check after join. The check is
/// lexical (via `strip_prefix`); for symlink safety see
/// [`assert_no_symlink_escape`].
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, RouteError> {
    validate_rel_path(rel)?;
    let abs = root.join(rel);
    // Lexical containment check. Use `strip_prefix` on the joined path
    // against a canonicalised root when possible; if root doesn't exist
    // yet (rare in apply paths), fall back to the lexical check already
    // done by validate_rel_path.
    if let Ok(root_canon) = root.canonicalize() {
        // canonicalize follows symlinks on the *root* side; the joined
        // file may not exist yet, so we canonicalize its parent.
        let parent = abs.parent().unwrap_or(root);
        if let Ok(parent_canon) = parent.canonicalize() {
            if !parent_canon.starts_with(&root_canon) {
                return Err(RouteError::UnsafePath(format!(
                    "resolved path escapes project root: {rel:?}"
                )));
            }
        }
    }
    Ok(abs)
}

/// Walk a metadata path (a file or directory inside the project) and
/// verify that **no component along the resolved chain is a symlink
/// pointing outside `root`**. Returns `Ok(())` if the path is safe,
/// `Err(RouteError::UnsafePath)` if a symlink escape is detected.
///
/// Route's policy: symlinks are **rejected** during rollback/restore
/// rather than followed, because following them could write to the
/// symlink target (which may live outside the project). We do not
/// silently skip — we refuse with a structured error so the user knows
/// exactly why the operation stopped.
pub fn assert_no_symlink_escape(root: &Path, rel: &str) -> Result<(), RouteError> {
    validate_rel_path(rel)?;
    let root_canon = root
        .canonicalize()
        .map_err(|e| RouteError::UnsafePath(format!("cannot canonicalize project root: {e}")))?;
    // Walk each prefix component of `abs` and check for symlinks.
    let mut acc = root_canon.clone();
    let rel_path = Path::new(rel);
    for comp in rel_path.components() {
        match comp {
            Component::Normal(name) => {
                acc.push(name);
                match std::fs::symlink_metadata(&acc) {
                    Ok(md) => {
                        if md.file_type().is_symlink() {
                            // Route policy: reject symlinks outright in
                            // rollback/restore paths. We do not attempt to
                            // resolve and re-check, because a symlink that
                            // currently points inside could be repointed
                            // later, and we want a deterministic refusal.
                            return Err(RouteError::UnsafePath(format!(
                                "symlink refused in rollback/restore path: {rel:?}"
                            )));
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        // The component doesn't exist yet — that's fine
                        // for a write target. No symlink to follow.
                    }
                    Err(e) => {
                        return Err(RouteError::UnsafePath(format!("cannot stat {acc:?}: {e}")));
                    }
                }
            }
            Component::CurDir => {}
            // ParentDir / RootDir / Prefix already rejected by validate_rel_path.
            _ => {}
        }
    }
    Ok(())
}

/// Like [`assert_no_symlink_escape`] but for the *existing* file at
/// `root/rel` (used by the scanner's decision to track a file). Returns
/// `true` if the entry is a plain file (not a symlink) inside the root.
pub fn is_plain_file_inside(root: &Path, abs: &Path) -> bool {
    let Ok(root_canon) = root.canonicalize() else {
        return false;
    };
    let Ok(md) = std::fs::symlink_metadata(abs) else {
        return false;
    };
    if md.file_type().is_symlink() {
        return false;
    }
    // abs may already be absolute; verify containment.
    abs.strip_prefix(&root_canon).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(validate_rel_path("../x").is_err());
        assert!(validate_rel_path("a/../../b").is_err());
        assert!(validate_rel_path("/etc/passwd").is_err());
        assert!(validate_rel_path("C:/x").is_err());
        assert!(validate_rel_path("a\0b").is_err());
        assert!(validate_rel_path("").is_err());
    }

    #[test]
    fn accepts_safe_paths() {
        assert!(validate_rel_path("a.txt").is_ok());
        assert!(validate_rel_path("src/main.rs").is_ok());
        assert!(validate_rel_path("./a.txt").is_ok());
        assert!(validate_rel_path("a/b/c").is_ok());
    }
}
