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
