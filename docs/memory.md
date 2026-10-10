# Memory — Project, Architecture, and User Memory

> **V0.8.** The deep reference for the memory model defined in
> [ROUTE.md §7](../ROUTE.md#7-memory-and-context). Vendor-neutral. The
> Reference Engine mapping at the bottom states exactly what is implemented
> today — **User Memory is protocol-defined and not yet implemented**.

Canonical axiom:

```
MEMORY IS PERSISTENT. CONTEXT IS COMPILED.
```

---

## Memory Layers

### Project Memory

What **this project** must remember long-term:

- architecture
- domain vocabulary
- important components/files
- design decisions
- constraints
- historical failures
- successful strategies
- conventions
- task patterns
- agent-organization outcomes
- harness observations
- benchmarks

Project Memory is scoped to one project. It never silently crosses into other
projects.

### Architecture Memory

An evolvable subset of Project Memory describing the system's structure:

- components
- responsibilities
- dependencies
- boundaries
- data flow
- invariants
- risk areas

Architecture Memory updates continuously from: project inspection, tasks,
verified changes, user explanations, decisions, evidence. Every long-lived
fact keeps **provenance** — where it came from and what verified it.

### User Memory

What is stable about **this user across projects**:

- working preferences
- tool preferences
- architectural preferences
- interaction preferences
- tradeoff preferences
- recurring constraints
- reusable conventions

Project Memory answers: *"What does this project need to know long-term?"*
User Memory answers: *"What is stably true about this user across projects?"*

Prohibited in User Memory:

- credentials / secrets
- one-off inferences without basis
- automatic cross-project leakage of project-private facts

Only **cross-project stable** knowledge enters User Memory, and only through
promotion (below).

---

## ROUTE_HOME

The external Route home (default `Documents/Route/`) is a **semantic layout** —
the file implementation is not locked:

```
ROUTE_HOME/
  user/                     # memory, preferences (cross-project)
  projects/<project-id>/    # original, known-good, archive/history
  route/                    # Route self: original, known-good, candidates,
                            #   history, benchmarks
  shared/                   # strategies, benchmark-patterns
```

- `projects/<id>/` — per-project isolation; a project's saves never leak into
  another project.
- `route/` — Route's own self-save domain (see
  [ROUTE.md §11](../ROUTE.md#11-save-self-save-and-self-evolution)).
- `shared/` — cross-project validated strategies and benchmark patterns, gated
  by promotion.

Deleting a project directory never deletes its ROUTE_HOME archive.

---

## Memory Scope and Promotion

Scopes, lowest to highest:

```
TASK → PROJECT → USER → ROUTE_SHARED
```

Knowledge moves **upward only by promotion**:

```
Task Finding
  → validated / repeated
  → Project Memory
  → cross-project evidence
  → User Memory / Shared Strategy
```

Every memory item records at least:

- `scope`
- `source`
- `provenance`
- `confidence`
- `last_verified`
- `related tasks/projects`

Rules:

- **Conflicts never silently overwrite.** A conflicting item is recorded and
  surfaced (existing/new both kept with lineage) until resolved.
- Shared State does not mean scope disappears — USER-scope items are not
  PROJECT-readable by default and vice versa.
- A single success never promotes a strategy; repetition and validation gate
  promotion (same rule as organization learning in
  [agents.md](agents.md#tripartite-organization)).

---

## Context Compilation

```
MEMORY IS PERSISTENT. CONTEXT IS COMPILED.
```

Agent context is a **task-scoped view** of Route State — never a copy of the
database.

Each agent receives only:

- goal
- constraints
- relevant memory
- relevant architecture
- relevant references
- relevant failures/history
- permissions
- expected outputs

Prohibited: bulk-injecting Project Memory, User Memory, References, or Route
History into every prompt. Context is selected dynamically by **task,
AgentSpec, and permission** (`scoped_context[]` / `excluded_context[]`).

### Raw vs distilled vs trusted

Distinguish three levels:

| Level | Meaning |
|-------|---------|
| **raw history** | everything that happened (append-only) |
| **distilled memory** | validated summaries with provenance |
| **trusted knowledge** | promoted and verified (usable as ground for decisions) |

Raw conversation is **not** active memory. Only distilled, validated items are
candidates for context compilation; only trusted knowledge may gate decisions.

---

## Co-Learning Gate

AI → Route knowledge flow (findings, evidence, failures, hypotheses,
benchmark/strategy candidates, architecture discoveries) is **untrusted until
evaluated**:

```
AI output → Proposal → Evidence → Validation
          → Replication / Reference / Benchmark (when needed)
          → Distillation → Scoped promotion
```

This prevents self-confirming hallucination: an AI may not mark its own
finding as trusted. See [protocol.md](protocol.md#7-co-learning).

---

## Reference Engine Surface

What the Rust Reference Engine provides **today**:

| Protocol concept | Engine surface | Status |
|------------------|----------------|--------|
| Project Memory | `route memory show/history/refresh/apply/why/supersede/map` (persisted per-project) | Implemented (Beta) |
| Architecture Memory | Project Memory fields (`current_architecture`, decisions, invariants) + knowledge map | Partial (subset of the full semantic model) |
| Knowledge graph | `route memory map` (node/edge knowledge map) | Implemented (Beta) |
| Distilled view | `route brain show/brief/for --task/explain/conflicts/compact/doctor` (derived view, not SSOT) | Implemented (Beta) |
| Context compilation | task-scoped context assembly at `task begin` / `context --task` / `brain for --task` | Implemented (Beta) |
| **User Memory** | — | **Protocol only — not implemented** |
| **ROUTE_HOME `user/`, `route/`, `shared/`** | archive layout has `projects/<id>/` + `registry.json` only | **Protocol only — not implemented** |
| Memory promotion across scopes | trajectory has a cross-project pattern transform; no user-level store to receive it | Partial |

A compatible AI can uphold the full protocol (including User Memory) with
filesystem + reasoning — the ROUTE_HOME `user/` directory is a plain,
provenance-carrying store any agent may maintain under the promotion rules.
The engine catch-up is future work; see [status.md](status.md).