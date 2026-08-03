//! LLM integration — pre-injected prompts, injection slots, and file
//! content injection for third-party LLM clients.
//!
//! ## Overview
//!
//! Route can act as a proxy between the user and third-party LLMs.
//! When the user calls an LLM through Route, the following are
//! automatically injected into the prompt:
//!
//! 1. **Pre-injected prompts** — system-level instructions that define
//!    how the LLM should behave. These are set by Route and can be
//!    customized by the user.
//! 2. **Reserved injection slot** — a user-defined text snippet that
//!    is prepended to every LLM call. This can be used for persistent
//!    instructions, context, or constraints.
//! 3. **File content injection** — when enabled, the contents of
//!    specified files are injected into the prompt. This allows the
//!    LLM to see the full project context.
//!
//! ## Token consumption warning
//!
//! Large files or long presets can consume significant token budgets.
//! Route displays a warning when the total injected content exceeds
//! a configurable threshold (default: 4096 characters ≈ 1000 tokens).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for LLM injection, stored in `.route/llm-injection.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmInjectionConfig {
    /// Whether file content injection is enabled.
    pub enabled: bool,

    /// Pre-injected system prompts. These are always included.
    /// Route ships with default prompts; users can override them.
    pub system_prompts: Vec<String>,

    /// Reserved injection slot — user-defined text that is prepended
    /// to every LLM call. Empty by default.
    pub user_slot: String,

    /// Glob patterns for files to inject. Relative to the project root.
    /// Examples: ["src/**/*.rs", "docs/**/*.md", "*.toml"]
    pub file_patterns: Vec<String>,

    /// Maximum number of files to inject (to prevent runaway token usage).
    pub max_files: usize,

    /// Maximum total characters to inject from files (soft limit).
    /// When exceeded, a warning is returned alongside the injected text.
    pub max_chars: usize,

    /// Whether to show the token consumption warning.
    pub show_warning: bool,
}

impl Default for LlmInjectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            system_prompts: vec![
                "You are an AI assistant integrated with the Route version management system. \
                 You have access to the project's file contents and version history. \
                 Use this context to provide accurate, context-aware assistance."
                    .to_string(),
            ],
            user_slot: String::new(),
            file_patterns: Vec::new(),
            max_files: 20,
            max_chars: 50_000,
            show_warning: true,
        }
    }
}

impl LlmInjectionConfig {
    /// Path to the config file within a project's `.route` directory.
    pub fn config_path(project_path: &Path) -> std::path::PathBuf {
        project_path.join(".route").join("llm-injection.json")
    }

    /// Load the config from disk. Returns the default if the file
    /// doesn't exist or can't be parsed.
    pub fn load(project_path: &Path) -> Self {
        let path = Self::config_path(project_path);
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save the config to disk.
    pub fn save(&self, project_path: &Path) -> Result<(), String> {
        let path = Self::config_path(project_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create config directory: {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("serialization error: {e}"))?;
        std::fs::write(&path, &raw)
            .map_err(|e| format!("cannot write config: {e}"))?;
        Ok(())
    }

    /// Build the full injected text for an LLM call.
    ///
    /// Returns `(injected_text, warnings)` where `warnings` contains
    /// token-consumption warnings when the content exceeds thresholds.
    pub fn build_injection(
        &self,
        project_path: &Path,
        user_prompt: &str,
    ) -> (String, Vec<String>) {
        let mut parts: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut total_chars: usize = 0;

        // 1. Pre-injected system prompts.
        if !self.system_prompts.is_empty() {
            let mut sys_block = String::from("=== System Instructions ===\n");
            for (i, prompt) in self.system_prompts.iter().enumerate() {
                sys_block.push_str(&format!("[{i}] {prompt}\n"));
            }
            sys_block.push('\n');
            total_chars += sys_block.len();
            parts.push(sys_block);
        }

        // 2. Reserved user slot.
        if !self.user_slot.is_empty() {
            let slot = format!("=== Custom Instructions ===\n{}\n\n", self.user_slot);
            total_chars += slot.len();
            parts.push(slot);
        }

        // 3. User's current prompt.
        let user_block = format!("=== User Prompt ===\n{user_prompt}\n\n");
        total_chars += user_block.len();
        parts.push(user_block);

        // 4. File content injection.
        if self.enabled && !self.file_patterns.is_empty() {
            let files = self.collect_files(project_path);
            if !files.is_empty() {
                let mut file_block = String::from("=== Project Files ===\n");
                let mut file_count = 0;
                for f in &files {
                    if file_count >= self.max_files {
                        warnings.push(format!(
                            "File injection limit reached ({}/{}) — some files were skipped.",
                            self.max_files, files.len()
                        ));
                        break;
                    }
                    match std::fs::read_to_string(f) {
                        Ok(content) => {
                            let rel = f
                                .strip_prefix(project_path)
                                .unwrap_or(f)
                                .to_string_lossy();
                            // Truncate each file to 10k chars to prevent
                            // a single massive file from blowing the budget.
                            let truncated = if content.len() > 10_000 {
                                let mut t: String = content.chars().take(10_000).collect();
                                t.push_str("\n... [truncated at 10k chars]");
                                t
                            } else {
                                content
                            };
                            file_block.push_str(&format!("\n--- {rel} ---\n{truncated}\n"));
                            total_chars += rel.len() + truncated.len() + 20;
                            file_count += 1;
                        }
                        Err(e) => {
                            file_block.push_str(&format!(
                                "\n--- {} ---\n(cannot read: {e})\n",
                                f.display()
                            ));
                        }
                    }
                }
                file_block.push('\n');
                parts.push(file_block);
            }
        }

        // 5. Warning check.
        if self.show_warning && total_chars > self.max_chars {
            let estimated_tokens = total_chars / 4;
            warnings.push(format!(
                "⚠️  Injected content is approximately {total_chars} characters \
                 (~{estimated_tokens} tokens). This may consume significant token \
                 budget. Consider reducing file patterns or disabling file injection \
                 if token costs are a concern."
            ));
        }

        (parts.concat(), warnings)
    }

    /// Collect file paths matching the configured patterns.
    fn collect_files(&self, project_path: &Path) -> Vec<std::path::PathBuf> {
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for pattern in &self.file_patterns {
            let glob_pattern = project_path.join(pattern);
            if let Ok(entries) = glob::glob(&glob_pattern.to_string_lossy()) {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        files.push(entry);
                    }
                }
            }
        }
        files.sort();
        files.dedup();
        files
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

use tauri::State;

use crate::state::AppState;

/// DTO for the LLM injection config (returned to frontend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmInjectionConfigDto {
    pub enabled: bool,
    pub system_prompts: Vec<String>,
    pub user_slot: String,
    pub file_patterns: Vec<String>,
    pub max_files: usize,
    pub max_chars: usize,
    pub show_warning: bool,
}

impl From<LlmInjectionConfig> for LlmInjectionConfigDto {
    fn from(c: LlmInjectionConfig) -> Self {
        Self {
            enabled: c.enabled,
            system_prompts: c.system_prompts,
            user_slot: c.user_slot,
            file_patterns: c.file_patterns,
            max_files: c.max_files,
            max_chars: c.max_chars,
            show_warning: c.show_warning,
        }
    }
}

/// Get the current LLM injection configuration.
#[tauri::command]
pub fn llm_injection_get(state: State<'_, AppState>) -> Result<LlmInjectionConfigDto, String> {
    let path = crate::git_commands::project_path(&state)?;
    let config = LlmInjectionConfig::load(&path);
    Ok(config.into())
}

/// Update the LLM injection configuration.
#[tauri::command]
pub fn llm_injection_set(
    state: State<'_, AppState>,
    config: LlmInjectionConfigDto,
) -> Result<(), String> {
    let path = crate::git_commands::project_path(&state)?;
    let new_config = LlmInjectionConfig {
        enabled: config.enabled,
        system_prompts: config.system_prompts,
        user_slot: config.user_slot,
        file_patterns: config.file_patterns,
        max_files: config.max_files,
        max_chars: config.max_chars,
        show_warning: config.show_warning,
    };
    new_config.save(&path)
}

/// Build the full injected text. Returns the text and any warnings.
#[tauri::command]
pub fn llm_injection_build(
    state: State<'_, AppState>,
    user_prompt: String,
) -> Result<LlmBuildResult, String> {
    let path = crate::git_commands::project_path(&state)?;
    let config = LlmInjectionConfig::load(&path);
    let (text, warnings) = config.build_injection(&path, &user_prompt);
    Ok(LlmBuildResult { text, warnings })
}

/// Result of building an LLM injection.
#[derive(Debug, Clone, Serialize)]
pub struct LlmBuildResult {
    pub text: String,
    pub warnings: Vec<String>,
}

/// Send a chat to the LLM with injection enabled. This is a convenience
/// command that wraps `ai_chat` with automatic injection of the configured
/// prompts and files.
#[tauri::command]
pub async fn llm_chat_with_injection(
    state: State<'_, AppState>,
    provider: String,
    endpoint: String,
    key: String,
    model: String,
    user_prompt: String,
) -> Result<String, String> {
    let path = crate::git_commands::project_path(&state)?;
    let config = LlmInjectionConfig::load(&path);
    let (injected_text, warnings) = config.build_injection(&path, &user_prompt);

    // Build the messages array with the injected text as a system message
    // and the user prompt as the user message.
    let mut messages = vec![crate::ai_commands::ChatMessage {
        role: "system".to_string(),
        content: injected_text,
    }];

    // Only add the user message if it's not already included in the injection.
    messages.push(crate::ai_commands::ChatMessage {
        role: "user".to_string(),
        content: user_prompt,
    });

    // Call the AI provider.
    let result = crate::ai_commands::ai_chat(provider, endpoint, key, model, messages).await;

    // Prepend warnings if any.
    match result {
        Ok(reply) => {
            if warnings.is_empty() {
                Ok(reply)
            } else {
                let warning_block = warnings.join("\n");
                Ok(format!("[WARNINGS]\n{warning_block}\n\n[REPLY]\n{reply}"))
            }
        }
        Err(e) => Err(e),
    }
}