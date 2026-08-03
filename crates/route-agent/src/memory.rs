//! Agent 记忆系统：短期对话记忆 + 长期项目记忆 + 量化漂移检测。
//!
//! 设计目标：
//!   - 记忆不丢失：高强度 CRUD 后仍能还原关键信息
//!   - 可量化：MemoryDriftReport 给出漂移分数（0 = 完美，100 = 完全丢失）

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// 短期对话（随会话消失）
    Ephemeral,
    /// 长期项目记忆（持久化到 .route/agent-memory.jsonl）
    Project,
    /// 全局用户偏好
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub kind: MemoryKind,
    pub key: String,
    pub value: String,
    pub created_at: i64,
    pub accessed_at: i64,
    pub access_count: u64,
    /// 内容哈希（SHA-256），用于漂移检测
    pub content_hash: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub project_entries: usize,
    pub ephemeral_entries: usize,
    pub user_entries: usize,
    pub total_accesses: u64,
    pub oldest_entry_ts: Option<i64>,
    pub newest_entry_ts: Option<i64>,
}

/// 记忆漂移检测报告。量化：AI 在高频操作下项目记忆是否丢失。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDriftReport {
    /// 基准检查点时写入的条目数
    pub baseline_count: usize,
    /// 检测时实际存在的条目数
    pub current_count: usize,
    /// 正确保留（key+value 完全一致，hash 匹配）的条目数
    pub preserved: usize,
    /// 修改过的条目数（hash 不匹配但 key 存在）
    pub modified: usize,
    /// 丢失的条目数
    pub lost: usize,
    /// 意外新增的条目数
    pub unexpected: usize,
    /// 综合漂移分数：0 = 完美，100 = 灾难性丢失
    /// drift_score = (lost * 2 + modified * 1) / baseline_count * 100
    pub drift_score: f64,
    /// 漂移严重等级
    pub severity: DriftSeverity,
    pub details: Vec<DriftDetail>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    Perfect,   // score == 0
    Negligible, // 0 < score <= 2
    Minor,      // 2 < score <= 10
    Moderate,   // 10 < score <= 25
    Severe,     // 25 < score <= 50
    Critical,   // score > 50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftDetail {
    pub key: String,
    pub kind: DriftDetailKind,
    pub expected_hash: Option<String>,
    pub actual_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftDetailKind {
    Preserved,
    Modified,
    Lost,
    Unexpected,
}

/// Agent 记忆：短期 + 长期持久化 + 漂移检测
pub struct AgentMemory {
    pub entries: std::collections::BTreeMap<String, MemoryEntry>,
    /// 基准检查点（snapshot），用于漂移检测
    pub baseline: Option<std::collections::BTreeMap<String, (String, MemoryKind)>>,
    pub project_path: Option<PathBuf>,
}

impl AgentMemory {
    pub fn new() -> Self {
        Self {
            entries: Default::default(),
            baseline: None,
            project_path: None,
        }
    }

    /// 从项目目录加载持久化记忆
    pub fn load_project(project_path: &Path) -> anyhow::Result<Self> {
        let path = project_path.join(".route").join("agent-memory.jsonl");
        let mut mem = Self {
            entries: Default::default(),
            baseline: None,
            project_path: Some(project_path.to_path_buf()),
        };
        if path.exists() {
            for line in std::fs::read_to_string(&path)?.lines() {
                if line.trim().is_empty() { continue; }
                if let Ok(entry) = serde_json::from_str::<MemoryEntry>(line) {
                    mem.entries.insert(entry.key.clone(), entry);
                }
            }
        }
        Ok(mem)
    }

    pub fn save_project(&self) -> anyhow::Result<()> {
        let Some(project) = &self.project_path else { return Ok(()); };
        let dir = project.join(".route");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("agent-memory.jsonl");
        let mut out = String::new();
        for entry in self.entries.values() {
            if matches!(entry.kind, MemoryKind::Project | MemoryKind::User) {
                out.push_str(&serde_json::to_string(entry)?);
                out.push('\n');
            }
        }
        std::fs::write(&path, out)?;
        Ok(())
    }

    pub fn put(&mut self, kind: MemoryKind, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        let now = chrono::Utc::now().timestamp();
        let content_hash = hash_content(&value);
        let id = format!("{:08x}", id_hash(&key));
        let entry = MemoryEntry {
            id,
            kind,
            key: key.clone(),
            value,
            created_at: now,
            accessed_at: now,
            access_count: 0,
            content_hash,
        };
        self.entries.insert(key, entry);
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        let entry = self.entries.get_mut(key)?;
        entry.access_count += 1;
        entry.accessed_at = chrono::Utc::now().timestamp();
        Some(entry.value.as_str())
    }

    pub fn delete(&mut self, key: &str) -> bool { self.entries.remove(key).is_some() }

    pub fn stats(&self) -> MemoryStats {
        let mut s = MemoryStats::default();
        s.total_entries = self.entries.len();
        let mut oldest: Option<i64> = None;
        let mut newest: Option<i64> = None;
        for e in self.entries.values() {
            s.total_accesses += e.access_count;
            match e.kind {
                MemoryKind::Project => s.project_entries += 1,
                MemoryKind::Ephemeral => s.ephemeral_entries += 1,
                MemoryKind::User => s.user_entries += 1,
            }
            oldest = Some(oldest.map(|o| o.min(e.created_at)).unwrap_or(e.created_at));
            newest = Some(newest.map(|n| n.max(e.created_at)).unwrap_or(e.created_at));
        }
        s.oldest_entry_ts = oldest;
        s.newest_entry_ts = newest;
        s
    }

    /// 建立基准检查点（写入后调用，随后 benchmark 做 CRUD 再 compare）
    pub fn snapshot_baseline(&mut self) {
        self.baseline = Some(self.entries.iter().map(|(k, e)| {
            (k.clone(), (e.content_hash.clone(), e.kind))
        }).collect());
    }

    /// 对比基准，计算记忆漂移报告（量化高强度 CRUD 后记忆丢失）
    pub fn compare_to_baseline(&self) -> MemoryDriftReport {
        let start = std::time::Instant::now();
        let baseline = match &self.baseline {
            Some(b) => b,
            None => return MemoryDriftReport {
                baseline_count: 0, current_count: self.entries.len(),
                preserved: 0, modified: 0, lost: 0, unexpected: 0,
                drift_score: 0.0, severity: DriftSeverity::Perfect,
                details: Vec::new(),
                duration_ms: start.elapsed().as_millis(),
            },
        };
        let baseline_count = baseline.len();
        let mut preserved = 0usize;
        let mut modified = 0usize;
        let mut lost = 0usize;
        let mut unexpected = 0usize;
        let mut details = Vec::new();

        for (key, (expected_hash, _kind)) in baseline {
            match self.entries.get(key) {
                Some(actual) => {
                    if actual.content_hash == *expected_hash {
                        preserved += 1;
                        details.push(DriftDetail {
                            key: key.clone(),
                            kind: DriftDetailKind::Preserved,
                            expected_hash: Some(expected_hash.clone()),
                            actual_hash: Some(actual.content_hash.clone()),
                        });
                    } else {
                        modified += 1;
                        details.push(DriftDetail {
                            key: key.clone(),
                            kind: DriftDetailKind::Modified,
                            expected_hash: Some(expected_hash.clone()),
                            actual_hash: Some(actual.content_hash.clone()),
                        });
                    }
                }
                None => {
                    lost += 1;
                    details.push(DriftDetail {
                        key: key.clone(),
                        kind: DriftDetailKind::Lost,
                        expected_hash: Some(expected_hash.clone()),
                        actual_hash: None,
                    });
                }
            }
        }
        for key in self.entries.keys() {
            if !baseline.contains_key(key) {
                unexpected += 1;
                details.push(DriftDetail {
                    key: key.clone(),
                    kind: DriftDetailKind::Unexpected,
                    expected_hash: None,
                    actual_hash: self.entries.get(key).map(|e| e.content_hash.clone()),
                });
            }
        }
        let drift_score = if baseline_count == 0 { 0.0 } else {
            ((lost * 2 + modified) as f64 / baseline_count as f64) * 100.0
        };
        let severity = match drift_score {
            s if s == 0.0 => DriftSeverity::Perfect,
            s if s <= 2.0 => DriftSeverity::Negligible,
            s if s <= 10.0 => DriftSeverity::Minor,
            s if s <= 25.0 => DriftSeverity::Moderate,
            s if s <= 50.0 => DriftSeverity::Severe,
            _ => DriftSeverity::Critical,
        };
        MemoryDriftReport {
            baseline_count,
            current_count: self.entries.len(),
            preserved, modified, lost, unexpected,
            drift_score, severity, details,
            duration_ms: start.elapsed().as_millis(),
        }
    }
}

fn hash_content(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

fn id_hash(s: &str) -> u32 {
    std::num::Wrapping(s.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))).0
}

impl Default for AgentMemory { fn default() -> Self { Self::new() } }
