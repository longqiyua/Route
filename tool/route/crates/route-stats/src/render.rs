//! Renderers for RepoStats — JSON and Markdown.

use crate::collector::{format_bucket, format_ts};
use crate::models::{RepoStats, TimelineKind};

/// Render as pretty-printed JSON.
pub fn render_json(stats: &RepoStats) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(stats)?)
}

/// Render as a Markdown report.
pub fn render_markdown(stats: &RepoStats) -> anyhow::Result<String> {
    let mut s = String::new();
    s.push_str("# Route 统计报告\n\n");
    s.push_str(&format!("- **项目路径**: `{}`\n", stats.project_path));
    s.push_str(&format!(
        "- **采集时间**: {}\n",
        format_ts(stats.collected_at)
    ));
    if let Some(r) = &stats.range {
        s.push_str(&format!(
            "- **范围过滤**: {} → {}\n",
            format_ts(r.from),
            format_ts(r.to)
        ));
    }
    s.push('\n');

    // Summary
    s.push_str("## 概览\n\n");
    s.push_str(&format!("- 分支数: **{}**\n", stats.summary.branch_count));
    s.push_str(&format!("- 快照数: **{}**\n", stats.summary.snapshot_count));
    s.push_str(&format!(
        "- Commit 数: **{}**\n",
        stats.summary.commit_count
    ));
    s.push_str(&format!(
        "- 注释数: **{}**\n",
        stats.summary.annotation_count
    ));
    s.push_str(&format!(
        "- 首次 commit: {}\n",
        stats
            .summary
            .first_commit_at
            .map(format_ts)
            .unwrap_or_else(|| "—".into())
    ));
    s.push_str(&format!(
        "- 最近 commit: {}\n",
        stats
            .summary
            .latest_commit_at
            .map(format_ts)
            .unwrap_or_else(|| "—".into())
    ));
    s.push_str(&format!("- 活跃天数: **{}**\n", stats.summary.active_days));
    s.push('\n');

    // Commits by kind
    s.push_str("## Commit 类型分布\n\n");
    if stats.commits_by_kind.is_empty() {
        s.push_str("_(无 commit)_\n");
    } else {
        s.push_str("| 类型 | 数量 |\n| --- | ---: |\n");
        let mut kinds: Vec<_> = stats.commits_by_kind.iter().collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1));
        for (k, v) in kinds {
            s.push_str(&format!("| {} | {} |\n", k, v));
        }
    }
    s.push('\n');

    // Branches
    s.push_str("## 分支活动度\n\n");
    if stats.branches.is_empty() {
        s.push_str("_(无分支)_\n");
    } else {
        s.push_str(
            "| 分支 | 类型 | Commit 数 | 最近 commit | HEAD |\n| --- | --- | ---: | --- | --- |\n",
        );
        for b in &stats.branches {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                b.name,
                b.kind,
                b.commit_count,
                b.latest_commit_at
                    .map(format_ts)
                    .unwrap_or_else(|| "—".into()),
                b.head_snapshot
                    .as_deref()
                    .map(short)
                    .unwrap_or_else(|| "—".into()),
            ));
        }
    }
    s.push('\n');

    // Hourly timeline
    s.push_str("## Commit 时间分布（按小时，最近 24 桶）\n\n");
    render_timeline(&mut s, &stats.commits_by_hour, TimelineKind::Hourly);
    s.push('\n');

    // Daily timeline
    s.push_str("## Commit 时间分布（按天，最近 30 桶）\n\n");
    render_timeline(&mut s, &stats.commits_by_day, TimelineKind::Daily);
    s.push('\n');

    // Top files
    s.push_str("## 改动最多的文件\n\n");
    if stats.top_files.is_empty() {
        s.push_str("_(无文件改动记录)_\n");
    } else {
        s.push_str("| 文件路径 | 改动次数 | 最近出现 |\n| --- | ---: | --- |\n");
        for f in &stats.top_files {
            s.push_str(&format!(
                "| `{}` | {} | {} |\n",
                f.path,
                f.modifications,
                format_ts(f.last_seen_at),
            ));
        }
    }
    s.push('\n');

    // Storage
    s.push_str("## 存储统计\n\n");
    s.push_str(&format!("- Blob 数: **{}**\n", stats.storage.blob_count));
    s.push_str(&format!(
        "- 物理大小: **{}**\n",
        format_bytes(stats.storage.total_blob_size)
    ));
    s.push_str(&format!(
        "- 逻辑大小（含跨快照重复）: **{}**\n",
        format_bytes(stats.storage.logical_size)
    ));
    s.push_str(&format!(
        "- Manifest 数: **{}**\n",
        stats.storage.manifest_count
    ));
    s.push_str(&format!(
        "- 去重率: **{:.2}%**\n",
        stats.storage.dedup_ratio * 100.0
    ));

    Ok(s)
}

fn render_timeline(s: &mut String, buckets: &[crate::models::TimelineBucket], _kind: TimelineKind) {
    if buckets.is_empty() {
        s.push_str("_(无数据)_\n");
        return;
    }
    let max_count = buckets.iter().map(|b| b.count).max().unwrap_or(0).max(1);
    for b in buckets {
        let bar_len = (b.count * 30 / max_count).max(if b.count > 0 { 1 } else { 0 });
        let bar: String = "█".repeat(bar_len);
        s.push_str(&format!(
            "- `{}` {}: {}\n",
            format_bucket(b.ts, b.kind),
            bar,
            b.count
        ));
    }
}

fn short(s: &str) -> String {
    if s.len() > 8 {
        s[..8].to_string()
    } else {
        s.to_string()
    }
}

fn format_bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if n >= GB {
        format!("{:.2} GB", n as f64 / GB as f64)
    } else if n >= MB {
        format!("{:.2} MB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.2} KB", n as f64 / KB as f64)
    } else {
        format!("{} B", n)
    }
}
