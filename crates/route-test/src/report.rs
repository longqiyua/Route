//! 报告生成 — 测试结果报告

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub suite_id: String,
    pub passed: bool,
    pub score: f64,
    pub metrics: HashMap<String, f64>,
    pub details: Vec<String>,
    pub duration_ms: u128,
    pub recommendations: Vec<String>,
}

/// 测试报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    pub name: String,
    pub timestamp: String,
    pub total_suites: usize,
    pub passed: usize,
    pub failed: usize,
    pub overall_score: f64,
    pub results: Vec<TestResult>,
    pub recommendations: Vec<String>,
}

impl TestReport {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_suites: 0,
            passed: 0,
            failed: 0,
            overall_score: 0.0,
            results: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: TestResult) {
        let passed = result.passed;
        let score = result.score;

        self.results.push(result);
        self.total_suites += 1;

        if passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }

        // 更新总体分数（移动平均）
        let total = self.total_suites as f64;
        self.overall_score =
            (self.overall_score * (total - 1.0) + score) / total;
    }

    pub fn finalize(&mut self) {
        // 收集所有推荐建议
        let mut all_recommendations: Vec<String> = Vec::new();
        for result in &self.results {
            for rec in &result.recommendations {
                if !all_recommendations.contains(rec) {
                    all_recommendations.push(rec.clone());
                }
            }
        }
        self.recommendations = all_recommendations;
    }

    /// 导出为 JSON 字符串
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// 导出为格式化文本
    pub fn to_pretty(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("╔══════════════════════════════════════╗\n"));
        output.push_str(&format!("║         Route Test Report           ║\n"));
        output.push_str(&format!("╚══════════════════════════════════════╝\n"));
        output.push_str(&format!("\n"));
        output.push_str(&format!("Name:     {}\n", self.name));
        output.push_str(&format!("Time:     {}\n", self.timestamp));
        output.push_str(&format!("Overall:  {:.1}/100\n", self.overall_score));
        output.push_str(&format!("Passed:   {}/{}\n", self.passed, self.total_suites));
        output.push_str(&format!("Failed:   {}/{}\n", self.failed, self.total_suites));
        output.push_str(&format!("\n"));

        for result in &self.results {
            let icon = if result.passed { "✅" } else { "❌" };
            output.push_str(&format!(
                "{} [{}] Score: {:.1}/100 ({}ms)\n",
                icon, result.suite_id, result.score, result.duration_ms
            ));
            for detail in &result.details {
                output.push_str(&format!("   • {}\n", detail));
            }
            if !result.recommendations.is_empty() {
                output.push_str("   建议:\n");
                for rec in &result.recommendations {
                    output.push_str(&format!("     - {}\n", rec));
                }
            }
            output.push_str("\n");
        }

        if !self.recommendations.is_empty() {
            output.push_str(&format!("📋 综合建议:\n"));
            for rec in &self.recommendations {
                output.push_str(&format!("  • {}\n", rec));
            }
        }

        output
    }

    /// 生成 Markdown 报告
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str(&format!("# Route Test Report\n\n"));
        md.push_str(&format!("**Name:** {}  \n", self.name));
        md.push_str(&format!("**Time:** {}  \n", self.timestamp));
        md.push_str(&format!("**Overall Score:** {:.1}/100  \n", self.overall_score));
        md.push_str(&format!("**Passed:** {}/{}  |  **Failed:** {}/{}  \n\n", self.passed, self.total_suites, self.failed, self.total_suites));

        md.push_str("## Summary\n\n");
        md.push_str("| Suite | Status | Score | Duration |\n");
        md.push_str("|-------|--------|-------|----------|\n");

        for result in &self.results {
            let status = if result.passed { "✅ Pass" } else { "❌ Fail" };
            md.push_str(&format!(
                "| {} | {} | {:.1}/100 | {}ms |\n",
                result.suite_id, status, result.score, result.duration_ms
            ));
        }

        md.push_str("\n## Details\n\n");

        for result in &self.results {
            let icon = if result.passed { "✅" } else { "❌" };
            md.push_str(&format!("### {} {}\n\n", icon, result.suite_id));

            if !result.details.is_empty() {
                md.push_str("**Details:**\n");
                for detail in &result.details {
                    md.push_str(&format!("- {}\n", detail));
                }
                md.push_str("\n");
            }

            if !result.metrics.is_empty() {
                md.push_str("**Metrics:**\n\n");
                md.push_str("| Metric | Value |\n");
                md.push_str("|--------|-------|\n");
                let mut sorted_metrics: Vec<_> = result.metrics.iter().collect();
                sorted_metrics.sort_by_key(|(k, _)| *k);
                for (key, value) in &sorted_metrics {
                    md.push_str(&format!("| {} | {:.2} |\n", key, value));
                }
                md.push_str("\n");
            }

            if !result.recommendations.is_empty() {
                md.push_str("**Recommendations:**\n");
                for rec in &result.recommendations {
                    md.push_str(&format!("- 💡 {}\n", rec));
                }
                md.push_str("\n");
            }
        }

        if !self.recommendations.is_empty() {
            md.push_str("## Overall Recommendations\n\n");
            for rec in &self.recommendations {
                md.push_str(&format!("- 💡 {}\n", rec));
            }
            md.push_str("\n");
        }

        md.push_str("---\n");
        md.push_str(&format!("*Generated by Route Test at {}*\n", self.timestamp));

        md
    }

    /// 生成 Mermaid 图表
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph TD\n");
        mermaid.push_str(&format!("    TITLE[\"Route Test: {}\"]\n", self.name));
        mermaid.push_str(&format!(
            "    TITLE --> OVERALL[\"Overall: {:.1}/100\"]\n",
            self.overall_score
        ));
        mermaid.push_str(&format!(
            "    OVERALL --> PASSED[\"Passed: {}/{} ✓\"]\n",
            self.passed, self.total_suites
        ));
        mermaid.push_str(&format!(
            "    OVERALL --> FAILED[\"Failed: {}/{} ✗\"]\n",
            self.failed, self.total_suites
        ));

        for result in &self.results {
            let node_id = format!("SUITE_{}", result.suite_id.replace('-', "_"));
            let status_icon = if result.passed { "✓" } else { "✗" };
            mermaid.push_str(&format!(
                "    OVERALL --> {node_id}[\"{suite}: {score:.1}% {status_icon}\"]\n",
                node_id = node_id,
                suite = result.suite_id,
                score = result.score,
                status_icon = status_icon
            ));
        }

        mermaid
    }
}

impl Default for TestReport {
    fn default() -> Self {
        Self::new()
    }
}