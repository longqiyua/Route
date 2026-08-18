//! Project Knowledge Map — lightweight relationship graph over project memory.
//!
//! Upgrades ProjectMemory from a flat list to a graph of Nodes and Edges.
//! Uses only existing structured data + explicit candidates — no embedding/RAG.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Knowledge map directory under `.route/`.
pub fn knowledge_map_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("knowledge_map")
}

/// Path to the knowledge map file.
pub fn knowledge_map_path(project_root: &Path) -> PathBuf {
    knowledge_map_dir(project_root).join("map.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The kind of a knowledge node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A project module or component
    Module,
    /// A design or architectural decision
    Decision,
    /// An invariant that must always hold
    Invariant,
    /// A problem that was encountered
    Problem,
    /// An attempt that was made (may have failed)
    Attempt,
    /// A reference to an external resource
    Reference,
    /// A task that was performed
    Task,
    /// A person or role
    Person,
    /// Other — for extension
    Other,
}

impl Default for NodeKind {
    fn default() -> Self {
        Self::Other
    }
}

/// The kind of relationship between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Node A depends on Node B
    DependsOn,
    /// Node A was changed by Node B (task, decision)
    ChangedBy,
    /// Node A was decided because of Node B
    DecidedBecause,
    /// Node A supersedes Node B
    Supersedes,
    /// Node A failed because of Node B
    FailedBecause,
    /// Node A is related to Node B (unspecified relationship)
    RelatedTo,
}

impl Default for EdgeKind {
    fn default() -> Self {
        Self::RelatedTo
    }
}

/// A node in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeNode {
    /// Unique node ID
    pub id: String,
    /// Human-readable label
    pub label: String,
    /// Kind of node
    #[serde(default)]
    pub kind: NodeKind,
    /// Optional description
    #[serde(default)]
    pub description: String,
    /// Source of this node (memory item ID, reference ID, etc.)
    #[serde(default)]
    pub source: String,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// An edge between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEdge {
    /// Source node ID
    pub source: String,
    /// Target node ID
    pub target: String,
    /// Kind of relationship
    #[serde(default)]
    pub kind: EdgeKind,
    /// Optional description of the relationship
    #[serde(default)]
    pub description: String,
    /// Source evidence (memory item, session, etc.)
    #[serde(default)]
    pub evidence: String,
}

/// The knowledge map — a graph of nodes and edges.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeMap {
    pub nodes: Vec<KnowledgeNode>,
    pub edges: Vec<KnowledgeEdge>,
}

impl KnowledgeMap {
    /// Load the knowledge map from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = knowledge_map_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save the knowledge map to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = knowledge_map_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(knowledge_map_path(project_root), json)?;
        Ok(())
    }

    /// Add a node.
    pub fn add_node(&mut self, node: KnowledgeNode) {
        if !self.nodes.iter().any(|n| n.id == node.id) {
            self.nodes.push(node);
        }
    }

    /// Add an edge.
    pub fn add_edge(&mut self, edge: KnowledgeEdge) {
        self.edges.push(edge);
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&KnowledgeNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Find nodes related to a given node (via edges).
    pub fn related_nodes(&self, node_id: &str) -> Vec<(&KnowledgeNode, &KnowledgeEdge)> {
        let mut result = Vec::new();
        for edge in &self.edges {
            if edge.source == node_id {
                if let Some(target) = self.get_node(&edge.target) {
                    result.push((target, edge));
                }
            } else if edge.target == node_id {
                if let Some(source) = self.get_node(&edge.source) {
                    result.push((source, edge));
                }
            }
        }
        result
    }

    /// Find all nodes that would be affected by a change to a given module.
    pub fn impact_scope(&self, module_id: &str) -> Vec<&KnowledgeNode> {
        let mut affected = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![module_id.to_string()];

        while let Some(current) = queue.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            for (related, edge) in self.related_nodes(&current) {
                if edge.kind == EdgeKind::DependsOn || edge.kind == EdgeKind::RelatedTo {
                    if visited.insert(related.id.clone()) {
                        affected.push(related);
                        queue.push(related.id.clone());
                    }
                }
            }
        }

        affected
    }

    /// Build a map from project memory items.
    pub fn build_from_memory(&mut self, memory: &crate::memory::ProjectMemory) {
        // Add all items as nodes
        for item in memory.all_items() {
            let kind = match item.kind {
                crate::memory::MemoryItemKind::Current => NodeKind::Other,
                crate::memory::MemoryItemKind::Decision => NodeKind::Decision,
                crate::memory::MemoryItemKind::FailedAttempt => NodeKind::Attempt,
                crate::memory::MemoryItemKind::Invariant => NodeKind::Invariant,
                crate::memory::MemoryItemKind::OpenQuestion => NodeKind::Problem,
                crate::memory::MemoryItemKind::Historical => NodeKind::Other,
            };

            self.add_node(KnowledgeNode {
                id: item.id.clone(),
                label: item.content.chars().take(60).collect(),
                kind,
                description: item.content.clone(),
                source: format!("memory:{}", item.id),
                tags: item.tags.clone(),
            });
        }

        // Create edges from supersede relationships
        for item in memory.all_items() {
            if let Some(superseded) = &item.supersedes {
                self.add_edge(KnowledgeEdge {
                    source: item.id.clone(),
                    target: superseded.clone(),
                    kind: EdgeKind::Supersedes,
                    description: "Newer item supersedes older one".to_string(),
                    evidence: format!("memory:{}", item.id),
                });
            }
        }
    }

    /// Render the map as a simple text graph.
    pub fn render_map(&self, topic: Option<&str>) -> String {
        let mut out = String::new();

        let nodes: Vec<&KnowledgeNode> = if let Some(t) = topic {
            let t_lower = t.to_lowercase();
            self.nodes
                .iter()
                .filter(|n| {
                    n.label.to_lowercase().contains(&t_lower)
                        || n.description.to_lowercase().contains(&t_lower)
                        || n.tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&t_lower))
                })
                .collect()
        } else {
            self.nodes.iter().collect()
        };

        for node in &nodes {
            out.push_str(&format!(
                "◉ {label} ({kind:?}) [{id}]\n",
                label = node.label,
                kind = node.kind,
                id = node.id
            ));

            // Find edges from this node
            let edges: Vec<&KnowledgeEdge> =
                self.edges.iter().filter(|e| e.source == node.id).collect();

            for edge in &edges {
                if let Some(target) = self.get_node(&edge.target) {
                    out.push_str(&format!(
                        "  └─ {kind:?} → {label} [{id}]\n",
                        kind = edge.kind,
                        label = target.label,
                        id = target.id
                    ));
                }
            }
        }

        if nodes.is_empty() {
            out.push_str("(no nodes found)\n");
        }

        out
    }
}

/// Format a node for display.
pub fn format_node(node: &KnowledgeNode) -> String {
    format!(
        "[{id}] {label}
  Kind:   {kind:?}
  Desc:   {desc}
  Source: {source}
  Tags:   {tags}",
        id = node.id,
        label = node.label,
        kind = node.kind,
        desc = node.description,
        source = node.source,
        tags = node.tags.join(", "),
    )
}
