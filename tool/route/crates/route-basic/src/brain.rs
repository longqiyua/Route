//! Project Brain — knowledge consolidation from all development history.
//!
//! Brain is a DERIVED VIEW (not SSOT) that compresses savepoints, trajectories,
//! decisions, failures, patterns, goals, guardian findings, and studies into a
//! compact, task-relevant knowledge base for AI consumption.
//!
//! Principles:
//!   - Original history is NEVER deleted.
//!   - Brain is always regenerable from raw data.
//!   - Every brain item has source_ids, confidence, status, updated_at.
//!   - Conflicts are preserved, not papered over.
//!   - Current vs Historical vs Archived are explicitly separated.
//!
//! Data stored in `.route/brain/`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR as DOT_DIR;

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn brain_dir(project_root: &Path) -> PathBuf {
    project_root.join(DOT_DIR).join("brain")
}

fn brain_current_path(project_root: &Path) -> PathBuf {
    brain_dir(project_root).join("current.json")
}

fn brain_history_path(project_root: &Path) -> PathBuf {
    brain_dir(project_root).join("history.json")
}

fn brain_conflicts_path(project_root: &Path) -> PathBuf {
    brain_dir(project_root).join("conflicts.json")
}

// ---------------------------------------------------------------------------
// P0: BrainItem — a single piece of knowledge in the brain
// ---------------------------------------------------------------------------

/// Status of a brain item in the current view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrainItemStatus {
    /// Actively relevant — AI should know this
    Current,
    /// Explains past behavior but no longer active
    Historical,
    /// Low value but traceable
    Archived,
}

impl Default for BrainItemStatus {
    fn default() -> Self {
        Self::Current
    }
}

/// A single piece of knowledge in the Project Brain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainItem {
    /// Stable ID (ULID)
    pub id: String,
    /// Category: "architecture" | "invariant" | "decision" | "risk" | "failure" | "pattern" | "goal" | "open_loop" | "capability" | "workflow_lesson" | "agent_lesson" | "change" | "unknown"
    pub category: String,
    /// Short, actionable statement
    pub statement: String,
    /// Why this is true / relevant
    pub rationale: String,
    /// Scope this applies to (module, workflow, pattern, etc.)
    pub scope: String,
    /// Exceptions or caveats
    pub exceptions: Vec<String>,
    /// IDs of source artifacts (trajectory, decision, failure, evidence, etc.)
    pub source_ids: Vec<String>,
    /// 0.0 - 1.0
    pub confidence: f64,
    /// Current / Historical / Archived
    #[serde(default)]
    pub status: BrainItemStatus,
    /// ID of item that superseded this one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// ID of item this supersedes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unix-millis when created
    pub created_at: i64,
    /// Unix-millis when last updated
    pub updated_at: i64,
}

// ---------------------------------------------------------------------------
// P3: BrainKnowledge — compressed knowledge from multiple sources
// ---------------------------------------------------------------------------

/// Compressed knowledge derived from multiple source records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainKnowledge {
    /// Stable ID
    pub id: String,
    /// Short statement of the knowledge
    pub statement: String,
    /// Scope of applicability
    pub scope: String,
    /// Why this knowledge exists
    pub why: String,
    /// Exceptions or caveats
    pub exceptions: Vec<String>,
    /// 0.0 - 1.0
    pub confidence: f64,
    /// Number of source records this was compressed from
    pub evidence_count: u32,
    /// Source IDs
    pub source_ids: Vec<String>,
    /// How many were explicit user decisions
    pub explicit_user_decisions: u32,
    /// How many were verified evidence
    pub verified_evidence_count: u32,
    /// Created at
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// P4: KnowledgeConflict
// ---------------------------------------------------------------------------

/// A detected contradiction between two brain items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeConflict {
    /// Stable ID
    pub id: String,
    /// Claim A
    pub claim_a: String,
    /// Claim B
    pub claim_b: String,
    /// Source IDs for A
    pub sources_a: Vec<String>,
    /// Source ID's for B
    pub sources_b: Vec<String>,
    /// Scope of the conflict
    pub scope: String,
    /// Status: "unresolved" | "resolved" | "context_dependent"
    #[serde(default)]
    pub status: String,
    /// Resolution explanation (if resolved)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    /// Created at
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// P0: ProjectBrain
// ---------------------------------------------------------------------------

/// The complete Project Brain — a derived view of all development knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBrain {
    /// Brain version (monotonic)
    pub version: u32,
    /// A short summary of the project
    pub project_summary: String,
    /// Current architecture items
    pub current_architecture: Vec<BrainItem>,
    /// Critical invariants that must hold
    pub critical_invariants: Vec<BrainItem>,
    /// Active decisions in play
    pub active_decisions: Vec<BrainItem>,
    /// Known risks and their mitigations
    pub known_risks: Vec<BrainItem>,
    /// Recurring failure patterns
    pub recurring_failures: Vec<BrainItem>,
    /// Proven patterns that work
    pub proven_patterns: Vec<BrainItem>,
    /// Current goals and priorities
    pub current_goals: Vec<BrainItem>,
    /// Active open loops / unresolved questions
    pub active_open_loops: Vec<BrainItem>,
    /// Important capabilities the project has
    pub important_capabilities: Vec<BrainItem>,
    /// Workflow lessons learned
    pub workflow_lessons: Vec<BrainItem>,
    /// Agent organization lessons
    pub agent_lessons: Vec<BrainItem>,
    /// Recent major changes
    pub recent_major_changes: Vec<BrainItem>,
    /// Known unknowns / gaps in knowledge
    pub unknowns: Vec<BrainItem>,
    /// All items flattened (for iteration)
    pub items: Vec<BrainItem>,
    /// Compressed knowledge items (P3)
    pub knowledge: Vec<BrainKnowledge>,
    /// When this brain was created
    pub created_at: i64,
    /// When this brain was last updated
    pub updated_at: i64,
    /// Savepoint ID this brain is associated with (P10, optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub savepoint_id: Option<String>,
}

impl ProjectBrain {
    /// Create an empty brain.
    pub fn new() -> Self {
        let now = now_millis();
        Self {
            version: 1,
            project_summary: String::new(),
            current_architecture: Vec::new(),
            critical_invariants: Vec::new(),
            active_decisions: Vec::new(),
            known_risks: Vec::new(),
            recurring_failures: Vec::new(),
            proven_patterns: Vec::new(),
            current_goals: Vec::new(),
            active_open_loops: Vec::new(),
            important_capabilities: Vec::new(),
            workflow_lessons: Vec::new(),
            agent_lessons: Vec::new(),
            recent_major_changes: Vec::new(),
            unknowns: Vec::new(),
            items: Vec::new(),
            knowledge: Vec::new(),
            created_at: now,
            updated_at: now,
            savepoint_id: None,
        }
    }

    /// Return all items across all categories.
    pub fn all_items(&self) -> Vec<&BrainItem> {
        let mut all: Vec<&BrainItem> = self.items.iter().collect();
        let ids: HashSet<&str> = self.items.iter().map(|i| i.id.as_str()).collect();
        for item in self
            .current_architecture
            .iter()
            .chain(&self.critical_invariants)
            .chain(&self.active_decisions)
            .chain(&self.known_risks)
            .chain(&self.recurring_failures)
            .chain(&self.proven_patterns)
            .chain(&self.current_goals)
            .chain(&self.active_open_loops)
            .chain(&self.important_capabilities)
            .chain(&self.workflow_lessons)
            .chain(&self.agent_lessons)
            .chain(&self.recent_major_changes)
            .chain(&self.unknowns)
        {
            if !ids.contains(item.id.as_str()) {
                all.push(item);
            }
        }
        all
    }

    /// Get an item by ID.
    pub fn get_item(&self, id: &str) -> Option<&BrainItem> {
        self.all_items().into_iter().find(|i| i.id == id)
    }

    /// Get a knowledge item by ID.
    pub fn get_knowledge(&self, id: &str) -> Option<&BrainKnowledge> {
        self.knowledge.iter().find(|k| k.id == id)
    }
}

impl Default for ProjectBrain {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// P1: BrainProposal
// ---------------------------------------------------------------------------

/// A proposal to modify the current brain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainProposal {
    /// Stable ID
    pub id: String,
    /// Action: "add" | "update" | "supersede" | "archive"
    pub action: String,
    /// Category of the proposed change
    pub category: String,
    /// The proposed brain item (for add/update)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<BrainItem>,
    /// ID of item to supersede/archive (for supersede/archive)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    /// Reason for the proposal
    pub reason: String,
    /// Source IDs supporting this proposal
    pub source_ids: Vec<String>,
    /// Status: "open" | "applied" | "rejected"
    #[serde(default)]
    pub status: String,
    /// Created at
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// P9: BrainDiff
// ---------------------------------------------------------------------------

/// A diff between two brain versions.
#[derive(Debug, Clone)]
pub struct BrainDiff {
    pub version_a: u32,
    pub version_b: u32,
    pub new_facts: Vec<String>,
    pub superseded_decisions: Vec<String>,
    pub resolved_risks: Vec<String>,
    pub established_patterns: Vec<String>,
    pub disappeared_unknowns: Vec<String>,
    pub new_unknowns: Vec<String>,
    pub changed_confidence: Vec<(String, f64, f64)>,
}

// ---------------------------------------------------------------------------
// P13: BrainDoctorReport
// ---------------------------------------------------------------------------

/// A quality check report for the brain.
#[derive(Debug, Clone)]
pub struct BrainDoctorReport {
    pub unsupported_claims: Vec<UnsupportedClaim>,
    pub missing_sources: Vec<MissingSource>,
    pub stale_current_items: Vec<StaleItem>,
    pub contradictions: Vec<KnowledgeConflict>,
    pub duplicates: Vec<DuplicateItem>,
    pub orphan_sources: Vec<OrphanSource>,
    pub over_broad_patterns: Vec<OverBroadPattern>,
    pub unknown_provenance: Vec<UnknownProvenance>,
}

#[derive(Debug, Clone)]
pub struct UnsupportedClaim {
    pub item_id: String,
    pub statement: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct MissingSource {
    pub item_id: String,
    pub source_id: String,
}

#[derive(Debug, Clone)]
pub struct StaleItem {
    pub item_id: String,
    pub category: String,
    pub days_since_update: i64,
}

#[derive(Debug, Clone)]
pub struct DuplicateItem {
    pub item_a: String,
    pub item_b: String,
    pub similarity: String,
}

#[derive(Debug, Clone)]
pub struct OrphanSource {
    pub source_id: String,
    pub referenced_by: String,
}

#[derive(Debug, Clone)]
pub struct OverBroadPattern {
    pub item_id: String,
    pub statement: String,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct UnknownProvenance {
    pub item_id: String,
    pub statement: String,
}

// ---------------------------------------------------------------------------
// P6: Layered disclosure types
// ---------------------------------------------------------------------------

/// L0 Brief — the absolute minimum AI needs to know.
#[derive(Debug, Clone)]
pub struct BrainBrief {
    pub project_summary: String,
    pub critical_invariants: Vec<String>,
    pub current_goals: Vec<String>,
    pub active_decisions: Vec<String>,
    pub known_risks: Vec<String>,
    pub items_count: usize,
    pub version: u32,
    pub updated_at: i64,
}

/// L1 Relevant Knowledge — task-specific brain items.
#[derive(Debug, Clone)]
pub struct RelevantKnowledge {
    pub invariants: Vec<BrainItem>,
    pub decisions: Vec<BrainItem>,
    pub failures: Vec<BrainItem>,
    pub patterns: Vec<BrainItem>,
    pub capabilities: Vec<BrainItem>,
    pub constraints: Vec<BrainItem>,
    pub open_questions: Vec<BrainItem>,
}

// ---------------------------------------------------------------------------
// P1: Consolidation — refresh brain from all sources
// ---------------------------------------------------------------------------

/// Refresh the brain by consolidating all available data sources.
///
/// Returns a `Vec<BrainProposal>` that can be reviewed and applied.
/// Does NOT directly modify the current brain.
pub fn refresh_brain(project_root: &Path) -> Result<Vec<BrainProposal>> {
    let mut proposals: Vec<BrainProposal> = Vec::new();
    let now = now_millis();

    let mut proposal_id = 0u64;
    let mut next_id = || {
        proposal_id += 1;
        format!("bp-{}-{}", now, proposal_id)
    };

    // --- 1. Scan ProjectMemory (memory.rs) ---
    if let Ok(mem_store) = crate::memory::MemoryStore::load(project_root) {
        if let Some(mem) = mem_store.current_memory() {
            // Summary
            if !mem.summary.is_empty() {
                proposals.push(BrainProposal {
                    id: next_id(),
                    action: "add".to_string(),
                    category: "project_summary".to_string(),
                    item: Some(BrainItem {
                        id: next_id(),
                        category: "project_summary".to_string(),
                        statement: mem.summary.clone(),
                        rationale: "Derived from ProjectMemory.refresh".to_string(),
                        scope: "project".to_string(),
                        exceptions: vec![],
                        source_ids: vec!["memory::current".to_string()],
                        confidence: 0.7,
                        status: BrainItemStatus::Current,
                        superseded_by: None,
                        supersedes: None,
                        tags: vec!["memory".to_string()],
                        created_at: mem.updated_at,
                        updated_at: mem.updated_at,
                    }),
                    target_id: None,
                    reason: "Consolidated from ProjectMemory summary".to_string(),
                    source_ids: vec!["memory::current".to_string()],
                    status: "open".to_string(),
                    created_at: now,
                });
            }

            // Decisions
            for d in &mem.decisions {
                proposals.push(BrainProposal {
                    id: next_id(),
                    action: "add".to_string(),
                    category: "active_decisions".to_string(),
                    item: Some(BrainItem {
                        id: next_id(),
                        category: "decision".to_string(),
                        statement: d.content.clone(),
                        rationale: "From ProjectMemory decisions".to_string(),
                        scope: "project".to_string(),
                        exceptions: vec![],
                        source_ids: d.source_ids.clone(),
                        confidence: d.confidence,
                        status: if d.kind == crate::memory::MemoryItemKind::Historical {
                            BrainItemStatus::Historical
                        } else {
                            BrainItemStatus::Current
                        },
                        superseded_by: d.superseded_by.clone(),
                        supersedes: d.supersedes.clone(),
                        tags: d.tags.clone(),
                        created_at: d.created_at,
                        updated_at: d.updated_at,
                    }),
                    target_id: None,
                    reason: "Consolidated from ProjectMemory".to_string(),
                    source_ids: d.source_ids.clone(),
                    status: "open".to_string(),
                    created_at: now,
                });
            }

            // Known risks
            for r in &mem.known_risks {
                proposals.push(BrainProposal {
                    id: next_id(),
                    action: "add".to_string(),
                    category: "known_risks".to_string(),
                    item: Some(BrainItem {
                        id: next_id(),
                        category: "risk".to_string(),
                        statement: r.content.clone(),
                        rationale: "From ProjectMemory known risks".to_string(),
                        scope: "project".to_string(),
                        exceptions: vec![],
                        source_ids: r.source_ids.clone(),
                        confidence: r.confidence,
                        status: BrainItemStatus::Current,
                        superseded_by: None,
                        supersedes: None,
                        tags: r.tags.clone(),
                        created_at: r.created_at,
                        updated_at: r.updated_at,
                    }),
                    target_id: None,
                    reason: "Consolidated from ProjectMemory".to_string(),
                    source_ids: r.source_ids.clone(),
                    status: "open".to_string(),
                    created_at: now,
                });
            }

            // Open questions
            for q in &mem.open_questions {
                proposals.push(BrainProposal {
                    id: next_id(),
                    action: "add".to_string(),
                    category: "active_open_loops".to_string(),
                    item: Some(BrainItem {
                        id: next_id(),
                        category: "open_loop".to_string(),
                        statement: q.content.clone(),
                        rationale: "Open question from ProjectMemory".to_string(),
                        scope: "project".to_string(),
                        exceptions: vec![],
                        source_ids: q.source_ids.clone(),
                        confidence: q.confidence,
                        status: BrainItemStatus::Current,
                        superseded_by: None,
                        supersedes: None,
                        tags: q.tags.clone(),
                        created_at: q.created_at,
                        updated_at: q.updated_at,
                    }),
                    target_id: None,
                    reason: "Consolidated from ProjectMemory".to_string(),
                    source_ids: q.source_ids.clone(),
                    status: "open".to_string(),
                    created_at: now,
                });
            }
        }
    }

    // --- 2. Scan Trajectories (trajectory.rs) ---
    if let Ok(traj_store) = crate::trajectory::TrajectoryStore::load(project_root) {
        let mut all_decisions: Vec<String> = Vec::new();
        let mut all_failures: Vec<String> = Vec::new();
        let mut success_count = 0u32;
        let mut rollback_count = 0u32;

        for t in &traj_store.trajectories {
            // Collect decisions
            for d in &t.decisions {
                let key = format!("{}:{}", d.description, d.rationale);
                if !all_decisions.contains(&key) {
                    all_decisions.push(key.clone());
                    proposals.push(BrainProposal {
                        id: next_id(),
                        action: "add".to_string(),
                        category: "active_decisions".to_string(),
                        item: Some(BrainItem {
                            id: next_id(),
                            category: "decision".to_string(),
                            statement: format!("{} — {}", d.description, d.rationale),
                            rationale: "From trajectory decision record".to_string(),
                            scope: d.module.clone().unwrap_or_else(|| "unknown".to_string()),
                            exceptions: vec![],
                            source_ids: vec![t.id.clone(), d.id.clone()],
                            confidence: 0.6,
                            status: BrainItemStatus::Current,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec!["trajectory".to_string()],
                            created_at: d.created_at,
                            updated_at: d.created_at,
                        }),
                        target_id: None,
                        reason: "Extracted from trajectory".to_string(),
                        source_ids: vec![t.id.clone()],
                        status: "open".to_string(),
                        created_at: now,
                    });
                }
            }

            // Collect failures
            for f in &t.failures {
                let key = f.description.clone();
                if !all_failures.contains(&key) {
                    all_failures.push(key);
                    proposals.push(BrainProposal {
                        id: next_id(),
                        action: "add".to_string(),
                        category: "recurring_failures".to_string(),
                        item: Some(BrainItem {
                            id: next_id(),
                            category: "failure".to_string(),
                            statement: format!(
                                "[{}] {} — {}",
                                f.kind,
                                f.description,
                                f.resolution.as_deref().unwrap_or("unresolved")
                            ),
                            rationale: "From trajectory failure record".to_string(),
                            scope: f.module.clone().unwrap_or_else(|| "unknown".to_string()),
                            exceptions: vec![],
                            source_ids: vec![t.id.clone(), f.id.clone()],
                            confidence: 0.7,
                            status: BrainItemStatus::Current,
                            superseded_by: None,
                            supersedes: None,
                            tags: vec!["trajectory".to_string(), f.kind.clone()],
                            created_at: f.created_at,
                            updated_at: f.created_at,
                        }),
                        target_id: None,
                        reason: "Extracted from trajectory".to_string(),
                        source_ids: vec![t.id.clone()],
                        status: "open".to_string(),
                        created_at: now,
                    });
                }
            }

            // Track outcome counts
            match t.outcome.as_str() {
                "success" => success_count += 1,
                "rolled_back" => rollback_count += 1,
                _ => {}
            }
        }

        // Workflow lessons from trajectory outcomes
        if rollback_count > 0 && success_count > 0 {
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "workflow_lessons".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "workflow_lesson".to_string(),
                    statement: format!(
                        "{} trajectories: {} succeeded, {} rolled back. Rollback rate: {:.1}%",
                        success_count + rollback_count,
                        success_count,
                        rollback_count,
                        (rollback_count as f64 / (success_count + rollback_count).max(1) as f64)
                            * 100.0
                    ),
                    rationale: "Learned from trajectory outcomes".to_string(),
                    scope: "workflow".to_string(),
                    exceptions: vec![],
                    source_ids: traj_store
                        .trajectories
                        .iter()
                        .map(|t| t.id.clone())
                        .collect(),
                    confidence: 0.8,
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: vec!["trajectory".to_string(), "workflow".to_string()],
                    created_at: now,
                    updated_at: now,
                }),
                target_id: None,
                reason: "Aggregated from trajectory outcomes".to_string(),
                source_ids: traj_store
                    .trajectories
                    .iter()
                    .map(|t| t.id.clone())
                    .collect(),
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    // --- 3. Scan Failure Library (failure.rs) ---
    if let Ok(fail_lib) = crate::failure::FailureLibrary::load(project_root) {
        for f in &fail_lib.cases {
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "recurring_failures".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "failure".to_string(),
                    statement: format!(
                        "{} — root: {} — resolved: {}",
                        f.problem,
                        f.root_cause.as_deref().unwrap_or("unknown"),
                        f.resolved
                    ),
                    rationale: "From FailureLibrary".to_string(),
                    scope: f.affected_scope.join(", "),
                    exceptions: vec![],
                    source_ids: vec![f.id.clone()],
                    confidence: if f.resolved { 0.9 } else { 0.7 },
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: f.tags.clone(),
                    created_at: f.created_at,
                    updated_at: f.resolved_at.unwrap_or(f.created_at),
                }),
                target_id: None,
                reason: "Consolidated from FailureLibrary".to_string(),
                source_ids: vec![f.id.clone()],
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    // --- 4. Scan Patterns (pattern.rs) ---
    if let Ok(pattern_store) = crate::pattern::PatternStore::load(project_root) {
        for p in &pattern_store.patterns {
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "proven_patterns".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "pattern".to_string(),
                    statement: format!(
                        "{}: {} — when: {:?} — avoid: {:?}",
                        p.name, p.solution, p.when_to_use, p.when_not_to_use
                    ),
                    rationale: p.problem.clone(),
                    scope: "project".to_string(),
                    exceptions: p.when_not_to_use.clone(),
                    source_ids: p.evidence_sources.clone(),
                    confidence: p.confidence,
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: p.tags.clone(),
                    created_at: p.created_at,
                    updated_at: p.updated_at,
                }),
                target_id: None,
                reason: "Consolidated from PatternStore".to_string(),
                source_ids: p.evidence_sources.clone(),
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    // --- 5. Scan Goals (goal.rs) ---
    if let Ok(goal_store) = crate::goal::GoalStore::load(project_root) {
        for g in &goal_store.goals {
            if g.status == crate::goal::GoalStatus::Done
                || g.status == crate::goal::GoalStatus::Abandoned
            {
                continue;
            }
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "current_goals".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "goal".to_string(),
                    statement: format!(
                        "[P{}/{}] {} — {}",
                        g.priority,
                        if g.status == crate::goal::GoalStatus::Active {
                            "active"
                        } else {
                            "paused"
                        },
                        g.title,
                        g.description.as_deref().unwrap_or("")
                    ),
                    rationale: "From GoalStore".to_string(),
                    scope: "project".to_string(),
                    exceptions: vec![],
                    source_ids: vec![g.id.clone()],
                    confidence: 0.9,
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: g.tags.clone(),
                    created_at: g.created_at,
                    updated_at: g.updated_at,
                }),
                target_id: None,
                reason: "Consolidated from GoalStore".to_string(),
                source_ids: vec![g.id.clone()],
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    // --- 6. Scan Guardian Findings (guardian.rs) ---
    if let Ok(guard_store) = crate::guardian::GuardianFindingsStore::load(project_root) {
        let open_findings: Vec<_> = guard_store
            .findings
            .iter()
            .filter(|f| f.status == crate::guardian::FindingStatus::Open)
            .collect();
        for f in &open_findings {
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "active_open_loops".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "open_loop".to_string(),
                    statement: format!("[{}] {} — {}", f.kind, f.title, f.explanation),
                    rationale: "From Guardian findings".to_string(),
                    scope: f.affected_scope.join(", "),
                    exceptions: vec![],
                    source_ids: f.evidence_ids.clone(),
                    confidence: severity_to_confidence(&f.severity),
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: vec!["guardian".to_string(), f.kind.clone()],
                    created_at: f.created_at,
                    updated_at: f.updated_at,
                }),
                target_id: None,
                reason: "Consolidated from Guardian".to_string(),
                source_ids: f.evidence_ids.clone(),
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    // --- 7. Scan Studies (study.rs) ---
    if let Ok(study_lib) = crate::study::study_library_load(project_root) {
        for s in &study_lib.studies {
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "important_capabilities".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "capability".to_string(),
                    statement: format!("Study: {} — architecture: {}", s.summary, s.architecture),
                    rationale: "From StudyLibrary".to_string(),
                    scope: "project".to_string(),
                    exceptions: vec![],
                    source_ids: vec![s.id.clone()],
                    confidence: 0.8,
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: vec!["study".to_string()],
                    created_at: s.created_at,
                    updated_at: s.created_at,
                }),
                target_id: None,
                reason: "Consolidated from StudyLibrary".to_string(),
                source_ids: vec![s.id.clone()],
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    // --- 8. Scan Learning Proposals (learn.rs) ---
    if let Ok(exp_store) = crate::learn::ExperienceStore::load(project_root) {
        for event in &exp_store.events {
            if matches!(
                event.kind,
                crate::learn::EventKind::TestFail | crate::learn::EventKind::UserReject
            ) {
                proposals.push(BrainProposal {
                    id: next_id(),
                    action: "add".to_string(),
                    category: "recurring_failures".to_string(),
                    item: Some(BrainItem {
                        id: next_id(),
                        category: "failure".to_string(),
                        statement: format!(
                            "[{}] {} — {}",
                            event.kind.as_str(),
                            event.outcome,
                            event.evidence
                        ),
                        rationale: "From ExperienceEvent".to_string(),
                        scope: "project".to_string(),
                        exceptions: vec![],
                        source_ids: vec![event.id.clone()],
                        confidence: 0.6,
                        status: BrainItemStatus::Current,
                        superseded_by: None,
                        supersedes: None,
                        tags: vec!["experience".to_string(), event.kind.as_str().to_string()],
                        created_at: event.created_at,
                        updated_at: event.created_at,
                    }),
                    target_id: None,
                    reason: "Consolidated from ExperienceEvent".to_string(),
                    source_ids: vec![event.id.clone()],
                    status: "open".to_string(),
                    created_at: now,
                });
            }
        }
    }

    // --- 9. Scan Execution Evidence (execution.rs) ---
    if let Ok(ev_store) = crate::execution::EvidenceStore::load(project_root) {
        let rollback_count = ev_store
            .evidence
            .iter()
            .filter(|e| matches!(e.kind, crate::execution::EvidenceKind::Rollback))
            .count();
        if rollback_count > 0 {
            proposals.push(BrainProposal {
                id: next_id(),
                action: "add".to_string(),
                category: "known_risks".to_string(),
                item: Some(BrainItem {
                    id: next_id(),
                    category: "risk".to_string(),
                    statement: format!(
                        "{} rollback events recorded in evidence ledger",
                        rollback_count
                    ),
                    rationale: "From Execution EvidenceStore".to_string(),
                    scope: "project".to_string(),
                    exceptions: vec![],
                    source_ids: ev_store
                        .evidence
                        .iter()
                        .filter(|e| matches!(e.kind, crate::execution::EvidenceKind::Rollback))
                        .map(|e| e.id.clone())
                        .collect(),
                    confidence: 0.8,
                    status: BrainItemStatus::Current,
                    superseded_by: None,
                    supersedes: None,
                    tags: vec!["execution".to_string(), "rollback".to_string()],
                    created_at: now,
                    updated_at: now,
                }),
                target_id: None,
                reason: "Consolidated from Execution Evidence".to_string(),
                source_ids: vec![],
                status: "open".to_string(),
                created_at: now,
            });
        }
    }

    Ok(proposals)
}

// ---------------------------------------------------------------------------
// P1: Apply proposals to create a new brain version
// ---------------------------------------------------------------------------

/// Apply a set of proposals to produce a new brain version.
/// Returns the resulting brain.
pub fn apply_brain_proposals(
    current: &ProjectBrain,
    proposals: &[BrainProposal],
) -> Result<ProjectBrain> {
    let mut brain = current.clone();
    let now = now_millis();

    brain.version += 1;
    brain.updated_at = now;

    for p in proposals {
        if p.status == "rejected" {
            continue;
        }

        match p.action.as_str() {
            "add" => {
                if let Some(ref item) = p.item {
                    let mut item = item.clone();
                    // Assign a stable ID
                    item.id = format!("bi-{}-{}", now, brain.items.len() + 1);
                    item.updated_at = now;
                    brain.items.push(item.clone());

                    // Also add to the relevant category
                    match item.category.as_str() {
                        "project_summary" => brain.project_summary = item.statement.clone(),
                        "architecture" => brain.current_architecture.push(item),
                        "invariant" => brain.critical_invariants.push(item),
                        "decision" => brain.active_decisions.push(item),
                        "risk" => brain.known_risks.push(item),
                        "failure" => brain.recurring_failures.push(item),
                        "pattern" => brain.proven_patterns.push(item),
                        "goal" => brain.current_goals.push(item),
                        "open_loop" => brain.active_open_loops.push(item),
                        "capability" => brain.important_capabilities.push(item),
                        "workflow_lesson" => brain.workflow_lessons.push(item),
                        "agent_lesson" => brain.agent_lessons.push(item),
                        "change" => brain.recent_major_changes.push(item),
                        "unknown" => brain.unknowns.push(item),
                        _ => {}
                    }
                }
            }
            "supersede" => {
                if let Some(ref target_id) = p.target_id {
                    // Mark old item as historical
                    for item in &mut brain.items {
                        if item.id == *target_id {
                            item.status = BrainItemStatus::Historical;
                            item.superseded_by = p.item.as_ref().map(|i| i.id.clone());
                        }
                    }
                    // Add new item
                    if let Some(ref item) = p.item {
                        let mut item = item.clone();
                        item.id = format!("bi-{}-{}", now, brain.items.len() + 1);
                        item.supersedes = Some(target_id.clone());
                        brain.items.push(item.clone());
                    }
                }
            }
            "archive" => {
                if let Some(ref target_id) = p.target_id {
                    for item in &mut brain.items {
                        if item.id == *target_id {
                            item.status = BrainItemStatus::Archived;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Deduplicate by statement
    brain = deduplicate_brain(brain);

    Ok(brain)
}

/// Deduplicate brain items by statement (keeping the higher confidence one).
fn deduplicate_brain(brain: ProjectBrain) -> ProjectBrain {
    let mut brain = brain;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut keep: Vec<bool> = vec![true; brain.items.len()];

    for (i, item) in brain.items.iter().enumerate() {
        let key = item.statement.trim().to_lowercase();
        if let Some(&prev_idx) = seen.get(&key) {
            if item.confidence > brain.items[prev_idx].confidence {
                keep[prev_idx] = false;
                seen.insert(key, i);
            } else {
                keep[i] = false;
            }
        } else {
            seen.insert(key, i);
        }
    }

    let mut deduped: Vec<BrainItem> = Vec::new();
    for (i, item) in brain.items.drain(..).enumerate() {
        if keep[i] {
            deduped.push(item);
        }
    }
    brain.items = deduped;
    brain
}

// ---------------------------------------------------------------------------
// P2: Current vs Historical separation
// ---------------------------------------------------------------------------

/// Get only current (non-historical, non-archived) items.
pub fn current_items(brain: &ProjectBrain) -> Vec<&BrainItem> {
    brain
        .all_items()
        .into_iter()
        .filter(|i| i.status == BrainItemStatus::Current)
        .collect()
}

/// Get only historical items.
pub fn historical_items(brain: &ProjectBrain) -> Vec<&BrainItem> {
    brain
        .all_items()
        .into_iter()
        .filter(|i| i.status == BrainItemStatus::Historical)
        .collect()
}

/// Get only archived items.
pub fn archived_items(brain: &ProjectBrain) -> Vec<&BrainItem> {
    brain
        .all_items()
        .into_iter()
        .filter(|i| i.status == BrainItemStatus::Archived)
        .collect()
}

// ---------------------------------------------------------------------------
// P3: Knowledge Compression
// ---------------------------------------------------------------------------

/// Compress multiple brain items on the same topic into a single knowledge entry.
pub fn compress_knowledge(brain: &ProjectBrain, topic: &str) -> BrainKnowledge {
    let now = now_millis();
    let all = brain.all_items();
    let related: Vec<&&BrainItem> = all
        .iter()
        .filter(|i| {
            i.statement.to_lowercase().contains(&topic.to_lowercase())
                || i.tags
                    .iter()
                    .any(|t| t.to_lowercase().contains(&topic.to_lowercase()))
        })
        .collect();

    let evidence_count = related.len() as u32;
    let explicit_user = related.iter().filter(|i| i.confidence >= 0.8).count() as u32;
    let verified = related.iter().filter(|i| i.confidence >= 0.7).count() as u32;

    let source_ids: Vec<String> = related.iter().flat_map(|i| i.source_ids.clone()).collect();

    let confidences: Vec<f64> = related.iter().map(|i| i.confidence).collect();
    let avg_confidence = if confidences.is_empty() {
        0.0
    } else {
        confidences.iter().sum::<f64>() / confidences.len() as f64
    };

    BrainKnowledge {
        id: format!("bk-{}-{}", now, topic.len()),
        statement: format!("{}: {} items compressed", topic, evidence_count),
        scope: "project".to_string(),
        why: format!("Compressed from {} related brain items", evidence_count),
        exceptions: vec![],
        confidence: avg_confidence,
        evidence_count,
        source_ids,
        explicit_user_decisions: explicit_user,
        verified_evidence_count: verified,
        created_at: now,
    }
}

// ---------------------------------------------------------------------------
// P4: Contradiction Detection
// ---------------------------------------------------------------------------

/// Detect contradictions between brain items.
pub fn detect_contradictions(brain: &ProjectBrain) -> Vec<KnowledgeConflict> {
    let mut conflicts = Vec::new();
    let now = now_millis();
    let all = brain.all_items();

    // Simple heuristic: items with opposite confidence (high vs low) on same topic
    let mut by_topic: HashMap<String, Vec<&BrainItem>> = HashMap::new();
    for item in &all {
        let words: Vec<&str> = item.statement.split_whitespace().collect();
        for w in words {
            if w.len() > 4 {
                by_topic.entry(w.to_lowercase()).or_default().push(item);
            }
        }
    }

    for (topic, items) in &by_topic {
        if items.len() < 2 {
            continue;
        }
        // Check for opposite claims
        for i in 0..items.len() {
            for j in i + 1..items.len() {
                let a = items[i];
                let b = items[j];
                if a.confidence > 0.7 && b.confidence < 0.3 {
                    conflicts.push(KnowledgeConflict {
                        id: format!("kc-{}-{}", now, conflicts.len()),
                        claim_a: a.statement.clone(),
                        claim_b: b.statement.clone(),
                        sources_a: a.source_ids.clone(),
                        sources_b: b.source_ids.clone(),
                        scope: topic.clone(),
                        status: "unresolved".to_string(),
                        resolution: None,
                        created_at: now,
                    });
                }
            }
        }
    }

    conflicts
}

// ---------------------------------------------------------------------------
// P5: Task-Specific Brain
// ---------------------------------------------------------------------------

/// Build a task-specific brain view, filtering only relevant items.
pub fn task_brain(brain: &ProjectBrain, task: &str) -> RelevantKnowledge {
    let task_lower = task.to_lowercase();
    let task_words: HashSet<&str> = task_lower.split_whitespace().collect();

    let matches = |item: &BrainItem| -> bool {
        let s = item.statement.to_lowercase();
        let t = item
            .tags
            .iter()
            .any(|t| task_lower.contains(t.as_str()) || t.to_lowercase().contains(&task_lower));
        let c = item.category.to_lowercase().contains(&task_lower);
        let w = task_words.iter().any(|w| s.contains(w));
        w || t || c
    };

    RelevantKnowledge {
        invariants: brain
            .critical_invariants
            .iter()
            .filter(|i| matches(i))
            .cloned()
            .collect(),
        decisions: brain
            .active_decisions
            .iter()
            .filter(|d| matches(d))
            .cloned()
            .collect(),
        failures: brain
            .recurring_failures
            .iter()
            .filter(|f| matches(f))
            .cloned()
            .collect(),
        patterns: brain
            .proven_patterns
            .iter()
            .filter(|p| matches(p))
            .cloned()
            .collect(),
        capabilities: brain
            .important_capabilities
            .iter()
            .filter(|c| matches(c))
            .cloned()
            .collect(),
        constraints: {
            let mut c = brain
                .critical_invariants
                .iter()
                .filter(|i| matches(i))
                .cloned()
                .collect::<Vec<_>>();
            c.extend(brain.known_risks.iter().filter(|r| matches(r)).cloned());
            c
        },
        open_questions: brain
            .active_open_loops
            .iter()
            .filter(|o| matches(o))
            .cloned()
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// P6: Layered Disclosure
// ---------------------------------------------------------------------------

/// L0 Brief — the absolute minimum the AI needs to know about the project.
pub fn brain_brief(brain: &ProjectBrain) -> BrainBrief {
    BrainBrief {
        project_summary: brain.project_summary.clone(),
        critical_invariants: brain
            .critical_invariants
            .iter()
            .map(|i| i.statement.clone())
            .collect(),
        current_goals: brain
            .current_goals
            .iter()
            .map(|g| g.statement.clone())
            .collect(),
        active_decisions: brain
            .active_decisions
            .iter()
            .map(|d| d.statement.clone())
            .collect(),
        known_risks: brain
            .known_risks
            .iter()
            .map(|r| r.statement.clone())
            .collect(),
        items_count: brain.items.len(),
        version: brain.version,
        updated_at: brain.updated_at,
    }
}

/// L2 Expand — show full details of a brain item with source chain.
pub fn expand_item(brain: &ProjectBrain, id: &str) -> Option<String> {
    let item = brain.get_item(id)?;
    let mut out = String::new();

    out.push_str(&format!("Item: {}\n", item.id));
    out.push_str(&format!("Category: {}\n", item.category));
    out.push_str(&format!("Statement: {}\n", item.statement));
    out.push_str(&format!("Rationale: {}\n", item.rationale));
    out.push_str(&format!("Scope: {}\n", item.scope));
    out.push_str(&format!("Confidence: {:.2}\n", item.confidence));
    out.push_str(&format!("Status: {:?}\n", item.status));
    out.push_str(&format!("Created: {}\n", item.created_at));
    out.push_str(&format!("Updated: {}\n", item.updated_at));
    if !item.exceptions.is_empty() {
        out.push_str(&format!("Exceptions: {}\n", item.exceptions.join(", ")));
    }
    if !item.source_ids.is_empty() {
        out.push_str(&format!("Sources: {}\n", item.source_ids.join(", ")));
    }
    if let Some(ref by) = item.superseded_by {
        out.push_str(&format!("Superseded by: {}\n", by));
    }
    if let Some(ref sup) = item.supersedes {
        out.push_str(&format!("Supersedes: {}\n", sup));
    }
    if !item.tags.is_empty() {
        out.push_str(&format!("Tags: {}\n", item.tags.join(", ")));
    }

    Some(out)
}

// ---------------------------------------------------------------------------
// P9: Brain Diff
// ---------------------------------------------------------------------------

/// Diff two project brains to show what changed in project understanding.
pub fn diff_brain(a: &ProjectBrain, b: &ProjectBrain) -> BrainDiff {
    let a_items = a.all_items();
    let b_items = b.all_items();

    let a_ids: HashSet<&str> = a_items.iter().map(|i| i.id.as_str()).collect();
    let b_ids: HashSet<&str> = b_items.iter().map(|i| i.id.as_str()).collect();

    let a_map: HashMap<&str, &BrainItem> = a_items.iter().map(|i| (i.id.as_str(), *i)).collect();
    let b_map: HashMap<&str, &BrainItem> = b_items.iter().map(|i| (i.id.as_str(), *i)).collect();

    // New facts (items in B not in A)
    let new_facts: Vec<String> = b_items
        .iter()
        .filter(|i| !a_ids.contains(i.id.as_str()) && i.status == BrainItemStatus::Current)
        .map(|i| {
            format!(
                "[{}] {} (conf: {:.2})",
                i.category, i.statement, i.confidence
            )
        })
        .collect();

    // Superseded decisions (items in A that are historical in B)
    let superseded_decisions: Vec<String> = b_items
        .iter()
        .filter(|i| a_ids.contains(i.id.as_str()) && i.status == BrainItemStatus::Historical)
        .map(|i| format!("[{}] {} → superseded", i.category, i.statement))
        .collect();

    // Resolved risks (risks in A not in B as current)
    let resolved_risks: Vec<String> = a_items
        .iter()
        .filter(|i| i.category == "risk" && !b_ids.contains(i.id.as_str()))
        .map(|i| format!("{}", i.statement))
        .collect();

    // Established patterns
    let established_patterns: Vec<String> = b_items
        .iter()
        .filter(|i| i.category == "pattern" && !a_ids.contains(i.id.as_str()))
        .map(|i| format!("{} (conf: {:.2})", i.statement, i.confidence))
        .collect();

    // Disappeared unknowns
    let disappeared_unknowns: Vec<String> = a_items
        .iter()
        .filter(|i| i.category == "unknown" && !b_ids.contains(i.id.as_str()))
        .map(|i| i.statement.clone())
        .collect();

    // New unknowns
    let new_unknowns: Vec<String> = b_items
        .iter()
        .filter(|i| i.category == "unknown" && !a_ids.contains(i.id.as_str()))
        .map(|i| i.statement.clone())
        .collect();

    // Changed confidence
    let mut changed_confidence = Vec::new();
    for (id, a_item) in &a_map {
        if let Some(b_item) = b_map.get(id) {
            if (a_item.confidence - b_item.confidence).abs() > 0.1 {
                changed_confidence.push((
                    a_item.statement.clone(),
                    a_item.confidence,
                    b_item.confidence,
                ));
            }
        }
    }

    BrainDiff {
        version_a: a.version,
        version_b: b.version,
        new_facts,
        superseded_decisions,
        resolved_risks,
        established_patterns,
        disappeared_unknowns,
        new_unknowns,
        changed_confidence,
    }
}

// ---------------------------------------------------------------------------
// P10: Brain at Savepoint
// ---------------------------------------------------------------------------

/// Load the brain that was current at a given savepoint.
pub fn brain_at_savepoint(project_root: &Path, savepoint_id: &str) -> Result<Option<ProjectBrain>> {
    // Brain history is stored in `.route/brain/history.json`
    let history_path = brain_history_path(project_root);
    if !history_path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&history_path)
        .with_context(|| format!("reading brain history from {}", history_path.display()))?;
    let brains: Vec<ProjectBrain> = serde_json::from_str(&raw)?;

    // Find the brain closest to the savepoint
    // First, try exact match on savepoint_id
    if let Some(brain) = brains
        .iter()
        .find(|b| b.savepoint_id.as_deref() == Some(savepoint_id))
    {
        return Ok(Some(brain.clone()));
    }

    // Fallback: find the latest brain created before the savepoint
    if let Ok(sp_store) = crate::savepoint::SavepointStore::load(project_root) {
        if let Some(sp) = sp_store.get(savepoint_id) {
            let brain = brains
                .iter()
                .filter(|b| b.created_at <= sp.created_at)
                .max_by_key(|b| b.created_at);
            return Ok(brain.cloned());
        }
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// P13: Brain Doctor
// ---------------------------------------------------------------------------

/// Run quality checks on the brain.
pub fn brain_doctor(brain: &ProjectBrain, project_root: &Path) -> BrainDoctorReport {
    let now = now_millis();
    let one_day_ms = 86_400_000;
    let thirty_days_ms = 30 * one_day_ms;

    let all = brain.all_items();

    // Unsupported claims: items with confidence >= 0.9 but no source_ids
    let unsupported_claims: Vec<UnsupportedClaim> = all
        .iter()
        .filter(|i| i.confidence >= 0.9 && i.source_ids.is_empty())
        .map(|i| UnsupportedClaim {
            item_id: i.id.clone(),
            statement: i.statement.clone(),
            reason: "High confidence but no source IDs".to_string(),
        })
        .collect();

    // Missing sources: items referencing non-existent source IDs
    let mut missing_sources = Vec::new();
    for item in &all {
        for sid in &item.source_ids {
            // Check if source file exists
            let source_path = project_root
                .join(DOT_DIR)
                .join("trajectory")
                .join(format!("{}.json", sid));
            if !source_path.exists() {
                missing_sources.push(MissingSource {
                    item_id: item.id.clone(),
                    source_id: sid.clone(),
                });
            }
        }
    }

    // Stale current items: items marked Current but not updated in 30 days
    let stale_current_items: Vec<StaleItem> = all
        .iter()
        .filter(|i| i.status == BrainItemStatus::Current && (now - i.updated_at) > thirty_days_ms)
        .map(|i| StaleItem {
            item_id: i.id.clone(),
            category: i.category.clone(),
            days_since_update: (now - i.updated_at) / one_day_ms,
        })
        .collect();

    // Contradictions: delegate to detect_contradictions
    let contradictions = detect_contradictions(brain);

    // Duplicates: items with very similar statements
    let mut duplicates = Vec::new();
    for i in 0..all.len() {
        for j in i + 1..all.len() {
            let a = all[i].statement.to_lowercase();
            let b = all[j].statement.to_lowercase();
            if a == b {
                duplicates.push(DuplicateItem {
                    item_a: all[i].id.clone(),
                    item_b: all[j].id.clone(),
                    similarity: "exact".to_string(),
                });
            } else if a.contains(&b) || b.contains(&a) {
                duplicates.push(DuplicateItem {
                    item_a: all[i].id.clone(),
                    item_b: all[j].id.clone(),
                    similarity: "partial".to_string(),
                });
            }
        }
    }

    // Over-broad patterns: patterns with no exceptions/scope
    let over_broad_patterns: Vec<OverBroadPattern> = all
        .iter()
        .filter(|i| i.category == "pattern" && i.exceptions.is_empty() && i.scope == "project")
        .map(|i| OverBroadPattern {
            item_id: i.id.clone(),
            statement: i.statement.clone(),
            reason: "Pattern has no exceptions or scope — may be too broad".to_string(),
        })
        .collect();

    // Unknown provenance: items with no source_ids and no tags
    let unknown_provenance: Vec<UnknownProvenance> = all
        .iter()
        .filter(|i| i.source_ids.is_empty() && i.tags.is_empty())
        .map(|i| UnknownProvenance {
            item_id: i.id.clone(),
            statement: i.statement.clone(),
        })
        .collect();

    BrainDoctorReport {
        unsupported_claims,
        missing_sources,
        stale_current_items,
        contradictions,
        duplicates,
        orphan_sources: Vec::new(), // requires scanning all source files
        over_broad_patterns,
        unknown_provenance,
    }
}

// ---------------------------------------------------------------------------
// P7: Brain Compact — rebuild indices without deleting data
// ---------------------------------------------------------------------------

/// Compact the brain: re-derive summary, deduplicate, re-index.
/// Does NOT delete any original data.
pub fn compact_brain(brain: &ProjectBrain) -> ProjectBrain {
    let mut brain = brain.clone();
    brain.version += 1;
    brain.updated_at = now_millis();

    // Deduplicate
    brain = deduplicate_brain(brain);

    // Rebuild summary
    let current = current_items(&brain);
    brain.project_summary = format!(
        "{} items: {} current, {} historical, {} archived. {} categories.",
        brain.items.len(),
        current.len(),
        historical_items(&brain).len(),
        archived_items(&brain).len(),
        count_categories(&brain).len(),
    );

    brain
}

// ---------------------------------------------------------------------------
// P8: Knowledge Promotion
// ---------------------------------------------------------------------------

/// Check if a brain item has enough evidence to be promoted.
pub fn can_promote(brain: &ProjectBrain, item_id: &str) -> Option<String> {
    let item = brain.get_item(item_id)?;
    let _current = current_items(brain).len();
    let total = brain.items.len();

    Some(format!(
        "Item '{}' ({}): confidence={:.2}, sources={}, status={:?}, total_items={}",
        item_id,
        item.category,
        item.confidence,
        item.source_ids.len(),
        item.status,
        total
    ))
}

// ---------------------------------------------------------------------------
// P11: Cross-Project Boundary
// ---------------------------------------------------------------------------

/// Check that a brain item doesn't contain cross-project references.
pub fn validate_project_boundary(brain: &ProjectBrain, project_name: &str) -> Vec<String> {
    let mut violations = Vec::new();
    for item in brain.all_items() {
        for sid in &item.source_ids {
            if sid.contains("project:") && !sid.contains(project_name) {
                violations.push(format!(
                    "Item '{}' references external project source: {}",
                    item.id, sid
                ));
            }
        }
    }
    violations
}

// ---------------------------------------------------------------------------
// P12: Host Context Formatting
// ---------------------------------------------------------------------------

/// Format the brain as a compact host context string for AI injection.
pub fn format_host_context(brain: &ProjectBrain, task: Option<&str>) -> String {
    let mut out = String::new();

    // Header
    out.push_str("PROJECT BRAIN BRIEF\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    out.push_str(&format!(
        "Version: {}, Items: {}\n\n",
        brain.version,
        brain.items.len()
    ));

    // Summary
    out.push_str(&format!("Summary: {}\n\n", brain.project_summary));

    // Critical invariants
    if !brain.critical_invariants.is_empty() {
        out.push_str("CRITICAL INVARIANTS\n");
        for item in &brain.critical_invariants {
            out.push_str(&format!("  • [{}] {}\n", item.id, item.statement));
        }
        out.push('\n');
    }

    // Task-relevant knowledge (if task provided)
    if let Some(task_str) = task {
        let relevant = task_brain(brain, task_str);
        let has_relevant = !relevant.invariants.is_empty()
            || !relevant.decisions.is_empty()
            || !relevant.failures.is_empty()
            || !relevant.patterns.is_empty()
            || !relevant.capabilities.is_empty()
            || !relevant.open_questions.is_empty();

        if has_relevant {
            out.push_str("TASK-RELEVANT KNOWLEDGE\n");
            for item in &relevant.invariants {
                out.push_str(&format!(
                    "  • Invariant [{}]: {}\n",
                    item.id, item.statement
                ));
            }
            for item in &relevant.decisions {
                out.push_str(&format!("  • Decision [{}]: {}\n", item.id, item.statement));
            }
            for item in &relevant.failures {
                out.push_str(&format!("  • Failure [{}]: {}\n", item.id, item.statement));
            }
            for item in &relevant.patterns {
                out.push_str(&format!("  • Pattern [{}]: {}\n", item.id, item.statement));
            }
            for item in &relevant.capabilities {
                out.push_str(&format!(
                    "  • Capability [{}]: {}\n",
                    item.id, item.statement
                ));
            }
            for item in &relevant.open_questions {
                out.push_str(&format!("  • Open [{}]: {}\n", item.id, item.statement));
            }
            out.push('\n');
        }
    }

    // Known failures
    if !brain.recurring_failures.is_empty() {
        out.push_str("KNOWN FAILURES\n");
        for item in brain.recurring_failures.iter().take(5) {
            out.push_str(&format!("  • [{}] {}\n", item.id, item.statement));
        }
        if brain.recurring_failures.len() > 5 {
            out.push_str(&format!(
                "  • ... and {} more\n",
                brain.recurring_failures.len() - 5
            ));
        }
        out.push('\n');
    }

    // Open questions
    if !brain.active_open_loops.is_empty() {
        out.push_str("OPEN QUESTIONS\n");
        for item in brain.active_open_loops.iter().take(3) {
            out.push_str(&format!("  • [{}] {}\n", item.id, item.statement));
        }
        if brain.active_open_loops.len() > 3 {
            out.push_str(&format!(
                "  • ... and {} more\n",
                brain.active_open_loops.len() - 3
            ));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format the full brain for display.
pub fn format_brain(brain: &ProjectBrain) -> String {
    let mut out = String::new();

    out.push_str("╔══════════════════════════════════════════╗\n");
    out.push_str(&format!(
        "║         Project Brain v{}                  ║\n",
        brain.version
    ));
    out.push_str("╚══════════════════════════════════════════╝\n\n");

    out.push_str(&format!("Summary: {}\n", brain.project_summary));
    out.push_str(&format!("Items: {} total\n", brain.items.len()));
    out.push_str(&format!(
        "Knowledge: {} compressed\n",
        brain.knowledge.len()
    ));
    out.push_str(&format!("Updated: {}\n\n", brain.updated_at));

    write_category(&mut out, "Architecture", &brain.current_architecture);
    write_category(&mut out, "Critical Invariants", &brain.critical_invariants);
    write_category(&mut out, "Active Decisions", &brain.active_decisions);
    write_category(&mut out, "Known Risks", &brain.known_risks);
    write_category(&mut out, "Recurring Failures", &brain.recurring_failures);
    write_category(&mut out, "Proven Patterns", &brain.proven_patterns);
    write_category(&mut out, "Current Goals", &brain.current_goals);
    write_category(&mut out, "Open Loops", &brain.active_open_loops);
    write_category(&mut out, "Capabilities", &brain.important_capabilities);
    write_category(&mut out, "Workflow Lessons", &brain.workflow_lessons);
    write_category(&mut out, "Agent Lessons", &brain.agent_lessons);
    write_category(&mut out, "Recent Changes", &brain.recent_major_changes);
    write_category(&mut out, "Unknowns", &brain.unknowns);

    if !brain.knowledge.is_empty() {
        out.push_str("── Compressed Knowledge ──\n");
        for k in &brain.knowledge {
            out.push_str(&format!(
                "  [{}] {} (conf: {:.2}, sources: {})\n",
                &k.id[..16.min(k.id.len())],
                k.statement,
                k.confidence,
                k.evidence_count
            ));
        }
        out.push('\n');
    }

    out
}

fn write_category(out: &mut String, label: &str, items: &[BrainItem]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("── {} ({}) ──\n", label, items.len()));
    for item in items {
        let status_mark = match item.status {
            BrainItemStatus::Current => "",
            BrainItemStatus::Historical => " [HIST]",
            BrainItemStatus::Archived => " [ARCH]",
        };
        out.push_str(&format!(
            "  [{}] {}{}\n",
            &item.id[..8.min(item.id.len())],
            item.statement,
            status_mark
        ));
    }
    out.push('\n');
}

/// Format a brain brief for display.
pub fn format_brain_brief(brief: &BrainBrief) -> String {
    let mut out = String::new();

    out.push_str("PROJECT BRAIN BRIEF\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");
    out.push_str(&format!("Summary: {}\n\n", brief.project_summary));

    if !brief.critical_invariants.is_empty() {
        out.push_str("Critical Invariants:\n");
        for inv in &brief.critical_invariants {
            out.push_str(&format!("  • {}\n", inv));
        }
        out.push('\n');
    }

    if !brief.current_goals.is_empty() {
        out.push_str("Current Goals:\n");
        for g in &brief.current_goals {
            out.push_str(&format!("  • {}\n", g));
        }
        out.push('\n');
    }

    if !brief.active_decisions.is_empty() {
        out.push_str("Active Decisions:\n");
        for d in &brief.active_decisions {
            out.push_str(&format!("  • {}\n", d));
        }
        out.push('\n');
    }

    if !brief.known_risks.is_empty() {
        out.push_str("Known Risks:\n");
        for r in &brief.known_risks {
            out.push_str(&format!("  • {}\n", r));
        }
        out.push('\n');
    }

    out.push_str(&format!(
        "Version: {}, Items: {}\n",
        brief.version, brief.items_count
    ));
    out.push_str(&format!("Updated: {}\n", brief.updated_at));

    out
}

/// Format a brain diff for display.
pub fn format_brain_diff(diff: &BrainDiff) -> String {
    let mut out = String::new();

    out.push_str("Project Understanding Diff\n");
    out.push_str(&format!("v{} → v{}\n\n", diff.version_a, diff.version_b));

    if !diff.new_facts.is_empty() {
        out.push_str(&format!("New Facts ({}):\n", diff.new_facts.len()));
        for f in &diff.new_facts {
            out.push_str(&format!("  [+] {}\n", f));
        }
        out.push('\n');
    }

    if !diff.superseded_decisions.is_empty() {
        out.push_str(&format!(
            "Superseded Decisions ({}):\n",
            diff.superseded_decisions.len()
        ));
        for d in &diff.superseded_decisions {
            out.push_str(&format!("  [~] {}\n", d));
        }
        out.push('\n');
    }

    if !diff.resolved_risks.is_empty() {
        out.push_str(&format!(
            "Resolved Risks ({}):\n",
            diff.resolved_risks.len()
        ));
        for r in &diff.resolved_risks {
            out.push_str(&format!("  [-] {}\n", r));
        }
        out.push('\n');
    }

    if !diff.established_patterns.is_empty() {
        out.push_str(&format!(
            "Established Patterns ({}):\n",
            diff.established_patterns.len()
        ));
        for p in &diff.established_patterns {
            out.push_str(&format!("  [+] {}\n", p));
        }
        out.push('\n');
    }

    if !diff.disappeared_unknowns.is_empty() {
        out.push_str(&format!(
            "Disappeared Unknowns ({}):\n",
            diff.disappeared_unknowns.len()
        ));
        for u in &diff.disappeared_unknowns {
            out.push_str(&format!("  [-] {}\n", u));
        }
        out.push('\n');
    }

    if !diff.new_unknowns.is_empty() {
        out.push_str(&format!("New Unknowns ({}):\n", diff.new_unknowns.len()));
        for u in &diff.new_unknowns {
            out.push_str(&format!("  [?] {}\n", u));
        }
        out.push('\n');
    }

    if !diff.changed_confidence.is_empty() {
        out.push_str(&format!(
            "Changed Confidence ({}):\n",
            diff.changed_confidence.len()
        ));
        for (statement, old, new) in &diff.changed_confidence {
            out.push_str(&format!("  [Δ] '{}': {:.2} → {:.2}\n", statement, old, new));
        }
        out.push('\n');
    }

    if out
        == format!(
            "Project Understanding Diff\nv{} → v{}\n\n",
            diff.version_a, diff.version_b
        )
    {
        out.push_str("No changes detected.\n");
    }

    out
}

/// Format a doctor report for display.
pub fn format_doctor_report(report: &BrainDoctorReport) -> String {
    let mut out = String::new();

    out.push_str("Brain Doctor Report\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    out.push_str(&format!(
        "Unsupported Claims: {}\n",
        report.unsupported_claims.len()
    ));
    for c in &report.unsupported_claims {
        out.push_str(&format!(
            "  • [{}] {} — {}\n",
            c.item_id, c.statement, c.reason
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "Missing Sources: {}\n",
        report.missing_sources.len()
    ));
    for m in &report.missing_sources {
        out.push_str(&format!(
            "  • [{}] missing source: {}\n",
            m.item_id, m.source_id
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "Stale Current Items: {}\n",
        report.stale_current_items.len()
    ));
    for s in &report.stale_current_items {
        out.push_str(&format!(
            "  • [{}] ({}) last updated {} days ago\n",
            s.item_id, s.category, s.days_since_update
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "Contradictions: {}\n",
        report.contradictions.len()
    ));
    for c in &report.contradictions {
        out.push_str(&format!(
            "  • {} | {} | status: {}\n",
            c.claim_a, c.claim_b, c.status
        ));
    }
    out.push('\n');

    out.push_str(&format!("Duplicates: {}\n", report.duplicates.len()));
    for d in &report.duplicates {
        out.push_str(&format!(
            "  • {} ~ {} ({})\n",
            d.item_a, d.item_b, d.similarity
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "Orphan Sources: {}\n",
        report.orphan_sources.len()
    ));
    for o in &report.orphan_sources {
        out.push_str(&format!(
            "  • {} referenced by {}\n",
            o.source_id, o.referenced_by
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "Over-Broad Patterns: {}\n",
        report.over_broad_patterns.len()
    ));
    for p in &report.over_broad_patterns {
        out.push_str(&format!(
            "  • [{}] {} — {}\n",
            p.item_id, p.statement, p.reason
        ));
    }
    out.push('\n');

    out.push_str(&format!(
        "Unknown Provenance: {}\n",
        report.unknown_provenance.len()
    ));
    for u in &report.unknown_provenance {
        out.push_str(&format!("  • [{}] {}\n", u.item_id, u.statement));
    }
    out.push('\n');

    out
}

/// Format knowledge conflicts for display.
pub fn format_conflicts(conflicts: &[KnowledgeConflict]) -> String {
    let mut out = String::new();
    out.push_str("Knowledge Conflicts\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    if conflicts.is_empty() {
        out.push_str("No conflicts detected.\n");
        return out;
    }

    for c in conflicts {
        out.push_str(&format!("Conflict: {}\n", c.id));
        out.push_str(&format!(
            "  A: {} (sources: {})\n",
            c.claim_a,
            c.sources_a.join(", ")
        ));
        out.push_str(&format!(
            "  B: {} (sources: {})\n",
            c.claim_b,
            c.sources_b.join(", ")
        ));
        out.push_str(&format!("  Scope: {}\n", c.scope));
        out.push_str(&format!("  Status: {}\n", c.status));
        if let Some(ref res) = c.resolution {
            out.push_str(&format!("  Resolution: {}\n", res));
        }
        out.push('\n');
    }

    out
}

/// Format task-relevant knowledge for display.
pub fn format_relevant_knowledge(rk: &RelevantKnowledge) -> String {
    let mut out = String::new();
    out.push_str("TASK-RELEVANT KNOWLEDGE\n");
    out.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

    if !rk.invariants.is_empty() {
        out.push_str("Invariants:\n");
        for i in &rk.invariants {
            out.push_str(&format!("  • [{}] {}\n", i.id, i.statement));
        }
        out.push('\n');
    }

    if !rk.decisions.is_empty() {
        out.push_str("Decisions:\n");
        for d in &rk.decisions {
            out.push_str(&format!("  • [{}] {}\n", d.id, d.statement));
        }
        out.push('\n');
    }

    if !rk.failures.is_empty() {
        out.push_str("Failures:\n");
        for f in &rk.failures {
            out.push_str(&format!("  • [{}] {}\n", f.id, f.statement));
        }
        out.push('\n');
    }

    if !rk.patterns.is_empty() {
        out.push_str("Patterns:\n");
        for p in &rk.patterns {
            out.push_str(&format!("  • [{}] {}\n", p.id, p.statement));
        }
        out.push('\n');
    }

    if !rk.capabilities.is_empty() {
        out.push_str("Capabilities:\n");
        for c in &rk.capabilities {
            out.push_str(&format!("  • [{}] {}\n", c.id, c.statement));
        }
        out.push('\n');
    }

    if !rk.open_questions.is_empty() {
        out.push_str("Open Questions:\n");
        for q in &rk.open_questions {
            out.push_str(&format!("  • [{}] {}\n", q.id, q.statement));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Store types
// ---------------------------------------------------------------------------

/// Brain store — manages current brain and history.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrainStore {
    /// Current brain
    pub current: Option<ProjectBrain>,
    /// History of past brains
    pub history: Vec<ProjectBrain>,
    /// Knowledge conflicts
    pub conflicts: Vec<KnowledgeConflict>,
}

impl BrainStore {
    /// Load from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let mut store = BrainStore::default();

        // Load current brain
        let current_path = brain_current_path(project_root);
        if current_path.exists() {
            let raw = std::fs::read_to_string(&current_path)?;
            if !raw.trim().is_empty() {
                store.current = Some(serde_json::from_str(&raw)?);
            }
        }

        // Load history
        let history_path = brain_history_path(project_root);
        if history_path.exists() {
            let raw = std::fs::read_to_string(&history_path)?;
            if !raw.trim().is_empty() {
                store.history = serde_json::from_str(&raw)?;
            }
        }

        // Load conflicts
        let conflicts_path = brain_conflicts_path(project_root);
        if conflicts_path.exists() {
            let raw = std::fs::read_to_string(&conflicts_path)?;
            if !raw.trim().is_empty() {
                store.conflicts = serde_json::from_str(&raw)?;
            }
        }

        Ok(store)
    }

    /// Save to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = brain_dir(project_root);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating brain directory {}", dir.display()))?;

        // Save current brain
        if let Some(ref current) = self.current {
            let json = serde_json::to_string_pretty(current)?;
            std::fs::write(brain_current_path(project_root), &json)?;
        }

        // Save history
        let json = serde_json::to_string_pretty(&self.history)?;
        std::fs::write(brain_history_path(project_root), &json)?;

        // Save conflicts
        let json = serde_json::to_string_pretty(&self.conflicts)?;
        std::fs::write(brain_conflicts_path(project_root), &json)?;

        Ok(())
    }

    /// Apply proposals and create a new brain version.
    pub fn apply_proposals(&mut self, proposals: &[BrainProposal]) -> Result<()> {
        let current = self.current.clone().unwrap_or_default();

        // Push current to history before applying
        if self.current.is_some() {
            self.history.push(current.clone());
        }

        let new_brain = apply_brain_proposals(&current, proposals)?;
        self.current = Some(new_brain);
        Ok(())
    }

    /// Get brain history
    pub fn history(&self) -> &[ProjectBrain] {
        &self.history
    }

    /// Detect contradictions and store them
    pub fn detect_and_store_conflicts(&mut self) {
        if let Some(ref brain) = self.current {
            self.conflicts = detect_contradictions(brain);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn severity_to_confidence(severity: &crate::guardian::FindingSeverity) -> f64 {
    match severity {
        crate::guardian::FindingSeverity::Critical => 0.9,
        crate::guardian::FindingSeverity::Warning => 0.7,
        crate::guardian::FindingSeverity::Info => 0.5,
    }
}

fn count_categories(brain: &ProjectBrain) -> Vec<(&str, usize)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for item in brain.all_items() {
        *counts
            .entry(if item.category.is_empty() {
                "unknown"
            } else {
                &item.category
            })
            .or_insert(0) += 1;
    }
    let mut result: Vec<(&str, usize)> = counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result
}
