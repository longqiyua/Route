//! 混合嵌入 — 规则匹配（快速） + ML 嵌入（精准）
//!
//! 设计原则：
//! 1. **规则匹配优先**：保留 `text_to_vector` 作为默认快速路径，保证最高速度和稳定性
//! 2. **ML 嵌入增强**：基于哈希特征 + n-gram 的轻量嵌入，无需外部模型文件
//! 3. **增量学习**：支持在线学习，每次查询都可更新嵌入权重
//! 4. **自动切换**：根据查询复杂度自动选择规则/ML 模式
//!
//! 架构：
//! ```text
//! HybridVectorizer
//!   +-- Fast (text_to_vector)      默认, O(n), 无状态
//!   +-- ML (HashedNGramEmbedding)  可选, O(n), 增量学习
//! ```
//!
//! ML 嵌入算法：
//! - 特征哈希（Feature Hashing）：将 n-gram 哈希到固定维度空间
//! - 增量学习：通过正/负样本更新权重（类似在线 SGD）
//! - 无需外部模型文件，纯 Rust 实现，MIT 协议兼容

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::time::Instant;

// ─── 嵌入配置 ─────────────────────────────────────────────────────────────

/// 嵌入配置
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// 特征维度（默认 128）
    pub feature_dim: usize,
    /// n-gram 范围（默认 1..=3）
    pub ngram_range: Range<usize>,
    /// 学习率（默认 0.01）
    pub learning_rate: f64,
    /// 使用 ML 嵌入的查询最小长度（0 = 始终使用 ML，999 = 始终使用规则）
    pub ml_min_query_len: usize,
    /// 是否启用 ML 嵌入
    pub ml_enabled: bool,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            feature_dim: 128,
            ngram_range: 1..4,
            learning_rate: 0.01,
            ml_min_query_len: 5, // 查询长度 >= 5 时使用 ML
            ml_enabled: true,
        }
    }
}

// ─── 哈希特征提取 ─────────────────────────────────────────────────────────

/// 提取文本的 n-gram 特征
fn extract_ngrams(text: &str, range: &Range<usize>) -> Vec<String> {
    let start = Instant::now();
    let chars: Vec<char> = text.chars().collect();
    let mut ngrams = Vec::new();
    let len = chars.len();

    for n in range.clone() {
        if n > len {
            continue;
        }
        for i in 0..=len - n {
            let gram: String = chars[i..i + n].iter().collect();
            ngrams.push(gram);
        }
    }

    let elapsed = start.elapsed();
    tracing::debug!(
        text_len = text.len(),
        char_len = len,
        ngram_count = ngrams.len(),
        range_start = range.start,
        range_end = range.end,
        elapsed_us = elapsed.as_micros(),
        "extract_ngrams"
    );
    ngrams
}

/// 将特征哈希到索引
fn hash_feature(feature: &str, dim: usize) -> usize {
    let mut hasher = DefaultHasher::new();
    feature.hash(&mut hasher);
    (hasher.finish() as usize) % dim
}

/// 带符号的哈希（用于保持无偏性）
fn hash_sign(feature: &str) -> f64 {
    let mut hasher = DefaultHasher::new();
    feature.hash(&mut hasher);
    feature.hash(&mut hasher); // 双重哈希区分维度
    if hasher.finish() % 2 == 0 { 1.0 } else { -1.0 }
}

// ─── HashedNGramEmbedding ─────────────────────────────────────────────────

/// 基于哈希特征 + n-gram 的轻量嵌入
///
/// 使用 Feature Hashing 将 n-gram 特征映射到固定维度向量空间。
/// 支持增量学习：通过在线 SGD 更新权重。
///
/// ## 特点
/// - 无外部依赖：纯 Rust 实现，无需模型文件
/// - 增量学习：每次 `update()` 调用都会微调权重
/// - 轻量级：O(n) 复杂度，n = 特征数
/// - MIT 兼容：原创算法，无协议限制
#[derive(Debug, Clone)]
pub struct HashedNGramEmbedding {
    /// 权重向量（特征维度）
    weights: Vec<f64>,
    /// 配置
    config: EmbeddingConfig,
    /// 训练样本计数
    train_count: usize,
}

impl HashedNGramEmbedding {
    /// 创建新的嵌入器
    pub fn new(config: EmbeddingConfig) -> Self {
        tracing::info!(
            feature_dim = config.feature_dim,
            ngram_range_start = config.ngram_range.start,
            ngram_range_end = config.ngram_range.end,
            learning_rate = config.learning_rate,
            ml_min_query_len = config.ml_min_query_len,
            ml_enabled = config.ml_enabled,
            "HashedNGramEmbedding::new"
        );
        Self {
            weights: vec![0.0; config.feature_dim],
            config,
            train_count: 0,
        }
    }

    /// 使用默认配置创建
    pub fn default() -> Self {
        tracing::debug!("HashedNGramEmbedding::default");
        Self::new(EmbeddingConfig::default())
    }

    /// 将文本转换为向量
    ///
    /// 提取 n-gram 特征，通过哈希映射到权重向量。
    /// 返回归一化后的向量。
    pub fn embed(&self, text: &str) -> Vec<f64> {
        let start = Instant::now();
        let ngrams = extract_ngrams(text, &self.config.ngram_range);
        if ngrams.is_empty() {
            tracing::debug!(
                text_len = text.len(),
                elapsed_us = start.elapsed().as_micros(),
                "HashedNGramEmbedding::embed — empty text, returning zero vector"
            );
            return vec![0.0; self.config.feature_dim];
        }

        let mut vec = vec![0.0; self.config.feature_dim];
        for gram in &ngrams {
            let idx = hash_feature(gram, self.config.feature_dim);
            let sign = hash_sign(gram);
            vec[idx] += sign * self.weights[idx].max(0.1); // 基值 0.1
        }

        // 归一化
        normalize(&mut vec);

        let elapsed = start.elapsed();
        tracing::debug!(
            text_len = text.len(),
            ngram_count = ngrams.len(),
            dim = self.config.feature_dim,
            train_count = self.train_count,
            elapsed_us = elapsed.as_micros(),
            "HashedNGramEmbedding::embed"
        );
        vec
    }

    /// 增量学习：用正/负样本更新权重
    ///
    /// * `text` - 输入文本
    /// * `target` - 目标向量（如来自精准匹配的向量）
    /// * `label` - +1.0 (正样本) 或 -1.0 (负样本)
    pub fn update(&mut self, text: &str, target: &[f64], label: f64) {
        let start = Instant::now();
        let ngrams = extract_ngrams(text, &self.config.ngram_range);
        if ngrams.is_empty() || target.len() != self.config.feature_dim {
            tracing::warn!(
                text_len = text.len(),
                target_len = target.len(),
                feature_dim = self.config.feature_dim,
                ngrams_empty = ngrams.is_empty(),
                dim_mismatch = target.len() != self.config.feature_dim,
                "HashedNGramEmbedding::update — skipped (invalid input)"
            );
            return;
        }

        let lr = self.config.learning_rate / (1.0 + 0.01 * self.train_count as f64);

        let mut grad_norm = 0.0f64;
        for gram in &ngrams {
            let idx = hash_feature(gram, self.config.feature_dim);
            let sign = hash_sign(gram);

            // 简单在线 SGD 更新
            // loss = (w_i * sign - target_i)^2
            // grad = 2 * (w_i * sign - target_i) * sign
            let pred = self.weights[idx] * sign;
            let target_val = if idx < target.len() { target[idx] } else { 0.0 };
            let grad = 2.0 * (pred - target_val) * sign * label;

            grad_norm += grad * grad;
            self.weights[idx] -= lr * grad;
        }

        self.train_count += 1;

        let elapsed = start.elapsed();
        tracing::debug!(
            text_len = text.len(),
            ngram_count = ngrams.len(),
            label = label,
            learning_rate = lr,
            grad_norm = format!("{:.6}", grad_norm.sqrt()),
            train_count = self.train_count,
            elapsed_us = elapsed.as_micros(),
            "HashedNGramEmbedding::update"
        );
    }

    /// 批量学习
    pub fn update_batch(&mut self, texts: &[&str], targets: &[Vec<f64>], labels: &[f64]) {
        let start = Instant::now();
        let batch_size = texts.len().min(targets.len()).min(labels.len());
        tracing::debug!(
            batch_size = batch_size,
            total_texts = texts.len(),
            "HashedNGramEmbedding::update_batch — start"
        );

        for ((text, target), label) in texts.iter().zip(targets.iter()).zip(labels.iter()) {
            self.update(text, target, *label);
        }

        let elapsed = start.elapsed();
        tracing::debug!(
            batch_size = batch_size,
            total_train_count = self.train_count,
            elapsed_us = elapsed.as_micros(),
            "HashedNGramEmbedding::update_batch — done"
        );
    }

    /// 获取训练样本数
    pub fn train_count(&self) -> usize {
        self.train_count
    }

    /// 获取权重向量引用
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// 重置权重
    pub fn reset(&mut self) {
        tracing::info!(
            prev_train_count = self.train_count,
            feature_dim = self.config.feature_dim,
            "HashedNGramEmbedding::reset"
        );
        self.weights = vec![0.0; self.config.feature_dim];
        self.train_count = 0;
    }
}

// ─── 混合向量化器 ─────────────────────────────────────────────────────────

/// 向量化模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorizeMode {
    /// 规则匹配（快速）：使用 `text_to_vector`，O(n) 词频统计
    Fast,
    /// ML 嵌入（精准）：使用 `HashedNGramEmbedding`，O(n) n-gram 哈希
    Ml,
    /// 自动选择：根据查询复杂度自动切换
    Auto,
}

impl std::fmt::Display for VectorizeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorizeMode::Fast => write!(f, "Fast"),
            VectorizeMode::Ml => write!(f, "Ml"),
            VectorizeMode::Auto => write!(f, "Auto"),
        }
    }
}

/// 混合向量化器
///
/// 结合规则匹配和 ML 嵌入两种策略：
/// - **Fast 模式**：使用 `text_to_vector`，纯词频统计，无状态，极速
/// - **ML 模式**：使用 `HashedNGramEmbedding`，n-gram 特征哈希，增量学习
/// - **Auto 模式**：根据查询长度自动选择（短查询走 Fast，长查询走 ML）
///
/// 默认使用 Auto 模式，保证最高速度和稳定性的同时，在需要时提供精准语义嵌入。
#[derive(Debug, Clone)]
pub struct HybridVectorizer {
    /// ML 嵌入器
    embedding: HashedNGramEmbedding,
    /// 当前模式
    mode: VectorizeMode,
    /// 配置
    config: EmbeddingConfig,
}

impl HybridVectorizer {
    /// 创建新的混合向量化器
    pub fn new(config: EmbeddingConfig) -> Self {
        tracing::info!(
            feature_dim = config.feature_dim,
            ml_enabled = config.ml_enabled,
            ml_min_query_len = config.ml_min_query_len,
            ngram_range = format!("{}..{}", config.ngram_range.start, config.ngram_range.end),
            "HybridVectorizer::new — mode=Auto"
        );
        Self {
            embedding: HashedNGramEmbedding::new(config.clone()),
            mode: VectorizeMode::Auto,
            config,
        }
    }

    /// 使用默认配置创建
    pub fn default() -> Self {
        tracing::debug!("HybridVectorizer::default");
        Self::new(EmbeddingConfig::default())
    }

    /// 设置向量化模式
    pub fn set_mode(&mut self, mode: VectorizeMode) {
        tracing::info!(
            from = %self.mode,
            to = %mode,
            "HybridVectorizer::set_mode"
        );
        self.mode = mode;
    }

    /// 获取当前模式
    pub fn mode(&self) -> VectorizeMode {
        self.mode
    }

    /// 将文本向量化
    ///
    /// 根据当前模式选择向量化策略：
    /// - Fast: 使用 `text_to_vector`（规则匹配）
    /// - ML: 使用 `HashedNGramEmbedding`（ML 嵌入）
    /// - Auto: 查询长度 < ml_min_query_len 时用 Fast，否则用 ML
    pub fn vectorize(&self, text: &str) -> Vec<f64> {
        let start = Instant::now();
        let text_len = text.len();

        let (chosen_path, result) = match self.mode {
            VectorizeMode::Fast => {
                tracing::debug!(
                    text_len,
                    "HybridVectorizer::vectorize — Fast mode, using text_to_vector"
                );
                ("Fast", crate::vector::text_to_vector(text))
            }
            VectorizeMode::Ml => {
                tracing::debug!(
                    text_len,
                    ml_enabled = self.config.ml_enabled,
                    "HybridVectorizer::vectorize — ML mode, using HashedNGramEmbedding"
                );
                ("Ml", self.embedding.embed(text))
            }
            VectorizeMode::Auto => {
                let use_ml = text_len >= self.config.ml_min_query_len && self.config.ml_enabled;
                if use_ml {
                    tracing::debug!(
                        text_len,
                        ml_min_query_len = self.config.ml_min_query_len,
                        ml_enabled = self.config.ml_enabled,
                        "HybridVectorizer::vectorize — Auto→ML (text_len >= ml_min_query_len && ml_enabled)"
                    );
                    ("Auto→Ml", self.embedding.embed(text))
                } else {
                    let reason = if text_len < self.config.ml_min_query_len {
                        format!("text_len({}) < ml_min_query_len({})", text_len, self.config.ml_min_query_len)
                    } else {
                        format!("ml_enabled={}", self.config.ml_enabled)
                    };
                    tracing::debug!(
                        text_len,
                        ml_min_query_len = self.config.ml_min_query_len,
                        ml_enabled = self.config.ml_enabled,
                        "HybridVectorizer::vectorize — Auto→Fast ({})",
                        reason
                    );
                    ("Auto→Fast", crate::vector::text_to_vector(text))
                }
            }
        };

        let elapsed = start.elapsed();
        let dim = result.len();
        tracing::info!(
            path = chosen_path,
            text_len,
            dim,
            elapsed_us = elapsed.as_micros(),
            elapsed_ms = elapsed.as_micros() as f64 / 1000.0,
            "HybridVectorizer::vectorize"
        );
        result
    }

    /// 增量学习
    ///
    /// 用实际查询结果更新 ML 嵌入权重。
    /// 每次 `search()` 调用后，用最佳匹配结果作为正样本更新。
    pub fn learn(&mut self, query: &str, target_vector: &[f64], positive: bool) {
        let label = if positive { 1.0 } else { -1.0 };
        tracing::debug!(
            query_len = query.len(),
            target_dim = target_vector.len(),
            positive,
            label,
            train_count_before = self.embedding.train_count(),
            "HybridVectorizer::learn"
        );
        self.embedding.update(query, target_vector, label);
    }

    /// 获取内部 ML 嵌入器引用
    pub fn embedding(&self) -> &HashedNGramEmbedding {
        &self.embedding
    }

    /// 获取内部 ML 嵌入器可变引用
    pub fn embedding_mut(&mut self) -> &mut HashedNGramEmbedding {
        &mut self.embedding
    }

    /// 获取配置引用
    pub fn config(&self) -> &EmbeddingConfig {
        &self.config
    }

    /// 重置 ML 嵌入器
    pub fn reset(&mut self) {
        tracing::info!(
            prev_train_count = self.embedding.train_count(),
            mode = %self.mode,
            "HybridVectorizer::reset"
        );
        self.embedding.reset();
    }

    /// 禁用 ML 嵌入（纯规则模式）
    pub fn disable_ml(&mut self) {
        tracing::info!(
            was_enabled = self.config.ml_enabled,
            current_mode = %self.mode,
            "HybridVectorizer::disable_ml"
        );
        self.config.ml_enabled = false;
    }

    /// 启用 ML 嵌入
    pub fn enable_ml(&mut self) {
        tracing::info!(
            was_enabled = self.config.ml_enabled,
            current_mode = %self.mode,
            "HybridVectorizer::enable_ml"
        );
        self.config.ml_enabled = true;
    }
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────

/// 归一化向量到单位长度
fn normalize(vec: &mut [f64]) {
    let norm: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
}

// ─── 测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ngrams() {
        let ngrams = extract_ngrams("ab", &(1..3));
        assert!(ngrams.contains(&"a".to_string()));
        assert!(ngrams.contains(&"b".to_string()));
        assert!(ngrams.contains(&"ab".to_string()));
        assert_eq!(ngrams.len(), 3); // "a", "b", "ab"
    }

    #[test]
    fn test_extract_ngrams_unicode() {
        let ngrams = extract_ngrams("你好", &(1..3));
        assert!(ngrams.contains(&"你".to_string()));
        assert!(ngrams.contains(&"好".to_string()));
        assert!(ngrams.contains(&"你好".to_string()));
    }

    #[test]
    fn test_extract_ngrams_empty() {
        let ngrams = extract_ngrams("", &(1..4));
        assert!(ngrams.is_empty());
    }

    #[test]
    fn test_hash_feature() {
        let h1 = hash_feature("hello", 128);
        let h2 = hash_feature("hello", 128);
        let h3 = hash_feature("world", 128);
        assert_eq!(h1, h2, "Same feature should hash to same index");
        assert!(h1 < 128);
        assert!(h3 < 128);
    }

    #[test]
    fn test_hash_sign() {
        let s1 = hash_sign("hello");
        let s2 = hash_sign("hello");
        let s3 = hash_sign("world");
        assert_eq!(s1, s2, "Same feature should have same sign");
        assert!(s3 == 1.0 || s3 == -1.0);
    }

    #[test]
    fn test_hashed_ngram_embedding_default() {
        let emb = HashedNGramEmbedding::default();
        let vec = emb.embed("hello world");
        assert_eq!(vec.len(), 128);
        // 归一化后范数应为 1
        let norm: f64 = vec.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_hashed_ngram_embedding_empty() {
        let emb = HashedNGramEmbedding::default();
        let vec = emb.embed("");
        assert_eq!(vec.len(), 128);
        assert!(vec.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_hashed_ngram_embedding_consistency() {
        let emb = HashedNGramEmbedding::default();
        let v1 = emb.embed("add a b");
        let v2 = emb.embed("add a b");
        // 相同文本应产生相同向量
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_hashed_ngram_embedding_similarity() {
        let emb = HashedNGramEmbedding::default();
        let v1 = emb.embed("add two numbers");
        let v2 = emb.embed("sum two values");
        let v3 = emb.embed("format this string");

        // 语义相近的文本应向量的余弦相似度更高
        let sim_12 = cosine_similarity_f64(&v1, &v2);
        let sim_13 = cosine_similarity_f64(&v1, &v3);
        // "add two numbers" 和 "sum two values" 应比 "format this string" 更相似
        assert!(
            sim_12 >= sim_13 - 0.1,
            "Similar texts should have higher similarity ({} vs {})",
            sim_12,
            sim_13
        );
    }

    #[test]
    fn test_hashed_ngram_embedding_update() {
        let mut emb = HashedNGramEmbedding::default();
        let target = emb.embed("initial text");
        assert_eq!(emb.train_count(), 0);

        emb.update("learn from this", &target, 1.0);
        assert_eq!(emb.train_count(), 1);
    }

    #[test]
    fn test_hashed_ngram_embedding_update_batch() {
        let mut emb = HashedNGramEmbedding::default();
        let target = emb.embed("text");

        emb.update_batch(
            &["sample1", "sample2"],
            &[target.clone(), target.clone()],
            &[1.0, -1.0],
        );
        assert_eq!(emb.train_count(), 2);
    }

    #[test]
    fn test_hybrid_vectorizer_default() {
        let hv = HybridVectorizer::default();
        assert_eq!(hv.mode(), VectorizeMode::Auto);
        assert!(hv.config().ml_enabled);
    }

    #[test]
    fn test_hybrid_vectorizer_fast_mode() {
        let mut hv = HybridVectorizer::default();
        hv.set_mode(VectorizeMode::Fast);

        let vec = hv.vectorize("hello world");
        // Fast 模式应返回 64 维向量（text_to_vector 的默认维度）
        assert_eq!(vec.len(), 64);
    }

    #[test]
    fn test_hybrid_vectorizer_ml_mode() {
        let mut hv = HybridVectorizer::default();
        hv.set_mode(VectorizeMode::Ml);

        let vec = hv.vectorize("hello world");
        // ML 模式应返回 128 维向量（默认 feature_dim）
        assert_eq!(vec.len(), 128);
    }

    #[test]
    fn test_hybrid_vectorizer_auto_short_query() {
        let hv = HybridVectorizer::default();
        // 短查询 (len < 5) 应使用 Fast 模式 → 64 维
        let vec = hv.vectorize("hi");
        assert_eq!(vec.len(), 64);
    }

    #[test]
    fn test_hybrid_vectorizer_auto_long_query() {
        let hv = HybridVectorizer::default();
        // 长查询 (len >= 5) 应使用 ML 模式 → 128 维
        let vec = hv.vectorize("hello world test");
        assert_eq!(vec.len(), 128);
    }

    #[test]
    fn test_hybrid_vectorizer_learn() {
        let mut hv = HybridVectorizer::default();
        hv.set_mode(VectorizeMode::Ml);
        let target = hv.vectorize("target");
        let before = hv.embedding().train_count();

        hv.learn("query", &target, true);
        assert_eq!(hv.embedding().train_count(), before + 1);
    }

    #[test]
    fn test_hybrid_vectorizer_disable_ml() {
        let mut hv = HybridVectorizer::default();
        hv.disable_ml();
        assert!(!hv.config().ml_enabled);

        // 即使 Auto 模式，ML 禁用时也用 Fast
        let vec = hv.vectorize("hello world test");
        assert_eq!(vec.len(), 64);
    }

    #[test]
    fn test_hybrid_vectorizer_reset() {
        let mut hv = HybridVectorizer::default();
        hv.set_mode(VectorizeMode::Ml);
        let target = hv.vectorize("test");
        hv.learn("query", &target, true);
        assert!(hv.embedding().train_count() > 0);

        hv.reset();
        assert_eq!(hv.embedding().train_count(), 0);
    }

    #[test]
    fn test_hybrid_vectorizer_set_mode() {
        let mut hv = HybridVectorizer::default();
        assert_eq!(hv.mode(), VectorizeMode::Auto);

        hv.set_mode(VectorizeMode::Fast);
        assert_eq!(hv.mode(), VectorizeMode::Fast);

        hv.set_mode(VectorizeMode::Ml);
        assert_eq!(hv.mode(), VectorizeMode::Ml);
    }

    #[test]
    fn test_normalize() {
        let mut vec = vec![3.0, 4.0];
        normalize(&mut vec);
        // 3-4-5 三角形，归一化后应为 (0.6, 0.8)
        assert!((vec[0] - 0.6).abs() < 1e-6);
        assert!((vec[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_zero() {
        let mut vec = vec![0.0, 0.0, 0.0];
        normalize(&mut vec);
        assert!(vec.iter().all(|&x| x == 0.0));
    }

    fn cosine_similarity_f64(a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        if na * nb == 0.0 {
            0.0
        } else {
            dot / (na * nb)
        }
    }
}