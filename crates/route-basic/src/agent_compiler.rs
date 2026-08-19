//! Agent plan compiler — generates vendor-neutral AgentPlans from
//! AgentPolicy + task + Workflow + Memory + Reference.
//!
//! This is a pure plan compiler: it analyzes input and produces a structured
//! plan. It does NOT execute agents or LLM calls.

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::agent_org::{classify_task_pattern, OrganizationExperienceStore};
use crate::constitutive::{AgentMode, AgentPolicy, ReferenceEntry};
use crate::memory::ProjectMemory;
use crate::workflow::{WorkflowDefinition, WorkflowStep};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A vendor-neutral agent plan generated from AgentPolicy + task + Workflow +
/// Memory + Reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    /// `"single"` | `"multi"` | `"adaptive"`
    pub mode: String,
    /// Agent specifications (primary + sub-agents).
    pub agents: Vec<AgentSpec>,
    /// Unix-millis when the plan was created.
    pub created_at: i64,
    /// Stable fingerprint of the input context.
    pub context_hash: String,
    /// Serialized agent policy that was used.
    pub base_policy: String,
    /// Organization Memory hint (optional).
    /// Generated from OrganizationExperienceStore recall signals.
    pub org_memory_note: Option<String>,
}

/// Specification for a single agent in the plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    /// Role identifier, e.g. `"primary"`, `"verifier"`, `"builder"`.
    pub role: String,
    /// Natural-language goal description.
    pub goal: String,
    /// Reference IDs this agent should have access to.
    pub context_refs: Vec<String>,
    /// Tools this agent is allowed to use.
    pub tools: Vec<String>,
    /// Permissions granted to this agent.
    pub permissions: Vec<String>,
    /// Actions this agent is explicitly forbidden from performing.
    pub forbidden: Vec<String>,
    /// Criteria for considering this agent's work complete.
    pub completion_criteria: Vec<String>,
    /// Agent roles this agent depends on (must complete first).
    pub dependencies: Vec<String>,
}

/// Capabilities of a host (Claude, Codex, DeepSeek, etc.).
/// Route uses this to determine how to compile plans and what
/// features to assume available.
///
/// ## Model vs Harness
///
/// `host` names the model (e.g. `"deepseek"`, `"claude"`).
/// `profile` names the harness environment (e.g. `"dsh-standard"`, `"dsh-pic"`).
///
/// A model may run inside many harnesses; a harness may expose
/// capabilities the model supports but the environment does not.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCapabilities {
    /// Canonical host model name (e.g. "deepseek", "claude").
    pub host: String,
    /// Harness profile identifier (e.g. "generic-deepseek", "dsh-standard", "dsh-pic").
    /// Empty string means "no specific profile — use model defaults".
    #[serde(default)]
    pub profile: String,
    /// Whether the host supports tool calls.
    pub tool_calls: bool,
    /// Whether the host supports structured output (JSON mode).
    pub structured_output: bool,
    /// Whether the host supports reasoning / chain-of-thought.
    pub reasoning: bool,
    /// Whether the host supports MCP (Model Context Protocol).
    /// Use `TriBool` to distinguish unknown (model-dependent) from known.
    #[serde(default)]
    pub mcp: TriBool,
    /// Whether the host supports skills/plugins.
    /// Use `TriBool` to distinguish unknown (model-dependent) from known.
    #[serde(default)]
    pub skills: TriBool,
    /// Whether the host supports sub-agents.
    /// Use `TriBool` to distinguish unknown (model-dependent) from known.
    #[serde(default)]
    pub subagents: TriBool,
    /// Whether the host supports hooks/callbacks.
    /// Use `TriBool` to distinguish unknown (model-dependent) from known.
    #[serde(default)]
    pub hooks: TriBool,
    /// Whether the host supports shell/command execution.
    #[serde(default)]
    pub shell: TriBool,
    /// Whether the host supports filesystem read/write.
    #[serde(default)]
    pub filesystem: TriBool,
    /// Whether the host supports web/network access.
    #[serde(default)]
    pub web: TriBool,
    /// Whether the host supports code mode (programmatic tool orchestration).
    #[serde(default)]
    pub code_mode: TriBool,
    /// Whether the host supports workflow execution.
    #[serde(default)]
    pub workflows: TriBool,
    /// Optional context window size.
    pub context_window: Option<u32>,
}

/// A three-state boolean: true, false, or unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriBool {
    True,
    False,
    Unknown,
}

impl TriBool {
    pub fn is_true(self) -> bool {
        matches!(self, TriBool::True)
    }

    pub fn is_false(self) -> bool {
        matches!(self, TriBool::False)
    }

    pub fn is_unknown(self) -> bool {
        matches!(self, TriBool::Unknown)
    }
}

impl Default for TriBool {
    fn default() -> Self {
        TriBool::Unknown
    }
}

impl std::fmt::Display for TriBool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriBool::True => write!(f, "true"),
            TriBool::False => write!(f, "false"),
            TriBool::Unknown => write!(f, "unknown"),
        }
    }
}

/// Harness profile identifiers.
pub mod profiles {
    /// Generic DeepSeek model — no harness capabilities assumed.
    pub const GENERIC_DEEPSEEK: &str = "generic-deepseek";
    /// Standard DSH harness: shell, fs, skills, subagents, workflows, web.
    pub const DSH_STANDARD: &str = "dsh-standard";
    /// DSH PIC harness: inherits standard + code_mode.
    pub const DSH_PIC: &str = "dsh-pic";
}

impl HostCapabilities {
    /// Default capabilities for a known host model.
    /// These are model-level defaults — harness capabilities are
    /// expressed via `for_profile()`.
    pub fn for_host(host: &str) -> Self {
        match host.to_ascii_lowercase().as_str() {
            "claude" => Self {
                host: "claude".to_string(),
                profile: String::new(),
                tool_calls: true,
                structured_output: true,
                reasoning: true,
                mcp: TriBool::True,
                skills: TriBool::True,
                subagents: TriBool::True,
                hooks: TriBool::True,
                shell: TriBool::True,
                filesystem: TriBool::True,
                web: TriBool::True,
                code_mode: TriBool::Unknown,
                workflows: TriBool::True,
                context_window: Some(200_000),
            },
            "codex" => Self {
                host: "codex".to_string(),
                profile: String::new(),
                tool_calls: true,
                structured_output: true,
                reasoning: true,
                mcp: TriBool::False,
                skills: TriBool::False,
                subagents: TriBool::False,
                hooks: TriBool::False,
                shell: TriBool::True,
                filesystem: TriBool::True,
                web: TriBool::True,
                code_mode: TriBool::Unknown,
                workflows: TriBool::Unknown,
                context_window: Some(128_000),
            },
            "deepseek" => Self {
                host: "deepseek".to_string(),
                profile: String::new(),
                tool_calls: true,
                structured_output: true,
                reasoning: true,
                mcp: TriBool::Unknown,
                skills: TriBool::Unknown,
                subagents: TriBool::Unknown,
                hooks: TriBool::Unknown,
                shell: TriBool::Unknown,
                filesystem: TriBool::Unknown,
                web: TriBool::Unknown,
                code_mode: TriBool::Unknown,
                workflows: TriBool::Unknown,
                context_window: Some(1_000_000),
            },
            _ => Self {
                host: host.to_string(),
                profile: String::new(),
                tool_calls: false,
                structured_output: false,
                reasoning: false,
                mcp: TriBool::False,
                skills: TriBool::False,
                subagents: TriBool::False,
                hooks: TriBool::False,
                shell: TriBool::False,
                filesystem: TriBool::False,
                web: TriBool::False,
                code_mode: TriBool::False,
                workflows: TriBool::False,
                context_window: None,
            },
        }
    }

    /// Return capabilities for a known harness profile.
    /// These override model-level unknowns with realistic defaults.
    pub fn for_profile(profile: &str) -> Self {
        match profile {
            profiles::GENERIC_DEEPSEEK => Self {
                host: "deepseek".to_string(),
                profile: profiles::GENERIC_DEEPSEEK.to_string(),
                tool_calls: true,
                structured_output: true,
                reasoning: true,
                mcp: TriBool::Unknown,
                skills: TriBool::Unknown,
                subagents: TriBool::Unknown,
                hooks: TriBool::Unknown,
                shell: TriBool::Unknown,
                filesystem: TriBool::Unknown,
                web: TriBool::Unknown,
                code_mode: TriBool::Unknown,
                workflows: TriBool::Unknown,
                context_window: Some(1_000_000),
            },
            profiles::DSH_STANDARD => Self {
                host: "deepseek".to_string(),
                profile: profiles::DSH_STANDARD.to_string(),
                tool_calls: true,
                structured_output: true,
                reasoning: true,
                mcp: TriBool::False,
                skills: TriBool::True,
                subagents: TriBool::True,
                hooks: TriBool::False,
                shell: TriBool::True,
                filesystem: TriBool::True,
                web: TriBool::True,
                code_mode: TriBool::Unknown,
                workflows: TriBool::True,
                context_window: Some(1_000_000),
            },
            profiles::DSH_PIC => {
                let mut base = Self::for_profile(profiles::DSH_STANDARD);
                base.profile = profiles::DSH_PIC.to_string();
                base.code_mode = TriBool::True;
                base
            }
            _ => Self::for_host("deepseek"),
        }
    }

    /// Merge override capabilities on top of these defaults.
    /// Fields set to `true` or `false` in `override_caps` replace
    /// the corresponding fields; `Unknown` fields are left as-is.
    pub fn with_overrides(mut self, override_caps: &HostCapabilities) -> Self {
        self.tool_calls = override_caps.tool_calls;
        self.structured_output = override_caps.structured_output;
        self.reasoning = override_caps.reasoning;
        // Carry over the profile identifier from the override if set
        if !override_caps.profile.is_empty() {
            self.profile.clone_from(&override_caps.profile);
        }
        if !override_caps.mcp.is_unknown() {
            self.mcp = override_caps.mcp;
        }
        if !override_caps.skills.is_unknown() {
            self.skills = override_caps.skills;
        }
        if !override_caps.subagents.is_unknown() {
            self.subagents = override_caps.subagents;
        }
        if !override_caps.hooks.is_unknown() {
            self.hooks = override_caps.hooks;
        }
        if !override_caps.shell.is_unknown() {
            self.shell = override_caps.shell;
        }
        if !override_caps.filesystem.is_unknown() {
            self.filesystem = override_caps.filesystem;
        }
        if !override_caps.web.is_unknown() {
            self.web = override_caps.web;
        }
        if !override_caps.code_mode.is_unknown() {
            self.code_mode = override_caps.code_mode;
        }
        if !override_caps.workflows.is_unknown() {
            self.workflows = override_caps.workflows;
        }
        if override_caps.context_window.is_some() {
            self.context_window = override_caps.context_window;
        }
        self
    }

    /// True if the profile field is non-empty, indicating a known harness profile.
    pub fn has_profile(&self) -> bool {
        !self.profile.is_empty()
    }
}

// ---------------------------------------------------------------------------
// HostProfile helpers
// ---------------------------------------------------------------------------

/// Resolve effective capabilities from a host name and optional profile.
///
/// 1. Start with model-level defaults (`for_host`).
/// 2. If a profile is provided, apply profile-level defaults.
/// 3. If detected/reported capabilities are provided, merge them as overrides.
pub fn effective_capabilities(
    host: &str,
    profile: Option<&str>,
    detected: Option<&HostCapabilities>,
) -> HostCapabilities {
    let mut caps = HostCapabilities::for_host(host);

    // Apply profile defaults if provided
    if let Some(prof) = profile {
        let profile_caps = HostCapabilities::for_profile(prof);
        caps = caps.with_overrides(&profile_caps);
    }

    // Apply detected/reported capabilities as overrides
    if let Some(det) = detected {
        caps = caps.with_overrides(det);
    }

    caps
}

/// Input to the plan compiler.
pub struct CompilerInput {
    /// Task description (free-form).
    pub task: String,
    /// Optional agent policy override (JSON string).
    pub agent_policy: Option<String>,
    /// Optional workflow definition.
    pub workflow: Option<WorkflowDefinition>,
    /// Optional project memory.
    pub memory: Option<ProjectMemory>,
    /// Reference entries for context.
    pub references: Vec<ReferenceEntry>,
    /// Stable fingerprint of the current context.
    pub context_hash: String,
    /// Project root path — used to load OrganizationExperienceStore.
    /// When set, recall signals will be added as a note in the plan.
    pub project_root: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

/// Generate an AgentPlan from the given input.
///
/// Rules:
/// - Simple tasks → single agent mode, no sub-agents
/// - Complex tasks → adaptive mode with sub-agents
/// - Tasks with "test"/"check"/"verify" → add a verification agent
/// - Workflow steps → create agent roles from steps
/// - Agent_policy overrides mode
/// - Agent_policy.max_agents limits sub-agents
/// - Deterministic: same input always produces the same output
pub fn compile_plan(input: CompilerInput) -> Result<AgentPlan> {
    // 1. Parse agent policy (None = no explicit override)
    let policy_override = parse_agent_policy(input.agent_policy.as_deref());

    // 2. Determine mode
    let mode = determine_mode(&input.task, policy_override.as_ref());

    // 3. Build agents (use the override or default for constraints)
    let policy = policy_override.as_ref().cloned().unwrap_or_default();
    let agents = build_agents(&input, &policy, &mode);

    // 4. Serialize base policy
    let base_policy = serde_json::to_string(&policy).unwrap_or_else(|_| "{}".to_string());

    // 5. Check OrganizationExperienceStore for recall signals
    let org_memory_note = if let Some(ref project_root) = input.project_root {
        load_org_memory_note(project_root, &input.task)
    } else {
        None
    };

    Ok(AgentPlan {
        mode,
        agents,
        created_at: route_core::now_millis(),
        context_hash: input.context_hash,
        base_policy,
        org_memory_note,
    })
}

/// Render the plan as a YAML-like string for Claude/Codex.
pub fn render_plan(plan: &AgentPlan) -> String {
    let mut out = String::new();
    out.push_str("agent_plan:\n");
    out.push_str(&format!("  mode: {}\n", plan.mode));
    out.push_str(&format!("  created_at: {}\n", plan.created_at));
    out.push_str(&format!("  context_hash: {}\n", plan.context_hash));
    out.push_str(&format!("  base_policy: {}\n", plan.base_policy));
    if let Some(ref note) = plan.org_memory_note {
        out.push_str(&format!("  org_memory_note: {}\n", note));
    }
    out.push_str(&format!("  agents ({}):\n", plan.agents.len()));

    for agent in &plan.agents {
        out.push_str(&format!("    - role: {}\n", agent.role));
        out.push_str(&format!("      goal: {}\n", agent.goal));
        if !agent.context_refs.is_empty() {
            out.push_str(&format!(
                "      context_refs: [{}]\n",
                agent.context_refs.join(", ")
            ));
        }
        if !agent.tools.is_empty() {
            out.push_str(&format!("      tools: [{}]\n", agent.tools.join(", ")));
        }
        if !agent.permissions.is_empty() {
            out.push_str(&format!(
                "      permissions: [{}]\n",
                agent.permissions.join(", ")
            ));
        }
        if !agent.forbidden.is_empty() {
            out.push_str(&format!(
                "      forbidden: [{}]\n",
                agent.forbidden.join(", ")
            ));
        }
        if !agent.completion_criteria.is_empty() {
            out.push_str("      completion_criteria:\n");
            for c in &agent.completion_criteria {
                out.push_str(&format!("        - {}\n", c));
            }
        }
        if !agent.dependencies.is_empty() {
            out.push_str(&format!(
                "      dependencies: [{}]\n",
                agent.dependencies.join(", ")
            ));
        }
    }

    out
}

/// Compile the AgentPlan into host-specific spawn instructions.
///
/// Supported targets:
/// - `"claude"`  → Claude Code XML format
/// - `"codex"`   → OpenAI Codex markdown format
/// - `"deepseek"` → DeepSeek harness format (with optional profile)
/// - `"generic"` → YAML-like format (same as `render_plan`)
///
/// `profile` is an optional harness profile identifier
/// (e.g. `"dsh-standard"`, `"dsh-pic"`). When `None`, model-level
/// defaults are used.
pub fn compile_to_host(plan: &AgentPlan, target: &str) -> Result<String> {
    compile_to_host_with_profile(plan, target, None)
}

/// Like `compile_to_host` but accepts an optional harness profile.
pub fn compile_to_host_with_profile(
    plan: &AgentPlan,
    target: &str,
    profile: Option<&str>,
) -> Result<String> {
    match target {
        "claude" => Ok(render_claude(plan)),
        "codex" => Ok(render_codex(plan)),
        "deepseek" => Ok(render_deepseek(plan, profile)),
        "generic" | _ => Ok(render_plan(plan)),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse agent policy from an optional JSON string.
/// Returns `None` if no override was provided.
fn parse_agent_policy(override_json: Option<&str>) -> Option<AgentPolicy> {
    if let Some(json) = override_json {
        if let Ok(policy) = serde_json::from_str::<AgentPolicy>(json) {
            return Some(policy);
        }
    }
    None
}

/// Determine the agent mode based on task complexity and policy.
fn determine_mode(task: &str, policy_override: Option<&AgentPolicy>) -> String {
    // Explicit policy override takes priority
    if let Some(policy) = policy_override {
        match policy.mode {
            AgentMode::Single => return "single".to_string(),
            AgentMode::Adaptive => { /* check task complexity below */ }
        }
    }

    // Check task complexity
    let lower = task.to_lowercase();
    let complex_keywords = [
        "refactor",
        "feature",
        " add ",
        "complex",
        "multi-file",
        "large",
    ];
    let mut is_complex = complex_keywords.iter().any(|kw| lower.contains(kw));

    // Also check for "add" as a standalone word (not just " add " with spaces)
    if !is_complex {
        is_complex = lower.contains("add") && !lower.contains("adds") && !lower.contains("added");
    }

    if is_complex {
        "adaptive".to_string()
    } else {
        "single".to_string()
    }
}

/// Build the list of AgentSpecs from input.
fn build_agents(input: &CompilerInput, policy: &AgentPolicy, mode: &str) -> Vec<AgentSpec> {
    let mut agents: Vec<AgentSpec> = Vec::new();

    // Primary agent is always present
    let primary = build_primary_agent(input, policy);
    agents.push(primary);

    // In single mode, no sub-agents
    if mode == "single" {
        return agents;
    }

    // Verification agent for test/check/verify tasks
    if has_verification_keywords(&input.task) {
        agents.push(build_verification_agent(input));
    }

    // Workflow-derived agents
    if let Some(ref wf) = input.workflow {
        if let Some(ref steps) = wf.steps {
            let step_agents = build_workflow_agents(steps, input, policy);
            agents.extend(step_agents);
        }
    }

    // Apply max_agents limit
    if let Some(max) = policy.max_agents {
        let max = max as usize;
        if agents.len() > max {
            agents.truncate(max);
        }
    }

    // Deduplicate by role name (keep first occurrence)
    let mut seen = std::collections::HashSet::new();
    agents.retain(|a| seen.insert(a.role.clone()));

    agents
}

/// Build the primary agent spec.
fn build_primary_agent(input: &CompilerInput, policy: &AgentPolicy) -> AgentSpec {
    let mut tools = vec!["read".to_string(), "edit".to_string(), "search".to_string()];
    let mut permissions = vec!["read".to_string()];
    let mut forbidden = vec![
        "delete files without confirmation".to_string(),
        "modify .route/ or .route-basic/ data".to_string(),
    ];

    // Add tools from policy roles
    for role in &policy.roles {
        if role.id == "primary" || role.id == "default" {
            tools.extend(role.allowed_tools.clone());
            if !role.forbidden_actions.is_empty() {
                forbidden = role.forbidden_actions.clone();
            }
        }
    }

    // Collect reference IDs
    let context_refs: Vec<String> = input.references.iter().map(|r| r.id.clone()).collect();

    // Collect completion criteria from policy or defaults
    let completion_criteria = get_completion_criteria(
        policy,
        "primary",
        vec![
            "task description is fully implemented".to_string(),
            "all tests pass".to_string(),
            "no regressions introduced".to_string(),
        ],
    );

    // Collect permissions from policy roles
    if let Some(role) = policy
        .roles
        .iter()
        .find(|r| r.id == "primary" || r.id == "default")
    {
        if !role.allowed_tools.is_empty() {
            permissions = role.allowed_tools.clone();
        }
    }

    AgentSpec {
        role: "primary".to_string(),
        goal: input.task.clone(),
        context_refs,
        tools,
        permissions,
        forbidden,
        completion_criteria,
        dependencies: Vec::new(),
    }
}

/// Build a verification agent spec.
fn build_verification_agent(input: &CompilerInput) -> AgentSpec {
    let context_refs: Vec<String> = input.references.iter().map(|r| r.id.clone()).collect();

    AgentSpec {
        role: "verifier".to_string(),
        goal: format!("Verify: {}", input.task),
        context_refs,
        tools: vec!["read".to_string(), "diff".to_string(), "test".to_string()],
        permissions: vec!["read".to_string()],
        forbidden: vec![
            "modify source files".to_string(),
            "delete files".to_string(),
        ],
        completion_criteria: vec![
            "all tests pass".to_string(),
            "no regressions found".to_string(),
            "verification report generated".to_string(),
        ],
        dependencies: vec!["primary".to_string()],
    }
}

/// Build agent specs from workflow steps.
fn build_workflow_agents(
    steps: &[WorkflowStep],
    _input: &CompilerInput,
    policy: &AgentPolicy,
) -> Vec<AgentSpec> {
    let mut agents: Vec<AgentSpec> = Vec::new();
    let mut prev_role: Option<String> = None;

    for (i, step) in steps.iter().enumerate() {
        let role = slugify_role(&step.name);
        let goal = if step.description.is_empty() {
            format!("Step {}: {}", i + 1, step.name)
        } else {
            format!("{}: {}", step.name, step.description)
        };

        let context_refs: Vec<String> = step
            .references
            .iter()
            .chain(step.skills.iter())
            .cloned()
            .collect();

        let completion_criteria = get_completion_criteria(
            policy,
            &role,
            vec![
                format!("complete step: {}", step.name),
                if let Some(ref outcome) = step.expected_outcome {
                    outcome.clone()
                } else {
                    format!("step '{}' finished successfully", step.name)
                },
            ],
        );

        let mut deps = Vec::new();
        if let Some(ref prev) = prev_role {
            deps.push(prev.clone());
        }

        agents.push(AgentSpec {
            role: role.clone(),
            goal,
            context_refs,
            tools: vec![
                "read".to_string(),
                "edit".to_string(),
                "execute".to_string(),
            ],
            permissions: vec!["read".to_string(), "write".to_string()],
            forbidden: vec![
                "modify .route/ or .route-basic/ data".to_string(),
                "delete files without confirmation".to_string(),
            ],
            completion_criteria,
            dependencies: deps,
        });

        prev_role = Some(role);
    }

    agents
}

/// Check if the task contains verification-related keywords.
fn has_verification_keywords(task: &str) -> bool {
    let lower = task.to_lowercase();
    let keywords = ["test", "check", "verify", "validate", "audit"];
    keywords.iter().any(|kw| {
        lower.contains(kw)
            || lower.contains(&format!("{}ing", kw))
            || lower.contains(&format!("{}ed", kw))
            || lower.contains(&format!("{}s", kw))
    })
}

/// Get completion criteria for a role from policy or fallback.
fn get_completion_criteria(
    policy: &AgentPolicy,
    role_id: &str,
    fallback: Vec<String>,
) -> Vec<String> {
    for role in &policy.roles {
        if role.id == role_id {
            if let Some(ref criteria) = role.completion_criteria {
                return vec![criteria.clone()];
            }
            return fallback;
        }
    }
    fallback
}

/// Convert a human-readable name into a slug suitable for a role id.
fn slugify_role(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

// ---------------------------------------------------------------------------
// Host-specific renderers
// ---------------------------------------------------------------------------

/// Render the plan for Claude Code (XML format).
fn render_claude(plan: &AgentPlan) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<agentPlan>\n");
    out.push_str(&format!("  <mode>{}</mode>\n", plan.mode));
    out.push_str(&format!(
        "  <contextHash>{}</contextHash>\n",
        plan.context_hash
    ));
    out.push_str(&format!("  <agents>\n"));

    for agent in &plan.agents {
        out.push_str(&format!("    <agent role=\"{}\">\n", agent.role));
        out.push_str(&format!("      <goal>{}</goal>\n", escape_xml(&agent.goal)));
        if !agent.context_refs.is_empty() {
            out.push_str(&format!(
                "      <contextRefs>{}</contextRefs>\n",
                agent.context_refs.join(",")
            ));
        }
        if !agent.tools.is_empty() {
            out.push_str(&format!("      <tools>{}</tools>\n", agent.tools.join(",")));
        }
        if !agent.forbidden.is_empty() {
            out.push_str("      <forbidden>\n");
            for f in &agent.forbidden {
                out.push_str(&format!("        <action>{}</action>\n", escape_xml(f)));
            }
            out.push_str("      </forbidden>\n");
        }
        if !agent.completion_criteria.is_empty() {
            out.push_str("      <completionCriteria>\n");
            for c in &agent.completion_criteria {
                out.push_str(&format!(
                    "        <criterion>{}</criterion>\n",
                    escape_xml(c)
                ));
            }
            out.push_str("      </completionCriteria>\n");
        }
        if !agent.dependencies.is_empty() {
            out.push_str(&format!(
                "      <dependencies>{}</dependencies>\n",
                agent.dependencies.join(",")
            ));
        }
        out.push_str(&format!("    </agent>\n"));
    }

    out.push_str("  </agents>\n");
    out.push_str("</agentPlan>\n");
    out
}

/// Render the plan for OpenAI Codex (markdown format).
fn render_codex(plan: &AgentPlan) -> String {
    let mut out = String::new();
    out.push_str("# Agent Plan\n\n");
    out.push_str(&format!("- **Mode**: {}\n", plan.mode));
    out.push_str(&format!("- **Context Hash**: `{}`\n", plan.context_hash));
    out.push_str(&format!("- **Created At**: `{}`\n", plan.created_at));
    out.push_str("\n## Agents\n\n");

    for agent in &plan.agents {
        out.push_str(&format!("### {}\n\n", agent.role));
        out.push_str(&format!("**Goal**: {}\n\n", agent.goal));

        if !agent.context_refs.is_empty() {
            out.push_str("**Context References**:\n");
            for r in &agent.context_refs {
                out.push_str(&format!("- `{}`\n", r));
            }
            out.push_str("\n");
        }

        if !agent.tools.is_empty() {
            out.push_str(&format!("**Tools**: `{}`\n\n", agent.tools.join("`, `")));
        }

        if !agent.forbidden.is_empty() {
            out.push_str("**Forbidden**:\n");
            for f in &agent.forbidden {
                out.push_str(&format!("- {}\n", f));
            }
            out.push_str("\n");
        }

        if !agent.completion_criteria.is_empty() {
            out.push_str("**Completion Criteria**:\n");
            for c in &agent.completion_criteria {
                out.push_str(&format!("- {}\n", c));
            }
            out.push_str("\n");
        }

        if !agent.dependencies.is_empty() {
            out.push_str(&format!(
                "**Dependencies**: `{}`\n\n",
                agent.dependencies.join("`, `")
            ));
        }
    }

    out
}

/// Render AgentPlan for DeepSeek-compatible harness.
///
/// `profile` is an optional harness profile identifier:
/// - `None` or `"generic-deepseek"` → model-level defaults (unknown capabilities)
/// - `"dsh-standard"` → standard DSH harness (skills, subagents, shell, web)
/// - `"dsh-pic"` → PIC harness (standard + code_mode)
fn render_deepseek(plan: &AgentPlan, profile: Option<&str>) -> String {
    let caps = effective_capabilities("deepseek", profile, None);

    let mut out = String::new();
    out.push_str("agent_plan:\n");
    out.push_str(&format!("  mode: {}\n", plan.mode));
    out.push_str(&format!("  created_at: {}\n", plan.created_at));
    out.push_str(&format!("  context_hash: {}\n", plan.context_hash));
    out.push_str(&format!("  base_policy: {}\n", plan.base_policy));
    if let Some(ref note) = plan.org_memory_note {
        out.push_str(&format!("  org_memory_note: {}\n", note));
    }
    out.push_str("  host:\n");
    out.push_str(&format!("    model: {}\n", caps.host));
    out.push_str(&format!("    profile: {}\n", if caps.has_profile() { caps.profile.as_str() } else { "default" }));
    out.push_str("  host_capabilities:\n");
    out.push_str(&format!("    tool_calls: {}\n", caps.tool_calls));
    out.push_str(&format!("    structured_output: {}\n", caps.structured_output));
    out.push_str(&format!("    reasoning: {}\n", caps.reasoning));
    out.push_str(&format!("    mcp: {}\n", caps.mcp));
    out.push_str(&format!("    subagents: {}\n", caps.subagents));
    out.push_str(&format!("    skills: {}\n", caps.skills));
    out.push_str(&format!("    hooks: {}\n", caps.hooks));
    out.push_str(&format!("    shell: {}\n", caps.shell));
    out.push_str(&format!("    filesystem: {}\n", caps.filesystem));
    out.push_str(&format!("    web: {}\n", caps.web));
    out.push_str(&format!("    code_mode: {}\n", caps.code_mode));
    out.push_str(&format!("    workflows: {}\n", caps.workflows));
    if let Some(win) = caps.context_window {
        out.push_str(&format!("    context_window: {}\n", win));
    }
    out.push_str(&format!("  agents ({}):\n", plan.agents.len()));

    for agent in &plan.agents {
        out.push_str(&format!("    - role: {}\n", agent.role));
        out.push_str(&format!("      goal: {}\n", agent.goal));
        if !agent.context_refs.is_empty() {
            out.push_str(&format!("      context_refs: [{}]\n", agent.context_refs.join(", ")));
        }
        if !agent.tools.is_empty() {
            out.push_str(&format!("      tools: [{}]\n", agent.tools.join(", ")));
        }
        if !agent.permissions.is_empty() {
            out.push_str(&format!("      permissions: [{}]\n", agent.permissions.join(", ")));
        }
        if !agent.forbidden.is_empty() {
            out.push_str(&format!("      forbidden: [{}]\n", agent.forbidden.join(", ")));
        }
        if !agent.completion_criteria.is_empty() {
            out.push_str("      completion_criteria:\n");
            for c in &agent.completion_criteria {
                out.push_str(&format!("        - {}\n", c));
            }
        }
        if !agent.dependencies.is_empty() {
            out.push_str(&format!("      dependencies: [{}]\n", agent.dependencies.join(", ")));
        }
    }
    out
}

/// Escape XML special characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Load the organization memory note from the OrganizationExperienceStore.
///
/// Returns `None` if the store cannot be loaded or if there are no matching
/// experiences. The note is composed of all recall signals for the task's
/// pattern, one per line.
fn load_org_memory_note(project_root: &std::path::Path, task: &str) -> Option<String> {
    let store = OrganizationExperienceStore::load(project_root).ok()?;
    let pattern = classify_task_pattern(task);
    let signals = store.recall(&pattern);
    if signals.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = signals
        .iter()
        .map(|s| {
            format!(
                "Organization Memory hint: {} has {:.2} reliability from {} similar tasks",
                s.role,
                s.reliability,
                s.evidence.len()
            )
        })
        .collect();
    lines.sort();
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constitutive::Origin;
    use crate::workflow::WorkflowStep;

    fn make_input(task: &str) -> CompilerInput {
        CompilerInput {
            task: task.to_string(),
            agent_policy: None,
            workflow: None,
            memory: None,
            references: vec![],
            context_hash: "abc123".to_string(),
            project_root: None,
        }
    }

    #[test]
    fn test_simple_task_single_mode() {
        let input = make_input("fix typo in README");
        let plan = compile_plan(input).unwrap();
        assert_eq!(plan.mode, "single");
        assert_eq!(plan.agents.len(), 1);
        assert_eq!(plan.agents[0].role, "primary");
    }

    #[test]
    fn test_complex_task_adaptive_mode() {
        let input = make_input("refactor the authentication module");
        let plan = compile_plan(input).unwrap();
        assert_eq!(plan.mode, "adaptive");
        assert_eq!(plan.agents[0].role, "primary");
    }

    #[test]
    fn test_verification_agent_added() {
        let input = make_input("add tests for the parser");
        let plan = compile_plan(input).unwrap();
        // Should be "adaptive" because "add" is a complex keyword
        assert_eq!(plan.mode, "adaptive");
        // Should have a verifier agent
        let has_verifier = plan.agents.iter().any(|a| a.role == "verifier");
        assert!(
            has_verifier,
            "expected a verifier agent for task with 'test'"
        );
    }

    #[test]
    fn test_workflow_agents() {
        let wf = WorkflowDefinition {
            id: "wf-test".to_string(),
            name: "Test Workflow".to_string(),
            source: String::new(),
            description: "A test workflow".to_string(),
            steps: Some(vec![
                WorkflowStep {
                    name: "Build".to_string(),
                    description: "Build the project".to_string(),
                    references: vec!["ref-build".to_string()],
                    skills: vec![],
                    command: None,
                    expected_outcome: Some("build succeeds".to_string()),
                },
                WorkflowStep {
                    name: "Test".to_string(),
                    description: "Run tests".to_string(),
                    references: vec![],
                    skills: vec![],
                    command: None,
                    expected_outcome: Some("all tests pass".to_string()),
                },
            ]),
            skills: vec![],
            references: vec![],
            agent_policy: None,
            verification_policy: None,
            enabled: true,
            created_at: 0,
            origin: Origin::UserCreated,
            tags: vec![],
            upstream: None,
            base_version: 0,
            current_revision: 0,
        };

        let input = CompilerInput {
            task: "refactor core module".to_string(),
            agent_policy: None,
            workflow: Some(wf),
            memory: None,
            references: vec![],
            context_hash: "def456".to_string(),
            project_root: None,
        };

        let plan = compile_plan(input).unwrap();
        assert_eq!(plan.mode, "adaptive");
        // Should have primary + build + test agents
        assert!(plan.agents.len() >= 3);
        let has_build = plan.agents.iter().any(|a| a.role == "build");
        let has_test = plan.agents.iter().any(|a| a.role == "test");
        assert!(has_build, "expected a 'build' agent from workflow step");
        assert!(has_test, "expected a 'test' agent from workflow step");
    }

    #[test]
    fn test_policy_override_single() {
        let policy = AgentPolicy {
            mode: AgentMode::Single,
            max_agents: None,
            spawn_when: vec![],
            do_not_spawn_when: vec![],
            roles: vec![],
        };
        let policy_json = serde_json::to_string(&policy).unwrap();

        let input = CompilerInput {
            task: "refactor the whole project".to_string(),
            agent_policy: Some(policy_json),
            workflow: None,
            memory: None,
            references: vec![],
            context_hash: "ghi789".to_string(),
            project_root: None,
        };

        let plan = compile_plan(input).unwrap();
        assert_eq!(
            plan.mode, "single",
            "policy override should force single mode"
        );
        assert_eq!(plan.agents.len(), 1);
    }

    #[test]
    fn test_max_agents_limit() {
        let policy = AgentPolicy {
            mode: AgentMode::Adaptive,
            max_agents: Some(2),
            spawn_when: vec![],
            do_not_spawn_when: vec![],
            roles: vec![],
        };
        let policy_json = serde_json::to_string(&policy).unwrap();

        let wf = WorkflowDefinition {
            id: "wf-test".to_string(),
            name: "Test Workflow".to_string(),
            source: String::new(),
            description: String::new(),
            steps: Some(vec![
                WorkflowStep {
                    name: "Step A".to_string(),
                    description: String::new(),
                    references: vec![],
                    skills: vec![],
                    command: None,
                    expected_outcome: None,
                },
                WorkflowStep {
                    name: "Step B".to_string(),
                    description: String::new(),
                    references: vec![],
                    skills: vec![],
                    command: None,
                    expected_outcome: None,
                },
                WorkflowStep {
                    name: "Step C".to_string(),
                    description: String::new(),
                    references: vec![],
                    skills: vec![],
                    command: None,
                    expected_outcome: None,
                },
            ]),
            skills: vec![],
            references: vec![],
            agent_policy: None,
            verification_policy: None,
            enabled: true,
            created_at: 0,
            origin: Origin::UserCreated,
            tags: vec![],
            upstream: None,
            base_version: 0,
            current_revision: 0,
        };

        let input = CompilerInput {
            task: "complex feature".to_string(),
            agent_policy: Some(policy_json),
            workflow: Some(wf),
            memory: None,
            references: vec![],
            context_hash: "jkl012".to_string(),
            project_root: None,
        };

        let plan = compile_plan(input).unwrap();
        // max_agents=2 limits to 2 agents (primary + 1 step agent)
        assert!(plan.agents.len() <= 2, "max_agents=2 should limit agents");
    }

    #[test]
    fn test_deterministic_output() {
        let input = make_input("fix typo in README");
        let plan1 = compile_plan(CompilerInput {
            context_hash: "same".to_string(),
            ..input
        })
        .unwrap();
        let plan2 = compile_plan(CompilerInput {
            context_hash: "same".to_string(),
            ..make_input("fix typo in README")
        })
        .unwrap();

        assert_eq!(plan1.mode, plan2.mode);
        assert_eq!(plan1.agents.len(), plan2.agents.len());
        assert_eq!(plan1.agents[0].role, plan2.agents[0].role);
    }

    #[test]
    fn test_render_plan_yaml() {
        let plan = AgentPlan {
            mode: "single".to_string(),
            agents: vec![AgentSpec {
                role: "primary".to_string(),
                goal: "fix typo".to_string(),
                context_refs: vec!["doc-readme".to_string()],
                tools: vec!["read".to_string(), "edit".to_string()],
                permissions: vec!["read".to_string()],
                forbidden: vec!["delete files".to_string()],
                completion_criteria: vec!["typo fixed".to_string()],
                dependencies: vec![],
            }],
            created_at: 1000,
            context_hash: "abc".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        let rendered = render_plan(&plan);
        assert!(rendered.contains("mode: single"));
        assert!(rendered.contains("role: primary"));
        assert!(rendered.contains("typo fixed"));
    }

    #[test]
    fn test_compile_to_host_claude() {
        let plan = AgentPlan {
            mode: "single".to_string(),
            agents: vec![AgentSpec {
                role: "primary".to_string(),
                goal: "fix typo".to_string(),
                context_refs: vec![],
                tools: vec![],
                permissions: vec![],
                forbidden: vec![],
                completion_criteria: vec![],
                dependencies: vec![],
            }],
            created_at: 1000,
            context_hash: "abc".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        let result = compile_to_host(&plan, "claude").unwrap();
        assert!(result.contains("<agentPlan>"));
        assert!(result.contains("<mode>single</mode>"));
        assert!(result.contains("</agentPlan>"));
    }

    #[test]
    fn test_compile_to_host_codex() {
        let plan = AgentPlan {
            mode: "adaptive".to_string(),
            agents: vec![
                AgentSpec {
                    role: "primary".to_string(),
                    goal: "refactor".to_string(),
                    context_refs: vec![],
                    tools: vec![],
                    permissions: vec![],
                    forbidden: vec![],
                    completion_criteria: vec![],
                    dependencies: vec![],
                },
                AgentSpec {
                    role: "verifier".to_string(),
                    goal: "verify".to_string(),
                    context_refs: vec![],
                    tools: vec![],
                    permissions: vec![],
                    forbidden: vec![],
                    completion_criteria: vec![],
                    dependencies: vec!["primary".to_string()],
                },
            ],
            created_at: 1000,
            context_hash: "abc".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        let result = compile_to_host(&plan, "codex").unwrap();
        assert!(result.contains("# Agent Plan"));
        assert!(result.contains("## Agents"));
        assert!(result.contains("### primary"));
        assert!(result.contains("### verifier"));
    }

    #[test]
    fn test_compile_to_host_generic() {
        let plan = AgentPlan {
            mode: "single".to_string(),
            agents: vec![],
            created_at: 1000,
            context_hash: "abc".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        let result = compile_to_host(&plan, "generic").unwrap();
        assert!(result.contains("agent_plan:"));
        assert!(result.contains("mode: single"));
    }

    #[test]
    fn host_capabilities_deepseek_defaults() {
        let caps = HostCapabilities::for_host("deepseek");
        assert_eq!(caps.host, "deepseek");
        assert!(caps.tool_calls);
        assert!(caps.structured_output);
        assert!(caps.reasoning);
        assert!(caps.mcp.is_unknown());    // host-dependent, not assumed
        assert!(caps.skills.is_unknown()); // host-dependent, not assumed
        assert!(caps.subagents.is_unknown()); // host-dependent, not assumed
        assert!(caps.hooks.is_unknown());  // host-dependent, not assumed
        assert_eq!(caps.context_window, Some(1_000_000));
    }

    #[test]
    fn host_capabilities_dsh_standard() {
        let caps = HostCapabilities::for_profile("dsh-standard");
        assert_eq!(caps.profile, "dsh-standard");
        assert!(caps.skills.is_true());
        assert!(caps.subagents.is_true());
        assert!(caps.shell.is_true());
        assert!(caps.web.is_true());
        assert!(caps.workflows.is_true());
        assert!(caps.code_mode.is_unknown());
    }

    #[test]
    fn host_capabilities_dsh_pic() {
        let caps = HostCapabilities::for_profile("dsh-pic");
        assert_eq!(caps.profile, "dsh-pic");
        assert!(caps.skills.is_true());
        assert!(caps.subagents.is_true());
        assert!(caps.code_mode.is_true());
    }

    #[test]
    fn effective_capabilities_overrides_unknown() {
        // Default deepseek: subagents=unknown
        let default = HostCapabilities::for_host("deepseek");
        assert!(default.subagents.is_unknown());

        // Apply dsh-standard profile: subagents=true
        let effective = effective_capabilities("deepseek", Some("dsh-standard"), None);
        assert!(effective.subagents.is_true());
        assert_eq!(effective.profile, "dsh-standard");
    }

    #[test]
    fn effective_capabilities_detected_overrides_profile() {
        // Profile says subagents=true, but detected says false
        let detected = HostCapabilities {
            subagents: TriBool::False,
            ..HostCapabilities::for_host("deepseek")
        };
        let effective = effective_capabilities("deepseek", Some("dsh-standard"), Some(&detected));
        assert!(effective.subagents.is_false(), "detected override should win");
    }

    #[test]
    fn host_capabilities_claude_defaults() {
        let caps = HostCapabilities::for_host("claude");
        assert_eq!(caps.host, "claude");
        assert!(caps.tool_calls);
        assert!(caps.mcp.is_true());
        assert!(caps.subagents.is_true());
        assert!(caps.skills.is_true());
    }

    #[test]
    fn host_capabilities_unknown_defaults() {
        let caps = HostCapabilities::for_host("unknown-host");
        assert_eq!(caps.host, "unknown-host");
        assert!(!caps.tool_calls);
        assert!(caps.mcp.is_false());
        assert!(caps.subagents.is_false());
        assert!(caps.context_window.is_none());
    }

    #[test]
    fn deepseek_compile_to_host() {
        let plan = AgentPlan {
            mode: "single".to_string(),
            agents: vec![
                AgentSpec {
                    role: "primary".to_string(),
                    goal: "fix the bug".to_string(),
                    context_refs: vec!["api-docs".to_string()],
                    tools: vec!["read".to_string(), "edit".to_string()],
                    permissions: vec!["read".to_string()],
                    forbidden: vec!["delete".to_string()],
                    completion_criteria: vec!["tests pass".to_string()],
                    dependencies: vec![],
                },
            ],
            created_at: 1000,
            context_hash: "abc123".to_string(),
            base_policy: "{}".to_string(),
            org_memory_note: None,
        };

        let result = compile_to_host(&plan, "deepseek").unwrap();
        assert!(result.contains("agent_plan:"));
        assert!(result.contains("mode: single"));
        assert!(result.contains("host_capabilities:"));
        assert!(result.contains("tool_calls: true"));
        assert!(result.contains("subagents: unknown"));
        assert!(result.contains("role: primary"));
        assert!(result.contains("goal: fix the bug"));
        assert!(result.contains("context_hash: abc123"));
    }

    #[test]
    fn deepseek_agent_plan_matches_claude_core_structure() {
        let plan = AgentPlan {
            mode: "adaptive".to_string(),
            agents: vec![
                AgentSpec {
                    role: "primary".to_string(),
                    goal: "refactor database".to_string(),
                    context_refs: vec![],
                    tools: vec!["read".to_string(), "edit".to_string()],
                    permissions: vec!["read".to_string()],
                    forbidden: vec![],
                    completion_criteria: vec!["done".to_string()],
                    dependencies: vec![],
                },
                AgentSpec {
                    role: "verifier".to_string(),
                    goal: "verify refactor".to_string(),
                    context_refs: vec![],
                    tools: vec!["test".to_string()],
                    permissions: vec!["read".to_string()],
                    forbidden: vec!["edit".to_string()],
                    completion_criteria: vec!["tests pass".to_string()],
                    dependencies: vec!["primary".to_string()],
                },
            ],
            created_at: 2000,
            context_hash: "def456".to_string(),
            base_policy: r#"{"mode":"adaptive"}"#.to_string(),
            org_memory_note: Some("previous similar task: db-refactor".to_string()),
        };

        // Claude and DeepSeek should share the same core agent structure
        let claude = compile_to_host(&plan, "claude").unwrap();
        let deepseek = compile_to_host(&plan, "deepseek").unwrap();

        // Both should have the same agents
        assert!(claude.contains("role: primary") || claude.contains("primary"));
        assert!(deepseek.contains("role: primary"));
        assert!(claude.contains("role: verifier") || claude.contains("verifier"));
        assert!(deepseek.contains("role: verifier"));

        // DeepSeek should include host_capabilities
        assert!(deepseek.contains("host_capabilities:"));
    }

    #[test]
    fn deepseek_compile_to_host_from_plan() {
        // Test that the plan -> host pipeline works
        let input = CompilerInput {
            task: "fix login bug".to_string(),
            agent_policy: None,
            workflow: None,
            memory: None,
            references: vec![],
            context_hash: "test-hash".to_string(),
            project_root: None,
        };

        let plan = compile_plan(input).unwrap();
        let result = compile_to_host(&plan, "deepseek").unwrap();
        assert!(result.contains("agent_plan:"));
        assert!(result.contains("host_capabilities:"));
        assert!(result.contains("tool_calls: true"));
    }
}
