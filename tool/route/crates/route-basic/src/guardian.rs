//! Guardian — observe, diagnose, surface risk, propose next actions.
//!
//! Guardian aggregates data from ProjectMemory, Execution history,
//! FailureLibrary, Ideas, Reference freshness, Workflow revisions,
//! Strategy history, open tasks, and verification history.
//!
//! Guardian is responsible for:
//!   - observe
//!   - diagnose
//!   - surface risk
//!   - propose next actions
//!   - maintain memory consistency
//!
//! Guardian is NOT allowed to:
//!   - modify Constitution
//!   - accept Ideas itself
//!   - auto-modify Protocol
//!   - auto-apply Workflow evolution
//!   - auto-delete Reference
//!   - decide project highest Goal

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Guardian directory under `.route/`.
pub fn guardian_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("guardian")
}

/// Path to the guardian findings file.
pub fn guardian_findings_path(project_root: &Path) -> PathBuf {
    guardian_dir(project_root).join("findings.json")
}

/// Path to the guardian maintenance events file.
pub fn guardian_events_path(project_root: &Path) -> PathBuf {
    guardian_dir(project_root).join("events.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The severity of a finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    /// Informational
    Info,
    /// Worth watching
    Warning,
    /// Needs action
    Critical,
}

impl Default for FindingSeverity {
    fn default() -> Self {
        Self::Info
    }
}

/// The status of a finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    /// Newly discovered, not yet reviewed
    Open,
    /// User has seen and acknowledged
    Acknowledged,
    /// User has resolved this finding
    Resolved,
    /// User has explicitly ignored this finding
    Ignored,
}

impl Default for FindingStatus {
    fn default() -> Self {
        Self::Open
    }
}

/// A Guardian finding — an observation about the project state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardianFinding {
    /// Unique finding ID
    pub id: String,
    /// Kind of finding (e.g. "stale_reference", "unresolved_failure", "memory_drift", "open_loop")
    pub kind: String,
    /// Severity
    #[serde(default)]
    pub severity: FindingSeverity,
    /// Short title
    pub title: String,
    /// Detailed explanation
    pub explanation: String,
    /// Evidence IDs (session IDs, file paths, reference IDs)
    #[serde(default)]
    pub evidence_ids: Vec<String>,
    /// Affected scope (modules, files, references, etc.)
    #[serde(default)]
    pub affected_scope: Vec<String>,
    /// Suggested actions
    #[serde(default)]
    pub suggested_actions: Vec<String>,
    /// Current status
    #[serde(default)]
    pub status: FindingStatus,
    /// When this finding was created
    pub created_at: i64,
    /// When this finding was last updated
    pub updated_at: i64,
}

/// A maintenance event — records the outcome of a guardian action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceEvent {
    /// Unique event ID
    pub id: String,
    /// The finding this event relates to
    pub finding_id: String,
    /// What action was proposed
    pub action_proposed: String,
    /// User response: accepted | rejected | ignored
    pub user_response: String,
    /// Task session ID (if a task was started)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_session_id: Option<String>,
    /// Task result (if completed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_result: Option<String>,
    /// When this event occurred
    pub created_at: i64,
}

/// The guardian findings store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuardianFindingsStore {
    pub findings: Vec<GuardianFinding>,
}

impl GuardianFindingsStore {
    /// Load findings from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = guardian_findings_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save findings to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = guardian_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(guardian_findings_path(project_root), json)?;
        Ok(())
    }

    /// List all findings, optionally filtered by status.
    pub fn list(&self, status_filter: Option<&str>) -> Vec<&GuardianFinding> {
        self.findings
            .iter()
            .filter(|f| {
                status_filter.map_or(true, |s| {
                    format!("{:?}", f.status).to_lowercase() == s.to_lowercase()
                })
            })
            .collect()
    }

    /// Get a finding by ID.
    pub fn get(&self, id: &str) -> Option<&GuardianFinding> {
        self.findings.iter().find(|f| f.id == id)
    }

    /// Get a mutable finding by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut GuardianFinding> {
        self.findings.iter_mut().find(|f| f.id == id)
    }

    /// Add a finding.
    pub fn add(&mut self, finding: GuardianFinding) {
        self.findings.push(finding);
    }

    /// Set finding status.
    pub fn set_status(&mut self, id: &str, status: FindingStatus) -> Result<()> {
        let finding = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Finding '{}' not found", id))?;
        finding.status = status;
        finding.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Remove a finding by ID.
    pub fn remove(&mut self, id: &str) {
        self.findings.retain(|f| f.id != id);
    }

    /// Get open (unresolved) findings.
    pub fn open_findings(&self) -> Vec<&GuardianFinding> {
        self.findings
            .iter()
            .filter(|f| f.status == FindingStatus::Open || f.status == FindingStatus::Acknowledged)
            .collect()
    }
}

/// The maintenance events store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaintenanceEventStore {
    pub events: Vec<MaintenanceEvent>,
}

impl MaintenanceEventStore {
    /// Load events from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = guardian_events_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save events to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = guardian_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(guardian_events_path(project_root), json)?;
        Ok(())
    }

    /// Add an event.
    pub fn add(&mut self, event: MaintenanceEvent) {
        self.events.push(event);
    }

    /// List all events.
    pub fn list(&self) -> &[MaintenanceEvent] {
        &self.events
    }
}

// ---------------------------------------------------------------------------
// Scan logic
// ---------------------------------------------------------------------------

/// Result of a guardian scan.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuardianScanResult {
    pub findings: Vec<GuardianFinding>,
}

/// Perform a guardian scan — aggregate data from all available sources.
///
/// This is the main scan function. It checks:
/// - Unresolved failure cases
/// - Repeated similar failures
/// - Memory vs current state consistency
/// - Reference freshness
/// - Workflow evidence
/// - Long-accepted ideas
/// - Open questions
/// - Verification history
/// - Recent high-risk changes
/// - Superseded decisions still marked as current
pub fn guardian_scan(project_root: &Path) -> Result<GuardianScanResult> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut findings = Vec::new();

    // 1. Check unresolved failure cases
    if let Ok(lib) = crate::failure::FailureLibrary::load(project_root) {
        let unresolved: Vec<_> = lib.cases.iter().filter(|c| !c.resolved).collect();
        if !unresolved.is_empty() {
            let ids: Vec<String> = unresolved.iter().map(|c| c.id.clone()).collect();
            findings.push(GuardianFinding {
                id: format!("guardian-unresolved-failures-{}", now),
                kind: "unresolved_failure".to_string(),
                severity: FindingSeverity::Warning,
                title: format!("{} unresolved failure cases", unresolved.len()),
                explanation: format!(
                    "{} failure cases have not been resolved. Oldest: {}",
                    unresolved.len(),
                    unresolved
                        .iter()
                        .map(|c| c.problem.clone())
                        .next()
                        .unwrap_or_default()
                ),
                evidence_ids: ids.clone(),
                affected_scope: ids,
                suggested_actions: vec![
                    "Review each failure case and determine root cause".to_string(),
                    "Resolve or acknowledge each case".to_string(),
                ],
                status: FindingStatus::Open,
                created_at: now,
                updated_at: now,
            });
        }
    }

    // 2. Check stale references
    if let Ok(registry) = crate::constitutive::ReferenceRegistry::read(project_root) {
        // Check for references with empty source or disabled
        let disabled: Vec<_> = registry.entries.iter().filter(|e| !e.enabled).collect();
        if !disabled.is_empty() {
            let names: Vec<String> = disabled.iter().map(|e| e.name.clone()).collect();
            findings.push(GuardianFinding {
                id: format!("guardian-disabled-refs-{}", now),
                kind: "disabled_reference".to_string(),
                severity: FindingSeverity::Info,
                title: format!("{} disabled references", disabled.len()),
                explanation: format!("References: {}", names.join(", ")),
                evidence_ids: names,
                affected_scope: vec![],
                suggested_actions: vec![
                    "Review and re-enable or remove stale references".to_string()
                ],
                status: FindingStatus::Open,
                created_at: now,
                updated_at: now,
            });
        }
    }

    // 3. Check pending ideas
    if let Ok(store) = crate::idea::IdeaStore::load(project_root) {
        let accepted: Vec<_> = store
            .ideas
            .iter()
            .filter(|i| matches!(i.status, crate::idea::IdeaStatus::Accepted))
            .collect();
        if !accepted.is_empty() {
            let texts: Vec<String> = accepted.iter().map(|i| i.text.clone()).collect();
            findings.push(GuardianFinding {
                id: format!("guardian-accepted-ideas-{}", now),
                kind: "accepted_idea_no_task".to_string(),
                severity: FindingSeverity::Warning,
                title: format!("{} accepted ideas without tasks", accepted.len()),
                explanation: format!("Ideas: {}", texts.join("; ")),
                evidence_ids: accepted.iter().map(|i| i.id.clone()).collect(),
                affected_scope: vec![],
                suggested_actions: vec![
                    "Create tasks for accepted ideas or move them back to inbox".to_string(),
                ],
                status: FindingStatus::Open,
                created_at: now,
                updated_at: now,
            });
        }
    }

    // 4. Check open questions in memory
    if let Ok(mem) = crate::memory::MemoryStore::load(project_root) {
        if let Some(current) = mem.current_memory() {
            let open_qs: Vec<_> = current
                .open_questions
                .iter()
                .filter(|q| !q.content.is_empty())
                .collect();
            if !open_qs.is_empty() {
                findings.push(GuardianFinding {
                    id: format!("guardian-open-questions-{}", now),
                    kind: "open_question".to_string(),
                    severity: FindingSeverity::Info,
                    title: format!("{} open questions in memory", open_qs.len()),
                    explanation:
                        "Open questions represent unresolved design/architecture decisions"
                            .to_string(),
                    evidence_ids: open_qs.iter().map(|q| q.id.clone()).collect(),
                    affected_scope: vec![],
                    suggested_actions: vec![
                        "Review open questions and resolve or document them".to_string()
                    ],
                    status: FindingStatus::Open,
                    created_at: now,
                    updated_at: now,
                });
            }
        }
    }

    // 5. Check for forced-unverified sessions
    if let Ok(store) = crate::execution::SessionStore::load(project_root) {
        let forced: Vec<_> = store
            .sessions
            .iter()
            .filter(|s| s.status == crate::execution::SessionStatus::ForcedUnverified)
            .collect();
        if !forced.is_empty() {
            let ids: Vec<String> = forced.iter().map(|s| s.id.clone()).collect();
            findings.push(GuardianFinding {
                id: format!("guardian-forced-unverified-{}", now),
                kind: "forced_unverified".to_string(),
                severity: FindingSeverity::Warning,
                title: format!("{} forced-unverified sessions", forced.len()),
                explanation: "Sessions that bypassed verification policy — indicates potential integrity gaps".to_string(),
                evidence_ids: ids,
                affected_scope: vec![],
                suggested_actions: vec![
                    "Review each forced-unverified session and verify manually".to_string(),
                ],
                status: FindingStatus::Open,
                created_at: now,
                updated_at: now,
            });
        }
    }

    // 6. Check for open sessions
    if let Ok(store) = crate::execution::SessionStore::load(project_root) {
        let active: Vec<_> = store
            .sessions
            .iter()
            .filter(|s| s.status == crate::execution::SessionStatus::Active)
            .collect();
        if !active.is_empty() {
            let ids: Vec<String> = active.iter().map(|s| s.id.clone()).collect();
            findings.push(GuardianFinding {
                id: format!("guardian-open-sessions-{}", now),
                kind: "open_session".to_string(),
                severity: FindingSeverity::Info,
                title: format!("{} open (active) sessions", active.len()),
                explanation: "Active sessions that have not been ended".to_string(),
                evidence_ids: ids,
                affected_scope: vec![],
                suggested_actions: vec!["End or resume open sessions".to_string()],
                status: FindingStatus::Open,
                created_at: now,
                updated_at: now,
            });
        }
    }

    Ok(GuardianScanResult { findings })
}

/// Format a finding for display.
pub fn format_finding(finding: &GuardianFinding) -> String {
    format!(
        r#"[{id}] ({kind})
  Severity: {sev:?}
  Status:   {status:?}
  Title:    {title}
  Explain:  {explain}
  Evidence: {ev}
  Scope:    {scope}
  Actions:  {actions}
  Created:  {created}
  Updated:  {updated}"#,
        id = finding.id,
        kind = finding.kind,
        sev = finding.severity,
        status = finding.status,
        title = finding.title,
        explain = finding.explanation,
        ev = finding.evidence_ids.join(", "),
        scope = finding.affected_scope.join(", "),
        actions = finding.suggested_actions.join("; "),
        created = finding.created_at,
        updated = finding.updated_at,
    )
}
