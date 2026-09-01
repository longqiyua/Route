# Feature Status

> **V0.6 Beta.** This matrix is the authoritative statement of what is
> implemented, partial, experimental, or planned. It reflects the current code
> and parser. Nothing here is a design goal disguised as a feature.

## Version Metadata Note

**V0.6 Beta is the current product milestone.** Crate/package version metadata
remains unchanged (`1.0.0`) during this closure round. This is an intentional,
temporary distinction pending the user's decision at the final Release Gate —
it is **not** a silently-applied fix. See
[CHANGELOG.md](../CHANGELOG.md) and [ROUTE.md](../ROUTE.md).

## Status Legend

| Status | Meaning |
|--------|---------|
| **Stable** | Implemented, tested, and relied upon. Safe to depend on. |
| **Beta** | Implemented and usable, but may lack polish or edge hardening. |
| **Experimental** | Implemented behind explicit opt-in, off by default, or actively evolving. Behavior may change. |
| **Partial** | Implemented for some cases only. The gap is called out. |
| **Planned** | Not implemented. Roadmap intent only. |

## Feature Matrix

| Feature | Status | Notes |
|---------|--------|-------|
| `route init` | **Stable** | Creates `.route/`, config, Original archive save. |
| `route status` | **Stable** | Project overview, JSON output. |
| `commit` / `log` / `rollback` | **Stable** | Snapshot commits and rollback. |
| Archive `save/list/show/diff` | **Stable** | External saves in `Documents/Route/`. |
| Archive `restore` | **Stable** | Full / project / route-state / paths scopes. |
| Archive `recover` (deleted project) | **Beta** | Restores saved project files from the archive; does **not** restore `.route` state (re-init required, new project ID, history not carried). |
| Archive `check` | **Stable** | Invariant check. |
| Task lifecycle (`begin/start/exec/verify/end/show/resume/report/replay`) | **Stable** | Session, evidence, causal chain. |
| Verification (System evidence only) | **Stable** | Only TestPass/Commit/CheckPass accepted for `latest_verified`. |
| Constitution / Protocol | **Stable** | Three-layer context, versioned. |
| Reference registry | **Beta** | Import from markdown, skill, git, CLI help. |
| Apply targets (`claude`/`codex`/`generic`) | **Beta** | Generates host context files. |
| `deepseek` apply target | **Beta** | Writes `.route/generated/deepseek-context.md`. |
| Task-scoped context (`context --task`) | **Beta** | Deterministic lexical reference selection. |
| Memory (`memory`) | **Beta** | Project knowledge layer. |
| Learning (`learn`) | **Beta** | Experience → proposal → reference. Never auto-applied. |
| Study (`study` / `study-apply`) | **Beta** | Project analysis → reference candidates. |
| Guardian / Maintain | **Beta** | Issue detection and maintenance planning. |
| Strategy / Savepoint | **Beta** | Strategy selection; named savepoints. |
| Profile / Workflow | **Beta** | Active profile and reusable workflows. |
| Evolution (`evolve`) | **Experimental** | Candidate-first, gated. Off by default. |
| Emergence (`emerge`) | **Experimental** | Hardening on top of evolution. Off by default. |
| Campaign (`evolve campaign`) | **Experimental** | Experiment-set governance with budgets. |
| TUI (`route-tui`) | **Experimental** | Terminal UI. |
| Python bindings (`route-pyo3`) | **Experimental** | PyO3 bindings. |
| Open Development Substrate | **Beta** | Project-global revisioned `DevelopmentEvent` ledger, stable Worker identity, presence, typed WorkerMessage, shared-state projection, attached-workspace visibility, and cross-process tests. No institution runtime or social simulation. |
| `route/1` stdio/JSONL machine interface | **Beta** | Host/model-neutral capability negotiation, structured errors/receipts, durable cross-process idempotency, shared-development query/mutation methods. |
| HTTP API (`route-http`) | **Partial** | Real REST server; not a documented primary surface for V0.6. |
| Sync / plugins / packages | **Partial** | Implemented; niche, not the V0.6 focus. |
| Cross-platform (non-Windows) | **Partial** | Windows is the primary platform; others experimental. |
| Native Harness / Cordis bridge | **Planned** | Not implemented. `FUTURE`. |
| Shared work graph | **Planned** | Not implemented. The DevelopmentEvent ledger is an ordered commons, not a dependency graph or scheduler. |
| Boom frontier cognition | **Planned** | Now canonically a **Mode of Yuich** (independent private repo `longqiyua/Yuich`); historical spec in [boom.md](boom.md) (VNEXT: §29 SLL-I + §30 SLL-II + §31 Design Closure frozen + §32 ISM). Optional, hot-pluggable; no engine surface, no runtime. **MRS** mapped onto existing engine (`evolve`/`learn`/`task`) in [mrs.md](mrs.md); First Real Boom Cycle = OBSERVED_PASS (gate denied neutral candidate). |
| Yuich — Persistent Artificial Subject | **Planned** | Protocol-only spec in the independent private repo `longqiyua/Yuich`: Subject/Prime/Memory/SelfModel/Learning + Cognition (ISM, Operators, BoomMode) + Capabilities (`yuich.route`/life/research/creative) + ModelGateway + ToolGateway. Tool × Capability duality: Route = external tool; `yuich.route` = internal capability; neither depends on the other. Dogfoods A–J NOT_RUN. |
| Richer machine interface beyond `route/1` | **Planned** | `route/1` stdio/JSONL is implemented; HTTP/MCP parity and future protocol majors are not implied. |

## Platforms

| Platform | Status |
|----------|--------|
| Windows | **Stable** (primary) |
| Linux / macOS | **Partial** (experimental) |

## CORE FREEZE

**No new V0.6 Beta core features after this gate.**

Only release-blocking correctness and documentation fixes are accepted from
this point. Stable / Beta / Experimental labels above remain authoritative and
visible. No additional subsystem is added in this round.

## Evolution V0.6 Freeze

**Evolution Campaign is EXPERIMENTAL but end-to-end integrated** (Task →
Campaign → NextAction → external Harness → Evidence → Campaign evaluation →
Task continuation).

**No further V0.6 Evolution features.** Next Evolution development must be
driven by real dogfood findings from using this integrated surface, not by new
design proposals.

## V0.8 Agent-Native Protocol

**V0.8 is a protocol milestone, not a new runtime.** It defines the
agent-native protocol — Atomic Capability, Shared State, Dynamic AgentSpec,
Explore/Execute/Evaluate, Project/User Memory, Co-Learning, Route self-save —
in [ROUTE.md Part I](../ROUTE.md#part-i--route-protocol),
[docs/agents.md](agents.md), and [docs/memory.md](memory.md). No LLM runtime,
scheduler, harness, HTTP, database, or GUI was added.

| Protocol layer (V0.8) | Doc status | Engine implementation |
|-----------------------|------------|----------------------|
| Core Creed / Route Core definition | Documented | n/a (protocol) |
| Minimal Intent (`objective` + optional HOW) | Documented (agents.md) | Partial — `route plan` / `route agent-plan` cover analytical derivation; no `route intent` command |
| Auto organization (minimum-complexity) | Documented (agents.md) | Partial — deterministic plan compiler exists; no Explore plane |
| Atomic capability fabric (26 capabilities) | Documented (agents.md) | Partial — `route capability` is a Reference-registry view, not the full catalog |
| Global Shared State / auditable write objects | Documented (protocol.md) | Partial — evidence/ledger objects exist; MemoryCandidate/StrategyCandidate/BenchmarkCandidate as *named* write kinds are protocol-level |
| AgentSpec (dynamic, temporary) | Documented (agents.md) | Beta — `agent_compiler` AgentSpec/AgentPlan + `route agent-plan` (no execution) |
| Route creates/manages agents | Documented (agents.md) | Partial — plan compilation only; engine never spawns agents |
| Tripartite EXPLORE/EXECUTE/EVALUATE | Documented (agents.md, evolution.md) | Partial — Evaluate exists (`evolve evaluate`); Explore plane / N-explorer divergence **not implemented** |
| Strict execution rules (method_hint etc.) | Documented (agents.md) | Protocol only |
| Deliberate divergence | Documented (agents.md) | **Not implemented** |
| Organization learning | Documented (agents.md) | Beta — `route agent-org` records/recalls organization experience |
| Project Memory | Documented (memory.md) | Beta — `route memory` (persisted per-project) |
| Architecture Memory | Documented (memory.md) | Partial — subset fields + knowledge map |
| User Memory | Documented (memory.md) | **Planned — not implemented** |
| ROUTE_HOME `user/` `route/` `shared/` | Documented (memory.md) | **Planned** — archive has `projects/<id>/` + `registry.json` only |
| Memory scope promotion (TASK→PROJECT→USER→SHARED) | Documented (memory.md) | Partial — trajectory cross-project transform exists; no user-level store |
| Context compilation | Documented (memory.md) | Beta — task-scoped context assembly (`context --task`, `brain for --task`) |
| Co-Learning gate | Documented (protocol.md) | Beta — `route learn` proposals, never auto-applied |
| Route self-save / self-evolution | Documented (evolution.md) | Protocol only — autonomous self-modification NOT enabled by default |
| Semantic permissions | Documented (agents.md) | Partial — `route permission` (`normal|high`) + string permissions; full semantic set is protocol-level |
| Route Steward | Documented (harness.md) | Protocol only — a responsibility, not a process |
| Bootstrap V2 (12 steps) | Documented (ROUTE.md §2) | Spec — benchmark defined |
| Benchmarks (org/memory/co-learning/self-update/game-save) | Spec (bootstrap-benchmark.md) | **Not automated** — reusable specs for external evaluators |
| Operational lifecycle (continuity) | Documented (ROUTE.md §12, maintenance.md) | Partial — guardian/maintain/repair-plan/memory cover fragments; ProblemModel/taxonomy/deprecation/health protocol-only |
| Existing-project takeover + ProblemModel/Architecture reconstruction | Documented (maintenance.md §1–3) | **Protocol only — not implemented** |
| Change taxonomy + restructure protocol (OLD→BRIDGE→NEW) | Documented (maintenance.md §4–5) | **Protocol only** |
| Maintenance mode + Maintenance Memory (DebtRecord) | Documented (maintenance.md §7, §10) | Partial — `guardian`/`maintain` detect issues; DebtRecord shape not persisted |
| Project health (multi-dimensional, no single score) | Documented (maintenance.md §8) | **Protocol only** |
| Operability honesty (NOT_RUN/UNKNOWN) | Documented (maintenance.md §11) | Aligned with existing evidence policy; no new engine surface |
| Deprecation/retirement states + abandoned takeover | Documented (maintenance.md §13–14) | Partial — deleted-project file recovery is Beta; state machine protocol-only |
| User intent semantics + consequence levels (L0–L4) | Documented (maintenance.md §15–16) | Partial — `permission normal\|high` is coarse; L-levels protocol-only |
| Boom frontier cognition (optional, hot-pluggable) | Spec VNEXT (boom.md) | **Planned — no engine surface** |
| Governance: responsibility boundary + canonical semantics + notice | Documented (governance.md) | Notice lives in docs only; engine adds nothing new |
| Lifecycle benchmarks (13 specs incl. FALSE_VERIFICATION) | Spec (maintenance.md §18) | **Not automated** — reusable specs for external evaluators |

**No V0.6 Evolution algorithms, no new runtime features were added.**

## Conformance evidence (Reality Gate, 2026-08-16)

Claim discipline: `SPECIFIED` ≠ `REFERENCE_IMPLEMENTED` ≠
`EMPIRICALLY_TESTED` ≠ `CROSS_HARNESS_TESTED`. Full matrix and run evidence:
[conformance.md](conformance.md) + [conformance-runs/](conformance-runs/).

| Capability | Protocol | Reference Engine | Empirically tested (this harness) | Cross-harness |
|------------|----------|------------------|-----------------------------------|---------------|
| Takeover (fresh/legacy/broken/partial) | SPECIFIED | REFERENCE_IMPLEMENTED | EMPIRICALLY_TESTED (RUN-001/002/003/007) | NOT_RUN |
| Repair + evidence chain | SPECIFIED | REFERENCE_IMPLEMENTED | EMPIRICALLY_TESTED (RUN-003/008) | NOT_RUN |
| Maintenance (debt finding) | SPECIFIED | REFERENCE_IMPLEMENTED | EMPIRICALLY_TESTED (RUN-004) | NOT_RUN |
| Failure + rollback recovery | SPECIFIED | REFERENCE_IMPLEMENTED | EMPIRICALLY_TESTED (RUN-005) | NOT_RUN |
| Cross-session continuity | SPECIFIED | REFERENCE_IMPLEMENTED | EMPIRICALLY_TESTED, same-model only (RUN-006) | NOT_RUN |
| Boom absence (first-class) | SPECIFIED | n/a | EMPIRICALLY_TESTED (RUN-008) | NOT_RUN |
| Refactor / restructure lifecycle | SPECIFIED | partial tooling | NOT_RUN | NOT_RUN |
| User Memory | SPECIFIED | NOT_IMPLEMENTED | NOT_RUN | NOT_RUN |

KNOWN_LIMITATION (engine, evidence-backed): `task end --result success` is
not gated on passing system evidence (AMB-001); `route check` does not cover
`.route/` semantic domains (AMB-003). See
[conformance-runs/AMBIGUITIES.md](conformance-runs/AMBIGUITIES.md).

## V0.7 Protocol-First Bootstrap

**V0.7 is a protocolization milestone, not a new runtime.** It strengthens the
canonical guide so that **ROUTE.md + any capable AI/Harness = a usable
Route-managed project**, without requiring the `route` executable.

| Protocol layer | Status | Notes |
|----------------|--------|-------|
| Route Core definition (persistent protocol/state/governance) | **Documented** | [ROUTE.md](../ROUTE.md#part-i--route-protocol); not an LLM/agent/harness/executable. |
| Single-file bootstrap | **Documented** | [ROUTE.md §2](../ROUTE.md#2-the-bootstrap-protocol-v2) (V0.7: 10 steps; V0.8: 12 steps). |
| Canonical state schema (semantic vs reference format) | **Documented** | [docs/protocol.md](protocol.md#3-state-schema). |
| Atomic capability axiom + shared state | **Documented** | [ROUTE.md §4](../ROUTE.md#4-architectural-axioms). |
| Dynamic AgentSpec | **Documented** | [ROUTE.md §5](../ROUTE.md#5-dynamic-agency). |
| Tripartite evolution protocol | **Documented** | [ROUTE.md §6](../ROUTE.md#6-tripartite-organization). |
| Co-learning | **Documented** | [ROUTE.md §8](../ROUTE.md#8-co-learning). |
| Harness optionality (Tier 0/1/2) + adaptation | **Documented** | [docs/harness.md](harness.md). |
| Self-update protocol | **Documented** | [ROUTE.md §11](../ROUTE.md#11-save-self-save-and-self-evolution). |
| Reference Engine boundary | **Documented** | [ROUTE.md Part II intro](../ROUTE.md#part-ii--reference-engine). |
| Bootstrap benchmark | **Spec** | [docs/bootstrap-benchmark.md](bootstrap-benchmark.md). Reusable against different models/harnesses. |
| Protocol reference | **Documented** | [docs/protocol.md](protocol.md). |

**No new V0.6 Evolution features were added in V0.7.** The Rust
implementation remains the reference implementation, validator, benchmark
oracle, and compatibility implementation — it is **not** the required product
runtime.

## Roadmap (V0.6 → V1.0)

These are **direction only, not implemented**:

| Milestone | Meaning |
|-----------|---------|
| **V0.6 Beta** | Engine-heavy reference implementation + frozen Evolution integration. |
| **V0.7** | Protocol-first bootstrap. |
| **V0.8** (this) | **Agent-native protocol** — Atomic Capability, Shared State, Dynamic AgentSpec, Explore/Execute/Evaluate, Project/User Memory, Co-Learning, Route self-save semantics. |
| **V0.9** | Real multi-model/harness parity; self-hosting; ROUTE.md candidate evolution; long-run dogfood. |
| **V1.0** | Fresh AI + ROUTE.md = complete Route lifecycle, no mandatory executable. |
