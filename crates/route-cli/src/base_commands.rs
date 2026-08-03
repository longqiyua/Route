//! Root Base CLI commands.
//!
//! Integrates Root Base (顶层编排器) into the CLI,
//! providing `route base status`, `route base search <query>`,
//! `route base init`, `route base memory`, and `route base causal`.

use anyhow::{Context, Result};
use route_base::RootBase;
use std::path::Path;

fn open_base() -> Result<RootBase> {
    let cwd = std::env::current_dir()?;
    RootBase::new(&cwd).context("Failed to initialize Root Base")
}

/// Print the Root Base status summary.
pub fn base_status() -> Result<()> {
    let base = open_base()?;
    let status = base.status();

    println!("Root Base Status");
    println!("  Project:         {}", status.project);
    println!("  Memory Mode:     {}", if status.memory_mode { "ON" } else { "OFF" });
    println!("  Causal Control:  {}", if status.causal_control { "ON" } else { "OFF" });
    println!("  Auto Git:        {}", if status.auto_git { "ON" } else { "OFF" });
    println!("  Adaptive Debounce: {}", if status.adaptive_debounce { "ON" } else { "OFF" });
    println!("  GUI Desktop:     {}", if status.gui_enabled { "ENABLED" } else { "DISABLED (hidden)" });
    println!();
    println!("Memory:");
    println!("  Entries:         {}", status.memory_entries);
    println!("  Causal Chains:   {}", status.memory_chains);
    println!("  Hot Blocks:      {}", status.hot_blocks);
    println!("  Cold Blocks:     {}", status.cold_blocks);
    println!("  Estimated Bytes: {}", status.estimated_bytes);
    println!();
    println!("Services ({} registered):", status.services.len());
    for svc in &status.services {
        println!("  - {}", svc);
    }
    Ok(())
}

/// Search the project code using Root Base's three-mechanism engine.
pub fn base_search(query: &str, top_k: usize) -> Result<()> {
    let base = open_base()?;
    let results = base.search(query, top_k);

    if results.is_empty() {
        println!("(no results for '{}')", query);
        return Ok(());
    }

    println!("Search results for '{}' (top {}):", query, top_k);
    for (i, r) in results.iter().enumerate() {
        println!(
            "  {}. [{:.3}] {}",
            i + 1,
            r.score,
            r.text
        );
        if let Some(ref fp) = r.file_path {
            println!("     File: {} (line {})", fp, r.line);
        }
        println!("     Source: {}", r.source);
    }
    Ok(())
}

/// Initialize Root Base data (project memory, engine index).
pub fn base_init() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut base = RootBase::new(&cwd).context("Failed to initialize Root Base")?;
    base.init().context("Failed to initialize Root Base components")?;
    println!("✓ Root Base initialized at {}", cwd.display());
    println!("  Memory mode: {}", if base.config.memory_mode { "ON" } else { "OFF" });
    println!("  Causal control: {}", if base.config.causal_control { "ON" } else { "OFF" });
    Ok(())
}

/// Show Root Memory statistics.
pub fn base_memory() -> Result<()> {
    let base = open_base()?;
    let stats = base.memory.stats();
    let chains = base.memory.chain.links.len();

    println!("Root Memory Statistics");
    println!("  Total entries:  {}", stats.total_entries);
    println!("  Causal chains:  {}", chains);
    if let Some(ref oldest) = stats.oldest_entry {
        println!("  Oldest entry:   {}", oldest);
    }
    if let Some(ref newest) = stats.newest_entry {
        println!("  Newest entry:   {}", newest);
    }
    if !stats.by_kind.is_empty() {
        println!();
        println!("  By kind:");
        for (kind, count) in &stats.by_kind {
            println!("    {}: {}", kind, count);
        }
    }

    // Show project structure if available
    if let Some(mermaid) = base.project_structure() {
        println!();
        println!("Project Structure (Mermaid):");
        for line in mermaid.lines() {
            println!("  {}", line);
        }
    }

    // Show project meta if available
    if let Some(meta) = base.project_meta() {
        println!();
        println!("Project Meta:");
        for line in meta.lines() {
            println!("  {}", line);
        }
    }

    Ok(())
}

/// Show the causal chain.
pub fn base_causal(limit: usize) -> Result<()> {
    let base = open_base()?;
    let links = &base.memory.chain.links;

    if links.is_empty() {
        println!("(no causal links recorded)");
        return Ok(());
    }

    let count = links.len().min(limit);
    println!("Causal Chain (last {} of {}):", count, links.len());
    for link in links.iter().rev().take(count).rev() {
        println!();
        println!("  [{}] {}", link.id, link.action);
        println!("  Reason: {}", link.reason);
        println!("  Effect: {}", link.effect);
        if let Some(ref fp) = link.file_path {
            println!("  File: {}", fp);
        }
        println!("  Status: {:?}", link.status);
        println!("  Time: {}", link.timestamp);
        println!("  Actor: {}", link.actor);
    }
    Ok(())
}

// ─── Self-Referential Management ──────────────────────────────────────────

/// Index Route's own source code into Root Engine
pub fn self_manage_index() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut base = RootBase::new(&cwd).context("Failed to initialize Root Base")?;

    // Find all Rust source files in the project
    let mut files_parsed = 0usize;
    let mut symbols_found = 0usize;
    let src_dir = cwd.join("crates");

    if src_dir.exists() {
        visit_rust_files(&src_dir, &mut base, &mut files_parsed, &mut symbols_found)?;
    }

    // Also index the top-level src if it exists
    let top_src = cwd.join("src");
    if top_src.exists() {
        visit_rust_files(&top_src, &mut base, &mut files_parsed, &mut symbols_found)?;
    }

    // Auto-balance hot/cold index
    base.engine.hot_cold_index.auto_balance();

    println!("✓ Self-index complete: {} files parsed, {} symbols indexed", files_parsed, symbols_found);
    println!("  Total engine entries: {}", base.engine.count());
    println!("  Hot index: {} blocks, Cold index: {} blocks",
        base.engine.hot_cold_index.hot_count(),
        base.engine.hot_cold_index.cold_count());
    Ok(())
}

/// Show Route's own project structure in Mermaid format
pub fn self_manage_structure() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let _base = RootBase::new(&cwd).context("Failed to initialize Root Base")?;

    // Build a Mermaid graph from the crate structure
    let mut mermaid = String::from("graph TD\n");
    mermaid.push_str("    R[\"Route (Root Base)\"]\n");

    let crates_dir = cwd.join("crates");
    if crates_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&crates_dir) {
            let mut crate_names: Vec<String> = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() && name.starts_with("route-") {
                    let short = name.strip_prefix("route-").unwrap_or(&name);
                    crate_names.push(short.to_string());
                    let node_id = format!("C_{}", short.replace('-', "_"));
                    mermaid.push_str(&format!("    {}[\"{} (crate)\"]\n", node_id, name));
                    mermaid.push_str(&format!("    R --> {}\n", node_id));
                }
            }
            crate_names.sort();
        }
    }

    println!("Route Self Structure (Mermaid):");
    println!("```mermaid");
    for line in mermaid.lines() {
        println!("{}", line);
    }
    println!("```");
    Ok(())
}

/// Record a causal link for Route's own development
pub fn self_manage_record(action: &str, file: &str, reason: &str, effect: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut base = RootBase::new(&cwd).context("Failed to initialize Root Base")?;

    base.record_causal_link(action, file, reason, effect)?;
    println!("✓ Causal link recorded:");
    println!("  Action: {}", action);
    println!("  File:   {}", file);
    println!("  Reason: {}", reason);
    println!("  Effect: {}", effect);
    Ok(())
}

/// Show Route's own git history as a causal chain
pub fn self_manage_git_log(limit: usize) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Run git log to get Route's own commit history
    let output = std::process::Command::new("git")
        .args(["-C", &cwd.to_string_lossy(), "log", "--oneline", &format!("-{}", limit)])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git log: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Route Git History (last {}):", limit);
    for line in stdout.lines() {
        println!("  {}", line);
    }

    // Also show the causal chain from Root Memory
    let base = RootBase::new(&cwd)?;
    let links = &base.memory.chain.links;
    if !links.is_empty() {
        println!();
        println!("Causal Chain ({} entries):", links.len());
        for link in links.iter().rev().take(5).rev() {
            println!("  [{}] {} → {}", link.id, link.action, link.effect);
        }
    }
    Ok(())
}

/// Full introspection — scan all Route crates, update memory, index code
pub fn self_manage_introspect() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut base = RootBase::new(&cwd).context("Failed to initialize Root Base")?;

    println!("Route Self-Introspection");
    println!("========================");
    println!();

    println!("Project: {}", cwd.display());
    println!();

    // 2. Index the codebase
    println!("[1/3] Indexing source code...");
    let mut files_parsed = 0usize;
    let mut symbols_found = 0usize;

    let crates_dir = cwd.join("crates");
    if crates_dir.exists() {
        visit_rust_files(&crates_dir, &mut base, &mut files_parsed, &mut symbols_found)?;
    }
    let top_src = cwd.join("src");
    if top_src.exists() {
        visit_rust_files(&top_src, &mut base, &mut files_parsed, &mut symbols_found)?;
    }
    println!("  ✓ {} files parsed, {} symbols indexed", files_parsed, symbols_found);

    // 3. Auto-balance index
    println!("[2/3] Balancing hot/cold index...");
    base.engine.hot_cold_index.auto_balance();
    println!("  ✓ Hot: {} blocks, Cold: {} blocks",
        base.engine.hot_cold_index.hot_count(),
        base.engine.hot_cold_index.cold_count());

    // 4. Record introspection as a causal link
    println!("[3/3] Recording introspection event...");
    base.record_causal_link(
        "introspect",
        "crates/",
        "Self-introspection: full scan of Route codebase",
        &format!("Indexed {} files, {} symbols", files_parsed, symbols_found),
    )?;
    println!("  ✓ Causal link recorded");

    // 5. Show status
    println!();
    let status = base.status();
    println!("Status:");
    println!("  Memory entries: {}", status.memory_entries);
    println!("  Causal chains:  {}", status.memory_chains);
    println!("  Hot blocks:     {}", status.hot_blocks);
    println!("  Cold blocks:    {}", status.cold_blocks);
    println!("  Estimated:      {} bytes", status.estimated_bytes);
    println!();
    println!("✓ Self-introspection complete");

    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────

/// Recursively visit Rust source files and index them into Root Engine
fn visit_rust_files(
    dir: &Path,
    base: &mut RootBase,
    files_parsed: &mut usize,
    symbols_found: &mut usize,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir).map_err(|e| anyhow::anyhow!("Failed to read dir {:?}: {}", dir, e))? {
        let entry = entry.map_err(|e| anyhow::anyhow!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "target" || name == ".git" || name == "node_modules"
                || name == ".route" || name == "gen" || name == "dist"
            {
                continue;
            }
            visit_rust_files(&path, base, files_parsed, symbols_found)?;
        } else if path.extension().map_or(false, |ext| ext == "rs") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let rel_path = path.strip_prefix(&base.config.project_path)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                for (i, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("//") && !trimmed.starts_with("#") {
                        base.engine.add_line(trimmed, &rel_path, i + 1, "rust");
                        *symbols_found += 1;
                    }
                }
                *files_parsed += 1;
            }
        }
    }
    Ok(())
}

// ─── GUI Feature Toggle ─────────────────────────────────────────────────

#[cfg(feature = "route-base")]
/// Enable GUI desktop app.
pub fn gui_enable() -> Result<()> {
    let mut base = open_base()?;
    base.gui_enable()?;
    println!("✓ GUI desktop app enabled");
    println!("  To build the GUI: cd packages/desktop && npm install && npm run build && cargo tauri build");
    println!("  See explain.md in the repo root for details.");
    Ok(())
}

#[cfg(feature = "route-base")]
/// Disable GUI desktop app.
pub fn gui_disable() -> Result<()> {
    let mut base = open_base()?;
    base.gui_disable()?;
    println!("✓ GUI desktop app disabled");
    Ok(())
}

#[cfg(feature = "route-base")]
/// Show GUI status.
pub fn gui_status() -> Result<()> {
    let base = open_base()?;
    let enabled = base.gui_status();
    println!("GUI Desktop App: {}", if enabled { "ENABLED" } else { "DISABLED (hidden)" });
    println!();
    if enabled {
        println!("  The GUI is enabled. Build it with:");
        println!("    cd packages/desktop && npm install && npm run build && cargo tauri build");
    } else {
        println!("  The GUI is a hidden feature toggle. Enable it with:");
        println!("    route base gui enable");
        println!("  See explain.md for details.");
    }
    Ok(())
}