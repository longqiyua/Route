//! ReusablePattern management — P1 Pattern Distillation.
//!
//! Patterns are distilled from study evidence. They capture reusable solutions
//! to common problems encountered during development. When applied, patterns
//! are registered as Reference entries in the Reference registry
//! (patterns are NOT a fourth world).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;
use crate::study::StudyRecord;

/// Directory name under `.route/`.
pub const PATTERN_DIR: &str = "pattern";

/// Pattern store file name.
pub const PATTERN_FILE: &str = "pattern.json";

/// A reusable pattern distilled from study evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReusablePattern {
    pub id: String,
    pub name: String,
    pub problem: String,
    pub solution: String,
    pub when_to_use: Vec<String>,
    pub when_not_to_use: Vec<String>,
    pub tradeoffs: Vec<String>,
    pub evidence_sources: Vec<String>, // study IDs, file paths
    pub confidence: f64,               // 0.0 - 1.0, increases with multiple supporting studies
    pub created_at: i64,
    pub updated_at: i64,
    pub tags: Vec<String>,
}

/// A proposal to add a pattern from a study.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternProposal {
    pub id: String,
    pub pattern: ReusablePattern,
    pub source_study_id: String,
    pub reason: String,
    pub status: String, // "pending" | "applied" | "rejected"
    pub created_at: i64,
}

/// A store of all patterns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternStore {
    pub patterns: Vec<ReusablePattern>,
    pub proposals: Vec<PatternProposal>,
}

/// Canonical pattern directory path.
pub fn pattern_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(PATTERN_DIR)
}

/// Canonical pattern store file path.
pub fn pattern_path(project_root: &Path) -> PathBuf {
    pattern_dir(project_root).join(PATTERN_FILE)
}

impl PatternStore {
    /// Load the pattern store from disk. Returns an empty store if the file
    /// does not exist yet.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = pattern_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read pattern store at {}", path.display()))?;
        let store: Self = serde_json::from_str(&json)
            .with_context(|| format!("failed to parse pattern store at {}", path.display()))?;
        Ok(store)
    }

    /// Save the pattern store to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = pattern_dir(project_root);
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create pattern directory at {}", dir.display()))?;
        let path = pattern_path(project_root);
        let json = serde_json::to_string_pretty(self)
            .with_context(|| "failed to serialize pattern store")?;
        std::fs::write(&path, &json)
            .with_context(|| format!("failed to write pattern store at {}", path.display()))?;
        Ok(())
    }

    /// List all patterns in the store.
    pub fn list(&self) -> Vec<&ReusablePattern> {
        self.patterns.iter().collect()
    }

    /// Get a specific pattern by ID.
    pub fn get(&self, id: &str) -> Option<&ReusablePattern> {
        self.patterns.iter().find(|p| p.id == id)
    }

    /// Propose a new pattern from a study record.
    ///
    /// Extracts evidence from the study record's patterns and decisions fields.
    /// Confidence increases when multiple study entries support the same pattern.
    pub fn propose_from_study(
        study_record: &StudyRecord,
        pattern_name: &str,
        problem: &str,
        solution: &str,
    ) -> PatternProposal {
        let now = chrono::Utc::now().timestamp();
        let id = ulid::Ulid::new().to_string();

        // Collect evidence from the study record's patterns and decisions
        let mut evidence_sources: Vec<String> = study_record
            .patterns
            .iter()
            .map(|p| format!("pattern: {}", p))
            .collect();
        evidence_sources.extend(
            study_record
                .decisions
                .iter()
                .map(|d| format!("decision: {}", d)),
        );

        // Base confidence: each matching pattern/decision entry adds 0.1
        let base_confidence = 0.3;
        let boost = (study_record.patterns.len() + study_record.decisions.len()) as f64 * 0.1;
        let confidence = (base_confidence + boost).min(1.0);

        let pattern = ReusablePattern {
            id: id.clone(),
            name: pattern_name.to_string(),
            problem: problem.to_string(),
            solution: solution.to_string(),
            when_to_use: Vec::new(),
            when_not_to_use: Vec::new(),
            tradeoffs: Vec::new(),
            evidence_sources,
            confidence,
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        };

        PatternProposal {
            id: ulid::Ulid::new().to_string(),
            pattern,
            source_study_id: study_record.id.clone(),
            reason: format!(
                "Proposed from study: pattern '{}' with {} supporting entries",
                pattern_name,
                study_record.patterns.len() + study_record.decisions.len()
            ),
            status: "pending".to_string(),
            created_at: now,
        }
    }

    /// Apply a pattern proposal: move it from pending to applied and add
    /// the pattern to the store.
    pub fn apply_proposal(&mut self, proposal_id: &str) -> Result<()> {
        let proposal = self
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| anyhow::anyhow!("proposal '{}' not found", proposal_id))?;

        if proposal.status != "pending" {
            anyhow::bail!(
                "proposal '{}' has status '{}' (expected 'pending')",
                proposal_id,
                proposal.status
            );
        }

        // Check for duplicates: if a pattern with the same name already exists,
        // increase confidence instead of adding a duplicate
        let existing = self
            .patterns
            .iter_mut()
            .find(|p| p.name == proposal.pattern.name);

        if let Some(existing_pattern) = existing {
            // Merge evidence and boost confidence
            for ev in &proposal.pattern.evidence_sources {
                if !existing_pattern.evidence_sources.contains(ev) {
                    existing_pattern.evidence_sources.push(ev.clone());
                }
            }
            existing_pattern.confidence = (existing_pattern.confidence + 0.1).min(1.0);
            existing_pattern.updated_at = chrono::Utc::now().timestamp();
        } else {
            // Add the pattern from the proposal
            let mut pattern = proposal.pattern.clone();
            // Check for similar patterns to boost confidence
            for existing in &self.patterns {
                if existing.tags.iter().any(|t| pattern.tags.contains(t))
                    || existing.problem == pattern.problem
                {
                    pattern.confidence = (pattern.confidence + 0.05).min(1.0);
                }
            }
            pattern.updated_at = chrono::Utc::now().timestamp();
            self.patterns.push(pattern);
        }

        proposal.status = "applied".to_string();
        Ok(())
    }

    /// Reject a pattern proposal.
    pub fn reject_proposal(&mut self, proposal_id: &str) -> Result<()> {
        let proposal = self
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| anyhow::anyhow!("proposal '{}' not found", proposal_id))?;

        if proposal.status != "pending" {
            anyhow::bail!(
                "proposal '{}' has status '{}' (expected 'pending')",
                proposal_id,
                proposal.status
            );
        }

        proposal.status = "rejected".to_string();
        Ok(())
    }

    /// Find patterns by tag.
    pub fn find_by_tag(&self, tag: &str) -> Vec<&ReusablePattern> {
        self.patterns
            .iter()
            .filter(|p| p.tags.iter().any(|t| t == tag))
            .collect()
    }
}
