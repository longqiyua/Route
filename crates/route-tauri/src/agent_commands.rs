//! Agent + Benchmark IPC commands.
//!
//! Exposes the `route-agent` and `route-bench` crates to the Tauri frontend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

// ============================================================
// Model registry helpers (shared with route-cli)
// ============================================================

fn build_registry(project_path: Option<&std::path::Path>) -> Result<route_agent::ModelRegistry, String> {
    use route_agent::model::{MockPluginModel, OpenAiLikeConfig, OpenAiPluginModel};
    let mut reg = route_agent::ModelRegistry::new();
    reg.register(MockPluginModel::new("agent-mock"));

    // Try plugins.json -> fallback to env vars.
    let mut key: Option<String> = None;
    let mut base: Option<String> = None;
    let mut model: Option<String> = None;
    if let Some(proj) = project_path {
        let rd = proj.join(".route");
        if let Ok(cfg) = route_plugins::load_plugin_config(&rd) {
            for p in cfg.plugins {
                if let Some(m) = p.config.as_object() {
                    if key.is_none() { key = m.get("openai_api_key").and_then(|v| v.as_str()).map(String::from); }
                    if base.is_none() { base = m.get("openai_base_url").and_then(|v| v.as_str()).map(String::from); }
                    if model.is_none() { model = m.get("openai_model").and_then(|v| v.as_str()).map(String::from); }
                }
            }
        }
    }
    if key.is_none() { key = std::env::var("OPENAI_API_KEY").ok(); }
    if base.is_none() { base = std::env::var("OPENAI_BASE_URL").ok(); }
    if model.is_none() {
        model = std::env::var("OPENAI_MODEL").ok().or(Some("gpt-4o-mini".to_string()));
    }
    if let Some(k) = key {
        reg.register(OpenAiPluginModel::new(OpenAiLikeConfig {
            api_key: k,
            base_url: base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model: model.unwrap_or_else(|| "gpt-4o-mini".to_string()),
            temperature: 0.7,
            max_tokens: None,
        }));
    }
    reg.register(MockPluginModel::echo("agent-echo"));
    Ok(reg)
}

// ============================================================
// Return types
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelInfo {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub code_quality: u8,
    pub tool_calling: bool,
    pub long_context: bool,
    pub features: Vec<String>,
    pub cost_per_second_cents: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkillInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub triggers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResponse {
    pub success: bool,
    pub status: String,
    pub iterations: usize,
    pub duration_ms: u128,
    pub final_answer: String,
    pub tool_calls: Vec<String>,
    pub skill_executions: Vec<String>,
    pub memory_total_entries: usize,
}

// ============================================================
// Tauri commands
// ============================================================

#[tauri::command]
pub async fn agent_list_models(
    _state: State<'_, AppState>,
    project_path: Option<String>,
) -> Result<Vec<AgentModelInfo>, String> {
    let pp = project_path.map(std::path::PathBuf::from);
    let reg = build_registry(pp.as_deref())?;
    Ok(reg.list_models().into_iter().map(|m| AgentModelInfo {
        id: m.id.clone(),
        display_name: m.display_name,
        kind: format!("{:?}", m.capability.kind).to_lowercase(),
        code_quality: m.capability.code_quality,
        tool_calling: m.capability.tool_calling,
        long_context: m.capability.long_context,
        features: m.capability.features,
        cost_per_second_cents: m.capability.cost_per_second_cents,
    }).collect())
}

#[tauri::command]
pub async fn agent_list_skills(
    _state: State<'_, AppState>,
    project_path: Option<String>,
) -> Result<Vec<AgentSkillInfo>, String> {
    let pp = project_path.map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let skills_dir = pp.as_ref()
        .map(|p| p.join(".route").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".route/skills"));
    let harness = route_agent::skill::SkillHarness::load_from_dir(&skills_dir)
        .unwrap_or_else(|_| route_agent::skill::SkillHarness::new());
    Ok(harness.list().into_iter().map(|s| AgentSkillInfo {
        id: s.id.clone(),
        name: s.name.clone(),
        version: s.version.clone(),
        description: s.description.clone(),
        triggers: s.triggers.clone(),
    }).collect())
}

#[tauri::command]
pub async fn agent_run_task(
    _state: State<'_, AppState>,
    task: String,
    project_path: Option<String>,
    model_id: Option<String>,
    max_iterations: Option<usize>,
) -> Result<AgentRunResponse, String> {
    let pp = project_path.map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| "cannot determine project dir".to_string())?;
    let reg = build_registry(Some(&pp))?;
    let skills_dir = pp.join(".route").join("skills");
    let harness = route_agent::skill::SkillHarness::load_from_dir(&skills_dir)
        .unwrap_or_else(|_| route_agent::skill::SkillHarness::new());
    let mut agent = route_agent::Agent::new(reg, harness)
        .with_project(pp.clone())
        .with_max_iterations(max_iterations.unwrap_or(20));
    if let Some(mid) = model_id {
        agent.select_model(&mid).map_err(|e| e.to_string())?;
    }
    let r = agent.run(task);
    let mut tools = Vec::new();
    let mut skills = Vec::new();
    for log in &r.logs {
        tools.extend(log.tool_calls.clone());
        skills.extend(log.skill_executions.clone());
    }
    Ok(AgentRunResponse {
        success: r.success,
        status: format!("{:?}", r.status).to_lowercase(),
        iterations: r.iterations,
        duration_ms: r.total_duration_ms,
        final_answer: r.final_response,
        tool_calls: tools,
        skill_executions: skills,
        memory_total_entries: r.memory_stats.total_entries,
    })
}

// ============================================================
// Benchmark commands
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSuiteInfo {
    pub id: String,
    pub name: String,
    pub cases: usize,
}

#[tauri::command]
pub async fn bench_list_suites(
    _state: State<'_, AppState>,
) -> Result<Vec<BenchSuiteInfo>, String> {
    use route_bench::suite::BenchmarkRegistry;
    let reg = BenchmarkRegistry::new();
    Ok(reg.list_suites().into_iter().map(|(id, name, cases)| BenchSuiteInfo {
        id: id.to_string(),
        name: name.to_string(),
        cases,
    }).collect())
}

#[tauri::command]
pub async fn bench_run_suite(
    _state: State<'_, AppState>,
    suite_id: String,
    format: Option<String>,
    work_dir: Option<String>,
) -> Result<String, String> {
    use route_bench::suite::BenchmarkRegistry;
    use route_bench::report::{Format, render_report};
    let reg = match work_dir {
        Some(p) => BenchmarkRegistry::new().with_temp_root(p),
        None => BenchmarkRegistry::new(),
    };
    let report = reg.run(&suite_id)
        .ok_or_else(|| format!("suite not found: {} (use bench_list_suites)", suite_id))?;
    let fmt = match format.as_deref().unwrap_or("markdown") {
        "json" => Format::Json,
        "pretty" => Format::Pretty,
        _ => Format::Markdown,
    };
    Ok(render_report(&report, fmt))
}
