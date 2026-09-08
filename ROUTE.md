# ROUTE.md — The Canonical Guide

> **V0.8 — Agent-Native Development Protocol.** Documentation milestone.
> Codebase version `1.0.0`; the version-metadata conflict is deferred to the
> final Release Gate and is not resolved in this document.

Route is a **persistent development protocol/state/governance layer** for
AI-assisted work. It is not an LLM, not a coding agent, not a harness, and not
a required executable. This file is the single canonical reference for both
humans and agents working with Route.

**The Route Core is a protocol, not a binary.** A compatible AI — given this
file, its filesystem access, its reasoning, and whatever harness capabilities
it has — can uphold Route without the `route` executable. The Rust
implementation in this repository is the **Reference Engine**: a validator,
benchmark oracle, compatibility implementation, and optional convenience
tool. It is not the required product runtime.

**V0.8 advances Route from a Human↔AI protocol into an agent-native protocol:**

1. **Human ↔ AI** — a development protocol (WHAT + optional HOW).
2. **AI ↔ AI** — a coordination protocol (shared state, not private truths).
3. **AI ↔ Route** — a learning protocol (validated knowledge, not self-report).

---

## Core Creed

```
ROUTE ORGANIZES.
AGENTS EXECUTE.
EVIDENCE DECIDES.
MEMORY ACCUMULATES.

CAPABILITIES ARE ATOMIC.
COMPOSITIONS ARE TEMPORARY.
PROJECT STATE IS SHARED.
MEMORY IS PERSISTENT.

ROUTE IS NOT ONLY FOR CREATION. ROUTE IS FOR CONTINUITY.
UNDERSTAND BEFORE REWRITE.
PATCH WHEN PATCH IS ENOUGH.
UGLY != WRONG. OLD != BAD. NEW != BETTER.
PRESERVE WORKING VALUE. MAKE CHANGE REVERSIBLE. VERIFY BEFORE PROMOTION.
```

Canonical phrases used throughout:

- **AGENTS MAY DIE. ROUTE REMEMBERS.** — agents are temporary; evidence,
  findings, failures, decisions, strategy, memory, and lineage persist.
- **MEMORY IS PERSISTENT. CONTEXT IS COMPILED.** — agent context is a
  task-scoped view of Route state, never a copy of the database.
- **EXPLORE FREELY. EXECUTE STRICTLY.**
- **ROUTE IS NOT ONLY FOR CREATION. ROUTE IS FOR CONTINUITY.** — Route is a
  long-term project continuity protocol, not a one-shot code generator; the
  full lifecycle (understand/repair/refactor/restructure/maintain/migrate/
  deprecate/retire/recover) is defined in
  [§12](#12-operational-lifecycle-continuity-protocol).

"Route executes everything" means **Route owns organization/state/policy/
evaluation decisions**; physical execution is still performed by AI/harness/
tools.

---

# Part I — Route Protocol

Part I is the vendor-neutral protocol. Any capable AI/harness can operate from
Part I alone. Part II describes the Reference Engine that implements and
validates this protocol.

## 1. Route Core Definition

**Route Core** = persistent development protocol / state / governance for
AI-assisted work.

Route Core provides:

- **A persistent truth** — project state (tasks, sessions, evidence, saves,
  memory, references) that outlives any single agent conversation or host.
- **A causal chain** — Task → Context → Agent Plan → Execution Evidence →
  Result → Learning. Every durable change is auditable.
- **Governance** — Constitution / Protocol / Reference context, and a
  verification policy that trusts only validated evidence, not AI self-report.
- **Recovery** — save/restore/recovery so a project survives crashes, host
  handoffs, and even deleted-project events.

**Route is NOT:**

- an LLM or model provider
- a coding agent
- a harness (tool loop / shell / sandbox / subagent runtime)
- a required executable
- a substitute for Git

**Hard invariant: Route Core MUST NOT require a route executable.** A
compatible AI upholds Route through filesystem + reasoning + whatever harness
capabilities it has. A capability that only Rust can execute and whose
semantics ROUTE.md cannot describe is not a complete Route Core capability.

### Minimal Intent

Human / upstream AI needs to state only:

```
Intent {
  objective          // required: WHAT must become true
  method_hint?       // optional: HOW the user prefers it done
  hard_constraints?  // optional: invariants that must hold
  acceptance?        // optional: what counts as done
}
```

- `objective` is a goal that must become true — not a task list.
- `method_hint`, when present and not violating higher constraints, must not
  be casually rewritten.
- When HOW is missing, Route derives it from: Constitution, Project Memory,
  User Memory, Reference, Strategy, Failure knowledge, Benchmarks.
- Ask the user **only** for genuinely user-owned choices that cannot be
  resolved from existing state.

Intent detail: [docs/agents.md](docs/agents.md#minimal-intent).

## 2. The Bootstrap Protocol (V2)

ROUTE.md is the **bootstrap entry**, not the entire persistent database. A
fresh Agent bootstraps by **decisions**, in this order:

1. **Detect the project** — what is on disk, what user files exist.
2. **Detect existing Route state** — look for the Route state marker (`.route/`).
   If present, read and resume; do not re-initialize.
3. **Locate/init `ROUTE_HOME`** — the external Route home (default
   `Documents/Route/`); see [§7](#7-memory-and-context).
4. **Safely initialize state** — create the minimal state layout without
   touching user files.
5. **Load permitted User Memory** — only cross-project preferences the user
   has allowed; never project-private facts.
6. **Build initial Project/Architecture Memory** — from inspection, never by
   fabricating.
7. **Establish/verify Original** — the immutable baseline save.
8. **Detect the Harness tier** — enumerate actual capabilities (filesystem,
   shell, web, subagents, code mode, skills, workflows).
9. **Accept Intent** — WHAT + optional HOW (see [§1](#1-route-core-definition)).
10. **Derive the WorkGraph** — goals decomposed into verifiable work units.
11. **Derive the minimum Agent organization** — smallest complexity that
    completes the task (a typo needs no 8-agent council).
12. **Begin the managed Task** — bind a session, record baseline, capture
    evidence.

**Invariant**: initialization is idempotent and non-destructive. Detecting
existing state must not create a second, conflicting truth.

Bootstrap detail: [docs/protocol.md](docs/protocol.md#2-bootstrap).

## 3. Canonical State Schema

The `.route/` layout is a set of **stable semantic objects**. The exact file
format is an implementation choice; the semantic domains are the contract.

```
.route/
  project        # identity, root, initialization metadata
  constitution   # immutable development principles
  protocol       # versioned execution playbook
  reference      # external resource descriptions (CLI, repo, doc, skill, workflow)
  task           # work units, sessions, plans
  evidence       # recorded facts / events / test results / commits
  memory         # project + architecture memory (see docs/memory.md)
  save           # savepoints, recovery metadata, KnownGood
  learning       # experience events, proposals
  evolution      # candidates, benchmarks, campaigns (experimental)
```

Separate **semantic requirement** (what must be recorded) from **reference
implementation format** (how the Reference Engine writes it). A compatible AI
may store state in any format that preserves the semantics and the causal
chain.

State schema detail: [docs/protocol.md](docs/protocol.md#3-state-schema).

## 4. Architectural Axioms

### Atomic Capability Fabric

Capabilities decompose into the smallest useful, single-responsibility
operations:

```
observe    inspect   query      retrieve   record    classify
compare    checkpoint save      hash       restore   verify
test       propose   diverge    compose_agent execute report
evaluate   score     replicate  ablate     promote   reject
distill    remember
```

Each capability has one primary responsibility, explicit inputs, explicit
outputs, is composable, and owns **no** authoritative project truth. Agents
and workflows are temporary compositions of these capabilities.

**CAPABILITIES ARE ATOMIC. COMPOSITIONS ARE TEMPORARY. PROJECT STATE IS
PERSISTENT.**

Capability catalog: [docs/agents.md](docs/agents.md#atomic-capability-fabric).

### Global Shared State (Blackboard)

All agents operate on **one logical project truth**. Agents may hold scoped
working context; agents must **not** build private long-term project truths.

- **Reads**: shared by default, by permission.
- **Durable writes**: must enter through an auditable object:

  `Fact · Event · Evidence · Proposal · Decision · MemoryCandidate ·
  StrategyCandidate · BenchmarkCandidate`

  each recording actor, role, task/session, model/harness (if known),
  timestamp, source, provenance, and confidence when relevant.

- Shared state does **not** mean one giant JSON anyone may mutate. Physical
  storage may be partitioned; logically there is one authoritative truth.

Shared-state detail: [docs/protocol.md](docs/protocol.md#4-architectural-axioms).

## 5. Dynamic Agency

Agents are **temporary capability compositions**, not permanent personalities.
Route derives them from Intent and the WorkGraph.

```
AgentSpec {
  id
  role                       // what it is for
  goal                       // what it must achieve
  required_capabilities[]    // atomic capabilities it needs
  scoped_context[]           // what it may read
  excluded_context[]         // what it must not read
  permissions                // semantic permissions (see §9)
  expected_outputs[]
  evidence_requirements[]
  lifetime                   // single_action | task | experiment | campaign
  parent_task
  campaign?                  // when serving an evolution campaign
}
```

Do **not** define permanent hardcoded agents (`CoderAgent`, `ReviewerAgent`,
`ArchitectAgent`, …).

**Route manages**: why the agent exists, goal, capabilities, context,
permissions, outputs, evidence, lifetime.
**Harness manages**: model invocation, subagent spawn, tool execution,
sandbox, concurrency. Harness capability changes never change Route project
identity/state.

If the harness supports subagents → AgentSpec becomes a native agent. If not
→ degrade to isolated sequential phases of one agent, preserving role
boundaries.

**AGENTS MAY DIE. ROUTE REMEMBERS.** What persists after a task: Evidence,
Findings, Failures, Decisions, Strategy, Memory, Lineage.

AgentSpec detail: [docs/agents.md](docs/agents.md#agentspec).

## 6. Tripartite Organization

Three planes organize non-trivial work. This is **state-centered** — all roles
collaborate through Shared Route State — not a Manager→TeamLead→Worker
bureaucracy.

| Plane | Purpose | Binding constraints |
|-------|---------|---------------------|
| **EXPLORE** | maximize the possibility space. N independent divergent thinkers produce Hypotheses, adversarial cases, BenchmarkCandidates. May think beyond current architecture. | Cannot mutate Stable; cannot Promote; cannot declare its own proposal trusted. |
| **EXECUTE** | implement the selected hypothesis into a Candidate, strictly. | Must obey Intent + selected plan, minimal scope, evidence-producing; no unrelated refactors; no requirement invention; must not replace a user-requested method with a "better" one; when an assumption fails — report, do not improvise endlessly. |
| **EVALUATE** | decide what is worth keeping. Independent review via Benchmark / Reference / Evidence. | Cannot secretly modify the Candidate; implementer may not be sole evaluator; deterministic/trusted evidence outranks AI judgment. High-risk changes may use M independent evaluators. |

**EXPLORE FREELY. EXECUTE STRICTLY.** When execution hits a major new
situation: report blocker/evidence → Route decides retry / repair / explore /
ask-user. Route owns the final persisted Promote/Reject state. No plane
self-promotes.

On significant or uncertain tasks Route deliberately preserves **divergence**
(different models, framings, reference subsets, opposite assumptions, radical
vs conservative alternatives, architecture inversion, constraint-removal
experiments) instead of collapsing to Top-1 too early. Quality **and** novelty
are both retained. Route additionally learns **which task patterns suit which
organization structures** — but a single success only produces a
StrategyCandidate, never a permanent organization rule.

### Boom — Open-World Frontier Cognition (optional, protocol-only component)

> **Migration note:** Boom's subject-level semantics (Subject/Prime/ISM/
> SelfLearning) now canonically live in the **Yuich** Persistent Artificial
> Subject spec, maintained in the independent private repository
> `longqiyua/Yuich`. Boom is a cognitive **Mode** of Yuich. For Route
> nothing changes: Boom remains an optional, hot-pluggable provider behind
> the same narrow contract below, and Route never depends on Yuich. The
> optional YuichRouteAdapter is defined Yuich-side only.

```
BOOM ENCOUNTERS.  BOOM OPENS.  BOOM MUTATES.  REALITY SELECTS.
NO CURRENT BOOM FORM IS FINAL.
```

Boom is Route's independent, **hot-pluggable** open-world frontier
cognition component (**V0.8: protocol-only, EXPERIMENTAL / PLANNED — no
engine surface**; full spec: [docs/boom.md](docs/boom.md)). It is not a
"creative idea generator": it holds an internal **BoomSubject** that faces
the open world, receives **Encounters**, maintains an **OpenFrontier**
(explicit unknowns), recognizes **Impasse**, runs heterogeneous explorers,
and — Candidate-first — revises **Boom itself**.

Summary of the contract:

- **ROUTE DOES NOT DEPEND ON BOOM.** Boom integrates through one narrow
  contract: `FrontierRequest → Boom Provider → FrontierResult`; if Boom is
  absent, Route works perfectly (ordinary Explore / direct execution /
  `needs_human` — never pretending Boom ran).
- **Output** is the FrontierArtifact (20 kinds incl. NewQuestion,
  OntologyBreak, BlindSpot, OperatorCandidate, AxisCandidate,
  InterfaceLimitationProposal, BoomStrategyCandidate), preserving four
  layers: RAW / INTERPRETATION / STRUCTURED / **RESIDUAL** —
  structuralization must not erase residual.
- **Method** is engineered heterogeneity: BoomExplorerSpecs across
  knowledge space × 16 open-set cognitive operators × conceptual distance
  (0–5) × stance × representation × destructive constraints × context
  aperture (memory must not become a prison) × 11 search topologies.
- **Autonomy is epistemic, not operational**: Boom may challenge hypotheses
  and `method_hint` via BetterClaim ("might be better", never "IS better"),
  but never violates hard constraints or mutates Stable/Route/KnownGood.
- **Ecology**: hallucination is mutation source, never Evidence; a Boom run
  succeeds even with zero implementation ideas if it discovers a valuable
  frontier; anti-monoculture and a **non-zero exploration floor** are
  invariants; Boom self-change is Candidate-first (cannot self-promote),
  plus periodic **SelfEstrangement** — Boom must be capable of escaping
  Boom.
- **Maintenance handoff**: on repeated repair failure / architecture
  impasse / candidate monoculture / unknown cause, Route may issue a
  `FrontierRequest`; selected results are converted by Route into
  engineering Candidates (see [§12](#12-operational-lifecycle-continuity-protocol)).

Tripartite + divergence detail: [docs/agents.md](docs/agents.md#tripartite-organization).

## 7. Memory and Context

Route accumulates memory in layers. See [docs/memory.md](docs/memory.md) for
the full model.

- **Project Memory** — what this project must remember long-term:
  architecture, domain vocabulary, important components, design decisions,
  constraints, historical failures, successful strategies, conventions, task
  patterns, agent-organization outcomes, harness observations, benchmarks.
- **Architecture Memory** (evolvable subset) — components, responsibilities,
  dependencies, boundaries, data flow, invariants, risk areas; updated
  continuously from inspection, tasks, verified changes, user explanations,
  decisions, evidence.
- **User Memory** — what is stable about **this user across projects**:
  working/tool/architectural/interaction preferences, tradeoff tendencies,
  recurring constraints, reusable conventions. Never credentials; never
  one-off inferences; never automatic cross-project leakage of project-private
  facts.

`ROUTE_HOME` (semantic layout; default `Documents/Route/`):

```
ROUTE_HOME/
  user/                     # memory, preferences (cross-project)
  projects/<project-id>/    # original, known-good, archive/history
  route/                    # Route self: original, known-good, candidates,
                            #   history, benchmarks
  shared/                   # strategies, benchmark-patterns
```

#### Document-area backup: LOCAL git, no remote

`Documents/Route/` is also a **local git repository** — but one that never
connects to any remote. The repo lives *inside* the document folder itself, so
entire-archive history, rollback and archival are recorded purely on disk and
no data ever leaves the machine. This is machine-level history on top of the
structured self-archive (`route/versions/`, append-only). Commands:

- `route self-archive git-init` — idempotently `git init`s `Documents/Route/`
  and guarantees it stays remote-less (any configured remote is removed).
- `route self-archive git-commit --message "..."` — `git add -A` + commit a
  snapshot of the whole document area. No-op when nothing changed.
- `route self-archive git-log [--limit N]` — inspect the local backup history.

Route also self-evolves its own standard into a **Standard Operating
Procedure** from real project memory + its own constitution/protocol/reference
(`route self-sop`, writes `.route/sop.md`), so development constraints are
upgraded into a repeatable, evidence-based SOP rather than a hand-written demo.

### Memory scope and promotion

Scopes: `TASK → PROJECT → USER → ROUTE_SHARED`. Knowledge moves **upward only
by promotion**: a Task Finding becomes Project Memory only after
validation/repetition; cross-project evidence may then promote to User Memory
or Shared Strategy. Every memory item records scope, source, provenance,
confidence, last_verified, related tasks/projects. Conflicts never silently
overwrite. Shared state does not erase scope.

### Context compilation

```
MEMORY IS PERSISTENT. CONTEXT IS COMPILED.
```

Agent context is a **task-scoped view** of Route State, not a database copy.
Each agent receives only: goal, constraints, relevant memory, relevant
architecture, relevant references, relevant failures/history, permissions,
expected outputs. Bulk-injecting Project Memory / User Memory / References /
Route History into every prompt is prohibited. Context is selected
dynamically by task, AgentSpec, and permission.

Distinguish **raw history** (everything that happened), **distilled memory**
(validated summaries), and **trusted knowledge** (promoted, verified).

Memory detail: [docs/memory.md](docs/memory.md).

## 8. Co-Learning

Route and the AI co-evolve, but AI output is **untrusted until evaluated**.

| Route → AI | AI → Route |
|-----------|-----------|
| history | findings |
| architecture | evidence |
| constraints | failures |
| user preferences | hypotheses |
| strategies | benchmark candidates |
| references | strategy candidates |
| known failures | architecture discoveries, agent-organization outcomes |

Learning chain:

```
AI output → Proposal → Evidence → Validation
          → Replication/Reference/Benchmark (when needed)
          → Distillation → Scoped promotion
```

Only **validated** knowledge becomes Experience, Strategy, Reference-derived
rule, Benchmark, or Agent-composition preference. This creates co-evolution
without self-confirming hallucination.

Co-learning detail: [docs/protocol.md](docs/protocol.md#7-co-learning).

## 9. Permissions and Steward

### Semantic permissions

Harness-neutral, enforced by protocol; the harness only maps them to actual
capabilities:

```
read_project          write_candidate       write_project
read_route_state      propose_route_state   mutate_route_state
read_user_memory      propose_user_memory   read_reference
execute_shell         run_tests             evaluate
promote
```

Defaults per role:

- **Explorer** — broad reads + write Hypothesis/Proposal. No Stable mutation,
  no Promote.
- **Executor** — write Candidate/task scope. No self-Promote.
- **Evaluator** — read Candidate/Evidence/Reference. No Candidate mutation;
  never sole judge of something it implemented.

Permissions detail: [docs/agents.md](docs/agents.md#permissions).

### Route Steward

The **Steward** is a **responsibility, not a permanent process**:
maintain continuity, compile context, maintain memory, organize agents, watch
evidence, preserve saves, supervise recovery, learn outcomes, prevent scope
drift, maintain Route state. Any capable AI may temporarily assume it.

## 10. Harness Optionality

**Model ≠ Harness ≠ Route.** Route must be usable across tiers:

| Tier | Name | Route context/state carried by |
|------|------|-------------------------------|
| **Tier 0** | Manual AI | the agent, manually. |
| **Tier 1** | Coding Harness | Claude Code / Codex / DSH / compatible systems, automatically. |
| **Tier 2** | Advanced Harness | subagents / skills / workflows / Code Mode realize dynamic AgentSpecs and campaign orchestration. |

At every tier: same project identity, same memory, same evidence lineage, same
Route semantics. Harness integrations are **optional adapters**, never core
dependencies. Adaptation guidance: [docs/harness.md](docs/harness.md).

## 11. Save, Self-Save, and Self-Evolution

### Game-save model

Every project maintains: **ORIGINAL** (immutable baseline), **HISTORY**,
**KNOWN_GOOD** (last verified promotion), **CANDIDATES** (isolated,
reversible). Save = project state + Route state. The external archive lives
outside the project; deleting a project never deletes its archive. Restore is
inspectable, scoped where possible, reversible; dangerous restore/self-update
first creates a pre-operation save. Candidates can never directly overwrite
Original/KnownGood.

### Route self-save and self-evolution

Route itself has: Route Original, Route KnownGood, Route Candidates, Route
History. Canonical Route must never have only one writable copy. Route may
learn and produce a Candidate of itself (repeated failures, user corrections,
bootstrap benchmarks, cross-project experience, harness behavior,
agent-organization results):

```
Current Route → detect limitation → Candidate ROUTE.md/policy
→ benchmark: fixed invariants, historical tasks, regressions,
  known-good cases, trusted references, independent evaluator
→ compare → promote/reject → retain previous KnownGood
```

A Route Candidate cannot declare itself better using benchmarks it just wrote
for itself. **Route must not be its own only judge.** Self-update preserves:
Protocol First, Atomic Capability, Shared State, Dynamic Agency, Co-Learning,
recoverability.

Self-evolution detail: [docs/evolution.md](docs/evolution.md#route-self-evolution).

## 12. Operational Lifecycle (Continuity Protocol)

```
UNDERSTAND → PLAN → CHECKPOINT
→ BUILD | PATCH | REPAIR | REFACTOR | RESTRUCTURE | MIGRATE
→ VERIFY → COMPARE → PROMOTE → OBSERVE → MAINTAIN
→ new issue? → UNDERSTAND …
```

Route is a **long-term project continuity protocol, not a one-shot code
generator**. Canonical summary of the full lifecycle — existing-project
takeover (LEGACY / PARTIALLY_BROKEN / ABANDONED / …), ProblemModel and
Architecture Memory reconstruction, the strict change taxonomy
(PATCH / REPAIR / REFACTOR / RESTRUCTURE / REWRITE / MIGRATE — **no automatic
escalation; minimal sufficient intervention**), the restructure protocol
(baseline → invariants → staged reversible transition; **OLD → BRIDGE →
NEW, never OLD → DELETE → HOPE**), preservation defaults, maintenance mode,
multi-dimensional project health (**no fabricated single Health Score**),
the repair loop, maintenance memory (`DebtRecord`), real-verification
operability (**NOT_RUN/UNKNOWN, never fake PASS**), deprecation/retirement
states, and abandoned-project takeover — is specified in
[docs/maintenance.md](docs/maintenance.md).

Key canonical rules:

- **UNDERSTAND BEFORE REWRITE. UGLY != WRONG. OLD != BAD. NEW != BETTER.**
  Never rewrite because code is ugly; deletion is not the default meaning
  of "cleanup".
- **User intent semantics** — Objective / HardConstraint / Acceptance /
  MethodHint / Preference / Hypothesis are split; Route may challenge and
  counter-propose, but never silently overrides a HardConstraint
  (semantic loyalty over literal imitation, hard boundaries explicit).
- **Consequence levels** — L0 read/analyze · L1 local reversible · L2
  project mutation · L3 high-risk/large-scope · L4 external/irreversible;
  higher levels demand stronger permission / checkpoint / evidence /
  recovery / confirmation.
- **Responsibility boundary & canonical semantics** — what Route owns vs
  what the operator/user owns, and the ROUTE OFFICIAL / COMPATIBLE /
  DERIVATIVE / INSPIRED compatibility model with the disclaimer/compliance
  notice, are defined in [docs/governance.md](docs/governance.md).
- **Boom integration** — Boom stays optional; maintenance never depends on
  it. On repeated repair failure / architecture impasse / candidate
  monoculture / unknown cause, Route may issue a `FrontierRequest`; the
  selected `FrontierResult` is converted by Route into
  Plan/Architecture/Experiment/Benchmark Candidates before entering this
  lifecycle. **BOOM DISCOVERS POSSIBILITY; ROUTE MAKES SELECTED POSSIBILITY
  OPERABLE.**

Lifecycle detail + benchmarks: [docs/maintenance.md](docs/maintenance.md).

---

# Part II — Reference Engine

Part II describes the Rust implementation in this repository. It is one valid
implementation of Part I, not the protocol itself.

### Reference Engine Boundary

The Rust implementation is an **optional Reference Engine**. It may provide:
validation, archive implementation, CLI, machine JSON, tests, stronger
recovery, benchmarking. ROUTE.md semantics must remain understandable and
usable without it.

| Level | What it guarantees |
|-------|--------------------|
| **Protocol Guarantee** | Semantics a compatible AI must uphold by reasoning + filesystem. |
| **Reference Engine Enforcement** | Enforced by the Rust implementation when present. |
| **Harness / OS Enforcement** | Enforced by the host environment (permissions, sandbox, process boundaries). |

Reference entries are **external reality anchors** — usable for architecture,
workflow, standards, tool usage, benchmark seeds, comparison evidence — each
describing authority, freshness, scope, provenance, conflicts. References are
not absolute truth.

Trust ordering:

```
hard invariant / trusted physical evidence
  > verified benchmark / trusted reference
  > independent AI evaluation
  > candidate self-report
```

**Do not claim Markdown alone provides an unbypassable security boundary.**

> The version-metadata note: **V0.6 Beta is the current product milestone.**
> Crate/package version metadata remains unchanged (`1.0.0`). This is an
> intentional, temporary distinction pending the user's decision at the final
> Release Gate.

## 13. Constitution / Protocol / Reference

- **Constitution** — immutable, stable development principles.
- **Protocol** — the versioned execution playbook (task lifecycle, verify,
  review gates).
- **Reference** — a registry of external resource descriptions (CLI tools,
  repositories, documents, skills, workflows) injectable into agent context.

`route apply --target <claude|codex|deepseek|generic>` compiles the assembled
context into a host file. The generated file is **not** the source of truth;
`.route/` is.

## 14. Task / Session / Evidence

- **Task** — a unit of work (declared Intent).
- **Session** — concrete execution under a target host:
  `begin → exec → verify → end`.
- **Evidence** — anything recorded during a session. Only **system evidence**
  (trusted test pass, commit, check pass) verifies; AI self-reports are
  `AgentFeedback`, never proof of success.

A session may reference a parent Evolution Campaign (`campaign_id`); a
campaign may reference its parent Task (`task_id`). Ordinary tasks without a
campaign are unaffected.

### Open development commons

Independent workers may coordinate through the project-scoped
`DevelopmentEvent` ledger exposed by `route/1`. Every accepted event advances
one monotonic project revision. Worker identity persists independently of
model, provider, process, session, role, and workspace; presence is bounded
development metadata rather than runtime truth. Worker messages are visible
coordination records and are never Evidence.

The shared-state view projects existing Intent/ExecutionSession, Evidence,
Git, KnownGood, worker, presence, and recent-event truth. It does not create a
second owner for those domains. See
[docs/open-development-substrate.md](docs/open-development-substrate.md) and
[docs/route-cooperation-protocol.md](docs/route-cooperation-protocol.md).

## 15. Save / Original / KnownGood / Recovery

- **Save** — `route archive save` (external archive) or `route save`
  (development savepoint in `.route/`).
- **Original** — the immutable archive save created at `route init`.
- **KnownGood** — the last verified, promoted configuration.
- **Recovery** — restore from an archive save; preview-first; full/project/
  route-state/paths scopes; deleted-project recovery.

## 16. Learning / Evolution

- **Learning** (`route learn`) — experience → proposals; never auto-applied.
- **Evolution** (`route evolve`) — candidate-first verified loop. **EXPERIMENTAL.**
- **Emergence** (`route emerge`) — hardening layer. **EXPERIMENTAL, off by default.**
- **Campaign** (`route evolve campaign`) — experiment-set governance with
  budgets. **EXPERIMENTAL.** Harness handoff via machine-readable
  `next_action` contract: [docs/dsh-pic.md](docs/dsh-pic.md).

## 17. Agent-Surfaces in the Reference Engine

Implemented surfaces that map to Part I §5/§6 (analytical only — the engine
never runs agents):

- `route plan` / `route agent-plan` — deterministic plan compilation
  (counterfactual preview, no execution).
- `route agents` — role templates accumulated from successful plans
  (statistics, not permanent personalities).
- `route agent-org` — organization experience recall (what topology worked
  for what task pattern).
- `route capability` — capability view over the Reference registry.

## 18. Memory Surfaces in the Reference Engine

- `route memory` — project memory (show/history/refresh/apply/why/supersede/map).
- `route brain` — derived compressed view (brief/for --task/explain/conflicts).
- Project Memory and Architecture Memory are persisted per-project
  ([docs/memory.md](docs/memory.md#reference-engine-surface)). **User Memory
  and ROUTE_HOME `user/`/`shared/` are protocol-defined and not yet
  implemented** — see [docs/status.md](docs/status.md).

## 19. Project Lifecycle

1. `route init` — creates `.route/`, config, profiles, Original archive save.
2. `route task begin "task" --target <host>` — session, task-scoped context,
   host apply.
3. Work happens; `route task exec S -- <command>` records evidence.
4. `route task verify S` — checks verification policy.
5. `route task end S --result success` — auto-verify, save, learning
   proposals.
6. `route archive save` — external snapshots for recovery.
7. `route archive recover` — restore a deleted project.

## 20. Safety Invariants

- Restore/repair default to **preview first**.
- The **Original** archive save is **immutable**; candidates never overwrite
  Original/KnownGood.
- `latest_verified` accepts only **system evidence**.
- **Auto-save** before destructive operations (PRE_RESTORE, PRE_REPAIR,
  PRE_CHANGE).
- File writes are **atomic** with hash verification.
- Projects are **cross-project isolated** in the archive.
- **Local-first** — no cloud upload.
- `.route/` is protected by the Route Guard.

## 21. CLI Entry Map

| Group | Commands |
|-------|----------|
| Project | `init`, `status`, `commit`, `log`, `rollback`, `diff`, `changes`, `undo`, `redo`, `checkpoint`, `backup` |
| Branching | `branch`, `tag`, `annotate`, `annotations`, `export` |
| Stats | `stats`, `stats-report` |
| Data / IO | `sync`, `tracking`, `plugin`, `backup` |
| Git bridge | `git` |
| Governance | `constitution`, `protocol`, `reference`, `workflow`, `profile`, `context`, `apply`, `check`, `repair-plan` |
| Tasks | `task`, `begin`, `plan`, `agent-plan`, `impact` |
| Agents | `agents`, `agent-org`, `capability` |
| Knowledge | `memory`, `brain`, `knowledge-map`, `strategy`, `experiment`, `pattern`, `discover`, `pack`, `idea`, `failure`, `principle`, `trajectory`, `roadmap`, `goal` |
| Recovery | `archive`, `save`, `savepoint` |
| Health | `health`, `health-history`, `guardian`, `maintain`, `loops`, `drift`, `next` |
| AI bridge | `ai`, `project-context`, `mcp`, `permission`, `base`, `conversation`, `extension` |
| Learning | `learn`, `study`, `study-apply`, `self-improve` |
| Evolution | `evolve`, `emerge` |
| Docs / handoff | `brief`, `handoff`, `curator` |

See [docs/cli.md](docs/cli.md) for the full command reference.

## Open Institution Runtime (bounded declarative workflow)

Route provides the substrate, not the society. The optional project-scoped
[Institution Runtime](docs/open-institution-runtime.md) adds immutable package
versions, explicit operator-owned bindings, six generic hooks, typed effects,
deterministic composition and read-only replay on the existing development
ledger. It does not add a Worker, Task system, event bus or state database.
Requested capabilities are not authority; institution outputs are not Evidence
or authorized actions. Only safe declarative packages execute today, through
explicit CLI/route/1 invocation. Parliament, markets and autonomous
self-modification remain unimplemented, not mandatory core policy.

## 22. Documents Map

| File | Purpose |
|------|---------|
| [README.md](README.md) | First-visitor landing page |
| [ROUTE.md](ROUTE.md) | This file — canonical human + agent guide (agent-native protocol) |
| [docs/protocol.md](docs/protocol.md) | Vendor-neutral Route protocol reference |
| [docs/route-cooperation-protocol.md](docs/route-cooperation-protocol.md) | Canonical host-neutral `route/1` machine protocol and stdio transport |
| [docs/open-development-substrate.md](docs/open-development-substrate.md) | Project event ledger, revision, Worker identity, presence, message/evidence boundary, and institution ownership boundaries |
| [docs/open-institution-runtime.md](docs/open-institution-runtime.md) | Institution author contract, CLI/RPC, bounds, authority and verified workflow |
| [docs/agents.md](docs/agents.md) | **Agent-native reference: AgentSpec, atomic capabilities, tripartite, permissions, steward (V0.8)** |
| [docs/memory.md](docs/memory.md) | **Memory model: project/architecture/user memory, ROUTE_HOME, scopes, context compilation (V0.8)** |
| [docs/concepts.md](docs/concepts.md) | Core concepts explained |
| [docs/architecture.md](docs/architecture.md) | Layers and data flow |
| [docs/recovery.md](docs/recovery.md) | Save / recovery / deleted-project recovery |
| [docs/evolution.md](docs/evolution.md) | EXPERIMENTAL evolution + learning |
| [docs/harness.md](docs/harness.md) | Model vs Harness vs Route; tiers + adaptation |
| [docs/dsh-pic.md](docs/dsh-pic.md) | DSH/PIC machine-to-machine example |
| [docs/references.md](docs/references.md) | Reference compatibility + context |
| [docs/bootstrap-benchmark.md](docs/bootstrap-benchmark.md) | Reusable bootstrap + protocol benchmarks |
| [docs/boom.md](docs/boom.md) | **EXPERIMENTAL / PLANNED — Boom open-world frontier-cognition component spec (optional, hot-pluggable; protocol-only)** |
| [docs/quickstart.md](docs/quickstart.md) | Shortest real run |
| [docs/cli.md](docs/cli.md) | Full CLI reference |
| [docs/status.md](docs/status.md) | Feature matrix (Stable/Beta/Experimental/Partial/Planned) |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |
| [ROADMAP.md](ROADMAP.md) | Short-term priorities |
| [SECURITY.md](SECURITY.md) | Security model and reporting |

## 23. Glossary

| Term | Meaning | Do not say |
|------|---------|-----------|
| **Intent** | The minimal user input: objective (+ optional method/constraints/acceptance). | "prompt" |
| **WorkGraph** | The decomposition of an Intent into verifiable work units. | "todo list" |
| **AgentSpec** | A vendor-neutral, temporary agent composition description. | "persona" |
| **Steward** | The responsibility of maintaining Route continuity; any capable AI may assume it. | "daemon" / "process" |
| **Save** | A captured project state (archive save or dev savepoint). | "backup" |
| **Snapshot** | A commit-like state in the project ledger (`route commit`). | "save" |
| **Archive** | The external store at `Documents/Route/` (ROUTE_HOME). | "backup" |
| **Original** | The immutable baseline archive save created at `route init`. | "stable" |
| **KnownGood** | The last verified, promoted configuration. | "stable" / "original" |
| **Rollback** | Step back to a snapshot in the project ledger. | "recovery" / "restore" |
| **Restore** | Bring a save's contents back into the project. | "recovery" / "rollback" |
| **Recovery** | Rebuilding a project (incl. deleted) from the archive. | "rollback" |
| **Agent** | A model driven by a harness, doing work. | "host" / "harness" |
| **Harness** | The runtime driving the model. | "agent" / "model" |
| **Model** | The underlying LLM. | "harness" / "agent" |
| **Candidate** | A proposed change, isolated and reversible. | "patch" |
| **Divergence** | Deliberately preserved difference between explorers' outputs. | "noise" |
| **Boom** | The optional, hot-pluggable (protocol-only) open-world frontier-cognition component. | "agent runtime" / "brainstorm feature" |
| **BoomSubject** | The continuity/decision position inside Boom only (frontier identity, exploration memory). | "Route Subject" / "permanent process" |
| **Encounter** | Anything that may disturb or expand Boom's current model; assessed, not automatically chased. | "trigger" / "input event" |
| **OpenFrontier** | Boom's explicit representation of unknowns, conflicts, and unclassifiables. | "backlog" / "error log" |
| **FrontierArtifact** | A Boom output unit (20 types incl. NewQuestion, OntologyBreak, AxisCandidate, BoomStrategyCandidate). | "idea" / "solution" |
| **Route Core** | The persistent protocol/state/governance layer (vendor-neutral). | "the route binary" |
| **Reference Engine** | The Rust implementation: validator, benchmark oracle, CLI. | "Route Core" |
| **ROUTE_HOME** | The external Route home directory (`Documents/Route/`). | "the archive" alone |

---

**License:** AGPL-3.0 — see [LICENSE](LICENSE).
