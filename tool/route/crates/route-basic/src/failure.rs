//! Failure Library — structured failure cases for learning.
//!
//! Each failure case records: what was attempted, what went wrong,
//! what the symptoms were, the root cause (if known), and how it
//! was resolved (if resolved). Failures persist and are recalled
//! when relevant tasks are encountered.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Failure directory under `.route/`.
pub fn failure_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("failure")
}

/// Path to the failure library file.
pub fn failure_path(project_root: &Path) -> PathBuf {
    failure_dir(project_root).join("library.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A structured failure case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureCase {
    /// Unique failure case ID
    pub id: String,
    /// What was the problem/goal?
    pub problem: String,
    /// What was attempted?
    pub attempt: String,
    /// What symptoms were observed?
    pub symptom: String,
    /// Root cause (if determined)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_cause: Option<String>,
    /// How was it resolved? (if resolved)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    /// What modules/files were affected?
    #[serde(default)]
    pub affected_scope: Vec<String>,
    /// Evidence links (session IDs, commit IDs, rollback IDs)
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// When this failure occurred
    pub created_at: i64,
    /// When this was resolved (if resolved)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<i64>,
    /// Severity: critical | major | minor | cosmetic
    #[serde(default)]
    pub severity: String,
    /// Whether this failure is resolved
    #[serde(default)]
    pub resolved: bool,
}

/// The failure library.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FailureLibrary {
    pub cases: Vec<FailureCase>,
}

impl FailureLibrary {
    /// Load failure library from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = failure_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save failure library to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = failure_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(failure_path(project_root), json)?;
        Ok(())
    }

    /// Add a failure case.
    pub fn add(&mut self, case: FailureCase) {
        self.cases.push(case);
    }

    /// List all failure cases.
    pub fn list(&self) -> &[FailureCase] {
        &self.cases
    }

    /// Get a failure case by ID.
    pub fn get(&self, id: &str) -> Option<&FailureCase> {
        self.cases.iter().find(|c| c.id == id)
    }

    /// Get a mutable failure case by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut FailureCase> {
        self.cases.iter_mut().find(|c| c.id == id)
    }

    /// Search for failure cases by keyword across all text fields.
    pub fn search(&self, query: &str) -> Vec<&FailureCase> {
        let q = query.to_lowercase();
        self.cases
            .iter()
            .filter(|c| {
                c.problem.to_lowercase().contains(&q)
                    || c.attempt.to_lowercase().contains(&q)
                    || c.symptom.to_lowercase().contains(&q)
                    || c.root_cause
                        .as_deref()
                        .map_or(false, |r| r.to_lowercase().contains(&q))
                    || c.resolution
                        .as_deref()
                        .map_or(false, |r| r.to_lowercase().contains(&q))
                    || c.affected_scope
                        .iter()
                        .any(|s| s.to_lowercase().contains(&q))
                    || c.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Find failures relevant to a given module/topic.
    pub fn relevant_to(&self, topic: &str) -> Vec<&FailureCase> {
        let t = topic.to_lowercase();
        self.cases
            .iter()
            .filter(|c| {
                c.affected_scope
                    .iter()
                    .any(|s| s.to_lowercase().contains(&t))
                    || c.tags.iter().any(|tag| tag.to_lowercase().contains(&t))
                    || c.problem.to_lowercase().contains(&t)
            })
            .collect()
    }

    /// Mark a failure as resolved with a resolution description.
    pub fn resolve(&mut self, id: &str, resolution: &str) -> Result<()> {
        let case = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Failure case '{}' not found", id))?;

        case.resolution = Some(resolution.to_string());
        case.resolved = true;
        case.resolved_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        );
        Ok(())
    }

    /// Remove a failure case by ID.
    pub fn remove(&mut self, id: &str) {
        self.cases.retain(|c| c.id != id);
    }
}

/// Format a failure case for display.
pub fn format_failure_case(case: &FailureCase) -> String {
    let status = if case.resolved { "RESOLVED" } else { "OPEN" };
    format!(
        r#"[{id}] ({status})
  Problem:     {problem}
  Attempt:     {attempt}
  Symptom:     {symptom}
  Root Cause:  {cause}
  Resolution:  {res}
  Severity:    {sev}
  Affected:    {scope}
  Evidence:    {ev}
  Tags:        {tags}
  Created:     {created}
  Resolved:    {resolved}"#,
        id = case.id,
        status = status,
        problem = case.problem,
        attempt = case.attempt,
        symptom = case.symptom,
        cause = case.root_cause.as_deref().unwrap_or("(unknown)"),
        res = case.resolution.as_deref().unwrap_or("(none)"),
        sev = case.severity,
        scope = case.affected_scope.join(", "),
        ev = case.evidence.join(", "),
        tags = case.tags.join(", "),
        created = case.created_at,
        resolved = case
            .resolved_at
            .map(|t| t.to_string())
            .unwrap_or_else(|| "(none)".to_string()),
    )
}
