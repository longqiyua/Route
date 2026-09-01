# Route Protocol — Vendor-Neutral Reference

> **V0.8.** This is the deep reference for the Route Core protocol described in
> [ROUTE.md](../ROUTE.md#part-i--route-protocol). It is **vendor-neutral**: no
> executable, harness, or model is required to uphold it. The Rust
> implementation is one Reference Engine over this protocol; see
> [ROUTE.md Part II — Reference Engine Boundary](../ROUTE.md#part-ii--reference-engine).

The protocol is a **governance contract**, not a runtime. A compatible AI
upholds it through filesystem access, reasoning, and whatever harness
capabilities it has. It is not an unbypassable security boundary.

V0.8 makes the protocol **agent-native**:

1. **Human ↔ AI** — development protocol (minimal Intent).
2. **AI ↔ AI** — coordination protocol (shared state).
3. **AI ↔ Route** — learning protocol (validated knowledge).

---

## 1. Route Core

Route Core = persistent development protocol / state / governance for
AI-assisted work.

- **Persistent truth** outliving any single agent conversation or host.
- **Causal chain**: Task → Context → Agent Plan → Execution Evidence → Result
  → Learning.
- **Governance**: Constitution / Protocol / Reference; verification policy that
  trusts validated evidence, not AI self-report.
- **Recovery**: save / restore / recovery independent of the project directory.

## Shared development substrate

Route Core provides a host/model-neutral project commons through a durable,
ordered `DevelopmentEvent` ledger. A Worker is a persistent participant
identifier—not a model, process, role, session, or workspace. Each event has a
stable id, project id, global sequence, timestamp, typed payload, actor and
optional intent/task/session/workspace/causation/correlation/source/evidence
references. Presence and messages are projections and coordination facts;
they do not become Evidence.

The ledger is a primitive below any future institution runtime. Parliament,
market, swarm, hierarchy, reputation, social simulation, and autonomous worker
self-modification are not Route Core capabilities. The complete persistence,
visibility, and reasoning boundary is in
[open-development-substrate.md](open-development-substrate.md).

**Route Core MUST NOT require a route executable.** A capability that only
Rust can execute, and whose semantics ROUTE.md cannot describe, is not a
complete Route Core capability.

---

## 2. Bootstrap

ROUTE.md ([§2](../ROUTE.md#2-the-bootstrap-protocol-v2)) is the bootstrap
entry, not the entire persistent database. A fresh Agent bootstraps by
**decisions**, in this order:

1. Detect the project (what is on disk).
2. Detect existing Route state (`.route/` marker) — resume, never
   re-initialize.
3. Locate/init `ROUTE_HOME` (default `Documents/Route/`).
4. Safely initialize state — no user file touched.
5. Load permitted User Memory (never project-private facts).
6. Build initial Project/Architecture Memory from inspection.
7. Establish/verify the Original baseline save.
8. Detect the Harness tier (actual capabilities).
9. Accept Intent (objective + optional method/constraints/acceptance).
10. Derive the WorkGraph.
11. Derive the minimum Agent organization.
12. Begin the managed Task (session + baseline + evidence capture).

**Invariants**: initialization is idempotent and non-destructive; detecting
existing state must not create a second, conflicting truth; a typo-scale task
must not spawn an 8-agent council (minimum-organization principle).

Intent and organization detail: [agents.md](agents.md#minimal-intent).

---

## 3. State Schema

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
  memory         # project + architecture memory (see memory.md)
  save           # savepoints, recovery metadata, KnownGood
  learning       # experience events, proposals
  evolution      # candidates, benchmarks, campaigns (experimental)
```

- **Semantic requirement**: project identity; versioned
  Constitution/Protocol/Reference; tasks with sessions and plans; evidence
  with provenance; memory; savepoints and KnownGood; learning events;
  evolution candidates/benchmarks/campaigns.
- **Reference implementation format**: how the Reference Engine writes these.
  Any format preserving the semantics and causal chain is valid.

### Causal chain

Every durable mutation links to its cause: which task, which session, which
plan, which evidence, which result. Queries reference ids/hashes, not copied
blobs.

---

## 4. Architectural Axioms

### Atomic Capability

Capabilities decompose into the smallest useful, single-responsibility
operations (26 in the canonical catalog — `observe` … `remember`). Full
catalog: [agents.md](agents.md#atomic-capability-fabric).

**CAPABILITIES ARE ATOMIC. COMPOSITIONS ARE TEMPORARY. PROJECT STATE IS
PERSISTENT.**

### Global Shared State (Blackboard)

All agents operate on one logical project truth. Agents may hold scoped
working context; agents must not build private long-term project truths.

- **Reads**: shared by default, by permission.
- **Writes**: must enter through an auditable Route mutation:

  - **Fact** — a recorded observation.
  - **Event** — a point-in-time occurrence.
  - **Evidence** — anything recorded during a session, with provenance.
  - **Proposal** — a suggested change, never auto-applied.
  - **Decision** — a recorded decision with rationale.
  - **MemoryCandidate** — a proposed memory item awaiting validation.
  - **StrategyCandidate** — a proposed strategy awaiting validation.
  - **BenchmarkCandidate** — a proposed benchmark awaiting validation.

  Each records actor, role, task/session, model/harness (if known), timestamp,
  source, provenance, and confidence when relevant.

- Physical storage may be partitioned; logically one authoritative truth.
- Shared state does **not** mean arbitrary global mutation.

---

## 5. Dynamic Agency

Agents are temporary capability compositions. The canonical **AgentSpec**
(role, goal, required_capabilities, scoped/excluded context, permissions,
expected outputs, evidence requirements, lifetime, parent_task, campaign?) and
its realization rules are specified in
[agents.md](agents.md#agentspec).

Key rules: no permanent hardcoded agents; Route manages spec-level concerns,
harness manages physical realization; no-subagent harnesses degrade to
isolated sequential phases with preserved role boundaries; harness capability
changes never change Route project identity/state.

**AGENTS MAY DIE. ROUTE REMEMBERS.**

---

## 6. Tripartite Evolution Protocol

Evolution runs on three separate planes — EXPLORE / EXECUTE / EVALUATE — a
state-centered organization, not a manager-agent hierarchy. With deliberate
divergence and organization learning. Full specification:
[agents.md](agents.md#tripartite-organization) and
[evolution.md](evolution.md).

Binding invariants:

- EXPLORE cannot mutate Stable, cannot Promote, cannot self-declare trust.
- EXECUTE obeys Intent + selected plan, minimal scope, evidence-producing;
  never substitutes its own "better" method for a stated `method_hint`;
  reports blockers instead of endless improvisation. **EXPLORE FREELY.
  EXECUTE STRICTLY.**
- EVALUATE never secretly mutates the Candidate; implementer is never sole
  evaluator; deterministic/trusted evidence outranks AI judgment.
- Route owns the final persisted promotion state. No plane self-promotes.

---

## 7. Co-Learning

Route and the AI co-evolve, but AI output is **untrusted until evaluated**.

Route → AI: context, history, constraints, strategy, reference, failure
knowledge, user preferences.

AI → Route: findings, evidence, failures, hypotheses, benchmark proposals,
strategy proposals, architecture discoveries, agent-organization outcomes.

Learning chain:

```
AI output → Proposal → Evidence → Validation
          → Replication / Reference / Benchmark (when needed)
          → Distillation → Scoped promotion
```

Only **validated** knowledge may become: an Experience, a Strategy, a
Reference-derived rule, a Benchmark, or an Agent-composition preference. This
creates co-evolution **without self-confirming hallucination**: an AI may not
mark its own output as trusted proof of success.

---

## 8. Memory and Context Compilation

The memory model (Project / Architecture / User Memory), ROUTE_HOME, scope
promotion (`TASK → PROJECT → USER → ROUTE_SHARED`), and the context
compilation axiom are specified in [memory.md](memory.md).

Key invariants:

- Every memory item keeps scope, source, provenance, confidence,
  last_verified.
- Conflicts never silently overwrite.
- Context is a compiled task view — bulk prompt injection of memory/history is
  prohibited.
- Raw history ≠ distilled memory ≠ trusted knowledge.

---

## 9. Harness Optionality

Route must be usable across three operation tiers (Tier 0 Manual AI /
Tier 1 Coding Harness / Tier 2 Advanced Harness) with the same project
identity, memory, evidence lineage, and semantics. Harness integrations are
**optional adapters**, never core dependencies. Adaptation guidance:
[harness.md](harness.md).

### Route Steward

The Steward is a **responsibility, not a permanent process**: maintain
continuity, compile context, maintain memory, organize agents, watch
evidence, preserve saves, supervise recovery, learn outcomes, prevent scope
drift, maintain Route state. Any capable AI may temporarily assume it. Detail:
[agents.md](agents.md#permissions).

---

## 10. Self-Update Protocol

ROUTE.md and Route policy may evolve, but never directly overwrite canonical
policy from a single AI suggestion.

```
Current Route → detect limitation → Candidate ROUTE.md/policy
→ evaluate against: fixed invariants, historical tasks, regressions,
  known-good cases, trusted references, bootstrap tests,
  independent evaluator
→ compare
→ promote / reject
→ retain previous KnownGood
```

Route itself keeps: Route Original, Route KnownGood, Route Candidates, Route
History — canonical Route never has only one writable copy. A Route Candidate
cannot declare itself better using benchmarks it just wrote for itself;
**Route must not be its own only judge.**

Self-update must preserve: Protocol First, Atomic Capability, Shared State,
Dynamic Agency, Co-Learning, and recoverability. Detail:
[evolution.md](evolution.md#route-self-evolution).

---

## 11. Reference Engine Boundary

The Rust implementation is an **optional Reference Engine**. It may provide:
validation, archive implementation, CLI, machine JSON, tests, stronger
recovery, benchmarking. ROUTE.md semantics must remain understandable and
usable without it.

Three enforcement levels:

| Level | What it guarantees |
|-------|--------------------|
| **Protocol Guarantee** | Semantics a compatible AI must uphold by reasoning + filesystem. |
| **Reference Engine Enforcement** | Enforced by the Rust implementation when present. |
| **Harness / OS Enforcement** | Enforced by the host environment (filesystem permissions, sandbox, process boundaries). |

Reference trust ordering:

```
hard invariant / trusted physical evidence
  > verified benchmark / trusted reference
  > independent AI evaluation
  > candidate self-report
```

**Do not claim Markdown alone provides an unbypassable security boundary.**

---

## 12. Roadmap (protocol milestones)

| Milestone | Meaning |
|-----------|---------|
| **V0.6 Beta** | Engine-heavy reference implementation + frozen Evolution integration. |
| **V0.7** | Protocol-first bootstrap — ROUTE.md alone can bootstrap a fresh AI. |
| **V0.8** (this) | **Agent-native protocol** — Atomic Capability, Shared State, Dynamic AgentSpec, Explore/Execute/Evaluate, Project/User Memory, Co-Learning, Route self-save semantics. |
| **V0.9** | Real multi-model/harness parity; self-hosting; ROUTE.md candidate evolution; long-run dogfood. |
| **V1.0** | Fresh AI + ROUTE.md = complete Route lifecycle, no mandatory executable. |

These future milestones are **not implemented**; they are direction only. See
[status.md](status.md) and [ROADMAP.md](../ROADMAP.md).
