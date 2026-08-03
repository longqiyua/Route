//! Tauri IPC commands for `route sync ...` — directory synchronization.

use std::path::PathBuf;

use route_core::RoutePaths;
use route_sync::{
    ArchiveRule, ConflictResolution, RemoteCredentials, Scheduler, SyncConfig, SyncEngine,
    SyncMode, SyncResult, SyncTarget, TransportType,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// DTO for remote credentials. Secrets (password, secret_key) are returned as
/// `*_set` boolean flags only — the actual values never leave the backend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialsDto {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password_set: bool,
    #[serde(default)]
    pub access_key: Option<String>,
    #[serde(default)]
    pub secret_key_set: bool,
    #[serde(default)]
    pub bucket: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTargetDto {
    pub name: String,
    pub source: String,
    pub destination: String,
    pub mode: String,
    pub transport: String,
    pub conflict: String,
    pub archive_rule: ArchiveRuleDto,
    pub ignore_patterns: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub credentials: Option<CredentialsDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRuleDto {
    pub folder_format: String,
    pub max_snapshots: Option<usize>,
    pub max_age_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncResultDto {
    pub target_name: String,
    pub mode: String,
    pub stats: SyncStatsDto,
    pub timestamp: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatsDto {
    pub files_scanned: usize,
    pub files_copied: usize,
    pub files_skipped: usize,
    pub files_deleted: usize,
    pub conflicts_resolved: usize,
    pub errors: usize,
    pub bytes_copied: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncStatusDto {
    pub scheduler_running: bool,
    pub targets_count: usize,
    pub enabled_count: usize,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn route_paths(state: &AppState) -> Result<RoutePaths, String> {
    let path = state.project_path.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let path = path.ok_or("No repository open")?;
    let paths = RoutePaths::new(&path);
    if !paths.is_initialized() {
        return Err("Not a Route basic repository".to_string());
    }
    Ok(paths)
}

fn sync_config_path(paths: &RoutePaths) -> PathBuf {
    paths.route_dir.join("sync.json")
}

fn load_config(paths: &RoutePaths) -> Result<SyncConfig, String> {
    let path = sync_config_path(paths);
    if !path.exists() {
        return Ok(SyncConfig::default());
    }
    SyncConfig::load(&path).map_err(|e| format!("Failed to load sync config: {e}"))
}

fn save_config(paths: &RoutePaths, cfg: &SyncConfig) -> Result<(), String> {
    let path = sync_config_path(paths);
    cfg.save(&path).map_err(|e| format!("Failed to save sync config: {e}"))
}

fn target_to_dto(t: &SyncTarget) -> SyncTargetDto {
    let credentials = t.credentials.as_ref().map(|c| CredentialsDto {
        url: c.url.clone(),
        username: c.username.clone(),
        password_set: c.password.is_some(),
        access_key: c.access_key.clone(),
        secret_key_set: c.secret_key.is_some(),
        bucket: c.bucket.clone(),
        region: c.region.clone(),
    });
    SyncTargetDto {
        name: t.name.clone(),
        source: t.source.to_string_lossy().to_string(),
        destination: t.destination.to_string_lossy().to_string(),
        mode: t.mode.as_str().to_string(),
        transport: format!("{:?}", t.transport).to_lowercase(),
        conflict: t.conflict.as_str().to_string(),
        archive_rule: ArchiveRuleDto {
            folder_format: t.archive_rule.folder_format.clone(),
            max_snapshots: t.archive_rule.max_snapshots,
            max_age_days: t.archive_rule.max_age_days,
        },
        ignore_patterns: t.ignore_patterns.clone(),
        enabled: t.enabled,
        credentials,
    }
}

fn result_to_dto(r: &SyncResult) -> SyncResultDto {
    SyncResultDto {
        target_name: r.target_name.clone(),
        mode: r.mode.clone(),
        stats: SyncStatsDto {
            files_scanned: r.stats.files_scanned,
            files_copied: r.stats.files_copied,
            files_skipped: r.stats.files_skipped,
            files_deleted: r.stats.files_deleted,
            conflicts_resolved: r.stats.conflicts_resolved,
            errors: r.stats.errors,
            bytes_copied: r.stats.bytes_copied,
        },
        timestamp: r.timestamp,
        error: r.error.clone(),
    }
}

fn parse_mode(s: &str) -> Result<SyncMode, String> {
    SyncMode::from_str(s).ok_or_else(|| format!("Unknown mode: {s} (use mirror|backup|archive)"))
}

fn parse_transport(s: &str) -> Result<TransportType, String> {
    match s {
        "local" => Ok(TransportType::Local),
        "relay" => Ok(TransportType::Relay),
        "server" => Ok(TransportType::Server),
        "p2p" => Ok(TransportType::P2P),
        "webdav" => Ok(TransportType::Webdav),
        "s3" => Ok(TransportType::S3),
        "ssh" | "scp" => Ok(TransportType::Ssh),
        other => Err(format!(
            "Unknown transport: {other} (use local|relay|server|p2p|webdav|s3|ssh)"
        )),
    }
}

fn parse_conflict(s: &str) -> Result<ConflictResolution, String> {
    match s {
        "keep_both" => Ok(ConflictResolution::KeepBoth),
        "skip" => Ok(ConflictResolution::SkipExisting),
        "overwrite" => Ok(ConflictResolution::Overwrite),
        other => Err(format!("Unknown conflict: {other} (use keep_both|skip|overwrite)")),
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn sync_list(state: State<'_, AppState>) -> Result<Vec<SyncTargetDto>, String> {
    let paths = route_paths(&state)?;
    let cfg = load_config(&paths)?;
    Ok(cfg.targets.iter().map(target_to_dto).collect())
}

#[tauri::command]
pub fn sync_show(name: String, state: State<'_, AppState>) -> Result<SyncTargetDto, String> {
    let paths = route_paths(&state)?;
    let cfg = load_config(&paths)?;
    let t = cfg
        .find(&name)
        .ok_or_else(|| format!("Sync target '{name}' not found"))?;
    Ok(target_to_dto(t))
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn sync_add(
    name: String,
    source: String,
    destination: String,
    mode: String,
    transport: Option<String>,
    conflict: Option<String>,
    max_snapshots: Option<usize>,
    max_age_days: Option<u32>,
    folder_format: Option<String>,
    ignore_patterns: Option<Vec<String>>,
    enabled: Option<bool>,
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
    access_key: Option<String>,
    secret_key: Option<String>,
    bucket: Option<String>,
    region: Option<String>,
    state: State<'_, AppState>,
) -> Result<SyncTargetDto, String> {
    let paths = route_paths(&state)?;
    let mut cfg = load_config(&paths)?;

    if cfg.find(&name).is_some() {
        return Err(format!("Sync target '{name}' already exists"));
    }

    let transport = parse_transport(transport.as_deref().unwrap_or("local"))?;

    // Build remote credentials based on transport kind. Only fields relevant
    // to the chosen transport are populated.
    let credentials: Option<RemoteCredentials> = match transport {
        TransportType::Webdav => {
            let url_val = url.ok_or_else(|| {
                "url is required for webdav transport".to_string()
            })?;
            Some(RemoteCredentials {
                url: url_val,
                username,
                password,
                ..Default::default()
            })
        }
        TransportType::S3 => {
            let bucket_val = bucket.ok_or_else(|| {
                "bucket is required for s3 transport".to_string()
            })?;
            let access_key_val = access_key.ok_or_else(|| {
                "access_key is required for s3 transport".to_string()
            })?;
            let secret_key_val = secret_key.ok_or_else(|| {
                "secret_key is required for s3 transport".to_string()
            })?;
            Some(RemoteCredentials {
                url: url.unwrap_or_default(),
                access_key: Some(access_key_val),
                secret_key: Some(secret_key_val),
                bucket: Some(bucket_val),
                region,
                ..Default::default()
            })
        }
        TransportType::Ssh => {
            // SSH uses ssh-agent / ~/.ssh/config. The `url` is the
            // `ssh://[user@]host[:port]/path` endpoint. `username` is
            // optional — it overrides the user in the URL when set.
            let url_val = url.ok_or_else(|| {
                "url is required for ssh transport (e.g. ssh://user@host:22/var/backups)".to_string()
            })?;
            Some(RemoteCredentials {
                url: url_val,
                username,
                ..Default::default()
            })
        }
        _ => None,
    };

    let target = SyncTarget {
        name: name.clone(),
        source: PathBuf::from(&source),
        destination: PathBuf::from(&destination),
        mode: parse_mode(&mode)?,
        transport,
        conflict: parse_conflict(conflict.as_deref().unwrap_or("keep_both"))?,
        archive_rule: ArchiveRule {
            folder_format: folder_format.unwrap_or_else(|| "%Y%m%d_%H%M%S".to_string()),
            max_snapshots,
            max_age_days,
        },
        ignore_patterns: ignore_patterns.unwrap_or_default(),
        enabled: enabled.unwrap_or(true),
        credentials,
    };

    let dto = target_to_dto(&target);
    cfg.upsert(target);
    save_config(&paths, &cfg)?;
    Ok(dto)
}

#[tauri::command]
pub fn sync_remove(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let paths = route_paths(&state)?;
    let mut cfg = load_config(&paths)?;
    if !cfg.remove(&name) {
        return Err(format!("Sync target '{name}' not found"));
    }
    save_config(&paths, &cfg)
}

#[tauri::command]
pub fn sync_set_enabled(
    name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let paths = route_paths(&state)?;
    let mut cfg = load_config(&paths)?;
    let t = cfg
        .find_mut(&name)
        .ok_or_else(|| format!("Sync target '{name}' not found"))?;
    t.enabled = enabled;
    save_config(&paths, &cfg)
}

#[tauri::command]
pub fn sync_run(
    name: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<SyncResultDto>, String> {
    let paths = route_paths(&state)?;
    let cfg = load_config(&paths)?;

    // Build a sync engine, optionally with the project's event bus attached
    // so plugin dispatch (logger/webhook/...) fires for SyncStarted/Completed.
    let engine = SyncEngine::new();
    if let Ok(Some(bus)) = route_plugins::build_bus_from_config(&paths.route_dir) {
        if let Err(e) = engine.set_event_bus(bus) {
            tracing::warn!(error = %e, "could not attach event bus to sync engine");
        }
    }

    let results: Vec<SyncResult> = match &name {
        Some(n) => {
            let t = cfg
                .find(n)
                .ok_or_else(|| format!("Sync target '{n}' not found"))?;
            vec![engine.run(t)]
        }
        None => cfg
            .targets
            .iter()
            .filter(|t| t.enabled)
            .map(|t| engine.run(t))
            .collect(),
    };

    Ok(results.iter().map(result_to_dto).collect())
}

#[tauri::command]
pub fn sync_start(
    interval_secs: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let paths = route_paths(&state)?;
    let cfg = load_config(&paths)?;

    if cfg.targets.iter().filter(|t| t.enabled).count() == 0 {
        return Err("No enabled sync targets to schedule".to_string());
    }

    let mut guard = state.sync_scheduler.lock().unwrap_or_else(|p| p.into_inner());
    // If a scheduler is already running, stop it first
    if let Some(mut existing) = guard.take() {
        existing.stop_all();
    }
    let scheduler = Scheduler::start_all(&cfg, interval_secs);
    *guard = Some(scheduler);
    Ok(())
}

#[tauri::command]
pub fn sync_stop(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.sync_scheduler.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(mut scheduler) = guard.take() {
        scheduler.stop_all();
    }
    Ok(())
}

#[tauri::command]
pub fn sync_status(state: State<'_, AppState>) -> Result<SyncStatusDto, String> {
    let paths = route_paths(&state)?;
    let cfg = load_config(&paths)?;
    let scheduler_running = state.sync_scheduler.lock().unwrap_or_else(|p| p.into_inner()).is_some();
    let enabled_count = cfg.targets.iter().filter(|t| t.enabled).count();
    Ok(SyncStatusDto {
        scheduler_running,
        targets_count: cfg.targets.len(),
        enabled_count,
    })
}
