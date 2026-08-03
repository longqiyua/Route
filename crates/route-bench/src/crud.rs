//! 高强度 CRUD Benchmark：检测项目记忆在高频操作下的漂移率。
//!
//! 测试流程：
//!   1. 写入 N 条 Project 记忆（snapshot_baseline）
//!   2. 执行 M 轮随机 put / get / update / delete 混合操作
//!   3. 重新加载记忆（模拟跨进程/重启）
//!   4. 调用 compare_to_baseline → drift_score
//!   5. 阈值：drift_score > 5 → fail

use route_agent::memory::{AgentMemory, MemoryKind, MemoryDriftReport};
use serde::{Deserialize, Serialize};

use crate::case::{BenchmarkCase, CaseContext, CaseResult, CaseStatus, Metric};

pub const CRUD_OPS_DEFAULT: usize = 1000;
pub const CRUD_BASELINE_KEYS_DEFAULT: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighFreqCrudBench {
    pub id: String,
    pub name: String,
    pub baseline_keys: usize,
    pub operations: usize,
    pub drift_threshold_fail: f64,
    pub drift_threshold_warn: f64,
}

impl Default for HighFreqCrudBench {
    fn default() -> Self {
        Self::with_custom(CRUD_OPS_DEFAULT, CRUD_BASELINE_KEYS_DEFAULT)
    }
}

impl HighFreqCrudBench {
    pub fn small() -> Self { Self::with_custom(100, 20) }
    pub fn large() -> Self { Self::with_custom(5000, 500) }
    pub fn file_heavy() -> Self { Self::with_custom(2000, 200) }

    pub fn with_custom(operations: usize, baseline_keys: usize) -> Self {
        let scale_tag = match (operations, baseline_keys) {
            (100, 20) => "small",
            (1000, 100) => "default",
            (5000, 500) => "large",
            _ => "custom",
        };
        Self {
            id: format!("hf_crud_{}", scale_tag),
            name: format!("HighFreq CRUD ({} keys, {} ops)", baseline_keys, operations),
            baseline_keys,
            operations,
            drift_threshold_fail: 5.0,
            drift_threshold_warn: 1.0,
        }
    }

    fn gen_key(i: usize) -> String { format!("bench:key:{:0>6}", i) }
    fn gen_value(seed: usize, extra: usize) -> String {
        // 生成一段可验证、非平凡的 value（含数字/字符混合）
        let base = seed * 9301 + 49297 + extra * 233;
        let mut s = String::with_capacity(64);
        s.push_str(&format!("val-{:0>8x}-", base));
        for ch in 0..16 {
            let c = ((base >> ch) & 0xFF) as u8;
            s.push(b"abcdefghijklmnopqrstuvwxyz0123456789"[(c % 36) as usize] as char);
        }
        s
    }

    fn run_internal(&self, ctx: &CaseContext) -> CaseResult {
        let start = std::time::Instant::now();
        let work = match ctx.work_dir.as_ref() {
            Some(p) => p.clone(),
            None => {
                return CaseResult::error(&self.id, "work_dir not set: cannot run CRUD benchmark");
            }
        };

        // Phase 1: 写入 baseline
        let mut mem = AgentMemory::new();
        mem.project_path = Some(work.clone());
        for i in 0..self.baseline_keys {
            let k = Self::gen_key(i);
            let v = Self::gen_value(i, 0);
            mem.put(MemoryKind::Project, k, v);
        }
        mem.snapshot_baseline();
        if let Err(e) = mem.save_project() {
            return CaseResult::error(&self.id, format!("save baseline: {e}"));
        }

        // Phase 2: M 轮混合 CRUD (put/update/delete 比例 3:5:2)
        let mut seed = 7u64;
        for i in 0..self.operations {
            let mut rand_u = || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); seed };
            let op = rand_u() % 10;
            let key_idx = (rand_u() as usize) % self.baseline_keys.max(1);
            match op {
                0..=2 => { // put
                    let k = format!("bench:extra:{:0>6}", rand_u() as usize % (self.baseline_keys / 2 + 1));
                    let v = Self::gen_value(i, 1);
                    mem.put(MemoryKind::Project, k, v);
                }
                3..=7 => { // update
                    let k = Self::gen_key(key_idx);
                    let v = Self::gen_value(key_idx, (rand_u() % 5) as usize + 1);
                    mem.put(MemoryKind::Project, k, v);
                }
                _ => { // delete
                    let k = format!("bench:volatile:{:0>6}", (rand_u() as usize) % (self.baseline_keys / 5 + 1));
                    mem.delete(&k);
                }
            }
        }

        // Phase 3: 重启记忆（模拟跨进程）
        drop(mem);
        let mut mem2 = match AgentMemory::load_project(&work) {
            Ok(m) => m,
            Err(e) => return CaseResult::error(&self.id, format!("reload memory: {e}")),
        };
        mem2.baseline = AgentMemory::new().baseline; // 先清空
        let baseline_snapshot: std::collections::BTreeMap<String, (String, MemoryKind)> =
            (0..self.baseline_keys).map(|i| {
                let k = Self::gen_key(i);
                let v = Self::gen_value(i, 0);
                let hash = hash_str(&v);
                (k, (hash, MemoryKind::Project))
            }).collect();
        mem2.baseline = Some(baseline_snapshot);

        let drift: MemoryDriftReport = mem2.compare_to_baseline();
        let drift_score = drift.drift_score;
        let passed = drift_score <= self.drift_threshold_fail;
        let mut metrics = vec![
            Metric::new("drift_score", drift_score, "%").with_fail(self.drift_threshold_fail).with_warn(self.drift_threshold_warn),
            Metric::new("baseline_count", drift.baseline_count as f64, "items"),
            Metric::new("preserved", drift.preserved as f64, "items"),
            Metric::new("modified", drift.modified as f64, "items"),
            Metric::new("lost", drift.lost as f64, "items"),
            Metric::new("unexpected", drift.unexpected as f64, "items"),
            Metric::new("operations", self.operations as f64, "ops"),
        ];
        metrics.push(Metric::new("severity_code", severity_to_num(&drift.severity), "code"));
        let msg = format!("drift={:.2}% ({:?}), {} preserved of {}",
            drift.drift_score, drift.severity, drift.preserved, drift.baseline_count);

        let dur = start.elapsed().as_millis();
        if passed {
            CaseResult::new_pass(&self.id, &self.name, dur, metrics)
        } else {
            CaseResult::new_fail(&self.id, &self.name, dur, metrics, msg)
        }
    }
}

impl BenchmarkCase for HighFreqCrudBench {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str {
        "Write baseline memory, then run thousands of random CRUD operations, reload memory from disk and compare drift vs baseline."
    }
    fn run(&self, ctx: &CaseContext) -> anyhow::Result<CaseResult> { Ok(self.run_internal(ctx)) }
}

fn hash_str(s: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn severity_to_num(s: &route_agent::memory::DriftSeverity) -> f64 {
    match s {
        route_agent::memory::DriftSeverity::Perfect => 0.0,
        route_agent::memory::DriftSeverity::Negligible => 1.0,
        route_agent::memory::DriftSeverity::Minor => 2.0,
        route_agent::memory::DriftSeverity::Moderate => 3.0,
        route_agent::memory::DriftSeverity::Severe => 4.0,
        route_agent::memory::DriftSeverity::Critical => 5.0,
    }
}

#[allow(dead_code)]
fn _unused(_cs: CaseStatus) { let _x: Option<&CaseContext> = None; }
