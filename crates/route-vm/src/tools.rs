//! 工具注册表
//!
//! 支持内置工具和三方工具钩子（MCP 注册）。

use std::fmt;

use anyhow::{Context, Result};
use serde_json::Value;

/// Agent 工具 trait
pub trait AgentTool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;
    /// 工具描述
    fn description(&self) -> &str;
    /// 执行工具
    fn execute(&self, args: &Value) -> Result<String>;
}

impl fmt::Debug for dyn AgentTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentTool")
            .field("name", &self.name())
            .field("description", &self.description())
            .finish()
    }
}

/// 三方工具
#[derive(Debug, Clone)]
pub struct ThirdPartyTool {
    pub name: String,
    pub description: String,
    /// MCP 端点
    pub endpoint: String,
}

/// 工具注册表
#[derive(Debug)]
pub struct ToolRegistry {
    pub builtins: Vec<Box<dyn AgentTool>>,
    pub third_party: Vec<ThirdPartyTool>,
}

impl ToolRegistry {
    /// 创建新的工具注册表
    pub fn new() -> Self {
        Self {
            builtins: Vec::new(),
            third_party: Vec::new(),
        }
    }

    /// 注册三方工具
    pub fn register_third_party(&mut self, tool: ThirdPartyTool) {
        // 如果已存在同名工具，替换之
        if let Some(pos) = self.third_party.iter().position(|t| t.name == tool.name) {
            self.third_party[pos] = tool;
        } else {
            self.third_party.push(tool);
        }
    }

    /// 注册内置工具
    pub fn register_builtin(&mut self, tool: Box<dyn AgentTool>) {
        // 如果已存在同名工具，替换之
        if let Some(pos) = self.builtins.iter().position(|t| t.name() == tool.name()) {
            self.builtins[pos] = tool;
        } else {
            self.builtins.push(tool);
        }
    }

    /// 执行工具
    pub fn execute(&self, name: &str, args: &Value) -> Result<String> {
        // 先查找内置工具
        for tool in &self.builtins {
            if tool.name() == name {
                return tool.execute(args);
            }
        }

        // 再查找三方工具
        for tool in &self.third_party {
            if tool.name == name {
                return Self::execute_third_party(tool, args);
            }
        }

        Err(anyhow::anyhow!("Tool '{}' not found in registry", name))
    }

    /// 执行三方工具（通过 MCP 端点）
    fn execute_third_party(tool: &ThirdPartyTool, args: &Value) -> Result<String> {
        // 构建 MCP 请求
        let request = serde_json::json!({
            "tool": tool.name,
            "args": args,
        });

        // 通过 HTTP 调用 MCP 端点
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .with_context(|| format!("Failed to create HTTP client for MCP tool '{}'", tool.name))?;

        let response = client
            .post(&tool.endpoint)
            .json(&request)
            .send()
            .with_context(|| format!("Failed to call MCP tool '{}' at {}", tool.name, tool.endpoint))?;

        let text = response
            .text()
            .with_context(|| format!("Failed to read response from MCP tool '{}'", tool.name))?;

        Ok(text)
    }

    /// 获取所有工具名称列表
    pub fn list_tools(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .builtins
            .iter()
            .map(|t| t.name().to_string())
            .collect();
        names.extend(self.third_party.iter().map(|t| t.name.clone()));
        names
    }

    /// 获取工具数量
    pub fn count(&self) -> usize {
        self.builtins.len() + self.third_party.len()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 内置工具：读取文件
pub struct ReadFileTool;

impl AgentTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file"
    }

    fn execute(&self, args: &Value) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path' argument for read_file")?;

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path))?;

        Ok(content)
    }
}

/// 内置工具：写入文件
pub struct WriteFileTool;

impl AgentTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file"
    }

    fn execute(&self, args: &Value) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path' argument for write_file")?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .context("Missing 'content' argument for write_file")?;

        // 确保父目录存在
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {:?}", parent))?;
        }

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write file: {}", path))?;

        Ok(format!("Written {} bytes to {}", content.len(), path))
    }
}

/// 内置工具：列出目录
pub struct ListDirTool;

impl AgentTool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "List contents of a directory"
    }

    fn execute(&self, args: &Value) -> Result<String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path' argument for list_dir")?;

        let entries = std::fs::read_dir(path)
            .with_context(|| format!("Failed to read directory: {}", path))?;

        let mut result = String::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = if entry.file_type()?.is_dir() {
                "dir"
            } else {
                "file"
            };
            result.push_str(&format!("  [{}] {}\n", file_type, name));
        }

        Ok(result)
    }
}

/// 内置工具：执行命令
pub struct RunCommandTool;

impl AgentTool for RunCommandTool {
    fn name(&self) -> &str {
        "run_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command"
    }

    fn execute(&self, args: &Value) -> Result<String> {
        let cmd = args
            .get("command")
            .and_then(|v| v.as_str())
            .context("Missing 'command' argument for run_command")?;

        let output = std::process::Command::new("cmd")
            .args(["/C", cmd])
            .output()
            .with_context(|| format!("Failed to execute command: {}", cmd))?;

        let mut result = String::new();
        if !output.stdout.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registry_new() {
        let registry = ToolRegistry::new();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn test_register_builtin() {
        let mut registry = ToolRegistry::new();
        registry.register_builtin(Box::new(ReadFileTool));
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_register_third_party() {
        let mut registry = ToolRegistry::new();
        registry.register_third_party(ThirdPartyTool {
            name: "mcp_tool".to_string(),
            description: "MCP tool".to_string(),
            endpoint: "http://localhost:8080/mcp".to_string(),
        });
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_list_tools() {
        let mut registry = ToolRegistry::new();
        registry.register_builtin(Box::new(ReadFileTool));
        registry.register_third_party(ThirdPartyTool {
            name: "mcp_tool".to_string(),
            description: "MCP tool".to_string(),
            endpoint: "http://localhost:8080/mcp".to_string(),
        });
        let names = registry.list_tools();
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"mcp_tool".to_string()));
    }

    #[test]
    fn test_execute_tool_not_found() {
        let registry = ToolRegistry::new();
        let result = registry.execute("nonexistent", &serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_read_file_tool_missing_arg() {
        let tool = ReadFileTool;
        let result = tool.execute(&serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_write_file_tool_missing_arg() {
        let tool = WriteFileTool;
        let result = tool.execute(&serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_list_dir_tool_missing_arg() {
        let tool = ListDirTool;
        let result = tool.execute(&serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_run_command_tool_missing_arg() {
        let tool = RunCommandTool;
        let result = tool.execute(&serde_json::json!({}));
        assert!(result.is_err());
    }
}