//! MCP (Model Context Protocol) server management commands.
//!
//! The Route desktop app does not run the MCP server itself — the AI
//! client (Claude Desktop, Cursor, Trae, Codex, …) spawns `route-mcp`
//! as a child process and talks to it over stdio. The Tauri app's job is:
//!
//! 1. Locate the `route-mcp` binary.
//! 2. Build the JSON config snippet for each supported AI client,
//!    pre-filled with `--project <path>`.
//! 3. Surface all of this to the frontend so the settings page can
//!    display it with a copy button.
//!
//! ## Supported Clients
//!
//! | Client       | Config Location                                  | Format              |
//! |--------------|--------------------------------------------------|---------------------|
//! | Claude Desktop | `claude_desktop_config.json`                   | `mcpServers` map    |
//! | Claude Code  | `.mcp.json` (project root) or `~/.claude/.mcp.json` | `mcpServers` map |
//! | Codex (OpenAI) | `~/.codex/config.toml` or env var             | TOML / env          |
//! | Trae IDE     | `mcp_servers.json` in IDE config dir             | `mcpServers` map    |
//! | Cursor       | `.cursor/mcp.json` (project root)                | `mcpServers` map    |
//! | Continue.dev | `~/.continue/config.json`                        | `mcpServers` map    |
//! | Generic      | Any MCP-compatible client (stdio JSON-RPC)       | `mcpServers` map    |

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Client types
// ---------------------------------------------------------------------------

/// Supported AI clients that can consume MCP servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpClientType {
    /// Claude Desktop (macOS / Windows app)
    ClaudeDesktop,
    /// Claude Code (CLI tool, `claude` command)
    ClaudeCode,
    /// OpenAI Codex CLI
    Codex,
    /// Trae CN IDE
    Trae,
    /// Cursor IDE
    Cursor,
    /// Continue.dev (VS Code / JetBrains extension)
    Continue,
    /// Generic MCP-compatible client (stdio JSON-RPC 2.0)
    Generic,
}

impl McpClientType {
    /// Human-readable display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            McpClientType::ClaudeDesktop => "Claude Desktop",
            McpClientType::ClaudeCode => "Claude Code",
            McpClientType::Codex => "OpenAI Codex CLI",
            McpClientType::Trae => "Trae IDE",
            McpClientType::Cursor => "Cursor IDE",
            McpClientType::Continue => "Continue.dev",
            McpClientType::Generic => "Generic MCP Client",
        }
    }

    /// Description shown to the user.
    pub fn description(&self) -> &'static str {
        match self {
            McpClientType::ClaudeDesktop => "Anthropic Claude Desktop app. Paste into claude_desktop_config.json.",
            McpClientType::ClaudeCode => "Anthropic Claude Code CLI. Place .mcp.json in your project root or ~/.claude/.",
            McpClientType::Codex => "OpenAI Codex CLI. Set CODEC_MCP_SERVERS env var or use config.toml.",
            McpClientType::Trae => "Trae CN IDE. Configure in IDE settings → MCP Servers.",
            McpClientType::Cursor => "Cursor IDE. Place .cursor/mcp.json in your project root.",
            McpClientType::Continue => "Continue.dev extension. Add to ~/.continue/config.json under `mcpServers`.",
            McpClientType::Generic => "Any MCP-compatible client using stdio JSON-RPC 2.0 transport.",
        }
    }

    /// Icon / emoji for the client.
    pub fn icon(&self) -> &'static str {
        match self {
            McpClientType::ClaudeDesktop => "🧠",
            McpClientType::ClaudeCode => "🖥️",
            McpClientType::Codex => "🤖",
            McpClientType::Trae => "🛠️",
            McpClientType::Cursor => "➡️",
            McpClientType::Continue => "🔄",
            McpClientType::Generic => "🔌",
        }
    }

    /// Config file path hint (relative to home or project).
    pub fn config_path_hint(&self) -> &'static str {
        match self {
            McpClientType::ClaudeDesktop => "~/Library/Application Support/Claude/claude_desktop_config.json (macOS)\n%APPDATA%\\Claude\\claude_desktop_config.json (Windows)",
            McpClientType::ClaudeCode => ".mcp.json (project root) or ~/.claude/.mcp.json",
            McpClientType::Codex => "~/.codex/config.toml or CODEC_MCP_SERVERS env var",
            McpClientType::Trae => "IDE Settings → MCP Servers configuration",
            McpClientType::Cursor => ".cursor/mcp.json (project root)",
            McpClientType::Continue => "~/.continue/config.json",
            McpClientType::Generic => "Any MCP-compatible client configuration",
        }
    }

    /// Whether this client uses `mcpServers` JSON format.
    pub fn uses_mcp_servers_json(&self) -> bool {
        match self {
            McpClientType::Codex => false, // Codex uses TOML or env var
            _ => true,
        }
    }

    /// List all supported clients.
    pub fn all() -> &'static [McpClientType] {
        &[
            McpClientType::ClaudeDesktop,
            McpClientType::ClaudeCode,
            McpClientType::Codex,
            McpClientType::Trae,
            McpClientType::Cursor,
            McpClientType::Continue,
            McpClientType::Generic,
        ]
    }
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// DTO for a single client's config snippet.
#[derive(Debug, Clone, Serialize)]
pub struct ClientConfigDto {
    /// Client type identifier.
    pub client_type: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Description.
    pub description: String,
    /// Emoji icon.
    pub icon: String,
    /// Config file path hint.
    pub config_path_hint: String,
    /// The config snippet (JSON, TOML, or shell command).
    pub config_snippet: String,
    /// Format of the snippet: "json", "toml", "shell", "env".
    pub snippet_format: String,
}

/// DTO returned by `mcp_get_config`. The frontend renders the
/// `config_snippet` in a code block with a copy button.
#[derive(Debug, Serialize)]
pub struct McpConfigDto {
    /// Absolute path to the `route-mcp` binary.
    pub binary_path: String,
    /// True if the binary was found on disk.
    pub binary_exists: bool,
    /// The Route project path that will be passed via `--project`.
    pub project_path: String,
    /// Pretty-printed JSON for Claude Desktop (backward compat).
    pub config_snippet: String,
    /// Config snippets for all supported clients.
    pub clients: Vec<ClientConfigDto>,
}

// ---------------------------------------------------------------------------
// Binary path resolution
// ---------------------------------------------------------------------------

fn resolve_binary_path() -> (String, bool) {
    let binary_name = if cfg!(windows) {
        "route-mcp.exe"
    } else {
        "route-mcp"
    };

    // 1. Next to the current executable (production bundle).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(binary_name);
            if candidate.exists() {
                return (candidate.display().to_string(), true);
            }
        }
    }

    // 2. Workspace target/debug (development).
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_candidate = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("target").join("debug").join(binary_name));

    if let Some(p) = dev_candidate {
        if p.exists() {
            return (p.display().to_string(), true);
        }
    }

    // 3. Also check target/release.
    let release_candidate = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("target").join("release").join(binary_name));

    if let Some(p) = release_candidate {
        if p.exists() {
            return (p.display().to_string(), true);
        }
    }

    ("route-mcp".to_string(), false)
}

/// Get the MCP command string (resolved binary path or bare name).
fn mcp_command() -> String {
    let (path, exists) = resolve_binary_path();
    if exists { path } else { "route-mcp".to_string() }
}

// ---------------------------------------------------------------------------
// Config generators for each client
// ---------------------------------------------------------------------------

fn build_claude_desktop_config(project_path: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": mcp_command(),
                "args": ["--project", project_path, "--permission-level", "normal"]
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn build_claude_code_config(project_path: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": mcp_command(),
                "args": ["--project", project_path, "--permission-level", "normal"]
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn build_codex_config(project_path: &str) -> String {
    let cmd = mcp_command();
    // Codex uses TOML config or environment variable
    format!(
        r#"# Option 1: Set environment variable (Normal mode — remote blocked)
export CODEC_MCP_SERVERS='{{"route":{{"command":"{}","args":["--project","{}","--permission-level","normal"]}}}}'

# Option 2: High permission mode (all operations allowed)
# export CODEC_MCP_SERVERS='{{"route":{{"command":"{}","args":["--project","{}","--permission-level","high"]}}}}'

# Option 3: Add to ~/.codex/config.toml
# [mcp_servers.route]
# command = "{}"
# args = ["--project", "{}", "--permission-level", "normal"]
"#,
        cmd, project_path, cmd, project_path, cmd, project_path
    )
}

fn build_trae_config(project_path: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": mcp_command(),
                "args": ["--project", project_path, "--permission-level", "normal"]
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn build_cursor_config(project_path: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": mcp_command(),
                "args": ["--project", project_path, "--permission-level", "normal"]
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn build_continue_config(project_path: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": mcp_command(),
                "args": ["--project", project_path, "--permission-level", "normal"]
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn build_generic_config(project_path: &str) -> String {
    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": mcp_command(),
                "args": ["--project", project_path, "--permission-level", "normal"],
                "transport": "stdio"
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

fn build_config_for_client(client: McpClientType, project_path: &str) -> ClientConfigDto {
    let (config_snippet, snippet_format) = match client {
        McpClientType::ClaudeDesktop => (build_claude_desktop_config(project_path), "json"),
        McpClientType::ClaudeCode => (build_claude_code_config(project_path), "json"),
        McpClientType::Codex => (build_codex_config(project_path), "shell"),
        McpClientType::Trae => (build_trae_config(project_path), "json"),
        McpClientType::Cursor => (build_cursor_config(project_path), "json"),
        McpClientType::Continue => (build_continue_config(project_path), "json"),
        McpClientType::Generic => (build_generic_config(project_path), "json"),
    };

    ClientConfigDto {
        client_type: format!("{:?}", client).to_lowercase().replace('_', "-"),
        display_name: client.display_name().to_string(),
        description: client.description().to_string(),
        icon: client.icon().to_string(),
        config_path_hint: client.config_path_hint().to_string(),
        config_snippet,
        snippet_format: snippet_format.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Return the MCP server configuration for the currently-open project.
/// Includes config snippets for all supported AI clients.
#[tauri::command]
pub fn mcp_get_config(state: State<'_, AppState>) -> Result<McpConfigDto, String> {
    let project_path = state
        .project_path
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let (binary_path, binary_exists) = resolve_binary_path();

    // Build config snippets for all clients.
    let clients: Vec<ClientConfigDto> = McpClientType::all()
        .iter()
        .map(|c| build_config_for_client(*c, &project_path))
        .collect();

    // Backward-compatible: Claude Desktop config as the default snippet.
    let config_snippet = build_claude_desktop_config(&project_path);

    Ok(McpConfigDto {
        binary_path,
        binary_exists,
        project_path,
        config_snippet,
        clients,
    })
}

/// Return config for a specific client type.
#[tauri::command]
pub fn mcp_get_config_for_client(
    state: State<'_, AppState>,
    client_type: String,
) -> Result<ClientConfigDto, String> {
    let project_path = state
        .project_path
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let client = match client_type.to_lowercase().as_str() {
        "claude-desktop" | "claude_desktop" => McpClientType::ClaudeDesktop,
        "claude-code" | "claude_code" => McpClientType::ClaudeCode,
        "codex" => McpClientType::Codex,
        "trae" => McpClientType::Trae,
        "cursor" => McpClientType::Cursor,
        "continue" | "continue-dev" | "continue_dev" => McpClientType::Continue,
        "generic" => McpClientType::Generic,
        other => return Err(format!(
            "Unknown client type '{}'. Supported: claude-desktop, claude-code, codex, trae, cursor, continue, generic",
            other
        )),
    };

    Ok(build_config_for_client(client, &project_path))
}

/// List all supported AI clients with their metadata.
#[tauri::command]
pub fn mcp_list_clients() -> Vec<serde_json::Value> {
    McpClientType::all()
        .iter()
        .map(|c| {
            serde_json::json!({
                "client_type": format!("{:?}", c).to_lowercase().replace('_', "-"),
                "display_name": c.display_name(),
                "description": c.description(),
                "icon": c.icon(),
                "config_path_hint": c.config_path_hint(),
                "uses_mcp_servers_json": c.uses_mcp_servers_json(),
            })
        })
        .collect()
}