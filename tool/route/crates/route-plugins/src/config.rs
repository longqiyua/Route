//! Plugin configuration file (`<project>/.route/plugins.json`).
//!
//! Shared by route-cli and route-tauri so both front-ends read and write
//! the same format. The CLI is the authoritative editor; the Tauri GUI
//! reuses the same types for its IPC layer.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};

use crate::bus::EventBus;
use crate::plugins::builtin::{LoggerPlugin, WebhookConfig, WebhookPlugin};

/// One entry in the plugins config file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginEntry {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Free-form JSON config (shape depends on the plugin).
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_true() -> bool {
    true
}

/// Top-level plugins config file. Serialised to
/// `<project>/.route/plugins.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PluginsFile {
    #[serde(default)]
    pub plugins: Vec<PluginEntry>,
}

/// Built-in plugin names. `plugin install` only accepts these.
pub const BUILTIN_PLUGINS: &[&str] = &["logger", "webhook"];

/// Path of the plugins config file: `<route_dir>/plugins.json`.
pub fn plugins_config_path(route_dir: &std::path::Path) -> PathBuf {
    route_dir.join("plugins.json")
}

/// Load the plugins config. Returns an empty config if the file does not
/// exist yet.
pub fn load_config(route_dir: &std::path::Path) -> Result<PluginsFile> {
    let path = plugins_config_path(route_dir);
    if !path.exists() {
        return Ok(PluginsFile::default());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read plugins config: {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse plugins config: {}", path.display()))
}

/// Save the plugins config.
pub fn save_config(route_dir: &std::path::Path, cfg: &PluginsFile) -> Result<()> {
    let path = plugins_config_path(route_dir);
    let content = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write plugins config: {}", path.display()))
}

/// Validate that a plugin name is a known built-in.
pub fn validate_builtin(name: &str) -> Result<()> {
    if BUILTIN_PLUGINS.contains(&name) {
        Ok(())
    } else {
        bail!(
            "Unknown plugin '{}'. Built-in plugins: {}",
            name,
            BUILTIN_PLUGINS.join(", ")
        )
    }
}

/// Validate a plugin's config at install time. Currently only the webhook
/// plugin has structural requirements (non-empty `url`).
pub fn validate_config(name: &str, config: &serde_json::Value) -> Result<()> {
    if name == "webhook" {
        if config.is_null() {
            bail!("webhook plugin requires config: {{\"url\": \"...\"}}");
        }
        let wh: WebhookConfig =
            serde_json::from_value(config.clone()).context("Invalid webhook config")?;
        if wh.url.is_empty() {
            bail!("webhook plugin requires a non-empty 'url'");
        }
    }
    Ok(())
}

/// Build an `EventBus` from the config file. Disabled plugins are kept
/// in the config but not subscribed. Returns `Ok(None)` if the config is
/// empty or missing — callers should treat that as "no bus, no-op".
pub fn build_bus_from_config(route_dir: &std::path::Path) -> Result<Option<Arc<EventBus>>> {
    let cfg = load_config(route_dir)?;
    if cfg.plugins.is_empty() {
        return Ok(None);
    }
    let bus = Arc::new(EventBus::new());
    for entry in &cfg.plugins {
        if !entry.enabled {
            continue;
        }
        match entry.name.as_str() {
            "logger" => {
                bus.subscribe(Box::new(LoggerPlugin::new()));
            }
            "webhook" => {
                if entry.config.is_null() {
                    tracing::warn!(plugin = "webhook", "missing config, skipping");
                    continue;
                }
                match serde_json::from_value::<WebhookConfig>(entry.config.clone()) {
                    Ok(wh) => {
                        if wh.url.is_empty() {
                            tracing::warn!(plugin = "webhook", "empty url, skipping");
                            continue;
                        }
                        bus.subscribe(Box::new(WebhookPlugin::new(wh)));
                    }
                    Err(e) => {
                        tracing::warn!(plugin = "webhook", error = %e, "invalid config, skipping");
                    }
                }
            }
            other => {
                tracing::warn!(plugin = %other, "unknown plugin in config, skipping");
            }
        }
    }
    Ok(Some(bus))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_default_when_missing() {
        let dir = tempdir().unwrap();
        let cfg = load_config(dir.path()).unwrap();
        assert!(cfg.plugins.is_empty());
    }

    #[test]
    fn round_trip_save_load() {
        let dir = tempdir().unwrap();
        let cfg = PluginsFile {
            plugins: vec![PluginEntry {
                name: "logger".into(),
                enabled: true,
                config: serde_json::Value::Null,
            }],
        };
        save_config(dir.path(), &cfg).unwrap();
        let loaded = load_config(dir.path()).unwrap();
        assert_eq!(loaded.plugins.len(), 1);
        assert_eq!(loaded.plugins[0].name, "logger");
    }

    #[test]
    fn validate_builtin_rejects_unknown() {
        assert!(validate_builtin("logger").is_ok());
        assert!(validate_builtin("webhook").is_ok());
        assert!(validate_builtin("bogus").is_err());
    }

    #[test]
    fn validate_config_webhook_requires_url() {
        assert!(validate_config("webhook", &serde_json::Value::Null).is_err());
        assert!(validate_config("webhook", &serde_json::json!({"url": ""})).is_err());
        assert!(validate_config("webhook", &serde_json::json!({"url": "https://x"})).is_ok());
        assert!(validate_config("logger", &serde_json::Value::Null).is_ok());
    }

    #[test]
    fn build_bus_none_when_empty() {
        let dir = tempdir().unwrap();
        assert!(build_bus_from_config(dir.path()).unwrap().is_none());
    }

    #[test]
    fn build_bus_subscribes_enabled_only() {
        let dir = tempdir().unwrap();
        let cfg = PluginsFile {
            plugins: vec![
                PluginEntry {
                    name: "logger".into(),
                    enabled: true,
                    config: serde_json::Value::Null,
                },
                PluginEntry {
                    name: "webhook".into(),
                    enabled: false,
                    config: serde_json::json!({"url": "https://x"}),
                },
            ],
        };
        save_config(dir.path(), &cfg).unwrap();
        let bus = build_bus_from_config(dir.path()).unwrap().unwrap();
        assert_eq!(bus.plugin_names(), vec!["logger"]);
    }
}
