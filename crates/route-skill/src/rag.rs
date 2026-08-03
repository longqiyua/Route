//! RAG 外置知识库
//!
//! RAG 是"外置知识库"——基于 route-engine 的行级搜索。
//! 当 route-engine 可用时，使用其 SearchPipeline 进行搜索；
//! 否则使用简单的关键词匹配作为 fallback。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// RAG 知识条目
#[derive(Debug, Clone)]
pub struct RagKnowledge {
    pub id: String,
    pub content: String,
    pub source: String,
    pub metadata: HashMap<String, String>,
}

/// RAG 搜索结果
#[derive(Debug, Clone)]
pub struct RagResult {
    pub knowledge: RagKnowledge,
    pub score: f64,
    pub snippet: String,
}

/// RAG 引擎
#[derive(Debug, Clone)]
pub struct RagEngine {
    #[cfg(feature = "route-engine")]
    pub engine: Option<route_engine::SearchPipeline>,
    #[cfg(not(feature = "route-engine"))]
    #[allow(dead_code)]
    engine: Option<PlaceholderEngine>,
    pub knowledge_base: Vec<RagKnowledge>,
    pub enabled: bool,
}

// 当 route-engine feature 未启用时，使用简单的占位引擎
#[cfg(not(feature = "route-engine"))]
#[derive(Debug, Clone)]
struct PlaceholderEngine;

impl RagEngine {
    pub fn new() -> Self {
        RagEngine {
            #[cfg(feature = "route-engine")]
            engine: None,
            #[cfg(not(feature = "route-engine"))]
            engine: None,
            knowledge_base: Vec::new(),
            enabled: false,
        }
    }

    /// 从项目路径构建 RAG 引擎
    ///
    /// 扫描项目中的代码文件，构建搜索索引。
    pub fn build_from_project(path: &Path) -> Result<Self> {
        let engine = RagEngine::new();

        if !path.exists() {
            tracing::warn!("项目路径不存在: {:?}", path);
            return Ok(engine);
        }

        // 收集代码文件
        let mut files = Vec::new();
        collect_code_files(path, &mut files, 0)?;

        if files.is_empty() {
            tracing::warn!("在 {:?} 中未找到代码文件", path);
            return Ok(engine);
        }

        Self::build_from_files(&files)
    }

    /// 从指定文件列表构建 RAG 引擎
    pub fn build_from_files(files: &[PathBuf]) -> Result<Self> {
        let mut engine = RagEngine::new();

        if files.is_empty() {
            return Ok(engine);
        }

        // 初始化搜索管线
        #[cfg(feature = "route-engine")]
        {
            let mut pipeline = route_engine::SearchPipeline::new();
            let mut knowledge_base = Vec::new();

            for file_path in files {
                if !file_path.exists() {
                    continue;
                }

                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("无法读取文件 {:?}: {}", file_path, e);
                        continue;
                    }
                };

                let file_stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let ext = file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();

                // 添加行级索引
                for (line_num, line) in content.lines().enumerate() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let file_path_str = file_path.to_string_lossy().to_string();
                    pipeline.add_line(line, &file_path_str, line_num + 1, &ext);
                }

                // 添加知识库条目
                let mut metadata = HashMap::new();
                metadata.insert("path".to_string(), file_path.to_string_lossy().to_string());
                metadata.insert("extension".to_string(), ext.clone());
                metadata.insert("lines".to_string(), content.lines().count().to_string());

                knowledge_base.push(RagKnowledge {
                    id: file_stem,
                    content,
                    source: file_path.to_string_lossy().to_string(),
                    metadata,
                });
            }

            engine.engine = Some(pipeline);
            engine.knowledge_base = knowledge_base;
            engine.enabled = true;
        }

        #[cfg(not(feature = "route-engine"))]
        {
            // 没有 route-engine 时，只存储知识库，使用简单搜索
            let mut knowledge_base = Vec::new();

            for file_path in files {
                if !file_path.exists() {
                    continue;
                }

                let content = match std::fs::read_to_string(file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!("无法读取文件 {:?}: {}", file_path, e);
                        continue;
                    }
                };

                let file_stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let mut metadata = HashMap::new();
                metadata.insert("path".to_string(), file_path.to_string_lossy().to_string());
                metadata.insert("lines".to_string(), content.lines().count().to_string());

                knowledge_base.push(RagKnowledge {
                    id: file_stem,
                    content,
                    source: file_path.to_string_lossy().to_string(),
                    metadata,
                });
            }

            engine.knowledge_base = knowledge_base;
            engine.enabled = true;
        }

        Ok(engine)
    }

    /// 搜索 RAG 知识库
    ///
    /// * `query` - 搜索查询
    /// * `top_k` - 返回前 k 个结果
    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<RagResult>> {
        if !self.enabled {
            return Ok(Vec::new());
        }

        #[cfg(feature = "route-engine")]
        {
            if let Some(ref pipeline) = self.engine {
                let results = pipeline.search(query, top_k);

                // 将 SearchResult 映射为 RagResult
                let mut rag_results: Vec<RagResult> = results
                    .iter()
                    .map(|r| {
                        let snippet = r.text.clone();
                        let source = r
                            .file_path
                            .as_deref()
                            .unwrap_or("unknown")
                            .to_string();

                        // 查找对应的 knowledge 条目
                        let knowledge = self
                            .knowledge_base
                            .iter()
                            .find(|k| k.source == source)
                            .cloned()
                            .unwrap_or_else(|| RagKnowledge {
                                id: "unknown".to_string(),
                                content: snippet.clone(),
                                source: source.clone(),
                                metadata: HashMap::new(),
                            });

                        RagResult {
                            knowledge,
                            score: r.score,
                            snippet,
                        }
                    })
                    .collect();

                rag_results.truncate(top_k);
                return Ok(rag_results);
            }

            // 没有 pipeline 时 fallthrough 到文本搜索
        }

        // 简单关键词匹配（fallback）
        Ok(self.simple_search(query, top_k))
    }

    /// 简单关键词搜索（不使用 route-engine）
    fn simple_search(&self, query: &str, top_k: usize) -> Vec<RagResult> {
        let query_lower = query.to_lowercase();
        let keywords: Vec<&str> = query_lower.split_whitespace().filter(|w| w.len() > 1).collect();

        if keywords.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(f64, &RagKnowledge, String)> = self
            .knowledge_base
            .iter()
            .filter_map(|knowledge| {
                let content_lower = knowledge.content.to_lowercase();
                let mut score = 0.0;
                let mut snippet = String::new();

                for keyword in &keywords {
                    if content_lower.contains(keyword) {
                        score += 1.0;
                        // 提取包含关键词的行作为 snippet
                        for line in knowledge.content.lines() {
                            if line.to_lowercase().contains(keyword) {
                                snippet.push_str(line.trim());
                                snippet.push_str(" ... ");
                                if snippet.len() > 200 {
                                    snippet.truncate(200);
                                    snippet.push_str("...");
                                    break;
                                }
                            }
                        }
                    }
                }

                if score > 0.0 {
                    if snippet.is_empty() {
                        snippet = knowledge.content.chars().take(200).collect();
                    }
                    Some((score, knowledge, snippet))
                } else {
                    None
                }
            })
            .collect();

        // 按得分降序排列
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(score, knowledge, snippet)| RagResult {
                knowledge: knowledge.clone(),
                score,
                snippet,
            })
            .collect()
    }

    /// 添加知识条目
    pub fn add_knowledge(&mut self, knowledge: RagKnowledge) {
        // 如果已存在相同 ID 的知识，替换之
        if let Some(pos) = self
            .knowledge_base
            .iter()
            .position(|k| k.id == knowledge.id)
        {
            self.knowledge_base[pos] = knowledge;
        } else {
            self.knowledge_base.push(knowledge);
        }
    }

    /// 检查 RAG 引擎是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for RagEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 递归收集代码文件
///
/// 跳过 .git, node_modules, target 等目录
fn collect_code_files(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 10 {
        return Ok(());
    }

    let skip_dirs = [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".vscode",
        ".idea",
        "__pycache__",
        ".route",
    ];

    let code_extensions = [
        "rs", "go", "py", "js", "ts", "tsx", "jsx", "java", "c", "cpp", "h", "hpp",
        "rb", "php", "swift", "kt", "scala", "clj", "ex", "exs", "erl",
        "sql", "r", "m", "mm", "dart", "lua", "sh", "bash", "zsh",
        "toml", "yaml", "yml", "json", "xml", "md", "css", "scss", "html",
    ];

    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let dir_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if skip_dirs.contains(&dir_name) {
                continue;
            }
            collect_code_files(&path, files, depth + 1)?;
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if code_extensions.contains(&ext) {
                    files.push(path);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_engine_new() {
        let engine = RagEngine::new();
        assert!(!engine.is_enabled());
        assert!(engine.knowledge_base.is_empty());
    }

    #[test]
    fn test_add_knowledge() {
        let mut engine = RagEngine::new();
        let mut metadata = HashMap::new();
        metadata.insert("path".to_string(), "test.rs".to_string());

        engine.add_knowledge(RagKnowledge {
            id: "test".to_string(),
            content: "fn main() { println!(\"hello\"); }".to_string(),
            source: "test.rs".to_string(),
            metadata,
        });

        assert_eq!(engine.knowledge_base.len(), 1);
    }

    #[test]
    fn test_simple_search() {
        let mut engine = RagEngine::new();

        let mut metadata = HashMap::new();
        metadata.insert("path".to_string(), "main.rs".to_string());

        engine.add_knowledge(RagKnowledge {
            id: "main".to_string(),
            content: "fn main() {\n    println!(\"Hello, world!\");\n}".to_string(),
            source: "main.rs".to_string(),
            metadata,
        });

        // 手动设置 enabled 以便 simple_search 工作
        engine.enabled = true;

        let results = engine.search("main", 10).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_disabled() {
        let engine = RagEngine::new();
        let results = engine.search("test", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_collect_code_files() {
        let temp_dir = std::env::temp_dir().join("route-skill-test-collect");
        let _ = std::fs::create_dir_all(&temp_dir);
        let _ = std::fs::write(temp_dir.join("test.rs"), "fn main() {}");
        let _ = std::fs::write(temp_dir.join("test.py"), "print('hello')");
        let _ = std::fs::write(temp_dir.join("README.md"), "# readme");

        let mut files = Vec::new();
        collect_code_files(&temp_dir, &mut files, 0).unwrap();
        assert!(!files.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}