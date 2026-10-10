//! Development Principles — extract principles from decisions, failures, and feedback.
//!
//! Generates PrincipleCandidates from project experience. Principles can be
//! applied to memory, promoted to protocol, or promoted to constitution
//! (human only — AI cannot auto-promote to constitution).

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Principle directory under `.route/`.
pub fn principle_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("principle")
}

/// Path to the principle store file.
pub fn principle_path(project_root: &Path) -> PathBuf {
    principle_dir(project_root).join("principles.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The target level for a principle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrincipleTarget {
    /// Store in project memory
    #[default]
    Memory,
    /// Promote to Protocol (execution playbook)
    Protocol,
    /// Promote to Constitution (user only)
    Constitution,
}

/// A principle candidate — a proposed rule derived from experience.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipleCandidate {
    /// Unique candidate ID
    pub id: String,
    /// The principle statement
    pub principle: String,
    /// Rationale — why this principle exists
    pub rationale: String,
    /// Source evidence (session IDs, failure IDs, decision IDs)
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Target level for this principle
    #[serde(default)]
    pub target: PrincipleTarget,
    /// Status: pending | applied_to_memory | promoted_to_protocol | promoted_to_constitution | rejected
    #[serde(default)]
    pub status: String,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// When this candidate was created
    pub created_at: i64,
    /// When this was last updated
    pub updated_at: i64,
}

/// The principle store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrincipleStore {
    pub candidates: Vec<PrincipleCandidate>,
}

impl PrincipleStore {
    /// Load principles from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = principle_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save principles to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = principle_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(principle_path(project_root), json)?;
        Ok(())
    }

    /// List all candidates, optionally filtered by status.
    pub fn list(&self, status_filter: Option<&str>) -> Vec<&PrincipleCandidate> {
        self.candidates
            .iter()
            .filter(|c| status_filter.map_or(true, |s| c.status.to_lowercase() == s.to_lowercase()))
            .collect()
    }

    /// Get a candidate by ID.
    pub fn get(&self, id: &str) -> Option<&PrincipleCandidate> {
        self.candidates.iter().find(|c| c.id == id)
    }

    /// Get a mutable candidate by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut PrincipleCandidate> {
        self.candidates.iter_mut().find(|c| c.id == id)
    }

    /// Add a principle candidate.
    pub fn add(&mut self, candidate: PrincipleCandidate) {
        self.candidates.push(candidate);
    }

    /// Generate principles from a set of failure cases.
    pub fn generate_from_failures(
        &mut self,
        failures: &[crate::failure::FailureCase],
    ) -> Vec<PrincipleCandidate> {
        let mut generated = Vec::new();
        for f in failures {
            if let Some(cause) = &f.root_cause {
                let principle = format!("Avoid '{}' — this caused: {}", f.attempt, cause);
                let candidate = PrincipleCandidate {
                    id: format!("principle-fail-{}", f.id),
                    principle: principle.clone(),
                    rationale: format!(
                        "Derived from failure case '{}': {} — root cause: {}",
                        f.id, f.problem, cause
                    ),
                    evidence: vec![f.id.clone()],
                    confidence: 0.5,
                    target: PrincipleTarget::Memory,
                    status: "pending".to_string(),
                    tags: vec!["derived-from-failure".to_string()],
                    created_at: f.created_at,
                    updated_at: f.created_at,
                };
                self.candidates.push(candidate.clone());
                generated.push(candidate);
            }
        }
        generated
    }

    /// Generate principles from memory decisions.
    pub fn generate_from_decisions(
        &mut self,
        decisions: &[crate::memory::MemoryItem],
    ) -> Vec<PrincipleCandidate> {
        let mut generated = Vec::new();
        for d in decisions {
            let principle = format!("Decision: {}", d.content);
            let candidate = PrincipleCandidate {
                id: format!("principle-decision-{}", d.id),
                principle,
                rationale: format!("Decision recorded in project memory: {}", d.content),
                evidence: d.source_ids.clone(),
                confidence: 0.6,
                target: PrincipleTarget::Memory,
                status: "pending".to_string(),
                tags: vec!["derived-from-decision".to_string()],
                created_at: d.created_at,
                updated_at: d.created_at,
            };
            self.candidates.push(candidate.clone());
            generated.push(candidate);
        }
        generated
    }

    /// Apply a principle to memory (mark as applied_to_memory).
    pub fn apply_to_memory(&mut self, id: &str) -> Result<()> {
        let candidate = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Principle candidate '{}' not found", id))?;
        candidate.status = "applied_to_memory".to_string();
        candidate.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Promote a principle to protocol level.
    pub fn promote_to_protocol(&mut self, id: &str) -> Result<()> {
        let candidate = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Principle candidate '{}' not found", id))?;
        candidate.status = "promoted_to_protocol".to_string();
        candidate.target = PrincipleTarget::Protocol;
        candidate.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Promote a principle to constitution level (human only — AI cannot call this).
    pub fn promote_to_constitution(&mut self, id: &str) -> Result<()> {
        let candidate = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Principle candidate '{}' not found", id))?;
        candidate.status = "promoted_to_constitution".to_string();
        candidate.target = PrincipleTarget::Constitution;
        candidate.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Reject a principle candidate.
    pub fn reject(&mut self, id: &str) -> Result<()> {
        let candidate = self
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Principle candidate '{}' not found", id))?;
        candidate.status = "rejected".to_string();
        candidate.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(())
    }

    /// Remove a candidate by ID.
    pub fn remove(&mut self, id: &str) {
        self.candidates.retain(|c| c.id != id);
    }
}

/// Format a principle candidate for display.
pub fn format_principle(candidate: &PrincipleCandidate) -> String {
    format!(
        r#"[{id}]
  Principle: {principle}
  Rationale: {rationale}
  Target:    {target:?}
  Status:    {status}
  Confidence: {conf:.0}%
  Evidence:  {ev}
  Tags:      {tags}
  Created:   {created}
  Updated:   {updated}"#,
        id = candidate.id,
        principle = candidate.principle,
        rationale = candidate.rationale,
        target = candidate.target,
        status = candidate.status,
        conf = candidate.confidence * 100.0,
        ev = candidate.evidence.join(", "),
        tags = candidate.tags.join(", "),
        created = candidate.created_at,
        updated = candidate.updated_at,
    )
}
