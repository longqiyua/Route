//! Event types emitted by Route subsystems.
//!
//! Events are strongly-typed so plugin authors get exhaustiveness checking
//! when matching. Every variant carries enough context for a plugin to act
//! without re-querying the database; for deeper inspection, plugins can use
//! the `PluginContext` passed alongside the event.

use serde::{Deserialize, Serialize};

/// All Route lifecycle events.
///
/// Add new variants at the end. Plugins should use a wildcard match arm
/// (`_ => Ok(())`) so they keep working when new events are introduced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A new snapshot was created (always precedes `CommitCreated`).
    SnapshotCreated {
        snapshot_id: String,
        branch_id: String,
        branch_name: String,
        manifest_hash: String,
        timestamp: i64,
    },
    /// A new commit (edge) was recorded.
    CommitCreated {
        commit_id: String,
        branch_id: String,
        branch_name: String,
        from_snapshot: String,
        to_snapshot: String,
        message: String,
        author: Option<String>,
        kind: CommitKind,
        timestamp: i64,
    },
    /// A rollback operation started (before any state changes).
    RollbackStarted {
        target_snapshot: String,
        reason: Option<String>,
        timestamp: i64,
    },
    /// A rollback completed — a new `rollback` kind commit was created.
    RollbackCompleted {
        new_commit_id: String,
        target_snapshot: String,
        timestamp: i64,
    },
    /// A new branch was created.
    BranchCreated {
        name: String,
        kind: BranchKind,
        parent_branch: Option<String>,
        baseline_snapshot: Option<String>,
        timestamp: i64,
    },
    /// A branch was deleted.
    BranchDeleted { name: String, timestamp: i64 },
    /// A sync target run started.
    SyncStarted {
        target_name: String,
        mode: String,
        transport: String,
        timestamp: i64,
    },
    /// A sync target run completed (possibly with errors).
    SyncCompleted {
        target_name: String,
        mode: String,
        transport: String,
        files_scanned: usize,
        files_copied: usize,
        files_skipped: usize,
        files_deleted: usize,
        conflicts_resolved: usize,
        errors: usize,
        bytes_copied: u64,
        error_message: Option<String>,
        timestamp: i64,
    },
    /// An annotation was added to a commit.
    AnnotationAdded {
        commit_id: String,
        text: String,
        timestamp: i64,
    },
    /// Emitted when a repository is opened (e.g. by the CLI or GUI).
    RepoOpened {
        project_path: String,
        current_branch: String,
        timestamp: i64,
    },
}

impl Event {
    /// Return a short, human-readable label for the event kind.
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::SnapshotCreated { .. } => "snapshot_created",
            Self::CommitCreated { .. } => "commit_created",
            Self::RollbackStarted { .. } => "rollback_started",
            Self::RollbackCompleted { .. } => "rollback_completed",
            Self::BranchCreated { .. } => "branch_created",
            Self::BranchDeleted { .. } => "branch_deleted",
            Self::SyncStarted { .. } => "sync_started",
            Self::SyncCompleted { .. } => "sync_completed",
            Self::AnnotationAdded { .. } => "annotation_added",
            Self::RepoOpened { .. } => "repo_opened",
        }
    }

    /// Timestamp of the event (Unix seconds).
    pub fn timestamp(&self) -> i64 {
        match self {
            Self::SnapshotCreated { timestamp, .. }
            | Self::CommitCreated { timestamp, .. }
            | Self::RollbackStarted { timestamp, .. }
            | Self::RollbackCompleted { timestamp, .. }
            | Self::BranchCreated { timestamp, .. }
            | Self::BranchDeleted { timestamp, .. }
            | Self::SyncStarted { timestamp, .. }
            | Self::SyncCompleted { timestamp, .. }
            | Self::AnnotationAdded { timestamp, .. }
            | Self::RepoOpened { timestamp, .. } => *timestamp,
        }
    }

    /// Convenience: serialise to a JSON string (used by Logger/Webhook).
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

/// Lightweight classification of an event for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    SnapshotCreated,
    CommitCreated,
    RollbackStarted,
    RollbackCompleted,
    BranchCreated,
    BranchDeleted,
    SyncStarted,
    SyncCompleted,
    AnnotationAdded,
    RepoOpened,
}

impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Self::SnapshotCreated { .. } => EventKind::SnapshotCreated,
            Self::CommitCreated { .. } => EventKind::CommitCreated,
            Self::RollbackStarted { .. } => EventKind::RollbackStarted,
            Self::RollbackCompleted { .. } => EventKind::RollbackCompleted,
            Self::BranchCreated { .. } => EventKind::BranchCreated,
            Self::BranchDeleted { .. } => EventKind::BranchDeleted,
            Self::SyncStarted { .. } => EventKind::SyncStarted,
            Self::SyncCompleted { .. } => EventKind::SyncCompleted,
            Self::AnnotationAdded { .. } => EventKind::AnnotationAdded,
            Self::RepoOpened { .. } => EventKind::RepoOpened,
        }
    }
}

/// Commit kind — mirrors `route_basic::CommitKind` but kept independent so
/// `route-plugins` doesn't pull in the full basic crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitKind {
    Incremental,
    Full,
    Merge,
    Rollback,
}

impl CommitKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Incremental => "incremental",
            Self::Full => "full",
            Self::Merge => "merge",
            Self::Rollback => "rollback",
        }
    }
}

/// Branch kind — mirrors `route_core::BranchKind` but kept independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchKind {
    Main,
    Inherited,
    Sandbox,
}

impl BranchKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Inherited => "inherited",
            Self::Sandbox => "sandbox",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serializes_as_tagged_json() {
        let ev = Event::RepoOpened {
            project_path: "/tmp/x".into(),
            current_branch: "main".into(),
            timestamp: 1_700_000_000,
        };
        let s = ev.to_json().unwrap();
        assert!(s.contains("\"event\":\"repo_opened\""));
        assert!(s.contains("\"current_branch\":\"main\""));
    }

    #[test]
    fn kind_label_matches_snake_case() {
        let ev = Event::CommitCreated {
            commit_id: "c1".into(),
            branch_id: "b1".into(),
            branch_name: "main".into(),
            from_snapshot: "s0".into(),
            to_snapshot: "s1".into(),
            message: "hi".into(),
            author: None,
            kind: CommitKind::Incremental,
            timestamp: 0,
        };
        assert_eq!(ev.kind_label(), "commit_created");
        assert_eq!(ev.kind(), EventKind::CommitCreated);
    }

    #[test]
    fn commit_kind_serializes_snake_case() {
        let k = CommitKind::Full;
        let s = serde_json::to_string(&k).unwrap();
        assert_eq!(s, "\"full\"");
    }
}
