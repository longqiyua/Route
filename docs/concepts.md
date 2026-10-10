# Concepts

This document explains Route's core concepts. It is the conceptual companion
to [architecture.md](architecture.md) (structure) and [cli.md](cli.md)
(commands). Everything described here is implemented; anything marked
**experimental** or **partial** is explicitly labeled.

## Constitution / Protocol / Reference

Route governs how an agent works in a project with a three-layer context
system.

### Constitution

The **Constitution** holds stable, immutable development principles. An agent
must read it before making changes. It is not meant to change often and is
treated as a stable anchor. Example invariants: data safety, user control,
reversibility.

- CLI: `route constitution show|init|path`

### Protocol

The **Protocol** is the versioned execution playbook. It describes how to run
tasks (start, exec, verify, end), how to verify, and what review gates apply.
It can be revised explicitly, and changes are versioned.

- CLI: `route protocol show|init|path`

### Reference

The **Reference** registry describes external resources — CLI tools,
repositories, documents, skills, workflows — that can be injected into agent
context on demand. References have a type and a source, and can be enabled or
disabled.

- CLI: `route reference list|show|add|remove|import|enable|disable|review|apply`

## Handoff between the three layers

```
Constitution (stable principles)
   ↓
Protocol (versioned execution rules)
   ↓
Reference registry (external resource descriptions)
   ↓
Task context → applied to a host file (route apply)
```

## Intent (protocol, V0.8)

An **Intent** is the minimal user input: an `objective` (WHAT must become
true) plus optional `method_hint` (HOW), `hard_constraints`, and
`acceptance`. Route derives the rest — WorkGraph, organization, verification —
from Constitution, Memory, Reference, Strategy, and Failure knowledge. A
stated `method_hint` must not be casually replaced by a "better" one.

Status: **protocol-defined** ([agents.md](agents.md#minimal-intent)). The
Reference Engine covers the analytical part (`route plan` /
`route agent-plan`); there is no `route intent` command.

## AgentSpec (protocol, V0.8)

An **AgentSpec** is a vendor-neutral description of a *temporary* agent
composition: role, goal, required atomic capabilities, scoped/excluded
context, permissions, expected outputs, evidence requirements, lifetime. There
are **no permanent hardcoded agents**; agents are compositions that may die,
while evidence and memory persist (**AGENTS MAY DIE. ROUTE REMEMBERS.**).

Status: **protocol-defined**; engine surface is `route agent-plan`
(deterministic compiler, no execution). See [agents.md](agents.md#agentspec).

## Memory Layers (protocol, V0.8)

- **Project Memory** — what this project must remember long-term (implemented:
  `route memory`).
- **Architecture Memory** — evolvable structural knowledge (partial: fields in
  Project Memory + knowledge map).
- **User Memory** — stable cross-project user preferences (**protocol only,
  not yet implemented**).

Scopes promote upward only: `TASK → PROJECT → USER → ROUTE_SHARED`. Context is
**compiled** per task/agent, never bulk-injected. See
[memory.md](memory.md).

## Route Steward (protocol, V0.8)

The **Steward** is a responsibility, not a process: continuity, context
compilation, memory, agent organization, evidence watching, saves, recovery,
learning, scope-drift prevention. Any capable AI may temporarily assume it.

## Task

A **task** is a unit of work you want an agent to do. It is a declared intent
(e.g. "add login page"). Route records the task, its scope, and its outcome.

## Session

A **session** is the concrete execution of a task under a target host. It has
a lifecycle and a status:

- Lifecycle: `begin → exec → verify → end`
- Status: `Active | Succeeded | Failed | RolledBack | Aborted |
  ForcedUnverified`

Only one session may be active at a time (unless `--concurrent`), preserving a
clean causal chain.

- CLI: `route task begin|exec|verify|end|show|start|resume|report|replay`

## Evidence

**Evidence** is anything recorded during a session. Evidence is categorized by
kind and source:

- **System evidence** — trusted test pass, commit, check pass. Only system
  evidence is accepted for verification.
- **Agent evidence** — AI self-reports (`AgentFeedback`). Recorded but never
  treated as proof of success.

The causal chain is: Task → Context → Agent Plan → Execution Evidence →
Result → Learning. Consistent lineage is required.

## Save

A **save** is a snapshot of project state. Two kinds exist:

- **Archive save** (`route archive save`) — stored outside the project
  directory in `Documents/Route/`, enabling recovery after deletion.
- **Development savepoint** (`route save`) — stored inside `.route/`, like a
  game save for the current project state.

## Original

The **Original** is the immutable archive save created at `route init`. It is
the baseline for recovery and is never overwritten.

## KnownGood

**KnownGood** is the last verified, promoted configuration. It is used by
evolution, recovery, and rollback as the safe fallback. It is a trust root
that candidates cannot modify.

## Candidate

A **candidate** is a proposed change in the evolution loop. Candidates are
evaluated against a benchmark suite and compared to the baseline. A candidate
can never promote itself; promotion requires the Engine gate.

## Operational Lifecycle (protocol, V0.8)

**ROUTE IS NOT ONLY FOR CREATION. ROUTE IS FOR CONTINUITY.** Route carries a
project through understand / repair / refactor / restructure / maintain /
migrate / deprecate / retire / recover — not only greenfield builds. Core
protocol concepts:

- **Change taxonomy** — PATCH (minimal local correction) · REPAIR (restore
  intended behavior) · REFACTOR (same contract, new internals) ·
  RESTRUCTURE (new boundaries) · REWRITE (replace implementation) · MIGRATE
  (move, preserving continuity). No automatic escalation; minimal sufficient
  intervention.
- **ProblemModel** — the reconstructed problem record; symptoms ≠ causes,
  suspected ≠ verified.
- **DebtRecord** — maintenance memory of why weird code / workarounds /
  accepted debt exist.
- **Consequence levels** — L0 read → L4 external/irreversible, gating
  permission/checkpoint/evidence.

Status: **protocol-defined** ([maintenance.md](maintenance.md)); engine
covers fragments (`guardian` / `maintain` / `repair-plan` — Beta).

## Governance (protocol, V0.8)

Responsibility is split: Route upholds protocol semantics, honest
uncertainty, provenance, and recovery; the operator/user authorizes important
operations and owns production/legal/compliance risk. "Official Route"
semantics use the tiers **ROUTE OFFICIAL / COMPATIBLE / DERIVATIVE / INSPIRED**
— LICENSE, canonical specification, and brand are separate things. The
disclaimer/compliance notice lives once in
[governance.md](governance.md).

## Glossary of related terms

| Term | Meaning |
|------|---------|
| **task** | A unit of work (declared intent) |
| **session** | Concrete execution of a task under a host |
| **evidence** | A recorded fact during a session |
| **save** | A snapshot of project state |
| **original** | The immutable baseline archive save |
| **known-good** | The last verified, promoted configuration |
| **candidate** | A proposed change in the evolution loop |
| **problem model** | Reconstructed problem record (symptoms ≠ causes) |
| **debt record** | Maintenance memory of accepted debt / workarounds |
| **consequence level** | L0–L4 action risk classification |

See the [Glossary](../ROUTE.md#23-glossary) for the full terminology reference.