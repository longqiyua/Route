//! Route error model.
//!
//! Distinguishes ordinary command failures from errors that may have
//! compromised repository integrity. The former are recoverable with a
//! retry or a different input; the latter mean the user should stop,
//! inspect, and possibly run `route check` before continuing.
//!
//! Design principle: keep this small. We do not enumerate every command
//! failure — `anyhow` is fine for those. We only carve out the categories
//! where the *integrity* of the repository is at stake, so that callers
//! (CLI / MCP / TUI / AI agents) can react differently: surface a loud
//! warning, refuse further mutations, or trigger recovery.

use std::path::PathBuf;

/// A repository-integrity-class error.
///
/// When a function returns `Err(RouteError.into())` (as `anyhow::Error`),
/// callers can test `is_integrity_error(&e)` to decide whether the
/// repository may be in an inconsistent state.
#[derive(Debug, thiserror::Error)]
pub enum RouteError {
    /// The directory is not a Route repository, or its config is
    /// missing/unparseable.
    #[error("not a valid route repository: {0}")]
    InvalidRepository(String),

    /// Metadata (config, DB rows, manifest JSON) exists but is damaged
    /// or internally inconsistent.
    #[error("corrupted metadata: {0}")]
    CorruptedMetadata(String),

    /// A referenced snapshot id does not exist, or its manifest is
    /// missing.
    #[error("invalid snapshot: {0}")]
    InvalidSnapshot(String),

    /// A path in a manifest or snapshot would escape the project root
    /// (traversal, absolute path, drive letter, symlink escape). The
    /// operation was refused to protect files outside the repository.
    #[error("unsafe path refused: {0}")]
    UnsafePath(String),

    /// An OS permission denied error occurred partway through a
    /// destructive operation. The repository may be in a partially
    /// applied state and should be checked.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// A destructive operation was interrupted. Its transaction journal
    /// entry still exists and must be recovered before the repository
    /// can be trusted again.
    #[error("transaction incomplete (tx={tx_id}, phase={phase}): {detail}")]
    TransactionIncomplete {
        tx_id: String,
        phase: String,
        detail: String,
    },

    /// Recovery was attempted but failed to converge to a consistent
    /// state. Manual intervention is required.
    #[error("recovery failed: {0}")]
    RecoveryFailed(String),

    /// A file format or encoding Route does not support was encountered
    /// (e.g. a manifest with an unrecognised schema version).
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// A transaction journal directory could not be read or parsed.
    #[error("corrupted transaction journal at {path}: {detail}")]
    CorruptedJournal { path: PathBuf, detail: String },
}

impl RouteError {
    /// Returns `true` if this error indicates the repository may be in
    /// an inconsistent or partially applied state and should not be
    /// trusted without recovery or inspection.
    pub fn is_integrity_class(&self) -> bool {
        matches!(
            self,
            RouteError::CorruptedMetadata(_)
                | RouteError::InvalidSnapshot(_)
                | RouteError::UnsafePath(_)
                | RouteError::PermissionDenied(_)
                | RouteError::TransactionIncomplete { .. }
                | RouteError::RecoveryFailed(_)
                | RouteError::CorruptedJournal { .. }
        )
    }
}

/// Walk an `anyhow::Error` chain and return `true` if any layer is a
/// `RouteError` that is integrity-class. Use this at API boundaries
/// (CLI / MCP / HTTP handlers) to decide whether to surface a loud
/// "repository integrity may be affected" warning.
pub fn is_integrity_error(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<RouteError>()
            .is_some_and(|r| r.is_integrity_class())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_classification() {
        assert!(RouteError::CorruptedMetadata("x".into()).is_integrity_class());
        assert!(RouteError::UnsafePath("x".into()).is_integrity_class());
        assert!(!RouteError::InvalidRepository("x".into()).is_integrity_class());
        assert!(!RouteError::UnsupportedFormat("x".into()).is_integrity_class());
    }

    #[test]
    fn is_integrity_error_walks_chain() {
        let inner = RouteError::UnsafePath("../etc".into());
        let outer: anyhow::Error = anyhow::Error::new(inner).context("while applying rollback");
        assert!(is_integrity_error(&outer));

        let plain: anyhow::Error = anyhow::anyhow!("just a normal failure");
        assert!(!is_integrity_error(&plain));
    }
}
