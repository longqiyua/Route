//! route-tui — Route interactive REPL (Claude-Code-style).
//!
//! Entry point: parse `--path`, open the Route repository (if any),
//! print the banner, and hand off to the REPL loop. When the cwd is
//! not a Route repository the REPL still starts but only `help` /
//! `exit` / `clear` are usable.

mod ai;
mod banner;
mod commands;
mod fmt;
mod git;
mod repl;

use std::path::PathBuf;

use anyhow::Result;
use route_basic::BasicRepository;

fn main() -> Result<()> {
    fmt::init_colors();

    let project_path = parse_path_arg()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Try to open the repo; None if not a Route repository. We don't
    // treat this as fatal — the banner shows a hint and the REPL still
    // runs so the user can `help`/`exit`. Git mode (`git ...`) works even
    // without a route_basic repo, so we pass project_path through to the
    // REPL separately from `repo`.
    let repo = match BasicRepository::open(&project_path) {
        Ok(r) => Some(r),
        Err(_) => None,
    };

    banner::print_banner(repo.as_ref())?;
    repl::run(repo, project_path)?;
    Ok(())
}

/// Parse `--path <dir>` from argv. Returns `Err` if not given (caller
/// falls back to cwd). Also handles `-h` / `--help`.
fn parse_path_arg() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--path" {
            if let Some(p) = args.next() {
                return Ok(PathBuf::from(p));
            }
            anyhow::bail!("--path requires a value");
        } else if arg == "--help" || arg == "-h" {
            eprintln!("route-tui — Route interactive REPL");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  route-tui [--path <dir>]");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  --path <dir>  Route project directory (defaults to current dir).");
            eprintln!("  -h, --help    Show this help.");
            std::process::exit(0);
        }
    }
    anyhow::bail!("no --path given");
}
