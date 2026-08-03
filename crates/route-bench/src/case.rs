//! BenchmarkCase trait + CaseResult/CaseContext。

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

pub type CaseStatus = &'static str;
pub const CASE_PASS: CaseStatus = "pass";
pub const CASE_FAIL: CaseStatus = "fail";
pub const CASE_SKIP: CaseStatus = "skip";
pub const CASE_ERROR: CaseStatus = "error";

#[derive(Debug, Clone)]
pub struct CaseContext {
    /// 可写的临时工作目录
    pub work_dir: Option<PathBuf>,
    /// 可选：实际项目路径
    pub project_path: Option<PathBuf>,
    /// 输出目录（报告、工件）
    pub output_dir: Option<PathBuf>,
}

/// 单个 Benchmark 指标（key-value，带单位 + 阈值）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub unit: String,
    /// 阈值：如果超过 threshold_fail → fail
    #[serde(default)]
    pub threshold_fail: Option<f64>,
    /// 阈值：如果超过 threshold_warn → warn
    #[serde(default)]
    pub threshold_warn: Option<f64>,
}

impl Metric {
    pub fn new(name: impl Into<String>, value: f64, unit: impl Into<String>) -> Self {
        Self { name: name.into(), value, unit: unit.into(), threshold_fail: None, threshold_warn: None }
    }
    pub fn with_fail(mut self, t: f64) -> Self { self.threshold_fail = Some(t); self }
    pub fn with_warn(mut self, t: f64) -> Self { self.threshold_warn = Some(t); self }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub case_name: String,
    pub status: String,
    pub duration_ms: u128,
    pub metrics: Vec<Metric>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub details: Vec<String>,
}

impl CaseResult {
    pub fn new_pass(id: &str, name: &str, duration_ms: u128, metrics: Vec<Metric>) -> Self {
        Self {
            case_id: id.to_string(), case_name: name.to_string(),
            status: CASE_PASS.to_string(), duration_ms, metrics,
            message: String::new(), details: Vec::new(),
        }
    }
    pub fn new_fail(id: &str, name: &str, duration_ms: u128, metrics: Vec<Metric>, msg: impl Into<String>) -> Self {
        Self {
            case_id: id.to_string(), case_name: name.to_string(),
            status: CASE_FAIL.to_string(), duration_ms, metrics,
            message: msg.into(), details: Vec::new(),
        }
    }
    pub fn error(id: &str, msg: impl Into<String>) -> Self {
        Self {
            case_id: id.to_string(), case_name: id.to_string(),
            status: CASE_ERROR.to_string(), duration_ms: 0,
            metrics: Vec::new(), message: msg.into(), details: Vec::new(),
        }
    }
    pub fn passed(&self) -> bool { self.status == CASE_PASS }
}

/// Benchmark Case trait。实现者定义：id/name + 运行逻辑。
pub trait BenchmarkCase: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str { "" }
    /// 运行此 case。返回 CaseResult 或运行时错误。
    fn run(&self, ctx: &CaseContext) -> anyhow::Result<CaseResult>;
    /// 所需最小资源（比如需要多少 RAM），默认无限制
    fn resource_tier(&self) -> u8 { 1 }
}
