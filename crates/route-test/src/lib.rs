//! Route Test — 内置 Benchmark 系统
//!
//! 专门检测 AI 漂移、记忆丢失、结构理解偏差。
//! 所有依赖（route-engine, route-memory, route-vm）均为 optional。

pub mod drift;
pub mod benchmark;
pub mod report;

pub use report::TestReport;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// 测试用例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub description: String,
    pub expected: Option<String>,
    pub weight: f64,
}

/// 测试类别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestCategory {
    /// 记忆漂移
    MemoryDrift,
    /// 结构理解偏差
    StructureDrift,
    /// 因果控制
    CausalControl,
    /// 技能执行
    SkillExecution,
    /// CRUD 稳定性
    CrdStability,
    /// 全量
    Full,
}

/// 测试套件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuite {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cases: Vec<TestCase>,
    pub category: TestCategory,
}

/// 测试配置
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub name: String,
    pub description: String,
    pub iterations: usize,
    pub verbose: bool,
    pub work_dir: Option<PathBuf>,
    pub memory_mode: bool,
    pub causal_check: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            name: "Route Test".into(),
            description: String::new(),
            iterations: 100,
            verbose: false,
            work_dir: None,
            memory_mode: false,
            causal_check: false,
        }
    }
}

/// Route Test 核心入口
#[derive(Debug, Clone)]
pub struct RouteTest {
    pub suites: Vec<TestSuite>,
    pub results: Vec<report::TestResult>,
    pub config: TestConfig,
}

impl RouteTest {
    pub fn new(config: TestConfig) -> Self {
        Self {
            suites: benchmark::default_suites(),
            results: Vec::new(),
            config,
        }
    }

    /// 运行所有测试套件
    pub fn run_all(&mut self) -> Vec<report::TestResult> {
        let mut runner = benchmark::BenchmarkRunner::new(self.config.clone());
        let results = runner.run_full();
        self.results = results.clone();
        results
    }

    /// 根据 ID 运行指定测试套件
    pub fn run_suite(&mut self, suite_id: &str) -> Option<report::TestResult> {
        let suite = self.suites.iter().find(|s| s.id == suite_id)?;
        let mut runner = benchmark::BenchmarkRunner::new(self.config.clone());
        let result = match suite.category {
            TestCategory::MemoryDrift => runner.run_memory_drift(),
            TestCategory::StructureDrift => runner.run_structure_drift(),
            TestCategory::CausalControl => runner.run_causal_control(),
            TestCategory::SkillExecution => runner.run_skill_execution(),
            TestCategory::CrdStability => runner.run_crud_stability(),
            TestCategory::Full => {
                let results = runner.run_full();
                self.results.extend(results.clone());
                return results.last().cloned();
            }
        };
        self.results.push(result.clone());
        Some(result)
    }
}

/// 预设默认测试套件
pub fn default_suites() -> Vec<TestSuite> {
    benchmark::default_suites()
}