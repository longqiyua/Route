//! route-tui — Route interactive REPL (Claude-Code-style portal).
//!
//! This crate is both a standalone binary and a library. The library
//! surface exposes a single [`run`] entry point so the main `route` CLI
//! can launch the same interactive REPL via the `route tui` subcommand —
//! the default-hidden TUI that becomes reachable on demand.

pub mod ai;
pub mod banner;
pub mod commands;
pub mod fmt;
pub mod git;
pub mod repl;

use anyhow::Result;
use route_basic::BasicRepository;
use std::path::PathBuf;

/// Launch the interactive REPL for `project_path`.
///
/// When `project_path` is a valid Route repository the banner shows its
/// branch / record count; otherwise the REPL still starts so the user can
/// reach `help`, `git`, `clear` and `exit`. Returns when the user exits.
pub fn run(project_path: PathBuf) -> Result<()> {
    fmt::init_colors();

    // Try to open the repo; None if not a Route repository — not fatal.
    let repo = BasicRepository::open(&project_path).ok();

    banner::print_banner(repo.as_ref())?;
    repl::run(repo, project_path)?;
    Ok(())
}
