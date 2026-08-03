//! 语义模糊匹配
//!
//! 增强的模糊匹配功能，支持：
//! - 语义级匹配：按实现逻辑而非名字匹配（如 `a + b` → "加法模块"）
//! - 双模式动态切换：精确模式 ↔ 语义模式
//! - 标注系统：为代码符号添加语义标签
//! - RAG 向量化：将函数符号转为向量供检索
//! - AI 联想 → 模块化重构 → RAG 重排 → 向量化 六步循环

use std::collections::HashMap;

use crate::symbol::{CodeSymbol, SymbolInfo};
use crate::vector::{cosine_similarity, text_to_vector};

// ─── 匹配模式 ─────────────────────────────────────────────────────────────

/// 匹配模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// 精确模式：按函数名、文件名精确匹配
    Exact,
    /// 语义模式：按实现逻辑、语义特征匹配（忽略函数名差异）
    Semantic,
    /// 混合模式：先精确再语义，综合评分
    Hybrid,
}

impl Default for MatchMode {
    fn default() -> Self {
        MatchMode::Hybrid
    }
}

impl MatchMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchMode::Exact => "exact",
            MatchMode::Semantic => "semantic",
            MatchMode::Hybrid => "hybrid",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "exact" => MatchMode::Exact,
            "semantic" => MatchMode::Semantic,
            "hybrid" => MatchMode::Hybrid,
            _ => MatchMode::Hybrid,
        }
    }
}

// ─── 语义标注 ─────────────────────────────────────────────────────────────

/// 语义标签 — 描述代码的"做什么"而非"叫什么"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SemanticLabel {
    /// 操作类型（如：add, multiply, transform, validate, parse, format）
    pub operation: String,
    /// 输入类型（如：number, string, list, struct, file）
    pub input: String,
    /// 输出类型
    pub output: String,
    /// 核心操作描述（如："a + b", "a * b", "format!()"）
    pub core_expr: Option<String>,
}

impl SemanticLabel {
    /// 从函数实现中提取语义标签
    pub fn extract(code: &str, fn_name: &str) -> Option<Self> {
        let code_lower = code.to_lowercase();
        let name_lower = fn_name.to_lowercase();

        // 提取核心表达式
        let core_expr = Self::extract_core_expression(code);

        // 推断操作类型
        let operation = if code_lower.contains("+") || code_lower.contains("add") || name_lower.contains("add") || name_lower.contains("sum") || name_lower.contains("plus") {
            "addition".to_string()
        } else if (code_lower.contains('-') && !code_lower.contains("->")) || code_lower.contains("sub") || name_lower.contains("sub") || name_lower.contains("minus") || name_lower.contains("diff") {
            "subtraction".to_string()
        } else if code_lower.contains("*") || code_lower.contains("mul") || name_lower.contains("mul") || name_lower.contains("product") || name_lower.contains("times") {
            "multiplication".to_string()
        } else if code_lower.contains("/") || code_lower.contains("div") || name_lower.contains("div") || name_lower.contains("quotient") {
            "division".to_string()
        } else if code_lower.contains("format") || code_lower.contains("fmt") || name_lower.contains("format") {
            "formatting".to_string()
        } else if code_lower.contains("parse") || name_lower.contains("parse") || code_lower.contains("deserialize") {
            "parsing".to_string()
        } else if code_lower.contains("validate") || name_lower.contains("validate") || code_lower.contains("check") || name_lower.contains("is_valid") {
            "validation".to_string()
        } else if code_lower.contains("transform") || code_lower.contains("convert") || name_lower.contains("to_") {
            "transformation".to_string()
        } else if code_lower.contains("sort") || name_lower.contains("sort") || code_lower.contains("order") {
            "sorting".to_string()
        } else if code_lower.contains("filter") || name_lower.contains("filter") {
            "filtering".to_string()
        } else if code_lower.contains("search") || name_lower.contains("search") || code_lower.contains("find") {
            "searching".to_string()
        } else if code_lower.contains("read") || code_lower.contains("load") || name_lower.contains("read") || name_lower.contains("load") {
            "input".to_string()
        } else if code_lower.contains("write") || code_lower.contains("save") || name_lower.contains("write") || name_lower.contains("save") {
            "output".to_string()
        } else {
            // 默认：从函数名提取
            let clean = name_lower
                .replace("fn_", "")
                .replace("_fn", "")
                .replace("_func", "");
            if clean.len() > 3 { clean } else { "unknown".to_string() }
        };

        // 推断输入/输出类型（简化版）
        let input = if code.contains("i32") || code.contains("i64") || code.contains("u32") || code.contains("usize") {
            "number".to_string()
        } else if code.contains("f32") || code.contains("f64") {
            "float".to_string()
        } else if code.contains("String") || code.contains("&str") {
            "string".to_string()
        } else if code.contains("Vec") || code.contains("vec!") {
            "list".to_string()
        } else if code.contains("HashMap") || code.contains("BTreeMap") {
            "map".to_string()
        } else {
            "unknown".to_string()
        };

        let output = if code.contains("->") {
            // 尝试提取返回类型
            if let Some(ret) = code.split("->").nth(1) {
                let ret = ret.trim().split('{').next().unwrap_or(ret).trim();
                if ret.contains("i32") || ret.contains("i64") || ret.contains("u32") { "number".to_string() }
                else if ret.contains("f32") || ret.contains("f64") { "float".to_string() }
                else if ret.contains("String") || ret.contains("&str") { "string".to_string() }
                else if ret.contains("Vec") { "list".to_string() }
                else if ret.contains("bool") { "boolean".to_string() }
                else if ret.contains("Result") || ret.contains("Option") { "result".to_string() }
                else { "unknown".to_string() }
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };

        Some(SemanticLabel {
            operation,
            input,
            output,
            core_expr,
        })
    }

    /// 从代码中提取核心表达式（如 `a + b`, `format!("{}", x)` 等）
    fn extract_core_expression(code: &str) -> Option<String> {
        // 尝试提取函数体中的核心表达式
        let body = if let Some(body_start) = code.find('{') {
            if let Some(body_end) = code.rfind('}') {
                &code[body_start + 1..body_end]
            } else {
                return None;
            }
        } else {
            return None;
        };

        let body = body.trim();
        if body.is_empty() {
            return None;
        }

        // 去掉 return 关键字
        let body = body.strip_prefix("return ").unwrap_or(body).trim();
        // 去掉尾部分号
        let body = body.strip_suffix(';').unwrap_or(body).trim();
        // 去掉 let 声明
        let body = if let Some(rhs) = body.split('=').nth(1) {
            rhs.trim()
        } else {
            body
        };

        if body.len() > 5 && body.len() < 80 {
            Some(body.to_string())
        } else {
            // 尝试提取单个表达式行
            for line in body.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with("//") && !line.starts_with("/*") {
                    let cleaned = line.strip_prefix("return ").unwrap_or(line).trim();
                    let cleaned = cleaned.strip_suffix(';').unwrap_or(cleaned).trim();
                    if cleaned.len() > 2 && cleaned.len() < 80 && !cleaned.starts_with("let") && !cleaned.starts_with("if") && !cleaned.starts_with("for") && !cleaned.starts_with("while") {
                        return Some(cleaned.to_string());
                    }
                }
            }
            None
        }
    }

    /// 将语义标签转为向量
    pub fn to_vector(&self) -> Vec<f64> {
        let text = format!("{} {} {} {} {:?}", self.operation, self.input, self.output, self.core_expr.as_deref().unwrap_or(""), self.core_expr);
        text_to_vector(&text)
    }

    /// 标签的文本表示
    pub fn to_label_text(&self) -> String {
        format!("[{}]({})→{}", self.operation, self.input, self.output)
    }
}

// ─── 语义索引 ─────────────────────────────────────────────────────────────

/// 语义索引条目
#[derive(Debug, Clone)]
pub struct SemanticEntry {
    /// 原始符号信息
    pub symbol: SymbolInfo,
    /// 原始函数名
    pub original_name: String,
    /// 语义标签（描述"做什么"）
    pub label: SemanticLabel,
    /// 函数实现源码
    pub implementation: String,
    /// 向量表示
    pub vector: Vec<f64>,
    /// 标注（额外注释，由 AI 生成）
    pub annotations: Vec<String>,
}

/// 语义索引
#[derive(Debug, Clone, Default)]
pub struct SemanticIndex {
    /// 所有语义条目
    pub entries: Vec<SemanticEntry>,
    /// 当前匹配模式
    pub mode: MatchMode,
    /// 操作 → 条目 ID 映射（用于快速按操作类型查找）
    operation_index: HashMap<String, Vec<usize>>,
}

impl SemanticIndex {
    /// 创建一个新的语义索引
    pub fn new() -> Self {
        SemanticIndex {
            entries: Vec::new(),
            mode: MatchMode::Hybrid,
            operation_index: HashMap::new(),
        }
    }

    /// 设置匹配模式
    pub fn set_mode(&mut self, mode: MatchMode) {
        self.mode = mode;
    }

    /// 获取当前匹配模式
    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    /// 从符号信息添加条目
    pub fn add_entry(&mut self, symbol: SymbolInfo, implementation: &str) {
        let label = SemanticLabel::extract(implementation, &symbol.name)
            .unwrap_or_else(|| SemanticLabel {
                operation: "unknown".to_string(),
                input: "unknown".to_string(),
                output: "unknown".to_string(),
                core_expr: None,
            });

        let vector = label.to_vector();

        let idx = self.entries.len();
        self.entries.push(SemanticEntry {
            symbol,
            original_name: String::new(),
            label,
            implementation: implementation.to_string(),
            vector,
            annotations: Vec::new(),
        });

        self.operation_index
            .entry(self.entries[idx].label.operation.clone())
            .or_default()
            .push(idx);
    }

    /// 添加条目（带标注）
    pub fn add_entry_with_annotations(
        &mut self,
        symbol: SymbolInfo,
        implementation: &str,
        annotations: Vec<String>,
    ) {
        let idx = self.entries.len();
        self.add_entry(symbol, implementation);
        self.entries[idx].annotations = annotations;
    }

    /// 搜索语义匹配的函数
    ///
    /// 根据当前模式进行匹配：
    /// - Exact: 按函数名精确匹配
    /// - Semantic: 按语义标签匹配
    /// - Hybrid: 综合评分
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SemanticMatch> {
        let query_lower = query.to_lowercase();
        let query_vec = text_to_vector(query);

        let mut scores: Vec<SemanticMatch> = Vec::new();

        for (idx, entry) in self.entries.iter().enumerate() {
            let score = match self.mode {
                MatchMode::Exact => {
                    // 精确模式：按文件名匹配
                    let name_score = if entry.symbol.name.to_lowercase() == query_lower {
                        1.0
                    } else if entry.symbol.name.to_lowercase().contains(&query_lower) {
                        0.6
                    } else {
                        // 回退到语义相似度
                        let label_sim = cosine_similarity(&query_vec, &entry.vector);
                        label_sim * 0.3
                    };
                    name_score
                }
                MatchMode::Semantic => {
                    // 语义模式：按实现逻辑匹配
                    let label_sim = cosine_similarity(&query_vec, &entry.vector);

                    // 额外检查操作类型
                    let op_score = if entry.label.operation.contains(&query_lower) || query_lower.contains(&entry.label.operation) {
                        0.3
                    } else {
                        0.0
                    };

                    // 检查核心表达式是否匹配
                    let expr_score = if let Some(ref core) = entry.label.core_expr {
                        let core_lower = core.to_lowercase();
                        if core_lower.contains(&query_lower) || query_lower.contains(&core_lower) {
                            0.2
                        } else {
                            // 模糊匹配核心表达式中的操作符
                            let query_ops: Vec<char> = query_lower.chars().filter(|c| "+-*/%&|!=".contains(*c)).collect();
                            let core_ops: Vec<char> = core_lower.chars().filter(|c| "+-*/%&|!=".contains(*c)).collect();
                            if !query_ops.is_empty() && query_ops == core_ops {
                                0.15
                            } else {
                                0.0
                            }
                        }
                    } else {
                        0.0
                    };

                    label_sim * 0.5 + op_score + expr_score
                }
                MatchMode::Hybrid => {
                    // 混合模式：综合评分
                    let name_score = if entry.symbol.name.to_lowercase() == query_lower {
                        1.0
                    } else if entry.symbol.name.to_lowercase().contains(&query_lower) {
                        0.5
                    } else {
                        // 检查函数名中的词根
                        let name_tokens: Vec<&str> = entry.symbol.name.split('_').collect();
                        let query_tokens: Vec<&str> = query_lower.split(&['_', ' '][..]).collect();
                        let mut token_match = 0.0f64;
                        for qt in &query_tokens {
                            if name_tokens.iter().any(|nt| nt.contains(qt) || qt.contains(nt)) {
                                token_match += 0.15;
                            }
                        }
                        token_match.min(0.5)
                    };

                    let label_sim = cosine_similarity(&query_vec, &entry.vector);

                    let op_score = if entry.label.operation.contains(&query_lower) || query_lower.contains(&entry.label.operation) {
                        0.2
                    } else {
                        0.0
                    };

                    name_score * 0.4 + label_sim * 0.4 + op_score * 0.2
                }
            };

            if score > 0.05 {
                scores.push(SemanticMatch {
                    idx,
                    symbol_name: entry.symbol.name.clone(),
                    operation: entry.label.operation.clone(),
                    core_expr: entry.label.core_expr.clone(),
                    file_path: entry.symbol.file_path.clone(),
                    line: entry.symbol.line_start,
                    annotation: entry.annotations.first().cloned().unwrap_or_default(),
                    score,
                });
            }
        }

        // 按评分排序
        scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }

    /// 按操作类型获取条目
    pub fn by_operation(&self, operation: &str) -> Vec<&SemanticEntry> {
        self.operation_index
            .get(operation)
            .map(|indices| {
                indices.iter().filter_map(|&i| self.entries.get(i)).collect()
            })
            .unwrap_or_default()
    }

    /// 获取所有操作类型
    pub fn operations(&self) -> Vec<&str> {
        let mut ops: Vec<&str> = self.operation_index.keys().map(|s| s.as_str()).collect();
        ops.sort();
        ops
    }

    /// 获取条目数
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

// ─── 搜索结果 ─────────────────────────────────────────────────────────────

/// 语义匹配结果
#[derive(Debug, Clone)]
pub struct SemanticMatch {
    /// 条目索引
    pub idx: usize,
    /// 符号名称
    pub symbol_name: String,
    /// 操作类型
    pub operation: String,
    /// 核心表达式
    pub core_expr: Option<String>,
    /// 文件路径
    pub file_path: String,
    /// 行号
    pub line: usize,
    /// AI 标注
    pub annotation: String,
    /// 匹配得分
    pub score: f64,
}

// ─── 第三方 AI 数据切分 ────────────────────────────────────────────────────

/// 第三方 AI 数据切分后的代码块
///
/// 模拟外部 AI（如 GPT、Claude）对代码进行 RAG 划分后的结果。
/// 包含语义标签、置信度等信息，用于后续的自有小 AI 向量化。
#[derive(Debug, Clone)]
pub struct ThirdPartyChunk {
    /// 切分后的代码块内容
    pub content: String,
    /// 语义标签（由第三方 AI 标注）
    pub label: SemanticLabel,
    /// 原始符号名称
    pub original_name: String,
    /// 文件路径
    pub file_path: String,
    /// 置信度 (0.0 ~ 1.0)
    pub confidence: f64,
    /// 额外标注（由第三方 AI 生成的自然语言描述）
    pub annotations: Vec<String>,
}

/// 第三方 AI 数据切分器
///
/// 负责：
/// (a) 接收第三方 AI 的 RAG 划分结果
/// (b) 将切分结果注入自有小 AI 进行向量化
/// (c) 向量化后辅助架构设计
#[derive(Debug, Clone, Default)]
pub struct ThirdPartyChunking {
    /// 所有切分后的代码块
    pub chunks: Vec<ThirdPartyChunk>,
    /// 已向量化的块索引
    vectorized_chunks: Vec<usize>,
}

impl ThirdPartyChunking {
    pub fn new() -> Self {
        ThirdPartyChunking {
            chunks: Vec::new(),
            vectorized_chunks: Vec::new(),
        }
    }

    /// 添加第三方 AI 切分结果
    pub fn add_chunk(&mut self, chunk: ThirdPartyChunk) {
        self.chunks.push(chunk);
    }

    /// 批量添加第三方 AI 切分结果
    pub fn add_chunks(&mut self, chunks: Vec<ThirdPartyChunk>) {
        self.chunks.extend(chunks);
    }

    /// 从代码片段模拟第三方 AI 切分
    ///
    /// 根据函数实现自动提取语义标签，模拟第三方 AI 的 RAG 划分。
    /// 实际使用中，应替换为真实第三方 AI 的 API 调用。
    pub fn simulate_chunking(&mut self, code: &str, fn_name: &str, file_path: &str) {
        let label = SemanticLabel::extract(code, fn_name)
            .unwrap_or_else(|| SemanticLabel {
                operation: "unknown".to_string(),
                input: "unknown".to_string(),
                output: "unknown".to_string(),
                core_expr: None,
            });

        // 模拟第三方 AI 的标注
        let annotations = vec![
            format!("AI identified as: {}", label.operation),
            format!("Core expression: {:?}", label.core_expr),
            format!("Data flow: {} → {}", label.input, label.output),
        ];

        self.chunks.push(ThirdPartyChunk {
            content: code.to_string(),
            label,
            original_name: fn_name.to_string(),
            file_path: file_path.to_string(),
            confidence: 0.85, // 模拟置信度
            annotations,
        });
    }

    /// 按置信度重排切分结果
    pub fn rerank_by_confidence(&mut self) {
        self.chunks.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// 获取高置信度切分（> 阈值）
    pub fn high_confidence_chunks(&self, threshold: f64) -> Vec<&ThirdPartyChunk> {
        self.chunks
            .iter()
            .filter(|c| c.confidence >= threshold)
            .collect()
    }

    /// 执行 (a) → (b) 流程：将第三方 AI 切分结果注入自有小 AI 向量化
    ///
    /// 1. 对每个切分块提取语义标签
    /// 2. 将标签转为向量
    /// 3. 存入语义索引
    /// 4. 标记已向量化
    pub fn chunk_rag(&mut self, semantic_index: &mut SemanticIndex) -> usize {
        let mut count = 0;
        for (idx, chunk) in self.chunks.iter().enumerate() {
            if self.vectorized_chunks.contains(&idx) {
                continue;
            }

            let symbol = SymbolInfo {
                kind: CodeSymbol::Function,
                name: chunk.original_name.clone(),
                file_path: chunk.file_path.clone(),
                line_start: 1,
                line_end: 3,
                doc_comment: Some(chunk.annotations.join("\n")),
            };

            // 使用自有小 AI（内置向量化器）创建向量
            semantic_index.add_entry_with_annotations(
                symbol,
                &chunk.content,
                chunk.annotations.clone(),
            );

            self.vectorized_chunks.push(idx);
            count += 1;
        }
        count
    }

    /// 获取已向量化的块数
    pub fn vectorized_count(&self) -> usize {
        self.vectorized_chunks.len()
    }

    /// 获取总块数
    pub fn count(&self) -> usize {
        self.chunks.len()
    }

    /// 清空
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.vectorized_chunks.clear();
    }
}

// ─── 架构设计辅助 ─────────────────────────────────────────────────────────

/// 架构建议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionKind {
    /// 模块划分建议
    ModulePartition,
    /// 接口设计建议
    InterfaceDesign,
    /// 依赖关系建议
    Dependency,
    /// 重构建议
    Refactoring,
}

/// 架构建议
#[derive(Debug, Clone)]
pub struct ArchitectureSuggestion {
    /// 建议类型
    pub kind: SuggestionKind,
    /// 模块名称
    pub module_name: String,
    /// 建议描述
    pub description: String,
    /// 相关操作类型
    pub related_operations: Vec<String>,
    /// 建议包含的文件
    pub suggested_files: Vec<String>,
    /// 置信度
    pub confidence: f64,
}

/// 架构设计辅助器
///
/// 从语义索引/向量索引中提取信息，生成架构设计建议。
/// 第 (c) 步：向量化后的结果辅助架构设计。
#[derive(Debug, Clone, Default)]
pub struct ArchitectureAssistant {
    /// 生成的架构建议
    pub suggestions: Vec<ArchitectureSuggestion>,
}

impl ArchitectureAssistant {
    pub fn new() -> Self {
        ArchitectureAssistant {
            suggestions: Vec::new(),
        }
    }

    /// 从语义索引分析架构
    ///
    /// 根据语义索引中的操作类型分布，生成模块划分建议。
    pub fn analyze(&mut self, semantic_index: &SemanticIndex) -> Vec<ArchitectureSuggestion> {
        self.suggestions.clear();

        let ops = semantic_index.operations();
        let total = semantic_index.count();

        if total == 0 {
            return Vec::new();
        }

        // 为每种操作类型生成模块建议
        for op in &ops {
            let entries = semantic_index.by_operation(op);
            let file_count = entries.len();

            let module_name = format!("{}_module", op);
            let files: Vec<String> = entries
                .iter()
                .map(|e| e.symbol.file_path.clone())
                .filter(|p| !p.is_empty())
                .collect();

            let confidence = (file_count as f64) / (total as f64).max(1.0);

            self.suggestions.push(ArchitectureSuggestion {
                kind: SuggestionKind::ModulePartition,
                module_name: module_name.clone(),
                description: format!(
                    "基于 {} 个 {} 操作函数，建议提取为独立模块",
                    file_count, op
                ),
                related_operations: vec![op.to_string()],
                suggested_files: files,
                confidence: confidence.min(1.0),
            });
        }

        // 如果存在多种操作类型，生成接口设计建议
        if ops.len() > 1 {
            self.suggestions.push(ArchitectureSuggestion {
                kind: SuggestionKind::InterfaceDesign,
                module_name: "service_trait".to_string(),
                description: format!(
                    "检测到 {} 种操作类型（{}），建议抽象为统一 Service Trait",
                    ops.len(),
                    ops.join(", ")
                ),
                related_operations: ops.iter().map(|s| s.to_string()).collect(),
                suggested_files: Vec::new(),
                confidence: 0.7,
            });
        }

        self.suggestions.clone()
    }

    /// 建议模块化划分
    ///
    /// 根据语义相似度，建议将函数分组到不同模块。
    pub fn suggest_modules(
        &self,
        semantic_index: &SemanticIndex,
        min_cluster_size: usize,
    ) -> Vec<ArchitectureSuggestion> {
        let mut module_suggestions = Vec::new();
        let ops = semantic_index.operations();

        for op in &ops {
            let entries = semantic_index.by_operation(op);
            if entries.len() < min_cluster_size {
                continue;
            }

            let files: Vec<String> = entries
                .iter()
                .map(|e| e.symbol.file_path.clone())
                .filter(|p| !p.is_empty())
                .collect();

            let core_exprs: Vec<&str> = entries
                .iter()
                .filter_map(|e| e.label.core_expr.as_deref())
                .collect();

            let desc = if core_exprs.is_empty() {
                format!("所有 {} 操作，建议归入同一模块", op)
            } else {
                format!(
                    "所有 {} 操作（核心表达式：{}），建议归入同一模块",
                    op,
                    core_exprs.join(", ")
                )
            };

            module_suggestions.push(ArchitectureSuggestion {
                kind: SuggestionKind::ModulePartition,
                module_name: format!("{}_module", op),
                description: desc,
                related_operations: vec![op.to_string()],
                suggested_files: files,
                confidence: (entries.len() as f64).min(10.0) / 10.0,
            });
        }

        module_suggestions
    }

    /// 从向量索引生成架构图谱描述
    pub fn to_architecture_graph(&self) -> String {
        if self.suggestions.is_empty() {
            return "No architecture suggestions available.".to_string();
        }

        let mut graph = String::from("## Architecture Design\n\n");
        for s in &self.suggestions {
            let kind_str = match s.kind {
                SuggestionKind::ModulePartition => "📦 Module",
                SuggestionKind::InterfaceDesign => "🔌 Interface",
                SuggestionKind::Dependency => "🔗 Dependency",
                SuggestionKind::Refactoring => "🔄 Refactoring",
            };
            graph.push_str(&format!("### {}: {}\n", kind_str, s.module_name));
            graph.push_str(&format!("- Description: {}\n", s.description));
            graph.push_str(&format!("- Confidence: {:.2}\n", s.confidence));
            if !s.related_operations.is_empty() {
                graph.push_str(&format!("- Operations: {}\n", s.related_operations.join(", ")));
            }
            if !s.suggested_files.is_empty() {
                graph.push_str(&format!("- Files: {}\n", s.suggested_files.join(", ")));
            }
            graph.push('\n');
        }
        graph
    }
}

// ─── RAG 流程 ─────────────────────────────────────────────────────────────

/// RAG 流程状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RagStage {
    /// 第一步：用户写代码
    CodeWriting,
    /// 第二步：AI 联想
    AiAssociation,
    /// 第三步：模块化重构
    ModularRefactoring,
    /// 第四步：RAG 重排
    RagReranking,
    /// 第五步：RAG 向量化
    RagVectorization,
    /// 第六步：回到第一步
    LoopBack,
}

impl RagStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            RagStage::CodeWriting => "code_writing",
            RagStage::AiAssociation => "ai_association",
            RagStage::ModularRefactoring => "modular_refactoring",
            RagStage::RagReranking => "rag_reranking",
            RagStage::RagVectorization => "rag_vectorization",
            RagStage::LoopBack => "loop_back",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            RagStage::CodeWriting => RagStage::AiAssociation,
            RagStage::AiAssociation => RagStage::ModularRefactoring,
            RagStage::ModularRefactoring => RagStage::RagReranking,
            RagStage::RagReranking => RagStage::RagVectorization,
            RagStage::RagVectorization => RagStage::LoopBack,
            RagStage::LoopBack => RagStage::CodeWriting,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            RagStage::CodeWriting => "用户编写代码阶段",
            RagStage::AiAssociation => "AI 联想阶段：根据代码实现进行语义联想",
            RagStage::ModularRefactoring => "模块化重构阶段：按函数模块重组代码",
            RagStage::RagReranking => "RAG 重排阶段：对联想结果进行相关性重排",
            RagStage::RagVectorization => "RAG 向量化阶段：将结果向量化存入索引",
            RagStage::LoopBack => "循环：回到编写代码阶段",
        }
    }
}

/// RAG 流程控制器
#[derive(Debug, Clone)]
pub struct RagPipeline {
    /// 当前阶段
    pub stage: RagStage,
    /// 条目总数
    pub total_entries: usize,
    /// 是否启用 AI 联想
    pub enable_association: bool,
    /// 是否启用重排
    pub enable_reranking: bool,
}

impl Default for RagPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl RagPipeline {
    pub fn new() -> Self {
        RagPipeline {
            stage: RagStage::CodeWriting,
            total_entries: 0,
            enable_association: true,
            enable_reranking: true,
        }
    }

    /// 推进到下一阶段
    pub fn advance(&mut self) -> RagStage {
        self.stage = self.stage.next();
        self.stage
    }

    /// 执行 AI 联想
    ///
    /// 根据代码实现内容进行语义联想，匹配到相应的操作类型。
    /// 联想结果 = 所有语义相似的操作模块
    pub fn associate(&self, semantic_index: &SemanticIndex, code: &str, top_k: usize) -> Vec<SemanticMatch> {
        semantic_index.search(code, top_k)
    }

    /// 执行模块化重构
    ///
    /// 将联想结果按操作类型分组，形成模块化结构。
    pub fn modular_refactor(&self, matches: &[SemanticMatch], _semantic_index: &SemanticIndex) -> HashMap<String, Vec<SemanticMatch>> {
        let mut modules: HashMap<String, Vec<SemanticMatch>> = HashMap::new();
        for m in matches {
            modules.entry(m.operation.clone()).or_default().push(m.clone());
        }
        modules
    }

    /// 执行 RAG 重排
    ///
    /// 对模块化结果进行按相关性重排，提高匹配质量。
    pub fn rerank(&self, modules: &mut HashMap<String, Vec<SemanticMatch>>, query: &str) {
        let query_lower = query.to_lowercase();
        for (_op, items) in modules.iter_mut() {
            // 对每个模块内的结果按评分重排
            items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

            // 额外加权：如果查询包含模块名，提升该模块所有结果
            if !query_lower.is_empty() {
                for item in items.iter_mut() {
                    if item.operation.contains(&query_lower) || query_lower.contains(&item.operation) {
                        item.score = (item.score + 0.1).min(1.0);
                    }
                }
            }
        }
    }

    /// 执行 RAG 向量化
    ///
    /// 将重排后的结果向量化，存入语义索引。
    pub fn vectorize(&mut self, semantic_index: &mut SemanticIndex, symbol: SymbolInfo, implementation: &str) {
        semantic_index.add_entry(symbol, implementation);
        self.total_entries = semantic_index.count();
    }

    /// 执行完整六步循环
    ///
    /// 从用户代码开始，走完 AI 联想 → 模块化重构 → RAG 重排 → 向量化 → 回到起点
    pub fn run_cycle(
        &mut self,
        semantic_index: &mut SemanticIndex,
        code: &str,
        symbol: SymbolInfo,
        implementation: &str,
        top_k: usize,
    ) -> HashMap<String, Vec<SemanticMatch>> {
        // Step 1: 用户写代码（已隐含在调用中）
        self.stage = RagStage::CodeWriting;

        // Step 2: AI 联想
        self.stage = RagStage::AiAssociation;
        let associations = self.associate(semantic_index, code, top_k);

        // Step 3: 模块化重构
        self.stage = RagStage::ModularRefactoring;
        let mut modules = self.modular_refactor(&associations, semantic_index);

        // Step 4: RAG 重排
        self.stage = RagStage::RagReranking;
        self.rerank(&mut modules, code);

        // Step 5: RAG 向量化
        self.stage = RagStage::RagVectorization;
        self.vectorize(semantic_index, symbol, implementation);

        // Step 6: 循环回到起点
        self.stage = RagStage::LoopBack;
        self.advance(); // 回到 CodeWriting

        modules
    }

    /// 带第三方 AI 数据切分的完整流程
    ///
    /// 完整执行 (a) → (b) → (c) 流程：
    /// (a) 第三方 AI 数据切分 → 初步重排
    /// (b) 自有小 AI 向量化 → 注入语义索引
    /// (c) 架构设计辅助
    /// 然后走完 ②→⑥ 循环
    pub fn run_cycle_with_chunking(
        &mut self,
        chunking: &mut ThirdPartyChunking,
        semantic_index: &mut SemanticIndex,
        architect: &mut ArchitectureAssistant,
        code: &str,
        symbol: SymbolInfo,
        implementation: &str,
        top_k: usize,
    ) -> RunCycleResult {
        // (a) 第三方 AI 数据切分
        self.stage = RagStage::CodeWriting;
        chunking.simulate_chunking(implementation, &symbol.name, &symbol.file_path);
        chunking.rerank_by_confidence();

        // 在可变借用前先收集高置信度计数
        let high_conf_count = chunking.high_confidence_chunks(0.7).len();

        // (b) 自有小 AI 向量化
        let vectorized = chunking.chunk_rag(semantic_index);

        // Step 2: AI 联想
        self.stage = RagStage::AiAssociation;
        let associations = self.associate(semantic_index, code, top_k);

        // Step 3: 模块化重构
        self.stage = RagStage::ModularRefactoring;
        let mut modules = self.modular_refactor(&associations, semantic_index);

        // Step 4: RAG 重排
        self.stage = RagStage::RagReranking;
        self.rerank(&mut modules, code);

        // Step 5: RAG 向量化
        self.stage = RagStage::RagVectorization;
        self.vectorize(semantic_index, symbol, implementation);

        // (c) 架构设计辅助
        let architecture_suggestions = architect.analyze(semantic_index);

        // Step 6: 循环
        self.stage = RagStage::LoopBack;
        self.advance();

        RunCycleResult {
            modules,
            architectures: architecture_suggestions,
            high_confidence_chunks: high_conf_count,
            total_chunks: chunking.count(),
            vectorized_chunks: vectorized,
            total_entries: semantic_index.count(),
        }
    }
}

/// 完整循环执行结果
#[derive(Debug, Clone)]
pub struct RunCycleResult {
    /// 模块化重构结果
    pub modules: HashMap<String, Vec<SemanticMatch>>,
    /// 架构设计建议
    pub architectures: Vec<ArchitectureSuggestion>,
    /// 高置信度切分块数
    pub high_confidence_chunks: usize,
    /// 总切分块数
    pub total_chunks: usize,
    /// 已向量化块数
    pub vectorized_chunks: usize,
    /// 语义索引总条目数
    pub total_entries: usize,
}

// ─── 双模式切换器 ─────────────────────────────────────────────────────────

/// 双模式切换器
#[derive(Debug, Clone)]
pub struct ModeSwitcher {
    /// 当前模式
    pub current: MatchMode,
    /// 切换历史
    pub history: Vec<(MatchMode, String)>,
}

impl ModeSwitcher {
    pub fn new() -> Self {
        ModeSwitcher {
            current: MatchMode::Hybrid,
            history: Vec::new(),
        }
    }

    /// 切换到指定模式
    pub fn switch_to(&mut self, mode: MatchMode, reason: &str) {
        self.history.push((self.current, format!("切换到 {}: {}", mode.as_str(), reason)));
        self.current = mode;
    }

    /// 自动切换模式
    ///
    /// 根据查询特征自动选择最佳模式：
    /// - 如果查询包含操作符（+、-、*、/）→ 切换到语义模式
    /// - 如果查询是精确标识符 → 切换到精确模式
    /// - 默认 → 混合模式
    pub fn auto_switch(&mut self, query: &str) -> MatchMode {
        let has_operators = query.contains('+') || query.contains('-') || query.contains('*') || query.contains('/');
        let is_identifier = query.chars().all(|c| c.is_alphanumeric() || c == '_');

        let new_mode = if has_operators {
            MatchMode::Semantic
        } else if is_identifier && query.len() < 50 {
            MatchMode::Exact
        } else {
            MatchMode::Hybrid
        };

        if new_mode != self.current {
            self.switch_to(new_mode, &format!("auto-switch (query={})", query));
        }

        new_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::CodeSymbol;

    #[test]
    fn test_match_mode_from_str() {
        assert_eq!(MatchMode::from_str("exact"), MatchMode::Exact);
        assert_eq!(MatchMode::from_str("semantic"), MatchMode::Semantic);
        assert_eq!(MatchMode::from_str("hybrid"), MatchMode::Hybrid);
        assert_eq!(MatchMode::from_str("unknown"), MatchMode::Hybrid);
    }

    #[test]
    fn test_semantic_label_extract_addition() {
        let code = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let label = SemanticLabel::extract(code, "add").unwrap();
        assert_eq!(label.operation, "addition");
        assert_eq!(label.input, "number");
        assert_eq!(label.output, "number");
    }

    #[test]
    fn test_semantic_label_extract_core_expr() {
        let code = "fn multiply(x: f64, y: f64) -> f64 { x * y }";
        let label = SemanticLabel::extract(code, "multiply").unwrap();
        assert_eq!(label.operation, "multiplication");
        assert!(label.core_expr.is_some());
        assert!(label.core_expr.as_deref().unwrap().contains('*'));
    }

    #[test]
    fn test_semantic_label_extract_formatting() {
        let code = "fn fmt(msg: &str) -> String { format!(\"Hello, {}\", msg) }";
        let label = SemanticLabel::extract(code, "fmt").unwrap();
        assert_eq!(label.operation, "formatting");
    }

    #[test]
    fn test_semantic_index_add_and_search() {
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Semantic);

        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "my_custom_adder".to_string(),
                file_path: "lib.rs".to_string(),
                line_start: 1,
                line_end: 3,
                doc_comment: None,
            },
            "fn my_custom_adder(a: i32, b: i32) -> i32 { a + b }",
        );

        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "format_string".to_string(),
                file_path: "fmt.rs".to_string(),
                line_start: 5,
                line_end: 7,
                doc_comment: None,
            },
            "fn format_string(msg: &str) -> String { format!(\"Hello, {}\", msg) }",
        );

        // 语义搜索："加法" 应该匹配到 my_custom_adder
        let results = index.search("加法", 5);
        assert!(!results.is_empty());
        let top = results.first().unwrap();
        assert_eq!(top.operation, "addition");
    }

    #[test]
    fn test_semantic_index_search_by_operator() {
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Semantic);

        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "do_add".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 10,
                line_end: 12,
                doc_comment: None,
            },
            "fn do_add(x: i32, y: i32) -> i32 { x + y }",
        );

        // 搜索 "a + b" 应该匹配到加法操作
        let results = index.search("a + b", 5);
        assert!(!results.is_empty());
        let top = results.first().unwrap();
        assert_eq!(top.operation, "addition");
    }

    #[test]
    fn test_hybrid_mode_prioritizes_name() {
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Hybrid);

        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 1,
                line_end: 3,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );

        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "subtract".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 5,
                line_end: 7,
                doc_comment: None,
            },
            "fn subtract(a: i32, b: i32) -> i32 { a - b }",
        );

        // 精确搜索 "add" 应该优先匹配到 add 函数
        let results = index.search("add", 5);
        assert!(!results.is_empty());
        assert_eq!(results.first().unwrap().symbol_name, "add");
    }

    #[test]
    fn test_mode_switcher() {
        let mut switcher = ModeSwitcher::new();
        assert_eq!(switcher.current, MatchMode::Hybrid);

        // 自动切换到语义模式
        let mode = switcher.auto_switch("a + b");
        assert_eq!(mode, MatchMode::Semantic);
        assert_eq!(switcher.history.len(), 1);

        // 自动切换到精确模式
        let mode = switcher.auto_switch("my_function");
        assert_eq!(mode, MatchMode::Exact);
    }

    #[test]
    fn test_rag_pipeline_cycle() {
        let mut pipeline = RagPipeline::new();
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Semantic);

        // 先添加一些已有条目
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 1,
                line_end: 3,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );

        // 运行六步循环
        let new_code = "fn my_sum(x: i32, y: i32) -> i32 { x + y }";
        let new_symbol = SymbolInfo {
            kind: CodeSymbol::Function,
            name: "my_sum".to_string(),
            file_path: "lib.rs".to_string(),
            line_start: 10,
            line_end: 12,
            doc_comment: None,
        };

        let modules = pipeline.run_cycle(&mut index, new_code, new_symbol, new_code, 5);

        // 验证循环流程
        assert_eq!(pipeline.stage, RagStage::CodeWriting);

        // 验证条目已添加
        assert_eq!(index.count(), 2);

        // 验证模块化结果
        assert!(modules.contains_key("addition"));
    }

    #[test]
    fn test_label_to_vector() {
        let label = SemanticLabel {
            operation: "addition".to_string(),
            input: "number".to_string(),
            output: "number".to_string(),
            core_expr: Some("a + b".to_string()),
        };
        let vec = label.to_vector();
        assert_eq!(vec.len(), 64);
        // 应该非零
        assert!(vec.iter().any(|&v| v > 0.0));
    }

    #[test]
    fn test_by_operation() {
        let mut index = SemanticIndex::new();
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "m.rs".to_string(),
                line_start: 1,
                line_end: 1,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "sub".to_string(),
                file_path: "m.rs".to_string(),
                line_start: 3,
                line_end: 3,
                doc_comment: None,
            },
            "fn sub(a: i32, b: i32) -> i32 { a - b }",
        );

        let add_entries = index.by_operation("addition");
        assert_eq!(add_entries.len(), 1);
        assert_eq!(add_entries[0].symbol.name, "add");
    }

    #[test]
    fn test_operations_list() {
        let mut index = SemanticIndex::new();
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "m.rs".to_string(),
                line_start: 1,
                line_end: 1,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );

        let ops = index.operations();
        assert!(ops.contains(&"addition"));
    }

    // ─── 第三方 AI 切分测试 ──────────────────────────────────────────────

    #[test]
    fn test_third_party_chunking_new() {
        let chunking = ThirdPartyChunking::new();
        assert_eq!(chunking.count(), 0);
        assert_eq!(chunking.vectorized_count(), 0);
    }

    #[test]
    fn test_third_party_chunking_simulate() {
        let mut chunking = ThirdPartyChunking::new();
        chunking.simulate_chunking("fn add(a: i32, b: i32) -> i32 { a + b }", "add", "math.rs");
        assert_eq!(chunking.count(), 1);
        assert_eq!(chunking.chunks[0].label.operation, "addition");
    }

    #[test]
    fn test_third_party_chunking_rerank() {
        let mut chunking = ThirdPartyChunking::new();
        chunking.add_chunk(ThirdPartyChunk {
            content: "fn foo() {}".to_string(),
            label: SemanticLabel {
                operation: "unknown".to_string(),
                input: "unknown".to_string(),
                output: "unknown".to_string(),
                core_expr: None,
            },
            original_name: "foo".to_string(),
            file_path: "f.rs".to_string(),
            confidence: 0.3,
            annotations: vec![],
        });
        chunking.add_chunk(ThirdPartyChunk {
            content: "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            label: SemanticLabel {
                operation: "addition".to_string(),
                input: "number".to_string(),
                output: "number".to_string(),
                core_expr: Some("a + b".to_string()),
            },
            original_name: "add".to_string(),
            file_path: "math.rs".to_string(),
            confidence: 0.9,
            annotations: vec!["AI identified as: addition".to_string()],
        });

        chunking.rerank_by_confidence();
        // 高置信度排在前面
        assert_eq!(chunking.chunks[0].confidence, 0.9);
        assert_eq!(chunking.chunks[0].original_name, "add");

        let high = chunking.high_confidence_chunks(0.7);
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].original_name, "add");
    }

    #[test]
    fn test_third_party_chunking_chunk_rag() {
        let mut chunking = ThirdPartyChunking::new();
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Semantic);

        chunking.simulate_chunking(
            "fn my_adder(a: i32, b: i32) -> i32 { a + b }",
            "my_adder",
            "lib.rs",
        );

        let count = chunking.chunk_rag(&mut index);
        assert_eq!(count, 1);
        assert_eq!(index.count(), 1);
        // 再次调用不应重复向量化
        let count2 = chunking.chunk_rag(&mut index);
        assert_eq!(count2, 0);
        assert_eq!(index.count(), 1);
    }

    // ─── 架构设计辅助测试 ────────────────────────────────────────────────

    #[test]
    fn test_architecture_assistant_new() {
        let architect = ArchitectureAssistant::new();
        assert!(architect.suggestions.is_empty());
    }

    #[test]
    fn test_architecture_assistant_analyze() {
        let mut index = SemanticIndex::new();
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 1,
                line_end: 3,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "sub".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 5,
                line_end: 7,
                doc_comment: None,
            },
            "fn sub(a: i32, b: i32) -> i32 { a - b }",
        );
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "fmt_msg".to_string(),
                file_path: "fmt.rs".to_string(),
                line_start: 10,
                line_end: 12,
                doc_comment: None,
            },
            "fn fmt_msg(msg: &str) -> String { format!(\"Hello, {}\", msg) }",
        );

        let mut architect = ArchitectureAssistant::new();
        let suggestions = architect.analyze(&index);

        // 应该有 3 个模块建议（addition, subtraction, formatting）+ 1 个接口设计建议
        assert!(suggestions.len() >= 3);
        // 验证包含接口设计建议（多种操作类型）
        let has_interface = suggestions.iter().any(|s| s.kind == SuggestionKind::InterfaceDesign);
        assert!(has_interface);
    }

    #[test]
    fn test_architecture_assistant_suggest_modules() {
        let mut index = SemanticIndex::new();
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 1,
                line_end: 3,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "sum".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 5,
                line_end: 7,
                doc_comment: None,
            },
            "fn sum(a: i64, b: i64) -> i64 { a + b }",
        );

        let architect = ArchitectureAssistant::new();
        let modules = architect.suggest_modules(&index, 1);
        assert!(modules.len() >= 1);
        assert!(modules.iter().any(|m| m.related_operations.contains(&"addition".to_string())));
    }

    #[test]
    fn test_architecture_assistant_empty() {
        let index = SemanticIndex::new();
        let mut architect = ArchitectureAssistant::new();
        let suggestions = architect.analyze(&index);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_architecture_assistant_to_graph() {
        let mut architect = ArchitectureAssistant::new();
        // 空时返回提示
        let graph = architect.to_architecture_graph();
        assert!(graph.contains("No architecture suggestions"));

        // 添加建议后生成图谱
        architect.suggestions.push(ArchitectureSuggestion {
            kind: SuggestionKind::ModulePartition,
            module_name: "addition_module".to_string(),
            description: "Test module".to_string(),
            related_operations: vec!["addition".to_string()],
            suggested_files: vec!["math.rs".to_string()],
            confidence: 0.9,
        });
        let graph = architect.to_architecture_graph();
        assert!(graph.contains("addition_module"));
        assert!(graph.contains("addition"));
    }

    // ─── 增强 RAG 流程测试 ──────────────────────────────────────────────

    #[test]
    fn test_rag_pipeline_with_chunking() {
        let mut pipeline = RagPipeline::new();
        let mut chunking = ThirdPartyChunking::new();
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Semantic);
        let mut architect = ArchitectureAssistant::new();

        // 先添加已有条目供联想
        index.add_entry(
            SymbolInfo {
                kind: CodeSymbol::Function,
                name: "add".to_string(),
                file_path: "math.rs".to_string(),
                line_start: 1,
                line_end: 3,
                doc_comment: None,
            },
            "fn add(a: i32, b: i32) -> i32 { a + b }",
        );

        let new_code = "fn my_sum(x: i32, y: i32) -> i32 { x + y }";
        let new_symbol = SymbolInfo {
            kind: CodeSymbol::Function,
            name: "my_sum".to_string(),
            file_path: "lib.rs".to_string(),
            line_start: 10,
            line_end: 12,
            doc_comment: None,
        };

        let result = pipeline.run_cycle_with_chunking(
            &mut chunking, &mut index, &mut architect,
            new_code, new_symbol, new_code, 5,
        );

        // 验证流程结果
        assert_eq!(pipeline.stage, RagStage::CodeWriting);
        assert!(result.total_chunks > 0);
        assert!(result.vectorized_chunks > 0);
        assert!(result.total_entries > 1);
        // 应该有架构建议
        assert!(!result.architectures.is_empty());
    }

    #[test]
    fn test_run_cycle_result_struct() {
        let result = RunCycleResult {
            modules: HashMap::new(),
            architectures: vec![],
            high_confidence_chunks: 0,
            total_chunks: 5,
            vectorized_chunks: 3,
            total_entries: 10,
        };
        assert_eq!(result.total_chunks, 5);
        assert_eq!(result.vectorized_chunks, 3);
        assert_eq!(result.total_entries, 10);
    }
}