//! route-tui — Route interactive REPL (Claude-Code-style).
//!
//! Binary entry point: parse `--path`, then hand off to the shared
//! library [`route_tui::run`][crate::run]. The same `run` is reused by the
//! main `route` CLI via the `route tui` subcommand.

use std::path::PathBuf;

use anyhow::Result;

fn main() -> Result<()> {
    let project_path = parse_path_arg()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    route_tui::run(project_path)
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
