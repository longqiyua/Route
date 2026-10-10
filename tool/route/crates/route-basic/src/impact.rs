//! Change Impact Analysis — evaluate the impact of a proposed change.
//!
//! Uses ProjectMemory + code metadata + history + workflows to produce
//! an impact report. First version uses rule/path/history associations
//! — no semantic code graph.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Impact directory under `.route/`.
pub fn impact_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("impact")
}

/// Path to the impact history file.
pub fn impact_path(project_root: &Path) -> PathBuf {
    impact_dir(project_root).join("history.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A single impact finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactFinding {
    /// Kind of finding: affected_module | invariant | past_failure | recommended_ref | checkpoint | verification
    pub kind: String,
    /// Description of the finding
    pub description: String,
    /// Confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Source evidence (memory item, session, commit, etc.)
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// The complete impact analysis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    /// The change that was analyzed
    pub change: String,
    /// When this analysis was performed
    pub created_at: i64,
    /// Affected modules/components
    #[serde(default)]
    pub affected_modules: Vec<ImpactFinding>,
    /// Known invariants that might be violated
    #[serde(default)]
    pub known_invariants: Vec<ImpactFinding>,
    /// Relevant past failures that touched similar areas
    #[serde(default)]
    pub relevant_past_failures: Vec<ImpactFinding>,
    /// Recommended references/tools for the change
    #[serde(default)]
    pub recommended_refs: Vec<ImpactFinding>,
    /// Suggested checkpoints for safe execution
    #[serde(default)]
    pub suggested_checkpoints: Vec<ImpactFinding>,
    /// Verification steps
    #[serde(default)]
    pub verification: Vec<ImpactFinding>,
}

/// The impact analysis store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImpactStore {
    pub reports: Vec<ImpactReport>,
}

impl ImpactStore {
    /// Load impact reports from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = impact_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save impact reports to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = impact_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(impact_path(project_root), json)?;
        Ok(())
    }

    /// Add a report.
    pub fn add(&mut self, report: ImpactReport) {
        self.reports.push(report);
    }

    /// List all reports.
    pub fn list(&self) -> &[ImpactReport] {
        &self.reports
    }
}

/// Analyze the impact of a change.
///
/// Uses ProjectMemory, failure library, and knowledge map to generate
/// an impact report. All findings are rule-based — no code analysis.
pub fn analyze_impact(
    change: &str,
    memory: Option<&crate::memory::ProjectMemory>,
    failures: Option<&crate::failure::FailureLibrary>,
    knowledge_map: Option<&crate::knowledge_map::KnowledgeMap>,
) -> Result<ImpactReport> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let change_lower = change.to_lowercase();
    let change_words: Vec<&str> = change_lower.split_whitespace().collect();

    let mut affected_modules = Vec::new();
    let mut known_invariants = Vec::new();
    let mut relevant_past_failures = Vec::new();
    let recommended_refs = Vec::new();
    let mut suggested_checkpoints = Vec::new();
    let mut verification = Vec::new();

    // Check memory for relevant items
    if let Some(mem) = memory {
        for item in mem.all_items() {
            let content_lower = item.content.to_lowercase();
            let matches = change_words.iter().any(|w| content_lower.contains(w));

            if matches {
                match item.kind {
                    crate::memory::MemoryItemKind::Invariant => {
                        known_invariants.push(ImpactFinding {
                            kind: "invariant".to_string(),
                            description: format!("Invariant: {}", item.content),
                            confidence: 0.8,
                            evidence: vec![format!("memory:{}", item.id)],
                        });
                    }
                    crate::memory::MemoryItemKind::Decision => {
                        affected_modules.push(ImpactFinding {
                            kind: "related_decision".to_string(),
                            description: format!("Related decision: {}", item.content),
                            confidence: 0.6,
                            evidence: vec![format!("memory:{}", item.id)],
                        });
                    }
                    crate::memory::MemoryItemKind::FailedAttempt => {
                        relevant_past_failures.push(ImpactFinding {
                            kind: "past_failure".to_string(),
                            description: format!("Past failure: {}", item.content),
                            confidence: 0.7,
                            evidence: vec![format!("memory:{}", item.id)],
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // Check failure library for relevant failures
    if let Some(lib) = failures {
        for case in &lib.cases {
            if case.resolved {
                continue; // Skip resolved failures
            }
            let matches = change_words
                .iter()
                .any(|w| case.problem.to_lowercase().contains(w))
                || case
                    .affected_scope
                    .iter()
                    .any(|s| change_words.iter().any(|w| s.to_lowercase().contains(w)));

            if matches {
                relevant_past_failures.push(ImpactFinding {
                    kind: "failure_case".to_string(),
                    description: format!(
                        "Failure '{}': {} — affected: {}",
                        case.id,
                        case.problem,
                        case.affected_scope.join(", ")
                    ),
                    confidence: 0.8,
                    evidence: case.evidence.clone(),
                });
            }
        }
    }

    // Check knowledge map for related modules
    if let Some(map) = knowledge_map {
        for node in &map.nodes {
            let matches = change_words
                .iter()
                .any(|w| node.label.to_lowercase().contains(w))
                || change_words
                    .iter()
                    .any(|w| node.description.to_lowercase().contains(w));

            if matches {
                affected_modules.push(ImpactFinding {
                    kind: "affected_module".to_string(),
                    description: format!("Module '{}': {}", node.label, node.description),
                    confidence: 0.7,
                    evidence: vec![node.source.clone()],
                });

                // Find related nodes
                for (related, edge) in map.related_nodes(&node.id) {
                    affected_modules.push(ImpactFinding {
                        kind: "related_module".to_string(),
                        description: format!(
                            "  → {kind:?} → {label}: {desc}",
                            kind = edge.kind,
                            label = related.label,
                            desc = related.description
                        ),
                        confidence: 0.5,
                        evidence: vec![edge.evidence.clone()],
                    });
                }
            }
        }
    }

    // Always suggest a checkpoint
    suggested_checkpoints.push(ImpactFinding {
        kind: "checkpoint".to_string(),
        description: "Create a savepoint before making this change".to_string(),
        confidence: 1.0,
        evidence: vec![],
    });

    // Always suggest basic verification
    verification.push(ImpactFinding {
        kind: "verification".to_string(),
        description: "Run existing tests after the change".to_string(),
        confidence: 1.0,
        evidence: vec![],
    });

    Ok(ImpactReport {
        change: change.to_string(),
        created_at: now,
        affected_modules,
        known_invariants,
        relevant_past_failures,
        recommended_refs,
        suggested_checkpoints,
        verification,
    })
}

/// Format an impact report for display.
pub fn format_impact_report(report: &ImpactReport) -> String {
    let mut out = format!(
        "Impact Analysis: {}\nTimestamp: {}\n\n",
        report.change, report.created_at
    );

    if !report.affected_modules.is_empty() {
        out.push_str(&format!(
            "Affected Modules ({}):\n",
            report.affected_modules.len()
        ));
        for f in &report.affected_modules {
            out.push_str(&format!(
                "  - [{}] {} (conf: {:.0}%)\n",
                f.kind,
                f.description,
                f.confidence * 100.0
            ));
        }
        out.push('\n');
    }

    if !report.known_invariants.is_empty() {
        out.push_str("Known Invariants:\n");
        for f in &report.known_invariants {
            out.push_str(&format!("  ⚠ {}\n", f.description));
        }
        out.push('\n');
    }

    if !report.relevant_past_failures.is_empty() {
        out.push_str("Past Failures:\n");
        for f in &report.relevant_past_failures {
            out.push_str(&format!("  ✗ {}\n", f.description));
        }
        out.push('\n');
    }

    if !report.suggested_checkpoints.is_empty() {
        out.push_str("Suggested Checkpoints:\n");
        for f in &report.suggested_checkpoints {
            out.push_str(&format!("  ▶ {}\n", f.description));
        }
        out.push('\n');
    }

    if !report.verification.is_empty() {
        out.push_str("Verification:\n");
        for f in &report.verification {
            out.push_str(&format!("  ✓ {}\n", f.description));
        }
        out.push('\n');
    }

    out
}
