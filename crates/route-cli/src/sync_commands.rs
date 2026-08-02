//! CLI command implementations for `route sync ...` — directory synchronization.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use route_core::RoutePaths;
use route_sync::{
    ArchiveRule, ConflictResolution, RemoteCredentials, Scheduler, SyncConfig, SyncEngine,
    SyncMode, SyncResult, SyncTarget, TransportType,
};

// ---------------------------------------------------------------------------
// Config helpers
// ---------------------------------------------------------------------------

fn cwd_paths() -> Result<RoutePaths> {
    let cwd = std::env::current_dir()?;
    let paths = RoutePaths::new(&cwd);
    if !paths.is_initialized() {
        return Err(anyhow!(
            "Not in a Route basic repository. Run `route init` first. (expected at {})",
            paths.route_dir.display()
        ));
    }
    Ok(paths)
}

fn sync_config_path(paths: &RoutePaths) -> PathBuf {
    paths.route_dir.join("sync.json")
}

fn load_config(paths: &RoutePaths) -> Result<SyncConfig> {
    let path = sync_config_path(paths);
    if !path.exists() {
        return Ok(SyncConfig::default());
    }
    SyncConfig::load(&path).with_context(|| format!("Failed to load sync config: {}", path.display()))
}

fn save_config(paths: &RoutePaths, cfg: &SyncConfig) -> Result<()> {
    let path = sync_config_path(paths);
    cfg.save(&path)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// `route sync add ...` — add a new sync target.
#[allow(clippy::too_many_arguments)]
pub fn sync_add(
    name: String,
    source: String,
    dest: String,
    mode: String,
    transport: Option<String>,
    conflict: Option<String>,
    max_snapshots: Option<usize>,
    max_age_days: Option<u32>,
    folder_format: Option<String>,
    ignore: Vec<String>,
    disable: bool,
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
    access_key: Option<String>,
    secret_key: Option<String>,
    bucket: Option<String>,
    region: Option<String>,
) -> Result<()> {
    let paths = cwd_paths()?;
    let mut cfg = load_config(&paths)?;

    if cfg.find(&name).is_some() {
        return Err(anyhow!("Sync target '{}' already exists", name));
    }

    let mode = SyncMode::from_str(&mode)
        .ok_or_else(|| anyhow!("Unknown mode: {} (use mirror|backup|archive)", mode))?;
    let transport = match transport.as_deref() {
        Some("local") | None => TransportType::Local,
        Some("relay") => TransportType::Relay,
        Some("server") => TransportType::Server,
        Some("p2p") => TransportType::P2P,
        Some("webdav") => TransportType::Webdav,
        Some("s3") => TransportType::S3,
        Some(other) => return Err(anyhow!(
            "Unknown transport: {} (use local|relay|server|p2p|webdav|s3)",
            other
        )),
    };
    let conflict = match conflict.as_deref() {
        Some("keep_both") | None => ConflictResolution::KeepBoth,
        Some("skip") => ConflictResolution::SkipExisting,
        Some("overwrite") => ConflictResolution::Overwrite,
        Some(other) => return Err(anyhow!("Unknown conflict: {} (use keep_both|skip|overwrite)", other)),
    };

    // Build remote credentials. Only populate fields relevant to the chosen
    // transport; leave the rest as None so the JSON config stays clean.
    let credentials: Option<RemoteCredentials> = match transport {
        TransportType::Webdav => {
            let url_val = url.clone().ok_or_else(|| {
                anyhow!("--url is required for webdav transport (or set ROUTE_REMOTE_URL)")
            })?;
            Some(RemoteCredentials {
                url: url_val,
                username,
                password,
                ..Default::default()
            })
        }
        TransportType::S3 => {
            let bucket_val = bucket.clone().ok_or_else(|| {
                anyhow!("--bucket is required for s3 transport (or set ROUTE_REMOTE_BUCKET)")
            })?;
            let access_key_val = access_key.clone().ok_or_else(|| {
                anyhow!("--access-key is required for s3 transport (or set ROUTE_REMOTE_ACCESS_KEY)")
            })?;
            let secret_key_val = secret_key.clone().ok_or_else(|| {
                anyhow!("--secret-key is required for s3 transport (or set ROUTE_REMOTE_SECRET_KEY)")
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
        _ => None,
    };

    let archive_rule = ArchiveRule {
        folder_format: folder_format.unwrap_or_else(|| "%Y%m%d_%H%M%S".to_string()),
        max_snapshots,
        max_age_days,
    };

    let target = SyncTarget {
        name: name.clone(),
        source: PathBuf::from(&source),
        destination: PathBuf::from(&dest),
        mode,
        transport,
        conflict,
        archive_rule,
        ignore_patterns: ignore,
        enabled: !disable,
        credentials,
    };

    println!("✓ Adding sync target '{}'", target.name);
    println!("  source:      {}", target.source.display());
    println!("  destination: {}", target.destination.display());
    println!("  mode:        {}", target.mode.as_str());
    println!("  transport:   {:?}", target.transport);
    println!("  conflict:    {}", target.conflict.as_str());
    println!("  enabled:     {}", target.enabled);
    if let Some(c) = &target.credentials {
        println!("  remote url:  {}", if c.url.is_empty() { "(none)" } else { &c.url });
        if c.username.is_some() {
            println!("  username:    (set)");
        }
        if c.password.is_some() {
            println!("  password:    (set, hidden)");
        }
        if c.bucket.is_some() {
            println!("  bucket:      {}", c.bucket.as_ref().unwrap());
        }
        if c.access_key.is_some() {
            println!("  access_key:  (set)");
        }
        if c.secret_key.is_some() {
            println!("  secret_key:  (set, hidden)");
        }
        if let Some(r) = &c.region {
            println!("  region:      {}", r);
        }
    }

    cfg.upsert(target);
    save_config(&paths, &cfg)?;
    println!("✓ Saved to {}", sync_config_path(&paths).display());
    Ok(())
}

/// `route sync list` — list all sync targets.
pub fn sync_list() -> Result<()> {
    let paths = cwd_paths()?;
    let cfg = load_config(&paths)?;

    if cfg.targets.is_empty() {
        println!("(no sync targets — run `route sync add ...` to create one)");
        return Ok(());
    }

    println!("{:<16} {:<8} {:<10} {:<8} {:<8} src → dst", "NAME", "MODE", "TRANSPORT", "ENABLED", "CONFLICT");
    println!("{}", "-".repeat(80));
    for t in &cfg.targets {
        println!(
            "{:<16} {:<8} {:<10} {:<8} {:<8} {} → {}",
            t.name,
            t.mode.as_str(),
            format!("{:?}", t.transport).to_lowercase(),
            if t.enabled { "yes" } else { "no" },
            t.conflict.as_str(),
            t.source.display(),
            t.destination.display(),
        );
    }
    Ok(())
}

/// `route sync remove <name>` — remove a sync target.
pub fn sync_remove(name: String) -> Result<()> {
    let paths = cwd_paths()?;
    let mut cfg = load_config(&paths)?;
    if !cfg.remove(&name) {
        return Err(anyhow!("Sync target '{}' not found", name));
    }
    save_config(&paths, &cfg)?;
    println!("✓ Removed sync target '{}'", name);
    Ok(())
}

/// `route sync show <name>` — show details of a sync target.
pub fn sync_show(name: String) -> Result<()> {
    let paths = cwd_paths()?;
    let cfg = load_config(&paths)?;
    let t = cfg.find(&name).ok_or_else(|| anyhow!("Sync target '{}' not found", name))?;

    println!("Name:        {}", t.name);
    println!("Source:      {}", t.source.display());
    println!("Destination: {}", t.destination.display());
    println!("Mode:        {}", t.mode.as_str());
    println!("Transport:   {:?}", t.transport);
    println!("Conflict:    {}", t.conflict.as_str());
    println!("Enabled:     {}", t.enabled);
    println!("Archive rule:");
    println!("  folder_format:  {}", t.archive_rule.folder_format);
    println!("  max_snapshots:  {:?}", t.archive_rule.max_snapshots);
    println!("  max_age_days:   {:?}", t.archive_rule.max_age_days);
    if !t.ignore_patterns.is_empty() {
        println!("Ignore patterns:");
        for p in &t.ignore_patterns {
            println!("  - {}", p);
        }
    }
    if let Some(c) = &t.credentials {
        println!("Remote credentials:");
        if !c.url.is_empty() {
            println!("  url:        {}", c.url);
        }
        if c.username.is_some() {
            println!("  username:   (set)");
        }
        if c.password.is_some() {
            println!("  password:   (set, hidden)");
        }
        if let Some(b) = &c.bucket {
            println!("  bucket:     {}", b);
        }
        if c.access_key.is_some() {
            println!("  access_key: (set)");
        }
        if c.secret_key.is_some() {
            println!("  secret_key: (set, hidden)");
        }
        if let Some(r) = &c.region {
            println!("  region:     {}", r);
        }
    }
    Ok(())
}

/// `route sync enable <name>` / `route sync disable <name>`.
pub fn sync_set_enabled(name: String, enabled: bool) -> Result<()> {
    let paths = cwd_paths()?;
    let mut cfg = load_config(&paths)?;
    let t = cfg.find_mut(&name).ok_or_else(|| anyhow!("Sync target '{}' not found", name))?;
    t.enabled = enabled;
    let label = if enabled { "enabled" } else { "disabled" };
    save_config(&paths, &cfg)?;
    println!("✓ Sync target '{}' {}", name, label);
    Ok(())
}

/// `route sync run [name]` — run sync immediately (all targets or one).
pub fn sync_run(name: Option<String>, verbose: bool) -> Result<()> {
    let paths = cwd_paths()?;
    let cfg = load_config(&paths)?;

    // Build a sync engine, optionally with the project's event bus attached
    // so plugin dispatch (logger/webhook/...) fires for SyncStarted/Completed.
    let engine = SyncEngine::new();
    match crate::plugin_commands::build_bus(&paths) {
        Ok(Some(bus)) => {
            if let Err(e) = engine.set_event_bus(bus) {
                tracing::warn!(error = %e, "could not attach event bus to sync engine");
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "failed to build event bus for sync"),
    }

    let results: Vec<SyncResult> = match &name {
        Some(n) => {
            let t = cfg.find(n).ok_or_else(|| anyhow!("Sync target '{}' not found", n))?;
            vec![engine.run(t)]
        }
        None => cfg
            .targets
            .iter()
            .filter(|t| t.enabled)
            .map(|t| engine.run(t))
            .collect(),
    };

    if results.is_empty() {
        println!("(no enabled sync targets to run)");
        return Ok(());
    }

    let mut total_ok = 0;
    let mut total_err = 0;
    for r in &results {
        match &r.error {
            None => {
                total_ok += 1;
                println!(
                    "✓ {} [{}] scanned={} copied={} skipped={} deleted={} conflicts={} bytes={}",
                    r.target_name,
                    r.mode,
                    r.stats.files_scanned,
                    r.stats.files_copied,
                    r.stats.files_skipped,
                    r.stats.files_deleted,
                    r.stats.conflicts_resolved,
                    r.stats.bytes_copied,
                );
                if verbose && r.stats.errors > 0 {
                    println!("    ({} errors suppressed)", r.stats.errors);
                }
            }
            Some(e) => {
                total_err += 1;
                println!("✗ {} [{}] error: {}", r.target_name, r.mode, e);
            }
        }
    }
    println!();
    println!("Summary: {} ok, {} errors", total_ok, total_err);
    if total_err > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// `route sync start --interval <secs>` — run scheduler in foreground until Ctrl+C.
pub fn sync_start(interval_secs: u64) -> Result<()> {
    let paths = cwd_paths()?;
    let cfg = load_config(&paths)?;

    if cfg.targets.iter().filter(|t| t.enabled).count() == 0 {
        println!("(no enabled sync targets — nothing to schedule)");
        return Ok(());
    }

    println!("Starting scheduler with interval={}s", interval_secs);
    println!("Targets:");
    for t in &cfg.targets {
        if t.enabled {
            println!("  - {} [{}] {} → {}", t.name, t.mode.as_str(), t.source.display(), t.destination.display());
        }
    }
    println!("Press Ctrl+C to stop.");

    install_ctrlc_handler();

    let mut scheduler = Scheduler::start_all(&cfg, interval_secs);

    // Wait until stop signal
    while !is_stop_requested() {
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("\nStopping scheduler...");
    scheduler.stop_all();
    println!("✓ Scheduler stopped.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Ctrl-C handler (platform-agnostic-ish)
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

fn install_ctrlc_handler() {
    #[cfg(unix)]
    unsafe {
        use std::os::raw::c_int;
        extern "C" {
            fn signal(signum: c_int, handler: extern "C" fn(c_int)) -> usize;
        }
        const SIGINT: c_int = 2;
        extern "C" fn handle(_sig: c_int) {
            STOP_FLAG.store(true, AtomicOrdering::Relaxed);
        }
        signal(SIGINT, handle);
    }
    #[cfg(windows)]
    unsafe {
        use std::os::raw::c_int;
        extern "system" fn handle(_ctrl: c_int) -> i32 {
            STOP_FLAG.store(true, AtomicOrdering::Relaxed);
            1
        }
        extern "system" {
            fn SetConsoleCtrlHandler(
                handler: Option<unsafe extern "system" fn(c_int) -> i32>,
                add: i32,
            ) -> i32;
        }
        SetConsoleCtrlHandler(Some(handle), 1);
    }
}

fn is_stop_requested() -> bool {
    STOP_FLAG.load(AtomicOrdering::Relaxed)
}
