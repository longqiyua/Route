//! 增强型 Harness
//!
//! 统一入口，管理 Skill / Reference / RAG 三层。

use std::path::Path;

use anyhow::Result;

use crate::rag::{RagEngine, RagResult};
use crate::reference::{Reference, ReferenceRegistry};
use crate::skill::{Skill, SkillContext, SkillHarness};

/// 任务计划
#[derive(Debug, Clone)]
pub struct TaskPlan {
    pub task: String,
    pub matched_skills: Vec<String>,
    pub relevant_references: Vec<Reference>,
    pub rag_results: Vec<RagResult>,
    pub execution_order: Vec<String>,
}

/// 技能引擎（统一入口）
#[derive(Debug, Clone)]
pub struct SkillEngine {
    pub harness: SkillHarness,
    pub references: ReferenceRegistry,
    pub rag: RagEngine,
}

impl SkillEngine {
    pub fn new() -> Self {
        SkillEngine {
            harness: SkillHarness::new(),
            references: ReferenceRegistry::new(),
            rag: RagEngine::new(),
        }
    }

    /// 从项目路径加载
    ///
    /// 从项目的 `.route/skills/`、`.route/references/` 和 `.route/rag/` 加载。
    pub fn load_from_project(path: &Path) -> Result<Self> {
        let mut engine = SkillEngine::new();

        let skills_dir = path.join(".route").join("skills");
        let references_dir = path.join(".route").join("references");
        let rag_dir = path.join(".route").join("rag");

        // 加载技能
        if skills_dir.exists() {
            match SkillHarness::load_from_dir(&skills_dir) {
                Ok(harness) => {
                    engine.harness = harness;
                    tracing::info!("已加载 {} 个技能", engine.harness.skills.len());
                }
                Err(e) => {
                    tracing::warn!("加载技能失败: {}", e);
                }
            }
        } else {
            tracing::info!("技能目录不存在，跳过: {:?}", skills_dir);
        }

        // 加载参考资料
        if references_dir.exists() {
            match ReferenceRegistry::load_from_dir(&references_dir) {
                Ok(registry) => {
                    engine.references = registry;
                    tracing::info!("已加载 {} 个参考资料", engine.references.references.len());
                }
                Err(e) => {
                    tracing::warn!("加载参考资料失败: {}", e);
                }
            }
        } else {
            tracing::info!("参考资料目录不存在，跳过: {:?}", references_dir);
        }

        // 加载 RAG
        if rag_dir.exists() {
            // 尝试从项目目录构建 RAG 索引
            match RagEngine::build_from_project(path) {
                Ok(rag) => {
                    engine.rag = rag;
                    tracing::info!("RAG 引擎已构建");
                }
                Err(e) => {
                    tracing::warn!("构建 RAG 引擎失败: {}", e);
                }
            }
        } else {
            tracing::info!("RAG 目录不存在，跳过: {:?}", rag_dir);
        }

        Ok(engine)
    }

    /// 分析任务并生成执行计划
    ///
    /// 1. 匹配技能（根据触发词）
    /// 2. 获取相关参考资料
    /// 3. 搜索 RAG 知识库
    /// 4. 生成执行计划
    pub fn process_task(&self, task: &str) -> Result<TaskPlan> {
        // 1. 匹配技能
        let matched_skills: Vec<&Skill> = self.harness.find_by_trigger(task);

        // 收集技能引用的参考资料
        let mut reference_ids: Vec<String> = Vec::new();
        for skill in &matched_skills {
            for ref_id in &skill.manifest.references {
                if !reference_ids.contains(ref_id) {
                    reference_ids.push(ref_id.clone());
                }
            }
        }

        // 2. 获取相关参考资料
        let relevant_references = self.references.get_relevant(task);

        // 也加入技能明确引用的参考资料
        for ref_id in &reference_ids {
            if let Some(reference) = self.references.find_by_id(ref_id) {
                if !relevant_references.iter().any(|r| r.id == reference.id) {
                    // 将引用加入但注意所有权
                }
            }
        }

        // 合并引用（去重）
        let mut all_references: Vec<Reference> = relevant_references.into_iter().cloned().collect();
        for ref_id in &reference_ids {
            if let Some(reference) = self.references.find_by_id(ref_id) {
                if !all_references.iter().any(|r| r.id == reference.id) {
                    all_references.push(reference.clone());
                }
            }
        }

        // 3. 搜索 RAG 知识库
        let rag_results = self.rag.search(task, 10).unwrap_or_default();

        // 4. 生成执行顺序
        let execution_order: Vec<String> = matched_skills
            .iter()
            .map(|s| s.manifest.id.clone())
            .collect();

        Ok(TaskPlan {
            task: task.to_string(),
            matched_skills: execution_order.clone(),
            relevant_references: all_references,
            rag_results,
            execution_order,
        })
    }

    /// 列出所有技能
    pub fn list_skills(&self) -> Vec<&Skill> {
        self.harness.skills.iter().collect()
    }

    /// 列出所有参考资料
    pub fn list_references(&self) -> Vec<&Reference> {
        self.references.all()
    }

    /// 获取 RAG 状态
    pub fn rag_status(&self) -> String {
        if self.rag.is_enabled() {
            format!(
                "RAG 引擎已启用 ({} 条知识, {} 个文件)",
                self.rag.knowledge_base.len(),
                self.rag.knowledge_base.len()
            )
        } else {
            "RAG 引擎未启用".to_string()
        }
    }

    /// 执行技能
    pub fn execute_skill(&self, id: &str, ctx: &SkillContext) -> Result<crate::skill::SkillExecution> {
        self.harness.execute_with_embeds(id, ctx)
    }
}

impl Default for SkillEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_skill_engine_new() {
        let engine = SkillEngine::new();
        assert!(engine.harness.skills.is_empty());
        assert!(engine.references.references.is_empty());
        assert!(!engine.rag.is_enabled());
    }

    #[test]
    fn test_process_task_no_skills() {
        let engine = SkillEngine::new();
        let plan = engine.process_task("进行代码审查").unwrap();
        assert!(plan.matched_skills.is_empty());
        assert_eq!(plan.task, "进行代码审查");
    }

    #[test]
    fn test_process_task_with_skills() {
        let mut engine = SkillEngine::new();

        // 注册一个技能
        let skill = Skill {
            manifest: crate::skill::SkillManifest {
                id: "code-review".to_string(),
                name: "代码审查".to_string(),
                version: "1.0.0".to_string(),
                description: "执行代码审查".to_string(),
                author: None,
                triggers: vec!["审查".to_string(), "review".to_string()],
                embed_skills: vec![],
                references: vec!["coding-style".to_string()],
                tags: vec!["code".to_string(), "review".to_string()],
            },
            body: crate::skill::SkillBody::Markdown("审查代码".to_string()),
            source_path: PathBuf::from("review.md"),
        };
        engine.harness.register(skill);

        // 注册参考资料
        engine.references.register(Reference {
            id: "coding-style".to_string(),
            title: "编码规范".to_string(),
            description: "项目编码规范".to_string(),
            content: "使用蛇形命名法".to_string(),
            source: None,
            tags: vec!["coding".to_string(), "style".to_string()],
            language: None,
            priority: crate::reference::ReferencePriority::High,
        });

        let plan = engine.process_task("请对 main.rs 进行代码审查").unwrap();
        assert!(!plan.matched_skills.is_empty());
        assert_eq!(plan.matched_skills[0], "code-review");
    }

    #[test]
    fn test_list_skills_and_references() {
        let mut engine = SkillEngine::new();

        let skill = Skill {
            manifest: crate::skill::SkillManifest {
                id: "test".to_string(),
                name: "测试".to_string(),
                version: "1.0.0".to_string(),
                description: "测试技能".to_string(),
                author: None,
                triggers: vec![],
                embed_skills: vec![],
                references: vec![],
                tags: vec![],
            },
            body: crate::skill::SkillBody::Markdown("".to_string()),
            source_path: PathBuf::from("test.md"),
        };
        engine.harness.register(skill);

        engine.references.register(Reference {
            id: "ref1".to_string(),
            title: "参考1".to_string(),
            description: "".to_string(),
            content: "内容".to_string(),
            source: None,
            tags: vec![],
            language: None,
            priority: crate::reference::ReferencePriority::Medium,
        });

        assert_eq!(engine.list_skills().len(), 1);
        assert_eq!(engine.list_references().len(), 1);
    }

    #[test]
    fn test_rag_status() {
        let engine = SkillEngine::new();
        assert!(engine.rag_status().contains("未启用"));
    }
}