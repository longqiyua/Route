use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

// The desktop app is a windowed binary (`windows_subsystem = "windows"`),
// so it has no console. Spawning the `node` bridge without this flag would
// flash a black console window on every cold start of the bridge.
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct BridgeState {
    child: Mutex<Option<Child>>,
    script_path: PathBuf,
}

fn resolve_bridge_script() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("desktop-bridge")
        .join("dist")
        .join("index.js")
}

fn ensure_bridge(state: &BridgeState) -> Result<(), String> {
    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    if guard.is_some() {
        return Ok(());
    }

    if !state.script_path.exists() {
        return Err(format!(
            "Bridge script not found: {}. Run `npm run build` first.",
            state.script_path.display()
        ));
    }

    let mut cmd = Command::new("node");
    cmd.arg(&state.script_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start bridge: {e}"))?;

    *guard = Some(child);
    Ok(())
}

#[tauri::command]
fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let folder = app.dialog().file().blocking_pick_folder();
    Ok(folder.map(|p| p.to_string()))
}

#[tauri::command]
fn route_call(
    state: State<'_, BridgeState>,
    project_path: String,
    method: String,
    params: Value,
) -> Result<Value, String> {
    ensure_bridge(&state)?;

    let request_id = Uuid::new_v4().to_string();
    let mut merged = params.as_object().cloned().unwrap_or_default();
    merged.insert("projectPath".to_string(), json!(project_path));

    let request = json!({
        "id": request_id,
        "method": method,
        "params": merged,
    });

    let mut guard = state.child.lock().map_err(|e| e.to_string())?;
    let child = guard.as_mut().ok_or("Bridge not running")?;

    let stdin = child.stdin.as_mut().ok_or("Bridge stdin unavailable")?;
    let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
    writeln!(stdin, "{line}").map_err(|e| e.to_string())?;
    stdin.flush().map_err(|e| e.to_string())?;

    let stdout = child.stdout.as_mut().ok_or("Bridge stdout unavailable")?;
    let mut reader = BufReader::new(stdout);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .map_err(|e| e.to_string())?;

    let response: Value = serde_json::from_str(&response_line).map_err(|e| e.to_string())?;
    if response.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        Ok(response.get("data").cloned().unwrap_or(json!(null)))
    } else {
        Err(response
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown bridge error")
            .to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let script_path = resolve_bridge_script();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(BridgeState {
            child: Mutex::new(None),
            script_path,
        })
        .invoke_handler(tauri::generate_handler![pick_folder, route_call])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
