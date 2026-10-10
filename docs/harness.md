# Harness Relationship

**Model ≠ Harness ≠ Route.**

## The Three Roles

| Role | What it does | Who implements it |
|------|--------------|-------------------|
| **Model** | The underlying LLM that produces text/tool calls | The model provider |
| **Harness** | The runtime that drives the model: tool loop, shell, sandbox, subagents | The coding-agent provider (Claude Code, Codex, DeepSeek, etc.) |
| **Route** | The persistent control/state layer: context, history, task, evidence, save, recovery, learning, governance | Route (this project) |

Route does **not** implement a harness. It never runs a model loop, a shell, a
sandbox, or a subagent runtime. The harness executes; Route records,
constrains, verifies, selects, and recovers.

Route Core is **vendor-neutral** and does not require any harness. Harness
integrations are **optional adapters**, never core dependencies.

## Operation Tiers

Route must be usable across three operation tiers. Route semantics survive
moving between them.

| Tier | Name | How Route context/state is carried |
|------|------|-----------------------------------|
| **Tier 0** | Manual AI | Route context/state carried manually by the agent. |
| **Tier 1** | Coding Harness | Claude Code / Codex / DSH / compatible systems consume Route context automatically. |
| **Tier 2** | Advanced Harness | subagents / skills / workflows / Code Mode realize dynamic AgentSpecs and campaign orchestration. |

- **Tier 0**: no harness needed — the agent reads/writes Route state directly.
- **Tier 1**: the harness reads the generated context file and the CLI/JSON
  surface.
- **Tier 2**: the harness composes atomic capabilities into temporary agents
  and drives campaigns (see [docs/dsh-pic.md](dsh-pic.md)).

## Route Steward

The **Steward** is a **responsibility, not a permanent process** — no daemon,
no required runtime. Duties:

- maintain continuity
- compile context (task-scoped views, never bulk injection)
- maintain memory
- organize agents (derive AgentSpecs from Intent/WorkGraph)
- watch evidence
- preserve saves
- supervise recovery
- learn outcomes
- prevent scope drift
- maintain Route state

Any capable AI may temporarily assume the Steward role at any tier. Because
all state lives in Shared Route State, handing the Steward role to a different
AI or harness loses nothing. Detail:
[agents.md](agents.md#permissions).

## Harness Adaptation

Concise guidance for currently known integrations. Each section covers only:
discovery, instruction delivery, available capabilities, degradation, and
state handoff. There is **no separate Route state per harness** — every harness
operates on the one shared project truth.

**Claude Code** (`claude`)
- Discovery: reads `CLAUDE.md`.
- Instruction: Route applies context to `CLAUDE.md`; managed-block markers keep
  user content intact.
- Capabilities: MCP, sub-agents, skills.
- Degradation: if sub-agents/MCP are unavailable, fall back to sequential
  single-agent work.
- Handoff: session state is read from `.route/`, so another harness can resume.

**Codex / AGENTS** (`codex`)
- Discovery: reads `AGENTS.md`.
- Instruction: Route applies context to `AGENTS.md`.
- Capabilities: no MCP/sub-agent assumed.
- Degradation: operate as a single agent; use shell/filesystem.
- Handoff: same `.route/` state as all other harnesses.

**DeepSeek Harness** (`deepseek`)
- Discovery: reads `.route/generated/deepseek-context.md`.
- Instruction: tool calls, JSON mode, reasoning.
- Capabilities: tool calls; sub-agents/MCP/skills are host-dependent.
- Degradation: if a capability is absent, degrade to the available set and
  never pretend execution was possible.
- Handoff: same `.route/` state.

**Generic** (`generic`)
- Discovery: reads `.route/generated/context.md` (portable Markdown).
- Instruction: plain Markdown context.
- Capabilities: filesystem + reasoning minimum; anything else is explicit.
- Degradation: operate at the highest tier actually available (Tier 0 if
  nothing else).
- Handoff: same `.route/` state.

## Apply Targets

Route compiles the assembled context into a host file. The generated file is
**not** the source of truth — `.route/` is.

| Target | File written | Notes |
|--------|--------------|-------|
| `claude` | `CLAUDE.md` | MCP and sub-agent capable |
| `codex` | `AGENTS.md` | No MCP/sub-agent assumed |
| `deepseek` | `.route/generated/deepseek-context.md` | Tool calls, JSON mode, reasoning; sub-agents/MCP/skills are host-dependent |
| `generic` | `.route/generated/context.md` | Portable Markdown |

```bash
route apply --target <claude|codex|deepseek|generic>
# Optionally scoped to a task:
route apply --target claude --task "fix storage"
```

## Cross-Host Handoff

Task state can be transferred between hosts. Start with one host, then apply
for another; the receiving host knows the task, current save, changes,
verification status, known-good, selected references, and remaining plan.

```bash
route task begin "fix storage" --target deepseek
route apply claude
```

All hosts share the same Save Core, recovery foundation, and learning system.

## PIC / Machine Interface

PIC-style automation can drive Route through the CLI and machine-readable JSON
(often `--json`). Examples:

```bash
route status --json
route archive list --json
route evolve campaign status <id> --json
route evolve campaign next <id> --json
route check --json          # check findings
route repair-plan --json    # repair plan from check findings
```

Use these to query Route state without injecting the whole Route status into a
prompt.

## Operability & Real Verification

AI-generated files are not "done". Route's operability protocol requires
verifying build/tests/runtime/artifact/migration/data preservation against
what the project actually requires — and recording only verifications that
**actually executed**. When the environment or tools are missing, the
honest states are `NOT_RUN` / `UNKNOWN`, never a claimed PASS. A harness
that cannot run a check must say so; pretending execution is a protocol
violation. Detail: [maintenance.md §11](maintenance.md#11-operability-real-verification-only).

## Native Harness / Cordis Bridge

A **native harness / Cordis bridge is a FUTURE plan and is NOT implemented.**
Route currently integrates with harnesses through the generated context files
and the CLI/JSON interface above.