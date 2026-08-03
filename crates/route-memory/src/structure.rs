use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 文件节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
    pub size: u64,
    pub last_modified: String,
}

/// 变更类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Removed,
    Modified,
}

/// 文件变更记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub change_type: ChangeType,
    pub old_size: Option<u64>,
    pub new_size: Option<u64>,
}

/// 项目结构快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStructure {
    pub root: FileNode,
    pub total_files: usize,
    pub total_dirs: usize,
    pub languages: HashMap<String, usize>,
}

impl ProjectStructure {
    /// 扫描项目目录，构建结构快照
    pub fn scan(path: &Path) -> Result<Self> {
        let root_path = path
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize {:?}", path))?;

        let mut total_files = 0usize;
        let mut total_dirs = 0usize;
        let mut languages: HashMap<String, usize> = HashMap::new();

        let root_node = Self::scan_node(&root_path, &root_path, &mut total_files, &mut total_dirs, &mut languages)?;

        Ok(Self {
            root: root_node,
            total_files,
            total_dirs,
            languages,
        })
    }

    /// 递归扫描单个节点
    fn scan_node(
        base: &Path,
        current: &Path,
        total_files: &mut usize,
        total_dirs: &mut usize,
        languages: &mut HashMap<String, usize>,
    ) -> Result<FileNode> {
        let metadata = current
            .metadata()
            .with_context(|| format!("Failed to read metadata for {:?}", current))?;

        let relative = current
            .strip_prefix(base)
            .unwrap_or(current)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        let last_modified = if let Ok(modified) = metadata.modified() {
            let datetime: chrono::DateTime<chrono::Utc> = modified.into();
            datetime.to_rfc3339()
        } else {
            String::new()
        };

        let is_dir = metadata.is_dir();
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };

        if is_dir {
            *total_dirs += 1;
        } else {
            *total_files += 1;
            // 通过扩展名推断语言
            if let Some(ext) = current.extension() {
                let lang = ext.to_string_lossy().to_lowercase();
                *languages.entry(lang).or_insert(0) += 1;
            }
        }

        let mut children = Vec::new();
        if is_dir {
            if let Ok(entries) = std::fs::read_dir(current) {
                let mut dirs: Vec<_> = Vec::new();
                let mut files: Vec<_> = Vec::new();

                for entry in entries.flatten() {
                    let child_path = entry.path();
                    // 跳过隐藏目录（以 . 开头）
                    if let Some(name) = child_path.file_name() {
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with('.') && child_path.is_dir() {
                            continue;
                        }
                    }
                    match Self::scan_node(base, &child_path, total_files, total_dirs, languages) {
                        Ok(node) => {
                            if node.is_dir {
                                dirs.push(node);
                            } else {
                                files.push(node);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to scan {:?}: {}", child_path, e);
                        }
                    }
                }

                // 目录在前，文件在后，各自按名称排序
                dirs.sort_by(|a, b| a.path.cmp(&b.path));
                files.sort_by(|a, b| a.path.cmp(&b.path));
                children.extend(dirs);
                children.extend(files);
            }
        }

        Ok(FileNode {
            path: if relative.is_empty() {
                ".".to_string()
            } else {
                relative
            },
            is_dir,
            children,
            size,
            last_modified,
        })
    }

    /// 生成完整项目结构 Mermaid 图（graph TD）
    pub fn to_mermaid(&self) -> String {
        let mut mermaid = String::from("graph TD\n");
        self.build_mermaid(&self.root, &mut mermaid, &mut 0usize);
        mermaid
    }

    fn build_mermaid(&self, node: &FileNode, output: &mut String, counter: &mut usize) -> String {
        let current_id = format!("N{}", counter);
        *counter += 1;

        let label = if node.is_dir {
            let path = &node.path;
            let dir_name = path.rsplit('/').next().unwrap_or(path);
            format!("{}/", dir_name)
        } else {
            let path = &node.path;
            path.rsplit('/').next().unwrap_or(path).to_string()
        };

        let shape = if node.is_dir { "[" } else { "[" };
        let shape_end = if node.is_dir { "]" } else { "]" };
        output.push_str(&format!("    {}{}\"{}\"{}\n", current_id, shape, label, shape_end));

        for child in &node.children {
            let child_id = self.build_mermaid(child, output, counter);
            output.push_str(&format!("    {} --> {}\n", current_id, child_id));
        }

        current_id
    }

    /// 生成模块级 Mermaid 图（仅显示目录结构）
    pub fn to_mermaid_modules(&self) -> String {
        let mut mermaid = String::from("graph TD\n");
        self.build_module_mermaid(&self.root, &mut mermaid, &mut 0usize, 0);
        mermaid
    }

    fn build_module_mermaid(
        &self,
        node: &FileNode,
        output: &mut String,
        counter: &mut usize,
        depth: usize,
    ) -> Option<String> {
        if !node.is_dir {
            return None;
        }

        // 只显示到模块级别（深度 <= 2）
        if depth > 2 {
            return None;
        }

        let current_id = format!("M{}", counter);
        *counter += 1;

        let dir_name = node.path.rsplit('/').next().unwrap_or(&node.path);
        output.push_str(&format!(
            "    {}[\"{}\"]\n",
            current_id,
            if dir_name == "." { "root" } else { dir_name }
        ));

        for child in &node.children {
            if child.is_dir {
                if let Some(child_id) =
                    self.build_module_mermaid(child, output, counter, depth + 1)
                {
                    output.push_str(&format!("    {} --> {}\n", current_id, child_id));
                }
            }
        }

        // 如果当前目录有文件，添加一个文件节点汇总
        let files: Vec<_> = node.children.iter().filter(|c| !c.is_dir).collect();
        if !files.is_empty() {
            let file_id = format!("M{}F", current_id);
            output.push_str(&format!(
                "    {}[\"{} files\"]\n",
                file_id,
                files.len()
            ));
            output.push_str(&format!("    {} --> {}\n", current_id, file_id));
        }

        Some(current_id)
    }

    /// 比较两个结构快照，找出变更
    pub fn find_changes(&self, old: &ProjectStructure) -> Vec<Change> {
        let mut changes = Vec::new();

        let old_files = Self::collect_files(&old.root);
        let new_files = Self::collect_files(&self.root);

        // 找出新增和修改的文件
        for (path, size) in &new_files {
            match old_files.get(path) {
                None => {
                    changes.push(Change {
                        path: path.clone(),
                        change_type: ChangeType::Added,
                        old_size: None,
                        new_size: Some(*size),
                    });
                }
                Some(old_size) if old_size != size => {
                    changes.push(Change {
                        path: path.clone(),
                        change_type: ChangeType::Modified,
                        old_size: Some(*old_size),
                        new_size: Some(*size),
                    });
                }
                _ => {}
            }
        }

        // 找出删除的文件
        for (path, size) in &old_files {
            if !new_files.contains_key(path) {
                changes.push(Change {
                    path: path.clone(),
                    change_type: ChangeType::Removed,
                    old_size: Some(*size),
                    new_size: None,
                });
            }
        }

        changes.sort_by(|a, b| a.path.cmp(&b.path));
        changes
    }

    fn collect_files(node: &FileNode) -> HashMap<String, u64> {
        let mut files = HashMap::new();
        Self::collect_files_recursive(node, &mut files);
        files
    }

    fn collect_files_recursive(node: &FileNode, files: &mut HashMap<String, u64>) {
        if !node.is_dir {
            files.insert(node.path.clone(), node.size);
        }
        for child in &node.children {
            Self::collect_files_recursive(child, files);
        }
    }
}