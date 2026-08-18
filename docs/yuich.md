# Yuich — Canonical Architecture & Subject Protocol

> **This file is the single canonical description of what Yuich is, what it is
> not, which of its core abilities are implemented, which are only fixture
> verified, and which have never run against the real world.** History records
> *how Yuich became this way*; this file records *what Yuich is now*. See
> [status.md](status.md) for the Route-side authoritative status matrix.

```
CANONICAL_STATUS
SUBJECT_MODEL            = "persistent artificial subject protocol/runtime"
ROUTE_RELATION           = "independent development system; decoupled"
CURRENT_IMPLEMENTATION   = "deterministic Python runtime, fixture-verified"
LAST_ARCHITECTURE_REVIEW = "this revision"
CURRENT_SPEC_REVISION    = "VNEXT"
YUICH_RELEASE            = "v1.0 beta"   (canonical machine version: 1.0.0-beta)
ARCHITECTURE_STATE       = "CANONICAL_BETA_FREEZE"
DEVELOPMENT_MODE         = "EXPERIENCE_DRIVEN"
```

**Current release: Yuich v1.0 beta** (`1.0.0-beta`). This is an **Experimental Beta**
(see RELEASE/STATUS below); it sets the first-generation canonical architecture and
MRS Runtime in stone and does **not** promise production stability. Unverified
real-world closed loops (real cognitive model, real web research, real unknown-tool
acquisition, real self-host Route loop) retain their actual `NOT_RUN`/`PARTIAL`
status — beta does not upgrade them to `REAL_OBSERVED`. See §14.

Machine-readable markers are intentionally minimal Markdown blocks (nothing
parsed by a parser dependency). Yuich / an Agent can grep the two lines above
to locate the current canonical definition instead of reading the whole file.

---

## 0. Positioning — what this document is

`docs/yuich.md` = **Yuich Canonical Architecture & Subject Protocol.**

It defines:
- what Yuich is (and is not);
- core invariants;
- module ownership;
- runtime relationships;
- the semantics of state / capacity / learning / tools / self-modification;
- real implementation status (implemented vs fixture vs not-run);
- navigation to the code and to other docs.

It is **not**:
- a changelog;
- an implementation dump;
- a prompt collection;
- all dogfood output;
- a Route specification;
- regulation text;
- a historical brainstorming notebook.

Canonical:

```
YUICH.MD DESCRIBES WHAT YUICH IS NOW.
HISTORY RECORDS HOW IT BECAME THIS WAY.
```

### 0.1 How to read this (for a fresh Cognitive Model / developer)

Suggested order — read the numbered sections in order; you will rarely need the
full historical material:

1. **§1 Current Canonical Snapshot** — the compact definition.
2. **§2 Invariants** — the boundaries that must never move.
3. **§3 Architecture** — the tree; where internal layers sit, where external
   systems sit.
4. **§4 Primary Runtime Loop** — how a turn actually flows.
5. **§5 Execution Resolution** — how an ability gets realizations.
6. **§8 Canonical ownership** — what belongs to Yuich vs to Route.
7. **§13 Implementation status** — what really runs today.
8. **§14 Current limitations** — what has not run.

Jump to detailed sections only when you need a specific mechanism. Weak models
do not need to read the full historical migration (§15).

---

## 1. Current Canonical Snapshot

**Yuich = a persistent artificial subject protocol/runtime** implemented as a
deterministic Python package (`yuich/*.py`). It maintains one Subject that can
move across sessions/models/harnesses without losing identity.

- One Subject.
- Models are replaceable cognitive resources.
- Capacities belong to Yuich.
- Tools are external realizations.
- Skills are learned procedures.
- Handles are native cooperation surfaces.
- Agents are temporary compositions.
- Route is an independent development system.
- Steward governs maintenance stability but is not sovereign.
- Human Constitution bounds Prime.
- Learning may change Yuich; evidence determines promotion.

Non-negotiable distinctions:

```
YUICH IS THE SUBJECT.
MODEL IS NOT THE SUBJECT.
ROUTE IS NOT YUICH.
HANDLE IS NOT SKILL.
SKILL IS NOT CAPACITY.
TOOL IS NOT CAPACITY.
AGENT IS NOT SUBJECT.
STEWARD IS NOT PRIME.
RHAPSODY IS NOT CONSCIOUSNESS PROOF.
```

**State of the implementation in one line:** the subject/runtime, memory,
capacity, tool-learning, governance, evolution, steward and the native
`handle:route` cooperation surface are implemented and fixture-verified as a
deterministic engine. The full real-world closed loops — a real cognitive
model, real web research, real acquisition of an unknown external tool, and a
real Yuich self-modification executed through Route — are **NOT_RUN**. See
§13/§14 for the precise table.

**Human Evolution / Update History.**

- `Yuich v1.0 beta — First Canonical Beta Freeze`
  - Trigger: Canonical architecture convergence + MRS mechanisms completed.
  - Changed: Release identity set to `v1.0 beta` (`1.0.0-beta`); architecture
    scope frozen for beta.
  - Why: Transition from architecture-driven development to experience-driven
    development.
  - Evidence: canonical doc convergence + regression suite refs (all `dogfood*`
    suites + fresh-boot/resume checks).
  - Important boundary: Real model / tool / web / self-host capabilities retain
    their actual `NOT_RUN`/`PARTIAL` status. Beta does **not** mean
    "fully autonomous" or "production ready".

- `Yuich v1.0 beta — Tool Container / Route as Bundled Tool`
  - Trigger: Fruit dogfood (FRUIT_DOGFOOD) — need clear physical boundary
    between Yuich Core and external tools.
  - Changed: Created `tool/` container directory; Route declared as BUNDLED
    native handle (tool/route/TOOL.json + adapter); Yuich gained
    `discover_bundled_tools()` method; Route physical migration to
    tool/route/ is PROVISIONAL (Cargo workspace restructuring pending).
  - Why: Tools are external affordances; Yuich Core is Subject implementation.
    Guard against tools gradually dissolving into core.
  - Evidence: Yuich boots without Route (PASS); Route standalone 323 cargo
    tests PASS; zero import coupling verified; tool discovery distinguishes
    BUNDLED (Route) from SYSTEM (tar); enforcement tests for branch/path/
    command/diff gates PASS.
  - Important boundary: Route migration status = PROVISIONAL. Full physical
    move requires Cargo workspace restructuring; current adapter layer
    is the canonical discovery reference.

---

## 2. Invariants & boundaries

These bind everything below and are not open for silent change.

1. **One Subject.** A single `YuichSubject` identity owns continuity. Domain
   capability modules are not sub-personalities and do not fork the Subject.
   A *Route Avatar* with its own persistent personality is forbidden.
2. **Model ≠ Subject.** Any model may fail or be replaced; that is capability
   degradation, never subject death.
3. **Route is external.** Route is a standalone development protocol/system. It
   must not depend on Yuich; Yuich must not require the Route Tool to exist.
   No second Sovereign exists inside Yuich — Prime is the sole final approval
   location inside Yuich.
4. **Route ≠ Yuich state.** Route project state is never copied into Yuich
   global memory; Yuich private state is never put into Route's public state.
5. **Constitution is root.** Human Constitution Root sits above Prime. Prime
   cannot override Root, and Constitution cannot be self-modified autonomously.
6. **Evidence over claim.** "Success" claimed by a model is not, by itself,
   evidence. Promotion requires verification.
7. **Prompt/spec is not runtime evidence.** A spec describing an ability does
   not make it real; only implemented + verified behavior does.

### 2.1 Independence matrix (must all hold)

| Configuration | Allowed |
|---|---|
| Yuich without Route | YES |
| Route without Yuich | YES |
| Development capacity without Route | YES (realization degraded) |
| Development capacity + Route Handle | YES (native fast path) |
| Route used by non-Yuich AI / human | YES |

---

## 3. Architecture

```
YUICH SUBJECT
├─ Identity / Continuity
├─ Prime                       (sole approval authority)
├─ Human Constitution / Governance
├─ Capacity Layer              (what Yuich can do)
├─ Memory / History
├─ Context
│  ├─ Prethink
│  └─ Context Compiler
├─ Cognition
│  ├─ Cognitive Models
│  ├─ ISM
│  ├─ Negativity
│  ├─ BoomMode
│  └─ Rhapsody
├─ Learning
│  ├─ SelfLearning
│  ├─ AcquisitionLearning
│  └─ Evolution
├─ Tool / Handle Gateway
├─ Steward
└─ Evidence / Persistence

EXTERNAL (not inside Yuich)
├─ Route                      (independent development system)
├─ Other Tools
├─ Models                     (replaceable cognitive resources)
├─ Research / Web
└─ Harnesses
```

Route is **never drawn inside Yuich**. Yuich may use Route through the
`handle:route` cooperation surface (see §5, §7, §9).

### 3.1 Modules → code map (authoritative)

| Canonical concern | Where it lives (code) |
|---|---|
| Subject runtime, state, persistence, Prime, governance, context, cognition, learning, tools | `yuich/mrs.py` (`class Yuich`) |
| Capacity layer + execution resolution | `yuich/capacity.py` |
| Evolution, agents, transfer, architecture history | `yuich/evolution.py` |
| Steward, maintenance, KnownGood, recovery | `yuich/steward.py` |
| Tool/skill acquisition | `yuich/active_learning.py` |
| Native cooperation surface (`handle:route`) | `yuich/handle.py` |
| Package entry points / exports | `yuich/__init__.py`, `yuich/__main__.py` |

---

## 4. Primary Runtime Loop

The one main loop (there is no separate competing loop):

```
Encounter
→ understand current need
→ Capacity resolution
→ choose cheapest adequate realization
→ Context / Cognition as needed
→ Native API | Native Handle | Skill | Acquisition | ToolGenesis
→ Action
→ Outcome
→ History
→ SelfLearning
→ Evolution if worthwhile
→ Steward if a change is required
→ persist
```

This is **not a fixed pipeline**. The pipeline is a tool, not a ritual.
Which stages run depends on the need, cost, risk, and available realizations.
Low-cost actions can skip most stages; only high-impact change forces the
full evidence/Prime/Steward path.

---

## 5. Execution Resolution

Resolution priority (single authoritative source — not repeated elsewhere):

```
1 Native API / Rule
2 Native Handle
3 Valid Skill
4 Acquisition Learning
5 Capacity composition
6 Tool Genesis
7 Human / Defer
```

- **Native API / Rule** — a known structured contract → rule binding.
- **Native Handle** — a tool-provided high-cooperation protocol (e.g.
  `handle:route`) → prefer it over re-learning a Skill for the same tool.
- **Valid Skill** — a learned procedural use of a tool.
- **Acquisition Learning** — learn an unknown external object.
- **Capacity composition** — combine several capacities.
- **Tool Genesis** — build the missing realization.
- **Human / Defer** — cannot resolve safely.

Development tasks map to **tool-independent capacities**:

```
cap:develop-software   (build software)
cap:modify-software    (repair / refactor software)
cap:self-modify        (modify Yuich's own implementation)
```

When a `handle:route` is available and compatible, a development goal goes to
**Native Handle Mode** instead of re-running tool-skill learning. The legacy
name `cap:yuich.route` exists **only as a compatibility alias** that resolves
onto `cap:develop-software`; it is never the canonical capacity name.

---

## 6. Capacity

**Capacity = what Yuich can do.** Capacity is independent of Tool / Model /
Handle. A capacity has N:M realizations. Losing a tool does not erase the
capacity (the realization becomes `UNAVAILABLE` or `DEGRADED`). Capacity has
evidence and status. **Capacity ≠ permission**, and **Capacity ≠ availability**.

Small canonical examples (not an exhaustive table):

```
research        calculate        learn        develop-software
self-modify     remember         prethink
```

Capability/capacity names are the primary vocabulary throughout this file.
`Yuich` seeds a small capacity registry at boot (see `_seed_capacity_registry`
in `yuich/mrs.py`) including `cap:code-execute`, `cap:understand-code`, and the
canonical development capacities above.

---

## 7. Tool / Interface / Skill / Handle / Adapter / Harness / Agent

Core semantic table (single canonical source):

| Term | Meaning |
|---|---|
| **Capacity** | Internal ability — what Yuich can do. |
| **Tool** | External invokable object — what Yuich can use. |
| **Interface** | Machine-callable contract of a tool — *how something is called*. |
| **Skill** | Yuich's learned procedure for using a tool. |
| **Handle** | Tool-provided, host-discoverable cooperation surface — *the tool knows how to cooperate*. |
| **Adapter** | A bridge that simplifies / normalizes an awkward tool. |
| **Harness** | Execution environment. |
| **Agent** | Temporary capacity composition. |

Tool learning lifecycle:

```
Unknown Tool
→ Interface / Skill / Docs / Source
→ ToolSkill
→ repeated verified use
→ Adapter / native binding possible
```

**If a Handle exists, do not re-learn its base contract.** A discovered
`handle:route` elevates a development goal to Native Handle Mode. A known
Handle does not trigger AcquisitionLearning. `ToolSkill:route` may still exist
to cover edge cases of legacy or forked Route environments, but a Skill never
overrides a Handle's authoritative contract (default: Handle > Skill).

The conceptual (vendor-neutral) interface used by the Subject is:

```
Yuich.activate_capability(capacity_id, encounter)
Yuich.choose realization (Native API | Handle | Skill | Acquisition | Genesis)
Tool.return(outcome, evidence)
Yuich.learn(outcome)
```

---

## 8. Canonical ownership — Route vs Yuich

**Route** is a standalone development protocol/system/tool. **Route owns**:

```
ProjectState          ProjectHistory        KnownGood
development lifecycle build/test            repair / refactor / restructure
project recovery      development Agent/Harness   project evidence
```

**Yuich owns**:

```
SubjectHistory        Capacity              Skill
cross-project experience                    SelfLearning
Evolution             Steward               general context / model / tool strategies
```

They cooperate through the `handle:route` surface:

```
Yuich → Route : DevelopmentIntentPacket
Route → Yuich : RouteOutcomePacket
both          : Feedback, TransferCandidates
```

**They do not share canonical state.** A `DevelopmentIntentPacket` carries a
goal, target refs, protected invariants, known failures, acceptance criteria —
**not** the full SubjectHistory. A `RouteOutcomePacket` carries what was
understood / changed / tested / verified — **not** the full project trace.

```
YUICH OWNS WHY / LONG-TERM SUBJECT MEANING.
ROUTE OWNS DEVELOPMENT EXECUTION.
HANDLE CONNECTS THEM.
STATE OWNERSHIP REMAINS SEPARATE.
```

This is not an absolute silence on advice — Route may raise an architecture
concern, Yuich may raise an implementation constraint — but the authoritative
owner of each concern is fixed. §16 below is the canonical ownership table.

---

## 9. Self-hosting

**`YuichSubject` ≠ `YuichCodebase`.** Yuich may treat its own implementation
as a development world. The canonical self-modification path:

```
Real friction → EvolutionCandidate → Steward → KnownGood
→ Route Handle → Route modifies Yuich implementation → verify
→ PROVISIONAL → observation → CONFIRM or ROLLBACK
→ history / learning
```

**Current status — be precise:** the self-modification infrastructure
(steward gates, KnownGood, checkpoint/verify/rollback, `handle:route`) is
**IMPLEMENTED** and **fixture-verified**. The **real** end-to-end self-host
loop — a real friction observed in a running Yuich, executed against the real
Route system, observed, confirmed — is **NOT_RUN**. Yuich is not yet shown to
stably rewrite itself in the real world. The full real loop is flagged
`SELF_HOSTED_ROUTE_HANDLE_REAL_LOOP` and is `NOT_RUN` until it is actually
executed.

Self-hosted development is marked `SELF_HOSTED_YUICH` and automatically raises
protections: continuity, KnownGood, state/schema compatibility, Constitution
invariants, fresh-process boot, fresh-model resume, rollback requirement, and
the Steward gate. It is **not** a special Subject mode — it is an ordinary
development target that happens to be Yuich's own implementation.

---

## 10. Learning

There are three learning forms plus Steward (which is a stabilizer, not a
fourth learning form):

| Form | Meaning |
|---|---|
| **SelfLearning** | Learn from one's own Action → Outcome. |
| **AcquisitionLearning** | Learn external world tools / knowledge. |
| **Evolution** | Change one's own strategy, relationships, components, or structure. |
| **Steward** | Keep changes stable, recoverable, trackable. |

Flow of knowledge:

```
World knowledge → Acquisition → Skill → Action → Outcome → SelfLearning
→ EvolutionCandidate → Steward → Route (if a code change) → observation
→ stable change
```

### 10.1 Active Learning Ladder (canonical summary)

```
Existing experience → Interface → Skill → Local docs/help → Examples/tests
→ Source → Internet research → Safe probe → Adapter if needed
```

Principles:

```
INTERFACE ≠ LEARNED.
DOCS ≠ VERIFIED.
SOURCE READING ≠ VERIFIED.
KNOWING HOW ≠ PERMISSION.
```

The 900-line mechanism lives in `yuich/active_learning.py`; it is not repeated
here.

---

## 11. Prethink, Context, History

### 11.1 Prethink

Prethink is a **lightweight pre-cognitive enhancer**. It may run before
interpretation, before retrieval, after retrieval, or before context
compilation. It:

- discovers potentially-noteworthy relations;
- does **not** decide truth;
- does **not** execute actions;
- preserves raw input;
- may be skipped;
- fails open.

Prethink's self-observation/extensibility is exposed as
`prethink_self_evolution` in `yuich/mrs.py`.

### 11.2 Context

```
MEMORY IS PERSISTENT.
CONTEXT IS COMPILED.
```

The Context Compiler builds a `SubjectView` (identity + active goal/concern +
**relevant** memories + required capacity/tool + hard constraints + unresolved
state) from the current Encounter. Key rules:

- raw source lineage is preserved;
- context failure is its own failure class;
- different models may receive different scaffolding (weak/strong handled by
  the compiler, not by dumping partial history);
- never dump the full history to a model.

For the Route surface, a narrower **DevelopmentView** is compiled (current
intent + relevant refs + protected invariants + known failures + acceptance
criteria + available evidence + related artifacts + hard constraints), not the
full SubjectView.

### 11.3 History

History is layered:

```
Raw Trace
WorkEpisode
SubjectHistory
ProjectHistory (Route-owned)
Decision / ArchitectureHistory
ToolSkill History
Update / Evolution History
```

Core principle: **remember consequences, not every token.** Intent changes are
recorded as temporal-continuity deltas (e.g. an ABC→ABD intent delta), not as
full copies of every step. Explicit current user instruction always ranks above
an older recorded preference.

---

## 12. Evolution, Steward, Update History

### 12.1 Evolution

Implemented machinery (see `yuich/evolution.py`):

```
EvolutionTrigger, EvolutionAgent, dynamic AgentCluster, InteractionFriction,
RelationshipCandidate, StructuralCandidate, RouteLearningReview,
ArchitectureHistory
```

- `NO_CHANGE` is a valid result — evolution is not forced.
- **Agents are temporary.** After an agent's work, Evidence/Skill/Lesson/
  Artifact return to the Subject; the agent's personality is not persisted.
- **An EvolutionCandidate cannot promote itself**; promotion goes through
  Prime/Steward and requires evidence.

### 12.2 Steward

Steward is the maintenance-stability layer. Responsibilities:

```
maintenance orchestration   ChangeBudget    KnownGood      ObservationWindow
Incident                     CircuitBreaker  UpdateJournal  Recovery
Human Update History
```

```
LEARN CONTINUOUSLY; MUTATE CONSERVATIVELY.
CHANGE IS NOT IMPROVEMENT UNTIL OUTCOME SUPPORTS IT.
```

- **Steward ≠ Evolution.** Steward stabilizes changes; Evolution proposes them.
- **Steward ≠ SelfLearning.** Steward does not in itself learn.
- **Steward ≠ Prime.** Steward has no approval sovereignty.

The self-modification update lifecycle (single canonical version):

```
Observed friction → Evidence → Candidate → Steward screening
→ Prime experiment approval → KnownGood → Route (if implementation change)
→ verification → PROVISIONAL → ObservationWindow → CONFIRMED or ROLLBACK
```

(Not "self-detect → self-rewrite → stable".)

### 12.3 Update History

There are two update histories:

- **Machine Journal** — for recovery/audit (e.g. `UpdateJournal`).
- **Human Evolution History** — for *why Yuich became this way*.

Every significant learning, capacity growth, self modification, structural
change, rollback, and Route self-host change should produce a readable history
entry. **Failed changes are also kept**; they are not erased.

---

## 13. Governance, Model, Agent, Rhapsody

### 13.1 Governance priority

```
Human Constitution Root
↓
Applicable Compliance
↓
Constitute
↓
Prime
↓
Capabilities / Tools / Agents
```

- **Constitute** = rule interpretation / boundary enforcement
  (`ALLOW | ALLOW_WITH_CONSTRAINTS | REQUIRE_HUMAN | DEFER | BLOCK | UNKNOWN`).
- **Justify** = humanistic rational evaluation; it cannot unlock a
  Constitution **BLOCK**.
- **Rhapsody** = bounded, non-instrumental candidate generation.
- Constitution cannot be modified autonomously.
- Compliance facts are externally verified / updateable.
- Prime cannot override Root.

The full `governed_decide` pipeline is:
`Constitute → Justify → (Rhapsody) → Evidence → Prime`, and records all voices.

### 13.2 Rhapsody / consciousness language

- **Rhapsody** is phenomenology / self-narrative **candidate generation**. It
  is not evidence of consciousness.
- **Do not claim consciousness.** Do not dogmatically define it away either.
  Status of any phenomenal/biological/corporate-personhood claim:
  **UNKNOWN / UNPROVEN**.
- Yuich may be called a **functional / persistent artificial subject** in
  project semantics; that is **not** a claim of biological/phenomenal
  consciousness or legal personhood.

### 13.3 Model

```
Model = replaceable cognitive resource.
```

On model swap, the following remain continuous:
`SubjectState, History, Capacity, ToolSkills, ArchitectureHistory, Evolution`.
Model-specific strengths/weaknesses enter `ModelUseExperience`, and never
become Subject identity.

**Real-model status:** the runtime is deterministic; the `ModelGateway`
exposes **slots** (`deterministic`, optional `relay`, `cognitive`) and subject
continuity across sessions is implemented. A real external LLM / cognitive
model is **NOT integrated** (no network/LLM dependency ships in the package).
See §14.

### 13.4 Agent

`AGENTS ARE TEMPORARY CAPACITY COMPOSITIONS.` Agents may be single, clustered,
or dynamic. On completion, their Evidence/Skill/Lesson/Artifact return to the
Subject; an agent's personality is not persisted. `EvolutionAgent`,
`ToolScout`, `Learner`, etc. are **specializations**, not separate subjects.

---

## 14. Evidence status & implementation status (authoritative)

### 14.1 Evidence status legend

A unified legend for all run-status labels in this project (the old
`OBSERVED_PASS` vocabulary is superseded; never write a fixture as
`OBSERVED_PASS`):

```
REAL_OBSERVED        — executed against the real external world / Route system
LOCAL_OBSERVED       — executed locally
DETERMINISTIC_FIXTURE— ran deterministically now (state-machine / protocol logic)
SIMULATED_MODEL      — a simulated model made the decision
HISTORICAL_REPLAY    — replayed from recorded history
SPECIFIED_ONLY       — described in spec, not run
NOT_RUN              — not run
```

Three dimensions must not be conflated:

```
IMPLEMENTED   = code exists
VERIFIED      = a test ran and passed
REAL_OBSERVED = executed against the real external world
```

(`INFRASTRUCTURE IMPLEMENTED` + `FIXTURE VERIFIED` does **not** equal
`REAL_OBSERVED`.)

Special label for the fully-run real self-host loop:

```
SELF_HOSTED_ROUTE_HANDLE_REAL_LOOP   (currently NOT_RUN)
```

### 14.2 Implementation status table (from real code inspection)

| Subsystem | Implementation | Verification | Real-world status |
|---|---|---|---|
| Subject runtime / persistence (state JSON, save/resume) | **IMPLEMENTED** (`mrs.py`) | Fixture-tested (dogfood) | **NOT_RUN** (no long-running real subject) |
| Capacity layer + execution resolution | **IMPLEMENTED** (`capacity.py`) | Fixture (dogfood-capacity) | LOCAL (deterministic runs) |
| Memory / History | **IMPLEMENTED** (scoped memories, work episodes, deltas) | Fixture | **NOT_RUN** long-term |
| Tool learning (ladder, ToolSkill) | **IMPLEMENTED** (`active_learning.py`) | Fixture | Real unknown-tool acquisition **NOT_RUN** |
| ToolGenesis | **IMPLEMENTED** (`capacity.py`) | Fixture | **NOT_RUN** real |
| Prethink | **PARTIAL** (component + self-evolution; no standalone discrete enhancer path) | Fixture | **NOT_RUN** real |
| Research | **IMPLEMENTED** (combinatorial/local) | Fixture | Real web research **NOT_RUN** (no network lib ships) |
| Governance (Constitution/Compliance/Constitute/Justify/Prime) | **IMPLEMENTED** (`mrs.py`) | Fixture (deterministic) | LOCAL (deterministic runs) |
| Evolution | **IMPLEMENTED** (`evolution.py`) | Fixture (dogfood-evolution) | LOCAL (dogfood runs) |
| Steward | **IMPLEMENTED** (`steward.py`) | Fixture | LOCAL (dogfood runs) |
| RouteHandle (`handle:route`) | **IMPLEMENTED** (`handle.py`) | `DETERMINISTIC_FIXTURE` (dogfood-handle) | Real Route integration **NOT_RUN** |
| Self-host loop | **PARTIAL** (infrastructure present) | Fixture | **NOT_RUN** (`SELF_HOSTED_ROUTE_HANDLE_REAL_LOOP`) |
| Cognitive model / model swap | **PARTIAL** (slots + continuity implemented) | Fixture (dogfood C/L) | Real external model **NOT_RUN** |
| Real web research | **SPECIFIED_ONLY** | — | **NOT_RUN** |
| Real unknown tool acquisition | **SPECIFIED_ONLY** | — | **NOT_RUN** |
| Real Route integration | **SPECIFIED_ONLY** | Fixture only | **NOT_RUN** (fixture-only) |

Run-status for the Handle layer follows §14.1 (`REAL_OBSERVED | LOCAL_OBSERVED
| DETERMINISTIC_FIXTURE | HISTORICAL_REPLAY | SPECIFIED_ONLY | NOT_RUN`).

---

## 15. Current limitations (real, explicit)

From actual code inspection:

- **Real Cognitive Model is not integrated.** Runtime is deterministic; the
  ModelGateway is slots + continuity. No external LLM dependency ships.
- **Real web research does not run.** No network library is imported anywhere;
  `https://…` values are simulated fixtures only.
- **Real unknown-tool acquisition does not run.** The ladder exists; no real
  external tool has been acquired/verified in the wild.
- **Real self-host Route loop is NOT_RUN.** `SELF_HOSTED_ROUTE_HANDLE_REAL_LOOP`
  is unexecuted; Yuich has not yet been shown to stably self-modify via Route.
- **Heuristic thresholds lack real calibration.** Confidence / risk / budget
  thresholds are deterministic fixtures, not tuned on real-world evidence.
- **Prethink is partial.** There is no standalone discrete `prethink()`
  enhancer path; the behavior is embedded / a component self-evolution hook.
- **Frame the above honestly:** `IMPLEMENTED` describes code presence; it does
  not claim real-world success.

---

## 16. Canonical ownership table

Use this to prevent modules from being re-mixed.

| Concern | Canonical owner | External relationship |
|---|---|---|
| Subject identity / continuity | **Yuich** | survives model/session/harness |
| Human Constitution | **Yuich** (governance / root) | binds Prime |
| Subject history | **Yuich** | private |
| Capacity | **Yuich** | independent of tools |
| ToolSkill | **Yuich** | learned use of an external tool |
| General context / tool / model strategy | **Yuich** | compiler decides scaffolding |
| SelfLearning / Evolution / Steward | **Yuich** | internal change machinery |
| Project KnownGood / project state | **Route** | external |
| Route project history | **Route** | external |
| RouteHandle contract | **Route / public contract** | host-neutral |
| Handle binding on Yuich side | **Yuich / host** | consumes the handle |
| Self-update stability | **Steward** (Yuich) | Route may execute the change |
| Software modification realization | **Route** | development execution |
| Model inference | **Cognitive resource** | replaceable |

---

## 17. Historical / migration (not canonical today)

The following sections are **HISTORICAL / MIGRATED**. They exist for
traceability only; nothing in them is the current canonical definition.

- **BoomSubject → YuichSubject.** Boom was demoted from "component-subject" to
  a cognitive **BoomMode** of Yuich. Full historical spec:
  [boom.md](boom.md).
- **Boom → BoomMode.** BoomMode is a high-openness cognitive state entered on
  impasse / unknown-unknowns / candidate monoculture / ontology failure /
  self-estrangement. Its outputs still enter the same Yuich learning/evidence
  pipeline. Toggling BoomMode never changes `subject_id` and it holds no
  independent memory authority.
- **General learning moved to Yuich**, not Boom.
- **Route Boom-era subject semantics are superseded.** Route owns development
  state only; the Subject semantics are Yuich's.
- **ISM** (internal cognitive atlas): optional lens ([ism-atlas.md](ism-atlas.md)).
  ISM scans feed hypotheses, never evidence. If the self-contained atlas is
  unavailable, wire the interface only and mark `NOT_IMPLEMENTED / NOT_RUN`;
  Yuich/BoomMode are fully usable with ISM disabled.
- **Boom → Yuich migration mapping:** §4 BoomSubject(owner) → YuichSubject;
  Boom's OpenFrontier/frontier-artifacts → BoomMode outputs;
  cognitive operators → Cognition; axes/topology/diversity/ExplorerSpec →
  BoomMode parameters; SLL-I/II + Design Closure → Yuich self-learning
  semantics (gates unchanged); ISM §32 → Cognition/ISM.
  Where the two disagree on subject ownership, **this file is canonical.**

---

## 18. Fresh reader: 20 questions, concise answers

1. **What is Yuich?** A persistent artificial subject protocol/runtime
   (deterministic Python). One Subject across models/sessions.
2. **Who is the Subject?** `YuichSubject` — identity/continuity owned by Yuich.
   Not a model, not an agent, not Route, not Steward.
3. **What is a Model?** A replaceable cognitive resource. Not integrated for
   real; slots only.
4. **What is a Capacity?** What Yuich can do — tool-independent (`cap:*`).
5. **Tool / Interface / Skill / Handle?** External object / its call contract /
   learned use / tool-provided cooperation surface.
6. **What is Route?** An independent development protocol/system/tool.
7. **Who owns which state?** Yuich owns Subject/Capacity/Skill/learning;
   Route owns project/KnownGood/history.
8. **How does Yuich learn an unknown tool?** Acquisition ladder → ToolSkill
   (§10.1). Real-world acquisition NOT_RUN.
9. **How does Yuich learn from Outcome?** Outcome → LearningEvent → SelfModel
   update; verification only upgrades status (§12.2, §14).
10. **How does Yuich change itself?** Friction → Candidate → Steward → Prime →
   KnownGood → Route → verification → PROVISIONAL → observation → CONFIRM/
   ROLLBACK (§9).
11. **Why does Steward exist?** To keep changes stable/recoverable/trackable
    (`LEARN CONTINUOUSLY; MUTATE CONSERVATIVELY`). Not sovereign.
12. **How does Yuich develop itself?** As a development world; via
    `cap:self-modify` + `handle:route` (§9). Real loop NOT_RUN.
13. **Which capabilities really exist?** Implemented deterministic runtime:
    subject/persistence, capacity, memory, tool-learning, governance,
    evolution, steward, handle (§14.2).
14. **Which are fixture-only?** All of the above are fixture-verified; the
    status table separates `DETERMINISTIC_FIXTURE` from `REAL_OBSERVED`.
15. **Which real loops have not run?** Real cognitive model, real web research,
    real unknown-tool acquisition, real self-host Route loop (§14, §15).
16. **What survives a model swap?** SubjectState, History, Capacity,
    ToolSkills, ArchitectureHistory, Evolution (§13.3).
17. **What survives Route disappearing?** Subject identity, capacity,
    learning, history, ordinary tools; development realization degrades (§2.1).
18. **Can Yuich self-modify Root Constitution?** No. Constitution is root and
    cannot be autonomously modified (§13.1).
19. **Is an Agent a subject?** No. Agents are temporary capacity compositions.
20. **Does Yuich claim consciousness?** No. Status is UNKNOWN/UNPROVEN (§13.2).

---

## 19. Machine-readable summary markers (consolidated)

```
CANONICAL_STATUS                    = current (this revision)
SUBJECT_MODEL                       = persistent artificial subject protocol/runtime
ROUTE_RELATION                      = independent development system; decoupled
CURRENT_IMPLEMENTATION_STATUS       = deterministic Python runtime, fixture-verified
    real cognitive model            = NOT_RUN
    real self-host loop             = NOT_RUN
    real web research               = NOT_RUN
    real unknown-tool acquisition   = NOT_RUN
LAST_ARCHITECTURE_REVIEW            = this revision
```

The 20 answers in §18 are the acceptance check: if any answer becomes ambiguous
from this file alone, the document is not yet canonical; fix the relevant
section rather than the code.