//! Route Tauri backend — exposes BasicRepository via IPC commands.

mod state;
mod commands;
pub mod example_seed;
mod plugin_commands;
mod sync_commands;
mod stats_commands;
mod watcher;
mod autostart_commands;
mod git_commands;
mod mcp_commands;
mod ai_commands;

use autostart_commands::AutostartConfig;
use state::AppState;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, WindowEvent,
};

/// Apply the configured process priority to the current process.
/// Best-effort: on Windows we call `SetPriorityClass` via FFI so we
/// don't need an extra `windows`/`winapi` dependency just for this
/// one call. On other platforms this is a no-op (priority is stored
/// in the config but not enforced).
fn apply_process_priority(priority: &str) {
    #[cfg(windows)]
    {
        use std::os::raw::{c_int, c_void};
        extern "system" {
            fn GetCurrentProcess() -> *mut c_void;
            fn SetPriorityClass(h_process: *mut c_void, dw_priority_class: u32) -> c_int;
        }
        // Windows priority class constants.
        const BELOW_NORMAL: u32 = 0x00004000;
        const NORMAL: u32 = 0x00000020;
        const ABOVE_NORMAL: u32 = 0x00008000;
        let class = match priority {
            "low" => BELOW_NORMAL,
            "high" => ABOVE_NORMAL,
            _ => NORMAL,
        };
        unsafe {
            let h = GetCurrentProcess();
            SetPriorityClass(h, class);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = priority;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // `--silent` is appended to the boot-launch args by the autostart
    // config when the user enables "静默启动". We capture it here so the
    // setup hook can hide the window on boot.
    let silent = std::env::args().any(|a| a == "--silent");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(move |app| {
            // Apply the configured startup priority to this process.
            // We load the config the same way `autostart_get` does so the
            // priority the user picked in Settings actually takes effect.
            if let Ok(cfg_raw) = std::fs::read_to_string(
                app.path()
                    .app_config_dir()
                    .unwrap_or_default()
                    .join("autostart.json"),
            ) {
                if let Ok(cfg) = serde_json::from_str::<AutostartConfig>(&cfg_raw) {
                    apply_process_priority(&cfg.priority);
                }
            }

            // Build the system-tray menu. Two items: show the window,
            // and quit. The tray is what lets the user bring Route back
            // to the front after a silent (hidden) boot launch.
            let show_item = MenuItem::with_id(app, "show", "显示 Route", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            let mut tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("Route")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    // Left-click toggles the window: show if hidden, hide
                    // if visible. Right-click opens the menu (default).
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            match window.is_visible() {
                                Ok(true) => {
                                    let _ = window.hide();
                                }
                                _ => {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // Handle tray menu clicks.
            let app_handle = app.handle().clone();
            app.on_menu_event(move |_app, event| match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app_handle.exit(0);
                }
                _ => {}
            });

            // Hide the window on silent boot. The tray icon is the way
            // back in. We also keep `autostart_was_silent` so the
            // frontend can query this if it needs to.
            if silent {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Check the close-behavior config each time. "hide" means
            // hide to tray (process keeps running); "quit" means exit.
            if let WindowEvent::CloseRequested { api, .. } = event {
                let behavior = autostart_commands::read_close_behavior(window.app_handle());
                if behavior == "hide" {
                    let _ = window.hide();
                    api.prevent_close();
                }
                // "quit" → do nothing, the app exits normally.
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::pick_folder,
            commands::default_seed_dir,
            commands::open_repo,
            commands::init_repo,
            commands::seed_example,
            commands::create_blank_demo,
            commands::status,
            commands::commit,
            commands::log,
            commands::rollback,
            commands::checkpoint_create,
            commands::undo_last,
            commands::redo_last,
            commands::can_redo,
            commands::watch_start,
            commands::watch_stop,
            commands::watch_flush,
            commands::watch_status,
            commands::backup_to_dir,
            commands::branch_list,
            commands::branch_create,
            commands::branch_delete,
            commands::branch_switch,
            commands::annotate,
            commands::list_annotations,
            commands::export_data,
            commands::export_file,
            commands::stats,
            commands::history_tree,
            commands::file_history,
            commands::restore_file,
            commands::commit_diff_detail,
            commands::working_dir_status,
            commands::diff_snapshots,
            commands::tag_list,
            commands::tag_create,
            commands::tag_delete,
            commands::set_ai_operator,
            commands::clear_ai_operator,
            commands::get_ai_operator,
            commands::track_get,
            commands::track_set,
            commands::track_set_all,
            commands::track_set_verify_sha256,
            commands::track_set_memory_buffer_ms,
            commands::track_set_on,
            commands::ai_summary_prompt,
            commands::route_index_refresh,
            commands::route_index_path,
            commands::ai_conflict_report,
            commands::ai_conflict_resolve,
            commands::ai_conflict_list,
            sync_commands::sync_list,
            sync_commands::sync_show,
            sync_commands::sync_add,
            sync_commands::sync_remove,
            sync_commands::sync_set_enabled,
            sync_commands::sync_run,
            sync_commands::sync_start,
            sync_commands::sync_stop,
            sync_commands::sync_status,
            plugin_commands::plugin_list,
            plugin_commands::plugin_show,
            plugin_commands::plugin_install,
            plugin_commands::plugin_remove,
            plugin_commands::plugin_set_enabled,
            plugin_commands::plugin_builtins,
            stats_commands::stats_collect,
            stats_commands::stats_markdown,
            stats_commands::stats_json,
            autostart_commands::autostart_get,
            autostart_commands::autostart_set,
            autostart_commands::autostart_was_silent,
            git_commands::git_detect,
            git_commands::git_init,
            git_commands::git_commit,
            git_commands::git_rollback,
            git_commands::git_log,
            git_commands::git_branch_list,
            git_commands::git_branch_create,
            git_commands::git_branch_switch,
            git_commands::git_current_branch,
            git_commands::git_merge,
            git_commands::git_status,
            git_commands::git_diff,
            git_commands::git_stash_list,
            git_commands::git_stash_push,
            git_commands::git_stash_pop,
            git_commands::git_tag_list,
            git_commands::git_tag_create,
            git_commands::git_tag_delete,
            git_commands::git_restore,
            git_commands::git_mode_set,
            commands::branch_merge,
            mcp_commands::mcp_get_config,
            ai_commands::ai_chat,
            show_window,
            hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Show the main window and give it focus. Used by the frontend (e.g.
/// a "reveal" button) and conceptually paired with the tray "显示" item.
#[tauri::command]
fn show_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Hide the main window. The process keeps running in the background;
/// the user brings it back via the tray icon or `show_window`.
#[tauri::command]
fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
