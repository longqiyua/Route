//! Project Health Model — evidence-backed health snapshots.
//!
//! Health is NOT a code quality score. It only outputs evidence-backed
//! signals: OK / INFO / WARNING / ACTION / UNKNOWN.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Health directory under `.route/`.
pub fn health_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("health")
}

/// Path to the health history file.
pub fn health_path(project_root: &Path) -> PathBuf {
    health_dir(project_root).join("history.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The signal level for a health item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HealthSignal {
    /// Everything is fine
    Ok,
    /// Something worth noting
    Info,
    /// Something to watch
    Warning,
    /// Action required
    Action,
    /// Cannot determine
    Unknown,
}

impl Default for HealthSignal {
    fn default() -> Self {
        Self::Unknown
    }
}

/// A single health signal item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSignalItem {
    /// Signal level
    pub signal: HealthSignal,
    /// Title of the signal
    pub title: String,
    /// Explanation
    pub explanation: String,
    /// Evidence references (session IDs, file paths, etc.)
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// A health risk item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthRisk {
    /// Risk title
    pub title: String,
    /// Severity: low | medium | high | critical
    pub severity: String,
    /// Explanation of the risk
    pub explanation: String,
    /// Evidence references
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Suggested actions to mitigate
    #[serde(default)]
    pub suggested_actions: Vec<String>,
}

/// A complete project health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHealthSnapshot {
    /// Unique snapshot ID
    pub id: String,
    /// When this snapshot was taken
    pub created_at: i64,
    /// Context hash at the time of snapshot
    #[serde(default)]
    pub context_hash: String,
    /// All signals
    #[serde(default)]
    pub signals: Vec<HealthSignalItem>,
    /// Identified risks
    #[serde(default)]
    pub risks: Vec<HealthRisk>,
    /// Stale items count
    #[serde(default)]
    pub stale_items: Vec<String>,
    /// Unresolved failure case IDs
    #[serde(default)]
    pub unresolved_failures: Vec<String>,
    /// Open questions from memory
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// Pending idea IDs
    #[serde(default)]
    pub pending_ideas: Vec<String>,
    /// Memory drift descriptions
    #[serde(default)]
    pub memory_drift: Vec<String>,
    /// Reference drift descriptions
    #[serde(default)]
    pub reference_drift: Vec<String>,
    /// Workflow drift descriptions
    #[serde(default)]
    pub workflow_drift: Vec<String>,
    /// Suggested actions
    #[serde(default)]
    pub suggested_actions: Vec<HealthAction>,
}

/// A suggested action from a health snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthAction {
    /// Title
    pub title: String,
    /// Why this action is suggested
    pub reason: String,
    /// Expected impact
    pub expected_impact: String,
    /// Related signal/risk IDs
    #[serde(default)]
    pub related_to: Vec<String>,
}

/// The health history store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthStore {
    pub snapshots: Vec<ProjectHealthSnapshot>,
}

impl HealthStore {
    /// Load health history from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = health_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save health history to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = health_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(health_path(project_root), json)?;
        Ok(())
    }

    /// Add a snapshot.
    pub fn add(&mut self, snapshot: ProjectHealthSnapshot) {
        self.snapshots.push(snapshot);
    }

    /// List all snapshots (most recent first).
    pub fn list(&self) -> Vec<&ProjectHealthSnapshot> {
        let mut snapshots: Vec<&ProjectHealthSnapshot> = self.snapshots.iter().collect();
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        snapshots
    }

    /// Get the latest snapshot.
    pub fn latest(&self) -> Option<&ProjectHealthSnapshot> {
        self.list().into_iter().next()
    }

    /// Get a snapshot by ID.
    pub fn get(&self, id: &str) -> Option<&ProjectHealthSnapshot> {
        self.snapshots.iter().find(|s| s.id == id)
    }
}

/// Format a health snapshot for display.
pub fn format_health_snapshot(snapshot: &ProjectHealthSnapshot, explain: bool) -> String {
    let mut out = format!(
        "Health Snapshot: {}\nTimestamp: {}\n\n",
        snapshot.id, snapshot.created_at
    );

    if !snapshot.context_hash.is_empty() {
        out.push_str(&format!("Context: {}\n\n", snapshot.context_hash));
    }

    // Signals
    if !snapshot.signals.is_empty() {
        out.push_str(&format!("Signals ({}):\n", snapshot.signals.len()));
        for s in &snapshot.signals {
            let icon = match s.signal {
                HealthSignal::Ok => "✓",
                HealthSignal::Info => "ℹ",
                HealthSignal::Warning => "⚠",
                HealthSignal::Action => "▶",
                HealthSignal::Unknown => "?",
            };
            out.push_str(&format!(
                "  {} [{}] {}\n",
                icon,
                format!("{:?}", s.signal).to_uppercase(),
                s.title
            ));
            if explain {
                out.push_str(&format!("       {}\n", s.explanation));
                for ev in &s.evidence {
                    out.push_str(&format!("       Evidence: {}\n", ev));
                }
            }
        }
        out.push('\n');
    }

    // Risks
    if !snapshot.risks.is_empty() {
        out.push_str(&format!("Risks ({}):\n", snapshot.risks.len()));
        for r in &snapshot.risks {
            out.push_str(&format!(
                "  ⚠ [{}] {} ({})\n",
                r.severity.to_uppercase(),
                r.title,
                r.explanation
            ));
            if explain {
                for a in &r.suggested_actions {
                    out.push_str(&format!("       → {}\n", a));
                }
            }
        }
        out.push('\n');
    }

    // Summary counts
    out.push_str(&format!(
        "Unresolved failures: {}\nOpen questions: {}\nPending ideas: {}\nStale items: {}\n",
        snapshot.unresolved_failures.len(),
        snapshot.open_questions.len(),
        snapshot.pending_ideas.len(),
        snapshot.stale_items.len(),
    ));

    if !snapshot.suggested_actions.is_empty() {
        out.push_str(&format!(
            "\nSuggested Actions ({}):\n",
            snapshot.suggested_actions.len()
        ));
        for (i, a) in snapshot.suggested_actions.iter().enumerate() {
            out.push_str(&format!("  {}. {} — {}\n", i + 1, a.title, a.reason));
        }
    }

    out
}
