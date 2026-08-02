// Hide the console window on Windows. The app uses Tauri's webview for
// all UI; there is no reason to show a terminal window to the end user.
// Dev logs are still captured by `cargo tauri dev`'s stdout pipe.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    route_tauri::run();
}
