//! Command implementations for the route-tui REPL.
//!
//! Each `cmd_*` function takes the open repository, the whitespace-split
//! args, and the raw remainder (for free-form text like commit messages),
//! and returns `Result<()>`. `dispatch` parses a line, resolves aliases,
//! routes to the handler, and prints any error in red.

use std::path::Path;

use anyhow::{anyhow, Result};
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use route_basic::{
    BasicRepository, BranchKind, CommitOptions, CreateBranchOptions, ExportFormat,
};
use route_core::short_id;

use crate::fmt::{accent, dim, err, faint, format_ts, head, id, ok, warn};
use crate::git;

pub enum Action {
    Continue,
    Exit,
}

/// Parse one input line and execute it.
///
/// `project_path` is the resolved project folder (cwd or `--path`). It's
/// passed separately from `repo` because git mode can operate in a plain
/// folder even when the folder isn't a route_basic repository.
pub fn dispatch(repo: Option<&BasicRepository>, project_path: &Path, input: &str) -> Action {
    let input = input.trim();
    if input.is_empty() {
        return Action::Continue;
    }

    // Split into command word + remainder (for messages).
    let (cmd_word, rest) = match input.find(char::is_whitespace) {
        Some(i) => (&input[..i], input[i..].trim()),
        None => (input, ""),
    };
    let args: Vec<String> = rest.split_whitespace().map(String::from).collect();

    match cmd_word {
        "exit" | "quit" | "q" => {
            println!("{}", dim("bye."));
            return Action::Exit;
        }
        "help" | "?" => {
            print_help();
            return Action::Continue;
        }
        "clear" | "cls" => {
            // ANSI clear screen + move cursor home.
            print!("\x1b[2J\x1b[H");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            return Action::Continue;
        }
        "git" => {
            // Git namespace — works without a route_basic repo. Re-split
            // `rest` into subcommand + its own remainder (for messages).
            // Default (protected) mode: network ops are refused.
            let (sub, sub_rest) = match rest.find(char::is_whitespace) {
                Some(i) => (&rest[..i], rest[i..].trim()),
                None => (rest, ""),
            };
            let sub_args: Vec<String> = sub_rest.split_whitespace().map(String::from).collect();
            let res = cmd_git(project_path, false, sub, &sub_args, sub_rest);
            if let Err(e) = res {
                eprintln!("{}", err(&format!("✗ {e}")));
            }
            return Action::Continue;
        }
        "root" => {
            // High-privilege prefix. `root git <network-sub>` lifts the
            // protected-mode restriction so Route runs push/pull/fetch/
            // remote itself. `root` alone or `root help` explains the model.
            let rest = rest.trim();
            if rest.is_empty() || rest == "help" || rest == "?" {
                print_root_help();
                return Action::Continue;
            }
            // Expect `git <sub> ...`.
            let (git_word, after_git) = match rest.find(char::is_whitespace) {
                Some(i) => (&rest[..i], rest[i..].trim()),
                None => (rest, ""),
            };
            if git_word != "git" {
                eprintln!(
                    "{}",
                    err("✗ `root` only applies to git commands. Try: root git push")
                );
                return Action::Continue;
            }
            let (sub, sub_rest) = match after_git.find(char::is_whitespace) {
                Some(i) => (&after_git[..i], after_git[i..].trim()),
                None => (after_git, ""),
            };
            let sub_args: Vec<String> =
                sub_rest.split_whitespace().map(String::from).collect();
            let res = cmd_git(project_path, true, sub, &sub_args, sub_rest);
            if let Err(e) = res {
                eprintln!("{}", err(&format!("✗ {e}")));
            }
            return Action::Continue;
        }
        _ => {}
    }

    let repo = match repo {
        Some(r) => r,
        None => {
            eprintln!(
                "{}",
                err("✗ Not in a Route repository. Use --path <dir> or run `route init`.")
            );
            return Action::Continue;
        }
    };

    let res: Result<()> = match cmd_word {
        "status" | "st" => cmd_status(repo, &args, rest),
        "log" | "lg" => cmd_log(repo, &args, rest),
        "commit" | "ci" => cmd_commit(repo, &args, rest),
        "changes" => cmd_changes(repo, &args, rest),
        "branch" | "br" => cmd_branch(repo, &args, rest),
        "rollback" | "rb" => cmd_rollback(repo, &args, rest),
        "undo" => cmd_undo(repo, &args, rest),
        "redo" => cmd_redo(repo, &args, rest),
        "checkpoint" | "cp" => cmd_checkpoint(repo, &args, rest),
        "diff" => cmd_diff(repo, &args, rest),
        "export" => cmd_export(repo, &args, rest),
        other => {
            eprintln!(
                "{}",
                err(&format!("✗ Unknown command: {other}. Type `help` for list."))
            );
            Ok(())
        }
    };
    if let Err(e) = res {
        eprintln!("{}", err(&format!("✗ {e}")));
    }
    Action::Continue
}

fn print_help() {
    println!();
    println!("{}", head("Route — commands"));
    println!();
    let rows: &[(&str, &str, &str)] = &[
        ("help, ?", "", "Show this help"),
        ("status, st", "", "Repository status"),
        ("log, lg", "[limit]", "Commit history (table)"),
        ("commit, ci", "<message>", "Commit working directory"),
        ("changes", "", "Pending changes"),
        ("branch, br", "[list|create|switch|merge]", "Branch operations"),
        ("rollback, rb", "<snapshot>", "Roll back to a snapshot"),
        ("undo", "", "Undo last commit"),
        ("redo", "", "Redo last undone commit"),
        ("checkpoint, cp", "<title>", "Create a checkpoint"),
        ("diff", "<from> <to>", "Diff two snapshots"),
        ("export", "<json|markdown|mermaid>", "Export repository"),
        ("git", "<sub>", "Git-mode commands (type `git help`)"),
        ("root", "git <net-sub>", "High-privilege git (push/pull/fetch/remote)"),
        ("clear, cls", "", "Clear screen"),
        ("exit, quit, q", "", "Quit route-tui"),
    ];
    for (cmd, args, desc) in rows {
        println!("  {} {}  {}", accent(cmd), faint(args), dim(desc));
    }
    println!();
}

/// `git` namespace help. Lists the local git-wrapping subcommands (default /
/// protected mode) plus a pointer to `root` for the network operations.
fn print_git_help() {
    println!();
    println!("{}", head("Route — git mode (protected)"));
    println!(
        "  {}",
        dim("drive your own `git` · checkpoints become real commits")
    );
    println!(
        "  {}",
        faint("network ops (push/pull/fetch/remote) need the `root` prefix.")
    );
    println!();
    let rows: &[(&str, &str, &str)] = &[
        ("git help", "", "Show this help"),
        ("git detect", "", "Check if `git` is available"),
        ("git init", "", "Initialize a git repo here (idempotent)"),
        ("git add", "[path...]", "Stage paths (or all if none given)"),
        ("git status, st", "", "Working-tree status + branch"),
        ("git changes", "", "Pending changes (porcelain)"),
        ("git diff, df", "[staged|<a> <b>]", "Diff (unstaged / staged / two refs)"),
        ("git log, lg", "[limit] [branch]", "Commit history"),
        ("git commit, ci", "<message>", "Stage all + commit (checkpoint)"),
        ("git branch, br", "[list|create|switch]", "Branch operations"),
        ("git merge", "<source>", "Merge a branch into HEAD"),
        ("git stash", "[push|pop|list|drop]", "Stash operations"),
        ("git tag", "[list|create|delete]", "Tag operations"),
        ("git reset", "[--soft|--mixed|--hard] <ref>", "Reset HEAD"),
        ("git revert", "<commit>", "Revert a commit"),
        ("git restore", "<path>...", "Restore working-tree files"),
        ("git config", "[get|set] <key> [val]", "Read/write local config"),
    ];
    for (cmd, args, desc) in rows {
        println!("  {} {}  {}", accent(cmd), faint(args), dim(desc));
    }
    println!();
    println!(
        "  {} {}",
        dim("Network ops:"),
        accent("root git push|pull|fetch|remote ...")
    );
    println!(
        "  {}",
        faint("type `root` or `root help` for the high-privilege model.")
    );
    println!();
}

/// `root` (high-privilege) help. Explains the two-tier permission model and
/// the network subcommands unlocked by the `root` prefix.
fn print_root_help() {
    println!();
    println!("{}", head("Route — root (high-privilege) mode"));
    println!();
    println!(
        "  {}",
        dim("Two-tier permission model:")
    );
    println!(
        "  {} {}",
        accent("default"),
        faint("`git <cmd>` — local ops only. Route never touches the")
    );
    println!(
        "  {}   {}",
        faint(""),
        faint("network. Maximum protection — push/pull/fetch are yours.")
    );
    println!(
        "  {}   {}",
        accent("root"),
        faint("`root git <cmd>` — Route runs network ops itself, as")
    );
    println!(
        "  {}   {}",
        faint(""),
        faint("software + AI collaboration. Warns before each run.")
    );
    println!();
    println!("  {}", dim("Unlocked by `root`:"));
    let rows: &[(&str, &str, &str)] = &[
        ("root git fetch", "[remote]", "Fetch from a remote (default origin)"),
        ("root git pull", "[remote] [branch]", "Pull from a remote"),
        ("root git push", "[remote] [branch] [--force]", "Push to a remote"),
        ("root git remote", "[list|add|remove|set-url]", "Manage remotes"),
    ];
    for (cmd, args, desc) in rows {
        println!("  {} {}  {}", accent(cmd), faint(args), dim(desc));
    }
    println!();
    println!(
        "  {}",
        faint("Note: GIT_TERMINAL_PROMPT stays off — configure a credential")
    );
    println!(
        "  {}",
        faint("helper or SSH key yourself, or Route's push/pull will fail cleanly.")
    );
    println!();
}

// --- helpers ---

fn resolve_prefix(repo: &BasicRepository, prefix: &str) -> Result<String> {
    let snapshots = repo.all_snapshots()?;
    let matches: Vec<_> = snapshots
        .iter()
        .filter(|s| s.snapshot.id.starts_with(prefix))
        .collect();
    match matches.len() {
        0 => Err(anyhow!("no snapshot matches prefix '{}'", prefix)),
        1 => Ok(matches[0].snapshot.id.clone()),
        _ => Err(anyhow!(
            "ambiguous prefix '{}': {} candidates",
            prefix,
            matches.len()
        )),
    }
}

/// Strip surrounding double quotes from a message argument.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// --- commands ---

fn cmd_status(repo: &BasicRepository, _args: &[String], _rest: &str) -> Result<()> {
    let branches = repo.list_branches()?;
    let current = repo.get_current_branch_name()?;
    let history = repo.history(1)?;

    println!("  {}    {}", dim("Project"), repo.project_path().display());
    println!("  {}       {}", dim("Mode"), repo.config.mode);
    println!(
        "  {}     {} {}",
        dim("Branch"),
        accent(&current),
        dim("(current)")
    );
    println!("  {}   {}", dim("Branches"), branches.len());
    if let Some(latest) = history.first() {
        println!(
            "  {}      [{}] {}",
            dim("Latest"),
            id(&short_id(&latest.snapshot.id)),
            dim(&format_ts(latest.snapshot.created_at))
        );
    }
    println!();
    println!("{}", head("  Branches"));
    for b in &branches {
        let marker = if b.name == current { accent("*") } else { dim(" ") };
        let h = b
            .head_snapshot
            .as_deref()
            .map(|s| id(&short_id(s)))
            .unwrap_or_else(|| faint("(empty)"));
        println!(
            "  {} {} {} head={}",
            marker,
            b.name,
            dim(&format!("({})", b.kind.as_str())),
            h
        );
    }
    Ok(())
}

fn cmd_log(repo: &BasicRepository, args: &[String], _rest: &str) -> Result<()> {
    let limit = args
        .first()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20);
    let commits = repo.list_commits(None, limit)?;
    if commits.is_empty() {
        println!("{}", dim("(no commits yet)"));
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            head("ID"),
            head("Time"),
            head("Branch"),
            head("Kind"),
            head("Message"),
        ]);
    for c in &commits {
        let branch_name = repo
            .get_branch_by_id(&c.branch_id)
            .map(|b| b.name)
            .unwrap_or_else(|_| "?".to_string());
        table.add_row(vec![
            id(&short_id(&c.id)),
            dim(&format_ts(c.created_at)),
            accent(&branch_name),
            dim(c.kind.as_str()),
            c.message.clone(),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn cmd_commit(repo: &BasicRepository, _args: &[String], rest: &str) -> Result<()> {
    let message = strip_quotes(rest);
    if message.trim().is_empty() {
        return Err(anyhow!("usage: commit <message>"));
    }
    let c = repo.commit(CommitOptions {
        message: message.clone(),
        author: None,
        force_full: false,
        branch: None,
        operator: Some("user".to_string()),
        body: None,
        is_checkpoint: false,
        is_ai: false,
    })?;
    println!("{} {}", ok("✓ Commit"), id(&short_id(&c.id)));
    println!("  {} {}", dim("Kind:"), c.kind.as_str());
    println!(
        "  {} {} → {}",
        dim("Snapshot:"),
        id(&short_id(&c.from_snapshot)),
        id(&short_id(&c.to_snapshot))
    );
    Ok(())
}

fn cmd_changes(repo: &BasicRepository, _args: &[String], _rest: &str) -> Result<()> {
    let changes = repo.working_dir_status()?;
    if changes.is_empty() {
        println!("{}", ok("✓ Working directory clean — nothing to commit."));
        return Ok(());
    }
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    for w in &changes {
        match w.change.as_str() {
            "added" => added.push(&w.path),
            "modified" => modified.push(&w.path),
            "removed" => removed.push(&w.path),
            _ => {}
        }
    }
    if !added.is_empty() {
        println!("{} (+{})", ok("added"), added.len());
        for p in &added {
            println!("  {}", p);
        }
    }
    if !modified.is_empty() {
        println!("{} (~{})", warn("modified"), modified.len());
        for p in &modified {
            println!("  {}", p);
        }
    }
    if !removed.is_empty() {
        println!("{} (-{})", err("removed"), removed.len());
        for p in &removed {
            println!("  {}", p);
        }
    }
    Ok(())
}

fn cmd_branch(repo: &BasicRepository, args: &[String], _rest: &str) -> Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "list" | "ls" => {
            let branches = repo.list_branches()?;
            let current = repo.get_current_branch_name()?;
            for b in &branches {
                let marker = if b.name == current { accent("*") } else { dim(" ") };
                let h = b
                    .head_snapshot
                    .as_deref()
                    .map(|s| id(&short_id(s)))
                    .unwrap_or_else(|| faint("(empty)"));
                println!(
                    "  {} {} {} head={}",
                    marker,
                    b.name,
                    dim(&format!("({})", b.kind.as_str())),
                    h
                );
            }
            Ok(())
        }
        "create" | "new" => {
            let name = args.get(1).ok_or_else(|| {
                anyhow!("usage: branch create <name> [--kind <main|inherited|sandbox>] [--from <branch>]")
            })?;
            let mut kind = BranchKind::Inherited;
            let mut from: Option<String> = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--kind" | "-k" => {
                        if let Some(k) = args.get(i + 1) {
                            kind = match k.as_str() {
                                "main" => BranchKind::Main,
                                "sandbox" => BranchKind::Sandbox,
                                _ => BranchKind::Inherited,
                            };
                            i += 2;
                            continue;
                        }
                    }
                    "--from" | "-f" => {
                        if let Some(f) = args.get(i + 1) {
                            from = Some(f.clone());
                            i += 2;
                            continue;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            let current = repo.get_current_branch_name()?;
            let b = repo.create_branch(
                name,
                CreateBranchOptions {
                    kind,
                    from_branch: from,
                },
                &current,
            )?;
            println!(
                "{} {} {}",
                ok("✓ Branch"),
                accent(&b.name),
                dim(&format!("({})", b.kind.as_str()))
            );
            Ok(())
        }
        "switch" | "sw" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: branch switch <name>"))?;
            repo.set_current_branch(name)?;
            println!("{} {}", ok("✓ Switched to"), accent(name));
            Ok(())
        }
        "merge" => {
            let source = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: branch merge <source>"))?;
            let c = repo.merge(source, None)?;
            println!(
                "{} {} → {}",
                ok("✓ Merge"),
                id(&short_id(&c.id)),
                dim(&c.message)
            );
            Ok(())
        }
        other => Err(anyhow!("unknown branch subcommand: {other}")),
    }
}

fn cmd_rollback(repo: &BasicRepository, args: &[String], _rest: &str) -> Result<()> {
    let prefix = args
        .first()
        .ok_or_else(|| anyhow!("usage: rollback <snapshot_id> [reason]"))?;
    let reason = if args.len() > 1 {
        Some(args[1..].join(" "))
    } else {
        None
    };
    let resolved = resolve_prefix(repo, prefix)?;
    let c = repo.rollback_to(&resolved, reason.as_deref())?;
    println!(
        "{} {} {}",
        ok("✓ Rolled back to"),
        id(&short_id(&c.to_snapshot)),
        dim(&format!("via commit [{}]", short_id(&c.id)))
    );
    Ok(())
}

fn cmd_undo(repo: &BasicRepository, _args: &[String], _rest: &str) -> Result<()> {
    let c = repo.undo_last()?;
    println!("{} {}", ok("✓ Undone"), id(&short_id(&c.id)));
    Ok(())
}

fn cmd_redo(repo: &BasicRepository, _args: &[String], _rest: &str) -> Result<()> {
    let c = repo.redo_last()?;
    println!("{} {}", ok("✓ Redone"), id(&short_id(&c.id)));
    Ok(())
}

fn cmd_checkpoint(repo: &BasicRepository, _args: &[String], rest: &str) -> Result<()> {
    let title = strip_quotes(rest);
    if title.trim().is_empty() {
        return Err(anyhow!("usage: checkpoint <title>"));
    }
    let c = repo.checkpoint_create(&title, None, None)?;
    println!(
        "{} {} {}",
        ok("✓ Checkpoint"),
        id(&short_id(&c.id)),
        dim(&title)
    );
    Ok(())
}

fn cmd_diff(repo: &BasicRepository, args: &[String], _rest: &str) -> Result<()> {
    let from = args
        .first()
        .ok_or_else(|| anyhow!("usage: diff <from> <to>"))?;
    let to = args
        .get(1)
        .ok_or_else(|| anyhow!("usage: diff <from> <to>"))?;
    let from_id = resolve_prefix(repo, from)?;
    let to_id = resolve_prefix(repo, to)?;
    let diff = repo.diff_snapshots(&from_id, &to_id)?;

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    for e in &diff {
        match e.change.as_str() {
            "added" => added.push(&e.path),
            "modified" => modified.push(&e.path),
            "removed" => removed.push(&e.path),
            _ => {}
        }
    }
    println!(
        "{} {} → {}",
        dim("diff"),
        id(&short_id(&from_id)),
        id(&short_id(&to_id))
    );
    if !added.is_empty() {
        println!("{} (+{})", ok("added"), added.len());
        for p in &added {
            println!("  {}", p);
        }
    }
    if !modified.is_empty() {
        println!("{} (~{})", warn("modified"), modified.len());
        for p in &modified {
            println!("  {}", p);
        }
    }
    if !removed.is_empty() {
        println!("{} (-{})", err("removed"), removed.len());
        for p in &removed {
            println!("  {}", p);
        }
    }
    if added.is_empty() && modified.is_empty() && removed.is_empty() {
        println!("{}", dim("(no differences)"));
    }
    Ok(())
}

fn cmd_export(repo: &BasicRepository, args: &[String], _rest: &str) -> Result<()> {
    let format = args
        .first()
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow!("usage: export <json|markdown|mermaid|emacs>"))?;
    let fmt = match format {
        "json" => ExportFormat::Json,
        "markdown" | "md" => ExportFormat::Markdown,
        "mermaid" => ExportFormat::Mermaid,
        "emacs" => ExportFormat::EmacsOrg,
        other => return Err(anyhow!("unsupported format: {other}")),
    };
    let ctx = repo.build_export_context()?;
    let exporter = route_basic::DefaultExporters::for_format(fmt)
        .ok_or_else(|| anyhow!("no exporter for format: {format}"))?;
    let mut buf: Vec<u8> = Vec::new();
    exporter.export(&ctx, &mut buf)?;
    let text = String::from_utf8(buf)?;
    println!("{text}");
    Ok(())
}

// --- git mode ---
//
// The `git` namespace wraps the user's own `git` binary so checkpoints
// become real commits and branches become real git branches. It mirrors
// `crates/route-tauri/src/git_commands.rs` but takes the project path
// directly (no Tauri state). Local ops run by default; network ops
// (push/pull/fetch/remote) require the `root` prefix (high-privilege).

fn cmd_git(
    project_path: &Path,
    elevated: bool,
    sub: &str,
    args: &[String],
    rest: &str,
) -> Result<()> {
    match sub {
        "" | "help" | "?" => {
            print_git_help();
            Ok(())
        }
        "detect" | "version" => cmd_git_detect(),
        "init" => cmd_git_init(project_path),
        "add" => cmd_git_add(project_path, args),
        "status" | "st" => cmd_git_status(project_path),
        "changes" => cmd_git_changes(project_path),
        "diff" | "df" => cmd_git_diff(project_path, args),
        "log" | "lg" => cmd_git_log(project_path, args),
        "commit" | "ci" | "checkpoint" | "cp" => cmd_git_commit(project_path, rest),
        "branch" | "br" => cmd_git_branch(project_path, args),
        "merge" => cmd_git_merge(project_path, args),
        "stash" => cmd_git_stash(project_path, args),
        "tag" => cmd_git_tag(project_path, args),
        "reset" => cmd_git_reset(project_path, args),
        "revert" => cmd_git_revert(project_path, args),
        "restore" => cmd_git_restore(project_path, args),
        "config" => cmd_git_config(project_path, args),
        // --- network / remote ops: gated on `root` (elevated) ---
        "fetch" | "pull" | "push" | "remote" | "clone" => {
            if !elevated {
                return Err(anyhow!(
                    "`{sub}` is a network operation — Route refuses it in protected mode.\n  \
                     Run `git {sub}` yourself in your terminal, or prepend `root` \
                     (`root git {sub}`) to let Route do it in high-privilege mode."
                ));
            }
            // High-privilege: warn, then execute. The user opted in by
            // typing `root`, so we proceed after a one-line reminder.
            println!(
                "{}",
                warn("⚡ high-privilege: Route is running this on your behalf.")
            );
            println!(
                "  {}",
                faint("verify your remote with `git remote -v` first if unsure.")
            );
            match sub {
                "fetch" => cmd_git_fetch(project_path, args),
                "pull" => cmd_git_pull(project_path, args),
                "push" => cmd_git_push(project_path, args),
                "remote" => cmd_git_remote(project_path, args),
                "clone" => Err(anyhow!(
                    "clone is not supported — `git init` then `root git remote add` instead"
                )),
                _ => unreachable!(),
            }
        }
        other => Err(anyhow!("unknown git subcommand: {other}. Try `git help`.")),
    }
}

fn cmd_git_detect() -> Result<()> {
    let d = git::detect();
    if d.available {
        println!("{} {}", ok("✓"), d.version);
        println!("  {}", dim("git mode available — checkpoints become real commits"));
    } else {
        println!("{} {}", err("✗ git not available"), dim(&d.error));
        println!(
            "  {}",
            faint("install git, or use route's built-in engine (status/log/commit)")
        );
    }
    Ok(())
}

fn cmd_git_init(project_path: &Path) -> Result<()> {
    let path = git::init(project_path)?;
    println!("{} {}", ok("✓ Initialized git repository"), dim("at"));
    println!("  {}", path);
    Ok(())
}

fn cmd_git_status(project_path: &Path) -> Result<()> {
    let branch = git::current_branch(project_path)?;
    let st = git::status(project_path)?;
    let branch_label = if branch.is_empty() {
        faint("(unborn — no commits yet)")
    } else {
        accent(&branch)
    };
    println!("  {} {}", dim("Branch:"), branch_label);
    if st.is_empty() {
        println!("{}", ok("✓ Working tree clean — nothing to commit."));
        return Ok(());
    }
    let total = st.added.len() + st.modified.len() + st.removed.len() + st.untracked.len();
    println!(
        "  {} {}",
        dim("Changes:"),
        id(&total.to_string())
    );
    if !st.added.is_empty() {
        println!("  {} (+{})", ok("added"), st.added.len());
        for p in &st.added {
            println!("    {}", p);
        }
    }
    if !st.modified.is_empty() {
        println!("  {} (~{})", warn("modified"), st.modified.len());
        for p in &st.modified {
            println!("    {}", p);
        }
    }
    if !st.removed.is_empty() {
        println!("  {} (-{})", err("removed"), st.removed.len());
        for p in &st.removed {
            println!("    {}", p);
        }
    }
    if !st.untracked.is_empty() {
        println!("  {} (?{})", faint("untracked"), st.untracked.len());
        for p in &st.untracked {
            println!("    {}", p);
        }
    }
    Ok(())
}

fn cmd_git_changes(project_path: &Path) -> Result<()> {
    let st = git::status(project_path)?;
    if st.is_empty() {
        println!("{}", ok("✓ Working tree clean — nothing to commit."));
        return Ok(());
    }
    if !st.added.is_empty() {
        println!("{} (+{})", ok("added"), st.added.len());
        for p in &st.added {
            println!("  {}", p);
        }
    }
    if !st.modified.is_empty() {
        println!("{} (~{})", warn("modified"), st.modified.len());
        for p in &st.modified {
            println!("  {}", p);
        }
    }
    if !st.removed.is_empty() {
        println!("{} (-{})", err("removed"), st.removed.len());
        for p in &st.removed {
            println!("  {}", p);
        }
    }
    if !st.untracked.is_empty() {
        println!("{} (?{})", faint("untracked"), st.untracked.len());
        for p in &st.untracked {
            println!("  {}", p);
        }
    }
    Ok(())
}

fn cmd_git_log(project_path: &Path, args: &[String]) -> Result<()> {
    // `git log [limit] [branch]` — first numeric token is the limit; a
    // non-numeric token (when no limit was given) is treated as a branch.
    let mut limit: Option<usize> = None;
    let mut branch: Option<&str> = None;
    for a in args {
        if limit.is_none() {
            if let Ok(n) = a.parse::<usize>() {
                limit = Some(n);
                continue;
            }
        }
        if branch.is_none() {
            branch = Some(a.as_str());
        }
    }
    let entries = git::log(project_path, limit, branch)?;
    if entries.is_empty() {
        println!("{}", dim("(no commits yet — run `git commit <message>`)"));
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            head("SHA"),
            head("Time"),
            head("Flags"),
            head("Message"),
        ]);
    for e in &entries {
        let mut flags = String::new();
        if e.is_checkpoint {
            flags.push_str(&accent("●"));
        }
        if e.is_ai {
            if !flags.is_empty() {
                flags.push(' ');
            }
            flags.push_str(&dim("ai"));
        }
        if flags.is_empty() {
            flags.push_str(&faint("·"));
        }
        table.add_row(vec![
            id(&e.short_sha),
            dim(&git::format_ts(e.timestamp_ms)),
            flags,
            e.message.clone(),
        ]);
    }
    println!("{table}");
    println!(
        "  {}",
        faint("● = Route checkpoint · ai = AI-driven commit")
    );
    Ok(())
}

fn cmd_git_commit(project_path: &Path, rest: &str) -> Result<()> {
    let title = strip_quotes(rest);
    if title.trim().is_empty() {
        return Err(anyhow!("usage: git commit <message>"));
    }
    let entry = git::commit(project_path, &title, None, None)?;
    println!("{} {}", ok("✓ Commit"), id(&entry.short_sha));
    println!("  {} {}", dim("Message:"), entry.message);
    if entry.is_checkpoint {
        println!("  {} {}", dim("Flag:"), accent("checkpoint"));
    }
    Ok(())
}

fn cmd_git_branch(project_path: &Path, args: &[String]) -> Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "list" | "ls" => {
            let branches = git::branch_list(project_path)?;
            if branches.is_empty() {
                println!("{}", dim("(no branches — run `git init` then `git commit`)"));
                return Ok(());
            }
            for b in &branches {
                let marker = if b.current { accent("*") } else { dim(" ") };
                let h = if b.head.is_empty() {
                    faint("(empty)")
                } else {
                    id(&b.head)
                };
                println!("  {} {} {}", marker, b.name, h);
            }
            Ok(())
        }
        "create" | "new" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git branch create <name>"))?;
            let b = git::branch_create(project_path, name)?;
            println!(
                "{} {} {}",
                ok("✓ Branch"),
                accent(&b.name),
                dim(&format!("head={}", b.head))
            );
            println!(
                "  {}",
                faint("switch with: git branch switch <name>")
            );
            Ok(())
        }
        "switch" | "sw" | "checkout" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git branch switch <name>"))?;
            git::branch_switch(project_path, name)?;
            println!("{} {}", ok("✓ Switched to"), accent(name));
            Ok(())
        }
        other => Err(anyhow!("unknown git branch subcommand: {other}")),
    }
}

fn cmd_git_merge(project_path: &Path, args: &[String]) -> Result<()> {
    let source = args
        .first()
        .ok_or_else(|| anyhow!("usage: git merge <source-branch>"))?;
    let head = git::merge(project_path, source)?;
    println!(
        "{} {} {}",
        ok("✓ Merged"),
        accent(source),
        dim(&format!("→ HEAD {}", short_id(&head)))
    );
    Ok(())
}

// --- git: local commands ---

fn cmd_git_add(project_path: &Path, args: &[String]) -> Result<()> {
    // `git add` with paths, or `git add -A` when no paths given.
    git::add(project_path, args)?;
    if args.is_empty() {
        println!("{} {}", ok("✓ Staged"), dim("all changes (git add -A)"));
    } else {
        println!(
            "{} {} {}",
            ok("✓ Staged"),
            id(&args.len().to_string()),
            dim("path(s)")
        );
    }
    Ok(())
}

fn cmd_git_diff(project_path: &Path, args: &[String]) -> Result<()> {
    // `git diff`               — unstaged (working tree vs index)
    // `git diff staged|cached` — staged (index vs HEAD)
    // `git diff <a> <b>`       — between two refs
    let text = match args.first().map(|s| s.as_str()) {
        None => git::diff_workdir(project_path)?,
        Some("staged") | Some("cached") | Some("--staged") | Some("--cached") => {
            git::diff_staged(project_path)?
        }
        Some(a) => {
            let b = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git diff <a> <b>  (or: git diff [staged])"))?;
            git::diff_commits(project_path, a, b)?
        }
    };
    if text.trim().is_empty() {
        println!("{}", dim("(no differences)"));
    } else {
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn cmd_git_stash(project_path: &Path, args: &[String]) -> Result<()> {
    let action = args.first().map(|s| s.as_str()).unwrap_or("push");
    match action {
        "push" | "save" => {
            // `git stash push [-m <message>]` — any tokens after the action
            // are treated as the stash message.
            let mut sub_args: Vec<String> = Vec::new();
            if args.len() > 1 {
                sub_args.push("-m".into());
                sub_args.push(args[1..].join(" "));
            }
            let out = git::stash(project_path, "push", &sub_args)?;
            println!("{}", out.trim());
            Ok(())
        }
        "pop" | "apply" => {
            let out = git::stash(project_path, action, &args[1..])?;
            println!("{}", out.trim());
            Ok(())
        }
        "list" | "ls" => {
            let out = git::stash(project_path, "list", &[])?;
            if out.trim().is_empty() {
                println!("{}", dim("(no stashes)"));
            } else {
                print!("{out}");
            }
            Ok(())
        }
        "drop" | "clear" => {
            let out = git::stash(project_path, action, &args[1..])?;
            println!("{}", out.trim());
            Ok(())
        }
        other => Err(anyhow!("unknown stash action: {other} (push/pop/list/drop)")),
    }
}

fn cmd_git_tag(project_path: &Path, args: &[String]) -> Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "list" | "ls" => {
            let tags = git::tag_list(project_path)?;
            if tags.is_empty() {
                println!("{}", dim("(no tags)"));
            } else {
                for t in &tags {
                    println!("  {}", accent(t));
                }
            }
            Ok(())
        }
        "create" | "new" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git tag create <name> [-m <message>]"))?;
            let message = if args.len() > 3 && (args[2] == "-m" || args[2] == "--message") {
                Some(args[3].as_str())
            } else {
                None
            };
            git::tag_create(project_path, name, message)?;
            println!("{} {}", ok("✓ Tag"), accent(name));
            Ok(())
        }
        "delete" | "rm" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git tag delete <name>"))?;
            git::tag_delete(project_path, name)?;
            println!("{} {}", ok("✓ Deleted tag"), accent(name));
            Ok(())
        }
        other => Err(anyhow!("unknown tag subcommand: {other} (list/create/delete)")),
    }
}

fn cmd_git_reset(project_path: &Path, args: &[String]) -> Result<()> {
    // `git reset [--soft|--mixed|--hard] <commit>`
    let mut mode = git::ResetMode::Mixed;
    let mut commit: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--soft" => mode = git::ResetMode::Soft,
            "--mixed" => mode = git::ResetMode::Mixed,
            "--hard" => {
                mode = git::ResetMode::Hard;
                // Hard reset is destructive — confirm unless the user already
                // typed it explicitly (they did, via the flag). Still warn.
                println!("{}", warn("⚠ --hard discards working-tree changes."));
            }
            other => commit = Some(other),
        }
        i += 1;
    }
    let commit = commit.ok_or_else(|| anyhow!("usage: git reset [--soft|--mixed|--hard] <commit>"))?;
    git::reset(project_path, mode, commit)?;
    let mode_str = match mode {
        git::ResetMode::Soft => "soft",
        git::ResetMode::Mixed => "mixed",
        git::ResetMode::Hard => "hard",
    };
    println!(
        "{} {} {}",
        ok("✓ Reset"),
        dim(mode_str),
        id(&short_id(commit))
    );
    Ok(())
}

fn cmd_git_revert(project_path: &Path, args: &[String]) -> Result<()> {
    let commit = args
        .first()
        .ok_or_else(|| anyhow!("usage: git revert <commit>"))?;
    git::revert(project_path, commit)?;
    println!("{} {}", ok("✓ Reverted"), id(&short_id(commit)));
    Ok(())
}

fn cmd_git_restore(project_path: &Path, args: &[String]) -> Result<()> {
    if args.is_empty() {
        return Err(anyhow!("usage: git restore <path>..."));
    }
    git::restore(project_path, args)?;
    println!(
        "{} {} {}",
        ok("✓ Restored"),
        id(&args.len().to_string()),
        dim("path(s)")
    );
    Ok(())
}

fn cmd_git_config(project_path: &Path, args: &[String]) -> Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("get");
    match sub {
        "get" => {
            let key = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git config get <key>"))?;
            let val = git::config_get(project_path, key)?;
            if val.is_empty() {
                println!("{}", dim("(unset)"));
            } else {
                println!("  {} {}", dim(&format!("{key}:")), accent(&val));
            }
            Ok(())
        }
        "set" => {
            let key = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git config set <key> <value>"))?;
            let value = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: git config set <key> <value>"))?;
            git::config_set(project_path, key, value)?;
            println!("{} {}={}", ok("✓ Set"), dim(key), accent(value));
            Ok(())
        }
        other => Err(anyhow!("unknown config subcommand: {other} (get/set)")),
    }
}

// --- git: network commands (only reached via `root`, elevated=true) ---

fn cmd_git_fetch(project_path: &Path, args: &[String]) -> Result<()> {
    let remote = args.first().map(|s| s.as_str());
    let out = git::fetch(project_path, remote)?;
    if out.trim().is_empty() {
        println!("{}", ok("✓ Fetch complete (already up to date)"));
    } else {
        print!("{out}");
        if !out.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn cmd_git_pull(project_path: &Path, args: &[String]) -> Result<()> {
    let remote = args.first().map(|s| s.as_str());
    let branch = args.get(1).map(|s| s.as_str());
    let out = git::pull(project_path, remote, branch)?;
    if out.trim().is_empty() {
        println!("{}", ok("✓ Pull complete (already up to date)"));
    } else {
        print!("{out}");
        if !out.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn cmd_git_push(project_path: &Path, args: &[String]) -> Result<()> {
    // `root git push [remote] [branch] [--force]`
    let mut force = false;
    let mut positional: Vec<&str> = Vec::new();
    for a in args {
        if a == "--force" || a == "-f" {
            force = true;
        } else {
            positional.push(a.as_str());
        }
    }
    if force {
        println!("{}", warn("⚠ --force: this rewrites the remote ref."));
    }
    let remote = positional.first().copied();
    let branch = positional.get(1).copied();
    let out = git::push(project_path, remote, branch, force)?;
    if out.trim().is_empty() {
        println!("{}", ok("✓ Push complete"));
    } else {
        print!("{out}");
        if !out.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn cmd_git_remote(project_path: &Path, args: &[String]) -> Result<()> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("list");
    match sub {
        "list" | "ls" | "-v" => {
            let remotes = git::remote_list(project_path)?;
            if remotes.is_empty() {
                println!("{}", dim("(no remotes — add one: root git remote add <name> <url>)"));
            } else {
                for r in &remotes {
                    println!("  {} {}", accent(&r.name), dim(&r.url));
                }
            }
            Ok(())
        }
        "add" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git remote add <name> <url>"))?;
            let url = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: git remote add <name> <url>"))?;
            git::remote_add(project_path, name, url)?;
            println!("{} {} {}", ok("✓ Remote"), accent(name), dim(url));
            Ok(())
        }
        "remove" | "rm" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git remote remove <name>"))?;
            git::remote_remove(project_path, name)?;
            println!("{} {}", ok("✓ Removed remote"), accent(name));
            Ok(())
        }
        "set-url" => {
            let name = args
                .get(1)
                .ok_or_else(|| anyhow!("usage: git remote set-url <name> <url>"))?;
            let url = args
                .get(2)
                .ok_or_else(|| anyhow!("usage: git remote set-url <name> <url>"))?;
            git::remote_set_url(project_path, name, url)?;
            println!("{} {} {}", ok("✓ Set URL"), accent(name), dim(url));
            Ok(())
        }
        other => Err(anyhow!("unknown remote subcommand: {other} (list/add/remove/set-url)")),
    }
}
