//! 代码图与 MECE 模块化
//!
//! 提供代码图结构，支持节点和边的添加、调用图/依赖图查询，
//! 以及基于 MECE 原则的模块化分组。

use std::collections::{HashMap, HashSet};

/// 节点类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Function,
    Class,
    Module,
    File,
}

/// 图节点
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: usize,
    pub name: String,
    pub kind: NodeKind,
    pub file_path: String,
    pub line: usize,
}

/// 边类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Calls,
    Implements,
    Extends,
    Contains,
    References,
}

/// 图边
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from_id: usize,
    pub to_id: usize,
    pub edge_kind: EdgeKind,
}

/// 代码图
#[derive(Debug, Clone, Default)]
pub struct CodeGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    node_id_counter: usize,
}

impl CodeGraph {
    /// 创建一个新的代码图
    pub fn new() -> Self {
        CodeGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
            node_id_counter: 0,
        }
    }

    /// 添加节点，返回节点 ID
    pub fn add_node(&mut self, name: &str, kind: NodeKind, file_path: &str, line: usize) -> usize {
        let id = self.node_id_counter;
        self.node_id_counter += 1;
        self.nodes.push(GraphNode {
            id,
            name: name.to_string(),
            kind,
            file_path: file_path.to_string(),
            line,
        });
        id
    }

    /// 添加边
    pub fn add_edge(&mut self, from_id: usize, to_id: usize, edge_kind: EdgeKind) {
        // 检查节点是否存在
        let from_exists = self.nodes.iter().any(|n| n.id == from_id);
        let to_exists = self.nodes.iter().any(|n| n.id == to_id);
        if !from_exists || !to_exists {
            return;
        }
        // 避免重复边
        if self.edges.iter().any(|e| e.from_id == from_id && e.to_id == to_id && e.edge_kind == edge_kind) {
            return;
        }
        self.edges.push(GraphEdge {
            from_id,
            to_id,
            edge_kind,
        });
    }

    /// 获取调用图（Calls 类型的边）
    pub fn get_call_graph(&self) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.edge_kind == EdgeKind::Calls)
            .collect()
    }

    /// 获取依赖图（所有非 Calls 类型的边）
    pub fn get_dependency_graph(&self) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.edge_kind != EdgeKind::Calls)
            .collect()
    }

    /// MECE 模块化分组
    ///
    /// 按照功能（而非文件）对节点进行分组，遵循 MECE（Mutually Exclusive, Collectively Exhaustive）原则。
    /// 分组策略：
    /// 1. 每个模块（Module 类型节点）作为一个独立分组
    /// 2. 未被模块包含的函数/类按文件分组
    /// 3. 每个节点只属于一个分组
    pub fn mece_modularize(&self) -> HashMap<String, Vec<&GraphNode>> {
        let mut groups: HashMap<String, Vec<&GraphNode>> = HashMap::new();
        let mut assigned: HashSet<usize> = HashSet::new();

        // 1. 按模块分组
        let module_nodes: Vec<&GraphNode> = self.nodes.iter().filter(|n| n.kind == NodeKind::Module).collect();
        for module in &module_nodes {
            let group_name = format!("module:{}", module.name);
            let mut members = Vec::new();
            members.push(*module);
            assigned.insert(module.id);

            // 查找通过 Contains 边关联到此模块的节点
            let contained_ids: HashSet<usize> = self
                .edges
                .iter()
                .filter(|e| e.edge_kind == EdgeKind::Contains && e.from_id == module.id)
                .map(|e| e.to_id)
                .collect();

            for node in &self.nodes {
                if contained_ids.contains(&node.id) && !assigned.contains(&node.id) {
                    members.push(node);
                    assigned.insert(node.id);
                }
            }

            groups.insert(group_name, members);
        }

        // 2. 按文件分组未分配的节点
        let mut file_groups: HashMap<String, Vec<&GraphNode>> = HashMap::new();
        for node in &self.nodes {
            if assigned.contains(&node.id) {
                continue;
            }
            file_groups
                .entry(node.file_path.clone())
                .or_default()
                .push(node);
            assigned.insert(node.id);
        }

        for (file, members) in file_groups {
            groups.insert(format!("file:{}", file), members);
        }

        groups
    }

    /// 生成 Mermaid 流程图
    pub fn to_mermaid(&self) -> String {
        let mut output = String::from("graph TD\n");

        // 添加节点定义
        for node in &self.nodes {
            let label = match node.kind {
                NodeKind::Function => format!("{}({})", node.name, "fn"),
                NodeKind::Class => format!("{}({})", node.name, "cls"),
                NodeKind::Module => format!("{}({})", node.name, "mod"),
                NodeKind::File => format!("{}({})", node.name, "file"),
            };
            output.push_str(&format!("    n{}[\"{}\"]\n", node.id, label));
        }

        // 添加边
        for edge in &self.edges {
            let style = match edge.edge_kind {
                EdgeKind::Calls => "-- calls -->",
                EdgeKind::Implements => "-- implements -->",
                EdgeKind::Extends => "-- extends -->",
                EdgeKind::Contains => "-- contains -->",
                EdgeKind::References => "-- references -->",
            };
            output.push_str(&format!("    n{} {} n{}\n", edge.from_id, style, edge.to_id));
        }

        output
    }

    /// 获取节点的所有邻居
    pub fn neighbors(&self, node_id: usize) -> Vec<&GraphNode> {
        let neighbor_ids: HashSet<usize> = self
            .edges
            .iter()
            .filter(|e| e.from_id == node_id)
            .map(|e| e.to_id)
            .collect();

        self.nodes
            .iter()
            .filter(|n| neighbor_ids.contains(&n.id))
            .collect()
    }

    /// 获取所有节点
    pub fn nodes(&self) -> &[GraphNode] {
        &self.nodes
    }

    /// 获取所有边
    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }

    /// 查找从起始节点出发的完整调用链
    pub fn call_chain(&self, start_id: usize) -> Vec<Vec<usize>> {
        let mut chains = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        stack.push(start_id);

        self.dfs_call_chain(start_id, &mut visited, &mut Vec::new(), &mut chains);
        chains
    }

    fn dfs_call_chain(
        &self,
        current: usize,
        visited: &mut HashSet<usize>,
        path: &mut Vec<usize>,
        chains: &mut Vec<Vec<usize>>,
    ) {
        if visited.contains(&current) {
            return;
        }
        visited.insert(current);
        path.push(current);

        let callees: Vec<usize> = self
            .edges
            .iter()
            .filter(|e| e.from_id == current && e.edge_kind == EdgeKind::Calls)
            .map(|e| e.to_id)
            .collect();

        if callees.is_empty() {
            chains.push(path.clone());
        } else {
            for &callee in &callees {
                self.dfs_call_chain(callee, visited, path, chains);
            }
        }

        path.pop();
        visited.remove(&current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_node() {
        let mut graph = CodeGraph::new();
        let id = graph.add_node("main", NodeKind::Function, "main.rs", 1);
        assert_eq!(id, 0);
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].name, "main");
    }

    #[test]
    fn test_add_edge() {
        let mut graph = CodeGraph::new();
        let id1 = graph.add_node("main", NodeKind::Function, "main.rs", 1);
        let id2 = graph.add_node("helper", NodeKind::Function, "main.rs", 5);
        graph.add_edge(id1, id2, EdgeKind::Calls);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from_id, id1);
        assert_eq!(graph.edges[0].to_id, id2);
    }

    #[test]
    fn test_add_edge_invalid_node() {
        let mut graph = CodeGraph::new();
        graph.add_edge(0, 1, EdgeKind::Calls);
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_get_call_graph() {
        let mut graph = CodeGraph::new();
        let id1 = graph.add_node("main", NodeKind::Function, "main.rs", 1);
        let id2 = graph.add_node("helper", NodeKind::Function, "main.rs", 5);
        graph.add_edge(id1, id2, EdgeKind::Calls);
        graph.add_edge(id1, id2, EdgeKind::References);

        let calls = graph.get_call_graph();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].edge_kind, EdgeKind::Calls);
    }

    #[test]
    fn test_mece_modularize() {
        let mut graph = CodeGraph::new();
        let mod_id = graph.add_node("core", NodeKind::Module, "core.rs", 1);
        let fn1 = graph.add_node("init", NodeKind::Function, "core.rs", 3);
        let fn2 = graph.add_node("process", NodeKind::Function, "core.rs", 10);

        graph.add_edge(mod_id, fn1, EdgeKind::Contains);
        graph.add_edge(mod_id, fn2, EdgeKind::Contains);

        let groups = graph.mece_modularize();
        // 应该有模块组 module:core
        assert!(groups.contains_key("module:core"));
        // module:core 应该包含模块节点和两个函数
        assert_eq!(groups["module:core"].len(), 3);
    }

    #[test]
    fn test_to_mermaid() {
        let mut graph = CodeGraph::new();
        let id1 = graph.add_node("main", NodeKind::Function, "main.rs", 1);
        let id2 = graph.add_node("helper", NodeKind::Function, "main.rs", 5);
        graph.add_edge(id1, id2, EdgeKind::Calls);

        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("n0"));
        assert!(mermaid.contains("n1"));
        assert!(mermaid.contains("calls"));
    }

    #[test]
    fn test_neighbors() {
        let mut graph = CodeGraph::new();
        let id1 = graph.add_node("main", NodeKind::Function, "main.rs", 1);
        let id2 = graph.add_node("helper", NodeKind::Function, "main.rs", 5);
        graph.add_edge(id1, id2, EdgeKind::Calls);

        let neighbors = graph.neighbors(id1);
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].name, "helper");
    }

    #[test]
    fn test_call_chain() {
        let mut graph = CodeGraph::new();
        let a = graph.add_node("a", NodeKind::Function, "lib.rs", 1);
        let b = graph.add_node("b", NodeKind::Function, "lib.rs", 5);
        let c = graph.add_node("c", NodeKind::Function, "lib.rs", 10);
        graph.add_edge(a, b, EdgeKind::Calls);
        graph.add_edge(b, c, EdgeKind::Calls);

        let chains = graph.call_chain(a);
        assert!(!chains.is_empty());
        // 应该有一条路径 a -> b -> c
        assert!(chains.iter().any(|chain| chain.len() == 3));
    }

    #[test]
    fn test_empty_graph() {
        let graph = CodeGraph::new();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.get_call_graph().is_empty());
        assert!(graph.to_mermaid().contains("graph TD"));
    }
}