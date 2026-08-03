//! 模型代理
//!
//! 让用户的 vibecoding AI 可以复用其模型（通过 MCP 传递），
//! 而不需要额外配置 API Key。

/// 代理模式
#[derive(Debug, Clone, PartialEq)]
pub enum ProxyMode {
    /// 用户 AI 直接调用，Route 只做 git/memory/skill 等辅助操作
    Passthrough,
    /// Route 通过 MCP 返回指令，用户 AI 执行
    Instruct,
    /// Route 内置模型（需要 API Key）
    Builtin,
}

/// 模型代理
#[derive(Debug, Clone)]
pub struct ModelProxy {
    /// 代理模式
    pub mode: ProxyMode,
    /// MCP 端点
    pub endpoint: Option<String>,
}

impl Default for ModelProxy {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelProxy {
    /// 创建新的模型代理，默认使用 Passthrough 模式
    pub fn new() -> Self {
        Self {
            mode: ProxyMode::Passthrough,
            endpoint: None,
        }
    }

    /// 设置代理模式
    pub fn with_mode(mode: ProxyMode) -> Self {
        Self {
            mode,
            endpoint: None,
        }
    }

    /// 设置 MCP 端点
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    /// 是否为透传模式
    pub fn is_passthrough(&self) -> bool {
        self.mode == ProxyMode::Passthrough
    }

    /// 将任务和项目记忆包装成提示词，返回给用户 AI
    ///
    /// 在 Passthrough 模式下，用户 AI 拿到此提示词后自行调用模型。
    /// 在 Instruct 模式下，Route 返回指令让用户 AI 执行。
    /// 在 Builtin 模式下，Route 自行调用内置模型。
    pub fn wrap_prompt(&self, task: &str, context: &str) -> String {
        match self.mode {
            ProxyMode::Passthrough => {
                format!(
                    r#"[Route Vibe — 任务提示]

## 项目上下文
{}

## 用户任务
{}

## 可用工具
通过 MCP 调用以下工具完成此任务：
- vm_execute: 执行版本管理操作（git add/commit/branch/status）
- memory_query: 查询项目记忆
- structure_view: 查看项目结构
- skill_execute: 执行注册的技能
- engine_search: 搜索代码
- causal_check: 检测副作用
- project_info: 获取项目信息

请根据任务选择合适的工具组合。
"#,
                    context, task
                )
            }
            ProxyMode::Instruct => {
                format!(
                    r#"[Route Vibe — 执行指令]

## 项目上下文
{}

## 用户任务
{}

请按照以下步骤执行：
1. 分析任务需求
2. 选择合适工具
3. 执行操作
4. 返回结果
"#,
                    context, task
                )
            }
            ProxyMode::Builtin => {
                format!(
                    r#"[Route Vibe — 内置模型处理]

## 项目上下文
{}

## 用户任务
{}

Route 将自动处理此任务。
"#,
                    context, task
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_proxy_default() {
        let proxy = ModelProxy::new();
        assert!(proxy.is_passthrough());
        assert!(proxy.endpoint.is_none());
    }

    #[test]
    fn test_model_proxy_modes() {
        let passthrough = ModelProxy::with_mode(ProxyMode::Passthrough);
        assert!(passthrough.is_passthrough());

        let instruct = ModelProxy::with_mode(ProxyMode::Instruct);
        assert!(!instruct.is_passthrough());

        let builtin = ModelProxy::with_mode(ProxyMode::Builtin);
        assert!(!builtin.is_passthrough());
    }

    #[test]
    fn test_wrap_prompt_passthrough() {
        let proxy = ModelProxy::new();
        let prompt = proxy.wrap_prompt("改 greeting 函数", "项目: demo");
        assert!(prompt.contains("Route Vibe"));
        assert!(prompt.contains("改 greeting 函数"));
        assert!(prompt.contains("项目: demo"));
        assert!(prompt.contains("vm_execute"));
        assert!(prompt.contains("memory_query"));
    }

    #[test]
    fn test_wrap_prompt_instruct() {
        let proxy = ModelProxy::with_mode(ProxyMode::Instruct);
        let prompt = proxy.wrap_prompt("改 greeting 函数", "项目: demo");
        assert!(prompt.contains("执行指令"));
        assert!(prompt.contains("1. 分析任务需求"));
    }

    #[test]
    fn test_wrap_prompt_builtin() {
        let proxy = ModelProxy::with_mode(ProxyMode::Builtin);
        let prompt = proxy.wrap_prompt("改 greeting 函数", "项目: demo");
        assert!(prompt.contains("内置模型处理"));
    }

    #[test]
    fn test_with_endpoint() {
        let proxy = ModelProxy::new().with_endpoint("http://localhost:8080".to_string());
        assert_eq!(proxy.endpoint, Some("http://localhost:8080".to_string()));
    }
}