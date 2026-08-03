//! Benchmark 报告：聚合 + JSON/Markdown 渲染。

use serde::{Deserialize, Serialize};
use crate::case::CaseResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Json,
    Markdown,
    Pretty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    /// 通过率 (passed/total * 100)
    pub pass_rate: f64,
    /// 平均 case 耗时 ms
    pub avg_case_duration_ms: f64,
    /// 指标：最差 drift_score（越高越差）
    pub max_drift_score: f64,
    /// 指标：最差 f1_score（越高越好）
    pub min_f1_score: Option<f64>,
    /// 指标：总工具调用轮次成功率
    pub avg_metric_values: std::collections::BTreeMap<String, f64>,
}

impl BenchmarkSummary {
    pub fn compute(results: &[CaseResult]) -> Self {
        let total = results.len().max(1) as f64;
        let passed = results.iter().filter(|r| r.passed()).count() as f64;
        let avg_dur = results.iter().map(|r| r.duration_ms as f64).sum::<f64>() / total;
        let mut max_drift = 0.0f64;
        let mut min_f1: Option<f64> = None;
        let mut sums: std::collections::BTreeMap<String, (f64, usize)> = std::collections::BTreeMap::new();
        for r in results {
            for m in &r.metrics {
                if m.name.eq_ignore_ascii_case("drift_score") && m.value > max_drift {
                    max_drift = m.value;
                }
                if m.name.eq_ignore_ascii_case("f1") || m.name.eq_ignore_ascii_case("f1_score") {
                    min_f1 = Some(min_f1.map(|cur| cur.min(m.value)).unwrap_or(m.value));
                }
                let entry = sums.entry(m.name.clone()).or_insert((0.0, 0));
                entry.0 += m.value;
                entry.1 += 1;
            }
        }
        let avg_metric_values = sums.into_iter()
            .map(|(k, (s, c))| (k, s / c as f64))
            .collect();
        Self {
            pass_rate: passed / total * 100.0,
            avg_case_duration_ms: avg_dur,
            max_drift_score: max_drift,
            min_f1_score: min_f1,
            avg_metric_values,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub suite_id: String,
    pub suite_name: String,
    pub started_at: i64,
    pub duration_ms: u128,
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<CaseResult>,
    pub summary: BenchmarkSummary,
}

pub fn render_report(report: &BenchmarkReport, fmt: Format) -> String {
    match fmt {
        Format::Json => serde_json::to_string_pretty(report).unwrap_or_default(),
        Format::Markdown => render_markdown(report),
        Format::Pretty => render_pretty(report),
    }
}

fn render_pretty(r: &BenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Route Benchmark Report: {} ({})\n", r.suite_name, r.suite_id));
    out.push_str(&format!("Started: {}    Duration: {}ms\n",
        chrono_timestamp(r.started_at), r.duration_ms));
    out.push_str(&format!("Cases: {} passed / {} total    Pass Rate: {:.2}%\n\n",
        r.passed, r.total_cases, r.summary.pass_rate));
    out.push_str("─────────────────────────────────────────────────────\n");
    for case in &r.results {
        let icon = if case.passed() { "✓" } else { "✗" };
        out.push_str(&format!("{} [{:>5}] {:<30}  {:>8}ms  {}\n",
            icon, case.status,
            truncate(&case.case_name, 30),
            case.duration_ms,
            if case.message.is_empty() { String::new() } else { format!("— {}", truncate(&case.message, 60)) }
        ));
        for m in &case.metrics {
            out.push_str(&format!("       · {:<25} = {:.3} {}\n",
                truncate(&m.name, 25), m.value, m.unit));
        }
    }
    out.push_str("─────────────────────────────────────────────────────\n");
    out.push_str(&format!("Summary: max_drift={:.2}  pass_rate={:.2}%\n",
        r.summary.max_drift_score, r.summary.pass_rate));
    out
}

fn render_markdown(r: &BenchmarkReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Route Benchmark: {}\n\n", r.suite_name));
    out.push_str(&format!("**Suite ID**: `{}`  \n", r.suite_id));
    out.push_str(&format!("**Started**: {}  \n", chrono_timestamp(r.started_at)));
    out.push_str(&format!("**Duration**: {}ms  \n", r.duration_ms));
    out.push_str(&format!("**Pass Rate**: **{:.2}%** ({} / {})\n\n",
        r.summary.pass_rate, r.passed, r.total_cases));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Value |\n|---|---|\n");
    out.push_str(&format!("| Pass rate | {:.2}% |\n", r.summary.pass_rate));
    out.push_str(&format!("| Avg case duration | {:.0} ms |\n", r.summary.avg_case_duration_ms));
    out.push_str(&format!("| Max drift score | {:.3} |\n", r.summary.max_drift_score));
    if let Some(f1) = r.summary.min_f1_score {
        out.push_str(&format!("| Min F1 structure | {:.3} |\n", f1));
    }
    out.push_str("\n## Cases\n\n");
    out.push_str("| # | Case | Status | Duration | Message |\n|---|---|---|---|---|\n");
    for (i, c) in r.results.iter().enumerate() {
        out.push_str(&format!("| {} | {} | {} | {}ms | {} |\n",
            i + 1, c.case_name, c.status, c.duration_ms,
            truncate(&c.message.replace('|', "/"), 80)));
    }
    out.push_str("\n## Metrics per Case\n\n");
    for c in &r.results {
        out.push_str(&format!("### {}\n\n", c.case_name));
        out.push_str("| Metric | Value | Unit | Fail threshold |\n|---|---|---|---|\n");
        for m in &c.metrics {
            out.push_str(&format!("| {} | {:.3} | {} | {} |\n",
                m.name, m.value, m.unit,
                m.threshold_fail.map(|t| format!("{:.3}", t)).unwrap_or_else(|| "—".into())));
        }
        out.push('\n');
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() }
    else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

fn chrono_timestamp(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| ms.to_string())
}
