//! CLI git commands — let Route drive the user's own `git` binary.
//! Mirrors the Tauri backend's `git_commands.rs` for command-line use.

use std::process::Command;

use anyhow::{anyhow, Result};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn git_command() -> Command {
    let mut cmd = Command::new("git");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn run_git(args: &[&str]) -> Result<String> {
    let cwd = std::env::current_dir()?;
    let mut cmd = git_command();
    cmd.current_dir(&cwd).args(args);
    cmd.env("LC_ALL", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");
    let output = cmd.output().map_err(|e| anyhow!("failed to spawn git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() {
            anyhow!("git {} failed", args.join(" "))
        } else {
            anyhow!("git {}: {detail}", args.join(" "))
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn init() -> Result<()> {
    let dot_git = std::env::current_dir()?.join(".git");
    if dot_git.exists() {
        println!("✓ Already a git repository");
        return Ok(());
    }
    run_git(&["init", "--quiet"])?;
    // Bootstrap identity if none set.
    let name = run_git(&["config", "user.name"]).unwrap_or_default();
    if name.trim().is_empty() {
        let _ = run_git(&["config", "user.name", "Route"]);
    }
    let email = run_git(&["config", "user.email"]).unwrap_or_default();
    if email.trim().is_empty() {
        let _ = run_git(&["config", "user.email", "route@local"]);
    }
    println!("✓ Git repository initialized");
    Ok(())
}

pub fn status() -> Result<()> {
    let out = run_git(&["status", "--short", "--branch"])?;
    println!("{}", out);
    Ok(())
}

pub fn log(limit: usize, graph: bool, all: bool) -> Result<()> {
    let n = format!("-{}", limit.clamp(1, 500));
    let mut args = vec!["log", &n, "--oneline", "--decorate"];
    if graph {
        args.push("--graph");
    }
    if all {
        args.push("--all");
    }
    let out = run_git(&args)?;
    print!("{}", out);
    Ok(())
}

pub fn commit(message: String) -> Result<()> {
    run_git(&["add", "-A"])?;
    run_git(&["commit", "--quiet", "-m", &message])?;
    let sha = run_git(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
    println!("✓ [{sha}] {message}");
    Ok(())
}

pub fn branch_list() -> Result<()> {
    let out = run_git(&["branch", "--list", "--format=%(refname:short)"])?;
    let current = run_git(&["rev-parse", "--abbrev-ref", "HEAD"])?.trim().to_string();
    for line in out.lines() {
        let name = line.trim();
        if name.is_empty() { continue; }
        let marker = if name == current { "* " } else { "  " };
        println!("{marker}{name}");
    }
    Ok(())
}

pub fn branch_create(name: String) -> Result<()> {
    run_git(&["branch", &name])?;
    println!("✓ Branch '{}' created", name);
    Ok(())
}

pub fn branch_switch(name: String) -> Result<()> {
    match run_git(&["switch", &name]) {
        Ok(_) => {}
        Err(e) if e.to_string().contains("unknown switch") || e.to_string().contains("usage: git") => {
            run_git(&["checkout", &name])?;
        }
        Err(e) => return Err(e),
    }
    println!("✓ Switched to branch '{}'", name);
    Ok(())
}

pub fn branch_delete(name: String, force: bool) -> Result<()> {
    if force {
        run_git(&["branch", "-D", &name])?;
    } else {
        run_git(&["branch", "-d", &name])?;
    }
    println!("✓ Branch '{}' deleted", name);
    Ok(())
}

pub fn remote_list() -> Result<()> {
    let out = run_git(&["remote", "-v"])?;
    if out.trim().is_empty() {
        println!("(no remotes configured)");
    } else {
        print!("{}", out);
    }
    Ok(())
}

pub fn remote_add(name: String, url: String) -> Result<()> {
    run_git(&["remote", "add", &name, &url])?;
    println!("✓ Remote '{name}' added → {url}");
    Ok(())
}

pub fn remote_remove(name: String) -> Result<()> {
    run_git(&["remote", "remove", &name])?;
    println!("✓ Remote '{name}' removed");
    Ok(())
}

pub fn fetch(remote: Option<String>) -> Result<()> {
    let r = remote.as_deref().unwrap_or("origin");
    let out = run_git(&["fetch", "--prune", r])?;
    print!("{}", out);
    println!("✓ Fetch from '{r}' completed");
    Ok(())
}

pub fn pull(remote: Option<String>, branch: Option<String>) -> Result<()> {
    let r = remote.unwrap_or_else(|| "origin".to_string());
    let b = branch.unwrap_or_else(|| {
        run_git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    let out = run_git(&["pull", "--rebase", "--autostash", &r, &b])?;
    println!("{}", out);
    Ok(())
}

pub fn push(remote: Option<String>, branch: Option<String>, force: bool) -> Result<()> {
    let r = remote.unwrap_or_else(|| "origin".to_string());
    let b = branch.unwrap_or_else(|| {
        run_git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .unwrap_or_default()
            .trim()
            .to_string()
    });
    if force {
        let out = run_git(&["push", "--force-with-lease", &r, &b])?;
        println!("[FORCE PUSH] {}", out);
    } else {
        let out = run_git(&["push", &r, &b])?;
        println!("{}", out);
    }
    println!("✓ Push to '{r}/{b}' completed");
    Ok(())
}

pub fn diff(target: Option<String>) -> Result<()> {
    let args: &[&str] = match target.as_deref() {
        Some(t) => &["diff", t, "--no-color", "--no-ext-diff"],
        None => &["diff", "HEAD", "--no-color", "--no-ext-diff"],
    };
    let out = run_git(args)?;
    print!("{}", out);
    Ok(())
}

pub fn add(paths: Vec<String>) -> Result<()> {
    if paths.is_empty() {
        run_git(&["add", "-A"])?;
        println!("✓ All files staged");
    } else {
        let mut args = vec!["add"];
        for p in &paths { args.push(p.as_str()); }
        run_git(&args)?;
        println!("✓ Staged: {}", paths.join(", "));
    }
    Ok(())
}

pub fn reset(paths: Vec<String>) -> Result<()> {
    if paths.is_empty() {
        run_git(&["reset", "HEAD"])?;
        println!("✓ All files unstaged");
    } else {
        let mut args = vec!["reset", "HEAD", "--"];
        for p in &paths { args.push(p.as_str()); }
        run_git(&args)?;
        println!("✓ Unstaged: {}", paths.join(", "));
    }
    Ok(())
}

pub fn stash_push(message: Option<String>) -> Result<()> {
    match &message {
        Some(m) => run_git(&["stash", "push", "-m", m])?,
        None => run_git(&["stash", "push"])?,
    };
    println!("✓ Stashed");
    Ok(())
}

pub fn stash_pop() -> Result<()> {
    run_git(&["stash", "pop"])?;
    println!("✓ Stash popped");
    Ok(())
}

pub fn stash_list() -> Result<()> {
    let out = run_git(&["stash", "list"])?;
    if out.trim().is_empty() {
        println!("(no stashes)");
    } else {
        print!("{}", out);
    }
    Ok(())
}

pub fn tag_list() -> Result<()> {
    let out = run_git(&["tag", "--list"])?;
    if out.trim().is_empty() {
        println!("(no tags)");
    } else {
        print!("{}", out);
    }
    Ok(())
}

pub fn tag_create(name: String, message: Option<String>) -> Result<()> {
    match &message {
        Some(m) => run_git(&["tag", "-a", &name, "-m", m])?,
        None => run_git(&["tag", &name])?,
    };
    println!("✓ Tag '{}' created", name);
    Ok(())
}

pub fn tag_delete(name: String) -> Result<()> {
    run_git(&["tag", "-d", &name])?;
    println!("✓ Tag '{}' deleted", name);
    Ok(())
}

pub fn config_get(key: String) -> Result<()> {
    let out = run_git(&["config", "--get", &key])?;
    print!("{}", out);
    Ok(())
}

pub fn config_set(key: String, value: String, scope: Option<String>) -> Result<()> {
    let scope_flag = match scope.as_deref() {
        Some("global") => "--global",
        Some("system") => "--system",
        _ => "--local",
    };
    run_git(&[scope_flag, &key, &value])?;
    println!("✓ {key} = {value} ({})", scope_flag.trim_start_matches("--"));
    Ok(())
}

pub fn revert(sha: String) -> Result<()> {
    run_git(&["revert", "--no-edit", "--no-ff", &sha])?;
    let head = run_git(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
    println!("✓ Reverted {sha} → new commit [{head}]");
    Ok(())
}

pub fn cherry_pick(shas: Vec<String>, message: Option<String>) -> Result<()> {
    // Validate all commits exist first.
    for sha in &shas {
        run_git(&["cat-file", "-e", &format!("{sha}^{{commit}}")])
            .map_err(|_| anyhow!("commit not found: {sha}"))?;
    }
    // Use --no-commit to stage all changes, then commit.
    let mut args = vec!["cherry-pick", "--no-commit"];
    for sha in &shas { args.push(sha.as_str()); }
    match run_git(&args) {
        Ok(_) => {
            let msg = message.unwrap_or_else(|| format!("cherry-pick: {}", shas.join(", ")));
            run_git(&["commit", "--quiet", "-m", &msg])?;
            let head = run_git(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
            println!("✓ Cherry-pick → [{head}] {msg}");
            Ok(())
        }
        Err(e) => {
            let _ = run_git(&["cherry-pick", "--abort"]);
            Err(anyhow!("cherry-pick failed (aborted): {e}"))
        }
    }
}

pub fn rebase(target: String) -> Result<()> {
    match run_git(&["rebase", "--autostash", &target]) {
        Ok(_) => {
            let head = run_git(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
            println!("✓ Rebased onto '{target}' → [{head}]");
            Ok(())
        }
        Err(e) => {
            let _ = run_git(&["rebase", "--abort"]);
            Err(anyhow!("rebase failed (aborted): {e}"))
        }
    }
}

pub fn rebase_abort() -> Result<()> {
    run_git(&["rebase", "--abort"])?;
    println!("✓ Rebase aborted");
    Ok(())
}

pub fn rebase_continue() -> Result<()> {
    run_git(&["rebase", "--continue", "--no-edit"])?;
    let head = run_git(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
    println!("✓ Rebase continued → [{head}]");
    Ok(())
}

pub fn clean(dry_run: bool, directories: bool, force: bool) -> Result<()> {
    let mut args = vec!["clean"];
    if dry_run { args.push("--dry-run"); }
    if directories { args.push("-d"); }
    if force { args.push("-f"); }
    let out = run_git(&args)?;
    if out.trim().is_empty() {
        println!("(nothing to clean)");
    } else {
        print!("{}", out);
    }
    Ok(())
}

pub fn show(sha: String) -> Result<()> {
    let out = run_git(&["show", "--no-color", "--no-ext-diff", &sha])?;
    print!("{}", out);
    Ok(())
}

pub fn archive(output: String, format: Option<String>, treeish: Option<String>) -> Result<()> {
    let fmt = format.as_deref().unwrap_or("zip");
    let ref_name = treeish.as_deref().unwrap_or("HEAD");
    run_git(&["archive", &format!("--format={fmt}"), &format!("--output={output}"), ref_name])?;
    println!("✓ Archive created: {output}");
    Ok(())
}

pub fn clone(url: String, target: String) -> Result<()> {
    let target_path = std::path::Path::new(&target);
    if target_path.exists() {
        return Err(anyhow!("target path already exists: {target}"));
    }
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cmd = git_command();
    cmd.arg("clone");
    cmd.arg(&url);
    cmd.arg(&target);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");
    let output = cmd.output().map_err(|e| anyhow!("failed to spawn git clone: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(anyhow!("{}", if stderr.is_empty() { "git clone failed".to_string() } else { stderr }));
    }
    println!("✓ Cloned '{url}' → {target}");
    Ok(())
}

pub fn merge(source: String) -> Result<()> {
    run_git(&["merge", "--no-edit", "--no-ff", &source])?;
    let head = run_git(&["rev-parse", "--short", "HEAD"])?.trim().to_string();
    println!("✓ Merged '{source}' → [{head}]");
    Ok(())
}

pub fn backup() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let backup_dir = cwd.join(".route").join("git-backups");
    std::fs::create_dir_all(&backup_dir)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let archive_path = backup_dir.join(format!("pre-op-{ts}.tar"));
    run_git(&["archive", "--format=tar", &format!("--output={}", archive_path.to_string_lossy()), "HEAD"])?;
    let head = run_git(&["rev-parse", "HEAD"])?.trim().to_string();
    std::fs::write(backup_dir.join(format!("pre-op-{ts}.head")), &head)?;
    println!("✓ Backup created: {}", archive_path.display());
    Ok(())
}