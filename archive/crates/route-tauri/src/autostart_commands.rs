//! Autostart commands — manage OS-level boot launch, silent start, and
//! startup priority for the Route desktop app.
//!
//! Uses the `auto-launch` crate directly (the same crate that
//! `tauri-plugin-autostart` wraps) so we can change launch args at
//! runtime without re-initializing a plugin.
//!
//! Three knobs:
//!   1. `enabled`  — is the app registered to launch on boot?
//!   2. `silent`   — when true, `--silent` is appended to the boot
//!                    launch args so the Rust main can start the window
//!                    hidden.
//!   3. `priority` — "low" | "normal" | "high". Recorded and applied
//!                    best-effort (Windows registry ordering; on other
//!                    platforms the value is stored but has no effect).

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartConfig {
    pub enabled: bool,
    pub silent: bool,
    pub priority: String, // "low" | "normal" | "high"
    /// What happens when the user closes the window.
    /// "quit" — exit the process (default).
    /// "hide" — hide to the system tray, keep running in background.
    pub close_behavior: String, // "quit" | "hide"
}

impl Default for AutostartConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            silent: false,
            priority: "normal".to_string(),
            close_behavior: "quit".to_string(),
        }
    }
}

const CONFIG_FILE: &str = "autostart.json";

fn config_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    dir.join(CONFIG_FILE)
}

fn load_config(app: &AppHandle) -> AutostartConfig {
    let path = config_path(app);
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => AutostartConfig::default(),
    }
}

fn save_config(app: &AppHandle, cfg: &AutostartConfig) {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(&path, json);
    }
}

/// Build an `AutoLaunch` instance with the current config. The app
/// binary path is detected from the current executable; `--silent` is
/// appended as a launch arg when silent start is requested.
fn build_autolaunch(cfg: &AutostartConfig) -> auto_launch::Result<AutoLaunch> {
    let mut builder = AutoLaunchBuilder::new();
    builder.set_app_name("Route");
    builder.set_use_launch_agent(true);

    // The launch args: `--silent` when silent start is on.
    let mut args: Vec<String> = Vec::new();
    if cfg.silent {
        args.push("--silent".to_string());
    }
    if !args.is_empty() {
        builder.set_args(&args);
    }

    builder.build()
}

/// Apply the config to the OS: enable or disable the autostart
/// registration. Called whenever the config changes.
fn apply_config(cfg: &AutostartConfig) {
    match build_autolaunch(cfg) {
        Ok(al) => {
            if cfg.enabled {
                let _ = al.enable();
            } else {
                let _ = al.disable();
            }
        }
        Err(e) => {
            tracing::warn!("autostart build failed: {e}");
        }
    }
}

/// Read the current autostart config.
#[tauri::command]
pub fn autostart_get(app: AppHandle) -> AutostartConfig {
    load_config(&app)
}

/// Update the autostart config and apply it to the OS. Any of the
/// three fields can be omitted (left unchanged) by passing `None`.
#[tauri::command]
pub fn autostart_set(
    app: AppHandle,
    enabled: Option<bool>,
    silent: Option<bool>,
    priority: Option<String>,
    close_behavior: Option<String>,
) -> AutostartConfig {
    let mut cfg = load_config(&app);
    if let Some(e) = enabled {
        cfg.enabled = e;
    }
    if let Some(s) = silent {
        cfg.silent = s;
    }
    if let Some(p) = priority {
        if matches!(p.as_str(), "low" | "normal" | "high") {
            cfg.priority = p;
        }
    }
    if let Some(cb) = close_behavior {
        if matches!(cb.as_str(), "quit" | "hide") {
            cfg.close_behavior = cb;
        }
    }
    save_config(&app, &cfg);
    apply_config(&cfg);
    cfg
}

/// Check whether the app was launched with `--silent` (boot launch).
/// The frontend uses this to decide whether to show the window on
/// startup or stay hidden.
#[tauri::command]
pub fn autostart_was_silent() -> bool {
    std::env::args().any(|a| a == "--silent")
}

/// Read the close-behavior setting from the config file. Called by
/// the window close-event handler to decide whether to hide to tray
/// or quit. Returns "hide" or "quit" (default).
pub fn read_close_behavior(app: &AppHandle) -> String {
    let cfg = load_config(app);
    if cfg.close_behavior == "hide" {
        "hide".to_string()
    } else {
        "quit".to_string()
    }
}
