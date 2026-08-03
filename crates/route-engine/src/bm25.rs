//! BM25 语义检索
//!
//! 基于 BM25 算法的文档检索，支持中文 unigram + bigram 分词，
//! IDF 加权和文档长度归一化。

use std::collections::HashMap;

/// BM25 搜索结果
#[derive(Debug, Clone)]
pub struct BM25Result {
    /// 文档 ID
    pub id: String,
    /// BM25 得分
    pub score: f64,
    /// 内容摘要
    pub snippet: String,
}

/// BM25 索引
#[derive(Debug, Clone)]
pub struct BM25Index {
    /// 文档总数
    doc_count: usize,
    /// 文档长度（词数）
    doc_lengths: HashMap<String, usize>,
    /// 文档内容
    doc_contents: HashMap<String, String>,
    /// 倒排索引：term -> (doc_id -> term_frequency)
    inverted_index: HashMap<String, HashMap<String, usize>>,
    /// 平均文档长度
    avg_doc_length: f64,
    /// BM25 参数
    k1: f64,
    /// BM25 参数
    b: f64,
}

impl Default for BM25Index {
    fn default() -> Self {
        Self::new()
    }
}

impl BM25Index {
    /// 创建一个新的 BM25 索引
    ///
    /// 使用默认参数 k1=1.5, b=0.75
    pub fn new() -> Self {
        BM25Index {
            doc_count: 0,
            doc_lengths: HashMap::new(),
            doc_contents: HashMap::new(),
            inverted_index: HashMap::new(),
            avg_doc_length: 0.0,
            k1: 1.5,
            b: 0.75,
        }
    }

    /// 创建带有自定义 BM25 参数的索引
    pub fn with_params(k1: f64, b: f64) -> Self {
        BM25Index {
            doc_count: 0,
            doc_lengths: HashMap::new(),
            doc_contents: HashMap::new(),
            inverted_index: HashMap::new(),
            avg_doc_length: 0.0,
            k1,
            b,
        }
    }

    /// 添加文档到索引
    pub fn add_document(&mut self, id: &str, text: &str) {
        let id = id.to_string();
        let text = text.to_string();

        // 分词
        let tokens = tokenize_chinese(&text);
        let doc_len = tokens.len();

        // 更新文档信息
        self.doc_contents.insert(id.clone(), text);
        self.doc_lengths.insert(id.clone(), doc_len);
        self.doc_count += 1;

        // 更新倒排索引
        let mut term_freq: HashMap<String, usize> = HashMap::new();
        for token in &tokens {
            *term_freq.entry(token.clone()).or_insert(0) += 1;
        }

        for (term, freq) in term_freq {
            self.inverted_index
                .entry(term)
                .or_default()
                .insert(id.clone(), freq);
        }

        // 更新平均文档长度
        self.avg_doc_length = if self.doc_count > 0 {
            let total: usize = self.doc_lengths.values().sum();
            total as f64 / self.doc_count as f64
        } else {
            0.0
        };
    }

    /// 搜索文档
    ///
    /// * `query` - 查询字符串
    /// * `top_k` - 返回前 k 个结果
    pub fn search(&self, query: &str, top_k: usize) -> Vec<BM25Result> {
        if self.doc_count == 0 || query.is_empty() {
            return Vec::new();
        }

        let query_tokens = tokenize_chinese(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }

        // 计算每个文档的 BM25 得分
        let mut scores: HashMap<String, f64> = HashMap::new();

        for term in &query_tokens {
            let idf = self.idf(term);
            if idf == 0.0 {
                continue;
            }

            if let Some(postings) = self.inverted_index.get(term) {
                for (doc_id, &tf) in postings {
                    let doc_len = *self.doc_lengths.get(doc_id).unwrap_or(&0) as f64;
                    let score = idf
                        * ((tf as f64 * (self.k1 + 1.0))
                            / (tf as f64
                                + self.k1 * (1.0 - self.b + self.b * doc_len / self.avg_doc_length)));
                    *scores.entry(doc_id.clone()).or_insert(0.0) += score;
                }
            }
        }

        // 排序并取 top_k
        let mut results: Vec<BM25Result> = scores
            .into_iter()
            .map(|(id, score)| {
                let snippet = self
                    .doc_contents
                    .get(&id)
                    .cloned()
                    .unwrap_or_default();
                BM25Result { id, score, snippet }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        results
    }

    /// 计算 IDF（逆文档频率）
    fn idf(&self, term: &str) -> f64 {
        match self.inverted_index.get(term) {
            Some(postings) => {
                let df = postings.len();
                ((self.doc_count as f64 - df as f64 + 0.5) / (df as f64 + 0.5) + 1.0).ln()
            }
            None => 0.0,
        }
    }

    /// 获取索引中文档总数
    pub fn doc_count(&self) -> usize {
        self.doc_count
    }

    /// 获取词汇表大小
    pub fn vocabulary_size(&self) -> usize {
        self.inverted_index.len()
    }
}

/// 中文分词：unigram + bigram
///
/// 对英文部分按单词分词，对中文部分做 unigram + bigram 分词。
fn tokenize_chinese(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    // 先按字符分类处理
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_ascii_alphanumeric() || ch == '_' {
            // 提取连续的英文/数字单词
            let mut word = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                word.push(chars[i]);
                i += 1;
            }
            if !word.is_empty() {
                tokens.push(word.to_lowercase());
            }
        } else if ch as u32 > 127 && !ch.is_ascii() {
            // 中文字符：unigram
            let unigram = ch.to_string();
            tokens.push(unigram);

            // bigram（如果下一个也是中文字符）
            if i + 1 < chars.len() && chars[i + 1] as u32 > 127 && !chars[i + 1].is_ascii() {
                let bigram = format!("{}{}", ch, chars[i + 1]);
                tokens.push(bigram);
            }

            i += 1;
        } else {
            // 其他字符（标点、空格等）跳过
            i += 1;
        }
    }

    tokens
}

/// 生成摘要：截取查询词附近的内容
#[allow(dead_code)]
fn generate_snippet(text: &str, query: &str, context_chars: usize) -> String {
    if text.len() <= context_chars * 2 {
        return text.to_string();
    }

    if let Some(pos) = text.to_lowercase().find(&query.to_lowercase()) {
        let start = pos.saturating_sub(context_chars);
        let end = (pos + query.len() + context_chars).min(text.len());
        let mut snippet = String::new();

        if start > 0 {
            snippet.push_str("...");
        }
        snippet.push_str(&text[start..end]);
        if end < text.len() {
            snippet.push_str("...");
        }
        snippet
    } else {
        // 如果找不到查询词，返回前 context_chars*2 个字符
        let end = context_chars * 2;
        let mut snippet = text[..end.min(text.len())].to_string();
        if end < text.len() {
            snippet.push_str("...");
        }
        snippet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_search() {
        let mut index = BM25Index::new();
        index.add_document("doc1", "Rust is a systems programming language");
        index.add_document("doc2", "Python is a high-level programming language");
        index.add_document("doc3", "JavaScript runs in the browser");

        let results = index.search("programming language", 5);
        assert!(!results.is_empty());
        // 包含 "programming language" 的文档应该排在前面
        assert!(results[0].score > 0.0);
    }

    #[test]
    fn test_search_top_k() {
        let mut index = BM25Index::new();
        index.add_document("doc1", "hello world");
        index.add_document("doc2", "hello rust");
        index.add_document("doc3", "hello china");

        let results = index.search("hello", 2);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_chinese_search() {
        let mut index = BM25Index::new();
        index.add_document("doc1", "你好世界");
        index.add_document("doc2", "世界很大");
        index.add_document("doc3", "hello world");

        let results = index.search("世界", 5);
        assert!(!results.is_empty());
        // 包含"世界"的文档应该被检索到
        assert!(results.iter().any(|r| r.id == "doc1" || r.id == "doc2"));
    }

    #[test]
    fn test_empty_index() {
        let index = BM25Index::new();
        let results = index.search("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_vocabulary_size() {
        let mut index = BM25Index::new();
        index.add_document("doc1", "hello world");
        index.add_document("doc2", "hello rust");
        // 词汇：hello, world, rust（3个唯一词）
        assert_eq!(index.vocabulary_size(), 3);
    }

    #[test]
    fn test_tokenize_chinese() {
        let tokens = tokenize_chinese("hello world");
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));

        let tokens = tokenize_chinese("你好世界");
        // unigram: 你, 好, 世, 界
        assert!(tokens.contains(&"你".to_string()));
        assert!(tokens.contains(&"好".to_string()));
        // bigram: 你好, 好世, 世界
        assert!(tokens.contains(&"你好".to_string()));
        assert!(tokens.contains(&"世界".to_string()));
    }
}