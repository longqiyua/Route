//! Startup banner — Route glyph + project info.
//!
//! Rendered once on launch before the REPL prompt. The glyph is the Route
//! mark: a Reuleaux triangle (the outer structural shell) with a circular
//! hole at its center. That inner circle is the "conversation mediator" —
//! the channel between the user and the software / AI, the same idea the
//! desktop app's logo expresses.
//!
//! We render the glyph with `█` (U+2588 FULL BLOCK) rather than the
//! box-drawing characters used previously. Box-drawing glyphs (═╗╚║) render
//! inconsistently across Windows terminal fonts and previously showed up as
//! garbled "2oa"-style noise; full-block renders solidly everywhere.

use anyhow::Result;
use route_basic::BasicRepository;

use crate::fmt::{accent, brand, dim, faint, id, warn};

/// The Route glyph — Reuleaux triangle (apex up, bulging sides + bottom)
/// with a circular hole at the visual center. Computed geometrically so the
/// arcs are true to the SVG logo in `crates/route-tauri/web/src/Logo.tsx`.
const GLYPH: &str = r#"
            █
           ███
          █████
         ███████
        █████████
       █████ █████
      ████     ████
      ████     ████
      ███       ███
     █████     █████
     █████     █████
     ███████ ███████
      █████████████
         ███████
"#;

/// Print the startup banner. `repo` is `Some` when the cwd (or --path)
/// is a valid Route repository.
pub fn print_banner(repo: Option<&BasicRepository>) -> Result<()> {
    // Glyph — full-block characters, accent purple. Trim the leading
    // newline so the first row sits flush at the top.
    for line in GLYPH.trim_start_matches('\n').lines() {
        println!("{}", brand(line));
    }
    println!();

    // Wordmark — lowercase "route" in the brand color, matching the
    // desktop app's wordmark style. No box-drawing ASCII art (that was
    // the source of the garbled "2oa" rendering).
    println!("  {}", brand("route"));
    println!("  {}", dim("lightweight version manager for Web Coding"));
    println!("  {}", dim(&format!("v{}", env!("CARGO_PKG_VERSION"))));
    println!();

    match repo {
        Some(r) => {
            let path = r.project_path().display().to_string();
            let branch = r.get_current_branch_name().unwrap_or_else(|_| "?".to_string());
            let count = r.list_commits(None, 100_000).map(|c| c.len()).unwrap_or(0);

            println!("  {} {}", dim("Project:"), path);
            println!("  {} {}", dim("Branch:"), accent(&branch));
            println!("  {} {}", dim("Records:"), id(&count.to_string()));
        }
        None => {
            println!("  {}", warn("Not a Route repository."));
            println!(
                "  {}",
                faint("Run `route init` first, or start with --path <dir>.")
            );
        }
    }
    println!();
    println!(
        "  {} {} {} {} {}",
        dim("Type"),
        accent("help"),
        dim("for commands ·"),
        accent("exit"),
        dim("to quit")
    );
    println!();
    Ok(())
}
