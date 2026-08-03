//! 索引树
//!
//! 管理多个索引（英文单词树、中文汉字树、拼音树），
//! 支持构建索引、搜索和重建。

use crate::fuzzy::SearchResult;
use crate::trie::{IndexEntry, TrieIndex};

/// 索引条目
#[derive(Debug, Clone)]
pub struct IndexEntryInfo {
    pub file_path: String,
    pub line: usize,
    pub content: String,
    pub language: String,
}

/// 索引树，管理多个独立索引
#[derive(Debug, Clone)]
pub struct IndexTree {
    /// 英文单词索引
    english_tree: TrieIndex,
    /// 中文汉字索引
    chinese_tree: TrieIndex,
    /// 拼音索引（预留）
    pinyin_tree: TrieIndex,
    /// 原始条目存储
    entries: Vec<IndexEntryInfo>,
    /// 条目计数
    count: usize,
}

impl Default for IndexTree {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexTree {
    /// 创建一个新的索引树
    pub fn new() -> Self {
        IndexTree {
            english_tree: TrieIndex::new(),
            chinese_tree: TrieIndex::new(),
            pinyin_tree: TrieIndex::new(),
            entries: Vec::new(),
            count: 0,
        }
    }

    /// 构建索引
    ///
    /// 将文本解析后分别插入到英文单词索引和中文汉字索引中。
    ///
    /// * `text` - 文本内容
    /// * `file_path` - 源文件路径
    /// * `line` - 行号
    pub fn build(&mut self, text: &str, file_path: &str, line: usize) {
        let entry = IndexEntryInfo {
            file_path: file_path.to_string(),
            line,
            content: text.to_string(),
            language: detect_language(text),
        };

        let _entry_idx = self.entries.len();
        self.entries.push(entry);
        self.count += 1;

        // 提取英文单词
        for word in extract_english_words(text) {
            if !word.is_empty() {
                let trie_entry = IndexEntry {
                    file_path: file_path.to_string(),
                    line,
                    content: text.to_string(),
                    language: "en".to_string(),
                };
                self.english_tree.insert(&word.to_lowercase(), trie_entry);
            }
        }

        // 提取中文字符串
        for chinese_str in extract_chinese_chars(text) {
            if !chinese_str.is_empty() {
                let trie_entry = IndexEntry {
                    file_path: file_path.to_string(),
                    line,
                    content: text.to_string(),
                    language: "zh".to_string(),
                };
                self.chinese_tree.insert(&chinese_str, trie_entry);
            }
        }

        // 拼音索引（预留，当前仅按原样索引）
        // 拼音索引需要额外的拼音转换库，此处暂不实现
    }

    /// 搜索索引
    ///
    /// 在所有索引中搜索查询字符串，返回合并后的结果。
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();

        let query = query.trim();

        if query.is_empty() {
            return results;
        }

        // 判断查询类型，选择对应的索引搜索
        let has_english = query.chars().any(|c| c.is_ascii_alphabetic());
        let has_chinese = query.chars().any(|c| c.is_ascii_alphabetic() == false && c as u32 > 127);

        // 英文搜索
        if has_english {
            let lower_query = query.to_lowercase();
            // 先尝试精确搜索
            for entry in self.english_tree.search(&lower_query) {
                results.push(SearchResult {
                    text: entry.content.clone(),
                    score: 1.0,
                    file_path: Some(entry.file_path.clone()),
                    line: entry.line,
                });
            }
            // 再尝试前缀搜索
            for entry in self.english_tree.prefix_search(&lower_query) {
                // 避免重复
                if !results.iter().any(|r| r.line == entry.line && r.file_path.as_deref() == Some(&entry.file_path)) {
                    results.push(SearchResult {
                        text: entry.content.clone(),
                        score: 0.8,
                        file_path: Some(entry.file_path.clone()),
                        line: entry.line,
                    });
                }
            }
        }

        // 中文搜索
        if has_chinese {
            // 对中文，按字符逐字搜索
            for ch in query.chars() {
                if ch as u32 > 127 && !ch.is_ascii() {
                    let key = ch.to_string();
                    for entry in self.chinese_tree.search(&key) {
                        if !results.iter().any(|r| r.line == entry.line && r.file_path.as_deref() == Some(&entry.file_path)) {
                            results.push(SearchResult {
                                text: entry.content.clone(),
                                score: 1.0,
                                file_path: Some(entry.file_path.clone()),
                                line: entry.line,
                            });
                        }
                    }
                }
            }
            // 中文前缀搜索（对整个查询字符串）
            for entry in self.chinese_tree.prefix_search(query) {
                if !results.iter().any(|r| r.line == entry.line && r.file_path.as_deref() == Some(&entry.file_path)) {
                    results.push(SearchResult {
                        text: entry.content.clone(),
                        score: 0.8,
                        file_path: Some(entry.file_path.clone()),
                        line: entry.line,
                    });
                }
            }
        }

        // 按 score 降序排列
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        results
    }

    /// 重建索引树
    ///
    /// 清空所有索引并基于当前存储的条目重新构建。
    pub fn rebuild(&mut self) {
        // 保存现有条目
        let old_entries: Vec<IndexEntryInfo> = std::mem::take(&mut self.entries);
        let _old_count = self.count;

        // 清空所有 Trie 索引
        self.english_tree.clear();
        self.chinese_tree.clear();
        self.pinyin_tree.clear();

        // 重置计数
        self.count = 0;

        // 重新插入所有条目到 Trie 索引中
        for entry in &old_entries {
            // 提取英文单词
            for word in extract_english_words(&entry.content) {
                if !word.is_empty() {
                    self.english_tree.insert(&word.to_lowercase(), IndexEntry {
                        file_path: entry.file_path.clone(),
                        line: entry.line,
                        content: entry.content.clone(),
                        language: "en".to_string(),
                    });
                }
            }

            // 提取中文字符串
            for chinese_str in extract_chinese_chars(&entry.content) {
                if !chinese_str.is_empty() {
                    self.chinese_tree.insert(&chinese_str, IndexEntry {
                        file_path: entry.file_path.clone(),
                        line: entry.line,
                        content: entry.content.clone(),
                        language: "zh".to_string(),
                    });
                }
            }

            self.count += 1;
        }

        self.entries = old_entries;
    }

    /// 获取条目总数
    pub fn count(&self) -> usize {
        self.count
    }

    /// 获取所有条目
    pub fn entries(&self) -> &[IndexEntryInfo] {
        &self.entries
    }
}

/// 检测文本语言
fn detect_language(text: &str) -> String {
    for ch in text.chars() {
        if ch as u32 > 127 && !ch.is_ascii() {
            return "zh".to_string();
        }
    }
    "en".to_string()
}

/// 提取文本中的英文单词
fn extract_english_words(text: &str) -> Vec<String> {
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
        }
    }
    if !current.is_empty() {
        words.push(current);
    }

    words
}

/// 提取文本中的中文字符串（连续的中文字符作为一个整体）
fn extract_chinese_chars(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch as u32 > 127 && !ch.is_ascii() {
            current.push(ch);
        } else {
            if !current.is_empty() {
                result.push(current.clone());
                current.clear();
            }
        }
    }
    if !current.is_empty() {
        result.push(current);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_search_english() {
        let mut tree = IndexTree::new();
        tree.build("fn main() {", "src/main.rs", 1);
        tree.build("    println!(\"hello\");", "src/main.rs", 2);

        let results = tree.search("main");
        assert!(!results.is_empty());
        // "main" 在 "fn main() {" 中是一个单词，应该被匹配
        assert!(results.iter().any(|r| r.line == 1));
    }

    #[test]
    fn test_build_and_search_chinese() {
        let mut tree = IndexTree::new();
        tree.build("你好世界", "src/lib.rs", 1);
        tree.build("hello world", "src/lib.rs", 2);

        let results = tree.search("你好");
        // 中文搜索应该能匹配到
        assert!(!results.is_empty());
    }

    #[test]
    fn test_rebuild() {
        let mut tree = IndexTree::new();
        tree.build("fn main() {", "src/main.rs", 1);
        assert_eq!(tree.count(), 1);

        tree.rebuild();
        assert_eq!(tree.count(), 1);

        let results = tree.search("main");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_empty() {
        let tree = IndexTree::new();
        let results = tree.search("");
        assert!(results.is_empty());
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("hello world"), "en");
        assert_eq!(detect_language("你好世界"), "zh");
    }

    #[test]
    fn test_extract_english_words() {
        let words = extract_english_words("fn main() {");
        assert_eq!(words, vec!["fn", "main"]);
    }

    #[test]
    fn test_extract_chinese_chars() {
        let chars = extract_chinese_chars("你好世界hello");
        assert_eq!(chars, vec!["你好世界"]);
    }
}