//! 热/冷索引管理器
//!
//! 实现 Hot/Cold 两级索引管理策略：
//! - Hot 索引：频繁访问的热数据，驻留在内存中
//! - Cold 索引：较少访问的冷数据，可被降级
//! - 自动平衡：当 Hot 索引超过内存限制时，自动冷却最旧的条目

use std::time::{SystemTime, UNIX_EPOCH};

/// 索引条目
#[derive(Debug, Clone)]
pub struct IndexEntry {
    pub id: String,
    pub content: String,
    pub file_path: String,
    pub last_accessed: u64,
    pub access_count: u64,
    pub is_hot: bool,
}

impl IndexEntry {
    /// 创建新的索引条目
    pub fn new(id: &str, content: &str, file_path: &str) -> Self {
        let now = current_time_ms();
        IndexEntry {
            id: id.to_string(),
            content: content.to_string(),
            file_path: file_path.to_string(),
            last_accessed: now,
            access_count: 1,
            is_hot: true, // 默认添加到热索引
        }
    }

    /// 更新访问时间
    pub fn touch(&mut self) {
        self.last_accessed = current_time_ms();
        self.access_count += 1;
    }
}

/// 热/冷索引管理器
#[derive(Debug, Clone)]
pub struct HotColdIndex {
    /// 热索引条目
    pub hot: Vec<IndexEntry>,
    /// 冷索引条目
    pub cold: Vec<IndexEntry>,
    /// 热索引最大内存（字节）
    pub max_hot_bytes: usize,
}

impl Default for HotColdIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl HotColdIndex {
    /// 创建一个新的 HotColdIndex
    ///
    /// 默认最大热索引内存为 10MB
    pub fn new() -> Self {
        HotColdIndex {
            hot: Vec::new(),
            cold: Vec::new(),
            max_hot_bytes: 10 * 1024 * 1024, // 10MB
        }
    }

    /// 创建带自定义最大内存的 HotColdIndex
    pub fn with_max_hot_bytes(max_hot_bytes: usize) -> Self {
        HotColdIndex {
            hot: Vec::new(),
            cold: Vec::new(),
            max_hot_bytes,
        }
    }

    /// 添加条目到索引（默认添加到热索引）
    pub fn add_entry(&mut self, id: &str, content: &str, file_path: &str) {
        let entry = IndexEntry::new(id, content, file_path);
        self.hot.push(entry);
        self.auto_balance();
    }

    /// 搜索所有索引（热索引优先）
    pub fn search(&self, query: &str) -> Vec<&IndexEntry> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        // 先搜索热索引
        for entry in &self.hot {
            if entry.content.to_lowercase().contains(&query_lower) {
                results.push(entry);
            }
        }

        // 再搜索冷索引
        for entry in &self.cold {
            if entry.content.to_lowercase().contains(&query_lower) {
                results.push(entry);
            }
        }

        results
    }

    /// 将条目从冷索引移到热索引
    pub fn warm_up(&mut self, entry_id: &str) -> bool {
        if let Some(pos) = self.cold.iter().position(|e| e.id == entry_id) {
            let mut entry = self.cold.remove(pos);
            entry.is_hot = true;
            entry.touch();
            self.hot.push(entry);
            self.auto_balance();
            true
        } else {
            false
        }
    }

    /// 将条目从热索引移到冷索引
    pub fn cool_down(&mut self, entry_id: &str) -> bool {
        if let Some(pos) = self.hot.iter().position(|e| e.id == entry_id) {
            let mut entry = self.hot.remove(pos);
            entry.is_hot = false;
            self.cold.push(entry);
            true
        } else {
            false
        }
    }

    /// 自动平衡热索引
    ///
    /// 检查热索引内存使用量，如果超过限制，冷却最旧的条目。
    pub fn auto_balance(&mut self) {
        let mut hot_size = self.estimate_memory_usage();

        // 按最后访问时间排序（最早的在前）
        self.hot.sort_by_key(|e| e.last_accessed);

        // 冷却最旧的条目，直到热索引大小低于限制
        while hot_size > self.max_hot_bytes && !self.hot.is_empty() {
            if let Some(oldest) = self.hot.first() {
                let id = oldest.id.clone();
                if self.cool_down(&id) {
                    hot_size = self.estimate_memory_usage();
                } else {
                    break;
                }
            }
        }
    }

    /// 估算当前热索引内存使用量（字节）
    pub fn estimate_memory_usage(&self) -> usize {
        let mut total = 0;
        for entry in &self.hot {
            // 估算字符串内存：id + content + file_path + 元数据开销
            total += entry.id.len();
            total += entry.content.len();
            total += entry.file_path.len();
            total += 32; // 元数据（u64 字段等）
        }
        total
    }

    /// 获取热索引条目数
    pub fn hot_count(&self) -> usize {
        self.hot.len()
    }

    /// 获取冷索引条目数
    pub fn cold_count(&self) -> usize {
        self.cold.len()
    }

    /// 获取总条目数
    pub fn total_count(&self) -> usize {
        self.hot.len() + self.cold.len()
    }

    /// 清空所有索引
    pub fn clear(&mut self) {
        self.hot.clear();
        self.cold.clear();
    }

    /// 获取热索引中最高访问频率的条目
    pub fn most_frequent_hot(&self, top_k: usize) -> Vec<&IndexEntry> {
        let mut sorted: Vec<&IndexEntry> = self.hot.iter().collect();
        sorted.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        sorted.truncate(top_k);
        sorted
    }

    /// 获取冷索引中最低访问频率的条目
    pub fn least_frequent_cold(&self, top_k: usize) -> Vec<&IndexEntry> {
        let mut sorted: Vec<&IndexEntry> = self.cold.iter().collect();
        sorted.sort_by(|a, b| a.access_count.cmp(&b.access_count));
        sorted.truncate(top_k);
        sorted
    }
}

/// 获取当前时间戳（毫秒）
fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_index() {
        let index = HotColdIndex::new();
        assert_eq!(index.hot_count(), 0);
        assert_eq!(index.cold_count(), 0);
        assert_eq!(index.total_count(), 0);
    }

    #[test]
    fn test_add_entry() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello world", "file1.rs");
        assert_eq!(index.hot_count(), 1);
        assert_eq!(index.total_count(), 1);
        assert!(index.hot[0].is_hot);
        assert_eq!(index.hot[0].access_count, 1);
    }

    #[test]
    fn test_search() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello world", "file1.rs");
        index.add_entry("doc2", "foo bar", "file2.rs");

        let results = index.search("hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_search_hot_first() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello world", "f1.rs");
        // 手动将一个条目移到冷索引
        index.cool_down("doc1");

        let results = index.search("hello");
        assert_eq!(results.len(), 1);
        // 热索引搜索结果应该在前
        assert!(results[0].id == "doc1");
    }

    #[test]
    fn test_search_no_match() {
        let index = HotColdIndex::new();
        let results = index.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_warm_up() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello", "f1.rs");
        assert!(index.cool_down("doc1"));
        assert_eq!(index.cold_count(), 1);
        assert_eq!(index.hot_count(), 0);

        assert!(index.warm_up("doc1"));
        assert_eq!(index.hot_count(), 1);
        assert_eq!(index.cold_count(), 0);
    }

    #[test]
    fn test_warm_up_not_found() {
        let mut index = HotColdIndex::new();
        assert!(!index.warm_up("nonexistent"));
    }

    #[test]
    fn test_cool_down() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello", "f1.rs");
        assert!(index.cool_down("doc1"));
        assert_eq!(index.hot_count(), 0);
        assert_eq!(index.cold_count(), 1);
        assert!(!index.cold[0].is_hot);
    }

    #[test]
    fn test_cool_down_not_found() {
        let mut index = HotColdIndex::new();
        assert!(!index.cool_down("nonexistent"));
    }

    #[test]
    fn test_auto_balance() {
        // 设置很小的最大内存
        let mut index = HotColdIndex::with_max_hot_bytes(50);
        index.add_entry("doc1", "hello world this is a test", "f1.rs");
        index.add_entry("doc2", "another test entry here", "f2.rs");

        // 添加更多条目触发自动平衡
        index.add_entry("doc3", "yet another piece of content", "f3.rs");

        // 由于内存限制很小，应该有一些条目被冷却
        let total = index.total_count();
        assert_eq!(total, 3);
    }

    #[test]
    fn test_estimate_memory_usage() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello", "f1.rs");
        let usage = index.estimate_memory_usage();
        assert!(usage > 0);
    }

    #[test]
    fn test_clear() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello", "f1.rs");
        index.add_entry("doc2", "world", "f2.rs");
        index.clear();
        assert_eq!(index.total_count(), 0);
    }

    #[test]
    fn test_most_frequent_hot() {
        let mut index = HotColdIndex::new();
        index.add_entry("doc1", "hello", "f1.rs");
        index.add_entry("doc2", "world", "f2.rs");

        // 访问 doc1 多次
        if let Some(entry) = index.hot.iter_mut().find(|e| e.id == "doc1") {
            entry.touch();
            entry.touch();
        }

        let freq = index.most_frequent_hot(1);
        assert_eq!(freq.len(), 1);
        assert_eq!(freq[0].id, "doc1");
        assert!(freq[0].access_count >= 3); // 1（初始）+ 2（touch）
    }

    #[test]
    fn test_entry_touch() {
        let mut entry = IndexEntry::new("doc1", "hello", "f1.rs");
        let old_access = entry.access_count;
        let old_time = entry.last_accessed;

        entry.touch();

        assert_eq!(entry.access_count, old_access + 1);
        assert!(entry.last_accessed >= old_time);
    }

    #[test]
    fn test_with_max_hot_bytes() {
        let index = HotColdIndex::with_max_hot_bytes(1024);
        assert_eq!(index.max_hot_bytes, 1024);
    }
}