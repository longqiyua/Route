//! CLI command implementations.

use std::io::Write;
use std::path::{Path, PathBuf};

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

pub fn init(path: Option<String>, scan: bool) -> Result<()> {
    let target = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };
    let already = route_core::RoutePaths::new(&target).is_initialized();
    let repo = if already {
        BasicRepository::open(&target)?
    } else {
        BasicRepository::init(&target)?
    };
    // Create the Route Guard anchor to protect Route's data files
    let _ = route_core::ensure_guard_anchor(&target);
    // Initialize profile store with built-in profiles
    if let Err(e) = route_basic::init_profile_store(&target) {
        tracing::warn!(error = %e, "could not initialize profile store");
    }
    // Initialize external archive (Original save) in Documents/Route/
    let route_version = env!("CARGO_PKG_VERSION");
    let project_id = match route_basic::init_project_archive(&target, &repo, route_version) {
        Ok(id) => {
            if already {
                println!("✓ Archive already initialized");
            } else {
                let archive_dir = route_basic::project_archive_dir(&id)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "(unknown)".to_string());
                println!("✓ Original save created");
                println!("  Project ID: {}", &id[..16]);
                println!("  Archive:    {}", archive_dir);
            }
            Some(id)
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not initialize project archive");
            println!("⚠ Archive init skipped: {}", e);
            None
        }
    };

    if already {
        println!("✓ Route initialized (re-open)");
        println!("  Path:    {}", repo.paths.route_dir.display());
    } else {
        println!("✓ Route initialized");
        println!("  Path:    {}", repo.paths.route_dir.display());
        println!("  Branch:  main (default)");
        println!("  Profiles: default, fast, strict (active: default)");
    }

    // Show suggested next commands
    println!();
    println!("Next:");
    println!("  route status              → project overview");
    println!("  route task start \"...\"   → start a development session");
    if project_id.is_some() {
        println!("  route archive list        → list saves");
    }

    // If --scan is set, run bootstrap scan
    if scan {
        println!();
        println!("Running bootstrap scan...");
        match route_basic::bootstrap::bootstrap_scan(&target) {
            Ok(result) => {
                println!("✓ Bootstrap scan complete");
                println!("  Discovered items: {}", result.discovered_items);
                if !result.detected_stacks.is_empty() {
                    println!("  Detected stacks: {}", result.detected_stacks.join(", "));
                }
                println!("  Suggested profile: {}", result.suggested_profile);
                if !result.suggested_workflows.is_empty() {
                    println!(
                        "  Suggested workflows: {}",
                        result.suggested_workflows.join(", ")
                    );
                }
                if !result.suggested_packs.is_empty() {
                    println!("  Suggested packs: {}", result.suggested_packs.join(", "));
                }
                if !result.memory_candidates.is_empty() {
                    println!("  Memory candidates: {}", result.memory_candidates.len());
                }
                println!("  Understanding: {}", result.understanding);
            }
            Err(e) => {
                tracing::warn!(error = %e, "bootstrap scan failed");
                println!("⚠ Bootstrap scan failed: {}", e);
            }
        }
    }

    Ok(())
}

pub fn status(json: bool) -> Result<()> {
    let repo = open_repo()?;
    let current = repo.get_current_branch_name()?;

    let cwd = current_project_root();
    let project_id = route_basic::project_id_from_path(&cwd);

    // --- JSON output path ---
    if json {
        let check_result = repo.verify(&route_basic::VerifyOptions {
            check_blob_existence: false,
            verify_blob_content: false,
        });
        let integrity = match &check_result {
            Ok(report) => {
                let has_warnings = report
                    .findings
                    .iter()
                    .any(|f| matches!(f.severity, route_basic::VerifySeverity::Warning));
                let has_issues = report.findings.iter().any(|f| {
                    !matches!(
                        f.severity,
                        route_basic::VerifySeverity::Ok | route_basic::VerifySeverity::Warning
                    )
                });
                if !has_issues && !has_warnings {
                    "ok".to_string()
                } else if !has_issues {
                    format!("{} warnings", report.findings.len())
                } else {
                    format!("{} issues", report.findings.len())
                }
            }
            Err(_) => "check_failed".to_string(),
        };

        use route_basic::{
            BrainStore, ProfileStore, ProjectArchiveMeta, ReferenceRegistry, SessionStore,
            StrategyStore, WorkflowStore,
        };
        let session_store = SessionStore::load(&cwd).unwrap_or_default();
        let active_sessions: Vec<String> = session_store
            .sessions
            .iter()
            .filter(|s| matches!(s.status, route_basic::SessionStatus::Active))
            .map(|s| s.id.clone())
            .collect();
        let active_profile = ProfileStore::active_id(&cwd).ok().flatten();
        let meta = ProjectArchiveMeta::load(&project_id).ok().flatten();
        let brain_store = BrainStore::load(&cwd).ok();
        let brain_count = brain_store
            .as_ref()
            .and_then(|b| b.current.as_ref().map(|c| c.items.len()))
            .unwrap_or(0);
        let registry = ReferenceRegistry::read(&cwd).unwrap_or_default();
        let total_refs = registry.entries.len();
        let enabled_refs = registry.entries.iter().filter(|e| e.enabled).count();
        let active_strategy = StrategyStore::load(&cwd)
            .ok()
            .and_then(|s| s.current)
            .unwrap_or_else(|| "(none)".to_string());
        let workflow_store = WorkflowStore::load(&cwd).unwrap_or_default();
        let enabled_wf = workflow_store
            .workflows
            .iter()
            .filter(|w| w.enabled)
            .count();
        let ctx_dir = route_basic::context_history_dir(&cwd);
        let ctx_count = if ctx_dir.exists() {
            std::fs::read_dir(&ctx_dir)
                .ok()
                .map(|e| e.filter_map(|e| e.ok()).count())
                .unwrap_or(0)
        } else {
            0
        };

        let output = serde_json::json!({
            "project": {
                "id": &project_id[..16],
                "path": repo.project_path().display().to_string(),
                "mode": repo.config.mode,
                "branch": current,
                "integrity": integrity,
            },
            "work": {
                "active_sessions": active_sessions,
                "total_sessions": session_store.sessions.len(),
                "profile": active_profile,
            },
            "save": meta.map(|m| serde_json::json!({
                "original": route_basic::load_original(&project_id).ok().flatten().is_some(),
                "latest_save": m.latest_save_id.as_deref().map(|s| &s[..16]),
                "latest_verified": m.latest_verified_save_id.as_deref().map(|s| &s[..16]),
                "total_saves": m.save_count,
            })),
            "recovery": {
                "unresolved": route_basic::list_recovery_cases(&project_id).unwrap_or_default().iter().filter(|c| !c.resolved).count(),
                "total": route_basic::list_recovery_cases(&project_id).unwrap_or_default().len(),
            },
            "knowledge": {
                "brain_items": brain_count,
                "references_total": total_refs,
                "references_enabled": enabled_refs,
            },
            "ai_context": {
                "strategy": active_strategy,
                "workflows_enabled": enabled_wf,
            },
            "context": {
                "history_snapshots": ctx_count,
            },
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // --- Human-readable output ---
    println!("PROJECT");
    println!("  ID:              {}", &project_id[..16]);
    println!("  Path:            {}", repo.project_path().display());
    println!("  Mode:            {}", repo.config.mode);
    println!("  Branch:          {} (current)", current);

    // Repo integrity
    let check_result = repo.verify(&route_basic::VerifyOptions {
        check_blob_existence: false,
        verify_blob_content: false,
    });
    match check_result {
        Ok(report) => {
            let has_warnings = report
                .findings
                .iter()
                .any(|f| matches!(f.severity, route_basic::VerifySeverity::Warning));
            let has_issues = report.findings.iter().any(|f| {
                !matches!(
                    f.severity,
                    route_basic::VerifySeverity::Ok | route_basic::VerifySeverity::Warning
                )
            });
            let integrity = if !has_issues && !has_warnings {
                "✓ ok".to_string()
            } else if !has_issues {
                format!("⚠ {} warnings", report.findings.len())
            } else {
                format!("✗ {} issues", report.findings.len())
            };
            println!("  Integrity:       {}", integrity);
        }
        Err(_) => {
            println!("  Integrity:       ? (check failed)");
        }
    }

    // CURRENT WORK
    println!();
    println!("WORK");
    use route_basic::SessionStore;
    let session_store = SessionStore::load(&cwd).unwrap_or_default();
    let active_sessions: Vec<_> = session_store
        .sessions
        .iter()
        .filter(|s| matches!(s.status, route_basic::SessionStatus::Active))
        .collect();
    if active_sessions.is_empty() {
        println!("  Active session:  (none)");
    } else {
        for s in &active_sessions {
            println!("  Active session:  {} ({})", &s.id[..16], s.task);
        }
    }
    println!("  Total sessions:  {}", session_store.sessions.len());
    use route_basic::ProfileStore;
    let active_profile = ProfileStore::active_id(&cwd).ok().flatten();
    println!(
        "  Profile:         {}",
        active_profile.as_deref().unwrap_or("(none)")
    );

    // SAVE
    use route_basic::ProjectArchiveMeta;
    if let Ok(Some(meta)) = ProjectArchiveMeta::load(&project_id) {
        let original = route_basic::load_original(&project_id)
            .ok()
            .flatten()
            .map(|_| "✓".to_string())
            .unwrap_or_else(|| "✗".to_string());
        println!();
        println!("SAVE");
        println!("  Original:        {}", original);
        println!(
            "  Latest:          {}",
            meta.latest_save_id
                .as_deref()
                .map(|s| &s[..16])
                .unwrap_or("(none)")
        );
        println!(
            "  Latest verified: {}",
            meta.latest_verified_save_id
                .as_deref()
                .map(|s| &s[..16])
                .unwrap_or("(none)")
        );
        println!("  Total saves:     {}", meta.save_count);
    }

    // RECOVERY
    use route_basic::list_recovery_cases;
    let cases = list_recovery_cases(&project_id).unwrap_or_default();
    let unresolved = cases.iter().filter(|c| !c.resolved).count();
    let resolved = cases.iter().filter(|c| c.resolved).count();
    println!();
    println!("RECOVERY");
    if unresolved > 0 {
        println!("  ⚠ {} unresolved cases", unresolved);
    }
    println!(
        "  Cases:           {} total ({} resolved)",
        cases.len(),
        resolved
    );

    // KNOWLEDGE
    let brain_store = route_basic::BrainStore::load(&cwd).ok();
    let brain_count = brain_store
        .as_ref()
        .and_then(|b| b.current.as_ref().map(|c| c.items.len()))
        .unwrap_or(0);
    println!();
    println!("KNOWLEDGE");
    println!("  Brain items:     {}", brain_count);

    use route_basic::ReferenceRegistry;
    let registry = ReferenceRegistry::read(&cwd).unwrap_or_default();
    let total_refs = registry.entries.len();
    let enabled_refs = registry.entries.iter().filter(|e| e.enabled).count();
    println!(
        "  References:      {} total, {} enabled",
        total_refs, enabled_refs
    );

    // AI CONTEXT
    let active_strategy = route_basic::StrategyStore::load(&cwd)
        .ok()
        .and_then(|s| s.current)
        .unwrap_or_else(|| "(none)".to_string());
    use route_basic::WorkflowStore;
    let workflow_store = WorkflowStore::load(&cwd).unwrap_or_default();
    let enabled_wf = workflow_store
        .workflows
        .iter()
        .filter(|w| w.enabled)
        .count();
    println!();
    println!("AI CONTEXT");
    println!("  Strategy:        {}", active_strategy);
    println!("  Workflows:       {} enabled", enabled_wf);
    println!(
        "  Apply target:    {}",
        route_basic::adapter::ApplyTarget::Generic.display_name()
    );

    // CONTEXT
    let ctx_dir = route_basic::context_history_dir(&cwd);
    let ctx_count = if ctx_dir.exists() {
        std::fs::read_dir(&ctx_dir)
            .ok()
            .map(|e| e.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
    } else {
        0
    };
    println!();
    println!("CONTEXT");
    println!("  History snapshots: {}", ctx_count);

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
        println!(
            "[{}] {} ({})",
            short_id(&c.id),
            format_ts(c.created_at),
            branch_name
        );
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

/// Show the engine-owned Route command history. This command is deliberately
/// read-only: no append, edit, delete, truncate, or backfill action exists.
pub fn history(limit: usize, verify_only: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = route_basic::find_route_project_root(&cwd)
        .ok_or_else(|| anyhow!("Not in a Route project"))?;
    let verification = route_basic::verify_route_history(&root);
    if !verification.valid {
        anyhow::bail!(
            "Route history integrity failure: {}",
            verification
                .error
                .unwrap_or_else(|| "unknown integrity error".to_string())
        );
    }

    let events_path = route_basic::route_history_path(&root);
    let head_path = route_basic::route_history_head_path(&root);
    let events_locked = events_path
        .metadata()
        .map(|m| m.permissions().readonly())
        .unwrap_or(false);
    let head_locked = head_path
        .metadata()
        .map(|m| m.permissions().readonly())
        .unwrap_or(false);

    println!("ROUTE HISTORY");
    println!("  Integrity:  ✓ SHA-256 chain valid");
    println!("  Entries:    {}", verification.entries);
    println!(
        "  Locked:     {}",
        if verification.entries == 0 || (events_locked && head_locked) {
            "yes (read-only)"
        } else {
            "no"
        }
    );
    if !verification.head_hash.is_empty() {
        println!("  Head:       {}", &verification.head_hash[..16]);
    }

    if verify_only {
        return Ok(());
    }

    let events = route_basic::load_route_history(&root)?;
    if events.is_empty() {
        println!("\n(no Route command history yet)");
        return Ok(());
    }
    println!();
    for event in events.iter().rev().take(limit) {
        println!(
            "#{:04} {}  {}  {}  [{}]",
            event.sequence,
            format_ts(event.timestamp),
            if event.success { "PASS" } else { "FAIL" },
            event.operation,
            &event.hash[..12]
        );
    }
    Ok(())
}

pub fn rollback(snapshot_id: String, reason: Option<String>) -> Result<()> {
    let repo = open_repo()?;
    let full_id = resolve_snapshot_prefix(&repo, &snapshot_id)?;

    // P1: Auto-save before rollback (best-effort)
    let root = std::env::current_dir()?;
    let _ = route_basic::auto_save_before_rollback(
        &root,
        &route_basic::project_id_from_path(&root),
        &repo,
        env!("CARGO_PKG_VERSION"),
    );

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
        other => {
            return Err(anyhow!(
                "Unknown branch kind: {} (use main|inherited|sandbox)",
                other
            ))
        }
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
    println!(
        "✓ Branch '{}' created ({})",
        branch.name,
        branch.kind.as_str()
    );
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
    println!(
        "✓ Annotation [{}] added to commit {}",
        short_id(&annotation.id),
        short_id(&commit_id)
    );
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
    let fmt = ExportFormat::from_str(&format).ok_or_else(|| {
        anyhow!(
            "Unknown format: {} (use json|markdown|mermaid|emacs|zip|folder)",
            format
        )
    })?;

    // File-based formats (ZIP, Folder) require a path and use repository methods.
    if fmt.is_file_based() {
        let path =
            out.ok_or_else(|| anyhow!("File-based format {:?} requires --out <path>", fmt))?;
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

    let main_count = branches
        .iter()
        .filter(|b| b.kind == BranchKind::Main)
        .count();
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
    println!(
        "{:<24} {:<14} {:<24} {}",
        "NAME", "SNAPSHOT", "CREATED", "MESSAGE"
    );
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

pub fn checkpoint(title: String, body: Option<String>, json: bool) -> Result<()> {
    let repo = open_repo()?;
    let commit = repo.checkpoint_create(&title, body.as_deref(), Some("user"))?;
    if json {
        let output = serde_json::json!({
            "id": commit.id,
            "title": title,
            "created_at": commit.created_at,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("✓ Checkpoint [{}] created: {}", short_id(&commit.id), title);
    }
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
    println!(
        "✓ Tracking target added: {} → {} ({})",
        local, remote, branch
    );
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
            let filtered: Vec<_> = folders
                .iter()
                .filter(|f| f["local_path"].as_str() == Some(l.as_str()))
                .collect();
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
                        println!(
                            "  ✓ Pull OK: {}",
                            String::from_utf8_lossy(&pout.stdout)
                                .trim()
                                .lines()
                                .last()
                                .unwrap_or("done")
                        );
                    }
                    Ok(pout) => {
                        println!(
                            "  ⚠ Pull issue: {}",
                            String::from_utf8_lossy(&pout.stderr).trim()
                        );
                    }
                    Err(e) => {
                        println!("  ⚠ Pull failed: {e}");
                    }
                }
            }
            Ok(out) => {
                println!(
                    "  ⚠ Fetch issue: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
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
    println!(
        "  Provider:   {}",
        std::env::var("ROUTE_AI_PROVIDER").unwrap_or_else(|_| "openai".to_string())
    );
    println!(
        "  Endpoint:   {}",
        std::env::var("ROUTE_AI_ENDPOINT")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string())
    );
    println!(
        "  Model:      {}",
        std::env::var("ROUTE_AI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string())
    );
    println!("  Key:        {}", masked);
    Ok(())
}

// ---------------------------------------------------------------------------
// AI Chat
// ---------------------------------------------------------------------------

pub fn ai_chat(
    message: String,
    system: Option<String>,
    session_id: Option<String>,
    create_snapshot: bool,
) -> Result<()> {
    let key = std::env::var("ROUTE_AI_KEY")
        .map_err(|_| anyhow!("ROUTE_AI_KEY not set. Set ROUTE_AI_KEY to enable AI chat."))?;
    let endpoint = std::env::var("ROUTE_AI_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("ROUTE_AI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());

    // ---- Conversation tracking ----
    let mut store = open_conversation_store()?;
    let sid = match session_id {
        Some(id) => {
            // Verify the session exists
            if store.get_session(&id).is_none() {
                return Err(anyhow!("Session '{}' not found", id));
            }
            id
        }
        None => {
            // Auto-create a session with the message as title
            let title = if message.len() > 50 {
                format!("{}...", &message[..50])
            } else {
                message.clone()
            };
            store.create_session(&title)?;
            store.list_sessions().first().unwrap().id.clone()
        }
    };

    // Optionally create a snapshot before recording the user message
    let snapshot_id: Option<String> = if create_snapshot {
        match open_repo() {
            Ok(repo) => {
                let commit = repo.commit(CommitOptions {
                    message: format!(
                        "AI chat: {}",
                        if message.len() > 80 {
                            format!("{}...", &message[..80])
                        } else {
                            message.clone()
                        }
                    ),
                    author: Some("ai-chat".to_string()),
                    force_full: false,
                    branch: None,
                    operator: Some("ai".to_string()),
                    body: None,
                    is_checkpoint: false,
                    is_ai: true,
                });
                match commit {
                    Ok(c) => Some(c.to_snapshot),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Record user message
    store.add_message(&sid, "user", &message, snapshot_id.clone())?;
    store.save()?;

    // ---- AI API call ----
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
        // Record the error as a system message
        let error_msg = format!("AI API error ({}): {}", status.as_u16(), body_text);
        let _ = store.add_message(&sid, "system", &error_msg, None);
        let _ = store.save();
        return Err(anyhow!("{}", error_msg));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body_text).map_err(|e| anyhow!("Cannot parse AI response: {}", e))?;

    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("No content in AI response"))?;

    // Record AI response
    store.add_message(&sid, "ai", content, None)?;
    store.save()?;

    println!("{}", content);
    println!();
    println!("[session: {}]", sid);
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
        .args(["-C", &cwd.to_string_lossy(), "log", "--oneline", "-10"])
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

// ---------------------------------------------------------------------------
// Route Memory: context pipeline (assembles project memory + structure + causal chain)
// ---------------------------------------------------------------------------

/// Show the assembled AI context using route-memory and route-engine.
/// This is the primary way AI gets project context — reads memory and structure
/// first, then falls back to the engine if content exceeds token budget.
pub fn base_context(max_tokens: Option<usize>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let project_name = cwd
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default()
        .to_string();

    // Build memory store (in-memory for now; can be persisted later)
    let mut memory = route_memory::MemoryStore::new();
    memory.set(
        "route/version",
        "Route is a version control and code understanding tool. Not an information hub.",
        route_memory::MemoryTier::Core,
        vec!["route".to_string()],
    );
    memory.set(
        "route/principle",
        "Route data is protected. AI must read memory before searching code.",
        route_memory::MemoryTier::Core,
        vec!["route".to_string()],
    );

    // Scan project structure
    let structure = route_memory::ProjectStructure::scan(&cwd, 4)
        .map_err(|e| anyhow!("failed to scan project structure: {e}"))?;

    // Build causal chain (in-memory)
    let causal = route_memory::CausalChain::new();

    // Assemble context with token budget
    let config = route_memory::ContextConfig {
        max_tokens: max_tokens.unwrap_or(route_engine::TokenBudget::DEFAULT_MAX),
        ..Default::default()
    };
    let pipeline = route_memory::ContextPipeline::with_config(config);
    let ctx = pipeline.assemble(&memory, &structure, &causal, &project_name);

    println!("{}", ctx.formatted);
    println!();
    println!("--- Context Stats ---");
    println!(
        "  Token count: {} ({} budget)",
        ctx.token_count, config.max_tokens
    );
    println!("  Memory entries: {}", ctx.memory_entries);
    println!("  Structure entries: {}", ctx.structure_entries);
    println!("  Causal entries: {}", ctx.causal_entries);
    if ctx.truncated {
        println!("  ⚠ Context was truncated to fit token budget");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Route Guard: protection status
// ---------------------------------------------------------------------------

/// Show the Route Guard protection status.
pub fn guard_status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let active = route_core::is_route_active(&cwd);
    let guard = route_core::RouteGuard::new();

    println!("Route Guard Status");
    println!("==================");
    println!(
        "Active: {}",
        if active {
            "yes"
        } else {
            "no (run `route init` first)"
        }
    );
    println!();
    println!("Protected directories:");
    println!("  - .route/     (Route base configuration)");
    println!("  - .route-basic/ (Route basic mode data)");
    println!("  - .route-guard  (Route anchor file)");
    println!();
    println!("Check a path:");
    for dir in &[".route", ".route-basic", "src", "Cargo.toml"] {
        let target = cwd.join(dir);
        let result = guard.check(&cwd, &target);
        println!("  {} → {:?}", dir, result);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Conversation: record, list, show, rollback
// ---------------------------------------------------------------------------

fn conversation_store_path() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".route").join("conversations.json")
}

fn open_conversation_store() -> Result<route_memory::ConversationStore> {
    let path = conversation_store_path();
    Ok(route_memory::ConversationStore::with_path(path))
}

/// List all conversation sessions.
pub fn conversation_list() -> Result<()> {
    let store = open_conversation_store()?;
    let sessions = store.list_sessions();

    if sessions.is_empty() {
        println!("(no conversations — start one with `route conversation new <title>`)");
        return Ok(());
    }

    println!(
        "{:<32}  {:<20}  {:<6}  {}",
        "SESSION ID", "TITLE", "MSGS", "LAST ACTIVITY"
    );
    println!("{}", "-".repeat(90));
    for session in sessions {
        let time = chrono::DateTime::from_timestamp_millis(session.updated_at)
            .map(|t| t.format("%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            "{:<32}  {:<20}  {:<6}  {}",
            session.id, session.title, session.message_count, time
        );
    }
    Ok(())
}

/// Create a new conversation session.
pub fn conversation_new(title: &str) -> Result<()> {
    let mut store = open_conversation_store()?;
    let session = store.create_session(title)?;
    println!("✓ Created session '{}' (id: {})", session.title, session.id);
    Ok(())
}

/// Show details of a conversation session.
pub fn conversation_show(session_id: &str, limit: Option<usize>) -> Result<()> {
    let store = open_conversation_store()?;
    let session = match store.get_session(session_id) {
        Some(s) => s,
        None => {
            return Err(anyhow!("Session '{}' not found", session_id));
        }
    };

    println!("Session: {} (id: {})", session.title, session.id);
    println!(
        "Created: {:?}",
        chrono::DateTime::from_timestamp_millis(session.created_at)
    );
    println!("Messages: {}", session.message_count);
    println!("Tags: {:?}", session.tags);
    println!();

    let msgs = store.session_messages(session_id);
    let msgs = if let Some(limit) = limit {
        msgs.into_iter().rev().take(limit).rev().collect::<Vec<_>>()
    } else {
        msgs
    };

    for msg in &msgs {
        let time = chrono::DateTime::from_timestamp_millis(msg.created_at)
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_else(|| "?".to_string());
        let snapshot = msg
            .snapshot_id
            .as_ref()
            .map(|s| format!(" [snap: {}]", s))
            .unwrap_or_default();
        let role_tag = match msg.role.as_str() {
            "user" => "USER",
            "ai" => "AI",
            "system" => "SYS",
            _ => &msg.role,
        };
        println!("  [{:<4}] {} {}{}", role_tag, time, msg.content, snapshot);
        println!();
    }
    Ok(())
}

/// Record a message in a conversation session.
pub fn conversation_record(
    session_id: &str,
    role: &str,
    content: &str,
    snapshot_id: Option<String>,
) -> Result<()> {
    use route_basic::{ContextHistoryManifest, ContextSnapshot};
    let mut store = open_conversation_store()?;
    let cwd = current_project_root();
    let context_meta = match ContextSnapshot::collect(&cwd) {
        Ok(snap) => {
            let _ = ContextHistoryManifest::record_if_new(&cwd, &snap);
            Some((
                snap.fingerprint,
                snap.constitution_version,
                snap.protocol_revision,
                snap.reference_entries_hash,
            ))
        }
        Err(_) => None,
    };
    match store.add_message_with_context(session_id, role, content, snapshot_id, context_meta)? {
        Some(msg) => {
            println!(
                "✓ Recorded message '{}' in session '{}'",
                msg.id, session_id
            );
            if let Some(h) = &msg.context_hash {
                println!("  context_hash: {}", h);
            }
            Ok(())
        }
        None => Err(anyhow!("Session '{}' not found", session_id)),
    }
}

/// Rollback a conversation session to a specific message.
pub fn conversation_rollback(session_id: &str, message_id: &str) -> Result<()> {
    let mut store = open_conversation_store()?;
    let result = store.rollback_to_message(session_id, message_id, Some("CLI rollback"), None);

    if result.success {
        println!(
            "✓ Rolled back to message '{}' in session '{}'",
            message_id, session_id
        );
        println!("  Snapshot: {}", result.snapshot_id);
        println!("  Messages removed: {}", result.messages_removed);
        Ok(())
    } else {
        Err(anyhow!(
            "Rollback failed: {}",
            result.error.unwrap_or_default()
        ))
    }
}

/// Archive a conversation session.
pub fn conversation_archive(session_id: &str) -> Result<()> {
    let mut store = open_conversation_store()?;
    if store.archive_session(session_id)? {
        println!("✓ Archived session '{}'", session_id);
        Ok(())
    } else {
        Err(anyhow!("Session '{}' not found", session_id))
    }
}

/// Delete a conversation session.
pub fn conversation_delete(session_id: &str) -> Result<()> {
    let mut store = open_conversation_store()?;
    if store.delete_session(session_id)? {
        println!("✓ Deleted session '{}'", session_id);
        Ok(())
    } else {
        Err(anyhow!("Session '{}' not found", session_id))
    }
}

/// `route check` — verify repository integrity.
///
/// Opens the repository (which runs crash recovery first), then runs
/// [`BasicRepository::verify`] and prints a human-readable report. The
/// process exits non-zero when the repo is in a `Recoverable` or
/// `Corrupted` state so scripts can detect problems; `Ok` and
/// `Warning` exit zero.
pub fn check(full: bool, no_blobs: bool, json: bool) -> Result<()> {
    use route_basic::{VerifyOptions, VerifySeverity};

    let repo = open_repo()?;
    let opts = VerifyOptions {
        check_blob_existence: !no_blobs,
        verify_blob_content: full,
    };
    let report = repo.verify(&opts)?;

    let cwd = current_project_root();
    let ledger_findings = route_basic::check_execution_ledger(&cwd)?;

    if json {
        let findings: Vec<serde_json::Value> = report
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "severity": format!("{:?}", f.severity),
                    "code": f.code,
                    "detail": f.detail,
                })
            })
            .collect();
        let ledger: Vec<serde_json::Value> = ledger_findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "severity": f.severity,
                    "category": f.category,
                    "message": f.message,
                })
            })
            .collect();
        let output = serde_json::json!({
            "status": format!("{:?}", report.status),
            "snapshots_checked": report.snapshots_checked,
            "manifests_checked": report.manifests_checked,
            "branches_checked": report.branches_checked,
            "commits_checked": report.commits_checked,
            "transactions_checked": report.transactions_checked,
            "blobs_checked": report.blobs_checked,
            "findings": findings,
            "ledger_findings": ledger,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Route repository: {}", repo.project_path().display());
    println!(
        "Checked: {} snapshots, {} manifests, {} branches, {} commits, {} transactions, {} blobs",
        report.snapshots_checked,
        report.manifests_checked,
        report.branches_checked,
        report.commits_checked,
        report.transactions_checked,
        report.blobs_checked,
    );

    let label = match report.status {
        VerifySeverity::Ok => "OK",
        VerifySeverity::Warning => "WARNING",
        VerifySeverity::NeedsCleanup => "NEEDS_CLEANUP",
        VerifySeverity::Recoverable => "RECOVERABLE",
        VerifySeverity::Corrupted => "CORRUPTED",
        VerifySeverity::Unsupported => "UNSUPPORTED",
    };
    println!("Status: {label}");

    if report.findings.is_empty() {
        println!("No issues found.");
    } else {
        println!("Findings ({}):", report.findings.len());
        for f in &report.findings {
            let sev = match f.severity {
                VerifySeverity::Ok => "ok",
                VerifySeverity::Warning => "warn",
                VerifySeverity::NeedsCleanup => "cleanup",
                VerifySeverity::Recoverable => "recov",
                VerifySeverity::Corrupted => "corrupt",
                VerifySeverity::Unsupported => "unsup",
            };
            println!("  [{sev}] {}: {}", f.code, f.detail);
        }
    }

    if !ledger_findings.is_empty() {
        println!();
        println!("Execution Ledger ({}):", ledger_findings.len());
        for f in &ledger_findings {
            println!("  [{}] {}: {}", f.severity, f.category, f.message);
        }
    }

    // Non-zero exit for states that need user action, so CI / scripts
    // can detect a repo that is not fully trusted.
    match report.status {
        VerifySeverity::Ok | VerifySeverity::Warning | VerifySeverity::NeedsCleanup => Ok(()),
        VerifySeverity::Recoverable => Err(anyhow!(
            "repository is in a RECOVERABLE state — re-open or run recovery to converge"
        )),
        VerifySeverity::Corrupted => Err(anyhow!(
            "repository is CORRUPTED — manual intervention required"
        )),
        VerifySeverity::Unsupported => Err(anyhow!(
            "repository format is UNSUPPORTED — this build cannot read this repository"
        )),
    }
}

/// `route repair-plan` — generate a repair plan from current check findings.
pub fn repair_plan(json: bool) -> Result<()> {
    use route_basic::{VerifyOptions, VerifySeverity};
    let repo = open_repo()?;
    let opts = VerifyOptions {
        check_blob_existence: true,
        verify_blob_content: false,
    };
    let report = repo.verify(&opts)?;

    if json {
        let findings: Vec<serde_json::Value> = report
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "severity": format!("{:?}", f.severity),
                    "code": f.code,
                    "detail": f.detail,
                    "repair": match f.code.as_str() {
                        "missing_snapshot" => "Run `route commit` to create a snapshot",
                        "orphan_blob" => "Run `route check --full` to reconcile",
                        "broken_manifest" => "Run `route snapshot` to rebuild manifest",
                        "stale_evidence" => "Re-run verification evidence commands",
                        "incomplete_session" => "Run `route task end` to close the session",
                        _ => "Manual review recommended",
                    },
                })
            })
            .collect();
        let output = serde_json::json!({
            "status": format!("{:?}", report.status),
            "snapshots_checked": report.snapshots_checked,
            "snapshots_total": report.snapshots_checked,
            "findings": findings,
            "finding_count": report.findings.len(),
            "repairable": report.findings.iter().any(|f| {
                matches!(f.severity, VerifySeverity::Warning | VerifySeverity::NeedsCleanup)
            }),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    // Human-readable output
    println!("Repair Plan");
    println!("{}", "-".repeat(60));
    println!("Status: {:?}", report.status);
    println!("Snapshots checked: {}", report.snapshots_checked);
    println!("Findings: {}", report.findings.len());
    if report.findings.is_empty() {
        println!("No issues found — repair not needed.");
    } else {
        println!();
        for f in &report.findings {
            let sev = match f.severity {
                VerifySeverity::Ok => "ok",
                VerifySeverity::Warning => "warn",
                VerifySeverity::NeedsCleanup => "cleanup",
                VerifySeverity::Recoverable => "recov",
                VerifySeverity::Corrupted => "corrupt",
                VerifySeverity::Unsupported => "unsup",
            };
            let repair = match f.code.as_str() {
                "missing_snapshot" => "Run `route commit` to create a snapshot",
                "orphan_blob" => "Run `route check --full` to reconcile",
                "broken_manifest" => "Run `route snapshot` to rebuild manifest",
                "stale_evidence" => "Re-run verification evidence commands",
                "incomplete_session" => "Run `route task end` to close the session",
                _ => "Manual review recommended",
            };
            println!("  [{sev}] {}: {}", f.code, f.detail);
            println!("        -> {repair}");
        }
    }
    Ok(())
}

// ---------------- Evolution Core commands ----------------

/// `route evolve propose` — record a candidate experiment (candidate-first).
pub fn evolve_propose(
    target: String,
    hypothesis: String,
    baseline_id: String,
    candidate_id: String,
    changed_scope: Vec<String>,
    rationale: String,
    generated_by: String,
    reversible_save_id: String,
    reference_ids: Vec<String>,
    studio_ids: Vec<String>,
    task_id: Option<String>,
    session_id: Option<String>,
    context_hash: Option<String>,
    harness: Option<String>,
) -> Result<()> {
    use route_basic::evolution::*;
    let cwd = current_project_root();
    let mut store = EvolutionStore::load(&cwd)?;

    let target_enum = match target.as_str() {
        "project" => EvolutionTarget::Project,
        "workflow" => EvolutionTarget::Workflow,
        "context" => EvolutionTarget::Context,
        "harness" => EvolutionTarget::Harness,
        "route" => EvolutionTarget::Route,
        _ => anyhow::bail!("invalid target '{}'", target),
    };

    // Build a benchmark suite: fixed baseline + holdout (both authoritative,
    // immutable), plus reference-derived cases from the external anchors.
    let mut cases = vec![
        BenchmarkCase {
            id: "fixed-correctness".to_string(),
            name: "fixed correctness".to_string(),
            origin: BenchmarkOrigin::FixedBaseline,
            scope: "project".to_string(),
            expected: "pass".to_string(),
            trust: "high".to_string(),
            mutable: false,
            evidence: vec![],
            reference_id: None,
            studio: None,
            simcise_check: false,
        },
        BenchmarkCase {
            id: "holdout-regression".to_string(),
            name: "holdout regression".to_string(),
            origin: BenchmarkOrigin::Holdout,
            scope: "project".to_string(),
            expected: "pass".to_string(),
            trust: "high".to_string(),
            mutable: false,
            evidence: vec![],
            reference_id: None,
            studio: None,
            simcise_check: false,
        },
    ];
    for ref_id in &reference_ids {
        if let Some(reference) = load_reference(&cwd, ref_id) {
            cases.push(derive_benchmark_from_reference(
                &reference,
                &format!("anchor-{}", ref_id),
                "must not regress reference constraint",
            ));
        }
    }
    let suite = BenchmarkSuite {
        id: route_core::new_id(),
        name: format!("suite-{}", candidate_id),
        generated_by: "cli-propose".to_string(),
        cases,
    };

    let manifest = CandidateManifest {
        candidate_id: candidate_id.clone(),
        baseline_id: baseline_id.clone(),
        changed_scope,
        rationale: rationale.clone(),
        generated_by,
        input_context_hash: route_core::sha256_hex(hypothesis.as_bytes()),
        reversible_save_id: Some(reversible_save_id.clone()),
        parent_candidate: None,
    };

    let exp = propose_experiment(
        &cwd,
        &mut store,
        target_enum,
        hypothesis,
        baseline_id,
        candidate_id,
        manifest,
        suite,
        reference_ids,
        studio_ids,
        task_id,
        session_id,
        context_hash,
        harness,
    )?;

    println!("✓ Proposed evolution experiment");
    println!("  id:        {}", exp.id);
    println!("  target:    {}", exp.target.as_str());
    println!("  status:    {}", exp.status.as_str());
    println!("  candidate: {}", exp.candidate_id);
    println!("  baseline:  {}", exp.baseline_id);
    println!(
        "  save:      {}",
        exp.manifest.reversible_save_id.as_deref().unwrap_or("none")
    );
    println!("  references: {}", exp.reference_ids.len());
    println!(
        "  studios:   {}",
        if exp.studio_ids.is_empty() {
            "none".to_string()
        } else {
            exp.studio_ids.join(",")
        }
    );
    if let Some(t) = &exp.task_id {
        println!("  task:      {}", t);
    }
    if let Some(s) = &exp.session_id {
        println!("  session:   {}", s);
    }
    if let Some(h) = &exp.harness_provenance {
        println!("  harness:   {}", h);
    }
    println!();
    println!(
        "Next: `route evolve evaluate {}` then `route evolve promote {} --label ...`",
        &exp.id[..8],
        &exp.id[..8]
    );
    Ok(())
}

/// `route evolve show [--explain]` — inspect an experiment and its causal chain.
pub fn evolve_show(id: String, explain: bool, json: bool) -> Result<()> {
    use route_basic::evolution::*;
    let cwd = current_project_root();
    let store = EvolutionStore::load(&cwd)?;
    let exp = store
        .get(&id)
        .ok_or_else(|| anyhow!("experiment '{}' not found", id))?;

    if json {
        let output = serde_json::json!({
            "id": exp.id,
            "target": exp.target.as_str(),
            "hypothesis": exp.hypothesis,
            "baseline_id": exp.baseline_id,
            "candidate_id": exp.candidate_id,
            "status": exp.status.as_str(),
            "decision": exp.decision.map(|d| d.as_str()),
            "decision_reason": exp.decision_reason,
            "aio_flagged": exp.aio_flagged,
            "simcise_score": exp.simcise_score,
            "reference_ids": exp.reference_ids,
            "studio_ids": exp.studio_ids,
            "metrics": exp.metrics.iter().map(|m| serde_json::json!({
                "name": m.name,
                "baseline": m.baseline,
                "candidate": m.candidate,
                "unit": m.unit,
                "higher_is_better": m.higher_is_better,
            })).collect::<Vec<_>>(),
            "evidence": exp.evidence,
            "reversible_save_id": exp.manifest.reversible_save_id,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Evolution Experiment");
    println!("{}", "-".repeat(60));
    println!("  id:        {}", exp.id);
    println!("  target:    {}", exp.target.as_str());
    println!("  status:    {}", exp.status.as_str());
    println!("  hypothesis: {}", exp.hypothesis);
    println!("  candidate: {}", exp.candidate_id);
    println!("  baseline:  {}", exp.baseline_id);
    println!(
        "  save:      {}",
        exp.manifest.reversible_save_id.as_deref().unwrap_or("none")
    );
    if let Some(d) = exp.decision {
        println!("  decision:  {}", d.as_str());
    }
    if let Some(r) = &exp.decision_reason {
        println!("  reason:    {}", r);
    }
    if exp.aio_flagged {
        println!("  ⚠ AIO FLAGGED: candidate crosses too many concerns");
    }
    if let Some(s) = exp.simcise_score {
        println!("  simcise:   {:.2}", s);
    }
    if !exp.metrics.is_empty() {
        println!("  metrics:");
        for m in &exp.metrics {
            println!(
                "    {}: {} -> {} {}",
                m.name,
                m.baseline
                    .map(|v| format!("{:.3}", v))
                    .unwrap_or_else(|| "?".to_string()),
                m.candidate
                    .map(|v| format!("{:.3}", v))
                    .unwrap_or_else(|| "?".to_string()),
                m.unit
            );
        }
    }

    if explain {
        println!();
        println!("=== EXPLAIN (P11) ===");
        println!("Why proposed?: {}", exp.hypothesis);
        println!(
            "What changed?: scopes = {}",
            exp.manifest.changed_scope.join(", ")
        );
        println!(
            "Baseline:     {} (rationale: {})",
            exp.baseline_id, exp.manifest.rationale
        );
        println!("Generated by: {}", exp.manifest.generated_by);
        println!(
            "Benchmarks:   suite {} ({} cases)",
            exp.benchmark_suite_id,
            store
                .suite(&exp.benchmark_suite_id)
                .map(|s| s.cases.len())
                .unwrap_or(0)
        );
        println!(
            "References:   {}",
            if exp.reference_ids.is_empty() {
                "(none)".to_string()
            } else {
                exp.reference_ids.join(", ")
            }
        );
        println!(
            "Decision:     {}",
            exp.decision
                .map(|d| d.as_str())
                .unwrap_or("(not evaluated)")
        );
        if let Some(r) = &exp.decision_reason {
            println!("  reason: {}", r);
        }
        println!(
            "Recovery:     reversible_save = {}",
            exp.manifest
                .reversible_save_id
                .as_deref()
                .unwrap_or("MISSING")
        );
    }
    Ok(())
}

/// Parse a harness metric arg `name=baseline:candidate[:hib]` into a Metric.
pub fn parse_metric_arg(arg: &str) -> Result<route_basic::evolution::Metric> {
    use route_basic::evolution::Metric;
    let (name, rest) = arg.split_once('=').ok_or_else(|| {
        anyhow!(
            "metric format: name=baseline:candidate[:hib], got '{}'",
            arg
        )
    })?;
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() < 2 {
        anyhow::bail!("metric '{}' needs baseline:candidate", arg);
    }
    let parse_f = |s: &str| -> Result<f64> {
        s.trim()
            .parse::<f64>()
            .with_context(|| format!("invalid number '{}' in metric '{}'", s, arg))
    };
    let baseline = parse_f(parts[0])?;
    let candidate = parse_f(parts[1])?;
    let higher_is_better = if let Some(h) = parts.get(2) {
        h.trim() != "0"
    } else {
        true
    };
    Ok(Metric {
        name: name.trim().to_string(),
        baseline: Some(baseline),
        candidate: Some(candidate),
        unit: String::new(),
        higher_is_better,
    })
}

/// `route evolve evaluate` — run the benchmark suite, produce a recommendation.
/// Never promotes. Promotion is gated (P3/P5).
pub fn evolve_evaluate(id: String, json: bool, metric_args: Vec<String>) -> Result<()> {
    use route_basic::evolution::*;
    use route_basic::VerifyOptions;
    let cwd = current_project_root();
    let mut store = EvolutionStore::load(&cwd)?;

    // Real, deterministic evaluation: run repository verification as the
    // authoritative correctness signal for the candidate.
    let repo = open_repo()?;
    let verify = repo.verify(&VerifyOptions {
        check_blob_existence: true,
        verify_blob_content: false,
    });
    let verified = matches!(
        verify,
        Ok(r) if {
            let ok = matches!(r.status, route_basic::VerifySeverity::Ok);
            let no_hard_issues = !r.findings.iter().any(|f| {
                !matches!(f.severity, route_basic::VerifySeverity::Ok | route_basic::VerifySeverity::Warning)
            });
            ok || no_hard_issues
        }
    );

    // Build baseline (always passes — it is the KnownGood) and candidate results.
    let suite_id = store.get(&id).map(|e| e.benchmark_suite_id.clone());
    let suite = suite_id
        .as_ref()
        .and_then(|sid| store.suite(sid).cloned())
        .unwrap_or_else(default_eval_suite);

    let baseline: Vec<BenchmarkResult> = suite
        .cases
        .iter()
        .map(|c| BenchmarkResult {
            case_id: c.id.clone(),
            name: c.name.clone(),
            origin: c.origin,
            baseline_passed: true,
            candidate_passed: verified,
            normalized: !c.mutable && c.origin.authoritative(),
            note: String::new(),
        })
        .collect();

    // Correctness signal from real verification, plus any harness-measured
    // metrics reported via --metric (P5 / P8).
    let mut metrics = vec![
        Metric {
            name: "correctness".to_string(),
            baseline: Some(1.0),
            candidate: Some(if verified { 1.0 } else { 0.0 }),
            unit: "".to_string(),
            higher_is_better: true,
        },
        Metric {
            name: "verification_pass".to_string(),
            baseline: Some(1.0),
            candidate: Some(if verified { 1.0 } else { 0.0 }),
            unit: "".to_string(),
            higher_is_better: true,
        },
    ];
    for arg in &metric_args {
        metrics.push(parse_metric_arg(arg)?);
    }

    let policy = PromotionPolicy::default();
    let exp = evaluate_experiment(
        &cwd, &mut store, &id, &baseline, &baseline, metrics, &policy,
    )?;

    if json {
        let output = serde_json::json!({
            "id": exp.id,
            "status": exp.status.as_str(),
            "decision": exp.decision.map(|d| d.as_str()),
            "decision_reason": exp.decision_reason,
            "verified": verified,
            "metrics": exp.metrics.iter().map(|m| serde_json::json!({
                "name": m.name,
                "baseline": m.baseline,
                "candidate": m.candidate,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Evaluation for experiment {}", &exp.id[..8]);
    println!("{}", "-".repeat(60));
    println!(
        "  Repository verify: {}",
        if verified { "OK" } else { "FAILED" }
    );
    println!(
        "  Decision:         {}",
        exp.decision.map(|d| d.as_str()).unwrap_or("(none)")
    );
    if let Some(r) = &exp.decision_reason {
        println!("  Reason:           {}", r);
    }
    println!();
    println!("  NOTE: this is a RECOMMENDATION only. Promotion is gated.");
    println!("  Run `route evolve promote <id> --label <name>` to promote (if allowed).");
    Ok(())
}

fn default_eval_suite() -> route_basic::evolution::BenchmarkSuite {
    use route_basic::evolution::*;
    BenchmarkSuite {
        id: route_core::new_id(),
        name: "default-eval".to_string(),
        generated_by: "cli".to_string(),
        cases: vec![
            BenchmarkCase {
                id: "fixed-correctness".to_string(),
                name: "fixed correctness".to_string(),
                origin: BenchmarkOrigin::FixedBaseline,
                scope: "project".to_string(),
                expected: "pass".to_string(),
                trust: "high".to_string(),
                mutable: false,
                evidence: vec![],
                reference_id: None,
                studio: None,
                simcise_check: false,
            },
            BenchmarkCase {
                id: "holdout-regression".to_string(),
                name: "holdout regression".to_string(),
                origin: BenchmarkOrigin::Holdout,
                scope: "project".to_string(),
                expected: "pass".to_string(),
                trust: "high".to_string(),
                mutable: false,
                evidence: vec![],
                reference_id: None,
                studio: None,
                simcise_check: false,
            },
        ],
    }
}

/// `route evolve promote` — gated promotion to KnownGood.
pub fn evolve_promote(id: String, label: String) -> Result<()> {
    use route_basic::evolution::*;
    let cwd = current_project_root();
    let mut store = EvolutionStore::load(&cwd)?;
    let policy = PromotionPolicy::default();
    let exp = promote_experiment(&cwd, &mut store, &id, &label, &policy)?;
    println!("✓ Promoted to KnownGood");
    println!("  experiment: {}", exp.id);
    println!("  label:      {}", label);
    println!(
        "  known_good: {}",
        store
            .known_good
            .as_ref()
            .map(|k| k.id.clone())
            .unwrap_or_default()
    );
    Ok(())
}

/// `route evolve reject` — reject a candidate (stable untouched).
pub fn evolve_reject(id: String, reason: Option<String>) -> Result<()> {
    use route_basic::evolution::*;
    let cwd = current_project_root();
    let mut store = EvolutionStore::load(&cwd)?;
    let exp = reject_experiment(&cwd, &mut store, &id, reason.as_deref())?;
    println!("✗ Rejected experiment {}", exp.id);
    println!("  stable state untouched");
    Ok(())
}

/// `route evolve history` — list experiments + KnownGood chain.
pub fn evolve_history() -> Result<()> {
    use route_basic::evolution::*;
    let cwd = current_project_root();
    let store = EvolutionStore::load(&cwd)?;
    println!("KnownGood chain:");
    for kg in &store.known_good_history {
        println!(
            "  {}  {}  (from experiment {})",
            &kg.id[..8],
            kg.label,
            kg.from_experiment.as_deref().unwrap_or("-")
        );
    }
    if let Some(kg) = store.current_known_good() {
        println!("  → current: {} ({})", &kg.id[..8], kg.label);
    }
    println!();
    println!("Experiments:");
    for e in &store.experiments {
        println!(
            "  {}  [{}] target={} decision={}",
            &e.id[..8],
            e.status.as_str(),
            e.target.as_str(),
            e.decision.map(|d| d.as_str()).unwrap_or("-")
        );
    }
    Ok(())
}

// ---------------- Emergence Hardening (P13–P23) ----------------

fn load_emergence() -> Result<route_basic::emergence::EmergenceStore> {
    let cwd = current_project_root();
    route_basic::emergence::EmergenceStore::load(&cwd)
}

fn save_emergence(store: &route_basic::emergence::EmergenceStore) -> Result<()> {
    let cwd = current_project_root();
    store.save(&cwd)
}

fn print_json<T: serde::Serialize>(json: bool, val: &T) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(val)?);
    }
    Ok(())
}

/// `route emerge status` — experimental status, blackboard, champion counts.
pub fn emerge_status(json: bool) -> Result<()> {
    let store = load_emergence()?;
    if json {
        print_json(true, &store)?;
        return Ok(());
    }
    println!(
        "Emergence (experimental): {}",
        if store.experimental_enabled {
            "enabled"
        } else {
            "disabled (default)"
        }
    );
    println!("  blackboard events: {}", store.blackboard.events.len());
    println!("  champions:         {}", store.champions.len());
    println!("  novelty archive:   {}", store.novelty_archive.len());
    println!("  innovations:       {}", store.innovations.len());
    println!("  bench court:       {}", store.bench_court.len());
    println!("  failure patterns:  {}", store.failure_patterns.len());
    println!("  runs:              {}", store.runs.len());
    Ok(())
}

/// `route emerge enable` — explicitly opt-in to experimental capability.
pub fn emerge_enable() -> Result<()> {
    let mut store = load_emergence()?;
    store.set_experimental_enabled(true);
    save_emergence(&store)?;
    println!("emergence enabled (experimental)");
    Ok(())
}

/// `route emerge event` — append a blackboard event through the Engine.
pub fn emerge_event(
    actor: String,
    source: String,
    confidence: f64,
    session_id: Option<String>,
    task_id: Option<String>,
) -> Result<()> {
    use route_basic::emergence::*;
    let mut store = load_emergence()?;
    let ev = BlackboardEvent {
        id: route_core::new_id(),
        ts: route_core::now_millis(),
        actor,
        plane: Plane::Explore,
        harness: None,
        session_id,
        task_id,
        source,
        confidence,
        evidence: vec![],
        payload: serde_json::json!({}),
    };
    engine_project_event(&mut store, ev)?;
    save_emergence(&store)?;
    println!("blackboard event recorded");
    Ok(())
}

/// `route emerge run` — show the search budget / stop state of the latest run.
pub fn emerge_run(json: bool) -> Result<()> {
    use route_basic::emergence::*;
    let store = load_emergence()?;
    let now = route_core::now_millis();
    if let Some(run) = store.runs.last() {
        if json {
            let v = serde_json::json!({
                "id": run.id,
                "stop": should_stop(run, now).as_str(),
                "candidates": run.candidates,
                "rounds": run.rounds,
                "failures": run.failures,
                "novelty_plateau": run.novelty_plateau,
                "budget": run.budget,
            });
            print_json(true, &v)?;
        } else {
            println!(
                "run {}  stop={}  candidates={} rounds={} failures={} plateau={}",
                &run.id[..8],
                should_stop(run, now).as_str(),
                run.candidates,
                run.rounds,
                run.failures,
                run.novelty_plateau
            );
        }
    } else {
        println!("(no evolution run recorded)");
    }
    Ok(())
}

/// `route emerge court` — show the benchmark court ledger.
pub fn emerge_court(json: bool) -> Result<()> {
    let store = load_emergence()?;
    if json {
        print_json(true, &store.bench_court)?;
        return Ok(());
    }
    if store.bench_court.is_empty() {
        println!("(benchmark court empty)");
        return Ok(());
    }
    for e in &store.bench_court {
        println!(
            "  {}  lifecycle={:?}  origin={:?}  produced_by={}",
            &e.id[..8],
            e.lifecycle,
            e.case.origin,
            e.produced_by.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

/// `route emerge novelty` — show the high-novelty archive.
pub fn emerge_novelty(json: bool) -> Result<()> {
    let store = load_emergence()?;
    if json {
        print_json(true, &store.novelty_archive)?;
        return Ok(());
    }
    if store.novelty_archive.is_empty() {
        println!("(novelty archive empty)");
        return Ok(());
    }
    for n in &store.novelty_archive {
        println!(
            "  {}  candidate={}  novelty={:.2} quality={:.2} decision={}",
            &n.id[..8],
            &n.candidate_id[..n.candidate_id.len().min(8)],
            n.novelty_score,
            n.quality_score,
            n.decision.map(|d| d.as_str()).unwrap_or("-")
        );
    }
    Ok(())
}

/// `route evolve explain <id>` — emergence lineage (P24).
pub fn evolve_explain(id: String, json: bool) -> Result<()> {
    use route_basic::evolution::EvolutionStore;
    let cwd = current_project_root();
    let store = EvolutionStore::load(&cwd)?;
    let exp = store
        .get(&id)
        .ok_or_else(|| anyhow!("experiment '{}' not found", id))?;

    if json {
        let v = serde_json::json!({
            "id": exp.id,
            "parent_candidate": exp.manifest.parent_candidate,
            "mutation": exp.manifest.rationale,
            "hypothesis": exp.hypothesis,
            "generated_by": exp.manifest.generated_by,
            "references": exp.reference_ids,
            "novelty": exp.novelty_score,
            "quality": exp.quality_score,
            "decision": exp.decision.map(|d| d.as_str()),
            "decision_reason": exp.decision_reason,
            "recovery": exp.manifest.reversible_save_id,
            "benchmark_suite": exp.benchmark_suite_id,
            "replication": exp.replication_count,
            "ablation_passed": exp.ablation_passed,
        });
        print_json(true, &v)?;
        return Ok(());
    }

    println!("=== EMERGENCE EXPLAIN (P24) ===");
    println!(
        "Parent candidate: {}",
        exp.manifest.parent_candidate.as_deref().unwrap_or("(root)")
    );
    println!("Mutation:         {}", exp.manifest.rationale);
    println!("Hypothesis:       {}", exp.hypothesis);
    println!("Generated by:     {}", exp.manifest.generated_by);
    println!(
        "References:       {}",
        if exp.reference_ids.is_empty() {
            "(none)".to_string()
        } else {
            exp.reference_ids.join(", ")
        }
    );
    println!("Novelty:          {:.2}", exp.novelty_score);
    println!("Quality:          {:.2}", exp.quality_score);
    println!("Benchmark suite:  {}", exp.benchmark_suite_id);
    println!(
        "Decision:         {}",
        exp.decision
            .map(|d| d.as_str())
            .unwrap_or("(not evaluated)")
    );
    if let Some(r) = &exp.decision_reason {
        println!("  reason:         {}", r);
    }
    println!(
        "Replicated:       {} (threshold for strategy: cross-task or ablation)",
        exp.replication_count
    );
    println!("Ablation passed:  {}", exp.ablation_passed);
    println!(
        "Recovery:         reversible_save = {}",
        exp.manifest
            .reversible_save_id
            .as_deref()
            .unwrap_or("MISSING")
    );
    Ok(())
}

// ---------------- Campaign governance (P26–P43) ----------------

/// Create a new campaign in PLANNED status (P26).
pub fn campaign_create(
    goal: String,
    scope: String,
    strategy: String,
    task_id: Option<String>,
) -> Result<()> {
    use route_basic::campaign::{CampaignStatus, CampaignStore, EvolutionCampaign};
    let cwd = current_project_root();
    let mut store = CampaignStore::load(&cwd)?;
    let now = route_core::now_millis();
    let id = route_core::new_id();
    let campaign = EvolutionCampaign {
        id: id.clone(),
        goal,
        scope,
        strategy,
        budget: Default::default(),
        experiment_ids: vec![],
        champion: None,
        challengers: vec![],
        status: CampaignStatus::Planned,
        stop_reason: None,
        summary: String::new(),
        task_id,
        created_at: now,
        updated_at: now,
    };
    store.campaigns.push(campaign);
    store.save(&cwd)?;
    println!("created campaign {}", id);
    Ok(())
}

/// Show campaign status (P43 machine-readable with --json).
pub fn campaign_status(id: String, json: bool) -> Result<()> {
    use route_basic::campaign::CampaignStore;
    let cwd = current_project_root();
    let store = CampaignStore::load(&cwd)?;
    let c = store
        .campaigns
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("campaign '{}' not found", id))?;
    if json {
        let v = serde_json::json!({
            "id": c.id,
            "goal": c.goal,
            "scope": c.scope,
            "strategy": c.strategy,
            "task_id": c.task_id,
            "status": c.status.as_str(),
            "experiment_count": c.experiment_ids.len(),
            "budget": {
                "max_experiments": c.budget.max_experiments,
                "max_failed_experiments": c.budget.max_failed_experiments,
                "max_wall_time_ms": c.budget.max_wall_time_ms,
                "max_tokens": c.budget.max_tokens,
                "max_cost": c.budget.max_cost,
                "disk_budget": c.budget.disk_budget,
                "max_parallel_external": c.budget.max_parallel_external,
            },
            "champion": c.champion,
            "challengers": c.challengers,
            "stop_reason": c.stop_reason,
        });
        print_json(true, &v)?;
        return Ok(());
    }
    println!("Campaign {}", c.id);
    println!("{}", "-".repeat(60));
    println!("  goal:     {}", c.goal);
    println!("  scope:    {}", c.scope);
    println!("  strategy: {}", c.strategy);
    println!("  task:     {}", c.task_id.as_deref().unwrap_or("(none)"));
    println!("  status:   {}", c.status.as_str());
    println!(
        "  experiments: {} / {}",
        c.experiment_ids.len(),
        c.budget.max_experiments
    );
    println!("  champion: {}", c.champion.as_deref().unwrap_or("(none)"));
    if let Some(r) = &c.stop_reason {
        println!("  stop:     {}", r);
    }
    Ok(())
}

/// Compute the machine-readable next_action contract for an unattended
/// campaign (P36/P43/P1-P2). `cap` is the executing harness's declared
/// capability list; if it cannot satisfy the action, the action degrades to
/// needs_human with `satisfiable = false` — Route never pretends execution is
/// possible.
pub fn campaign_next(id: String, json: bool, cap: Vec<String>) -> Result<()> {
    use route_basic::campaign::{build_next_action_contract, CampaignStore, NextAction};
    let cwd = current_project_root();
    let store = CampaignStore::load(&cwd)?;
    let c = store
        .campaigns
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("campaign '{}' not found", id))?;
    let now = route_core::now_millis();
    let failed_count = 0;
    let monoculture = false;
    let has_candidate = !c.experiment_ids.is_empty();
    let candidate_needs_ablation = false;
    let evidence_sufficient = false;
    let available: Vec<&str> = cap.iter().map(|s| s.as_str()).collect();
    let contract = build_next_action_contract(
        c,
        now,
        failed_count,
        monoculture,
        has_candidate,
        candidate_needs_ablation,
        evidence_sufficient,
        &available,
    );
    if json {
        let v = serde_json::json!({
            "campaign_id": contract.campaign_id,
            "task_id": contract.task_id,
            "experiment_id": contract.experiment_id,
            "status": c.status.as_str(),
            "next_action": contract.action.as_str(),
            "goal": contract.goal,
            "scope": contract.scope,
            "required_capabilities": contract.required_capabilities,
            "context_required": contract.context_required,
            "checkpoint_required": contract.checkpoint_required,
            "candidate_isolation_required": contract.candidate_isolation_required,
            "expected_evidence": contract.expected_evidence,
            "budget_remaining_experiments": contract.budget_remaining_experiments,
            "budget_remaining_wall_ms": contract.budget_remaining_wall_ms,
            "report_contract": contract.report_contract,
            "satisfiable": contract.satisfiable,
            "missing_capabilities": contract.missing_capabilities,
            "message": match contract.action {
                NextAction::ExecuteExperiment => "propose a new experiment within budget",
                NextAction::Diversify => "request holder to explore a different direction",
                NextAction::Replicate => "request a different run to replicate the candidate",
                NextAction::Ablate => "design an ablation study for the candidate",
                NextAction::Stop => "campaign is in a terminal state or budget exhausted",
                NextAction::NeedsHuman => "insufficient evidence; needs a human decision",
            },
        });
        print_json(true, &v)?;
        return Ok(());
    }
    println!(
        "campaign {} next_action: {}",
        c.id,
        contract.action.as_str()
    );
    if !contract.missing_capabilities.is_empty() {
        println!(
            "  degraded: missing capabilities {}",
            contract.missing_capabilities.join(", ")
        );
    }
    Ok(())
}

/// Report / explain / ingest outcome for a campaign (P42 / P4 / P11).
///
/// - `json`: machine-readable report on stdout (no human diagnostics leaked).
/// - `explain`: prose explanation derived from persisted data (P11).
/// - `outcome`: ingest a harness execution outcome as UNTRUSTED input. It is
///   recorded with `trusted = false` and never promotes anything by itself;
///   Route resolves trust through its Evidence hierarchy (P4). Idempotent:
///   exact re-submission returns `duplicate`; a disagreeing duplicate returns
///   `conflict` and records conflict evidence (P9).
pub fn campaign_report(
    id: String,
    json: bool,
    explain: bool,
    outcome: Option<String>,
) -> Result<()> {
    use route_basic::campaign::{
        task_recommendation, CampaignReport, CampaignStore, IngestReportResult, TaskRecommendation,
    };

    // Ingest path: mutating, must reload + save.
    if let Some(outcome_raw) = outcome {
        let mut store = CampaignStore::load(&cwd()?)?;
        let report: CampaignReport = serde_json::from_str(&outcome_raw)
            .map_err(|e| anyhow!("invalid --outcome JSON: {}", e))?;
        if report.campaign_id != id {
            anyhow::bail!(
                "outcome.campaign_id '{}' does not match requested '{}'",
                report.campaign_id,
                id
            );
        }
        let result = store.ingest_report(&report);
        store.save(&cwd()?)?;
        let status_str = match result {
            IngestReportResult::Recorded => "recorded",
            IngestReportResult::Duplicate => "duplicate",
            IngestReportResult::Conflict => "conflict",
        };
        if json {
            let v = serde_json::json!({
                "campaign_id": report.campaign_id,
                "experiment_id": report.experiment_id,
                "execution_status": report.execution_status,
                "trusted": false,
                "ingest": status_str,
                "report_count": store.reports.len(),
                "conflict_evidence_count": store.conflict_evidence.len(),
            });
            print_json(true, &v)?;
            return Ok(());
        }
        println!("campaign {} report: {}", id, status_str);
        println!("  execution_status: {}", report.execution_status);
        println!(
            "  trusted: false (harness report is UNTRUSTED input; resolve via route evidence)"
        );
        println!("  experiment_id: {}", report.experiment_id);
        return Ok(());
    }

    let store = CampaignStore::load(&cwd()?)?;
    let c = store
        .campaigns
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow!("campaign '{}' not found", id))?;

    if explain {
        let rec = task_recommendation(c);
        let rec_str = match rec {
            TaskRecommendation::None => "none — campaign not linked or still exploring",
            TaskRecommendation::RecommendComplete => {
                "recommend task completion (task must still pass its own verification)"
            }
            TaskRecommendation::RecommendContinue => {
                "recommend continue — no accepted solution yet, task stays actionable"
            }
            TaskRecommendation::RecommendReview => {
                "recommend review — campaign exhausted without accepted solution, task history intact"
            }
            TaskRecommendation::RecommendAbort => {
                "recommend abort — campaign aborted, task survives"
            }
        };
        let mut out = String::new();
        out.push_str(&format!("=== CAMPAIGN EXPLAIN (P11) ===\n"));
        out.push_str(&format!(
            "What started this campaign: task '{}' (goal: {})\n",
            c.task_id.as_deref().unwrap_or("(none)"),
            c.goal
        ));
        out.push_str(&format!(
            "What was attempted: {} candidate experiment(s)\n",
            c.experiment_ids.len()
        ));
        out.push_str(&format!(
            "Champion: {}\n",
            c.champion.as_deref().unwrap_or("(none)")
        ));
        out.push_str(&format!(
            "Harness reports ingested: {} (all UNTRUSTED until resolved)\n",
            store.reports.len()
        ));
        let trusted = store.reports.iter().filter(|r| r.trusted).count();
        out.push_str(&format!(
            "Trusted reports: {} (resolved via Route evidence)\n",
            trusted
        ));
        out.push_str(&format!(
            "Conflict evidence: {}\n",
            store.conflict_evidence.len()
        ));
        out.push_str(&format!("Status: {}\n", c.status.as_str()));
        if let Some(r) = &c.stop_reason {
            out.push_str(&format!("Why stopped: {}\n", r));
        }
        out.push_str(&format!("Task recommendation: {}\n", rec_str));
        out.push_str(&format!(
            "What happens to the parent task: campaign never silently marks it Succeeded; it only produces a recommendation. The task must still pass its own verification.\n"
        ));
        if json {
            let v = serde_json::json!({
                "campaign_id": c.id,
                "task_id": c.task_id,
                "goal": c.goal,
                "attempted": c.experiment_ids.len(),
                "champion": c.champion,
                "status": c.status.as_str(),
                "stop_reason": c.stop_reason,
                "reports_ingested": store.reports.len(),
                "trusted_reports": trusted,
                "conflict_evidence": store.conflict_evidence.len(),
                "task_recommendation": match rec {
                    TaskRecommendation::None => "none",
                    TaskRecommendation::RecommendComplete => "recommend_complete",
                    TaskRecommendation::RecommendContinue => "recommend_continue",
                    TaskRecommendation::RecommendReview => "recommend_review",
                    TaskRecommendation::RecommendAbort => "recommend_abort",
                },
            });
            print_json(true, &v)?;
            return Ok(());
        }
        print!("{}", out);
        return Ok(());
    }

    if json {
        let v = serde_json::json!({
            "id": c.id,
            "goal": c.goal,
            "scope": c.scope,
            "strategy": c.strategy,
            "task_id": c.task_id,
            "status": c.status.as_str(),
            "experiment_count": c.experiment_ids.len(),
            "champion": c.champion,
            "challengers": c.challengers,
            "stop_reason": c.stop_reason,
            "summary": c.summary,
            "exhausted": c.is_terminal(),
            "created_at": c.created_at,
            "updated_at": c.updated_at,
        });
        print_json(true, &v)?;
        return Ok(());
    }
    println!("=== CAMPAIGN REPORT (P42) ===");
    println!("Goal to optimize:      {}", c.goal);
    println!("Scope:                 {}", c.scope);
    println!("Strategy:              {}", c.strategy);
    println!(
        "Parent task:           {}",
        c.task_id.as_deref().unwrap_or("(none)")
    );
    println!(
        "Routes explored:       {} candidate experiment(s)",
        c.experiment_ids.len()
    );
    println!(
        "Champion:              {}",
        c.champion.as_deref().unwrap_or("(none)")
    );
    println!(
        "Challengers:           {}",
        if c.challengers.is_empty() {
            "(none)".to_string()
        } else {
            c.challengers.join(", ")
        }
    );
    println!("Status:                {}", c.status.as_str());
    if let Some(r) = &c.stop_reason {
        println!("Why stopped:           {}", r);
    }
    println!(
        "Summary:               {}",
        if c.summary.is_empty() {
            "(not yet summarized)".to_string()
        } else {
            c.summary.clone()
        }
    );
    Ok(())
}

/// Small helper mirroring `current_project_root()` for the ingest path.
fn cwd() -> Result<PathBuf> {
    Ok(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

// ---------------- Constitution / Protocol / Reference / Context commands ----------------

fn current_project_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Attach the current checkout to a persisted project identity.
pub fn project_attach(path: String) -> Result<()> {
    let root = route_basic::discover_project_root(&current_project_root())
        .unwrap_or_else(current_project_root);
    let source = PathBuf::from(path);
    let identity = route_basic::attach_project_identity(&root, &source)?;
    println!("Attached project identity: {}", identity.project_id);
    println!("Workspace identity: {}", identity.workspace_id);
    Ok(())
}

pub fn project_identity() -> Result<()> {
    let root = route_basic::discover_project_root(&current_project_root())
        .unwrap_or_else(current_project_root);
    let identity = route_basic::ensure_identity(&root)?;
    println!("Project identity: {}", identity.project_id);
    println!("Workspace identity: {}", identity.workspace_id);
    Ok(())
}

pub fn constitution_show() -> Result<()> {
    use route_basic::Constitution;
    let cwd = current_project_root();
    match Constitution::read(&cwd) {
        Ok(c) => {
            println!("# Constitution — version {}", c.version);
            println!("# created_at: {}\n", c.created_at);
            println!("{}", c.body.trim());
            Ok(())
        }
        Err(_) => {
            let default = Constitution::default();
            println!("# (no constitution present)");
            println!("# A default would be:\n");
            println!("{}", default.body.trim());
            Ok(())
        }
    }
}

pub fn constitution_init() -> Result<()> {
    use route_basic::Constitution;
    let cwd = current_project_root();
    let written = Constitution::ensure_exists(&cwd)?;
    if written {
        println!(
            "✓ Initialized default Constitution at {}",
            Constitution::path(&cwd).display()
        );
    } else {
        println!(
            "– Constitution already exists at {}",
            Constitution::path(&cwd).display()
        );
    }
    Ok(())
}

pub fn constitution_path() -> Result<()> {
    use route_basic::Constitution;
    let cwd = current_project_root();
    println!("{}", Constitution::path(&cwd).display());
    Ok(())
}

pub fn protocol_show() -> Result<()> {
    use route_basic::Protocol;
    let cwd = current_project_root();
    match Protocol::read(&cwd) {
        Ok(p) => {
            println!(
                "# Protocol — version {} / revision {}",
                p.version, p.revision
            );
            println!("# last_updated: {}\n", p.updated_at);
            println!("{}", p.body.trim());
            Ok(())
        }
        Err(_) => {
            let default = Protocol::default();
            println!("# (no protocol present)");
            println!("# A default would be:\n");
            println!("{}", default.body.trim());
            Ok(())
        }
    }
}

pub fn protocol_init() -> Result<()> {
    use route_basic::Protocol;
    let cwd = current_project_root();
    let written = Protocol::ensure_exists(&cwd)?;
    if written {
        println!(
            "✓ Initialized default Protocol at {}",
            Protocol::path(&cwd).display()
        );
    } else {
        println!(
            "– Protocol already exists at {}",
            Protocol::path(&cwd).display()
        );
    }
    Ok(())
}

pub fn protocol_status() -> Result<()> {
    use route_basic::Protocol;
    let cwd = current_project_root();
    match Protocol::read(&cwd) {
        Ok(p) => {
            println!("Protocol file: {}", Protocol::path(&cwd).display());
            println!("  version:      {}", p.version);
            println!("  revision:     {}", p.revision);
            println!("  last_updated: {}", p.updated_at);
        }
        Err(_) => {
            println!("No protocol file present. Run `route protocol init` first.");
        }
    }
    Ok(())
}

pub fn protocol_path() -> Result<()> {
    use route_basic::Protocol;
    let cwd = current_project_root();
    println!("{}", Protocol::path(&cwd).display());
    Ok(())
}

pub fn reference_show() -> Result<()> {
    use route_basic::ReferenceRegistry;
    let cwd = current_project_root();
    let registry = match ReferenceRegistry::read(&cwd) {
        Ok(r) => r,
        Err(_) => ReferenceRegistry::default(),
    };
    let json = serde_json::to_string_pretty(&registry)?;
    println!("{}", json);
    Ok(())
}

pub fn reference_add(
    id: String,
    type_: String,
    source: String,
    path: Option<String>,
    description: String,
    capabilities: String,
    constraints: String,
    tags: Vec<String>,
    enabled: Option<bool>,
    name: Option<String>,
    project_scope: Option<String>,
    trust: Option<String>,
    entrypoint: Option<String>,
) -> Result<()> {
    use route_basic::{ReferenceEntry, ReferenceRegistry, ReferenceType};
    let cwd = current_project_root();
    let mut registry = match ReferenceRegistry::read(&cwd) {
        Ok(r) => r,
        Err(_) => ReferenceRegistry::default(),
    };
    let t: ReferenceType = type_
        .parse()
        .map_err(|e: String| anyhow!("Invalid reference type '{}': {}", type_, e))?;
    let mut builder = ReferenceEntry::builder(id.clone(), t, source, description)
        .with_opt_path(path)
        .with_capabilities(capabilities)
        .with_constraints(constraints)
        .with_tags(tags);
    if let Some(v) = enabled {
        builder = builder.with_enabled(v);
    }
    if let Some(v) = name {
        builder = builder.with_name(v);
    }
    if let Some(v) = project_scope {
        builder = builder.with_project_scope(v);
    }
    if let Some(v) = trust {
        builder = builder.with_trust(v);
    }
    if let Some(v) = entrypoint {
        builder = builder.with_entrypoint(v);
    }
    let entry = builder.build();
    let added = registry.upsert(entry);
    registry.write(&cwd)?;
    if added {
        println!("✓ Added reference '{}'", id);
    } else {
        println!("✓ Updated reference '{}'", id);
    }
    Ok(())
}

pub fn reference_remove(id: String, force: bool) -> Result<()> {
    use route_basic::{Origin, ReferenceRegistry};
    let cwd = current_project_root();
    let mut registry = match ReferenceRegistry::read(&cwd) {
        Ok(r) => r,
        Err(_) => return Err(anyhow!("No reference registry present")),
    };
    // Check if the entry is UserCreated and requires --force
    if let Some(entry) = registry.get(&id) {
        if entry.origin == Origin::UserCreated && !force {
            return Err(anyhow!(
                "Reference '{}' is UserCreated and requires --force to remove",
                id
            ));
        }
    }
    if registry.remove(&id) {
        registry.write(&cwd)?;
        println!("✓ Removed reference '{}'", id);
    } else {
        return Err(anyhow!("Reference '{}' not found", id));
    }
    Ok(())
}

pub fn reference_list(type_filter: Option<String>, show_disabled: bool) -> Result<()> {
    use route_basic::ReferenceRegistry;
    let cwd = current_project_root();
    let registry = match ReferenceRegistry::read(&cwd) {
        Ok(r) => r,
        Err(_) => ReferenceRegistry::default(),
    };
    let filter: Option<String> = type_filter.map(|s| s.to_ascii_lowercase());
    let mut entries: Vec<_> = registry
        .entries
        .iter()
        .filter(|e| {
            if !show_disabled && !e.enabled {
                return false;
            }
            filter
                .as_ref()
                .map(|f| {
                    serde_json::to_value(&e.type_)
                        .ok()
                        .and_then(|v| v.as_str().map(|s| s.to_ascii_lowercase().contains(f)))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
        })
        .collect();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    if entries.is_empty() {
        println!("(no references registered)");
        return Ok(());
    }
    // Header
    println!(
        "{:<24} {:<12} {:<8} {}",
        "ID", "TYPE", "ENABLED", "DESCRIPTION"
    );
    println!("{}", "-".repeat(80));
    for e in entries {
        let type_str = serde_json::to_value(&e.type_)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let enabled_str = if e.enabled { "yes" } else { "no" };
        let desc = if e.description.is_empty() {
            e.source.clone()
        } else {
            e.description.clone()
        };
        println!("{:<24} {:<12} {:<8} {}", e.id, type_str, enabled_str, desc);
    }
    Ok(())
}

pub fn reference_path() -> Result<()> {
    use route_basic::ReferenceRegistry;
    let cwd = current_project_root();
    println!("{}", ReferenceRegistry::path(&cwd).display());
    Ok(())
}

pub fn reference_import(
    source: String,
    explicit_id: Option<String>,
    explicit_type: Option<String>,
    explicit_description: Option<String>,
    replace: bool,
) -> Result<()> {
    use route_basic::{
        classify_source, import_reference_source, Origin, ReferenceEntry, ReferenceRegistry,
        ReferenceType,
    };
    use std::str::FromStr;
    let cwd = current_project_root();
    let imported = import_reference_source(&source)?;
    let type_ = match explicit_type {
        Some(t) => ReferenceType::from_str(&t).unwrap_or(imported.type_),
        None => imported.type_,
    };
    // If the classifier couldn't settle and the user didn't pass --type,
    // refuse to create junk. They can pass --type explicitly to override.
    if matches!(type_, ReferenceType::Unknown) {
        return Err(anyhow!(
            "could not classify source {source:?}; pass --type document|repo|skill|cli explicitly"
        ));
    }

    let id = match explicit_id {
        Some(i) => i,
        None => imported
            .id
            .ok_or_else(|| anyhow!("could not auto-generate an id; pass --id <name> explicitly"))?,
    };

    let mut reg = ReferenceRegistry::read(&cwd)?;
    let already_exists = reg.index_of(&id).is_some();
    if already_exists && !replace {
        let (kind_dbg, _src_dbg) = classify_source(&source);
        return Err(anyhow!(
            "reference id {id:?} already exists (detected type: {type_dbg:?}). \
             Pass --replace if you want to overwrite it.",
            type_dbg = kind_dbg
        ));
    }

    let now_ts = chrono::Utc::now().timestamp_millis();
    let desc = explicit_description.unwrap_or(imported.description);
    let entry = ReferenceEntry::builder(id.clone(), type_, &imported.source, &desc)
        .with_capabilities(&imported.capabilities)
        .with_constraints(&imported.constraints)
        .with_origin(Origin::Imported)
        .with_import_origin(&imported.source)
        .with_imported_at(now_ts)
        .with_content_hash(&imported.content_hash)
        .with_last_checked(now_ts)
        .with_opt_path(imported.path.clone())
        .with_created_at(now_ts)
        .build();
    let inserted = reg.upsert(entry);
    reg.write(&cwd)?;

    println!(
        "✓ {} reference {:?} (type={:?})",
        if inserted { "Imported new" } else { "Replaced" },
        id,
        type_
    );
    println!("  source:        {}", imported.source);
    if !imported.content_hash.is_empty() {
        println!("  content_hash:  {}", imported.content_hash);
    }
    if let Some(p) = &imported.path {
        println!("  path:          {}", p);
    }
    println!("  created_at:    {}", now_ts);
    if !desc.is_empty() {
        println!(
            "  description:   {}",
            desc.chars().take(160).collect::<String>()
        );
    }
    if !imported.capabilities.is_empty() {
        println!(
            "  capabilities:  {} lines, {} chars",
            imported.capabilities.lines().count(),
            imported.capabilities.len()
        );
    }
    Ok(())
}

pub fn print_effective_context(
    hash_only: bool,
    metadata_only: bool,
    include_memory: bool,
) -> Result<()> {
    use route_basic::{
        effective_context, effective_context_fingerprint, ContextHistoryManifest, ContextSnapshot,
    };
    let cwd = current_project_root();
    if hash_only {
        let fp = effective_context_fingerprint(&cwd)?;
        println!("{}", fp);
        return Ok(());
    }
    if metadata_only {
        let snap = ContextSnapshot::collect(&cwd)?;
        let (manifest, is_new) = ContextHistoryManifest::record_if_new(&cwd, &snap)?;
        println!("context_hash:           {}", snap.fingerprint);
        println!("constitution_version:   {}", snap.constitution_version);
        println!("constitution_hash:      {}", snap.constitution_hash);
        println!("protocol_version:       {}", snap.protocol_version);
        println!("protocol_revision:      {}", snap.protocol_revision);
        println!("protocol_hash:          {}", snap.protocol_hash);
        println!("reference_entries_hash: {}", snap.reference_entries_hash);
        println!("history_created_at:     {}", manifest.created_at);
        println!("history_is_new_record:  {}", is_new);
        return Ok(());
    }
    let snap = ContextSnapshot::collect(&cwd)?;
    let _ = ContextHistoryManifest::record_if_new(&cwd, &snap)?;
    let mut text = effective_context(&cwd)?;

    // Append memory section if --memory flag is passed
    if include_memory {
        text.push_str("\n\n");
        text.push_str(&route_basic::build_memory_context(&cwd, None));
    }

    // Write to stdout via a single buffer write so shell redirection captures cleanly.
    let mut stdout = std::io::stdout();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

pub fn context_history() -> Result<()> {
    use route_basic::{ContextHistoryManifest, ContextSnapshot};
    let cwd = current_project_root();
    let mut list = ContextHistoryManifest::list_all(&cwd)?;
    if list.is_empty() {
        let snap = ContextSnapshot::collect(&cwd)?;
        let (m, _) = ContextHistoryManifest::record_if_new(&cwd, &snap)?;
        list = vec![m];
    }
    println!(
        "{: <8}  {: <64}  {: <10}  {: <10}  {: <14}",
        "AGE", "CONTEXT_HASH", "CONST_V", "PROTO_REV", "CREATED_AT"
    );
    let now = route_core::now_millis();
    for m in &list {
        let age_days = (now - m.created_at).max(0) / (24 * 60 * 60 * 1000);
        let age = if age_days == 0 {
            let hrs = (now - m.created_at).max(0) / (60 * 60 * 1000);
            format!("{hrs:>2}h")
        } else {
            format!("{age_days:>3}d")
        };
        let ts = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(m.created_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| m.created_at.to_string());
        println!(
            "{: <8}  {: <64}  {: <10}  {: <10}  {}",
            age, m.context_hash, m.constitution_version, m.protocol_revision, ts
        );
    }
    Ok(())
}

pub fn context_show(hash: String) -> Result<()> {
    use route_basic::ContextHistoryManifest;
    let cwd = current_project_root();
    let m = ContextHistoryManifest::read(&cwd, &hash)?
        .ok_or_else(|| anyhow::anyhow!("no context history manifest for hash {hash}"))?;
    println!("context_hash:            {}", m.context_hash);
    println!("constitution_version:    {}", m.constitution_version);
    println!("constitution_hash:       {}", m.constitution_hash);
    println!("protocol_version:        {}", m.protocol_version);
    println!("protocol_revision:       {}", m.protocol_revision);
    println!("protocol_hash:           {}", m.protocol_hash);
    println!("reference_entries_hash:  {}", m.reference_entries_hash);
    let ts = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(m.created_at)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| m.created_at.to_string());
    println!("created_at:              {ts}");
    Ok(())
}

pub fn context_diff(hasha: String, hashb: String) -> Result<()> {
    use route_basic::ContextHistoryManifest;
    let cwd = current_project_root();
    let a = ContextHistoryManifest::read(&cwd, &hasha)?
        .ok_or_else(|| anyhow::anyhow!("no manifest for hash {hasha}"))?;
    let b = ContextHistoryManifest::read(&cwd, &hashb)?
        .ok_or_else(|| anyhow::anyhow!("no manifest for hash {hashb}"))?;
    let diff = ContextHistoryManifest::diff(&a, &b);
    println!("from:  {}", diff.from_hash);
    println!("to:    {}", diff.to_hash);
    if !diff.any_changed {
        println!("\n(no semantic difference — Constitution, Protocol and Reference are identical)");
        return Ok(());
    }
    let sections = [
        ("Constitution", &diff.constitution),
        ("Protocol", &diff.protocol),
        ("Reference", &diff.reference),
    ];
    for (name, delta) in sections {
        if delta.changed {
            println!("\n  ✗ {name}: changed — {}", delta.summary);
        } else {
            println!("\n  ✓ {name}: unchanged");
        }
    }
    Ok(())
}

pub fn context_replay(hash: String, target: Option<String>, to_stdout: bool) -> Result<()> {
    use route_basic::{replay_context, AgentPolicy, ApplyTarget, ContextSnapshot};
    let cwd = current_project_root();

    let context_text = replay_context(&cwd, &hash)?;

    // If target is specified, compile into a format-specific managed block
    let target = target.as_deref().unwrap_or("generic");
    match target {
        "claude" | "codex" | "generic" => {
            let target_enum = ApplyTarget::parse(target)?;

            // Parse the agent policy from the replayed protocol
            let snap = ContextSnapshot::collect(&cwd)?;
            let policy = AgentPolicy::from_protocol_body(&snap.protocol_body);

            let compiled = match target_enum {
                ApplyTarget::Claude => {
                    let mut out = String::new();
                    out.push_str("# Replayed Context — Claude Code\n\n");
                    out.push_str(&context_text);
                    out.push_str("\n\n");
                    out.push_str(&policy.render_claude());
                    out
                }
                ApplyTarget::Codex => {
                    let mut out = String::new();
                    out.push_str("# Replayed Context — Codex\n\n");
                    out.push_str(&context_text);
                    out.push_str("\n\n");
                    out.push_str(&policy.render_codex());
                    out
                }
                ApplyTarget::DeepSeek => {
                    let mut out = String::new();
                    out.push_str("# Replayed Context — DeepSeek\n\n");
                    out.push_str(&context_text);
                    out.push_str("\n\n");
                    out.push_str(&policy.render_deepseek());
                    out
                }
                ApplyTarget::Generic => {
                    let mut out = String::new();
                    out.push_str("# Replayed Context — Generic\n\n");
                    out.push_str(&context_text);
                    out.push_str("\n\n");
                    out.push_str(&policy.render_generic());
                    out
                }
            };

            if to_stdout {
                let mut stdout = std::io::stdout();
                use std::io::Write;
                stdout.write_all(compiled.as_bytes())?;
                stdout.flush()?;
            } else {
                // Write to a replay file
                let replay_path = cwd.join(format!(
                    ".route/context/replay-{}.md",
                    &hash[..16.min(hash.len())]
                ));
                if let Some(parent) = replay_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&replay_path, &compiled)?;
                println!(
                    "✓ Replayed context hash '{}' → {}",
                    &hash[..16.min(hash.len())],
                    replay_path.display()
                );
            }
        }
        other => {
            return Err(anyhow::anyhow!(
                "unknown target '{other}'. Expected: claude | codex | generic"
            ));
        }
    }

    Ok(())
}

pub fn context_task(task: String, target: Option<String>, top_k: usize, json: bool) -> Result<()> {
    use route_basic::{inject_learned_experiences, task_scoped_context, AgentPolicy, ApplyTarget};
    use std::io::Write;
    let cwd = current_project_root();

    let result = task_scoped_context(&cwd, &task, top_k, target.as_deref())?;

    // Inject learned experiences (P6)
    let result = inject_learned_experiences(result, &cwd, top_k)?;

    if json {
        // Output structured JSON: references, policy, context_hash
        let refs: Vec<serde_json::Value> = result
            .selected_references
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "type": r.type_,
                    "source": r.source,
                    "description": r.description,
                })
            })
            .collect();
        let output = serde_json::json!({
            "task": task,
            "profile_id": result.profile_id,
            "workflow_ids": result.workflow_ids,
            "references": refs,
            "protocol_body": result.protocol_body,
        });
        let mut stdout = std::io::stdout();
        stdout.write_all(serde_json::to_string_pretty(&output)?.as_bytes())?;
        stdout.flush()?;
        return Ok(());
    }

    // Render the Markdown output
    let mut out = result.render_markdown();

    // If target is specified, append the agent policy
    if let Some(t) = &target {
        let target_enum = ApplyTarget::parse(t)?;
        let policy = AgentPolicy::from_protocol_body(&result.protocol_body);
        let policy_block = match target_enum {
            ApplyTarget::Claude => policy.render_claude(),
            ApplyTarget::Codex => policy.render_codex(),
            ApplyTarget::DeepSeek => policy.render_deepseek(),
            ApplyTarget::Generic => policy.render_generic(),
        };
        out.push_str("\n\n");
        out.push_str(&policy_block);
    }

    let mut stdout = std::io::stdout();
    stdout.write_all(out.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

pub fn context_dispatch(
    hash_only: bool,
    metadata_only: bool,
    has_history: bool,
    show_hash: Option<String>,
    diff_pair: Option<(String, String)>,
    _memory: bool,
) -> Result<()> {
    if has_history {
        return context_history();
    }
    if let Some(h) = show_hash {
        return context_show(h);
    }
    if let Some((a, b)) = diff_pair {
        return context_diff(a, b);
    }
    print_effective_context(hash_only, metadata_only, _memory)
}

pub fn reference_review(json_out: bool, dry_run: bool) -> Result<()> {
    use route_basic::curator_analyze;
    let cwd = current_project_root();
    let (store, report) = curator_analyze(&cwd, !dry_run)?;

    if json_out {
        let wrapper = serde_json::json!({
            "report": report,
            "proposals": store.proposals,
        });
        println!("{}", serde_json::to_string_pretty(&wrapper)?);
        return Ok(());
    }

    println!("Reference Curator — review pass");
    println!("-------------------------------");
    println!(
        "  unknown entries seen:           {}",
        report.unknown_entries_seen
    );
    println!(
        "  unknown entries proposed class: {}",
        report.unknown_entries_classified
    );
    println!(
        "  imported entries seen:          {}",
        report.imported_entries_seen
    );
    println!(
        "  duplicates suspected:           {}",
        report.duplicates_suspected
    );
    println!(
        "  proposals generated:            {}",
        report.proposals_generated
    );
    println!(
        "  persist:                        {}",
        if dry_run {
            "dry-run (none)"
        } else {
            "written to proposals.json"
        }
    );
    if !report.notes.is_empty() {
        println!("\nNotes:");
        for n in &report.notes {
            println!("  • {n}");
        }
    }
    if store.proposals.is_empty() {
        println!("\n(no proposals — registry looks clean)");
        return Ok(());
    }
    println!("\nProposals:");
    for p in &store.proposals {
        let before_kind = p
            .before
            .as_ref()
            .map(|e| format!("{} ({})", e.type_.as_str(), e.origin.as_str()))
            .unwrap_or_else(|| "-".to_string());
        println!("\n  id:          {}", p.id);
        println!("  reference:   {} [action: {:?}]", p.reference_id, p.action);
        println!(
            "  confidence:  {:.2}   (origin-before: {})",
            p.confidence, before_kind
        );
        println!("  reason:      {}", p.reason);
        if let Some(after) = &p.after {
            println!("  after type:  {}", after.type_.as_str());
            use route_basic::ReferenceType;
            if after.type_ != ReferenceType::Unknown {
                println!("  → once applied the entry will appear in Effective Context.");
            }
        }
        println!("  run `route reference apply {}` to approve", p.id);
    }
    Ok(())
}

pub fn reference_apply(proposal_id: String) -> Result<()> {
    use route_basic::{apply_proposal, ProposalAction};
    let cwd = current_project_root();
    let applied = apply_proposal(&cwd, &proposal_id)?;
    println!(
        "Applied proposal {} on reference '{}' (action: {:?})",
        applied.id, applied.reference_id, applied.action
    );
    println!("Reason: {}", applied.reason);
    if matches!(
        applied.action,
        ProposalAction::Classify | ProposalAction::Update
    ) {
        if let Some(after) = &applied.after {
            println!(
                "New entry type: {}   origin: {}",
                after.type_.as_str(),
                after.origin.as_str()
            );
        }
    }
    // Notify user that Effective Context fingerprint has likely changed.
    use route_basic::effective_context_fingerprint;
    match effective_context_fingerprint(&cwd) {
        Ok(fp) => println!("Current context fingerprint: {fp}"),
        Err(_) => {}
    }
    Ok(())
}

pub fn reference_refresh(id: Option<String>, dry_run: bool) -> Result<()> {
    use route_basic::curator_refresh;
    let cwd = current_project_root();
    let results = curator_refresh(&cwd, id.as_deref(), !dry_run)?;
    if results.is_empty() {
        match id {
            Some(s) => println!("no Imported entry with id '{s}' — nothing to do"),
            None => {
                println!("no Imported entries in registry yet — run `route reference import` first")
            }
        }
        return Ok(());
    }
    println!(
        "Reference refresh — {} entry(ies)  ({})",
        results.len(),
        if dry_run { "dry-run" } else { "persisted" }
    );
    let mut n_changed = 0usize;
    for r in &results {
        let badge = if r.changed {
            "CHANGED → proposal"
        } else {
            "ok"
        };
        n_changed += r.changed as usize;
        println!("  [{badge}] {:<20}  {:<30}", r.id, r.note);
        if let Some(p) = &r.proposal {
            println!(
                "           new proposal: {}  (conf {:.2})",
                p.id, p.confidence
            );
        }
    }
    println!("\n  unchanged entries: {}", results.len() - n_changed);
    println!("  changed entries:    {n_changed} → produce Update proposals; run `route reference review` to list or `route reference apply <id>` to approve");
    Ok(())
}

/// Show detailed info about a reference entry.
pub fn reference_inspect(id: String) -> Result<()> {
    use route_basic::ReferenceRegistry;
    let cwd = current_project_root();
    let registry = ReferenceRegistry::read(&cwd)?;
    let entry = registry
        .get(&id)
        .ok_or_else(|| anyhow!("Reference '{}' not found", id))?;

    println!("Reference: {}", entry.id);
    println!("  Type:        {}", entry.type_.as_str());
    if !entry.name.is_empty() {
        println!("  Name:        {}", entry.name);
    }
    println!("  Source:      {}", entry.source);
    println!("  Enabled:     {}", entry.enabled);
    if !entry.description.is_empty() {
        println!("  Description: {}", entry.description);
    }
    if !entry.capabilities.is_empty() {
        println!("  Capabilities: {}", entry.capabilities);
    }
    if !entry.constraints.is_empty() {
        println!("  Constraints:  {}", entry.constraints);
    }
    if !entry.tags.is_empty() {
        println!("  Tags:         [{}]", entry.tags.join(", "));
    }
    println!("  Origin:       {}", entry.origin.as_str());
    println!("  Created:      {}", format_ts(entry.created_at));
    if let Some(ref scope) = entry.project_scope {
        println!("  ProjectScope: {}", scope);
    }
    if let Some(ref trust) = entry.trust {
        println!("  Trust:        {}", trust);
    }
    if let Some(ref ep) = entry.entrypoint {
        println!("  Entrypoint:   {}", ep);
    }
    if let Some(ref p) = entry.path {
        println!("  Path:         {}", p);
    }
    if let Some(ref ch) = entry.content_hash {
        println!("  ContentHash:  {}", ch);
    }
    if let Some(ref lc) = entry.last_checked {
        println!("  LastChecked:  {}", format_ts(*lc));
    }
    Ok(())
}

/// Enable a reference entry.
pub fn reference_enable(id: String) -> Result<()> {
    use route_basic::ReferenceRegistry;
    let cwd = current_project_root();
    let mut registry = ReferenceRegistry::read(&cwd)?;
    if registry.enable(&id) {
        registry.write(&cwd)?;
        println!("✓ Enabled reference '{}'", id);
    } else {
        return Err(anyhow!("Reference '{}' not found", id));
    }
    Ok(())
}

/// Disable a reference entry.
pub fn reference_disable(id: String) -> Result<()> {
    use route_basic::ReferenceRegistry;
    let cwd = current_project_root();
    let mut registry = ReferenceRegistry::read(&cwd)?;
    if registry.disable(&id) {
        registry.write(&cwd)?;
        println!("✓ Disabled reference '{}'", id);
    } else {
        return Err(anyhow!("Reference '{}' not found", id));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Workflow commands
// ---------------------------------------------------------------------------

/// List all workflows.
pub fn workflow_list() -> Result<()> {
    use route_basic::WorkflowStore;
    let cwd = current_project_root();
    let store = WorkflowStore::load(&cwd)?;
    if store.workflows.is_empty() {
        println!("(no workflows registered)");
        return Ok(());
    }
    println!(
        "{:<24} {:<30} {:<8} {}",
        "ID", "NAME", "ENABLED", "DESCRIPTION"
    );
    println!("{}", "-".repeat(90));
    for wf in &store.workflows {
        let enabled_str = if wf.enabled { "yes" } else { "no" };
        let name = if wf.name.is_empty() { &wf.id } else { &wf.name };
        println!(
            "{:<24} {:<30} {:<8} {}",
            wf.id, name, enabled_str, wf.description
        );
    }
    Ok(())
}

/// Show details of a workflow.
pub fn workflow_show(id: String) -> Result<()> {
    use route_basic::WorkflowStore;
    let cwd = current_project_root();
    let store = WorkflowStore::load(&cwd)?;
    let wf = store
        .get(&id)
        .ok_or_else(|| anyhow!("Workflow '{}' not found", id))?;

    println!("Workflow: {}", wf.id);
    if !wf.name.is_empty() {
        println!("  Name:        {}", wf.name);
    }
    println!("  Source:      {}", wf.source);
    println!("  Description: {}", wf.description);
    println!("  Enabled:     {}", wf.enabled);
    if let Some(ref steps) = wf.steps {
        println!("  Steps:       {}", steps.len());
        for (i, step) in steps.iter().enumerate() {
            println!("    {}. {} - {}", i + 1, step.name, step.description);
        }
    } else {
        println!("  Steps:       External workflow — see source for usage instructions");
    }
    if !wf.skills.is_empty() {
        println!("  Skills:      {}", wf.skills.join(", "));
    }
    if !wf.references.is_empty() {
        println!("  References:  {}", wf.references.join(", "));
    }
    if !wf.tags.is_empty() {
        println!("  Tags:        [{}]", wf.tags.join(", "));
    }
    println!("  Origin:      {}", wf.origin.as_str());
    println!("  Created:     {}", format_ts(wf.created_at));
    if let Some(upstream) = &wf.upstream {
        println!("  Upstream:    {}", upstream);
    }
    println!("  Base ver:    {}", wf.base_version);
    println!("  Revision:    {}", wf.current_revision);
    Ok(())
}

/// Import a workflow from a file or URL.
///
/// Generic external workflow import (P5):
/// - Tries to parse as `WorkflowDefinition` JSON first.
/// - For Markdown files: extracts headings as steps, stores raw content as source.
/// - For other formats: stores as a raw reference workflow with source + usage info.
///   The AI reads the native source — Route does not re-implement external systems.
pub fn workflow_import(
    source: String,
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
) -> Result<()> {
    use route_basic::{WorkflowDefinition, WorkflowStep, WorkflowStore};

    let cwd = current_project_root();
    let raw = if source.starts_with("http://") || source.starts_with("https://") {
        // Fetch from URL using reqwest (blocking)
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        let response = client
            .get(&source)
            .send()
            .map_err(|e| anyhow!("Failed to fetch URL '{}': {}", source, e))?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch URL '{}': HTTP {}",
                source,
                response.status()
            ));
        }
        response
            .text()
            .map_err(|e| anyhow!("Failed to read response body: {}", e))?
    } else {
        // Read from file
        let path = std::path::Path::new(&source);
        if !path.exists() {
            return Err(anyhow!("Source file '{}' not found", source));
        }
        std::fs::read_to_string(path)?
    };

    let source_path = std::path::Path::new(&source);
    let wf_id = id.clone().unwrap_or_else(|| {
        source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("imported")
            .to_string()
    });

    let mut store = WorkflowStore::load(&cwd)?;

    // Try to parse as JSON (WorkflowDefinition format) first
    let wf = if let Ok(mut parsed) = serde_json::from_str::<WorkflowDefinition>(&raw) {
        // When parsing JSON successfully, only override the id if explicitly
        // passed via --id flag. Otherwise, preserve the JSON-defined id.
        if id.is_some() {
            parsed.id = wf_id.clone();
        }
        if let Some(n) = name {
            parsed.name = n;
        }
        if let Some(d) = description {
            parsed.description = d;
        }
        if parsed.source.is_empty() {
            parsed.source = source.clone();
        }
        parsed
    } else if source_path
        .extension()
        .map(|e| e == "md" || e == "mdx")
        .unwrap_or(false)
    {
        // Markdown: extract headings as steps, store raw content as source
        let steps: Vec<WorkflowStep> = raw
            .lines()
            .filter(|l| l.starts_with("## ") || l.starts_with("### "))
            .enumerate()
            .map(|(i, line)| {
                let name = line
                    .trim_start_matches('#')
                    .trim()
                    .trim_start_matches('#')
                    .trim()
                    .to_string();
                WorkflowStep {
                    name: if name.is_empty() {
                        format!("Step {}", i + 1)
                    } else {
                        name
                    },
                    description: String::new(),
                    references: Vec::new(),
                    skills: Vec::new(),
                    command: None,
                    expected_outcome: None,
                }
            })
            .collect();

        let wf_name = name.unwrap_or_else(|| {
            source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("imported")
                .to_string()
        });

        WorkflowDefinition {
            id: wf_id.clone(),
            name: wf_name,
            source: source.clone(),
            description: description
                .unwrap_or_else(|| "Imported workflow from Markdown source".to_string()),
            steps: if steps.is_empty() { None } else { Some(steps) },
            skills: Vec::new(),
            references: Vec::new(),
            agent_policy: None,
            verification_policy: None,
            enabled: true,
            created_at: route_core::now_millis(),
            origin: route_basic::Origin::Imported,
            tags: vec!["workflow".to_string(), "markdown".to_string()],
            upstream: None,
            base_version: 0,
            current_revision: 0,
        }
    } else {
        // Raw text / YAML / unknown format: store as raw reference workflow.
        // The AI reads the native source directly — Route does not re-parse.
        let wf_name = name.unwrap_or_else(|| {
            source_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("imported")
                .to_string()
        });

        WorkflowDefinition {
            id: wf_id.clone(),
            name: wf_name,
            source: source.clone(),
            description: description.unwrap_or_else(|| {
                "Imported workflow — AI should read the native source for details".to_string()
            }),
            steps: None,
            skills: Vec::new(),
            references: Vec::new(),
            agent_policy: None,
            verification_policy: None,
            enabled: true,
            created_at: route_core::now_millis(),
            origin: route_basic::Origin::Imported,
            tags: vec!["workflow".to_string(), "external".to_string()],
            upstream: None,
            base_version: 0,
            current_revision: 0,
        }
    };

    // Use the workflow's actual id for saving, not the filename-derived wf_id
    let save_id = wf.id.clone();
    let inserted = store.upsert(wf);
    WorkflowStore::save(&cwd, store.get(&save_id).unwrap())?;

    if inserted {
        println!("✓ Imported workflow '{}'", wf_id);
    } else {
        println!("✓ Updated workflow '{}'", wf_id);
    }
    println!("  Source: {}", source);
    Ok(())
}

/// Enable a workflow.
pub fn workflow_enable(id: String) -> Result<()> {
    use route_basic::WorkflowStore;
    let cwd = current_project_root();
    let mut store = WorkflowStore::load(&cwd)?;
    let _ = store.enable(&id)?;
    WorkflowStore::save(&cwd, store.get(&id).unwrap())?;
    println!("✓ Enabled workflow '{}'", id);
    Ok(())
}

/// Disable a workflow.
pub fn workflow_disable(id: String) -> Result<()> {
    use route_basic::WorkflowStore;
    let cwd = current_project_root();
    let mut store = WorkflowStore::load(&cwd)?;
    let _ = store.disable(&id)?;
    WorkflowStore::save(&cwd, store.get(&id).unwrap())?;
    println!("✓ Disabled workflow '{}'", id);
    Ok(())
}

/// Generate an evolve proposal for a workflow.
///
/// Creates a `WorkflowChangeProposal` and saves it to the proposal store.
/// Does NOT auto-apply — the proposal must be applied via `workflow_apply_evolve`.
pub fn workflow_evolve(id: String, reason: String, effect: String, risk: String) -> Result<()> {
    use route_basic::{generate_evolve_proposal, WorkflowProposalStore, WorkflowStore};

    let cwd = current_project_root();

    // Verify the workflow exists
    let store = WorkflowStore::load(&cwd)?;
    if store.get(&id).is_none() {
        return Err(anyhow!("Workflow '{}' not found", id));
    }

    let proposal = generate_evolve_proposal(&cwd, &id, &reason, &effect, &risk);

    let mut pstore = WorkflowProposalStore::load(&cwd)?;
    pstore.add(proposal.clone());
    pstore.save(&cwd)?;

    println!(
        "✓ Generated evolve proposal '{}' for workflow '{}'",
        proposal.id, id
    );
    println!("  From revision:  {}", proposal.from_revision);
    println!("  To revision:    {}", proposal.to_revision);
    println!("  Reason:         {}", proposal.reason);
    println!("  Effect:         {}", proposal.expected_effect);
    println!("  Risk:           {}", proposal.risk);
    println!("  Status:         {}", proposal.status);
    println!();
    println!("  Apply with: route workflow apply-evolve {}", proposal.id);
    Ok(())
}

/// Apply a workflow change proposal.
///
/// Increments the workflow's `current_revision` and records a strategy
/// snapshot for traceability.
pub fn workflow_apply_evolve(proposal_id: String) -> Result<()> {
    use route_basic::{WorkflowProposalStore, WorkflowStore};

    let cwd = current_project_root();

    let mut pstore = WorkflowProposalStore::load(&cwd)?;
    let proposal_idx = pstore
        .proposals
        .iter()
        .position(|p| p.id == proposal_id || p.id.starts_with(&proposal_id))
        .ok_or_else(|| anyhow!("Proposal '{}' not found", proposal_id))?;

    let proposal = &pstore.proposals[proposal_idx];
    if proposal.status != "pending" {
        return Err(anyhow!(
            "Proposal '{}' is not pending (status: {})",
            proposal_id,
            proposal.status
        ));
    }

    let workflow_id = proposal.workflow_id.clone();
    let from_revision = proposal.from_revision;
    let to_revision = proposal.to_revision;
    let before = proposal.before.clone();
    let after = proposal.after.clone();
    let proposal_id_clone = proposal.id.clone();

    let mut store = WorkflowStore::load(&cwd)?;
    store.evolve(&workflow_id, proposal)?;

    // Save the updated workflow
    let wf = store
        .get(&workflow_id)
        .ok_or_else(|| anyhow!("Workflow '{}' not found", workflow_id))?;
    WorkflowStore::save(&cwd, wf)?;

    // Mark proposal as applied (re-borrow pstore mutably)
    if let Some(p) = pstore.get_mut(&proposal_id_clone) {
        p.status = "applied".to_string();
    }
    pstore.save(&cwd)?;

    // Record strategy snapshot for traceability
    let mut strategy_store = route_basic::StrategyStore::load(&cwd)?;
    strategy_store.record_snapshot(
        &cwd,
        Some(format!("workflow-evolve:{}", workflow_id)),
        Some(format!(
            "Applied evolve proposal '{}': {} -> {} (rev {} -> {})",
            proposal_id_clone, before, after, from_revision, to_revision
        )),
    )?;

    println!(
        "✓ Applied evolve proposal '{}' to workflow '{}'",
        proposal_id_clone, workflow_id
    );
    println!("  Revision: {} -> {}", from_revision, to_revision);
    Ok(())
}

/// List all workflow change proposals.
pub fn workflow_list_proposals() -> Result<()> {
    use route_basic::WorkflowProposalStore;

    let cwd = current_project_root();
    let pstore = WorkflowProposalStore::load(&cwd)?;
    let proposals = pstore.list();

    if proposals.is_empty() {
        println!("(no workflow change proposals)");
        return Ok(());
    }

    println!(
        "{:<24} {:<24} {:<10} {:<12} {}",
        "PROPOSAL ID", "WORKFLOW", "REVISION", "STATUS", "REASON"
    );
    println!("{}", "-".repeat(100));
    for p in &proposals {
        let rev = format!("{}->{}", p.from_revision, p.to_revision);
        let reason = if p.reason.len() > 35 {
            format!("{}...", &p.reason[..32])
        } else {
            p.reason.clone()
        };
        println!(
            "{:<24} {:<24} {:<10} {:<12} {}",
            p.id, p.workflow_id, rev, p.status, reason
        );
    }
    Ok(())
}

/// Reset a workflow to upstream or a specific revision.
pub fn workflow_reset(id: String, to: String) -> Result<()> {
    use route_basic::WorkflowStore;

    let cwd = current_project_root();

    let mut store = WorkflowStore::load(&cwd)?;

    if to == "upstream" {
        store.reset_to_upstream(&id)?;
        let wf = store.get(&id).unwrap();
        WorkflowStore::save(&cwd, wf)?;

        // Record strategy snapshot
        let mut strategy_store = route_basic::StrategyStore::load(&cwd)?;
        strategy_store.record_snapshot(
            &cwd,
            Some(format!("workflow-reset-upstream:{}", id)),
            Some(format!("Reset workflow '{}' to upstream", id)),
        )?;

        println!("✓ Reset workflow '{}' to upstream", id);
    } else {
        let revision: u32 = to.parse().map_err(|_| {
            anyhow!(
                "Invalid revision '{}' — expected a number or 'upstream'",
                to
            )
        })?;
        store.reset_to_revision(&id, revision)?;
        let wf = store.get(&id).unwrap();
        WorkflowStore::save(&cwd, wf)?;

        // Record strategy snapshot
        let mut strategy_store = route_basic::StrategyStore::load(&cwd)?;
        strategy_store.record_snapshot(
            &cwd,
            Some(format!("workflow-reset-revision:{}", id)),
            Some(format!("Reset workflow '{}' to revision {}", id, revision)),
        )?;

        println!("✓ Reset workflow '{}' to revision {}", id, revision);
    }

    Ok(())
}

/// Create a workflow from a study candidate.
pub fn workflow_from_study(index: usize, id_override: Option<String>) -> Result<()> {
    use route_basic::{workflow_from_candidate, WorkflowStore};

    let cwd = current_project_root();
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("no study report available. Run `route study <path>` first."))?;
    let report = route_basic::load_report(&root)
        .with_context(|| anyhow!("no study report available. Run `route study <path>` first."))?;

    let candidate = report.candidates.get(index).ok_or_else(|| {
        anyhow!(
            "candidate index {index} out of range (max {})",
            report.candidates.len().saturating_sub(1)
        )
    })?;

    let wf = workflow_from_candidate(candidate, id_override.as_deref());
    let wf_id = wf.id.clone();

    let mut store = WorkflowStore::load(&cwd)?;
    let inserted = store.upsert(wf);
    if let Some(ws) = store.get(&wf_id) {
        WorkflowStore::save(&cwd, ws)?;
    }

    if inserted {
        println!(
            "✓ Imported workflow from study candidate [{}] '{}'",
            index, candidate.name
        );
    } else {
        println!(
            "✓ Updated workflow from study candidate [{}] '{}'",
            index, candidate.name
        );
    }
    println!("  kind:       {}", candidate.kind);
    println!("  source:     {}", candidate.source);
    println!("  evidence:   {}", candidate.evidence);
    println!("  confidence: {:.2}", candidate.confidence);

    Ok(())
}

// ---------------------------------------------------------------------------
// Plan command — counterfactual preview
// ---------------------------------------------------------------------------

/// Plan a task without executing — counterfactual preview.
pub fn plan(
    task: String,
    strategy: Option<String>,
    compare: Option<String>,
    to_stdout: bool,
) -> Result<()> {
    use route_basic::plan::{render_comparison, render_plan};
    use route_basic::{compare_plans, plan_task};

    let cwd = current_project_root();

    if let Some(compare_str) = compare {
        // Comparison mode: comma-separated strategy IDs
        let strategies: Vec<String> = compare_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if strategies.len() < 2 {
            anyhow::bail!("--compare requires at least two strategy IDs (comma-separated)");
        }

        let comparison = compare_plans(&cwd, &task, &strategies)?;
        let output = render_comparison(&comparison);

        if to_stdout {
            println!("{}", output);
        } else {
            let plan_dir = cwd.join(".route").join("plans");
            std::fs::create_dir_all(&plan_dir)?;
            let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
            let plan_path = plan_dir.join(format!("plan-compare-{}.txt", ts));
            std::fs::write(&plan_path, &output)?;
            println!("Plan comparison written to {}", plan_path.display());
        }
    } else {
        // Single plan mode
        let plan = plan_task(&cwd, &task, strategy.as_deref())?;
        let output = render_plan(&plan);

        if to_stdout {
            println!("{}", output);
        } else {
            let plan_dir = cwd.join(".route").join("plans");
            std::fs::create_dir_all(&plan_dir)?;
            let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
            let plan_path = plan_dir.join(format!("plan-{}.txt", ts));
            std::fs::write(&plan_path, &output)?;
            println!("Plan written to {}", plan_path.display());
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// AgentPlan command
// ---------------------------------------------------------------------------

/// Generate an AgentPlan for the current task/policy.
pub fn agent_plan(
    task: String,
    workflow_id: Option<String>,
    target: String,
    to_stdout: bool,
) -> Result<()> {
    use route_basic::{
        compile_plan, compile_to_host, render_plan, CompilerInput, Protocol, ReferenceRegistry,
        WorkflowStore,
    };

    let cwd = current_project_root();

    // Read protocol for agent policy
    let protocol = Protocol::read(&cwd)?;
    let agent_policy = protocol.body.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("<!-- route-agent-policy:")
            .and_then(|s| s.strip_suffix(" -->"))
            .map(|s| s.trim().to_string())
    });

    // Read workflow if specified
    let workflow = if let Some(ref wf_id) = workflow_id {
        let store = WorkflowStore::load(&cwd)?;
        store.get(wf_id).cloned()
    } else {
        None
    };

    // Read references
    let reg = ReferenceRegistry::read(&cwd).unwrap_or_default();
    let references: Vec<_> = reg.entries.iter().filter(|e| e.enabled).cloned().collect();

    // Get context hash
    let context_hash =
        route_basic::effective_context_fingerprint(&cwd).unwrap_or_else(|_| "unknown".to_string());

    // Build input and compile
    let input = CompilerInput {
        task,
        agent_policy,
        workflow,
        memory: None,
        references,
        context_hash,
        project_root: Some(cwd.clone()),
    };

    let plan = compile_plan(input)?;

    // Render or compile to host
    let output = match target.as_str() {
        "claude" | "codex" | "deepseek" => compile_to_host(&plan, &target)?,
        _ => render_plan(&plan),
    };

    if to_stdout {
        println!("{}", output);
    } else {
        let plan_dir = cwd.join(".route").join("agent-plans");
        std::fs::create_dir_all(&plan_dir)?;
        let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let plan_path = plan_dir.join(format!("agent-plan-{}.md", ts));
        std::fs::write(&plan_path, &output)?;
        println!("AgentPlan written to {}", plan_path.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Profile commands
// ---------------------------------------------------------------------------

/// List all profiles.
pub fn profile_list() -> Result<()> {
    use route_basic::ProfileStore;
    let cwd = current_project_root();
    let store = ProfileStore::load(&cwd)?;
    let active_id = ProfileStore::active_id(&cwd)?.unwrap_or_default();
    if store.profiles.is_empty() {
        println!("(no profiles defined)");
        return Ok(());
    }
    println!(
        "{:<24} {:<24} {:<8} {}",
        "ID", "NAME", "ACTIVE", "DESCRIPTION"
    );
    println!("{}", "-".repeat(90));
    for p in &store.profiles {
        let active = if p.id == active_id { "yes" } else { "" };
        println!(
            "{:<24} {:<24} {:<8} {}",
            p.id, p.name, active, p.description
        );
    }
    Ok(())
}

/// Show current active profile.
pub fn profile_show() -> Result<()> {
    use route_basic::ProfileStore;
    let cwd = current_project_root();
    let store = ProfileStore::load(&cwd)?;
    let active_id = ProfileStore::active_id(&cwd)?;
    match active_id {
        Some(id) => match store.get(&id) {
            Some(p) => {
                println!("Active profile: {} ({})", p.name, p.id);
                println!("  Description: {}", p.description);
                if !p.workflow_ids.is_empty() {
                    println!("  Workflows: {}", p.workflow_ids.join(", "));
                }
                if !p.reference_ids.is_empty() {
                    println!("  References: {}", p.reference_ids.join(", "));
                }
                if p.agent_policy_override.is_some() {
                    println!("  Agent policy override: yes");
                }
                if p.verification_policy_override.is_some() {
                    println!("  Verification policy override: yes");
                }
            }
            None => println!("Active profile '{}' not found in store", id),
        },
        None => println!("No active profile set"),
    }
    Ok(())
}

/// Use a specific profile.
pub fn profile_use(id: String) -> Result<()> {
    use route_basic::ProfileStore;
    let cwd = current_project_root();
    let store = ProfileStore::load(&cwd)?;
    if store.get(&id).is_none() {
        return Err(anyhow!("Profile '{}' not found", id));
    }
    ProfileStore::set_active(&cwd, &id)?;
    println!("✓ Switched to profile '{}'", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// P6: Context Explain — `route context task "<task>" --explain`
// ---------------------------------------------------------------------------

/// Show a detailed explanation of reference selection for a task-scoped
/// context. Outputs selected/excluded refs, scores, signals, budget usage,
/// learned confidence, and final context hash.
pub fn context_explain(task: String, target: Option<String>, top_k: usize) -> Result<()> {
    use route_basic::{build_context_explain, ContextBudget};
    let cwd = current_project_root();
    let budget = ContextBudget::default();
    let result = build_context_explain(&cwd, &task, target.as_deref(), top_k, budget)?;

    println!("# Context Explain: {}", result.task);
    if let Some(t) = &result.target {
        println!("Target: {}", t);
    }
    println!();
    println!("## Context Hash");
    println!("  {}", result.context_hash);
    println!();
    println!("## Budget");
    println!(
        "  Used: {} chars (max: {})",
        result.budget_used_chars, result.budget_max_chars
    );
    println!(
        "  - Constitution: {} chars (reserved: {})",
        result.constitution_len, budget.reserved_constitution
    );
    println!(
        "  - Protocol: {} chars (reserved: {})",
        result.protocol_len, budget.reserved_protocol
    );
    println!("  - Max reference items: {}", budget.max_reference_items);
    println!();

    if result.selected.is_empty() {
        println!("## Selected References");
        println!("  (none)");
    } else {
        println!("## Selected References ({})", result.selected.len());
        for sr in &result.selected {
            println!();
            println!("  - ID:       {}", sr.reference_id);
            println!("    Score:    {:.2}", sr.score);
            if !sr.signals.is_empty() {
                println!("    Signals:  {}", sr.signals.join("; "));
            }
            if let Some(reason) = &sr.exclusion_reason {
                println!("    Reason:   {}", reason);
            }
        }
    }
    println!();

    if result.excluded.is_empty() {
        println!("## Excluded References");
        println!("  (none)");
    } else {
        println!("## Excluded References ({})", result.excluded.len());
        for sr in &result.excluded {
            println!();
            println!("  - ID:       {}", sr.reference_id);
            println!("    Score:    {:.2}", sr.score);
            if !sr.signals.is_empty() {
                println!("    Signals:  {}", sr.signals.join("; "));
            }
            if let Some(reason) = &sr.exclusion_reason {
                println!("    Reason:   {}", reason);
            }
        }
    }
    println!();

    if result.learned_injected {
        println!("## Learned Experiences: YES (injected into context)");
    } else {
        println!("## Learned Experiences: none selected");
    }
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// AI Delivery Adapter — `route apply`
// ---------------------------------------------------------------------------

pub fn apply_target(target: route_basic::ApplyTarget, task: Option<&str>) -> Result<()> {
    let cwd = current_project_root();
    let record = route_basic::apply_context(&cwd, target, task)?;
    println!(
        "✓ Applied {} context → {}",
        target.display_name(),
        target.relative_path()
    );
    println!("  context_hash:   {}", record.context_hash);
    println!("  generated_hash: {}", record.generated_hash);
    println!("  applied_at:     {}", record.applied_at);
    println!("\n  The managed block (<!-- ROUTE:BEGIN --> … <!-- ROUTE:END -->) contains");
    println!("  the compiled Effective Context. User content outside the block is preserved.");
    println!("  Run `route apply status` to check freshness.");
    Ok(())
}

pub fn apply_status() -> Result<()> {
    let cwd = current_project_root();
    let targets = [
        route_basic::ApplyTarget::Claude,
        route_basic::ApplyTarget::Codex,
        route_basic::ApplyTarget::Generic,
    ];
    for target in targets {
        let (status, applied, current) = route_basic::check_status(&cwd, target)?;
        let label = match status {
            route_basic::ApplyStatus::Current => "CURRENT",
            route_basic::ApplyStatus::Outdated => "OUTDATED",
            route_basic::ApplyStatus::NotApplied => "NOT APPLIED",
            route_basic::ApplyStatus::FileModified => "FILE_MODIFIED",
            route_basic::ApplyStatus::Malformed => "MALFORMED",
        };
        println!("{}", target.display_name());
        match status {
            route_basic::ApplyStatus::NotApplied => {
                println!("  Status: {label}");
            }
            _ => {
                println!("  Applied context: {applied}");
                println!("  Current context: {current}");
                println!("  Status: {label}");
            }
        }
        println!();
    }
    Ok(())
}

pub fn apply_verify(target: Option<String>) -> Result<()> {
    use route_basic::{verify_target, ApplyStatus, ApplyTarget};
    let cwd = current_project_root();

    let targets: Vec<ApplyTarget> = match target {
        Some(t) => vec![ApplyTarget::parse(&t)?],
        None => vec![
            ApplyTarget::Claude,
            ApplyTarget::Codex,
            ApplyTarget::Generic,
        ],
    };

    for t in targets {
        let info = verify_target(&cwd, t)?;
        let label = match info.status {
            ApplyStatus::Current => "CURRENT",
            ApplyStatus::Outdated => "CONTEXT_OUTDATED",
            ApplyStatus::NotApplied => "NOT_APPLIED",
            ApplyStatus::FileModified => "FILE_MODIFIED",
            ApplyStatus::Malformed => "MALFORMED",
        };
        println!("{}", info.target.display_name());
        println!("  Status: {}", label);
        println!("  Detail: {}", info.detail);
        if !info.applied_context_hash.is_empty() {
            println!("  Applied context hash: {}", info.applied_context_hash);
            println!("  Current context hash: {}", info.current_context_hash);
        }
        if !info.applied_generated_hash.is_empty() {
            println!("  Applied generated hash: {}", info.applied_generated_hash);
            if !info.actual_generated_hash.is_empty() {
                println!("  Actual generated hash:  {}", info.actual_generated_hash);
            }
        }
        println!();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Adaptive Learning CLI
// ---------------------------------------------------------------------------

/// Parse a scope string with optional value into a LearnScope.
fn parse_learn_scope(
    scope: Option<String>,
    scope_value: Option<String>,
) -> route_basic::LearnScope {
    use route_basic::LearnScope;
    match scope.as_deref() {
        Some("task-pattern") => LearnScope::TaskPattern(scope_value.unwrap_or_default()),
        Some("tool-or-host") => LearnScope::ToolOrHost(scope_value.unwrap_or_default()),
        _ => LearnScope::Project,
    }
}

/// Parse an event kind string into an EventKind.
fn parse_event_kind(kind: &str) -> Result<route_basic::EventKind> {
    use route_basic::EventKind;
    match kind.to_ascii_lowercase().as_str() {
        "user_accept" | "user-accept" => Ok(EventKind::UserAccept),
        "user_reject" | "user-reject" => Ok(EventKind::UserReject),
        "rollback" => Ok(EventKind::Rollback),
        "test_pass" | "test-pass" => Ok(EventKind::TestPass),
        "test_fail" | "test-fail" => Ok(EventKind::TestFail),
        "agent_result" | "agent-result" => Ok(EventKind::AgentResult),
        "manual_note" | "manual-note" => Ok(EventKind::ManualNote),
        other => Err(anyhow::anyhow!(
            "unknown event kind '{}': valid kinds are user_accept, user_reject, rollback, test_pass, test_fail, agent_result, manual_note",
            other
        )),
    }
}

pub fn learn_record(
    kind: String,
    outcome: String,
    evidence: Option<String>,
    task: Option<String>,
    scope: Option<String>,
    scope_value: Option<String>,
    tags: Vec<String>,
    provenance: Option<String>,
) -> Result<()> {
    use route_basic::ExperienceStore;
    let cwd = current_project_root();

    let kind = parse_event_kind(&kind)?;
    let scope = parse_learn_scope(scope, scope_value);
    let evidence = evidence.unwrap_or_default();
    let tags = if tags.is_empty() {
        vec![kind.as_str().to_string()]
    } else {
        tags
    };

    let mut store = ExperienceStore::load(&cwd)?;
    let event_id = store.record(
        &cwd,
        kind,
        &outcome,
        &evidence,
        scope,
        task.as_deref(),
        tags,
        None,
        provenance,
    )?;
    store.save(&cwd)?;

    println!("✓ Experience event recorded: {}", event_id);
    println!("  kind: {}", kind.as_str());
    println!("  outcome: {}", outcome);
    if let Some(t) = task {
        println!("  task: {}", t);
    }
    Ok(())
}

pub fn learn_review(json_out: bool) -> Result<()> {
    use route_basic::LearningProposalStore;
    let cwd = current_project_root();

    let store = LearningProposalStore::load(&cwd)?;
    let open: Vec<_> = store.open_proposals();

    if open.is_empty() {
        println!("No open learning proposals. Run `route learn analyze` first.");
        return Ok(());
    }

    if json_out {
        let wrapper = serde_json::json!({
            "count": open.len(),
            "proposals": open,
        });
        println!("{}", serde_json::to_string_pretty(&wrapper)?);
        return Ok(());
    }

    println!("Open Learning Proposals:");
    for p in &open {
        println!();
        println!("  id:         {}", p.id);
        println!("  claim:      {}", p.claim);
        println!("  scope:      {}", p.scope.as_str());
        println!("  confidence: {:.2}", p.confidence);
        println!("  positive:   {}", p.positive);
        println!("  negative:   {}", p.negative);
        println!("  reason:     {}", p.reason);
        println!("  status:     {:?}", p.status);
    }
    println!();
    println!("{} open proposal(s).", open.len());
    println!("Use `route learn apply <id>` to promote or `route learn reject <id> --reason ...` to reject.");
    Ok(())
}

pub fn learn_apply(proposal_id: String) -> Result<()> {
    use route_basic::apply_learning_proposal;
    let cwd = current_project_root();

    let proposal = apply_learning_proposal(&cwd, &proposal_id)?;
    println!("✓ Learning proposal applied: {}", proposal.id);
    println!("  claim:      {}", proposal.claim);
    println!("  confidence: {:.2}", proposal.confidence);
    println!("  scope:      {}", proposal.scope.as_str());
    println!("The learned experience is now available in the Reference Registry.");
    println!("Use `route context task --task <task>` to see it in context.");
    Ok(())
}

pub fn learn_reject(proposal_id: String, reason: String) -> Result<()> {
    use route_basic::reject_learning_proposal;
    let cwd = current_project_root();

    let proposal = reject_learning_proposal(&cwd, &proposal_id, &reason)?;
    println!("✓ Learning proposal rejected: {}", proposal.id);
    println!("  claim:  {}", proposal.claim);
    println!("  reason: {}", reason);
    println!("A rejection event has been recorded. The same proposal will not be re-generated.");
    Ok(())
}

pub fn learn_history(id: Option<String>) -> Result<()> {
    use route_basic::build_audit;
    let cwd = current_project_root();

    let claim_or_id = id.unwrap_or_else(|| "".to_string());
    let audit = build_audit(&cwd, &claim_or_id)?;

    if audit.entries.is_empty() {
        println!("No audit trail found for '{}'.", claim_or_id);
        println!("Record some events with `route learn record`, then analyze and apply.");
        return Ok(());
    }

    println!("Learning Audit Trail");
    println!("  claim:        {}", audit.claim);
    if let Some(ref pid) = audit.proposal_id {
        println!("  proposal_id:  {}", pid);
    }
    if let Some(ref rid) = audit.reference_id {
        println!("  reference_id: {}", rid);
    }
    println!();
    println!("Events ({})", audit.entries.len());
    println!("---");
    for entry in &audit.entries {
        let ts = entry.timestamp;
        let kind = match entry.kind {
            route_basic::AuditKind::EventRecorded => "EVENT",
            route_basic::AuditKind::ProposalGenerated => "PROPOSAL",
            route_basic::AuditKind::ProposalApproved => "APPROVED",
            route_basic::AuditKind::ProposalRejected => "REJECTED",
            route_basic::AuditKind::ReferenceCreated => "REFERENCE",
            route_basic::AuditKind::ConfidenceRecomputed => "RECOMPUTE",
            route_basic::AuditKind::MarkedStale => "STALE",
            route_basic::AuditKind::Superseded => "SUPERSEDED",
        };
        println!("  [{}] {} {}", ts, kind, entry.detail);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Profile / workflow resolution helpers
// ---------------------------------------------------------------------------

/// Resolve a profile id. If `explicit` is Some, use it directly.
/// If None, use the active profile. Returns Ok(None) if no profile is set.
fn resolve_profile(
    project_root: &std::path::Path,
    explicit: Option<String>,
) -> Result<Option<String>> {
    use route_basic::ProfileStore;
    match explicit {
        Some(id) => {
            // Verify the profile exists
            let store = ProfileStore::load(project_root)?;
            if store.get(&id).is_none() {
                anyhow::bail!("Profile '{}' not found", id);
            }
            Ok(Some(id))
        }
        None => Ok(ProfileStore::active_id(project_root).ok().flatten()),
    }
}

/// Resolve a workflow id. If `explicit` is Some, use it directly.
/// If None, use the first workflow from the active profile.
/// Returns Ok(None) if no workflow is selected.
fn resolve_workflow(
    project_root: &std::path::Path,
    explicit: Option<String>,
    profile_id: Option<&str>,
) -> Result<Option<String>> {
    use route_basic::{ProfileStore, WorkflowStore};
    if let Some(id) = explicit {
        // Verify the workflow exists
        let store = WorkflowStore::load(project_root)?;
        if store.get(&id).is_none() {
            anyhow::bail!("Workflow '{}' not found", id);
        }
        return Ok(Some(id));
    }
    // Try to get from active profile
    if let Some(pid) = profile_id {
        let store = ProfileStore::load(project_root)?;
        if let Some(profile) = store.get(pid) {
            if !profile.workflow_ids.is_empty() {
                return Ok(Some(profile.workflow_ids[0].clone()));
            }
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Task execution commands
// ---------------------------------------------------------------------------

/// Begin a new execution session.
pub fn task_begin(
    task: String,
    target: String,
    concurrent: bool,
    profile: Option<String>,
    workflow: Option<String>,
) -> Result<()> {
    use route_basic::{begin_session, ApplyTarget, ProfileStore};

    let cwd = current_project_root();
    let target = ApplyTarget::parse(&target)?;

    // Resolve profile: explicit override or active profile
    let resolved_profile = resolve_profile(&cwd, profile.clone())?;
    if let Some(ref p) = resolved_profile {
        // If explicitly requested, set the profile as active
        if profile.is_some() {
            ProfileStore::set_active(&cwd, p)?;
        }
        println!("  Profile:        {}", p);
    }

    // Resolve workflow: explicit override or from profile
    let workflow_id = resolve_workflow(&cwd, workflow, resolved_profile.as_deref())?;
    if let Some(ref wf) = workflow_id {
        println!("  Workflow:       {}", wf);
    }

    let session = begin_session(&cwd, &task, target, concurrent, None)?;

    println!("✓ Task session created");
    println!("  id:             {}", session.id);
    println!("  task:           {}", session.task);
    println!("  task_hash:      {}", session.task_hash);
    println!("  target:         {}", session.target.as_str());
    println!("  context_hash:   {}", session.context_hash);
    if !session.selected_reference_ids.is_empty() {
        println!(
            "  selected_refs:  {}",
            session.selected_reference_ids.join(", ")
        );
    }
    if let Some(ref policy_hash) = session.agent_policy_hash {
        println!("  policy_hash:    {}", policy_hash);
    }
    println!("  status:         Active");
    println!();
    println!(
        "To end this session: `route task end {} --result <success|failed|aborted>`",
        &session.id[..12]
    );

    Ok(())
}

/// Show session status.
pub fn task_status(id: Option<String>) -> Result<()> {
    use route_basic::session_status;

    let cwd = current_project_root();
    let sessions = session_status(&cwd, id.as_deref())?;

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    for session in &sessions {
        let status_str = match session.status {
            route_basic::SessionStatus::Active => "Active",
            route_basic::SessionStatus::Succeeded => "Succeeded",
            route_basic::SessionStatus::Failed => "Failed",
            route_basic::SessionStatus::RolledBack => "RolledBack",
            route_basic::SessionStatus::Aborted => "Aborted",
            route_basic::SessionStatus::ForcedUnverified => "ForcedUnverified",
        };
        println!(
            "  {:<12}  {:<8}  {}",
            &session.id[..12],
            status_str,
            session.task
        );
    }

    Ok(())
}

/// End a session with a result.
pub fn task_end(id: String, result: String) -> Result<()> {
    use route_basic::end_session;

    let cwd = current_project_root();
    let session = end_session(&cwd, &id, &result)?;

    let status_str = match session.status {
        route_basic::SessionStatus::Succeeded => "Succeeded",
        route_basic::SessionStatus::Failed => "Failed (verification policy not satisfied)",
        route_basic::SessionStatus::Aborted => "Aborted",
        _ => "Unknown",
    };

    // P7: Generate learning signals on session end.
    let _ = route_basic::generate_learning_from_session(&cwd, &session);

    println!("✓ Session ended");
    println!("  id:     {}", &session.id[..12]);
    println!("  status: {}", status_str);
    Ok(())
}

/// Force-end a session with ForcedUnverified status (P3).
pub fn task_force_end(id: String) -> Result<()> {
    use route_basic::force_end_session;

    let cwd = current_project_root();
    let session = force_end_session(&cwd, &id)?;

    println!("⚠ Session force-ended (ForcedUnverified)");
    println!("  id:     {}", &session.id[..12]);
    println!("  status: ForcedUnverified");
    println!("  note:   This is a permanent audit marker. Cannot be used as strong positive learning signal.");
    Ok(())
}

/// Execute a command under a session (P2).
pub fn task_exec(session: String, argv: Vec<String>, check_id: Option<String>) -> Result<()> {
    use route_basic::exec_command;

    let cwd = current_project_root();
    let evidence = exec_command(&cwd, &session, &argv, check_id.as_deref())?;

    let status_str = if evidence.exit_code == 0 {
        "PASS"
    } else {
        "FAIL"
    };
    println!("Command: {}", argv.join(" "));
    println!("  exit_code: {}", evidence.exit_code);
    println!("  duration:  {}ms", evidence.duration_ms);
    println!("  status:    {}", status_str);
    println!("  state:     {}", &evidence.state_hash[..16]);
    if evidence.exit_code != 0 {
        // Show stderr preview on failure.
        if !evidence.stderr_preview.is_empty() {
            println!("  stderr:");
            for line in evidence.stderr_preview.lines().take(10) {
                println!("    {}", line);
            }
        }
    }
    Ok(())
}

/// Verify a session's verification policy (P3).
pub fn task_verify(id: String, json: bool) -> Result<()> {
    use route_basic::verify_session;

    let cwd = current_project_root();
    let result = verify_session(&cwd, &id)?;

    let status_str = match result {
        route_basic::VerifyResult::Verified => "VERIFIED",
        route_basic::VerifyResult::Failed => "FAILED",
        route_basic::VerifyResult::Incomplete => "INCOMPLETE",
    };

    if json {
        let output = serde_json::json!({
            "session_id": id,
            "result": status_str,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!(
        "Verification result for session '{}': {}",
        &id[..12],
        status_str
    );

    match result {
        route_basic::VerifyResult::Verified => {
            println!("✓ All required checks are satisfied with valid system evidence.");
        }
        route_basic::VerifyResult::Failed => {
            println!("✗ One or more checks have stale evidence. Re-run required checks.");
        }
        route_basic::VerifyResult::Incomplete => {
            println!("! Some required checks are missing. Run `route task exec` to execute them.");
        }
    }
    Ok(())
}

/// One-command entry: start a new task session (P4).
pub fn task_start(
    task: String,
    target: String,
    profile: Option<String>,
    workflow: Option<String>,
    strategy: Option<String>,
    savepoint: bool,
    campaign_id: Option<String>,
) -> Result<()> {
    use route_basic::{start_task_session, ApplyTarget, ProfileStore, StrategyStore};

    let cwd = current_project_root();
    let target = ApplyTarget::parse(&target)?;

    // P3: Auto savepoint before task start
    if savepoint {
        let savepoint_name = format!("pre-task:{}", &task[..task.len().min(40)]);
        match savepoint_create(savepoint_name, None) {
            Ok(_) => println!("  Savepoint:      auto-created before task"),
            Err(e) => {
                // Non-fatal: task can continue without savepoint
                tracing::warn!("  Savepoint:      failed to auto-create ({})", e);
            }
        }
    }

    // If a strategy is specified, load and apply it first.
    if let Some(ref strategy_id) = strategy {
        let store = StrategyStore::load(&cwd)?;
        let snapshot = store
            .list()
            .into_iter()
            .find(|s| s.id == *strategy_id || s.id.starts_with(strategy_id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Strategy snapshot '{}' not found", strategy_id))?;
        println!(
            "  Strategy:       {} ({})",
            snapshot.id,
            snapshot.label.as_deref().unwrap_or("no label")
        );
        // Apply the strategy's profile
        let profile_store: route_basic::ProfileStore =
            serde_json::from_value(snapshot.profile.clone())?;
        profile_store.save(&cwd)?;
        if !snapshot.profile_id.is_empty() {
            ProfileStore::set_active(&cwd, &snapshot.profile_id)?;
        }
        println!("  Strategy profile: {}", snapshot.profile_id);
    }

    // Resolve profile: explicit override or active profile
    let resolved_profile = resolve_profile(&cwd, profile.clone())?;
    if let Some(ref p) = resolved_profile {
        // If explicitly requested, set the profile as active
        if profile.is_some() {
            ProfileStore::set_active(&cwd, p)?;
        }
        println!("  Profile:        {}", p);
    }

    // Resolve workflow: explicit override or from profile
    let workflow_id = resolve_workflow(&cwd, workflow, resolved_profile.as_deref())?;
    if let Some(ref wf) = workflow_id {
        println!("  Workflow:       {}", wf);
    }

    let result = start_task_session(&cwd, &task, target, strategy.clone(), campaign_id)?;

    println!("✓ Task session started");
    println!("  id:             {}", result.session.id);
    println!("  task:           {}", result.session.task);
    println!("  context_hash:   {}", result.session.context_hash);
    println!("  target:         {}", result.session.target.as_str());
    println!("  archived:       {}", result.archived);
    if let Some(ref sid) = strategy {
        println!("  strategy:       {}", sid);
    }
    println!();
    println!("Host instructions:");
    println!("{}", result.host_instructions);
    println!();
    println!(
        "To end: `route task end {} --result <success|failed|aborted>`",
        &result.session.id[..12]
    );
    Ok(())
}

/// Resume a session after restart (P6).
pub fn task_resume(id: String) -> Result<()> {
    use route_basic::resume_session;

    let cwd = current_project_root();
    let result = resume_session(&cwd, &id)?;

    println!("Resume result for session '{}':", &result.session.id[..12]);
    println!("  task:     {}", result.session.task);
    println!("  drift:    {}", result.drift.as_str());

    if result.details.is_empty() {
        println!("  ✓ No drift detected — session is active and current.");
    } else {
        println!("  ⚠ Drift details:");
        for detail in &result.details {
            println!("    - {}", detail);
        }
        println!();
        println!("  Drift is not auto-repaired. To continue:");
        println!(
            "    - Re-apply context: `route apply {}`",
            result.session.target.as_str()
        );
        println!("    - Resume working: continue with the session as-is");
    }
    Ok(())
}

/// Submit an execution report (AI feedback) for the session (P5).
pub fn task_report(id: String, observations: Vec<String>) -> Result<()> {
    use route_basic::{ingest_host_report, HostReport};

    let cwd = current_project_root();
    let report = HostReport {
        session_id: id.clone(),
        agent_roles_used: vec![],
        actions: vec![],
        verification_requested: vec![],
        observations,
    };
    let ids = ingest_host_report(&cwd, report)?;

    println!("✓ Report recorded for session '{}'", &id[..12]);
    println!("  evidence ids: {}", ids.len());
    for ev_id in &ids {
        println!("    - {}", ev_id);
    }
    Ok(())
}

/// Show detailed audit for a session.
pub fn task_show(id: String, explain: bool) -> Result<()> {
    use route_basic::show_session;

    let cwd = current_project_root();
    let audit = show_session(&cwd, &id, explain)?;

    println!("# Task Session Audit");
    println!();
    println!("## Session");
    println!("  id:             {}", audit.session.id);
    println!("  task:           {}", audit.session.task);
    println!("  task_hash:      {}", audit.session.task_hash);
    println!("  target:         {}", audit.session.target.as_str());
    let status_str = match audit.session.status {
        route_basic::SessionStatus::Active => "Active",
        route_basic::SessionStatus::Succeeded => "Succeeded",
        route_basic::SessionStatus::Failed => "Failed",
        route_basic::SessionStatus::RolledBack => "RolledBack",
        route_basic::SessionStatus::Aborted => "Aborted",
        route_basic::SessionStatus::ForcedUnverified => "ForcedUnverified",
    };
    println!("  status:         {}", status_str);
    if let Some(cid) = &audit.session.campaign_id {
        println!("  campaign_id:    {}", cid);
    }
    if let Some(ended_at) = audit.session.ended_at {
        println!("  ended_at:       {}", ended_at);
    }
    println!();

    println!("## Context");
    println!("  context_hash:   {}", audit.context_hash);
    if !audit.selected_reference_ids.is_empty() {
        println!(
            "  selected refs:  {}",
            audit.selected_reference_ids.join(", ")
        );
    }
    if let Some(ref policy_hash) = audit.agent_policy_hash {
        println!("  policy_hash:    {}", policy_hash);
    }
    println!();

    println!("## Evidence ({})", audit.evidence.len());
    for ev in &audit.evidence {
        let source_str = match ev.source {
            route_basic::EvidenceSource::System => "system",
            route_basic::EvidenceSource::Agent => "agent",
            route_basic::EvidenceSource::User => "user",
        };
        let meta_str: Vec<String> = ev
            .metadata
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let meta = if meta_str.is_empty() {
            String::new()
        } else {
            format!(" ({})", meta_str.join(", "))
        };
        // P8: Include state identity in evidence display.
        let state_info = if let Some(ref sh) = ev.state_hash {
            format!(" state={}", &sh[..12])
        } else {
            String::new()
        };
        println!(
            "  [{}] {}/{}{}: {}{}",
            &ev.id[..12],
            source_str,
            ev.kind.as_str(),
            state_info,
            &ev.payload_hash[..12],
            meta
        );
    }
    println!();

    if !audit.learning_proposals.is_empty() {
        println!("## Learning Proposals ({})", audit.learning_proposals.len());
        for claim in &audit.learning_proposals {
            println!("  - {}", claim);
        }
        println!();
    }

    // P8: Enhanced --explain output.
    if explain {
        println!("## Causal Chain Explanation");
        println!("---");

        // Build a structured explanation of the causal chain.
        println!();
        println!("### Task → Context");
        println!("  Task: '{}'", audit.session.task);
        println!("  Target: {}", audit.session.target.as_str());
        println!("  Context hash: {}", &audit.context_hash[..16]);
        if let Some(cid) = &audit.session.campaign_id {
            println!("  Campaign: {}", cid);
        }
        if !audit.selected_reference_ids.is_empty() {
            println!(
                "  Selected references: {}",
                audit.selected_reference_ids.join(", ")
            );
        }
        println!();

        println!("### Agent Policy");
        if let Some(ref hash) = audit.agent_policy_hash {
            println!("  Policy hash: {}", hash);
        } else {
            println!("  (no agent policy — using default single-agent mode)");
        }
        println!();

        println!("### Baseline");
        let baseline_evidence: Vec<&route_basic::Evidence> = audit
            .evidence
            .iter()
            .filter(|e| e.kind == route_basic::EvidenceKind::Snapshot)
            .collect();
        if !baseline_evidence.is_empty() {
            println!("  Baseline snapshots: {}", baseline_evidence.len());
            for ev in &baseline_evidence {
                let sid = ev
                    .metadata
                    .get("snapshot_id")
                    .map(|s| s.as_str())
                    .unwrap_or("?");
                println!(
                    "    - snapshot {} (evidence {})",
                    &sid[..12.min(sid.len())],
                    &ev.id[..12]
                );
            }
        } else {
            println!("  (no baseline snapshot recorded)");
        }
        println!();

        println!("### Execution Commands & Checks");
        let check_evidence: Vec<&route_basic::Evidence> = audit
            .evidence
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    route_basic::EvidenceKind::CheckPass
                        | route_basic::EvidenceKind::CheckFail
                        | route_basic::EvidenceKind::TestPass
                        | route_basic::EvidenceKind::TestFail
                )
            })
            .collect();
        if !check_evidence.is_empty() {
            for ev in &check_evidence {
                let argv = ev.metadata.get("argv").map(|s| s.as_str()).unwrap_or("?");
                let exit_code = ev
                    .metadata
                    .get("exit_code")
                    .map(|s| s.as_str())
                    .unwrap_or("?");
                let state = ev.state_hash.as_ref().map(|s| &s[..12]).unwrap_or("?");
                println!(
                    "  [{}] {} (exit={}, state={})",
                    &ev.id[..12],
                    argv,
                    exit_code,
                    state
                );
            }
        } else {
            println!("  (no commands executed)");
        }
        println!();

        println!("### Commits & Rollbacks");
        let commit_evidence: Vec<&route_basic::Evidence> = audit
            .evidence
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    route_basic::EvidenceKind::Commit | route_basic::EvidenceKind::Rollback
                )
            })
            .collect();
        if !commit_evidence.is_empty() {
            for ev in &commit_evidence {
                let detail = if ev.kind == route_basic::EvidenceKind::Commit {
                    let cid = ev
                        .metadata
                        .get("commit_id")
                        .map(|s| s.as_str())
                        .unwrap_or("?");
                    format!("commit {}", &cid[..12])
                } else {
                    let rid = ev
                        .metadata
                        .get("rollback_snapshot")
                        .map(|s| s.as_str())
                        .unwrap_or("?");
                    format!("rollback to snapshot {}", &rid[..12])
                };
                println!("  [{}] {}", &ev.id[..12], detail);
            }
        } else {
            println!("  (no commits or rollbacks)");
        }
        println!();

        // Verification explanation.
        println!("### Verification");
        let verification_evidence: Vec<&route_basic::Evidence> = audit
            .evidence
            .iter()
            .filter(|e| e.metadata.get("forced_unverified").map(|s| s.as_str()) == Some("true"))
            .collect();
        if !verification_evidence.is_empty() {
            println!("  ⚠ ForcedUnverified (user override)");
        } else {
            // Check if any evidence is stale.
            let cwd = std::env::current_dir()?;
            let repo = route_basic::BasicRepository::open(&cwd);
            let current_state = repo
                .ok()
                .and_then(|_| route_basic::compute_state_hash(&cwd).ok())
                .unwrap_or_default();
            if !current_state.is_empty() {
                let stale_count = audit
                    .evidence
                    .iter()
                    .filter(|e| {
                        if let Some(ref sh) = e.state_hash {
                            sh != &current_state
                        } else {
                            false
                        }
                    })
                    .count();
                if stale_count > 0 {
                    println!(
                        "  ⚠ {} stale evidence entries (state has changed since)",
                        stale_count
                    );
                } else {
                    println!(
                        "  ✓ All evidence is current for state {}",
                        &current_state[..12]
                    );
                }
            }
        }
        println!();

        // Learning explanation.
        println!("### Learning");
        if !audit.learning_proposals.is_empty() {
            println!("  {} proposals generated:", audit.learning_proposals.len());
            for claim in &audit.learning_proposals {
                println!("    - {}", claim);
            }
        } else {
            println!("  (no learning proposals generated)");
        }
        println!();
    }

    println!("## Timeline ({})", audit.timeline.len());
    println!("---");
    for entry in &audit.timeline {
        let ts = entry.timestamp;
        let ref_str = entry
            .ref_id
            .as_ref()
            .map(|r| format!(" ({})", r))
            .unwrap_or_default();
        println!("  [{}] {}{}", ts, entry.kind, ref_str);
        println!("         {}", entry.detail);
    }

    Ok(())
}

/// Replay the exact context for a session.
pub fn task_replay(id: String, target: Option<String>, to_stdout: bool) -> Result<()> {
    use route_basic::{replay_session, ApplyTarget};

    let cwd = current_project_root();
    let target = match target {
        Some(t) => Some(ApplyTarget::parse(&t)?),
        None => None,
    };

    let result = replay_session(&cwd, &id, target, to_stdout)?;

    if result.partial {
        println!("⚠  Partial replay — historical context archive missing.");
        if let Some(ref warning) = result.warning {
            println!("   {}", warning);
        }
    } else {
        println!("✓ Context replayed from archive");
        println!("  session:     {}", &result.session_id[..12]);
        println!("  context:     {}", &result.context_hash[..16]);
        if !to_stdout {
            // Write to file.
            let replay_path = cwd
                .join(".route")
                .join("context")
                .join(format!("replay-{}.md", &result.session_id[..12]));
            if let Some(parent) = replay_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating directory {}", parent.display()))?;
            }
            std::fs::write(&replay_path, &result.context_body)
                .with_context(|| format!("writing replay to {}", replay_path.display()))?;
            println!("  written to:  {}", replay_path.display());
        }
    }

    Ok(())
}

/// `route study <path>` — analyze a project and print the StudyReport.
pub fn study(path: String) -> Result<()> {
    let report = route_basic::study_project(&path)
        .with_context(|| format!("failed to study project at {path:?}"))?;

    let cwd = std::env::current_dir()?;
    let project_root = find_project_root_or(&cwd);
    if let Some(root) = &project_root {
        let _ = route_basic::save_report(root, &report);

        // Create a StudyRecord from the report and add to the library
        let record = study_report_to_record(&report, &path);
        if let Err(e) = route_basic::study_library_add(root, record) {
            eprintln!("[warning] failed to save study to library: {e}");
        }
    }

    let formatted = route_basic::format_report(&report);
    println!("{}", formatted);
    Ok(())
}

/// Convert a StudyReport into a StudyRecord for library storage.
fn study_report_to_record(
    report: &route_basic::StudyReport,
    source: &str,
) -> route_basic::StudyRecord {
    use chrono::Utc;

    let patterns: Vec<String> = report
        .patterns
        .iter()
        .map(|p| format!("{}: {}", p.name, p.description))
        .collect();
    let workflows: Vec<String> = report
        .workflows
        .iter()
        .map(|w| format!("{}: {} (source: {})", w.name, w.description, w.source))
        .collect();
    let skills: Vec<String> = report
        .skills
        .iter()
        .map(|s| format!("{}: {} (source: {})", s.name, s.description, s.source))
        .collect();
    let decisions: Vec<String> = report
        .interesting_decisions
        .iter()
        .map(|d| format!("{}: {} (source: {})", d.title, d.description, d.source))
        .collect();

    let mut evidence: Vec<String> = Vec::new();
    for wf in &report.workflows {
        evidence.push(format!("workflow '{}' evidence: {}", wf.name, wf.evidence));
    }
    for sk in &report.skills {
        evidence.push(format!("skill '{}' evidence: {}", sk.name, sk.evidence));
    }
    for pt in &report.patterns {
        evidence.push(format!("pattern '{}' evidence: {}", pt.name, pt.evidence));
    }
    for dec in &report.interesting_decisions {
        evidence.push(format!(
            "decision '{}' evidence: {}",
            dec.title, dec.evidence
        ));
    }
    for cand in &report.candidates {
        evidence.push(format!(
            "candidate '{}' evidence: {}",
            cand.name, cand.evidence
        ));
    }

    route_basic::StudyRecord {
        id: ulid::Ulid::new().to_string(),
        source: source.to_string(),
        summary: report.summary.clone(),
        architecture: report.architecture.clone(),
        patterns,
        workflows,
        skills,
        decisions,
        evidence,
        applicability: report.applicability.clone(),
        created_at: Utc::now().timestamp(),
        tags: Vec::new(),
    }
}

/// `route study-list` — list all studies in the library.
pub fn study_list() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("not inside a Route project. Run `route init` first."))?;
    let lib = route_basic::study_library_load(&root)?;

    if lib.studies.is_empty() {
        println!("(no studies in library)");
        return Ok(());
    }

    println!("Study Library:");
    println!("{:-<80}", "");
    for s in &lib.studies {
        let ts = chrono::DateTime::from_timestamp(s.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| s.created_at.to_string());
        println!("  ID:    {}", s.id);
        println!("  Source: {}", s.source);
        println!("  Date:  {}", ts);
        let tags_str = if s.tags.is_empty() {
            "(none)".to_string()
        } else {
            s.tags.join(", ")
        };
        println!("  Tags:  {}", tags_str);
        if !s.summary.is_empty() {
            let first_line = s.summary.lines().next().unwrap_or("");
            println!("  Summary: {}", first_line);
        }
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route study-show <id>` — show a specific study.
pub fn study_show(id: String) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("not inside a Route project. Run `route init` first."))?;
    let record = route_basic::study_library_get(&root, &id)?
        .ok_or_else(|| anyhow!("study not found: {}", id))?;

    let ts = chrono::DateTime::from_timestamp(record.created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| record.created_at.to_string());

    println!("## Study: {}", record.id);
    println!("- Source: {}", record.source);
    println!("- Created: {}", ts);
    if !record.tags.is_empty() {
        println!("- Tags: {}", record.tags.join(", "));
    }
    println!();

    println!("### Summary");
    for line in record.summary.lines() {
        println!("  {}", line);
    }
    println!();

    println!("### Architecture");
    for line in record.architecture.lines() {
        println!("  {}", line);
    }
    println!();

    if !record.patterns.is_empty() {
        println!("### Patterns");
        for p in &record.patterns {
            println!("  - {}", p);
        }
        println!();
    }

    if !record.workflows.is_empty() {
        println!("### Workflows");
        for w in &record.workflows {
            println!("  - {}", w);
        }
        println!();
    }

    if !record.skills.is_empty() {
        println!("### Skills");
        for s in &record.skills {
            println!("  - {}", s);
        }
        println!();
    }

    if !record.decisions.is_empty() {
        println!("### Decisions");
        for d in &record.decisions {
            println!("  - {}", d);
        }
        println!();
    }

    println!("### Applicability");
    for line in record.applicability.lines() {
        println!("  {}", line);
    }
    println!();

    if !record.evidence.is_empty() {
        println!("### Evidence");
        for e in &record.evidence {
            println!("  - {}", e);
        }
        println!();
    }

    Ok(())
}

/// `route study-diff <a> <b>` — diff two studies.
pub fn study_diff(a: String, b: String) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("not inside a Route project. Run `route init` first."))?;
    let diff = route_basic::study_library_diff(&root, &a, &b)?;
    println!("{}", diff);
    Ok(())
}

/// `route study-compare <ids>...` — compare multiple studies.
pub fn study_compare(ids: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("not inside a Route project. Run `route init` first."))?;
    let result = route_basic::study_library_compare(&root, &ids)?;
    println!("{}", result);
    Ok(())
}

/// Snapshot Route's current standard files as a new self-version.
///
/// `from` may be a directory (all files, recursive) or a single file.
/// Snapshots are append-only — an old version is never overwritten.
pub fn self_archive_archive(from: &Path, message: &str, route_version: &str) -> Result<()> {
    let mut files: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if from.is_dir() {
        collect_dir_files(from, "", &mut files)?;
    } else if from.is_file() {
        let name = from
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .ok_or_else(|| anyhow!("invalid source file"))?;
        let content = std::fs::read_to_string(from)
            .with_context(|| format!("failed to read {}", from.display()))?;
        files.insert(name, content);
    } else {
        anyhow::bail!("`--from` path does not exist: {}", from.display());
    }
    if files.is_empty() {
        anyhow::bail!("no files found under {}", from.display());
    }

    let version = route_basic::self_archive::archive_current(&files, route_version, message)?;
    println!(
        "📦 Archived self-version {} (seq {}) at:",
        version.meta.version_id, version.meta.seq
    );
    println!(
        "   {}",
        route_basic::self_archive::self_archive_root()?.display()
    );
    println!("   files: {}", version.meta.files.join(", "));
    Ok(())
}

fn collect_dir_files(
    dir: &Path,
    prefix: &str,
    out: &mut std::collections::HashMap<String, String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let logical = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if path.is_dir() {
            collect_dir_files(&path, &logical, out)?;
        } else if let Ok(content) = std::fs::read_to_string(&path) {
            out.insert(logical, content);
        }
    }
    Ok(())
}

/// List all archived self-versions (history).
pub fn self_archive_list() -> Result<()> {
    let versions = route_basic::self_archive::list()?;
    if versions.is_empty() {
        println!("No self-versions archived yet.");
        println!("  Try: route self-archive archive --from <dir> --message \"why\"");
        return Ok(());
    }
    println!("📚 Route self-archive history ({}):", versions.len());
    for v in versions.iter().rev() {
        println!(
            "  v{:06}  {}  route={}  {}",
            v.seq,
            chrono_like_ts(v.created_at),
            v.route_version,
            if v.message.is_empty() {
                "(no message)"
            } else {
                &v.message
            }
        );
        print!("       files: ");
        println!("{}", v.files.join(", "));
    }
    Ok(())
}

fn chrono_like_ts(millis: i64) -> String {
    // Compact UTC timestamp without adding chrono to the CLI.
    let secs = millis / 1000;
    let days = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("+{}d {:02}:{:02}:{:02}", days, h, m, s)
}

/// Show a specific archived self-version, optionally as JSON.
pub fn self_archive_show(seq: u64, json: bool) -> Result<()> {
    let v = match route_basic::self_archive::get(seq)? {
        Some(v) => v,
        None => anyhow::bail!("no self-version with seq {seq}"),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    println!("Self-version v{:06} ({})", v.meta.seq, v.meta.version_id);
    println!("  route {}", v.meta.route_version);
    println!("  message: {}", v.meta.message);
    for (name, content) in v.file_contents.iter() {
        println!("  --- {name} ---");
        println!("{}", content.trim_end());
        println!();
    }
    Ok(())
}

/// Roll the self-archive back to a version.
///
/// Prints the archived files, or writes them under `out` when provided so the
/// harness/user can adopt them. Never mutates the repo itself.
pub fn self_archive_apply(seq: u64, out: Option<PathBuf>) -> Result<()> {
    let v = route_basic::self_archive::apply(seq)?;
    match out {
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;
            for (name, content) in &v.file_contents {
                let safe = route_basic::game_save::sanitize_dir_name(name);
                let dest = dir.join(&safe);
                std::fs::write(&dest, content)?;
            }
            println!(
                "Applied self-version v{:06} to {}",
                v.meta.seq,
                dir.display()
            );
        }
        None => {
            for (name, content) in v.file_contents.iter() {
                println!("--- {name} ---");
                println!("{}", content.trim_end());
                println!();
            }
        }
    }
    Ok(())
}

/// `route self-archive git-init`: ensure the document-area archive is a
/// local-only (no remote) git repository. Idempotent.
pub fn self_archive_git_init() -> Result<()> {
    let root = route_basic::backup::git_init_archive_no_remote()?;
    println!("✓ Document-area git backup is ready (LOCAL ONLY, no remote)");
    println!("  repo: {}", root.display());
    Ok(())
}

/// `route self-archive git-commit`: snapshot the whole archive as a local git
/// commit (history tracking for the document area). No-op when nothing changed.
pub fn self_archive_git_commit(message: &str) -> Result<()> {
    match route_basic::backup::git_commit_archive(message)? {
        Some(sha) => {
            println!("✓ Archived document area → [{sha}] {message}");
            println!(
                "  repo: {}",
                route_basic::backup::archive_repo_root()?.display()
            );
        }
        None => println!("ℹ No changes in the document area to commit."),
    }
    Ok(())
}

/// `route self-archive git-log`: show the local backup history.
pub fn self_archive_git_log(limit: usize) -> Result<()> {
    let log = route_basic::backup::git_log_archive(limit)?;
    if log.trim().is_empty() {
        println!(
            "No backups yet. Run `route self-archive archive` or `route self-archive git-commit`."
        );
        return Ok(());
    }
    println!("📚 Document-area backup history (local git, no remote):");
    print!("{}", log);
    Ok(())
}

/// `route self-sop`: upgrade Route's own development constraints into a
/// Standard Operating Procedure from real project memory + standards, then
/// persist it as a versioned capability in the central (unified) archive.
pub fn self_sop() -> Result<()> {
    let root = std::env::current_dir()?;
    let path = route_basic::sop::generate_and_write(&root)?;
    println!("✓ SOP generated from project memory + standards.");
    println!("  written: {}", path.display());

    let seq = route_basic::sop::persist_sop_capability(&root)?;
    println!(
        "📦 Persisted as persistent capability → self-archive v{:06}",
        seq
    );
    println!(
        "  {}, local-only git backup",
        route_basic::self_archive::self_archive_root()?.display()
    );
    Ok(())
}

/// Emit cross-project self-evolution input, read-only.
pub fn self_evolve() -> Result<()> {
    let input = route_basic::self_archive::collect_self_evolve_input()?;
    println!(
        "🧬 Cross-project self-evolution input (project_count: {}):",
        input.project_count
    );
    println!("   latest self-version: {:?}", input.latest_self_version);
    for p in &input.projects {
        println!();
        println!("  - {}", p.project_name);
        println!("      id: {}", p.project_id);
        println!(
            "      known_path: {}",
            p.known_path.as_deref().unwrap_or("(none)")
        );
        println!(
            "      save_count: {}, latest_save: {}",
            p.save_count,
            p.latest_save_id.as_deref().unwrap_or("(none)")
        );
        if let Some(c) = &p.constitution_preview {
            for line in c.lines() {
                println!("      | {}", line);
            }
        }
    }
    Ok(())
}

/// `route self-improve` — study Route itself and generate improvement proposals.
///
/// 1. Studies Route itself (current directory)
/// 2. Saves the study to the library
/// 3. Creates Pattern proposals from the study
/// 4. Creates Workflow evolution proposals
/// 5. Creates Memory proposals
/// 6. Prints a summary of all proposals
/// 7. Does NOT auto-apply anything
pub fn self_improve() -> Result<()> {
    use route_basic::{
        generate_evolve_proposal,
        memory::MemoryStore,
        memory::ProjectMemory,
        study::{study_library_add, study_project},
        PatternStore, WorkflowChangeProposal, WorkflowProposalStore,
    };

    let cwd = std::env::current_dir()?;
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("not inside a Route project. Run `route init` first."))?;
    let path_str = cwd.to_string_lossy().to_string();

    println!("🔍 Studying Route itself at: {}", root.display());
    println!();

    // 1. Study Route itself
    let report = study_project(&path_str).with_context(|| "failed to study Route project")?;

    // 2. Save the study to the library
    let record = study_report_to_record(&report, &path_str);
    study_library_add(&root, record.clone())?;
    println!("📚 Study report saved to library (ID: {})", &record.id);
    println!();

    // Print the formatted report
    let summary_line = report.summary.lines().next().unwrap_or("(no summary)");
    println!("📋 Study Summary: {}", summary_line);
    println!(
        "   Architecture: {} lines",
        report.architecture.lines().count()
    );
    println!("   Patterns: {} detected", report.patterns.len());
    println!("   Workflows: {} detected", report.workflows.len());
    println!(
        "   Decisions: {} detected",
        report.interesting_decisions.len()
    );
    println!("   Candidates: {} proposed", report.candidates.len());
    println!();

    // 3. Create Pattern proposals from the study
    let mut pattern_store = PatternStore::load(&root)?;
    let mut pattern_proposals = Vec::new();

    // Propose patterns from the study's detected patterns
    for study_pattern in &report.patterns {
        let proposal = PatternStore::propose_from_study(
            &record,
            &study_pattern.name,
            &format!("Need to understand and apply {}", study_pattern.name),
            &study_pattern.description,
        );
        pattern_store.proposals.push(proposal.clone());
        pattern_proposals.push(proposal);
    }
    pattern_store.save(&root)?;
    println!("📐 Pattern Proposals:");
    if pattern_proposals.is_empty() {
        println!("  (none generated)");
    } else {
        for p in &pattern_proposals {
            println!(
                "  [{}] {} — confidence: {:.2}",
                p.pattern.name, p.reason, p.pattern.confidence
            );
        }
    }
    println!();

    // 4. Create Workflow evolution proposals
    let mut wf_proposal_store = WorkflowProposalStore::load(&root)?;
    let mut wf_proposals = Vec::new();

    // Generate evolution proposals for existing workflows
    let wf_ids = ["wf-cargo-build", "wf-readme-study", "demo-workflow"];
    for wf_id in &wf_ids {
        let proposal = generate_evolve_proposal(
            &root,
            wf_id,
            "Self-dogfood: Route should evolve its own workflows based on study findings",
            "Improved workflow definition with better step descriptions and references",
            "Low risk — proposal only, no auto-apply",
        );
        wf_proposal_store.add(proposal.clone());
        wf_proposals.push(proposal);
    }

    // Generate a new workflow proposal from self-improvement study candidates
    for candidate in &report.candidates {
        if candidate.kind == "workflow" {
            let wf = route_basic::workflow_from_candidate(candidate, None);
            // Save as a workflow definition (proposal)
            let proposal = WorkflowChangeProposal {
                id: route_core::new_id(),
                workflow_id: wf.id.clone(),
                before: "Workflow does not exist yet".to_string(),
                after: format!("New workflow '{}' from study candidate", wf.id),
                reason: candidate.reason.clone(),
                evidence: vec![candidate.evidence.clone()],
                expected_effect: format!("Import workflow '{}' from study", wf.id),
                risk: "Low — proposal only, no auto-apply".to_string(),
                from_revision: 0,
                to_revision: 1,
                status: "pending".to_string(),
                created_at: route_core::now_millis(),
            };
            wf_proposal_store.add(proposal.clone());
            wf_proposals.push(proposal);
        }
    }
    wf_proposal_store.save(&root)?;
    println!("⚙️  Workflow Evolution Proposals:");
    if wf_proposals.is_empty() {
        println!("  (none generated)");
    } else {
        for p in &wf_proposals {
            println!("  [{}] {} → {}", p.workflow_id, p.before, p.after);
            println!("         reason: {}", p.reason);
        }
    }
    println!();

    // 5. Create Memory proposals
    let mut mem_store = MemoryStore::load(&root)?;
    let memory = ProjectMemory::refresh(&root, None)?;
    mem_store.memories.push(memory.clone());
    mem_store.save(&root)?;
    println!("🧠 Memory Proposals:");
    if memory.decisions.is_empty()
        && memory.known_risks.is_empty()
        && memory.failed_attempts.is_empty()
        && memory.conventions.is_empty()
        && memory.open_questions.is_empty()
    {
        println!("  (none generated)");
    } else {
        for item in &memory.decisions {
            println!(
                "  [decision] {} (conf: {:.2})",
                item.content, item.confidence
            );
        }
        for item in &memory.known_risks {
            println!("  [risk] {} (conf: {:.2})", item.content, item.confidence);
        }
        for item in &memory.failed_attempts {
            println!(
                "  [failed_attempt] {} (conf: {:.2})",
                item.content, item.confidence
            );
        }
        for item in &memory.conventions {
            println!(
                "  [convention] {} (conf: {:.2})",
                item.content, item.confidence
            );
        }
        for item in &memory.open_questions {
            println!(
                "  [open_question] {} (conf: {:.2})",
                item.content, item.confidence
            );
        }
    }
    println!();

    // 6. Print summary of all proposals
    println!("{:=^60}", " Summary ");
    println!("  Study Record ID:  {}", record.id);
    println!(
        "  Pattern Proposals: {} (saved, status: pending)",
        pattern_proposals.len()
    );
    println!(
        "  Workflow Proposals: {} (saved, status: pending)",
        wf_proposals.len()
    );
    println!("  Memory Proposals:  1 memory snapshot saved");
    println!();
    println!("📌 All proposals are in 'pending' status — nothing auto-applied.");
    println!("📌 Use `route pattern list` to view pattern proposals.");
    println!("📌 Use `route workflow` commands to manage workflow proposals.");
    println!("📌 Use `route memory` commands to review memory proposals.");
    println!();
    println!("✅ Self-improvement analysis complete.");

    Ok(())
}

/// Find the project root, or use the given path as fallback.
fn find_project_root_or(path: &Path) -> Option<PathBuf> {
    if route_core::RoutePaths::new(path).is_initialized() {
        return Some(path.to_path_buf());
    }
    let mut current = path;
    loop {
        if current.join(".route-basic").join("config.json").exists() {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

/// `route study-apply <candidate_id>` — apply a study candidate.
///
/// Looks up the candidate by index from the last `route study` output and
/// registers it as a reference entry.
pub fn study_apply(candidate_id: String) -> Result<()> {
    use route_basic::{Origin, ReferenceEntry, ReferenceRegistry, ReferenceType};
    use std::str::FromStr;

    let cwd = std::env::current_dir()?;
    let root = find_project_root_or(&cwd)
        .ok_or_else(|| anyhow!("no study report available. Run `route study <path>` first."))?;
    let report = route_basic::load_report(&root)
        .with_context(|| anyhow!("no study report available. Run `route study <path>` first."))?;

    let idx: usize = candidate_id.parse().with_context(|| {
        format!("invalid candidate ID: {candidate_id:?} — expected a numeric index")
    })?;

    let candidate = report.candidates.get(idx).ok_or_else(|| {
        anyhow!(
            "candidate index {idx} out of range (max {})",
            report.candidates.len().saturating_sub(1)
        )
    })?;

    // Only apply reference-type candidates.
    if candidate.kind != "reference" {
        anyhow::bail!(
            "candidate '{}' has kind '{}' — only 'reference' candidates can be applied via this command",
            candidate.name,
            candidate.kind
        );
    }

    let cwd = std::env::current_dir()?;
    let mut reg = ReferenceRegistry::read(&cwd).unwrap_or_default();

    let type_ = if candidate.source.ends_with(".md") || candidate.source.ends_with(".mdx") {
        ReferenceType::Document
    } else if candidate.source.ends_with(".yml") || candidate.source.ends_with(".yaml") {
        ReferenceType::Workflow
    } else {
        ReferenceType::from_str("document").unwrap_or(ReferenceType::Document)
    };

    let now_ts = chrono::Utc::now().timestamp_millis();
    let entry = ReferenceEntry::builder(
        candidate.name.clone(),
        type_,
        &candidate.source,
        &candidate.reason,
    )
    .with_capabilities(&candidate.reason)
    .with_origin(Origin::Imported)
    .with_import_origin(&candidate.source)
    .with_imported_at(now_ts)
    .with_content_hash(&candidate.evidence)
    .with_last_checked(now_ts)
    .build();

    let already_exists = reg.index_of(&candidate.name).is_some();
    let _inserted = reg.upsert(entry);
    reg.write(&cwd)?;

    if already_exists {
        println!("✓ Replaced existing reference '{}'", candidate.name);
    } else {
        println!("✓ Added new reference '{}'", candidate.name);
    }
    println!("  kind:       {}", candidate.kind);
    println!("  source:     {}", candidate.source);
    println!("  evidence:   {}", candidate.evidence);
    println!("  confidence: {:.2}", candidate.confidence);

    Ok(())
}

fn find_project_root() -> Result<std::path::PathBuf> {
    let cwd = std::env::current_dir()?;
    // Check if cwd IS a route project
    if route_core::RoutePaths::new(&cwd).is_initialized() {
        return Ok(cwd);
    }
    // Walk up to find .route-basic/
    let mut current = cwd.as_path();
    loop {
        if current.join(".route-basic").join("config.json").exists() {
            return Ok(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => anyhow::bail!("not inside a Route project (no .route-basic/ found)"),
        }
    }
}

/// `route memory show` — display current project memory.
pub fn memory_show() -> Result<()> {
    let root = find_project_root()?;
    let store = route_basic::MemoryStore::load(&root)?;
    match store.current_memory() {
        Some(memory) => {
            println!("{}", memory.show());
            Ok(())
        }
        None => {
            println!("No project memory available. Run `route memory refresh` to generate one.");
            Ok(())
        }
    }
}

/// `route memory history` — list historical memory snapshots.
pub fn memory_history() -> Result<()> {
    let root = find_project_root()?;
    let hist_dir = route_basic::memory::history_path(&root);
    if !hist_dir.exists() {
        println!("No memory history found.");
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&hist_dir)
        .context("reading memory history directory")?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path().metadata().ok().and_then(|m| m.modified().ok()));

    if entries.is_empty() {
        println!("No memory history snapshots found.");
        return Ok(());
    }

    println!("Memory History ({} snapshots):", entries.len());
    for (i, entry) in entries.iter().enumerate() {
        let name = entry.file_name().to_string_lossy().to_string();
        let modified = entry
            .path()
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                duration.as_secs() as i64
            })
            .unwrap_or(0);
        println!("  {}. {} (modified: {})", i + 1, name, modified);
    }

    Ok(())
}

/// `route memory refresh` — generate memory proposals from existing data.
pub fn memory_refresh() -> Result<()> {
    let root = find_project_root()?;
    let proposed = route_basic::memory::ProjectMemory::refresh(&root, None)?;

    // Load existing memory to produce a diff
    let existing_store = route_basic::MemoryStore::load(&root)?;
    if let Some(current) = existing_store.current_memory() {
        println!("=== Proposed Changes ===");
        println!("{}", current.diff(&proposed));
    } else {
        println!("=== New Memory Proposal ===");
        println!("{}", proposed.show());
    }

    // Save the proposed memory as a new snapshot
    let mut store = existing_store;
    store.memories.push(proposed);
    store.current = Some(store.memories.len().saturating_sub(1).to_string());
    store.save(&root)?;

    // Also archive to history
    let hist_dir = route_basic::memory::history_path(&root);
    std::fs::create_dir_all(&hist_dir).context("creating memory history directory")?;
    let ts = chrono::Utc::now().timestamp_millis();
    let hist_path = hist_dir.join(format!("memory-{}.json", ts));
    if let Some(latest) = store.current_memory() {
        let json = serde_json::to_vec_pretty(latest)?;
        std::fs::write(&hist_path, &json)
            .with_context(|| format!("writing memory history to {}", hist_path.display()))?;
    }

    println!("✓ Memory refreshed and saved.");
    Ok(())
}

/// `route memory apply <index>` — apply a specific memory proposal by index.
pub fn memory_apply(index: usize) -> Result<()> {
    let root = find_project_root()?;
    let store = route_basic::MemoryStore::load(&root)?;

    if index >= store.memories.len() {
        anyhow::bail!(
            "index {} out of range — only {} memories available",
            index,
            store.memories.len()
        );
    }

    // Set the current memory to the one at the given index
    let mut store = store;
    store.current = Some(index.to_string());
    store.save(&root)?;

    if let Some(memory) = store.current_memory() {
        println!("✓ Applied memory proposal #{}", index);
        println!("  Summary: {}", memory.summary);
        println!("  Items: {} total", memory.items.len());
    }

    Ok(())
}

/// `route memory why <topic>` — trace the decision/source graph to explain why.
pub fn memory_why(topic: String) -> Result<()> {
    let root = find_project_root()?;
    let store = route_basic::MemoryStore::load(&root)?;
    let memory = store
        .current_memory()
        .ok_or_else(|| anyhow!("No project memory available. Run `route memory refresh` first."))?;

    let explanation = memory.explain_why(&topic)?;

    println!("Memory Why: {:?}", explanation.topic);
    println!();
    println!("Current State:");
    println!("  {}", explanation.current_state);
    println!();
    println!("Decision Chain:");
    for (i, link) in explanation.decision_chain.iter().enumerate() {
        let sources = if link.source_ids.is_empty() {
            String::new()
        } else {
            format!("Source: {}", link.source_ids.join(", "))
        };
        println!(
            "  {}. [{}] {:?} (confidence: {:.2})",
            i + 1,
            link.kind,
            link.content,
            link.confidence
        );
        if !sources.is_empty() {
            println!("     {}", sources);
        }
    }
    println!();
    if !explanation.alternatives_considered.is_empty() {
        println!("Alternatives Considered:");
        for alt in &explanation.alternatives_considered {
            println!("  - {:?}", alt);
        }
        println!();
    }
    if !explanation.evidence.is_empty() {
        println!("Evidence:");
        for ev in &explanation.evidence {
            println!("  - {}", ev);
        }
    }

    Ok(())
}

/// `route memory supersede <old> <new>` — mark an old item as superseded.
pub fn memory_supersede(old: String, new: String) -> Result<()> {
    let root = find_project_root()?;
    let mut store = route_basic::MemoryStore::load(&root)?;
    let memory = store
        .current_memory_mut()
        .ok_or_else(|| anyhow!("No project memory available. Run `route memory refresh` first."))?;

    memory.supersede_item(&old, &new)?;
    store.save(&root)?;
    println!("✓ Item '{}' marked as superseded by '{}'", old, new);
    Ok(())
}

// ---------------------------------------------------------------------------
// Strategy — route strategy record | list | show | diff | restore | history
// ---------------------------------------------------------------------------

/// Record current strategy as a snapshot.
pub fn strategy_record(label: Option<String>, description: Option<String>) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::StrategyStore::load(&root)?;
    let snap = store.record_snapshot(&root, label, description)?;
    println!("✓ Recorded strategy snapshot");
    println!("  ID:     {}", snap.id);
    println!("  Profile: {}", snap.profile_id);
    println!("  Protocol rev: {}", snap.protocol_revision);
    println!("  Workflows: {}", snap.workflow_ids.len());
    if let Some(label) = &snap.label {
        println!("  Label:  {}", label);
    }
    if let Some(desc) = &snap.description {
        println!("  Desc:   {}", desc);
    }
    Ok(())
}

/// List all strategy snapshots.
pub fn strategy_list() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::StrategyStore::load(&root)?;
    let list = store.list();
    if list.is_empty() {
        println!("(no strategy snapshots recorded)");
        return Ok(());
    }
    println!(
        "{:<30} {:<16} {:<20} {:<12} {:<8} {}",
        "ID", "CREATED", "PROFILE", "PROTOCOL REV", "WKFS", "LABEL"
    );
    println!("{}", "-".repeat(120));
    for s in &list {
        let is_current = store.current.as_ref().map_or(false, |c| c == &s.id);
        let marker = if is_current { "*" } else { " " };
        let id_display = if s.id.len() > 28 {
            format!("{}{}", &s.id[..28], marker)
        } else {
            format!("{:<29}{}", s.id, marker)
        };
        let label = s.label.as_deref().unwrap_or("");
        println!(
            "{} {:>16} {:>20} {:>12} {:>8} {}",
            id_display,
            s.created_at,
            s.profile_id,
            s.protocol_revision,
            s.workflow_ids.len(),
            label
        );
    }
    if store.current.is_some() {
        println!("\n  * = current strategy");
    }
    Ok(())
}

/// Show a strategy snapshot.
pub fn strategy_show(id: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::StrategyStore::load(&root)?;
    let output = route_basic::StrategyStore::show(&id, &store)?;
    print!("{}", output);
    Ok(())
}

/// Diff two strategy snapshots.
pub fn strategy_diff(a: String, b: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::StrategyStore::load(&root)?;
    let output = route_basic::StrategyStore::diff(&a, &b, &store)?;
    print!("{}", output);
    Ok(())
}

/// Restore a strategy snapshot (preserves current first).
pub fn strategy_restore(id: String) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::StrategyStore::load(&root)?;

    // Resolve prefix
    let snapshot = store
        .list()
        .into_iter()
        .find(|s| s.id == id || s.id.starts_with(&id))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Snapshot '{}' not found", id))?;

    store.restore(&root, &snapshot.id)?;
    println!("✓ Restored strategy snapshot '{}'", snapshot.id);
    println!("  Profile: {}", snapshot.profile_id);
    println!("  Protocol rev: {}", snapshot.protocol_revision);
    if let Some(label) = &snapshot.label {
        println!("  Label: {}", label);
    }
    Ok(())
}

/// Show strategy history timeline.
pub fn strategy_history() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::StrategyStore::load(&root)?;
    let list = store.list();
    if list.is_empty() {
        println!("(no strategy history)");
        return Ok(());
    }
    println!("Strategy History Timeline\n");
    for (i, s) in list.iter().enumerate() {
        let is_current = store.current.as_ref().map_or(false, |c| c == &s.id);
        let marker = if is_current { " ← current" } else { "" };
        println!("{}. [{}]", list.len() - i, s.id);
        println!("   Created:      {}", s.created_at);
        println!("   Profile:      {}", s.profile_id);
        println!("   Protocol rev: {}", s.protocol_revision);
        println!("   Workflows:    {}", s.workflow_ids.len());
        if let Some(label) = &s.label {
            println!("   Label:        {}", label);
        }
        if let Some(desc) = &s.description {
            println!("   Description:  {}", desc);
        }
        println!("{}", marker);
        println!();
    }
    Ok(())
}

/// Fork a new strategy from an existing snapshot (or current state).
pub fn strategy_fork(name: String, from: Option<String>) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::StrategyStore::load(&root)?;
    let snap = store.fork(&root, &name, from.as_deref())?;
    println!("✓ Forked new strategy '{}'", name);
    println!("  ID:     {}", snap.id);
    if let Some(parent) = &snap.parent_id {
        println!("  Parent: {}", parent);
    }
    println!("  Profile: {}", snap.profile_id);
    println!("  Protocol rev: {}", snap.protocol_revision);
    println!("  Workflows: {}", snap.workflow_ids.len());
    Ok(())
}

/// Use (activate) a strategy.
pub fn strategy_use(id: String) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::StrategyStore::load(&root)?;

    // Resolve prefix
    let snapshot = store
        .list()
        .into_iter()
        .find(|s| s.id == id || s.id.starts_with(&id))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", id))?;

    store.use_strategy(&root, &snapshot.id)?;
    println!("✓ Activated strategy '{}'", snapshot.id);
    println!("  Profile: {}", snapshot.profile_id);
    println!("  Protocol rev: {}", snapshot.protocol_revision);
    if let Some(label) = &snapshot.label {
        println!("  Label: {}", label);
    }
    Ok(())
}

/// Compare two strategies.
pub fn strategy_compare(a: String, b: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::StrategyStore::load(&root)?;
    let output = store.compare(&a, &b)?;
    print!("{}", output);
    Ok(())
}

/// Delete a non-current strategy.
pub fn strategy_delete(id: String) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::StrategyStore::load(&root)?;

    // Resolve prefix
    let snapshot = store
        .list()
        .into_iter()
        .find(|s| s.id == id || s.id.starts_with(&id))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", id))?;

    let label = snapshot.label.clone();
    store.delete(&root, &snapshot.id)?;
    println!("✓ Deleted strategy '{}'", snapshot.id);
    if let Some(label) = &label {
        println!("  Label: {}", label);
    }
    Ok(())
}

pub fn curator() -> Result<()> {
    let caps = route_basic::curator::curator_capabilities();
    for role in caps.roles {
        println!("## {}", role.name);
        println!("  {}", role.description);
        println!("  Responsibilities:");
        for r in &role.responsibilities {
            println!("    - {}", r);
        }
        println!("  Tools:");
        for t in &role.tools {
            println!("    - {}", t);
        }
        println!("  Constraints:");
        for c in &role.constraints {
            println!("    - {}", c);
        }
        println!();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pattern commands — route pattern list | show | propose | apply | proposals
// ---------------------------------------------------------------------------

/// `route pattern list` — list all patterns.
pub fn pattern_list() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::PatternStore::load(&root)?;
    let patterns = store.list();

    if patterns.is_empty() {
        println!("(no patterns in store)");
        return Ok(());
    }

    println!("Reusable Patterns ({})", patterns.len());
    println!("{:-<80}", "");
    for p in &patterns {
        let confidence_bar = format_confidence(p.confidence);
        println!(
            "  {:<30}  confidence: {:.2}  {}",
            p.name, p.confidence, confidence_bar
        );
        if !p.tags.is_empty() {
            println!("  tags: {}", p.tags.join(", "));
        }
        println!("  ID: {}", p.id);
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route pattern show <id>` — show a specific pattern.
pub fn pattern_show(id: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::PatternStore::load(&root)?;
    let pattern = store
        .get(&id)
        .ok_or_else(|| anyhow!("pattern '{}' not found", id))?;

    println!("## Pattern: {}", pattern.name);
    println!();
    println!("**ID:** {}", pattern.id);
    println!("**Confidence:** {:.2}", pattern.confidence);
    println!();
    println!("### Problem");
    println!("{}", pattern.problem);
    println!();
    println!("### Solution");
    println!("{}", pattern.solution);
    println!();

    if !pattern.when_to_use.is_empty() {
        println!("### When to Use");
        for item in &pattern.when_to_use {
            println!("  - {}", item);
        }
        println!();
    }

    if !pattern.when_not_to_use.is_empty() {
        println!("### When Not to Use");
        for item in &pattern.when_not_to_use {
            println!("  - {}", item);
        }
        println!();
    }

    if !pattern.tradeoffs.is_empty() {
        println!("### Tradeoffs");
        for item in &pattern.tradeoffs {
            println!("  - {}", item);
        }
        println!();
    }

    if !pattern.evidence_sources.is_empty() {
        println!("### Evidence Sources");
        for ev in &pattern.evidence_sources {
            println!("  - {}", ev);
        }
        println!();
    }

    if !pattern.tags.is_empty() {
        println!("**Tags:** {}", pattern.tags.join(", "));
    }

    Ok(())
}

/// `route pattern propose` — create a proposal from a study record.
pub fn pattern_propose(
    from_study: String,
    name: String,
    problem: String,
    solution: String,
) -> Result<()> {
    let root = current_project_root();
    let root = find_project_root_or(&root)
        .ok_or_else(|| anyhow!("not inside a Route project. Run `route init` first."))?;

    // Load the study record from the library
    let study_record = route_basic::study_library_get(&root, &from_study)?
        .ok_or_else(|| anyhow!("study '{}' not found in the study library", from_study))?;

    let proposal =
        route_basic::PatternStore::propose_from_study(&study_record, &name, &problem, &solution);

    // Save the proposal
    let mut store = route_basic::PatternStore::load(&root)?;
    store.proposals.push(proposal);
    store.save(&root)?;

    println!("✓ Pattern proposal created");
    println!("  Proposal ID: {}", store.proposals.last().unwrap().id);
    println!("  Pattern:     {}", name);
    println!(
        "  Confidence:  {:.2}",
        store.proposals.last().unwrap().pattern.confidence
    );
    println!(
        "  Evidence:    {} sources",
        store
            .proposals
            .last()
            .unwrap()
            .pattern
            .evidence_sources
            .len()
    );
    println!("  Status:      pending");
    Ok(())
}

/// `route pattern apply <id>` — apply a pattern proposal.
pub fn pattern_apply(id: String) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::PatternStore::load(&root)?;

    // Resolve prefix
    let proposal = store
        .proposals
        .iter()
        .find(|p| p.id == id || p.id.starts_with(&id))
        .cloned()
        .ok_or_else(|| anyhow!("proposal '{}' not found", id))?;

    store.apply_proposal(&proposal.id)?;
    store.save(&root)?;

    // Also register the pattern as a Reference entry
    // Patterns are NOT a fourth world — they are tracked as Reference entries
    let now_ts = chrono::Utc::now().timestamp_millis();
    let entry = route_basic::ReferenceEntry::builder(
        format!("pattern-{}", proposal.pattern.name),
        route_basic::ReferenceType::Document,
        &format!("pattern:{}", proposal.pattern.id),
        &proposal.reason,
    )
    .with_capabilities(&format!(
        "Reusable pattern: {} — {}",
        proposal.pattern.name, proposal.pattern.solution
    ))
    .with_origin(route_basic::Origin::Imported)
    .with_import_origin("pattern-distillation")
    .with_imported_at(now_ts)
    .with_content_hash(&proposal.pattern.problem)
    .with_last_checked(now_ts)
    .build();

    let mut reg = route_basic::ReferenceRegistry::read(&root).unwrap_or_default();
    reg.upsert(entry);
    reg.write(&root)?;

    println!("✓ Applied pattern proposal '{}'", proposal.id);
    println!("  Pattern: {}", proposal.pattern.name);
    println!("  Confidence: {:.2}", proposal.pattern.confidence);
    println!("  Also registered as a Reference entry.");
    Ok(())
}

/// `route pattern proposals` — list pending proposals.
pub fn pattern_proposals() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::PatternStore::load(&root)?;

    let pending: Vec<_> = store
        .proposals
        .iter()
        .filter(|p| p.status == "pending")
        .collect();

    if pending.is_empty() {
        println!("(no pending proposals)");
        return Ok(());
    }

    println!("Pending Pattern Proposals ({})", pending.len());
    println!("{:-<80}", "");
    for p in &pending {
        let ts = chrono::DateTime::from_timestamp(p.created_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| p.created_at.to_string());
        println!("  Proposal ID: {}", p.id);
        println!("  Pattern:     {}", p.pattern.name);
        println!("  Confidence:  {:.2}", p.pattern.confidence);
        println!("  Created:     {}", ts);
        println!("  Reason:      {}", p.reason);
        println!(
            "  Evidence:    {} sources",
            p.pattern.evidence_sources.len()
        );
        println!("{:-<80}", "");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Experiment commands — route experiment stats | suggest | history | show
// ---------------------------------------------------------------------------

/// `route experiment stats` — show strategy statistics.
pub fn experiment_stats() -> Result<()> {
    use route_basic::ExperimentStore;

    let root = current_project_root();
    let store = ExperimentStore::load(&root)?;
    let all = store.all_stats();

    if all.is_empty() {
        println!("(no experiment data recorded)");
        return Ok(());
    }

    println!("Strategy Experiment Statistics");
    println!("{:-<100}", "");
    for s in &all {
        let total = s.total_tasks;
        let success_rate = if total > 0 {
            s.success_count as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        println!(
            "  {:<30}  total: {:<4}  success: {:<4} ({:.0}%)  failed: {:<4}  aborted: {:<4}  rolled_back: {:<4}",
            s.strategy_id,
            total,
            s.success_count,
            success_rate,
            s.failed_count,
            s.aborted_count,
            s.rolled_back_count,
        );
        println!(
            "  {:>30}  avg checks: {:.1} passed / {:.1} failed",
            "", s.avg_checks_passed, s.avg_checks_failed,
        );
        if let Some(avg_dur) = s.avg_duration_secs {
            println!("  {:>30}  avg duration: {:.0}s", "", avg_dur);
        }
        if !s.common_agent_roles.is_empty() {
            println!(
                "  {:>30}  common roles: {}",
                "",
                s.common_agent_roles.join(", ")
            );
        }
        if let Some(last) = s.last_used {
            let ts = chrono::DateTime::from_timestamp_millis(last)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| last.to_string());
            println!("  {:>30}  last used: {}", "", ts);
        }
        println!("{:-<100}", "");
    }
    Ok(())
}

/// `route experiment suggest <task> --strategies <ids>` — rank strategies.
pub fn experiment_suggest(task: String, strategies: String) -> Result<()> {
    use route_basic::ExperimentStore;

    let root = current_project_root();
    let store = ExperimentStore::load(&root)?;
    let strategy_ids: Vec<String> = strategies
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let rankings = store.suggest_strategy(&task, &strategy_ids);

    if rankings.is_empty() {
        println!("(no strategies to rank)");
        return Ok(());
    }

    println!("Strategy Rankings for: \"{}\"", task);
    println!("{:-<80}", "");
    for (i, r) in rankings.iter().enumerate() {
        println!("{}. {}  (score: {:.3})", i + 1, r.strategy_id, r.score);
        println!("   Reason: {}", r.reason);
        if !r.evidence.is_empty() {
            println!("   Evidence:");
            for e in &r.evidence {
                println!("     - {}", e);
            }
        }
        println!("{:-<80}", "");
    }
    println!();
    println!("Note: Strategy suggestion is advisory only. Protocol always has higher priority.");
    Ok(())
}

/// `route experiment history` — show all experiment records.
pub fn experiment_history() -> Result<()> {
    use route_basic::ExperimentStore;

    let root = current_project_root();
    let store = ExperimentStore::load(&root)?;

    if store.records.is_empty() {
        println!("(no experiment records)");
        return Ok(());
    }

    // Sort by created_at descending.
    let mut records = store.records.clone();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    println!("Experiment History ({} total)", records.len());
    println!("{:-<120}", "");
    for r in &records {
        let ts = chrono::DateTime::from_timestamp_millis(r.created_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| r.created_at.to_string());
        println!("  ID:       {}", &r.id[..12]);
        println!("  Task:     {}", r.task);
        println!("  Strategy: {}", r.strategy_id);
        println!("  Result:   {}", r.result);
        println!(
            "  Checks:   {} passed / {} failed",
            r.checks_passed, r.checks_failed
        );
        if let Some(dur) = r.duration_secs {
            println!("  Duration: {}s", dur);
        }
        println!("  Created:  {}", ts);
        println!("{:-<120}", "");
    }
    Ok(())
}

/// `route experiment show <strategy>` — show experiments for a specific strategy.
pub fn experiment_show(strategy: String) -> Result<()> {
    use route_basic::ExperimentStore;

    let root = current_project_root();
    let store = ExperimentStore::load(&root)?;

    let records: Vec<&route_basic::ExperimentRecord> = store
        .records
        .iter()
        .filter(|r| r.strategy_id == strategy || r.strategy_id.starts_with(&strategy))
        .collect();

    if records.is_empty() {
        println!("(no experiments for strategy '{}')", strategy);
        return Ok(());
    }

    // Show stats first.
    if let Some(stats) = store.strategy_stats(&records[0].strategy_id) {
        let total = stats.total_tasks;
        let success_rate = if total > 0 {
            stats.success_count as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        println!("Strategy: {} ({} experiments)", stats.strategy_id, total);
        println!(
            "  Success rate: {:.0}% ({}/{})",
            success_rate, stats.success_count, total
        );
        println!(
            "  Avg checks: {:.1} passed / {:.1} failed",
            stats.avg_checks_passed, stats.avg_checks_failed
        );
        if let Some(avg_dur) = stats.avg_duration_secs {
            println!("  Avg duration: {:.0}s", avg_dur);
        }
        println!();
    }

    // Sort by created_at descending.
    let mut sorted: Vec<&&route_basic::ExperimentRecord> = records.iter().collect();
    sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    println!("Experiments:");
    println!("{:-<120}", "");
    for r in sorted {
        let ts = chrono::DateTime::from_timestamp_millis(r.created_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| r.created_at.to_string());
        println!(
            "  Session: {}  Result: {}  Checks: {}/{}  Created: {}",
            &r.session_id[..12],
            r.result,
            r.checks_passed,
            r.checks_failed,
            ts,
        );
        println!("  Task: {}", r.task);
        println!("{:-<120}", "");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent organization memory commands
// ---------------------------------------------------------------------------

/// Show organization experience history.
pub fn agent_org_history() -> Result<()> {
    use route_basic::OrganizationExperienceStore;

    let root = current_project_root();
    let store = OrganizationExperienceStore::load(&root)?;
    let experiences = store.list();

    if experiences.is_empty() {
        println!("(no organization experiences recorded)");
        return Ok(());
    }

    println!("Organization Experiences ({} total):", experiences.len());
    println!("{:-<140}", "");
    for exp in experiences {
        let ts = chrono::DateTime::from_timestamp_millis(exp.created_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| exp.created_at.to_string());
        println!(
            "  Session: {}  Pattern: {}  Result: {}  Verdict: {}  Roles: [{}]  {}",
            &exp.session_id[..12.min(exp.session_id.len())],
            exp.task_pattern,
            exp.task_result,
            exp.verification_result,
            exp.agent_roles.join(", "),
            ts,
        );
        println!("{:-<140}", "");
    }
    Ok(())
}

/// Explain why a specific session had its agent plan.
pub fn agent_org_explain(id: String) -> Result<()> {
    use route_basic::OrganizationExperienceStore;

    let root = current_project_root();
    let store = OrganizationExperienceStore::load(&root)?;

    // Try exact match first, then prefix match
    let result = store.explain(&id);
    match result {
        Ok(explanation) => {
            println!("{}", explanation);
            Ok(())
        }
        Err(_) => {
            // Try prefix match
            let matches: Vec<&route_basic::OrganizationExperience> = store
                .experiences
                .iter()
                .filter(|e| e.session_id.starts_with(&id))
                .collect();
            if matches.is_empty() {
                anyhow::bail!("no organization experience found for session '{}'", id);
            }
            if matches.len() == 1 {
                let explanation = store.explain(&matches[0].session_id)?;
                println!("{}", explanation);
                Ok(())
            } else {
                println!("Multiple sessions match prefix '{}':", id);
                for m in &matches {
                    println!("  {}", m.session_id);
                }
                Ok(())
            }
        }
    }
}

/// Show recall signals for a task pattern.
pub fn agent_org_recall(pattern: String) -> Result<()> {
    use route_basic::{classify_task_pattern, OrganizationExperienceStore};

    let root = current_project_root();
    let store = OrganizationExperienceStore::load(&root)?;
    let signals = store.recall(&pattern);

    if signals.is_empty() {
        println!("(no recall signals for pattern '{}')", pattern);
        // Show classification hint
        let classified = classify_task_pattern(&pattern);
        if classified != pattern {
            println!(
                "Hint: task '{}' would be classified as '{}'",
                pattern, classified
            );
        }
        return Ok(());
    }

    println!(
        "Recall Signals for pattern '{}' ({} signals):",
        pattern,
        signals.len()
    );
    println!("{:-<100}", "");
    for signal in &signals {
        println!(
            "  Role: {}  Reliability: {:.2}  Evidence: {} sessions",
            signal.role,
            signal.reliability,
            signal.evidence.len(),
        );
        println!("  Note: {}", signal.note);
        println!("{:-<100}", "");
    }
    Ok(())
}

/// Format a confidence value as a simple ASCII bar.
fn format_confidence(confidence: f64) -> String {
    let filled = (confidence * 10.0).round() as usize;
    let filled = filled.min(10);
    let empty = 10 - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

// ---------------------------------------------------------------------------
// Savepoint commands — route save create | list | show | preview | restore | diff | delete
// ---------------------------------------------------------------------------

/// `route save create <name>` — create a savepoint capturing current state.
pub fn savepoint_create(name: String, _description: Option<String>) -> Result<()> {
    use route_basic::{
        constitutive::Protocol, create_memory_snapshot, create_savepoint,
        execution::compute_state_hash, goal::GoalStore, strategy::StrategyStore,
    };

    let root = current_project_root();

    // Collect current state references
    // Use history to get latest snapshot ID
    let code_snapshot_id = match route_basic::BasicRepository::open(&root) {
        Ok(repo) => {
            if let Ok(history) = repo.history(1) {
                if let Some(latest) = history.first() {
                    latest.snapshot.id.clone()
                } else {
                    "no-snapshot".to_string()
                }
            } else {
                "no-history".to_string()
            }
        }
        Err(_) => "no-repo".to_string(),
    };

    let context_hash = match compute_state_hash(&root) {
        Ok(h) => h,
        Err(_) => "unknown".to_string(),
    };

    // Memory snapshot
    let memory_snapshot_id = create_memory_snapshot(&root, "").ok();

    // Strategy
    let strategy_store = StrategyStore::load(&root).ok();
    let strategy_id = strategy_store.and_then(|s| s.current_strategy().map(|s| s.to_string()));

    // Protocol revision
    let protocol = Protocol::read(&root).ok();
    let protocol_revision = protocol
        .map(|p| p.revision.to_string())
        .unwrap_or_else(|| "0".to_string());

    // Workflow revisions
    let workflow_revisions: Vec<String> = Vec::new();

    // Active references
    let active_reference_ids: Vec<String> = Vec::new();

    // Agent policy
    let agent_policy_hash: Option<String> = None;

    // Active goals
    let goal_store = GoalStore::load(&root).ok();
    let active_goal_ids = if let Some(store) = goal_store {
        store
            .list(Some("active"))
            .iter()
            .map(|g| g.id.clone())
            .collect()
    } else {
        Vec::new()
    };

    // Current task/session
    let current_task_id: Option<String> = None;
    let current_session_id: Option<String> = None;

    let sp = create_savepoint(
        &root,
        name,
        code_snapshot_id,
        context_hash,
        memory_snapshot_id,
        strategy_id,
        protocol_revision,
        workflow_revisions,
        active_reference_ids,
        agent_policy_hash,
        active_goal_ids,
        current_task_id,
        current_session_id,
    )?;

    println!("Created savepoint");
    println!("  ID:     {}", sp.id);
    println!("  Name:   {}", sp.name);
    println!(
        "  Code:   {}",
        &sp.code_snapshot_id[..16.min(sp.code_snapshot_id.len())]
    );
    println!(
        "  Ctx:    {}",
        &sp.context_hash[..16.min(sp.context_hash.len())]
    );
    Ok(())
}

/// `route save list` — list all savepoints.
pub fn savepoint_list() -> Result<()> {
    use route_basic::SavepointStore;

    let root = current_project_root();
    let store = SavepointStore::load(&root)?;
    let list = store.list();
    if list.is_empty() {
        println!("(no savepoints — create one with `route save create <name>`)");
        return Ok(());
    }
    println!(
        "{:<30} {:<20} {:<14} {:<14} {:<8}",
        "ID", "NAME", "CREATED", "CODE", "WKFS"
    );
    println!("{}", "-".repeat(90));
    for s in &list {
        let ts = chrono::DateTime::from_timestamp_millis(s.created_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| s.created_at.to_string());
        let code = &s.code_snapshot_id[..16.min(s.code_snapshot_id.len())];
        let wkf_count = s.workflow_revisions.len().to_string();
        let id_display = if s.id.len() > 28 {
            format!("{}", &s.id[..28])
        } else {
            format!("{:<28}", s.id)
        };
        println!(
            "{} {:>20} {:>14} {:>14} {:>8}",
            id_display, s.name, ts, code, wkf_count
        );
    }
    Ok(())
}

/// `route save show <id>` — show savepoint details.
pub fn savepoint_show(id: String) -> Result<()> {
    use route_basic::{format_savepoint, SavepointStore};

    let root = current_project_root();
    let store = SavepointStore::load(&root)?;
    let sp = store
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Savepoint '{}' not found", id))?;

    println!("{}", format_savepoint(sp, true));
    Ok(())
}

/// `route save preview <id> --scope <scope>` — show what restore would do.
pub fn savepoint_preview(id: String, scope: String) -> Result<()> {
    use route_basic::preview_restore;

    let root = current_project_root();
    let preview = preview_restore(&root, &id, &scope)?;

    println!("Restore Preview");
    println!(
        "  Savepoint: {} ({})",
        &preview.savepoint_id[..16.min(preview.savepoint_id.len())],
        &preview.savepoint_id
    );
    println!("  Scope:     {:?}", preview.scope);
    println!();
    println!("Actions:");
    for action in &preview.changes {
        println!("  - {}", action);
    }
    if preview.constitution_requires_approval {
        println!();
        println!("Warning: Constitution requires explicit approval.");
    }
    println!();
    println!(
        "Run `route save restore {} --scope {:?} --force` to execute.",
        &id, &preview.scope
    );
    Ok(())
}

/// `route save restore <id> --scope <scope> --force` — restore a savepoint.
pub fn savepoint_restore(id: String, scope: String, force: bool) -> Result<()> {
    use route_basic::execute_restore;

    let root = current_project_root();
    execute_restore(&root, &id, &scope, force)?;
    println!(
        "Restored savepoint '{}' with scope '{}'",
        &id[..16.min(id.len())],
        scope
    );
    Ok(())
}

/// `route save diff <a> <b>` — diff two savepoints.
pub fn savepoint_diff(a: String, b: String) -> Result<()> {
    use route_basic::{diff_savepoints, format_diff, SavepointStore};

    let root = current_project_root();
    let store = SavepointStore::load(&root)?;
    let spa = store
        .get(&a)
        .ok_or_else(|| anyhow::anyhow!("Savepoint '{}' not found", a))?;
    let spb = store
        .get(&b)
        .ok_or_else(|| anyhow::anyhow!("Savepoint '{}' not found", b))?;
    let diff = diff_savepoints(spa, spb);
    print!("{}", format_diff(&diff));
    Ok(())
}

/// `route save delete <id>` — delete a savepoint.
pub fn savepoint_delete(id: String) -> Result<()> {
    use route_basic::delete_savepoint;

    let root = current_project_root();
    delete_savepoint(&root, &id)?;
    println!("Deleted savepoint '{}'", &id[..16.min(id.len())]);
    Ok(())
}

// ---------------------------------------------------------------------------
// Trajectory commands — route trajectory list | show | diff | analyze
// ---------------------------------------------------------------------------

/// `route trajectory list` — list all development trajectories.
pub fn trajectory_list() -> Result<()> {
    use route_basic::TrajectoryStore;

    let root = current_project_root();
    let store = TrajectoryStore::load(&root)?;
    let list = store.list();
    if list.is_empty() {
        println!("(no trajectories — start a task with `route task start` to create one)");
        return Ok(());
    }
    println!(
        "{:<30} {:<30} {:<14} {:<10} {:<14}",
        "ID", "INTENT", "CREATED", "OUTCOME", "STRATEGY"
    );
    println!("{}", "-".repeat(100));
    for t in &list {
        let ts = chrono::DateTime::from_timestamp_millis(t.created_at)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| t.created_at.to_string());
        let intent = t.intent.as_deref().unwrap_or("(none)");
        let strategy = t.strategy_id.as_deref().unwrap_or("(none)");
        let id_display = if t.id.len() > 28 {
            format!("{}", &t.id[..28])
        } else {
            format!("{:<28}", t.id)
        };
        println!(
            "{} {:>28} {:>14} {:>10} {:>14}",
            id_display, intent, ts, t.outcome, strategy
        );
    }
    Ok(())
}

/// `route trajectory show <id>` — show trajectory details.
pub fn trajectory_show(id: String) -> Result<()> {
    use route_basic::{format_trajectory, TrajectoryStore};

    let root = current_project_root();
    let store = TrajectoryStore::load(&root)?;
    let t = store
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Trajectory '{}' not found", id))?;
    println!("{}", format_trajectory(t, true));
    Ok(())
}

/// `route trajectory diff <a> <b>` — compare two trajectories.
pub fn trajectory_diff(a: String, b: String) -> Result<()> {
    use route_basic::{diff_trajectories, format_trajectory_diff, TrajectoryStore};

    let root = current_project_root();
    let store = TrajectoryStore::load(&root)?;
    let ta = store
        .get(&a)
        .ok_or_else(|| anyhow::anyhow!("Trajectory '{}' not found", a))?;
    let tb = store
        .get(&b)
        .ok_or_else(|| anyhow::anyhow!("Trajectory '{}' not found", b))?;
    let diff = diff_trajectories(ta, tb);
    print!("{}", format_trajectory_diff(&diff));
    Ok(())
}

/// `route trajectory analyze <id>` — analyze a trajectory for reversal.
pub fn trajectory_analyze(id: String) -> Result<()> {
    use route_basic::{analyze_reversal, TrajectoryStore};

    let root = current_project_root();
    let store = TrajectoryStore::load(&root)?;
    let t = store
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Trajectory '{}' not found", id))?;

    if let Some(analysis) = analyze_reversal(t) {
        println!("Reversal Analysis (P3):");
        println!("  Trajectory: {}", analysis.trajectory_id);
        println!("  Rolled back to: {}", analysis.rolled_back_to);
        println!("  Observation: {}", analysis.observation);
        if let Some(reason) = &analysis.explicit_reason {
            println!("  Explicit reason: {}", reason);
        }
        println!("  Status: observation candidate (not a rule)");
    } else {
        println!("No rollback found in this trajectory — no reversal analysis available.");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Learn commands — route learn status | why | reject | supersede | downgrade | analyze
// ---------------------------------------------------------------------------

/// `route learn status` — show learning dashboard.
pub fn learn_status() -> Result<()> {
    use route_basic::LearningActionStore;
    use route_basic::{format_dashboard, generate_learning_dashboard, TrajectoryStore};

    let root = current_project_root();
    let traj_store = TrajectoryStore::load(&root)?;
    let proposals = vec![]; // strategy proposals loaded from elsewhere
    let wf_proposals = vec![]; // workflow proposals
    let actions = LearningActionStore::load(&root)?;
    let dashboard =
        generate_learning_dashboard(&traj_store, &proposals, &wf_proposals, &actions.actions);
    print!("{}", format_dashboard(&dashboard));
    Ok(())
}

/// `route learn why <id> --kind <kind>` — explain a learning result.
pub fn learn_why(id: String, kind: String, json: bool) -> Result<()> {
    use route_basic::{explain_learning, TrajectoryStore};

    let root = current_project_root();
    let store = TrajectoryStore::load(&root)?;
    let explanation = explain_learning(&store, &id, &kind);

    if json {
        let chain: Vec<serde_json::Value> = explanation
            .chain
            .iter()
            .map(|link| {
                serde_json::json!({
                    "kind": link.kind,
                    "id": link.id,
                    "description": link.description,
                })
            })
            .collect();
        let output = serde_json::json!({
            "target_id": id,
            "target_type": explanation.target_type,
            "title": explanation.title,
            "evidence_chain": chain,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Learning Explanation (P11):");
    println!(
        "  Target: {} [{}]",
        explanation.title, explanation.target_type
    );
    println!();
    println!("Evidence chain:");
    for (i, link) in explanation.chain.iter().enumerate() {
        println!(
            "  {}. [{}] {} — {}",
            i + 1,
            link.kind,
            link.id,
            link.description
        );
    }
    Ok(())
}

/// `route learn reject <id> [reason]` — reject a learning item (trajectory version).
pub fn trajectory_learn_reject(id: String, reason: Option<String>) -> Result<()> {
    use route_basic::LearningAction;
    use route_basic::LearningActionStore;

    let root = current_project_root();
    let mut store = LearningActionStore::load(&root)?;
    store.add(LearningAction {
        id: format!("la_{}", ulid::Ulid::new().to_string()),
        action_type: "reject".to_string(),
        target_id: id.clone(),
        target_kind: "proposal".to_string(),
        reason,
        new_confidence: None,
        superseded_by: None,
        created_at: route_core::now_millis(),
    });
    store.save(&root)?;
    println!(
        "Rejected '{}' — original trajectory data preserved.",
        &id[..16.min(id.len())]
    );
    Ok(())
}

/// `route learn supersede <target> --by <id>` — supersede old knowledge.
pub fn learn_supersede(target: String, by: String) -> Result<()> {
    use route_basic::LearningAction;
    use route_basic::LearningActionStore;

    let root = current_project_root();
    let mut store = LearningActionStore::load(&root)?;
    store.add(LearningAction {
        id: format!("la_{}", ulid::Ulid::new().to_string()),
        action_type: "supersede".to_string(),
        target_id: target.clone(),
        target_kind: "knowledge".to_string(),
        reason: Some(format!("Superseded by {}", by)),
        new_confidence: None,
        superseded_by: Some(by.clone()),
        created_at: route_core::now_millis(),
    });
    store.save(&root)?;
    println!(
        "Superseded '{}' by '{}'",
        &target[..16.min(target.len())],
        &by[..16.min(by.len())]
    );
    Ok(())
}

/// `route learn downgrade <id> --confidence <value>` — downgrade confidence.
pub fn learn_downgrade(id: String, confidence: f64) -> Result<()> {
    use route_basic::LearningAction;
    use route_basic::LearningActionStore;

    let root = current_project_root();
    let mut store = LearningActionStore::load(&root)?;
    store.add(LearningAction {
        id: format!("la_{}", ulid::Ulid::new().to_string()),
        action_type: "downgrade".to_string(),
        target_id: id.clone(),
        target_kind: "proposal".to_string(),
        reason: Some(format!("Confidence downgraded to {:.2}", confidence)),
        new_confidence: Some(confidence),
        superseded_by: None,
        created_at: route_core::now_millis(),
    });
    store.save(&root)?;
    println!(
        "Downgraded '{}' to confidence {:.2}",
        &id[..16.min(id.len())],
        confidence
    );
    Ok(())
}

/// `route learn analyze` — analyze trajectories and generate learning proposals (trajectory version).
pub fn trajectory_learn_analyze() -> Result<()> {
    use route_basic::TrajectoryWorkflowProposalStore;
    use route_basic::{generate_strategy_learning_proposals, TrajectoryStore};

    let root = current_project_root();
    let traj_store = TrajectoryStore::load(&root)?;

    // Generate strategy learning proposals
    let proposals = generate_strategy_learning_proposals(&traj_store);
    if proposals.is_empty() {
        println!("No strategy learning proposals generated (need at least 2 trajectories with same pattern).");
    } else {
        println!("Strategy Learning Proposals (P5):");
        for p in &proposals {
            println!("  [{}] {}", &p.id[..16.min(p.id.len())], p.claim);
            println!(
                "        Trajectories: {}, Rollback: {:.0}%, Success: {:.0}%",
                p.trajectory_count,
                p.rollback_rate * 100.0,
                p.success_rate * 100.0
            );
            println!("        Confidence: {:.2}", p.confidence);
            println!();
        }
    }

    // Generate workflow evolution proposals
    // For each workflow referenced in trajectories
    let mut wf_ids: Vec<String> = Vec::new();
    for t in &traj_store.trajectories {
        for wf in &t.workflow_revisions {
            if !wf_ids.contains(wf) {
                wf_ids.push(wf.clone());
            }
        }
    }
    let mut wf_proposal_store = TrajectoryWorkflowProposalStore::load(&root)?;
    for wf_id in &wf_ids {
        let wf_proposals = route_basic::generate_workflow_evolution_proposals(&traj_store, wf_id);
        for p in wf_proposals {
            println!(
                "  Workflow Proposal: [{}] {}",
                &p.id[..16.min(p.id.len())],
                p.description
            );
            println!(
                "        Change: {}, Confidence: {:.2}",
                p.change_type, p.confidence
            );
            wf_proposal_store.add(p);
        }
    }
    wf_proposal_store.save(&root)?;

    // Generate agent plan insights
    let insights = route_basic::analyze_agent_plans(&traj_store);
    if !insights.is_empty() {
        println!("Agent Plan Insights (P7):");
        for i in &insights {
            println!(
                "  [{}] {} — effective: {}",
                &i.id[..16.min(i.id.len())],
                i.observation,
                i.effective
            );
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Workflow learn command — route workflow learn <id>
// ---------------------------------------------------------------------------

/// `route workflow learn <id>` — generate workflow evolution proposals from trajectory analysis.
pub fn workflow_learn_from_trajectories(id: String) -> Result<()> {
    use route_basic::TrajectoryWorkflowProposalStore;
    use route_basic::{generate_workflow_evolution_proposals, TrajectoryStore};

    let root = current_project_root();
    let traj_store = TrajectoryStore::load(&root)?;
    let proposals = generate_workflow_evolution_proposals(&traj_store, &id);

    if proposals.is_empty() {
        println!("No workflow evolution proposals generated for '{}' (need at least 2 related trajectories).", id);
        return Ok(());
    }

    let mut store = TrajectoryWorkflowProposalStore::load(&root)?;
    for p in &proposals {
        println!("Workflow Evolution Proposal (P6):");
        println!("  ID:           {}", &p.id[..16.min(p.id.len())]);
        println!("  Workflow:     {}", p.workflow_id);
        println!("  Change:       {}", p.change_type);
        println!("  Description:  {}", p.description);
        println!("  Confidence:   {:.2}", p.confidence);
        println!("  Effect:       {}", p.expected_effect);
        println!(
            "  Supporting trajectories: {}",
            p.supporting_trajectory_ids.len()
        );
        println!();
        store.add(p.clone());
    }
    store.save(&root)?;
    println!("{} proposal(s) saved to store.", proposals.len());
    println!("Use `route workflow apply-evolve <id>` to apply a proposal.");
    Ok(())
}

// ---------------------------------------------------------------------------
// Capability commands — route capability list | show | inspect
// ---------------------------------------------------------------------------

/// `route capability list [--kind <kind>]` — list capabilities.
pub fn capability_list(kind: Option<String>) -> Result<()> {
    use route_basic::capability::CapabilityRegistry;

    let cwd = current_project_root();
    let registry = CapabilityRegistry::load(&cwd)?;

    let caps: Vec<_> = match kind {
        Some(ref k) => registry
            .capabilities
            .iter()
            .filter(|c| format!("{:?}", c.kind).to_lowercase() == k.to_lowercase())
            .collect(),
        None => registry.capabilities.iter().collect(),
    };

    if caps.is_empty() {
        println!("(no capabilities found)");
        return Ok(());
    }

    println!("Capabilities ({})", caps.len());
    println!("{:-<120}", "");
    for cap in &caps {
        let avail = if cap.availability { "✓" } else { "✗" };
        println!(
            "  {}  {}  {:?}  {:?}  {}  {}",
            avail,
            &cap.id[..28.min(cap.id.len())],
            cap.kind,
            cap.level,
            cap.name,
            cap.entrypoint.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

/// `route capability show <id>` — show capability details.
pub fn capability_show(id: String) -> Result<()> {
    use route_basic::capability::CapabilityRegistry;

    let cwd = current_project_root();
    let registry = CapabilityRegistry::load(&cwd)?;

    let cap = registry
        .get(&id)
        .ok_or_else(|| anyhow!("Capability '{}' not found", id))?;

    println!("Capability: {}", cap.id);
    println!("  Name:       {}", cap.name);
    println!("  Reference:  {}", cap.reference_id);
    println!("  Kind:       {:?}", cap.kind);
    println!("  Level:      {:?}", cap.level);
    println!(
        "  Entrypoint: {}",
        cap.entrypoint.as_deref().unwrap_or("(none)")
    );
    println!("  Usage:      {}", cap.usage);
    println!("  Inputs:     {}", cap.inputs.join(", "));
    println!("  Outputs:    {}", cap.outputs.join(", "));
    println!("  Permissions: {}", cap.permissions.join(", "));
    println!("  Constraints: {}", cap.constraints.join(", "));
    println!(
        "  Available:  {}",
        if cap.availability { "yes" } else { "no" }
    );
    Ok(())
}

/// `route capability inspect <id>` — inspect capability with detailed analysis.
pub fn capability_inspect(id: String) -> Result<()> {
    use route_basic::capability::CapabilityRegistry;

    let cwd = current_project_root();
    let registry = CapabilityRegistry::load(&cwd)?;
    let detail = registry.inspect(&id)?;
    println!("{}", detail);
    Ok(())
}

/// `route capability promote-skill [id] [--force]` — promote Skill-kind
/// capabilities into reusable `.route/skills/*.md` files (idempotent).
pub fn capability_promote_skill(id: Option<String>, force: bool) -> Result<()> {
    use route_basic::capability::CapabilityRegistry;

    let cwd = current_project_root();
    let registry = CapabilityRegistry::load(&cwd)?;
    let report = registry.promote_skills(&cwd, id.as_deref(), force)?;
    if report.promoted.is_empty() && report.skipped_existing.is_empty() {
        println!("(no Skill capabilities to promote)");
        return Ok(());
    }
    for f in &report.promoted {
        println!("✓ promoted  {}.md", f);
    }
    for f in &report.skipped_existing {
        println!("· skipped   {}.md (already exists — use --force)", f);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Discover commands — route discover [path]
// ---------------------------------------------------------------------------

/// `route discover [path] [--promote-skill]` — scan a project for discoverable
/// capabilities. With `--promote-skill`, also register discovered skills into
/// the capability registry and promote them to `.route/skills/`.
pub fn discover(path: Option<String>, promote_skill: bool) -> Result<()> {
    use route_basic::capability::CapabilityRegistry;
    use route_basic::discovery::{format_discovery_proposal, scan_project, DiscoveryStore};

    let cwd = match path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir()?,
    };

    let proposal = scan_project(&cwd)?;

    // Save the proposal to the discovery store
    let root = current_project_root();
    let mut store = DiscoveryStore::load(&root)?;
    store.add(proposal.clone());
    store.save(&root)?;

    println!("{}", format_discovery_proposal(&proposal));
    println!("✓ Proposal saved to discovery store");

    if promote_skill {
        let mut registry = CapabilityRegistry::load(&root)?;
        let report = registry.integrate_discovery(&root, &proposal)?;
        println!(
            "✓ {n} capabilities registered, {p} as reusable skills",
            n = report.registered.len(),
            p = report.promoted.len()
        );
        for name in &report.registered {
            println!("  + registered  {name}");
        }
        for f in &report.promoted {
            println!("  + skill       {f}.md");
        }
        for f in &report.skipped_existing {
            println!("  · skill       {f}.md (already exists)");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Pack commands — route pack create | list | show | use | deactivate | export | import | delete
// ---------------------------------------------------------------------------

/// `route pack create <name> [--description <desc>] [--workflows <ids>] ...` — create a pack.
pub fn pack_create(
    name: String,
    description: Option<String>,
    workflows: Vec<String>,
    skills: Vec<String>,
    tools: Vec<String>,
    refs: Vec<String>,
    capabilities: Vec<String>,
    strategy: Option<String>,
    tags: Vec<String>,
) -> Result<()> {
    use route_basic::pack::PackIndex;

    let root = current_project_root();
    let mut index = PackIndex::load(&root)?;

    let desc = description.unwrap_or_default();
    let pack = index.create_pack(
        &name,
        &desc,
        workflows,
        skills,
        tools,
        refs,
        capabilities,
        strategy,
        tags,
        None,
    )?;
    index.save(&root)?;

    println!("✓ Pack created: {}", pack.id);
    println!("  Name: {}", pack.name);
    println!("  Version: {}", pack.version);
    Ok(())
}

/// `route pack list` — list all packs.
pub fn pack_list() -> Result<()> {
    use route_basic::pack::{format_pack, PackIndex};

    let root = current_project_root();
    let index = PackIndex::load(&root)?;

    let packs = index.list();
    if packs.is_empty() {
        println!("(no packs found)");
        return Ok(());
    }

    for pack in packs {
        println!("{}", format_pack(pack));
        let active = if index.active_pack_ids.contains(&pack.id) {
            " (active)"
        } else {
            ""
        };
        println!("  Active: {}", if active.is_empty() { "no" } else { "yes" });
        println!("{:-<60}", "");
    }
    Ok(())
}

/// `route pack show <id>` — show a specific pack.
pub fn pack_show(id: String) -> Result<()> {
    use route_basic::pack::{format_pack, PackIndex};

    let root = current_project_root();
    let index = PackIndex::load(&root)?;

    let pack = index
        .get(&id)
        .ok_or_else(|| anyhow!("Pack '{}' not found", id))?;

    println!("{}", format_pack(pack));
    let active = if index.active_pack_ids.contains(&pack.id) {
        "yes"
    } else {
        "no"
    };
    println!("  Active: {}", active);
    Ok(())
}

/// `route pack use <id>` — activate a pack.
pub fn pack_use(id: String) -> Result<()> {
    use route_basic::pack::PackIndex;

    let root = current_project_root();
    let mut index = PackIndex::load(&root)?;

    index.activate(&id)?;
    index.save(&root)?;

    println!("✓ Pack '{}' activated", id);
    Ok(())
}

/// `route pack deactivate <id>` — deactivate a pack.
pub fn pack_deactivate(id: String) -> Result<()> {
    use route_basic::pack::PackIndex;

    let root = current_project_root();
    let mut index = PackIndex::load(&root)?;

    index.deactivate(&id);
    index.save(&root)?;

    println!("✓ Pack '{}' deactivated", id);
    Ok(())
}

/// `route pack export <id>` — export a pack as JSON to stdout.
pub fn pack_export(id: String) -> Result<()> {
    use route_basic::pack::PackIndex;

    let root = current_project_root();
    let index = PackIndex::load(&root)?;

    let json = index.export_pack(&id)?;
    println!("{}", json);
    Ok(())
}

/// `route pack import <path>` — import a pack from a JSON file.
pub fn pack_import(path: String) -> Result<()> {
    use route_basic::pack::PackIndex;

    let root = current_project_root();
    let mut index = PackIndex::load(&root)?;

    let json = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read pack file: {}", path))?;
    let id = index.import_pack(&json)?;
    index.save(&root)?;

    println!("✓ Pack imported: {}", id);
    Ok(())
}

/// `route pack delete <id>` — delete a pack.
pub fn pack_delete(id: String) -> Result<()> {
    use route_basic::pack::PackIndex;

    let root = current_project_root();
    let mut index = PackIndex::load(&root)?;

    index.remove(&id);
    index.save(&root)?;

    println!("✓ Pack '{}' deleted", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Idea commands — route idea add | list | show | accept | reject | implemented | delete
// ---------------------------------------------------------------------------

/// `route idea add <text> --source <source>` — add a new idea.
pub fn idea_add(
    text: String,
    source: String,
    task: Option<String>,
    tags: Vec<String>,
) -> Result<()> {
    use route_basic::idea::IdeaStore;

    let root = current_project_root();
    let mut store = IdeaStore::load(&root)?;

    let idea = store.add(&text, &source, task, tags)?;
    store.save(&root)?;

    println!("✓ Idea added: {}", idea.id);
    println!("  Text:   {}", idea.text);
    println!("  Source: {}", idea.source);
    println!("  Status: {:?}", idea.status);
    Ok(())
}

/// `route idea list [--status <status>]` — list ideas.
pub fn idea_list(status: Option<String>) -> Result<()> {
    use route_basic::idea::{format_idea, IdeaStore};

    let root = current_project_root();
    let store = IdeaStore::load(&root)?;

    let ideas = store.list(status.as_deref());
    if ideas.is_empty() {
        println!("(no ideas found)");
        return Ok(());
    }

    println!("Ideas ({})", ideas.len());
    println!("{:-<80}", "");
    for idea in ideas {
        println!("{}", format_idea(idea));
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route idea show <id>` — show an idea.
pub fn idea_show(id: String) -> Result<()> {
    use route_basic::idea::{format_idea, IdeaStore};

    let root = current_project_root();
    let store = IdeaStore::load(&root)?;

    let idea = store
        .get(&id)
        .ok_or_else(|| anyhow!("Idea '{}' not found", id))?;

    println!("{}", format_idea(idea));
    Ok(())
}

/// `route idea accept <id>` — accept an idea.
pub fn idea_accept(id: String) -> Result<()> {
    use route_basic::idea::{IdeaStatus, IdeaStore};

    let root = current_project_root();
    let mut store = IdeaStore::load(&root)?;

    store.set_status(&id, IdeaStatus::Accepted)?;
    store.save(&root)?;

    println!("✓ Idea '{}' accepted", id);
    Ok(())
}

/// `route idea reject <id>` — reject an idea.
pub fn idea_reject(id: String) -> Result<()> {
    use route_basic::idea::{IdeaStatus, IdeaStore};

    let root = current_project_root();
    let mut store = IdeaStore::load(&root)?;

    store.set_status(&id, IdeaStatus::Rejected)?;
    store.save(&root)?;

    println!("✓ Idea '{}' rejected", id);
    Ok(())
}

/// `route idea implemented <id>` — mark an idea as implemented.
pub fn idea_implemented(id: String) -> Result<()> {
    use route_basic::idea::{IdeaStatus, IdeaStore};

    let root = current_project_root();
    let mut store = IdeaStore::load(&root)?;

    store.set_status(&id, IdeaStatus::Implemented)?;
    store.save(&root)?;

    println!("✓ Idea '{}' marked as implemented", id);
    Ok(())
}

/// `route idea delete <id>` — delete an idea.
pub fn idea_delete(id: String) -> Result<()> {
    use route_basic::idea::IdeaStore;

    let root = current_project_root();
    let mut store = IdeaStore::load(&root)?;

    store.remove(&id);
    store.save(&root)?;

    println!("✓ Idea '{}' deleted", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Failure commands — route failure list | show | search | resolve | delete | add
// ---------------------------------------------------------------------------

/// `route failure list [--severity <severity>]` — list failure cases.
pub fn failure_list(severity: Option<String>) -> Result<()> {
    use route_basic::failure::{format_failure_case, FailureLibrary};

    let root = current_project_root();
    let lib = FailureLibrary::load(&root)?;

    let cases: Vec<_> = match severity {
        Some(ref sev) => lib
            .list()
            .iter()
            .filter(|c| c.severity.to_lowercase() == sev.to_lowercase())
            .collect(),
        None => lib.list().iter().collect(),
    };

    if cases.is_empty() {
        println!("(no failure cases found)");
        return Ok(());
    }

    println!("Failure Cases ({})", cases.len());
    println!("{:-<80}", "");
    for case in cases {
        println!("{}", format_failure_case(case));
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route failure show <id>` — show a failure case.
pub fn failure_show(id: String) -> Result<()> {
    use route_basic::failure::{format_failure_case, FailureLibrary};

    let root = current_project_root();
    let lib = FailureLibrary::load(&root)?;

    let case = lib
        .get(&id)
        .ok_or_else(|| anyhow!("Failure case '{}' not found", id))?;

    println!("{}", format_failure_case(case));
    Ok(())
}

/// `route failure search <query>` — search failure cases.
pub fn failure_search(query: String) -> Result<()> {
    use route_basic::failure::{format_failure_case, FailureLibrary};

    let root = current_project_root();
    let lib = FailureLibrary::load(&root)?;

    let results = lib.search(&query);
    if results.is_empty() {
        println!("(no failure cases matching '{}')", query);
        return Ok(());
    }

    println!("Failure Cases matching '{}' ({}):", query, results.len());
    println!("{:-<80}", "");
    for case in results {
        println!("{}", format_failure_case(case));
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route failure resolve <id> <resolution>` — resolve a failure case.
pub fn failure_resolve(id: String, resolution: String) -> Result<()> {
    use route_basic::failure::FailureLibrary;

    let root = current_project_root();
    let mut lib = FailureLibrary::load(&root)?;

    lib.resolve(&id, &resolution)?;
    lib.save(&root)?;

    println!("✓ Failure case '{}' resolved", id);
    println!("  Resolution: {}", resolution);
    Ok(())
}

/// `route failure delete <id>` — delete a failure case.
pub fn failure_delete(id: String) -> Result<()> {
    use route_basic::failure::FailureLibrary;

    let root = current_project_root();
    let mut lib = FailureLibrary::load(&root)?;

    lib.remove(&id);
    lib.save(&root)?;

    println!("✓ Failure case '{}' deleted", id);
    Ok(())
}

/// `route failure add <problem> <attempt> <symptom>` — add a failure case.
pub fn failure_add(
    problem: String,
    attempt: String,
    symptom: String,
    cause: Option<String>,
    scope: Vec<String>,
    tags: Vec<String>,
    severity: String,
) -> Result<()> {
    use route_basic::failure::{FailureCase, FailureLibrary};

    let root = current_project_root();
    let mut lib = FailureLibrary::load(&root)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let id = format!("failure-{}", now);

    let case = FailureCase {
        id: id.clone(),
        problem,
        attempt,
        symptom,
        root_cause: cause,
        resolution: None,
        affected_scope: scope,
        evidence: Vec::new(),
        tags,
        created_at: now,
        resolved_at: None,
        severity,
        resolved: false,
    };

    lib.add(case);
    lib.save(&root)?;

    println!("✓ Failure case added: {}", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Principle commands — route principle list | show | apply | promote | reject
// ---------------------------------------------------------------------------

/// `route principle list [--status <status>]` — list principle candidates.
pub fn principle_list(status: Option<String>) -> Result<()> {
    use route_basic::principle::{format_principle, PrincipleStore};

    let root = current_project_root();
    let store = PrincipleStore::load(&root)?;

    let candidates = store.list(status.as_deref());
    if candidates.is_empty() {
        println!("(no principle candidates found)");
        return Ok(());
    }

    println!("Principle Candidates ({})", candidates.len());
    println!("{:-<80}", "");
    for c in candidates {
        println!("{}", format_principle(c));
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route principle show <id>` — show a principle candidate.
pub fn principle_show(id: String) -> Result<()> {
    use route_basic::principle::{format_principle, PrincipleStore};

    let root = current_project_root();
    let store = PrincipleStore::load(&root)?;

    let candidate = store
        .get(&id)
        .ok_or_else(|| anyhow!("Principle candidate '{}' not found", id))?;

    println!("{}", format_principle(candidate));
    Ok(())
}

/// `route principle apply <id>` — apply a principle to memory.
pub fn principle_apply(id: String) -> Result<()> {
    use route_basic::principle::PrincipleStore;

    let root = current_project_root();
    let mut store = PrincipleStore::load(&root)?;

    store.apply_to_memory(&id)?;
    store.save(&root)?;

    println!("✓ Principle '{}' applied to memory", id);
    Ok(())
}

/// `route principle promote <id>` — promote a principle to protocol.
pub fn principle_promote(id: String) -> Result<()> {
    use route_basic::principle::PrincipleStore;

    let root = current_project_root();
    let mut store = PrincipleStore::load(&root)?;

    store.promote_to_protocol(&id)?;
    store.save(&root)?;

    println!("✓ Principle '{}' promoted to protocol", id);
    Ok(())
}

/// `route principle reject <id>` — reject a principle candidate.
pub fn principle_reject(id: String) -> Result<()> {
    use route_basic::principle::PrincipleStore;

    let root = current_project_root();
    let mut store = PrincipleStore::load(&root)?;

    store.reject(&id)?;
    store.save(&root)?;

    println!("✓ Principle '{}' rejected", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Agent template commands — route agent templates | show | record-success | record-failure
// ---------------------------------------------------------------------------

/// `route agent templates [--pattern <pattern>]` — list role templates.
pub fn agent_templates(pattern: Option<String>) -> Result<()> {
    use route_basic::role_template::{format_role_template, RoleTemplateStore};

    let root = current_project_root();
    let store = RoleTemplateStore::load(&root)?;

    let templates: Vec<&route_basic::role_template::RoleTemplate> = match pattern {
        Some(ref p) => store.find_for_pattern(p),
        None => store.list().iter().collect(),
    };

    if templates.is_empty() {
        println!("(no role templates found)");
        return Ok(());
    }

    println!("Role Templates ({})", templates.len());
    println!("{:-<80}", "");
    for t in templates {
        println!("{}", format_role_template(t));
        println!("{:-<80}", "");
    }
    Ok(())
}

/// `route agent show <id>` — show a role template.
pub fn agent_show(id: String) -> Result<()> {
    use route_basic::role_template::{format_role_template, RoleTemplateStore};

    let root = current_project_root();
    let store = RoleTemplateStore::load(&root)?;

    let template = store
        .get(&id)
        .ok_or_else(|| anyhow!("Role template '{}' not found", id))?;

    println!("{}", format_role_template(template));
    Ok(())
}

/// `route agent record-success <id> <session>` — record a successful usage.
pub fn agent_record_success(id: String, session: String) -> Result<()> {
    use route_basic::role_template::RoleTemplateStore;

    let root = current_project_root();
    let mut store = RoleTemplateStore::load(&root)?;

    store.record_success(&id, &session)?;
    store.save(&root)?;

    println!(
        "✓ Success recorded for template '{}' (session: {})",
        id, session
    );
    Ok(())
}

/// `route agent record-failure <id> <session>` — record a failed usage.
pub fn agent_record_failure(id: String, session: String) -> Result<()> {
    use route_basic::role_template::RoleTemplateStore;

    let root = current_project_root();
    let mut store = RoleTemplateStore::load(&root)?;

    store.record_failure(&id, &session)?;
    store.save(&root)?;

    println!(
        "✓ Failure recorded for template '{}' (session: {})",
        id, session
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Impact command — route impact <change>
// ---------------------------------------------------------------------------

/// `route impact <change>` — analyze the impact of a change.
pub fn impact(change: String) -> Result<()> {
    use route_basic::failure::FailureLibrary;
    use route_basic::impact::{analyze_impact, format_impact_report, ImpactStore};
    use route_basic::knowledge_map::KnowledgeMap;
    use route_basic::memory::ProjectMemory;

    let root = current_project_root();

    let memory = ProjectMemory::refresh(&root, None).ok();
    let failures = FailureLibrary::load(&root).ok();
    let knowledge_map = KnowledgeMap::load(&root).ok();

    let report = analyze_impact(
        &change,
        memory.as_ref(),
        failures.as_ref(),
        knowledge_map.as_ref(),
    )?;

    // Save the report
    let mut store = ImpactStore::load(&root)?;
    store.add(report.clone());
    store.save(&root)?;

    println!("{}", format_impact_report(&report));
    Ok(())
}

// ---------------------------------------------------------------------------
// Memory map command — route memory map [--topic <topic>]
// ---------------------------------------------------------------------------

/// `route memory map [--topic <topic>]` — show the knowledge map.
pub fn memory_map(topic: Option<String>) -> Result<()> {
    use route_basic::knowledge_map::KnowledgeMap;
    use route_basic::memory::ProjectMemory;

    let root = current_project_root();
    let mut map = KnowledgeMap::load(&root)?;

    // If empty, build from memory
    if map.nodes.is_empty() {
        if let Ok(memory) = ProjectMemory::refresh(&root, None) {
            map.build_from_memory(&memory);
            let _ = map.save(&root);
        }
    }

    println!("Knowledge Map");
    println!("{:-<60}", "");

    let rendered = map.render_map(topic.as_deref());
    println!("{}", rendered);

    if !map.nodes.is_empty() {
        println!("Nodes: {}  Edges: {}", map.nodes.len(), map.edges.len());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Task inspect resources — route task inspect-resources <id>
// ---------------------------------------------------------------------------

/// `route task inspect-resources <id>` — inspect task binding resources.
pub fn task_inspect_resources(id: String) -> Result<()> {
    use route_basic::binding::BindingStore;

    let root = current_project_root();
    let store = BindingStore::load(&root)?;

    let detail = store.inspect_resources(&id)?;
    println!("{}", detail);
    Ok(())
}

// ---------------------------------------------------------------------------
// Health commands — route health / route health-history
// ---------------------------------------------------------------------------

/// `route health` — generate a project health snapshot.
pub fn health(explain: bool) -> Result<()> {
    use route_basic::{format_health_snapshot, HealthStore, ProjectHealthSnapshot};

    let root = current_project_root();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut store = HealthStore::load(&root)?;
    let mut signals = Vec::new();
    let mut risks = Vec::new();
    let mut stale_items = Vec::new();
    let mut unresolved = Vec::new();
    let mut open_questions = Vec::new();
    let mut pending_ideas = Vec::new();
    let memory_drift = Vec::new();
    let mut reference_drift = Vec::new();
    let workflow_drift = Vec::new();
    let suggested_actions = Vec::new();

    // Check unresolved failures
    if let Ok(lib) = route_basic::failure::FailureLibrary::load(&root) {
        let unresolved_count: Vec<_> = lib.cases.iter().filter(|c| !c.resolved).collect();
        if !unresolved_count.is_empty() {
            risks.push(route_basic::HealthRisk {
                title: format!("{} unresolved failure cases", unresolved_count.len()),
                severity: "medium".to_string(),
                explanation: "Failures that have not been resolved".to_string(),
                evidence: unresolved_count.iter().map(|c| c.id.clone()).collect(),
                suggested_actions: vec!["Review and resolve each failure case".to_string()],
            });
            unresolved = unresolved_count.iter().map(|c| c.id.clone()).collect();
        }
    }

    // Check disabled references
    if let Ok(registry) = route_basic::constitutive::ReferenceRegistry::read(&root) {
        let disabled: Vec<_> = registry.entries.iter().filter(|e| !e.enabled).collect();
        if !disabled.is_empty() {
            reference_drift.push(format!("{} disabled references", disabled.len()));
            stale_items.push(format!("{} disabled references", disabled.len()));
        }
    }

    // Check accepted ideas
    if let Ok(store) = route_basic::idea::IdeaStore::load(&root) {
        let accepted: Vec<_> = store
            .ideas
            .iter()
            .filter(|i| matches!(i.status, route_basic::IdeaStatus::Accepted))
            .collect();
        if !accepted.is_empty() {
            pending_ideas = accepted.iter().map(|i| i.id.clone()).collect();
            signals.push(route_basic::HealthSignalItem {
                signal: route_basic::HealthSignal::Info,
                title: format!("{} accepted ideas awaiting implementation", accepted.len()),
                explanation: "Ideas approved but not yet started".to_string(),
                evidence: accepted.iter().map(|i| i.id.clone()).collect(),
            });
        }
    }

    // Check memory
    if let Ok(mem) = route_basic::memory::MemoryStore::load(&root) {
        if let Some(current) = mem.current_memory() {
            let qs: Vec<_> = current
                .open_questions
                .iter()
                .filter(|q| !q.content.is_empty())
                .collect();
            if !qs.is_empty() {
                open_questions = qs.iter().map(|q| q.id.clone()).collect();
            }
        }
    }

    // Check forced-unverified sessions
    if let Ok(store) = route_basic::execution::SessionStore::load(&root) {
        let forced: Vec<_> = store
            .sessions
            .iter()
            .filter(|s| s.status == route_basic::execution::SessionStatus::ForcedUnverified)
            .collect();
        if !forced.is_empty() {
            risks.push(route_basic::HealthRisk {
                title: format!("{} forced-unverified sessions", forced.len()),
                severity: "high".to_string(),
                explanation: "Sessions that bypassed verification".to_string(),
                evidence: forced.iter().map(|s| s.id.clone()).collect(),
                suggested_actions: vec!["Review each forced-unverified session".to_string()],
            });
        }
    }

    let snapshot = ProjectHealthSnapshot {
        id: format!("health-{}", now),
        created_at: now,
        context_hash: String::new(),
        signals,
        risks,
        stale_items,
        unresolved_failures: unresolved,
        open_questions,
        pending_ideas,
        memory_drift,
        reference_drift,
        workflow_drift,
        suggested_actions,
    };

    store.add(snapshot);
    store.save(&root)?;

    let latest = store.latest().unwrap();
    println!("{}", format_health_snapshot(latest, explain));
    Ok(())
}

/// `route health history` — show health history.
pub fn health_history() -> Result<()> {
    use route_basic::HealthStore;

    let root = current_project_root();
    let store = HealthStore::load(&root)?;

    let snapshots = store.list();
    if snapshots.is_empty() {
        println!("No health snapshots yet. Run `route health` first.");
        return Ok(());
    }

    println!("Health History ({} snapshots):\n", snapshots.len());
    for s in snapshots {
        println!("  [{}] {}", s.id, s.created_at);
        println!(
            "       Signals: {}, Risks: {}, Unresolved: {}",
            s.signals.len(),
            s.risks.len(),
            s.unresolved_failures.len()
        );
        println!();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Guardian commands — route guardian scan / findings / show / resolve / ignore
// ---------------------------------------------------------------------------

/// `route guardian scan` — run a guardian scan.
pub fn guardian_scan() -> Result<()> {
    use route_basic::{format_finding, guardian_scan, GuardianFindingsStore};

    let root = current_project_root();
    let result = guardian_scan(&root)?;

    // Save findings to store
    let mut store = GuardianFindingsStore::load(&root)?;
    for finding in &result.findings {
        store.add(finding.clone());
    }
    store.save(&root)?;

    if result.findings.is_empty() {
        println!("No findings detected. Project state looks clean.");
        return Ok(());
    }

    println!("Guardian Scan — {} findings:\n", result.findings.len());
    for finding in &result.findings {
        println!("{}", format_finding(finding));
        println!();
    }
    Ok(())
}

/// `route guardian findings [--status]` — list findings.
pub fn guardian_findings(status: Option<String>) -> Result<()> {
    use route_basic::GuardianFindingsStore;

    let root = current_project_root();
    let store = GuardianFindingsStore::load(&root)?;
    let findings = store.list(status.as_deref());

    if findings.is_empty() {
        println!("No findings.");
        return Ok(());
    }

    println!("Guardian Findings ({}):\n", findings.len());
    for f in &findings {
        println!(
            "  [{}] {:?} — {} (status: {:?})",
            f.id, f.severity, f.title, f.status
        );
    }
    Ok(())
}

/// `route guardian show <id>` — show a specific finding.
pub fn guardian_show(id: String) -> Result<()> {
    use route_basic::{format_finding, GuardianFindingsStore};

    let root = current_project_root();
    let store = GuardianFindingsStore::load(&root)?;
    let finding = store
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Finding '{}' not found", &id))?;

    println!("{}", format_finding(finding));
    Ok(())
}

/// `route guardian resolve <id>` — resolve a finding.
pub fn guardian_resolve(id: String) -> Result<()> {
    use route_basic::{FindingStatus, GuardianFindingsStore};

    let root = current_project_root();
    let mut store = GuardianFindingsStore::load(&root)?;
    store.set_status(&id, FindingStatus::Resolved)?;
    store.save(&root)?;
    println!("Finding '{}' resolved.", id);
    Ok(())
}

/// `route guardian ignore <id>` — ignore a finding.
pub fn guardian_ignore(id: String) -> Result<()> {
    use route_basic::{FindingStatus, GuardianFindingsStore};

    let root = current_project_root();
    let mut store = GuardianFindingsStore::load(&root)?;
    store.set_status(&id, FindingStatus::Ignored)?;
    store.save(&root)?;
    println!("Finding '{}' ignored.", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Next action commands — route next / route next start
// ---------------------------------------------------------------------------

/// `route next` — show next action proposals.
pub fn next(limit: usize) -> Result<()> {
    use route_basic::{
        format_action_proposal, generate_next_actions, GoalStore, GuardianFindingsStore,
        NextActionStore,
    };

    let root = current_project_root();
    let findings = GuardianFindingsStore::load(&root)?;
    let goals = GoalStore::load(&root)?;

    let proposals = generate_next_actions(&root, &findings.findings, &goals.goals, limit)?;

    let mut store = NextActionStore::load()?;
    store.proposals = proposals.clone();

    if proposals.is_empty() {
        println!("No action proposals. Project state looks clean.");
        return Ok(());
    }

    println!("Next Action Proposals ({}):\n", proposals.len());
    for p in &proposals {
        println!("{}", format_action_proposal(p));
        println!();
    }
    Ok(())
}

/// `route next start <proposal-id> --target claude` — start a task from a proposal.
pub fn next_start(proposal_id: String, target: String) -> Result<()> {
    use route_basic::{
        adapter::ApplyTarget, execution::start_task_session, generate_next_actions, GoalStore,
        GuardianFindingsStore,
    };

    let root = current_project_root();
    let findings = GuardianFindingsStore::load(&root)?;
    let goals = GoalStore::load(&root)?;

    let proposals = generate_next_actions(&root, &findings.findings, &goals.goals, 10)?;
    let proposal = proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .ok_or_else(|| anyhow::anyhow!("Proposal '{}' not found", &proposal_id))?;

    let task_desc = format!("{}: {}", proposal.title, proposal.why_now);

    // Start task session directly — context building is handled inside
    let target_enum = match target.as_str() {
        "claude" => ApplyTarget::Claude,
        "codex" => ApplyTarget::Codex,
        "generic" => ApplyTarget::Generic,
        _ => ApplyTarget::Claude,
    };

    match start_task_session(
        &root,
        &task_desc,
        target_enum,
        proposal.suggested_strategy.clone(),
        None,
    ) {
        Ok(result) => {
            println!(
                "Task started: {} (session: {})",
                task_desc, result.session.id
            );
            if !result.host_instructions.is_empty() {
                println!("\nHost Instructions:\n{}", result.host_instructions);
            }
        }
        Err(e) => {
            if e.to_string().contains("is still Active") {
                // Extract session id from error message
                println!("Active session already exists. Check: {}", e);
            } else {
                return Err(e);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Goal commands — route goal add/list/show/update/delete
// ---------------------------------------------------------------------------

/// `route goal add` — add a new goal.
pub fn goal_add(
    title: String,
    description: Option<String>,
    priority: u8,
    criteria: Vec<String>,
    tags: Vec<String>,
) -> Result<()> {
    use route_basic::GoalStore;

    let root = current_project_root();
    let mut store = GoalStore::load(&root)?;
    let goal = store.create_goal(&title, description, priority, None, criteria, tags)?;
    store.save(&root)?;

    println!("Goal created: [{}] {}", goal.id, goal.title);
    Ok(())
}

/// `route goal list` — list goals.
pub fn goal_list(status: Option<String>) -> Result<()> {
    use route_basic::{format_goal, GoalStore};

    let root = current_project_root();
    let store = GoalStore::load(&root)?;
    let goals = store.list(status.as_deref());

    if goals.is_empty() {
        println!("No goals.");
        return Ok(());
    }

    println!("Goals ({}):\n", goals.len());
    for g in &goals {
        println!("{}", format_goal(g));
        println!();
    }
    Ok(())
}

/// `route goal show <id>` — show a specific goal.
pub fn goal_show(id: String) -> Result<()> {
    use route_basic::{format_goal, GoalStore};

    let root = current_project_root();
    let store = GoalStore::load(&root)?;
    let goal = store
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Goal '{}' not found", &id))?;

    println!("{}", format_goal(goal));
    Ok(())
}

/// `route goal update <id> --status <status>` — update goal status.
pub fn goal_update(id: String, status: String) -> Result<()> {
    use route_basic::GoalStore;

    let root = current_project_root();
    let mut store = GoalStore::load(&root)?;

    let new_status = match status.to_lowercase().as_str() {
        "active" => route_basic::goal::GoalStatus::Active,
        "paused" => route_basic::goal::GoalStatus::Paused,
        "done" => route_basic::goal::GoalStatus::Done,
        "abandoned" => route_basic::goal::GoalStatus::Abandoned,
        _ => {
            return Err(anyhow::anyhow!(
                "Invalid status: {}. Use: active | paused | done | abandoned",
                status
            ))
        }
    };

    store.set_status(&id, new_status.clone())?;
    store.save(&root)?;
    println!("Goal '{}' updated to {:?}.", id, new_status);
    Ok(())
}

/// `route goal delete <id>` — delete a goal.
pub fn goal_delete(id: String) -> Result<()> {
    use route_basic::GoalStore;

    let root = current_project_root();
    let mut store = GoalStore::load(&root)?;
    store.remove(&id);
    store.save(&root)?;
    println!("Goal '{}' deleted.", id);
    Ok(())
}

// ---------------------------------------------------------------------------
// Brain commands — route brain
// ---------------------------------------------------------------------------

/// `route brain show` — show current brain.
pub fn brain_show() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;
    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found. Run `route brain refresh` first."))?;
    println!("{}", route_basic::format_brain(brain));
    Ok(())
}

/// `route brain brief` — L0 brief.
pub fn brain_brief() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;
    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found. Run `route brain refresh` first."))?;
    let brief = route_basic::brain_brief(brain);
    println!("{}", route_basic::format_brain_brief(&brief));
    Ok(())
}

/// `route brain refresh` — refresh brain from all data sources.
pub fn brain_refresh() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;

    let proposals = route_basic::refresh_brain(&root)?;

    if proposals.is_empty() {
        println!("No brain proposals generated (no data sources found).");
        println!("  Run some tasks, studies, or add project memory first.");
        return Ok(());
    }

    println!("Brain Proposals ({}):", proposals.len());
    println!("{:-<80}", "");
    for (i, p) in proposals.iter().enumerate() {
        let action_char = match p.action.as_str() {
            "add" => '+',
            "supersede" => '~',
            "archive" => '-',
            _ => '?',
        };
        println!(
            "  [{:3}] {}{} — {} ({})",
            i, action_char, p.category, p.reason, p.status
        );
    }
    println!();
    println!("Run `route brain apply all` to apply all proposals, or `route brain apply <index>` for a specific one.");

    // Save current store state (proposals are not persisted here, they're regenerated on apply)
    store.save(&root)?;
    Ok(())
}

/// `route brain apply <index>` — apply brain proposals.
pub fn brain_apply(index: String) -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::BrainStore::load(&root)?;

    let proposals = route_basic::refresh_brain(&root)?;

    if proposals.is_empty() {
        println!("No proposals to apply.");
        return Ok(());
    }

    if index == "all" {
        store.apply_proposals(&proposals)?;
        store.detect_and_store_conflicts();
        store.save(&root)?;
        println!(
            "Applied all {} proposals. Brain v{} created.",
            proposals.len(),
            store.current.as_ref().map(|b| b.version).unwrap_or(0)
        );
    } else {
        let idx: usize = index
            .parse()
            .map_err(|_| anyhow!("Invalid index: {}", index))?;
        if idx >= proposals.len() {
            return Err(anyhow!(
                "Index {} out of range (0-{})",
                idx,
                proposals.len() - 1
            ));
        }
        store.apply_proposals(&[proposals[idx].clone()])?;
        store.detect_and_store_conflicts();
        store.save(&root)?;
        println!(
            "Applied proposal [{}]. Brain v{} created.",
            idx,
            store.current.as_ref().map(|b| b.version).unwrap_or(0)
        );
    }

    Ok(())
}

/// `route brain history` — show brain history.
pub fn brain_history() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;

    let history = store.history();
    if history.is_empty() {
        println!("No brain history available.");
        return Ok(());
    }

    println!("Brain History ({} versions):", history.len());
    println!("{:-<80}", "");
    for (i, brain) in history.iter().enumerate().rev() {
        let current_count = route_basic::current_items(brain).len();
        let historical_count = route_basic::historical_items(brain).len();
        println!(
            "  [{:3}] v{} — {} items ({} current, {} historical) — updated: {}",
            i,
            brain.version,
            brain.items.len(),
            current_count,
            historical_count,
            brain.updated_at
        );
    }
    Ok(())
}

/// `route brain explain <id>` — explain a brain item with full source chain.
pub fn brain_explain(id: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;
    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found."))?;

    let expanded = route_basic::expand_item(brain, &id)
        .ok_or_else(|| anyhow!("Brain item '{}' not found", id))?;
    println!("{}", expanded);
    Ok(())
}

/// `route brain for <task>` — task-specific brain view.
pub fn brain_for(task: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;
    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found."))?;

    let relevant = route_basic::task_brain(brain, &task);
    println!("{}", route_basic::format_relevant_knowledge(&relevant));
    Ok(())
}

/// `route brain expand <id>` — expand a brain item.
pub fn brain_expand(id: String) -> Result<()> {
    // Same as explain for now
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;
    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found."))?;

    let expanded = route_basic::expand_item(brain, &id)
        .ok_or_else(|| anyhow!("Brain item '{}' not found", id))?;
    println!("{}", expanded);
    Ok(())
}

/// `route brain conflicts` — show knowledge conflicts.
pub fn brain_conflicts() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;

    if store.conflicts.is_empty() {
        // Try detecting conflicts from current brain
        if let Some(ref brain) = store.current {
            let conflicts = route_basic::detect_contradictions(brain);
            if conflicts.is_empty() {
                println!("No knowledge conflicts detected.");
                return Ok(());
            }
            println!("{}", route_basic::format_conflicts(&conflicts));
        } else {
            println!("No brain found. Run `route brain refresh` first.");
        }
        return Ok(());
    }

    println!("{}", route_basic::format_conflicts(&store.conflicts));
    Ok(())
}

/// `route brain compact` — compact brain without deleting data.
pub fn brain_compact() -> Result<()> {
    let root = current_project_root();
    let mut store = route_basic::BrainStore::load(&root)?;

    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found."))?;

    let compacted = route_basic::compact_brain(brain);
    let old_count = brain.items.len();
    let new_count = compacted.items.len();
    store.current = Some(compacted);
    store.save(&root)?;

    println!(
        "Brain compacted: {} items → {} items (removed {} duplicates).",
        old_count,
        new_count,
        old_count - new_count
    );
    Ok(())
}

/// `route brain doctor` — run quality checks.
pub fn brain_doctor() -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;
    let brain = store
        .current
        .as_ref()
        .ok_or_else(|| anyhow!("No brain found."))?;

    let report = route_basic::brain_doctor(brain, &root);
    println!("{}", route_basic::format_doctor_report(&report));
    Ok(())
}

/// `route brain diff <a> <b>` — diff two brain versions.
pub fn brain_diff(a: String, b: String) -> Result<()> {
    let root = current_project_root();
    let store = route_basic::BrainStore::load(&root)?;

    let brain_a = if a == "current" {
        store
            .current
            .as_ref()
            .ok_or_else(|| anyhow!("No current brain."))?
            .clone()
    } else {
        let idx: usize = a
            .parse()
            .map_err(|_| anyhow!("Invalid version index: {}", a))?;
        store
            .history()
            .get(idx)
            .ok_or_else(|| anyhow!("History index {} out of range", idx))?
            .clone()
    };

    let brain_b = if b == "current" {
        store
            .current
            .as_ref()
            .ok_or_else(|| anyhow!("No current brain."))?
            .clone()
    } else {
        let idx: usize = b
            .parse()
            .map_err(|_| anyhow!("Invalid version index: {}", b))?;
        store
            .history()
            .get(idx)
            .ok_or_else(|| anyhow!("History index {} out of range", idx))?
            .clone()
    };

    let diff = route_basic::diff_brain(&brain_a, &brain_b);
    println!("{}", route_basic::format_brain_diff(&diff));
    Ok(())
}

// ---------------------------------------------------------------------------
// Archive commands — route archive
// ---------------------------------------------------------------------------

fn get_project_id_and_repo() -> Result<(String, route_basic::BasicRepository)> {
    let root = current_project_root();
    let project_id = route_basic::project_id_from_path(&root);
    let repo = route_basic::BasicRepository::open(&root)?;
    Ok((project_id, repo))
}

/// `route archive init` — initialize archive for current project.
pub fn archive_init() -> Result<()> {
    let root = current_project_root();
    let repo = route_basic::BasicRepository::open(&root)?;
    let route_version = env!("CARGO_PKG_VERSION");
    let project_id = route_basic::init_project_archive(&root, &repo, route_version)?;
    println!("Archive initialized for project '{}'.", project_id);
    println!(
        "  Location: {}",
        route_basic::project_archive_dir(&project_id)?.display()
    );
    println!("  Original snapshot created.");
    Ok(())
}

/// `route archive save <reason>` — create a manual save.
pub fn archive_save(reason: String) -> Result<()> {
    let root = current_project_root();
    let (project_id, repo) = get_project_id_and_repo()?;
    let route_version = env!("CARGO_PKG_VERSION");
    let save_id = route_basic::create_save(
        &root,
        &project_id,
        &repo,
        &reason,
        true,
        route_version,
        None,
        None,
        None,
        None,
        None,
    )?;
    println!("Save created: {} ({})", &save_id[..16], reason);
    Ok(())
}

/// `route archive list` — list all saves.
pub fn archive_list(json: bool) -> Result<()> {
    let (project_id, _repo) = get_project_id_and_repo()?;
    let saves = route_basic::list_saves(&project_id)?;
    if json {
        let output = serde_json::json!({
            "project_id": &project_id[..16],
            "count": saves.len(),
            "saves": saves,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }
    if saves.is_empty() {
        println!("No saves found for project '{}'.", project_id);
        println!("  Run `route archive save \"reason\"` or `route archive init` first.");
        return Ok(());
    }
    println!(
        "Saves for project '{}' ({} total):",
        &project_id[..16],
        saves.len()
    );
    println!("{:-<80}", "");
    for (i, s) in saves.iter().enumerate() {
        println!("{}", route_basic::format_save_summary(s, i));
    }
    Ok(())
}

/// `route archive show <id>` — show save details.
pub fn archive_show(id: String, json: bool) -> Result<()> {
    let (project_id, _repo) = get_project_id_and_repo()?;
    let save = route_basic::load_save(&project_id, &id)?
        .ok_or_else(|| anyhow!("Save '{}' not found", id))?;

    if json {
        let output = serde_json::json!({
            "id": save.id,
            "reason": save.reason,
            "created_at": save.created_at,
            "is_manual": save.is_manual,
            "file_count": save.project_state.file_count,
            "manifest_hash": &save.project_state.manifest_hash[..16],
            "verification_state": save.verification_state,
            "task_id": save.task_id,
            "session_id": save.session_id,
            "learning_link": save.learning_link,
            "route_version": save.route_state.route_version,
            "context_hash": save.route_state.context_hash,
            "memory_ref": save.route_state.memory_ref,
            "strategy_ref": save.route_state.strategy_ref,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Save: {}", save.id);
    println!("  Reason: {}", save.reason);
    println!("  Created: {}", save.created_at);
    println!("  Manual: {}", save.is_manual);
    println!("  Files: {}", save.project_state.file_count);
    println!(
        "  Manifest hash: {}",
        &save.project_state.manifest_hash[..16]
    );
    println!(
        "  Verification: {}",
        save.verification_state.as_deref().unwrap_or("none")
    );
    println!("  Task: {}", save.task_id.as_deref().unwrap_or("none"));
    println!(
        "  Session: {}",
        save.session_id.as_deref().unwrap_or("none")
    );
    println!(
        "  Learning link: {}",
        save.learning_link.as_deref().unwrap_or("none")
    );
    println!();
    println!("  Project files:");
    for entry in &save.project_state.entries {
        println!("    {} ({})", entry.path, &entry.blob_hash[..12]);
    }
    println!();
    println!("  Route state:");
    println!("    Version: {}", save.route_state.route_version);
    println!(
        "    Context: {}",
        save.route_state.context_hash.as_deref().unwrap_or("none")
    );
    println!(
        "    Memory: {}",
        save.route_state.memory_ref.as_deref().unwrap_or("none")
    );
    println!(
        "    Strategy: {}",
        save.route_state.strategy_ref.as_deref().unwrap_or("none")
    );
    Ok(())
}

/// `route archive diff <a> <b>` — diff two saves.
pub fn archive_diff(a: String, b: String) -> Result<()> {
    let (project_id, _repo) = get_project_id_and_repo()?;

    let save_a = if a == "original" {
        let orig = route_basic::load_original(&project_id)?.ok_or_else(|| {
            anyhow!("No original snapshot found. Run `route archive init` first.")
        })?;
        // Convert original to SaveEntry for comparison
        route_basic::SaveEntry {
            id: "original".to_string(),
            parent_id: None,
            created_at: orig.created_at,
            reason: "original".to_string(),
            is_manual: false,
            project_state: orig.project_state,
            route_state: orig.route_state,
            context_hash: None,
            task_id: None,
            session_id: None,
            verification_state: None,
            learning_link: None,
            tags: vec![],
        }
    } else {
        route_basic::load_save(&project_id, &a)?.ok_or_else(|| anyhow!("Save '{}' not found", a))?
    };

    let save_b = if b == "current" {
        // Get latest save
        let saves = route_basic::list_saves(&project_id)?;
        let latest = saves
            .first()
            .ok_or_else(|| anyhow!("No saves found. Create a save first."))?;
        route_basic::load_save(&project_id, &latest.id)?
            .ok_or_else(|| anyhow!("Save '{}' not found on disk", latest.id))?
    } else {
        route_basic::load_save(&project_id, &b)?.ok_or_else(|| anyhow!("Save '{}' not found", b))?
    };

    let diff = route_basic::diff_saves(&save_a, &save_b);
    println!("{}", route_basic::format_save_diff(&diff));
    Ok(())
}

/// `route archive restore <id>` — restore from a save.
pub fn archive_restore(id: String, scope: String, paths: Option<String>) -> Result<()> {
    let root = current_project_root();
    let (project_id, mut repo) = get_project_id_and_repo()?;
    let route_version = env!("CARGO_PKG_VERSION");

    let restore_scope = match scope.as_str() {
        "full" => route_basic::ArchiveRestoreScope::Full,
        "project" => route_basic::ArchiveRestoreScope::ProjectOnly,
        "route-state" => route_basic::ArchiveRestoreScope::RouteStateOnly,
        "paths" => {
            let paths = paths.ok_or_else(|| anyhow!("--paths is required for scope=paths"))?;
            route_basic::ArchiveRestoreScope::Paths(
                paths.split(',').map(|s| s.trim().to_string()).collect(),
            )
        }
        _ => {
            return Err(anyhow!(
                "Invalid scope: {}. Expected: full | project | route-state | paths",
                scope
            ))
        }
    };

    let result = route_basic::restore_from_save(
        &root,
        &project_id,
        &mut repo,
        &id,
        &restore_scope,
        route_version,
    )?;

    println!("Restore completed:");
    println!("  Save: {}", result.save_id);
    println!("  Scope: {}", result.scope);
    println!("  Restored files: {}", result.files_restored);
    println!("  Verified hashes: {}", result.files_verified);
    println!("  Route state restored: {}", result.route_state_restored);
    if let Some(ref pre_id) = result.pre_restore_save_id {
        println!("  Pre-restore save: {}", &pre_id[..16]);
    }
    println!("  Restore state: {:?}", result.restore_state());
    println!("  Recovery verification: {}", result.verification_status());
    if !result.errors.is_empty() {
        println!("  Errors ({}):", result.errors.len());
        for e in &result.errors {
            println!("    - {}", e);
        }
    }
    Ok(())
}

/// `route archive recover <project-id> --save <id> --to <path>`
pub fn archive_recover(project_id: String, save: Option<String>, to: String) -> Result<()> {
    let target_path = std::path::PathBuf::from(&to);
    let result = route_basic::recover_project(&project_id, save.as_deref(), &target_path)?;
    println!("Recovery completed:");
    println!("  Project: {}", &result.project_id[..16]);
    println!("  Save: {}", &result.save_id[..16]);
    println!("  Target: {}", result.target_path);
    println!("  Restored files: {}", result.files_restored);
    println!("  Verified hashes: {}", result.files_verified);
    println!("  Recovery verification: {}", result.verification_status());
    if !result.errors.is_empty() {
        println!("  Errors:");
        for e in &result.errors {
            println!("    - {}", e);
        }
    }
    Ok(())
}

/// `route archive recover-list <project-id>`
pub fn archive_recover_list(project_id: String) -> Result<()> {
    let options = route_basic::get_recovery_options(&project_id)?
        .ok_or_else(|| anyhow!("No archive found for project '{}'", project_id))?;
    println!("{}", route_basic::format_recovery_options(&options));
    Ok(())
}

/// `route archive check` — check invariants.
pub fn archive_check() -> Result<()> {
    let (project_id, _repo) = get_project_id_and_repo()?;
    let result = route_basic::check_invariants(&project_id)?;
    println!("{}", route_basic::format_invariant_check(&result));
    Ok(())
}

/// `route archive path-history <path>`
pub fn archive_path_history(path: String) -> Result<()> {
    let (project_id, _repo) = get_project_id_and_repo()?;
    let history = route_basic::path_history(&project_id, &path)?;
    if history.is_empty() {
        println!(
            "No history for path '{}' in project '{}'.",
            path,
            &project_id[..16]
        );
        return Ok(());
    }
    println!("Path history for '{}':", path);
    println!("{:-<80}", "");
    for (save_id, hash, timestamp) in &history {
        let dt = chrono::DateTime::from_timestamp_millis(*timestamp)
            .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!("  {} {} ({})", &save_id[..12], dt, &hash[..12]);
    }
    Ok(())
}

/// `route archive delete` — delete a save.
pub fn archive_delete(id: String, force: bool) -> Result<()> {
    let (project_id, _repo) = get_project_id_and_repo()?;

    if !force {
        let save = route_basic::load_save(&project_id, &id)?
            .ok_or_else(|| anyhow!("Save '{}' not found", id))?;
        eprintln!(
            "WARNING: This will delete save '{}' (reason: {})",
            &id[..12.min(id.len())],
            save.reason
        );
        eprintln!("Use --force to confirm deletion.");
        return Err(anyhow!("Use --force to confirm deletion"));
    }

    match route_basic::delete_save(&project_id, &id)? {
        true => println!("Save '{}' deleted.", &id[..12.min(id.len())]),
        false => {
            if id == "original" {
                println!("Cannot delete the original snapshot.");
            } else {
                println!("Save '{}' not found.", &id[..12.min(id.len())]);
            }
        }
    }
    Ok(())
}

/// `route archive delete-project` — delete entire project archive.
pub fn archive_delete_project(project_id: Option<String>, force: bool) -> Result<()> {
    let pid = match project_id {
        Some(id) => id,
        None => {
            let (id, _repo) = get_project_id_and_repo()?;
            id
        }
    };

    if !force {
        eprintln!(
            "WARNING: This is a HIGH-RISK operation. It will permanently delete the entire archive"
        );
        eprintln!(
            "         for project '{}' including all saves, objects, and original snapshot.",
            &pid[..16.min(pid.len())]
        );
        eprintln!("This action CANNOT be undone.");
        eprintln!("Use --force to confirm deletion.");
        return Err(anyhow!("Use --force to confirm deletion"));
    }

    match route_basic::delete_project_archive(&pid)? {
        true => println!("Project archive '{}' deleted.", &pid[..16.min(pid.len())]),
        false => println!(
            "Project '{}' not found in archive.",
            &pid[..16.min(pid.len())]
        ),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Roadmap command — route roadmap
// ---------------------------------------------------------------------------

/// `route roadmap` — show project roadmap.
pub fn roadmap() -> Result<()> {
    use route_basic::Roadmap;

    let root = current_project_root();
    let roadmap = Roadmap::build(&root)?;
    println!("{}", roadmap.render());
    Ok(())
}

// ---------------------------------------------------------------------------
// Open loops command — route loops
// ---------------------------------------------------------------------------

/// `route loops` — detect open loops.
pub fn loops() -> Result<()> {
    use route_basic::{format_open_loops, scan_open_loops};

    let root = current_project_root();
    let result = scan_open_loops(&root)?;
    println!("{}", format_open_loops(&result));
    Ok(())
}

// ---------------------------------------------------------------------------
// Drift command — route drift
// ---------------------------------------------------------------------------

/// `route drift` — detect drift/decay.
pub fn drift() -> Result<()> {
    use route_basic::{format_drift, scan_drift};

    let root = current_project_root();
    let result = scan_drift(&root)?;
    println!("{}", format_drift(&result));
    Ok(())
}

// ---------------------------------------------------------------------------
// Maintain commands — route maintain check / plan / list / show / start
// ---------------------------------------------------------------------------

/// `route maintain check` — run a single maintenance check.
pub fn maintain_check() -> Result<()> {
    use route_basic::{guardian_scan, scan_drift, scan_open_loops};

    let root = current_project_root();

    println!("=== Maintenance Check ===\n");

    // Guardian scan
    let scan_result = guardian_scan(&root)?;
    println!("Findings: {}\n", scan_result.findings.len());

    // Open loops
    let loop_result = scan_open_loops(&root)?;
    println!("Open Loops: {}\n", loop_result.loops.len());

    // Drift
    let drift_result = scan_drift(&root)?;
    println!("Drift Items: {}\n", drift_result.items.len());

    if scan_result.findings.is_empty()
        && loop_result.loops.is_empty()
        && drift_result.items.is_empty()
    {
        println!("No issues detected. Project state looks clean.");
    } else {
        println!("Run `route guardian scan` for details on findings.");
        println!("Run `route next` for prioritized action proposals.");
    }

    Ok(())
}

/// `route maintain --plan` — generate a new maintenance plan.
pub fn maintain_plan() -> Result<()> {
    use route_basic::{
        format_maintainer_plan, generate_maintainer_plan, guardian_scan, GuardianFindingsStore,
        MaintainerStore,
    };

    let root = current_project_root();

    let scan_result = guardian_scan(&root)?;
    let mut findings_store = GuardianFindingsStore::load(&root)?;
    for f in &scan_result.findings {
        findings_store.add(f.clone());
    }
    findings_store.save(&root)?;

    let plan = generate_maintainer_plan(&root, &findings_store.findings, 3)?;

    let mut store = MaintainerStore::load(&root)?;
    store.add(plan.clone());
    store.save(&root)?;

    println!("{}", format_maintainer_plan(&plan));
    Ok(())
}

/// `route maintain --list` — list maintenance plans.
pub fn maintain_list() -> Result<()> {
    use route_basic::MaintainerStore;

    let root = current_project_root();
    let store = MaintainerStore::load(&root)?;

    let plans = store.list();
    if plans.is_empty() {
        println!("No maintenance plans. Run `route maintain --plan` to generate one.");
        return Ok(());
    }

    println!("Maintenance Plans:\n");
    for p in &plans {
        let completed = p.proposed_tasks.iter().filter(|t| t.completed).count();
        println!(
            "  [{}] {} tasks ({} completed)",
            p.id,
            p.proposed_tasks.len(),
            completed
        );
        for obj in &p.objectives {
            println!("       ◉ {}", obj);
        }
        println!();
    }
    Ok(())
}

/// `route maintain --show <id>` — show a specific maintenance plan.
pub fn maintain_show(id: String) -> Result<()> {
    use route_basic::{format_maintainer_plan, MaintainerStore};

    let root = current_project_root();
    let store = MaintainerStore::load(&root)?;
    let plan = store
        .get(&id)
        .ok_or_else(|| anyhow::anyhow!("Plan '{}' not found", &id))?;

    println!("{}", format_maintainer_plan(plan));
    Ok(())
}

/// `route maintain --start <id> --target claude` — start executing a plan.
pub fn maintain_start(plan_id: String, target: String) -> Result<()> {
    use route_basic::{adapter::ApplyTarget, execution::start_task_session, MaintainerStore};

    let root = current_project_root();
    let mut store = MaintainerStore::load(&root)?;
    let plan = store
        .get(&plan_id)
        .ok_or_else(|| anyhow::anyhow!("Plan '{}' not found", &plan_id))?
        .clone();

    if plan.proposed_tasks.is_empty() {
        println!("No tasks in this plan.");
        return Ok(());
    }

    // Start the first task
    let task = &plan.proposed_tasks[0];
    let task_desc = format!("[Maintenance] {}: {}", task.title, task.description);

    // Update task as started
    if let Some(p) = store.get_mut(&plan_id) {
        if let Some(t) = p.proposed_tasks.get_mut(0) {
            t.started = true;
        }
    }
    store.save(&root)?;

    let target_enum = match target.as_str() {
        "claude" => ApplyTarget::Claude,
        "codex" => ApplyTarget::Codex,
        "generic" => ApplyTarget::Generic,
        _ => ApplyTarget::Claude,
    };

    match start_task_session(&root, &task_desc, target_enum, None, None) {
        Ok(result) => {
            println!("Started maintenance task: {}", task_desc);
            println!("Session: {}", result.session.id);
            if !result.host_instructions.is_empty() {
                println!("\nHost Instructions:\n{}", result.host_instructions);
            }
            println!(
                "\nNote: After this task completes, re-evaluate before starting the next task."
            );
        }
        Err(e) => {
            if e.to_string().contains("is still Active") {
                println!("Active session already exists. Check: {}", e);
            } else {
                return Err(e);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Brief command — route brief
// ---------------------------------------------------------------------------

/// `route brief [--task <task>]` — generate a project brief.
pub fn brief(task: Option<String>) -> Result<()> {
    use route_basic::{format_brief, generate_brief};

    let root = current_project_root();
    let brief = generate_brief(&root, task.as_deref())?;
    println!("{}", format_brief(&brief));
    Ok(())
}

// ---------------------------------------------------------------------------
// Handoff command — route handoff
// ---------------------------------------------------------------------------

/// `route handoff` — generate a handoff document for a new AI session.
pub fn handoff() -> Result<()> {
    use route_basic::{format_handoff, generate_handoff};

    let root = current_project_root();
    let handoff = generate_handoff(&root)?;
    println!("{}", format_handoff(&handoff));
    Ok(())
}

// ---------------------------------------------------------------------------
// Error UX — user-friendly error formatting
// ---------------------------------------------------------------------------

/// Format an error for end-user display with ERROR/CAUSE/SAFE STATE/NEXT
/// sections. Walks the error chain to find a `RouteError` and produces
/// a structured message.
pub fn format_user_error(e: &anyhow::Error) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    // Try to find a RouteError in the chain
    let route_err = e
        .chain()
        .find_map(|c| c.downcast_ref::<route_core::RouteError>());

    match route_err {
        Some(err) => {
            let (error, cause, safe_state, next) = format_route_error(err);
            let _ = writeln!(out, "ERROR: {}", error);
            let _ = writeln!(out, "CAUSE: {}", cause);
            let _ = writeln!(out, "SAFE STATE: {}", safe_state);
            let _ = write!(out, "NEXT: {}", next);
        }
        None => {
            // Generic fallback for non-RouteError. Surface the deepest
            // cause message instead of an opaque "unknown" so the user can
            // actually act on the failure.
            let mut chain: Vec<String> = e.chain().map(|c| c.to_string()).collect();
            chain.dedup();
            let cause = chain
                .last()
                .filter(|c| !c.is_empty())
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let _ = writeln!(out, "ERROR: {}", e);
            let _ = writeln!(out, "CAUSE: {}", cause);
            let _ = writeln!(out, "SAFE STATE: operation may have failed");
            let _ = write!(out, "NEXT: check the error message above and retry");
        }
    }

    out
}

fn format_route_error(
    err: &route_core::RouteError,
) -> (&'static str, &'static str, &'static str, &'static str) {
    match err {
        route_core::RouteError::InvalidRepository(_) => (
            "Not a Route repository",
            "directory is not initialized",
            "no data loss",
            "run `route init`",
        ),
        route_core::RouteError::CorruptedMetadata(_) => (
            "Repository metadata is corrupted",
            "files in .route/ are damaged",
            "repository may be inconsistent",
            "run `route check`",
        ),
        route_core::RouteError::InvalidSnapshot(_) => (
            "Snapshot not found",
            "the referenced snapshot ID doesn't exist",
            "current state is unchanged",
            "check snapshot ID with `route log`",
        ),
        route_core::RouteError::UnsafePath(_) => (
            "Unsafe path refused",
            "path would escape project root",
            "operation was refused",
            "check the path for '..' or absolute paths",
        ),
        route_core::RouteError::PermissionDenied(_) => (
            "Permission denied",
            "OS denied access to a file",
            "partial operation possible",
            "check file permissions",
        ),
        route_core::RouteError::TransactionIncomplete { .. } => (
            "Incomplete transaction",
            "a previous operation was interrupted",
            "repository may be inconsistent",
            "run `route check` to assess state",
        ),
        route_core::RouteError::RecoveryFailed(_) => (
            "Recovery failed",
            "could not restore to a consistent state",
            "project is in the state before recovery",
            "manual intervention required",
        ),
        route_core::RouteError::UnsupportedFormat(_) => (
            "Unsupported format",
            "file format is not recognized",
            "operation refused",
            "check file format",
        ),
        route_core::RouteError::CorruptedJournal { .. } => (
            "Corrupted transaction journal",
            "journal file is damaged",
            "repository may be inconsistent",
            "run `route check`",
        ),
    }
}
