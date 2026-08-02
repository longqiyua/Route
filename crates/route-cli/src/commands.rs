//! CLI command implementations.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use route_basic::{
    BasicRepository, BranchKind, CommitOptions, CreateBranchOptions, ExportContext, ExportFormat,
};
use route_core::short_id;

fn open_repo() -> Result<BasicRepository> {
    let cwd = std::env::current_dir()?;
    let repo = BasicRepository::open(&cwd)
        .context("Not in a Route basic repository. Run `route init` first.")?;
    // Auto-attach event bus from plugins.json if any plugins are configured.
    // Errors during bus construction are logged but never fatal —
    // a misconfigured plugin must not block normal CLI operations.
    let paths = route_core::RoutePaths::new(&cwd);
    match crate::plugin_commands::build_bus(&paths) {
        Ok(Some(bus)) => {
            if let Err(e) = repo.set_event_bus(bus) {
                tracing::warn!(error = %e, "could not attach event bus");
            }
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(error = %e, "failed to build event bus from plugins.json");
        }
    }
    Ok(repo)
}

fn resolve_snapshot_prefix(repo: &BasicRepository, prefix: &str) -> Result<String> {
    let snapshots = repo.all_snapshots()?;
    let matches: Vec<_> = snapshots
        .iter()
        .filter(|s| s.snapshot.id.starts_with(prefix))
        .collect();
    match matches.len() {
        0 => Err(anyhow!("No snapshot matches prefix: {}", prefix)),
        1 => Ok(matches[0].snapshot.id.clone()),
        _ => Err(anyhow!(
            "Ambiguous snapshot prefix '{}': matches {} snapshots",
            prefix,
            matches.len()
        )),
    }
}

pub fn init(path: Option<String>) -> Result<()> {
    let target = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };
    let repo = BasicRepository::init(&target)?;
    println!("✓ Route basic repository initialized");
    println!("  Path: {}", repo.paths.route_dir.display());
    println!("  Branch: main (default)");
    Ok(())
}

pub fn status() -> Result<()> {
    let repo = open_repo()?;
    let branches = repo.list_branches()?;
    let current = repo.get_current_branch_name()?;
    let history = repo.history(1)?;

    println!("Project:  {}", repo.project_path().display());
    println!("Mode:     {}", repo.config.mode);
    println!("Branch:   {} (current)", current);
    println!("Branches: {}", branches.len());

    if let Some(latest) = history.first() {
        println!(
            "Latest:   [{}] {}",
            short_id(&latest.snapshot.id),
            format_ts(latest.snapshot.created_at)
        );
    }

    println!();
    println!("Branches:");
    for b in &branches {
        let marker = if b.name == current { "*" } else { " " };
        let head = b
            .head_snapshot
            .as_deref()
            .map(|s| short_id(s))
            .unwrap_or_else(|| "(empty)".to_string());
        println!("  {} {} ({}) head={}", marker, b.name, b.kind.as_str(), head);
    }
    Ok(())
}

pub fn commit(
    message: String,
    author: Option<String>,
    full: bool,
    branch: Option<String>,
) -> Result<()> {
    let repo = open_repo()?;
    let commit = repo.commit(CommitOptions {
        message,
        author,
        force_full: full,
        branch,
        operator: Some("user".to_string()),
        body: None,
        is_checkpoint: false,
        is_ai: false,
    })?;
    println!("✓ Commit [{}]", short_id(&commit.id));
    println!("  Kind: {}", commit.kind.as_str());
    println!(
        "  Snapshot: {} → {}",
        short_id(&commit.from_snapshot),
        short_id(&commit.to_snapshot)
    );
    if let Some(diff) = &commit.diff_summary {
        if let Ok(d) = serde_json::from_str::<route_basic::DiffSummary>(diff) {
            println!("  Diff: {}", d.short());
        }
    }
    Ok(())
}

pub fn log(limit: usize, branch: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let commits = repo.list_commits(branch.as_deref(), limit)?;
    if commits.is_empty() {
        println!("(no commits yet)");
        return Ok(());
    }
    for c in commits {
        let branch_name = repo
            .get_branch_by_id(&c.branch_id)
            .map(|b| b.name)
            .unwrap_or_else(|_| "?".to_string());
        println!("[{}] {} ({})", short_id(&c.id), format_ts(c.created_at), branch_name);
        println!("  {}", c.message);
        if let Some(diff) = &c.diff_summary {
            if let Ok(d) = serde_json::from_str::<route_basic::DiffSummary>(diff) {
                println!("  diff: {}", d.short());
            }
        }
        println!();
    }
    Ok(())
}

pub fn rollback(snapshot_id: String, reason: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let full_id = resolve_snapshot_prefix(&repo, &snapshot_id)?;
    let commit = repo.rollback_to(&full_id, reason.as_deref())?;
    println!(
        "✓ Rolled back via commit [{}] to snapshot {}",
        short_id(&commit.id),
        short_id(&commit.to_snapshot)
    );
    Ok(())
}

pub fn backup(target: String) -> Result<()> {
    let repo = open_repo()?;
    let target_path = PathBuf::from(&target);
    let result = repo.full_backup_to_dir(&target_path)?;
    println!("✓ Full backup completed to: {}", result.display());
    Ok(())
}

pub fn branch_list() -> Result<()> {
    let repo = open_repo()?;
    let branches = repo.list_branches()?;
    let current = repo.get_current_branch_name()?;
    for b in branches {
        let marker = if b.name == current { "*" } else { " " };
        let head = b
            .head_snapshot
            .as_deref()
            .map(short_id)
            .unwrap_or_else(|| "(empty)".to_string());
        println!("{} {} ({}) head={}", marker, b.name, b.kind.as_str(), head);
    }
    Ok(())
}

pub fn branch_create(name: String, kind: String, from: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let kind = match kind.as_str() {
        "main" => BranchKind::Main,
        "inherited" => BranchKind::Inherited,
        "sandbox" => BranchKind::Sandbox,
        other => return Err(anyhow!("Unknown branch kind: {} (use main|inherited|sandbox)", other)),
    };
    let current = repo.get_current_branch_name()?;
    let branch = repo.create_branch(
        &name,
        CreateBranchOptions {
            kind,
            from_branch: from,
        },
        &current,
    )?;
    println!("✓ Branch '{}' created ({})", branch.name, branch.kind.as_str());
    Ok(())
}

pub fn branch_delete(name: String) -> Result<()> {
    let repo = open_repo()?;
    repo.delete_branch(&name)?;
    println!("✓ Branch '{}' deleted", name);
    Ok(())
}

pub fn branch_switch(name: String) -> Result<()> {
    let repo = open_repo()?;
    repo.set_current_branch(&name)?;
    println!("✓ Switched to branch '{}'", name);
    Ok(())
}

pub fn annotate(commit_id: String, text: String) -> Result<()> {
    let repo = open_repo()?;
    let annotation = repo.add_path_annotation(&commit_id, &text)?;
    println!("✓ Annotation [{}] added to commit {}", short_id(&annotation.id), short_id(&commit_id));
    Ok(())
}

pub fn annotations(commit_id: String) -> Result<()> {
    let repo = open_repo()?;
    let list = repo.list_path_annotations(&commit_id)?;
    if list.is_empty() {
        println!("(no annotations)");
        return Ok(());
    }
    for a in list {
        println!("[{}] {}", short_id(&a.id), format_ts(a.created_at));
        println!("  {}", a.text);
    }
    Ok(())
}

pub fn export(format: String, out: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let fmt = ExportFormat::from_str(&format)
        .ok_or_else(|| anyhow!("Unknown format: {} (use json|markdown|mermaid|emacs|zip|folder)", format))?;

    // File-based formats (ZIP, Folder) require a path and use repository methods.
    if fmt.is_file_based() {
        let path = out.ok_or_else(|| {
            anyhow!("File-based format {:?} requires --out <path>", fmt)
        })?;
        let result = match fmt {
            ExportFormat::Zip => repo.export_zip(std::path::Path::new(&path))?,
            ExportFormat::Folder => repo.export_folder(std::path::Path::new(&path))?,
            _ => unreachable!(),
        };
        println!("✓ Exported to {}", result.display());
        return Ok(());
    }

    // Text formats: use Exporter trait.
    let branches = repo.list_branches()?;
    let snapshots = repo.all_snapshots()?;
    let commits = repo.list_commits(None, 1000)?;

    // Collect annotations per commit
    let mut annotations = std::collections::HashMap::new();
    for c in &commits {
        let anns = repo.list_path_annotations(&c.id).unwrap_or_default();
        annotations.insert(c.id.clone(), anns);
    }

    let project_name = repo
        .project_path()
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "route-project".to_string());

    let ctx = ExportContext {
        project_name,
        project_path: repo.project_path().to_string_lossy().to_string(),
        branches,
        snapshots: snapshots.into_iter().map(|s| s.snapshot).collect(),
        commits,
        annotations,
    };

    let exporter = route_basic::DefaultExporters::for_format(fmt)
        .ok_or_else(|| anyhow!("No exporter for format: {:?}", fmt))?;

    let mut writer: Box<dyn Write> = match &out {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };
    exporter.export(&ctx, &mut writer)?;

    if let Some(path) = out {
        println!("✓ Exported to {}", path);
    }
    Ok(())
}

pub fn stats() -> Result<()> {
    let repo = open_repo()?;
    let branches = repo.list_branches()?;
    let snapshots = repo.all_snapshots()?;
    let commits = repo.list_commits(None, 10000)?;

    let main_count = branches.iter().filter(|b| b.kind == BranchKind::Main).count();
    let inherited_count = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Inherited)
        .count();
    let sandbox_count = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Sandbox)
        .count();

    let inc = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Incremental)
        .count();
    let full = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Full)
        .count();
    let rollback = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Rollback)
        .count();
    let merge = commits
        .iter()
        .filter(|c| c.kind == route_basic::CommitKind::Merge)
        .count();

    println!("Route 统计");
    println!("-----------");
    println!("分支总数:   {}", branches.len());
    println!("  main:     {}", main_count);
    println!("  inherited:{}", inherited_count);
    println!("  sandbox:  {}", sandbox_count);
    println!("快照总数:   {}", snapshots.len());
    println!("Commit 总数:{}", commits.len());
    println!("  incremental: {}", inc);
    println!("  full:        {}", full);
    println!("  rollback:    {}", rollback);
    println!("  merge:       {}", merge);
    Ok(())
}

pub fn stats_report(
    format: String,
    out: Option<String>,
    top: usize,
    hourly: usize,
    daily: usize,
) -> Result<()> {
    let repo = open_repo()?;
    let opts = route_stats::CollectOptions {
        range: None,
        top_files_limit: top,
        hourly_buckets: hourly,
        daily_buckets: daily,
    };
    let stats = route_stats::StatsCollector::collect_with(&repo, opts)?;
    let content = match format.as_str() {
        "json" => route_stats::render_json(&stats)?,
        "markdown" | "md" => route_stats::render_markdown(&stats)?,
        other => return Err(anyhow!("Unknown format: {} (use json|markdown)", other)),
    };

    match &out {
        Some(path) => {
            std::fs::write(path, &content)?;
            println!("✓ Stats report written to {}", path);
        }
        None => {
            print!("{}", content);
            if !content.ends_with('\n') {
                println!();
            }
        }
    }
    Ok(())
}

fn format_ts(millis: i64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

// ---------------------------------------------------------------------------
// Tags, snapshot diff, working-directory changes
// ---------------------------------------------------------------------------

pub fn tag_list() -> Result<()> {
    let repo = open_repo()?;
    let tags = repo.tag_list()?;
    if tags.is_empty() {
        println!("(no tags)");
        return Ok(());
    }
    println!("{:<24} {:<14} {:<24} {}", "NAME", "SNAPSHOT", "CREATED", "MESSAGE");
    for t in tags {
        let msg = t.message.as_deref().unwrap_or("");
        println!(
            "{:<24} {:<14} {:<24} {}",
            t.name,
            route_core::short_id(&t.snapshot_id),
            format_ts(t.created_at),
            msg,
        );
    }
    Ok(())
}

pub fn tag_add(name: String, snapshot_id: String, message: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let tag = repo.tag_create(&name, &snapshot_id, message.as_deref())?;
    println!(
        "✓ Tag '{}' → {} ({})",
        tag.name,
        route_core::short_id(&tag.snapshot_id),
        tag.id,
    );
    Ok(())
}

pub fn tag_remove(name: String) -> Result<()> {
    let repo = open_repo()?;
    repo.tag_delete(&name)?;
    println!("✓ Tag '{}' deleted", name);
    Ok(())
}

pub fn diff_snapshots(from: String, to: String) -> Result<()> {
    let repo = open_repo()?;
    let entries = repo.diff_snapshots(&from, &to)?;
    if entries.is_empty() {
        println!("No differences between {} and {}.", from, to);
        return Ok(());
    }
    let added = entries.iter().filter(|e| e.change == "added").count();
    let modified = entries.iter().filter(|e| e.change == "modified").count();
    let removed = entries.iter().filter(|e| e.change == "removed").count();
    println!(
        "Diff {} → {}  (+{} ~{} -{})",
        route_core::short_id(&from),
        route_core::short_id(&to),
        added,
        modified,
        removed,
    );
    println!("{:<10} {}", "CHANGE", "PATH");
    for e in &entries {
        println!("{:<10} {}", e.change, e.path);
    }
    Ok(())
}

pub fn changes() -> Result<()> {
    let repo = open_repo()?;
    let status = repo.working_dir_status()?;
    if status.is_empty() {
        println!("Working directory clean — nothing to commit.");
        return Ok(());
    }
    let added = status.iter().filter(|s| s.change == "added").count();
    let modified = status.iter().filter(|s| s.change == "modified").count();
    let removed = status.iter().filter(|s| s.change == "removed").count();
    println!(
        "Working directory changes  (+{} ~{} -{})",
        added, modified, removed,
    );
    println!("{:<10} {:<14} {}", "CHANGE", "SIZE", "PATH");
    for s in &status {
        let size = s
            .size_bytes
            .map(|n| format_bytes(n))
            .unwrap_or_else(|| "-".to_string());
        println!("{:<10} {:<14} {}", s.change, size, s.path);
    }
    Ok(())
}

fn format_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = n as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx + 1 < UNITS.len() {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", n, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit_idx])
    }
}

// ---------------------------------------------------------------------------
// Undo / Redo / Checkpoint
// ---------------------------------------------------------------------------

pub fn undo() -> Result<()> {
    let repo = open_repo()?;
    let commit = repo.undo_last()?;
    println!("✓ Undo — commit [{}] undone", short_id(&commit.id));
    Ok(())
}

pub fn redo() -> Result<()> {
    let repo = open_repo()?;
    let commit = repo.redo_last()?;
    println!("✓ Redo — commit [{}] re-applied", short_id(&commit.id));
    Ok(())
}

pub fn checkpoint(title: String, body: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let commit = repo.checkpoint_create(&title, body.as_deref(), Some("user"))?;
    println!("✓ Checkpoint [{}] created: {}", short_id(&commit.id), title);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tracking (active sync with remote repos)
// ---------------------------------------------------------------------------

/// Helper: read the tracking config file.
fn read_tracking_config(project_path: &std::path::Path) -> Result<serde_json::Value> {
    let path = project_path.join(".route").join("tracking.json");
    if !path.exists() {
        return Ok(serde_json::json!({ "folders": [] }));
    }
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Helper: write the tracking config file.
fn write_tracking_config(project_path: &std::path::Path, config: &serde_json::Value) -> Result<()> {
    let path = project_path.join(".route").join("tracking.json");
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(&path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

pub fn tracking_list() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config = read_tracking_config(&cwd)?;
    let folders = config["folders"].as_array().cloned().unwrap_or_default();
    if folders.is_empty() {
        println!("(no tracking targets)");
        return Ok(());
    }
    println!("Active tracking targets:");
    for (i, f) in folders.iter().enumerate() {
        let local = f["local_path"].as_str().unwrap_or("");
        let remote = f["remote_url"].as_str().unwrap_or("");
        let branch = f["branch"].as_str().unwrap_or("main");
        let enabled = f["enabled"].as_bool().unwrap_or(false);
        let status = f["last_status"].as_str().unwrap_or("pending");
        println!(
            "  {}. {} [{}] {} → {} ({})",
            i + 1,
            if enabled { "✓" } else { "○" },
            branch,
            local,
            remote,
            status
        );
    }
    Ok(())
}

pub fn tracking_add(local: String, remote: String, branch: String, interval: u64) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut config = read_tracking_config(&cwd)?;
    let folders = config["folders"].as_array_mut().unwrap();
    folders.push(serde_json::json!({
        "local_path": local,
        "remote_url": remote,
        "branch": branch,
        "enabled": true,
        "interval_secs": interval,
        "last_sync": null,
        "last_status": "pending",
        "syncing": false,
    }));
    write_tracking_config(&cwd, &config)?;
    println!("✓ Tracking target added: {} → {} ({})", local, remote, branch);
    Ok(())
}

pub fn tracking_remove(local: String) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut config = read_tracking_config(&cwd)?;
    let folders = config["folders"].as_array_mut().unwrap();
    folders.retain(|f| f["local_path"].as_str() != Some(&local));
    write_tracking_config(&cwd, &config)?;
    println!("✓ Tracking target removed: {}", local);
    Ok(())
}

pub fn tracking_sync(local: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config = read_tracking_config(&cwd)?;
    let folders = config["folders"].as_array().cloned().unwrap_or_default();
    let targets: Vec<&serde_json::Value> = match &local {
        Some(l) => {
            let filtered: Vec<_> = folders.iter().filter(|f| f["local_path"].as_str() == Some(l.as_str())).collect();
            if filtered.is_empty() {
                return Err(anyhow!("no tracking target matches: {}", l));
            }
            filtered
        }
        None => folders.iter().collect(),
    };
    for target in targets {
        let local_path = target["local_path"].as_str().unwrap();
        let remote_url = target["remote_url"].as_str().unwrap();
        let branch = target["branch"].as_str().unwrap_or("main");
        println!("Syncing {} → {} ({})...", local_path, remote_url, branch);
        let path = std::path::Path::new(local_path);
        if !path.exists() {
            println!("  ⚠ Local path does not exist, skipping");
            continue;
        }
        // Perform git fetch + pull
        let fetch_result = std::process::Command::new("git")
            .args(["-C", local_path, "fetch", "origin"])
            .output();
        match fetch_result {
            Ok(out) if out.status.success() => {
                println!("  ✓ Fetch OK");
                let pull_result = std::process::Command::new("git")
                    .args(["-C", local_path, "pull", "--rebase", "origin", branch])
                    .output();
                match pull_result {
                    Ok(pout) if pout.status.success() => {
                        println!("  ✓ Pull OK: {}", String::from_utf8_lossy(&pout.stdout).trim().lines().last().unwrap_or("done"));
                    }
                    Ok(pout) => {
                        println!("  ⚠ Pull issue: {}", String::from_utf8_lossy(&pout.stderr).trim());
                    }
                    Err(e) => {
                        println!("  ⚠ Pull failed: {e}");
                    }
                }
            }
            Ok(out) => {
                println!("  ⚠ Fetch issue: {}", String::from_utf8_lossy(&out.stderr).trim());
            }
            Err(e) => {
                println!("  ⚠ Fetch failed: {e}");
            }
        }
    }
    Ok(())
}

pub fn tracking_history() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let history_path = cwd.join(".route").join("tracking-history.json");
    if !history_path.exists() {
        println!("(no sync history)");
        return Ok(());
    }
    let content = std::fs::read_to_string(&history_path)?;
    let history: Vec<serde_json::Value> = serde_json::from_str(&content)?;
    if history.is_empty() {
        println!("(no sync history)");
        return Ok(());
    }
    println!("Sync history (newest first):");
    for entry in history.iter().rev().take(20) {
        let time = entry["timestamp"].as_str().unwrap_or("?");
        let local = entry["local_path"].as_str().unwrap_or("?");
        let status = entry["status"].as_str().unwrap_or("?");
        println!("  [{}] {} — {}", time, local, status);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Extensions (skills and references)
// ---------------------------------------------------------------------------

pub fn extensions_skills() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let skills_dir = cwd.join(".route").join("skills");
    if !skills_dir.exists() {
        println!("(no skills directory — create .route/skills/ to add skills)");
        return Ok(());
    }
    let mut files: Vec<_> = std::fs::read_dir(&skills_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();
    files.sort_by_key(|e| e.file_name());
    if files.is_empty() {
        println!("(no skill files)");
        return Ok(());
    }
    println!("Skill files:");
    for f in &files {
        let meta = f.metadata()?;
        let name = f.file_name().to_string_lossy().to_string();
        let size = format_bytes(meta.len());
        println!("  {} ({})", name, size);
    }
    Ok(())
}

pub fn extensions_references() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let refs_dir = cwd.join(".route").join("references");
    if !refs_dir.exists() {
        println!("(no references directory — create .route/references/ to add references)");
        return Ok(());
    }
    let mut files: Vec<_> = std::fs::read_dir(&refs_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .collect();
    files.sort_by_key(|e| e.file_name());
    if files.is_empty() {
        println!("(no reference files)");
        return Ok(());
    }
    println!("Reference files:");
    for f in &files {
        let meta = f.metadata()?;
        let name = f.file_name().to_string_lossy().to_string();
        let size = format_bytes(meta.len());
        println!("  {} ({})", name, size);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// AI Configuration
// ---------------------------------------------------------------------------

pub fn ai_config(show: bool) -> Result<()> {
    if !show {
        println!("AI Configuration:");
    }
    let key = std::env::var("ROUTE_AI_KEY").unwrap_or_default();
    if key.is_empty() {
        println!("  Status: disabled (set ROUTE_AI_KEY to enable)");
        return Ok(());
    }
    let masked = if key.len() > 8 {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    } else {
        "****".to_string()
    };
    println!("  Provider:   {}", std::env::var("ROUTE_AI_PROVIDER").unwrap_or_else(|_| "openai".to_string()));
    println!("  Endpoint:   {}", std::env::var("ROUTE_AI_ENDPOINT").unwrap_or_else(|_| "https://api.openai.com/v1".to_string()));
    println!("  Model:      {}", std::env::var("ROUTE_AI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string()));
    println!("  Key:        {}", masked);
    Ok(())
}

// ---------------------------------------------------------------------------
// AI Chat
// ---------------------------------------------------------------------------

pub fn ai_chat(message: String, system: Option<String>) -> Result<()> {
    let key = std::env::var("ROUTE_AI_KEY").map_err(|_| {
        anyhow!("ROUTE_AI_KEY not set. Set ROUTE_AI_KEY to enable AI chat.")
    })?;
    let endpoint = std::env::var("ROUTE_AI_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("ROUTE_AI_MODEL")
        .unwrap_or_else(|_| "gpt-4o".to_string());

    let client = reqwest::blocking::Client::new();
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    let system_prompt = system.unwrap_or_else(|| "You are a helpful AI assistant.".to_string());

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": message}
        ],
        "temperature": 0.7,
        "max_tokens": 4096,
    });

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| anyhow!("AI request failed: {}", e))?;

    let status = response.status();
    let body_text = response
        .text()
        .map_err(|e| anyhow!("Cannot read AI response: {}", e))?;

    if !status.is_success() {
        return Err(anyhow!("AI API error ({}): {}", status.as_u16(), body_text));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body_text).map_err(|e| anyhow!("Cannot parse AI response: {}", e))?;

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("No content in AI response"))?;

    println!("{}", content);
    Ok(())
}

// ---------------------------------------------------------------------------
// Project Context
// ---------------------------------------------------------------------------

pub fn project_context() -> Result<()> {
    let cwd = std::env::current_dir()?;

    // 1. Git branch
    println!("=== Git Branch ===");
    let branch_output = std::process::Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "branch", "--show-current"])
        .output();
    match branch_output {
        Ok(out) if out.status.success() => {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            println!("  {}", branch);
        }
        _ => {
            println!("  (not a git repository or git not available)");
        }
    }
    println!();

    // 2. Recent commits
    println!("=== Recent Commits ===");
    let log_output = std::process::Command::new("git")
        .args([
            "-C",
            &cwd.to_string_lossy(),
            "log",
            "--oneline",
            "-10",
        ])
        .output();
    match log_output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            let lines: Vec<&str> = text.lines().collect();
            if lines.is_empty() {
                println!("  (no commits)");
            } else {
                for line in &lines {
                    println!("  {}", line);
                }
            }
        }
        _ => {
            println!("  (no git history)");
        }
    }
    println!();

    // 3. Skills
    println!("=== Skills (.route/skills/) ===");
    let skills_dir = cwd.join(".route").join("skills");
    if skills_dir.exists() {
        let mut files: Vec<_> = std::fs::read_dir(&skills_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();
        files.sort_by_key(|e| e.file_name());
        if files.is_empty() {
            println!("  (no skill files)");
        } else {
            for f in &files {
                let name = f.file_name().to_string_lossy().to_string();
                let content = std::fs::read_to_string(f.path()).unwrap_or_default();
                let preview: String = content.chars().take(200).collect();
                let trunc = if content.len() > 200 { "..." } else { "" };
                println!("  - {}:\n    {}{}", name, preview, trunc);
            }
        }
    } else {
        println!("  (no skills directory)");
    }
    println!();

    // 4. References
    println!("=== References (.route/references/) ===");
    let refs_dir = cwd.join(".route").join("references");
    if refs_dir.exists() {
        let mut files: Vec<_> = std::fs::read_dir(&refs_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();
        files.sort_by_key(|e| e.file_name());
        if files.is_empty() {
            println!("  (no reference files)");
        } else {
            for f in &files {
                let name = f.file_name().to_string_lossy().to_string();
                let content = std::fs::read_to_string(f.path()).unwrap_or_default();
                let preview: String = content.chars().take(200).collect();
                let trunc = if content.len() > 200 { "..." } else { "" };
                println!("  - {}:\n    {}{}", name, preview, trunc);
            }
        }
    } else {
        println!("  (no references directory)");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// MCP Configuration
// ---------------------------------------------------------------------------

pub fn mcp_config(show_config: bool) -> Result<()> {
    if show_config {
        let mcp_config = serde_json::json!({
            "mcpServers": {
                "route": {
                    "command": "route",
                    "args": ["mcp"],
                    "env": {}
                }
            }
        });
        println!("{}", serde_json::to_string_pretty(&mcp_config)?);
    } else {
        println!("MCP Server Configuration");
        println!("  Command: route mcp");
        println!("  Use --config to print JSON config snippet for AI client integration");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Permission level
// ---------------------------------------------------------------------------

pub fn permission_status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join(".route").join("permission.json");
    let level = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: serde_json::Value = serde_json::from_str(&content)?;
        config["level"].as_str().unwrap_or("normal").to_string()
    } else {
        "normal".to_string()
    };
    println!("Current permission level: {}", level);
    Ok(())
}

pub fn permission_set(level: String) -> Result<()> {
    let l = level.to_lowercase();
    if l != "normal" && l != "high" {
        return Err(anyhow!("invalid permission level: {} (use normal|high)", l));
    }
    let cwd = std::env::current_dir()?;
    let route_dir = cwd.join(".route");
    std::fs::create_dir_all(&route_dir)?;
    let config_path = route_dir.join("permission.json");
    let config = serde_json::json!({ "level": l });
    std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
    println!("✓ Permission level set to: {}", l);
    Ok(())
}
