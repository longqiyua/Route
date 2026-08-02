//! Centralized color + formatting helpers for route-tui.
//!
//! Colors use truecolor (24-bit) so the terminal palette matches the
//! desktop app's design tokens exactly (accent #B4AEEA etc.). When the
//! output is piped / redirected (non-tty), color is auto-disabled via
//! `owo_colors::set_override(false)` so captured text stays clean.

use chrono::{DateTime, Utc};
use owo_colors::OwoColorize;
use std::io::IsTerminal;

/// Initialize color support. Disables ANSI on non-tty stdout so piped
/// output (e.g. `route-tui < commands.txt`) stays readable.
pub fn init_colors() {
    if !std::io::stdout().is_terminal() {
        owo_colors::set_override(false);
    }
}

/// Format a millisecond timestamp (route's `created_at: i64`) as a
/// readable UTC string, matching route-cli's `format_ts`.
pub fn format_ts(millis: i64) -> String {
    let dt = DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

// ---------------------------------------------------------------------------
// Color helpers — each takes &str and returns a String with ANSI codes.
// Truecolor values mirror the desktop app's CSS tokens (styles.css :root).
// ---------------------------------------------------------------------------

/// Brand / accent purple (#B4AEEA) — logo, prompt, current branch.
pub fn brand(s: &str) -> String {
    s.truecolor(180, 174, 234).bold().to_string()
}

/// Accent purple without bold — highlights, current branch marker.
pub fn accent(s: &str) -> String {
    s.truecolor(180, 174, 234).to_string()
}

/// Teal/cyan — table headers, command names in help.
pub fn head(s: &str) -> String {
    s.truecolor(120, 180, 200).bold().to_string()
}

/// Success green (#9ec9a6) — ✓ markers.
pub fn ok(s: &str) -> String {
    s.truecolor(158, 201, 166).to_string()
}

/// Error red (#d98a82) — ✗ markers, failures.
pub fn err(s: &str) -> String {
    s.truecolor(217, 138, 130).to_string()
}

/// Ochre (#c8a87a) — warnings, "not a repo" notice.
pub fn warn(s: &str) -> String {
    s.truecolor(200, 168, 122).to_string()
}

/// Amber — snapshot / commit IDs.
pub fn id(s: &str) -> String {
    s.truecolor(212, 188, 120).to_string()
}

/// Muted gray (#909090) — labels, secondary text.
pub fn dim(s: &str) -> String {
    s.truecolor(144, 144, 144).to_string()
}

/// Faint gray (#6a6a6a) — hints, footnotes.
pub fn faint(s: &str) -> String {
    s.truecolor(106, 106, 106).to_string()
}

/// Build the colored prompt string `route❯ `.
pub fn prompt() -> String {
    format!("{} ", "route❯".truecolor(180, 174, 234).bold())
}
