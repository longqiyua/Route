//! ProjectMemory — semantic layer over raw history.
//!
//! Maintains "what the project is / why" rather than raw logs.
//! Stored in `.route/memory/` directory.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;
use crate::execution::{EvidenceKind, EvidenceStore, SessionStore};
use crate::learn::ExperienceStore;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The kind of a memory item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum MemoryItemKind {
    /// Current state/fact about the project
    #[default]
    #[serde(rename = "current")]
    Current,
    /// A decision that was made
    #[serde(rename = "decision")]
    Decision,
    /// Something that was tried and failed
    #[serde(rename = "failed_attempt")]
    FailedAttempt,
    /// A property that must always hold
    #[serde(rename = "invariant")]
    Invariant,
    /// An unresolved question
    #[serde(rename = "open_question")]
    OpenQuestion,
    /// Superseded or replaced by newer items
    #[serde(rename = "historical")]
    Historical,
}

impl fmt::Display for MemoryItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MemoryItemKind::Current => write!(f, "current"),
            MemoryItemKind::Decision => write!(f, "decision"),
            MemoryItemKind::FailedAttempt => write!(f, "failed_attempt"),
            MemoryItemKind::Invariant => write!(f, "invariant"),
            MemoryItemKind::OpenQuestion => write!(f, "open_question"),
            MemoryItemKind::Historical => write!(f, "historical"),
        }
    }
}

/// A single semantic item in the project memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    /// Kind of this memory item
    #[serde(default)]
    pub kind: MemoryItemKind,
    pub content: String,
    /// Commit/task/conversation/evidence/reference IDs
    #[serde(default)]
    pub source_ids: Vec<String>,
    /// 0.0 - 1.0
    pub confidence: f64,
    pub updated_at: i64,
    pub created_at: i64,
    /// ID of the item that superseded this one (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// ID of the item this supersedes (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A chain-of-reasoning explanation of why something is the way it is.
#[derive(Debug, Clone)]
pub struct MemoryWhyExplanation {
    pub topic: String,
    pub current_state: String,
    pub decision_chain: Vec<MemoryWhyLink>,
    pub alternatives_considered: Vec<String>,
    pub evidence: Vec<String>,
}

/// A single link in the decision chain.
#[derive(Debug, Clone)]
pub struct MemoryWhyLink {
    pub item_id: String,
    pub kind: String,
    pub content: String,
    pub source_ids: Vec<String>,
    pub confidence: f64,
}

/// Semantic layer over raw history. Maintains "what the project is / why"
/// rather than raw logs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub summary: String,
    pub current_architecture: String,
    #[serde(default)]
    pub decisions: Vec<MemoryItem>,
    #[serde(default)]
    pub known_risks: Vec<MemoryItem>,
    #[serde(default)]
    pub failed_attempts: Vec<MemoryItem>,
    #[serde(default)]
    pub conventions: Vec<MemoryItem>,
    #[serde(default)]
    pub open_questions: Vec<MemoryItem>,
    pub current_focus: String,
    pub updated_at: i64,
    pub version: u32,
    #[serde(default)]
    pub items: Vec<MemoryItem>,
}

/// Container for multiple project memories, with a pointer to the current one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStore {
    #[serde(default)]
    pub memories: Vec<ProjectMemory>,
    /// ID of current memory (index into memories, or a ULID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Directory for memory data: `.route/memory/`
pub fn memory_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("memory")
}

/// Path to the current memory JSON: `.route/memory/memory.json`
pub fn memory_path(project_root: &Path) -> PathBuf {
    memory_dir(project_root).join("memory.json")
}

/// Directory for historical memory snapshots: `.route/memory/history/`
pub fn history_path(project_root: &Path) -> PathBuf {
    memory_dir(project_root).join("history")
}

// ---------------------------------------------------------------------------
// MemoryStore
// ---------------------------------------------------------------------------

impl MemoryStore {
    /// Load memory store from disk. Returns an empty store if the file does
    /// not exist.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = memory_path(project_root);
        if !p.exists() {
            return Ok(Self {
                memories: Vec::new(),
                current: None,
            });
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading memory store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self {
                memories: Vec::new(),
                current: None,
            });
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    /// Persist memory store to disk atomically.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = memory_path(project_root);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        // Write to a temp file and rename for atomicity
        let tmp = p.with_extension("tmp");
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&tmp, &json)
            .with_context(|| format!("writing temporary memory store to {}", tmp.display()))?;
        std::fs::rename(&tmp, &p).with_context(|| {
            format!(
                "renaming memory store from {} to {}",
                tmp.display(),
                p.display()
            )
        })?;
        Ok(())
    }

    /// Get the current active memory, if any.
    pub fn current_memory(&self) -> Option<&ProjectMemory> {
        match &self.current {
            Some(id) => self.memories.iter().find(|m| {
                // Match by the first decision's id as a heuristic, or by index
                format!("{}", m.updated_at) == *id || m.summary == *id
            }),
            None => self.memories.last(),
        }
        .or_else(|| self.memories.last())
    }

    /// Get a mutable reference to the current active memory, if any.
    pub fn current_memory_mut(&mut self) -> Option<&mut ProjectMemory> {
        let idx = match &self.current {
            Some(id) => self
                .memories
                .iter()
                .position(|m| format!("{}", m.updated_at) == *id || m.summary == *id),
            None => {
                let len = self.memories.len();
                if len == 0 {
                    return None;
                }
                Some(len - 1)
            }
        };
        idx.and_then(move |i| self.memories.get_mut(i))
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            memories: Vec::new(),
            current: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ProjectMemory
// ---------------------------------------------------------------------------

impl ProjectMemory {
    /// Create a new empty ProjectMemory with the given version.
    fn new() -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            summary: String::new(),
            current_architecture: String::new(),
            decisions: Vec::new(),
            known_risks: Vec::new(),
            failed_attempts: Vec::new(),
            conventions: Vec::new(),
            open_questions: Vec::new(),
            current_focus: String::new(),
            updated_at: now,
            version: 1,
            items: Vec::new(),
        }
    }

    /// Generate memory proposals from existing ledger, evidence, experience
    /// events, commit history, and reference registry.
    ///
    /// This is a read-only operation — it generates proposals but does NOT
    /// auto-apply them. Returns a new `ProjectMemory` with the proposed items.
    pub fn refresh(project_root: &Path, _ledger: Option<&str>) -> Result<ProjectMemory> {
        let mut memory = ProjectMemory::new();
        let now = chrono::Utc::now().timestamp_millis();
        memory.updated_at = now;

        let mut item_id = 0u64;
        let mut next_id = || {
            item_id += 1;
            format!("prop-{}", item_id)
        };

        // --- Scan experience events ---
        if let Ok(exp_store) = ExperienceStore::load(project_root) {
            let mut failed_count = 0;
            let mut total_events = 0;

            for event in &exp_store.events {
                total_events += 1;
                match event.kind {
                    crate::learn::EventKind::TestFail | crate::learn::EventKind::UserReject => {
                        failed_count += 1;
                        let item = MemoryItem {
                            id: next_id(),
                            kind: MemoryItemKind::FailedAttempt,
                            content: format!("{}: {}", event.kind.as_str(), event.outcome),
                            source_ids: vec![event.id.clone()],
                            confidence: 0.6,
                            updated_at: event.created_at,
                            created_at: event.created_at,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec![],
                        };
                        memory.failed_attempts.push(item);
                    }
                    crate::learn::EventKind::ManualNote => {
                        let item = MemoryItem {
                            id: next_id(),
                            kind: MemoryItemKind::Decision,
                            content: event.outcome.clone(),
                            source_ids: vec![event.id.clone()],
                            confidence: 0.4,
                            updated_at: event.created_at,
                            created_at: event.created_at,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec![],
                        };
                        memory.decisions.push(item);
                    }
                    crate::learn::EventKind::ProposalApproved => {
                        let item = MemoryItem {
                            id: next_id(),
                            kind: MemoryItemKind::Invariant,
                            content: format!("Approved: {}", event.outcome),
                            source_ids: vec![event.id.clone()],
                            confidence: 0.7,
                            updated_at: event.created_at,
                            created_at: event.created_at,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec![],
                        };
                        memory.conventions.push(item);
                    }
                    _ => {}
                }
            }

            if total_events > 0 {
                memory.summary = format!(
                    "Project has {} experience events recorded, {} of which are failures or rejections.",
                    total_events, failed_count
                );
            }
        }

        // --- Scan evidence store ---
        if let Ok(ev_store) = EvidenceStore::load(project_root) {
            let mut rollback_count = 0;
            let mut commit_count = 0;

            for ev in &ev_store.evidence {
                match ev.kind {
                    EvidenceKind::Rollback => {
                        rollback_count += 1;
                        let item = MemoryItem {
                            id: next_id(),
                            kind: MemoryItemKind::Invariant,
                            content: format!(
                                "Rollback occurred (snapshot: {}, commit: {})",
                                ev.snapshot_id.as_deref().unwrap_or("unknown"),
                                ev.commit_id.as_deref().unwrap_or("unknown")
                            ),
                            source_ids: vec![ev.id.clone()],
                            confidence: 0.8,
                            updated_at: ev.created_at,
                            created_at: ev.created_at,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec![],
                        };
                        memory.known_risks.push(item);
                    }
                    EvidenceKind::Commit => {
                        commit_count += 1;
                    }
                    EvidenceKind::CheckFail => {
                        let item = MemoryItem {
                            id: next_id(),
                            kind: MemoryItemKind::FailedAttempt,
                            content: format!("Check failed: {}", ev.payload_hash),
                            source_ids: vec![ev.id.clone()],
                            confidence: 0.7,
                            updated_at: ev.created_at,
                            created_at: ev.created_at,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec![],
                        };
                        memory.failed_attempts.push(item);
                    }
                    _ => {}
                }
            }

            if rollback_count > 0 {
                memory.current_architecture = format!(
                    "{} rollbacks have been recorded, indicating instability in the project history.",
                    rollback_count
                );
            }
            if commit_count > 0 {
                if !memory.summary.is_empty() {
                    memory.summary.push_str(" ");
                }
                memory.summary.push_str(&format!(
                    "There are {} commit evidence records.",
                    commit_count
                ));
            }
        }

        // --- Scan session ledger ---
        if let Ok(sess_store) = SessionStore::load(project_root) {
            let active_sessions: Vec<_> = sess_store
                .sessions
                .iter()
                .filter(|s| s.ended_at.is_none())
                .collect();

            if !active_sessions.is_empty() {
                memory.current_focus = active_sessions
                    .iter()
                    .map(|s| s.task.clone())
                    .collect::<Vec<_>>()
                    .join("; ");
            }

            if !sess_store.sessions.is_empty() {
                memory.summary = format!(
                    "{} execution sessions have been recorded.",
                    sess_store.sessions.len()
                );
            }
        }

        // --- Scan reference registry ---
        if let Ok(registry) = crate::constitutive::ReferenceRegistry::read(project_root) {
            for entry in &registry.entries {
                let item = MemoryItem {
                    id: next_id(),
                    kind: MemoryItemKind::Invariant,
                    content: format!(
                        "Reference [{}] ({:?}): {} — {}",
                        entry.id, entry.type_, entry.description, entry.source
                    ),
                    source_ids: vec![entry.id.clone()],
                    confidence: 0.9,
                    updated_at: now,
                    created_at: now,
                    superseded_by: None,
                    supersedes: None,
                    tags: vec![],
                };
                memory.conventions.push(item);
            }
        }

        // --- Scan commit history via BasicRepository ---
        if let Ok(repo) = crate::repository::BasicRepository::open(project_root) {
            if let Ok(commits) = repo.list_commits(None, 50) {
                let mut total_commits = 0;
                let mut ai_commits = 0;
                let mut user_commits = 0;
                let mut checkpoint_count = 0;

                for c in &commits {
                    total_commits += 1;
                    if c.is_ai {
                        ai_commits += 1;
                    } else {
                        user_commits += 1;
                    }
                    if c.is_checkpoint {
                        checkpoint_count += 1;
                    }
                }

                if total_commits > 0 {
                    let summary = format!(
                        "Recent history: {} commits ({} AI-driven, {} user, {} checkpoints).",
                        total_commits, ai_commits, user_commits, checkpoint_count
                    );
                    if !memory.summary.is_empty() {
                        // memory.summary is already set from sessions/evidence;
                        // append to the current_focus instead
                        memory.current_focus = if memory.current_focus.is_empty() {
                            summary
                        } else {
                            format!("{}; {}", memory.current_focus, summary)
                        };
                    } else {
                        memory.summary = summary;
                    }
                }

                // Check for rollback commits
                for c in &commits {
                    if c.kind == crate::models::CommitKind::Rollback {
                        let item = MemoryItem {
                            id: next_id(),
                            kind: MemoryItemKind::Invariant,
                            content: format!(
                                "Rollback commit: {} (from {} to {})",
                                c.message, c.from_snapshot, c.to_snapshot
                            ),
                            source_ids: vec![c.id.clone()],
                            confidence: 0.9,
                            updated_at: c.created_at,
                            created_at: c.created_at,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec![],
                        };
                        memory.known_risks.push(item);
                    }
                }
            }
        }

        // If nothing was generated, provide a default
        if memory.summary.is_empty() && memory.current_focus.is_empty() {
            memory.summary =
                "No data available yet. Run some sessions and commits to build memory.".to_string();
        }

        // Copy all items into the flattened items list
        memory.items = Vec::new();
        for item in &memory.decisions {
            memory.items.push(item.clone());
        }
        for item in &memory.known_risks {
            memory.items.push(item.clone());
        }
        for item in &memory.failed_attempts {
            memory.items.push(item.clone());
        }
        for item in &memory.conventions {
            memory.items.push(item.clone());
        }
        for item in &memory.open_questions {
            memory.items.push(item.clone());
        }

        Ok(memory)
    }

    /// Format the memory as a readable string.
    pub fn show(&self) -> String {
        let mut out = String::new();

        out.push_str("╔══════════════════════════════════════════╗\n");
        out.push_str("║         Project Memory                   ║\n");
        out.push_str("╚══════════════════════════════════════════╝\n\n");

        out.push_str(&format!("Summary: {}\n", self.summary));
        out.push_str(&format!("Architecture: {}\n", self.current_architecture));
        out.push_str(&format!("Current Focus: {}\n", self.current_focus));
        out.push_str(&format!(
            "Version: {}, Updated: {}\n\n",
            self.version, self.updated_at
        ));

        if !self.decisions.is_empty() {
            out.push_str("── Decisions ──\n");
            for item in &self.decisions {
                out.push_str(&format!(
                    "  [{}] {} (conf: {:.2})\n",
                    item.id, item.content, item.confidence
                ));
            }
            out.push('\n');
        }

        if !self.known_risks.is_empty() {
            out.push_str("── Known Risks ──\n");
            for item in &self.known_risks {
                out.push_str(&format!(
                    "  [{}] {} (conf: {:.2})\n",
                    item.id, item.content, item.confidence
                ));
            }
            out.push('\n');
        }

        if !self.failed_attempts.is_empty() {
            out.push_str("── Failed Attempts ──\n");
            for item in &self.failed_attempts {
                out.push_str(&format!(
                    "  [{}] {} (conf: {:.2})\n",
                    item.id, item.content, item.confidence
                ));
            }
            out.push('\n');
        }

        if !self.conventions.is_empty() {
            out.push_str("── Conventions ──\n");
            for item in &self.conventions {
                out.push_str(&format!(
                    "  [{}] {} (conf: {:.2})\n",
                    item.id, item.content, item.confidence
                ));
            }
            out.push('\n');
        }

        if !self.open_questions.is_empty() {
            out.push_str("── Open Questions ──\n");
            for item in &self.open_questions {
                out.push_str(&format!(
                    "  [{}] {} (conf: {:.2})\n",
                    item.id, item.content, item.confidence
                ));
            }
            out.push('\n');
        }

        out
    }

    /// Diff this memory against another, returning a human-readable description
    /// of the differences.
    pub fn diff(&self, other: &ProjectMemory) -> String {
        let mut out = String::new();
        out.push_str("Memory Diff\n");
        out.push_str("===========\n\n");

        if self.summary != other.summary {
            out.push_str(&format!(
                "Summary changed:\n  was: {}\n  now: {}\n\n",
                self.summary, other.summary
            ));
        }

        if self.current_architecture != other.current_architecture {
            out.push_str(&format!(
                "Architecture changed:\n  was: {}\n  now: {}\n\n",
                self.current_architecture, other.current_architecture
            ));
        }

        if self.current_focus != other.current_focus {
            out.push_str(&format!(
                "Focus changed:\n  was: {}\n  now: {}\n\n",
                self.current_focus, other.current_focus
            ));
        }

        let self_item_ids: Vec<&str> = self.items.iter().map(|i| i.id.as_str()).collect();
        let other_item_ids: Vec<&str> = other.items.iter().map(|i| i.id.as_str()).collect();

        let new_items: Vec<&MemoryItem> = other
            .items
            .iter()
            .filter(|i| !self_item_ids.contains(&i.id.as_str()))
            .collect();

        let removed_items: Vec<&MemoryItem> = self
            .items
            .iter()
            .filter(|i| !other_item_ids.contains(&i.id.as_str()))
            .collect();

        if !new_items.is_empty() {
            out.push_str(&format!("New items ({}):\n", new_items.len()));
            for item in &new_items {
                out.push_str(&format!(
                    "  [+] [{}] {} ({})\n",
                    item.kind, item.content, item.id
                ));
            }
            out.push('\n');
        }

        if !removed_items.is_empty() {
            out.push_str(&format!("Removed items ({}):\n", removed_items.len()));
            for item in &removed_items {
                out.push_str(&format!(
                    "  [-] [{}] {} ({})\n",
                    item.kind, item.content, item.id
                ));
            }
            out.push('\n');
        }

        let version_diff = other.version as i64 - self.version as i64;
        if version_diff != 0 {
            out.push_str(&format!(
                "Version: {} → {} ({:+})\n",
                self.version, other.version, version_diff
            ));
        }

        if out == "Memory Diff\n===========\n\n" {
            out.push_str("No differences found.\n");
        }

        out
    }

    /// Mark an item as superseded by a new one.
    ///
    /// The old item's kind is changed to `Historical`, and it is linked to the
    /// new item via `superseded_by` / `supersedes` fields.
    pub fn supersede_item(&mut self, old_id: &str, new_id: &str) -> Result<()> {
        // First check that both items exist
        let new_exists = self.items.iter().any(|i| i.id == new_id);
        if !new_exists {
            anyhow::bail!("no memory item with id '{}'", new_id);
        }

        let old_idx = self
            .items
            .iter()
            .position(|i| i.id == old_id)
            .ok_or_else(|| anyhow::anyhow!("no memory item with id '{}'", old_id))?;

        let old = &mut self.items[old_idx];
        old.kind = MemoryItemKind::Historical;
        old.superseded_by = Some(new_id.to_string());

        // Update the new item's supersedes field
        if let Some(new_item) = self.items.iter_mut().find(|i| i.id == new_id) {
            new_item.supersedes = Some(old_id.to_string());
        }

        Ok(())
    }

    /// Return all items across both the flattened `items` vec and the
    /// separate category fields (decisions, known_risks, etc.).
    ///
    /// This ensures that `explain_why` and `diff` find items even when
    /// `items` was not populated (e.g. after loading from an older save).
    pub fn all_items(&self) -> Vec<&MemoryItem> {
        let mut all: Vec<&MemoryItem> = self.items.iter().collect();

        // Deduplicate by id so we don't return the same item twice
        let ids_in_items: std::collections::BTreeSet<&str> =
            self.items.iter().map(|i| i.id.as_str()).collect();

        for item in self
            .decisions
            .iter()
            .chain(&self.known_risks)
            .chain(&self.failed_attempts)
            .chain(&self.conventions)
            .chain(&self.open_questions)
        {
            if !ids_in_items.contains(item.id.as_str()) {
                all.push(item);
            }
        }

        all
    }

    /// Explain why a topic is the way it is — trace the decision/source graph.
    ///
    /// Scans memory items for topic keywords and builds a chain of reasoning
    /// from decisions, failed attempts, and evidence.
    pub fn explain_why(&self, topic: &str) -> Result<MemoryWhyExplanation> {
        let topic_lower = topic.to_lowercase();

        // Find all items related to the topic (search both `items` and
        // the separate category fields)
        let all = self.all_items();
        let related: Vec<&MemoryItem> = all
            .into_iter()
            .filter(|item| {
                item.content.to_lowercase().contains(&topic_lower)
                    || item
                        .source_ids
                        .iter()
                        .any(|s| s.to_lowercase().contains(&topic_lower))
            })
            .collect();

        if related.is_empty() {
            anyhow::bail!("no memory items found for topic '{}'", topic);
        }

        // Sort by confidence (highest first) for chain ordering
        let mut sorted = related.clone();
        sorted.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut decision_chain: Vec<MemoryWhyLink> = Vec::new();
        let mut alternatives: Vec<String> = Vec::new();
        let mut evidence: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for item in &sorted {
            // Collect evidence from source IDs
            for sid in &item.source_ids {
                evidence.insert(sid.clone());
            }

            match item.kind {
                MemoryItemKind::Decision | MemoryItemKind::Invariant => {
                    decision_chain.push(MemoryWhyLink {
                        item_id: item.id.clone(),
                        kind: format!("{}", item.kind),
                        content: item.content.clone(),
                        source_ids: item.source_ids.clone(),
                        confidence: item.confidence,
                    });
                }
                MemoryItemKind::FailedAttempt => {
                    alternatives.push(item.content.clone());
                }
                _ => {}
            }
        }

        // Follow superseded_by chain to include superseding items
        let mut chained_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for item in &sorted {
            if let Some(ref by) = item.superseded_by {
                chained_ids.insert(by.clone());
            }
            if let Some(ref sup) = item.supersedes {
                chained_ids.insert(sup.clone());
            }
        }
        for cid in &chained_ids {
            if let Some(item) = self.items.iter().find(|i| i.id == *cid) {
                let already_in = decision_chain.iter().any(|l| l.item_id == item.id);
                if !already_in {
                    decision_chain.push(MemoryWhyLink {
                        item_id: item.id.clone(),
                        kind: format!("{}", item.kind),
                        content: item.content.clone(),
                        source_ids: item.source_ids.clone(),
                        confidence: item.confidence,
                    });
                }
            }
        }

        // Determine current state: prefer a Current-kind item, otherwise the
        // highest-confidence decision / invariant.
        let current_state = related
            .iter()
            .find(|item| item.kind == MemoryItemKind::Current)
            .map(|item| item.content.clone())
            .or_else(|| {
                sorted
                    .first()
                    .filter(|item| {
                        matches!(
                            item.kind,
                            MemoryItemKind::Decision | MemoryItemKind::Invariant
                        )
                    })
                    .map(|item| item.content.clone())
            })
            .unwrap_or_else(|| {
                sorted
                    .first()
                    .map(|item| item.content.clone())
                    .unwrap_or_default()
            });

        let evidence: Vec<String> = evidence.into_iter().collect();

        Ok(MemoryWhyExplanation {
            topic: topic.to_string(),
            current_state,
            decision_chain,
            alternatives_considered: alternatives,
            evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_empty() {
        let store = MemoryStore::default();
        assert!(store.memories.is_empty());
        assert!(store.current.is_none());
    }

    #[test]
    fn test_memory_paths() {
        let root = Path::new("/tmp/project");
        assert_eq!(memory_dir(&root), Path::new("/tmp/project/.route/memory"));
        assert_eq!(
            memory_path(&root),
            Path::new("/tmp/project/.route/memory/memory.json")
        );
        assert_eq!(
            history_path(&root),
            Path::new("/tmp/project/.route/memory/history")
        );
    }

    #[test]
    fn test_memory_show_format() {
        let mut memory = ProjectMemory::new();
        memory.summary = "Test project".to_string();
        memory.decisions.push(MemoryItem {
            id: "d1".to_string(),
            kind: MemoryItemKind::Decision,
            content: "Use Rust".to_string(),
            source_ids: vec![],
            confidence: 0.9,
            updated_at: 1000,
            created_at: 1000,
            superseded_by: None,
            supersedes: None,
            tags: vec![],
        });

        let output = memory.show();
        assert!(output.contains("Test project"));
        assert!(output.contains("Use Rust"));
        assert!(output.contains("0.9"));
    }

    #[test]
    fn test_memory_diff_identical() {
        let a = ProjectMemory::new();
        let b = ProjectMemory::new();
        let diff = a.diff(&b);
        assert!(diff.contains("No differences found"));
    }

    #[test]
    fn test_memory_diff_different() {
        let mut a = ProjectMemory::new();
        a.summary = "Version A".to_string();

        let mut b = ProjectMemory::new();
        b.summary = "Version B".to_string();
        b.items.push(MemoryItem {
            id: "new1".to_string(),
            kind: MemoryItemKind::Decision,
            content: "New item".to_string(),
            source_ids: vec![],
            confidence: 0.5,
            updated_at: 2000,
            created_at: 2000,
            superseded_by: None,
            supersedes: None,
            tags: vec![],
        });

        let diff = a.diff(&b);
        assert!(diff.contains("Summary changed"));
        assert!(diff.contains("Version A"));
        assert!(diff.contains("Version B"));
        assert!(diff.contains("New item"));
    }

    #[test]
    fn test_memory_store_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let mut store = MemoryStore::default();
        let mut memory = ProjectMemory::new();
        memory.summary = "Persisted".to_string();
        memory.decisions.push(MemoryItem {
            id: "p1".to_string(),
            kind: MemoryItemKind::Decision,
            content: "Persist with JSON".to_string(),
            source_ids: vec![],
            confidence: 0.8,
            updated_at: 3000,
            created_at: 3000,
            superseded_by: None,
            supersedes: None,
            tags: vec![],
        });
        store.memories.push(memory);
        store.current = Some("Persisted".to_string());

        store.save(root).unwrap();

        let loaded = MemoryStore::load(root).unwrap();
        assert_eq!(loaded.memories.len(), 1);
        assert_eq!(loaded.memories[0].summary, "Persisted");
        assert_eq!(loaded.current.as_deref(), Some("Persisted"));
    }

    #[test]
    fn test_current_memory_fallback() {
        let mut store = MemoryStore::default();
        assert!(store.current_memory().is_none());

        let memory = ProjectMemory::new();
        store.memories.push(memory);
        assert!(store.current_memory().is_some());
    }

    #[test]
    fn test_refresh_empty_project() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let memory = ProjectMemory::refresh(root, None).unwrap();
        // Should at least have a default summary
        assert!(!memory.summary.is_empty());
        assert_eq!(memory.version, 1);
    }
}
