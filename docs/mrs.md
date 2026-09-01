# Boom Self-Learning MRS — Implementation Map & Status

> **Design frozen (SELF_LEARNING_DESIGN=FROZEN).** This file records how the
> frozen Minimum Runnable Subset (§31.11) maps onto existing engine surfaces
> and what has been empirically exercised. It does **not** add protocol.
> Source of truth for Boom semantics remains [boom.md](boom.md) §29–§31.
> Status vocabulary follows §31.17 RunStatus.

## Mapping principle (§31.12)

No parallel state system. Every MRS capability maps onto existing engine
records: `task` (ExecutionSession), `evolve` (candidate lifecycle + gated
promote), `learn record` (experience event / LearningEvent), `evolve
campaign`, `commit/rollback` (Save/KnownGood). ONE FACT → ONE LINEAGE;
derived views may multiply.

## MRS capability → mapping → status

| # | MRS MUST capability | Engine mapping | Status |
|---|--------------------|----------------|--------|
| 1 | Mutation lifecycle (Candidate→Experimental→Shadow/Canary→Provisional→Stable; promote/reject/rollback) | `evolve propose` (status=proposed) → `evolve evaluate` → `evolve promote/reject` (gated) | IMPLEMENTED (engine) + FIXTURE_TESTED (RUN-BC1) |
| 2 | Minimal BetterClaim (baseline/intervention/dimensions/evidence/rollback) | `evolve propose --baseline-id/--candidate-id/--hypothesis/--rationale/--reversible-save-id/--changed-scope` | IMPLEMENTED + FIXTURE_TESTED (RUN-BC1) |
| 3 | EvidenceStrength E0–E5 | `evolve evaluate` decision (neutral/improve/regress) + `--metric` measured evidence; `learn record` kinds | IMPLEMENTED + FIXTURE_TESTED (E1 via learn; gate decision) |
| 4 | real Evidence refs + no-fake-PASS | promotion gate DENIED on 'neutral' | IMPLEMENTED + FIXTURE_TESTED (RUN-BC1) |
| 5 | MutationTrial baseline/candidate | `evolve propose` baseline/candidate/save refs + `evolve evaluate` | IMPLEMENTED + FIXTURE_TESTED |
| 6 | LearningEvent (evidence/confidence/context/horizon) | `learn record -k/-s/-p/-t/-e` | IMPLEMENTED + FIXTURE_TESTED (2 events) |
| 7 | incremental Credit status | `learn` status/downgrade/supersede/reject; `evolve` decision | PARTIAL (states exist; fine-grained windows MAY) |
| 8 | EvidenceDebt + hard debt priority | derived view over `evolve`/`learn` state; no engine command | SPECIFIED (derived view) — NOT_RUN automated |
| 9 | DeferredEvaluation TTL | pending evaluation metadata; opportunistic on resume | SPECIFIED — NOT_RUN (no long horizon yet) |
| 10 | Provisional learning + B→C | `evolve` candidate + `learn` repetition | SPECIFIED — NOT_RUN (needs ≥3 real episodes) |
| 11 | Mandatory PrimeChallenger gate | `evolve promote` gate (Engine policy) | IMPLEMENTED (gate) + FIXTURE_TESTED; challenger report = protocol |
| 12 | Stable fallback + rollback | `commit`/`rollback` + `--reversible-save-id` | IMPLEMENTED + FIXTURE_TESTED |
| 13 | minimal Episode/Exposure | `task` ExecutionSession + `evolve --task-id/--session-id` | IMPLEMENTED (carrier) — cross-episode NOT_RUN |
| 14 | Evidence-backed SelfModel derived view | derived from `evolve history` + `learn status` | SPECIFIED — PARTIAL (derivable, no dedicated cmd) |
| 15 | FrictionSignal + SchemaPrune gate | `learn record` maintenance observation + manual C9 check | SPECIFIED — PARTIAL |

## Real runs (this round)

### RUN-BC1 — First Real Boom Cycle (OBSERVED_PASS)

Fixture: `tests/fixtures/route-boom-mrs/` (inventory validator, baseline
committed `01M04HCT`).

1. `learn record` E1 baseline observation → event `01M04HD2...` ✓
2. Created candidate `validator-candidate.txt` (sorted-aware mutation).
3. `evolve propose` → experiment `01M04HDG2ME5...`, status=proposed,
   baseline=`01M04HCT`, candidate=`01M04HDK`, reversible-save=`01M04HCT` ✓
4. `evolve evaluate ... --metric defect_detection=0:0:1` → **decision: neutral,
   "no measurable improvement"** ✓
5. `evolve promote --label sorted-validator-v1` → **gate DENIED under
   default-promotion-policy** ✓ (no-fake-PASS invariant holds)
6. `learn record` agent_result capturing the denial → event `01M04HE4...` ✓

Observed: mutation lifecycle, BetterClaim fields, EvidenceStrength gate,
no-fake-PASS, LearningEvent, rollback anchor, incremental credit (neutral).
This is REAL engine evidence (OBSERVED_PASS), not simulated.

### RUN-BC2 — gate enforcement (OBSERVED_PASS, part of BC1)

Promotion gate correctly refused a neutral candidate; candidate did not
self-promote; stable baseline untouched. Confirms MRS #11 (mandatory gate) at
the engine level.

## Dogfood status summary

| Dogfood | Status | Note |
|---------|--------|------|
| A MultiDebt | SIMULATED_FIXTURE / NOT_RUN | debt-priority rule is a derived view; no engine debt governor; no multi-debt fixture executed |
| B DeferredZombie | NOT_RUN | no long-horizon episode existed; TTL logic is protocol |
| C MandatoryChallenger | PARTIAL | promotion gate enforced (OBSERVED); challenger report/response is protocol-only |
| D ColdStart | NOT_RUN | requires ≥3 real AI episodes; this fixture is single-task |
| E SchemaPrune | NOT_RUN | no schema prune attempted; C9 gate is protocol |
| F ContinuousCredit | NOT_RUN | single episode; cross-episode exposure not exercised |
| G PrimeBiasGovernor | NOT_RUN | no bias history; SelfModel is derived view only |

## Evidence-driven protocol changes this round

- **NONE.** Added §31.17 RunStatus and §31.18 DesignReopenGate only because
  they were **missing from the frozen closure** (item 0 allows adding
  omitted frozen pieces; not a new mechanism).

## Status matrix (per §22)

| Capability | Specified | Implemented | Fixture-tested | Empirically observed |
|-----------|-----------|-------------|----------------|-----------------------|
| Mutation lifecycle | YES | YES (engine) | YES (BC1) | YES (BC1) |
| BetterClaim | YES | YES (engine fields) | YES | YES |
| EvidenceStrength E0–E5 | YES | PARTIAL (decision+metric) | YES | YES (neutral) |
| no-fake-PASS gate | YES | YES | YES | YES (denied) |
| LearningEvent | YES | YES | YES | YES |
| Incremental Credit | YES | PARTIAL | PARTIAL | NOT_RUN (long) |
| EvidenceDebt+Governor | YES | NO | NOT_RUN | NOT_RUN |
| DeferredEvaluation TTL | YES | NO | NOT_RUN | NOT_RUN |
| Cold start B→C | YES | NO | NOT_RUN | NOT_RUN |
| Mandatory Challenger | YES | YES (gate) | YES | YES (gate) |
| Rollback/fallback | YES | YES | YES | YES (ref required) |
| Episode/Exposure | YES | PARTIAL (carrier) | NOT_RUN | NOT_RUN |
| SelfModel derived | YES | PARTIAL | NOT_RUN | NOT_RUN |
| FrictionSignal/Prune | YES | PARTIAL | NOT_RUN | NOT_RUN |