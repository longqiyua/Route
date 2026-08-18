//! AI Delivery Adapter v0.
//!
//! Route does not implement its own Agent Runtime. Instead it compiles
//! the Effective Development Context into a file that an external
//! Coding AI (Claude Code, Codex, ...) can consume natively.
//!
//! ```text
//! Constitution / Protocol / Reference
//!           ↓
//!    ContextSnapshot
//!           ↓
//!      Adapter
//!           ↓
//!  CLAUDE.md / AGENTS.md / .route/generated/context.md
//! ```
//!
//! The generated file is **never** the source of truth. The real data
//! lives in `.route/`. The adapter writes a managed block delimited by
//! `<!-- ROUTE:BEGIN -->` / `<!-- ROUTE:END -->` markers so user
//! content outside the block is preserved across re-applies.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::constitutive::{
    self, context_dir, AgentPolicy, ContextBudget, ContextHistoryManifest, ContextSnapshot,
    ReferenceSelector,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Opening marker for the Route managed block.
pub const ROUTE_BEGIN: &str = "<!-- ROUTE:BEGIN -->";
/// Closing marker for the Route managed block.
pub const ROUTE_END: &str = "<!-- ROUTE:END -->";

// ---------------------------------------------------------------------------
// ApplyTarget
// ---------------------------------------------------------------------------

/// Which Coding AI / output format to compile the context for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApplyTarget {
    /// Claude Code — writes `CLAUDE.md` at the project root.
    Claude,
    /// OpenAI Codex — writes `AGENTS.md` at the project root.
    Codex,
    /// DeepSeek-compatible harness — writes `.route/generated/deepseek-context.md`
    DeepSeek,
    /// Portable Markdown — writes `.route/generated/context.md`.
    Generic,
}

impl ApplyTarget {
    /// Human-readable label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::DeepSeek => "deepseek",
            Self::Generic => "generic",
        }
    }

    /// Display name for status output.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::DeepSeek => "DeepSeek",
            Self::Generic => "Generic",
        }
    }

    /// Relative path (from project root) of the target file.
    pub fn relative_path(self) -> &'static str {
        match self {
            Self::Claude => "CLAUDE.md",
            Self::Codex => "AGENTS.md",
            Self::DeepSeek => ".route/generated/deepseek-context.md",
            Self::Generic => ".route/generated/context.md",
        }
    }

    /// Absolute path of the target file.
    pub fn target_path(self, project_root: &Path) -> PathBuf {
        project_root.join(self.relative_path())
    }

    /// Parse a target name from a CLI argument.
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "deepseek" | "deepseek-harness" => Ok(Self::DeepSeek),
            "generic" => Ok(Self::Generic),
            other => Err(anyhow::anyhow!(
                "unknown apply target '{other}'. Expected: claude | codex | deepseek | generic"
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// ApplyRecord + Lineage
// ---------------------------------------------------------------------------

/// One apply event: which target, which context, when, and the hash
/// of the generated block content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyRecord {
    /// Which adapter was used.
    pub target: ApplyTarget,
    /// The `ContextSnapshot::fingerprint` that was compiled.
    pub context_hash: String,
    /// Unix-millis when the apply happened.
    pub applied_at: i64,
    /// SHA-256 of the managed block content (text between markers).
    pub generated_hash: String,
    /// Optional task hash for task-scoped apply (P7). SHA-256 of the task string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_hash: Option<String>,
    /// Optional selected reference IDs for task-scoped apply (P7).
    #[serde(default)]
    pub selected_reference_ids: Vec<String>,
}

/// Apply lineage stored in `.route/context/applied.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApplyLineage {
    /// One record per (target, context_hash) pair. The latest record
    /// for a given target is the one with the highest `applied_at`.
    #[serde(default)]
    pub records: Vec<ApplyRecord>,
}

impl ApplyLineage {
    fn path(project_root: &Path) -> PathBuf {
        context_dir(project_root).join("applied.json")
    }

    /// Load the lineage file. Returns an empty `ApplyLineage` if the
    /// file does not exist yet.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = Self::path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading apply lineage {}", p.display()))?;
        let lineage: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing apply lineage {}", p.display()))?;
        Ok(lineage)
    }

    /// Persist the lineage file atomically.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = Self::path(project_root);
        let json = serde_json::to_vec_pretty(self)?;
        constitutive::write_atomic(&p, &json)?;
        Ok(())
    }

    /// Return the most recent apply record for `target`, or `None`.
    pub fn latest_for(&self, target: ApplyTarget) -> Option<&ApplyRecord> {
        self.records
            .iter()
            .filter(|r| r.target == target)
            .max_by_key(|r| r.applied_at)
    }

    /// Insert or replace the record for `(target, context_hash)`.
    fn upsert(&mut self, record: ApplyRecord) {
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|r| r.target == record.target && r.context_hash == record.context_hash)
        {
            *existing = record;
        } else {
            self.records.push(record);
        }
    }
}

// ---------------------------------------------------------------------------
// Managed block operations
// ---------------------------------------------------------------------------

/// Result of scanning a file for managed block markers.
#[derive(Debug, PartialEq)]
enum BlockScan {
    /// No markers present at all.
    Absent,
    /// Exactly one well-formed `BEGIN … END` pair.
    Found {
        before: String,
        content: String,
        after: String,
    },
    /// Markers are present but malformed (unbalanced, nested, etc.).
    Malformed(String),
}

/// Scan `text` for `<!-- ROUTE:BEGIN -->` / `<!-- ROUTE:END -->` markers.
fn scan_block(text: &str) -> BlockScan {
    let begin_idx = text.find(ROUTE_BEGIN);
    let end_idx = text.find(ROUTE_END);

    match (begin_idx, end_idx) {
        (None, None) => BlockScan::Absent,
        (Some(_), None) => {
            BlockScan::Malformed("found ROUTE:BEGIN but no matching ROUTE:END".to_string())
        }
        (None, Some(_)) => {
            BlockScan::Malformed("found ROUTE:END but no matching ROUTE:BEGIN".to_string())
        }
        (Some(b), Some(e)) => {
            if b > e {
                return BlockScan::Malformed("ROUTE:BEGIN appears after ROUTE:END".to_string());
            }
            // Check for a second BEGIN before the END (nested/duplicate).
            let after_begin = &text[b + ROUTE_BEGIN.len()..];
            if let Some(second_begin) = after_begin.find(ROUTE_BEGIN) {
                if b + ROUTE_BEGIN.len() + second_begin < e {
                    return BlockScan::Malformed(
                        "multiple ROUTE:BEGIN markers before ROUTE:END".to_string(),
                    );
                }
            }
            let before = text[..b].to_string();
            let content_start = b + ROUTE_BEGIN.len();
            let content = text[content_start..e].to_string();
            let after = text[e + ROUTE_END.len()..].to_string();
            BlockScan::Found {
                before,
                content,
                after,
            }
        }
    }
}

/// Replace the managed block in `existing` with `block_content`, or
/// append a new managed block if no markers are present.
///
/// Returns an error if the markers are malformed — the original file
/// is never modified in that case.
fn replace_or_append_block(existing: &str, block_content: &str) -> Result<String> {
    match scan_block(existing) {
        BlockScan::Absent => {
            // No markers yet: append a new block at the end.
            let mut out = existing.to_string();
            // Ensure there's a blank line separating user content from
            // the managed block, but don't add excessive whitespace.
            if !out.is_empty() && !out.ends_with("\n\n") {
                if out.ends_with('\n') {
                    out.push('\n');
                } else {
                    out.push_str("\n\n");
                }
            }
            out.push_str(ROUTE_BEGIN);
            out.push('\n');
            out.push_str(block_content);
            if !block_content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(ROUTE_END);
            out.push('\n');
            Ok(out)
        }
        BlockScan::Found {
            before,
            content: _,
            after,
        } => {
            // Replace content between markers.
            let mut out = before;
            out.push_str(ROUTE_BEGIN);
            out.push('\n');
            out.push_str(block_content);
            if !block_content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(ROUTE_END);
            out.push_str(&after);
            Ok(out)
        }
        BlockScan::Malformed(msg) => Err(anyhow::anyhow!(
            "refusing to write: managed block markers are malformed — {msg}. \
             Fix or remove the ROUTE:BEGIN/ROUTE:END markers manually, then re-run `route apply`."
        )),
    }
}

// ---------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------

/// Compile the Effective Context into the target file.
///
/// 1. Collects a fresh `ContextSnapshot`.
/// 2. Generates the Markdown body via `effective_context()` or
///    `build_context(task, ...)` when a task is provided.
/// 3. Reads the existing target file (if any).
/// 4. Replaces or appends the managed block.
/// 5. Atomically writes the result.
/// 6. Records the apply lineage (with task hash and selected refs when
///    task-scoped).
///
/// Returns the `ApplyRecord` for this apply event.
pub fn apply_context(
    project_root: &Path,
    target: ApplyTarget,
    task: Option<&str>,
) -> Result<ApplyRecord> {
    // 1. Fresh snapshot + context hash.
    let snap = ContextSnapshot::collect(project_root)?;
    let context_hash = snap.fingerprint.clone();

    // 2. Generate context body — task-scoped if task provided, else global.
    let (base_body, task_hash, selected_reference_ids) = if let Some(t) = task {
        let budget = ContextBudget::default();
        let body = constitutive::build_context(
            project_root,
            Some(t),
            Some(target.as_str()),
            5,
            budget,
            None,
        )?;
        let task_hash = Some(route_core::sha256_hex(t.as_bytes()));

        // Extract selected reference IDs from the context
        let selector = ReferenceSelector::new(t, Some(target.as_str()), 5, budget);
        let (selected, _) = selector.select(&snap.registry);
        let ids: Vec<String> = selected.iter().map(|sr| sr.reference_id.clone()).collect();

        (body, task_hash, ids)
    } else {
        let body = constitutive::effective_context(project_root)?;
        (body, None, Vec::new())
    };

    let policy = AgentPolicy::from_protocol_body(&snap.protocol_body);

    let body = if policy.roles.is_empty() && matches!(policy.mode, constitutive::AgentMode::Single)
    {
        // Default policy — no need to append target-specific compilation.
        base_body
    } else {
        // Append target-specific agent policy compilation.
        let policy_block = match target {
            ApplyTarget::Claude => policy.render_claude(),
            ApplyTarget::Codex => policy.render_codex(),
            ApplyTarget::DeepSeek => policy.render_deepseek(),
            ApplyTarget::Generic => policy.render_generic(),
        };
        format!("{}\n\n{}", base_body, policy_block)
    };
    // The hash must match what scan_block will extract: replace_or_append_block
    // always writes `\n` after ROUTE_BEGIN before the body, and scan_block
    // extracts from right after ROUTE_BEGIN, so the extracted content is `\n{body}`.
    let scan_content = format!("\n{}", body);
    let generated_hash = route_core::sha256_hex(scan_content.as_bytes());

    // 3. Read existing file.
    let target_path = target.target_path(project_root);
    let existing = std::fs::read_to_string(&target_path).unwrap_or_default();

    // 4. Replace or append managed block.
    let new_content = replace_or_append_block(&existing, &body)?;

    // 5. Atomic write.
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    constitutive::write_atomic(&target_path, new_content.as_bytes())?;

    // Also record the context history manifest for this snapshot so
    // that `route context --history` shows the apply event.
    let _ = ContextHistoryManifest::record_if_new_triggered(project_root, &snap, Some("apply"))?;

    // 6. Record lineage.
    let record = ApplyRecord {
        target,
        context_hash: context_hash.clone(),
        applied_at: route_core::now_millis(),
        generated_hash,
        task_hash,
        selected_reference_ids,
    };
    let mut lineage = ApplyLineage::load(project_root)?;
    lineage.upsert(record.clone());
    lineage.save(project_root)?;

    Ok(record)
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// Status of an applied target relative to the current context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyStatus {
    /// The applied context hash matches the current context fingerprint,
    /// and the managed block content matches the recorded generated_hash.
    Current,
    /// The context has changed since the last apply.
    Outdated,
    /// No apply has ever been recorded for this target.
    NotApplied,
    /// The managed block file has been modified since the last apply
    /// (generated_hash mismatch). The user may have edited the block.
    FileModified,
    /// The managed block markers are malformed or missing.
    Malformed,
}

/// Check whether `target` is up-to-date with the current Effective
/// Context.
///
/// Returns `(status, applied_context_hash, current_context_hash)`.
pub fn check_status(
    project_root: &Path,
    target: ApplyTarget,
) -> Result<(ApplyStatus, String, String)> {
    let current_hash = ContextSnapshot::collect(project_root)?.fingerprint;
    let lineage = ApplyLineage::load(project_root)?;
    let latest = lineage.latest_for(target);
    let applied_hash = latest.map(|r| r.context_hash.clone()).unwrap_or_default();

    let status = if applied_hash.is_empty() {
        ApplyStatus::NotApplied
    } else if applied_hash != current_hash {
        ApplyStatus::Outdated
    } else {
        // Context is current — now check the actual file content.
        let target_path = target.target_path(project_root);
        let actual = std::fs::read_to_string(&target_path).unwrap_or_default();
        let scan_result = scan_block(&actual);
        match scan_result {
            BlockScan::Found { content, .. } => {
                let actual_hash = route_core::sha256_hex(content.as_bytes());
                let expected_hash = &latest.unwrap().generated_hash;
                if actual_hash == *expected_hash {
                    ApplyStatus::Current
                } else {
                    ApplyStatus::FileModified
                }
            }
            BlockScan::Absent => {
                // No markers at all — file modified or deleted.
                ApplyStatus::FileModified
            }
            BlockScan::Malformed(_) => ApplyStatus::Malformed,
        }
    };

    Ok((status, applied_hash, current_hash))
}

/// Verify a specific target, returning detailed verification info.
#[derive(Debug, Clone)]
pub struct VerifyInfo {
    pub target: ApplyTarget,
    pub status: ApplyStatus,
    pub applied_context_hash: String,
    pub current_context_hash: String,
    pub applied_generated_hash: String,
    pub actual_generated_hash: String,
    pub detail: String,
}

/// Run full verification for a target: checks context hash AND
/// managed block content integrity.
pub fn verify_target(project_root: &Path, target: ApplyTarget) -> Result<VerifyInfo> {
    let current_hash = ContextSnapshot::collect(project_root)?.fingerprint;
    let lineage = ApplyLineage::load(project_root)?;
    let latest = lineage.latest_for(target);

    let applied_hash = latest.map(|r| r.context_hash.clone()).unwrap_or_default();
    let applied_generated_hash = latest.map(|r| r.generated_hash.clone()).unwrap_or_default();

    let target_path = target.target_path(project_root);
    let actual = std::fs::read_to_string(&target_path).unwrap_or_default();
    let scan_result = scan_block(&actual);

    let (status, actual_generated_hash, detail) = if applied_hash.is_empty() {
        (
            ApplyStatus::NotApplied,
            String::new(),
            "no apply record exists for this target".to_string(),
        )
    } else if applied_hash != current_hash {
        (
            ApplyStatus::Outdated,
            String::new(),
            format!(
                "context hash mismatch: applied={} current={}",
                &applied_hash[..16.min(applied_hash.len())],
                &current_hash[..16.min(current_hash.len())]
            ),
        )
    } else {
        match scan_result {
            BlockScan::Found { content, .. } => {
                let actual_hash = route_core::sha256_hex(content.as_bytes());
                if actual_hash == applied_generated_hash {
                    (
                        ApplyStatus::Current,
                        actual_hash,
                        "context and file content both match".to_string(),
                    )
                } else {
                    (
                        ApplyStatus::FileModified,
                        actual_hash.clone(),
                        format!(
                            "managed block content hash mismatch: expected={} actual={}",
                            &applied_generated_hash[..16.min(applied_generated_hash.len())],
                            &actual_hash[..16.min(actual_hash.len())]
                        ),
                    )
                }
            }
            BlockScan::Absent => (
                ApplyStatus::FileModified,
                String::new(),
                "managed block markers not found in target file".to_string(),
            ),
            BlockScan::Malformed(msg) => (
                ApplyStatus::Malformed,
                String::new(),
                format!("malformed managed block markers: {msg}"),
            ),
        }
    };

    Ok(VerifyInfo {
        target,
        status,
        applied_context_hash: applied_hash,
        current_context_hash: current_hash,
        applied_generated_hash,
        actual_generated_hash,
        detail,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::BasicRepository;
    use tempfile::TempDir;

    // -------------------------------------------------------------------
    // Managed block
    // -------------------------------------------------------------------

    #[test]
    fn managed_block_absent_when_no_markers() {
        let text = "# My Project\n\nSome user content.\n";
        assert_eq!(scan_block(text), BlockScan::Absent);
    }

    #[test]
    fn managed_block_found_when_well_formed() {
        let text = "# My Project\n\n<!-- ROUTE:BEGIN -->\nhello\n<!-- ROUTE:END -->\n\ntail\n";
        let result = scan_block(text);
        match result {
            BlockScan::Found {
                before,
                content,
                after,
            } => {
                assert!(before.contains("# My Project"));
                assert_eq!(content.trim(), "hello");
                assert!(after.contains("tail"));
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn managed_block_malformed_when_only_begin() {
        let text = "<!-- ROUTE:BEGIN -->\nhello\n";
        assert!(matches!(scan_block(text), BlockScan::Malformed(_)));
    }

    #[test]
    fn managed_block_malformed_when_only_end() {
        let text = "hello\n<!-- ROUTE:END -->\n";
        assert!(matches!(scan_block(text), BlockScan::Malformed(_)));
    }

    #[test]
    fn managed_block_malformed_when_begin_after_end() {
        let text = "<!-- ROUTE:END -->\nhello\n<!-- ROUTE:BEGIN -->\n";
        assert!(matches!(scan_block(text), BlockScan::Malformed(_)));
    }

    #[test]
    fn managed_block_malformed_when_nested_begin() {
        let text = "<!-- ROUTE:BEGIN -->\n<!-- ROUTE:BEGIN -->\nhello\n<!-- ROUTE:END -->\n";
        assert!(matches!(scan_block(text), BlockScan::Malformed(_)));
    }

    #[test]
    fn replace_or_append_creates_new_block_when_absent() {
        let existing = "# My Project\n\nUser notes here.\n";
        let block = "## Route Context\n\nHello.\n";
        let result = replace_or_append_block(existing, block).unwrap();
        assert!(result.contains("<!-- ROUTE:BEGIN -->"));
        assert!(result.contains("<!-- ROUTE:END -->"));
        assert!(result.contains("## Route Context"));
        // User content preserved.
        assert!(result.contains("# My Project"));
        assert!(result.contains("User notes here."));
    }

    #[test]
    fn replace_or_append_replaces_existing_block() {
        let existing =
            "# My Project\n\n<!-- ROUTE:BEGIN -->\nOLD CONTENT\n<!-- ROUTE:END -->\n\ntail\n";
        let block = "NEW CONTENT";
        let result = replace_or_append_block(existing, block).unwrap();
        assert!(result.contains("NEW CONTENT"));
        assert!(!result.contains("OLD CONTENT"));
        assert!(result.contains("# My Project"));
        assert!(result.contains("tail"));
    }

    #[test]
    fn replace_or_append_rejects_malformed() {
        let existing = "<!-- ROUTE:BEGIN -->\nhello\n";
        let block = "new";
        let result = replace_or_append_block(existing, block);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("malformed"));
    }

    // -------------------------------------------------------------------
    // End-to-end apply
    // -------------------------------------------------------------------

    fn init_project() -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        BasicRepository::init(&root).unwrap();
        (tmp, root)
    }

    #[test]
    fn apply_claude_creates_managed_block() {
        let (_tmp, root) = init_project();
        let record = apply_context(&root, ApplyTarget::Claude, None).unwrap();
        assert_eq!(record.target, ApplyTarget::Claude);
        assert!(!record.context_hash.is_empty());
        assert!(!record.generated_hash.is_empty());

        let claude_md = root.join("CLAUDE.md");
        assert!(claude_md.exists(), "CLAUDE.md should exist");
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("<!-- ROUTE:BEGIN -->"));
        assert!(content.contains("<!-- ROUTE:END -->"));
        assert!(content.contains("Effective Development Context"));
    }

    #[test]
    fn apply_preserves_existing_user_content() {
        let (_tmp, root) = init_project();
        let claude_md = root.join("CLAUDE.md");
        std::fs::write(
            &claude_md,
            "# My Claude Instructions\n\nDo not hallucinate.\n\n## Custom Rules\n\n- Be concise.\n",
        )
        .unwrap();

        apply_context(&root, ApplyTarget::Claude, None).unwrap();

        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("# My Claude Instructions"));
        assert!(content.contains("Do not hallucinate."));
        assert!(content.contains("## Custom Rules"));
        assert!(content.contains("Be concise"));
        assert!(content.contains("<!-- ROUTE:BEGIN -->"));
        assert!(content.contains("<!-- ROUTE:END -->"));
    }

    #[test]
    fn repeated_apply_is_idempotent() {
        let (_tmp, root) = init_project();
        let r1 = apply_context(&root, ApplyTarget::Claude, None).unwrap();
        let content1 = std::fs::read_to_string(root.join("CLAUDE.md")).unwrap();

        // Apply again without any changes.
        let r2 = apply_context(&root, ApplyTarget::Claude, None).unwrap();
        let content2 = std::fs::read_to_string(root.join("CLAUDE.md")).unwrap();

        assert_eq!(
            r1.generated_hash, r2.generated_hash,
            "generated_hash must be stable across repeated applies"
        );
        assert_eq!(content1, content2, "file content must be identical");
    }

    #[test]
    fn protocol_change_makes_status_outdated() {
        let (_tmp, root) = init_project();
        apply_context(&root, ApplyTarget::Claude, None).unwrap();

        // Status should be Current.
        let (status, _, _) = check_status(&root, ApplyTarget::Claude).unwrap();
        assert_eq!(status, ApplyStatus::Current);

        // Modify Protocol → context hash changes.
        let mut p = constitutive::Protocol::read(&root).unwrap();
        p.body = "# New Protocol\n\nDifferent rules.\n".to_string();
        p.write(&root).unwrap();

        let (status, applied, current) = check_status(&root, ApplyTarget::Claude).unwrap();
        assert_eq!(status, ApplyStatus::Outdated);
        assert_ne!(applied, current);
    }

    #[test]
    fn re_apply_after_change_makes_status_current() {
        let (_tmp, root) = init_project();
        apply_context(&root, ApplyTarget::Claude, None).unwrap();

        let mut p = constitutive::Protocol::read(&root).unwrap();
        p.body = "# Changed\n".to_string();
        p.write(&root).unwrap();

        let (status, _, _) = check_status(&root, ApplyTarget::Claude).unwrap();
        assert_eq!(status, ApplyStatus::Outdated);

        apply_context(&root, ApplyTarget::Claude, None).unwrap();
        let (status, _, _) = check_status(&root, ApplyTarget::Claude).unwrap();
        assert_eq!(status, ApplyStatus::Current);
    }

    #[test]
    fn malformed_marker_prevents_overwrite() {
        let (_tmp, root) = init_project();
        let claude_md = root.join("CLAUDE.md");
        std::fs::write(&claude_md, "# My Project\n\n<!-- ROUTE:BEGIN -->\nhello\n").unwrap();

        let result = apply_context(&root, ApplyTarget::Claude, None);
        assert!(result.is_err(), "apply must fail on malformed markers");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("malformed"));

        // File must be unchanged.
        let content = std::fs::read_to_string(&claude_md).unwrap();
        assert!(content.contains("# My Project"));
        assert!(content.contains("hello"));
        assert!(!content.contains("<!-- ROUTE:END -->"));
    }

    #[test]
    fn codex_adapter_writes_agents_md() {
        let (_tmp, root) = init_project();
        let record = apply_context(&root, ApplyTarget::Codex, None).unwrap();
        assert_eq!(record.target, ApplyTarget::Codex);

        let agents_md = root.join("AGENTS.md");
        assert!(agents_md.exists(), "AGENTS.md should exist");
        let content = std::fs::read_to_string(&agents_md).unwrap();
        assert!(content.contains("<!-- ROUTE:BEGIN -->"));
        assert!(content.contains("Effective Development Context"));
    }

    #[test]
    fn generic_adapter_writes_route_generated() {
        let (_tmp, root) = init_project();
        let record = apply_context(&root, ApplyTarget::Generic, None).unwrap();
        assert_eq!(record.target, ApplyTarget::Generic);

        let ctx_md = root.join(".route/generated/context.md");
        assert!(ctx_md.exists(), ".route/generated/context.md should exist");
        let content = std::fs::read_to_string(&ctx_md).unwrap();
        assert!(content.contains("<!-- ROUTE:BEGIN -->"));
        assert!(content.contains("Effective Development Context"));
    }

    #[test]
    fn deepseek_adapter_writes_context_file() {
        let (_tmp, root) = init_project();
        let record = apply_context(&root, ApplyTarget::DeepSeek, None).unwrap();
        assert_eq!(record.target, ApplyTarget::DeepSeek);

        let ctx_md = root.join(".route/generated/deepseek-context.md");
        assert!(ctx_md.exists(), ".route/generated/deepseek-context.md should exist");
        let content = std::fs::read_to_string(&ctx_md).unwrap();
        assert!(content.contains("<!-- ROUTE:BEGIN -->"));
        assert!(content.contains("Effective Development Context"));
    }

    #[test]
    fn deepseek_target_parse_accepts_deepseek_harness_alias() {
        assert_eq!(ApplyTarget::parse("deepseek").unwrap(), ApplyTarget::DeepSeek);
        assert_eq!(ApplyTarget::parse("deepseek-harness").unwrap(), ApplyTarget::DeepSeek);
        assert_eq!(ApplyTarget::parse("DEEPSEEK").unwrap(), ApplyTarget::DeepSeek);
    }

    #[test]
    fn deepseek_and_claude_context_share_core_sections() {
        let (_tmp, root) = init_project();
        let task = "fix storage module";

        // Apply to both targets
        let record_claude = apply_context(&root, ApplyTarget::Claude, Some(task)).unwrap();
        let record_deepseek = apply_context(&root, ApplyTarget::DeepSeek, Some(task)).unwrap();

        // Both should have context hashes
        assert!(!record_claude.context_hash.is_empty());
        assert!(!record_deepseek.context_hash.is_empty());

        // Both should have generated hashes
        assert!(!record_claude.generated_hash.is_empty());
        assert!(!record_deepseek.generated_hash.is_empty());

        // The core context hash should be the same (same task, same project state)
        assert_eq!(
            record_claude.context_hash,
            record_deepseek.context_hash,
            "core context hash must be identical across hosts for the same task"
        );

        // Check status after apply
        let (status_claude, _, _) = check_status(&root, ApplyTarget::Claude).unwrap();
        let (status_deepseek, _, _) = check_status(&root, ApplyTarget::DeepSeek).unwrap();
        assert_eq!(status_claude, ApplyStatus::Current);
        assert_eq!(status_deepseek, ApplyStatus::Current);
    }

    #[test]
    fn deepseek_apply_verify_outdated_detection() {
        let (_tmp, root) = init_project();
        apply_context(&root, ApplyTarget::DeepSeek, None).unwrap();

        // Should be Current
        let (status, _, _) = check_status(&root, ApplyTarget::DeepSeek).unwrap();
        assert_eq!(status, ApplyStatus::Current);

        // Modify Protocol to change context
        let mut p = crate::constitutive::Protocol::read(&root).unwrap();
        p.body = "# New Protocol\n\nDifferent rules.\n".to_string();
        p.write(&root).unwrap();

        // Should be Outdated
        let (status, _, _) = check_status(&root, ApplyTarget::DeepSeek).unwrap();
        assert_eq!(status, ApplyStatus::Outdated);
    }

    #[test]
    fn deepseek_file_modified_detection() {
        let (_tmp, root) = init_project();
        apply_context(&root, ApplyTarget::DeepSeek, None).unwrap();

        // Modify content inside the managed block to trigger hash mismatch
        let ctx_md = root.join(".route/generated/deepseek-context.md");
        let content = std::fs::read_to_string(&ctx_md).unwrap();
        let modified = content.replace("<!-- ROUTE:BEGIN -->", "<!-- ROUTE:BEGIN -->\nUser modification");
        std::fs::write(&ctx_md, &modified).unwrap();

        // Should detect FILE_MODIFIED
        let (status, _, _) = check_status(&root, ApplyTarget::DeepSeek).unwrap();
        assert_eq!(status, ApplyStatus::FileModified);
    }

    #[test]
    fn status_not_applied_when_never_applied() {
        let (_tmp, root) = init_project();
        let (status, applied, _) = check_status(&root, ApplyTarget::Claude).unwrap();
        assert_eq!(status, ApplyStatus::NotApplied);
        assert!(applied.is_empty());
    }

    #[test]
    fn apply_does_not_overwrite_user_content_on_re_apply() {
        let (_tmp, root) = init_project();
        let claude_md = root.join("CLAUDE.md");

        // First apply.
        apply_context(&root, ApplyTarget::Claude, None).unwrap();
        let after_first = std::fs::read_to_string(&claude_md).unwrap();

        // User adds content outside the managed block.
        let with_user = format!("# Custom Header\n\n{}\n", after_first);
        std::fs::write(&claude_md, &with_user).unwrap();

        // Re-apply.
        apply_context(&root, ApplyTarget::Claude, None).unwrap();
        let after_second = std::fs::read_to_string(&claude_md).unwrap();

        assert!(
            after_second.contains("# Custom Header"),
            "user content outside managed block must survive re-apply"
        );
    }
}
