//! Strategy experiment tracking — record task results under different strategies.
//!
//! This module provides data structures and functions for tracking which
//! strategies were used for which tasks, aggregating statistics, and
//! suggesting the best strategy for a new task based on historical evidence.
//!
//! Strategy suggestion is **advisory only** — the Protocol always has
//! higher priority, and the system never auto-switches strategies.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::write_atomic;
use crate::constitutive::ROUTE_DOT_DIR;

/// Directory name for experiment data.
pub const EXPERIMENT_DIR: &str = "experiment";

/// File name for experiment store.
pub const EXPERIMENT_FILE: &str = "experiment.json";

/// A record of a task executed under a specific strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentRecord {
    pub id: String,
    pub task: String,
    pub strategy_id: String,
    pub session_id: String,
    pub result: String, // "success" | "failed" | "aborted" | "rolled_back"
    pub checks_passed: u32,
    pub checks_failed: u32,
    pub agent_roles: Vec<String>,
    pub duration_secs: Option<u64>,
    pub created_at: i64,
    pub evidence: Vec<String>,
}

/// Store for experiment records.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExperimentStore {
    pub records: Vec<ExperimentRecord>,
    pub strategies: Vec<StrategyStats>,
}

/// Aggregated statistics for a strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub strategy_id: String,
    pub total_tasks: u32,
    pub success_count: u32,
    pub failed_count: u32,
    pub aborted_count: u32,
    pub rolled_back_count: u32,
    pub avg_checks_passed: f64,
    pub avg_checks_failed: f64,
    pub common_agent_roles: Vec<String>,
    pub avg_duration_secs: Option<f64>,
    pub last_used: Option<i64>,
}

/// A ranked strategy suggestion.
#[derive(Debug, Clone)]
pub struct StrategyRanking {
    pub strategy_id: String,
    pub score: f64, // simple score based on success rate + task similarity
    pub reason: String,
    pub evidence: Vec<String>,
}

/// Canonical experiment directory path.
pub fn experiment_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join(EXPERIMENT_DIR)
}

/// Canonical experiment store file path.
pub fn experiment_path(project_root: &Path) -> PathBuf {
    experiment_dir(project_root).join(EXPERIMENT_FILE)
}

impl ExperimentStore {
    /// Load experiment store from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = experiment_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading experiment store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        let s: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing experiment store JSON at {}", p.display()))?;
        Ok(s)
    }

    /// Save experiment store to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = experiment_path(project_root);
        let dir = p.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing experiment store to {}", p.display()))?;
        Ok(())
    }

    /// Record a task completion under a strategy.
    ///
    /// Adds the record to the store and updates strategy statistics.
    pub fn record_experiment(
        &mut self,
        project_root: &Path,
        record: ExperimentRecord,
    ) -> Result<()> {
        // Recompute stats for the affected strategy before adding the new record.
        let strategy_id = record.strategy_id.clone();

        self.records.push(record);

        // Recompute statistics for the affected strategy.
        self.recompute_stats(&strategy_id);

        // Also recompute stats for any other strategies that have changed
        // (in practice only the one strategy, but we do a full refresh).
        let all_ids: Vec<String> = self
            .records
            .iter()
            .map(|r| r.strategy_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        for sid in &all_ids {
            self.recompute_stats(sid);
        }

        self.save(project_root)?;
        Ok(())
    }

    /// Recompute statistics for a single strategy.
    fn recompute_stats(&mut self, strategy_id: &str) {
        // Collect all records for this strategy.
        let strategy_records: Vec<&ExperimentRecord> = self
            .records
            .iter()
            .filter(|r| r.strategy_id == *strategy_id)
            .collect();

        let total_tasks = strategy_records.len() as u32;
        if total_tasks == 0 {
            self.strategies.retain(|s| s.strategy_id != strategy_id);
            return;
        }

        let success_count = strategy_records
            .iter()
            .filter(|r| r.result == "success")
            .count() as u32;
        let failed_count = strategy_records
            .iter()
            .filter(|r| r.result == "failed")
            .count() as u32;
        let aborted_count = strategy_records
            .iter()
            .filter(|r| r.result == "aborted")
            .count() as u32;
        let rolled_back_count = strategy_records
            .iter()
            .filter(|r| r.result == "rolled_back")
            .count() as u32;

        let total_checks_passed: u64 = strategy_records
            .iter()
            .map(|r| r.checks_passed as u64)
            .sum();
        let total_checks_failed: u64 = strategy_records
            .iter()
            .map(|r| r.checks_failed as u64)
            .sum();
        let avg_checks_passed = if total_tasks > 0 {
            total_checks_passed as f64 / total_tasks as f64
        } else {
            0.0
        };
        let avg_checks_failed = if total_tasks > 0 {
            total_checks_failed as f64 / total_tasks as f64
        } else {
            0.0
        };

        // Collect all agent roles across records.
        let mut role_counts: std::collections::BTreeMap<String, u32> =
            std::collections::BTreeMap::new();
        for r in &strategy_records {
            for role in &r.agent_roles {
                *role_counts.entry(role.clone()).or_insert(0) += 1;
            }
        }
        // Sort by frequency descending, take top.
        let mut role_vec: Vec<(String, u32)> = role_counts.into_iter().collect();
        role_vec.sort_by(|a, b| b.1.cmp(&a.1));
        let common_agent_roles: Vec<String> = role_vec.into_iter().map(|(r, _)| r).collect();

        // Average duration.
        let durations: Vec<u64> = strategy_records
            .iter()
            .filter_map(|r| r.duration_secs)
            .collect();
        let avg_duration_secs = if durations.is_empty() {
            None
        } else {
            let sum: u64 = durations.iter().sum();
            Some(sum as f64 / durations.len() as f64)
        };

        let last_used = strategy_records.iter().map(|r| r.created_at).max();

        // Find or create stats entry.
        if let Some(existing) = self
            .strategies
            .iter_mut()
            .find(|s: &&mut StrategyStats| s.strategy_id == strategy_id)
        {
            existing.total_tasks = total_tasks;
            existing.success_count = success_count;
            existing.failed_count = failed_count;
            existing.aborted_count = aborted_count;
            existing.rolled_back_count = rolled_back_count;
            existing.avg_checks_passed = avg_checks_passed;
            existing.avg_checks_failed = avg_checks_failed;
            existing.common_agent_roles = common_agent_roles;
            existing.avg_duration_secs = avg_duration_secs;
            existing.last_used = last_used;
        } else {
            self.strategies.push(StrategyStats {
                strategy_id: strategy_id.to_string(),
                total_tasks,
                success_count,
                failed_count,
                aborted_count,
                rolled_back_count,
                avg_checks_passed,
                avg_checks_failed,
                common_agent_roles,
                avg_duration_secs,
                last_used,
            });
        }
    }

    /// Get statistics for a strategy.
    pub fn strategy_stats(&self, strategy_id: &str) -> Option<StrategyStats> {
        self.strategies
            .iter()
            .find(|s| s.strategy_id == strategy_id)
            .cloned()
    }

    /// Get all strategy statistics.
    pub fn all_stats(&self) -> Vec<&StrategyStats> {
        let mut result: Vec<&StrategyStats> = self.strategies.iter().collect();
        result.sort_by(|a, b| b.total_tasks.cmp(&a.total_tasks));
        result
    }

    /// Suggest the best strategy for a task based on historical evidence.
    ///
    /// Only shows facts and rankings — never auto-switches. Rankings are
    /// based on: success rate, task keyword similarity, and recency.
    /// Protocol always has higher priority.
    pub fn suggest_strategy(&self, task: &str, strategies: &[String]) -> Vec<StrategyRanking> {
        let task_lower = task.to_lowercase();
        let task_words: std::collections::BTreeSet<String> = task_lower
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2)
            .collect();

        let mut rankings: Vec<StrategyRanking> = Vec::new();

        for strategy_id in strategies {
            let stats = self.strategy_stats(strategy_id);
            let records_for_strategy: Vec<&ExperimentRecord> = self
                .records
                .iter()
                .filter(|r| r.strategy_id == *strategy_id)
                .collect();

            if records_for_strategy.is_empty() {
                // No data for this strategy — low score.
                rankings.push(StrategyRanking {
                    strategy_id: strategy_id.clone(),
                    score: 0.0,
                    reason: "No historical data for this strategy".to_string(),
                    evidence: Vec::new(),
                });
                continue;
            }

            // Factor 1: Success rate (0.0 - 1.0)
            let success_rate = match stats {
                Some(ref s) if s.total_tasks > 0 => s.success_count as f64 / s.total_tasks as f64,
                _ => 0.0,
            };

            // Factor 2: Task keyword similarity (0.0 - 1.0)
            let mut max_similarity = 0.0_f64;
            let mut similar_tasks: Vec<String> = Vec::new();
            for record in &records_for_strategy {
                let record_lower = record.task.to_lowercase();
                let record_words: std::collections::BTreeSet<String> = record_lower
                    .split_whitespace()
                    .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                    .filter(|w| w.len() > 2)
                    .collect();

                if !task_words.is_empty() && !record_words.is_empty() {
                    let intersection: Vec<&String> =
                        task_words.intersection(&record_words).collect();
                    let similarity =
                        intersection.len() as f64 / task_words.len().max(record_words.len()) as f64;
                    if similarity > max_similarity {
                        max_similarity = similarity;
                    }
                    if similarity > 0.0 {
                        similar_tasks.push(record.task.clone());
                    }
                }
            }

            // Factor 3: Recency bonus (0.0 - 0.2)
            let recency_bonus = match stats {
                Some(ref s) => {
                    if let Some(last) = s.last_used {
                        let now = route_core::now_millis();
                        let days_since = (now - last) as f64 / 86400000.0;
                        // Bonus decays over 30 days
                        (0.2_f64).max(0.0) - (days_since / 150.0).min(0.2)
                    } else {
                        0.0
                    }
                }
                None => 0.0,
            };

            // Combined score: success rate (50%) + keyword similarity (30%) + recency (20%)
            let score = success_rate * 0.5 + max_similarity * 0.3 + recency_bonus * 0.2;
            let score = score.clamp(0.0, 1.0);

            let reason = format!(
                "Success rate: {:.1}%, Task similarity: {:.1}%, Recency: {:.1}%",
                success_rate * 100.0,
                max_similarity * 100.0,
                recency_bonus * 100.0
            );

            let evidence: Vec<String> = similar_tasks
                .into_iter()
                .take(5)
                .map(|t| format!("Similar task: {}", t))
                .collect();

            rankings.push(StrategyRanking {
                strategy_id: strategy_id.clone(),
                score,
                reason,
                evidence,
            });
        }

        // Sort by score descending.
        rankings.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        rankings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_record(
        strategy_id: &str,
        result: &str,
        task: &str,
        roles: Vec<&str>,
        checks_passed: u32,
        checks_failed: u32,
        duration_secs: Option<u64>,
    ) -> ExperimentRecord {
        ExperimentRecord {
            id: route_core::new_id(),
            task: task.to_string(),
            strategy_id: strategy_id.to_string(),
            session_id: route_core::new_id(),
            result: result.to_string(),
            checks_passed,
            checks_failed,
            agent_roles: roles.into_iter().map(|s| s.to_string()).collect(),
            duration_secs,
            created_at: route_core::now_millis(),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn test_experiment_dir_and_path() {
        let tmp = TempDir::new().unwrap();
        let dir = experiment_dir(tmp.path());
        assert!(dir.ends_with(".route/experiment"));
        let p = experiment_path(tmp.path());
        assert!(p.ends_with(".route/experiment/experiment.json"));
    }

    #[test]
    fn test_load_empty_store() {
        let tmp = TempDir::new().unwrap();
        let store = ExperimentStore::load(tmp.path()).unwrap();
        assert!(store.records.is_empty());
        assert!(store.strategies.is_empty());
    }

    #[test]
    fn test_record_experiment() {
        let tmp = TempDir::new().unwrap();
        let mut store = ExperimentStore::default();

        let record = make_record(
            "strat-a",
            "success",
            "fix bug",
            vec!["primary", "verifier"],
            5,
            0,
            Some(120),
        );
        store.record_experiment(tmp.path(), record).unwrap();

        assert_eq!(store.records.len(), 1);
        assert_eq!(store.strategies.len(), 1);
        let stats = store.strategy_stats("strat-a").unwrap();
        assert_eq!(stats.total_tasks, 1);
        assert_eq!(stats.success_count, 1);
        assert_eq!(stats.avg_checks_passed, 5.0);
    }

    #[test]
    fn test_all_stats() {
        let tmp = TempDir::new().unwrap();
        let mut store = ExperimentStore::default();

        store
            .record_experiment(
                tmp.path(),
                make_record("strat-a", "success", "task1", vec!["primary"], 3, 0, None),
            )
            .unwrap();
        store
            .record_experiment(
                tmp.path(),
                make_record("strat-b", "failed", "task2", vec!["primary"], 1, 2, None),
            )
            .unwrap();

        let all = store.all_stats();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_suggest_strategy_no_data() {
        let store = ExperimentStore::default();
        let rankings = store.suggest_strategy("fix something", &["strat-a".to_string()]);
        assert_eq!(rankings.len(), 1);
        assert_eq!(rankings[0].score, 0.0);
        assert!(rankings[0].reason.contains("No historical data"));
    }

    #[test]
    fn test_suggest_strategy_with_data() {
        let tmp = TempDir::new().unwrap();
        let mut store = ExperimentStore::default();

        // strat-a: 2 successes, 0 failures
        store
            .record_experiment(
                tmp.path(),
                make_record(
                    "strat-a",
                    "success",
                    "fix bug in parser",
                    vec!["primary"],
                    5,
                    0,
                    Some(60),
                ),
            )
            .unwrap();
        store
            .record_experiment(
                tmp.path(),
                make_record(
                    "strat-a",
                    "success",
                    "add feature x",
                    vec!["primary", "verifier"],
                    8,
                    1,
                    Some(120),
                ),
            )
            .unwrap();

        // strat-b: 0 successes, 1 failure
        store
            .record_experiment(
                tmp.path(),
                make_record(
                    "strat-b",
                    "failed",
                    "fix database",
                    vec!["primary"],
                    0,
                    3,
                    Some(30),
                ),
            )
            .unwrap();

        let rankings = store.suggest_strategy(
            "fix parser bug",
            &["strat-a".to_string(), "strat-b".to_string()],
        );
        assert_eq!(rankings.len(), 2);
        // strat-a should rank higher
        assert!(rankings[0].score >= rankings[1].score);
        // strat-a should have a higher score
        let strat_a = rankings
            .iter()
            .find(|r| r.strategy_id == "strat-a")
            .unwrap();
        let strat_b = rankings
            .iter()
            .find(|r| r.strategy_id == "strat-b")
            .unwrap();
        assert!(strat_a.score > strat_b.score);
    }

    #[test]
    fn test_save_and_load() {
        let tmp = TempDir::new().unwrap();
        let mut store = ExperimentStore::default();

        store
            .record_experiment(
                tmp.path(),
                make_record(
                    "strat-a",
                    "success",
                    "test task",
                    vec!["primary"],
                    2,
                    0,
                    None,
                ),
            )
            .unwrap();

        // Reload
        let loaded = ExperimentStore::load(tmp.path()).unwrap();
        assert_eq!(loaded.records.len(), 1);
        assert_eq!(loaded.strategies.len(), 1);
        assert_eq!(loaded.strategies[0].strategy_id, "strat-a");
    }
}
