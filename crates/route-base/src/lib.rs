//! Root Base — 顶层编排器
//!
//! 三层架构：Root Base（编排器）→ Root Engine（驱动层）→ Root Memory（存储层）
//!
//! Root Base 是系统的唯一入口，负责：
//! - 初始化 Root Engine 和 Root Memory
//! - 提供统一 API（CLI / MCP）
//! - 管理生命周期（启动/优雅关闭/重置）
//! - 服务注册与发现（支持后续扩展）
//!
//! # 自反性 (Self-Referential)
//!
//! Route 软件本身使用自己的架构管理自身代码：
//! - Root Engine 解析 Route 自身代码（Tree-sitter + Code Graph + GraphRAG）
//! - Root Memory 记忆 Route 自身项目（project.json + structure.mermaid）
//! - Root Base 用自身架构管理自身版本

pub mod registry;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub use registry::ServiceRegistry;

// ─── 配置 ─────────────────────────────────────────────────────────────────

/// Root Base 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    /// 项目路径
    pub project_path: PathBuf,
    /// 热索引上限 (MB)
    pub max_hot_mb: usize,
    /// 冷索引缓存时间 (秒)
    pub cold_cache_secs: u64,
    /// 是否开启记忆模式
    pub memory_mode: bool,
    /// 是否开启因果控制
    pub causal_control: bool,
    /// 是否开启自动 git
    pub auto_git: bool,
    /// 是否开启自适应防抖
    pub adaptive_debounce: bool,
    /// GUI 桌面应用（隐藏开关，默认关闭）
    pub gui_enabled: bool,
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self {
            project_path: PathBuf::from("."),
            max_hot_mb: 256,
            cold_cache_secs: 300, // 5 分钟
            memory_mode: true,
            causal_control: true,
            auto_git: true,
            adaptive_debounce: true,
            gui_enabled: false,
        }
    }
}

impl BaseConfig {
    /// 从项目路径加载配置
    pub fn load(project_path: &Path) -> Result<Self> {
        let config_path = project_path.join(".route").join("base-config.json");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config: {:?}", config_path))?;
            let config: BaseConfig = serde_json::from_str(&content)
                .with_context(|| "Failed to parse base-config.json")?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// 保存配置
    pub fn save(&self, project_path: &Path) -> Result<()> {
        let config_path = project_path.join(".route").join("base-config.json");
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

// ─── Root Base 主结构体 ───────────────────────────────────────────────────

/// Root Base — 顶层编排器
///
/// 持有 Root Engine 和 Root Memory，提供统一 API。
pub struct RootBase {
    /// 引擎（驱动层）
    pub engine: route_engine::RootEngine,
    /// 记忆（存储层）
    pub memory: route_memory::RootMemory,
    /// 配置
    pub config: BaseConfig,
    /// 服务注册表（可扩展）
    pub registry: ServiceRegistry,
}

impl RootBase {
    /// 创建新的 Root Base 实例
    pub fn new(project_path: &Path) -> Result<Self> {
        let config = BaseConfig::load(project_path)?;
        let engine = route_engine::RootEngine::new();
        let memory = route_memory::RootMemory::new(project_path)?;

        Ok(Self {
            engine,
            memory,
            config,
            registry: ServiceRegistry::new(),
        })
    }

    /// 初始化全部组件
    pub fn init(&mut self) -> Result<()> {
        tracing::info!("[BASE] Initializing Root Base...");

        // 1. 加载项目记忆
        if self.config.memory_mode {
            self.memory.load()?;
            let stats = self.memory.stats();
            tracing::info!("[BASE] Memory loaded: {} entries", stats.total_entries);
        }

        // 2. 初始化引擎索引（自动平衡热/冷）
        self.engine.hot_cold_index.auto_balance();
        tracing::info!("[BASE] Engine index initialized");

        tracing::info!("[BASE] Root Base initialized successfully");
        Ok(())
    }

    /// 搜索代码（三机制：Keyword → Graph → Vector）
    pub fn search(&self, query: &str, top_k: usize) -> Vec<route_engine::RootSearchResult> {
        self.engine.search(query, top_k)
    }

    /// 获取项目结构（Mermaid 格式）
    pub fn project_structure(&self) -> Option<String> {
        self.memory.structure_mermaid()
    }

    /// 获取项目元信息（JSON 格式）
    pub fn project_meta(&self) -> Option<String> {
        self.memory.project_json()
    }

    /// 记录因果链
    pub fn record_causal_link(
        &mut self,
        action: &str,
        file_path: &str,
        reason: &str,
        effect: &str,
    ) -> Result<()> {
        self.memory.add_causal_link(action, file_path, reason, effect)?;
        Ok(())
    }

    /// 获取状态摘要
    pub fn status(&self) -> BaseStatus {
        let mem_stats = self.memory.stats();
        let tiered_stats = self.memory.tiered.stats();
        BaseStatus {
            project: self.config.project_path.display().to_string(),
            memory_mode: self.config.memory_mode,
            causal_control: self.config.causal_control,
            auto_git: self.config.auto_git,
            adaptive_debounce: self.config.adaptive_debounce,
            gui_enabled: self.config.gui_enabled,
            memory_entries: mem_stats.total_entries,
            memory_chains: self.memory.chain.links.len(),
            hot_blocks: *tiered_stats.get("hot_count").unwrap_or(&0),
            cold_blocks: *tiered_stats.get("cold_count").unwrap_or(&0),
            estimated_bytes: *tiered_stats.get("hot_bytes").unwrap_or(&0)
                + *tiered_stats.get("cold_bytes").unwrap_or(&0),
            services: self.registry.list(),
        }
    }

    /// 重置所有数据
    pub fn reset(&mut self) -> Result<()> {
        self.memory.reset()?;
        self.engine = route_engine::RootEngine::new();
        self.registry.clear();
        tracing::info!("[BASE] Root Base reset complete");
        Ok(())
    }

    /// 启用 GUI 桌面应用
    pub fn gui_enable(&mut self) -> Result<()> {
        self.config.gui_enabled = true;
        self.config.save(&self.config.project_path)?;
        tracing::info!("[BASE] GUI enabled");
        Ok(())
    }

    /// 禁用 GUI 桌面应用
    pub fn gui_disable(&mut self) -> Result<()> {
        self.config.gui_enabled = false;
        self.config.save(&self.config.project_path)?;
        tracing::info!("[BASE] GUI disabled");
        Ok(())
    }

    /// 获取 GUI 状态
    pub fn gui_status(&self) -> bool {
        self.config.gui_enabled
    }
}

// ─── 状态摘要 ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct BaseStatus {
    pub project: String,
    pub memory_mode: bool,
    pub causal_control: bool,
    pub auto_git: bool,
    pub adaptive_debounce: bool,
    pub gui_enabled: bool,
    pub memory_entries: usize,
    pub memory_chains: usize,
    pub hot_blocks: usize,
    pub cold_blocks: usize,
    pub estimated_bytes: usize,
    pub services: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_base_config_default() {
        let config = BaseConfig::default();
        assert_eq!(config.max_hot_mb, 256);
        assert!(config.memory_mode);
    }

    #[test]
    fn test_base_new() {
        let dir = tempdir().unwrap();
        let base = RootBase::new(dir.path());
        assert!(base.is_ok());
    }

    #[test]
    fn test_config_save_load() {
        let dir = tempdir().unwrap();
        let config = BaseConfig {
            max_hot_mb: 128,
            ..Default::default()
        };
        config.save(dir.path()).unwrap();

        let loaded = BaseConfig::load(dir.path()).unwrap();
        assert_eq!(loaded.max_hot_mb, 128);
    }

    #[test]
    fn test_status() {
        let dir = tempdir().unwrap();
        let base = RootBase::new(dir.path()).unwrap();
        let status = base.status();
        assert_eq!(status.memory_entries, 0);
        assert!(status.memory_mode);
    }

    #[test]
    fn test_service_registry() {
        let dir = tempdir().unwrap();
        let mut base = RootBase::new(dir.path()).unwrap();
        base.registry.register("test", 42i32);
        assert!(base.registry.has("test"));
        assert_eq!(*base.registry.get::<i32>("test").unwrap(), 42);
    }
}