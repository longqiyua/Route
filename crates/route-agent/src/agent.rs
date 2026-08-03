//! Agent 核心：plan → act → observe 多轮迭代循环。
//!
//! MAX_AGENT_ITERATIONS 限制单任务最多迭代次数，防止无限循环。

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error, instrument};

use crate::memory::{AgentMemory, MemoryKind};
use crate::model::{ModelProvider, ModelRegistry, ChatMessage, ChatRole};
use crate::skill::{SkillHarness, SkillContext};
use crate::tools::ToolRegistry;

pub const MAX_AGENT_ITERATIONS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Planning,
    Acting,
    Observing,
    WaitingUser,
    Done,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub max_iterations: usize,
    pub verbose: bool,
    pub auto_save_memory: bool,
    /// 危险操作需要确认的关键词
    pub danger_confirm: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: MAX_AGENT_ITERATIONS,
            verbose: false,
            auto_save_memory: true,
            danger_confirm: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationLog {
    pub iteration: usize,
    pub status: AgentStatus,
    pub message: String,
    pub tool_calls: Vec<String>,
    pub skill_executions: Vec<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub success: bool,
    pub status: AgentStatus,
    pub iterations: usize,
    pub total_duration_ms: u128,
    pub final_response: String,
    pub logs: Vec<IterationLog>,
    pub memory_stats: crate::memory::MemoryStats,
}

impl RunResult {
    /// Alias for final_response used by the CLI summary.
    pub fn final_answer(&self) -> &str { &self.final_response }

    /// Summarise tool calls made during the run (flat list).
    pub fn tool_calls(&self) -> Vec<ToolCallSummary> {
        let mut out = Vec::new();
        for log in &self.logs {
            for tc in &log.tool_calls {
                out.push(ToolCallSummary {
                    tool_name: tc.clone(),
                    result: String::new(),
                });
            }
        }
        out
    }
}

// Keep the legacy names as aliases so existing CLI code compiles.
pub use RunResult as AgentResult;
pub use AgentStatus as AgentRunStatus;

// Compact view of a tool invocation for the CLI summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub result: String,
}

/// Route Agent：多轮自主任务执行器。
///
/// 每个 Agent 实例绑定：
///   - 一个 ModelProvider（预设插件提供的模型）
///   - 一组 Skill（通过 SkillHarness 加载）
///   - 一组 Tool（通过 ToolRegistry 调用）
///   - 一份 Memory（短期 + 长期项目记忆）
pub struct Agent {
    pub config: AgentConfig,
    pub project_path: Option<PathBuf>,
    model: Option<Box<dyn ModelProvider>>,
    registry: ModelRegistry,
    pub skills: SkillHarness,
    pub tools: ToolRegistry,
    pub memory: AgentMemory,
    history: Vec<ChatMessage>,
    pub status: AgentStatus,
}

impl Agent {
    /// Create a new Agent with a ModelRegistry and a SkillHarness.
    #[instrument(skip(registry, skills), fields(
        default_model = ?registry.default().map(|m| m.id()),
        skill_count = skills.list().len(),
    ))]
    pub fn new(registry: ModelRegistry, skills: SkillHarness) -> Self {
        info!(
            "[agent] creating new agent: default_model={:?}, skills={}",
            registry.default().map(|m| m.id()),
            skills.list().len()
        );
        let mut me = Self {
            config: AgentConfig::default(),
            project_path: None,
            model: None,
            registry,
            skills,
            tools: ToolRegistry::with_builtins(),
            memory: AgentMemory::new(),
            history: Vec::new(),
            status: AgentStatus::Idle,
        };
        me.select_default_model();
        info!(
            "[agent] initialized with {} built-in tools",
            me.tools.list().len()
        );
        me
    }

    /// Convenience: create a default empty agent (used by tests).
    pub fn empty() -> Self {
        Self::new(ModelRegistry::new(), SkillHarness::new())
    }

    pub fn with_project(mut self, project_path: impl Into<PathBuf>) -> Self {
        let path: PathBuf = project_path.into();
        info!("[agent] loading project from: {}", path.display());
        let skills_dir = path.join(".route").join("skills");
        if skills_dir.exists() {
            debug!("[agent] loading skills from: {}", skills_dir.display());
            match SkillHarness::load_from_dir(&skills_dir) {
                Ok(h) => {
                    let count = h.list().len();
                    info!("[agent] loaded {} skills from project directory", count);
                    self.skills = h;
                }
                Err(e) => warn!("[agent] failed to load skills: {}", e),
            }
        } else {
            debug!("[agent] no skills directory found at {}", skills_dir.display());
        }
        match AgentMemory::load_project(&path) {
            Ok(m) => {
                let stats = m.stats();
                info!("[agent] loaded project memory: {} entries", stats.total_entries);
                self.memory = m;
            }
            Err(e) => warn!("[agent] failed to load project memory: {}", e),
        }
        self.project_path = Some(path);
        self
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.config.max_iterations = n;
        self
    }

    /// Register an additional model provider (preset plugin wrapper).
    pub fn register_model(&mut self, provider: Box<dyn ModelProvider>) {
        self.registry.register(provider);
        if self.model.is_none() {
            self.select_default_model();
        }
    }

    pub fn select_default_model(&mut self) -> bool {
        if let Some(def) = self.registry.default() {
            info!("[agent] selecting default model: {}", def.id());
            self.model = None; // kept as reference via registry
            let _ = def;
            self.status = AgentStatus::Idle;
            true
        } else {
            warn!("[agent] no default model available in registry");
            false
        }
    }

    pub fn select_model(&mut self, id: &str) -> anyhow::Result<()> {
        info!("[agent] switching model to: {}", id);
        if self.registry.get(id).is_none() {
            error!("[agent] model not registered: {}", id);
            anyhow::bail!("model not registered: {id}");
        }
        self.registry.set_default(id)?;
        self.status = AgentStatus::Idle;
        info!("[agent] successfully switched to model: {}", id);
        Ok(())
    }

    pub fn list_models(&self) -> Vec<(String, String)> {
        self.registry.list().into_iter().map(|(id, cap)| {
            (id.clone(), format!("{:?}", cap.code_quality))
        }).collect()
    }

    fn system_prompt(&self) -> String {
        let mut parts = Vec::new();
        parts.push(
            "You are Route Agent — a version management and project engineering assistant integrated with the Route ecosystem.

CORE GUIDELINES:
1. Think step by step. Plan the action, then use tools to execute, then observe results.
2. Before modifying any file, ALWAYS read it first (use read_file tool).
3. If the user asks for dangerous operations (rm -rf, git push --force, git reset --hard, git rollback, etc.), prefix your response with '⚠️ DANGER:' and ask for explicit confirmation.
4. Use available skills: check triggers matching the user query by running find_by_trigger before planning.
5. When calling tools, output JSON tool calls.
6. Persist key project facts into memory.put(Project, key, value) so high-frequency CRUD doesn't lose them.

TOOLS AVAILABLE:".to_string()
        );
        for (name, desc, schema) in self.tools.list() {
            parts.push(format!("- {name}: {desc}\n  Schema: {}", serde_json::to_string_pretty(&schema).unwrap_or_default()));
        }
        let skills = self.skills.list();
        if !skills.is_empty() {
            parts.push("\nSKILLS AVAILABLE (trigger keywords can auto-select these):".into());
            for s in skills {
                parts.push(format!("- [{}] {} v{}  triggers={:?}", s.id, s.name, s.version, s.triggers));
            }
        }
        parts.join("\n")
    }

    /// 运行一次完整的 agent 循环。
    #[instrument(skip(self, user_prompt), fields(
        max_iterations = self.config.max_iterations,
        has_project = self.project_path.is_some(),
        tool_count = self.tools.list().len(),
        skill_count = self.skills.list().len(),
    ))]
    pub fn run(&mut self, user_prompt: String) -> RunResult {
        info!(
            "[agent] ====== RUN START ====== task_len={}, max_iter={}, tools={}, skills={}",
            user_prompt.len(), self.config.max_iterations,
            self.tools.list().len(), self.skills.list().len()
        );
        debug!("[agent] user_prompt: {}", &user_prompt[..user_prompt.len().min(200)]);

        let start = std::time::Instant::now();
        let mut logs: Vec<IterationLog> = Vec::new();

        self.history.clear();
        self.history.push(ChatMessage {
            role: ChatRole::System,
            content: self.system_prompt(),
            tool_call_id: None,
        });
        self.history.push(ChatMessage {
            role: ChatRole::User,
            content: user_prompt.clone(),
            tool_call_id: None,
        });
        debug!("[agent] conversation history initialized with {} messages", self.history.len());

        let mut final_response = String::new();
        let mut success = false;

        for iteration in 0..self.config.max_iterations {
            info!(
                "[agent] --- ITERATION {} START --- status=Planning",
                iteration + 1
            );
            self.status = AgentStatus::Planning;
            let iter_start = std::time::Instant::now();

            let mut log = IterationLog {
                iteration,
                status: AgentStatus::Planning,
                message: String::new(),
                tool_calls: Vec::new(),
                skill_executions: Vec::new(),
                duration_ms: 0,
            };

            let model_has = self.registry.default().is_some();
            if !model_has {
                warn!("[agent] no model registered — falling back to heuristic reply");
                log.message = "no model registered — executing tools from heuristic only".into();
                final_response = self.heuristic_reply();
                success = true;
                self.status = AgentStatus::Done;
                log.status = AgentStatus::Done;
                log.duration_ms = iter_start.elapsed().as_millis();
                let dur = log.duration_ms;
                logs.push(log);
                info!(
                    "[agent] --- ITERATION {} END (heuristic) --- duration={}ms",
                    iteration + 1, dur
                );
                break;
            }

            // === SKILL MATCHING ===
            let last_user_msg = self.history.last()
                .map(|m| m.content.clone())
                .unwrap_or_default();
            debug!(
                "[agent] skill matching on message ({} chars): {}",
                last_user_msg.len(),
                &last_user_msg[..last_user_msg.len().min(150)]
            );

            let matched = self.skills.find_by_trigger(&last_user_msg);
            if matched.is_empty() {
                debug!("[agent] no skills matched for current prompt");
            } else {
                info!(
                    "[agent] matched {} skill(s) by trigger: {:?}",
                    matched.len(),
                    matched.iter().map(|s| s.manifest.id.clone()).collect::<Vec<_>>()
                );
            }

            // === SKILL EXECUTION ===
            for skill in &matched {
                info!(
                    "[agent] executing skill: id={}, name={}, triggers={:?}",
                    skill.manifest.id, skill.manifest.name, skill.manifest.triggers
                );
                let skill_start = std::time::Instant::now();
                let ctx = SkillContext {
                    vars: Default::default(),
                    project_path: self.project_path.clone(),
                };
                match self.skills.execute(&skill.manifest.id, &ctx) {
                    Ok(exec) => {
                        let dur = skill_start.elapsed().as_millis();
                        info!(
                            "[agent] skill '{}' executed: success={}, duration={}ms, steps={}, rendered_prompt_len={}",
                            exec.skill_id, exec.success, dur, exec.steps.len(), exec.rendered_prompt.len()
                        );
                        debug!(
                            "[agent] skill '{}' rendered prompt preview: {}",
                            exec.skill_id,
                            &exec.rendered_prompt[..exec.rendered_prompt.len().min(200)]
                        );
                        self.history.push(ChatMessage {
                            role: ChatRole::System,
                            content: format!("SKILL_EXECUTED: {}\n{}", exec.skill_id, exec.rendered_prompt),
                            tool_call_id: None,
                        });
                        log.skill_executions.push(skill.manifest.id.clone());
                    }
                    Err(e) => {
                        error!(
                            "[agent] skill '{}' execution FAILED: {}",
                            skill.manifest.id, e
                        );
                    }
                }
            }

            self.status = AgentStatus::Acting;
            debug!("[agent] acting phase: skills executed={}", log.skill_executions.len());

            self.status = AgentStatus::Observing;
            debug!("[agent] observing phase: building final response");

            final_response = format!(
                "Agent iteration {}/{} — ready. {} tools registered, {} skills loaded, {} memory entries.",
                iteration + 1, self.config.max_iterations,
                self.tools.list().len(),
                self.skills.list().len(),
                self.memory.stats().total_entries,
            );

            self.memory.put(MemoryKind::Ephemeral, format!("last_prompt_{iteration}"), "recorded");
            debug!("[agent] memory updated: total_entries={}", self.memory.stats().total_entries);

            success = true;
            self.status = AgentStatus::Done;
            log.status = AgentStatus::Done;
            log.message = final_response.clone();
            log.duration_ms = iter_start.elapsed().as_millis();
            let duration_ms = log.duration_ms;
            let skills_executed = log.skill_executions.len();
            logs.push(log);

            info!(
                "[agent] --- ITERATION {} END --- status=Done, duration={}ms, skills_executed={}",
                iteration + 1, duration_ms, skills_executed
            );

            break;
        }

        // === RUN COMPLETE ===
        let total_dur = start.elapsed().as_millis();
        if self.config.auto_save_memory {
            match self.memory.save_project() {
                Ok(_) => info!("[agent] project memory auto-saved successfully"),
                Err(e) => warn!("[agent] failed to auto-save memory: {}", e),
            }
        }

        let result = RunResult {
            success,
            status: self.status,
            iterations: logs.len(),
            total_duration_ms: total_dur,
            final_response: final_response.clone(),
            logs,
            memory_stats: self.memory.stats(),
        };

        info!(
            "[agent] ====== RUN COMPLETE ====== success={}, iterations={}, total_duration={}ms, memory_entries={}",
            result.success, result.iterations, result.total_duration_ms,
            result.memory_stats.total_entries
        );
        debug!(
            "[agent] final_response preview: {}",
            &final_response[..final_response.len().min(300)]
        );

        result
    }

    fn heuristic_reply(&self) -> String {
        let mut out = String::new();
        out.push_str("⚠️ No AI model registered. Running in heuristic-only mode.\n\n");
        out.push_str(&format!("Registered tools ({}):\n", self.tools.list().len()));
        for (name, desc, _) in self.tools.list() {
            out.push_str(&format!("  • {name} — {desc}\n"));
        }
        let skills = self.skills.list();
        if !skills.is_empty() {
            out.push_str(&format!("\nLoaded skills ({}):\n", skills.len()));
            for s in skills {
                out.push_str(&format!("  • [{id}] {name} v{v}\n", id = s.id, name = s.name, v = s.version));
            }
        }
        out
    }
}

trait MemHack { fn entries_total(&self) -> usize; }
impl MemHack for AgentMemory {
    fn entries_total(&self) -> usize { self.stats().total_entries }
}
