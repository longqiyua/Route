//! Stats collector — runs SELECT-only queries against BasicRepository.

use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Duration, Timelike, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use route_basic::BasicRepository;

use crate::models::{
    BranchStats, FileStat, RepoStats, StorageStats, SummaryStats, TimeRange, TimelineBucket,
    TimelineKind,
};

/// Entry point for collecting stats.
pub struct StatsCollector;

/// Optional filters for collection.
#[derive(Debug, Clone)]
pub struct CollectOptions {
    /// Restrict commit-derived stats to a time range.
    pub range: Option<TimeRange>,
    /// Number of top files to return.
    pub top_files_limit: usize,
    /// How many hourly buckets to return (most recent N).
    pub hourly_buckets: usize,
    /// How many daily buckets to return (most recent N).
    pub daily_buckets: usize,
}

impl CollectOptions {
    /// Sensible defaults: top 20 files, 24 hourly buckets, 30 daily buckets.
    pub fn defaults() -> Self {
        Self {
            range: None,
            top_files_limit: 20,
            hourly_buckets: 24,
            daily_buckets: 30,
        }
    }
}

impl Default for CollectOptions {
    fn default() -> Self {
        Self::defaults()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiffSummaryJson {
    added: Vec<String>,
    modified: Vec<String>,
    removed: Vec<String>,
}

impl StatsCollector {
    /// Collect stats with default options.
    pub fn collect(repo: &BasicRepository) -> Result<RepoStats> {
        Self::collect_with(repo, CollectOptions::defaults())
    }

    /// Collect stats with custom options.
    pub fn collect_with(repo: &BasicRepository, opts: CollectOptions) -> Result<RepoStats> {
        let project_path = repo.project_path().to_string_lossy().to_string();
        let collected_at = Utc::now().timestamp_millis();

        let summary = collect_summary(repo, opts.range.as_ref())?;
        let branches = collect_branch_stats(repo, opts.range.as_ref())?;
        let commits_by_kind = collect_commits_by_kind(repo, opts.range.as_ref())?;
        let commits_by_hour = collect_timeline(
            repo,
            TimelineKind::Hourly,
            opts.hourly_buckets,
            opts.range.as_ref(),
        )?;
        let commits_by_day = collect_timeline(
            repo,
            TimelineKind::Daily,
            opts.daily_buckets,
            opts.range.as_ref(),
        )?;
        let top_files = collect_top_files(repo, opts.top_files_limit, opts.range.as_ref())?;
        let storage = collect_storage_stats(repo)?;

        Ok(RepoStats {
            project_path,
            collected_at,
            summary,
            branches,
            commits_by_kind,
            commits_by_hour,
            commits_by_day,
            top_files,
            storage,
            range: opts.range,
        })
    }
}

// ---------------------------------------------------------------------------
// Per-section collectors
// ---------------------------------------------------------------------------

fn collect_summary(repo: &BasicRepository, range: Option<&TimeRange>) -> Result<SummaryStats> {
    let conn = repo.db.lock();
    let branch_count: i64 = conn.query_row("SELECT COUNT(*) FROM branches", [], |r| r.get(0))?;
    let snapshot_count: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))?;
    let annotation_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM commit_path_annotations", [], |r| {
            r.get(0)
        })?;

    let (commit_count, first_commit_at, latest_commit_at): (i64, Option<i64>, Option<i64>) =
        if let Some(r) = range {
            conn.query_row(
                "SELECT COUNT(*), MIN(created_at), MAX(created_at) FROM commits WHERE created_at >= ?1 AND created_at <= ?2",
                params![r.from, r.to],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?
        } else {
            conn.query_row(
                "SELECT COUNT(*), MIN(created_at), MAX(created_at) FROM commits",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?
        };

    let active_days = match (first_commit_at, latest_commit_at) {
        (Some(f), Some(l)) => {
            let f_ms = DateTime::<Utc>::from_timestamp_millis(f).unwrap_or_default();
            let l_ms = DateTime::<Utc>::from_timestamp_millis(l).unwrap_or_default();
            (l_ms.date_naive() - f_ms.date_naive()).num_days().max(0)
        }
        _ => 0,
    };

    Ok(SummaryStats {
        branch_count: branch_count as usize,
        snapshot_count: snapshot_count as usize,
        commit_count: commit_count as usize,
        annotation_count: annotation_count as usize,
        first_commit_at,
        latest_commit_at,
        active_days,
    })
}

fn collect_branch_stats(
    repo: &BasicRepository,
    range: Option<&TimeRange>,
) -> Result<Vec<BranchStats>> {
    // IMPORTANT: list_branches() also locks the db mutex — call it BEFORE locking here
    // to avoid deadlock (std::sync::Mutex is NOT reentrant).
    let branches = repo.list_branches()?;
    let conn = repo.db.lock();
    let mut out = Vec::with_capacity(branches.len());
    for b in branches {
        let (commit_count, latest_commit_at): (i64, Option<i64>) = match range {
            Some(r) => conn
                .query_row(
                    "SELECT COUNT(*), MAX(created_at) FROM commits WHERE branch_id = ?1 AND created_at >= ?2 AND created_at <= ?3",
                    params![b.id, r.from, r.to],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((0, None)),
            None => conn
                .query_row(
                    "SELECT COUNT(*), MAX(created_at) FROM commits WHERE branch_id = ?1",
                    params![b.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((0, None)),
        };
        out.push(BranchStats {
            name: b.name,
            kind: b.kind.as_str().to_string(),
            commit_count: commit_count as usize,
            latest_commit_at,
            head_snapshot: b.head_snapshot,
        });
    }
    Ok(out)
}

fn collect_commits_by_kind(
    repo: &BasicRepository,
    range: Option<&TimeRange>,
) -> Result<HashMap<String, usize>> {
    let conn = repo.db.lock();
    let sql = if range.is_some() {
        "SELECT kind, COUNT(*) FROM commits WHERE created_at >= ?1 AND created_at <= ?2 GROUP BY kind"
    } else {
        "SELECT kind, COUNT(*) FROM commits GROUP BY kind"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(r) = range {
        stmt.query_map(params![r.from, r.to], kind_count_row)?
    } else {
        stmt.query_map([], kind_count_row)?
    };
    let mut out = HashMap::new();
    for r in rows {
        let (kind, count) = r?;
        out.insert(kind, count);
    }
    Ok(out)
}

fn kind_count_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, usize)> {
    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
}

fn collect_timeline(
    repo: &BasicRepository,
    kind: TimelineKind,
    buckets: usize,
    range: Option<&TimeRange>,
) -> Result<Vec<TimelineBucket>> {
    if buckets == 0 {
        return Ok(vec![]);
    }

    let conn = repo.db.lock();
    let mut stmt = conn.prepare("SELECT created_at FROM commits")?;
    let mut all_ts: Vec<i64> = Vec::new();
    {
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let ts: i64 = row.get(0)?;
            if let Some(r) = range {
                if ts < r.from || ts > r.to {
                    continue;
                }
            }
            all_ts.push(ts);
        }
    }
    drop(stmt);

    if all_ts.is_empty() {
        return Ok(vec![]);
    }

    // Determine the bucket boundaries: most recent N buckets ending at the latest commit.
    let latest = *all_ts.iter().max().unwrap();
    let latest_dt = DateTime::<Utc>::from_timestamp_millis(latest).unwrap_or_else(Utc::now);

    let mut bucket_starts: Vec<i64> = Vec::with_capacity(buckets);
    for i in (0..buckets).rev() {
        let bs = match kind {
            TimelineKind::Hourly => {
                let dt = latest_dt - Duration::hours(i as i64);
                dt.with_minute(0)
                    .and_then(|d| d.with_second(0))
                    .and_then(|d| d.with_nanosecond(0))
                    .unwrap_or(dt)
                    .timestamp_millis()
            }
            TimelineKind::Daily => {
                let dt = latest_dt - Duration::days(i as i64);
                dt.date_naive()
                    .and_hms_opt(0, 0, 0)
                    .map(|ndt| {
                        DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc).timestamp_millis()
                    })
                    .unwrap_or(dt.timestamp_millis())
            }
        };
        bucket_starts.push(bs);
    }
    bucket_starts.sort();

    let mut counts = vec![0usize; bucket_starts.len()];
    for ts in all_ts {
        // Find the bucket this ts falls into
        let idx = match bucket_starts.binary_search(&ts) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        if idx < counts.len() && ts >= bucket_starts[idx] {
            // For all but the last bucket, ensure ts < next bucket start
            let in_bucket = if idx + 1 < bucket_starts.len() {
                ts < bucket_starts[idx + 1]
            } else {
                true
            };
            if in_bucket {
                counts[idx] += 1;
            }
        }
    }

    Ok(bucket_starts
        .iter()
        .zip(counts.iter())
        .map(|(ts, c)| TimelineBucket {
            ts: *ts,
            kind,
            count: *c,
        })
        .collect())
}

fn collect_top_files(
    repo: &BasicRepository,
    limit: usize,
    range: Option<&TimeRange>,
) -> Result<Vec<FileStat>> {
    if limit == 0 {
        return Ok(vec![]);
    }

    let commits = repo.list_commits(None, 100000)?;
    let commits_iter = commits.into_iter().filter(|c| {
        if let Some(r) = range {
            c.created_at >= r.from && c.created_at <= r.to
        } else {
            true
        }
    });

    // path → (modifications, last_seen_at)
    let mut files: HashMap<String, (usize, i64)> = HashMap::new();
    for c in commits_iter {
        if let Some(diff_json) = &c.diff_summary {
            if let Ok(d) = serde_json::from_str::<DiffSummaryJson>(diff_json) {
                for path in d
                    .added
                    .iter()
                    .chain(d.modified.iter())
                    .chain(d.removed.iter())
                {
                    let entry = files.entry(path.clone()).or_insert((0, 0));
                    entry.0 += 1;
                    if c.created_at > entry.1 {
                        entry.1 = c.created_at;
                    }
                }
            }
        }
    }

    let mut out: Vec<FileStat> = files
        .into_iter()
        .map(|(path, (m, t))| FileStat {
            path,
            modifications: m,
            last_seen_at: t,
        })
        .collect();
    out.sort_by(|a, b| {
        b.modifications
            .cmp(&a.modifications)
            .then_with(|| a.path.cmp(&b.path))
    });
    out.truncate(limit);
    Ok(out)
}

fn collect_storage_stats(repo: &BasicRepository) -> Result<StorageStats> {
    let conn = repo.db.lock();

    let (blob_count, total_blob_size): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0) FROM blobs",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));

    let manifest_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM manifests", [], |r| r.get(0))
        .unwrap_or(0);

    // 1. Load hash → size map first, drop the statement before re-using the connection.
    let mut hash_to_size: HashMap<String, u64> = HashMap::new();
    {
        let mut s = conn.prepare("SELECT hash, size FROM blobs")?;
        let mut r = s.query([])?;
        while let Some(row) = r.next()? {
            let h: String = row.get(0)?;
            let sz: i64 = row.get(1)?;
            hash_to_size.insert(h, sz as u64);
        }
    }

    // 2. Load all manifest contents, drop the statement.
    let manifest_contents: Vec<String> = {
        let mut s = conn.prepare("SELECT content FROM manifests")?;
        let mut r = s.query([])?;
        let mut out = Vec::new();
        while let Some(row) = r.next()? {
            out.push(row.get::<_, String>(0)?);
        }
        out
    };

    // 3. Compute logical size outside any SQL borrow.
    let mut logical_size: u64 = 0;
    for content in &manifest_contents {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(content) {
            for (_path, hash) in map {
                if let Some(size) = hash_to_size.get(&hash) {
                    logical_size += size;
                }
            }
        }
    }

    let physical = total_blob_size as u64;
    let dedup_ratio = if logical_size > 0 {
        (logical_size - physical) as f64 / logical_size as f64
    } else {
        0.0
    };

    Ok(StorageStats {
        blob_count: blob_count as usize,
        total_blob_size: physical,
        manifest_count: manifest_count as usize,
        logical_size,
        dedup_ratio,
    })
}

// ---------------------------------------------------------------------------
// Helper: naive datetime formatting for renderers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub(crate) fn format_ts(ts: i64) -> String {
    let dt = DateTime::<Utc>::from_timestamp_millis(ts).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M UTC").to_string()
}

#[allow(dead_code)]
pub(crate) fn format_bucket(ts: i64, kind: TimelineKind) -> String {
    let dt = DateTime::<Utc>::from_timestamp_millis(ts).unwrap_or_default();
    match kind {
        TimelineKind::Hourly => dt.format("%m-%d %H:00").to_string(),
        TimelineKind::Daily => dt.format("%Y-%m-%d").to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use route_basic::{BasicRepository, CommitOptions};
    use tempfile::TempDir;

    fn setup_repo_with_commits() -> (TempDir, BasicRepository) {
        let tmp = TempDir::new().unwrap();
        let repo = BasicRepository::init(tmp.path()).unwrap();

        // Commit 1: add a.txt
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        repo.commit(CommitOptions {
            message: "add a".into(),
            ..Default::default()
        })
        .unwrap();

        // Commit 2: modify a.txt, add b.txt
        std::fs::write(tmp.path().join("a.txt"), b"hello world").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"b file").unwrap();
        repo.commit(CommitOptions {
            message: "update a, add b".into(),
            ..Default::default()
        })
        .unwrap();

        // Commit 3: remove b.txt
        std::fs::remove_file(tmp.path().join("b.txt")).unwrap();
        repo.commit(CommitOptions {
            message: "remove b".into(),
            ..Default::default()
        })
        .unwrap();

        (tmp, repo)
    }

    #[test]
    fn collect_returns_basic_counts() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();

        assert_eq!(stats.summary.commit_count, 3);
        assert_eq!(stats.summary.branch_count, 1); // main only
        assert_eq!(stats.summary.snapshot_count, 3); // 3 commits = 3 snapshots (each commit creates a new snapshot)
        assert!(stats.summary.first_commit_at.is_some());
        assert!(stats.summary.latest_commit_at.is_some());
        assert_eq!(stats.summary.active_days, 0); // all within same day
    }

    #[test]
    fn commits_by_kind_counts_incremental() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();
        assert_eq!(stats.commits_by_kind.get("incremental"), Some(&3));
    }

    #[test]
    fn top_files_ranks_modified_files() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();
        assert!(!stats.top_files.is_empty());

        // a.txt was touched in all 3 commits (added, modified, then NOT in commit 3's diff since removal of b is the diff)
        // Actually a.txt is added in commit 1, modified in commit 2, NOT in commit 3 diff (only b removed)
        // So a.txt has 2 modifications
        let a = stats.top_files.iter().find(|f| f.path.ends_with("a.txt"));
        assert!(
            a.is_some(),
            "a.txt should be in top files: {:?}",
            stats.top_files
        );
        assert_eq!(a.unwrap().modifications, 2);
    }

    #[test]
    fn storage_stats_reflects_blobs() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();
        assert!(
            stats.storage.blob_count > 0,
            "should have at least one blob"
        );
        assert!(stats.storage.total_blob_size > 0);
        assert!(stats.storage.manifest_count >= 1);
        assert!(stats.storage.logical_size >= stats.storage.total_blob_size);
        assert!(stats.storage.dedup_ratio >= 0.0 && stats.storage.dedup_ratio <= 1.0);
    }

    #[test]
    fn branch_stats_lists_main() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();
        assert_eq!(stats.branches.len(), 1);
        assert_eq!(stats.branches[0].name, "main");
        assert_eq!(stats.branches[0].commit_count, 3);
    }

    #[test]
    fn render_markdown_includes_sections() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();
        let md = crate::render::render_markdown(&stats).unwrap();
        assert!(md.contains("Route 统计报告"));
        assert!(md.contains("## 概览"));
        assert!(md.contains("## Commit 类型分布"));
        assert!(md.contains("## 分支活动度"));
        assert!(md.contains("## 存储统计"));
    }

    #[test]
    fn render_json_is_valid_json() {
        let (_tmp, repo) = setup_repo_with_commits();
        let stats = StatsCollector::collect(&repo).unwrap();
        let json = crate::render::render_json(&stats).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
        assert!(parsed["summary"]["commit_count"].as_u64() == Some(3));
    }

    #[test]
    fn range_filter_restricts_commits() {
        let (_tmp, repo) = setup_repo_with_commits();
        // Use a future range — should yield zero commits
        let now = chrono::Utc::now().timestamp_millis();
        let opts = CollectOptions {
            range: Some(TimeRange {
                from: now + 86400_000,
                to: now + 2 * 86400_000,
            }),
            ..CollectOptions::defaults()
        };
        let stats = StatsCollector::collect_with(&repo, opts).unwrap();
        assert_eq!(stats.summary.commit_count, 0);
        assert!(stats.top_files.is_empty());
    }
}
