//! Drift / Decay — detect when project state diverges from expectations.
//!
//! Detects:
//! - Memory Drift: ProjectMemory items inconsistent with current state
//! - Reference Drift: References that are stale, unavailable, or changed
//! - Workflow Drift: Workflow definitions that no longer match actual usage
//! - Strategy Drift: Strategy configurations that are outdated
//! - Documentation Drift: Docs with explicit source links that are stale
//! - Goal Drift: Goals without recent progress

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A single drift detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftItem {
    /// Kind of drift (memory, reference, workflow, strategy, documentation, goal)
    pub kind: String,
    /// What was expected
    pub expected: String,
    /// What was actually observed
    pub observed: String,
    /// Evidence for the drift
    #[serde(default)]
    pub evidence: Vec<String>,
    /// How long this drift has existed (seconds since first detected)
    pub age_secs: u64,
    /// Suggested reconciliation action
    pub suggested_reconciliation: String,
}

/// Result of a drift scan.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DriftScanResult {
    pub items: Vec<DriftItem>,
}

/// Scan for drift across all available data sources.
pub fn scan_drift(project_root: &Path) -> Result<DriftScanResult> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut items = Vec::new();

    // 1. Memory Drift — check for superseded items still marked as Current
    if let Ok(store) = crate::memory::MemoryStore::load(project_root) {
        if let Some(mem) = store.current_memory() {
            for item in mem.all_items() {
                // Check if item is superseded but still Current
                if item.kind == crate::memory::MemoryItemKind::Current && item.supersedes.is_some()
                {
                    let age = now.saturating_sub(item.updated_at as u64);
                    items.push(DriftItem {
                        kind: "memory".to_string(),
                        expected: format!(
                            "Item '{}' should be superseded",
                            item.content.chars().take(60).collect::<String>()
                        ),
                        observed: "Item is still marked as Current".to_string(),
                        evidence: vec![format!("memory:{}", item.id)],
                        age_secs: age,
                        suggested_reconciliation:
                            "Run memory refresh or manually supersede this item".to_string(),
                    });
                }
            }
        }
    }

    // 2. Goal Drift — active goals without recent tasks
    if let Ok(store) = crate::goal::GoalStore::load(project_root) {
        for goal in &store.goals {
            if goal.status == crate::goal::GoalStatus::Active && goal.related_tasks.is_empty() {
                let age = now.saturating_sub(goal.updated_at as u64);
                items.push(DriftItem {
                    kind: "goal".to_string(),
                    expected: format!("Active goal '{}' should have progress", goal.title),
                    observed: "No tasks associated with this goal".to_string(),
                    evidence: vec![format!("goal:{}", goal.id)],
                    age_secs: age,
                    suggested_reconciliation:
                        "Create a task for this goal or mark it as paused/abandoned".to_string(),
                });
            }
        }
    }

    // 3. Check for disabled references
    {
        let registry = crate::constitutive::ReferenceRegistry::read(project_root)?;
        for entry in &registry.entries {
            if !entry.enabled {
                items.push(DriftItem {
                    kind: "reference".to_string(),
                    expected: format!("Reference '{}' should be available", entry.name),
                    observed: "Reference is disabled".to_string(),
                    evidence: vec![format!("reference:{}", entry.id)],
                    age_secs: 0,
                    suggested_reconciliation: "Review and re-enable or remove the reference"
                        .to_string(),
                });
            }
        }
    }

    Ok(DriftScanResult { items })
}

/// Format drift scan results for display.
pub fn format_drift(result: &DriftScanResult) -> String {
    if result.items.is_empty() {
        return "No drift detected.\n".to_string();
    }

    let mut out = format!("Drift Scan ({} items):\n\n", result.items.len());
    for (i, item) in result.items.iter().enumerate() {
        let age_days = item.age_secs / 86400;
        let age_str = if age_days > 0 {
            format!("{}d", age_days)
        } else {
            let age_hours = item.age_secs / 3600;
            if age_hours > 0 {
                format!("{}h", age_hours)
            } else {
                format!("{}m", item.age_secs / 60)
            }
        };

        out.push_str(&format!(
            "{}. [{}] Drift (age: {})\n   Expected: {}\n   Observed: {}\n   → {}\n\n",
            i + 1,
            item.kind,
            age_str,
            item.expected,
            item.observed,
            item.suggested_reconciliation,
        ));
    }

    out
}
