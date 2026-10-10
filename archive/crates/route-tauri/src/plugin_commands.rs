//! Tauri IPC commands for `plugin ...` — plugin management.
//!
//! Mirrors the CLI's `route plugin ...` surface. The actual config types
//! and bus-builder live in `route_plugins::config` so the CLI and GUI
//! stay in sync.

use route_plugins::{
    load_plugin_config, save_plugin_config, validate_plugin_builtin, validate_plugin_config,
    PluginEntry,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntryDto {
    pub name: String,
    pub enabled: bool,
    /// Free-form JSON config. `null` if the plugin has no config.
    #[serde(default)]
    pub config: serde_json::Value,
}

impl From<PluginEntry> for PluginEntryDto {
    fn from(e: PluginEntry) -> Self {
        Self {
            name: e.name,
            enabled: e.enabled,
            config: e.config,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn route_dir(state: &State<'_, AppState>) -> Result<std::path::PathBuf, String> {
    let path = state
        .project_path
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or_else(|| "No repository open".to_string())?;
    Ok(route_core::RoutePaths::new(&path).route_dir)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn plugin_list(state: State<'_, AppState>) -> Result<Vec<PluginEntryDto>, String> {
    let dir = route_dir(&state)?;
    let cfg = load_plugin_config(&dir).map_err(|e| e.to_string())?;
    Ok(cfg.plugins.into_iter().map(PluginEntryDto::from).collect())
}

#[tauri::command]
pub fn plugin_show(name: String, state: State<'_, AppState>) -> Result<PluginEntryDto, String> {
    let dir = route_dir(&state)?;
    let cfg = load_plugin_config(&dir).map_err(|e| e.to_string())?;
    let entry = cfg
        .plugins
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Plugin '{name}' is not installed"))?;
    Ok(PluginEntryDto::from(entry))
}

#[tauri::command]
pub fn plugin_install(
    name: String,
    config: Option<String>,
    state: State<'_, AppState>,
) -> Result<PluginEntryDto, String> {
    validate_plugin_builtin(&name).map_err(|e| e.to_string())?;
    let dir = route_dir(&state)?;
    let mut cfg = load_plugin_config(&dir).map_err(|e| e.to_string())?;
    if cfg.plugins.iter().any(|p| p.name == name) {
        return Err(format!("Plugin '{name}' is already installed"));
    }
    let config_value: serde_json::Value = match config.as_deref() {
        None => serde_json::Value::Null,
        Some(s) => serde_json::from_str(s).map_err(|e| format!("Invalid JSON config: {e}"))?,
    };
    validate_plugin_config(&name, &config_value).map_err(|e| e.to_string())?;
    let entry = PluginEntry {
        name: name.clone(),
        enabled: true,
        config: config_value,
    };
    cfg.plugins.push(entry.clone());
    save_plugin_config(&dir, &cfg).map_err(|e| e.to_string())?;
    Ok(PluginEntryDto::from(entry))
}

#[tauri::command]
pub fn plugin_remove(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let dir = route_dir(&state)?;
    let mut cfg = load_plugin_config(&dir).map_err(|e| e.to_string())?;
    let before = cfg.plugins.len();
    cfg.plugins.retain(|p| p.name != name);
    if cfg.plugins.len() == before {
        return Err(format!("Plugin '{name}' is not installed"));
    }
    save_plugin_config(&dir, &cfg).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn plugin_set_enabled(
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let dir = route_dir(&state)?;
    let mut cfg = load_plugin_config(&dir).map_err(|e| e.to_string())?;
    let entry = cfg
        .plugins
        .iter_mut()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("Plugin '{name}' is not installed"))?;
    entry.enabled = enabled;
    save_plugin_config(&dir, &cfg).map_err(|e| e.to_string())?;
    Ok(())
}

/// Return the list of built-in plugin names the frontend can offer.
#[tauri::command]
pub fn plugin_builtins() -> Vec<String> {
    route_plugins::BUILTIN_PLUGINS
        .iter()
        .map(|s| s.to_string())
        .collect()
}
