//! Benchmark 套件：注册 + 批量运行。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::case::{BenchmarkCase, CaseContext, CaseResult};
use crate::report::{BenchmarkReport, BenchmarkSummary};

pub const DEFAULT_SUITE: &str = "default";
pub const SUITES: &[&str] = &[DEFAULT_SUITE, "memory", "structure", "full"];

pub struct BenchmarkSuite {
    pub id: String,
    pub name: String,
    pub description: String,
    pub cases: Vec<Box<dyn BenchmarkCase>>,
}

impl BenchmarkSuite {
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), description: description.into(), cases: Vec::new() }
    }

    pub fn add(&mut self, case: Box<dyn BenchmarkCase>) -> &mut Self {
        self.cases.push(case);
        self
    }

    pub fn run_all(&self, ctx: CaseContext) -> BenchmarkReport {
        let start = std::time::Instant::now();
        let mut results: Vec<CaseResult> = Vec::new();
        for case in &self.cases {
            let case_id = case.id().to_string();
            let res = match case.run(&ctx) {
                Ok(r) => r,
                Err(e) => CaseResult::error(&case_id, e.to_string()),
            };
            results.push(res);
        }
        let summary = BenchmarkSummary::compute(&results);
        BenchmarkReport {
            suite_id: self.id.clone(),
            suite_name: self.name.clone(),
            started_at: chrono::Utc::now().timestamp_millis(),
            duration_ms: start.elapsed().as_millis(),
            total_cases: self.cases.len(),
            passed: results.iter().filter(|r| r.passed()).count(),
            failed: results.iter().filter(|r| !r.passed()).count(),
            results,
            summary,
        }
    }
}

pub struct BenchmarkRegistry {
    suites: BTreeMap<String, BenchmarkSuite>,
    temp_root: Option<PathBuf>,
}

impl BenchmarkRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            suites: BTreeMap::new(),
            temp_root: None,
        };
        reg.register_defaults();
        reg
    }

    pub fn with_temp_root(mut self, path: impl Into<PathBuf>) -> Self {
        self.temp_root = Some(path.into());
        self
    }

    pub fn list_suites(&self) -> Vec<(&str, &str, usize)> {
        self.suites.values().map(|s| (s.id.as_str(), s.name.as_str(), s.cases.len())).collect()
    }

    pub fn list_cases(&self, suite_id: &str) -> Option<Vec<(&str, &str)>> {
        self.suites.get(suite_id)
            .map(|s| s.cases.iter().map(|c| (c.id(), c.name())).collect())
    }

    pub fn run(&self, suite_id: &str) -> Option<BenchmarkReport> {
        let suite = self.suites.get(suite_id)?;
        let temp = tempfile::TempDir::new().ok()
            .map(|t| { let p = t.path().to_path_buf(); drop(t); p });
        let work = self.temp_root.clone().or(temp);
        let ctx = CaseContext {
            work_dir: work.clone(),
            project_path: None,
            output_dir: work.clone(),
        };
        Some(suite.run_all(ctx))
    }

    fn register_defaults(&mut self) {
        // default suite: 2 cases
        let mut default = BenchmarkSuite::new(
            "default", "Default Benchmark Suite",
            "Fast checks for memory drift and structure accuracy (2 cases, ~10s)."
        );
        default.add(Box::new(crate::crud::HighFreqCrudBench::default()));
        default.add(Box::new(crate::structure::StructureAccuracyBench::default()));
        self.suites.insert("default".into(), default);

        // memory suite: CRUD variants
        let mut memory = BenchmarkSuite::new(
            "memory", "Memory Stability Suite",
            "High-intensity memory CRUD with varying scales (3 cases, ~30s)."
        );
        memory.add(Box::new(crate::crud::HighFreqCrudBench::small()));
        memory.add(Box::new(crate::crud::HighFreqCrudBench::default()));
        memory.add(Box::new(crate::crud::HighFreqCrudBench::large()));
        self.suites.insert("memory".into(), memory);

        // structure suite
        let mut structure = BenchmarkSuite::new(
            "structure", "Structure Understanding Suite",
            "Project structure accuracy with varying layouts (2 cases, ~15s)."
        );
        structure.add(Box::new(crate::structure::StructureAccuracyBench::default()));
        structure.add(Box::new(crate::structure::StructureAccuracyBench::deep_tree()));
        self.suites.insert("structure".into(), structure);

        // full suite
        let mut full = BenchmarkSuite::new(
            "full", "Full Benchmark Suite",
            "Run all benchmarks (7 cases, ~60s). Used for release gates."
        );
        full.add(Box::new(crate::crud::HighFreqCrudBench::small()));
        full.add(Box::new(crate::crud::HighFreqCrudBench::default()));
        full.add(Box::new(crate::crud::HighFreqCrudBench::large()));
        full.add(Box::new(crate::structure::StructureAccuracyBench::default()));
        full.add(Box::new(crate::structure::StructureAccuracyBench::deep_tree()));
        full.add(Box::new(crate::structure::StructureAccuracyBench::file_heavy()));
        full.add(Box::new(crate::crud::HighFreqCrudBench::with_custom(5000, 200)));
        self.suites.insert("full".into(), full);
    }
}

impl Default for BenchmarkRegistry { fn default() -> Self { Self::new() } }

// TempDir compatibility: define empty if tempfile not available at runtime
struct _Unused(Option<PathBuf>);
#[allow(dead_code)]
impl _Unused { fn _dummy() { let _: Option<&Path> = None; } }
