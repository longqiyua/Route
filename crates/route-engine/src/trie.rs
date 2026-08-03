//! Trie 前缀树
//!
//! 支持行级索引的 Trie 前缀树实现，每个文件每行作为一个条目进行索引。

use std::collections::HashMap;

/// Trie 节点
#[derive(Debug, Clone)]
pub struct TrieNode {
    children: HashMap<char, TrieNode>,
    /// 标记是否为某个词的结尾
    is_end: bool,
    /// 存储在该节点结束的条目索引
    entry_indices: Vec<usize>,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: HashMap::new(),
            is_end: false,
            entry_indices: Vec::new(),
        }
    }
}

/// 行级索引条目
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub file_path: String,
    pub line: usize,
    pub content: String,
    pub language: String,
}

/// Trie 索引
#[derive(Debug, Clone)]
pub struct TrieIndex {
    root: TrieNode,
    /// 所有条目存储
    entries: Vec<IndexEntry>,
    /// 条目计数
    count: usize,
}

impl Default for TrieIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl TrieIndex {
    /// 创建一个新的 Trie 索引
    pub fn new() -> Self {
        TrieIndex {
            root: TrieNode::new(),
            entries: Vec::new(),
            count: 0,
        }
    }

    /// 插入一个键值对
    /// key: 用于搜索的键（可以是单词、汉字等）
    /// entry: 行级索引条目
    pub fn insert(&mut self, key: &str, entry: IndexEntry) {
        let entry_idx = self.entries.len();
        self.entries.push(entry);
        self.count += 1;

        let mut node = &mut self.root;
        for ch in key.chars() {
            node = node.children.entry(ch).or_insert_with(TrieNode::new);
        }
        node.is_end = true;
        node.entry_indices.push(entry_idx);
    }

    /// 精确搜索
    /// 返回完全匹配 key 的所有条目
    pub fn search(&self, key: &str) -> Vec<&IndexEntry> {
        let mut node = &self.root;
        for ch in key.chars() {
            match node.children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        if node.is_end {
            node.entry_indices
                .iter()
                .map(|&idx| &self.entries[idx])
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 前缀搜索
    /// 返回所有以 prefix 为前缀的条目
    pub fn prefix_search(&self, prefix: &str) -> Vec<&IndexEntry> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(child) => node = child,
                None => return Vec::new(),
            }
        }
        // 收集该节点及所有子节点下的条目
        let mut results = Vec::new();
        Self::collect_entries(node, &self.entries, &mut results);
        results
    }

    /// 递归收集节点下的所有条目
    fn collect_entries<'a>(
        node: &'a TrieNode,
        entries: &'a [IndexEntry],
        results: &mut Vec<&'a IndexEntry>,
    ) {
        if node.is_end {
            for &idx in &node.entry_indices {
                results.push(&entries[idx]);
            }
        }
        for child in node.children.values() {
            Self::collect_entries(child, entries, results);
        }
    }

    /// 删除指定 key 下特定文件路径和行号的条目
    pub fn delete(&mut self, key: &str, file_path: &str, line: usize) -> bool {
        let mut node = &mut self.root;
        for ch in key.chars() {
            match node.children.get_mut(&ch) {
                Some(child) => node = child,
                None => return false,
            }
        }
        if !node.is_end {
            return false;
        }
        let before = node.entry_indices.len();
        node.entry_indices.retain(|&idx| {
            let entry = &self.entries[idx];
            !(entry.file_path == file_path && entry.line == line)
        });
        let removed = before - node.entry_indices.len();
        if removed > 0 {
            self.count -= removed;
        }
        removed > 0
    }

    /// 获取条目总数
    pub fn count(&self) -> usize {
        self.count
    }

    /// 获取所有条目
    pub fn entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    /// 清空索引
    pub fn clear(&mut self) {
        self.root = TrieNode::new();
        self.entries.clear();
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(file_path: &str, line: usize, content: &str, language: &str) -> IndexEntry {
        IndexEntry {
            file_path: file_path.to_string(),
            line,
            content: content.to_string(),
            language: language.to_string(),
        }
    }

    #[test]
    fn test_insert_and_search() {
        let mut trie = TrieIndex::new();
        trie.insert("hello", make_entry("a.rs", 1, "fn hello()", "rust"));
        trie.insert("hello", make_entry("b.rs", 2, "hello world", "rust"));

        let results = trie.search("hello");
        assert_eq!(results.len(), 2);
        assert_eq!(trie.count(), 2);
    }

    #[test]
    fn test_search_no_match() {
        let trie = TrieIndex::new();
        let results = trie.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_prefix_search() {
        let mut trie = TrieIndex::new();
        trie.insert("hello", make_entry("a.rs", 1, "hello", "rust"));
        trie.insert("help", make_entry("b.rs", 2, "help", "rust"));
        trie.insert("world", make_entry("c.rs", 3, "world", "rust"));

        let results = trie.prefix_search("hel");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_delete() {
        let mut trie = TrieIndex::new();
        trie.insert("hello", make_entry("a.rs", 1, "hello", "rust"));
        trie.insert("hello", make_entry("a.rs", 2, "hello2", "rust"));

        assert!(trie.delete("hello", "a.rs", 1));
        assert_eq!(trie.count(), 1);

        let results = trie.search("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line, 2);
    }

    #[test]
    fn test_clear() {
        let mut trie = TrieIndex::new();
        trie.insert("test", make_entry("a.rs", 1, "test", "rust"));
        trie.clear();
        assert_eq!(trie.count(), 0);
        assert!(trie.search("test").is_empty());
    }
}