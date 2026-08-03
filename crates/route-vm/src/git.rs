//! Git 版本管理操作
//!
//! 提供 GitOps 结构体，封装常用的 Git 命令操作。

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

/// Git 版本管理操作
#[derive(Debug, Clone)]
pub struct GitOps {
    pub repo_path: PathBuf,
}

impl GitOps {
    /// 创建新的 GitOps 实例
    pub fn new(path: &Path) -> Self {
        Self {
            repo_path: path.to_path_buf(),
        }
    }

    /// 执行 git 命令
    fn git(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()
            .with_context(|| format!("Failed to execute git {:?} in {:?}", args, self.repo_path))?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(anyhow::anyhow!("git {:?} failed: {}", args, stderr))
        }
    }

    /// 获取仓库状态
    pub fn status(&self) -> Result<String> {
        self.git(&["status", "--porcelain"])
    }

    /// 添加文件到暂存区
    pub fn add(&self, paths: &[&str]) -> Result<()> {
        let mut args = vec!["add"];
        args.extend(paths);
        self.git(&args)?;
        Ok(())
    }

    /// 提交变更
    pub fn commit(&self, message: &str) -> Result<String> {
        self.git(&["commit", "-m", message])
    }

    /// 获取当前分支名
    pub fn branch(&self) -> Result<String> {
        self.git(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// 切换分支
    pub fn checkout(&self, branch: &str) -> Result<()> {
        self.git(&["checkout", branch])?;
        Ok(())
    }

    /// 获取工作树与暂存区的差异
    pub fn diff(&self) -> Result<String> {
        self.git(&["diff", "--stat"])
    }

    /// 获取提交日志
    pub fn log(&self, n: usize) -> Result<String> {
        self.git(&["log", &format!("-{}", n), "--oneline"])
    }

    /// 自动提交：git add -A，检查有变更才提交
    pub fn auto_commit(&self, prefix: &str) -> Result<String> {
        // git add -A
        self.git(&["add", "-A"])?;

        // 检查是否有变更
        let staged = self.git(&["diff", "--cached", "--quiet"]);

        match staged {
            Ok(_) => {
                // 没有变更，git diff --cached --quiet 返回空表示无变更
                Ok("No changes to commit".to_string())
            }
            Err(_) => {
                // 有变更（exit code != 0），执行提交
                let branch = self.branch().unwrap_or_else(|_| "unknown".to_string());
                let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                let message = format!("[{}] {}: auto-commit {}", prefix, branch, timestamp);
                self.commit(&message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_ops_new() {
        let git = GitOps::new(Path::new("."));
        assert_eq!(git.repo_path, PathBuf::from("."));
    }

    #[test]
    fn test_git_ops_status() {
        let git = GitOps::new(Path::new("."));
        // 在当前目录执行 git status 应该不会失败
        let result = git.status();
        // 可能不在 git 仓库中，但不会 crash
        assert!(result.is_ok() || result.is_err());
    }
}