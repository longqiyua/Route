//! MCP 服务入口
//!
//! 定义 MCP 工具协议，让用户的 vibecoding AI 通过 MCP 调用 Route。

use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::VibeSession;

/// MCP 工具定义
#[derive(Debug, Clone)]
pub struct McpTool {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 输入 JSON Schema
    pub input_schema: Value,
}

/// MCP 服务器
pub struct McpServer {
    /// 注册的工具列表
    pub tools: Vec<McpTool>,
    /// 关联的 Vibe 会话
    pub session: VibeSession,
}

impl McpServer {
    /// 创建新的 MCP 服务器
    pub fn new(session: VibeSession) -> Self {
        Self {
            tools: Vec::new(),
            session,
        }
    }

    /// 注册默认工具集
    pub fn register_default_tools(&mut self) {
        self.tools = vec![
            McpTool {
                name: "vm_execute".to_string(),
                description: "执行版本管理任务（git add/commit/branch/status）".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task": { "type": "string", "description": "要执行的任务描述" },
                        "memory_mode": { "type": "boolean", "description": "是否启用记忆模式" }
                    },
                    "required": ["task"]
                }),
            },
            McpTool {
                name: "memory_query".to_string(),
                description: "查询项目记忆".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "查询关键词" },
                        "kind": { "type": "string", "description": "记忆类型过滤（可选）" }
                    },
                    "required": ["query"]
                }),
            },
            McpTool {
                name: "structure_view".to_string(),
                description: "查看项目结构（Mermaid）".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "format": { "type": "string", "enum": ["mermaid", "modules"], "description": "输出格式" }
                    }
                }),
            },
            McpTool {
                name: "skill_execute".to_string(),
                description: "执行技能".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "skill_id": { "type": "string", "description": "技能 ID" },
                        "task": { "type": "string", "description": "任务描述" }
                    },
                    "required": ["skill_id"]
                }),
            },
            McpTool {
                name: "engine_search".to_string(),
                description: "搜索代码".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "搜索关键词" },
                        "top_k": { "type": "integer", "description": "返回结果数量" }
                    },
                    "required": ["query"]
                }),
            },
            McpTool {
                name: "causal_check".to_string(),
                description: "检测副作用".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "文件路径" }
                    },
                    "required": ["file"]
                }),
            },
            McpTool {
                name: "project_info".to_string(),
                description: "获取项目信息".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ];
    }

    /// 处理 MCP 工具调用
    pub fn handle_call(&self, tool_name: &str, args: Value) -> Result<Value> {
        match tool_name {
            "vm_execute" => self.handle_vm_execute(args),
            "memory_query" => self.handle_memory_query(args),
            "structure_view" => self.handle_structure_view(args),
            "skill_execute" => self.handle_skill_execute(args),
            "engine_search" => self.handle_engine_search(args),
            "causal_check" => self.handle_causal_check(args),
            "project_info" => self.handle_project_info(args),
            _ => Err(anyhow::anyhow!("未知工具: {}", tool_name)),
        }
    }

    /// 列出所有已注册的工具
    pub fn list_tools(&self) -> Vec<McpTool> {
        self.tools.clone()
    }

    // --- 工具处理函数 ---

    fn handle_vm_execute(&self, args: Value) -> Result<Value> {
        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .context("缺少 task 参数")?;

        let _memory_mode = args
            .get("memory_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        #[cfg(feature = "vm")]
        {
            // 使用 session 的 vm 执行任务
            // 这里我们简单返回一条信息，实际执行需要通过可变引用
            Ok(json!({
                "status": "received",
                "message": format!("任务已接收: {}", task),
                "note": "VM 执行需要可变会话引用，请通过 process_natural_language 执行"
            }))
        }

        #[cfg(not(feature = "vm"))]
        {
            let _ = task;
            Ok(json!({
                "status": "unavailable",
                "message": "VM feature 未启用"
            }))
        }
    }

    fn handle_memory_query(&self, args: Value) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("缺少 query 参数")?;

        #[cfg(feature = "memory")]
        {
            if let Some(ref memory) = self.session.memory {
                let results = memory.search(query);
                let entries: Vec<Value> = results
                    .iter()
                    .map(|e| {
                        json!({
                            "id": e.id,
                            "kind": format!("{:?}", e.kind),
                            "key": e.key,
                            "content": e.content,
                            "tags": e.tags,
                            "created_at": e.created_at,
                        })
                    })
                    .collect();

                return Ok(json!({
                    "status": "ok",
                    "count": entries.len(),
                    "entries": entries
                }));
            }
            Ok(json!({
                "status": "unavailable",
                "message": "记忆未初始化"
            }))
        }

        #[cfg(not(feature = "memory"))]
        {
            let _ = query;
            Ok(json!({
                "status": "unavailable",
                "message": "Memory feature 未启用"
            }))
        }
    }

    fn handle_structure_view(&self, args: Value) -> Result<Value> {
        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("mermaid");

        #[cfg(feature = "memory")]
        {
            if let Some(ref memory) = self.session.memory {
                if let Some(ref structure) = memory.structure {
                    let mermaid = match format {
                        "modules" => structure.to_mermaid_modules(),
                        _ => structure.to_mermaid(),
                    };
                    return Ok(json!({
                        "status": "ok",
                        "format": format,
                        "mermaid": mermaid
                    }));
                }
                return Ok(json!({
                    "status": "unavailable",
                    "message": "项目结构未加载"
                }));
            }
            Ok(json!({
                "status": "unavailable",
                "message": "记忆未初始化"
            }))
        }

        #[cfg(not(feature = "memory"))]
        {
            let _ = format;
            Ok(json!({
                "status": "unavailable",
                "message": "Memory feature 未启用"
            }))
        }
    }

    fn handle_skill_execute(&self, args: Value) -> Result<Value> {
        let _skill_id = args
            .get("skill_id")
            .and_then(|v| v.as_str())
            .context("缺少 skill_id 参数")?;

        #[cfg(feature = "skill")]
        {
            Ok(json!({
                "status": "received",
                "message": format!("技能执行请求已接收: {}", _skill_id),
                "note": "技能执行需要可变会话引用"
            }))
        }

        #[cfg(not(feature = "skill"))]
        {
            Ok(json!({
                "status": "unavailable",
                "message": "Skill feature 未启用"
            }))
        }
    }

    fn handle_engine_search(&self, args: Value) -> Result<Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("缺少 query 参数")?;

        let top_k = args
            .get("top_k")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        #[cfg(feature = "engine")]
        {
            if let Some(ref engine) = self.session.engine {
                let results = engine.search(query, top_k);
                let items: Vec<Value> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "text": r.text,
                            "score": r.score,
                            "file_path": r.file_path,
                            "line": r.line,
                        })
                    })
                    .collect();

                return Ok(json!({
                    "status": "ok",
                    "count": items.len(),
                    "results": items
                }));
            }
            Ok(json!({
                "status": "unavailable",
                "message": "搜索引擎未初始化"
            }))
        }

        #[cfg(not(feature = "engine"))]
        {
            let _ = (query, top_k);
            Ok(json!({
                "status": "unavailable",
                "message": "Engine feature 未启用"
            }))
        }
    }

    fn handle_causal_check(&self, args: Value) -> Result<Value> {
        let _file = args
            .get("file")
            .and_then(|v| v.as_str())
            .context("缺少 file 参数")?;

        #[cfg(feature = "vm")]
        {
            #[cfg(feature = "memory")]
            {
                let chain = route_memory::chain::CausalChain::load(
                    &self.session.project_path.clone().unwrap_or_default(),
                );
                if let Ok(chain) = chain {
                    let effects = chain.detect_side_effects(_file);
                    let items: Vec<Value> = effects
                        .iter()
                        .map(|l| {
                            json!({
                                "id": l.id,
                                "action": l.action,
                                "effect": l.effect,
                                "status": format!("{:?}", l.status),
                            })
                        })
                        .collect();
                    return Ok(json!({
                        "status": "ok",
                        "count": items.len(),
                        "side_effects": items
                    }));
                }
            }
            Ok(json!({
                "status": "ok",
                "side_effects": []
            }))
        }

        #[cfg(not(feature = "vm"))]
        {
            let _ = _file;
            Ok(json!({
                "status": "unavailable",
                "message": "VM feature 未启用，无法检测副作用"
            }))
        }
    }

    fn handle_project_info(&self, _args: Value) -> Result<Value> {
        let mut info = HashMap::new();

        info.insert(
            "session_id".to_string(),
            json!(self.session.id),
        );
        info.insert(
            "start_time".to_string(),
            json!(self.session.start_time),
        );
        info.insert(
            "project_path".to_string(),
            json!(self.session.project_path),
        );
        info.insert(
            "config".to_string(),
            json!({
                "auto_mode": self.session.config.auto_mode,
                "memory_mode": self.session.config.memory_mode,
                "causal_control": self.session.config.causal_control,
                "auto_git": self.session.config.auto_git,
            }),
        );

        #[cfg(feature = "vm")]
        info.insert("vm".to_string(), json!("enabled"));
        #[cfg(not(feature = "vm"))]
        info.insert("vm".to_string(), json!("disabled"));

        #[cfg(feature = "skill")]
        info.insert("skill".to_string(), json!("enabled"));
        #[cfg(not(feature = "skill"))]
        info.insert("skill".to_string(), json!("disabled"));

        #[cfg(feature = "memory")]
        info.insert("memory".to_string(), json!("enabled"));
        #[cfg(not(feature = "memory"))]
        info.insert("memory".to_string(), json!("disabled"));

        #[cfg(feature = "engine")]
        info.insert("engine".to_string(), json!("enabled"));
        #[cfg(not(feature = "engine"))]
        info.insert("engine".to_string(), json!("disabled"));

        Ok(json!(info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VibeConfig, VibeSession};

    #[test]
    fn test_mcp_server_new() {
        let config = VibeConfig::default();
        let session = VibeSession::new(None, config);
        let server = McpServer::new(session);
        assert!(server.tools.is_empty());
    }

    #[test]
    fn test_register_default_tools() {
        let config = VibeConfig::default();
        let session = VibeSession::new(None, config);
        let mut server = McpServer::new(session);
        server.register_default_tools();
        assert_eq!(server.tools.len(), 7);
        assert_eq!(server.tools[0].name, "vm_execute");
        assert_eq!(server.tools[1].name, "memory_query");
        assert_eq!(server.tools[2].name, "structure_view");
        assert_eq!(server.tools[3].name, "skill_execute");
        assert_eq!(server.tools[4].name, "engine_search");
        assert_eq!(server.tools[5].name, "causal_check");
        assert_eq!(server.tools[6].name, "project_info");
    }

    #[test]
    fn test_list_tools() {
        let config = VibeConfig::default();
        let session = VibeSession::new(None, config);
        let mut server = McpServer::new(session);
        server.register_default_tools();
        let tools = server.list_tools();
        assert_eq!(tools.len(), 7);
    }

    #[test]
    fn test_handle_unknown_tool() {
        let config = VibeConfig::default();
        let session = VibeSession::new(None, config);
        let server = McpServer::new(session);
        let result = server.handle_call("unknown_tool", json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_project_info() {
        let config = VibeConfig::default();
        let session = VibeSession::new(None, config);
        let server = McpServer::new(session);
        let result = server.handle_call("project_info", json!({})).unwrap();
        assert!(result.get("session_id").is_some());
        assert!(result.get("start_time").is_some());
        assert!(result.get("config").is_some());
    }
}