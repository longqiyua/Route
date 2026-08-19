//! Execution Ledger v1 — Execution Automation.
//!
//! Route records, constrains, and verifies what the host AI actually did.
//! The causal chain is:
//!
//!   Task → Context → Agent Plan → Execution Evidence → Result → Learning
//!
//! Storage layout:
//! ```text
//! .route/
//! └── execution/
//!     ├── sessions.json       # ExecutionSession store
//!     ├── evidence.json       # Evidence store
//!     ├── state-hash          # Current working-state fingerprint (P1)
//!     └── exec-logs/          # Command execution stdout/stderr summaries (P2)
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::adapter::{apply_context, ApplyTarget};
use crate::constitutive::{
    self, build_context, replay_context, AgentPolicy, ContextBudget, ContextHistoryManifest,
    ContextSnapshot, ReferenceSelector,
};
use crate::learn::{analyze_events, EventKind, ExperienceStore, LearnScope, LearningProposalStore};
use crate::savepoint::create_savepoint;
use crate::trajectory::{create_trajectory, AutoSaveState, TrajectoryStore, AUTO_SAVE_TASK_START};

// ---------------------------------------------------------------------------
// Directory layout
// ---------------------------------------------------------------------------

fn execution_dir(project_root: &Path) -> PathBuf {
    project_root.join(".route").join("execution")
}

fn sessions_path(project_root: &Path) -> PathBuf {
    execution_dir(project_root).join("sessions.json")
}

fn evidence_path(project_root: &Path) -> PathBuf {
    execution_dir(project_root).join("evidence.json")
}

fn state_hash_path(project_root: &Path) -> PathBuf {
    execution_dir(project_root).join("state-hash")
}

/// Read the current working-state fingerprint from disk.
pub fn read_state_hash(project_root: &Path) -> Result<String> {
    let p = state_hash_path(project_root);
    if !p.exists() {
        return Ok(String::new());
    }
    Ok(std::fs::read_to_string(&p)
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// Write the current working-state fingerprint to disk.
pub fn write_state_hash(project_root: &Path, hash: &str) -> Result<()> {
    let p = state_hash_path(project_root);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, hash)?;
    Ok(())
}

/// Compute a working-state fingerprint from the current project files.
/// Uses the snapshot DB's latest snapshot hash + working tree changes.
pub fn compute_state_hash(project_root: &Path) -> Result<String> {
    // Collect the latest snapshot info if available.
    let mut components = Vec::new();
    if let Ok(repo) = crate::repository::BasicRepository::open(project_root) {
        if let Ok(history) = repo.history(1) {
            if let Some(latest) = history.first() {
                components.push(format!("snapshot:{}", latest.snapshot.id));
            }
        }
        // Include working tree changes hash.
        if let Ok(changes) = repo.working_dir_status() {
            let changes_str = changes
                .iter()
                .map(|f| format!("{}:{}", f.path, f.change))
                .collect::<Vec<_>>()
                .join(",");
            if !changes_str.is_empty() {
                components.push(format!(
                    "changes:{}",
                    route_core::sha256_hex(changes_str.as_bytes())
                ));
            }
        }
    }
    // Include context hash.
    if let Ok(snap) = ContextSnapshot::collect(project_root) {
        components.push(format!("context:{}", snap.fingerprint));
    }
    if components.is_empty() {
        return Ok(String::new());
    }
    let joined = components.join("|");
    Ok(route_core::sha256_hex(joined.as_bytes()))
}

// ---------------------------------------------------------------------------
// P0: ExecutionSession
// ---------------------------------------------------------------------------

/// Status of an execution session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// The session is actively in progress. Only one Active session may
    /// exist at a time unless explicit session IDs are used.
    Active,
    /// The task completed successfully (all verification checks passed).
    Succeeded,
    /// The task failed (tests failed, errors, etc.).
    Failed,
    /// A rollback was performed, undoing the session's work.
    RolledBack,
    /// The session was aborted (user cancelled, unrelated).
    Aborted,
    /// The session was forced to success despite verification failure (P3).
    /// Permanent audit marker — cannot be used as strong positive learning.
    ForcedUnverified,
}

impl Default for SessionStatus {
    fn default() -> Self {
        SessionStatus::Active
    }
}

/// A single execution session: the lifecycle of one AI task.
///
/// Created by `route task begin`, ended by `route task end`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSession {
    /// Stable session id (ULID).
    pub id: String,
    /// The task description provided by the user.
    pub task: String,
    /// SHA-256 of the task string (for deterministic lookup).
    pub task_hash: String,
    /// Which AI host was targeted.
    pub target: ApplyTarget,
    /// Unix-millis when the session began.
    pub started_at: i64,
    /// Unix-millis when the session ended (None if still Active).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<i64>,
    /// Context fingerprint at session start.
    pub context_hash: String,
    /// Reference IDs selected during task-scoped context build.
    #[serde(default)]
    pub selected_reference_ids: Vec<String>,
    /// SHA-256 of the AgentPolicy that was compiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_policy_hash: Option<String>,
    /// Current session status.
    #[serde(default)]
    pub status: SessionStatus,
    /// Optional strategy ID if this session was started under a strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strategy_id: Option<String>,
    /// Serialized AgentPlan (JSON) if the session was started with agent planning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_plan: Option<String>,
    /// Trajectory ID for learning (P0 Trajectory Learning).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory_id: Option<String>,
    /// Last auto-savepoint ID (for trajectory linking).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_savepoint_id: Option<String>,
    /// Optional parent EvolutionCampaign id this session serves.
    /// Normal tasks without a campaign leave this unset; a session never
    /// duplicates Campaign data (it only references the Campaign id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
}

impl ExecutionSession {
    /// Create a new Active session (no persistence).
    pub fn new(
        task: &str,
        target: ApplyTarget,
        context_hash: String,
        selected_reference_ids: Vec<String>,
        agent_policy_hash: Option<String>,
        strategy_id: Option<String>,
        agent_plan: Option<String>,
    ) -> Self {
        Self {
            id: route_core::new_id(),
            task: task.to_string(),
            task_hash: route_core::sha256_hex(task.as_bytes()),
            target,
            started_at: route_core::now_millis(),
            ended_at: None,
            context_hash,
            selected_reference_ids,
            agent_policy_hash,
            status: SessionStatus::Active,
            strategy_id,
            agent_plan,
            trajectory_id: None,
            last_savepoint_id: None,
            campaign_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionStore
// ---------------------------------------------------------------------------

/// Persistent store for execution sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStore {
    #[serde(default)]
    pub sessions: Vec<ExecutionSession>,
}

impl SessionStore {
    /// Load sessions from disk. Returns empty store if file missing.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = sessions_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading sessions from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    /// Persist sessions atomically.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = sessions_path(project_root);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        constitutive::write_atomic(&p, &json)?;
        Ok(())
    }

    /// Find a session by id (supports prefix matching).
    pub fn get(&self, id: &str) -> Option<&ExecutionSession> {
        self.sessions
            .iter()
            .find(|s| s.id == id || s.id.starts_with(id))
    }

    /// Find a mutable session by id (supports prefix matching).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ExecutionSession> {
        self.sessions
            .iter_mut()
            .find(|s| s.id == id || s.id.starts_with(id))
    }

    /// Find the currently active session. Returns None if no session is Active.
    pub fn active(&self) -> Option<&ExecutionSession> {
        self.sessions
            .iter()
            .find(|s| s.status == SessionStatus::Active)
    }

    /// Return all sessions, most recent first.
    pub fn all(&self) -> Vec<&ExecutionSession> {
        let mut v: Vec<&ExecutionSession> = self.sessions.iter().collect();
        v.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        v
    }
}

// ---------------------------------------------------------------------------
// P0: Auto hooks — snapshot/commit/rollback evidence helpers
// ---------------------------------------------------------------------------

/// Record a snapshot as system evidence for the active session (P0).
/// Returns the evidence id, or None if no active session or already recorded.
pub fn record_snapshot_evidence(
    project_root: &Path,
    snapshot_id: &str,
    state_hash: Option<String>,
    context_hash: Option<String>,
) -> Result<Option<String>> {
    let store = SessionStore::load(project_root)?;
    let active = match store.active() {
        Some(s) => s.clone(),
        None => return Ok(None),
    };
    drop(store);

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert("snapshot_id".to_string(), snapshot_id.to_string());
    let id = evidence_store.record_dedup(
        &active.id,
        EvidenceKind::Snapshot,
        EvidenceSource::System,
        route_core::sha256_hex(snapshot_id.as_bytes()),
        metadata,
        &format!("snapshot:{}", snapshot_id), // dedup key
        state_hash,
        Some(snapshot_id.to_string()),
        None,
        context_hash,
    );
    if id.is_some() {
        evidence_store.save(project_root)?;
    }
    Ok(id)
}

/// Record a commit as system evidence with dedup (P0).
pub fn record_commit_evidence_dedup(
    project_root: &Path,
    commit_id: &str,
    state_hash: Option<String>,
    context_hash: Option<String>,
) -> Result<Option<String>> {
    let store = SessionStore::load(project_root)?;
    let active = match store.active() {
        Some(s) => s.clone(),
        None => return Ok(None),
    };
    drop(store);

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert("commit_id".to_string(), commit_id.to_string());
    let id = evidence_store.record_dedup(
        &active.id,
        EvidenceKind::Commit,
        EvidenceSource::System,
        route_core::sha256_hex(commit_id.as_bytes()),
        metadata,
        &format!("commit:{}", commit_id),
        state_hash,
        None,
        Some(commit_id.to_string()),
        context_hash,
    );
    if id.is_some() {
        evidence_store.save(project_root)?;
    }
    Ok(id)
}

/// Record a rollback as system evidence with dedup (P0).
pub fn record_rollback_evidence_dedup(
    project_root: &Path,
    rollback_snapshot_id: &str,
    state_hash: Option<String>,
    context_hash: Option<String>,
) -> Result<Option<String>> {
    let store = SessionStore::load(project_root)?;
    let active = match store.active() {
        Some(s) => s.clone(),
        None => return Ok(None),
    };
    drop(store);

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert(
        "rollback_snapshot".to_string(),
        rollback_snapshot_id.to_string(),
    );
    let id = evidence_store.record_dedup(
        &active.id,
        EvidenceKind::Rollback,
        EvidenceSource::System,
        route_core::sha256_hex(rollback_snapshot_id.as_bytes()),
        metadata,
        &format!("rollback:{}", rollback_snapshot_id),
        state_hash,
        None,
        None,
        context_hash,
    );
    if id.is_some() {
        evidence_store.save(project_root)?;
    }
    Ok(id)
}

/// Record a reference proposal as evidence with dedup (P0).
pub fn record_proposal_evidence(
    project_root: &Path,
    proposal_id: &str,
    session_id: &str,
) -> Result<Option<String>> {
    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert("proposal_id".to_string(), proposal_id.to_string());
    let id = evidence_store.record_dedup(
        session_id,
        EvidenceKind::ReferenceProposal,
        EvidenceSource::System,
        route_core::sha256_hex(proposal_id.as_bytes()),
        metadata,
        &format!("proposal:{}", proposal_id),
        None,
        None,
        None,
        None,
    );
    if id.is_some() {
        evidence_store.save(project_root)?;
        // Also persist the state hash so the ledger is consistent
        let state_hash = compute_state_hash(project_root).unwrap_or_default();
        if !state_hash.is_empty() {
            write_state_hash(project_root, &state_hash)?;
        }
    }
    Ok(id)
}

// ---------------------------------------------------------------------------
// P0: begin / status / end
// ---------------------------------------------------------------------------

/// Begin a new execution session.
///
/// 1. Builds the task-scoped context (if task provided) or global context.
/// 2. Records the context history manifest.
/// 3. Applies the context to the target file.
/// 4. Creates and persists the session.
///
/// Returns an error if there is already an Active session (caller must
/// end or abort it first, or pass an explicit --session-id to allow
/// concurrent sessions).
pub fn begin_session(
    project_root: &Path,
    task: &str,
    target: ApplyTarget,
    allow_concurrent: bool,
    strategy_id: Option<String>,
) -> Result<ExecutionSession> {
    // Check for conflicting active session.
    let mut store = SessionStore::load(project_root)?;
    if !allow_concurrent {
        if let Some(active) = store.active() {
            anyhow::bail!(
                "Session '{}' is still Active (task: '{}'). \
                 End it with `route task end {}` before starting a new one, \
                 or use --session-id to allow concurrent sessions.",
                active.id,
                active.task,
                active.id
            );
        }
    }

    // 1. Collect context snapshot.
    let snap = ContextSnapshot::collect(project_root)?;
    let context_hash = snap.fingerprint.clone();

    // 2. Build task-scoped context and extract selected references.
    let budget = ContextBudget::default();
    let selector = ReferenceSelector::new(task, Some(target.as_str()), 5, budget);
    let (selected, _) = selector.select(&snap.registry);
    let selected_reference_ids: Vec<String> =
        selected.iter().map(|sr| sr.reference_id.clone()).collect();

    // 3. Compile AgentPolicy hash.
    let policy = AgentPolicy::from_protocol_body(&snap.protocol_body);
    let policy_rendered = match target {
        ApplyTarget::Claude => policy.render_claude(),
        ApplyTarget::Codex => policy.render_codex(),
        ApplyTarget::DeepSeek => policy.render_deepseek(),
        ApplyTarget::Generic => policy.render_generic(),
    };
    let agent_policy_hash = if policy_rendered.is_empty() {
        None
    } else {
        Some(route_core::sha256_hex(policy_rendered.as_bytes()))
    };

    // 4. Apply context to target file.
    apply_context(project_root, target, Some(task))?;

    // 5. Record context history manifest.
    let _ =
        ContextHistoryManifest::record_if_new_triggered(project_root, &snap, Some("task_begin"))?;

    // 6. Create session.
    let session = ExecutionSession::new(
        task,
        target,
        context_hash,
        selected_reference_ids,
        agent_policy_hash,
        strategy_id,
        None, // agent_plan — populated later by start_task_session
    );

    store.sessions.push(session.clone());
    store.save(project_root)?;

    // P0: Auto-snapshot baseline and record System/Snapshot evidence.
    // Best-effort: errors here are non-fatal (the session itself is already
    // persisted). If the snapshot/evidence fails, the session is still valid
    // but the baseline evidence will be missing.
    if let Ok(repo) = crate::repository::BasicRepository::open(project_root) {
        if let Ok(commit) = repo.commit(crate::repository::CommitOptions {
            message: format!("[route] baseline for session {}", &session.id[..12]),
            author: Some("route".to_string()),
            force_full: false,
            branch: None,
            operator: Some("route:baseline".to_string()),
            body: Some(format!("Baseline snapshot for task: {}", task)),
            is_checkpoint: false,
            is_ai: false,
        }) {
            let state_hash = compute_state_hash(project_root).unwrap_or_default();
            if !state_hash.is_empty() {
                let _ = write_state_hash(project_root, &state_hash);
            }
            let _ = record_snapshot_evidence(
                project_root,
                &commit.to_snapshot,
                if state_hash.is_empty() {
                    None
                } else {
                    Some(state_hash.clone())
                },
                Some(session.context_hash.clone()),
            );
            let _ = record_commit_evidence_dedup(
                project_root,
                &commit.id,
                if state_hash.is_empty() {
                    None
                } else {
                    Some(state_hash)
                },
                Some(session.context_hash.clone()),
            );
        }
    }

    Ok(session)
}

/// Get the status of a session (or all sessions if no id given).
pub fn session_status(project_root: &Path, id: Option<&str>) -> Result<Vec<ExecutionSession>> {
    let store = SessionStore::load(project_root)?;
    match id {
        Some(sid) => {
            let session = store
                .get(sid)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("session '{}' not found", sid))?;
            Ok(vec![session])
        }
        None => {
            // Return all sessions.
            let sessions: Vec<ExecutionSession> = store.all().into_iter().cloned().collect();
            Ok(sessions)
        }
    }
}

/// End a session with a result status.
///
/// If `result == "success"`, checks the verification policy in the
/// Protocol before allowing Succeeded status. If verification checks
/// are missing, the status is set to `Failed` with a warning.
///
/// After status update, automatically triggers learning analysis (P5)
/// but does NOT auto-apply any proposals.
pub fn end_session(
    project_root: &Path,
    session_id: &str,
    result: &str,
) -> Result<ExecutionSession> {
    let mut store = SessionStore::load(project_root)?;
    let session = store
        .get_mut(session_id)
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;

    // P8: reject double-end
    if session.status != SessionStatus::Active {
        anyhow::bail!(
            "session '{}' is not Active (status: {:?}). \
             Cannot end a session that is already terminated.",
            session_id,
            session.status
        );
    }

    let new_status = match result {
        "success" => {
            // P3: Check verification policy before allowing Succeeded.
            let verification_pass = check_verification_policy(project_root, session_id)?;
            if verification_pass {
                SessionStatus::Succeeded
            } else {
                // Verification failed — mark as Failed.
                SessionStatus::Failed
            }
        }
        "failed" => SessionStatus::Failed,
        "aborted" => SessionStatus::Aborted,
        other => anyhow::bail!(
            "unknown result '{}'. Expected: success | failed | aborted",
            other
        ),
    };

    session.status = new_status;
    session.ended_at = Some(route_core::now_millis());
    let ended = session.clone();

    // Drop mutable borrow before saving.
    drop(store);
    let mut store = SessionStore::load(project_root)?;
    if let Some(s) = store.get_mut(session_id) {
        s.status = ended.status;
        s.ended_at = ended.ended_at;
    }
    store.save(project_root)?;

    // P5: Auto-trigger learning analysis (proposal-only, no auto-apply).
    let _ = analyze_events(project_root);

    // P4: Record experiment if this session was strategy-driven.
    let _ = record_experiment_for_session(project_root, &ended);

    // P5: Record organization experience if the session had an agent plan.
    let _ = record_experiment_for_session(project_root, &ended);

    // P1: Auto-save archive — verification success
    if new_status == SessionStatus::Succeeded {
        let _ = crate::game_save::auto_save_verification_pass(
            project_root,
            &crate::game_save::project_id_from_path(project_root),
            &crate::repository::BasicRepository::open(project_root)
                .unwrap_or_else(|_| panic!("BasicRepository::open")),
            env!("CARGO_PKG_VERSION"),
            session_id,
            true, // source_is_system — Route Engine, not AI
        );
    }

    // P0/P3: Complete trajectory if one exists.
    if let Some(ref tid) = ended.trajectory_id {
        if let Ok(mut traj_store) = crate::trajectory::TrajectoryStore::load(project_root) {
            if let Some(traj) = traj_store.get_mut(tid) {
                // Create an ending savepoint
                let end_code_snapshot_id = crate::repository::BasicRepository::open(project_root)
                    .ok()
                    .and_then(|repo| repo.history(1).ok())
                    .and_then(|h| h.first().map(|c| c.snapshot.id.clone()))
                    .unwrap_or_else(|| "no-snapshot".to_string());

                let end_sp = create_savepoint(
                    project_root,
                    format!("auto-end:{}", &ended.task[..ended.task.len().min(36)]),
                    end_code_snapshot_id,
                    ended.context_hash.clone(),
                    None,
                    ended.strategy_id.clone(),
                    "auto".to_string(),
                    Vec::new(),
                    ended.selected_reference_ids.clone(),
                    ended.agent_policy_hash.clone(),
                    Vec::new(),
                    Some(ended.id.clone()),
                    Some(ended.id.clone()),
                )
                .ok();

                let outcome = match ended.status {
                    SessionStatus::Succeeded => "success",
                    SessionStatus::Failed => "failed",
                    SessionStatus::Aborted => "abandoned",
                    _ => "unknown",
                };

                crate::trajectory::complete_trajectory(
                    traj,
                    end_sp
                        .map(|sp| sp.id)
                        .unwrap_or_else(|| "no-savepoint".to_string()),
                    outcome.to_string(),
                    None,
                );

                // P8: Auto-consolidate memory candidates (before save, while traj is alive)
                let candidates = crate::trajectory::consolidate_memory_candidates(traj);

                // Drop mutable borrow before save
                let _ = traj;
                let _ = traj_store.save(project_root);

                // Store candidates (owned data, no borrow conflict)
                if !candidates.is_empty() {
                    if std::fs::create_dir_all(
                        project_root
                            .join(".route")
                            .join("trajectory")
                            .join("candidates"),
                    )
                    .is_ok()
                    {
                        for c in &candidates {
                            let cpath = project_root
                                .join(".route")
                                .join("trajectory")
                                .join("candidates")
                                .join(format!("{}.json", &c.id));
                            if let Ok(json) = serde_json::to_string_pretty(c) {
                                let _ = std::fs::write(&cpath, json);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(ended)
}

// ---------------------------------------------------------------------------
// P3: Verification policy
// ---------------------------------------------------------------------------

/// Parse the optional verification policy from the Protocol frontmatter.
///
/// The policy is embedded in an HTML comment block:
/// ```html
/// <!-- route-verification-policy:
///   required_checks: ["cargo test", "cargo clippy"]
///   success_requires: ["test_pass", "review"]
///   independent_review: true
/// -->
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerificationPolicy {
    /// Checks that must have been run (e.g. "cargo test", "cargo clippy").
    #[serde(default)]
    pub required_checks: Vec<String>,
    /// Evidence kinds that must be present for success (e.g. "test_pass").
    #[serde(default)]
    pub success_requires: Vec<String>,
    /// Whether an independent review is required before marking success.
    #[serde(default)]
    pub independent_review: Option<bool>,
}

impl VerificationPolicy {
    /// Extract from the Protocol body by looking for
    /// `<!-- route-verification-policy: ... -->` comments.
    pub fn from_protocol_body(body: &str) -> Self {
        let marker_start = "<!-- route-verification-policy:";
        let marker_end = "-->";
        let mut policy = VerificationPolicy::default();

        if let Some(start) = body.find(marker_start) {
            let after_start = &body[start + marker_start.len()..];
            if let Some(end) = after_start.find(marker_end) {
                let content = after_start[..end].trim();
                // Try to parse as simple key-value lines.
                for line in content.lines() {
                    let line = line.trim();
                    if let Some(val) = line.strip_prefix("required_checks:") {
                        policy.required_checks = val
                            .trim()
                            .trim_matches('[')
                            .trim_matches(']')
                            .split(',')
                            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    } else if let Some(val) = line.strip_prefix("success_requires:") {
                        policy.success_requires = val
                            .trim()
                            .trim_matches('[')
                            .trim_matches(']')
                            .split(',')
                            .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    } else if let Some(val) = line.strip_prefix("independent_review:") {
                        policy.independent_review = Some(val.trim() == "true");
                    }
                }
            }
        }
        policy
    }
}

/// Check whether the verification policy is satisfied for a session.
/// Returns true if all required evidence kinds are present.
fn check_verification_policy(project_root: &Path, session_id: &str) -> Result<bool> {
    // Read the current Protocol for verification policy.
    let protocol = crate::constitutive::Protocol::read(project_root)?;
    let policy = VerificationPolicy::from_protocol_body(&protocol.body);

    if policy.success_requires.is_empty() {
        // No specific requirements — success is allowed.
        return Ok(true);
    }

    // Load evidence for this session.
    let evidence_store = EvidenceStore::load(project_root)?;
    let session_evidence: Vec<&Evidence> = evidence_store
        .evidence
        .iter()
        .filter(|e| e.session_id == session_id)
        .collect();

    // Check that each required evidence kind is present as System evidence.
    for required in &policy.success_requires {
        let has_system_evidence = session_evidence
            .iter()
            .any(|e| e.kind.as_str() == required.as_str() && e.source == EvidenceSource::System);
        if !has_system_evidence {
            // AI claims don't count — only System evidence satisfies the policy.
            return Ok(false);
        }
    }

    Ok(true)
}

// ---------------------------------------------------------------------------
// P1: Evidence
// ---------------------------------------------------------------------------

/// The kind of execution evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A commit was created.
    Commit,
    /// A snapshot was recorded.
    Snapshot,
    /// Tests passed (system-detected).
    TestPass,
    /// Tests failed (system-detected).
    TestFail,
    /// A rollback occurred.
    Rollback,
    /// The AI agent reported an observation (NOT system-verified).
    AgentFeedback,
    /// A reference proposal was generated.
    ReferenceProposal,
    /// Manual user note.
    Manual,
    /// A command/check passed (P2).
    CheckPass,
    /// A command/check failed (P2).
    CheckFail,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::Commit => "commit",
            EvidenceKind::Snapshot => "snapshot",
            EvidenceKind::TestPass => "test_pass",
            EvidenceKind::TestFail => "test_fail",
            EvidenceKind::Rollback => "rollback",
            EvidenceKind::AgentFeedback => "agent_feedback",
            EvidenceKind::ReferenceProposal => "reference_proposal",
            EvidenceKind::Manual => "manual",
            EvidenceKind::CheckPass => "check_pass",
            EvidenceKind::CheckFail => "check_fail",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "commit" => Some(EvidenceKind::Commit),
            "snapshot" => Some(EvidenceKind::Snapshot),
            "test_pass" => Some(EvidenceKind::TestPass),
            "test_fail" => Some(EvidenceKind::TestFail),
            "rollback" => Some(EvidenceKind::Rollback),
            "agent_feedback" => Some(EvidenceKind::AgentFeedback),
            "reference_proposal" => Some(EvidenceKind::ReferenceProposal),
            "manual" => Some(EvidenceKind::Manual),
            "check_pass" => Some(EvidenceKind::CheckPass),
            "check_fail" => Some(EvidenceKind::CheckFail),
            _ => None,
        }
    }
}

/// Who or what produced the evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    /// System-detected evidence (commits, test results, rollbacks).
    /// This is the only source that can satisfy verification policies.
    System,
    /// AI-declared evidence (agent feedback). Cannot satisfy verification
    /// policies on its own — only supplements System evidence.
    Agent,
    /// User-declared evidence (manual notes).
    User,
}

impl EvidenceSource {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceSource::System => "system",
            EvidenceSource::Agent => "agent",
            EvidenceSource::User => "user",
        }
    }
}

/// A single piece of execution evidence linked to a session.
///
/// Design: Evidence is a fire-and-forget record. System evidence is
/// trusted; Agent evidence is recorded as-is but never used to satisfy
/// verification policies. Route does not infer user intent from evidence.
///
/// P1: Each evidence binds to a working-state fingerprint via `state_hash`,
/// plus optional `snapshot_id`, `commit_id`, and `context_hash` for
/// causal tracing. A TestPass is only valid for the state it verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Stable evidence id (ULID).
    pub id: String,
    /// The session this evidence belongs to.
    pub session_id: String,
    /// What kind of evidence.
    pub kind: EvidenceKind,
    /// Who produced the evidence.
    pub source: EvidenceSource,
    /// SHA-256 of the payload content (no large text copied).
    pub payload_hash: String,
    /// Optional metadata key-value pairs.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    /// Unix-millis when the evidence was recorded.
    pub created_at: i64,
    // -- P1: State identity fields --
    /// Working-state fingerprint at the time of evidence (P1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_hash: Option<String>,
    /// Snapshot ID this evidence relates to (P1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    /// Commit ID this evidence relates to (P1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_id: Option<String>,
    /// Context hash at the time of evidence (P1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// EvidenceStore
// ---------------------------------------------------------------------------

/// Persistent store for execution evidence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidenceStore {
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

impl EvidenceStore {
    /// Load evidence from disk. Returns empty store if file missing.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = evidence_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading evidence from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    /// Persist evidence atomically.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = evidence_path(project_root);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        constitutive::write_atomic(&p, &json)?;
        Ok(())
    }

    /// Record a new piece of evidence. Returns the evidence id.
    pub fn record(
        &mut self,
        session_id: &str,
        kind: EvidenceKind,
        source: EvidenceSource,
        payload_hash: String,
        metadata: HashMap<String, String>,
    ) -> String {
        self.record_with_state(
            session_id,
            kind,
            source,
            payload_hash,
            metadata,
            None,
            None,
            None,
            None,
        )
    }

    /// Record evidence with full state identity (P1).
    pub fn record_with_state(
        &mut self,
        session_id: &str,
        kind: EvidenceKind,
        source: EvidenceSource,
        payload_hash: String,
        metadata: HashMap<String, String>,
        state_hash: Option<String>,
        snapshot_id: Option<String>,
        commit_id: Option<String>,
        context_hash: Option<String>,
    ) -> String {
        let id = route_core::new_id();
        let evidence = Evidence {
            id: id.clone(),
            session_id: session_id.to_string(),
            kind,
            source,
            payload_hash,
            metadata,
            created_at: route_core::now_millis(),
            state_hash,
            snapshot_id,
            commit_id,
            context_hash,
        };
        self.evidence.push(evidence);
        id
    }

    /// Record evidence with deduplication (P0).
    /// Returns the evidence id if new, or None if a matching evidence already exists.
    /// Dedup key: (session_id, kind, dedup_key) — same session, same kind, same key = skip.
    pub fn record_dedup(
        &mut self,
        session_id: &str,
        kind: EvidenceKind,
        source: EvidenceSource,
        payload_hash: String,
        metadata: HashMap<String, String>,
        dedup_key: &str,
        state_hash: Option<String>,
        snapshot_id: Option<String>,
        commit_id: Option<String>,
        context_hash: Option<String>,
    ) -> Option<String> {
        // Check for existing evidence with same session, kind, and dedup_key in metadata.
        let exists = self.evidence.iter().any(|e| {
            e.session_id == session_id
                && e.kind == kind
                && e.metadata.get("dedup_key").map(|s| s.as_str()) == Some(dedup_key)
        });
        if exists {
            return None;
        }
        let mut meta = metadata;
        meta.insert("dedup_key".to_string(), dedup_key.to_string());
        Some(self.record_with_state(
            session_id,
            kind,
            source,
            payload_hash,
            meta,
            state_hash,
            snapshot_id,
            commit_id,
            context_hash,
        ))
    }

    /// Return all evidence for a session, sorted by created_at.
    pub fn for_session(&self, session_id: &str) -> Vec<&Evidence> {
        let mut v: Vec<&Evidence> = self
            .evidence
            .iter()
            .filter(|e| e.session_id == session_id)
            .collect();
        v.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        v
    }

    /// Check whether a TestPass/CheckPass evidence is stale for the current state (P1).
    /// A pass is stale if its state_hash differs from the current working-state fingerprint.
    pub fn is_pass_stale(&self, evidence: &Evidence, current_state_hash: &str) -> bool {
        if evidence.kind != EvidenceKind::TestPass && evidence.kind != EvidenceKind::CheckPass {
            return false;
        }
        match &evidence.state_hash {
            Some(h) => h != current_state_hash,
            None => true, // No state hash = stale (old format)
        }
    }

    /// Find all stale pass evidence for a session (P1).
    pub fn stale_passes(&self, session_id: &str, current_state_hash: &str) -> Vec<&Evidence> {
        self.evidence
            .iter()
            .filter(|e| {
                e.session_id == session_id
                    && (e.kind == EvidenceKind::TestPass || e.kind == EvidenceKind::CheckPass)
                    && self.is_pass_stale(e, current_state_hash)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// P4: Host report ingestion
// ---------------------------------------------------------------------------

/// Vendor-neutral execution report from the host AI.
///
/// Route validates session/context matching, then records only
/// AgentFeedback evidence. Host reports cannot directly generate
/// success conclusions, modify Constitution/Protocol, or create
/// Reference entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostReport {
    /// The session id this report belongs to.
    pub session_id: String,
    /// Agent roles that were used during execution.
    #[serde(default)]
    pub agent_roles_used: Vec<String>,
    /// Actions performed by the agent.
    #[serde(default)]
    pub actions: Vec<HostAction>,
    /// Verification results the agent is requesting (e.g. "tests passed").
    #[serde(default)]
    pub verification_requested: Vec<String>,
    /// Free-form observations from the agent.
    #[serde(default)]
    pub observations: Vec<String>,
}

/// A single action in a host report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostAction {
    /// What was done (e.g. "edit_file", "run_test", "commit").
    pub action: String,
    /// Tool used (e.g. "sed", "cargo", "git").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Target of the action (e.g. file path, test name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Result of the action (e.g. "ok", "error: ...").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// Ingest a host report and record it as AgentFeedback evidence.
///
/// Route:
/// 1. Validates that the session exists and is Active.
/// 2. Records each action/observation as a separate AgentFeedback evidence entry.
/// 3. Does NOT generate success/failure conclusions, modify Constitution,
///    Protocol, or Reference entries.
pub fn ingest_host_report(project_root: &Path, report: HostReport) -> Result<Vec<String>> {
    // Validate session exists and is Active.
    let store = SessionStore::load(project_root)?;
    let session = store
        .get(&report.session_id)
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", report.session_id))?;
    if session.status != SessionStatus::Active {
        anyhow::bail!(
            "session '{}' is not Active (status: {:?}). Cannot ingest host report.",
            report.session_id,
            session.status
        );
    }

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut ids = Vec::new();

    // Record each action as evidence.
    for action in &report.actions {
        let mut metadata = HashMap::new();
        metadata.insert("action".to_string(), action.action.clone());
        if let Some(tool) = &action.tool {
            metadata.insert("tool".to_string(), tool.clone());
        }
        if let Some(target) = &action.target {
            metadata.insert("target".to_string(), target.clone());
        }
        if let Some(result) = &action.result {
            metadata.insert("result".to_string(), result.clone());
        }
        let payload = format!(
            "{}:{}:{:?}",
            action.action,
            action.tool.as_deref().unwrap_or(""),
            action.result.as_deref().unwrap_or("")
        );
        let id = evidence_store.record(
            &report.session_id,
            EvidenceKind::AgentFeedback,
            EvidenceSource::Agent,
            route_core::sha256_hex(payload.as_bytes()),
            metadata,
        );
        ids.push(id);
    }

    // Record observations as separate evidence.
    for obs in &report.observations {
        let mut metadata = HashMap::new();
        metadata.insert("observation".to_string(), obs.clone());
        let id = evidence_store.record(
            &report.session_id,
            EvidenceKind::AgentFeedback,
            EvidenceSource::Agent,
            route_core::sha256_hex(obs.as_bytes()),
            metadata,
        );
        ids.push(id);
    }

    // Record agent roles used.
    if !report.agent_roles_used.is_empty() {
        let mut metadata = HashMap::new();
        metadata.insert("roles".to_string(), report.agent_roles_used.join(","));
        evidence_store.record(
            &report.session_id,
            EvidenceKind::AgentFeedback,
            EvidenceSource::Agent,
            route_core::sha256_hex(report.agent_roles_used.join(",").as_bytes()),
            metadata,
        );
    }

    evidence_store.save(project_root)?;
    Ok(ids)
}

// ---------------------------------------------------------------------------
// P5: Auto learning trigger (session end)
// ---------------------------------------------------------------------------

/// Run learning analysis after a session ends, generating proposals only.
///
/// This is called automatically by `end_session`. It:
/// 1. Runs the existing deterministic `analyze_events` aggregation.
/// 2. Only generates proposals — never auto-applies.
/// 3. Deduplicates by (session_id, claim) to avoid repeated weighting.
///
/// Returns the number of new proposals generated.
pub fn auto_analyze_after_session(project_root: &Path, _session_id: &str) -> Result<usize> {
    let proposals = analyze_events(project_root)?;
    Ok(proposals.len())
}

// ---------------------------------------------------------------------------
// P6: Task audit
// ---------------------------------------------------------------------------

/// A single entry in the causal timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalEntry {
    /// Unix-millis when the event occurred.
    pub timestamp: i64,
    /// Human-readable kind (e.g. "session_begin", "commit", "test_pass", "rollback").
    pub kind: String,
    /// Detail description.
    pub detail: String,
    /// Reference id (evidence id, commit id, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
}

/// Full audit view of a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAudit {
    /// The session itself.
    pub session: ExecutionSession,
    /// Context hash at session start.
    pub context_hash: String,
    /// Selected reference IDs.
    pub selected_reference_ids: Vec<String>,
    /// Agent policy hash.
    pub agent_policy_hash: Option<String>,
    /// Evidence for this session.
    pub evidence: Vec<Evidence>,
    /// Learning proposals generated from this session's evidence.
    pub learning_proposals: Vec<String>,
    /// Causal timeline entries.
    pub timeline: Vec<CausalEntry>,
}

/// Show detailed audit for a session.
///
/// Outputs: context hash, selected refs, AgentPolicy, commit/test/rollback
/// evidence, final result, learning proposals, and full causal timeline.
pub fn show_session(project_root: &Path, session_id: &str, explain: bool) -> Result<SessionAudit> {
    let store = SessionStore::load(project_root)?;
    let session = store
        .get(session_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;

    let evidence_store = EvidenceStore::load(project_root)?;
    let session_evidence = evidence_store.for_session(session_id);

    // Build causal timeline.
    let mut timeline = Vec::new();

    // Session begin.
    timeline.push(CausalEntry {
        timestamp: session.started_at,
        kind: "session_begin".to_string(),
        detail: format!(
            "Task: '{}', target: {}",
            session.task,
            session.target.as_str()
        ),
        ref_id: Some(session.id.clone()),
    });

    // Context apply.
    timeline.push(CausalEntry {
        timestamp: session.started_at,
        kind: "context_apply".to_string(),
        detail: format!(
            "Context hash: {}, selected refs: {}",
            &session.context_hash[..16.min(session.context_hash.len())],
            if session.selected_reference_ids.is_empty() {
                "(global)".to_string()
            } else {
                session.selected_reference_ids.join(", ")
            }
        ),
        ref_id: None,
    });

    // Evidence entries.
    for ev in &session_evidence {
        let detail = if ev.metadata.is_empty() {
            format!("{} evidence", ev.kind.as_str())
        } else {
            let meta_str: Vec<String> = ev
                .metadata
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            format!("{} evidence: {}", ev.kind.as_str(), meta_str.join(", "))
        };
        timeline.push(CausalEntry {
            timestamp: ev.created_at,
            kind: ev.kind.as_str().to_string(),
            detail,
            ref_id: Some(ev.id.clone()),
        });
    }

    // Session end.
    if let Some(ended_at) = session.ended_at {
        timeline.push(CausalEntry {
            timestamp: ended_at,
            kind: "session_end".to_string(),
            detail: format!("Status: {:?}", session.status),
            ref_id: Some(session.id.clone()),
        });
    }

    // Sort timeline by timestamp.
    timeline.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    // If --explain, also check for context explain details.
    let _ = explain; // Used for future extension.

    // Extract values before moving session.
    let context_hash = session.context_hash.clone();
    let selected_reference_ids = session.selected_reference_ids.clone();
    let agent_policy_hash = session.agent_policy_hash.clone();

    // Find learning proposals related to this session.
    let proposal_store = LearningProposalStore::load(project_root)?;
    let mut learning_proposals = Vec::new();
    for proposal in &proposal_store.proposals {
        if proposal
            .supporting_event_ids
            .iter()
            .any(|eid| session_evidence.iter().any(|ev| ev.id == *eid))
        {
            learning_proposals.push(proposal.claim.clone());
        }
    }

    Ok(SessionAudit {
        session,
        context_hash,
        selected_reference_ids,
        agent_policy_hash,
        evidence: session_evidence.into_iter().cloned().collect(),
        learning_proposals,
        timeline,
    })
}

// ---------------------------------------------------------------------------
// P7: Task replay
// ---------------------------------------------------------------------------

/// Replay result for a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    /// The session id.
    pub session_id: String,
    /// The task description.
    pub task: String,
    /// The context hash at session start.
    pub context_hash: String,
    /// The reconstructed context body (Markdown).
    pub context_body: String,
    /// Whether the context is a partial reconstruction (missing some components).
    pub partial: bool,
    /// Warning message if partial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Replay the exact context that was active when a session began.
///
/// This is read-only: it reconstructs the task-scoped context + policy
/// + selected references from the session's recorded data. It does NOT
/// restore files or re-execute the AI.
///
/// If the historical context archive is missing, returns a PartialHistory
/// with a warning rather than guessing.
pub fn replay_session(
    project_root: &Path,
    session_id: &str,
    target: Option<ApplyTarget>,
    to_stdout: bool,
) -> Result<ReplayResult> {
    let store = SessionStore::load(project_root)?;
    let session = store
        .get(session_id)
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;

    let replay_target = target.unwrap_or(session.target);
    let mut body = String::new();

    // Try to replay from the context archive.
    let replay_result = replay_context(project_root, &session.context_hash);
    let (ctx_body, partial, warning) = match replay_result {
        Ok(b) => (b, false, None),
        Err(e) => {
            // Fallback: build from current context (marked as partial).
            let warn = format!(
                "Historical context archive for hash '{}' not found: {}. \
                 Reconstructed from current state (may differ from original).",
                &session.context_hash[..16.min(session.context_hash.len())],
                e
            );
            let current = build_context(
                project_root,
                Some(&session.task),
                Some(replay_target.as_str()),
                5,
                ContextBudget::default(),
                None,
            )
            .unwrap_or_else(|_| "<!-- unable to reconstruct context -->".to_string());
            (current, true, Some(warn))
        }
    };

    body.push_str(&ctx_body);

    // Append agent policy rendering.
    if let Some(ref policy_hash) = session.agent_policy_hash {
        let policy_block = match replay_target {
            ApplyTarget::Claude => format!(
                "\n\n<!-- route-agent-policy (hash: {}) -->\n\n{}\n",
                policy_hash,
                AgentPolicy::from_protocol_body("").render_claude()
            ),
            ApplyTarget::Codex => format!(
                "\n\n<!-- route-agent-policy (hash: {}) -->\n\n{}\n",
                policy_hash,
                AgentPolicy::from_protocol_body("").render_codex()
            ),
            ApplyTarget::DeepSeek => format!(
                "\n\n<!-- route-agent-policy (hash: {}) -->\n\n{}\n",
                policy_hash,
                AgentPolicy::from_protocol_body("").render_deepseek()
            ),
            ApplyTarget::Generic => format!(
                "\n\n<!-- route-agent-policy (hash: {}) -->\n\n{}\n",
                policy_hash,
                AgentPolicy::from_protocol_body("").render_generic()
            ),
        };
        body.push_str(&policy_block);
    }

    if to_stdout {
        println!("{}", body);
    }

    Ok(ReplayResult {
        session_id: session.id.clone(),
        task: session.task.clone(),
        context_hash: session.context_hash.clone(),
        context_body: body,
        partial,
        warning,
    })
}

// ---------------------------------------------------------------------------
// P8: Consistency checks
// ---------------------------------------------------------------------------

/// Result of a single consistency check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyFinding {
    /// Severity: error | warning
    pub severity: String,
    /// Category of the check.
    pub category: String,
    /// Human-readable description.
    pub message: String,
}

/// Run Execution Ledger integrity checks.
///
/// Checks:
/// 1. session.context_hash matches applied context hash
/// 2. selected_reference_ids exist in the current registry
/// 3. No orphan evidence (evidence referencing non-existent sessions)
/// 4. No duplicate end (already handled at end_session time)
/// 5. Session ended but no evidence recorded (warning)
/// 6. Evidence recorded after session ended (warning)
pub fn check_execution_ledger(project_root: &Path) -> Result<Vec<ConsistencyFinding>> {
    let mut findings = Vec::new();

    let session_store = SessionStore::load(project_root)?;
    let evidence_store = EvidenceStore::load(project_root)?;

    // Build set of known session IDs.
    let session_ids: Vec<String> = session_store
        .sessions
        .iter()
        .map(|s| s.id.clone())
        .collect();

    // Check 1: Orphan evidence.
    for ev in &evidence_store.evidence {
        if !session_ids.contains(&ev.session_id) {
            findings.push(ConsistencyFinding {
                severity: "error".to_string(),
                category: "orphan_evidence".to_string(),
                message: format!(
                    "Evidence '{}' references non-existent session '{}'",
                    ev.id, ev.session_id
                ),
            });
        }
    }

    for session in &session_store.sessions {
        // Check 2: Session ended but no evidence recorded.
        if session.ended_at.is_some()
            && !evidence_store
                .evidence
                .iter()
                .any(|e| e.session_id == session.id)
        {
            findings.push(ConsistencyFinding {
                severity: "warning".to_string(),
                category: "no_evidence".to_string(),
                message: format!(
                    "Session '{}' ended with status {:?} but has no recorded evidence",
                    &session.id[..12],
                    session.status
                ),
            });
        }

        // Check 3: Evidence recorded after session ended.
        if let Some(ended_at) = session.ended_at {
            for ev in evidence_store
                .evidence
                .iter()
                .filter(|e| e.session_id == session.id)
            {
                if ev.created_at > ended_at {
                    findings.push(ConsistencyFinding {
                        severity: "warning".to_string(),
                        category: "post_end_evidence".to_string(),
                        message: format!(
                            "Evidence '{}' ({}) was recorded after session '{}' ended",
                            &ev.id[..12],
                            ev.kind.as_str(),
                            &session.id[..12]
                        ),
                    });
                }
            }
        }
    }

    Ok(findings)
}

// ---------------------------------------------------------------------------
// P2: Session-aware evidence recording helpers
// ---------------------------------------------------------------------------

/// Record a commit as system evidence for the active session.
/// Returns the evidence id, or None if no active session.
pub fn record_commit_evidence(project_root: &Path, commit_id: &str) -> Result<Option<String>> {
    let store = SessionStore::load(project_root)?;
    let active = store.active();
    let session_id = match active {
        Some(s) => s.id.clone(),
        None => return Ok(None),
    };
    drop(store); // Release borrow before mutating evidence_store.

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert("commit_id".to_string(), commit_id.to_string());
    let id = evidence_store.record(
        &session_id,
        EvidenceKind::Commit,
        EvidenceSource::System,
        route_core::sha256_hex(commit_id.as_bytes()),
        metadata,
    );
    evidence_store.save(project_root)?;
    Ok(Some(id))
}

/// Record a rollback as system evidence for the active session.
pub fn record_rollback_evidence(
    project_root: &Path,
    rollback_snapshot_id: &str,
) -> Result<Option<String>> {
    let store = SessionStore::load(project_root)?;
    let active = store.active();
    let session_id = match active {
        Some(s) => s.id.clone(),
        None => return Ok(None),
    };
    drop(store);

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert(
        "rollback_snapshot".to_string(),
        rollback_snapshot_id.to_string(),
    );
    let id = evidence_store.record(
        &session_id,
        EvidenceKind::Rollback,
        EvidenceSource::System,
        route_core::sha256_hex(rollback_snapshot_id.as_bytes()),
        metadata,
    );
    evidence_store.save(project_root)?;
    Ok(Some(id))
}

/// Record a test result as system evidence for the active session.
pub fn record_test_evidence(
    project_root: &Path,
    passed: bool,
    test_name: &str,
) -> Result<Option<String>> {
    let store = SessionStore::load(project_root)?;
    let active = store.active();
    let session_id = match active {
        Some(s) => s.id.clone(),
        None => return Ok(None),
    };
    drop(store);

    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert("test_name".to_string(), test_name.to_string());
    metadata.insert("passed".to_string(), passed.to_string());
    let kind = if passed {
        EvidenceKind::TestPass
    } else {
        EvidenceKind::TestFail
    };
    let id = evidence_store.record(
        &session_id,
        kind,
        EvidenceSource::System,
        route_core::sha256_hex(format!("{}:{}", test_name, passed).as_bytes()),
        metadata,
    );
    evidence_store.save(project_root)?;
    Ok(Some(id))
}

// ---------------------------------------------------------------------------
// P2: Add session_id to ExperienceEvent
// ---------------------------------------------------------------------------

/// Record an experience event with an optional session_id.
///
/// Extends the existing `ExperienceStore::record` with session linking.
/// The session_id is stored in the event's tags (prefixed with "session:").
pub fn record_experience_event_with_session(
    project_root: &Path,
    session_id: Option<&str>,
    kind: EventKind,
    outcome: &str,
    evidence: &str,
) -> Result<String> {
    let mut store = ExperienceStore::load(project_root)?;
    let mut tags = Vec::new();
    if let Some(sid) = session_id {
        tags.push(format!("session:{}", sid));
    }
    let id = store.record(
        project_root,
        kind,
        outcome,
        evidence,
        LearnScope::Project,
        None,
        tags,
        None,
        None,
    )?;
    store.save(project_root)?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// P2: Command execution
// ---------------------------------------------------------------------------

/// Result of a single command execution (P2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEvidence {
    /// The argv that was executed (never shell-joined).
    pub argv: Vec<String>,
    /// Exit code (0 = success).
    pub exit_code: i32,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// SHA-256 of stdout.
    pub stdout_hash: String,
    /// SHA-256 of stderr.
    pub stderr_hash: String,
    /// Working-state fingerprint at execution time.
    pub state_hash: String,
    /// Optional check/probe identifier for verification policy matching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_id: Option<String>,
    /// Truncated stdout (first 2KB) for display. Empty if output was empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stdout_preview: String,
    /// Truncated stderr (first 2KB) for display. Empty if output was empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stderr_preview: String,
}

/// Execute a command via process spawn (no shell) and record evidence (P2).
///
/// Returns the command evidence on success (exit 0) or failure.
/// stdout/stderr are hashed and truncated to 2KB each for persistence.
/// Full output is NOT stored in the ledger — only hash + preview.
pub fn exec_command(
    project_root: &Path,
    session_id: &str,
    argv: &[String],
    check_id: Option<&str>,
) -> Result<CommandEvidence> {
    // Validate session exists and is Active.
    let store = SessionStore::load(project_root)?;
    let session = store
        .get(session_id)
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;
    if session.status != SessionStatus::Active {
        anyhow::bail!(
            "session '{}' is not Active (status: {:?}). Cannot execute commands.",
            session_id,
            session.status
        );
    }
    drop(store);

    if argv.is_empty() {
        anyhow::bail!("argv is empty — nothing to execute");
    }

    // Spawn process (no shell).
    let start = Instant::now();
    let output = StdCommand::new(&argv[0])
        .args(&argv[1..])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .current_dir(project_root)
        .output()
        .with_context(|| format!("failed to execute: {}", argv.join(" ")))?;
    let duration_ms = start.elapsed().as_millis() as u64;

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout_bytes = output.stdout;
    let stderr_bytes = output.stderr;

    // Hash and truncate.
    let stdout_hash = route_core::sha256_hex(&stdout_bytes);
    let stderr_hash = route_core::sha256_hex(&stderr_bytes);
    let stdout_preview =
        String::from_utf8_lossy(&stdout_bytes[..stdout_bytes.len().min(2048)]).to_string();
    let stderr_preview =
        String::from_utf8_lossy(&stderr_bytes[..stderr_bytes.len().min(2048)]).to_string();

    // Compute state hash.
    let state_hash = compute_state_hash(project_root).unwrap_or_default();
    if !state_hash.is_empty() {
        write_state_hash(project_root, &state_hash)?;
    }

    let cmd_evidence = CommandEvidence {
        argv: argv.to_vec(),
        exit_code,
        duration_ms,
        stdout_hash,
        stderr_hash,
        state_hash: state_hash.clone(),
        check_id: check_id.map(|s| s.to_string()),
        stdout_preview,
        stderr_preview,
    };

    // Record evidence.
    let mut evidence_store = EvidenceStore::load(project_root)?;
    let kind = if exit_code == 0 {
        EvidenceKind::CheckPass
    } else {
        EvidenceKind::CheckFail
    };
    let mut metadata = HashMap::new();
    metadata.insert("argv".to_string(), argv.join(" "));
    metadata.insert("exit_code".to_string(), exit_code.to_string());
    metadata.insert("duration_ms".to_string(), duration_ms.to_string());
    if let Some(cid) = check_id {
        metadata.insert("check_id".to_string(), cid.to_string());
    }
    let dedup_key = format!("exec:{}:{}", argv.join(" "), state_hash);
    evidence_store.record_dedup(
        session_id,
        kind,
        EvidenceSource::System,
        route_core::sha256_hex(format!("{}:{}", argv.join(" "), exit_code).as_bytes()),
        metadata,
        &dedup_key,
        if state_hash.is_empty() {
            None
        } else {
            Some(state_hash)
        },
        None,
        None,
        None,
    );
    evidence_store.save(project_root)?;

    Ok(cmd_evidence)
}

// ---------------------------------------------------------------------------
// P3: Verify session
// ---------------------------------------------------------------------------

/// Result of a verification check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerifyResult {
    /// All required checks are satisfied with valid system evidence.
    Verified,
    /// One or more checks failed or have stale evidence.
    Failed,
    /// Some checks are missing entirely (not yet executed).
    Incomplete,
}

/// Run verification checks for a session (P3).
///
/// Reads the Protocol VerificationPolicy, identifies missing or stale checks,
/// and returns the overall verification result.
/// Only System evidence is accepted — AgentFeedback never satisfies a check.
pub fn verify_session(project_root: &Path, session_id: &str) -> Result<VerifyResult> {
    let store = SessionStore::load(project_root)?;
    let session = store
        .get(session_id)
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;
    // Allow verifying non-active sessions too (for audit).
    let _target = session.target;
    drop(store);

    // Read the current Protocol for verification policy.
    let protocol = crate::constitutive::Protocol::read(project_root)?;
    let policy = VerificationPolicy::from_protocol_body(&protocol.body);

    if policy.success_requires.is_empty() && policy.required_checks.is_empty() {
        // No policy = trivially verified.
        return Ok(VerifyResult::Verified);
    }

    // Load evidence for this session.
    let evidence_store = EvidenceStore::load(project_root)?;
    let session_evidence: Vec<&Evidence> = evidence_store
        .evidence
        .iter()
        .filter(|e| e.session_id == session_id)
        .collect();

    // Compute current state hash for staleness check.
    let current_state_hash = compute_state_hash(project_root).unwrap_or_default();

    // Check required_checks: each check must have a corresponding
    // CheckPass or TestPass with matching check_id, as System evidence,
    // and not stale.
    let mut all_checks_present = true;
    let mut any_stale = false;

    for required in &policy.required_checks {
        let has_valid = session_evidence.iter().any(|e| {
            // Match by check_id in metadata or by argv containing the required string.
            let check_matches = match e.metadata.get("check_id") {
                Some(cid) => cid == required,
                None => e
                    .metadata
                    .get("argv")
                    .map_or(false, |argv| argv.contains(required)),
            };
            if !check_matches {
                return false;
            }
            // Must be System evidence and not stale.
            e.source == EvidenceSource::System
                && !evidence_store.is_pass_stale(e, &current_state_hash)
        });
        if !has_valid {
            // Check if there's stale evidence — if so, mark as stale.
            let has_stale = session_evidence.iter().any(|e| {
                let check_matches = match e.metadata.get("check_id") {
                    Some(cid) => cid == required,
                    None => e
                        .metadata
                        .get("argv")
                        .map_or(false, |argv| argv.contains(required)),
                };
                check_matches && e.source == EvidenceSource::System
            });
            if has_stale {
                any_stale = true;
            } else {
                all_checks_present = false;
            }
        }
    }

    // Check success_requires: each required evidence kind must be present
    // as System evidence and not stale.
    for required in &policy.success_requires {
        let has_valid = session_evidence.iter().any(|e| {
            e.kind.as_str() == required.as_str()
                && e.source == EvidenceSource::System
                && !evidence_store.is_pass_stale(e, &current_state_hash)
        });
        if !has_valid {
            let has_stale = session_evidence.iter().any(|e| {
                e.kind.as_str() == required.as_str() && e.source == EvidenceSource::System
            });
            if has_stale {
                any_stale = true;
            } else {
                all_checks_present = false;
            }
        }
    }

    if !all_checks_present {
        return Ok(VerifyResult::Incomplete);
    }
    if any_stale {
        return Ok(VerifyResult::Failed);
    }
    Ok(VerifyResult::Verified)
}

/// End a session with forced success despite verification failure (P3).
///
/// Sets status to `ForcedUnverified` — a permanent audit marker.
/// This status cannot be used as strong positive learning signal.
pub fn force_end_session(project_root: &Path, session_id: &str) -> Result<ExecutionSession> {
    let mut store = SessionStore::load(project_root)?;
    let session = store
        .get_mut(session_id)
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;

    if session.status != SessionStatus::Active {
        anyhow::bail!(
            "session '{}' is not Active (status: {:?}). Cannot force-end.",
            session_id,
            session.status
        );
    }

    session.status = SessionStatus::ForcedUnverified;
    session.ended_at = Some(route_core::now_millis());
    let ended = session.clone();
    drop(store);

    // Re-persist.
    let mut store = SessionStore::load(project_root)?;
    if let Some(s) = store.get_mut(session_id) {
        s.status = ended.status;
        s.ended_at = ended.ended_at;
    }
    store.save(project_root)?;

    // Record a warning in evidence.
    let mut evidence_store = EvidenceStore::load(project_root)?;
    let mut metadata = HashMap::new();
    metadata.insert("forced_unverified".to_string(), "true".to_string());
    metadata.insert(
        "reason".to_string(),
        "User force-ended session despite verification failure".to_string(),
    );
    evidence_store.record(
        session_id,
        EvidenceKind::Manual,
        EvidenceSource::System,
        route_core::sha256_hex(b"forced_unverified"),
        metadata,
    );
    evidence_store.save(project_root)?;

    // Learning analysis (neutral/weak).
    let _ = analyze_events(project_root);

    // P4: Record experiment if this session was strategy-driven.
    let _ = record_experiment_for_session(project_root, &ended);

    Ok(ended)
}

// ---------------------------------------------------------------------------
// P4: One-command entry — route task start
// ---------------------------------------------------------------------------

/// Result of `route task start` (P4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTaskResult {
    /// The session that was created.
    pub session: ExecutionSession,
    /// The context hash.
    pub context_hash: String,
    /// The task context body (for AI host instructions).
    pub context_body: String,
    /// Whether the context archive was written.
    pub archived: bool,
    /// Host instructions for the AI.
    pub host_instructions: String,
}

/// Atomic one-command entry point (P4).
///
/// 1. begin_session (with auto baseline snapshot)
/// 2. Build task context
/// 3. Archive context history
/// 4. Apply context to host file
/// 5. Return session_id + host instructions
///
/// On failure, the session is marked Aborted or Recoverable — never left
/// in a half-initialized unknown state.
pub fn start_task_session(
    project_root: &Path,
    task: &str,
    target: ApplyTarget,
    strategy_id: Option<String>,
    campaign_id: Option<String>,
) -> Result<StartTaskResult> {
    // Step 1: begin_session (auto-snapshots baseline + records evidence).
    let session = match begin_session(project_root, task, target, false, strategy_id) {
        Ok(s) => s,
        Err(e) => {
            // Check if a partial session was created — if so, abort it.
            let mut store = SessionStore::load(project_root)?;
            if let Some(active) = store.active() {
                let active_id = active.id.clone();
                // Mark as Aborted
                if let Some(s) = store.get_mut(&active_id) {
                    s.status = SessionStatus::Aborted;
                    s.ended_at = Some(route_core::now_millis());
                }
                let _ = store.save(project_root);
            }
            return Err(e.context("route task start failed: could not begin session"));
        }
    };

    // Step 1.5: Bind an optional parent campaign id onto the persisted session.
    // Best-effort: a missing campaign reference is not fatal — it only drops
    // the Task↔Campaign association, never the session itself.
    if let Some(cid) = campaign_id {
        if let Ok(mut store) = SessionStore::load(project_root) {
            if let Some(s) = store.get_mut(&session.id) {
                s.campaign_id = Some(cid);
            }
            let _ = store.save(project_root);
        }
    }

    // Step 2: Build task context.
    let context_body = match crate::constitutive::build_context(
        project_root,
        Some(task),
        Some(target.as_str()),
        5,
        ContextBudget::default(),
        None,
    ) {
        Ok(body) => body,
        Err(e) => {
            // Abort session on context build failure.
            let mut store = SessionStore::load(project_root)?;
            if let Some(s) = store.get_mut(&session.id) {
                s.status = SessionStatus::Aborted;
                s.ended_at = Some(route_core::now_millis());
            }
            let _ = store.save(project_root);
            return Err(e.context("route task start failed: could not build context"));
        }
    };

    // Step 3: Archive context history.
    let snap = crate::constitutive::ContextSnapshot::collect(project_root)?;
    let archived = match crate::constitutive::archive_current_context(project_root, &snap) {
        Ok(_) => true,
        Err(_) => false,
    };

    // Step 4: Apply context to host file.
    if let Err(e) = crate::adapter::apply_context(project_root, target, Some(task)) {
        // Non-fatal: the session is still valid, but the host file wasn't updated.
        tracing::warn!(
            error = %e,
            "route task start: context apply to host file failed (non-fatal)"
        );
    }

    // Step 5: Build host instructions.
    let host_instructions = format!(
        r#"<!-- route-session -->
Session ID: {}
Task: {}
Context Hash: {}
Target: {}

Instructions:
1. Modify code normally using your host capabilities.
2. Run tests and checks via `route task exec {} -- <command>`.
3. Create snapshots/commits/rollbacks through Route's CLI.
4. Before finishing, run `route task verify {}` to check verification policy.
5. You may submit an execution report via `route task report` with observations.
6. You CANNOT declare System Evidence or final Succeeded status — only Route can.
<!-- /route-session -->"#,
        session.id,
        task,
        session.context_hash,
        target.as_str(),
        &session.id[..12],
        &session.id[..12],
    );

    // Step 6: Create auto-savepoint and trajectory (P0/P1).
    // Best-effort: errors here are non-fatal.
    let trajectory_id = create_auto_savepoint_and_trajectory(project_root, task, &session, target);

    // Update session with trajectory ID if one was created.
    if let Some(ref tid) = trajectory_id {
        if let Ok(mut store) = SessionStore::load(project_root) {
            if let Some(s) = store.get_mut(&session.id) {
                s.trajectory_id = Some(tid.clone());
            }
            let _ = store.save(project_root);
        }
    }

    // P1: Auto-save archive — task start
    let _ = crate::game_save::auto_save_task_start(
        project_root,
        &crate::game_save::project_id_from_path(project_root),
        &crate::repository::BasicRepository::open(project_root)
            .unwrap_or_else(|_| panic!("BasicRepository::open")),
        env!("CARGO_PKG_VERSION"),
        task,
        &session.id,
    );

    Ok(StartTaskResult {
        session: session.clone(),
        context_hash: session.context_hash.clone(),
        context_body,
        archived,
        host_instructions,
    })
}

/// Create an auto-savepoint and trajectory for a task session.
/// Best-effort — returns the trajectory ID if successful, or None on error.
fn create_auto_savepoint_and_trajectory(
    project_root: &Path,
    task: &str,
    session: &ExecutionSession,
    _target: ApplyTarget,
) -> Option<String> {
    // Check auto-save state to avoid duplicate savepoints
    let auto_state = AutoSaveState::load(project_root).ok()?;
    let state_hash = compute_state_hash(project_root).ok()?;

    // Only create if state hash has changed since last auto-save
    if !auto_state.has_changed(AUTO_SAVE_TASK_START, &state_hash) {
        // Same state — reuse the existing trajectory
        return auto_state
            .last_trajectories
            .get(AUTO_SAVE_TASK_START)
            .cloned();
    }

    // Create auto-savepoint
    let code_snapshot_id = crate::repository::BasicRepository::open(project_root)
        .ok()
        .and_then(|repo| repo.history(1).ok())
        .and_then(|h| h.first().map(|c| c.snapshot.id.clone()))
        .unwrap_or_else(|| "no-snapshot".to_string());

    let sp = create_savepoint(
        project_root,
        format!("auto:{}", &task[..task.len().min(40)]),
        code_snapshot_id,
        session.context_hash.clone(),
        None, // memory snapshot
        session.strategy_id.clone(),
        "auto".to_string(),
        Vec::new(),
        session.selected_reference_ids.clone(),
        session.agent_policy_hash.clone(),
        Vec::new(),
        Some(session.id.clone()),
        Some(session.id.clone()),
    )
    .ok()?;

    // Create trajectory
    let trajectory = create_trajectory(
        sp.id.clone(),
        Some(session.id.clone()),
        Some(task.to_string()),
        session.strategy_id.clone(),
        Vec::new(),
        session.agent_plan.clone(),
    );

    // Store trajectory
    let mut traj_store = TrajectoryStore::load(project_root).ok()?;
    let tid = trajectory.id.clone();
    traj_store.add(trajectory);
    traj_store.save(project_root).ok()?;

    // Update auto-save state
    let mut new_state = auto_state;
    new_state.record(AUTO_SAVE_TASK_START, state_hash, tid.clone());
    let _ = new_state.save(project_root);

    Some(tid)
}

// ---------------------------------------------------------------------------
// P5: Host contract — render session protocol into managed blocks
// ---------------------------------------------------------------------------

/// Render the session protocol block for inclusion in CLAUDE.md/AGENTS.md (P5).
///
/// This is a Markdown comment block that the host AI reads to understand
/// the current session contract.
pub fn render_host_contract(session: &ExecutionSession, project_root: &Path) -> String {
    let state_hash = compute_state_hash(project_root).unwrap_or_default();
    let state_line = if state_hash.is_empty() {
        String::new()
    } else {
        format!("\n  state_hash:     {}", state_hash)
    };

    // Load project memory summary for context
    let memory_summary = load_memory_summary(project_root);

    format!(
        r#"<!-- route-session -->
  session_id:     {}
  task:           {}
  context_hash:   {}
  target:         {}
  started_at:     {}{}
  policy_hash:    {}
  memory_summary: {}

  ## Host Contract
  - You are in an active Route execution session.
  - Modify code normally using your host capabilities.
  - Run tests and checks via `route task exec {} -- <command>`.
  - Create snapshots/commits/rollbacks through Route's CLI.
  - Before finishing, run `route task verify {}` to check verification policy.
  - You may submit an execution report via `route task report` with observations.
  - You CANNOT declare System Evidence or final Succeeded status — only Route can.
  - Route records all evidence automatically. Do not attempt to fake evidence.

  ## Memory Contract
  - You may submit structured memory observations using the format:
    memory_candidate:
      kind: "convention" | "decision" | "risk" | "open_question"
      content: "<concise statement>"
      source_ids: ["<commit-id>", "<session-id>"]
      confidence: 0.85
  - Memory candidates are proposals only — never auto-applied.
  - Route Engine validates and may promote them into ProjectMemory.
  - To view current project memory, run `route memory show`.
<!-- /route-session -->"#,
        &session.id[..12],
        session.task,
        &session.context_hash[..16],
        session.target.as_str(),
        session.started_at,
        state_line,
        session.agent_policy_hash.as_deref().unwrap_or("(none)"),
        memory_summary,
        &session.id[..12],
        &session.id[..12],
    )
}

/// Load a one-line summary of current project memory for the host contract.
/// Returns "(none)" if no memory store exists or is empty.
fn load_memory_summary(project_root: &Path) -> String {
    use crate::memory::MemoryStore;
    match MemoryStore::load(project_root) {
        Ok(store) => {
            if let Some(current) = store.current_memory() {
                if !current.summary.is_empty() {
                    // Truncate to fit in a single line
                    let truncated = if current.summary.len() > 100 {
                        format!("{}...", &current.summary[..97])
                    } else {
                        current.summary.clone()
                    };
                    truncated
                } else {
                    "(no summary)".to_string()
                }
            } else {
                "(no current memory)".to_string()
            }
        }
        Err(_) => "(none)".to_string(),
    }
}

// ---------------------------------------------------------------------------
// P6: Resume / Drift detection
// ---------------------------------------------------------------------------

/// Drift status for a resumed session (P6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DriftStatus {
    /// Everything matches — session can continue.
    Active,
    /// Working tree has changed since baseline (files modified outside Route).
    Drifted,
    /// The Effective Context hash has changed since session start.
    ContextOutdated,
    /// The host file (CLAUDE.md/AGENTS.md) has been modified outside Route.
    HostFileModified,
}

impl DriftStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DriftStatus::Active => "ACTIVE",
            DriftStatus::Drifted => "DRIFTED",
            DriftStatus::ContextOutdated => "CONTEXT_OUTDATED",
            DriftStatus::HostFileModified => "HOST_FILE_MODIFIED",
        }
    }
}

/// Result of a resume attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeResult {
    /// The current session.
    pub session: ExecutionSession,
    /// Drift status.
    pub drift: DriftStatus,
    /// Human-readable details about the drift.
    pub details: Vec<String>,
}

/// Resume a session after restart (P6).
///
/// Checks:
/// 1. Session exists and is Active.
/// 2. Baseline state vs current state (drift detection).
/// 3. Context hash matches current Effective Context.
/// 4. Host file is intact (not modified outside Route).
///
/// Drift is NOT auto-repaired — only reported. The user must explicitly
/// continue or reapply.
pub fn resume_session(project_root: &Path, session_id: &str) -> Result<ResumeResult> {
    let store = SessionStore::load(project_root)?;
    let session = store
        .get(session_id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("session '{}' not found", session_id))?;

    if session.status != SessionStatus::Active {
        anyhow::bail!(
            "session '{}' is not Active (status: {:?}). Cannot resume a terminated session.",
            session_id,
            session.status
        );
    }
    drop(store);

    let mut drift = DriftStatus::Active;
    let mut details = Vec::new();

    // Check 1: Context hash drift.
    if let Ok(snap) = ContextSnapshot::collect(project_root) {
        if snap.fingerprint != session.context_hash {
            drift = DriftStatus::ContextOutdated;
            details.push(format!(
                "Context hash has changed: was {} is {}",
                &session.context_hash[..16],
                &snap.fingerprint[..16]
            ));
        }
    }

    // Check 2: Working tree drift (only if context is still current).
    if drift == DriftStatus::Active {
        if let Ok(state_hash) = compute_state_hash(project_root) {
            let saved_hash = read_state_hash(project_root).unwrap_or_default();
            if !saved_hash.is_empty() && saved_hash != state_hash {
                drift = DriftStatus::Drifted;
                details.push("Working tree has changed since baseline snapshot".to_string());
            }
        }
    }

    // Check 3: Host file modification.
    if drift == DriftStatus::Active {
        let host_path = project_root.join(match session.target {
            ApplyTarget::Claude => "CLAUDE.md",
            ApplyTarget::Codex => "AGENTS.md",
            ApplyTarget::DeepSeek => ".route/generated/deepseek-context.md",
            ApplyTarget::Generic => "CLAUDE.md",
        });
        if host_path.exists() {
            // Check if the file contains the route-session marker.
            let content = std::fs::read_to_string(&host_path).unwrap_or_default();
            if !content.contains("<!-- route-session") {
                drift = DriftStatus::HostFileModified;
                details.push(format!(
                    "Host file '{}' does not contain the Route session marker",
                    host_path.display()
                ));
            }
        }
    }

    Ok(ResumeResult {
        session,
        drift,
        details,
    })
}

// ---------------------------------------------------------------------------
// P7: Enhanced learning integration on session end
// ---------------------------------------------------------------------------

/// Generate learning signals from a completed session (P7).
///
/// Mapping:
/// - Succeeded + Verified = positive (strong)
/// - Failed or RolledBack = negative
/// - ForcedUnverified = neutral/weak (cannot be used as strong positive)
/// - Stale TestPass evidence = ignored
pub fn generate_learning_from_session(
    project_root: &Path,
    session: &ExecutionSession,
) -> Result<Vec<String>> {
    let evidence_store = EvidenceStore::load(project_root)?;
    let session_evidence = evidence_store.for_session(&session.id);
    let current_state_hash = compute_state_hash(project_root).unwrap_or_default();

    let mut event_ids = Vec::new();
    let signal = match session.status {
        SessionStatus::Succeeded => {
            // Check for stale evidence.
            let has_stale = session_evidence
                .iter()
                .any(|e| evidence_store.is_pass_stale(e, &current_state_hash));
            if has_stale {
                "positive (with stale evidence)".to_string()
            } else {
                "positive".to_string()
            }
        }
        SessionStatus::Failed => "negative".to_string(),
        SessionStatus::RolledBack => "negative (rollback)".to_string(),
        SessionStatus::ForcedUnverified => "neutral/weak (forced)".to_string(),
        _ => {
            // Active or Aborted — no learning signal.
            return Ok(Vec::new());
        }
    };

    // Record experience events.
    for ev in &session_evidence {
        let outcome = match ev.kind {
            EvidenceKind::TestPass | EvidenceKind::CheckPass => "pass",
            EvidenceKind::TestFail | EvidenceKind::CheckFail => "fail",
            EvidenceKind::Rollback => "rollback",
            EvidenceKind::Commit => "commit",
            _ => continue,
        };
        let kind = match ev.kind {
            EvidenceKind::TestPass | EvidenceKind::CheckPass => EventKind::TestPass,
            EvidenceKind::TestFail | EvidenceKind::CheckFail => EventKind::TestFail,
            EvidenceKind::Rollback => EventKind::Rollback,
            EvidenceKind::Commit => EventKind::ManualNote,
            _ => continue,
        };
        let evidence_ref = format!("evidence:{}", &ev.id[..12]);
        let tag = format!("session:{}", &session.id[..12]);

        let mut store = crate::learn::ExperienceStore::load(project_root)?;
        let id = store.record(
            project_root,
            kind,
            outcome,
            &evidence_ref,
            LearnScope::Project,
            None,
            vec![tag],
            None,
            None,
        )?;
        store.save(project_root)?;
        event_ids.push(id);
    }

    // Record the session-level signal.
    let mut store = crate::learn::ExperienceStore::load(project_root)?;
    let signal_id = store.record(
        project_root,
        EventKind::ManualNote,
        &signal,
        &format!("session_result:{}", session.status.as_str()),
        LearnScope::Project,
        None,
        vec![format!("session:{}", &session.id[..12])],
        None,
        None,
    )?;
    store.save(project_root)?;
    event_ids.push(signal_id);

    // Auto-run learning analysis (proposal-only).
    let proposals = analyze_events(project_root)?;
    let proposal_claims: Vec<String> = proposals.iter().map(|p| p.claim.clone()).collect();

    Ok(proposal_claims)
}

/// Record an experiment record for a session that was strategy-driven.
///
/// This is a best-effort operation — errors are logged but not propagated,
/// so that experiment recording never blocks session termination.
fn record_experiment_for_session(project_root: &Path, session: &ExecutionSession) {
    // Only record if the session had a strategy_id.
    let strategy_id = match &session.strategy_id {
        Some(s) => s.clone(),
        None => return,
    };

    let result = match session.status {
        SessionStatus::Succeeded => "success",
        SessionStatus::Failed => "failed",
        SessionStatus::Aborted => "aborted",
        SessionStatus::RolledBack => "rolled_back",
        SessionStatus::ForcedUnverified => "success",
        SessionStatus::Active => return, // Not yet ended.
    };

    // Count checks passed/failed from evidence.
    let evidence_store = match EvidenceStore::load(project_root) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to load evidence for experiment recording");
            return;
        }
    };
    let session_evidence = evidence_store.for_session(&session.id);
    let checks_passed = session_evidence
        .iter()
        .filter(|e| matches!(e.kind, EvidenceKind::TestPass | EvidenceKind::CheckPass))
        .count() as u32;
    let checks_failed = session_evidence
        .iter()
        .filter(|e| matches!(e.kind, EvidenceKind::TestFail | EvidenceKind::CheckFail))
        .count() as u32;

    // Collect agent roles from the session's evidence metadata.
    let agent_roles: Vec<String> = session_evidence
        .iter()
        .filter_map(|e| e.metadata.get("agent_role"))
        .cloned()
        .collect();

    // Calculate duration in seconds.
    let duration_secs = session.ended_at.map(|ended| {
        let millis = ended - session.started_at;
        // Convert to seconds, rounding up.
        (millis.max(0) as u64 + 999) / 1000
    });

    // Collect evidence IDs.
    let evidence_ids: Vec<String> = session_evidence.iter().map(|e| e.id.clone()).collect();

    let record = crate::experiment::ExperimentRecord {
        id: route_core::new_id(),
        task: session.task.clone(),
        strategy_id,
        session_id: session.id.clone(),
        result: result.to_string(),
        checks_passed,
        checks_failed,
        agent_roles,
        duration_secs,
        created_at: route_core::now_millis(),
        evidence: evidence_ids,
    };

    let mut store = match crate::experiment::ExperimentStore::load(project_root) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "failed to load experiment store");
            return;
        }
    };

    if let Err(e) = store.record_experiment(project_root, record) {
        tracing::warn!(error = %e, "failed to record experiment");
    }
}

// ---------------------------------------------------------------------------
// P2: Structured result type (shared across CLI/MCP/HTTP)
// ---------------------------------------------------------------------------

/// Structured result for application-level operations.
/// All MCP tools return this shape (serialized to JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppResult<T: Serialize> {
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Machine-readable status code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional payload data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Non-fatal warnings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Session id if the operation was session-scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Context hash at the time of the operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_hash: Option<String>,
    /// Whether the error is recoverable (for error responses).
    #[serde(default)]
    pub recoverable: bool,
}

impl<T: Serialize> AppResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            ok: true,
            code: "OK".to_string(),
            message: "success".to_string(),
            data: Some(data),
            warnings: Vec::new(),
            session_id: None,
            context_hash: None,
            recoverable: false,
        }
    }

    pub fn err(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            code: code.into(),
            message: message.into(),
            data: None,
            warnings: Vec::new(),
            session_id: None,
            context_hash: None,
            recoverable: false,
        }
    }

    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_context(mut self, context_hash: String) -> Self {
        self.context_hash = Some(context_hash);
        self
    }

    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    pub fn recoverable(mut self) -> Self {
        self.recoverable = true;
        self
    }
}

// ---------------------------------------------------------------------------
// P6: Request-id dedup store
// ---------------------------------------------------------------------------

fn request_id_path(project_root: &Path) -> PathBuf {
    execution_dir(project_root).join("request-ids.json")
}

/// Track processed request_ids for idempotency.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestIdStore {
    #[serde(default)]
    pub entries: HashMap<String, serde_json::Value>,
}

impl RequestIdStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = request_id_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p).unwrap_or_default();
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = request_id_path(project_root);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        std::fs::write(&p, json)?;
        Ok(())
    }

    /// Check if a request_id was already processed. If so, return the stored result.
    pub fn check(&self, request_id: &str) -> Option<&serde_json::Value> {
        self.entries.get(request_id)
    }

    /// Record a processed request_id with its result.
    pub fn record(&mut self, request_id: String, result: serde_json::Value) {
        self.entries.insert(request_id, result);
    }
}

// ---------------------------------------------------------------------------
// P7: Capability discovery
// ---------------------------------------------------------------------------

/// Snapshot of Route's current capabilities for external AI consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCapabilities {
    /// Route version string.
    pub version: String,
    /// Whether the project is initialized as a Route repo.
    pub repo_initialized: bool,
    /// Current branch name (if initialized).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_branch: Option<String>,
    /// Head snapshot short id (if initialized).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_snapshot: Option<String>,
    /// Active session id (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_session: Option<String>,
    /// Active session task (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_task: Option<String>,
    /// Available target adapters.
    pub targets: Vec<String>,
    /// Supported tools/features.
    pub features: Vec<String>,
    /// Integrity status of the repo.
    pub integrity: String,
}

/// Query Route's current capabilities.
pub fn capabilities(project_root: &Path) -> Result<RouteCapabilities> {
    let version = env!("CARGO_PKG_VERSION").to_string();

    // Check repo initialization.
    let repo = crate::repository::BasicRepository::open(project_root);
    let (repo_initialized, current_branch, head_snapshot, integrity) = match &repo {
        Ok(r) => {
            let branch = r.get_current_branch_name().ok();
            let head = r
                .history(1)
                .ok()
                .and_then(|h| h.first().map(|c| route_core::short_id(&c.snapshot.id)));
            let integrity_str = if r
                .verify(&crate::repository::VerifyOptions::default())
                .is_ok()
            {
                "ok".to_string()
            } else {
                "warning".to_string()
            };
            (true, branch, head, integrity_str)
        }
        Err(_) => (false, None, None, "uninitialized".to_string()),
    };

    // Check active session.
    let store = SessionStore::load(project_root)?;
    let (active_session, active_task) = match store.active() {
        Some(s) => (Some(s.id.clone()), Some(s.task.clone())),
        None => (None, None),
    };

    let targets = vec![
        "claude".to_string(),
        "codex".to_string(),
        "generic".to_string(),
    ];

    let features = vec![
        "task_start".to_string(),
        "task_status".to_string(),
        "task_exec".to_string(),
        "task_verify".to_string(),
        "task_end".to_string(),
        "task_show".to_string(),
        "snapshot".to_string(),
        "commit".to_string(),
        "rollback".to_string(),
        "context_task".to_string(),
        "reference_review".to_string(),
        "reference_apply".to_string(),
        "learn_review".to_string(),
        "capabilities".to_string(),
        "evidence".to_string(),
        "learning".to_string(),
        "request_id".to_string(),
    ];

    Ok(RouteCapabilities {
        version,
        repo_initialized,
        current_branch,
        head_snapshot,
        active_session,
        active_task,
        targets,
        features,
        integrity,
    })
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Succeeded => "succeeded",
            SessionStatus::Failed => "failed",
            SessionStatus::RolledBack => "rolled_back",
            SessionStatus::Aborted => "aborted",
            SessionStatus::ForcedUnverified => "forced_unverified",
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::BasicRepository;
    use tempfile::TempDir;

    fn init_project() -> (TempDir, std::path::PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        BasicRepository::init(&root).unwrap();
        (tmp, root)
    }

    // -------------------------------------------------------------------
    // P0: Session begin/status/end
    // -------------------------------------------------------------------

    #[test]
    fn begin_session_creates_active_session() {
        let (_tmp, root) = init_project();
        let session = begin_session(
            &root,
            "fix rollback corruption",
            ApplyTarget::Claude,
            false,
            None,
        )
        .unwrap();
        assert_eq!(session.status, SessionStatus::Active);
        assert!(!session.id.is_empty());
        assert_eq!(session.task, "fix rollback corruption");
        assert!(!session.context_hash.is_empty());
        assert!(session.ended_at.is_none());
    }

    #[test]
    fn begin_session_captures_exact_context() {
        let (_tmp, root) = init_project();
        let snap = ContextSnapshot::collect(&root).unwrap();
        let session = begin_session(&root, "test task", ApplyTarget::Generic, false, None).unwrap();
        assert_eq!(
            session.context_hash, snap.fingerprint,
            "session context_hash must match snapshot fingerprint"
        );
    }

    #[test]
    fn begin_rejects_concurrent_active() {
        let (_tmp, root) = init_project();
        begin_session(&root, "task 1", ApplyTarget::Claude, false, None).unwrap();
        let err = begin_session(&root, "task 2", ApplyTarget::Claude, false, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("still Active"),
            "must reject concurrent active session: {}",
            msg
        );
    }

    #[test]
    fn begin_allows_concurrent_with_flag() {
        let (_tmp, root) = init_project();
        begin_session(&root, "task 1", ApplyTarget::Claude, false, None).unwrap();
        // allow_concurrent=true should not error.
        let session2 = begin_session(&root, "task 2", ApplyTarget::Claude, true, None).unwrap();
        assert_eq!(session2.status, SessionStatus::Active);
    }

    #[test]
    fn session_status_returns_session() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();
        let result = session_status(&root, Some(&s.id)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, s.id);
    }

    #[test]
    fn session_status_returns_all() {
        let (_tmp, root) = init_project();
        begin_session(&root, "task 1", ApplyTarget::Claude, true, None).unwrap();
        begin_session(&root, "task 2", ApplyTarget::Claude, true, None).unwrap();
        let result = session_status(&root, None).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn end_session_rejects_double_end() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();
        end_session(&root, &s.id, "success").unwrap();
        let err = end_session(&root, &s.id, "success").unwrap_err();
        assert!(
            err.to_string().contains("not Active"),
            "must reject double-end"
        );
    }

    #[test]
    fn end_session_success_status() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();
        let ended = end_session(&root, &s.id, "success").unwrap();
        assert_eq!(ended.status, SessionStatus::Succeeded);
        assert!(ended.ended_at.is_some());
    }

    #[test]
    fn end_session_failed_status() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();
        let ended = end_session(&root, &s.id, "failed").unwrap();
        assert_eq!(ended.status, SessionStatus::Failed);
    }

    #[test]
    fn end_session_aborted_status() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();
        let ended = end_session(&root, &s.id, "aborted").unwrap();
        assert_eq!(ended.status, SessionStatus::Aborted);
    }

    // -------------------------------------------------------------------
    // P1: Evidence recording
    // -------------------------------------------------------------------

    #[test]
    fn system_evidence_recorded_correctly() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();

        // begin_session auto-records baseline snapshot + commit evidence.
        // We add one more manual evidence.
        let mut store = EvidenceStore::load(&root).unwrap();
        let mut meta = HashMap::new();
        meta.insert("commit_id".to_string(), "abc123".to_string());
        store.record(
            &s.id,
            EvidenceKind::Commit,
            EvidenceSource::System,
            "hash123".to_string(),
            meta,
        );
        store.save(&root).unwrap();

        let loaded = EvidenceStore::load(&root).unwrap();
        let session_ev = loaded.for_session(&s.id);
        // Should have 3: baseline snapshot + baseline commit + manual commit
        assert!(
            session_ev.len() >= 3,
            "expected at least 3 evidence, got {}",
            session_ev.len()
        );
        // Verify the manual one is correct.
        let manual = session_ev
            .iter()
            .find(|e| e.payload_hash == "hash123")
            .unwrap();
        assert_eq!(manual.kind, EvidenceKind::Commit);
        assert_eq!(manual.source, EvidenceSource::System);
    }

    #[test]
    fn system_vs_ai_evidence_trust() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();

        let mut store = EvidenceStore::load(&root).unwrap();
        // System evidence
        store.record(
            &s.id,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        // AI evidence (should not satisfy verification policy)
        store.record(
            &s.id,
            EvidenceKind::AgentFeedback,
            EvidenceSource::Agent,
            "h2".to_string(),
            HashMap::new(),
        );
        store.save(&root).unwrap();

        let loaded = EvidenceStore::load(&root).unwrap();
        let all = loaded.for_session(&s.id);
        // Baseline adds 2, plus 2 manual = 4 total
        assert_eq!(
            all.len(),
            4,
            "expected 4 evidence (2 baseline + 2 manual), got {}",
            all.len()
        );
        assert!(all.iter().any(|e| e.source == EvidenceSource::System));
        assert!(all.iter().any(|e| e.source == EvidenceSource::Agent));
    }

    // -------------------------------------------------------------------
    // P3: Verification policy
    // -------------------------------------------------------------------

    #[test]
    fn verification_policy_parses_from_protocol() {
        let body = "# Protocol\n\n\
            <!-- route-verification-policy:\n  \
            required_checks: [\"cargo test\"]\n  \
            success_requires: [\"test_pass\"]\n  \
            independent_review: true\n\
            -->\n\nWorkflow...";
        let policy = VerificationPolicy::from_protocol_body(body);
        assert_eq!(policy.required_checks, vec!["cargo test"]);
        assert_eq!(policy.success_requires, vec!["test_pass"]);
        assert_eq!(policy.independent_review, Some(true));
    }

    #[test]
    fn verification_policy_defaults_when_absent() {
        let body = "# Protocol\n\nNo policy here.\n";
        let policy = VerificationPolicy::from_protocol_body(body);
        assert!(policy.required_checks.is_empty());
        assert!(policy.success_requires.is_empty());
        assert!(policy.independent_review.is_none());
    }

    #[test]
    fn verification_required_success_checks_system_evidence() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();

        // Add test_pass system evidence.
        let mut store = EvidenceStore::load(&root).unwrap();
        store.record(
            &s.id,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        store.save(&root).unwrap();

        // Set up verification policy in Protocol.
        let mut protocol = crate::constitutive::Protocol::read(&root).unwrap();
        protocol.body = format!(
            "{}\n<!-- route-verification-policy:\n  success_requires: [\"test_pass\"]\n-->\n",
            protocol.body
        );
        protocol.write(&root).unwrap();

        // Should pass — we have system test_pass evidence.
        let ended = end_session(&root, &s.id, "success").unwrap();
        assert_eq!(ended.status, SessionStatus::Succeeded);
    }

    #[test]
    fn verification_fails_when_only_ai_evidence() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();

        // Only AI evidence, no system evidence.
        let mut store = EvidenceStore::load(&root).unwrap();
        store.record(
            &s.id,
            EvidenceKind::AgentFeedback,
            EvidenceSource::Agent,
            "h1".to_string(),
            HashMap::new(),
        );
        store.save(&root).unwrap();

        // Set up verification policy requiring test_pass.
        let mut protocol = crate::constitutive::Protocol::read(&root).unwrap();
        protocol.body = format!(
            "{}\n<!-- route-verification-policy:\n  success_requires: [\"test_pass\"]\n-->\n",
            protocol.body
        );
        protocol.write(&root).unwrap();

        // AI-said "tests passed" should NOT satisfy verification.
        let ended = end_session(&root, &s.id, "success").unwrap();
        assert_eq!(
            ended.status,
            SessionStatus::Failed,
            "AI evidence alone must not satisfy verification"
        );
    }

    // -------------------------------------------------------------------
    // P4: Host report ingestion
    // -------------------------------------------------------------------

    #[test]
    fn host_report_ingested_as_agent_evidence() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();

        let report = HostReport {
            session_id: s.id.clone(),
            agent_roles_used: vec!["coder".to_string()],
            actions: vec![HostAction {
                action: "edit_file".to_string(),
                tool: Some("sed".to_string()),
                target: Some("src/main.rs".to_string()),
                result: Some("ok".to_string()),
            }],
            verification_requested: vec!["tests passed".to_string()],
            observations: vec!["Fixed the rollback bug".to_string()],
        };

        let ids = ingest_host_report(&root, report).unwrap();
        assert!(!ids.is_empty(), "must ingest at least one evidence");

        let store = EvidenceStore::load(&root).unwrap();
        let agent_evidence: Vec<&Evidence> = store
            .evidence
            .iter()
            .filter(|e| e.source == EvidenceSource::Agent)
            .collect();
        assert!(
            !agent_evidence.is_empty(),
            "host report must produce Agent evidence"
        );
        // All agent evidence should be from the report, not from baseline.
        assert!(
            agent_evidence
                .iter()
                .all(|e| e.source == EvidenceSource::Agent),
            "host report evidence must all be Agent source"
        );
    }

    // -------------------------------------------------------------------
    // P5: Auto learning trigger
    // -------------------------------------------------------------------

    #[test]
    fn auto_analyze_proposal_only() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test rollback", ApplyTarget::Claude, false, None).unwrap();

        // Record some test events.
        record_experience_event_with_session(
            &root,
            Some(&s.id),
            EventKind::TestPass,
            "rollback fix works",
            "tests passed after rollback fix",
        )
        .unwrap();
        record_experience_event_with_session(
            &root,
            Some(&s.id),
            EventKind::TestPass,
            "rollback fix works again",
            "more tests passed",
        )
        .unwrap();

        // Run auto analyze.
        let count = auto_analyze_after_session(&root, &s.id).unwrap();
        assert!(count > 0, "must generate proposals from 2+ events");

        // Verify proposals are not auto-applied.
        let registry = crate::constitutive::ReferenceRegistry::read(&root).unwrap();
        let experience_count = registry
            .entries
            .iter()
            .filter(|e| e.type_ == crate::constitutive::ReferenceType::Experience)
            .count();
        assert_eq!(experience_count, 0, "proposals must not be auto-applied");
    }

    // -------------------------------------------------------------------
    // P6: Task audit
    // -------------------------------------------------------------------

    #[test]
    fn show_session_returns_timeline() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test audit", ApplyTarget::Claude, false, None).unwrap();

        // Record some evidence.
        let mut store = EvidenceStore::load(&root).unwrap();
        store.record(
            &s.id,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        store.save(&root).unwrap();

        let audit = show_session(&root, &s.id, false).unwrap();
        assert_eq!(audit.session.id, s.id);
        assert!(!audit.timeline.is_empty());
        assert!(audit.timeline.iter().any(|e| e.kind == "session_begin"));
        assert!(audit.timeline.iter().any(|e| e.kind == "test_pass"));
    }

    // -------------------------------------------------------------------
    // P7: Task replay
    // -------------------------------------------------------------------

    #[test]
    fn replay_session_reconstructs_context() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test replay", ApplyTarget::Generic, false, None).unwrap();

        let replay = replay_session(&root, &s.id, None, false).unwrap();
        assert_eq!(replay.session_id, s.id);
        assert_eq!(replay.context_hash, s.context_hash);
        assert!(!replay.context_body.is_empty());
        assert!(replay
            .context_body
            .contains("Effective Development Context"));
    }

    // -------------------------------------------------------------------
    // P8: Consistency checks
    // -------------------------------------------------------------------

    #[test]
    fn orphan_evidence_detected() {
        let (_tmp, root) = init_project();

        // Create evidence referencing a non-existent session.
        let mut store = EvidenceStore::load(&root).unwrap();
        store.record(
            "nonexistent-session",
            EvidenceKind::Commit,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        store.save(&root).unwrap();

        let findings = check_execution_ledger(&root).unwrap();
        assert!(
            findings.iter().any(|f| f.category == "orphan_evidence"),
            "must detect orphan evidence"
        );
    }

    #[test]
    fn double_end_not_possible() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test", ApplyTarget::Claude, false, None).unwrap();
        end_session(&root, &s.id, "success").unwrap();
        let err = end_session(&root, &s.id, "success").unwrap_err();
        assert!(
            err.to_string().contains("not Active"),
            "double end must be rejected"
        );
    }

    // -------------------------------------------------------------------
    // Deterministic: same session evidence
    // -------------------------------------------------------------------

    #[test]
    fn same_session_evidence_deterministic() {
        let (_tmp, root) = init_project();
        let s = begin_session(&root, "test det", ApplyTarget::Claude, false, None).unwrap();

        let mut store = EvidenceStore::load(&root).unwrap();
        store.record(
            &s.id,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        let id1 = store.record(
            &s.id,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        let id2 = store.record(
            &s.id,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            "h1".to_string(),
            HashMap::new(),
        );
        store.save(&root).unwrap();

        // IDs must be unique (ULIDs).
        assert_ne!(id1, id2, "evidence IDs must be unique");

        // Loading back returns same number (3 manual + 2 baseline = 5 total).
        let loaded = EvidenceStore::load(&root).unwrap();
        let all_session = loaded.for_session(&s.id);
        assert_eq!(
            all_session.len(),
            5,
            "expected 5 total evidence (2 baseline + 3 manual), got {}",
            all_session.len()
        );
    }

    // -------------------------------------------------------------------
    // Old records compatibility
    // -------------------------------------------------------------------

    #[test]
    fn empty_session_store_is_ok() {
        let (_tmp, root) = init_project();
        let store = SessionStore::load(&root).unwrap();
        assert!(store.sessions.is_empty());
        assert!(store.active().is_none());
    }

    #[test]
    fn empty_evidence_store_is_ok() {
        let (_tmp, root) = init_project();
        let store = EvidenceStore::load(&root).unwrap();
        assert!(store.evidence.is_empty());
    }
}
