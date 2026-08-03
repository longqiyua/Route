//! 技能定义与执行
//!
//! 技能是"能力"——告诉 AI 怎么做某事。支持嵌入（一个技能可以引用另一个技能）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

/// 技能
#[derive(Debug, Clone)]
pub struct Skill {
    pub manifest: SkillManifest,
    pub body: SkillBody,
    pub source_path: PathBuf,
}

/// 技能清单
#[derive(Debug, Clone, Deserialize)]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub triggers: Vec<String>,
    pub embed_skills: Vec<String>,
    pub references: Vec<String>,
    pub tags: Vec<String>,
}

/// 技能体
#[derive(Debug, Clone)]
pub enum SkillBody {
    Markdown(String),
    Steps(Vec<SkillStep>),
}

/// 技能步骤
#[derive(Debug, Clone, Deserialize)]
pub struct SkillStep {
    pub order: usize,
    pub description: String,
    pub tool: Option<String>,
    pub tool_args: Option<Value>,
    pub embed_skill: Option<String>,
}

/// 技能执行上下文
#[derive(Debug, Clone)]
pub struct SkillContext {
    pub args: HashMap<String, Value>,
    pub working_dir: Option<PathBuf>,
}

impl SkillContext {
    pub fn new() -> Self {
        SkillContext {
            args: HashMap::new(),
            working_dir: None,
        }
    }

    pub fn with_args(args: HashMap<String, Value>) -> Self {
        SkillContext {
            args,
            working_dir: None,
        }
    }
}

impl Default for SkillContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 技能执行结果
#[derive(Debug, Clone)]
pub struct SkillExecution {
    pub skill_id: String,
    pub skill_name: String,
    pub output: String,
    pub embed_results: Vec<SkillExecution>,
    pub success: bool,
}

/// 技能容器
#[derive(Debug, Clone)]
pub struct SkillHarness {
    pub skills: Vec<Skill>,
}

impl SkillHarness {
    pub fn new() -> Self {
        SkillHarness { skills: Vec::new() }
    }

    /// 从目录加载技能文件
    ///
    /// 支持 .md 和 .json 格式的技能文件。
    /// - .md 文件：使用文件名作为 ID，正文作为 Markdown 技能体
    /// - .json 文件：包含完整的 Skill 结构
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut harness = SkillHarness::new();

        if !dir.exists() {
            tracing::warn!("技能目录不存在: {:?}", dir);
            return Ok(harness);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                continue;
            }

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            match ext.as_str() {
                "json" => {
                    let content = std::fs::read_to_string(&path)?;
                    match serde_json::from_str::<SkillDefinition>(&content) {
                        Ok(def) => {
                            let skill = Skill {
                                manifest: SkillManifest {
                                    id: def.id,
                                    name: def.name,
                                    version: def.version,
                                    description: def.description,
                                    author: def.author,
                                    triggers: def.triggers,
                                    embed_skills: def.embed_skills.unwrap_or_default(),
                                    references: def.references.unwrap_or_default(),
                                    tags: def.tags.unwrap_or_default(),
                                },
                                body: if let Some(steps) = def.steps {
                                    SkillBody::Steps(steps)
                                } else if let Some(markdown) = def.markdown {
                                    SkillBody::Markdown(markdown)
                                } else {
                                    SkillBody::Markdown(String::new())
                                },
                                source_path: path.clone(),
                            };
                            harness.register(skill);
                        }
                        Err(e) => {
                            tracing::warn!("跳过无效的技能文件 {:?}: {}", path, e);
                        }
                    }
                }
                "md" => {
                    let content = std::fs::read_to_string(&path)?;
                    let file_stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    // 从 Markdown 文件头解析元数据（如果有 frontmatter）
                    let (manifest, body_content) = parse_markdown_skill(&file_stem, &content);

                    let skill = Skill {
                        manifest,
                        body: SkillBody::Markdown(body_content),
                        source_path: path.clone(),
                    };
                    harness.register(skill);
                }
                _ => {
                    tracing::debug!("跳过不支持的文件类型: {:?}", path);
                }
            }
        }

        Ok(harness)
    }

    /// 注册技能
    pub fn register(&mut self, skill: Skill) {
        // 如果已存在相同 ID 的技能，替换之
        if let Some(pos) = self.skills.iter().position(|s| s.manifest.id == skill.manifest.id) {
            self.skills[pos] = skill;
        } else {
            self.skills.push(skill);
        }
    }

    /// 根据触发词查找技能
    pub fn find_by_trigger(&self, text: &str) -> Vec<&Skill> {
        let text_lower = text.to_lowercase();
        self.skills
            .iter()
            .filter(|s| {
                s.manifest
                    .triggers
                    .iter()
                    .any(|t| text_lower.contains(&t.to_lowercase()))
            })
            .collect()
    }

    /// 根据 ID 获取技能
    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.manifest.id == id)
    }

    /// 执行技能（不执行嵌入子技能）
    pub fn execute(&self, id: &str, ctx: &SkillContext) -> Result<SkillExecution> {
        let skill = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("技能未找到: {}", id))?;

        let output = match &skill.body {
            SkillBody::Markdown(md) => {
                // 简单渲染 Markdown，替换上下文变量
                render_markdown_template(md, ctx)
            }
            SkillBody::Steps(steps) => {
                // 渲染步骤描述
                let mut output = String::new();
                for step in steps {
                    output.push_str(&format!("步骤 {}: {}", step.order, step.description));
                    output.push('\n');
                    if let Some(ref tool) = step.tool {
                        output.push_str(&format!("  工具: {}\n", tool));
                    }
                }
                output
            }
        };

        Ok(SkillExecution {
            skill_id: skill.manifest.id.clone(),
            skill_name: skill.manifest.name.clone(),
            output,
            embed_results: Vec::new(),
            success: true,
        })
    }

    /// 执行技能并递归执行嵌入的子技能
    pub fn execute_with_embeds(&self, id: &str, ctx: &SkillContext) -> Result<SkillExecution> {
        let skill = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("技能未找到: {}", id))?;

        let mut embed_results = Vec::new();

        // 递归执行嵌入的子技能
        for embed_id in &skill.manifest.embed_skills {
            if self.get(embed_id).is_some() {
                // 合并上下文：嵌入技能可以访问父技能上下文
                match self.execute_with_embeds(embed_id, ctx) {
                    Ok(result) => embed_results.push(result),
                    Err(e) => {
                        tracing::warn!("嵌入技能 '{}' 执行失败: {}", embed_id, e);
                        embed_results.push(SkillExecution {
                            skill_id: embed_id.clone(),
                            skill_name: embed_id.clone(),
                            output: format!("执行失败: {}", e),
                            embed_results: Vec::new(),
                            success: false,
                        });
                    }
                }
            } else {
                tracing::warn!("嵌入技能 '{}' 未找到", embed_id);
            }
        }

        // 如果是 Steps 类型，可能步骤中也嵌入了技能
        if let SkillBody::Steps(steps) = &skill.body {
            for step in steps {
                if let Some(ref embed_skill_id) = step.embed_skill {
                    if !embed_results.iter().any(|r| r.skill_id == *embed_skill_id) {
                        if self.get(embed_skill_id).is_some() {
                            match self.execute_with_embeds(embed_skill_id, ctx) {
                                Ok(result) => embed_results.push(result),
                                Err(e) => {
                                    tracing::warn!("步骤嵌入技能 '{}' 执行失败: {}", embed_skill_id, e);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 执行当前技能
        let mut execution = self.execute(id, ctx)?;
        execution.embed_results = embed_results;

        Ok(execution)
    }

    /// 列出所有技能 ID
    pub fn list_skill_ids(&self) -> Vec<&str> {
        self.skills.iter().map(|s| s.manifest.id.as_str()).collect()
    }
}

impl Default for SkillHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON 技能定义（用于反序列化）
#[derive(Debug, Deserialize)]
struct SkillDefinition {
    id: String,
    name: String,
    version: String,
    description: String,
    author: Option<String>,
    triggers: Vec<String>,
    #[serde(default)]
    embed_skills: Option<Vec<String>>,
    #[serde(default)]
    references: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    steps: Option<Vec<SkillStep>>,
    #[serde(default)]
    markdown: Option<String>,
}

/// 解析 Markdown 技能文件
///
/// 支持简单的 YAML-like frontmatter（--- 分隔的元数据区域）
fn parse_markdown_skill(file_stem: &str, content: &str) -> (SkillManifest, String) {
    // 检查是否有 frontmatter
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let frontmatter = &content[3..3 + end];
            let body = &content[3 + end + 3..].trim();

            let mut id = file_stem.to_string();
            let mut name = String::new();
            let mut version = "0.1.0".to_string();
            let mut description = String::new();
            let mut author = None;
            let mut triggers = Vec::new();
            let mut embed_skills = Vec::new();
            let mut references = Vec::new();
            let mut tags = Vec::new();

            for line in frontmatter.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"').to_string();
                    match key {
                        "id" => id = value,
                        "name" => name = value,
                        "version" => version = value,
                        "description" => description = value,
                        "author" => author = Some(value),
                        "triggers" => {
                            triggers = value
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "embed_skills" | "embed-skills" => {
                            embed_skills = value
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "references" => {
                            references = value
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "tags" => {
                            tags = value
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        _ => {}
                    }
                }
            }

            if name.is_empty() {
                name = id.clone();
            }
            if description.is_empty() {
                description = format!("技能: {}", name);
            }

            let manifest = SkillManifest {
                id,
                name,
                version,
                description,
                author,
                triggers,
                embed_skills,
                references,
                tags,
            };

            return (manifest, body.to_string());
        }
    }

    // 没有 frontmatter，使用默认值
    let manifest = SkillManifest {
        id: file_stem.to_string(),
        name: file_stem.to_string(),
        version: "0.1.0".to_string(),
        description: format!("技能: {}", file_stem),
        author: None,
        triggers: Vec::new(),
        embed_skills: Vec::new(),
        references: Vec::new(),
        tags: Vec::new(),
    };

    (manifest, content.to_string())
}

/// 渲染 Markdown 模板，替换上下文变量
///
/// 支持 {{ variable_name }} 语法
fn render_markdown_template(template: &str, ctx: &SkillContext) -> String {
    let mut result = template.to_string();

    for (key, value) in &ctx.args {
        let placeholder = format!("{{{{ {} }}}}", key);
        let value_str = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => value.to_string(),
        };
        result = result.replace(&placeholder, &value_str);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_harness_new() {
        let harness = SkillHarness::new();
        assert!(harness.skills.is_empty());
    }

    #[test]
    fn test_register_and_get() {
        let mut harness = SkillHarness::new();
        let skill = Skill {
            manifest: SkillManifest {
                id: "test".to_string(),
                name: "测试技能".to_string(),
                version: "1.0.0".to_string(),
                description: "一个测试技能".to_string(),
                author: None,
                triggers: vec!["测试".to_string()],
                embed_skills: vec![],
                references: vec![],
                tags: vec![],
            },
            body: SkillBody::Markdown("这是测试内容".to_string()),
            source_path: PathBuf::from("test.md"),
        };
        harness.register(skill);
        assert!(harness.get("test").is_some());
        assert!(harness.get("nonexistent").is_none());
    }

    #[test]
    fn test_find_by_trigger() {
        let mut harness = SkillHarness::new();
        let skill = Skill {
            manifest: SkillManifest {
                id: "review".to_string(),
                name: "代码审查".to_string(),
                version: "1.0.0".to_string(),
                description: "执行代码审查".to_string(),
                author: None,
                triggers: vec!["审查".to_string(), "review".to_string()],
                embed_skills: vec![],
                references: vec![],
                tags: vec![],
            },
            body: SkillBody::Markdown("审查代码".to_string()),
            source_path: PathBuf::from("review.md"),
        };
        harness.register(skill);
        let results = harness.find_by_trigger("请进行代码审查");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].manifest.id, "review");
    }

    #[test]
    fn test_execute_markdown() {
        let mut harness = SkillHarness::new();
        let skill = Skill {
            manifest: SkillManifest {
                id: "hello".to_string(),
                name: "打招呼".to_string(),
                version: "1.0.0".to_string(),
                description: "打招呼技能".to_string(),
                author: None,
                triggers: vec!["你好".to_string()],
                embed_skills: vec![],
                references: vec![],
                tags: vec![],
            },
            body: SkillBody::Markdown("你好，{{ name }}！".to_string()),
            source_path: PathBuf::from("hello.md"),
        };
        harness.register(skill);

        let mut ctx = SkillContext::new();
        ctx.args.insert("name".to_string(), Value::String("世界".to_string()));
        let result = harness.execute("hello", &ctx).unwrap();
        assert!(result.output.contains("世界"));
    }

    #[test]
    fn test_execute_steps() {
        let mut harness = SkillHarness::new();
        let skill = Skill {
            manifest: SkillManifest {
                id: "build".to_string(),
                name: "构建".to_string(),
                version: "1.0.0".to_string(),
                description: "构建项目".to_string(),
                author: None,
                triggers: vec!["构建".to_string()],
                embed_skills: vec![],
                references: vec![],
                tags: vec![],
            },
            body: SkillBody::Steps(vec![
                SkillStep {
                    order: 1,
                    description: "编译代码".to_string(),
                    tool: Some("cargo".to_string()),
                    tool_args: Some(serde_json::json!({"command": "build"})),
                    embed_skill: None,
                },
                SkillStep {
                    order: 2,
                    description: "运行测试".to_string(),
                    tool: Some("cargo".to_string()),
                    tool_args: Some(serde_json::json!({"command": "test"})),
                    embed_skill: None,
                },
            ]),
            source_path: PathBuf::from("build.json"),
        };
        harness.register(skill);
        let result = harness.execute("build", &SkillContext::new()).unwrap();
        assert!(result.success);
        assert!(result.output.contains("步骤 1"));
        assert!(result.output.contains("步骤 2"));
    }

    #[test]
    fn test_execute_with_embeds() {
        let mut harness = SkillHarness::new();

        // 子技能
        let child = Skill {
            manifest: SkillManifest {
                id: "child".to_string(),
                name: "子技能".to_string(),
                version: "1.0.0".to_string(),
                description: "子技能".to_string(),
                author: None,
                triggers: vec![],
                embed_skills: vec![],
                references: vec![],
                tags: vec![],
            },
            body: SkillBody::Markdown("子技能执行".to_string()),
            source_path: PathBuf::from("child.md"),
        };
        harness.register(child);

        // 父技能，嵌入子技能
        let parent = Skill {
            manifest: SkillManifest {
                id: "parent".to_string(),
                name: "父技能".to_string(),
                version: "1.0.0".to_string(),
                description: "父技能".to_string(),
                author: None,
                triggers: vec![],
                embed_skills: vec!["child".to_string()],
                references: vec![],
                tags: vec![],
            },
            body: SkillBody::Markdown("父技能执行".to_string()),
            source_path: PathBuf::from("parent.md"),
        };
        harness.register(parent);

        let result = harness.execute_with_embeds("parent", &SkillContext::new()).unwrap();
        assert!(result.success);
        assert_eq!(result.embed_results.len(), 1);
        assert_eq!(result.embed_results[0].skill_id, "child");
    }
}