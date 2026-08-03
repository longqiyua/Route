use std::collections::HashMap;

use crate::chain::CausalChain;
use crate::memory::MemoryEntry;
use crate::project::ProjectMeta;
use crate::structure::ProjectStructure;

/// 输出格式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Json,
    Markdown,
    Mermaid,
}

/// 输出写入器 trait
pub trait OutputWriter {
    /// 写入 JSON 格式输出
    fn write_json(&self, output: &str) -> String;
    /// 写入 Markdown 格式输出
    fn write_markdown(&self, output: &str) -> String;
    /// 写入 Mermaid 格式输出
    fn write_mermaid(&self, output: &str) -> String;
}

/// 一个简单的默认输出写入器实现
pub struct DefaultOutputWriter;

impl OutputWriter for DefaultOutputWriter {
    fn write_json(&self, output: &str) -> String {
        output.to_string()
    }

    fn write_markdown(&self, output: &str) -> String {
        output.to_string()
    }

    fn write_mermaid(&self, output: &str) -> String {
        output.to_string()
    }
}

/// 记忆输出包装器，包裹 RootMemory 数据
pub struct MemoryOutput<'a> {
    pub project: &'a ProjectMeta,
    pub structure: Option<&'a ProjectStructure>,
    pub chain: &'a CausalChain,
    pub entries: &'a [MemoryEntry],
}

impl<'a> MemoryOutput<'a> {
    pub fn new(
        project: &'a ProjectMeta,
        structure: Option<&'a ProjectStructure>,
        chain: &'a CausalChain,
        entries: &'a [MemoryEntry],
    ) -> Self {
        Self {
            project,
            structure,
            chain,
            entries,
        }
    }

    /// 生成结构化 JSON 输出
    pub fn to_json(&self) -> String {
        let mut map = serde_json::Map::new();

        // 项目元数据
        let mut project_map = serde_json::Map::new();
        project_map.insert(
            "name".to_string(),
            serde_json::Value::String(self.project.name.clone()),
        );
        project_map.insert(
            "description".to_string(),
            serde_json::Value::String(self.project.description.clone()),
        );
        project_map.insert(
            "language".to_string(),
            serde_json::Value::String(self.project.language.clone()),
        );
        project_map.insert(
            "version".to_string(),
            serde_json::Value::String(self.project.version.clone()),
        );
        project_map.insert(
            "created_at".to_string(),
            serde_json::Value::String(self.project.created_at.clone()),
        );
        project_map.insert(
            "updated_at".to_string(),
            serde_json::Value::String(self.project.updated_at.clone()),
        );
        if let Some(ref fw) = self.project.framework {
            project_map.insert(
                "framework".to_string(),
                serde_json::Value::String(fw.clone()),
            );
        }
        project_map.insert(
            "entry_points".to_string(),
            serde_json::Value::Array(
                self.project
                    .entry_points
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        );
        project_map.insert(
            "key_modules".to_string(),
            serde_json::Value::Array(
                self.project
                    .key_modules
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        );
        project_map.insert(
            "constraints".to_string(),
            serde_json::Value::Array(
                self.project
                    .constraints
                    .iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ),
        );
        map.insert(
            "project".to_string(),
            serde_json::Value::Object(project_map),
        );

        // 结构信息
        if let Some(structure) = self.structure {
            let mut struct_map = serde_json::Map::new();
            struct_map.insert(
                "total_files".to_string(),
                serde_json::Value::Number(serde_json::Number::from(structure.total_files)),
            );
            struct_map.insert(
                "total_dirs".to_string(),
                serde_json::Value::Number(serde_json::Number::from(structure.total_dirs)),
            );

            let mut lang_map = serde_json::Map::new();
            for (lang, count) in &structure.languages {
                lang_map.insert(
                    lang.clone(),
                    serde_json::Value::Number(serde_json::Number::from(*count)),
                );
            }
            struct_map.insert(
                "languages".to_string(),
                serde_json::Value::Object(lang_map),
            );

            // 收集模块/符号信息
            let mut modules = Vec::new();
            collect_modules(&structure.root, &mut modules, 0);
            struct_map.insert(
                "modules".to_string(),
                serde_json::Value::Array(
                    modules
                        .into_iter()
                        .map(|m| serde_json::Value::String(m))
                        .collect(),
                ),
            );

            map.insert(
                "structure".to_string(),
                serde_json::Value::Object(struct_map),
            );
        }

        // 因果链
        let mut chain_list = Vec::new();
        for link in &self.chain.links {
            let mut link_map = serde_json::Map::new();
            link_map.insert(
                "id".to_string(),
                serde_json::Value::String(link.id.clone()),
            );
            if let Some(ref pid) = link.parent_id {
                link_map.insert(
                    "parent_id".to_string(),
                    serde_json::Value::String(pid.clone()),
                );
            }
            link_map.insert(
                "action".to_string(),
                serde_json::Value::String(link.action.clone()),
            );
            link_map.insert(
                "effect".to_string(),
                serde_json::Value::String(link.effect.clone()),
            );
            link_map.insert(
                "reason".to_string(),
                serde_json::Value::String(link.reason.clone()),
            );
            link_map.insert(
                "status".to_string(),
                serde_json::Value::String(format!("{:?}", link.status)),
            );
            link_map.insert(
                "timestamp".to_string(),
                serde_json::Value::String(link.timestamp.clone()),
            );
            link_map.insert(
                "actor".to_string(),
                serde_json::Value::String(link.actor.clone()),
            );
            link_map.insert(
                "side_effects".to_string(),
                serde_json::Value::Array(
                    link.side_effects
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
            chain_list.push(serde_json::Value::Object(link_map));
        }
        map.insert(
            "causal_chains".to_string(),
            serde_json::Value::Array(chain_list),
        );

        // 条目统计
        let mut entries_by_kind: HashMap<String, usize> = HashMap::new();
        for entry in self.entries {
            let kind = format!("{:?}", entry.kind);
            *entries_by_kind.entry(kind).or_insert(0) += 1;
        }
        let mut entries_map = serde_json::Map::new();
        entries_map.insert(
            "total".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.entries.len())),
        );
        let mut kind_map = serde_json::Map::new();
        for (k, v) in &entries_by_kind {
            kind_map.insert(
                k.clone(),
                serde_json::Value::Number(serde_json::Number::from(*v)),
            );
        }
        entries_map.insert(
            "by_kind".to_string(),
            serde_json::Value::Object(kind_map),
        );
        map.insert(
            "entries".to_string(),
            serde_json::Value::Object(entries_map),
        );

        // 索引统计
        let mut index_stats = serde_json::Map::new();
        index_stats.insert(
            "total_entries".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.entries.len())),
        );
        index_stats.insert(
            "total_links".to_string(),
            serde_json::Value::Number(serde_json::Number::from(self.chain.links.len())),
        );
        map.insert(
            "index_stats".to_string(),
            serde_json::Value::Object(index_stats),
        );

        serde_json::to_string_pretty(&serde_json::Value::Object(map))
            .unwrap_or_else(|_| "{}".to_string())
    }

    /// 生成 Markdown 输出（包含 Mermaid 代码块）
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        // 项目概览
        md.push_str("# Project Memory Report\n\n");
        md.push_str("## Project Overview\n\n");
        md.push_str(&format!("- **Name**: {}\n", self.project.name));
        md.push_str(&format!("- **Description**: {}\n", self.project.description));
        md.push_str(&format!("- **Language**: {}\n", self.project.language));
        if let Some(ref fw) = self.project.framework {
            md.push_str(&format!("- **Framework**: {}\n", fw));
        }
        md.push_str(&format!("- **Version**: {}\n", self.project.version));
        md.push_str(&format!("- **Created**: {}\n", self.project.created_at));
        md.push_str(&format!("- **Updated**: {}\n", self.project.updated_at));

        // 关键模块
        if !self.project.key_modules.is_empty() {
            md.push_str("\n### Key Modules\n\n");
            for km in &self.project.key_modules {
                md.push_str(&format!("- {}\n", km));
            }
        }

        // 入口点
        if !self.project.entry_points.is_empty() {
            md.push_str("\n### Entry Points\n\n");
            for ep in &self.project.entry_points {
                md.push_str(&format!("- {}\n", ep));
            }
        }

        // 架构图（Mermaid）
        md.push_str("\n## Architecture Diagram\n\n");
        md.push_str("```mermaid\n");
        if let Some(structure) = self.structure {
            md.push_str(&structure.to_mermaid());
        }
        md.push_str("```\n");

        // 项目 Mermaid
        md.push_str("\n### Project Structure\n\n");
        md.push_str("```mermaid\n");
        md.push_str(&self.project.to_mermaid());
        md.push_str("```\n");

        // 因果链
        if !self.chain.links.is_empty() {
            md.push_str("\n## Change History\n\n");
            md.push_str("```mermaid\n");
            md.push_str(&self.chain.to_mermaid());
            md.push_str("```\n");

            md.push_str("\n### Change Log\n\n");
            md.push_str("| ID | Action | Status | Timestamp | Actor |\n");
            md.push_str("|---|---|---|---|---|\n");
            for link in &self.chain.links {
                md.push_str(&format!(
                    "| {} | {} | {:?} | {} | {} |\n",
                    link.id, link.action, link.status, link.timestamp, link.actor
                ));
            }
        }

        // 记忆条目统计
        if !self.entries.is_empty() {
            md.push_str("\n## Memory Entries\n\n");
            md.push_str(&format!("- **Total entries**: {}\n", self.entries.len()));

            let mut by_kind: HashMap<String, usize> = HashMap::new();
            for entry in self.entries {
                let kind = format!("{:?}", entry.kind);
                *by_kind.entry(kind).or_insert(0) += 1;
            }
            md.push_str("- **By kind**:\n");
            for (kind, count) in by_kind {
                md.push_str(&format!("  - {}: {}\n", kind, count));
            }
        }

        md
    }

    /// 生成结构 Mermaid 图
    pub fn to_mermaid_structure(&self) -> String {
        if let Some(structure) = self.structure {
            structure.to_mermaid()
        } else {
            "graph TD\n    NoStructure[\"No structure data\"]\n".to_string()
        }
    }

    /// 生成因果链 Mermaid 图
    pub fn to_mermaid_chain(&self) -> String {
        self.chain.to_mermaid()
    }
}

/// 收集模块路径（递归遍历目录节点，最多 2 层深度）
fn collect_modules(node: &crate::structure::FileNode, modules: &mut Vec<String>, depth: usize) {
    if node.is_dir && depth <= 2 {
        if depth > 0 {
            modules.push(node.path.clone());
        }
        for child in &node.children {
            collect_modules(child, modules, depth + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{CausalLink, ChainStatus};
    use crate::memory::MemoryKind;
    use crate::structure::FileNode;

    fn make_project() -> ProjectMeta {
        ProjectMeta {
            name: "test-project".to_string(),
            description: "A test project".to_string(),
            language: "Rust".to_string(),
            framework: Some("Actix".to_string()),
            constraints: vec!["no unsafe".to_string()],
            entry_points: vec!["src/main.rs".to_string()],
            key_modules: vec!["core".to_string(), "cli".to_string()],
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            version: "0.1.0".to_string(),
        }
    }

    fn make_structure() -> ProjectStructure {
        ProjectStructure {
            root: FileNode {
                path: ".".to_string(),
                is_dir: true,
                children: vec![
                    FileNode {
                        path: "src".to_string(),
                        is_dir: true,
                        children: vec![
                            FileNode {
                                path: "src/main.rs".to_string(),
                                is_dir: false,
                                children: vec![],
                                size: 100,
                                last_modified: "2024-01-01T00:00:00Z".to_string(),
                            },
                            FileNode {
                                path: "src/lib.rs".to_string(),
                                is_dir: false,
                                children: vec![],
                                size: 200,
                                last_modified: "2024-01-01T00:00:00Z".to_string(),
                            },
                        ],
                        size: 0,
                        last_modified: "2024-01-01T00:00:00Z".to_string(),
                    },
                ],
                size: 0,
                last_modified: "2024-01-01T00:00:00Z".to_string(),
            },
            total_files: 2,
            total_dirs: 1,
            languages: [("rs".to_string(), 2)].into(),
        }
    }

    fn make_chain() -> CausalChain {
        use std::path::PathBuf;
        CausalChain {
            project_path: PathBuf::from("/test"),
            links: vec![CausalLink {
                id: "c1".to_string(),
                parent_id: None,
                action: "refactor".to_string(),
                file_path: Some("src/main.rs".to_string()),
                reason: "Improve structure".to_string(),
                effect: "Split into modules".to_string(),
                side_effects: vec!["src/lib.rs".to_string()],
                status: ChainStatus::Executed,
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                actor: "dev".to_string(),
            }],
        }
    }

    fn make_entries() -> Vec<MemoryEntry> {
        vec![
            MemoryEntry {
                id: "e1".to_string(),
                kind: MemoryKind::Decision,
                key: "arch".to_string(),
                content: "Use Actix framework".to_string(),
                tags: vec!["architecture".to_string()],
                created_at: "2024-01-01T00:00:00Z".to_string(),
                updated_at: "2024-01-01T00:00:00Z".to_string(),
                source: "user".to_string(),
            },
            MemoryEntry {
                id: "e2".to_string(),
                kind: MemoryKind::Context,
                key: "goal".to_string(),
                content: "Build a CLI tool".to_string(),
                tags: vec!["goal".to_string()],
                created_at: "2024-01-02T00:00:00Z".to_string(),
                updated_at: "2024-01-02T00:00:00Z".to_string(),
                source: "system".to_string(),
            },
        ]
    }

    #[test]
    fn test_to_json_contains_project() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let json_str = output.to_json();

        assert!(json_str.contains("test-project"));
        assert!(json_str.contains("Rust"));
        assert!(json_str.contains("Actix"));
    }

    #[test]
    fn test_to_json_contains_structure() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let json_str = output.to_json();

        assert!(json_str.contains("total_files"));
        assert!(json_str.contains("total_dirs"));
    }

    #[test]
    fn test_to_json_contains_chains() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let json_str = output.to_json();

        assert!(json_str.contains("causal_chains"));
        assert!(json_str.contains("refactor"));
    }

    #[test]
    fn test_to_json_contains_index_stats() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let json_str = output.to_json();

        assert!(json_str.contains("index_stats"));
    }

    #[test]
    fn test_to_json_valid() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let json_str = output.to_json();

        // 验证 JSON 可解析
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_to_markdown_contains_project() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let md = output.to_markdown();

        assert!(md.contains("Project Memory Report"));
        assert!(md.contains("test-project"));
        assert!(md.contains("Rust"));
    }

    #[test]
    fn test_to_markdown_contains_mermaid() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let md = output.to_markdown();

        assert!(md.contains("```mermaid"));
        assert!(md.contains("graph"));
    }

    #[test]
    fn test_to_markdown_contains_change_history() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let md = output.to_markdown();

        assert!(md.contains("Change History"));
        assert!(md.contains("refactor"));
    }

    #[test]
    fn test_to_mermaid_structure() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let mermaid = output.to_mermaid_structure();

        assert!(mermaid.contains("graph"));
        assert!(mermaid.contains("src"));
    }

    #[test]
    fn test_to_mermaid_chain() {
        let project = make_project();
        let structure = make_structure();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, Some(&structure), &chain, &entries);
        let mermaid = output.to_mermaid_chain();

        assert!(mermaid.contains("graph"));
        assert!(mermaid.contains("refactor"));
    }

    #[test]
    fn test_to_mermaid_structure_no_data() {
        let project = make_project();
        let chain = make_chain();
        let entries = make_entries();

        let output = MemoryOutput::new(&project, None, &chain, &entries);
        let mermaid = output.to_mermaid_structure();

        assert!(mermaid.contains("No structure data"));
    }

    #[test]
    fn test_output_format_debug() {
        assert_eq!(format!("{:?}", OutputFormat::Json), "Json");
        assert_eq!(format!("{:?}", OutputFormat::Markdown), "Markdown");
        assert_eq!(format!("{:?}", OutputFormat::Mermaid), "Mermaid");
    }

    #[test]
    fn test_default_output_writer() {
        let writer = DefaultOutputWriter;
        assert_eq!(writer.write_json("hello"), "hello");
        assert_eq!(writer.write_markdown("hello"), "hello");
        assert_eq!(writer.write_mermaid("hello"), "hello");
    }
}