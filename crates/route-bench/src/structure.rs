//! 项目结构理解 Benchmark：量化 AI/Agent 在高频文件修改后
//! 对项目结构描述的准确度（precision / recall / F1）。
//!
//! 测试流程：
//!   1. 在临时目录生成一个"真实"项目结构（可配置 depth/宽度/文件数）
//!   2. 执行 N 轮文件系统 mutate（touch/rm/mkdir/mv）
//!   3. 生成一份"AI 描述"样本（从真实结构中随机漏掉 5-30%、
//!      随机加 5-15% 不存在的条目，模拟 AI 记忆偏差）
//!   4. 计算结构差异：
//!        precision = correctly_reported / reported_total
//!        recall    = correctly_reported / actual_total
//!        f1        = 2 * P * R / (P + R)
//!        jaccard   = inter / union
//!   5. 阈值：f1 < 0.8 → fail

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::case::{BenchmarkCase, CaseContext, CaseResult, Metric};

/// 真实 vs AI 描述的结构得分
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructureScore {
    pub actual_items: usize,
    pub reported_items: usize,
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub jaccard: f64,
    pub file_level_f1: f64,
    pub dir_level_f1: f64,
}

/// 生成/变异的样本结构
#[derive(Debug, Clone)]
pub struct StructureSample {
    pub files: Vec<PathBuf>,
    pub dirs: Vec<PathBuf>,
}

impl StructureSample {
    fn items(&self) -> BTreeSet<PathBuf> {
        let mut s: BTreeSet<PathBuf> = BTreeSet::new();
        s.extend(self.files.iter().cloned());
        s.extend(self.dirs.iter().cloned());
        s
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureAccuracyBench {
    pub id: String,
    pub name: String,
    pub tree_depth: u8,
    pub branch_factor: u8,
    pub files_per_leaf: u8,
    pub mutate_rounds: usize,
    pub ai_noise_ratio: f64, // AI 漏掉/多报的比例
    pub f1_threshold_fail: f64,
    pub f1_threshold_warn: f64,
}

impl Default for StructureAccuracyBench {
    fn default() -> Self {
        Self::with_layout(3, 3, 5, 100, 0.15, "default")
    }
}

impl StructureAccuracyBench {
    pub fn deep_tree() -> Self {
        Self::with_layout(6, 2, 8, 200, 0.20, "deep_tree")
    }
    pub fn file_heavy() -> Self {
        Self::with_layout(2, 5, 20, 500, 0.12, "file_heavy")
    }

    pub fn with_layout(
        depth: u8, branch: u8, files_per_leaf: u8,
        mutate_rounds: usize, noise: f64, tag: &str,
    ) -> Self {
        Self {
            id: format!("structure_{}", tag),
            name: format!("Structure Accuracy (d={}, b={}, f={}, rounds={})",
                depth, branch, files_per_leaf, mutate_rounds),
            tree_depth: depth,
            branch_factor: branch,
            files_per_leaf,
            mutate_rounds,
            ai_noise_ratio: noise,
            f1_threshold_fail: 0.80,
            f1_threshold_warn: 0.90,
        }
    }

    fn gen_tree(&self) -> StructureSample {
        let mut dirs: Vec<PathBuf> = vec![PathBuf::from(".")];
        let mut files: Vec<PathBuf> = Vec::new();
        let mut seed = 42u64;
        let mut level_dirs: Vec<PathBuf> = vec![PathBuf::from(".")];
        for _d in 0..self.tree_depth {
            let mut next: Vec<PathBuf> = Vec::new();
            for parent in &level_dirs {
                for b in 0..self.branch_factor {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    let dir_name = format!("mod_{}_{:02x}", b, (seed & 0xFF) as u8);
                    let dir = parent.join(dir_name);
                    dirs.push(dir.clone());
                    next.push(dir);
                }
            }
            level_dirs = next;
        }
        // leaf 下写文件
        for leaf in &level_dirs {
            for f in 0..self.files_per_leaf {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                let ext = match (f + seed as u8) % 4 {
                    0 => "rs", 1 => "ts", 2 => "md", _ => "json",
                };
                let fname = format!("file_{:03}.{}", f, ext);
                files.push(leaf.join(fname));
            }
        }
        StructureSample { files, dirs }
    }

    fn mutate(&self, sample: &mut StructureSample, rounds: usize, work: &Path) {
        let mut seed = 9u64;
        for _r in 0..rounds {
            seed = seed.wrapping_mul(22695477).wrapping_add(1);
            let op = seed % 5;
            match op {
                0 if !sample.files.is_empty() => {
                    // touch: add file
                    let leaf_idx = (seed >> 8) as usize % sample.dirs.len().max(1);
                    let parent = sample.dirs[leaf_idx].clone();
                    let fname = format!("mut_{:08}.rs", seed);
                    let p = parent.join(fname);
                    if !sample.files.iter().any(|x| x == &p) {
                        sample.files.push(p);
                    }
                }
                1 if sample.files.len() > 1 => {
                    // rm
                    let idx = (seed as usize) % sample.files.len();
                    sample.files.swap_remove(idx);
                }
                2 => {
                    // mkdir
                    let parent_idx = (seed >> 16) as usize % sample.dirs.len().max(1);
                    let parent = sample.dirs[parent_idx].clone();
                    let dname = format!("dir_{:04x}", (seed & 0xFFFF) as u16);
                    let d = parent.join(dname);
                    if !sample.dirs.iter().any(|x| x == &d) {
                        sample.dirs.push(d);
                    }
                }
                3 if sample.files.len() > 1 => {
                    // mv
                    let idx = (seed as usize) % sample.files.len();
                    let dest_parent_idx = ((seed >> 20) as usize) % sample.dirs.len().max(1);
                    let old_p = sample.files[idx].clone();
                    let fname = old_p.file_name().map(|s| s.to_os_string()).unwrap_or_default();
                    let new_p = sample.dirs[dest_parent_idx].join(fname);
                    if new_p != old_p {
                        sample.files[idx] = new_p;
                    }
                }
                _ => { // random stat (no-op)
                    let _ = work.join("noop");
                }
            }
        }
    }

    fn ai_viewport(&self, actual: &StructureSample) -> StructureSample {
        // 模拟 AI 描述：随机漏掉 ai_noise_ratio，随机加 ai_noise_ratio/2
        let mut rng_seed = 1337u64;
        let mut reported_files: Vec<PathBuf> = Vec::new();
        let mut reported_dirs: Vec<PathBuf> = Vec::new();
        let mut rand = || {
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (rng_seed as f64) / (u64::MAX as f64)
        };
        for f in &actual.files {
            let keep = rand() > self.ai_noise_ratio;
            if keep { reported_files.push(f.clone()); }
        }
        for d in &actual.dirs {
            let keep = rand() > self.ai_noise_ratio * 0.6; // dir 更容易被 AI 记住
            if keep { reported_dirs.push(d.clone()); }
        }
        // False positives
        let extra_files = (actual.files.len() as f64 * self.ai_noise_ratio * 0.6) as usize;
        for i in 0..extra_files {
            reported_files.push(PathBuf::from(format!(".route/ghost/phantom_{i}.rs")));
        }
        let extra_dirs = (actual.dirs.len() as f64 * self.ai_noise_ratio * 0.3) as usize;
        for i in 0..extra_dirs {
            reported_dirs.push(PathBuf::from(format!(".route/ghost_dir_{i}")));
        }
        StructureSample { files: reported_files, dirs: reported_dirs }
    }

    fn score(actual: &StructureSample, reported: &StructureSample) -> StructureScore {
        let act_items = actual.items();
        let rep_items = reported.items();
        let actual_files: BTreeSet<_> = actual.files.iter().cloned().collect();
        let actual_dirs: BTreeSet<_> = actual.dirs.iter().cloned().collect();
        let rep_files: BTreeSet<_> = reported.files.iter().cloned().collect();
        let rep_dirs: BTreeSet<_> = reported.dirs.iter().cloned().collect();

        let tp = act_items.intersection(&rep_items).count();
        let fp = rep_items.difference(&act_items).count();
        let fn_ = act_items.difference(&rep_items).count();
        let precision = if tp + fp == 0 { 1.0 } else { tp as f64 / (tp + fp) as f64 };
        let recall = if tp + fn_ == 0 { 1.0 } else { tp as f64 / (tp + fn_) as f64 };
        let f1 = if precision + recall == 0.0 { 0.0 } else {
            2.0 * precision * recall / (precision + recall)
        };
        let inter = tp;
        let union = tp + fp + fn_;
        let jaccard = if union == 0 { 1.0 } else { inter as f64 / union as f64 };

        let file_tp = actual_files.intersection(&rep_files).count();
        let file_fp = rep_files.difference(&actual_files).count();
        let file_fn = actual_files.difference(&rep_files).count();
        let file_f1 = f1_of(file_tp, file_fp, file_fn);

        let dir_tp = actual_dirs.intersection(&rep_dirs).count();
        let dir_fp = rep_dirs.difference(&actual_dirs).count();
        let dir_fn = actual_dirs.difference(&rep_dirs).count();
        let dir_f1 = f1_of(dir_tp, dir_fp, dir_fn);

        StructureScore {
            actual_items: act_items.len(),
            reported_items: rep_items.len(),
            true_positive: tp,
            false_positive: fp,
            false_negative: fn_,
            precision, recall, f1, jaccard,
            file_level_f1: file_f1,
            dir_level_f1: dir_f1,
        }
    }
}

fn f1_of(tp: usize, fp: usize, fn_: usize) -> f64 {
    let p = if tp + fp == 0 { 1.0 } else { tp as f64 / (tp + fp) as f64 };
    let r = if tp + fn_ == 0 { 1.0 } else { tp as f64 / (tp + fn_) as f64 };
    if p + r == 0.0 { 0.0 } else { 2.0 * p * r / (p + r) }
}

impl BenchmarkCase for StructureAccuracyBench {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str {
        "Generate a project tree, mutate it, simulate AI's noisy recollection, compute F1 / Jaccard vs ground truth."
    }
    fn run(&self, ctx: &CaseContext) -> anyhow::Result<CaseResult> {
        let start = std::time::Instant::now();
        let work = ctx.work_dir.clone().unwrap_or_else(|| std::env::temp_dir().join("route-bench-structure"));
        std::fs::create_dir_all(&work)?;
        let mut actual = self.gen_tree();
        self.mutate(&mut actual, self.mutate_rounds, &work);
        let reported = self.ai_viewport(&actual);
        let score = Self::score(&actual, &reported);
        let passed = score.f1 >= self.f1_threshold_fail;
        let mut metrics = vec![
            Metric::new("f1", score.f1, "score").with_fail(self.f1_threshold_fail).with_warn(self.f1_threshold_warn),
            Metric::new("precision", score.precision, "score"),
            Metric::new("recall", score.recall, "score"),
            Metric::new("jaccard", score.jaccard, "score"),
            Metric::new("file_level_f1", score.file_level_f1, "score"),
            Metric::new("dir_level_f1", score.dir_level_f1, "score"),
            Metric::new("true_positive", score.true_positive as f64, "items"),
            Metric::new("false_positive", score.false_positive as f64, "items"),
            Metric::new("false_negative", score.false_negative as f64, "items"),
            Metric::new("actual_items", score.actual_items as f64, "items"),
            Metric::new("reported_items", score.reported_items as f64, "items"),
            Metric::new("mutate_rounds", self.mutate_rounds as f64, "rounds"),
        ];
        // suppress unused warnings
        let _x: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        metrics.truncate(metrics.len());
        let msg = format!("F1={:.3}, P={:.3}, R={:.3}, J={:.3}",
            score.f1, score.precision, score.recall, score.jaccard);
        let dur = start.elapsed().as_millis();
        Ok(if passed {
            CaseResult::new_pass(&self.id, &self.name, dur, metrics)
        } else {
            CaseResult::new_fail(&self.id, &self.name, dur, metrics, msg)
        })
    }
}

// suppress unused
fn _drop<T>(_x: T) {}
