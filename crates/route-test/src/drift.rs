//! 漂移检测 — 检测 AI 在多次操作后的记忆/理解偏差

use serde::{Deserialize, Serialize};

/// 基准条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub key: String,
    pub content: String,
    pub category: String,
}

/// 漂移基准
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftBaseline {
    /// 基准记忆条目
    pub memory_entries: Vec<BaselineEntry>,
    /// 项目结构哈希
    pub structure_hash: String,
    /// 关键事实
    pub key_facts: Vec<String>,
}

impl Default for DriftBaseline {
    fn default() -> Self {
        Self {
            memory_entries: Vec::new(),
            structure_hash: String::new(),
            key_facts: Vec::new(),
        }
    }
}

/// 漂移快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftSnapshot {
    pub memory_entries: Vec<BaselineEntry>,
    pub structure_hash: String,
    /// 回忆准确率 0-1
    pub recall_accuracy: f64,
    /// 幻觉率 0-1
    pub hallucination_rate: f64,
    /// 记忆丢失率 0-1
    pub memory_loss_rate: f64,
}

impl Default for DriftSnapshot {
    fn default() -> Self {
        Self {
            memory_entries: Vec::new(),
            structure_hash: String::new(),
            recall_accuracy: 1.0,
            hallucination_rate: 0.0,
            memory_loss_rate: 0.0,
        }
    }
}

/// 漂移严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DriftSeverity {
    /// 0-20
    Critical,
    /// 20-40
    High,
    /// 40-60
    Medium,
    /// 60-80
    Low,
    /// 80-95
    Minimal,
    /// 95-100
    None,
}

impl DriftSeverity {
    /// 根据分数（0-100）计算严重程度
    pub fn from_score(score: f64) -> Self {
        if score < 20.0 {
            DriftSeverity::Critical
        } else if score < 40.0 {
            DriftSeverity::High
        } else if score < 60.0 {
            DriftSeverity::Medium
        } else if score < 80.0 {
            DriftSeverity::Low
        } else if score < 95.0 {
            DriftSeverity::Minimal
        } else {
            DriftSeverity::None
        }
    }
}

/// 漂移报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    /// 0-100, 100=完美
    pub score: f64,
    /// 记忆丢失率
    pub memory_loss: f64,
    /// 幻觉率
    pub hallucination: f64,
    /// 结构漂移
    pub structure_drift: f64,
    /// 详细信息
    pub details: Vec<String>,
    /// 严重程度
    pub severity: DriftSeverity,
}

/// 漂移检测器
#[derive(Debug, Clone)]
pub struct DriftDetector {
    pub baseline: DriftBaseline,
    pub current: DriftSnapshot,
    /// 默认 0.05 (5%)
    pub threshold: f64,
}

impl DriftDetector {
    pub fn new() -> Self {
        Self {
            baseline: DriftBaseline::default(),
            current: DriftSnapshot::default(),
            threshold: 0.05,
        }
    }

    /// 从 route-memory 构建基准
    #[cfg(feature = "route-memory")]
    pub fn build_baseline(&mut self, memory: &route_memory::memory::ProjectMemory) {
        let mut entries = Vec::new();
        for entry in &memory.entries {
            entries.push(BaselineEntry {
                key: entry.key.clone(),
                content: entry.content.clone(),
                category: format!("{:?}", entry.kind),
            });
        }

        let structure_hash = memory
            .structure
            .as_ref()
            .map(|s| {
                let json = serde_json::to_string(s).unwrap_or_default();
                sha256_hash(&json)
            })
            .unwrap_or_default();

        let key_facts: Vec<String> = memory
            .meta
            .as_ref()
            .map(|m| {
                let mut facts = vec![
                    format!("Project: {}", m.name),
                    format!("Language: {}", m.language),
                ];
                facts.extend(m.key_modules.clone());
                facts
            })
            .unwrap_or_default();

        self.baseline = DriftBaseline {
            memory_entries: entries,
            structure_hash,
            key_facts,
        };
    }

    /// 从 route-memory 拍摄快照
    #[cfg(feature = "route-memory")]
    pub fn snapshot(&mut self, memory: &route_memory::memory::ProjectMemory) {
        let mut entries = Vec::new();
        for entry in &memory.entries {
            entries.push(BaselineEntry {
                key: entry.key.clone(),
                content: entry.content.clone(),
                category: format!("{:?}", entry.kind),
            });
        }

        let structure_hash = memory
            .structure
            .as_ref()
            .map(|s| {
                let json = serde_json::to_string(s).unwrap_or_default();
                sha256_hash(&json)
            })
            .unwrap_or_default();

        if self.baseline.memory_entries.is_empty() {
            // 没有基准时，直接使用当前作为基准
            let recall_accuracy = 1.0;
            let hallucination_rate = 0.0;
            let memory_loss_rate = 0.0;

            self.current = DriftSnapshot {
                memory_entries: entries,
                structure_hash,
                recall_accuracy,
                hallucination_rate,
                memory_loss_rate,
            };
            return;
        }

        // 计算回忆准确率：基准中的条目有多少还在
        let mut found = 0usize;
        let mut hallucinated = 0usize;
        let mut baseline_content_map: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
        for be in &self.baseline.memory_entries {
            baseline_content_map.insert(be.key.as_str(), be.content.as_str());
        }

        for entry in &entries {
            if let Some(&baseline_content) = baseline_content_map.get(entry.key.as_str()) {
                if entry.content == baseline_content {
                    found += 1;
                } else {
                    // 内容变化 — 算作幻觉（内容漂移）
                    hallucinated += 1;
                }
            } else {
                // 新条目不在基准中 — 算作幻觉（新增）
                hallucinated += 1;
            }
        }

        // 检查基准中的条目是否丢失
        let current_keys: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.key.as_str()).collect();
        let mut lost = 0usize;
        for be in &self.baseline.memory_entries {
            if !current_keys.contains(be.key.as_str()) {
                lost += 1;
            }
        }

        let total_baseline = self.baseline.memory_entries.len().max(1);
        let total_current = entries.len().max(1);

        let recall_accuracy = found as f64 / total_baseline as f64;
        let hallucination_rate = hallucinated as f64 / total_current as f64;
        let memory_loss_rate = lost as f64 / total_baseline as f64;

        self.current = DriftSnapshot {
            memory_entries: entries,
            structure_hash,
            recall_accuracy,
            hallucination_rate,
            memory_loss_rate,
        };
    }

    /// 计算漂移报告
    pub fn calculate_drift(&self) -> DriftReport {
        let mut details = Vec::new();

        // 记忆丢失分数
        let memory_loss_score = (1.0 - self.current.memory_loss_rate) * 100.0;
        if self.current.memory_loss_rate > self.threshold {
            details.push(format!(
                "记忆丢失率 {:.1}% 超过阈值 {:.1}%",
                self.current.memory_loss_rate * 100.0,
                self.threshold * 100.0
            ));
        }

        // 幻觉分数
        let hallucination_score = (1.0 - self.current.hallucination_rate) * 100.0;
        if self.current.hallucination_rate > self.threshold {
            details.push(format!(
                "幻觉率 {:.1}% 超过阈值 {:.1}%",
                self.current.hallucination_rate * 100.0,
                self.threshold * 100.0
            ));
        }

        // 结构漂移
        let structure_drift = if self.baseline.structure_hash.is_empty()
            || self.current.structure_hash.is_empty()
        {
            0.0
        } else if self.baseline.structure_hash == self.current.structure_hash {
            0.0
        } else {
            // 哈希不同，计算漂移程度
            1.0
        };
        let structure_score = (1.0 - structure_drift) * 100.0;
        if structure_drift > self.threshold {
            details.push("项目结构发生变化".into());
        }

        // 回忆准确率
        let recall_score = self.current.recall_accuracy * 100.0;
        if self.current.recall_accuracy < (1.0 - self.threshold) {
            details.push(format!(
                "回忆准确率 {:.1}% 低于预期",
                self.current.recall_accuracy * 100.0
            ));
        }

        // 综合评分（加权平均）
        let score = memory_loss_score * 0.35 + hallucination_score * 0.30 + structure_score * 0.15
            + recall_score * 0.20;

        let severity = DriftSeverity::from_score(score);

        DriftReport {
            score: score.clamp(0.0, 100.0),
            memory_loss: self.current.memory_loss_rate,
            hallucination: self.current.hallucination_rate,
            structure_drift,
            details,
            severity,
        }
    }

    /// 是否正在漂移
    pub fn is_drifting(&self) -> bool {
        self.current.memory_loss_rate > self.threshold
            || self.current.hallucination_rate > self.threshold
            || self.current.recall_accuracy < (1.0 - self.threshold)
    }
}

impl Default for DriftDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// 简单的哈希计算
#[cfg(feature = "route-memory")]
fn sha256_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}