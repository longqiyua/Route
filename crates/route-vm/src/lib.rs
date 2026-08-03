//! Route VM — 版本管理 Agent 核心
//!
//! 取代旧 route-agent 的新核心 crate，面向 vibecoding 场景，
//! 提供 plan→act→observe 循环、因果控制、Git 自动管理和工具注册表。

pub mod agent;
pub mod causal;
pub mod git;
pub mod tools;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::agent::VmResult;
use crate::causal::CausalController;
use crate::git::GitOps;
use crate::tools::ToolRegistry;

/// 版本管理 Agent 核心入口
pub struct VmAgent {
    pub config: VmConfig,
    pub project_path: Option<PathBuf>,
    #[cfg(feature = "route-memory")]
    pub memory: Option<route_memory::memory::ProjectMemory>,
    #[cfg(not(feature = "route-memory"))]
    pub memory: Option<()>,
    #[cfg(feature = "route-engine")]
    pub engine: Option<route_engine::SearchPipeline>,
    #[cfg(not(feature = "route-engine"))]
    pub engine: Option<()>,
    pub causal: CausalController,
    pub git: GitOps,
    pub tools: ToolRegistry,
}

/// Agent 运行模式
#[derive(Debug, Clone, PartialEq)]
pub enum VmMode {
    /// 全自动 Agent 模式
    Agent,
    /// 工具调用模式
    Tool,
    /// 监听模式（只追踪不操作）
    Monitor,
}

/// Agent 配置
#[derive(Debug, Clone)]
pub struct VmConfig {
    pub mode: VmMode,
    /// 是否开启记忆模式
    pub memory_mode: bool,
    /// 是否开启因果控制
    pub causal_control: bool,
    /// 是否自动 Git 管理
    pub auto_git: bool,
    pub max_iterations: usize,
    pub project_path: Option<PathBuf>,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            mode: VmMode::Agent,
            memory_mode: false,
            causal_control: false,
            auto_git: false,
            max_iterations: 10,
            project_path: None,
        }
    }
}

impl VmAgent {
    /// 创建新的 VmAgent
    pub fn new(config: VmConfig) -> Self {
        let project_path = config.project_path.clone();
        let repo_path = project_path.as_deref().unwrap_or_else(|| Path::new("."));

        Self {
            git: GitOps::new(repo_path),
            causal: CausalController::new(),
            tools: ToolRegistry::new(),
            project_path,
            config,
            memory: None,
            engine: None,
        }
    }

    /// 从项目路径创建 VmAgent，自动加载记忆和搜索引擎
    pub fn with_project(path: &Path) -> Result<Self> {
        let config = VmConfig {
            project_path: Some(path.to_path_buf()),
            ..Default::default()
        };

        #[allow(unused_mut)]
        let mut agent = Self::new(config);

        // 加载项目记忆
        #[cfg(feature = "route-memory")]
        {
            match route_memory::memory::ProjectMemory::load(path) {
                Ok(memory) => {
                    tracing::info!("[MEMORY] Loaded project memory ({} entries)", memory.entries.len());
                    agent.memory = Some(memory);
                }
                Err(e) => {
                    tracing::warn!("[MEMORY] Failed to load project memory: {}", e);
                }
            }
        }
        #[cfg(not(feature = "route-memory"))]
        {
            let _ = path;
            tracing::debug!("route-memory feature not enabled, skipping memory load");
        }

        // 加载搜索引擎
        #[cfg(feature = "route-engine")]
        {
            agent.engine = Some(route_engine::SearchPipeline::new());
            tracing::info!("[ENGINE] Initialized search pipeline");
        }
        #[cfg(not(feature = "route-engine"))]
        {
            tracing::debug!("route-engine feature not enabled, skipping engine init");
        }

        Ok(agent)
    }

    /// 运行 Agent 任务
    pub fn run(&mut self, task: &str) -> Result<VmResult> {
        let start = std::time::Instant::now();

        tracing::info!("[VM] Starting task: {}", task);

        // 执行 plan→act→observe 循环
        let result = self.run_cycle(task)?;

        let duration_ms = start.elapsed().as_millis();

        Ok(VmResult {
            duration_ms,
            ..result
        })
    }

    /// 带记忆的运行 Agent 任务
    pub fn run_with_memory(&mut self, task: &str) -> Result<VmResult> {
        let start = std::time::Instant::now();

        tracing::info!("[VM] Starting task with memory: {}", task);

        // 1. 加载项目记忆
        #[cfg(feature = "route-memory")]
        self.load_memory_context()?;

        // 2. 执行 cycle
        let mut result = self.run_cycle(task)?;

        // 3. 检测副作用
        if self.config.causal_control {
            if let Some(ref path) = self.project_path {
                let path_str = path.to_string_lossy();
                let side_effects = self.causal.detect_side_effects(&path_str);
                result.side_effects = side_effects;
            }
        }

        // 4. 自动 Git 提交
        if self.config.auto_git {
            if let Some(ref path) = self.project_path {
                self.git.repo_path = path.clone();
            }
            match self.git.auto_commit("vm") {
                Ok(msg) => {
                    result.git_changes.push(msg);
                }
                Err(e) => {
                    tracing::warn!("[GIT] Auto commit failed: {}", e);
                }
            }
        }

        // 5. 更新记忆
        #[cfg(feature = "route-memory")]
        self.update_memory(&mut result)?;

        let duration_ms = start.elapsed().as_millis();
        result.duration_ms = duration_ms;

        Ok(result)
    }

    /// 加载记忆上下文，输出思考链信息
    #[cfg(feature = "route-memory")]
    fn load_memory_context(&mut self) -> Result<()> {
        if !self.config.memory_mode {
            return Ok(());
        }

        let path = match self.config.project_path.as_ref().or(self.project_path.as_ref()) {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let memory_dir = path.join(".route").join("memory");

        // 加载项目元信息
        let meta_path = memory_dir.join("project.json");
        if meta_path.exists() {
            match route_memory::project::ProjectMeta::load(&meta_path) {
                Ok(meta) => {
                    tracing::info!(
                        "[MEMORY] Project: {} ({}) — {}",
                        meta.name,
                        meta.language,
                        meta.description
                    );
                    // 更新或创建 memory 对象
                    if let Some(ref mut mem) = self.memory {
                        mem.meta = Some(meta);
                    }
                }
                Err(e) => {
                    tracing::warn!("[MEMORY] Failed to load project meta: {}", e);
                }
            }
        }

        // 加载项目结构
        let structure_path = memory_dir.join("structure.json");
        if structure_path.exists() {
            match std::fs::read_to_string(&structure_path) {
                Ok(content) => {
                    if let Ok(structure) = serde_json::from_str::<route_memory::structure::ProjectStructure>(&content) {
                        tracing::info!(
                            "[MEMORY] Structure: {} files, {} dirs, {:?} languages",
                            structure.total_files,
                            structure.total_dirs,
                            structure.languages.keys().collect::<Vec<_>>()
                        );
                        if let Some(ref mut mem) = self.memory {
                            mem.structure = Some(structure);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("[MEMORY] Failed to load structure: {}", e);
                }
            }
        }

        // 加载因果链
        let chain_path = memory_dir.join("chain.jsonl");
        if chain_path.exists() {
            let chain = route_memory::chain::CausalChain::load(&path)?;
            tracing::info!("[MEMORY] Causal chain: {} links", chain.links.len());
            // 将已有的因果链事件导入 controller
            for link in &chain.links {
                let file = link.file_path.clone().unwrap_or_default();
                self.causal.record_event(causal::CausalEvent {
                    id: link.id.clone(),
                    action: link.action.clone(),
                    file,
                    timestamp: link.timestamp.clone(),
                    side_effects: link.side_effects.clone(),
                });
            }
        }

        Ok(())
    }

    /// 更新记忆
    #[cfg(feature = "route-memory")]
    fn update_memory(&mut self, result: &mut VmResult) -> Result<()> {
        if !self.config.memory_mode {
            return Ok(());
        }

        let path = match self.config.project_path.as_ref().or(self.project_path.as_ref()) {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let memory_dir = path.join(".route").join("memory");

        // 确保记忆目录存在
        if !memory_dir.exists() {
            return Ok(());
        }

        // 更新记忆条目
        let entries_path = memory_dir.join("entries.jsonl");
        if entries_path.exists() {
            #[cfg(feature = "route-memory")]
            if let Some(ref mut mem) = self.memory {
                if !result.output.is_empty() {
                    let entry = route_memory::memory::MemoryEntry {
                        id: format!("vm-{}", chrono::Utc::now().timestamp()),
                        kind: route_memory::memory::MemoryKind::Change,
                        key: format!("vm-change-{}", chrono::Utc::now().timestamp()),
                        content: result.output.clone(),
                        tags: vec!["vm".to_string(), "auto".to_string()],
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                        source: "route-vm".to_string(),
                    };
                    mem.put(entry);
                    result.memory_updates.push("Updated memory entries".to_string());
                }
            }
        }

        // 保存记忆
        #[cfg(feature = "route-memory")]
        if let Some(ref mem) = self.memory {
            if let Err(e) = mem.save() {
                tracing::warn!("[MEMORY] Failed to save memory: {}", e);
            }
        }

        Ok(())
    }

    /// 核心 plan→act→observe 循环
    fn run_cycle(&mut self, task: &str) -> Result<VmResult> {
        let mut output = String::new();
        let status = "completed".to_string();
        let mut git_changes = Vec::new();
        let memory_updates = Vec::new();
        let mut side_effects = Vec::new();

        for iteration in 0..self.config.max_iterations {
            tracing::info!("[CYCLE] Iteration {}/{}", iteration + 1, self.config.max_iterations);

            // Plan 阶段
            tracing::info!("[PLAN] Analyzing task: {}", task);
            output.push_str(&format!("[PLAN] Iteration {}: {}\n", iteration + 1, task));

            // Act 阶段 — 根据 mode 执行
            match self.config.mode {
                VmMode::Monitor => {
                    // Monitor 模式只追踪
                    tracing::info!("[ACT] Monitor mode — tracking only");
                    output.push_str("[ACT] Monitor mode — no actions taken\n");
                    break;
                }
                VmMode::Tool => {
                    // Tool 模式 — 执行工具调用
                    tracing::info!("[ACT] Tool mode — executing tools");
                    output.push_str("[ACT] Tool mode — awaiting tool calls\n");
                    break;
                }
                VmMode::Agent => {
                    // Agent 模式 — 执行操作
                    tracing::info!("[ACT] Agent mode — executing task");
                    output.push_str(&format!("[ACT] Executing: {}\n", task));

                    // 记录因果事件
                    if self.config.causal_control {
                        let event = causal::CausalEvent {
                            id: format!("iter-{}-{}", iteration, chrono::Utc::now().timestamp()),
                            action: task.to_string(),
                            file: "unknown".to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            side_effects: Vec::new(),
                        };
                        self.causal.record_event(event);
                    }
                }
            }

            // Observe 阶段
            tracing::info!("[OBSERVE] Checking results");
            output.push_str(&format!("[OBSERVE] Iteration {} complete\n", iteration + 1));

            // 检测副作用
            if self.config.causal_control {
                let detected = self.causal.detect_side_effects(task);
                for effect in &detected {
                    side_effects.push(effect.clone());
                    tracing::warn!("[CAUSAL] Side effect detected: {}", effect);
                }
            }

            // 自动 Git 提交
            if self.config.auto_git {
                match self.git.auto_commit("vm") {
                    Ok(msg) => {
                        git_changes.push(msg);
                    }
                    Err(e) => {
                        tracing::warn!("[GIT] Auto commit failed: {}", e);
                    }
                }
            }
        }

        Ok(VmResult {
            success: true,
            status,
            output,
            git_changes,
            memory_updates,
            side_effects,
            duration_ms: 0,
        })
    }
}