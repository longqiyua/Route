use std::collections::HashMap;

/// 记忆层级
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryTier {
    Hot,
    Cold,
    Archived,
}

/// 记忆块
#[derive(Debug, Clone)]
pub struct MemoryBlock {
    pub id: String,
    pub content: String,
    pub tier: MemoryTier,
    pub last_accessed: i64,
    pub access_count: u64,
    pub estimated_bytes: usize,
    pub kind: String,
}

/// 分层记忆存储
pub struct TieredMemory {
    pub hot: Vec<MemoryBlock>,
    pub cold: Vec<MemoryBlock>,
    pub archived: Vec<MemoryBlock>,
    max_hot_bytes: usize,
}

impl TieredMemory {
    /// 创建新的分层记忆存储
    pub fn new() -> Self {
        Self {
            hot: Vec::new(),
            cold: Vec::new(),
            archived: Vec::new(),
            max_hot_bytes: 1_000_000, // 默认 1MB
        }
    }

    /// 设置 hot 层最大字节数
    pub fn set_max_hot_bytes(&mut self, max: usize) {
        self.max_hot_bytes = max;
    }

    /// 添加记忆块（新条目进入 hot 层）
    pub fn add(&mut self, mut block: MemoryBlock) {
        block.tier = MemoryTier::Hot;
        self.hot.push(block);
    }

    /// 搜索记忆块（先搜索 hot，再搜索 cold）
    pub fn search(&self, query: &str, top_k: usize) -> Vec<&MemoryBlock> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&MemoryBlock> = Vec::new();

        // 先搜索 hot
        for block in &self.hot {
            if results.len() >= top_k {
                break;
            }
            if Self::matches(block, &query_lower) {
                results.push(block);
            }
        }

        // 再搜索 cold
        if results.len() < top_k {
            for block in &self.cold {
                if results.len() >= top_k {
                    break;
                }
                if Self::matches(block, &query_lower) {
                    results.push(block);
                }
            }
        }

        results
    }

    /// 判断记忆块是否匹配查询
    fn matches(block: &MemoryBlock, query_lower: &str) -> bool {
        block.content.to_lowercase().contains(query_lower)
            || block.id.to_lowercase().contains(query_lower)
            || block.kind.to_lowercase().contains(query_lower)
    }

    /// 提升记忆块：cold → hot
    pub fn promote(&mut self, id: &str) -> bool {
        if let Some(pos) = self.cold.iter().position(|b| b.id == id) {
            let mut block = self.cold.remove(pos);
            block.tier = MemoryTier::Hot;
            self.hot.push(block);
            true
        } else {
            false
        }
    }

    /// 降级记忆块：hot → cold
    pub fn demote(&mut self, id: &str) -> bool {
        if let Some(pos) = self.hot.iter().position(|b| b.id == id) {
            let mut block = self.hot.remove(pos);
            block.tier = MemoryTier::Cold;
            self.cold.push(block);
            true
        } else {
            false
        }
    }

    /// 自动分层：检查 hot 内存使用量，超出阈值时降级最旧的条目
    pub fn auto_tier(&mut self) {
        let total_hot_bytes: usize = self.hot.iter().map(|b| b.estimated_bytes).sum();
        if total_hot_bytes <= self.max_hot_bytes {
            return;
        }

        // 按最后访问时间排序（最旧在前）
        self.hot.sort_by_key(|b| b.last_accessed);

        // 持续降级直到 hot 内存降到阈值的一半以下
        while self.hot.iter().map(|b| b.estimated_bytes).sum::<usize>() > self.max_hot_bytes / 2 {
            if self.hot.is_empty() {
                break;
            }
            let mut block = self.hot.remove(0);
            block.tier = MemoryTier::Cold;
            self.cold.push(block);
        }
    }

    /// 估算总内存使用量（字节）
    pub fn estimate_total_bytes(&self) -> usize {
        self.hot.iter().map(|b| b.estimated_bytes).sum::<usize>()
            + self.cold.iter().map(|b| b.estimated_bytes).sum::<usize>()
            + self.archived.iter().map(|b| b.estimated_bytes).sum::<usize>()
    }

    /// 返回各层级的统计信息
    pub fn stats(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        map.insert("hot_count".to_string(), self.hot.len());
        map.insert("cold_count".to_string(), self.cold.len());
        map.insert("archived_count".to_string(), self.archived.len());
        map.insert(
            "hot_bytes".to_string(),
            self.hot.iter().map(|b| b.estimated_bytes).sum(),
        );
        map.insert(
            "cold_bytes".to_string(),
            self.cold.iter().map(|b| b.estimated_bytes).sum(),
        );
        map.insert(
            "archived_bytes".to_string(),
            self.archived.iter().map(|b| b.estimated_bytes).sum(),
        );
        map
    }
}

impl Default for TieredMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_block(id: &str, content: &str, bytes: usize) -> MemoryBlock {
        MemoryBlock {
            id: id.to_string(),
            content: content.to_string(),
            tier: MemoryTier::Hot,
            last_accessed: 0,
            access_count: 0,
            estimated_bytes: bytes,
            kind: "test".to_string(),
        }
    }

    #[test]
    fn test_add_goes_to_hot() {
        let mut tm = TieredMemory::new();
        let block = make_block("b1", "hello world", 100);
        tm.add(block);
        assert_eq!(tm.hot.len(), 1);
        assert_eq!(tm.cold.len(), 0);
    }

    #[test]
    fn test_search_hot_first() {
        let mut tm = TieredMemory::new();
        tm.add(make_block("hot1", "important data", 50));
        tm.cold.push(MemoryBlock {
            id: "cold1".to_string(),
            content: "other data".to_string(),
            tier: MemoryTier::Cold,
            last_accessed: 0,
            access_count: 0,
            estimated_bytes: 50,
            kind: "test".to_string(),
        });

        let results = tm.search("important", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "hot1");
    }

    #[test]
    fn test_search_cold_fallback() {
        let mut tm = TieredMemory::new();
        tm.add(make_block("hot1", "alpha", 50));
        tm.cold.push(MemoryBlock {
            id: "cold1".to_string(),
            content: "beta data".to_string(), ..make_block("", "", 50)
        });

        let results = tm.search("beta", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "cold1");
    }

    #[test]
    fn test_promote() {
        let mut tm = TieredMemory::new();
        tm.cold.push(MemoryBlock {
            id: "c1".to_string(),
            content: "cold content".to_string(),
            tier: MemoryTier::Cold,
            last_accessed: 0,
            access_count: 0,
            estimated_bytes: 50,
            kind: "test".to_string(),
        });

        assert!(tm.promote("c1"));
        assert_eq!(tm.hot.len(), 1);
        assert_eq!(tm.cold.len(), 0);
        assert_eq!(tm.hot[0].tier, MemoryTier::Hot);
    }

    #[test]
    fn test_promote_not_found() {
        let mut tm = TieredMemory::new();
        assert!(!tm.promote("nonexistent"));
    }

    #[test]
    fn test_demote() {
        let mut tm = TieredMemory::new();
        tm.add(make_block("h1", "hot content", 50));

        assert!(tm.demote("h1"));
        assert_eq!(tm.hot.len(), 0);
        assert_eq!(tm.cold.len(), 1);
        assert_eq!(tm.cold[0].tier, MemoryTier::Cold);
    }

    #[test]
    fn test_auto_tier_demotes_oldest() {
        let mut tm = TieredMemory::new();
        tm.set_max_hot_bytes(200);

        // 添加三个块，总大小 300 > 200
        tm.add(MemoryBlock {
            id: "old".to_string(),
            content: "oldest".to_string(),
            tier: MemoryTier::Hot,
            last_accessed: 100,
            access_count: 0,
            estimated_bytes: 100,
            kind: "test".to_string(),
        });
        tm.add(MemoryBlock {
            id: "mid".to_string(),
            content: "middle".to_string(),
            tier: MemoryTier::Hot,
            last_accessed: 200,
            access_count: 0,
            estimated_bytes: 100,
            kind: "test".to_string(),
        });
        tm.add(MemoryBlock {
            id: "new".to_string(),
            content: "newest".to_string(),
            tier: MemoryTier::Hot,
            last_accessed: 300,
            access_count: 0,
            estimated_bytes: 100,
            kind: "test".to_string(),
        });

        tm.auto_tier();

        // 至少 old 应该被降级
        assert!(tm.cold.iter().any(|b| b.id == "old"));
        // hot 总字节应该 <= 200 的一半即 100
        let hot_bytes: usize = tm.hot.iter().map(|b| b.estimated_bytes).sum();
        assert!(hot_bytes <= 100);
    }

    #[test]
    fn test_estimate_total_bytes() {
        let mut tm = TieredMemory::new();
        tm.add(make_block("b1", "content", 100));
        tm.add(make_block("b2", "content", 200));
        assert_eq!(tm.estimate_total_bytes(), 300);
    }

    #[test]
    fn test_stats() {
        let mut tm = TieredMemory::new();
        tm.add(make_block("b1", "content", 100));
        tm.add(make_block("b2", "content", 200));
        tm.cold.push(MemoryBlock {
            id: "c1".to_string(),
            content: "cold".to_string(),
            tier: MemoryTier::Cold,
            last_accessed: 0,
            access_count: 0,
            estimated_bytes: 50,
            kind: "test".to_string(),
        });

        let stats = tm.stats();
        assert_eq!(stats.get("hot_count"), Some(&2));
        assert_eq!(stats.get("cold_count"), Some(&1));
        assert_eq!(stats.get("archived_count"), Some(&0));
        assert_eq!(stats.get("hot_bytes"), Some(&300));
        assert_eq!(stats.get("cold_bytes"), Some(&50));
    }

    #[test]
    fn test_search_top_k() {
        let mut tm = TieredMemory::new();
        tm.add(make_block("b1", "apple pie", 10));
        tm.add(make_block("b2", "apple juice", 10));
        tm.add(make_block("b3", "apple cider", 10));

        let results = tm.search("apple", 2);
        assert_eq!(results.len(), 2);
    }
}