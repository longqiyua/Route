# Architecture

This document describes Route's layers and data flow. For concepts, see
[concepts.md](concepts.md); for commands, see [cli.md](cli.md). It does not
duplicate CLI tutorials.

## Layers

Since V0.8 the architecture has a **protocol layer above the code**: the
vendor-neutral Route Protocol ([ROUTE.md Part I](../ROUTE.md#part-i--route-protocol),
[protocol.md](protocol.md)) defines the semantics; the Rust workspace below is
the **optional Reference Engine** that implements, validates, and benchmarks
them. The protocol does not require the engine.

```
┌─────────────────────────────────────────────────────────────┐
│  Protocol (vendor-neutral, no executable required)          │
│  ROUTE.md Part I · docs/protocol.md · docs/agents.md ·      │
│  docs/memory.md                                             │
└─────────────────────────────────────────────────────────────┘
                     │  implements / validates
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  Interfaces                                                 │
│  route-cli   (CLI binary)                                   │
│  route-mcp   (MCP server, thin delegate)                    │
│  route-tui   (terminal UI — experimental)                   │
│  route-http  (HTTP REST API)                                │
│  route-pyo3  (Python bindings — experimental)               │
└─────────────────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  Domain logic                                               │
│  route-basic   (task, session, evidence, archive, recovery, │
│                governance, learning, evolution, campaign,   │
│                agent_compiler, memory, agent_org)           │
└─────────────────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│  Primitives                                                 │
│  route-core    (paths, hash, storage, safety, schema,       │
│                SQLite migrations, guard)                    │
└─────────────────────────────────────────────────────────────┘
```

Additional crates:

- `route-engine` — search / fuzzy matching / token budget.
- `route-memory` — memory store, causal chain, conversation.
- `route-sync` — sync transports (local, WebDAV, S3, SSH).
- `route-stats` — statistics collection / reporting.
- `route-plugins` — plugin bus (logger, webhook).

> Note on the **archived GUI**: `archive/` contains a previous Tauri-based
  desktop application. It is not part of the active codebase and is not
  maintained by this release.

## Shared State

Logically there is **one authoritative project truth** per project
(the blackboard axiom). Physically it is partitioned across `.route/` domains
(task, evidence, memory, save, learning, evolution). Agents coordinate through
this shared state — durable writes enter only as auditable objects (Fact,
Event, Evidence, Proposal, Decision, MemoryCandidate, StrategyCandidate,
BenchmarkCandidate). See [protocol.md §4](protocol.md#4-architectural-axioms).

## Core Data Flow

The central loop is the **task session**:

```
route task start
   → creates session
   → builds task-scoped context (Constitution → Protocol → Profile → Workflow
     → Memory/Brain → selected References)
   → auto-saves (PRE_CHANGE)
   → applies context to the host file
   → returns session_id + host instructions

route task exec S -- <command>
   → direct process spawn (no shell)
   → records CheckPass / CheckFail evidence
   → hashes + truncates stdout/stderr

route task verify S
   → checks the Protocol verification policy
   → accepts only system evidence
   → returns VERIFIED / FAILED / INCOMPLETE

route task end S --result <success|failed|aborted>
   → auto-verifies
   → auto-saves (VERIFIED on success)
   → records trajectory
   → generates learning proposals (no auto-apply)
```

## Storage Locations

- **Project working state:** `<project>/.route/`
  - config, SQLite database, object store, context archive, generated host
    files, memory, references, etc.
- **External recovery archive:** `Documents/Route/`
  - `projects/<project-id>/` — per-project saves
  - `registry.json` — project registry
  - The archive is independent of the project directory and is never included
    in any snapshot.

## Trust Roots

Some data is treated as a trust root that candidates and ordinary tools cannot
modify:

- **Original** archive (source of truth for disaster recovery)
- **KnownGood** (last verified, promoted configuration)
- **Evidence / Audit** ledger (append-only)
- **Benchmark baseline** (fixed + holdout cases)
- **Promotion policy** (who may promote, and what gate is required)
- **Recovery executor** (the mechanism that restores on disaster)

See [evolution.md](evolution.md) for how trust roots bound the evolution loop.

## Lifecycle Data Flow

Alongside the task-session loop, Route defines an **operational lifecycle**
(understand → plan → checkpoint → build/patch/repair/refactor/restructure/
migrate → verify → compare → promote → observe → maintain) so a project is
continuity-carried, not only created. Key data objects:
`ProblemModel` (symptoms vs verified causes), Architecture Memory with
`OBSERVED/INFERRED/…` annotations, `DebtRecord` (maintenance memory), and
consequence levels L0–L4 gating mutations. Canonical summary:
[ROUTE.md §12](../ROUTE.md#12-operational-lifecycle-continuity-protocol);
full spec: [maintenance.md](maintenance.md).

## Harness Boundary

Route does **not** implement a model loop, shell, sandbox, or subagent
runtime. That is the **harness's** responsibility. Route only records,
constrains, verifies, selects, and recovers. See
[harness.md](harness.md).