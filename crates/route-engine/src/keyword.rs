//! 关键词索引机制
//!
//! 结合 Trie 精确/前缀匹配和 BM25 语义检索的综合关键词索引。

use crate::bm25::BM25Index;
use crate::trie::TrieIndex;

/// 关键词索引
///
/// 内部维护三个索引组件：
/// - `trie`: Trie 前缀树，用于前缀匹配
/// - `bm25`: BM25 索引，用于语义检索
/// - `exact_match`: 精确匹配的快速查找表
#[derive(Debug, Clone)]
pub struct KeywordIndex {
    /// Trie 前缀树索引
    pub trie: TrieIndex,
    /// BM25 搜索引擎
    pub bm25: BM25Index,
    /// 精确匹配表：content -> set of ids
    exact_match: std::collections::HashMap<String, Vec<String>>,
    /// 文档计数器
    doc_counter: usize,
}

impl Default for KeywordIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl KeywordIndex {
    /// 创建一个新的关键词索引
    pub fn new() -> Self {
        KeywordIndex {
            trie: TrieIndex::new(),
            bm25: BM25Index::new(),
            exact_match: std::collections::HashMap::new(),
            doc_counter: 0,
        }
    }

    /// 添加条目到索引
    ///
    /// * `id` - 条目唯一标识
    /// * `content` - 文本内容
    pub fn add_entry(&mut self, id: &str, content: &str) {
        let id = id.to_string();

        // 1. 添加到 BM25 索引
        self.bm25.add_document(&id, content);

        // 2. 添加到精确匹配表
        self.exact_match
            .entry(content.to_string())
            .or_default()
            .push(id.clone());

        // 3. 添加到 Trie 索引（以单词为单位）
        let trie_entry = crate::trie::IndexEntry {
            file_path: id.clone(),
            line: self.doc_counter,
            content: content.to_string(),
            language: "text".to_string(),
        };

        // 对内容中的每个单词进行索引
        for word in extract_words(content) {
            if !word.is_empty() {
                self.trie.insert(&word.to_lowercase(), trie_entry.clone());
            }
        }
        // 也索引完整内容（用于中文搜索）
        self.trie.insert(content, trie_entry);

        self.doc_counter += 1;
    }

    /// 搜索关键词索引
    ///
    /// 结合三种匹配方式，返回综合得分结果。
    pub fn search(&self, query: &str, top_k: usize) -> Vec<KeywordResult> {
        let mut results: Vec<KeywordResult> = Vec::new();

        // 1. 精确匹配（最高优先级）
        if let Some(ids) = self.exact_match.get(query) {
            for id in ids {
                results.push(KeywordResult {
                    id: id.clone(),
                    score: 1.0,
                    match_type: MatchType::Exact,
                });
            }
        }

        // 2. 前缀匹配（Trie）
        let prefix_results = self.prefix_match(query);
        for pr in &prefix_results {
            if !results.iter().any(|r| r.id == pr.id) {
                results.push(pr.clone());
            }
        }

        // 3. BM25 语义搜索
        let bm25_results = self.bm25.search(query, top_k);
        for br in &bm25_results {
            if !results.iter().any(|r| r.id == br.id) {
                let score = 0.3 + br.score * 0.2; // 归一化到 0.3~0.5
                let score = score.min(0.5);
                results.push(KeywordResult {
                    id: br.id.clone(),
                    score,
                    match_type: MatchType::Bm25,
                });
            }
        }

        // 按得分降序排列
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// 精确匹配
    pub fn exact_match(&self, query: &str) -> Vec<KeywordResult> {
        self.exact_match
            .get(query)
            .map(|ids| {
                ids.iter()
                    .map(|id| KeywordResult {
                        id: id.clone(),
                        score: 1.0,
                        match_type: MatchType::Exact,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 前缀匹配
    pub fn prefix_match(&self, query: &str) -> Vec<KeywordResult> {
        let trie_results = self.trie.prefix_search(query);
        trie_results
            .iter()
            .map(|entry| KeywordResult {
                id: entry.file_path.clone(),
                score: 0.8,
                match_type: MatchType::Prefix,
            })
            .collect()
    }

    /// 获取条目总数
    pub fn count(&self) -> usize {
        self.doc_counter
    }

    /// 清空索引
    pub fn clear(&mut self) {
        self.trie.clear();
        self.bm25 = BM25Index::new();
        self.exact_match.clear();
        self.doc_counter = 0;
    }
}

/// 关键词搜索结果
#[derive(Debug, Clone)]
pub struct KeywordResult {
    pub id: String,
    pub score: f64,
    pub match_type: MatchType,
}

/// 匹配类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchType {
    Exact,
    Prefix,
    Bm25,
}

/// 提取文本中的单词
fn extract_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            // 非 ASCII 字符（如中文）作为单独单词
            if !ch.is_ascii() && ch as u32 > 127 {
                words.push(ch.to_string());
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_keyword_index() {
        let idx = KeywordIndex::new();
        assert_eq!(idx.count(), 0);
    }

    #[test]
    fn test_add_entry() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "hello world");
        assert_eq!(idx.count(), 1);
    }

    #[test]
    fn test_exact_match() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "hello world");
        idx.add_entry("doc2", "foo bar");

        let results = idx.exact_match("hello world");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
        assert_eq!(results[0].match_type, MatchType::Exact);
    }

    #[test]
    fn test_prefix_match() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "hello world");
        idx.add_entry("doc2", "help me");

        let results = idx.prefix_match("hel");
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.match_type == MatchType::Prefix));
    }

    #[test]
    fn test_search_combined() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "hello world");
        idx.add_entry("doc2", "hello rust");
        idx.add_entry("doc3", "goodbye");

        // 精确匹配 "hello world" 应该返回 doc1
        let results = idx.search("hello world", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "doc1");
        assert_eq!(results[0].score, 1.0);
    }

    #[test]
    fn test_search_top_k() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "hello");
        idx.add_entry("doc2", "hello world");
        idx.add_entry("doc3", "hello rust");
        idx.add_entry("doc4", "goodbye");

        let results = idx.search("hello", 2);
        assert!(results.len() <= 2);
    }

    #[test]
    fn test_search_empty() {
        let idx = KeywordIndex::new();
        let results = idx.search("test", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "hello");
        idx.clear();
        assert_eq!(idx.count(), 0);
        assert!(idx.search("hello", 10).is_empty());
    }

    #[test]
    fn test_extract_words() {
        let words = extract_words("hello world");
        assert_eq!(words, vec!["hello", "world"]);

        let words = extract_words("hello_world");
        assert_eq!(words, vec!["hello_world"]);
    }

    #[test]
    fn test_add_entry_chinese() {
        let mut idx = KeywordIndex::new();
        idx.add_entry("doc1", "你好世界");
        assert_eq!(idx.count(), 1);
    }
}