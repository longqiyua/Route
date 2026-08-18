//! Data models for basic mode.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Branch kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BranchKind {
    /// Main branch — only one per repo.
    #[default]
    Main,
    /// Inherited branch — forked from parent baseline, stores only diff edges,
    /// can rebase to parent's new HEAD.
    Inherited,
    /// Sandbox branch — full deep copy of parent's HEAD, completely isolated.
    Sandbox,
}

impl BranchKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BranchKind::Main => "main",
            BranchKind::Inherited => "inherited",
            BranchKind::Sandbox => "sandbox",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "main" => Some(BranchKind::Main),
            "inherited" => Some(BranchKind::Inherited),
            "sandbox" => Some(BranchKind::Sandbox),
            _ => None,
        }
    }
}

/// Commit kind (lives on the edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitKind {
    Incremental,
    Full,
    Merge,
    Rollback,
}

impl CommitKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommitKind::Incremental => "incremental",
            CommitKind::Full => "full",
            CommitKind::Merge => "merge",
            CommitKind::Rollback => "rollback",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "incremental" => Some(CommitKind::Incremental),
            "full" => Some(CommitKind::Full),
            "merge" => Some(CommitKind::Merge),
            "rollback" => Some(CommitKind::Rollback),
            _ => None,
        }
    }
}

/// Snapshot — pure state node, no user metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub manifest_hash: String,
    pub created_at: i64,
}

/// Manifest content: path → blob hash.
pub type Manifest = HashMap<String, String>;

/// Manifest entry for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub blob_hash: String,
}

/// Commit — edge between two snapshots, metadata lives here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: String,
    pub from_snapshot: String,
    pub to_snapshot: String,
    pub message: String,
    pub author: Option<String>,
    pub created_at: i64,
    pub branch_id: String,
    pub kind: CommitKind,
    /// JSON-serialized ManifestDiff
    pub diff_summary: Option<String>,
    /// Who performed the change.
    /// - "user" by default for direct user edits
    /// - "ai:<name>" for AI-driven commits (e.g. "ai:claude")
    /// The author field above still carries the human-readable author string.
    pub operator: Option<String>,
    /// Long-form note attached to the commit. Used as the body of a
    /// user checkpoint, or the embedded prompt for AI-driven changes.
    pub body: Option<String>,
    /// True if this commit is a user-marked checkpoint (the `message`
    /// field holds the title).
    pub is_checkpoint: bool,
    /// True if the change came from the AI control channel (CLI / MCP).
    pub is_ai: bool,
    /// Transaction journal id, set only for rollback commits that carry
    /// a conversation linkage. Populated by `rollback_to_with` so the
    /// caller can complete the transaction after conversation ops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    /// Development-context fingerprint that was active when this commit
    /// was created. Populated automatically by the CLI commit path so
    /// the lineage store can answer: "which rules was AI following when
    /// it produced this commit?"
    /// `None` on commits created before this field was introduced —
    /// backwards-compatible by design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
    /// Version of the constitution at commit time (redundant with the
    /// history manifest, but handy for quick per-row lookups).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constitution_version: Option<u32>,
    /// Protocol revision at commit time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_revision: Option<u64>,
    /// Hash of the reference-entries semantic view at commit time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_entries_hash: Option<String>,
}

/// Path annotation attached to a commit edge (N-N).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitPathAnnotation {
    pub id: String,
    pub commit_id: String,
    pub text: String,
    pub created_at: i64,
}

/// Branch — subgraph label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub kind: BranchKind,
    pub parent_branch: Option<String>,
    pub baseline_snapshot: Option<String>,
    pub head_snapshot: Option<String>,
    pub created_at: i64,
}

/// Repository config stored in `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub version: u32,
    pub project_path: String,
    pub mode: String,
    pub created_at: i64,
    pub main_branch_id: String,
    /// File-tracking configuration. Drives which files the scanner
    /// considers, and which hash algorithm to use. The default values
    /// are populated by `TrackConfig::default()` when the field is
    /// missing on disk (older configs).
    #[serde(default)]
    pub track: TrackConfig,
}

/// File-tracking configuration. Stored inside `RepoConfig` so it lives
/// alongside the project (not in app-level state), and the values are
/// shared with AI agents via the hidden `.route/index.json` file.
///
/// All file-system operations in the project — scanning, watching,
/// committing — consult this config to decide which files to consider
/// and how to verify their integrity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackConfig {
    /// When true, every file under the project (except ignore patterns)
    /// is tracked. When false, only files whose extension is in
    /// `track_suffixes` are considered.
    pub track_all: bool,
    /// File suffixes to track when `track_all = false`. Always stored
    /// lower-case with a leading dot (e.g. ".py"). Empty by default.
    pub track_suffixes: Vec<String>,
    /// Prefix filters — when non-empty, only files whose relative path
    /// starts with one of these prefixes are tracked. Empty by default
    /// (no prefix restriction).
    pub track_prefixes: Vec<String>,
    /// When true, the scanner will additionally record the SHA-256 hash
    /// of every tracked file into the manifest alongside the (always-on)
    /// xxh3 fast hash. Off by default — the fast hash is sufficient
    /// for change detection, SHA-256 only helps when an external system
    /// needs to verify the bytes haven't been tampered with.
    pub verify_sha256: bool,
    /// Memory-buffer debounce (in milliseconds). When the watcher sees
    /// rapid file changes, it buffers them in memory and only flushes
    /// to disk after this many ms of quiet. Defaults to 5000ms so a
    /// burst of typing does not produce dozens of commits.
    pub memory_buffer_ms: u64,
    /// Cross-OS tracking. The user can pick any combination of
    /// Windows / macOS / Linux paths. When the binary runs on one of
    /// the selected OSes, the scanner is active; on others it is a
    /// no-op. By default all three are enabled so the demo works
    /// everywhere.
    pub track_on: TrackOs,
}

impl Default for TrackConfig {
    fn default() -> Self {
        Self {
            track_all: true,
            track_suffixes: vec![
                ".py".into(),
                ".js".into(),
                ".jsx".into(),
                ".ts".into(),
                ".tsx".into(),
                ".css".into(),
            ],
            track_prefixes: vec![],
            verify_sha256: false,
            memory_buffer_ms: 5000,
            track_on: TrackOs::all(),
        }
    }
}

/// Which operating systems the tracking should be active on. This is a
/// bitflags-like struct so the user can pick a combination.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TrackOs {
    pub windows: bool,
    pub macos: bool,
    pub linux: bool,
}

impl TrackOs {
    pub const fn all() -> Self {
        Self {
            windows: true,
            macos: true,
            linux: true,
        }
    }
    pub const fn none() -> Self {
        Self {
            windows: false,
            macos: false,
            linux: false,
        }
    }
    pub fn is_active_now(&self) -> bool {
        if cfg!(target_os = "windows") {
            self.windows
        } else if cfg!(target_os = "macos") {
            self.macos
        } else {
            self.linux
        }
    }
}

/// Built-in prompt template that AI agents are expected to embed in
/// their commit `body` field. Kept here so it lives with the data
/// model and is easy to update in one place. AI agents should call
/// `AiPrompt::summary_prompt()` to receive the canonical system text
/// they need to produce a one-sentence intent summary for each change.
pub struct AiPrompt;

impl AiPrompt {
    /// The system prompt that an AI agent should follow when driving
    /// the software. The agent is expected to:
    ///   1. State a one-sentence intent of the change.
    ///   2. List the files touched and the new vs. old logic at a high
    ///      level (no need to paste the diff).
    ///   3. Surface any "智能取舍" (smart取舍) decisions where the user
    ///      should choose between their existing logic and the AI's
    ///      replacement.
    pub fn summary_prompt() -> &'static str {
        "You are an AI agent operating the route version-control app. \
         For every change you make, embed a structured summary in the \
         commit body using this exact format:\n\
         \n\
         INTENT: <one sentence — what the change is meant to accomplish>\n\
         FILES: <comma-separated relative paths, max 5>\n\
         LOGIC: <one-sentence description of new vs old logic, or 'no logic change'>\n\
         CONFLICTS: <one line per path that overwrites user code, in the \
         form 'path | keep_old | reason', or 'none'>\n\
         \n\
         When CONFLICTS is non-empty, route will surface a 智能取舍 dialog \
         to the user asking them to keep the AI version, keep their old \
         version, or keep both. Do not silently overwrite user logic."
    }

    /// Short header the agent can prepend to its own prompt so the
    /// commit's `body` field is self-describing.
    pub fn summary_header() -> &'static str {
        "AI 自动总结"
    }
}

/// Snapshot annotated with which branch owns it (derived from commit's branch_id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotWithBranch {
    #[serde(flatten)]
    pub snapshot: Snapshot,
    pub branch_id: Option<String>,
    pub branch_name: Option<String>,
}

/// Diff summary stored on commit edge.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
}

impl DiffSummary {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }

    pub fn total(&self) -> usize {
        self.added.len() + self.modified.len() + self.removed.len()
    }

    pub fn short(&self) -> String {
        format!(
            "+{} ~{} -{}",
            self.added.len(),
            self.modified.len(),
            self.removed.len()
        )
    }
}

/// Tag — named immutable pointer to a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub snapshot_id: String,
    pub message: Option<String>,
    pub created_at: i64,
}
