//! Statistics data models — all derived from the basic-mode schema.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level stats container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStats {
    pub project_path: String,
    pub collected_at: i64,
    pub summary: SummaryStats,
    pub branches: Vec<BranchStats>,
    pub commits_by_kind: HashMap<String, usize>,
    pub commits_by_hour: Vec<TimelineBucket>,
    pub commits_by_day: Vec<TimelineBucket>,
    pub top_files: Vec<FileStat>,
    pub storage: StorageStats,
    /// Optional range filter applied (None = no filter).
    pub range: Option<TimeRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: i64,
    pub to: i64,
}

/// Aggregate counts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SummaryStats {
    pub branch_count: usize,
    pub snapshot_count: usize,
    pub commit_count: usize,
    pub annotation_count: usize,
    pub first_commit_at: Option<i64>,
    pub latest_commit_at: Option<i64>,
    /// Days between first and latest commit (0 if fewer than 2 commits).
    pub active_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchStats {
    pub name: String,
    pub kind: String,
    pub commit_count: usize,
    pub latest_commit_at: Option<i64>,
    pub head_snapshot: Option<String>,
}

/// Aggregation bucket kind for commit timelines.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineKind {
    Hourly,
    Daily,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineBucket {
    /// Bucket start timestamp (millis, UTC).
    pub ts: i64,
    pub kind: TimelineKind,
    pub count: usize,
}

/// A file's modification stats (derived from diff_summary JSON across commits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStat {
    pub path: String,
    pub modifications: usize,
    pub last_seen_at: i64,
}

/// Storage statistics (blobs + manifests).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageStats {
    pub blob_count: usize,
    pub total_blob_size: u64,
    pub manifest_count: usize,
    /// Sum of all snapshot file sizes (with duplication across snapshots).
    pub logical_size: u64,
    /// (logical - physical) / logical, in [0, 1].
    pub dedup_ratio: f64,
}
