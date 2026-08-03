use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 因果链状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChainStatus {
    Proposed,
    Approved,
    Executed,
    RolledBack,
    Failed,
}

/// 因果链接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLink {
    pub id: String,
    pub parent_id: Option<String>,
    pub action: String,
    pub file_path: Option<String>,
    pub reason: String,
    pub effect: String,
    pub side_effects: Vec<String>,
    pub status: ChainStatus,
    pub timestamp: String,
    pub actor: String,
}

/// 因果链
#[derive(Debug, Clone)]
pub struct CausalChain {
    pub project_path: PathBuf,
    pub links: Vec<CausalLink>,
}

impl CausalChain {
    /// 获取存储文件路径
    fn chain_path(&self) -> PathBuf {
        self.project_path.join(".route").join("memory").join("chain.jsonl")
    }

    /// 从磁盘加载因果链
    pub fn load(project_path: &Path) -> Result<Self> {
        let chain_path = project_path.join(".route").join("memory").join("chain.jsonl");
        let mut links = Vec::new();

        if chain_path.exists() {
            let content = std::fs::read_to_string(&chain_path)
                .with_context(|| format!("Failed to read chain {:?}", chain_path))?;
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<CausalLink>(line) {
                    Ok(link) => links.push(link),
                    Err(e) => {
                        tracing::warn!("Failed to parse causal link: {}", e);
                    }
                }
            }
        }

        Ok(Self {
            project_path: project_path.to_path_buf(),
            links,
        })
    }

    /// 保存因果链到磁盘
    pub fn save(&self) -> Result<()> {
        let chain_path = self.chain_path();
        if let Some(parent) = chain_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        let mut content = String::new();
        for link in &self.links {
            let line = serde_json::to_string(link)
                .with_context(|| format!("Failed to serialize causal link {}", link.id))?;
            content.push_str(&line);
            content.push('\n');
        }

        std::fs::write(&chain_path, &content)
            .with_context(|| format!("Failed to write chain {:?}", chain_path))?;
        Ok(())
    }

    /// 添加因果链接
    pub fn add_link(&mut self, link: CausalLink) {
        // 如果存在相同 ID 的链接，替换之
        if let Some(pos) = self.links.iter().position(|l| l.id == link.id) {
            self.links[pos] = link;
        } else {
            self.links.push(link);
        }
    }

    /// 获取指定因果链接的完整因果链（从根节点到当前节点）
    pub fn get_chain(&self, id: &str) -> Vec<&CausalLink> {
        let mut chain = Vec::new();

        // 先找到当前节点
        let current = match self.links.iter().find(|l| l.id == id) {
            Some(link) => link,
            None => return chain,
        };

        chain.push(current);

        // 回溯父节点
        let mut parent_id = current.parent_id.as_deref();
        while let Some(pid) = parent_id {
            match self.links.iter().find(|l| l.id == pid) {
                Some(parent) => {
                    chain.push(parent);
                    parent_id = parent.parent_id.as_deref();
                }
                None => break,
            }
        }

        // 反转，使链从根到当前节点
        chain.reverse();
        chain
    }

    /// 检测涉及指定文件的所有因果链接（副作用检测）
    pub fn detect_side_effects(&self, file: &str) -> Vec<&CausalLink> {
        let mut affected = Vec::new();

        for link in &self.links {
            // 检查直接涉及的文件
            if let Some(ref fp) = link.file_path {
                if fp == file {
                    affected.push(link);
                    continue;
                }
            }
            // 检查副作用列表
            if link.side_effects.iter().any(|se| se == file) {
                affected.push(link);
            }
        }

        affected
    }

    /// 生成因果链 Mermaid 图
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph LR\n");

        // 添加所有节点
        for link in &self.links {
            let node_id = Self::sanitize_id(&link.id);
            let label = format!("{}: {}", link.action, link.effect);
            // 根据状态使用不同形状
            let (shape_start, shape_end) = match link.status {
                ChainStatus::Proposed => ("(", ")"),
                ChainStatus::Approved => ("[", "]"),
                ChainStatus::Executed => ("[", "]"),
                ChainStatus::RolledBack => ("([", "])"),
                ChainStatus::Failed => ("{{", "}}"),
            };
            // 状态标记
            let status_label = match link.status {
                ChainStatus::Proposed => "🟡 Proposed",
                ChainStatus::Approved => "🟢 Approved",
                ChainStatus::Executed => "🔵 Executed",
                ChainStatus::RolledBack => "🔴 RolledBack",
                ChainStatus::Failed => "⛔ Failed",
            };
            mermaid.push_str(&format!(
                "    {}{}\"{}<br/>[{}]\"{}\n",
                node_id, shape_start, label, status_label, shape_end
            ));
        }

        // 添加父子关系连线
        for link in &self.links {
            if let Some(ref parent_id) = link.parent_id {
                let child_id = Self::sanitize_id(&link.id);
                let parent_node = Self::sanitize_id(parent_id);
                mermaid.push_str(&format!("    {} --> {}\n", parent_node, child_id));
            }
        }

        mermaid
    }

    /// 清理 ID，使其适合作为 Mermaid 节点 ID
    fn sanitize_id(id: &str) -> String {
        // 替换非字母数字字符为下划线
        let sanitized: String = id
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        if sanitized.is_empty() {
            "node".to_string()
        } else if sanitized.chars().next().unwrap().is_numeric() {
            format!("n{}", sanitized)
        } else {
            sanitized
        }
    }
}