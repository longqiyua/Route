//! Route statistics — derived, read-only analytics for basic mode.
//!
//! route-stats is a thin layer over BasicRepository that computes
//! real-time metrics: branch activity, commit frequency timelines,
//! top-modified files, storage dedup ratios, etc.
//!
//! It does NOT modify the repository — all queries are SELECT-only.

pub mod collector;
pub mod models;
pub mod render;

pub use collector::{CollectOptions, StatsCollector};
pub use models::{
    BranchStats, FileStat, RepoStats, StorageStats, SummaryStats, TimeRange, TimelineBucket,
    TimelineKind,
};
pub use render::{render_json, render_markdown};
