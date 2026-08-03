//! 参考资料管理
//!
//! 参考资料是"知识"——告诉 AI 参考什么。Skill 告诉 AI "怎么做"，
//! Reference 告诉 AI "参考什么"。

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

/// 参考优先级
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum ReferencePriority {
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
}

impl ReferencePriority {
    /// 优先级数值（越高越优先）
    pub fn score(&self) -> u8 {
        match self {
            ReferencePriority::Critical => 4,
            ReferencePriority::High => 3,
            ReferencePriority::Medium => 2,
            ReferencePriority::Low => 1,
        }
    }
}

impl std::fmt::Display for ReferencePriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReferencePriority::Critical => write!(f, "critical"),
            ReferencePriority::High => write!(f, "high"),
            ReferencePriority::Medium => write!(f, "medium"),
            ReferencePriority::Low => write!(f, "low"),
        }
    }
}

impl Default for ReferencePriority {
    fn default() -> Self {
        ReferencePriority::Medium
    }
}

/// 参考资料
#[derive(Debug, Clone, Deserialize)]
pub struct Reference {
    pub id: String,
    pub title: String,
    pub description: String,
    pub content: String,
    pub source: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub priority: ReferencePriority,
}

/// 参考资料注册表
#[derive(Debug, Clone)]
pub struct ReferenceRegistry {
    pub references: HashMap<String, Reference>,
}

impl ReferenceRegistry {
    pub fn new() -> Self {
        ReferenceRegistry {
            references: HashMap::new(),
        }
    }

    /// 从目录加载参考资料
    ///
    /// 支持 .md 和 .json 格式。
    /// - .md 文件：使用 frontmatter 解析元数据
    /// - .json 文件：包含 Reference 结构
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut registry = ReferenceRegistry::new();

        if !dir.exists() {
            tracing::warn!("参考资料目录不存在: {:?}", dir);
            return Ok(registry);
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
                    match serde_json::from_str::<Reference>(&content) {
                        Ok(reference) => {
                            registry.register(reference);
                        }
                        Err(e) => {
                            tracing::warn!("跳过无效的参考资料文件 {:?}: {}", path, e);
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

                    if let Some(reference) = parse_markdown_reference(&file_stem, &content) {
                        registry.register(reference);
                    }
                }
                _ => {
                    tracing::debug!("跳过不支持的文件类型: {:?}", path);
                }
            }
        }

        Ok(registry)
    }

    /// 注册参考资料
    pub fn register(&mut self, reference: Reference) {
        self.references.insert(reference.id.clone(), reference);
    }

    /// 根据标签查找参考资料
    pub fn find_by_tag(&self, tag: &str) -> Vec<&Reference> {
        let tag_lower = tag.to_lowercase();
        self.references
            .values()
            .filter(|r| r.tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .collect()
    }

    /// 根据 ID 查找参考资料
    pub fn find_by_id(&self, id: &str) -> Option<&Reference> {
        self.references.get(id)
    }

    /// 根据上下文获取相关参考资料
    ///
    /// 根据上下文关键词匹配 tags 和 content 中的关键词。
    /// 结果按优先级排序。
    pub fn get_relevant(&self, context: &str) -> Vec<&Reference> {
        let context_lower = context.to_lowercase();
        let words: Vec<&str> = context_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        let mut scored: Vec<(&Reference, usize)> = self
            .references
            .values()
            .map(|r| {
                let mut score = 0;

                // 匹配 tags
                for tag in &r.tags {
                    if words.iter().any(|w| tag.to_lowercase().contains(*w) || w.contains(&tag.to_lowercase())) {
                        score += 3;
                    }
                }

                // 匹配 title 和 description
                for word in &words {
                    if r.title.to_lowercase().contains(word) {
                        score += 2;
                    }
                    if r.description.to_lowercase().contains(word) {
                        score += 1;
                    }
                    if r.content.to_lowercase().contains(word) {
                        score += 1;
                    }
                }

                // 优先级加分
                score += r.priority.score() as usize * 2;

                (r, score)
            })
            .filter(|(_, score)| *score > 0)
            .collect();

        // 按得分降序排列
        scored.sort_by(|a, b| b.1.cmp(&a.1));

        scored.into_iter().map(|(r, _)| r).collect()
    }

    /// 将指定参考资料渲染为 Markdown 文本
    pub fn render_markdown(&self, ids: &[String]) -> String {
        let mut output = String::new();

        for id in ids {
            if let Some(reference) = self.references.get(id) {
                output.push_str(&format!("## {}\n\n", reference.title));
                if !reference.description.is_empty() {
                    output.push_str(&format!("> {}\n\n", reference.description));
                }
                if let Some(ref source) = reference.source {
                    output.push_str(&format!("*来源: {}*\n\n", source));
                }
                if let Some(ref lang) = reference.language {
                    output.push_str(&format!("```{}\n{}\n```\n\n", lang, reference.content));
                } else {
                    output.push_str(&format!("{}\n\n", reference.content));
                }
                output.push_str(&format!("*优先级: {}*  \n", reference.priority));
                if !reference.tags.is_empty() {
                    output.push_str(&format!("*标签: {}*  \n", reference.tags.join(", ")));
                }
                output.push_str("\n---\n\n");
            }
        }

        output
    }

    /// 获取所有参考资料
    pub fn all(&self) -> Vec<&Reference> {
        self.references.values().collect()
    }
}

impl Default for ReferenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 从 Markdown 文件解析参考资料
///
/// 支持 frontmatter 格式：
/// ```markdown
/// ---
/// id: my-ref
/// title: 我的参考资料
/// tags: [rust, coding]
/// priority: high
/// ---
/// 内容正文...
/// ```
fn parse_markdown_reference(file_stem: &str, content: &str) -> Option<Reference> {
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let frontmatter = &content[3..3 + end];
            let body = &content[3 + end + 3..].trim().to_string();

            let mut id = file_stem.to_string();
            let mut title = String::new();
            let mut description = String::new();
            let mut source = None;
            let mut tags = Vec::new();
            let mut language = None;
            let mut priority = ReferencePriority::Medium;

            for line in frontmatter.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim().to_lowercase();
                    let value = value.trim().trim_matches('"').to_string();
                    match key.as_str() {
                        "id" => id = value,
                        "title" => title = value,
                        "description" => description = value,
                        "source" => source = Some(value),
                        "tags" => {
                            tags = value
                                .split(',')
                                .map(|s| s.trim().trim_matches('"').to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "language" | "lang" => language = Some(value),
                        "priority" => {
                            priority = match value.to_lowercase().as_str() {
                                "critical" => ReferencePriority::Critical,
                                "high" => ReferencePriority::High,
                                "medium" => ReferencePriority::Medium,
                                "low" => ReferencePriority::Low,
                                _ => ReferencePriority::Medium,
                            };
                        }
                        _ => {}
                    }
                }
            }

            if title.is_empty() {
                title = id.clone();
            }

            return Some(Reference {
                id,
                title,
                description,
                content: body.clone(),
                source,
                tags,
                language,
                priority,
            });
        }
    }

    // 没有 frontmatter，使用文件名作为标题
    Some(Reference {
        id: file_stem.to_string(),
        title: file_stem.to_string(),
        description: String::new(),
        content: content.to_string(),
        source: None,
        tags: Vec::new(),
        language: None,
        priority: ReferencePriority::Medium,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = ReferenceRegistry::new();
        assert!(registry.references.is_empty());
    }

    #[test]
    fn test_register_and_find() {
        let mut registry = ReferenceRegistry::new();
        let reference = Reference {
            id: "rust-style".to_string(),
            title: "Rust 编码规范".to_string(),
            description: "Rust 项目编码风格指南".to_string(),
            content: "使用 snake_case 命名变量".to_string(),
            source: Some("内部文档".to_string()),
            tags: vec!["rust".to_string(), "coding".to_string()],
            language: Some("rust".to_string()),
            priority: ReferencePriority::High,
        };
        registry.register(reference);

        assert!(registry.find_by_id("rust-style").is_some());
        assert!(registry.find_by_id("nonexistent").is_none());

        let by_tag = registry.find_by_tag("rust");
        assert_eq!(by_tag.len(), 1);
    }

    #[test]
    fn test_get_relevant() {
        let mut registry = ReferenceRegistry::new();
        registry.register(Reference {
            id: "rust-style".to_string(),
            title: "Rust 编码规范".to_string(),
            description: "Rust 项目编码风格指南".to_string(),
            content: "使用 snake_case 命名变量".to_string(),
            source: None,
            tags: vec!["rust".to_string(), "coding".to_string()],
            language: Some("rust".to_string()),
            priority: ReferencePriority::High,
        });
        registry.register(Reference {
            id: "python-style".to_string(),
            title: "Python 编码规范".to_string(),
            description: "Python 编码风格".to_string(),
            content: "使用 snake_case".to_string(),
            source: None,
            tags: vec!["python".to_string(), "coding".to_string()],
            language: Some("python".to_string()),
            priority: ReferencePriority::Medium,
        });

        let relevant = registry.get_relevant("rust 代码 review");
        assert!(!relevant.is_empty());
        // Rust 相关的应该排前面
        assert!(relevant.iter().any(|r| r.id == "rust-style"));
    }

    #[test]
    fn test_render_markdown() {
        let mut registry = ReferenceRegistry::new();
        registry.register(Reference {
            id: "test".to_string(),
            title: "测试文档".to_string(),
            description: "测试描述".to_string(),
            content: "测试内容".to_string(),
            source: Some("测试来源".to_string()),
            tags: vec!["test".to_string()],
            language: Some("text".to_string()),
            priority: ReferencePriority::Medium,
        });

        let rendered = registry.render_markdown(&["test".to_string()]);
        assert!(rendered.contains("测试文档"));
        assert!(rendered.contains("测试内容"));
        assert!(rendered.contains("测试来源"));
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(ReferencePriority::Critical.to_string(), "critical");
        assert_eq!(ReferencePriority::High.to_string(), "high");
        assert_eq!(ReferencePriority::Medium.to_string(), "medium");
        assert_eq!(ReferencePriority::Low.to_string(), "low");
    }

    #[test]
    fn test_priority_score() {
        assert_eq!(ReferencePriority::Critical.score(), 4);
        assert_eq!(ReferencePriority::High.score(), 3);
        assert_eq!(ReferencePriority::Medium.score(), 2);
        assert_eq!(ReferencePriority::Low.score(), 1);
    }
}