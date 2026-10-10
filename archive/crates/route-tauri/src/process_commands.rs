//! Process management commands — start/stop the Route CLI and MCP server
//! as child processes from the Tauri backend.
//!
//! Both `route-cli` (the binary is named `route`) and `route-mcp` are
//! workspace members, so their binaries are available alongside the Tauri
//! app in the build output directory. We resolve the binary path relative
//! to the current executable directory.

use std::process::{Command, Stdio};
use tauri::State;

use crate::state::ProcessManager;

/// Start the Route CLI server (`route`) as a background child process.
///
/// Returns an error if the CLI is already running or if the binary
/// cannot be found.
#[tauri::command]
pub fn start_route_cli(procman: State<'_, ProcessManager>) -> Result<String, String> {
    let mut guard = procman.0.lock().map_err(|e| e.to_string())?;
    if guard.cli.is_some() {
        return Err("Route CLI is already running".into());
    }

    let binary = resolve_binary("route");
    let child = Command::new(&binary)
        .args(["server", "--daemon"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start CLI: {e}"))?;

    let pid = child.id();
    guard.cli = Some(child);
    Ok(format!("Route CLI started (pid {pid})"))
}

/// Stop the Route CLI server.
#[tauri::command]
pub fn stop_route_cli(procman: State<'_, ProcessManager>) -> Result<String, String> {
    let mut guard = procman.0.lock().map_err(|e| e.to_string())?;
    match guard.cli.take() {
        Some(mut child) => {
            child.kill().map_err(|e| format!("Failed to stop CLI: {e}"))?;
            child.wait().ok();
            Ok("Route CLI stopped".into())
        }
        None => Err("Route CLI is not running".into()),
    }
}

/// Check if the Route CLI server is running.
#[tauri::command]
pub fn route_cli_status(procman: State<'_, ProcessManager>) -> Result<bool, String> {
    let guard = procman.0.lock().map_err(|e| e.to_string())?;
    Ok(guard.cli.is_some())
}

/// Start the Route MCP server (`route-mcp`) as a background child process.
///
/// The MCP server communicates over stdio (JSON-RPC 2.0) and is spawned
/// with the current project path so it can serve version-management tools.
#[tauri::command]
pub fn start_route_mcp(
    project_path: String,
    procman: State<'_, ProcessManager>,
) -> Result<String, String> {
    let mut guard = procman.0.lock().map_err(|e| e.to_string())?;
    if guard.mcp.is_some() {
        return Err("Route MCP is already running".into());
    }

    let binary = resolve_binary("route-mcp");
    let child = Command::new(&binary)
        .args(["--project", &project_path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to start MCP: {e}"))?;

    let pid = child.id();
    guard.mcp = Some(child);
    Ok(format!("Route MCP started (pid {pid})"))
}

/// Stop the Route MCP server.
#[tauri::command]
pub fn stop_route_mcp(procman: State<'_, ProcessManager>) -> Result<String, String> {
    let mut guard = procman.0.lock().map_err(|e| e.to_string())?;
    match guard.mcp.take() {
        Some(mut child) => {
            child.kill().map_err(|e| format!("Failed to stop MCP: {e}"))?;
            child.wait().ok();
            Ok("Route MCP stopped".into())
        }
        None => Err("Route MCP is not running".into()),
    }
}

/// Check if the Route MCP server is running.
#[tauri::command]
pub fn route_mcp_status(procman: State<'_, ProcessManager>) -> Result<bool, String> {
    let guard = procman.0.lock().map_err(|e| e.to_string())?;
    Ok(guard.mcp.is_some())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a binary name relative to the current executable directory.
/// Falls back to the bare name so `PATH` lookup still works during dev.
fn resolve_binary(name: &str) -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let candidates = [
        exe_dir.as_ref().map(|d| d.join(name)),
        exe_dir.as_ref().map(|d| {
            let mut p = d.join(name);
            p.set_extension("exe");
            p
        }),
    ];
    for c in candidates.iter().flatten() {
        if c.exists() {
            return c.to_string_lossy().to_string();
        }
    }
    name.to_string() // fallback: rely on PATH
}

/// Clean up any running processes (called on app exit).
#[allow(dead_code)]
pub fn cleanup_processes(procman: &ProcessManager) {
    let mut guard = match procman.0.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(mut child) = guard.cli.take() {
        child.kill().ok();
        child.wait().ok();
    }
    if let Some(mut child) = guard.mcp.take() {
        child.kill().ok();
        child.wait().ok();
    }
}