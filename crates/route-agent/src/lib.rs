//! Route Agent — 将 Route 包装成可自主执行任务的 Agent。
//!
//! 核心组件：
//!   - [`Agent`] trait：Agent 的核心循环（plan → act → observe → loop）
//!   - [`ModelProvider`] trait：模型提供者（来自 route-plugins 的插件作为模型）
//!   - [`SkillHarness`]：技能注册、加载与执行（来自 .route/skills/ 的用户技能）
//!   - [`AgentMemory`]：短期对话记忆 + 长期项目记忆（量化偏差检测）
//!   - [`AgentTool`] trait：Agent 可调用的工具（read_file, write_file, git_ops 等）
//!
//! 预设插件（route-plugins）= 给 Agent 加模型：
//!   每个 Plugin 可以注册为 ModelProvider，
//!   通过 ModelRegistry 切换不同 AI 提供商。

pub mod agent;
pub mod model;
pub mod skill;
pub mod memory;
pub mod tools;

pub use agent::{Agent, AgentConfig, AgentStatus, RunResult, MAX_AGENT_ITERATIONS};
pub use model::{ModelProvider, ModelRegistry, ModelId, ModelCapability, ChatMessage, ChatRole};
pub use skill::{Skill, SkillHarness, SkillId, SkillManifest, load_skills_from_dir};
pub use memory::{AgentMemory, MemoryEntry, MemoryKind, MemoryStats, MemoryDriftReport};
pub use tools::{AgentTool, ToolRegistry, ToolCall, ToolResult, builtin_tools};
