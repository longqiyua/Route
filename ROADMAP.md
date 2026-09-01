# Roadmap

> Short-term only. This is intentionally small — it lists what we are doing
> next, not a multi-year vision. Everything marked **Next** is a direction, not
> a commitment or an implemented capability.

## V0.6 Beta

- **Usable persistent development layer** — Route restores, records, and
  governs a project across AI sessions (`route task`, `route archive`,
  `route rollback`).
- **Recovery** — external archive saves, selective restore, and
  deleted-project recovery (`Beta`).
- **Context / Reference** — Constitution / Protocol / Reference compiled into
  host files; on-demand reference injection.
- **Harness compatibility** — `claude` / `codex` / `deepseek` / `generic`
  apply targets.
- **Experimental evolution foundation** — `route evolve`, off by default and
  gated.

## V0.7 — Protocol-First Bootstrap

- **Route Core is a protocol, not a binary.** ROUTE.md + any capable
  AI/Harness = a usable Route-managed project; the `route` executable is an
  optional Reference Engine, not the required runtime.
- **Single-file bootstrap** — ROUTE.md carries the 10-step bootstrap so a
  fresh AI can detect/init/preserve/establish Route state without repository
  source.
- **Canonical state schema** — semantic `.route/` domains separated from the
  reference implementation format.
- **Protocol reference** — `docs/protocol.md` (vendor-neutral), plus a
  reusable `docs/bootstrap-benchmark.md`.
- **Harness tiers** — Tier 0/1/2 optional adapters; adaptation guidance in
  `docs/harness.md`.

## V0.8 — Agent-Native Development Protocol

- **Agent-native protocol** — Route advances from Human↔AI to also cover
  AI↔AI coordination and AI↔Route learning
  (`ROUTE ORGANIZES. AGENTS EXECUTE. EVIDENCE DECIDES. MEMORY ACCUMULATES.`).
- **Minimal Intent** — the user states WHAT (+ optional HOW); Route derives
  the rest from Constitution/Memory/Reference/Strategy/Failure knowledge.
- **Atomic capability fabric** — 26 single-responsibility capabilities;
  agents are temporary compositions (`AGENTS MAY DIE. ROUTE REMEMBERS.`).
- **Shared State blackboard** — one logical project truth; durable writes only
  through auditable objects (Fact/Event/Evidence/Proposal/Decision/
  MemoryCandidate/StrategyCandidate/BenchmarkCandidate).
- **Dynamic AgentSpec + tripartite organization** — EXPLORE (divergent) /
  EXECUTE (strict) / EVALUATE (independent); no permanent agent hierarchy.
- **Memory model** — Project / Architecture / User Memory, ROUTE_HOME
  semantic layout, scope promotion (`TASK → PROJECT → USER → ROUTE_SHARED`),
  context compilation (`MEMORY IS PERSISTENT. CONTEXT IS COMPILED.`).
- **Route self-save semantics** — Route Original/KnownGood/Candidates/History;
  self-evolution is protocol-defined and **not** enabled by default.
- **Benchmarks** — bootstrap V2 + organization/memory/co-learning/
  self-update/game-save benchmark specs
  (`docs/bootstrap-benchmark.md`).

Engine catch-up (protocol-first, then implementation): User Memory store,
ROUTE_HOME `user/`/`route/`/`shared/`, Explore plane, full semantic
permissions. See [docs/status.md](docs/status.md) for the current matrix.

## V0.8.1 — Operational Lifecycle Protocol

- **Continuity over creation** — `ROUTE IS NOT ONLY FOR CREATION. ROUTE IS
  FOR CONTINUITY.` Existing projects (LEGACY / PARTIALLY_BROKEN /
  ABANDONED / …) are first-class; takeover follows
  observe → map → infer → verify → record.
- **Change taxonomy** — PATCH / REPAIR / REFACTOR / RESTRUCTURE / REWRITE /
  MIGRATE with no automatic escalation and minimal sufficient intervention.
- **Restructure protocol** — baseline → invariants → staged reversible
  transition; OLD → BRIDGE → NEW, never OLD → DELETE → HOPE.
- **Maintenance** — maintenance-mode findings, multi-dimensional project
  health (no single score), repair loop with StructuralDebtFinding,
  DebtRecord maintenance memory.
- **Operability honesty** — only real executed verifications count;
  NOT_RUN/UNKNOWN instead of fake PASS.
- **Deprecation & abandonment** — explicit component lifecycle states;
  abandoned takeover reconstructed from state, never old chats.
- **Governance** — responsibility boundary, ROUTE OFFICIAL/COMPATIBLE/
  DERIVATIVE/INSPIRED canonical semantics, disclaimer/compliance notice.
- Docs: [docs/maintenance.md](docs/maintenance.md),
  [docs/governance.md](docs/governance.md); canonical summary in
  [ROUTE.md §12](ROUTE.md#12-operational-lifecycle-continuity-protocol).
  Protocol-only — engine catch-up tracked in
  [docs/status.md](docs/status.md).

- **Conformance Reality Gate (observed 2026-08-16)** — first empirical
  round: fixture suite F1–F8 + 8 real runs on one harness (Trae+GLM);
  takeover/repair/maintenance/recovery/Boom-absence
  EMPIRICALLY_TESTED; cross-harness and true fresh-model resume NOT_RUN;
  4 evidence-backed engine divergences open. Evidence:
  [docs/conformance.md](docs/conformance.md),
  [docs/conformance-runs/](docs/conformance-runs/).

## Next (not yet implemented)

- **V0.9 — real multi-model/harness parity, self-hosting, ROUTE.md candidate
  evolution, long-run dogfood.**
- **V1.0 — Fresh AI + Fresh Project + ROUTE.md = complete Route lifecycle,
  without a mandatory Route executable.**
- **Native Harness / Cordis bridge** — a native integration replacing the
  generated-context-file approach. `FUTURE`, not implemented.
- **Richer machine interface** — beyond the current CLI and `--json` output.

> See [docs/status.md](docs/status.md) for the authoritative feature-status
> matrix. "Planned" here means not implemented.