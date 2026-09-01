# Conformance — Reality Gate

> **Purpose:** prove (or disprove) that ROUTE.md + persistent Route state
> lets a **real** AI on a **real** project take over, organize, execute,
> verify, maintain, recover, and resume — with evidence. Documentation
> phase is over; reality now attacks the protocol.
>
> **Honesty rules (absolute):**
> - Every result is one of `OBSERVED · NOT_RUN · UNSUPPORTED · FAILED ·
>   INFERRED`. **Never convert NOT_RUN into PASS.**
> - "Protocol defined" never implies "empirically observed".
> - Unavailable harness runs are **not simulated**; they stay NOT_RUN with
>   reproducible operator instructions (§5).
> - Reference Engine (`route.exe`) is optional evidence tooling, never a
>   requirement.

## 1. ConformanceRun record (P0)

Every real run is recorded in [conformance-runs/](conformance-runs/) using
these fields:

```
run_id, timestamp, route_spec_hash/reference, fixture/project_id,
project_initial_state, model_if_known, harness_if_known,
harness_capabilities, boom_available, session_id,
previous_session_available, input_instruction, observed_actions[],
route_state_created[], user_files_mutated[], agent_specs_created[],
agents_actually_realized[], degraded_roles[], evidence_refs[],
save/checkpoint_refs[], failures[], recovery_actions[], memory_updates[],
final_state, benchmark_results[], anti_check_results[], notes,
execution_provenance
```

## 2. Fixture suite (P1)

`tests/fixtures/route-conformance/` — minimal, text-based, each with a
`FIXTURE.md` manifest (initial state, expected invariants, allowed/forbidden
mutations):

| Fixture | Lifecycle state |
|---------|-----------------|
| `F1_fresh` | clean project, no `.route` |
| `F2_existing_healthy` | working multi-component project |
| `F3_legacy_messy` | confusing structure + load-bearing COMPAT constraint (**golden takeover**) |
| `F4_broken` | known reproducible defect + executable invariant check |
| `F5_repeated_failure` | symptom patch over structural issue + repair history |
| `F6_abandoned` | project + real Route state, no chat (built by RUN-006 setup) |
| `F7_partial_route` | project + deliberately staled Route state |
| `F8_boomless` | F4-equivalent task with Boom explicitly absent |

## 3. Run registry

| Run | Test | Fixture | Result | Evidence |
|-----|------|---------|--------|----------|
| RUN-000 | environment verification | — | OBSERVED (engine present; `route` name collision w/ Windows) | [RUN-000](conformance-runs/RUN-000.md) |
| RUN-001 | P2 Golden Takeover (+P3 agent reality) | F3 | OBSERVED PASS (14/14 steps; 1 real subagent spawn; COMPAT preserved) | [RUN-001](conformance-runs/RUN-001.md) |
| RUN-002 | healthy takeover + P11 memory scope | F2 | OBSERVED PASS takeover; **false-verification gap found (AMB-001)** | [RUN-002](conformance-runs/RUN-002.md) |
| RUN-003 | P6 BUG repair loop | F4 | OBSERVED PASS (minimal patch, system check evidence) | [RUN-003](conformance-runs/RUN-003.md) |
| RUN-004 | P6 REPEATED_BUG maintenance | F5 | OBSERVED PASS (StructuralDebtFinding, no auto-fix) | [RUN-004](conformance-runs/RUN-004.md) |
| RUN-005 | P7 failure/recovery | F4 | OBSERVED PASS (Failed session + rollback restore) | [RUN-005](conformance-runs/RUN-005.md) |
| RUN-006 | P8 cross-session continuity | F6 | OBSERVED PARTIAL (state semantics proven; same-model limitation) | [RUN-006](conformance-runs/RUN-006.md) |
| RUN-007 | partial/stale state takeover | F7 | OBSERVED PASS takeover; **integrity-check gap found (AMB-003)** | [RUN-007](conformance-runs/RUN-007.md) |
| RUN-008 | P5 Boom absence | F8 | OBSERVED PASS (full lifecycle, zero Boom; == RUN-003 outcome shape) | [RUN-008](conformance-runs/RUN-008.md) |

## 4. Conformance matrix (P16)

Values: `PASS · PARTIAL · FAIL · NOT_RUN · UNSUPPORTED · PROTOCOL_ONLY`.
Populated **only** from run evidence (§3); "CC/Codex/DSH" columns require
those harnesses to actually exist in the environment.

| Capability | Protocol Defined | Reference Engine | This harness (Trae+GLM) | CC | Codex | DSH | Generic | Notes |
|------------|------------------|------------------|-------------------------|----|----|-----|---------|-------|
| bootstrap | YES | PASS (init works) | PASS (RUN-001/002/008) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| safe takeover | YES | PASS | PASS (RUN-001/002/007, user files preserved) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | USER_FILES_PRESERVED=PASS |
| project reconstruction | YES | n/a | PASS (RUN-001: stale README flagged, COMPAT found) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| dynamic AgentSpec | YES | PARTIAL (`agent-plan`) | PASS (RUN-001: Explorer spec + spawn; no fan-out on trivial tasks) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| native agent execution | n/a | n/a | PASS (RUN-001: 1 subagent spawned) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| sequential degraded roles | YES | n/a | PASS (RUN-003/008: exec + engine-checked evaluation) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| Shared State | YES | PASS (`.route/` single truth) | PARTIAL (single-harness observation only) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | cross-role recovery shown RUN-006 |
| Evidence | YES | PARTIAL (CheckPass/CheckFail recorded; **end-success gate absent — AMB-001**) | PASS recording / FAIL gating (RUN-002 divergence, RUN-008 correct chain) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | FALSE_VERIFICATION_FOUND=YES (caught by AI honesty, not engine) |
| Save | YES | PASS (snapshots/checkpoints) | PASS (RUN-001/003/008 commits; RUN-005 pre-change) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| Recovery | YES | PASS (rollback works) | PASS (RUN-005: 2 rollbacks, known-good restored) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | archive `.route`-state restore still Beta |
| Project Memory | YES | PARTIAL (`route memory` derived; writes via `learn record` — AMB-004) | PASS (RUN-001/004: experience events recorded) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | no direct memory write surface |
| User Memory | YES | **Not implemented** | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | protocol-only |
| context compilation | YES | PARTIAL (`context --task`) | PARTIAL (scoped by construction; no counter-test) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| maintenance | YES | PASS (`repair-plan`; no `guardian`/`maintain` cmd in build) | PASS (RUN-004: debt finding, no auto-fix) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| repair | YES | PASS (repair + evidence chain) | PASS (RUN-003/008) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| refactor | YES | n/a | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | no run this round |
| restructure | YES | n/a | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | no run this round |
| cross-session | YES | PASS (state persists) | PARTIAL (RUN-006; same-model limitation) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | true fresh-model NOT_RUN |
| cross-harness | YES | PASS (state is host-neutral) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | no other harness in env |
| Boom absent | YES | n/a | PASS (all runs Boomless; RUN-008 explicit vs RUN-003) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| Boom integration | YES | **None** | NOT_RUN (no implementation) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | PROTOCOL_ONLY |
| uncertainty honesty | YES | n/a | PASS (RUN-001/007: UNKNOWN/STALE recorded) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | |
| integrity check | YES | **FAIL coverage** (`route check` OK after semantic-file deletion — AMB-003) | OBSERVED divergence (RUN-007) | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | new row from evidence |

## 5. NOT_RUN operator instructions (P20)

Cross-harness cells and fresh-model resumption cannot be executed in this
environment (single harness; single model instance per conversation). To
reproduce:

1. **Cross-harness (P9):** in fixture `F6_abandoned`, run
   `route handoff` with harness A's session, then open the SAME directory
   with harness B and follow the handoff doc only. Verify: same project
   identity (`route status --json`), same task/evidence lineage
   (`route task show <id>`), same KnownGood.
2. **True fresh-model resume (P8):** start a NEW conversation (no shared
   context) in `F6_abandoned`; instruction: `继续这个项目。` The AI may read
   ROUTE.md + project files + `.route/` only.
3. Record results as new ConformanceRuns; update §4 from evidence only.

## 6. Protocol ambiguities (P17)

Recorded only when evidence shows materially divergent behavior. This round:
**4 substantiated** (AMB-001 end-success gate, AMB-002 handoff pollution,
AMB-003 integrity coverage, AMB-004 memory write surface). None resolved;
no protocol text changed to hide a finding. Registry:
[conformance-runs/AMBIGUITIES.md](conformance-runs/AMBIGUITIES.md).

## 7. Status of this document (P19)

- Fixture suite: EMPIRICALLY_TESTED (8 fixtures, 8 real runs + 1 env check).
- Takeover / repair / maintenance / recovery / Boom-absence: EMPIRICALLY_TESTED
  on this harness only.
- Cross-harness (CC/Codex/DSH/Generic): NOT_RUN — no such harness in this
  environment; operator instructions in §5.
- Cross-session with a truly fresh model: NOT_RUN (single-model limitation).
- Refactor / restructure / User Memory: NOT_RUN this round.
