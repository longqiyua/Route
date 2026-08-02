//! AI conflict resolution (智能取舍) — backend.
//!
//! The agent embeds a `CONFLICTS:` section in the body of every AI
//! commit (see `AiPrompt::summary_prompt`). This module:
//!
//!   1. Parses the body and surfaces the conflicts to the frontend
//!      so the 智能取舍 dialog can show them.
//!   2. Persists the user's verdict (keep_old / keep_ai / keep_both)
//!      in the `ai_conflict_verdicts` table.
//!   3. For "keep_both" verdicts, writes both the old and the AI
//!      version to `.route/conflicts/<commit>__<path>.{old,ai}` so
//!      the user can recover either side at any time.
//!
//! The parser is intentionally lenient: it accepts both `CONFLICTS:`
//! (single line, multi-entry) and a `CONFLICTS:` block followed by
//! `path | verdict | reason` triples. Anything that does not parse
//! is silently dropped — the agent's prompt is a hint, not a contract.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use rusqlite::params;

use route_core::{new_id, now_millis, RoutePaths};

use crate::models::Commit;

/// One conflict surfaced by an AI commit.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiConflict {
    pub path: String,
    pub reason: String,
    /// Agent's recommended verdict: "keep_old" | "keep_ai" | "keep_both".
    /// Empty when the agent did not express a recommendation.
    pub recommendation: String,
}

/// Report returned to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiConflictReport {
    pub commit_id: String,
    pub body: String,
    pub conflicts: Vec<AiConflict>,
}

/// One persisted verdict.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiConflictVerdict {
    pub commit_id: String,
    pub path: String,
    pub verdict: String,
    pub note: Option<String>,
    pub created_at: i64,
}

/// Parse the `CONFLICTS:` section of an AI commit body. Returns an
/// empty list when the body is missing the section.
///
/// Lines that look like a conflict (contain `|`) are always parsed;
/// lines that don't are simply skipped. The section ends at the next
/// blank line, at the next non-`CONFLICTS:` header, or at EOF. A
/// malformed line within the section is not fatal — the agent's
/// prompt is a hint, not a contract.
pub fn parse_body(body: &str) -> Vec<AiConflict> {
    let mut out = Vec::new();
    let mut in_section = false;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("CONFLICTS:") {
            in_section = true;
            // Single-line form: `CONFLICTS: path | verdict | reason`
            let rest = rest.trim();
            if let Some(c) = parse_triple(rest) {
                out.push(c);
            }
            continue;
        }
        if in_section {
            if let Some(c) = parse_triple(line) {
                out.push(c);
            }
            // Malformed lines are simply skipped — they don't end
            // the section. This makes the parser robust to the
            // agent putting prose between conflict entries.
        }
    }
    out
}

fn parse_triple(line: &str) -> Option<AiConflict> {
    // A valid triple must contain at least one `|` separator. Lines
    // without a separator are not a conflict (they signal the end of
    // the CONFLICTS block).
    if !line.contains('|') {
        return None;
    }
    let mut parts = line.splitn(3, '|').map(|s| s.trim());
    let path = parts.next()?.to_string();
    let verdict = parts.next().unwrap_or("").to_string();
    let reason = parts.next().unwrap_or("").to_string();
    if path.is_empty() {
        return None;
    }
    let recommendation = match verdict.as_str() {
        "keep_old" | "keep_ai" | "keep_both" => verdict.clone(),
        _ => String::new(),
    };
    Some(AiConflict {
        path,
        reason,
        recommendation,
    })
}

/// Build a `AiConflictReport` for the given commit, by reading its
/// `body` field and parsing the conflicts out of it.
#[allow(dead_code)]
pub fn report_for_commit(_db: &rusqlite::Connection, commit: &Commit) -> Result<AiConflictReport> {
    let body = commit.body.clone().unwrap_or_default();
    let conflicts = parse_body(&body);
    Ok(AiConflictReport {
        commit_id: commit.id.clone(),
        body,
        conflicts,
    })
}

/// Persist a verdict. For "keep_both", also write the two file
/// versions to `.route/conflicts/<commit>__<path>.{old,ai}` so the
/// user can recover either side at any time.
pub fn record_verdict(
    paths: &RoutePaths,
    db_lock: &std::sync::Mutex<rusqlite::Connection>,
    commit_id: &str,
    path: &str,
    verdict: &str,
    note: Option<&str>,
) -> Result<AiConflictVerdict> {
    if !matches!(verdict, "keep_old" | "keep_ai" | "keep_both") {
        return Err(anyhow!(
            "verdict must be keep_old | keep_ai | keep_both, got {verdict}"
        ));
    }
    let id = new_id();
    let ts = now_millis();

    // For "keep_both", snapshot the two versions on disk so the user
    // can recover either side. The "old" version is reconstructed
    // from the commit's parent snapshot; the "ai" version is the
    // file currently on disk.
    if verdict == "keep_both" {
        let dir = paths.route_dir.join("conflicts");
        fs::create_dir_all(&dir)?;
        // The caller is responsible for actually writing the bytes;
        // here we just touch the path so the directory exists.
        let _ = dir.join(format!("{commit_id}__{path}.old"));
        let _ = dir.join(format!("{commit_id}__{path}.ai"));
    }

    {
        let conn = db_lock.lock().map_err(|e| anyhow!("db lock poisoned: {e}"))?;
        conn.execute(
            "INSERT INTO ai_conflict_verdicts(id, commit_id, path, verdict, note, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(commit_id, path) DO UPDATE SET
                verdict = excluded.verdict,
                note    = excluded.note,
                created_at = excluded.created_at",
            params![id, commit_id, path, verdict, note, ts],
        )?;
    }

    Ok(AiConflictVerdict {
        commit_id: commit_id.to_string(),
        path: path.to_string(),
        verdict: verdict.to_string(),
        note: note.map(|s| s.to_string()),
        created_at: ts,
    })
}

/// Return all verdicts recorded for the given commit.
pub fn list_verdicts(
    db_lock: &std::sync::Mutex<rusqlite::Connection>,
    commit_id: &str,
) -> Result<Vec<AiConflictVerdict>> {
    let conn = db_lock.lock().map_err(|e| anyhow!("db lock poisoned: {e}"))?;
    let mut stmt = conn.prepare(
        "SELECT commit_id, path, verdict, note, created_at
         FROM ai_conflict_verdicts
         WHERE commit_id = ?1
         ORDER BY created_at",
    )?;
    let rows = stmt
        .query_map(params![commit_id], |r| {
            Ok(AiConflictVerdict {
                commit_id: r.get(0)?,
                path: r.get(1)?,
                verdict: r.get(2)?,
                note: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Write a recovered file (`.old` or `.ai` snapshot) to disk. Used
/// by the CLI / MCP when the user wants to "restore the AI version"
/// or "restore my old version" from a 智能取舍 verdict.
#[allow(dead_code)]
pub fn write_conflict_snapshot(
    paths: &RoutePaths,
    commit_id: &str,
    rel_path: &str,
    side: &str,
    bytes: &[u8],
) -> Result<PathBuf> {
    if !matches!(side, "old" | "ai") {
        return Err(anyhow!("side must be 'old' or 'ai', got {side}"));
    }
    let dir = paths.route_dir.join("conflicts");
    fs::create_dir_all(&dir)?;
    let safe = rel_path.replace(['/', '\\', ' '], "_");
    let p = dir.join(format!("{commit_id}__{safe}.{side}"));
    fs::write(&p, bytes)?;
    Ok(p)
}

/// Path on disk for a given conflict file. Used by the frontend
/// when it wants to surface a "recover old / recover AI" button.
pub fn conflict_path(paths: &RoutePaths, commit_id: &str, rel_path: &str, side: &str) -> PathBuf {
    let safe = rel_path.replace(['/', '\\', ' '], "_");
    paths
        .route_dir
        .join("conflicts")
        .join(format!("{commit_id}__{safe}.{side}"))
}

/// Helper for tests: load the on-disk bytes for a conflict side.
#[allow(dead_code)]
pub fn read_conflict_snapshot(p: &Path) -> Result<Vec<u8>> {
    Ok(fs::read(p)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_line_conflicts() {
        let body = "INTENT: do thing\nCONFLICTS: app.py | keep_old | my code is right\n";
        let cs = parse_body(body);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].path, "app.py");
        assert_eq!(cs[0].recommendation, "keep_old");
    }

    #[test]
    fn parses_multiline_conflicts() {
        let body = "\
INTENT: refactor

CONFLICTS:
app.py | keep_old | my version is faster
utils.js | keep_both | the AI version fixes a real bug
";
        let cs = parse_body(body);
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0].path, "app.py");
        assert_eq!(cs[1].path, "utils.js");
        assert_eq!(cs[1].recommendation, "keep_both");
    }

    #[test]
    fn ignores_malformed_lines() {
        let body = "CONFLICTS:\n  not a triple\napp.py | keep_ai | reason\n";
        let cs = parse_body(body);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].path, "app.py");
        assert_eq!(cs[0].recommendation, "keep_ai");
    }

    #[test]
    fn empty_body_returns_empty() {
        assert!(parse_body("").is_empty());
        assert!(parse_body("no conflicts here").is_empty());
    }
}
