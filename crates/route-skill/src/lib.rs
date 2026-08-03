//! Route Skill — 增强型技能系统
//!
//! 三层模型：
//! - **Skill**（技能）：告诉 AI "怎么做"——能力层
//! - **Reference**（参考资料）：告诉 AI "参考什么"——知识层
//! - **RAG**（外置知识库）：基于 route-engine 的行级搜索——检索层
//!
//! # 架构
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │          SkillEngine                │  ← 统一入口
//! │  ┌──────────┐  ┌──────────┐  ┌───┐ │
//! │  │  Skill   │  │Reference │  │RAG│ │
//! │  │  Harness │  │Registry  │  │   │ │
//! │  └──────────┘  └──────────┘  └───┘ │
//! └─────────────────────────────────────┘
//! ```
//!
//! # 存储位置
//!
//! - `.route/skills/` — 技能文件（.md 或 .json）
//! - `.route/references/` — 参考资料文件（.md）
//! - `.route/rag/` — RAG 索引缓存（自动构建）

pub mod harness;
pub mod rag;
pub mod reference;
pub mod skill;

// 重新导出关键类型
pub use harness::{SkillEngine, TaskPlan};
pub use rag::{RagEngine, RagKnowledge, RagResult};
pub use reference::{Reference, ReferencePriority, ReferenceRegistry};
pub use skill::{Skill, SkillBody, SkillContext, SkillExecution, SkillHarness, SkillManifest, SkillStep};

/// 版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");