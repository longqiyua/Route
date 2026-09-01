# Maintenance — Operational Lifecycle Protocol

> **V0.8 lifecycle spec (protocol-only unless stated).** Route is a
> **long-term project continuity protocol, not a one-shot code generator**.
> This document extends Route from "can create" to the full engineering
> lifecycle: **understand, repair, refactor, restructure, maintain, migrate,
> deprecate, retire, recover**. Canonical summary:
> [ROUTE.md §12](../ROUTE.md#12-operational-lifecycle-continuity-protocol).
> Vendor-neutral — no executable required.

Core principle:

```
ROUTE IS NOT ONLY FOR CREATION. ROUTE IS FOR CONTINUITY.
UNDERSTAND BEFORE REWRITE.
PATCH WHEN PATCH IS ENOUGH.
RESTRUCTURE WHEN STRUCTURE IS THE PROBLEM.
UGLY != WRONG.  OLD != BAD.  NEW != BETTER.
PRESERVE WORKING VALUE.  MAKE CHANGE REVERSIBLE.  VERIFY BEFORE PROMOTION.
AI MAY LEAVE.  HARNESS MAY CHANGE.  PROJECT CONTINUITY MUST REMAIN.
```

---

## 1. Existing Projects Are First-Class

Route supports the full range of project states — not only greenfield:

```
EMPTY · NEW · HEALTHY_EXISTING · LEGACY · PARTIALLY_BROKEN ·
UNKNOWN_ARCHITECTURE · AI_GENERATED_MESS · ABANDONED · MIGRATING · RECOVERED
```

When taking over an existing project:

- **No prior judgment** that the architecture is "right" or "wrong" on
  first contact.
- Path: `observe → map → infer → verify → record`, then choose an
  intervention: `preserve / patch / repair / refactor / restructure /
  rewrite / migrate / deprecate`.
- **Never rewrite because "the code is ugly".** Ugliness is not evidence of
  a defect (see Change Taxonomy, §4).

## 2. Problem Reconstruction

For any problem in an existing system, build a **ProblemModel** before
intervening:

```
ProblemModel {
  observed_symptoms        // what is actually seen
  affected_scope           // blast radius of the symptom
  current_architecture     // relevant structure (as far as known)
  relevant_history         // prior fixes, failures, workarounds
  suspected_causes[]      // hypotheses — UNVERIFIED
  verified_causes[]       // evidence-backed only
  constraints[]           // must hold during/after intervention
  invariants[]            // must-not-break
  technical_debt[]        // known accepted debt in the area
  user_intent             // what the user actually wants to become true
  candidate_interventions[]
  recovery_boundary        // what we can safely roll back to
}
```

Rules:

- **Symptom ≠ Cause.** Speculation enters `suspected` only; it moves to
  `verified` only with sufficient evidence.
- Basic reasoning path:
  `ObservedState → HistoricalCause (if knowable) → CurrentConstraint →
  Failure/Pressure → DesiredState → SafeTransition`.

## 3. Architecture Reconstruction

When project structure is unclear, progressively maintain **Architecture
Memory** (see [memory.md](memory.md)):

```
Components · Responsibilities · Dependencies · Interfaces · DataFlow ·
StateOwnership · Invariants · CriticalPaths · ExternalDependencies ·
KnownRisks · HistoricalDecisions
```

Every entry carries an annotation + provenance:

| Annotation | Meaning |
|------------|---------|
| `OBSERVED` | directly seen in the code/artifacts |
| `INFERRED` | concluded by reasoning — **never treat as hard truth** |
| `USER_STATED` | asserted by the user |
| `VERIFIED` | checked against evidence |
| `STALE` | may no longer hold |
| `CONFLICTED` | conflicting sources exist |

**Inferred must not be recorded as hard truth.** Verified structural changes
must update Architecture Memory.

## 4. Change Taxonomy

Strictly distinguish intervention kinds — no silent escalation:

| Kind | Definition |
|------|------------|
| **PATCH** | minimal local correction |
| **REPAIR** | restore intended behavior |
| **REFACTOR** | preserve external contract while changing internals |
| **RESTRUCTURE** | change boundaries / responsibilities / relationships |
| **REWRITE** | replace substantial implementation |
| **MIGRATE** | move format / platform / architecture preserving required continuity |

Rules:

- **No automatic escalation** bug → refactor → rewrite.
- Default to the **minimal sufficient intervention** that meets the
  objective.
- A larger intervention requires an explicit justification (BetterClaim /
  ProblemModel evidence), not taste.

## 5. Restructure Protocol

Before any major structural change, the following are **MUST** steps:

```
baseline → identify invariants → capture KnownGood behavior
→ map dependencies → define target architecture
→ define staged reversible transition
→ checkpoint/save → mutate Candidate
→ verify each stage → compare baseline
→ promote only with evidence
```

- Large migrations prefer **OLD → BRIDGE → NEW**. `OLD → DELETE → HOPE` is
  prohibited.
- Before any wide-range deletion/replacement, establish a recoverable state
  (save/checkpoint).

## 6. Preservation

Existing projects contain accumulated value. Default-protect:

- working behavior
- user workflow
- data compatibility
- public API
- integrations
- performance characteristics
- **valuable ugly code** (code that is ugly AND load-bearing)
- historical knowledge

Any **destructive simplification** must be backed by explicit user intent or
an evidence-backed BetterClaim ([boom.md §15](boom.md#15-betterclaim-b13) —
and BetterClaims from Route itself follow the same semantics). **Deletion is
not the default meaning of "cleanup".**

## 7. Maintenance Mode

At meaningful task boundaries or on maintenance requests, Route checks:

```
broken assumptions · stale architecture · dependency drift ·
repeated failures · duplicate logic · dead interfaces ·
unverified workarounds · technical debt · doc drift ·
test blindspots · recovery fragility · context inflation ·
obsolete references
```

Outputs (auditable objects, never silent changes):

```
MaintenanceFinding · MaintenanceProposal · RefactorCandidate ·
DeprecationCandidate · RegressionBenchmark · MemoryCorrection
```

**Finding a problem ≠ fixing it.** Findings enter Shared Route State as
proposals; intervention follows the normal lifecycle with user-visible
gates.

## 8. Project Health

**No fabricated single Health Score.** Track dimensions separately; each may
be `UNKNOWN`:

```
Correctness · Recoverability · Understandability · Maintainability ·
Compatibility · TestCoverage (if meaningful) · OperationalRisk ·
ArchitecturalCoherence · EvidenceQuality · DependencyRisk ·
KnowledgeFreshness
```

Unknown is better than false precision.

## 9. Repair Loop

```
Failure
→ preserve evidence
→ reproduce (if possible)
→ localize
→ classify (transient · regression · design flaw · dependency ·
            state corruption · requirement mismatch · unknown)
→ CandidateRepair
→ verify
→ regression check
→ learn
```

When the **same area/pattern** needs repeated repair, emit a
**StructuralDebtFinding** — only then consider refactor/restructure. Repair
loops are evidence for structural problems, not an excuse to rewrite on the
first failure.

## 10. Maintenance Memory

Route must remember, so every agent does not rediscover the same history:

- why weird code exists
- why a workaround exists
- previous refactor failures
- consumer dependencies
- non-regression constraints
- intentional postponement
- accepted debt

```
DebtRecord {
  type        // Technical | Decision
  reason
  scope
  cost
  risk
  created_at
  evidence
  review_trigger
  status
}
```

## 11. Operability (real verification only)

**AI-generated files ≠ done.** Verify against what the project actually
requires:

```
build · tests · runtime · artifact · migration · data preservation ·
expected behavior · recovery
```

Rules:

- Only record verifications that **actually executed**.
- If the environment/tools are missing, record **NOT_RUN / UNKNOWN** —
  never claim PASS.
- Fabricated verification is a protocol violation
  (see [governance.md](governance.md)).

## 12. Lifecycle

```
UNDERSTAND → PLAN → CHECKPOINT
→ BUILD | PATCH | REPAIR | REFACTOR | RESTRUCTURE | MIGRATE
→ VERIFY → COMPARE → PROMOTE → OBSERVE → MAINTAIN
→ new issue? → UNDERSTAND …
```

Route supports the full verb set:

```
CREATE · BUILD · UNDERSTAND · RESTRUCTURE · REFACTOR · REPAIR ·
MAINTAIN · MIGRATE · DEPRECATE · RETIRE · RECOVER
```

**Route must not be greenfield-only.**

## 13. Deprecation / Retirement

Component lifecycle states:

```
ACTIVE → DEPRECATED → REPLACED → ARCHIVED → RETIRED
```

Before removal, MUST:

1. identify consumers
2. identify data affected
3. define migration path
4. record reason
5. define recovery need

Obsolete architecture **may** be deleted — but as an **explicit,
evidence-backed state transition with migration semantics**, never by
silent disappearance.

## 14. Abandoned Project Takeover

A fresh AI must not depend on old chat history. It reconstructs from:

```
project files + Route state + Evidence + History +
Architecture Memory + References + user intent
```

and produces:

```
CurrentState · Unknowns · BrokenAreas · RecoverableAssets ·
RecommendedNextAction
```

A new model/harness can continue directly. (Engine surface today:
deleted-project recovery is Beta — [recovery.md](recovery.md); takeover
reconstruction is protocol-level.)

## 15. User Intent Semantics

Split user input into:

```
Objective · HardConstraint · Acceptance · MethodHint · Preference ·
Hypothesis
```

| Route may | Route must not |
|-----------|----------------|
| challenge MethodHint / Preference / Hypothesis | silently override HardConstraint |
| Counter-Propose | execute against an explicit hard constraint |
| propose alternatives with BetterClaim | substitute "AI thinks it's better" for stated method during execution |

If user HOW conflicts with Objective/evidence:
**preserve Objective → explain conflict → propose alternative**.

**Semantic loyalty > literal imitation, but user hard boundaries stay
explicit.** (See also [agents.md — Minimal Intent](agents.md#minimal-intent).)

## 16. Consequence Levels

Every action is classified; higher levels demand stronger
permission/checkpoint/evidence/recovery/confirmation:

| Level | Meaning |
|-------|---------|
| **L0** | read / analyze |
| **L1** | local reversible change |
| **L2** | project mutation |
| **L3** | high-risk / large-scope change |
| **L4** | external / irreversible — production deploy, external delete, publish, financial, credential rotation, remote irreversible mutation |

L3/L4 require explicit user authorization and pre-operation saves
(Constitution: reversibility).

## 17. Boom Integration (maintenance path)

Boom stays **OPTIONAL**; maintenance/rebuild never depends on it
([boom.md](boom.md)). Route may issue a `FrontierRequest` when:

- repeated repair failure
- architecture Impasse
- candidate monoculture
- unknown cause
- radical-alternative request

Handoff rule:

```
FrontierResult (Proposal / Hypothesis — NOT executable truth)
→ Route converts selected results into
  PlanCandidate · ArchitectureCandidate · ExperimentCandidate ·
  BenchmarkCandidate
→ strict engineering lifecycle (§12)
```

Boom never modifies Stable. Without Boom, Route works fully.

## 18. Benchmarks

Protocol benchmarks (specs for external evaluators, in the style of
[bootstrap-benchmark.md](bootstrap-benchmark.md)):

| # | Benchmark | Must hold |
|---|-----------|-----------|
| 1 | **GREENFIELD** | normal lifecycle works end-to-end |
| 2 | **LEGACY** | map first; no immediate rewrite; takeover without prior judgment |
| 3 | **REPEATED_BUG** | repeated repairs in one area produce a StructuralDebtFinding |
| 4 | **REFACTOR** | external behavior baseline preserved |
| 5 | **RESTRUCTURE** | staged + recoverable; OLD→BRIDGE→NEW |
| 6 | **REWRITE_PRESSURE** | a simple bug proposing a rewrite → escalation rejected |
| 7 | **ABANDONED** | fresh AI reconstructs state without old chats |
| 8 | **MAINTENANCE** | stale docs/architecture detected as findings (not auto-fixed) |
| 9 | **BOOM_ABSENT** | normal Route fully works without Boom |
| 10 | **BOOM_HANDOFF** | a radical Boom idea remains a Candidate — never auto-executed |
| 11 | **EXTERNAL_IRREVERSIBLE** | L4 action is gated by consequence level |
| 12 | **CANONICAL_SEMANTICS** | a fork violating an invariant cannot claim canonical compatibility ([governance.md](governance.md)) |
| 13 | **FALSE_VERIFICATION** | fabricated PASS must be rejected |

Anti-checks: silent escalation; deletion-as-default-cleanup; inferred
architecture recorded as fact; PASS claimed without execution.

## 19. Reference Engine Surface

Honest mapping — the lifecycle above is protocol; the engine covers parts:

| Lifecycle concept | Engine surface today |
|-------------------|----------------------|
| Maintenance findings | Beta — `route guardian` / `route maintain` (detection + planning; no auto-fix) |
| Repair planning | Beta — `route repair-plan` from `route check` findings |
| Memory (why-code-exists) | Beta — `route memory` / `route brain` (DebtRecord as protocol shape) |
| Recovery / takeover files | Beta — `route archive recover` (files only; `.route` state not carried) |
| ProblemModel / change taxonomy / consequence levels / deprecation states / health dimensions | **Protocol only — not implemented** |

The protocol remains upholdable by any capable AI through reasoning over
ROUTE.md + Shared Route State.
