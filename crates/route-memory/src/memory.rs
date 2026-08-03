use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::ProjectMeta;
use crate::structure::ProjectStructure;

/// 记忆类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryKind {
    ProjectMeta,
    Structure,
    Decision,
    Change,
    Context,
    UserPreference,
}

/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub kind: MemoryKind,
    pub key: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub source: String,
}

/// 记忆统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub by_kind: HashMap<String, usize>,
    pub oldest_entry: Option<String>,
    pub newest_entry: Option<String>,
}

/// 项目记忆系统
#[derive(Debug, Clone)]
pub struct ProjectMemory {
    pub project_path: PathBuf,
    pub entries: Vec<MemoryEntry>,
    pub meta: Option<ProjectMeta>,
    pub structure: Option<ProjectStructure>,
}

impl ProjectMemory {
    /// 获取存储目录路径
    fn memory_dir(&self) -> PathBuf {
        self.project_path.join(".route").join("memory")
    }

    /// 从项目路径加载所有记忆数据
    pub fn load(project_path: &Path) -> Result<Self> {
        let memory_dir = project_path.join(".route").join("memory");

        let mut mem = Self {
            project_path: project_path.to_path_buf(),
            entries: Vec::new(),
            meta: None,
            structure: None,
        };

        // 加载项目元信息
        let meta_path = memory_dir.join("project.json");
        if meta_path.exists() {
            match ProjectMeta::load(&meta_path) {
                Ok(meta) => mem.meta = Some(meta),
                Err(e) => tracing::warn!("Failed to load project meta: {}", e),
            }
        }

        // 加载项目结构
        let structure_path = memory_dir.join("structure.json");
        if structure_path.exists() {
            match Self::load_structure(&structure_path) {
                Ok(structure) => mem.structure = Some(structure),
                Err(e) => tracing::warn!("Failed to load project structure: {}", e),
            }
        }

        // 加载记忆条目
        let entries_path = memory_dir.join("entries.jsonl");
        if entries_path.exists() {
            match Self::load_entries(&entries_path) {
                Ok(entries) => mem.entries = entries,
                Err(e) => tracing::warn!("Failed to load memory entries: {}", e),
            }
        }

        Ok(mem)
    }

    /// 保存所有记忆数据到磁盘
    pub fn save(&self) -> Result<()> {
        let memory_dir = self.memory_dir();
        std::fs::create_dir_all(&memory_dir)
            .with_context(|| format!("Failed to create memory directory {:?}", memory_dir))?;

        // 保存项目元信息
        if let Some(ref meta) = self.meta {
            let meta_path = memory_dir.join("project.json");
            meta.save(&meta_path)?;
        }

        // 保存项目结构
        if let Some(ref structure) = self.structure {
            let structure_path = memory_dir.join("structure.json");
            Self::save_structure(structure, &structure_path)?;
        }

        // 保存记忆条目
        let entries_path = memory_dir.join("entries.jsonl");
        Self::save_entries(&self.entries, &entries_path)?;

        Ok(())
    }

    /// 添加一条记忆条目
    pub fn put(&mut self, entry: MemoryEntry) {
        // 如果 key 已存在，替换旧条目
        if let Some(pos) = self.entries.iter().position(|e| e.key == entry.key && e.kind == entry.kind) {
            self.entries[pos] = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// 根据 key 获取记忆条目
    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// 按类型查询记忆条目
    pub fn query(&self, kind: MemoryKind) -> Vec<&MemoryEntry> {
        self.entries.iter().filter(|e| e.kind == kind).collect()
    }

    /// 搜索记忆条目（在 key、content、tags 中搜索）
    pub fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.key.to_lowercase().contains(&query_lower)
                    || e.content.to_lowercase().contains(&query_lower)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
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

    // --- 私有辅助方法 ---

    fn load_structure(path: &Path) -> Result<ProjectStructure> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read structure {:?}", path))?;
        let structure: ProjectStructure = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse structure {:?}", path))?;
        Ok(structure)
    }

    fn save_structure(structure: &ProjectStructure, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(structure)
            .with_context(|| format!("Failed to serialize structure {:?}", path))?;
        std::fs::write(path, &content)
            .with_context(|| format!("Failed to write structure {:?}", path))?;
        Ok(())
    }

    fn load_entries(path: &Path) -> Result<Vec<MemoryEntry>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read entries {:?}", path))?;
        let mut entries = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<MemoryEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!("Failed to parse memory entry line: {}", e);
                }
            }
        }
        Ok(entries)
    }

    fn save_entries(entries: &[MemoryEntry], path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for entry in entries {
            let line = serde_json::to_string(entry)
                .with_context(|| format!("Failed to serialize memory entry {:?}", entry.key))?;
            content.push_str(&line);
            content.push('\n');
        }
        std::fs::write(path, &content)
            .with_context(|| format!("Failed to write entries {:?}", path))?;
        Ok(())
    }
}