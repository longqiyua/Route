//! Three logical curator roles that Route exposes.
//!
//! These are NOT an Agent Runtime — they are role definitions
//! that the host AI can understand and execute.

/// Memory Curator: maintains ProjectMemory proposals
pub struct MemoryCurator;

/// Reference Curator: manages external capabilities
pub struct ReferenceCurator;

/// Agent Compiler: generates AgentPlan for current task
pub struct AgentCompiler;

/// Capabilities for a single curator role.
pub struct CuratorRole {
    pub name: String,
    pub description: String,
    pub responsibilities: Vec<String>,
    pub tools: Vec<String>,
    pub constraints: Vec<String>,
}

/// All curator capabilities exposed by Route.
pub struct CuratorCapabilities {
    pub roles: Vec<CuratorRole>,
    pub available_tools: Vec<String>,
}

/// Returns the three curator roles with their descriptions, tools, and constraints.
pub fn curator_capabilities() -> CuratorCapabilities {
    CuratorCapabilities {
        roles: vec![
            CuratorRole {
                name: "Memory Curator".into(),
                description: "Maintains ProjectMemory proposals".into(),
                responsibilities: vec![
                    "review session history".into(),
                    "generate ProjectMemory proposals".into(),
                    "maintain decision log".into(),
                    "track risks".into(),
                ],
                tools: vec![
                    "route memory show".into(),
                    "route memory history".into(),
                    "route memory refresh".into(),
                    "route learn history".into(),
                ],
                constraints: vec![
                    "MUST NOT auto-apply proposals".into(),
                    "MUST bind source_ids to evidence".into(),
                    "MUST NOT modify Constitution".into(),
                ],
            },
            CuratorRole {
                name: "Reference Curator".into(),
                description: "Manages external capabilities".into(),
                responsibilities: vec![
                    "import/update references".into(),
                    "assess compatibility".into(),
                    "maintain workflow catalog".into(),
                ],
                tools: vec![
                    "route study".into(),
                    "route reference import".into(),
                    "route reference list".into(),
                    "route workflow import".into(),
                ],
                constraints: vec![
                    "MUST NOT fabricate capabilities".into(),
                    "MUST mark L1/L2/L3 compatibility".into(),
                    "MUST NOT modify project code".into(),
                ],
            },
            CuratorRole {
                name: "Agent Compiler".into(),
                description: "Generates AgentPlan for current task".into(),
                responsibilities: vec![
                    "generate AgentPlan".into(),
                    "select appropriate mode".into(),
                    "define agent specs".into(),
                ],
                tools: vec![
                    "route agent-plan".into(),
                    "route profile list".into(),
                    "route workflow list".into(),
                    "route task status".into(),
                ],
                constraints: vec![
                    "MUST NOT run LLM".into(),
                    "MUST NOT spawn agents".into(),
                    "MUST be deterministic".into(),
                ],
            },
        ],
        available_tools: vec![
            "route memory show".into(),
            "route memory history".into(),
            "route memory refresh".into(),
            "route learn history".into(),
            "route study".into(),
            "route reference import".into(),
            "route reference list".into(),
            "route workflow import".into(),
            "route agent-plan".into(),
            "route profile list".into(),
            "route workflow list".into(),
            "route task status".into(),
        ],
    }
}

impl MemoryCurator {
    /// Returns Markdown instructions for the Memory Curator role.
    pub fn instructions() -> String {
        r#"## Memory Curator

You are the Memory Curator for Route. Your responsibilities are:

- **Review session history** — examine past execution sessions to identify patterns, decisions, and outcomes.
- **Generate ProjectMemory proposals** — synthesize meaningful project memory entries from session evidence.
- **Maintain decision log** — record key decisions, their rationale, and their consequences.
- **Track risks** — identify and document risks, assumptions, and uncertainties.

### Tools

| Tool | Purpose |
|------|---------|
| `route memory show` | View current project memory |
| `route memory history` | View memory change history and proposals |
| `route memory refresh` | Generate new memory proposals from ledger/history |
| `route learn history` | Inspect learning event audit trail |

### Constraints

- MUST NOT auto-apply proposals — proposals are reviewed and applied explicitly.
- MUST bind source_ids to evidence — every proposal must reference its source evidence.
- MUST NOT modify the Constitution — the Constitution is a user-controlled document.
"#
        .to_string()
    }
}

impl ReferenceCurator {
    /// Returns Markdown instructions for the Reference Curator role.
    pub fn instructions() -> String {
        r#"## Reference Curator

You are the Reference Curator for Route. Your responsibilities are:

- **Import/update references** — register external capabilities, tools, and documentation as reference entries.
- **Assess compatibility** — evaluate compatibility levels (L1, L2, L3) for external resources.
- **Maintain workflow catalog** — keep the workflow definitions up to date and organized.

### Tools

| Tool | Purpose |
|------|---------|
| `route study` | Analyze a project and generate study candidates |
| `route reference import` | Register a reference entry from a file or URL |
| `route reference list` | List all registered references |
| `route workflow import` | Import a workflow definition |

### Constraints

- MUST NOT fabricate capabilities — only register what has been verified.
- MUST mark L1/L2/L3 compatibility — assign a compatibility level to every reference.
- MUST NOT modify project code — reference curation is about metadata, not code.
"#
        .to_string()
    }
}

impl AgentCompiler {
    /// Returns Markdown instructions for the Agent Compiler role.
    pub fn instructions() -> String {
        r#"## Agent Compiler

You are the Agent Compiler for Route. Your responsibilities are:

- **Generate AgentPlan** — produce a deterministic plan for the current task based on the profile and workflow.
- **Select appropriate mode** — choose the right execution mode (e.g., strict, normal) for the task.
- **Define agent specs** — specify the agent configuration, tools, and constraints needed.

### Tools

| Tool | Purpose |
|------|---------|
| `route agent-plan` | Generate a plan for a task description |
| `route profile list` | List available project profiles |
| `route workflow list` | List available workflow definitions |
| `route task status` | Check the status of execution sessions |

### Constraints

- MUST NOT run an LLM — the Agent Compiler is a deterministic planner, not an AI runtime.
- MUST NOT spawn agents — route does not manage agent lifecycles; it only generates plans.
- MUST be deterministic — given the same inputs, the output must always be the same.
"#
        .to_string()
    }
}

/// Renders all three curator roles into a compact context block.
pub fn render_curator_context() -> String {
    let caps = curator_capabilities();
    let mut output = String::new();

    output.push_str("<!-- Curator Roles -->\n");
    output.push_str("Route provides three logical curator roles for the host AI:\n\n");

    for role in &caps.roles {
        output.push_str(&format!("### {}\n", role.name));
        output.push_str(&format!("{}\n\n", role.description));
        output.push_str("Responsibilities:\n");
        for r in &role.responsibilities {
            output.push_str(&format!("- {}\n", r));
        }
        output.push_str("\nTools:\n");
        for t in &role.tools {
            output.push_str(&format!("- `{}`\n", t));
        }
        output.push_str("\n");
    }

    output
}
