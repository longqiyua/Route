//! MCP (Model Context Protocol) server management commands.
//!
//! The Route desktop app does not run the MCP server itself — the AI
//! client (Claude Desktop, Cursor, a vibecoding agent, …) spawns
//! `route-mcp` as a child process and talks to it over stdio. The
//! Tauri app's job is to:
//!
//! 1. Locate the `route-mcp` binary (next to the main exe in production,
//!    or in the workspace `target/debug` dir during development).
//! 2. Build the JSON config snippet the user pastes into their AI
//!    client's MCP settings, pre-filled with `--project <path>` so the
//!    server operates on the currently-open project.
//! 3. Surface all of this to the frontend so the settings page can
//!    display it with a copy button.

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

/// DTO returned by `mcp_get_config`. The frontend renders the
/// `config_snippet` in a code block with a copy button.
#[derive(Debug, Serialize)]
pub struct McpConfigDto {
    /// Absolute path to the `route-mcp` binary, or just the binary name
    /// if it hasn't been built yet (so the user knows what to build).
    pub binary_path: String,
    /// True if the binary was found on disk.
    pub binary_exists: bool,
    /// The Route project path that will be passed via `--project`.
    pub project_path: String,
    /// Pretty-printed JSON the user pastes into their AI client's MCP
    /// config. Shape: `{ "mcpServers": { "route": { "command": "...",
    /// "args": ["--project", "..."] } } }`.
    pub config_snippet: String,
}

/// Resolve the `route-mcp` binary path.
///
/// In production the binary is bundled next to the main executable. In
/// development it lives in the workspace `target/debug` directory. We
/// check both locations and return the first match; if neither exists we
/// return the bare name `route-mcp` so the config snippet is still
/// syntactically valid (the user just needs to build it first).
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

    // 2. Workspace target/debug (development). CARGO_MANIFEST_DIR is
    //    crates/route-tauri, so two parent()s get us to the workspace
    //    root.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_candidate = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .map(|root| root.join("target").join("debug").join(binary_name));

    if let Some(p) = dev_candidate {
        if p.exists() {
            return (p.display().to_string(), true);
        }
    }

    // 3. Not found — return the bare name and let the frontend show a
    //    "build it first" hint.
    ("route-mcp".to_string(), false)
}

/// Return the MCP server configuration for the currently-open project.
/// The frontend uses this to render the config snippet + copy button in
/// the CLI/MCP settings card.
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

    // Build the config snippet. We use the resolved binary path if the
    // binary exists, otherwise the bare name — either way the JSON is
    // valid, the user just needs to build the binary if it's missing.
    let command = if binary_exists {
        binary_path.clone()
    } else {
        "route-mcp".to_string()
    };

    let config = serde_json::json!({
        "mcpServers": {
            "route": {
                "command": command,
                "args": ["--project", &project_path]
            }
        }
    });

    let config_snippet = serde_json::to_string_pretty(&config)
        .unwrap_or_else(|_| config.to_string());

    Ok(McpConfigDto {
        binary_path,
        binary_exists,
        project_path,
        config_snippet,
    })
}
