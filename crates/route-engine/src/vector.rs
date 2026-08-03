//! 向量相似度搜索
//!
//! 提供基于余弦相似度的向量搜索，包含一个简单的基于词频的文本向量化器，
//! 无需外部 ML/AI 依赖。

use std::collections::HashMap;

use crate::embedding::{EmbeddingConfig, HybridVectorizer, VectorizeMode};

/// 向量索引条目
#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub id: String,
    pub file_path: String,
    pub content: String,
    pub vector: Vec<f64>,
}

/// 向量索引
#[derive(Debug, Clone)]
pub struct VectorIndex {
    pub entries: Vec<VectorEntry>,
    /// 混合向量化器（规则匹配 + ML 嵌入）
    pub vectorizer: HybridVectorizer,
}

impl VectorIndex {
    /// 创建一个新的向量索引
    pub fn new() -> Self {
        VectorIndex {
            entries: Vec::new(),
            vectorizer: HybridVectorizer::default(),
        }
    }

    /// 使用自定义配置创建向量索引
    pub fn with_config(config: EmbeddingConfig) -> Self {
        VectorIndex {
            entries: Vec::new(),
            vectorizer: HybridVectorizer::new(config),
        }
    }

    /// 设置向量化模式
    pub fn set_vectorize_mode(&mut self, mode: VectorizeMode) {
        self.vectorizer.set_mode(mode);
    }

    /// 获取向量化模式
    pub fn vectorize_mode(&self) -> VectorizeMode {
        self.vectorizer.mode()
    }

    /// 启用 ML 嵌入
    pub fn enable_ml(&mut self) {
        self.vectorizer.enable_ml();
    }

    /// 禁用 ML 嵌入（纯规则匹配模式）
    pub fn disable_ml(&mut self) {
        self.vectorizer.disable_ml();
    }

    /// 增量学习：用查询结果更新 ML 权重
    pub fn learn(&mut self, query: &str, target: &[f64], positive: bool) {
        self.vectorizer.learn(query, target, positive);
    }

    /// 添加条目到索引
    ///
    /// 自动将文本内容转换为向量并存储。
    pub fn add_entry(&mut self, id: &str, file_path: &str, content: &str) {
        let vector = self.vectorizer.vectorize(content);
        self.entries.push(VectorEntry {
            id: id.to_string(),
            file_path: file_path.to_string(),
            content: content.to_string(),
            vector,
        });
    }

    /// 添加已构建好向量的条目
    pub fn add_entry_with_vector(&mut self, entry: VectorEntry) {
        self.entries.push(entry);
    }

    /// 搜索最相似的条目
    ///
    /// * `query` - 查询文本
    /// * `top_k` - 返回前 k 个结果
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SearchMatch> {
        if self.entries.is_empty() || query.is_empty() {
            return Vec::new();
        }

        let query_vec = self.vectorizer.vectorize(query);
        let mut scores: Vec<(&VectorEntry, f64)> = self
            .entries
            .iter()
            .map(|entry| {
                let sim = cosine_similarity(&query_vec, &entry.vector);
                (entry, sim)
            })
            .collect();

        // 按相似度降序排列
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 取 top_k
        scores
            .into_iter()
            .take(top_k)
            .map(|(entry, score)| SearchMatch {
                id: entry.id.clone(),
                file_path: entry.file_path.clone(),
                content: entry.content.clone(),
                score,
            })
            .collect()
    }

    /// 获取条目数
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// 清空索引
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// 向量搜索结果
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub id: String,
    pub file_path: String,
    pub content: String,
    pub score: f64,
}

/// 计算两个向量的余弦相似度
///
/// 余弦相似度范围：[-1.0, 1.0]，值越大表示越相似。
/// 对于非负向量（如词频向量），范围在 [0.0, 1.0]。
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for i in 0..a.len() {
        dot_product += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator == 0.0 {
        return 0.0;
    }

    dot_product / denominator
}

/// 将文本转换为向量（基于词频的简单向量化器）
///
/// 使用 token 计数构建向量，无需外部 ML 依赖。
/// 对英文按单词分词，对中文按字符（unigram）和双字符（bigram）分词。
pub fn text_to_vector(text: &str) -> Vec<f64> {
    let tokens = tokenize_for_vector(text);
    if tokens.is_empty() {
        return vec![0.0; MAX_FEATURES]; // 返回最小维度向量
    }

    // 构建词频统计
    let mut freq: HashMap<String, usize> = HashMap::new();
    for token in &tokens {
        *freq.entry(token.clone()).or_insert(0) += 1;
    }

    // 取频率最高的前 N 个词作为特征
    const MAX_FEATURES: usize = 64;
    let mut sorted: Vec<(&String, &usize)> = freq.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));

    let mut vector = Vec::with_capacity(MAX_FEATURES);
    for (_, count) in sorted.iter().take(MAX_FEATURES) {
        vector.push(**count as f64);
    }

    // 如果不足 MAX_FEATURES 维，补零
    while vector.len() < MAX_FEATURES {
        vector.push(0.0);
    }

    // 归一化
    let max_val = vector.iter().cloned().fold(0.0_f64, f64::max);
    if max_val > 0.0 {
        for val in vector.iter_mut() {
            *val /= max_val;
        }
    }

    vector
}

/// 分词：对英文按单词分词，对中文按字符和双字符分词
fn tokenize_for_vector(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_ascii_alphanumeric() || ch == '_' {
            let mut word = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                word.push(chars[i]);
                i += 1;
            }
            tokens.push(word.to_lowercase());
        } else if !ch.is_ascii() && ch as u32 > 127 {
            // 中文字符：unigram
            tokens.push(ch.to_string());
            // bigram
            if i + 1 < chars.len() && !chars[i + 1].is_ascii() && chars[i + 1] as u32 > 127 {
                let bigram = format!("{}{}", ch, chars[i + 1]);
                tokens.push(bigram);
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_entry() {
        let mut index = VectorIndex::new();
        index.add_entry("doc1", "file1.rs", "fn main() { println!(\"hello\"); }");
        assert_eq!(index.count(), 1);
    }

    #[test]
    fn test_search_basic() {
        let mut index = VectorIndex::new();
        index.add_entry("doc1", "file1.rs", "hello world");
        index.add_entry("doc2", "file2.rs", "foo bar baz");

        let results = index.search("hello", 5);
        assert!(!results.is_empty());
        // "hello world" 应该比 "foo bar baz" 更相关
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_search_empty_index() {
        let index = VectorIndex::new();
        let results = index.search("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_top_k() {
        let mut index = VectorIndex::new();
        index.add_entry("doc1", "f1.rs", "a");
        index.add_entry("doc2", "f2.rs", "b");
        index.add_entry("doc3", "f3.rs", "c");

        let results = index.search("a", 2);
        assert!(results.len() <= 2);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_cosine_similarity_partial() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn test_text_to_vector_non_empty() {
        let vec = text_to_vector("hello world hello");
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 64);
        // 归一化后最大值应为 1.0
        let max_val = vec.iter().cloned().fold(0.0_f64, f64::max);
        assert!((max_val - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_text_to_vector_empty() {
        let vec = text_to_vector("");
        // 空文本返回 64 维零向量（MAX_FEATURES 常量）
        assert_eq!(vec.len(), 64);
        assert!(vec.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn test_text_to_vector_chinese() {
        let vec = text_to_vector("你好世界");
        assert!(!vec.is_empty());
        assert_eq!(vec.len(), 64);
    }

    #[test]
    fn test_clear() {
        let mut index = VectorIndex::new();
        index.add_entry("doc1", "f1.rs", "hello");
        index.clear();
        assert_eq!(index.count(), 0);
    }

    #[test]
    fn test_add_entry_with_vector() {
        let mut index = VectorIndex::new();
        let entry = VectorEntry {
            id: "doc1".to_string(),
            file_path: "f1.rs".to_string(),
            content: "test".to_string(),
            vector: vec![1.0, 0.0, 0.0],
        };
        index.add_entry_with_vector(entry);
        assert_eq!(index.count(), 1);
    }

    #[test]
    fn test_search_query_empty() {
        let mut index = VectorIndex::new();
        index.add_entry("doc1", "f1.rs", "hello");
        let results = index.search("", 5);
        assert!(results.is_empty());
    }
}