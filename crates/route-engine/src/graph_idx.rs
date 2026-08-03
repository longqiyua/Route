//! 图索引机制
//!
//! 提供基于 BFS 和 PageRank 的图索引搜索，
//! 支持添加边、BFS 遍历、PageRank 计算和 Mermaid 图生成。

use std::collections::{HashMap, HashSet, VecDeque};

/// 图索引结果
#[derive(Debug, Clone)]
pub struct GraphIndexResult {
    pub node: String,
    pub score: f64,
    pub path: Vec<String>,
    pub depth: usize,
}

/// 图索引
#[derive(Debug, Clone, Default)]
pub struct GraphIndex {
    /// 邻接表：node -> [neighbors]
    pub adjacency: HashMap<String, Vec<String>>,
}

impl GraphIndex {
    /// 创建一个新的图索引
    pub fn new() -> Self {
        GraphIndex {
            adjacency: HashMap::new(),
        }
    }

    /// 添加边（从 from 到 to）
    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.adjacency
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());

        // 确保 to 节点存在（即使没有出边）
        self.adjacency.entry(to.to_string()).or_default();
    }

    /// 添加双向边
    pub fn add_undirected_edge(&mut self, a: &str, b: &str) {
        self.add_edge(a, b);
        self.add_edge(b, a);
    }

    /// 搜索图索引
    ///
    /// 使用 BFS + PageRank 组合搜索，返回最相关的节点。
    pub fn search(&self, query: &str, top_k: usize) -> Vec<GraphIndexResult> {
        if self.adjacency.is_empty() || query.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();

        // 1. 找到匹配查询的起始节点
        let start_nodes: Vec<String> = self
            .adjacency
            .keys()
            .filter(|k| k.contains(query) || k.eq_ignore_ascii_case(query))
            .cloned()
            .collect();

        if start_nodes.is_empty() {
            return Vec::new();
        }

        // 2. 从每个起始节点进行 BFS
        for start in &start_nodes {
            let bfs_results = self.bfs(start, 3);
            for r in bfs_results {
                results.push(r);
            }
        }

        // 3. 计算 PageRank
        let nodes: Vec<String> = self.adjacency.keys().cloned().collect();
        let pr = self.pagerank(&nodes, 10, 0.85);

        // 合并 BFS 和 PageRank 得分
        for result in &mut results {
            if let Some(&pr_score) = pr.get(&result.node) {
                result.score = result.score * 0.5 + pr_score * 0.5;
            }
        }

        // 排序去重
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.dedup_by(|a, b| a.node == b.node);
        results.truncate(top_k);

        results
    }

    /// BFS 遍历
    ///
    /// 从起始节点开始广度优先搜索，返回可达节点及其路径。
    pub fn bfs(&self, start: &str, max_depth: usize) -> Vec<GraphIndexResult> {
        let mut results = Vec::new();
        if !self.adjacency.contains_key(start) {
            return results;
        }
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((start.to_string(), vec![start.to_string()], 0));
        visited.insert(start.to_string());

        while let Some((node, path, depth)) = queue.pop_front() {
            let score = 1.0 / (depth as f64 + 1.0);
            results.push(GraphIndexResult {
                node: node.clone(),
                score,
                path: path.clone(),
                depth,
            });

            if depth >= max_depth {
                continue;
            }

            if let Some(neighbors) = self.adjacency.get(&node) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor.clone());
                        let mut new_path = path.clone();
                        new_path.push(neighbor.clone());
                        queue.push_back((neighbor.clone(), new_path, depth + 1));
                    }
                }
            }
        }

        results
    }

    /// PageRank 算法
    ///
    /// 计算图中每个节点的 PageRank 得分。
    pub fn pagerank(
        &self,
        nodes: &[String],
        iterations: usize,
        damping_factor: f64,
    ) -> HashMap<String, f64> {
        let n = nodes.len();
        if n == 0 {
            return HashMap::new();
        }

        // 初始化排名
        let initial_rank = 1.0 / n as f64;
        let mut ranks: HashMap<String, f64> = nodes.iter().map(|n| (n.clone(), initial_rank)).collect();

        // 构建反向邻接表（入边）
        let mut incoming: HashMap<String, Vec<String>> = HashMap::new();
        for node in nodes {
            incoming.insert(node.clone(), Vec::new());
        }
        for (from, to_list) in &self.adjacency {
            for to in to_list {
                if let Some(incoming_list) = incoming.get_mut(to) {
                    incoming_list.push(from.clone());
                }
            }
        }

        for _ in 0..iterations {
            let mut new_ranks: HashMap<String, f64> = HashMap::new();

            for node in nodes {
                let mut sum = 0.0;
                if let Some(incoming_list) = incoming.get(node) {
                    for in_node in incoming_list {
                        let out_degree = self.adjacency.get(in_node).map(|v| v.len()).unwrap_or(0) as f64;
                        if out_degree > 0.0 {
                            sum += ranks.get(in_node).copied().unwrap_or(0.0) / out_degree;
                        }
                    }
                }

                let rank = (1.0 - damping_factor) / n as f64 + damping_factor * sum;
                new_ranks.insert(node.clone(), rank);
            }

            ranks = new_ranks;
        }

        ranks
    }

    /// 生成 Mermaid 流程图
    pub fn to_mermaid(&self) -> String {
        let mut output = String::from("graph LR\n");

        for (from, to_list) in &self.adjacency {
            for to in to_list {
                output.push_str(&format!("    {}[\"{}\"] --> {}[\"{}\"]\n", from, from, to, to));
            }
        }

        output
    }

    /// 获取节点数
    pub fn node_count(&self) -> usize {
        self.adjacency.len()
    }

    /// 获取边数
    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|v| v.len()).sum()
    }

    /// 检查节点是否存在
    pub fn has_node(&self, node: &str) -> bool {
        self.adjacency.contains_key(node)
    }

    /// 获取节点的邻居
    pub fn neighbors(&self, node: &str) -> Vec<&str> {
        self.adjacency
            .get(node)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_edge() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        assert!(idx.has_node("a"));
        assert!(idx.has_node("b"));
        assert_eq!(idx.edge_count(), 1);
    }

    #[test]
    fn test_add_undirected_edge() {
        let mut idx = GraphIndex::new();
        idx.add_undirected_edge("a", "b");
        assert_eq!(idx.node_count(), 2);
        assert_eq!(idx.edge_count(), 2);
    }

    #[test]
    fn test_bfs() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("b", "c");
        idx.add_edge("b", "d");

        let results = idx.bfs("a", 3);
        assert_eq!(results.len(), 4); // a, b, c, d
        assert!(results.iter().any(|r| r.node == "a"));
        assert!(results.iter().any(|r| r.node == "b"));
        assert!(results.iter().any(|r| r.node == "c"));
        assert!(results.iter().any(|r| r.node == "d"));
    }

    #[test]
    fn test_bfs_max_depth() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("b", "c");
        idx.add_edge("c", "d");

        let results = idx.bfs("a", 1);
        // 深度 1：a, b
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.depth <= 1));
    }

    #[test]
    fn test_bfs_start_not_found() {
        let idx = GraphIndex::new();
        let results = idx.bfs("nonexistent", 3);
        assert!(results.is_empty());
    }

    #[test]
    fn test_pagerank() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("b", "c");
        idx.add_edge("c", "a");

        let nodes: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let ranks = idx.pagerank(&nodes, 20, 0.85);

        assert_eq!(ranks.len(), 3);
        // 在循环图中，所有节点的 PageRank 应该接近相等
        let scores: Vec<f64> = ranks.values().copied().collect();
        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        for score in &scores {
            assert!((score - avg).abs() < 0.05);
        }
    }

    #[test]
    fn test_pagerank_empty() {
        let idx = GraphIndex::new();
        let ranks = idx.pagerank(&[], 10, 0.85);
        assert!(ranks.is_empty());
    }

    #[test]
    fn test_pagerank_dangling() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("a", "c");
        // b 和 c 没有出边（悬挂节点）

        let nodes: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let ranks = idx.pagerank(&nodes, 10, 0.85);

        assert_eq!(ranks.len(), 3);
        // 在标准 PageRank 中，有入边的节点（b, c）得分高于无入边的节点（a）
        // 因为 a 的 rank 只来自 damping factor，而 b 和 c 从 a 获得 rank 传递
        assert!(ranks["b"] > 0.0);
        assert!(ranks["c"] > 0.0);
    }

    #[test]
    fn test_search() {
        let mut idx = GraphIndex::new();
        idx.add_edge("main", "helper");
        idx.add_edge("main", "parse");
        idx.add_edge("helper", "utils");

        let results = idx.search("main", 5);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.node == "main"));
    }

    #[test]
    fn test_search_empty() {
        let idx = GraphIndex::new();
        let results = idx.search("test", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_to_mermaid() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("b", "c");

        let mermaid = idx.to_mermaid();
        assert!(mermaid.contains("graph LR"));
        assert!(mermaid.contains("a[\"a\"] --> b[\"b\"]"));
        assert!(mermaid.contains("b[\"b\"] --> c[\"c\"]"));
    }

    #[test]
    fn test_neighbors() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("a", "c");

        let neighbors = idx.neighbors("a");
        assert_eq!(neighbors.len(), 2);
        assert!(neighbors.contains(&"b"));
        assert!(neighbors.contains(&"c"));
    }

    #[test]
    fn test_node_count() {
        let mut idx = GraphIndex::new();
        idx.add_edge("a", "b");
        idx.add_edge("a", "c");
        assert_eq!(idx.node_count(), 3); // a, b, c
    }
}