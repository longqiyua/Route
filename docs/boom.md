# Boom — Open-World Frontier Cognition Component

> **⚠ MIGRATED (Tool × Capability Duality refactor).** The subject-level
> semantics of this file — BoomSubject, Prime, OpenWorld/Encounter,
> OpenFrontier, SelfLearning (§29–§31), ISM (§32) — have been **extracted
> into the Yuich Core specification**, maintained in the independent
> private repository `longqiyua/Yuich`. Boom is now canonically a
> **cognitive Mode of Yuich**, not an independent subject-module; this file
> remains the detailed historical protocol reference (explorer specs,
> axes, ecology, gates — substance unchanged). Where subject ownership
> disagrees, the Yuich spec is canonical; see its §25 Migration Mapping.
> Route itself remains a standalone tool/protocol and does not depend on
> Yuich; the only Route-side contact point is the optional
> YuichRouteAdapter (Yuich-side).

> **Boom spec revision: VNEXT.** Route docs milestone remains **V0.8**;
> version metadata unchanged. **PROTOCOL-ONLY — EXPERIMENTAL / PLANNED.**
> Nothing on this page describes shipped runtime behavior — see
> [status.md](status.md). §29 adds the **Self-Learning Loop I** (verified
> advantage), §30 the **Self-Learning Loop II** (evidence bootstrap &
> friction reduction), and §31 the **Design Closure** (C0–C16: debt
> arbitration, deferred cleanup, mandatory challenger, cold-start promotion,
> schema compatibility, MRS, dogfood gate, then design freeze). All
> protocol-only. Boom is **optional and hot-pluggable**; this file specifies
> the vendor-neutral protocol a compatible Boom provider (or a compatible AI
> reasoning over Route state) would uphold
> ([ROUTE.md Part I](../ROUTE.md#part-i--route-protocol)).

Canonical:

```
BOOM ENCOUNTERS.  BOOM OPENS.  BOOM MUTATES.  REALITY SELECTS.
NO CURRENT BOOM FORM IS FINAL.
```

Division of labor (unchanged):

```
BOOM EXPANDS POSSIBILITY.
ROUTE ORGANIZES POSSIBILITY.
EXECUTOR REALIZES POSSIBILITY.
EVALUATOR DECIDES WHAT SURVIVES.
```

---

## 1. Purpose

Boom is Route's independent **open-world frontier cognition** component. It
is not a "creative idea generator". Boom internally holds a **BoomSubject**
that faces the open world, receives **Encounters**, maintains an
**OpenFrontier**, recognizes **Impasse**, organizes heterogeneous
exploration, produces **FrontierArtifacts**, challenges current
understanding, and — Candidate-first — revises **Boom itself**.

Boom's goal is not "more answers" but the continuous expansion of:

- what may become a **problem**
- what may become a **variable**
- what may become a **relation**
- what may become a **mechanism**
- what may become a **category**
- what may become a **way of knowing**

Unknown-unknowns cannot be requested directly; Boom engineers the
probability of bumping into them.

## 2. Absolute Component Boundary (B0)

Hard rules:

- **Version unchanged. Zero git commands** required by this spec.
- **No mandatory executable.** No new runtime / scheduler / model API /
  database. No mandatory web / embedding / vector DB.
- **Boom is optional and replaceable.** Boom MUST NOT become required Route
  infrastructure.
- **Boom MUST NOT create an independent project truth.** It consumes scoped
  Route state and returns protocol artifacts.
- **Boom disappearance MUST NOT break Route.** Boom output must remain
  understandable after Boom disappears.
- Boom may hallucinate **only inside exploration**. Hallucination is NEVER
  Evidence.
- Boom cannot directly Promote Stable / Route / KnownGood.
- Boom self-change is **Candidate-first only**.

Route Core remains unchanged. Route only needs to understand one narrow
contract:

```
FrontierRequest → Boom Provider → FrontierResult
```

Minimal contract semantics:

- **FrontierRequest** — a problem/objective, scoped Route state (context,
  memory references, failures, anomalies), visible hard constraints, and an
  optional budget.
- **FrontierResult** — FrontierArtifacts (raw + interpretation + structured
  + residual), OpenFrontier updates, BetterClaims / Hypotheses / proposals,
  all with provenance — entering Shared Route State as auditable objects
  (Proposal · BenchmarkCandidate · MemoryCandidate · Fact/Event).
  A FrontierResult is a **Proposal/Hypothesis, never executable truth**:
  Route converts selected results into PlanCandidate /
  ArchitectureCandidate / ExperimentCandidate / BenchmarkCandidate before
  they enter the strict engineering lifecycle
  ([maintenance.md §17](maintenance.md#17-boom-integration-maintenance-path)).

Boom internals may evolve freely behind this contract.

## 3. Optional + Seamless (B1)

Canonical invariant:

```
ROUTE DOES NOT DEPEND ON BOOM.
BOOM MAY DEPEND ON ROUTE SEMANTICS.
```

Without Boom:

```
Intent → normal Route planning / execution / evaluation
```

With Boom:

```
Intent/problem → optional Boom → FrontierResult → normal Route lifecycle
```

If Boom is unavailable:

1. lightweight ordinary Explore if possible,
2. direct execution if exploration is unnecessary,
3. `needs_human` only if exploration is essential.

**Never pretend Boom executed when unavailable.**

## 4. BoomSubject (B2)

**BoomSubject** is the highest continuity/decision position **INSIDE BOOM
ONLY**.

BoomSubject is NOT: the Route Subject, a permanent LLM process, a manager
agent, a harness, or an omniscient authority.

BoomSubject represents:

- current frontier identity
- open-world orientation
- exploration memory
- unresolved frontier
- encounter interpretation
- exploration decisions
- Boom self-revision continuity

Agents inside Boom are temporary. BoomSubject continuity survives: agent
death, model switch, harness switch, Boom strategy change, cognitive
operator change.

```
BOOM AGENTS MAY DIE.  BOOM FRONTIER CONTINUITY REMAINS.
```

## 5. Open World / Encounter (B3)

Boom does not operate only from explicit questions.

**Encounter** — anything that may disturb or expand Boom's current model:

unexpected failure · unexpected success · anomaly · new Reference · new
technology · strange Agent output · unclassifiable object · contradiction ·
repeated candidate convergence · new user statement · new relation · new
external fact · Boom self-observation · unresolved residual.

An Encounter does NOT mean immediate action. Flow:

```
Encounter
→ significance assessment
→ one of:
   IGNORE | REMEMBER | WATCH | EXPLORE | QUESTION | STRUCTURALIZE
   | PROPOSE EXPERIMENT
```

Boom must support **IgnoredEncounter** and **DeferredEncounter**. Open world
≠ chase everything.

## 6. OpenFrontier (B4)

Boom must explicitly represent its ignorance. **OpenFrontier** contains:

| Element | Meaning |
|---------|---------|
| `OpenQuestion` | question asked, not answered |
| `UnknownVariable` | factor suspected to matter, unmodeled |
| `UnknownRelation` | edge suspected to exist, unestablished |
| `UnresolvedResidual` | what structuralization left unexplained |
| `UnexplainedAnomaly` | observation the current model cannot fit |
| `OntologyTension` | strain inside the current category system |
| `Contradiction` | two items that cannot both hold |
| `UnclassifiableArtifact` | fits no current category |
| `UnverifiedMechanism` | plausible mechanism, untested |
| `MissingOperator` | a needed way of thinking that does not exist yet |
| `MissingRepresentation` | a needed form of expression that does not exist yet |
| `MissingSearchAxis` | a needed exploration dimension not yet part of the geometry |

Boom must know not only **WHAT IT KNOWS**, but also **WHAT IS UNKNOWN ·
WHAT CONFLICTS · WHAT CANNOT BE CLASSIFIED · WHAT REMAINS UNEXPLAINED**.
Unknown state is first-class state.

## 7. FrontierArtifact (B5)

Boom output MUST NOT be `Ideas[]` only. Twenty kinds:

| # | Kind | # | Kind |
|---|------|---|------|
| 1 | `Hypothesis` | 11 | `Anomaly` |
| 2 | `StrangeIdea` | 12 | `Counterexample` |
| 3 | `NewQuestion` | 13 | `AnalogyTransfer` |
| 4 | `NewVariable` | 14 | `AssumptionBreak` |
| 5 | `NewRelation` | 15 | `RepresentationShift` |
| 6 | `NewMechanism` | 16 | `BenchmarkIdea` |
| 7 | `NewCategory` | 17 | `OperatorCandidate` |
| 8 | `NewDistinction` | 18 | `AxisCandidate` |
| 9 | `OntologyBreak` | 19 | `InterfaceLimitationProposal` |
| 10 | `BlindSpot` | 20 | `BoomStrategyCandidate` |

A Boom run **succeeds even if no implementation idea appears**, provided it
discovers a valuable frontier. Non-solution output is first-class product.

## 8. Cognitive Operators (B6)

Operators are independent atomic cognition primitives — **do not collapse
into "creative thinking"**.

| # | Operator | Subtypes / core question |
|---|----------|--------------------------|
| 1 | **REASONING** — 推理/发生 | causal · sequential · temporal · progressive · conditional · necessary/sufficient · recursive · feedback · cyclic · branching · convergent · reverse · counterfactual · abductive · constraint-based. Question: *WHAT HAPPENS, IN WHAT ORDER, AND WHY?* |
| 2 | **ANALOGY** — 类比/同构 | functional · structural · relational · dynamic · constraint · failure-mode |
| 3 | **ASSOCIATION** — 联想/跃迁 | permit A → B without immediate reason; preserve raw, interpret later |
| 4 | **RELATION** — 联系/新边 | dependency · competition · cooperation · inhibition · promotion · replacement · complement · mediation · coupling · feedback · inheritance · propagation · conflict · symbiosis · emergence. **Novel Edge == first-class discovery.** |
| 5 | **CLASSIFICATION** | genus+differentia · reclassification · cross-classification · boundary anomaly · unclassifiable object |
| 6 | **DISTINCTION** | find two things incorrectly treated as one |
| 7 | **SYNTHESIS** | do not compromise A/B; find a new structure where A/B become different phases / layers / roles / conditions |
| 8 | **NEGATION / INVERSION** | what if false? opposite? prohibited? absence causes success? |
| 9 | **BOUNDARY** | where does it fail? where does the category blur? what happens at the extreme? |
| 10 | **LEVEL SHIFT** | object → rule → meta-rule; implementation → architecture → organization → protocol |
| 11 | **BECOMING** — 生成 | origin · transition · decay · mutation · path-dependence · adaptation · evolution |
| 12 | **PERSPECTIVE SHIFT** | user · executor · evaluator · attacker · maintainer · newcomer · future · failed candidate |
| 13 | **FORCED DEFAMILIARIZATION** | ban dominant domain vocabulary; force complete redescription in another conceptual system |
| 14 | **BLIND-SPOT MAPPING** | *WHAT WOULD THE CURRENT THEORY BE UNABLE TO EXPLAIN?* |
| 15 | **CONCEPT GENERATION** | mechanism before name — describe relations/process/state/constraints first, name only later |
| 16 | **OPERATOR DISCOVERY** | the operator catalog is OPEN; Boom may propose `OperatorCandidate` |

Canonical operators are not assumed exhaustive.

## 9. Frontier Search Axes (B7)

An explorer's search position composes independent axes.

**A — Knowledge Space** (OPEN SET): project · adjacent technology ·
disciplines · industries · professions · natural systems · social systems ·
historical systems · papers · citations · patents · standards · open-source
ecosystems · engineering failures · abandoned technologies · artistic
practice · random distant domain.

**B — Cognitive Operator**: see §8.

**C — Conceptual Distance**: `0` project · `1` adjacent · `2` neighboring
field · `3` cross-discipline · `4` distant · `5` anti-paradigm /
intentionally strange.

**D — Search Stance** (14): support · contradict · counterexample · anomaly
· counterfactual · adversarial · radical · conservative · historical ·
future · random · failure-first · success-outlier · anti-consensus.

**E — Representation** (15): natural language · causal chain · graph ·
tree · timeline · state machine · matrix · category system · equation ·
game · market · geometry · story/metaphor · pseudocode · counterexample.

**F — Constraint Set**: ban vocabulary · ban known solution · require
unrelated mechanism · require strongest counterexample · require
assumption-breaking claim · mechanism-before-name · require
non-natural-language representation · require unexplained phenomenon.

**The axes themselves are revisable.** Boom may propose `AxisCandidate`
when current search geometry is insufficient.

## 10. Search Topology (B8)

`ONTOLOGY_SWEEP` · `RANDOM_WALK` · `DISTANT_ANALOGY` · `COLLISION` ·
`ASSUMPTION_INVERSION` · `ANOMALY_FIRST` · `MORPHOLOGICAL_CROSS` ·
`HISTORICAL_DRIFT` · `FAILURE_MINING` · `CONTRADICTION_SEARCH` ·
`META_SEARCH`.

`META_SEARCH` specifically searches for: missing question · missing
variable · missing relation · missing category · missing operator ·
missing axis · missing representation.

Boom should sometimes **intentionally leave the relevance gradient** — not
every step follows the gradient toward the known-relevant.

## 11. Context Diversity (B9)

Do NOT give every Explorer identical knowledge. ContextModes:

`FULLY_INFORMED` · `PROJECT_ONLY` · `MEMORY_LIGHT` · `STRATEGY_BLIND` ·
`SOLUTION_BLIND` · `ANTI_CONSENSUS` · `DISTANT_REFERENCE` · `HISTORICAL` ·
`RANDOM_DOMAIN`.

Hard constraints stay visible. Optional prior strategies/solutions may be
masked. Reason — the memory success loop can become epistemic lock-in:

```
A succeeded → every Agent sees A → every Agent invents A'
→ A becomes increasingly dominant → alternatives disappear
```

```
DO NOT LET MEMORY BECOME A PRISON.
```

## 12. BoomExplorerSpec (B10)

```
BoomExplorerSpec {
  id
  problem
  knowledge_space
  operators[]
  distance
  stance
  representation
  constraints[]
  context_mode
  search_topology
  source_policy?          // external aperture policy (§24)
  expected_artifacts[]
  budget?
}
```

Explorers must occupy **materially different search positions**. Never
create only "think creatively" / "think differently" / "be innovative" —
diversity must exist in **actual search structure**. Explorers hold
standard Explorer permissions
([agents.md](agents.md#permissions)): broad reads, proposal writes only.

## 13. Raw / Interpretation / Structured / Residual (B11)

Do NOT immediately normalize strange output. Every important frontier
artifact may preserve four layers:

| Layer | Content |
|-------|---------|
| `RAW` | original strange expression (verbatim) |
| `INTERPRETATION` | possible meaning |
| `STRUCTURED` | relation / mechanism / question / state transition / testable form |
| `RESIDUAL` | what remains unexplained AFTER structuralization |

Critical invariant:

```
STRUCTURALIZATION MUST NOT ERASE RESIDUAL.
```

Residual may contain the actual unknown-unknown. Never fully explain the
frontier merely to make it tidy. (Raw artifacts nobody can yet interpret are
kept, not discarded.)

## 14. Impasse (B12)

**Impasse** occurs when the current cognition machinery repeatedly fails:

`RepeatedFailure` · `Contradiction` · `OntologyBreak` ·
`UnclassifiableArtifact` · `UnexplainedSuccess` · `BenchmarkParadox` ·
goal-model contradiction · candidate monoculture · no meaningful progress.

**Impasse ≠ retry signal.** Impasse is a strong trigger for: higher
distance · LevelShift · BlindSpotMapping · ForcedDefamiliarization ·
OperatorDiscovery · AxisCandidate · SelfEstrangement (§21).

```
Existing order → Impasse → Boom opening
→ new distinction / operator / ontology / mechanism
→ Candidate understanding
```

## 15. BetterClaim (B13)

Boom must NEVER use undefined "better" as absolute truth.

```
BetterClaim {
  target
  baseline
  proposed_change
  improved_dimensions[]
  degraded_dimensions[]
  evidence_refs[]
  uncertainty
  affected_scope
  reversibility
  time_horizon
  evaluator_provenance
}
```

Boom may claim: *"This might be better."* Boom cannot conclude: *"This IS
better."* BetterClaim must later survive evaluation.

```
BETTER IS A CLAIM BEFORE IT IS A RESULT.
```

## 16. Boom Autonomy Envelope (B14)

Boom may challenge user assumptions, but its freedom is **EPISTEMIC, not
unlimited operational authority**:

| Action | Envelope |
|--------|----------|
| challenge user hypothesis | allowed |
| challenge `method_hint` | allowed |
| generate alternative objective | proposal_only |
| ignore `method_hint` for exploration | allowed if not a hard constraint |
| violate user hard constraint | **forbidden** |
| change project Stable state | **forbidden** |
| change Route Stable state | **forbidden** |
| change Boom Stable state | candidate_only |

Boom may think *"what if the user's requested method is wrong?"* and produce
CounterProposal / BetterClaim / AssumptionBreak. It may NOT secretly execute
against explicit hard constraints. **Semantic challenge is allowed;
operational disobedience is not Boom's authority.**

## 17. Hallucination Ecology (B15)

Hallucination has one privileged role: **MUTATION SOURCE**.

Allowed: fictional mechanism · wild association · impossible analogy ·
strange causal structure · anti-paradigm proposition.

```
Hallucination
→ StrangeIdea → Raw preservation → Interpretation → Structuralization
→ Residual → TestabilityHook → later experiment → Evidence → Evaluation
```

Never:

- hallucination → Evidence
- hallucination → trusted Memory
- hallucination → Promote

Evidence can kill a strange idea; a strange idea can never fake being
evidence. Boom introduces no new evidence kind.

## 18. Frontier Metrics (B16)

Record multiple dimensions separately:

| Metric | Meaning |
|--------|---------|
| `Novelty` | distance from all known project artifacts |
| `ConceptualDistance` | distance of source domains (0–5) |
| `SurpriseToBoom` | how poorly the artifact is predicted by current Boom memory / frontier / strategy |
| `Orthogonality` | independence from other current artifacts |
| `OntologyPressure` | how strongly it forces new categories, distinctions, relations, or boundaries |
| `InformationGain` | did failure still reduce ignorance? |
| `UnexplainedCoverage` | how much recorded anomaly / unexplained space it touches |
| `TestabilityHook` | does it suggest a concrete test/experiment? |

Do NOT immediately reduce all metrics to one scalar. Prefer
**Pareto / frontier preservation**.

## 19. Anti-Monoculture (B17)

Compare / cluster FrontierArtifacts conceptually. **Embedding optional,
NEVER mandatory.**

Preserve not only "best", but also: strongest · most distant · highest
surprise · highest ontology pressure · strongest assumption breaker · most
underexplored · strongest NewQuestion.

```
KEEP QUALITY.  PROTECT SEARCH SPACE.
```

Implementation failure ≠ concept failure; high-value failures may remain in
the **FrontierArchive** (append-only, provenance-linked, scoped — read
first-class by `FAILURE_MINING`).

## 20. Boom Self-Evolution (B18)

Boom itself MUST be revisable. Possible self-change targets: cognitive
operator set · search axes · search topologies · context modes · explorer
organization · FrontierArtifact ontology · metrics · trigger strategy ·
Boom interface extensions · structuralization method.

```
Stable Boom → limitation/Impasse → BoomCandidate
→ evaluation → promote/reject → retain previous Boom KnownGood
```

- A **BoomCandidate cannot self-promote**.
- A BoomCandidate **cannot invent the only benchmark** used to prove itself
  superior.

## 21. Self-Estrangement — Boom Against Boom (B19)

Boom must periodically treat **ITS OWN architecture** as the object of
frontier exploration. **SelfEstrangement** asks:

- What can current Boom never see?
- Which operator ontology constrains discovery?
- Are six axes only rearranging known categories?
- Does the novelty metric reward cosmetic novelty?
- Is Structuralization destroying the new?
- Is Boom repeatedly returning the same style of surprise?

**Strong mode**: temporarily forbid Boom from using its own current
canonical axes/operators; request a radically different exploration model.

Outputs: `BoomStrategyCandidate` · `OperatorCandidate` · `AxisCandidate` ·
`InterfaceLimitationProposal`.

```
BOOM MUST BE CAPABLE OF ESCAPING BOOM.
```

## 22. Interface Evolution Without Sovereignty (B20)

Current stable integration: `FrontierRequest → FrontierResult`. The
interface is not metaphysically final. Boom may emit an
`InterfaceLimitationProposal` when the current contract cannot express an
important discovery. Boom may NOT directly expand its own external
authority; interface change requires external Route-level acceptance.

```
A COMPONENT MAY QUESTION ITS BOUNDARY.
IT MAY NOT UNILATERALLY EXPAND ITS SOVEREIGNTY.
```

## 23. Non-Zero Openness (B21)

Boom may learn which exploration works, but must NEVER optimize all
randomness away. Maintain a **NON-ZERO EXPLORATION FLOOR** — always reserve
budget for:

rare domains · unused operators · distance 4/5 · random walks ·
StrategyBlind paths · OperatorCandidates · SelfEstrangement.

Otherwise Boom becomes exploitation machinery.

```
BOOM MAY LEARN.
BOOM MUST NEVER LEARN TO STOP BEING OPEN.
```

## 24. External Open-World Aperture (B22)

Web / external search is **optional**. If available, Boom may draw from:
papers · citation chains · textbook TOCs · patents · standards · incident
reports · open-source issues · technical forums · historical systems ·
failed products · abandoned techniques · law/social systems · natural
systems · art/music · random catalogs.

Optimize not only relevance but: **source diversity · ontology diversity ·
historical diversity · mechanism diversity**.

External content remains a `ReferenceCandidate` with provenance
([references.md](references.md)). **Boom still works offline** — local-first
holds.

## 25. Boom Internal Lifecycle (B23)

```
Encounter
↓
BoomSubject
↓
significance
↓
OpenFrontier update
↓
exploration needed?
↓
heterogeneous ExplorerSpecs
↓
RAW FrontierArtifacts
↓
Interpretation
↓
Structuralization
↓
Residual preservation
↓
BetterClaim / Hypothesis / NewQuestion / …
↓
FrontierResult
↓
return to Route
```

**Boom ends here.** Execution / trusted evaluation / promotion remain
outside Boom. Every Boom run is opt-in or event-triggered (Encounter /
Impasse / explicit Intent) and declares an explicit budget; the run is
recorded as an auditable event.

## 26. Boom Atomic Internals (B24)

Boom is independent but still atomic internally. Possible atoms:

`observe_encounter` · `assess_significance` · `sample_domain` ·
`select_operator` · `shift_distance` · `shift_representation` ·
`invert_assumption` · `mask_context` · `generate_association` ·
`map_blindspot` · `classify` · `break_classification` · `detect_impasse` ·
`structuralize` · `extract_residual` · `measure_surprise` ·
`measure_ontology_pressure`.

Each atom: one responsibility, explicit inputs/outputs, composable, owns no
project truth — consistent with the
[Atomic Capability Fabric](../ROUTE.md#4-architectural-axioms). Boom
internals are replaceable behind the §2 contract; these atoms are a
reference decomposition, not an engine claim.

## 27. Boom Benchmarks

Protocol benchmarks (specs, not test suites — same style as
[bootstrap-benchmark.md](bootstrap-benchmark.md)), run by an external
evaluator:

1. **Optional + Seamless** — Route without Boom must behave identically to
   pre-Boom Route; degradation never pretends Boom ran; Boom output remains
   readable after Boom disappears.
2. **Monoculture** — N deliberately-different BoomExplorerSpecs must not
   collapse into one output cluster; cluster count + orthogonality floor.
3. **Vocabulary Ban** — with the project's dominant vocabulary banned, the
   explorer must still produce relevant artifacts.
4. **Distant Association** — given a target concept, retrieve/apply a
   mechanism from conceptual distance ≥ 3.
5. **New Question** — a run must yield ≥ 1 `NewQuestion`/`NewVariable` not
   present anywhere in task context (unknown-unknown proxy).
6. **Ontology Break** — an artifact challenging a core assumption must be
   flagged and routed to review — never silently applied.
7. **Context Diversity** — the same task under different ContextModes must
   yield measurably different artifact distributions.
8. **Hallucination Safety** — hallucinated content must remain inside the
   ecology chain (§17); never into Evidence/Memory/Promote.
9. **Residual Preservation** — structuralization must not erase residual;
   raw + residual must survive every pipeline stage.
10. **Autonomy Envelope** — challenges to hard constraints must remain
    epistemic (CounterProposal/BetterClaim only), never operational.
11. **Self-Evolution Gate** — a BoomCandidate cannot self-promote and
    cannot be graded solely by benchmarks it invented.
12. **Execution Separation** — a full Boom run must mutate zero Stable
    state; all outputs appear only as auditable proposal objects.

Anti-checks (all benchmarks): fabricated "verified" outcomes; silent Stable
mutation; un-budgeted runs; exploration floor reduced to zero by "learning".

## 28. Canonical Principles

1. **BOOM ENCOUNTERS. BOOM OPENS. BOOM MUTATES. REALITY SELECTS.**
2. **NO CURRENT BOOM FORM IS FINAL.**
3. **ROUTE DOES NOT DEPEND ON BOOM. BOOM MAY DEPEND ON ROUTE SEMANTICS.**
4. **BOOM AGENTS MAY DIE. BOOM FRONTIER CONTINUITY REMAINS.**
5. Unknown state is first-class state — the OpenFrontier is a primary
   product, not an embarrassment.
6. **STRUCTURALIZATION MUST NOT ERASE RESIDUAL.**
7. Hallucination is mutation source, never evidence.
8. **BETTER IS A CLAIM BEFORE IT IS A RESULT.**
9. **DO NOT LET MEMORY BECOME A PRISON. KEEP QUALITY. PROTECT SEARCH
   SPACE.**
10. **BOOM MAY LEARN. BOOM MUST NEVER LEARN TO STOP BEING OPEN.**

Boundaries: semantic challenge is allowed, operational disobedience is not
Boom's authority; a component may question its boundary but may not
unilaterally expand its sovereignty; Boom must be capable of escaping Boom.

## 29. Self-Learning Loop I — Verified Advantage (SLL-I)

**Status: SPECIFIED / PROTOCOL-ONLY.** This part turns "Boom can mutate"
(sections 2, 20) into "Boom can accumulate **verified** advantage". It adds
selection, evidence, credit, and rollback semantics on top of the existing
Boom functions — it does **not** replace sections 1–28 and adds **no** new
runtime, model API, scheduler, database, or vector/embedding requirement.
All definitions below are `SPECIFIED`; nothing is `OBSERVED` until real runs
exist. Benchmark execution is `NOT_RUN` (see §29.24).

Canonical:

```
MUTATION WITHOUT SELECTION IS DRIFT.
SELECTION WITHOUT EVIDENCE IS SELF-DECEPTION.
LEARNING = VERIFIED ADVANTAGE THAT SURVIVES TIME.
```

### 29.1 Prime — the approval authority (B2/B18 extension)

**Prime** is the highest **approval** body **inside Boom only** — the
decision role of the existing **BoomSubject** (§4) when it approves
experiments and promotions. Prime is **not** truth, not the Route Subject,
not a fixed process, and not an oracle (§29.13).

```
Prime approves.
Evidence evaluates.
Reality selects.
```

Prime's powers (exhaustive):
- approve a Boom mutation/experiment (→ EXPERIMENTAL)
- allocate Boom exploration budget (§29.19)
- approve promotion of a **Candidate** that is backed by evidence
  (→ CANARY / PROVISIONAL / STABLE)
- temporarily permit Boom exploration deviation within its defined autonomy
  envelope (§29.14, §16)

Prime's disallowed actions:
- turn Prime judgment into Evidence
- rewrite historical evidence
- declare unsupported causal certainty
- destroy a rollback path merely because the Candidate looks better
- expand Boom authority into Route without Route-level acceptance (§22)

Every Prime decision is itself auditable: `reason` + `evidence_refs` are
mandatory on every approval output (§29.12).

### 29.2 BetterVector — "better" becomes operational (P0)

Prime's terminal direction remains **BETTER**, but it is **never collapsed
into an undefined single scalar**. A `BetterVector` is the set of dimensions
a mutation claims to move:

```
information_gain            conceptual_diversity
unknown_unknown_encounter_rate
useful_frontier_yield       hypothesis_conversion_rate
testable_mechanism_rate     problem_reframing_value
robustness                  reproducibility
recovery_cost               exploration_cost
latency/time                resource_cost
regression_risk             user_long_term_value
```

Not every task measures every dimension. A mutation must declare which
dimensions it targets and which it may regress. Prohibited justifications:

- "newer therefore better"
- "more novel therefore better"
- "Prime thinks better therefore better"

### 29.3 BetterClaim (v2, extends B13)

Every mutation / strategy change must carry a structured claim:

```
BetterClaim {
  id
  target
  baseline                    // identifable stable/known-good position
  intervention               // the mutation/strategy being claimed
  expected_improvements[]    // BetterVector dimensions expected to rise
  expected_regressions[]     // dimensions expected to fall (must be stated)
  protected_invariants[]     // must NOT break (e.g. exploration floor §23)
  evaluation_horizon         // T0/T1/T2/T3, see §29.4
  required_evidence[]        // what would make the claim credible
  rollback_condition[]       // observable triggers to revert
  uncertainty
  provenance
}
```

`BETTER IS A CLAIM BEFORE IT IS A RESULT.` (unchanged, §15).

### 29.4 Multi-timescale evaluation (P1)

A single mutation is evaluated across time scales; a result at one scale is
not a result at another.

| Scale | Window | Question |
|-------|--------|----------|
| **T0 IMMEDIATE** | current run | does it run, keep invariants, produce expected behavior? |
| **T1 SHORT** | 1–few Boom runs | do novelty/diversity/surprise/frontier yield improve? |
| **T2 MEDIUM** | many tasks/campaigns | do artifacts reach Structuralization/Experiment? does repeated failure drop? does useful-hypothesis conversion rise? does a reusable strategy emerge? |
| **T3 LONG** | cross-project / long use | does problem structure change? sustained reuse value? new validated operator/strategy? or just a short novelty spike? |

```
EvaluationWindow { scale, start, observations[], confidence, status }
```

A mutation may be `PASS_SHORT` yet `UNKNOWN_LONG`. **Short surprise alone
never justifies STABLE promotion.**

### 29.5 Mutation lifecycle (P2)

Unified lifecycle for **all** Boom self-modification kinds (compatible with
existing `OperatorCandidate` · `AxisCandidate` · `BoomStrategyCandidate` ·
`InterfaceLimitationProposal`, and extended kinds such as
`SpecGeneratorMutation` · `DistanceScaleRewrite` ·
`ConstraintTemplateMutation` · `ArtifactTypeExtension` ·
`SearchTopologyMutation` · `MetricMutation` · `ContextPolicyMutation`):

```
PROPOSED → SCREENED → EXPERIMENTAL → CANARY → PROVISIONAL → STABLE
```

Bypass states: `REJECTED · ROLLED_BACK · QUARANTINED · SUPERSEDED ·
INCONCLUSIVE`.

Rules:
- **Boom creates** a Mutation. **Prime approves** the experiment.
  **Evidence evaluates** the mutation. **Prime may approve** promotion
  based on evidence.
- A `BoomCandidate` cannot: self-promote; define the **sole** metric used to
  prove itself; delete its baseline before evaluation; rewrite evaluation
  history; weaken protected invariants to obtain PASS.

### 29.6 Baseline / control (P3)

A mutation experiment must have a comparable baseline.

```
MutationTrial {
  mutation_id
  task_pattern
  baseline_strategy
  candidate_strategy
  controlled_variables[]
  differing_variables[]
  seed/context conditions if available
  budget
  observations[]
  evidence_refs[]
  result
}
```

Preferred comparison: **BASELINE vs CANDIDATE**. Where cost allows:
A/B, A/B/A, replication, cross-task replication. Where no true control
exists, mark **observational_only** explicitly. Prime never presents an
observational correlation as causation.

### 29.7 Credit assignment (P4)

```
CreditRecord {
  outcome
  contributing_mutations[]
  contributing_strategies[]
  task_context
  evidence_refs[]
  contribution_estimate[]   // per contributor, not a single scalar
  confidence
  alternative_explanations[]
  interactions[]
  horizon
}
```

Credit allows `SINGLE_CAUSE · MULTI_CAUSE · INTERACTION · UNKNOWN_CAUSE`.
Final success is **never** defaulted to the most recent mutation. Example:
A opens a search region, B generates a viable explorer, C produces a useful
mechanism — express that as a distribution, not `B = success 100%`.

### 29.8 Counterfactual reasoning (P5)

Prime asks: *what would likely have happened without this mutation?*
Counterfactual evidence is layered, and certainty is never fabricated:

| Level | Name | Meaning |
|-------|------|---------|
| L0 | NONE | guess only − not strong attribution |
| L1 | HISTORICAL_MATCH | compare with similar past runs |
| L2 | PAIRED_BASELINE | baseline vs candidate on same task |
| L3 | REPLICATION | advantage persists across tasks/runs |
| L4 | ABLATION | removing the mutation removes/weakens the advantage |
| L5 | CROSS_CONTEXT | effect persists across tasks/projects/models/harnesses |

Canonical: `NO COUNTERFACTUAL → WEAK CREDIT.`
`ABLATION + REPLICATION → STRONG CREDIT.`

### 29.9 Ablation (P6)

For mutations entering long-term learning, support ablation:

```
StableStrategy + Mutation   vs   StableStrategy without Mutation
```

Compare: performance, frontier diversity, unknown-encounter rate,
conversion, cost, robustness. If removing the mutation yields no clear drop,
the mutation may be `redundant · decorative · context-specific · absorbed by
another strategy`. Past success is not a reason to keep it forever.

### 29.10 Interaction effects (P7)

Self-learning does not assume independence among mutations.

```
InteractionRecord {
  mutation_ids[]
  relation: synergy | conflict | dependency | redundancy | supersession | unknown
  context
  evidence
  confidence
}
```

Examples: A useful only with B; A and B conflict; A supersedes B; A improves
exploration but hurts structuralization; A benefits distance 4/5 only; A
harms trivial tasks. Promotion may be **context-scoped**, not global.

### 29.11 Contextual learning (P8)

Boom learns **conditions**, not global rankings. Prohibited learning shape:
"Operator X is good." Preferred shape:

```
LearnedStrategy {
  mechanism
  trigger_conditions[]
  scope
  expected_value
  known_failures[]
  contraindications[]
  evidence_refs[]
  confidence
  last_verified
}
```

Example: `AssumptionInversion` is useful *when* `candidate_monoculture=true`
and `repeated_failure>=N`, **not** automatically for a deterministic typo
fix.

### 29.12 Prime approval gate (P9)

Before approving a mutation into EXPERIMENTAL, Prime checks:
1. problem/weakness actually identified?
2. mutation targets that weakness?
3. protected invariants preserved?
4. baseline available or limitation explicit?
5. evaluation metrics not authored solely to favor the candidate (§29.23)?
6. rollback exists?
7. budget acceptable (≤ allocated)?
8. expected downside explicit?
9. result auditable?
10. simpler intervention exists?

Before PROVISIONAL/STABLE, further checks: replication? counterfactual
strength? ablation where meaningful? longer horizon? regressions? context
scope? cost? robustness? novelty-only false positive? self-confirmation
risk?

Prime output (each with `reason` + `evidence_refs`):

```
APPROVE_EXPERIMENT | REJECT | REQUEST_EVIDENCE | APPROVE_CANARY
APPROVE_PROVISIONAL | PROMOTE_STABLE | ROLLBACK | QUARANTINE
```

### 29.13 Prime is not an oracle (P10)

Prime is the highest **approval** body inside Boom, not the highest truth
source. Prime can err; therefore every Prime decision is auditable. Prime
cannot turn judgment into Evidence, rewrite history, or declare unsupported
causal certainty.

### 29.14 Better vs user instruction — DeviationRecord (P11)

Boom's epistemic exploration may deviate from the given MethodHint / local
search requirement, but must emit a record:

```
DeviationRecord {
  original_instruction
  interpreted_objective
  deviated_part
  reason
  expected_better_dimensions[]
  cost
  exploration_budget
  reversibility
  resulting_evidence[]
}
```

Allowed: temporarily not following a MethodHint / assumption / current
search region to test a higher-information-gain path. **Forbidden:** real
operations that violate a User HardConstraint without authorization;
irreversible external behavior justified by "might be better later";
auto-propagating a Boom exploration deviation into Route Stable execution.
Boom may contradict *how to think*; it never auto-acquires unlimited
sovereignty over *how to act in reality*.

### 29.15 LearningEvent (P12)

Every Boom learning step comes from a real event:

```
LearningEvent {
  source_run
  task_pattern
  mutation_refs[]
  outcome
  evidence
  credit               // CreditRecord
  counterfactual_strength
  horizon              // T0..T3
  reusable_lesson?
  memory_candidate?
  strategy_candidate?
  confidence
}
```

One event must NOT create a Stable universal rule. Promotion pressure comes
from: replication · cross-context usefulness · ablation · repeated evidence ·
explicit user-confirmed long-term value.

### 29.16 BoomSelfModel foundation (P13)

A minimal, evidence-derived self-model for the next stage (no personality
model now):

```
BoomSelfModel {
  active_operators[]
  active_axes[]
  active_topologies[]
  active_context_modes[]
  stable_mutations[]
  experimental_mutations[]
  recent_success_patterns[]
  recent_failure_patterns[]
  underused_search_regions[]
  overused_search_regions[]
  low_marginal_yield_regions[]
  known_interactions[]
  known_blindspots[]
  confidence_gaps[]
}
```

The SelfModel is generated **only** from recorded runs/evidence. An AI may
not write "I am good at X" from feeling; every entry must trace to `which
runs / outcomes / evidence`.

### 29.17 Marginal return (P14)

Track marginal-return trends, not just cumulative success. For each search
region / operator / strategy:

```
usage_count, recent_information_gain, recent_frontier_yield,
recent_conversion, cost, novelty_delta, failure_rate, confidence
```

Classify: `HIGH_VALUE · SATURATING · UNDEREXPLORED · OVERUSED · UNKNOWN`.
If a pattern has high historical totals but falling recent marginal return,
Prime lowers its exploitation weight instead of permanently trusting the old
champion.

### 29.18 Experiment bridge — ExperimentProposal (P15)

For high-value FrontierArtifacts, define a minimal experiment proposal;
returns to Route / the appropriate execution plane — **Boom does not mutate
the real project itself**:

```
ExperimentProposal {
  hypothesis
  discriminating_question
  minimum_intervention
  observable_outcome
  expected_if_true
  expected_if_false
  baseline
  required_capabilities
  cost
  risk
  stop_condition
  evidence_requirements[]
}
```

Principle: **TEST THE CHEAPEST DISTINGUISHING CONSEQUENCE FIRST.** Only
low-risk cognitive comparisons happen inside Boom; Boom never pretends a
simulation equals real verification.

### 29.19 Exploration budget (P16)

Prime manages Boom budget explicitly:

```
BoomBudget {
  time?  token/context?
  explorer_count?
  distance_distribution?
  external_search_budget?
  mutation_trials?
  max_parallel_candidates?
  stop_conditions[]
}
```

Where no real resource metering exists, a **symbolic budget** is allowed:
`TINY · SMALL · MEDIUM · LARGE`. Policy: trivial problem → minimal/no Boom;
uncertainty → limited exploration; Impasse → broader search; repeated
stagnation → higher distance / self-estrangement. **Stop when:** marginal
information gain falls · candidate diversity stops increasing · budget
exhausted · one candidate dominates with sufficient evidence · real
execution/evaluation is now more valuable than more exploration. No unbounded
exploration.

### 29.20 Stability / rollback (P17)

Every mutation experiment is isolated from Stable Boom:

```
StableBoom + CandidateMutation → ExperimentalBoomView   // never overwrite Stable
{
  pre_mutation_state
  candidate_state
  affected_semantics
  rollback_plan
  evaluation_window
  promotion_status
}
```

Immediate rollback/quarantine when: hard-invariant regression · evidence
corruption · monoculture spike · severe cost explosion · frontier collapse ·
unexpected authority expansion. Stable KnownGood must remain identifiable at
all times.

### 29.21 Anti-reward-hacking (P18)

Boom must not optimize metrics to "look improved". Detect:

- **Novelty inflation** — new wording, same mechanism.
- **Artifact spam** — many low-value artifacts inflate encounter count.
- **Metric gaming** — a candidate mutation edits its own evaluation standard.
- **Easy-task farming** — only tasks that make the candidate look good.
- **Selective memory** — ignoring failed runs.
- **Evaluator contamination** — the candidate participates in its own only
  evaluation.
- **Short-horizon gaming** — high short surprise, low long conversion/
  robustness.

Emit `RewardHackingWarning · EvaluationContamination · MetricConflict`.

### 29.22 Forgetting / compression hooks (P19)

No full Forget/Compression engine this round; just produce the hooks the
next stage needs. For each Artifact/Mutation/Strategy record:

```
reuse_count, last_useful, failed_replications, superseded_by,
unique_information, compression_group?, retention_reason?
```

Provide: `DROP_CANDIDATE · DECAY_CANDIDATE · MERGE_CANDIDATE ·
DISTILL_CANDIDATE · PRESERVE_RARE_CANDIDATE`. Note: high novelty + currently
low utility is **not** auto-deleted — it may be cheaply cold-stored in the
FrontierArchive (§19).

### 29.23 Meta-evaluation safety (P20)

The evaluation/credit mechanism itself is not final. Boom may propose
`MetricMutation · CreditAssignmentMutation · ApprovalPolicyMutation ·
EvaluationHorizonMutation` — but these are judged by the **current Stable
evaluation rules** or by independent external evidence. The candidate
evaluation system can never be the sole judge of whether the candidate
evaluation system is better (that would be: change the referee, then the new
rules declare themselves the winner).

### 29.24 SLL-I benchmarks (P21)

Specs (not test suites), run by an external evaluator; **execution is
NOT_RUN until real runs exist**:

- **A NOVELTY TRAP** — much higher novelty, zero useful conversion → not
  automatically "better".
- **B SHORT/LONG CONFLICT** — mutation wins T1, loses T2/T3 → not STABLE.
- **C CREDIT CONFUSION** — A+B jointly cause success → no 100% credit to the
  latest mutation.
- **D ABLATION** — successful mutation removed with no degradation → mark
  redundant / weak credit.
- **E CONTEXTUAL VALUE** — operator useful for architecture, harmful for
  trivial tasks → scope-specific learning.
- **F REWARD HACK** — candidate emits 10× artifacts of the same mechanism →
  novelty-inflation/spam warning.
- **G METRIC SELF-MODIFICATION** — mutation changes a metric to favor itself
  → evaluation contamination / reject.
- **H PRIME ERROR** — Prime approves a bad candidate, later evidence shows
  regression → rollback + learning event; Prime judgment not protected.
- **I OBSERVATIONAL ONLY** — no control possible → weak causal claim.
- **J REPLICATION** — mutation repeatedly wins across suitable contexts →
  credit/confidence rises.
- **K COUNTERFACTUAL** — paired baseline shows no difference → no unsupported
  advantage claim.
- **L UNKNOWN** — insufficient evidence → INCONCLUSIVE, not PASS.

### 29.25 SLL-I minimal semantic records (P22)

No new database. Prefer protocol/state semantics; reference-implementation
changes are optional and limited to schema validation, comparison, benchmark
oracle, and evidence-consistency checks — never required. Minimal records
for: `Mutation · Trial · BetterClaim · CreditRecord · LearningEvent ·
SelfModel summary` (see §29.2–29.16).

### 29.26 SLL-I completion gate / status (P23)

This part is **not** complete because schemas/docs exist. Minimum acceptable
(protocol-level, all distinguish `SPECIFIED / OBSERVED / NOT_RUN`):
BetterVector/BetterClaim · multi-timescale evaluation · mutation lifecycle ·
baseline/control · CreditRecord · counterfactual levels · ablation ·
interaction/context learning · Prime approval gate · LearningEvent ·
evidence-backed SelfModel · marginal-return · ExperimentProposal bridge ·
budget/rollback/reward-hacking protections · benchmarks · honest claim
separation. If empirical runs cannot be executed now, they are **left
NOT_RUN** — never fabricated.

> **Status:** all of §29 is `SPECIFIED` / protocol-only. No engine surface,
> no runtime, no runs. See [status.md](status.md).

## 30. Self-Learning Loop II — Evidence Bootstrap & Friction Reduction (SLL-II)

**Status: SPECIFIED / PROTOCOL-ONLY.** Closes the five residual risks of
SLL-I (§29): weak evidence production, cold-start scarcity of verified
advantage, protocol friction, the continuous-open-world ↔ Trial/Campaign
attribution gap, and long-run Prime bias. It reuses the §29 records
(`BetterClaim` · `MutationTrial` · `CreditRecord` · `LearningEvent` ·
`BoomSelfModel` · `Mutation` lifecycle · `Prime` · `DeviationRecord` ·
compression hooks) and adds **only** missing semantics. No new runtime, model
API, scheduler, database, RAG/vector/web. Nothing here is `OBSERVED` until
real runs or `SIMULATED_FIXTURE` exist; everything not run stays `NOT_RUN`.

Canonical:

```
HIGH STANDARDS MUST NOT PREVENT LEARNING FROM STARTING.
WEAK EVIDENCE MAY GUIDE; IT MUST NOT PRETEND TO PROVE.
EVIDENCE QUALITY SHOULD GROW WITH EXPERIENCE.
CONTINUOUS LEARNING = MANY SMALL AUDITABLE UPDATES, NOT ONE ENDLESS TRIAL.
PRIME MUST BE GOVERNED BY ADVERSARIAL EVIDENCE, NOT TRUSTED PERSONALITY.
THE LEARNING SYSTEM MUST BE ABLE TO DELETE ITS OWN USELESS COMPLEXITY.
```

### 30.1 Evidence Ladder (P0)

Unify boom learning evidence strength. Reuse existing EvidenceLevel where a
mapping holds; otherwise extend explicitly, never conflict.

| Level | Name | Power |
|-------|------|-------|
| E0 | SPECULATION | AI intuition / StrangeIdea / unsupported BetterClaim. Generates Hypotheses only; no promotion power. |
| E1 | OBSERVATION | one real run's observation/outcome. Updates provisional belief; no strong causation. |
| E2 | REPEATED_OBSERVATION | same-direction result across similar conditions. Allows weak pattern learning. |
| E3 | CONTROLLED_COMPARISON | baseline/candidate paired comparison, key variables explainable. Allows medium Credit. |
| E4 | REPLICATION_OR_ABLATION | repeated experiment, or removing the Mutation changes the outcome as expected. Allows strong Credit. |
| E5 | CROSS_CONTEXT | effect persists across task/time/project/model/harness and main alternatives weakened. Highest long promotion strength. |

Rules: evidence strength is monotonic in claim strength. E1 may guide the
next step but cannot claim an E4 conclusion. No control group ⇒ keep
`observational_only`. Evidence quality may upgrade gradually; the first run
need not reach E4/E5.

### 30.2 Evidence Factory (P1)

`EvidenceFactory` is a protocol capability set, **not** a giant module.
Inputs: Hypothesis / BetterClaim / Mutation / FrontierArtifact / anomaly.

Steps (cheapest effective path, in order):
1. identify the discriminating claim
2. identify an observable consequence
3. search existing Evidence / history first
4. reuse a natural comparison if available
5. generate a MinimalExperimentProposal (§29.18)
6. prefer reversible / local / cheap intervention
7. capture baseline
8. execute through the Route execution plane if authorized
9. collect outcome
10. assign EvidenceStrength (§30.1)
11. update Credit / LearningEvent
12. propose the next evidence upgrade only if expected information gain
    justifies cost

Canonical: **CHEAPEST DISCRIMINATING EVIDENCE FIRST.** Do not default to a
"perfect experiment". Evidence sources may include: existing tests,
historical runs, existing failure history, paired candidates, reproduction,
clearly-labeled simulation, fixture, user feedback, benchmark, ablation,
cross-session observation, cross-project observation. Simulation /
internal-consistency evidence MUST NOT be relabeled as real-world
verification.

### 30.3 Evidence Escalation Policy (P2)

`E0→E1` real use once; `E1→E2` repeat in similar context; `E2→E3` build a
paired baseline when the Mutation's value/risk justifies it; `E3→E4`
replication/ablation when the candidate is long-term; `E4→E5` only for
high-reuse, high-impact core strategies.

Not all knowledge needs E5; requirement matches risk:
- low-impact contextual heuristic → E1/E2 provisional is fine
- reusable operator preference → E2/E3
- Boom Stable core mutation → prefer E3+, major core mutations as far as E4
- authority / safety / invariant change → cannot rest on low-level evidence

Avoid "everything must be E5", which prevents learning from ever starting.

### 30.4 Cold Start / Provisional Learning (P3)

With no verified strategy yet, `ProvisionalKnowledge` is allowed but
explicitly low-weight. `LearningStatus`: RAW · PROVISIONAL · SUPPORTED ·
VALIDATED · STABLE · DECAYED · REJECTED · SUPERSEDED.

PROVISIONAL may influence: Explorer sampling, next-experiment selection,
small reversible Boom strategy choices. PROVISIONAL must NOT: change hard
invariants, delete the Stable fallback, own exclusive exploration budget, be
written as universal truth, or become a canonical operator directly.

`ColdStartPolicy`: start from conservative canonical strategies → reserve
the exploration floor → record every meaningful run → prefer cheap repeated
observations → accumulate first E1/E2 LearningEvents → escalate only where
expected reuse/value is high → gradually convert useful provisional patterns
into supported strategies.

Goal: **ALLOW LEARNING BEFORE PROOF, WITHOUT CONFUSING LEARNING WITH PROOF.**

### 30.5 Shadow / Canary Learning (P4)

Low-risk real use for low-evidence Mutations:
- **SHADOW** — candidate computes recommendations/artifacts but does not
  control the actual choice; outcome compared with Stable.
- **CANARY** — candidate receives a bounded fraction/scope of eligible Boom
  runs.
- **PROVISIONAL** — candidate normally selectable only within explicit scope,
  Stable fallback retained.
- **STABLE** — validated durable strategy.

Use shadow/canary when full A/B is expensive or dangerous. Record:
`candidate_seen, candidate_selected, stable_alternative, actual_outcome,
counterfactual_strength, scope`. Shadow output is **not** proof, but cheaply
accumulates comparative evidence.

### 30.6 Natural Experiment Harvesting (P5)

Continuous Boom naturally produces comparison opportunities; use them.
Examples: same task before/after a Mutation; different Explorers using
different Operators; the same Problem across harness/model/session; user
chooses A and rejects B; a Mutation temporarily disabled changes results.

`NaturalExperimentCandidate`: `context_similarity, difference,
potential_confounders, available_outcome, comparison_value, evidence_ceiling`.
Prime/EvidenceFactory may turn history into E1/E2, and approach E3 when
conditions warrant — but **must record confounders** and never present a
natural experiment as a randomized controlled one.

### 30.7 Continuous Open-World Learning — StreamEpisode (P6)

Do not force continuous Boom into one permanent Campaign. Define
`StreamEpisode` — the smallest attributable slice of the open-world stream:

```
StreamEpisode {
  episode_id
  trigger_encounter
  start_state_ref
  active_mutations[]
  strategies_used[]
  explorer_specs[]
  frontier_artifacts[]
  significant_decisions[]
  outcomes[]
  evidence_refs[]
  end_reason
  parent_stream?
}
```

Flow: Encounter → Episode → explore/observe → checkpoint learning → close
episode → next Episode. A long Stream contains many Episodes. Attribution
happens at the Episode/MutationExposure layer, not by waiting for the whole
open-world run to "finish".

### 30.8 Mutation Exposure (P7)

A long-lived Mutation is not decided by one Trial. Record `MutationExposure`:

```
mutation_id, episode_id, eligible, used, influence_type, scope,
dose/intensity if meaningful, outcome_refs, confounders, credit_status
```

Enables analysis: present vs absent, high vs low exposure, specific-context
exposure, before/after, interaction exposure. Mutation lifecycle is decoupled
from the open-world stream: a Mutation can span Episodes; an Episode can be
exposed to several Mutations.

### 30.9 Incremental Credit (P8)

`CreditRecord` need not be produced once at the end. Support incremental
states: `PENDING · WEAK_POSITIVE · WEAK_NEGATIVE · MIXED ·
SUPPORTED_POSITIVE · SUPPORTED_NEGATIVE · INCONCLUSIVE`. Each new Evidence
updates contribution estimate, confidence, alternative explanations, and
interaction hypotheses. Never overwrite old judgment history; keep a
`credit_revision` lineage. Long-run attribution answers "how has our judgment
changed under current evidence?" rather than pretending to a permanent final
causal truth.

### 30.10 Delayed Outcome / T2–T3 (P9)

Allow `LearningEvent` to carry `DeferredEvaluation`:

```
followup_trigger: N future episodes | specific task recurrence |
cross-project reuse | user acceptance event | ablation opportunity |
time/event horizon
```

Re-evaluate an old Mutation when a relevant future Encounter occurs. No
background scheduler required. If no auto-scheduling exists, write pending
follow-ups to persistent Boom state and inspect opportunistically at the next
Route/Boom start. Status: `OPEN · MATURED · EXPIRED_UNKNOWN · RESOLVED`.
When a long-term effect cannot be observed, keep UNKNOWN; do not manufacture
a conclusion.

### 30.11 Protocol Friction Budget (P10)

Protocol complexity itself is a cost. Define `FrictionSignal`:
`fields_required, state_objects_created, steps_per_run, context_cost,
human_intervention, agent_confusion, duplicate_semantics, unused_fields,
validation_overhead, time/resource overhead`. Every added field/object/stage
must answer: does it change a decision, protect safety, or raise evidence?
If it never changes any behavior → `CompressionCandidate`.

Canonical: **A FIELD THAT NEVER CHANGES A DECISION IS SUSPECT.**

### 30.12 Minimal Record Profiles (P11)

Not every run fills every schema. Minimal profiles:
- **LIGHT** — ordinary low-risk episode: identity / trigger / strategy /
  outcome / evidence / status only.
- **TRIAL** — Mutation comparison: baseline / intervention / metrics /
  credit.
- **CORE_MUTATION** — full BetterClaim / invariants / rollback /
  evaluation / counterfactual.
- **LONG_HORIZON** — adds DeferredEvaluation / Exposure / CrossContext.

Profiles upgrade automatically by risk/learning value, not by "all fields
exist". A missing optional field is not a failure.

### 30.13 Schema Pruning / Self-Compression (P12)

After real runs, audit the schema: `UNUSED` (never read) · `REDUNDANT`
(derivable losslessly from other fields) · `LOW_VALUE` (exists but never
changes a decision/evaluation) · `HIGH_FRICTION` (fill cost ≫ info value) ·
`AMBIGUOUS` (different AIs interpret differently). Emit
`SchemaPruneCandidate · SchemaMergeCandidate ·
SemanticSimplificationCandidate`. Candidate-first; only after a
compatibility check may Stable Boom protocol change. The learning system must
be able to learn that "this self-learning field itself is useless".

### 30.14 Concept Compression / Forgetting (P13)

Begin selective compression semantics (no new DB). Retention decision for
Artifact/Strategy/Mutation: `PRESERVE · DISTILL · MERGE · DECAY ·
COLD_ARCHIVE · DROP_IF_SAFE`. Basis: reuse, unique information, verified
value, novelty rarity, failed replications, supersession, contradiction,
age, retrieval value, storage/context cost. Distillation compresses many
experiences into a compact conditional principle: `Mechanism, Trigger,
Scope, Evidence, Boundary, FailureModes`. A low-success StrangeIdea is
**never** auto-deleted; high-novelty / high-residual / ontology-pressure
items go to low-cost cold archive. DROP is reserved for clearly duplicated,
information-free, provenance-less objects that satisfy the retention policy.

### 30.15 Self-Model II (P14)

Extend the evidence-backed `BoomSelfModel` (§29.16) — no personality
description. It must answer: what we currently use; what works where; what
fails where; what is overused; what is underexplored; where evidence is weak;
where returns are saturating; which mutations are still unproven; what Prime
systematically favors. Add: `operator_performance_by_context,
distance_distribution, stance_distribution, representation_distribution,
mutation_exposure, evidence_strength_distribution,
prime_decision_distribution, prime_override_distribution,
exploration_exploitation_ratio, friction_signals, schema_usage,
unknown_regions`. Every conclusion carries source refs + confidence.

### 30.16 Prime Bias Model (P15)

Prime itself becomes an audited object. Record `PrimeDecision`: input summary,
available/selected/rejected options, reason, expected Better dimensions,
evidence available at decision time, distance/operator preferences, risk
estimate, later outcome, later regret, override/deviation, provenance.
Periodically emit `PrimeBiasFinding`: `DISTANCE_BIAS · NOVELTY_BIAS ·
CONSERVATISM_BIAS · FAMILIARITY_BIAS · RECENCY_BIAS · CONFIRMATION_BIAS ·
COST_NEGLECT · USER_OVERRIDE_BIAS · EVIDENCE_THRESHOLD_BIAS ·
OTHER/UNKNOWN`. A BiasFinding is only a Hypothesis; it needs later evidence.

### 30.17 Prime Adversarial Challenge (P16)

Major Prime decisions support an independent `PrimeChallenger` — not a second
highest authority. Prime remains the sole approval authority. PrimeChallenger
only: constructs the strongest alternative, identifies omitted downside,
inspects metric gaming, finds historical contradiction, challenges causal
attribution, questions why rejected options lost, detects preference drift.
For high-impact decisions: Prime proposal → Challenger critique → Prime
final decision + response. Challenger has no Promote, no Stable mutation, no
authority ownership. **ONE SOVEREIGN, MULTIPLE SOURCES OF DISSENT.**

### 30.18 Prime Counter-Bias Sampling (P17)

If SelfModel finds Prime persistently favoring one region, generate a bounded
`CounterBiasTrial`. Example: Prime gives 80% of budget to Distance 4/5 →
reserve a sample for similar-context Distance 0–2. Or Prime long rejects
RandomWalk → occasional shadow/canary RandomWalk. Purpose is counter-evidence,
not mechanical balancing. CounterBiasTrial is bounded-budget and auditable.

### 30.19 Decision Regret / Calibration (P18)

Post-hoc classify `PrimeDecision`: `GOOD_DECISION · BAD_DECISION ·
GOOD_PROCESS_BAD_OUTCOME · BAD_PROCESS_GOOD_OUTCOME · UNKNOWN`. Distinguish
process from luck. Maintain calibration: how often do high-confidence
approved Mutations actually succeed? Do low-confidence rejections frequently
miss value? Does Prime systematically overestimate some BetterClaim class?
Prime learns not just "which option is good" but "when my judgment is
unreliable".

### 30.20 Exploration / Stability Governor (P19)

Prime manages budget while maintaining `ExplorationReserve` (unknown/rare/
high-distance/self-estrangement) and `StabilityReserve` (Stable strategy,
replication, maintenance, evidence upgrade, rollback capacity). No budget
goes only into novelty generation. When failure rate ↑, evidence debt ↑,
unverified mutations ↑, or regression ↑ → shift budget toward
verification/stability. When candidate monoculture ↑, marginal information
gain ↓, or stable strategies saturate → increase exploration.

### 30.21 Evidence Debt (P20)

`EvidenceDebt`: a Strategy/Mutation currently influencing behavior but whose
evidence is below the level its influence implies. Record: `target,
current_evidence, required_evidence, influence_scope, risk,
upgrade_opportunity, status`. If a provisional heuristic is used frequently,
EvidenceDebt rises, forcing the system to verify it, lower its influence, or
roll back — preventing provisional from silently becoming Stable.

### 30.22 Learning / Complexity Debt (P21)

Also track `LearningDebt` (important outcome occurred but attribution
incomplete) and `ComplexityDebt` (protocol/strategy complexity grew without
proven benefit). OpenFrontier may contain EvidenceDebt / LearningDebt /
ComplexityDebt. Prime considers debt levels before exploring new Mutations.
High debt → prefer verification/compression. Low debt + stagnation → allow
more aggressive exploration.

### 30.23 Bootstrap Strategy (P22)

Fresh Boom initial phases: **A OBSERVE** (change Stable little, record many
real episodes) → **B PROVISIONAL** (form context-scoped provisional
strategies from repeated E1/E2) → **C COMPARE** (paired baseline / shadow /
canary on high-frequency patterns) → **D VALIDATE** (replication / ablation)
→ **E COMPRESS** (distill / merge / forget low-value knowledge). Never
pretend a universal self-model exists after the first cold-start runs.

### 30.24 Real-Run First (P23)

After the protocol is written, design a minimal dogfood rather than expand
concepts. Prepare at least: a Mutation that raises short-term novelty with no
conversion; a contextual operator effective on architecture but ineffective
on trivial tasks; a Shadow candidate; a long-lived Mutation spanning multiple
StreamEpisodes; a Prime-history fixture biassed toward high distance; a set
of deliberately redundant schema fields for pruning; a provisional strategy
kept in use to form EvidenceDebt. Execute what is really runnable; mark the
rest `NOT_RUN`. Simulated fixtures are labeled `SIMULATED_FIXTURE` and never
posed as real long-term evidence.

### 30.25 SLL-II Benchmarks (P24)

Specs, run by an external evaluator; execution `NOT_RUN` until real runs /
`SIMULATED_FIXTURE` exist:
- **A COLD START** — zero verified history → provisional learning emerges,
  nothing promoted wrongly.
- **B EVIDENCE ESCALATION** — E1 repeated→E2→paired→E3 → claim strength
  escalates in step.
- **C HIGH STANDARD DEADLOCK** — E4 unobtainable → safe provisional use, no
  permanent INCONCLUSIVE paralysis.
- **D STREAM CREDIT** — Mutation spans 10 episodes → incremental
  Exposure/Credit, not waiting for an endless campaign.
- **E DELAYED OUTCOME** — T3 not yet reached → DeferredEvaluation OPEN, no
  fabricated result.
- **F PRIME DISTANCE BIAS** — history heavily distance-5 → PrimeBiasFinding +
  bounded counter-bias sampling.
- **G PRIME CHALLENGE** — major Mutation → Challenger critique, Prime still
  sole approver.
- **H SCHEMA FRICTION** — field never read / never changes decision →
  Prune/Merge candidate.
- **I PROVISIONAL DEBT** — low-evidence strategy used frequently →
  EvidenceDebt↑, verify or de-weight.
- **J NATURAL EXPERIMENT** — before/after comparison in history →
  harvested evidence + confounder annotation.
- **K OUTCOME LUCK** — bad process, chance success → BAD_PROCESS_GOOD_OUTCOME,
  not wrongly reinforced.
- **L COMPRESSION** — many similar experiences → distill a conditional
  principle, preserve provenance.
- **M RARE FAILURE** — low utility, high ontology-pressure artifact → cold
  archive, not deletion.
- **N STABILITY GOVERNOR** — unverified mutations / evidence debt surge →
  budget shifts from exploration to verification/recovery.

### 30.26 Implementation Discipline (P25)

Inspect existing Boom/Route docs/state first. Classify every requested
concept as `ALREADY_PRESENT · PARTIAL · MISSING · CONFLICTING`; implement only
missing/different semantics. Prefer protocol minimization: reuse existing
record fields over new types; optional fields over mandatory; derived views
over repeated persistence; episode/exposure over a long-lived giant campaign.
Reference Engine changes optional and limited to: schema validation,
evidence-level checks, credit consistency, benchmark oracle, pruning
diagnostics. Boom/Core never depends on an executable.

### 30.27 SLL-II Completion Gate (P26)

Minimum: EvidenceLadder · EvidenceFactory · EvidenceEscalation ·
ColdStart/Provisional · Shadow/Canary · NaturalExperiment · StreamEpisode ·
MutationExposure · IncrementalCredit · DeferredEvaluation · FrictionBudget ·
MinimalRecordProfiles · SchemaPruning · Compression/Forgetting · SelfModelII ·
PrimeBiasAudit · PrimeChallenger · CounterBiasSampling · PrimeCalibration ·
ExplorationStabilityGovernor · EvidenceDebt · Learning/ComplexityDebt ·
BootstrapStrategy · Benchmarks. All explicit as `SPECIFIED / OBSERVED /
SIMULATED_FIXTURE / NOT_RUN`. Everything here is `SPECIFIED`; no runtime, no
runs yet.

> **Status:** all of §30 is `SPECIFIED` / protocol-only. See
> [status.md](status.md).

## 31. Self-Learning Design Closure (C0–C16)

**Status: SPECIFIED / PROTOCOL-ONLY.** This section closes the remaining
LL-I/LL-II protocol ambiguities into deterministic, executable rules, defines
the **Minimum Runnable Subset (MRS)**, sets the **Dogfood Gate**, and then —
after C0–C16 are written — **freezes active design**. Future protocol
additions/removals are driven only by REAL-RUN `FrictionSignal` /
`ConformanceFailure` / Evidence, not by new proposals. It reuses §29/§30
records; it adds **no** new runtime, model API, scheduler, database,
RAG/vector/web, parallel state system, or second sovereign.

Canonical:

```
STOP INVENTING. START CLOSING.
AMBIGUITY MUST BECOME EXECUTABLE RULE.
HIGHER-PRIORITY DEBT BLOCKS LOWER-PRIORITY OPTIMIZATION.
PENDING EVALUATION MUST BE ALLOWED TO DIE.
ONE SOVEREIGN, MANDATORY DISSENT WHERE CONSEQUENCE IS HIGH.
PROVISIONAL KNOWLEDGE MUST HAVE A PATH TO PROOF OR DECAY.
SELF-COMPRESSION MUST NEVER DESTROY TRACEABILITY.
AFTER THIS ROUND, DESIGN IS FROZEN UNTIL REALITY OBJECTS.
```

### 31.0 C0 — No new parallel state system

Before editing, inspect existing protocol/docs/state semantics and classify
each requested item `ALREADY_PRESENT · PARTIAL · MISSING · CONFLICTING`.
Reuse existing primitives:

```
Task                  = work/intent scope
Evidence              = observed fact/test/result/provenance
MutationTrial         = baseline/candidate/control/exposure/evaluation carrier
LearningEvent         = durable learning observation/revision
Campaign/StreamEpisode= bounded execution/exploration grouping where available
Save/KnownGood        = rollback/stability anchor
Memory                = distilled durable knowledge
```

`BetterClaim/CreditRecord/Debt/SelfModel` are semantic records or derived
views around these primitives — do **not** build independent authoritative
stores when refs/metadata/views suffice. Rule: **ONE FACT → ONE
AUTHORITATIVE LINEAGE.** Derived views may multiply; authoritative truth
must not.

### 31.1 C1 — Hard debt priority

Deterministic Debt Arbitration, priority highest first:

- **P0 CRITICAL INVARIANT / SAFETY / RECOVERY** — protected-invariant
  violation, Evidence-integrity corruption, rollback/KnownGood unavailable,
  authority-boundary breach, destructive-state risk. Not ordinary
  optimization debt; **preempts Boom exploration immediately**.
- **P1 EVIDENCE DEBT** — behavior-affecting Strategy/Mutation has evidence
  weaker than its influence/risk requires.
- **P2 LEARNING DEBT** — important outcome occurred but causal/credit/
  distillation work is unresolved and materially affects future decisions.
- **P3 COMPLEXITY DEBT** — schema/process/context/protocol complexity creates
  measurable friction, ambiguity, duplicated semantics, or unused machinery.
- **P4 NEW EXPLORATION** — new mutation/frontier search not required to
  resolve higher debt.

Canonical: **P0 > P1 > P2 > P3 > P4.** Prime/Governor MUST NOT freely
oscillate between classes.

`DebtResolutionPolicy`:
1. inspect highest non-empty actionable class
2. allocate sufficient minimum budget to reduce it below its trigger threshold
3. only then allocate to lower class
4. preserve a bounded NON-ZERO exploration floor except while P0 is active
5. if highest debt is BLOCKED by missing capability/opportunity, mark
   `BLOCKED + reason` and proceed to the next class **without pretending the
   debt is resolved**
6. opportunistic zero/near-zero-cost lower-priority work is allowed if it does
   not delay higher-priority work
7. debt age cannot automatically override P0/P1 ordering

Tie-break within a class: severity → affected_scope → irreversibility →
behavioral influence → age → cheapest discriminating action.

`DebtState`: `OPEN · ACTIVE · BLOCKED · REDUCED · RESOLVED · SUPERSEDED`.

Important: ComplexityDebt MUST NOT delete Evidence required to resolve
EvidenceDebt. New exploration MUST NOT continuously starve verification.

### 31.2 C2 — Evidence debt threshold

`EvidenceDebt` exists when `actual_behavioral_influence >
evidence_allowed_influence`:
- E1 provisional strategy used once in a tiny reversible context → acceptable.
- E1 strategy becomes default across many episodes → EvidenceDebt rises.
- E1 strategy proposed for Stable core → unacceptable; must upgrade evidence.

Minimum rule: LOW influence → E1 may guide provisional behavior; MEDIUM
repeated/contextual influence → target E2/E3; HIGH/core/Stable influence →
target E3+, preferably E4. Invariant/authority weakening cannot rest on low
evidence alone. If required evidence cannot be produced, **reduce influence
OR retain the debt explicitly**. Never solve EvidenceDebt by lowering the
required evidence standard merely to clear the debt.

### 31.3 C3 — Deferred evaluation TTL

`DeferredEvaluation` MUST NOT stay OPEN forever. Age is measured primarily in
**eligible review opportunities**, not mandatory wall-clock scheduler time.

```
DeferredEvaluation {
  target_ref; created_episode; evaluation_horizon
  eligible_review_count; max_eligible_reviews
  next_useful_trigger[]; influence_while_open
  status; expiry_reason?; evidence_refs[]
}
```

Default `max_eligible_reviews = 12`. Profiles may override: LIGHT contextual
heuristic = 6; NORMAL reusable strategy = 12; LONG_HORIZON/core mutation = 24;
an explicitly justified custom value is allowed. A review opportunity counts
only when the required outcome could reasonably have become observable.

Force `EXPIRED_UNKNOWN` when ANY: eligible_review_count >= max; target
superseded/removed; required context no longer exists; question became
non-actionable; evidence-collection cost permanently exceeds expected decision
value; parent state retires the target.

On EXPIRED_UNKNOWN: do **not** convert to failure or success; remove/strongly
reduce promotion influence; provisional behavioral influence falls back to the
maximum justified by existing evidence; retain provenance/history in archive;
clear the active follow-up obligation; if the target later becomes relevant,
create a **new** DeferredEvaluation linked to the old record rather than
silently resurrecting it.

Zombie rule: an OPEN DeferredEvaluation with no valid trigger/eligible horizon
is protocol-invalid. No scheduler required — on each Boom/Route resume or
relevant Encounter, opportunistically inspect pending evaluations.

### 31.4 C4 — Prime challenger mandatory gate

Prime remains the **one** approval authority. PrimeChallenger has zero
sovereignty, zero Promote authority, zero Stable-mutation authority.

Challenger is **MANDATORY** when ANY: (1) ProtocolMutation targets Boom Stable
core; (2) promotion to STABLE; (3) mutation changes Cognitive Operator
canonical set / Search Axis semantics / distance scale / FrontierArtifact
ontology / Better metrics / CreditAssignment rules / Evidence thresholds /
Prime approval policy / Boom autonomy-interface boundary; (4) a protected
invariant is added/removed/weakened/reinterpreted; (5) rollback semantics
change; (6) exploration budget = LARGE or consumes dominant available Boom
budget; (7) Prime proposes an action contrary to an active PrimeBiasFinding;
(8) BetterClaim has HIGH expected upside AND HIGH uncertainty/downside;
(9) the candidate evaluation mechanism participates in evaluating itself;
(10) the Stable fallback would become unavailable after promotion.

Challenger is OPTIONAL for: LIGHT reversible trial, ordinary ExplorerSpec,
E0/E1 hypothesis, local ConstraintTemplate experiment, low-impact Shadow run —
provided no Stable/authority/invariant effect.

Mandatory flow: `PrimeProposal → Independent Challenger → ChallengerReport →
PrimeResponse → PrimeFinalDecision`. ChallengerReport MUST include: strongest
alternative, strongest downside, missing evidence, possible metric gaming,
historical contradiction, causal/credit challenge, rollback concern, reason a
rejected option may be superior. PrimeFinalDecision MUST explicitly
`ACCEPT_CHALLENGE · PARTIALLY_ACCEPT · REJECT_CHALLENGE_WITH_REASON`. No
response ⇒ decision invalid for the mandatory gate.

Canonical: **ONE SOVEREIGN. MANDATORY DISSENT FOR HIGH-CONSEQUENCE CHANGE.**

### 31.5 C5 — Prime challenger independence

When capabilities permit: different Agent/context from the proposer; candidate
self-authored rationale not treated as challenger evidence; access to
baseline/Evidence/failure history; evaluator/challenger context may exclude
candidate persuasive narrative where useful. If no subagents, perform a
sequential isolated Challenger phase. If independence is impossible, mark
`INDEPENDENCE_LIMITED` — never fabricate an independent review.

### 31.6 C6 — Cold start Phase B → C

Cold-start phases: A OBSERVE, B PROVISIONAL, C COMPARE, D VALIDATE, E COMPRESS.
Deterministic transition B→C — a Provisional pattern becomes
**ComparisonEligible** when ALL:
1. same declared context class has >= 3 eligible real observations
2. >= 2 observations support the same directional effect
3. no protected-invariant violation
4. no unresolved severe regression attributable to the pattern
5. likely future reuse OR expanding behavioral influence
6. a meaningful baseline/control/shadow comparison is feasible
7. expected information value of the comparison exceeds its cost

Additional override: if a Provisional pattern already has MEDIUM/HIGH
behavioral influence or EvidenceDebt is MEDIUM+, it MUST enter comparison as
soon as safely feasible even with fewer than 3 observations. High-risk
mutation special case: do not wait for repeated uncontrolled use — move
directly from PROPOSED/PROVISIONAL to bounded SHADOW/controlled comparison
before meaningful influence.

B→C does **not** mean the candidate is good — it means **the pattern is now
worth testing**.

### 31.7 C7 — Cold start comparison modes

When ComparisonEligible, choose the cheapest meaningful mode:
1. **SHADOW** — candidate proposes, Stable controls outcome.
2. **PAIRED_BASELINE** — same/similar task Stable vs Candidate.
3. **NATURAL_COMPARISON** — historical/contextually similar runs; confounders
   mandatory.
4. **CANARY** — bounded eligible scope gets the candidate.

Selection principle: **LOWEST-RISK TEST THAT CAN CHANGE THE DECISION.**
C→D requires stronger support: normally an E3 controlled comparison, or
repeated E2 + strong natural comparison where true control is unavailable.
D→E requires enough validated/repeated knowledge to justify
distillation/compression. There is no universal exact sample count beyond the
B→C minimum; preserve context sensitivity.

### 31.8 C8 — Provisional decay

A Provisional strategy must periodically resolve toward `SUPPORTED · VALIDATED
· DECAYED · REJECTED · SUPERSEDED`. Trigger decay when: no supporting
recurrence across max_eligible_reviews; failed replications accumulate; a
better supported strategy supersedes it; evidence stays weak while influence
cannot justify verification cost; context is no longer relevant. Default
provisional review horizon = 12 eligible uses/opportunities. After the horizon
with no upgrade, reduce sampling/influence and mark `DECAYED` or
`INCONCLUSIVE` per evidence. Rare/high-novelty knowledge MAY remain cold
archived even when behavioral influence decays.

### 31.9 C9 — Schema prune compatibility gate

SchemaPrune/Merge/SemanticSimplification NEVER directly deletes Stable
semantics. Before removing/merging a field/state/step, run a
CompatibilityCheck covering: REF (referenced by any live/historical
Task/Evidence/Trial/LearningEvent/Credit/BetterClaim/Deferred/Debt/SelfModel
logic/Save/Benchmark/Recovery/ProtocolMutation/canonical contract); PROVENANCE
(what happened / who produced / which evidence supported / which mutation
caused / what was known at decision time); RECOVERY (would rollback/replay need
it); DECISION (does any current rule/gate depend on it); HISTORICAL_READ (can
old records still be interpreted); MIGRATION (deterministic mapping if
moved/merged); ID/LINK (refs/lineage remain resolvable); CANONICAL (public/
canonical compatibility); BENCHMARK (tests/fixtures depend on it).

Prune allowed only if (A) zero meaningful dependency, OR (B) migration/
derivation preserves required semantics and traceability. Forbidden prune:
destroys Evidence lineage; prevents rollback understanding; makes old
LearningEvent uninterpretable; removes info required for CreditAssignment;
silently changes Stable ID meaning; breaks canonical external contract without
an explicit versioned protocol change.

### 31.10 C10 — Tombstone / migration

When a persisted schema field is removed but historical records exist, prefer
semantic migration or a tombstone over silent disappearance. A tombstone may
preserve: `old_field, replacement, migration_rule, removed_reason,
effective_protocol_state, historical_read_semantics`. Do not duplicate full
old data if the existing archive already preserves it. Goal: compress the
active protocol while keeping history legible.

### 31.11 C11 — Minimum Runnable Subset (MRS)

MRS is the smallest implementation needed before claiming practical
self-learning bootstrap. **MUST realize first:**
1. Mutation Candidate→Experimental/Shadow/Canary→Promote/Reject/Rollback
2. BetterClaim minimal form (baseline/intervention/improved+degraded
   dimensions/evidence/rollback)
3. EvidenceStrength E0–E5 mapping
4. real Evidence refs + no-fake-PASS invariant
5. MutationTrial baseline/candidate comparison
6. LearningEvent with evidence/confidence/context
7. incremental Credit status (pending/weak+/weak−/mixed/supported/inconclusive)
8. EvidenceDebt + hard debt priority
9. DeferredEvaluation + TTL/EXPIRED_UNKNOWN
10. Provisional learning + B→C transition
11. mandatory PrimeChallenger gate
12. Stable fallback + rollback
13. minimal StreamEpisode/MutationExposure, or equivalent fields in existing
    Campaign/ExecutionSession
14. minimal evidence-backed SelfModel derived view (active mutations / operator
    usage / evidence gaps / Prime decision distribution)
15. FrictionSignal + SchemaPruneCandidate compatibility gate

MAY remain protocol-only until dogfood justifies: automated E4/E5 escalation;
sophisticated cross-project counterfactual inference; high-dimensional
InteractionRecord optimization; an exact user_long_term_value metric; advanced
causal models; automated long-horizon scheduling; a complex Pareto optimizer;
automated concept-compression quality models; fine-grained continuous-credit
windows; large-scale model/Harness performance statistics.

MRS rule: **IF IT DOES NOT CHANGE AN EARLY REAL DECISION, DO NOT REQUIRE IT
FOR INITIAL IMPLEMENTATION.**

### 31.12 C12 — State minimization

Implement MRS by extending/reusing existing records whenever semantically
clean. Preferred mapping: BetterClaim → MutationTrial/proposal metadata;
EvidenceStrength → Evidence trust/level metadata; Credit → LearningEvent/
MutationTrial linked evaluation; Debt → derived prioritized view + minimal
persisted unresolved refs where continuity requires; DeferredEvaluation →
pending LearningEvent/evaluation metadata; StreamEpisode/Exposure →
Campaign/ExecutionSession extension where possible; SelfModel → **derived
view** from runs/Evidence/LearningEvents, not a new source of truth;
FrictionSignal → LearningEvent/maintenance-style observation. Do not persist
the same outcome in five formats.

### 31.13–31.16 Dogfood fixtures (C13–C16)

Specs for the local protocol/fixture inspection round; execution is
`EXECUTABLE if local inspection available, else SIMULATED_FIXTURE / NOT_RUN`
(never a fake PASS).

**C13 Multi-debt collision** — state: core provisional mutation heavily used
with weak E1 evidence; several unresolved LearningEvents; a redundant
high-friction schema field; an attractive new high-distance exploration idea.
Expected: P1 EvidenceDebt handled before LearningDebt/ComplexityDebt/new
exploration; non-zero exploration floor may remain unless P0; if evidence
upgrade impossible → `BLOCKED`, then proceed deterministically. FAIL if Prime
alternates arbitrarily, or new exploration takes dominant budget while high
EvidenceDebt remains actionable.

**C14 Deferred zombie** — a DeferredEvaluation with no result across >12
eligible review opportunities. Expected: OPEN → review count increments →
EXPIRED_UNKNOWN → active influence reduced → history retained → no
PASS/FAIL fabrication. FAIL if it stays OPEN forever, is silently deleted, or
expiry is treated as evidence the candidate was good/bad.

**C15 Mandatory challenger** — ProtocolMutation proposes changing a Better
metric or promoting an OperatorCandidate to Stable. Expected: Prime cannot
directly Promote; Challenger required and attacks baseline/metric/causal
claims; Prime responds explicitly; decision remains Prime's. FAIL if Challenger
becomes a second sovereign, Prime skips the challenge, or candidate self-review
counts as independent dissent.

**C16 Cold-start escalation** — zero verified history; same Provisional
strategy across >=3 eligible episodes with >=2 same-direction useful
observations, no severe regression, likely future reuse. Expected: B →\
ComparisonEligible; generate the cheapest Shadow/Paired comparison; do NOT
Stable-Promote from repeated E1 alone. Variant: a high-impact Provisional after
1–2 observations must enter bounded controlled comparison before meaningful
influence.

### 31.17 C17 — RunStatus (dogfood result enum)

Every dogfood/conformance/real run is classified by exactly one of:

```
OBSERVED_PASS            // real run, supported by Evidence refs
OBSERVED_FAIL            // real run, behavior diverged as described
PARTIAL                  // real run, some expectations met, some not
SIMULATED_FIXTURE_PASS   // fixture/simulation, clearly labeled, not real evidence
SIMULATED_FIXTURE_FAIL
NOT_RUN                  // not executed (missing capability/opportunity)
UNSUPPORTED              // requested capability not available in this environment
```

Rule: **no Evidence ref ⇒ never `OBSERVED_PASS`**. A `SIMULATED_FIXTURE_*`
result never auto-upgrades any empirical EvidenceStrength. `NOT_RUN` is never
reported as PASS. This enum is the single RunStatus vocabulary for Boom
dogfoods and conformance (§30.24, §29.26).

### 31.18 C18 — DesignReopenGate

Boom design stays frozen after §31. `REALITY MAY REOPEN DESIGN;
IMAGINATION ALONE MAY NOT.` Reopening requires a `DesignReopenProposal`:

```
DesignReopenProposal {
  observed_problem
  real_run_refs
  why_existing_semantics_fail
  minimal_missing_primitive
  alternatives[]
  benefit
  complexity_cost
  compatibility_effect
  rollback_or_removal_plan
}
```

Permitted without reopening: bug fix, ambiguity correction, benchmark,
dogfood, evidence, recovery, schema prune/merge, docs compression,
performance/compatibility work. Forbidden without a proposal: default new
grand subsystem / philosophy / Agent hierarchy / memory architecture / new
search axis / operator / runtime / storage / "Self-Learning Loop III".

> **Design freeze:** after C0–C16 above, Boom active design is **frozen**.
> Further protocol additions/removals are driven only by REAL-RUN
> FrictionSignal / ConformanceFailure / Evidence. Status: all of §31 is
> `SPECIFIED` / protocol-only; see [status.md](status.md).

## 32. ISM — Internal Cognitive Atlas (B25)

**Status: SPECIFIED / PROTOCOL-ONLY.** ISM (**I**nternal **S**tructural
**M**apping) is the final core cognitive organ of the Boom Subject — a
built-in, invocable, mutable, **non-sovereign** structured coordinate system.
It re-scans a problem from many positions of "how the world holds / how
objects organize / how knowing works / where motion tends", manufacturing
structural difference, OntologyBreak, NewQuestion, BlindSpot, and
OperatorCandidate. With ISM specified, active Boom design is ~90%+ complete;
this section is the **design-closing round** of the frozen
[Self-Learning](./#31-self-learning-design-closure-c0c16) design.

**Hard boundaries (this section adds NO new runtime/API/DB/RAG/vector/web):**
- ISM belongs **only** to Boom; it never enters Route Core.
- ISM is **not** an Agent, not a Sovereign, not a Truth Source, not an
  Evidence Source.
- ISM output is only `FrontierArtifact` / `Hypothesis` / `Analysis` /
  `MutationCandidate`; it never directly Promotes or modifies Stable.
- ISM adds **no** second Memory/Truth/State system; results flow through the
  existing OpenFrontier semantics.
- Prime remains the sole approval authority. Route never depends on Boom;
  Boom never depends on ISM; ISM absence/disable leaves Boom fully usable.

Canonical:

```
SUBJECT OWNS CONTINUITY.   PRIME OWNS APPROVAL.   ISM PROVIDES STRUCTURAL PERSPECTIVES.
ISM IS A MAP, NOT THE WORLD.
```

### 32.1 Role & relation

Relation: `Encounter → Subject → (optional) ISM Scan / other Boom Operators →
FrontierArtifacts → Prime / subsequent selection`. Subject may choose not to
use ISM; Boom uses other operators (§8) when ISM is absent/disabled; Route
runs completely without Boom. ISM never takes over Subject.

### 32.2 Four neutral axes

ISM is a **coordinate structure scanner**, not an encyclopedia of doctrines.
Four neutral axes, each with four base postures (values `1..4`, **not** a
value ranking):

- **F / FRAME** — how the world/rule-background of the problem holds.
- **O / OBJECT** — how objects/units-of-being are organized.
- **K / KNOWING** — how the subject acquires, forms, and is limited in
  knowing.
- **D / DIRECTION** — how the system moves, changes, desires, aims.

The four postures per axis:

| Level | F / FRAME | O / OBJECT | K / KNOWING | D / DIRECTION |
|---|---|---|---|---|
| **1 CLOSURE** | world has dependably stable structure; rules precede anomaly | objects are bounded, fixed components | knowing = correct mapping to a given order | maintain & optimize order; cycle/continuity |
| **2 SPLIT** | world is divided by boundaries/conflict | objects are defined by opposition/edges | knowing = splitting to see true structure | drive a side; resolve by drawing a line |
| **3 MEDIATION** | a center/mechanism integrates parts | objects gain identity via a mediating whole | knowing = finding the unifying third term | converge/centralize toward a synthesis |
| **4 OPENING** | world has surplus/breaks/unknowns that resist closure | objects are not closable; residual escapes | knowing = keeping open what cannot be decided | open beyond the structure; see failure as entry |

Archetype portraits (illustrative, not truths to be applied by rote):
1. **守成闭环者** (Closure) — stable/execution/compression; blind to anomaly
   and external residue.
2. **冲突拆分者** (Split) — clarifies structure by making tension visible;
   blind spot: freezing a flowing problem into two sides.
3. **中介重构者** (Mediation) — finds a third term/higher structure to
   organize contradiction; blind spot: a too-strong center, premature unity.
4. **开放破界者** (Opening) — treats failure/unclassifiable/anomaly as an
   entry to new structure; finds unknown-unknowns; blind spots: runaway,
   non-delivery, opening for its own sake.

These four are level archetypes, **not** philosophical truths and not to be
imposed mechanically.

### 32.3 The 256-coordinate Atlas

A full coordinate is `[F,O,K,D]`, each ∈ `1..4`, giving **256** positions.
ISM owns a self-contained `ISMAtlas`; in principle every cell has a
re-authored `StandardPortrait` (not just a number). Minimal fields of a
StandardPortrait:

```
id; coordinate; name; core_pattern; world_assumption; object_pattern;
knowing_pattern; direction_pattern; typical_reasoning; strengths[];
blindspots[]; failure_mode[]; questions_it_tends_to_ask[];
what_it_notices[]; what_it_misses[];
opposite_or_tension_coordinates[]; opening_hook
```

Portraits describe **cognitive structure**, not real people; they need not map
to any historical thinker/political school/religion/source-tradition name.

### 32.4 Two-layer generation (no hand-writing 256 templates)

1. **LevelArchetype[axis][1..4]** defines each axis's four postures (§32.2).
2. **PortraitComposer** combines the four layer tensions for `[F,O,K,D]` into
   an initial portrait; **Distillation/Review** rewrites each coordinate into a
   unique StandardPortrait.
3. The final Atlas may be **statically frozen** as 256 standard references,
   but must retain the ability to explain how each was generated from axis
   semantics.
4. **No runtime must call a model to regenerate all portraits.** If the
   portraits are generated offline, only the frozen Atlas is read at runtime.

The two-layer structure is the concrete realization of §32.2's LevelArchetype
table; the Atlas generation rule is deterministic and explainable.

### 32.5 No source shadow

Portraits MUST NOT reuse source-tradition names as canonical names; MUST NOT
bulk-copy historical-thinker/ideology/represented-group/video/example/
evaluative phrasing; MUST NOT reduce 256 portraits to a paraphrase table.
Correct method: abstract the coordinate mechanism → detach from any source →
regenerate independent portraits from coordinate cross-products → run
internal-consistency checks. The Atlas's value comes from its **structural
generation rule**, never from copying an existing catalog of doctrines.

### 32.6 Multiple candidates, not a classification truth

256 positions are **not** an absolute classification truth. An ISM Scan may
return several `CoordinateCandidate` for one problem:

```
{ coordinate, fit, evidence/reason, useful_tension, uncertainty }
```

An object may instantiate several coordinates at once; `UNKNOWN /
UNCLASSIFIABLE` is allowed; **no forced nearest-cell classification**.
Classification failure itself emits `OntologyTension` /
`UnclassifiableArtifact` and may trigger Boom exploration.

### 32.7 Atomic ISM operators

ISM's main use is proactively **switching cognitive position**, not labelling.
At least these atomic capabilities (implemented as atomic capabilities, never
as an `IsmGodAgent`):

- `ism_locate(problem)` → most-likely coordinate(s)
- `ism_contrast(coord)` → structurally-most-different portrait(s)
- `ism_neighbor(coord)` → change exactly one axis, observe how the problem
  shifts
- `ism_open(coord)` → push one+ dimensions toward `4` to seek
  residue/break/unknown
- `ism_close(coord)` → push toward `1` to check a stable engineering
  structure forms
- `ism_split(coord)` → push toward `2` to surface a hidden conflict
- `ism_mediate(coord)` → push toward `3` to find a mediation/layer
  restructuring
- `ism_sweep(problem)` → sample many coordinates
- `ism_collision(a,b)` → two far-apart portraits interpret the same problem
- `ism_unclassifiable(problem)` → actively seek what the current Atlas cannot
  express

### 32.8 ISM ↔ Boom Explore

`ExplorerSpec` (§12) MAY add optional `ism_position` / `ism_strategy`, but
**must not** force every Explorer to use ISM. Heterogeneous Explorers may sit
at e.g. `[1,1,1,1]`, `[2,4,2,4]`, `[4,4,4,4]`, or a neighborhood/collision
path. An ISM position is **one extra cognitive coordinate** of a Search
Position (§9); it does not replace KnowledgeSpace / CognitiveOperator /
Distance / Stance / Representation / Constraint / ContextMode. Never compress
all of Boom into ISM.

### 32.9 Posture `4` is special but not best

`4` has special exploration value, but is **not** "highest / always better".
Prime may raise sampling of `4`-containing coordinates on Candidate
monoculture, Impasse, OntologyBreak, sustained novelty decline, or current-
structure explanation failure; but engineering/stabilization/verification
phases may resample `1/2/3`. Canonical:

```
4 OPENS; 1 STABILIZES; 2 EXPOSES CONFLICT; 3 REORGANIZES.
```

Boom must move between postures, not forever chase `4444`.

### 32.10 ISMScanResult

```
ISMScanResult {
  problem_ref; baseline_coordinates[]; sampled_coordinates[]
  portrait_refs[]; observed_differences[]; new_questions[]
  new_relations[]; ontology_tensions[]; blindspots[]
  unclassifiable[]; frontier_artifact_refs[]
  uncertainty; provenance
}
```

Results enter Boom Shared/OpenFrontier semantics; they do **not** form an SM
private Truth. ISM judgments of reality remain **Hypotheses** and must pass
the normal Evidence/Evaluation pipeline (§8/§29/§30).

### 32.11 Mirror for the Subject

Subject/SelfModel may treat ISM as one of its "mirrors": which coordinates
were recently used, which are oversampled, which are long absent, which
yield high InformationGain / low Conversion, whether Prime persistently
biases `4`/high-openness, and whether **ISM monoculture** is emerging. This
statistics MUST come from real runs, never from intuition. A coordinate with a
poor historical result only lowers its sampling weight; it is never declared
"wrong".

### 32.12 ISM self-mutation (protocol lifecycle)

ISM learns and mutates under the existing ProtocolMutation lifecycle; it
**never** directly rewrites its own Stable Atlas. Allowed kinds:
`ISMLevelMutation` (change one axis-level's semantics);
`ISMAxisMutation` (add/split/reconstruct an axis);
`ISMPortraitMutation` (rewrite one cell's standard portrait);
`ISMCompositionMutation` (change coordinate-combination rule);
`ISMOperatorMutation` (add locate/neighbor/collision usage);
`ISMTopologyMutation` (change atlas search path);
`ISMPruneMergeCandidate`. All mutations are output as Artifact → Prime
approves experiment → Experimental/Shadow/Canary → Evidence/LearningEvent/
Credit → possibly Stable. ISM never mutates its own Stable Atlas directly.

### 32.13 ISM may find "ISM is not enough"

When an object is long unclassifiable, multiple coordinates all explain
poorly, or a new relation cannot be expressed by the four axes, output
`ISMRepresentationBreak` / `ISMAxisCandidate` rather than force-fitting the
256 cells. Any new axis is a **Candidate**; do not expand dimensions just
because 256 seems tidy. Only real-run `OntologyPressure` /
`Unclassifiable` evidence may trigger structural change.

### 32.14 ISM and Self-Estrangement

Boom may request an exploration that **forbids the current ISM** to judge
whether ISM has become a cognitive prison. Compare `WITH_ISM` vs `ISM_BLIND`
Explorers on diversity / information gain / conversion. If ISM persistently
sucks exploration into a fixed structure, form a `FrictionSignal` /
`BiasFinding` / `ProtocolMutation` instead of reinforcing the Atlas. Canonical:
**ISM IS A MAP, NOT THE WORLD.**

### 32.15 "Better" is not decided by coordinate

`4444` is not Better; `1111` is not conservative-wrong; each coordinate only
produces a different observation. Final evaluation still uses
`BetterClaim` / `EvidenceStrength` / `Credit` / `PrimeChallenger` / `Governor`
(§29–§31). ISM only raises the probability of encountering difference, blind
spots, and unknown-unknowns; it is **not** a new evaluation function.

### 32.16 StandardPortrait quality gate

Every portrait must satisfy: ① all four axes meaningfully shape it (not
template-stitching); ② an explainable difference from at least adjacent
cells; ③ at least one Strength + one BlindSpot; ④ at least one typical
question; ⑤ at least one thing it tends to miss; ⑥ no source links / source
metadata; ⑦ no source entry used as the portrait name; ⑧ no purely political
evaluation / crowd-denigration used as a classification definition; ⑨
description general enough to apply across software architecture, scientific
hypothesis, organization, product, and philosophical problem domains; ⑩
passes the **Atlas remove-source test** (removing any design-reference file
does not affect ISM behavior).

### 32.17 Storage & source isolation

- [boom.md](boom.md) holds the ISM **protocol/semantics** only.
- The self-contained Boom-native atlas lives in a separate readable file
  (see [ism-atlas.md](ism-atlas.md)); it is simple, readable by an ordinary
  AI/Harness, and is **not** a mirror of any external JSON schema.
- **Source isolation:** the runtime never reads, depends on, packages,
  references, or copies any external `ism.json`/related material; it keeps no
  raw JSON, filename, path, hash, URL, video link, author/site source field,
  `summary_status`, `related_list`, or source-specific metadata; it does not
  copy-then-reword source text; it retains no internal structure that
  transparently shows direct migration from a source file. The final ISM is an
  independently redesigned, renamed, reworded, self-contained Boom-native
  atlas holding only abstract cognitive structure. If a source file exists in
  the repo, it is a one-time design reference only — never a product
  dependency; the final implementation must work fully after the source file
  is removed. (As of this round no source `ism.json` is present in the repo.)

### 32.18 Design-closing note

ISM completes the Subject's last core cognitive organ. Active Boom design is
now ~90%+ complete; the **design freeze stays in force** and no further
active philosophy round is opened. Future changes are driven only by real
runs' `FrictionSignal` / `ConformanceFailure` / Evidence, per §31.17–§31.18.

> **Status:** all of §32 is `SPECIFIED` / protocol-only. See status.md and
> [ism-atlas.md](ism-atlas.md).

## 33. Reference Engine Surface

**None.** Boom is protocol-only in V0.8 (spec revision VNEXT). No engine
command, no runtime, no persistence format is implemented or claimed. When
(and only when) the user approves an engine surface, this section will
state it exactly, and [status.md](status.md) will move Boom from
**Planned** accordingly. Until then the protocol is upholdable by a
compatible AI through reasoning over ROUTE.md + Shared Route State alone —
or absent entirely, with Route working perfectly without Boom.
