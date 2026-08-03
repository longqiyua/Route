//! 搜索管线
//!
//! 三级搜索管线：精确搜索 → 前缀搜索 → 模糊搜索 → BM25 搜索。
//! 结果合并去重，按 score 降序排列。

use crate::bm25::BM25Index;
use crate::fuzzy::{FuzzyMatch, SearchResult};
use crate::index::IndexTree;

/// 搜索配置
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// 精确匹配阈值（默认 1.0）
    pub exact_threshold: f64,
    /// 前缀匹配阈值（默认 0.8）
    pub prefix_threshold: f64,
    /// 模糊匹配阈值（默认 0.3）
    pub fuzzy_threshold: f64,
    /// BM25 权重（默认 0.5）
    pub bm25_weight: f64,
    /// 最大返回结果数（默认 50）
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            exact_threshold: 1.0,
            prefix_threshold: 0.8,
            fuzzy_threshold: 0.3,
            bm25_weight: 0.5,
            max_results: 50,
        }
    }
}

/// 搜索管线
#[derive(Debug, Clone)]
pub struct SearchPipeline {
    /// 索引树
    index_tree: IndexTree,
    /// BM25 索引
    bm25_index: BM25Index,
    /// 搜索配置
    config: SearchConfig,
    /// 文档计数（用于 BM25 的文档 ID）
    doc_counter: usize,
    /// 行级内容存储（用于 BM25 搜索）
    line_contents: Vec<(String, usize, String)>,
}

impl Default for SearchPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchPipeline {
    /// 创建一个新的搜索管线
    pub fn new() -> Self {
        SearchPipeline {
            index_tree: IndexTree::new(),
            bm25_index: BM25Index::new(),
            config: SearchConfig::default(),
            line_contents: Vec::new(),
            doc_counter: 0,
        }
    }

    /// 使用自定义配置创建搜索管线
    pub fn with_config(config: SearchConfig) -> Self {
        SearchPipeline {
            index_tree: IndexTree::new(),
            bm25_index: BM25Index::new(),
            config,
            line_contents: Vec::new(),
            doc_counter: 0,
        }
    }

    /// 添加行级索引
    ///
    /// * `content` - 行内容
    /// * `file_path` - 源文件路径
    /// * `line` - 行号
    /// * `language` - 编程语言
    pub fn add_line(&mut self, content: &str, file_path: &str, line: usize, _language: &str) {
        // 添加到索引树
        self.index_tree.build(content, file_path, line);

        // 添加到 BM25 索引
        let doc_id = format!("{}:{}:{}", file_path, line, self.doc_counter);
        self.doc_counter += 1;
        self.bm25_index.add_document(&doc_id, content);

        // 保存行级内容以便 BM25 结果映射
        self.line_contents.push((
            file_path.to_string(),
            line,
            content.to_string(),
        ));
    }

    /// 执行搜索
    ///
    /// 执行三级搜索管线：精确搜索 → 前缀搜索 → 模糊搜索 → BM25 搜索。
    ///
    /// * `query` - 查询字符串
    /// * `top_k` - 返回前 k 个结果
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let mut all_results: Vec<SearchResult> = Vec::new();
        let max_results = self.config.max_results.min(top_k);

        // 1. 精确搜索 + 前缀搜索（通过索引树）
        let index_results = self.index_tree.search(query);
        for r in index_results {
            // 精确匹配（score == 1.0）直接通过
            if r.score >= self.config.exact_threshold {
                all_results.push(r);
            }
        }

        // 2. 模糊搜索（通过 FuzzyMatch 对所有索引条目进行匹配）
        let fuzzy = FuzzyMatch::new(query, self.config.fuzzy_threshold);
        for entry in self.index_tree.entries() {
            if let Some(result) = fuzzy.match_text(&entry.content, Some(&entry.file_path), entry.line) {
                // 跳过已经在精确搜索结果中的条目
                if !all_results.iter().any(|r| {
                    r.line == result.line && r.file_path == result.file_path
                }) {
                    // 前缀匹配过滤
                    if result.score >= self.config.prefix_threshold
                        || (result.score < 1.0 && result.score >= 0.1)
                    {
                        all_results.push(result);
                    }
                }
            }
        }

        // 3. BM25 搜索
        let bm25_top_k = (max_results as f64 * 1.5) as usize;
        let bm25_results = self.bm25_index.search(query, bm25_top_k);

        // 将 BM25 结果映射为 SearchResult
        for bm25_result in &bm25_results {
            // 解析文档 ID 获取文件路径和行号
            let (file_path, line) = parse_doc_id(&bm25_result.id);

            // 跳过已在结果中的条目
            if !all_results.iter().any(|r| {
                r.line == line && r.file_path.as_deref() == Some(&file_path)
            }) {
                let score = bm25_result.score * self.config.bm25_weight;
                // 归一化 BM25 得分到 0.1 ~ 0.5 区间
                let normalized_score = 0.1 + (score.tanh() * 0.4);
                all_results.push(SearchResult {
                    text: bm25_result.snippet.clone(),
                    score: normalized_score,
                    file_path: Some(file_path),
                    line,
                });
            }
        }

        // 按 score 降序排列
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 去重（同一行同一文件只保留最高分）
        let mut deduped: Vec<SearchResult> = Vec::new();
        for result in all_results {
            if !deduped.iter().any(|r| {
                r.line == result.line && r.file_path == result.file_path
            }) {
                deduped.push(result);
            }
        }

        deduped.truncate(max_results);
        deduped
    }

    /// 获取索引树引用
    pub fn index_tree(&self) -> &IndexTree {
        &self.index_tree
    }

    /// 获取 BM25 索引引用
    pub fn bm25_index(&self) -> &BM25Index {
        &self.bm25_index
    }

    /// 获取搜索配置引用
    pub fn config(&self) -> &SearchConfig {
        &self.config
    }

    /// 更新搜索配置
    pub fn set_config(&mut self, config: SearchConfig) {
        self.config = config;
    }

    /// 重建所有索引
    pub fn rebuild(&mut self) {
        self.index_tree.rebuild();
        // 重新构建 BM25 索引
        let old_contents = std::mem::take(&mut self.line_contents);
        self.bm25_index = BM25Index::new();
        self.doc_counter = 0;
        for (file_path, line, content) in &old_contents {
            let doc_id = format!("{}:{}:{}", file_path, line, self.doc_counter);
            self.doc_counter += 1;
            self.bm25_index.add_document(&doc_id, content);
        }
        self.line_contents = old_contents;
    }

    /// 获取索引条目总数
    pub fn count(&self) -> usize {
        self.index_tree.count()
    }
}

/// 解析文档 ID 为文件路径和行号
fn parse_doc_id(doc_id: &str) -> (String, usize) {
    // 格式: file_path:line:counter
    if let Some(last_colon) = doc_id.rfind(':') {
        let without_counter = &doc_id[..last_colon];
        if let Some(second_colon) = without_counter.rfind(':') {
            let file_path = &without_counter[..second_colon];
            let line_str = &without_counter[second_colon + 1..];
            let line: usize = line_str.parse().unwrap_or(0);
            return (file_path.to_string(), line);
        }
    }
    (doc_id.to_string(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_pipeline() {
        let mut engine = SearchPipeline::new();
        engine.add_line("fn main() {", "src/main.rs", 1, "rust");
        engine.add_line("    println!(\"hello\");", "src/main.rs", 2, "rust");

        let results = engine.search("main", 10);
        assert!(!results.is_empty());
        // "main" 应该被精确匹配或前缀匹配
        assert!(results.iter().any(|r| r.line == 1));
    }

    #[test]
    fn test_search_empty_query() {
        let engine = SearchPipeline::new();
        let results = engine.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_no_match() {
        let engine = SearchPipeline::new();
        let results = engine.search("nonexistent", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_with_config() {
        let config = SearchConfig {
            exact_threshold: 1.0,
            prefix_threshold: 0.8,
            fuzzy_threshold: 0.3,
            bm25_weight: 0.5,
            max_results: 10,
        };
        let mut engine = SearchPipeline::with_config(config);
        engine.add_line("fn main() {", "src/main.rs", 1, "rust");
        engine.add_line("    println!(\"hello\");", "src/main.rs", 2, "rust");

        let results = engine.search("main", 10);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_rebuild() {
        let mut engine = SearchPipeline::new();
        engine.add_line("fn main() {", "src/main.rs", 1, "rust");
        assert_eq!(engine.count(), 1);

        engine.rebuild();
        assert_eq!(engine.count(), 1);
    }

    #[test]
    fn test_parse_doc_id() {
        let (path, line) = parse_doc_id("src/main.rs:1:0");
        assert_eq!(path, "src/main.rs");
        assert_eq!(line, 1);
    }

    #[test]
    fn test_complete_search_example() {
        // 完整搜索示例
        let mut engine = SearchPipeline::new();
        engine.add_line("fn main() {", "src/main.rs", 1, "rust");
        engine.add_line("    println!(\"hello\");", "src/main.rs", 2, "rust");
        engine.add_line("}", "src/main.rs", 3, "rust");

        let results = engine.search("main", 10);
        assert!(!results.is_empty());
        // 确保 "fn main() {" 行被匹配到
        assert!(results.iter().any(|r| r.line == 1));
    }
}