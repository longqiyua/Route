pub mod chain;
pub mod hot_cold;
pub mod memory;
pub mod output;
pub mod project;
pub mod structure;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::chain::{CausalChain, CausalLink, ChainStatus};
use crate::hot_cold::{MemoryBlock, MemoryTier, TieredMemory};
use crate::memory::{MemoryEntry, MemoryStats};
use crate::project::ProjectMeta;
use crate::structure::ProjectStructure;

/// 根记忆结构，整合所有记忆子系统
pub struct RootMemory {
    pub project: ProjectMeta,
    pub structure: Option<ProjectStructure>,
    pub chain: CausalChain,
    pub entries: Vec<MemoryEntry>,
    pub tiered: TieredMemory,
    pub project_path: std::path::PathBuf,
}

impl RootMemory {
    /// 从项目路径创建新的根记忆
    pub fn new(project_path: &Path) -> Result<Self> {
        let project = ProjectMeta::default();
        let chain = CausalChain::load(project_path)?;

        let mut memory = Self {
            project,
            structure: None,
            chain,
            entries: Vec::new(),
            tiered: TieredMemory::new(),
            project_path: project_path.to_path_buf(),
        };

        // 尝试加载
        let _ = memory.load_inner();

        Ok(memory)
    }

    /// 加载所有记忆数据
    pub fn load(&mut self) -> Result<()> {
        self.load_inner()
    }

    fn load_inner(&mut self) -> Result<()> {
        let memory_dir = self.project_path.join(".route").join("memory");

        // 加载项目元信息
        let meta_path = memory_dir.join("project.json");
        if meta_path.exists() {
            match ProjectMeta::load(&meta_path) {
                Ok(meta) => self.project = meta,
                Err(e) => tracing::warn!("Failed to load project meta: {}", e),
            }
        }

        // 加载项目结构
        let structure_path = memory_dir.join("structure.json");
        if structure_path.exists() {
            let content = std::fs::read_to_string(&structure_path)
                .with_context(|| format!("Failed to read structure {:?}", structure_path))?;
            match serde_json::from_str::<ProjectStructure>(&content) {
                Ok(structure) => self.structure = Some(structure),
                Err(e) => tracing::warn!("Failed to parse structure: {}", e),
            }
        }

        // 加载记忆条目
        let entries_path = memory_dir.join("entries.jsonl");
        if entries_path.exists() {
            match self.load_entries(&entries_path) {
                Ok(entries) => {
                    self.entries = entries;
                    // 同步到 tiered memory
                    for entry in &self.entries {
                        let block = MemoryBlock {
                            id: entry.id.clone(),
                            content: entry.content.clone(),
                            tier: MemoryTier::Hot,
                            last_accessed: 0,
                            access_count: 0,
                            estimated_bytes: entry.content.len() + entry.key.len(),
                            kind: format!("{:?}", entry.kind),
                        };
                        self.tiered.add(block);
                    }
                }
                Err(e) => tracing::warn!("Failed to load memory entries: {}", e),
            }
        }

        Ok(())
    }

    /// 保存所有记忆数据到磁盘
    pub fn save(&self) -> Result<()> {
        let memory_dir = self.project_path.join(".route").join("memory");
        std::fs::create_dir_all(&memory_dir)?;

        // 保存项目元信息
        let meta_path = memory_dir.join("project.json");
        self.project.save(&meta_path)?;

        // 保存项目结构
        if let Some(ref structure) = self.structure {
            let structure_path = memory_dir.join("structure.json");
            let content = serde_json::to_string_pretty(structure)?;
            std::fs::write(&structure_path, content)?;
        }

        // 保存记忆条目
        let entries_path = memory_dir.join("entries.jsonl");
        let mut content = String::new();
        for entry in &self.entries {
            let line = serde_json::to_string(entry)?;
            content.push_str(&line);
            content.push('\n');
        }
        std::fs::write(&entries_path, content)?;

        // 因果链自动保存
        self.chain.save()?;

        Ok(())
    }

    /// 添加记忆条目
    pub fn add_entry(&mut self, entry: MemoryEntry) {
        let estimated_bytes = entry.content.len() + entry.key.len() + entry.id.len();
        let block = MemoryBlock {
            id: entry.id.clone(),
            content: entry.content.clone(),
            tier: MemoryTier::Hot,
            last_accessed: 0,
            access_count: 0,
            estimated_bytes,
            kind: format!("{:?}", entry.kind),
        };
        self.tiered.add(block);
        self.entries.push(entry);
    }

    /// 添加因果链接
    pub fn add_causal_link(&mut self, action: &str, file_path: &str, reason: &str, effect: &str) -> Result<()> {
        let link = CausalLink {
            id: format!("cl-{}", chrono::Utc::now().timestamp_millis()),
            parent_id: self.chain.links.last().map(|l| l.id.clone()),
            action: action.to_string(),
            file_path: Some(file_path.to_string()),
            reason: reason.to_string(),
            effect: effect.to_string(),
            side_effects: Vec::new(),
            status: ChainStatus::Executed,
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: "route".to_string(),
        };
        self.chain.add_link(link);
        self.chain.save()?;
        Ok(())
    }

    /// 获取项目结构（Mermaid 格式）
    pub fn structure_mermaid(&self) -> Option<String> {
        self.structure.as_ref().map(|_s| {
            let mut mermaid = String::from("graph TD\n");
            mermaid.push_str(&format!("    P[\"Project: {}\"]\n", self.project.name));
            mermaid.push_str(&format!("    P --> |language| L[\"{}\"]\n", self.project.language));
            if !self.project.key_modules.is_empty() {
                for (i, km) in self.project.key_modules.iter().enumerate() {
                    let node_id = format!("KM{}", i);
                    mermaid.push_str(&format!("    {}[\"{}\"]\n", node_id, km));
                    if i > 0 {
                        mermaid.push_str(&format!("    KM{} --> KM{}\n", i - 1, i));
                    } else {
                        mermaid.push_str("    P --> KM0\n");
                    }
                }
            }
            mermaid
        })
    }

    /// 获取项目元信息（JSON 格式）
    pub fn project_json(&self) -> Option<String> {
        serde_json::to_string_pretty(&self.project).ok()
    }

    /// 获取记忆统计信息
    pub fn stats(&self) -> MemoryStats {
        let mut by_kind: HashMap<String, usize> = HashMap::new();
        for entry in &self.entries {
            let kind_str = format!("{:?}", entry.kind);
            *by_kind.entry(kind_str).or_insert(0) += 1;
        }

        let oldest = self.entries.iter().min_by_key(|e| &e.created_at);
        let newest = self.entries.iter().max_by_key(|e| &e.created_at);

        MemoryStats {
            total_entries: self.entries.len(),
            by_kind,
            oldest_entry: oldest.map(|e| e.created_at.clone()),
            newest_entry: newest.map(|e| e.created_at.clone()),
        }
    }

    /// 重置所有记忆数据
    pub fn reset(&mut self) -> Result<()> {
        self.entries.clear();
        self.structure = None;
        self.chain.links.clear();
        self.tiered = TieredMemory::new();
        self.project = ProjectMeta::default();

        let memory_dir = self.project_path.join(".route").join("memory");
        if memory_dir.exists() {
            std::fs::remove_dir_all(&memory_dir)?;
        }
        Ok(())
    }

    // --- 私有辅助 ---

    fn load_entries(&self, path: &Path) -> Result<Vec<MemoryEntry>> {
        let content = std::fs::read_to_string(path)?;
        let mut entries = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() { continue; }
            if let Ok(entry) = serde_json::from_str::<MemoryEntry>(line) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
}

/// 扩展 MemoryStats 添加 total_chains
#[derive(Debug, Clone, Serialize)]
pub struct RootMemoryStats {
    pub total_entries: usize,
    pub total_chains: usize,
    pub by_kind: HashMap<String, usize>,
    pub oldest_entry: Option<String>,
    pub newest_entry: Option<String>,
}

impl From<&RootMemory> for RootMemoryStats {
    fn from(mem: &RootMemory) -> Self {
        let stats = mem.stats();
        Self {
            total_entries: stats.total_entries,
            total_chains: mem.chain.links.len(),
            by_kind: stats.by_kind,
            oldest_entry: stats.oldest_entry,
            newest_entry: stats.newest_entry,
        }
    }
}