//! 模型管理层。预设插件（route-plugins）通过实现 ModelProvider trait，
//! 给 Agent 提供不同的 AI 模型能力。

use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error};

pub type ModelId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    /// Optional tool call id if this is a Tool response or a call
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Mock,
    OpenAi,
    Anthropic,
    Ollama,
    Custom,
}

/// 模型能力描述。用于 Agent 选择合适的模型。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapability {
    pub kind: ModelKind,
    /// 是否支持工具调用（function calling）
    pub tool_calling: bool,
    /// 是否支持长上下文（> 128k tokens）
    pub long_context: bool,
    /// 代码生成质量评分 0-100（主观，用于排序）
    pub code_quality: u8,
    /// 是否支持多模态
    pub multimodal: bool,
    /// 最快延迟等级（1=最快 < 1s，5=最慢 > 10s）
    pub latency_tier: u8,
    /// Feature flags (free-form list)
    pub features: Vec<String>,
    /// Cost estimate in cents per second (very rough; None = free/unknown)
    pub cost_per_second_cents: Option<u32>,
}

impl Default for ModelCapability {
    fn default() -> Self {
        Self {
            kind: ModelKind::Custom,
            tool_calling: false,
            long_context: false,
            code_quality: 50,
            multimodal: false,
            latency_tier: 3,
            features: Vec::new(),
            cost_per_second_cents: None,
        }
    }
}

/// Compact view returned by the registry list endpoint (owned)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: ModelId,
    pub display_name: String,
    pub capability: ModelCapability,
}

/// ModelProvider trait。预设插件可以实现这个 trait，
/// 将 route-plugins 的 Plugin 直接变成 Agent 可选模型。
pub trait ModelProvider: Send + Sync {
    /// 模型唯一标识（例如 "openai/gpt-4o", "anthropic/claude-3.5-sonnet"）
    fn id(&self) -> &ModelId;

    /// 展示名称
    fn display_name(&self) -> &str { self.id() }

    /// 模型能力
    fn capability(&self) -> &ModelCapability;

    /// 发送一次 chat completion。不流式。
    fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String>;

    /// 是否启用
    fn enabled(&self) -> bool { true }
}

/// 模型注册表。管理多个 ModelProvider（= 多个预设插件）。
pub struct ModelRegistry {
    providers: std::collections::BTreeMap<ModelId, Box<dyn ModelProvider>>,
    default_id: Option<ModelId>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self { providers: Default::default(), default_id: None }
    }

    /// 注册一个模型提供者（来自预设插件）
    pub fn register(&mut self, provider: Box<dyn ModelProvider>) -> &mut Self {
        let id = provider.id().clone();
        info!(
            "[model] registering model: id={}, kind={:?}, tool_calling={}, code_quality={}",
            id, provider.capability().kind, provider.capability().tool_calling,
            provider.capability().code_quality
        );
        if self.default_id.is_none() {
            self.default_id = Some(id.clone());
            info!("[model] set {} as default model (first registration)", id);
        }
        self.providers.insert(id, provider);
        self
    }

    pub fn get(&self, id: &str) -> Option<&dyn ModelProvider> {
        self.providers.get(id).map(|p| p.as_ref())
    }

    pub fn default(&self) -> Option<&dyn ModelProvider> {
        self.default_id.as_ref().and_then(|id| self.get(id))
    }

    pub fn list(&self) -> Vec<(&ModelId, &ModelCapability)> {
        self.providers.values().map(|p| (p.id(), p.capability())).collect()
    }

    /// Return owned infos with display name + capability
    pub fn list_models(&self) -> Vec<ModelInfo> {
        self.providers.values().map(|p| ModelInfo {
            id: p.id().clone(),
            display_name: p.display_name().to_string(),
            capability: p.capability().clone(),
        }).collect()
    }

    pub fn set_default(&mut self, id: &str) -> anyhow::Result<()> {
        if !self.providers.contains_key(id) {
            warn!("[model] cannot set default: model '{}' not registered", id);
            anyhow::bail!("model not registered: {id}");
        }
        info!("[model] switching default model to: {}", id);
        self.default_id = Some(id.to_string());
        Ok(())
    }
}

impl Default for ModelRegistry { fn default() -> Self { Self::new() } }

// ============================================================
// Built-in plugin wrappers: convert route-plugins into ModelProvider.
// These are the "预设插件" that give Agent different models.
// ============================================================

/// A lightweight deterministic "mock model" that always returns the last
/// user message with a prefix. Used as the zero-cost baseline model and
/// as a proof that plugins can directly back Agent models.
pub struct MockPluginModel {
    id: ModelId,
    capability: ModelCapability,
    prefix: String,
}

impl MockPluginModel {
    /// Build a mock model named `mock/<id>` that prefixes responses with `mock:`.
    pub fn new<S: Into<String>>(id: S) -> Box<dyn ModelProvider> {
        let inner_id = format!("mock/{}", id.into());
        Box::new(Self {
            id: inner_id,
            capability: ModelCapability {
                kind: ModelKind::Mock,
                tool_calling: false,
                long_context: true,
                code_quality: 10,
                multimodal: false,
                latency_tier: 1,
                features: vec!["deterministic".into(), "cheap".into()],
                cost_per_second_cents: Some(0),
            },
            prefix: "mock:".to_string(),
        })
    }

    /// Build a deterministic echo model (repeats the last user message).
    pub fn echo<S: Into<String>>(id: S) -> Box<dyn ModelProvider> {
        let inner_id = format!("echo/{}", id.into());
        Box::new(Self {
            id: inner_id,
            capability: ModelCapability {
                kind: ModelKind::Mock,
                tool_calling: false,
                long_context: true,
                code_quality: 5,
                multimodal: false,
                latency_tier: 1,
                features: vec!["echo".into()],
                cost_per_second_cents: Some(0),
            },
            prefix: "echo:".to_string(),
        })
    }
}

impl ModelProvider for MockPluginModel {
    fn id(&self) -> &ModelId { &self.id }
    fn capability(&self) -> &ModelCapability { &self.capability }
    fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        debug!(
            "[model] MockPluginModel.chat called: id={}, messages={}, roles={:?}",
            self.id, messages.len(),
            messages.iter().map(|m| format!("{:?}", m.role)).collect::<Vec<_>>()
        );
        let last = messages.last().map(|m| m.content.clone()).unwrap_or_default();
        let response = format!("{}{}\n\n(please use tool_call:read_file to inspect project)", self.prefix, last);
        debug!("[model] MockPluginModel response: {} chars", response.len());
        Ok(response)
    }
}

/// Configuration for an OpenAI-compatible endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiLikeConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    #[serde(default = "default_temp")] pub temperature: f32,
    #[serde(default)] pub max_tokens: Option<u32>,
}

fn default_temp() -> f32 { 0.7 }

impl Default for OpenAiLikeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.7,
            max_tokens: None,
        }
    }
}

/// Wraps an HTTP OpenAI-compatible endpoint as a ModelProvider.
///
/// This is how the preset plugin `openai_completions` is exposed to Agent.
/// When a user installs the OpenAI plugin, this wrapper is registered.
pub struct OpenAiPluginModel {
    id: ModelId,
    capability: ModelCapability,
    cfg: OpenAiLikeConfig,
}

impl OpenAiPluginModel {
    pub fn new(cfg: OpenAiLikeConfig) -> Box<dyn ModelProvider> {
        let id = format!("openai/{}", cfg.model);
        let m = cfg.model.to_lowercase();
        let long = m.contains("128k") || m.contains("gpt-4o") || m.contains("4o") || m.contains("sonnet") || m.contains("opus");
        let code = if m.contains("o1") || m.contains("gpt-4o") { 92 }
            else if m.contains("4-turbo") || m.contains("sonnet") { 88 }
            else if m.contains("mini") { 72 }
            else { 65 };
        let tc = !(m.contains("gpt-3.5") || m.contains("davinci"));
        Box::new(Self {
            id,
            capability: ModelCapability {
                kind: ModelKind::OpenAi,
                tool_calling: tc,
                long_context: long,
                code_quality: code,
                multimodal: m.contains("gpt-4o") || m.contains("vision"),
                latency_tier: if m.contains("o1") { 5 } else if m.contains("mini") { 2 } else { 3 },
                features: vec!["chat".into(), "tool_calling".into()],
                cost_per_second_cents: Some(if m.contains("o1") || m.contains("opus") { 50 } else { 5 }),
            },
            cfg,
        })
    }
}

impl ModelProvider for OpenAiPluginModel {
    fn id(&self) -> &ModelId { &self.id }
    fn capability(&self) -> &ModelCapability { &self.capability }
    fn chat(&self, messages: Vec<ChatMessage>) -> anyhow::Result<String> {
        info!(
            "[model] OpenAiPluginModel.chat: id={}, model={}, messages={}, api_key_set={}",
            self.id, self.cfg.model, messages.len(), !self.cfg.api_key.is_empty()
        );
        if self.cfg.api_key.is_empty() {
            error!("[model] OpenAI API key is empty");
            anyhow::bail!("OpenAI API key is empty; set OPENAI_API_KEY or configure plugins.json");
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;
        debug!(
            "[model] sending request to: {}/chat/completions, model={}, temp={}",
            self.cfg.base_url, self.cfg.model, self.cfg.temperature
        );
        let body = serde_json::json!({
            "model": self.cfg.model,
            "messages": messages.into_iter().map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                        ChatRole::Tool => "tool",
                    },
                    "content": m.content
                })
            }).collect::<Vec<_>>(),
            "temperature": self.cfg.temperature,
            "max_tokens": self.cfg.max_tokens,
        });

        let req_start = std::time::Instant::now();
        let resp = client
            .post(format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/')))
            .bearer_auth(&self.cfg.api_key)
            .header("content-type", "application/json")
            .json(&body)
            .send()?;

        let status = resp.status();
        info!(
            "[model] OpenAI response: status={}, elapsed={}ms",
            status, req_start.elapsed().as_millis()
        );
        if !status.is_success() {
            error!("[model] OpenAI API error: status={}", status);
        }

        let resp = resp.error_for_status()?;
        let val: serde_json::Value = resp.json()?;
        let txt = match val["choices"][0]["message"]["content"].as_str() {
            Some(s) => s.to_string(),
            None => {
                let err_msg = format!("no content in openai response: {}", val);
                error!("[model] {}", err_msg);
                anyhow::bail!(err_msg);
            }
        };
        info!("[model] OpenAI response content: {} chars", txt.len());
        Ok(txt)
    }
}
