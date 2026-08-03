//! GraphRAG — 基于 AST + LSP 的图检索增强生成索引
//!
//! 结合符号索引、依赖图、向量索引构建 GraphRAG 索引，
//! 支持基于查询的图结构搜索和 AST 上下文提取。

use crate::code_graph::{CodeGraph, EdgeKind, NodeKind};
use crate::symbol::{CodeSymbol, SymbolIndex, SymbolInfo};
use crate::vector::VectorIndex;

/// GraphRAG 配置
#[derive(Debug, Clone)]
pub struct GraphRagConfig {
    /// 搜索返回的最大结果数
    pub top_k: usize,
    /// 向量搜索权重（0.0 ~ 1.0）
    pub vector_weight: f64,
    /// 图遍历最大深度
    pub max_depth: usize,
    /// 是否启用 AST 上下文
    pub enable_ast_context: bool,
}

impl Default for GraphRagConfig {
    fn default() -> Self {
        GraphRagConfig {
            top_k: 10,
            vector_weight: 0.5,
            max_depth: 3,
            enable_ast_context: true,
        }
    }
}

/// GraphRAG 搜索结果
#[derive(Debug, Clone)]
pub struct RagResult {
    pub symbol: String,
    pub file_path: String,
    pub relevance: f64,
    pub context: String,
}

/// GraphRAG 索引
#[derive(Debug, Clone)]
pub struct GraphRagIndex {
    /// 依赖图
    pub dependency_graph: CodeGraph,
    /// 符号索引
    pub symbol_index: SymbolIndex,
    /// 向量索引
    pub vector_index: VectorIndex,
    /// 配置
    config: GraphRagConfig,
}

impl Default for GraphRagIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphRagIndex {
    /// 创建一个新的 GraphRAG 索引
    pub fn new() -> Self {
        GraphRagIndex {
            dependency_graph: CodeGraph::new(),
            symbol_index: SymbolIndex::new(),
            vector_index: VectorIndex::new(),
            config: GraphRagConfig::default(),
        }
    }

    /// 使用自定义配置创建
    pub fn with_config(config: GraphRagConfig) -> Self {
        GraphRagIndex {
            dependency_graph: CodeGraph::new(),
            symbol_index: SymbolIndex::new(),
            vector_index: VectorIndex::new(),
            config,
        }
    }

    /// 构建依赖图
    ///
    /// 根据符号信息创建依赖边（函数调用、类型引用等）。
    /// 这是一个启发式实现，基于符号名称的相似性建立依赖关系。
    pub fn build_dependency_graph(&mut self, symbols: &[SymbolInfo]) {
        // 为每个符号创建节点
        let mut symbol_to_node_id: Vec<(String, usize)> = Vec::new();

        for symbol in symbols {
            let kind = match symbol.kind {
                CodeSymbol::Function => NodeKind::Function,
                CodeSymbol::Class => NodeKind::Class,
                CodeSymbol::Struct => NodeKind::Class,
                CodeSymbol::Interface => NodeKind::Class,
                CodeSymbol::Enum => NodeKind::Class,
                CodeSymbol::Method => NodeKind::Function,
                CodeSymbol::Module => NodeKind::Module,
                CodeSymbol::Variable => NodeKind::Function,
            };
            let node_id = self.dependency_graph.add_node(
                &symbol.name,
                kind,
                &symbol.file_path,
                symbol.line_start,
            );
            symbol_to_node_id.push((symbol.name.clone(), node_id));
        }

        // 建立依赖边：通过名称匹配检测依赖关系
        for (name, from_id) in &symbol_to_node_id {
            for (other_name, to_id) in &symbol_to_node_id {
                if from_id == to_id {
                    continue;
                }
                // 如果符号名包含另一个符号名，可能是函数调用或引用
                if name.contains(other_name) || other_name.contains(name) {
                    self.dependency_graph.add_edge(*from_id, *to_id, EdgeKind::References);
                }
            }
        }

        // 也添加到向量索引
        for symbol in symbols {
            let content = format!("{} {}", symbol.name, symbol.doc_comment.as_deref().unwrap_or(""));
            self.vector_index.add_entry(
                &symbol.name,
                &symbol.file_path,
                &content,
            );
        }
    }

    /// 搜索 GraphRAG 索引
    ///
    /// 结合向量搜索和图遍历，返回相关的符号结果。
    pub fn search(&self, query: &str, top_k: usize) -> Vec<RagResult> {
        let k = top_k.min(self.config.top_k);
        let mut results = Vec::new();

        // 1. 向量搜索
        let vector_results = self.vector_index.search(query, k);
        for vr in &vector_results {
            results.push(RagResult {
                symbol: vr.id.clone(),
                file_path: vr.file_path.clone(),
                relevance: vr.score * self.config.vector_weight,
                context: vr.content.clone(),
            });
        }

        // 2. 符号搜索（通过名称匹配）
        let symbol_results = self.symbol_index.search_by_name(query);
        for sr in &symbol_results {
            let relevance = 0.5; // 基础相关性
            // 避免重复
            if !results.iter().any(|r| r.symbol == sr.name && r.file_path == sr.file_path) {
                results.push(RagResult {
                    symbol: sr.name.clone(),
                    file_path: sr.file_path.clone(),
                    relevance,
                    context: sr.doc_comment.clone().unwrap_or_default(),
                });
            }
        }

        // 3. 图遍历增强
        let graph_weight = 1.0 - self.config.vector_weight;
        let mut graph_results = Vec::new();
        for node in self.dependency_graph.nodes() {
            if node.name.contains(query) || node.name.eq_ignore_ascii_case(query) {
                graph_results.push(RagResult {
                    symbol: node.name.clone(),
                    file_path: node.file_path.clone(),
                    relevance: graph_weight * 0.8,
                    context: String::new(),
                });
            }
        }

        // 合并结果
        for gr in graph_results {
            if !results.iter().any(|r| r.symbol == gr.symbol && r.file_path == gr.file_path) {
                results.push(gr);
            }
        }

        // 按相关性排序
        results.sort_by(|a, b| b.relevance.partial_cmp(&a.relevance).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(k);
        results
    }

    /// 获取 AST 上下文
    ///
    /// 返回指定文件指定行附近的 AST 上下文（这是一个简化实现，
    /// 实际应使用 tree-sitter 或 LSP 服务获取精确 AST）。
    pub fn ast_context(&self, file_path: &str, line: usize) -> String {
        // 查找该文件中包含该行的符号
        let mut context = String::new();

        for symbol in &self.symbol_index.symbols {
            if symbol.file_path == file_path
                && line >= symbol.line_start
                && line <= symbol.line_end
            {
                context.push_str(&format!(
                    "Symbol: {} ({}) [{}:{}]\n",
                    symbol.name,
                    symbol.kind.as_str(),
                    symbol.line_start,
                    symbol.line_end
                ));
                if let Some(doc) = &symbol.doc_comment {
                    context.push_str(&format!("Doc: {}\n", doc));
                }
            }
        }

        if context.is_empty() {
            // 尝试读取文件内容
            if let Ok(content) = std::fs::read_to_string(file_path) {
                let lines: Vec<&str> = content.lines().collect();
                let start = line.saturating_sub(3);
                let end = (line + 2).min(lines.len());
                for i in start..end {
                    if let Some(l) = lines.get(i) {
                        context.push_str(&format!("{:>4}: {}\n", i + 1, l));
                    }
                }
            }
        }

        context
    }

    /// 获取配置引用
    pub fn config(&self) -> &GraphRagConfig {
        &self.config
    }

    /// 更新配置
    pub fn set_config(&mut self, config: GraphRagConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::CodeSymbol;

    fn make_symbol(name: &str, kind: CodeSymbol, file: &str, line: usize) -> SymbolInfo {
        SymbolInfo {
            kind,
            name: name.to_string(),
            file_path: file.to_string(),
            line_start: line,
            line_end: line + 2,
            doc_comment: None,
        }
    }

    #[test]
    fn test_new_index() {
        let index = GraphRagIndex::new();
        assert!(index.dependency_graph.nodes().is_empty());
        assert!(index.symbol_index.symbols().is_empty());
    }

    #[test]
    fn test_build_dependency_graph() {
        let mut index = GraphRagIndex::new();
        let symbols = vec![
            make_symbol("main", CodeSymbol::Function, "main.rs", 1),
            make_symbol("helper", CodeSymbol::Function, "main.rs", 10),
            make_symbol("Config", CodeSymbol::Struct, "config.rs", 1),
        ];

        index.build_dependency_graph(&symbols);
        assert!(!index.dependency_graph.nodes().is_empty());
        assert_eq!(index.dependency_graph.nodes().len(), 3);
    }

    #[test]
    fn test_search() {
        let mut index = GraphRagIndex::new();
        let symbols = vec![
            make_symbol("main", CodeSymbol::Function, "main.rs", 1),
            make_symbol("helper", CodeSymbol::Function, "main.rs", 10),
        ];
        index.build_dependency_graph(&symbols);

        let results = index.search("main", 5);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.symbol == "main"));
    }

    #[test]
    fn test_search_empty() {
        let index = GraphRagIndex::new();
        let results = index.search("nonexistent", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_ast_context() {
        let mut index = GraphRagIndex::new();
        let symbol = SymbolInfo {
            kind: CodeSymbol::Function,
            name: "test_fn".to_string(),
            file_path: "test.rs".to_string(),
            line_start: 1,
            line_end: 5,
            doc_comment: Some("Test function".to_string()),
        };
        index.symbol_index.add_symbol(symbol);

        let context = index.ast_context("test.rs", 3);
        assert!(context.contains("test_fn"));
        assert!(context.contains("Test function"));
    }

    #[test]
    fn test_ast_context_no_match() {
        let index = GraphRagIndex::new();
        let context = index.ast_context("nonexistent.rs", 1);
        // 应该返回空（因为文件不存在）
        assert!(context.is_empty() || context.contains("nonexistent.rs"));
    }

    #[test]
    fn test_config() {
        let config = GraphRagConfig {
            top_k: 5,
            vector_weight: 0.7,
            max_depth: 2,
            enable_ast_context: false,
        };
        let index = GraphRagIndex::with_config(config);
        assert_eq!(index.config().top_k, 5);
        assert_eq!(index.config().vector_weight, 0.7);
    }

    #[test]
    fn test_default_config() {
        let config = GraphRagConfig::default();
        assert_eq!(config.top_k, 10);
        assert_eq!(config.vector_weight, 0.5);
        assert_eq!(config.max_depth, 3);
        assert!(config.enable_ast_context);
    }
}