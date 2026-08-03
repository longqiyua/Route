//! Route Vibe — Vibecoding 集成层
//!
//! 用户 vibecoding AI 与 Route 的桥梁。
//! 提供 MCP 服务、模型代理、会话管理和自然语言处理入口。

pub mod mcp;
pub mod proxy;
pub mod session;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "vm")]
use route_vm::VmAgent;
#[cfg(feature = "skill")]
use route_skill::SkillEngine;
#[cfg(feature = "memory")]
use route_memory::memory::ProjectMemory;
#[cfg(feature = "engine")]
use route_engine::SearchPipeline;

/// Vibe 配置
#[derive(Debug, Clone)]
pub struct VibeConfig {
    /// 全自动模式（用户随便说）
    pub auto_mode: bool,
    /// 记忆模式
    pub memory_mode: bool,
    /// 因果控制
    pub causal_control: bool,
    /// 自动 Git
    pub auto_git: bool,
    /// 项目路径
    pub project_path: Option<PathBuf>,
}

impl Default for VibeConfig {
    fn default() -> Self {
        Self {
            auto_mode: true,
            memory_mode: false,
            causal_control: false,
            auto_git: false,
            project_path: None,
        }
    }
}

/// Vibe 会话 — 核心入口
///
/// 封装了 VibeSession 的创建、配置和生命周期管理。
/// 用户 AI 通过 MCP 调用此会话中的工具完成任务。
pub struct VibeSession {
    /// 会话 ID
    pub id: String,
    /// 项目路径
    pub project_path: Option<PathBuf>,
    /// 版本管理 Agent（需要 feature `vm`）
    #[cfg(feature = "vm")]
    pub vm: Option<VmAgent>,
    #[cfg(not(feature = "vm"))]
    #[allow(dead_code)]
    pub vm: Option<()>,
    /// 技能引擎（需要 feature `skill`）
    #[cfg(feature = "skill")]
    pub skill: Option<SkillEngine>,
    #[cfg(not(feature = "skill"))]
    #[allow(dead_code)]
    pub skill: Option<()>,
    /// 项目记忆（需要 feature `memory`）
    #[cfg(feature = "memory")]
    pub memory: Option<ProjectMemory>,
    #[cfg(not(feature = "memory"))]
    #[allow(dead_code)]
    pub memory: Option<()>,
    /// 搜索引擎（需要 feature `engine`）
    #[cfg(feature = "engine")]
    pub engine: Option<SearchPipeline>,
    #[cfg(not(feature = "engine"))]
    #[allow(dead_code)]
    pub engine: Option<()>,
    /// 会话配置
    pub config: VibeConfig,
    /// 会话开始时间
    pub start_time: String,
}

impl VibeSession {
    /// 创建新的 Vibe 会话
    pub fn new(project_path: Option<PathBuf>, config: VibeConfig) -> Self {
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("vibe-{}-{}", chrono::Utc::now().timestamp_millis(), counter);
        let start_time = chrono::Utc::now().to_rfc3339();

        Self {
            id,
            project_path,
            #[cfg(feature = "vm")]
            vm: None,
            #[cfg(not(feature = "vm"))]
            vm: None,
            #[cfg(feature = "skill")]
            skill: None,
            #[cfg(not(feature = "skill"))]
            skill: None,
            #[cfg(feature = "memory")]
            memory: None,
            #[cfg(not(feature = "memory"))]
            memory: None,
            #[cfg(feature = "engine")]
            engine: None,
            #[cfg(not(feature = "engine"))]
            engine: None,
            config,
            start_time,
        }
    }

    /// 初始化版本管理
    #[cfg(feature = "vm")]
    pub fn init_vm(&mut self) -> Result<()> {
        let path = match self.project_path.as_ref() {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        let vm_config = route_vm::VmConfig {
            memory_mode: self.config.memory_mode,
            causal_control: self.config.causal_control,
            auto_git: self.config.auto_git,
            project_path: Some(path.clone()),
            ..Default::default()
        };

        let mut agent = VmAgent::new(vm_config);
        agent.project_path = Some(path);
        self.vm = Some(agent);
        tracing::info!("[VIBE] VM initialized");
        Ok(())
    }

    /// 初始化项目记忆
    #[cfg(feature = "memory")]
    pub fn init_memory(&mut self) -> Result<()> {
        let path = match self.project_path.as_ref() {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        match ProjectMemory::load(&path) {
            Ok(memory) => {
                self.memory = Some(memory);
                tracing::info!("[VIBE] Memory loaded");
            }
            Err(e) => {
                tracing::warn!("[VIBE] Failed to load memory: {}", e);
            }
        }
        Ok(())
    }

    /// 初始化技能引擎
    #[cfg(feature = "skill")]
    pub fn init_skill(&mut self) -> Result<()> {
        let path = match self.project_path.as_ref() {
            Some(p) => p.clone(),
            None => return Ok(()),
        };

        match SkillEngine::load_from_project(&path) {
            Ok(engine) => {
                self.skill = Some(engine);
                tracing::info!("[VIBE] Skill engine loaded");
            }
            Err(e) => {
                tracing::warn!("[VIBE] Failed to load skill engine: {}", e);
            }
        }
        Ok(())
    }

    /// 初始化搜索引擎
    #[cfg(feature = "engine")]
    pub fn init_engine(&mut self) -> Result<()> {
        self.engine = Some(SearchPipeline::new());
        tracing::info!("[VIBE] Engine initialized");
        Ok(())
    }
}

/// 处理自然语言任务
///
/// 用户只需要说一句话，此函数会自动：
/// 1. 检测意图（git 操作 / 代码修改 / 查询 / 技能）
/// 2. 加载记忆（如果 memory_mode）
/// 3. 匹配技能（通过 route-skill）
/// 4. 执行操作
/// 5. 返回结果（人类可读）
#[allow(unused_variables)]
pub fn process_natural_language(session: &mut VibeSession, task: &str) -> Result<String> {
    tracing::info!("[VIBE] Processing task: {}", task);

    let mut output = String::new();

    // 1. 加载记忆（如果 memory_mode 且 memory feature 启用）
    #[cfg(feature = "memory")]
    if session.config.memory_mode {
        if let Some(ref memory) = session.memory {
            let stats = memory.stats();
            output.push_str(&format!("📚 记忆已加载: {} 条条目\n", stats.total_entries));
        }
    }

    // 2. 匹配技能（如果 skill feature 启用）
    #[cfg(feature = "skill")]
    {
        if let Some(ref skill_engine) = session.skill {
            match skill_engine.process_task(task) {
                Ok(plan) => {
                    if !plan.matched_skills.is_empty() {
                        output.push_str(&format!(
                            "🔧 匹配到技能: {}\n",
                            plan.matched_skills.join(", ")
                        ));
                    }
                    if !plan.relevant_references.is_empty() {
                        output.push_str(&format!(
                            "📖 参考资料: {} 份\n",
                            plan.relevant_references.len()
                        ));
                    }
                }
                Err(e) => {
                    tracing::warn!("[VIBE] Skill processing failed: {}", e);
                }
            }
        }
    }

    // 3. 通过 VM 执行任务（如果 vm feature 启用）
    #[cfg(feature = "vm")]
    {
        if let Some(ref mut vm) = session.vm {
            let result = if session.config.memory_mode {
                vm.run_with_memory(task)
            } else {
                vm.run(task)
            };

            match result {
                Ok(vm_result) => {
                    output.push_str(&format!("✅ 执行完成: {}\n", vm_result.output));
                    if !vm_result.git_changes.is_empty() {
                        output.push_str(&format!(
                            "📦 Git 变更: {}\n",
                            vm_result.git_changes.join(", ")
                        ));
                    }
                    if !vm_result.side_effects.is_empty() {
                        output.push_str(&format!(
                            "⚠️ 副作用检测: {} 项\n",
                            vm_result.side_effects.len()
                        ));
                    }
                }
                Err(e) => {
                    output.push_str(&format!("❌ 执行失败: {}\n", e));
                }
            }
        } else {
            output.push_str("ℹ️ VM 未初始化\n");
        }
    }

    #[cfg(not(feature = "vm"))]
    {
        let _ = task;
        output.push_str("ℹ️ VM feature 未启用，跳过任务执行\n");
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vibe_config_default() {
        let config = VibeConfig::default();
        assert!(config.auto_mode);
        assert!(!config.memory_mode);
        assert!(!config.causal_control);
        assert!(!config.auto_git);
        assert!(config.project_path.is_none());
    }

    #[test]
    fn test_vibe_session_new() {
        let config = VibeConfig::default();
        let session = VibeSession::new(None, config);
        assert!(session.id.starts_with("vibe-"));
        assert!(session.project_path.is_none());
    }
}