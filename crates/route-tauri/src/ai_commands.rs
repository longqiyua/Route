//! AI provider abstraction — lets the "active AI mode" talk to multiple LLM
//! backends through one command instead of being hardcoded to OpenAI.
//!
//! Supported providers (`provider` argument):
//!   - `openai` / `openai-compatible` — OpenAI Chat Completions API
//!     (`POST {endpoint}/chat/completions`, `Authorization: Bearer <key>`).
//!     Covers OpenAI itself and any OpenAI-compatible server (DeepSeek,
//!     Together, Groq, vLLM, LiteLLM, etc.).
//!   - `anthropic` — Anthropic Messages API
//!     (`POST {endpoint}/v1/messages`, `x-api-key`, `anthropic-version`).
//!     System messages are lifted to the top-level `system` field per the
//!     Anthropic schema.
//!   - `ollama` — local Ollama server (`POST {endpoint}/api/chat`, no key).
//!     Defaults to `http://localhost:11434`. Keeps the key field unused.
//!
//! Design notes:
//!   - The HTTP call runs in Rust (not the renderer) so the API key never
//!     touches the webview and there are no CORS headaches (Ollama on
//!     localhost, Anthropic's browser-access header, etc. are all moot).
//!   - Requests are async and run on Tauri's tokio runtime. A 60s timeout
//!     prevents a hung model from freezing the UI.
//!   - We return the assistant's text. Streaming is intentionally not
//!     supported here — this is the "one-shot completion" primitive the
//!     settings page uses for its connection test, and that a future agent
//!     loop can build on.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One chat message. `role` is `system` / `user` / `assistant`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Hard cap for a single LLM round-trip. Models can be slow; if 60s isn't
/// enough the user probably wants streaming, which is out of scope here.
const CHAT_TIMEOUT: Duration = Duration::from_secs(60);

/// Send a chat completion request to the configured provider and return the
/// assistant's reply text.
///
/// `endpoint` is the API base (e.g. `https://api.openai.com/v1`,
/// `https://api.anthropic.com`, `http://localhost:11434`). We append the
/// provider-specific path, detecting when the user already gave a full URL.
#[tauri::command]
pub async fn ai_chat(
    provider: String,
    endpoint: String,
    key: String,
    model: String,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    let provider = provider.trim().to_lowercase();
    let model = model.trim();
    if model.is_empty() && provider != "ollama" {
        // Ollama can fall back to its default model when none is given;
        // the cloud providers require one.
        return Err("model is required".to_string());
    }
    if messages.is_empty() {
        return Err("messages must not be empty".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(CHAT_TIMEOUT)
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    match provider.as_str() {
        "openai" | "openai-compatible" | "" => {
            chat_openai(&client, &endpoint, &key, model, &messages).await
        }
        "anthropic" => chat_anthropic(&client, &endpoint, &key, model, &messages).await,
        "ollama" => chat_ollama(&client, &endpoint, model, &messages).await,
        other => Err(format!("unknown provider: {other}")),
    }
}

/// Join an API base and a path, tolerating trailing slashes and a user who
/// already pasted the full URL. `suffix` should not include a leading slash.
fn join_url(base: &str, suffix: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.is_empty() {
        return format!("/{suffix}");
    }
    // If the base already ends with the suffix (user pasted the full URL),
    // don't double it up.
    if base.ends_with(suffix) {
        base.to_string()
    } else {
        format!("{base}/{suffix}")
    }
}

/// Extract the first `system` message's content (Anthropic puts it in a
/// top-level `system` field rather than the messages array). Returns the
/// system text and the remaining user/assistant messages.
fn split_system(messages: &[ChatMessage]) -> (Option<String>, Vec<Value>) {
    let mut system: Option<String> = None;
    let mut rest: Vec<Value> = Vec::new();
    for m in messages {
        if m.role == "system" && system.is_none() {
            system = Some(m.content.clone());
        } else {
            rest.push(json!({ "role": m.role, "content": m.content }));
        }
    }
    (system, rest)
}

// --- providers ---

async fn chat_openai(
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, String> {
    let url = join_url(endpoint, "chat/completions");
    let body = json!({
        "model": model,
        "messages": messages.iter().map(|m| json!({ "role": m.role, "content": m.content })).collect::<Vec<_>>(),
        "stream": false,
    });
    let mut req = client.post(&url).json(&body);
    if !key.trim().is_empty() {
        req = req.header("Authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {}", truncate(&text, 500)));
    }
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid JSON response: {e}"))?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no content in response: {}", truncate(&text, 200)))
}

async fn chat_anthropic(
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, String> {
    if key.trim().is_empty() {
        return Err("Anthropic requires an API key".to_string());
    }
    let url = join_url(endpoint, "v1/messages");
    let (system, rest) = split_system(messages);
    let mut body = json!({
        "model": model,
        "messages": rest,
        "max_tokens": 1024,
    });
    if let Some(s) = system {
        body["system"] = json!(s);
    }
    let resp = client
        .post(&url)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {}", truncate(&text, 500)));
    }
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid JSON response: {e}"))?;
    // Anthropic returns `content` as an array of blocks; the first text
    // block holds the reply.
    v["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no content in response: {}", truncate(&text, 200)))
}

async fn chat_ollama(
    client: &reqwest::Client,
    endpoint: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, String> {
    // Ollama has no default endpoint in the UI when the field is blank, so
    // fall back to the canonical local address.
    let base = if endpoint.trim().is_empty() {
        "http://localhost:11434"
    } else {
        endpoint
    };
    let url = join_url(base, "api/chat");
    let mut body = json!({
        "messages": messages.iter().map(|m| json!({ "role": m.role, "content": m.content })).collect::<Vec<_>>(),
        "stream": false,
    });
    if !model.is_empty() {
        body["model"] = json!(model);
    }
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("HTTP {status}: {}", truncate(&text, 500)));
    }
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| format!("invalid JSON response: {e}"))?;
    v["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no content in response: {}", truncate(&text, 200)))
}

/// Truncate a string to `max` chars, appending an ellipsis if cut. Used so
/// a huge error body from a provider doesn't flood the UI.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}
