# Agents — Agent-Native Reference

> **V0.8.** The deep reference for the agent-native protocol defined in
> [ROUTE.md Part I](../ROUTE.md#part-i--route-protocol). Vendor-neutral: no
> executable, harness, or model is required. The Reference Engine mapping at
> the bottom states exactly what the Rust implementation provides today —
> nothing here claims unimplemented runtime behavior.

Canonical creed:

```
ROUTE ORGANIZES.  AGENTS EXECUTE.  EVIDENCE DECIDES.  MEMORY ACCUMULATES.
CAPABILITIES ARE ATOMIC.  COMPOSITIONS ARE TEMPORARY.
PROJECT STATE IS SHARED.  MEMORY IS PERSISTENT.
AGENTS MAY DIE. ROUTE REMEMBERS.
```

---

## Minimal Intent

The Human / upstream AI only needs to state:

| Field | Required | Meaning |
|-------|----------|---------|
| `objective` | yes | WHAT must become true. A goal, not a task list. |
| `method_hint` | no | HOW the user prefers it done. |
| `hard_constraints` | no | invariants that must hold. |
| `acceptance` | no | what counts as done. |

Rules:

1. `objective` must become true — it is the contract.
2. If `method_hint` exists and does not violate higher constraints
   (Constitution, hard constraints), the Executor **must not casually rewrite
   it** — not even for a "better" approach. Changing a stated method requires
   reporting back, not silent substitution.
3. When HOW is missing, Route derives it from: Constitution → Project Memory →
   User Memory → Reference → Strategy → Failure knowledge → Benchmarks.
4. Route asks the user **only** for genuinely user-owned choices that cannot
   be resolved from existing state. Every user question should be
   traceable to "this cannot be derived and is not safely defaultable."

### Intent → organization

```
Intent
  → Goal
  → WorkGraph            (verifiable work units)
  → required atomic capabilities
  → AgentSpec(s)         (minimum viable organization)
  → execution / evaluation flow
```

Route classifies the work — trivial / simple / complex / experimental — and
decides: is Explore needed? an independent Evaluator? checkpoint scope? which
context each agent reads or is excluded from? required evidence? verification
and recovery boundary?

**Minimum-organization principle**: use the smallest organizational complexity
that completes the task. Do not create an 8-agent council for a typo.

---

## Atomic Capability Fabric

Capabilities are the smallest useful, single-responsibility operations:

| Capability | Responsibility |
|-----------|----------------|
| `observe` | notice and surface current state |
| `inspect` | examine a specific artifact in depth |
| `query` | ask a question of Route state |
| `retrieve` | fetch an external reference/resource |
| `record` | persist a Fact/Event |
| `classify` | categorize an artifact/finding |
| `compare` | diff two states/artifacts |
| `checkpoint` | create a recovery point |
| `save` | persist project state + Route state |
| `hash` | content-identify an artifact |
| `restore` | bring saved state back |
| `verify` | check a claim against policy |
| `test` | run deterministic checks |
| `propose` | submit a Proposal/Candidate |
| `diverge` | generate deliberately different alternatives |
| `compose_agent` | assemble a temporary AgentSpec |
| `execute` | perform the actual work |
| `report` | produce an outcome record |
| `evaluate` | independently judge a Candidate |
| `score` | quantify against a benchmark |
| `replicate` | reproduce a prior result |
| `ablate` | remove parts to test necessity |
| `promote` | persist acceptance (Route-owned) |
| `reject` | persist refusal (Route-owned) |
| `distill` | compress raw history into memory |
| `remember` | store validated knowledge |

Rules for every capability:

- one primary responsibility
- explicit inputs, explicit outputs
- composable with other capabilities
- **owns no authoritative project truth**

Agents, workflows, and "phases" are **temporary compositions** of these
capabilities for the duration of a task/experiment/campaign. Compositions die;
state persists.

---

## AgentSpec

Agents are temporary capability compositions, not permanent personalities.

```
AgentSpec {
  id
  role                       // what it is for
  goal                       // what it must achieve
  required_capabilities[]    // atomic capabilities (see catalog)
  scoped_context[]           // what it may read
  excluded_context[]         // what it must NOT read
  permissions                // semantic permissions (see Permissions)
  expected_outputs[]         // what it should produce
  evidence_requirements[]    // what evidence it must return
  lifetime                   // single_action | task | experiment | campaign
  parent_task
  campaign?                  // when serving an evolution campaign
}
```

Agent = Role + Capabilities + Context + Permissions + Goal.

**No permanent hardcoded agents.** `CoderAgent` / `ReviewerAgent` /
`ArchitectAgent` as fixed entities are prohibited. Role **templates** with
statistics are fine — they inform future compositions, they are not
personalities.

### Lifetimes

| Lifetime | Scope of existence |
|----------|--------------------|
| `single_action` | one atomic operation |
| `task` | one managed task |
| `experiment` | one evolution experiment |
| `campaign` | one campaign |

When the task ends, the agent may vanish. What persists: Evidence, Findings,
Failures, Decisions, Strategy, Memory, Lineage.

### Route creates and manages agents

Route owns, for each AgentSpec: why the agent exists, goal, capabilities,
context, permissions, outputs, evidence requirements, lifetime.

The harness owns: model invocation, subagent spawn, tool execution, sandbox,
concurrency/runtime.

Realization:
- Harness with subagents → AgentSpec → native agent.
- Harness without subagents → degrade to **isolated sequential phases** of a
  single agent, preserving role boundaries (Explorer phase, Executor phase,
  Evaluator phase — distinct context, distinct permissions).
- Harness capability changes never change Route project identity or state.

---

## Tripartite Organization

Three planes organize non-trivial work — a **state-centered** organization
(all roles collaborate through Shared Route State), not a
Manager→TeamLead→Worker bureaucracy.

### EXPLORE — maximize the possibility space

On complex/uncertain tasks, N independent Explorers:

- produce Hypotheses
- reflect on current assumptions
- propose alternative/radical architectures
- propose adversarial cases
- propose BenchmarkCandidates
- exploit model stochasticity to stay different

Thought space may go beyond the current architecture.

Binding constraints: **no Stable mutation, no Promote, no self-declared
trust.**

### EXECUTE — implement the selected hypothesis, strictly

The Executor must:

- obey the Intent (including `method_hint`)
- follow the selected plan
- minimal scope — no unrelated refactors, no requirement invention
- be evidence-producing
- never replace a user-requested method with a "better" one
- when an assumption fails: **report the blocker + evidence**, do not
  improvise endlessly

**EXPLORE FREELY. EXECUTE STRICTLY.**

On a major new situation during execution: report → Route decides
retry / repair / explore-again / ask-user.

### EVALUATE — decide what is worth keeping

The Evaluator:

- independently reviews Candidate / Evidence / Benchmark / Reference
- does not secretly modify the Candidate
- is never the sole evaluator of work it implemented
- ranks deterministic/trusted evidence above AI judgment

High-risk changes may use M independent evaluators.

Final Promote/Reject state is persisted by **Route**, never by a plane.

### Deliberate divergence

Explore must actively exploit probabilistic difference, not average toward a
single mean answer. Dimensions of divergence:

- different models
- different framings of the problem
- different Reference subsets
- opposite assumptions
- radical vs conservative alternatives
- architecture inversion
- constraint-removal thought experiments

Route stores **quality and novelty**. Do not collapse the search space to
Top-1 prematurely.

### Organization learning

Route learns which task patterns suit which organization structures. Recorded
per outcome:

```
task_pattern · AgentSpec topology · capabilities · model/harness
latency/cost (if available) · success · rollback · benchmark · user acceptance
```

A **single success only produces a StrategyCandidate** — never a permanent
organization rule. Promotion follows the normal co-learning gate.

---

## Permissions

Harness-neutral **semantic permissions**. The harness only maps them onto
actual capabilities; it does not redefine them.

```
read_project          write_candidate       write_project
read_route_state      propose_route_state   mutate_route_state
read_user_memory      propose_user_memory   read_reference
execute_shell         run_tests             evaluate
promote
```

Role defaults:

| Role | Reads | Writes | Explicitly denied |
|------|-------|--------|-------------------|
| **Explorer** | broad (project, route state, references, permitted user memory) | Hypothesis / Proposal | Stable mutation; Promote |
| **Executor** | scoped task context | Candidate / task scope | self-Promote; expanding own permissions |
| **Evaluator** | Candidate / Evidence / Reference | evaluation records | Candidate mutation; being sole judge of own implementation |
| **Steward** (temporary) | full Route state | audited Route mutations | silent policy change; bypassing evidence gates |

Notes:

- `mutate_route_state` is normally reserved for the Steward role under audit.
- `propose_user_memory` — anyone may propose; promotion into User Memory
  follows the memory gate (see [memory.md](memory.md#memory-scope-and-promotion)).
- `promote` — Route-owned final state change, gated by evaluation evidence.

### Route Steward

The Steward is a **responsibility, not a permanent process**:

- maintain continuity
- compile context
- maintain memory
- organize agents
- watch evidence
- preserve saves
- supervise recovery
- learn outcomes
- prevent scope drift
- maintain Route state

Any capable AI may temporarily assume it; when it stops, nothing is lost
because state, evidence, and memory persist in Shared Route State.

---

## Reference Engine Surface

What the Rust Reference Engine provides **today** for this protocol
(analytical only — the engine never runs agents):

| Protocol concept | Engine surface | Status |
|------------------|----------------|--------|
| AgentSpec / AgentPlan | `route agent-plan` (plan compilation + host rendering: claude/codex/deepseek/generic) | Implemented (deterministic compiler, no execution) |
| Counterfactual planning | `route plan` (+ `--compare` between strategies) | Implemented |
| Role templates (statistics) | `route agents templates/show/record-success/record-failure` | Implemented |
| Organization experience | `route agent-org history/explain/recall` | Implemented |
| Capability view | `route capability list/show/inspect` (view over Reference registry) | Implemented |
| Explore plane / N-explorer divergence | — | **Protocol only** (not implemented in engine) |
| Semantic permission set above | `route permission status/set` (`normal|high`) + string permissions in AgentSpec | **Partial** (engine has a coarse level, not the full semantic set) |
| `compose_agent` capability | `route agent-plan` covers plan compilation; no standalone `compose` command | Partial |

The protocol remains fully usable without any of these engine surfaces: a
compatible AI composes AgentSpecs by reasoning over ROUTE.md and Shared Route
State directly.