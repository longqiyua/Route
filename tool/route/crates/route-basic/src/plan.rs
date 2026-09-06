//! Counterfactual Preview — plan a task without executing it.
//!
//! This module provides purely analytical planning: it reads the current
//! project state (strategy, memory, references, patterns, workflows) and
//! compiles a deterministic plan showing what WOULD happen if the task
//! were executed. No LLM calls, no file mutations, no side effects.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agent_compiler::{self, AgentPlan, CompilerInput};
use crate::constitutive::{Protocol, ReferenceEntry, ReferenceRegistry};
use crate::memory::MemoryStore;
use crate::pattern::PatternStore;
use crate::strategy::{StrategySnapshot, StrategyStore};
use crate::workflow::{WorkflowDefinition, WorkflowStore};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A counterfactual plan — what WOULD happen if we ran this task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualPlan {
    pub task: String,
    pub strategy_id: Option<String>,
    pub selected_memory: Vec<String>,
    pub selected_references: Vec<String>,
    pub selected_patterns: Vec<String>,
    pub selected_workflow: Option<String>,
    pub agent_plan: AgentPlan,
    pub verification_estimate: String, // "minimal" | "standard" | "strict"
    pub context_budget_estimate: usize, // estimated characters
    pub checkpoints: Vec<String>,
    pub created_at: i64,
}

/// A comparison of two plans for the same task.
#[derive(Debug, Clone)]
pub struct PlanComparison {
    pub task: String,
    pub strategy_a: String,
    pub strategy_b: String,
    pub plan_a: CounterfactualPlan,
    pub plan_b: CounterfactualPlan,
    pub differences: Vec<String>,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Plan a task without executing it.
///
/// Reads the current project state and compiles a deterministic plan.
/// No LLM calls, no file mutations — purely analytical.
///
/// If `strategy_id` is `None`, the currently active strategy is used.
/// If no strategy is active, a plan is built from the raw project state.
pub fn plan_task(
    project_root: &Path,
    task: &str,
    strategy_id: Option<&str>,
) -> Result<CounterfactualPlan> {
    let now = route_core::now_millis();

    // Resolve strategy
    let (strategy_snapshot, resolved_strategy_id) = resolve_strategy(project_root, strategy_id)?;

    // Gather selected memory
    let selected_memory = gather_memory_summaries(project_root, strategy_snapshot.as_ref())?;

    // Gather selected references
    let selected_references = gather_reference_ids(project_root, strategy_snapshot.as_ref())?;

    // Gather selected patterns
    let selected_patterns = gather_pattern_ids(project_root)?;

    // Gather selected workflow
    let (selected_workflow_id, workflow) =
        resolve_workflow(project_root, strategy_snapshot.as_ref())?;

    // Read protocol for agent policy
    let agent_policy = read_agent_policy(project_root)?;

    // Collect reference entries for the compiler
    let registry = ReferenceRegistry::read(project_root)?;
    let references: Vec<ReferenceEntry> = if let Some(ref ss) = strategy_snapshot {
        registry
            .entries
            .iter()
            .filter(|e| ss.enabled_refs.contains(&e.id))
            .cloned()
            .collect()
    } else {
        registry
            .entries
            .iter()
            .filter(|e| e.enabled)
            .cloned()
            .collect()
    };

    // Get context hash
    let context_hash = crate::constitutive::effective_context_fingerprint(project_root)
        .unwrap_or_else(|_| "unknown".to_string());

    // Build compiler input and compile agent plan
    let input = CompilerInput {
        task: task.to_string(),
        agent_policy,
        workflow,
        memory: None,
        references,
        context_hash,
        project_root: Some(project_root.to_path_buf()),
    };
    let agent_plan = agent_compiler::compile_plan(input)?;

    // Estimate verification level
    let verification_estimate = estimate_verification(task, &agent_plan);

    // Estimate context budget (rough heuristic based on plan complexity)
    let context_budget_estimate =
        estimate_context_budget(&agent_plan, &selected_references, &selected_patterns);

    // Generate checkpoints
    let checkpoints = generate_checkpoints(task, &agent_plan);

    Ok(CounterfactualPlan {
        task: task.to_string(),
        strategy_id: resolved_strategy_id,
        selected_memory,
        selected_references,
        selected_patterns,
        selected_workflow: selected_workflow_id,
        agent_plan,
        verification_estimate,
        context_budget_estimate,
        checkpoints,
        created_at: now,
    })
}

/// Compare plans for the same task under different strategies.
pub fn compare_plans(
    project_root: &Path,
    task: &str,
    strategies: &[String],
) -> Result<PlanComparison> {
    if strategies.len() < 2 {
        anyhow::bail!("compare_plans requires at least two strategy IDs");
    }

    let strategy_a = &strategies[0];
    let strategy_b = &strategies[1];

    let plan_a = plan_task(project_root, task, Some(strategy_a))?;
    let plan_b = plan_task(project_root, task, Some(strategy_b))?;

    let differences = compute_plan_differences(&plan_a, &plan_b);
    let summary = generate_comparison_summary(&plan_a, &plan_b);

    Ok(PlanComparison {
        task: task.to_string(),
        strategy_a: strategy_a.clone(),
        strategy_b: strategy_b.clone(),
        plan_a,
        plan_b,
        differences,
        summary,
    })
}

// ---------------------------------------------------------------------------
// Plan rendering
// ---------------------------------------------------------------------------

/// Render a CounterfactualPlan as a human-readable string.
pub fn render_plan(plan: &CounterfactualPlan) -> String {
    let mut out = String::new();
    out.push_str(&format!("Plan for: {:?}\n", plan.task));
    out.push_str(&format!(
        "Strategy: {}\n\n",
        plan.strategy_id.as_deref().unwrap_or("current")
    ));

    // Selected Memory
    out.push_str("Selected Memory:\n");
    if plan.selected_memory.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for m in &plan.selected_memory {
            out.push_str(&format!("  - {}\n", m));
        }
    }
    out.push('\n');

    // Selected References
    out.push_str("Selected References:\n");
    if plan.selected_references.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for r in &plan.selected_references {
            out.push_str(&format!("  - {}\n", r));
        }
    }
    out.push('\n');

    // Selected Patterns
    out.push_str("Selected Patterns:\n");
    if plan.selected_patterns.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for p in &plan.selected_patterns {
            out.push_str(&format!("  - {}\n", p));
        }
    }
    out.push('\n');

    // Workflow
    out.push_str(&format!(
        "Workflow: {}\n\n",
        plan.selected_workflow.as_deref().unwrap_or("none")
    ));

    // Agent Plan
    out.push_str("Agent Plan:\n");
    out.push_str(&format!("  Mode: {}\n", plan.agent_plan.mode));
    out.push_str("  Agents:\n");
    for agent in &plan.agent_plan.agents {
        out.push_str(&format!("    - Role: {}\n", agent.role));
        out.push_str(&format!("      Goal: {}\n", agent.goal));
        if !agent.tools.is_empty() {
            out.push_str(&format!("      Tools: [{}]\n", agent.tools.join(", ")));
        }
        if !agent.completion_criteria.is_empty() {
            out.push_str("      Completion Criteria:\n");
            for c in &agent.completion_criteria {
                out.push_str(&format!("        - {}\n", c));
            }
        }
    }
    out.push('\n');

    // Verification
    out.push_str(&format!("Verification: {}\n", plan.verification_estimate));
    out.push_str(&format!(
        "Context Budget: ~{} chars\n\n",
        plan.context_budget_estimate
    ));

    // Checkpoints
    out.push_str("Checkpoints:\n");
    if plan.checkpoints.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for cp in &plan.checkpoints {
            out.push_str(&format!("  - {}\n", cp));
        }
    }

    out
}

/// Render a PlanComparison as a human-readable string.
pub fn render_comparison(comparison: &PlanComparison) -> String {
    let mut out = String::new();
    out.push_str(&format!("Plan Comparison for: {:?}\n", comparison.task));
    out.push_str(&format!(
        "Strategy A: {}  vs  Strategy B: {}\n\n",
        comparison.strategy_a, comparison.strategy_b
    ));

    out.push_str("Differences:\n");
    if comparison.differences.is_empty() {
        out.push_str("  (no differences)\n");
    } else {
        for d in &comparison.differences {
            out.push_str(&format!("  - {}\n", d));
        }
    }
    out.push('\n');

    out.push_str(&format!("Summary: {}\n", comparison.summary));

    out
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Resolve a strategy snapshot by ID, or use the current active strategy.
fn resolve_strategy(
    project_root: &Path,
    strategy_id: Option<&str>,
) -> Result<(Option<StrategySnapshot>, Option<String>)> {
    let store = StrategyStore::load(project_root)?;

    match strategy_id {
        Some(id) => {
            let snapshot = store
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Strategy '{}' not found", id))?;
            Ok((Some(snapshot), Some(id.to_string())))
        }
        None => {
            // Use current strategy if available
            if let Some(current_id) = store.current_strategy() {
                let snapshot = store.get(current_id).cloned().ok_or_else(|| {
                    anyhow::anyhow!("Current strategy '{}' not found in store", current_id)
                })?;
                Ok((Some(snapshot), Some(current_id.to_string())))
            } else {
                Ok((None, None))
            }
        }
    }
}

/// Gather memory summaries from the current project memory.
fn gather_memory_summaries(
    project_root: &Path,
    _strategy: Option<&StrategySnapshot>,
) -> Result<Vec<String>> {
    let mut summaries = Vec::new();

    // Load memory store
    if let Ok(store) = MemoryStore::load(project_root) {
        if let Some(memory) = store.current_memory() {
            if !memory.summary.is_empty() {
                summaries.push(memory.summary.clone());
            }
            if !memory.current_focus.is_empty() {
                summaries.push(format!("Focus: {}", memory.current_focus));
            }
            // Include key decisions and conventions
            for item in &memory.decisions {
                summaries.push(format!("Decision: {}", item.content));
            }
            for item in &memory.conventions {
                summaries.push(format!("Convention: {}", item.content));
            }
            // Include known risks
            for item in &memory.known_risks {
                summaries.push(format!("Risk: {}", item.content));
            }
        }
    }

    // Truncate to a reasonable limit
    summaries.truncate(20);
    Ok(summaries)
}

/// Gather enabled reference IDs from the registry.
fn gather_reference_ids(
    project_root: &Path,
    strategy: Option<&StrategySnapshot>,
) -> Result<Vec<String>> {
    let registry = ReferenceRegistry::read(project_root)?;

    let ids: Vec<String> = if let Some(ss) = strategy {
        // Use strategy's enabled refs
        ss.enabled_refs.clone()
    } else {
        // Use currently enabled refs
        registry
            .entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.id.clone())
            .collect()
    };

    Ok(ids)
}

/// Gather pattern IDs from the pattern store.
fn gather_pattern_ids(project_root: &Path) -> Result<Vec<String>> {
    let store = PatternStore::load(project_root).unwrap_or_default();
    let ids: Vec<String> = store.patterns.iter().map(|p| p.id.clone()).collect();
    Ok(ids)
}

/// Resolve the workflow to use.
fn resolve_workflow(
    project_root: &Path,
    strategy: Option<&StrategySnapshot>,
) -> Result<(Option<String>, Option<WorkflowDefinition>)> {
    let workflow_store = WorkflowStore::load(project_root)?;

    let wf_ids: Vec<String> = if let Some(ss) = strategy {
        ss.workflow_ids.clone()
    } else {
        workflow_store
            .workflows
            .iter()
            .filter(|w| w.enabled)
            .map(|w| w.id.clone())
            .collect()
    };

    if let Some(first_id) = wf_ids.first() {
        let wf = workflow_store.get(first_id).cloned().ok_or_else(|| {
            anyhow::anyhow!("Workflow '{}' referenced but not found on disk", first_id)
        })?;
        Ok((Some(first_id.clone()), Some(wf)))
    } else {
        Ok((None, None))
    }
}

/// Read the agent policy from the protocol.
fn read_agent_policy(project_root: &Path) -> Result<Option<String>> {
    match Protocol::read(project_root) {
        Ok(protocol) => {
            let policy = protocol.body.lines().find_map(|line| {
                let trimmed = line.trim();
                trimmed
                    .strip_prefix("<!-- route-agent-policy:")
                    .and_then(|s| s.strip_suffix(" -->"))
                    .map(|s| s.trim().to_string())
            });
            Ok(policy)
        }
        Err(_) => Ok(None),
    }
}

/// Estimate the verification level based on task keywords and agent plan.
fn estimate_verification(task: &str, plan: &AgentPlan) -> String {
    let lower = task.to_lowercase();

    // Check for strict verification keywords
    let strict_keywords = [
        "security",
        "audit",
        "production",
        "critical",
        "safety",
        "compliance",
        "regulatory",
        "certification",
    ];
    let has_strict = strict_keywords.iter().any(|kw| lower.contains(kw));

    // Check for standard verification keywords
    let standard_keywords = ["test", "verify", "validate", "refactor", "feature", " add "];
    let has_standard = standard_keywords.iter().any(|kw| lower.contains(kw));

    // Check agent plan for verification agents
    let has_verifier = plan.agents.iter().any(|a| a.role == "verifier");

    // Determine level
    if has_strict {
        "strict".to_string()
    } else if has_verifier || has_standard || plan.mode == "adaptive" {
        "standard".to_string()
    } else {
        "minimal".to_string()
    }
}

/// Estimate the context budget based on plan complexity.
fn estimate_context_budget(plan: &AgentPlan, refs: &[String], patterns: &[String]) -> usize {
    let mut budget: usize = 0;

    // Base overhead
    budget += 5000;

    // Per-agent overhead
    budget += plan.agents.len() * 2000;

    // Per-agent goal and tools
    for agent in &plan.agents {
        budget += agent.goal.len();
        budget += agent.tools.iter().map(|t| t.len() + 10).sum::<usize>();
        budget += agent
            .completion_criteria
            .iter()
            .map(|c| c.len())
            .sum::<usize>();
    }

    // References
    budget += refs.len() * 800;

    // Patterns
    budget += patterns.len() * 600;

    // Workflow overhead
    if plan.mode == "adaptive" {
        budget += 3000;
    }

    budget
}

/// Generate a list of checkpoint names for the plan.
fn generate_checkpoints(_task: &str, plan: &AgentPlan) -> Vec<String> {
    let mut checkpoints = Vec::new();

    // Always start with a pre-task checkpoint
    checkpoints.push("pre-task snapshot".to_string());

    // Add checkpoints for each agent
    for agent in &plan.agents {
        checkpoints.push(format!("before agent: {}", agent.role));
    }

    // Add verification checkpoint if applicable
    if plan.agents.iter().any(|a| a.role == "verifier") {
        checkpoints.push("before verification".to_string());
    }

    // Add a post-task checkpoint
    checkpoints.push("post-task snapshot".to_string());

    checkpoints
}

/// Compute a list of human-readable differences between two plans.
fn compute_plan_differences(a: &CounterfactualPlan, b: &CounterfactualPlan) -> Vec<String> {
    let mut diffs = Vec::new();

    // Agent plan mode
    if a.agent_plan.mode != b.agent_plan.mode {
        diffs.push(format!(
            "Agent Plan: {} vs {}",
            a.agent_plan.mode, b.agent_plan.mode
        ));
    }

    // Verification
    if a.verification_estimate != b.verification_estimate {
        diffs.push(format!(
            "Verification: {} vs {}",
            a.verification_estimate, b.verification_estimate
        ));
    }

    // Context budget
    if a.context_budget_estimate != b.context_budget_estimate {
        diffs.push(format!(
            "Context Budget: {} vs {}",
            a.context_budget_estimate, b.context_budget_estimate
        ));
    }

    // Workflow
    if a.selected_workflow != b.selected_workflow {
        diffs.push(format!(
            "Workflow: {:?} vs {:?}",
            a.selected_workflow, b.selected_workflow
        ));
    }

    // Number of agents
    if a.agent_plan.agents.len() != b.agent_plan.agents.len() {
        diffs.push(format!(
            "Agent Count: {} vs {}",
            a.agent_plan.agents.len(),
            b.agent_plan.agents.len()
        ));
    }

    // Agent roles
    let a_roles: Vec<&str> = a
        .agent_plan
        .agents
        .iter()
        .map(|a| a.role.as_str())
        .collect();
    let b_roles: Vec<&str> = b
        .agent_plan
        .agents
        .iter()
        .map(|a| a.role.as_str())
        .collect();
    if a_roles != b_roles {
        diffs.push(format!(
            "Agent Roles: [{:?}] vs [{:?}]",
            a_roles.join(", "),
            b_roles.join(", ")
        ));
    }

    // References
    if a.selected_references != b.selected_references {
        let a_refs: std::collections::BTreeSet<_> = a.selected_references.iter().collect();
        let b_refs: std::collections::BTreeSet<_> = b.selected_references.iter().collect();
        let added: Vec<_> = b_refs.difference(&a_refs).map(|s| s.to_string()).collect();
        let removed: Vec<_> = a_refs.difference(&b_refs).map(|s| s.to_string()).collect();
        if !added.is_empty() {
            diffs.push(format!(
                "References (+{}): {}",
                added.len(),
                added.join(", ")
            ));
        }
        if !removed.is_empty() {
            diffs.push(format!(
                "References (-{}): {}",
                removed.len(),
                removed.join(", ")
            ));
        }
    }

    // Memory
    if a.selected_memory.len() != b.selected_memory.len() {
        diffs.push(format!(
            "Memory Items: {} vs {}",
            a.selected_memory.len(),
            b.selected_memory.len()
        ));
    }

    // Patterns
    if a.selected_patterns.len() != b.selected_patterns.len() {
        diffs.push(format!(
            "Patterns: {} vs {}",
            a.selected_patterns.len(),
            b.selected_patterns.len()
        ));
    }

    // Checkpoints
    if a.checkpoints.len() != b.checkpoints.len() {
        diffs.push(format!(
            "Checkpoints: {} vs {}",
            a.checkpoints.len(),
            b.checkpoints.len()
        ));
    }

    diffs
}

/// Generate a summary of what the user can expect to be different.
fn generate_comparison_summary(a: &CounterfactualPlan, b: &CounterfactualPlan) -> String {
    let mut parts = Vec::new();

    if a.agent_plan.mode != b.agent_plan.mode {
        parts.push(format!(
            "agent mode changes from '{}' to '{}'",
            a.agent_plan.mode, b.agent_plan.mode
        ));
    }

    if a.verification_estimate != b.verification_estimate {
        parts.push(format!(
            "verification level changes from '{}' to '{}'",
            a.verification_estimate, b.verification_estimate
        ));
    }

    let budget_diff = b.context_budget_estimate as i64 - a.context_budget_estimate as i64;
    if budget_diff.abs() > 1000 {
        let direction = if budget_diff > 0 {
            "increases"
        } else {
            "decreases"
        };
        parts.push(format!(
            "context budget {} by {} chars",
            direction,
            budget_diff.abs()
        ));
    }

    if a.selected_workflow != b.selected_workflow {
        parts.push(format!(
            "workflow changes from {:?} to {:?}",
            a.selected_workflow, b.selected_workflow
        ));
    }

    let agent_diff = b.agent_plan.agents.len() as i64 - a.agent_plan.agents.len() as i64;
    if agent_diff != 0 {
        let direction = if agent_diff > 0 { "more" } else { "fewer" };
        parts.push(format!("{} {} agents", agent_diff.abs(), direction));
    }

    if parts.is_empty() {
        "No significant differences between the two strategies.".to_string()
    } else {
        format!("Switching strategies would: {}", parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_plan_task_no_strategy() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        let plan = plan_task(root, "fix typo in README", None).unwrap();
        assert_eq!(plan.task, "fix typo in README");
        assert!(plan.strategy_id.is_none());
        assert_eq!(plan.verification_estimate, "minimal");
        assert!(!plan.checkpoints.is_empty());
        assert!(plan.context_budget_estimate > 0);
    }

    #[test]
    fn test_plan_task_adaptive_detection() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        let plan = plan_task(
            root,
            "refactor the authentication module to use OAuth2",
            None,
        )
        .unwrap();
        // Should detect as adaptive
        assert_eq!(plan.agent_plan.mode, "adaptive");
        assert_eq!(plan.verification_estimate, "standard");
    }

    #[test]
    fn test_plan_task_verification_estimate() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        // Security-related task → strict
        let plan = plan_task(root, "security audit of the authentication module", None).unwrap();
        assert_eq!(plan.verification_estimate, "strict");

        // Test-related task → standard (has verifier agent)
        let plan = plan_task(root, "add tests for the parser", None).unwrap();
        assert_eq!(plan.verification_estimate, "standard");
    }

    #[test]
    fn test_plan_task_deterministic() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        let plan1 = plan_task(root, "fix typo in README", None).unwrap();
        let plan2 = plan_task(root, "fix typo in README", None).unwrap();

        // Same input → same output (modulo timestamps)
        assert_eq!(plan1.agent_plan.mode, plan2.agent_plan.mode);
        assert_eq!(plan1.agent_plan.agents.len(), plan2.agent_plan.agents.len());
        assert_eq!(plan1.verification_estimate, plan2.verification_estimate);
        assert_eq!(plan1.selected_references, plan2.selected_references);
    }

    #[test]
    fn test_plan_comparison_requires_two_strategies() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        // Write a minimal strategy store with two snapshots
        let mut store = StrategyStore::default();
        let snap1 = store
            .record_snapshot(root, Some("A".to_string()), None)
            .unwrap();
        let snap2 = store
            .record_snapshot(root, Some("B".to_string()), None)
            .unwrap();
        store.save(root).unwrap();

        let strategies = vec![snap1.id.clone(), snap2.id.clone()];
        let comparison = compare_plans(root, "fix typo", &strategies).unwrap();
        assert_eq!(comparison.task, "fix typo");
        assert_eq!(comparison.strategy_a, snap1.id);
        assert_eq!(comparison.strategy_b, snap2.id);
    }

    #[test]
    fn test_render_plan_output() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        let plan = plan_task(root, "fix typo in README", None).unwrap();
        let rendered = render_plan(&plan);
        assert!(rendered.contains("Plan for:"));
        assert!(rendered.contains("Strategy:"));
        assert!(rendered.contains("Agent Plan:"));
        assert!(rendered.contains("Verification:"));
        assert!(rendered.contains("Context Budget:"));
        assert!(rendered.contains("Checkpoints:"));
    }

    #[test]
    fn test_render_comparison_output() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        let mut store = StrategyStore::default();
        let snap1 = store
            .record_snapshot(root, Some("A".to_string()), None)
            .unwrap();
        let snap2 = store
            .record_snapshot(root, Some("B".to_string()), None)
            .unwrap();
        store.save(root).unwrap();

        let strategies = vec![snap1.id.clone(), snap2.id.clone()];
        let comparison = compare_plans(root, "fix typo", &strategies).unwrap();
        let rendered = render_comparison(&comparison);
        assert!(rendered.contains("Plan Comparison for:"));
        assert!(rendered.contains("Strategy A:"));
        assert!(rendered.contains("Strategy B:"));
        assert!(rendered.contains("Summary:"));
    }

    #[test]
    fn test_estimate_verification() {
        use crate::agent_compiler::AgentPlan;

        let plan_single = AgentPlan {
            mode: "single".to_string(),
            agents: vec![],
            created_at: 0,
            context_hash: "".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        assert_eq!(estimate_verification("fix typo", &plan_single), "minimal");
        assert_eq!(estimate_verification("add tests", &plan_single), "standard");
        assert_eq!(
            estimate_verification("security audit", &plan_single),
            "strict"
        );
    }

    #[test]
    fn test_generate_checkpoints() {
        use crate::agent_compiler::AgentPlan;

        let plan = AgentPlan {
            mode: "single".to_string(),
            agents: vec![],
            created_at: 0,
            context_hash: "".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        let checkpoints = generate_checkpoints("fix typo", &plan);
        assert!(checkpoints.contains(&"pre-task snapshot".to_string()));
        assert!(checkpoints.contains(&"post-task snapshot".to_string()));
    }

    #[test]
    fn test_compute_plan_differences() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        crate::init_profile_store(root).unwrap();

        let plan_a = plan_task(root, "fix typo", None).unwrap();
        let plan_b = plan_task(root, "refactor module", None).unwrap();

        let diffs = compute_plan_differences(&plan_a, &plan_b);
        // The two plans should differ in at least mode/verification
        assert!(
            !diffs.is_empty(),
            "expected differences between different tasks"
        );
    }
}
