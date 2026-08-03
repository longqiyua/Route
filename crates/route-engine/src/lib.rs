//! Route Engine — 基于 GOTO Engine 风格的模糊匹配搜索引擎
//!
//! 提供 Trie 前缀树、模糊匹配、索引树、BM25 语义检索、搜索管线、
//! 代码符号解析、代码图、GraphRAG、向量搜索、图索引、关键词索引、
//! 自适应防抖和热/冷索引管理。

pub mod adaptive;
pub mod bm25;
pub mod code_graph;
pub mod embedding;
pub mod fuzzy;
pub mod graph_idx;
pub mod graphrag;
pub mod index;
pub mod indexer;
pub mod keyword;
pub mod pipeline;
pub mod semantic_fuzzy;
pub mod symbol;
pub mod trie;
pub mod vector;

// 重新导出关键类型
pub use adaptive::{SearchOrchestrator, TypingSpeedTracker};
pub use bm25::{BM25Index, BM25Result};
pub use code_graph::{CodeGraph, EdgeKind, GraphEdge, GraphNode, NodeKind};
pub use embedding::{
    EmbeddingConfig, HashedNGramEmbedding, HybridVectorizer, VectorizeMode,
};
pub use fuzzy::{FuzzyMatch, SearchResult};
pub use graph_idx::{GraphIndex, GraphIndexResult};
pub use graphrag::{GraphRagConfig, GraphRagIndex, RagResult};
pub use index::{IndexEntryInfo, IndexTree};
pub use indexer::{HotColdIndex, IndexEntry as HotColdEntry};
pub use keyword::{KeywordIndex, KeywordResult, MatchType};
pub use pipeline::{SearchConfig, SearchPipeline};
pub use semantic_fuzzy::{
    ArchitectureAssistant, ArchitectureSuggestion, MatchMode, ModeSwitcher, RagPipeline, RagStage,
    RunCycleResult, SemanticIndex, SemanticLabel, SemanticMatch, SuggestionKind,
    ThirdPartyChunk, ThirdPartyChunking,
};
pub use symbol::{CodeSymbol, SymbolIndex, SymbolInfo};
pub use trie::{IndexEntry, TrieIndex, TrieNode};
pub use vector::{SearchMatch, VectorIndex};

/// RootEngine 搜索结果
#[derive(Debug, Clone)]
pub struct RootSearchResult {
    pub text: String,
    pub score: f64,
    pub file_path: Option<String>,
    pub line: usize,
    pub source: String,
}

/// RootEngine — 统一搜索引擎入口
///
/// 整合所有索引组件，提供统一的搜索接口。
#[derive(Debug, Clone)]
pub struct RootEngine {
    /// 搜索管线（Trie + BM25 组合搜索）
    pub pipeline: SearchPipeline,
    /// 符号索引
    pub symbol_index: SymbolIndex,
    /// 代码图
    pub code_graph: CodeGraph,
    /// GraphRAG 索引
    pub graphrag_index: GraphRagIndex,
    /// 向量索引
    pub vector_index: VectorIndex,
    /// 图索引
    pub graph_index: GraphIndex,
    /// 关键词索引
    pub keyword_index: KeywordIndex,
    /// 热/冷索引管理器
    pub hot_cold_index: HotColdIndex,
    /// 搜索编排器（自适应防抖）
    pub orchestrator: SearchOrchestrator,
}

impl Default for RootEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RootEngine {
    /// 创建一个新的 RootEngine
    pub fn new() -> Self {
        RootEngine {
            pipeline: SearchPipeline::new(),
            symbol_index: SymbolIndex::new(),
            code_graph: CodeGraph::new(),
            graphrag_index: GraphRagIndex::new(),
            vector_index: VectorIndex::new(),
            graph_index: GraphIndex::new(),
            keyword_index: KeywordIndex::new(),
            hot_cold_index: HotColdIndex::new(),
            orchestrator: SearchOrchestrator::new(),
        }
    }

    /// 添加行级索引
    pub fn add_line(&mut self, content: &str, file_path: &str, line: usize, language: &str) {
        self.pipeline.add_line(content, file_path, line, language);
        self.keyword_index
            .add_entry(&format!("{}:{}", file_path, line), content);
        self.hot_cold_index.add_entry(
            &format!("{}:{}", file_path, line),
            content,
            file_path,
        );
    }

    /// 统一搜索所有索引
    ///
    /// 返回合并后的搜索结果。
    pub fn search(&self, query: &str, top_k: usize) -> Vec<RootSearchResult> {
        let mut results: Vec<RootSearchResult> = Vec::new();

        // 1. 管线搜索（Trie + BM25）
        for r in self.pipeline.search(query, top_k) {
            results.push(RootSearchResult {
                text: r.text,
                score: r.score,
                file_path: r.file_path,
                line: r.line,
                source: "pipeline".to_string(),
            });
        }

        // 2. 关键词搜索
        for r in self.keyword_index.search(query, top_k) {
            if !results.iter().any(|res| res.file_path.as_deref() == Some(&r.id)) {
                results.push(RootSearchResult {
                    text: r.id.clone(),
                    score: r.score * 0.9,
                    file_path: Some(r.id),
                    line: 0,
                    source: "keyword".to_string(),
                });
            }
        }

        // 3. 向量搜索
        for r in self.vector_index.search(query, top_k) {
            if !results.iter().any(|res| res.text == r.id) {
                results.push(RootSearchResult {
                    text: r.content,
                    score: r.score * 0.7,
                    file_path: Some(r.file_path),
                    line: 0,
                    source: "vector".to_string(),
                });
            }
        }

        // 4. 图索引搜索
        for r in self.graph_index.search(query, top_k) {
            if !results.iter().any(|res| res.text == r.node) {
                results.push(RootSearchResult {
                    text: r.node.clone(),
                    score: r.score * 0.5,
                    file_path: None,
                    line: 0,
                    source: "graph".to_string(),
                });
            }
        }

        // 5. GraphRAG 搜索
        for r in self.graphrag_index.search(query, top_k) {
            if !results.iter().any(|res| res.text == r.symbol) {
                results.push(RootSearchResult {
                    text: r.symbol.clone(),
                    score: r.relevance * 0.6,
                    file_path: Some(r.file_path),
                    line: 0,
                    source: "graphrag".to_string(),
                });
            }
        }

        // 排序去重
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.dedup_by(|a, b| a.text == b.text && a.file_path == b.file_path);
        results.truncate(top_k);
        results
    }

    /// 处理输入事件（用于自适应防抖）
    pub fn on_input(&mut self, timestamp_ms: u64) {
        self.orchestrator.on_input(timestamp_ms);
    }

    /// 判断是否应该搜索
    pub fn should_search(&self, current_time_ms: u64) -> bool {
        self.orchestrator.should_search(current_time_ms)
    }

    /// 标记已搜索
    pub fn mark_searched(&mut self, timestamp_ms: u64) {
        self.orchestrator.mark_searched(timestamp_ms);
    }

    /// 获取搜索配置
    pub fn config(&self) -> &SearchConfig {
        self.pipeline.config()
    }

    /// 更新搜索配置
    pub fn set_config(&mut self, config: SearchConfig) {
        self.pipeline.set_config(config);
    }

    /// 获取索引总数
    pub fn count(&self) -> usize {
        self.pipeline.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_engine_new() {
        let engine = RootEngine::new();
        assert_eq!(engine.count(), 0);
    }

    #[test]
    fn test_root_engine_add_line() {
        let mut engine = RootEngine::new();
        engine.add_line("fn main() {}", "main.rs", 1, "rust");
        assert_eq!(engine.count(), 1);
    }

    #[test]
    fn test_root_engine_search() {
        let mut engine = RootEngine::new();
        engine.add_line("fn main() {}", "main.rs", 1, "rust");
        engine.add_line("fn helper() {}", "main.rs", 5, "rust");

        let results = engine.search("main", 10);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.text.contains("main")));
    }

    #[test]
    fn test_root_engine_search_empty() {
        let engine = RootEngine::new();
        let results = engine.search("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_root_engine_adaptive() {
        let mut engine = RootEngine::new();
        assert!(engine.should_search(1000));
        engine.mark_searched(1000);
        assert!(!engine.should_search(1050));
        assert!(engine.should_search(2000));
    }

    #[test]
    fn test_root_engine_config() {
        let mut engine = RootEngine::new();
        let config = SearchConfig {
            max_results: 100,
            ..Default::default()
        };
        engine.set_config(config);
        assert_eq!(engine.config().max_results, 100);
    }
}