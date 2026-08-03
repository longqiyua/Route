//! Route Benchmark — Route Agent 与项目记忆的自定义基准测试。
//!
//! 核心测试维度：
//!   1. **高强度 CRUD 记忆稳定性**（`crud.rs`）：1000+ 次 put/update/delete 后，
//!      关键信息是否丢失？使用 `MemoryDriftReport` 量化。
//!   2. **项目结构理解偏差**（`structure.rs`）：在高频文件修改后，
//!      让 AI 复述项目结构，和真实结构做 diff，计算 recall/precision/F1。
//!   3. **Skill 调用准确度**：给定触发词，Agent 能否正确选择并执行技能？
//!   4. **工具调用成功率**：50+ 工具轮次中，参数错误、权限错误的比例。
//!
//! 使用方式：
//!   - CLI:  `route benchmark run [--suite default] [--report json|markdown]`
//!   - MCP:  `route_benchmark_run` 工具
//!   - GUI:  Settings → Benchmark 面板
//!
//! 设计目标：全部结果**可量化**，便于做回归对比与 CI 门禁。

pub mod suite;
pub mod case;
pub mod report;
pub mod crud;
pub mod structure;

pub use suite::{BenchmarkSuite, BenchmarkRegistry, DEFAULT_SUITE, SUITES};
pub use case::{BenchmarkCase, CaseResult, CaseStatus, CaseContext};
pub use report::{BenchmarkReport, BenchmarkSummary, Format, render_report};
pub use crud::{HighFreqCrudBench, CRUD_OPS_DEFAULT, CRUD_BASELINE_KEYS_DEFAULT};
pub use structure::{StructureAccuracyBench, StructureSample, StructureScore};
