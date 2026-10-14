//! Host-neutral entry. Identity uses the same local binding as sidecar, read-only.
use anyhow::{ensure, Result};
use serde_json::json;

/// Preserve advanced archive status; metadata-only users get the same entry facade.
pub fn metadata_only_status(machine: bool) -> Result<bool> {
    let root = route_basic::entry::locate(&std::env::current_dir()?)?;
    if route_core::RoutePaths::new(&root).is_initialized()
        || route_basic::load_identity(&root)?.is_none()
    {
        return Ok(false);
    }
    run(machine, false)?;
    Ok(true)
}

pub fn run(machine: bool, independent: bool) -> Result<()> {
    let root = route_basic::entry::locate(&std::env::current_dir()?)?;
    let mut value = route_basic::entry::read(&root, independent)?;
    if value["attached"] == true {
        value["worker"] = crate::sidecar_commands::identity_projection(&root, "local");
    }
    route_basic::entry::bound(&mut value, route_basic::entry::MAX_BYTES - 1)?;
    let encoded = serde_json::to_string(&value)?;
    ensure!(
        encoded.len() + 1 <= route_basic::entry::MAX_BYTES,
        "ENTRY_SIZE_LIMIT"
    );
    if machine {
        println!("{encoded}")
    } else {
        println!(
            "Route V1.0 — {}",
            value["code"].as_str().unwrap_or("UNAVAILABLE")
        );
        if let Some(title) = value["goal"]["title"].as_str() {
            println!("Goal: {title}")
        }
        println!(
            "{}",
            json!({"worker":value["worker"],"next":value["suggested_actions"],"doorway":value["doorway"]})
        );
    }
    Ok(())
}
