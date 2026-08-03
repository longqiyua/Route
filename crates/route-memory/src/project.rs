use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 项目元信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub description: String,
    pub language: String,
    pub framework: Option<String>,
    pub constraints: Vec<String>,
    pub entry_points: Vec<String>,
    pub key_modules: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: String,
}

impl ProjectMeta {
    /// 创建默认的项目元信息
    pub fn default() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            name: String::new(),
            description: String::new(),
            language: String::new(),
            framework: None,
            constraints: Vec::new(),
            entry_points: Vec::new(),
            key_modules: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            version: "0.1.0".to_string(),
        }
    }

    /// 从 JSON 文件加载项目元信息
    pub fn load(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Failed to read {:?}", path))?;
        let meta: Self =
            serde_json::from_str(&content).with_context(|| format!("Failed to parse {:?}", path))?;
        Ok(meta)
    }

    /// 保存项目元信息到 JSON 文件
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }
        let content = serde_json::to_string_pretty(self)
            .with_context(|| format!("Failed to serialize project meta to {:?}", path))?;
        std::fs::write(path, &content)
            .with_context(|| format!("Failed to write {:?}", path))?;
        Ok(())
    }

    /// 生成 Mermaid 流程图
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph TD\n");
        mermaid.push_str(&format!("    PM[\"Project: {}\"]\n", self.name));
        mermaid.push_str(&format!("    PM --> |language| L[\"{}\"]\n", self.language));

        if let Some(ref fw) = self.framework {
            mermaid.push_str(&format!("    PM --> |framework| FW[\"{}\"]\n", fw));
        }

        if !self.entry_points.is_empty() {
            mermaid.push_str("    PM --> |entry points| EP[\"");
            for (i, ep) in self.entry_points.iter().enumerate() {
                if i > 0 {
                    mermaid.push_str("<br/>");
                }
                mermaid.push_str(ep);
            }
            mermaid.push_str("\"]\n");
        }

        if !self.key_modules.is_empty() {
            mermaid.push_str("    subgraph KeyModules[\"Key Modules\"]\n");
            for (i, km) in self.key_modules.iter().enumerate() {
                let node_id = format!("KM{}", i);
                mermaid.push_str(&format!("    {}[\"{}\"]\n", node_id, km));
                if i > 0 {
                    let prev = format!("KM{}", i - 1);
                    mermaid.push_str(&format!("    {} --> {}\n", prev, node_id));
                }
            }
            mermaid.push_str("    end\n");
            mermaid.push_str("    PM --> KeyModules\n");
        }

        if !self.constraints.is_empty() {
            mermaid.push_str("    subgraph Constraints[\"Constraints\"]\n");
            for (i, c) in self.constraints.iter().enumerate() {
                let node_id = format!("C{}", i);
                mermaid.push_str(&format!("    {}[\"{}\"]\n", node_id, c));
            }
            mermaid.push_str("    end\n");
            mermaid.push_str("    PM --> Constraints\n");
        }

        mermaid
    }
}