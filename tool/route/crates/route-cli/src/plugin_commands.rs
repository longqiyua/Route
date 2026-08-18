//! CLI command implementations for `route plugin ...` — plugin management.
//!
//! Thin wrapper around `route_plugins::config` which holds the shared
//! types and the `build_bus_from_config` helper. The CLI adds user-facing
//! argument parsing and pretty-printed output.

use std::sync::Arc;

use anyhow::{bail, Context, Result};
use route_core::RoutePaths;
use route_plugins::{
    build_bus_from_config, load_plugin_config, plugins_config_path, save_plugin_config,
    validate_plugin_builtin, validate_plugin_config, PluginEntry,
};

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn cwd_paths() -> Result<RoutePaths> {
    let cwd = std::env::current_dir()?;
    let paths = RoutePaths::new(&cwd);
    if !paths.is_initialized() {
        return Err(anyhow::anyhow!(
            "Not in a Route basic repository. Run `route init` first. (expected at {})",
            paths.route_dir.display()
        ));
    }
    Ok(paths)
}

/// Re-export of the shared bus-builder so `commands.rs` and `sync_commands.rs`
/// can attach a bus to the repository / sync engine with one call.
pub fn build_bus(paths: &RoutePaths) -> Result<Option<Arc<route_plugins::EventBus>>> {
    build_bus_from_config(&paths.route_dir)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// `route plugin list` — list configured plugins.
pub fn plugin_list() -> Result<()> {
    let paths = cwd_paths()?;
    let cfg = load_plugin_config(&paths.route_dir)?;
    if cfg.plugins.is_empty() {
        println!("No plugins configured. Use `route plugin install <name>` to add one.");
        println!();
        println!(
            "Built-in plugins: {}",
            route_plugins::BUILTIN_PLUGINS.join(", ")
        );
        return Ok(());
    }
    println!("{:<12} {:<8} {}", "NAME", "ENABLED", "CONFIG");
    for entry in &cfg.plugins {
        let enabled = if entry.enabled { "yes" } else { "no" };
        let cfg_summary = if entry.config.is_null() {
            "(none)".to_string()
        } else {
            serde_json::to_string(&entry.config).unwrap_or_else(|_| "(invalid)".into())
        };
        println!("{:<12} {:<8} {}", entry.name, enabled, cfg_summary);
    }
    Ok(())
}

/// `route plugin show <name>` — show one plugin's config.
pub fn plugin_show(name: String) -> Result<()> {
    let paths = cwd_paths()?;
    let cfg = load_plugin_config(&paths.route_dir)?;
    let entry = cfg
        .plugins
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{}' is not installed", name))?;
    println!("Name:    {}", entry.name);
    println!("Enabled: {}", if entry.enabled { "yes" } else { "no" });
    println!("Config:");
    if entry.config.is_null() {
        println!("  (none)");
    } else {
        let pretty = serde_json::to_string_pretty(&entry.config)
            .unwrap_or_else(|_| format!("{:?}", entry.config));
        for line in pretty.lines() {
            println!("  {}", line);
        }
    }
    Ok(())
}

/// `route plugin install <name> [config-json]` — add a plugin to the config.
pub fn plugin_install(name: String, config_json: Option<String>) -> Result<()> {
    validate_plugin_builtin(&name)?;
    let paths = cwd_paths()?;
    let mut cfg = load_plugin_config(&paths.route_dir)?;
    if cfg.plugins.iter().any(|p| p.name == name) {
        bail!("Plugin '{}' is already installed. Remove it first.", name);
    }
    let config: serde_json::Value = match config_json.as_deref() {
        None => serde_json::Value::Null,
        Some(s) => serde_json::from_str(s).context("Invalid JSON config")?,
    };
    validate_plugin_config(&name, &config)?;
    cfg.plugins.push(PluginEntry {
        name: name.clone(),
        enabled: true,
        config,
    });
    save_plugin_config(&paths.route_dir, &cfg)?;
    println!("✓ Installed plugin '{}'", name);
    Ok(())
}

/// `route plugin remove <name>` — remove a plugin from the config.
pub fn plugin_remove(name: String) -> Result<()> {
    let paths = cwd_paths()?;
    let mut cfg = load_plugin_config(&paths.route_dir)?;
    let before = cfg.plugins.len();
    cfg.plugins.retain(|p| p.name != name);
    if cfg.plugins.len() == before {
        bail!("Plugin '{}' is not installed", name);
    }
    save_plugin_config(&paths.route_dir, &cfg)?;
    println!("✓ Removed plugin '{}'", name);
    Ok(())
}

/// `route plugin enable <name>` / `route plugin disable <name>`.
pub fn plugin_set_enabled(name: String, enabled: bool) -> Result<()> {
    let paths = cwd_paths()?;
    let mut cfg = load_plugin_config(&paths.route_dir)?;
    let entry = cfg
        .plugins
        .iter_mut()
        .find(|p| p.name == name)
        .ok_or_else(|| anyhow::anyhow!("Plugin '{}' is not installed", name))?;
    entry.enabled = enabled;
    save_plugin_config(&paths.route_dir, &cfg)?;
    println!(
        "✓ Plugin '{}' {}",
        name,
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

// Re-export for tests that need the path helper.
#[allow(dead_code)]
fn _config_path(paths: &RoutePaths) -> std::path::PathBuf {
    plugins_config_path(&paths.route_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_core::RoutePaths;
    use route_plugins::PluginsFile;
    use tempfile::tempdir;

    fn setup_paths() -> (tempfile::TempDir, RoutePaths) {
        let dir = tempdir().unwrap();
        let paths = RoutePaths::new(dir.path());
        paths.ensure_dirs().unwrap();
        (dir, paths)
    }

    #[test]
    fn install_then_list_then_remove_round_trip() {
        let (_tmp, paths) = setup_paths();
        // Direct config manipulation via shared route_plugins API.
        let mut cfg = PluginsFile::default();
        cfg.plugins.push(PluginEntry {
            name: "logger".into(),
            enabled: true,
            config: serde_json::Value::Null,
        });
        save_plugin_config(&paths.route_dir, &cfg).unwrap();

        let cfg = load_plugin_config(&paths.route_dir).unwrap();
        assert_eq!(cfg.plugins.len(), 1);
        assert_eq!(cfg.plugins[0].name, "logger");
        assert!(cfg.plugins[0].enabled);

        // disable
        let mut cfg = load_plugin_config(&paths.route_dir).unwrap();
        cfg.plugins[0].enabled = false;
        save_plugin_config(&paths.route_dir, &cfg).unwrap();
        let cfg = load_plugin_config(&paths.route_dir).unwrap();
        assert!(!cfg.plugins[0].enabled);

        // remove
        let mut cfg = load_plugin_config(&paths.route_dir).unwrap();
        cfg.plugins.clear();
        save_plugin_config(&paths.route_dir, &cfg).unwrap();
        let cfg = load_plugin_config(&paths.route_dir).unwrap();
        assert!(cfg.plugins.is_empty());
    }

    #[test]
    fn build_bus_subscribes_enabled_only() {
        let (_tmp, paths) = setup_paths();
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
                    config: serde_json::json!({"url": "https://x.com/h"}),
                },
            ],
        };
        save_plugin_config(&paths.route_dir, &cfg).unwrap();
        let bus = build_bus(&paths).unwrap().unwrap();
        assert_eq!(bus.plugin_names().len(), 1);
        assert_eq!(bus.plugin_names()[0], "logger");
    }

    #[test]
    fn build_bus_none_when_empty() {
        let (_tmp, paths) = setup_paths();
        assert!(build_bus(&paths).unwrap().is_none());
    }
}
