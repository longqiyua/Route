//! P5 Agent Organization Memory — records AgentPlan results and provides
//! recall for future AgentCompiler invocations.
//!
//! OrganizationExperience is **reference-only** — the Protocol still has
//! higher priority. Recall signals are advisory, never mandatory.
//!
//! Storage layout:
//! ```text
//! .route/
//! └── agent-org/
//!     └── experiences.json   # OrganizationExperienceStore
//! ```

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{write_atomic, ROUTE_DOT_DIR};

// ---------------------------------------------------------------------------
// Directory layout
// ---------------------------------------------------------------------------

/// Directory name for agent organization data.
pub const AGENT_ORG_DIR: &str = "agent-org";

/// File name for the organization experience store.
pub const AGENT_ORG_FILE: &str = "experiences.json";

/// Returns the `agent-org/` directory path under `.route/`.
pub fn agent_org_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(AGENT_ORG_DIR)
}

/// Returns the full path to the experiences JSON file.
pub fn agent_org_path(project_root: &Path) -> PathBuf {
    agent_org_dir(project_root).join(AGENT_ORG_FILE)
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A record of an agent plan execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationExperience {
    pub id: String,
    pub session_id: String,
    /// Task pattern, e.g. "feature", "bugfix", "refactor", "storage-migration".
    pub task_pattern: String,
    pub agent_roles: Vec<String>,
    pub dependencies: Vec<String>,
    pub tools_used: Vec<String>,
    /// "pass" | "fail" | "partial"
    pub verification_result: String,
    /// "success" | "failed" | "aborted" | "rolled_back"
    pub task_result: String,
    pub had_rollback: bool,
    pub duration_secs: Option<u64>,
    pub created_at: i64,
}

/// Store for organization experiences.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrganizationExperienceStore {
    pub experiences: Vec<OrganizationExperience>,
}

/// A recall signal for the AgentCompiler.
#[derive(Debug, Clone)]
pub struct RecallSignal {
    pub role: String,
    /// Reliability score 0.0 – 1.0.
    pub reliability: f64,
    /// Session IDs that contributed to this signal.
    pub evidence: Vec<String>,
    pub note: String,
}

impl OrganizationExperienceStore {
    /// Load the store from disk. Returns an empty store if the file does
    /// not exist.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = agent_org_path(project_root);
        if p.exists() {
            let bytes = std::fs::read(&p)
                .with_context(|| format!("reading agent org store from {}", p.display()))?;
            serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing agent org store from {}", p.display()))
        } else {
            Ok(Self::default())
        }
    }

    /// Save the store to disk (atomic write).
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = agent_org_path(project_root);
        std::fs::create_dir_all(agent_org_dir(project_root)).with_context(|| {
            format!(
                "creating agent org directory {}",
                agent_org_dir(project_root).display()
            )
        })?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing agent org store to {}", p.display()))?;
        Ok(())
    }

    /// Add an experience to the store.
    pub fn add(&mut self, exp: OrganizationExperience) {
        self.experiences.push(exp);
    }

    /// Recall experiences for a given task pattern.
    ///
    /// Returns a list of `RecallSignal`s — one per unique agent role that
    /// has been observed in past experiences matching the pattern.
    ///
    /// Signals are **advisory** — the AgentCompiler treats them as hints.
    pub fn recall(&self, task_pattern: &str) -> Vec<RecallSignal> {
        let matching: Vec<_> = self
            .experiences
            .iter()
            .filter(|e| e.task_pattern == task_pattern)
            .collect();

        if matching.is_empty() {
            return Vec::new();
        }

        // Group by role
        let mut role_map: std::collections::HashMap<&str, Vec<&OrganizationExperience>> =
            std::collections::HashMap::new();
        for exp in &matching {
            for role in &exp.agent_roles {
                role_map.entry(role.as_str()).or_default().push(exp);
            }
        }

        let mut signals: Vec<RecallSignal> = Vec::new();
        for (role, exps) in role_map {
            let total = exps.len();
            let successes = exps.iter().filter(|e| e.task_result == "success").count();
            let reliability = if total > 0 {
                successes as f64 / total as f64
            } else {
                0.0
            };

            let evidence: Vec<String> = exps.iter().map(|e| e.session_id.clone()).collect();
            let note = format!(
                "Organization Memory hint: {} has {:.2} reliability from {} similar tasks",
                role, reliability, total
            );

            signals.push(RecallSignal {
                role: role.to_string(),
                reliability,
                evidence,
                note,
            });
        }

        // Sort by reliability descending
        signals.sort_by(|a, b| {
            b.reliability
                .partial_cmp(&a.reliability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        signals
    }

    /// Get all experiences.
    pub fn list(&self) -> Vec<&OrganizationExperience> {
        self.experiences.iter().collect()
    }

    /// Explain why a specific session had its agent plan.
    ///
    /// Returns a human-readable explanation of the organization experience
    /// for the given session, or an error if no experience is found.
    pub fn explain(&self, session_id: &str) -> Result<String> {
        let exp = self
            .experiences
            .iter()
            .find(|e| e.session_id == session_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no organization experience found for session '{}'",
                    session_id
                )
            })?;

        let mut out = String::new();
        out.push_str(&format!("Session: {}\n", exp.session_id));
        out.push_str(&format!("Task Pattern: {}\n", exp.task_pattern));
        out.push_str(&format!("Agent Roles: [{}]\n", exp.agent_roles.join(", ")));
        out.push_str(&format!(
            "Dependencies: [{}]\n",
            exp.dependencies.join(", ")
        ));
        out.push_str(&format!("Tools Used: [{}]\n", exp.tools_used.join(", ")));
        out.push_str(&format!(
            "Verification Result: {}\n",
            exp.verification_result
        ));
        out.push_str(&format!("Task Result: {}\n", exp.task_result));
        out.push_str(&format!("Had Rollback: {}\n", exp.had_rollback));
        if let Some(dur) = exp.duration_secs {
            out.push_str(&format!("Duration: {}s\n", dur));
        }
        out.push_str(&format!("Created At: {}\n", exp.created_at));
        Ok(out)
    }
}

/// Classify a task description into a pattern string.
///
/// Uses simple keyword matching to produce a deterministic classification.
pub fn classify_task_pattern(task: &str) -> String {
    let lower = task.to_lowercase();

    if lower.contains("feature") || lower.contains("add ") || lower.contains("new ") {
        "feature".to_string()
    } else if lower.contains("bug") || lower.contains("fix") || lower.contains("typo") {
        "bugfix".to_string()
    } else if lower.contains("refactor") || lower.contains("migrate") || lower.contains("migration")
    {
        if lower.contains("storage") || lower.contains("database") || lower.contains("db") {
            "storage-migration".to_string()
        } else {
            "refactor".to_string()
        }
    } else if lower.contains("test")
        || lower.contains("verify")
        || lower.contains("validate")
        || lower.contains("check")
    {
        "verification".to_string()
    } else if lower.contains("docs") || lower.contains("documentation") || lower.contains("readme")
    {
        "documentation".to_string()
    } else if lower.contains("config") || lower.contains("setup") || lower.contains("init") {
        "configuration".to_string()
    } else if lower.contains("perf") || lower.contains("performance") || lower.contains("optimize")
    {
        "performance".to_string()
    } else if lower.contains("update") || lower.contains("upgrade") || lower.contains("bump") {
        "update".to_string()
    } else {
        "general".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_agent_org_paths() {
        let root = Path::new("/tmp/project");
        let dir = agent_org_dir(root);
        assert!(dir.ends_with(".route/agent-org"));
        let p = agent_org_path(root);
        assert!(p.ends_with(".route/agent-org/experiences.json"));
    }

    #[test]
    fn test_load_empty_store() {
        let tmp = TempDir::new().unwrap();
        let store = OrganizationExperienceStore::load(tmp.path()).unwrap();
        assert!(store.experiences.is_empty());
    }

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let mut store = OrganizationExperienceStore::default();

        store.add(OrganizationExperience {
            id: "exp-1".to_string(),
            session_id: "session-1".to_string(),
            task_pattern: "feature".to_string(),
            agent_roles: vec!["primary".to_string(), "verifier".to_string()],
            dependencies: vec!["primary".to_string()],
            tools_used: vec!["read".to_string(), "edit".to_string()],
            verification_result: "pass".to_string(),
            task_result: "success".to_string(),
            had_rollback: false,
            duration_secs: Some(120),
            created_at: 1000,
        });

        store.save(tmp.path()).unwrap();

        let loaded = OrganizationExperienceStore::load(tmp.path()).unwrap();
        assert_eq!(loaded.experiences.len(), 1);
        assert_eq!(loaded.experiences[0].id, "exp-1");
        assert_eq!(loaded.experiences[0].task_pattern, "feature");
    }

    #[test]
    fn test_recall_no_matches() {
        let store = OrganizationExperienceStore::default();
        let signals = store.recall("nonexistent");
        assert!(signals.is_empty());
    }

    #[test]
    fn test_recall_with_matches() {
        let mut store = OrganizationExperienceStore::default();

        // Two successful feature experiences
        store.add(OrganizationExperience {
            id: "exp-1".to_string(),
            session_id: "s1".to_string(),
            task_pattern: "feature".to_string(),
            agent_roles: vec!["primary".to_string(), "verifier".to_string()],
            dependencies: vec!["primary".to_string()],
            tools_used: vec![],
            verification_result: "pass".to_string(),
            task_result: "success".to_string(),
            had_rollback: false,
            duration_secs: None,
            created_at: 1000,
        });

        store.add(OrganizationExperience {
            id: "exp-2".to_string(),
            session_id: "s2".to_string(),
            task_pattern: "feature".to_string(),
            agent_roles: vec!["primary".to_string(), "builder".to_string()],
            dependencies: vec![],
            tools_used: vec![],
            verification_result: "pass".to_string(),
            task_result: "success".to_string(),
            had_rollback: false,
            duration_secs: None,
            created_at: 2000,
        });

        let signals = store.recall("feature");
        // Should have 3 roles: primary, verifier, builder
        assert_eq!(signals.len(), 3);

        // primary appears in both → 2/2 = 1.0 reliability
        let primary = signals.iter().find(|s| s.role == "primary").unwrap();
        assert!((primary.reliability - 1.0).abs() < 0.001);
        assert_eq!(primary.evidence.len(), 2);
    }

    #[test]
    fn test_list() {
        let mut store = OrganizationExperienceStore::default();
        store.add(OrganizationExperience {
            id: "e1".to_string(),
            session_id: "s1".to_string(),
            task_pattern: "bugfix".to_string(),
            agent_roles: vec![],
            dependencies: vec![],
            tools_used: vec![],
            verification_result: "pass".to_string(),
            task_result: "success".to_string(),
            had_rollback: false,
            duration_secs: None,
            created_at: 1000,
        });
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn test_explain() {
        let mut store = OrganizationExperienceStore::default();
        store.add(OrganizationExperience {
            id: "exp-1".to_string(),
            session_id: "session-abc".to_string(),
            task_pattern: "refactor".to_string(),
            agent_roles: vec!["primary".to_string()],
            dependencies: vec![],
            tools_used: vec!["read".to_string(), "edit".to_string()],
            verification_result: "pass".to_string(),
            task_result: "success".to_string(),
            had_rollback: false,
            duration_secs: Some(60),
            created_at: 1000,
        });

        let result = store.explain("session-abc").unwrap();
        assert!(result.contains("session-abc"));
        assert!(result.contains("refactor"));
        assert!(result.contains("primary"));

        // Unknown session should error
        assert!(store.explain("unknown").is_err());
    }

    #[test]
    fn test_classify_task_pattern() {
        assert_eq!(classify_task_pattern("add a new feature"), "feature");
        assert_eq!(classify_task_pattern("fix a bug"), "bugfix");
        assert_eq!(classify_task_pattern("refactor the module"), "refactor");
        assert_eq!(
            classify_task_pattern("migrate storage to sqlite"),
            "storage-migration"
        );
        assert_eq!(classify_task_pattern("run tests"), "verification");
        assert_eq!(
            classify_task_pattern("update documentation"),
            "documentation"
        );
        assert_eq!(classify_task_pattern("setup config"), "configuration");
        assert_eq!(classify_task_pattern("optimize performance"), "performance");
        assert_eq!(classify_task_pattern("bump version"), "update");
        assert_eq!(classify_task_pattern("random task"), "general");
    }
}
