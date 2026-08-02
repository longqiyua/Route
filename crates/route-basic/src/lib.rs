//! Route basic mode — graph + tree mindmap, edge-centric commits.
//!
//! Innovation: commit metadata lives on **edges** (between snapshot nodes),
//! not on nodes. Nodes are pure file states. This enables N-N relationships
//! and path-attached text annotations on edges.

pub mod ai_conflict;
pub mod export;
pub mod index;
pub mod models;
pub mod repository;

pub use ai_conflict::{
    list_verdicts as list_conflict_verdicts, parse_body as parse_conflict_body,
    record_verdict as record_conflict_verdict, report_for_commit as conflict_report_for_commit,
    AiConflict, AiConflictReport, AiConflictVerdict,
};
pub use export::{ExportContext, ExportFormat, Exporter};
pub use export::exporters::DefaultExporters;
pub use index::{build_index, write_index, IndexBranch, IndexCommit, IndexFile, RouteIndex};
pub use models::{
    AiPrompt, Branch, BranchKind, Commit, CommitKind, CommitPathAnnotation, DiffSummary,
    ManifestEntry, RepoConfig, Snapshot, SnapshotWithBranch, Tag, TrackConfig, TrackOs,
};
pub use repository::{
    BasicRepository, CommitDiffEntry, CommitOptions, CreateBranchOptions, FileRevision,
    RollbackOptions, SnapshotDiffEntry, WorkingFileStatus,
};
