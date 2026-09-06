//! Project Brief and Handoff — compressed project state for AI onboarding.
//!
//! `route brief` generates a concise project summary from real data.
//! `route handoff` generates a session handoff document for a new AI.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A project brief — compressed, data-backed summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectBrief {
    /// Project purpose
    pub purpose: String,
    /// Current goals
    #[serde(default)]
    pub current_goals: Vec<String>,
    /// Architecture summary
    #[serde(default)]
    pub architecture: String,
    /// Critical invariants
    #[serde(default)]
    pub critical_invariants: Vec<String>,
    /// Current strategy label
    #[serde(default)]
    pub current_strategy: String,
    /// Active work
    #[serde(default)]
    pub active_work: Vec<String>,
    /// Known risks
    #[serde(default)]
    pub known_risks: Vec<String>,
    /// Recent decisions
    #[serde(default)]
    pub recent_decisions: Vec<String>,
    /// Recent failures
    #[serde(default)]
    pub recent_failures: Vec<String>,
    /// Important references
    #[serde(default)]
    pub important_references: Vec<String>,
    /// Open questions
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// Recommended next action
    #[serde(default)]
    pub recommended_next: String,
    /// When this brief was generated
    pub created_at: i64,
}

/// A handoff document — everything a new AI session needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffDocument {
    /// Project brief
    pub brief: ProjectBrief,
    /// Current task/session info
    #[serde(default)]
    pub current_task: Option<String>,
    /// Current session ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_session_id: Option<String>,
    /// Unfinished work
    #[serde(default)]
    pub unfinished_work: Vec<String>,
    /// Active strategy
    #[serde(default)]
    pub active_strategy: String,
    /// Active workflow
    #[serde(default)]
    pub active_workflow: String,
    /// Relevant failures
    #[serde(default)]
    pub relevant_failures: Vec<String>,
    /// Next expected action
    #[serde(default)]
    pub next_expected_action: String,
    /// When this handoff was generated
    pub created_at: i64,
}

/// Generate a project brief from available data.
pub fn generate_brief(project_root: &Path, task: Option<&str>) -> Result<ProjectBrief> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut goals = Vec::new();
    let active_work = Vec::new();
    let mut risks = Vec::new();
    let mut decisions = Vec::new();
    let mut failures = Vec::new();
    let mut refs = Vec::new();
    let mut questions = Vec::new();
    let mut invariants = Vec::new();

    // Load memory
    if let Ok(store) = crate::memory::MemoryStore::load(project_root) {
        if let Some(mem) = store.current_memory() {
            for item in mem.all_items() {
                match item.kind {
                    crate::memory::MemoryItemKind::Decision => {
                        decisions.push(item.content.clone());
                    }
                    crate::memory::MemoryItemKind::Invariant => {
                        invariants.push(item.content.clone());
                    }
                    crate::memory::MemoryItemKind::OpenQuestion => {
                        questions.push(item.content.clone());
                    }
                    crate::memory::MemoryItemKind::FailedAttempt => {
                        failures.push(item.content.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    // Load goals
    if let Ok(store) = crate::goal::GoalStore::load(project_root) {
        for g in store.active_goals() {
            goals.push(g.title.clone());
        }
    }

    // Load guardian findings for risks
    if let Ok(store) = crate::guardian::GuardianFindingsStore::load(project_root) {
        for f in store.open_findings() {
            risks.push(format!("[{}] {}", format!("{:?}", f.severity), f.title));
        }
    }

    // Load references
    {
        let registry = crate::constitutive::ReferenceRegistry::read(project_root)?;
        for e in &registry.entries {
            if e.enabled {
                refs.push(format!("{} ({})", e.name, e.id));
            }
        }
    }

    // Load current strategy
    let current_strategy =
        if let Ok(strategy_store) = crate::strategy::StrategyStore::load(project_root) {
            strategy_store
                .current_strategy()
                .unwrap_or("default")
                .to_string()
        } else {
            "default".to_string()
        };

    let purpose = if let Some(t) = task {
        format!("Task-specific session: {}", t)
    } else {
        "Route project — evolution & strategy lab for AI-assisted development".to_string()
    };

    let architecture = "Route uses a modular architecture: core (paths/hash/guard), basic (execution/memory/strategy/study), cli, engine (fuzzy search/token budget)".to_string();

    Ok(ProjectBrief {
        purpose,
        current_goals: goals,
        architecture,
        critical_invariants: invariants,
        current_strategy,
        active_work,
        known_risks: risks,
        recent_decisions: decisions,
        recent_failures: failures,
        important_references: refs,
        open_questions: questions,
        recommended_next: "Run `route guardian scan` then `route next`".to_string(),
        created_at: now,
    })
}

/// Generate a handoff document.
pub fn generate_handoff(project_root: &Path) -> Result<HandoffDocument> {
    let brief = generate_brief(project_root, None)?;

    let mut unfinished_work = Vec::new();

    // Check for open sessions
    if let Ok(store) = crate::execution::SessionStore::load(project_root) {
        for session in &store.sessions {
            if session.status == crate::execution::SessionStatus::Active {
                unfinished_work.push(format!("Open session: {} ({})", session.task, session.id));
            }
        }
    }

    // Check for accepted ideas
    if let Ok(store) = crate::idea::IdeaStore::load(project_root) {
        for idea in &store.ideas {
            if matches!(idea.status, crate::idea::IdeaStatus::Accepted) {
                unfinished_work.push(format!("Accepted idea: {}", idea.text));
            }
        }
    }

    // Load active workflow
    let active_workflow = "default (wf-cargo-build)".to_string();

    let handoff = HandoffDocument {
        brief,
        current_task: None,
        current_session_id: None,
        unfinished_work,
        active_strategy: "default".to_string(),
        active_workflow,
        relevant_failures: vec![],
        next_expected_action: "Review project brief and run `route guardian scan`".to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
    };

    Ok(handoff)
}

/// Format a project brief for display.
pub fn format_brief(brief: &ProjectBrief) -> String {
    let mut out = format!(
        "=== Project Brief ===\nGenerated: {}\n\nPurpose: {}\n\n",
        brief.created_at, brief.purpose
    );

    if !brief.current_goals.is_empty() {
        out.push_str(&format!("Goals ({}):\n", brief.current_goals.len()));
        for g in &brief.current_goals {
            out.push_str(&format!("  ◉ {}\n", g));
        }
        out.push('\n');
    }

    out.push_str(&format!("Architecture: {}\n\n", brief.architecture));

    if !brief.critical_invariants.is_empty() {
        out.push_str("Critical Invariants:\n");
        for inv in &brief.critical_invariants {
            out.push_str(&format!("  ⚠ {}\n", inv));
        }
        out.push('\n');
    }

    out.push_str(&format!("Strategy: {}\n\n", brief.current_strategy));

    if !brief.known_risks.is_empty() {
        out.push_str("Known Risks:\n");
        for r in &brief.known_risks {
            out.push_str(&format!("  ⚠ {}\n", r));
        }
        out.push('\n');
    }

    if !brief.recent_decisions.is_empty() {
        out.push_str(&format!(
            "Recent Decisions ({}):\n",
            brief.recent_decisions.len()
        ));
        for d in &brief.recent_decisions {
            out.push_str(&format!("  • {}\n", d));
        }
        out.push('\n');
    }

    if !brief.open_questions.is_empty() {
        out.push_str("Open Questions:\n");
        for q in &brief.open_questions {
            out.push_str(&format!("  ? {}\n", q));
        }
        out.push('\n');
    }

    out.push_str(&format!("Next: {}\n", brief.recommended_next));

    out
}

/// Format a handoff document for display.
pub fn format_handoff(handoff: &HandoffDocument) -> String {
    let mut out = String::from("=== HANDOFF ===\n\n");
    out.push_str(&format_brief(&handoff.brief));
    out.push('\n');

    if !handoff.unfinished_work.is_empty() {
        out.push_str("Unfinished Work:\n");
        for w in &handoff.unfinished_work {
            out.push_str(&format!("  ⊘ {}\n", w));
        }
        out.push('\n');
    }

    out.push_str(&format!("Active Strategy: {}\n", handoff.active_strategy));
    out.push_str(&format!("Active Workflow: {}\n", handoff.active_workflow));
    out.push_str(&format!(
        "\nNext Expected Action: {}\n",
        handoff.next_expected_action
    ));

    out
}
