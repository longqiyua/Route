# Route Base Positive Chain I

Internal implementation and acceptance reference, 2026-10-13.

## Scope and truth boundary

Route Base means narrowly scoped knowledge learned from identifiable project work.
It is not chat memory, a vector database, a second truth store, current Evidence,
or completion authority. Canonical records remain `CooperationKnowledgeRecorded`
inside the existing DevelopmentEvent ledger. Optional extraction jobs use existing
Team events. No new database, background daemon, provider routing or AI launch.

This batch first preserved the verified mixed reliability baseline, then added
the positive deterministic/manual/fixture path. Real AI synthesis is NOT_RUN:
the preceding Host availability check reported Codex `Not logged in`. No login,
credentials or provider settings were changed. RealAIHostRestart/CrossHost remain
NOT_RUN. A source-backed controlled fixture is not certification of AI quality.

## Preservation checkpoint

Starting branch `fruit`, HEAD and fetched origin/fruit:
`eb9751ba3d563e116ffcf0bf36a102c840cc0fca`, ahead/behind 0/0.

The original 181 default status entries expand to 189 individual paths with
`--untracked-files=all`. Local audit manifest:
`Release/local-tools/positive-chain-provenance-20261013.json` (ignored/private).

| Category | Paths | Disposition |
|---|---:|---|
| A: existing migration/archive/backup/console/handbook | 133 | Preserved, excluded from checkpoint |
| B: Execution Trust | 7 | Known Route reliability work |
| C: Run Continuity | 8 | Known Route reliability work |
| D: other known Route fixes and shared reliability hubs | 41 | Known Route work, overlap explicitly acknowledged |
| E: unresolved provenance | 0 | No guessed unknown file staged |

Checkpoint `01952e7ea111d137e270d42140efd8c5d7c9089e`:
`fix(route): checkpoint verified execution reliability baseline`.
Explicit 56-path allowlist; initial index empty; staged paths and working hashes
verified against the manifest. No stage-all. This combines known earlier
Q1–Q16 fixes, Execution Trust and Windows Run Continuity, not an invented clean
phase-by-phase historical boundary. It excludes every A path, including the
mixed internal handbook. Original manifest is intentionally the pre-checkpoint
snapshot; its `stage_approved=false` and checkpoint-not-created fields are not a
post-checkpoint receipt. This report and the actual Git commit record the result.

No push, main merge, tag, public Release, history rewrite or Yuich modification.
New Base changes remain separately reviewable in the working tree.

## Positive chain

1. A real Goal and Work name a component and a required check.
2. System Evidence records FAIL from an actual command.
3. An authenticated source Worker publishes a scoped `RECOVERY:` or
   `ROOT_CAUSE:` Finding, including the exact component and OS label.
4. Source changes; the same Work-required check produces fresh System PASS.
5. The detector constructs a bounded packet, not lesson prose.
6. An explicit optional deferred job stores that packet in a Team event.
7. A separate registered Knowledge creator supplies FIXTURE/MANUAL candidate
   text. Main Worker receives no extra lessons-learned prompt.
8. Candidate publication is gated on actual canonical provenance.
9. A different authenticated reviewer publishes a subsequent Finding naming
   the candidate. Trusted-local Operator explicitly publishes validation.
10. Bounded retrieval may expose the unit as historical knowledge in future Work
    Context; fresh separate reuse Evidence and review permit REUSED_CONFIRMED.

Ordinary PASS, CRUD and typo notes without the structured high-signal investigation
do not trigger extraction. Zero candidates is healthy. Only FAIL_THEN_VERIFIED_FIX
is implemented in this phase; other proposed detector families are not shipped.
Structured marking alone is insufficient without the full source/Evidence chain.

## Schema and validation

Existing Lesson fields supply problem pattern, resolution, falsification check,
Goal, Work, component/environment, creator, Finding and review references. Optional
positive metadata adds title, kind, symptoms, established cause, non-applicability,
bounded extraction packet, source revision and reuse Evidence IDs. Query exposes
related components, support/reuse/contradiction counts, revision, supersedes and
last validated revision. No synthetic confidence percentages.

FIX_PATTERN and FAILED_APPROACH are supported. Failed approaches require a reason,
retain their assumptions/non-applicability, and are not universal prohibitions.
Knowledge has epistemic status INFERRED, even when locally validated. It cannot
claim an OBSERVED capability or turn a Worker Finding into System Evidence.
The normal observed-capability Evidence gate is unchanged.

Gates reject generic advice in statements, action fields and positive text;
unknown/foreign Goal/Work/Finding/events/worker; changed source revision;
wrong or missing FAIL/PASS Evidence; Main-as-Knowledge creator; self-validation;
unreviewed reuse; scope widening; duplicate proof; or revival of retired history.
Initial condition must name the observed component and exact environment.
Validation also checks the actual component resource fingerprint and fresh PASS.
Same-OS-user trusted Operator is the existing boundary, not an adversarial-user
security sandbox. Schema accepts explicit local manual imports; it does not
prove that a human/fixture's prose is a correct explanation.

## Lifecycle and current-world precedence

States reuse the existing enum: CANDIDATE, VALIDATED_LOCAL, REUSED_CONFIRMED,
DISPUTED, STALE, SUPERSEDED, RETIRED. Supersession is represented by a new canonical
record referring to the prior ID; original events are never rewritten.
Candidate → validated requires independent review; validated → reused requires a
new distinct real System PASS and another independent review. Disputed/stale units
can be retired; unsupported revival is rejected. Scope widening needs a new
separately supported unit, not editing the old observation.
Reuse must also reference a check started after a later, different Work was
created in the same Goal/component with that required check. Re-running only the
original Work is not sufficient. Query exposes the reuse Evidence and derived
later Work IDs. The controlled dogfood injects Base into that later Work's Context.
The initial positive detector supports only the actually observed OS; it does not
accept arbitrary claimed environment labels. If a Finding names restricted
sandbox conditions, the candidate cannot silently drop that condition.

Read-only projections derive STALE from the current component fingerprint or
missing source Evidence, and DISPUTED from contradictory current units or a later
same-check System FAIL. They do not append a fake transition just by reading.
Unrelated-file changes alone do not stale scoped knowledge. Validation still
requires a fresh whole-source PASS, a deliberately more conservative write gate.
Exact duplicates are rejected, not silently merged. Simple explainable probable
overlap (same scoped resolution, different condition) is returned for review;
contradictions retain both histories. No semantic/vector/AI merge is claimed.

## Bounds and failure handling

| Bound | Default |
|---|---:|
| Extraction jobs per Goal | 1 |
| Pending jobs across configured Teams | 2 |
| Automatic retries | 0 |
| Packet bytes | 8192 |
| Original new candidates per Goal | 2 |
| Reuse proof IDs per unit | 8 |
| Retrieved/context units | 3 |
| Recent matching units inspected per query | 12 |
| Base JSON budget | 8192 bytes |
| Entire Context Pack | 16384 bytes |

Queueing is explicit and optional. Unconfigured Team, unavailable extraction
Worker or exhausted budget reports DEFERRED without changing Goal completion.
There is no unattended retry scheduler. An operator can finish an existing job
as failed (including timeout/unavailable) through the existing Team broker.
Goal completion does not wait on these jobs. Reads never initialize or repair
durable state. No candidate or job is generated automatically after every task.

Retrieval projects resources/knowledge from a single loaded ledger, and Context
reuses its loaded ledger for Base projection; it does not reread the ledger per
returned unit. Full ledger replay and Evidence whole-file load remain scalability
limitations; this batch does not claim an incremental index or whole-Context
single-revision transaction. Actual file fingerprinting is necessary scoped I/O.
Current-resource probes are limited to the 12 most recent matching units. Older
matches may be outside that diagnostic window. Optional Base projection errors
omit Base and set `base_projection_unavailable=true`; they do not prevent Work
Context. Errors loading mandatory current Evidence retain the existing fail-closed
behavior and are not disguised as successful verification.

## CLI guide

Run in the attached project:

```sh
route base packet GOAL_ID
route base defer GOAL_ID JOB_ID --operation-key unique-key
route base submit candidate.json --operation-key another-key
route base finish GOAL_ID JOB_ID --operation-key finish-key
route base query src/recovery.rs --environment Windows --json
route base query src/recovery.rs --environment Windows --independent --json
route base candidates
route base show KB_ID
```

Submit accepts at most 16 KiB and the existing CooperationKnowledge JSON schema
with a positive Lesson; it is an explicit trusted-local administrative action,
not permission for Worker RPC self-promotion. Read results are JSON; `--json`
is accepted on query for script ergonomics. Component relevance is exact, not
free-text semantic search. An empty environment filter is an explicit broad query;
automatic Context always uses exact Work component/current OS and at most three.
`candidates` is a bounded diagnostic window, not exhaustive pagination.
Explicit `show` returns the bounded raw record and superseded linkage; use query
for current derived staleness/dispute. It does not assert raw status is fresh.
`finish` defaults to failed (timeout/unavailable/invalid extraction); explicit
`--succeeded` marks only the optional job finished, never validates a candidate.
Existing `route base context` and `route base guard` remain compatible.

Existing event/history/cooperation surfaces retain Work, Finding, Knowledge
publication, validation and reuse events. Evidence remains in its existing
canonical Evidence store. Derived stale/disputed state is visible through Base
query; no second history log or full Route Console was added.

## Context and independent review

Relevant Base units include IDs, source references and the label
HISTORICAL_KNOWLEDGE_NOT_CURRENT_EVIDENCE. They grant no authority and cannot
satisfy CompletionGate. Oversized Context drops optional Base before failing the
existing overall bound. IndependentReviewer receives no Base conclusion or prior
Worker rationale; requirements/current code references and Evidence remain.
Security/root-cause/architecture investigations can select IndependentReviewer or
explicit `--independent`, then query Base separately for comparison after Finding.

## Acceptance evidence

Focused tests live in `route-basic/src/positive_base/tests.rs`. Each source-backed
scenario uses an isolated temporary project, authenticated source/review workers,
actual subprocess failure, a real fixture source edit and actual subprocess PASS.
The Knowledge text is labelled FIXTURE. Historical Q17/sandbox Evidence was not
invented or retroactively manufactured. Tests do not modify the real project Goal.

- Full chain: detector → candidate → independent review → validation → retrieval
  → future bounded Context → actual reuse PASS → REUSED_CONFIRMED.
- Component changed: STALE; subsequent current FAIL: DISPUTED; old records retained.
- Nine generic/overgeneralized statement fixtures rejected; invalid provenance,
  Evidence mismatch, creator collision and unsupported revision rejected.
- Duplicate rejected; contradictory failed approach retained/disputed; candidate
  budget enforced; pending/deferred/failed optional jobs do not change Goal state.
- Eight projection-only overlap units (not canonical Knowledge) verify <=3/8192B.
- Self-validation/missing review/reuse without proof/retired revival rejected.
- 100 synthetic ordinary successful Evidence records produce zero candidates;
  this is a no-garbage detector test, not 100 actual AI task executions.
- Repeated readonly retrieval leaves ledger bytes unchanged; independent review
  gets zero Base units; ordinary no-failure packet reads do not initialize history.

Final regression receipt: `Release/local-tools/base-positive-chain-20261013/results.json`.
Acceptance status and readiness are finalized only after the recorded commands
exit successfully. An ignored subprocess helper is intentionally invoked by the
real fixture, not silently counted as a standalone passed test.
The explicitly selected persistent audit helper retains a fresh controlled
project and `dogfood-receipt.json` under the ignored local log directory. Receipt
contains actual Goal/Work/Finding/Evidence/event/revision/Knowledge IDs, byte size,
and canonical ledger location. This is local audit data, not a public Release.

See [backlog reconciliation](architecture-improvement-backlog.md),
[readiness](practical-readiness.md), [Execution Trust](execution-trust-i.md) and
[Run Continuity](run-continuity-i.md). Prior historical claims are not rewritten.

## Final batch report

ROUTE_BASE_POSITIVE_CHAIN_I: **PASS — deterministic/manual, source-backed controlled
fixture scope. AI synthesis and unqualified AI daily-use certification NOT_RUN.**

```text
BASE:
starting HEAD=eb9751ba3d563e116ffcf0bf36a102c840cc0fca
branch=fruit
mixed tree=181 default entries / 189 individual original paths

PROVENANCE_AUDIT:
preexisting user paths=133
execution trust paths=7
run continuity paths=8
other known Route paths=41
unknown paths=0

CHECKPOINT:
commit=01952e7ea111d137e270d42140efd8c5d7c9089e
why safe=56 explicit known paths; shared historical phases acknowledged;
         every A path excluded; 133 original A file hashes/deletions unchanged
unrelated staged=NO

BACKLOG:
P0 total/done/partial/open=4/2/2/0
P1 total/done/partial/open=5/1/4/0
P2 total/done/partial/open=4/0/1/3
total/done/partial/open/blocked=13/3/7/3/0
strict completion=23.08% (3/13, scoped backlog only)
weighted completion=59.62% (15.5/26; P0/P1/P2=3/2/1, PARTIAL=0.5)

CHAT_VISION_GAPS:
implemented=2
partial=4
not_run=1
deferred=8
blocked_external=0

ROUTE_BASE:
canonical source=existing DevelopmentEvent CooperationKnowledge + existing Evidence
candidate schema=existing Lesson + optional bounded PositiveLesson
lifecycle=CANDIDATE / VALIDATED_LOCAL / REUSED_CONFIRMED / DISPUTED / STALE /
          SUPERSEDED (append linkage) / RETIRED

TRIGGERS:
implemented=FAIL_THEN_VERIFIED_FIX with structured RECOVERY:/ROOT_CAUSE: Finding
false positives=0 in 100 synthetic ordinary-PASS fixtures; no global rate claimed

ANTI_GENERIC:
fixtures rejected=9 statement attacks, plus malformed source/Evidence/scope/
                  creator/revision/nested-secret and self-promotion attacks
NARROW_SCOPE=PASS for tested exact component/OS; restricted condition cannot drop

PROVENANCE (retained fresh controlled scenario):
Goal=goal-0304523efff53f243d76dc81b7c271f922292543cead846ee714f066c3331e5f
Work=work-00e13ed7af55b27622f1d6eab5bec0147e68efe28dc2b12461117afa1a5ed40e
Finding=evt_01M4SK6A9YVM24A2DATDYY95JX
Evidence FAIL=01M4SK6A9MMK9P6P0D9P9P0FVJ
Evidence PASS=01M4SK6ABWFS0QXH5S02RHAVV1
Events=evt_01M4SK6A7ECG47SKZ4M3HCFX3X,
       evt_01M4SK6A9YVM24A2DATDYY95JX,
       evt_01M4SK69Y46251XX6VDC9CAKZ6
Revision=10

KNOWLEDGE_WORKER:
separate from Main Worker=separate registered creator + explicit optional job;
                          no live AI invocation is asserted
real AI synthesis=NOT_RUN
Host=preceding Codex availability evidence: Not logged in; no auth changes

BACKPRESSURE:
limits=1 job/Goal; 2 project pending; 0 auto retries; 8192B packet;
       2 original candidates/Goal; <=8 reuse proofs
queue behavior=explicit PENDING/DEFERRED; finish failed on unavailable/timeout
Goal blocked=NO in tested optional-job failures/budget denial

STALENESS:
behavior=current relevant fingerprint or missing source proof -> STALE;
         unrelated file alone does not stale scoped knowledge
CONTRADICTION:
behavior=retain both histories, derive DISPUTED; later same-check FAIL wins

RETRIEVAL:
max units=3, <=8192B, <=12 recent matching resource probes
query behavior=exact component/environment; candidates diagnostic window;
               no semantic index or silent merge
read-only growth=0: 100 actual CLI queries + independent query preserved
                 every retained project file hash

CONTEXT:
Base units injected=1 into a distinct later Work
provenance visible=YES
Knowledge treated as Evidence=NO
ANTI_HERDING:
independent context=zero Base conclusions, explicit later comparison available
result=PASS

DOGFOOD:
source=fresh controlled source fixture, not retroactive Q17/sandbox Evidence
trigger=actual subprocess FAIL -> attributed investigation -> source edit -> PASS
packet=884 bytes
candidate=KB-fixture-candidate (FIXTURE synthesis)
validation=KB-validated, independent review + explicit Operator
future Work=work-de517f29f6b3318f5e0455c083aec7d49113b18ece27264281e73c99b9b35d90
reuse Evidence=01M4SK6AGGPKQ7NQG0755HV8HP (check started at revision 15)
reuse=KB-reused, new independent review evt_01M4SK6AGTVQ8S9ZVD0SAYT1ZM

FULL_REGRESSION:
fmt=exit 0
route-core=exit 0 (25 unit + 1 doc)
positive focused=exit 0 (4 passed, 2 explicit helpers ignored by default)
route-basic=exit 0 (480 passed / 16 ignored)
route-cli=exit 0 (95 passed / 5 ignored across selected targets)
workspace excluding route-pyo3=exit 0
docs=exit 0
PowerShell parser=11 files / 0 errors; explicit UTF-8, PS 5.1
git diff --check=exit 0 (preexisting CRLF warnings are not whitespace failures)
explicit persistent dogfood helper=exit 0, 1 passed
actual CLI readonly/independent smoke=PASS, 100 queries + 1 blind query
source changed during final regression=NO (11 owned source SHA256 recorded)
preexisting A paths changed=0

READINESS:
KnowledgeTrigger=VERIFIED_FIXTURE
ExtractionPacket=VERIFIED_FIXTURE
KnowledgeCandidateFiltering=VERIFIED_FIXTURE
KnowledgeProvenance=VERIFIED_FIXTURE
KnowledgeLifecycle=VERIFIED_FIXTURE
KnowledgeStaleness=VERIFIED_FIXTURE
KnowledgeBackpressure=VERIFIED_FIXTURE
KnowledgeRetrieval=VERIFIED_FIXTURE
KnowledgeContextProjection=VERIFIED_FIXTURE
KnowledgeHerdingDefense=VERIFIED_FIXTURE
RouteBasePositiveChain=VERIFIED_FIXTURE
AIKnowledgeSynthesis=NOT_RUN
unqualified AI DAILY_USE_VERIFIED=NO

KNOWN_LIMITATIONS:
RealAIHostRestart/CrossHost/real AI synthesis NOT_RUN.
Only one structured detector and explainable exact relevance are implemented.
Human/fixture prose correctness is not automatically proven by these gates.
Full-ledger replay/Evidence whole-file load remain; no large-ledger RSS/p95 claim.
No auto scheduling, semantic merge, model/provider/token routing, DAG or GUI.
The 100 synthetic PASSs are detector fixtures, not 100 AI work executions.

GIT:
commit=01952e7ea111d137e270d42140efd8c5d7c9089e (reliability checkpoint only)
push=NO
local_SHA=01952e7ea111d137e270d42140efd8c5d7c9089e
remote_SHA=eb9751ba3d563e116ffcf0bf36a102c840cc0fca
ahead/behind=1/0 at fetched origin
worktree=133 preserved A paths + 14 known new-phase paths; index empty
new Base implementation/docs=verified, left uncommitted and separately reviewable

MAIN_MODIFIED:NO
TAG_CREATED:NO
RELEASE_CREATED:NO (no public Release; ignored local receipts are not a Release)
FORCE_PUSH:NO
YUICH_CHANGED:NO (preexisting in-repo migration deletions preserved, not new work)
```

Local audit artifacts (ignored, not portable repository links):

- `Release/local-tools/base-positive-chain-20261013/results.json`: final command
  exit codes, source SHA256s and A-path preservation; finished UTC
  `2026-10-13T02:59:05.0221048Z`.
- `Release/local-tools/base-positive-chain-20261013/dogfood/dogfood-receipt.json`:
  real IDs and canonical ledger location.
- Retained project `Release/local-tools/base-positive-chain-20261013/dogfood/source-backed-vre3LM`:
  17 files, 209857 bytes when measured before readonly CLI acceptance. Actual FAIL
  source hash differs from the fixed PASS hash; reuse retains fixed source bytes.
- `Release/local-tools/base-positive-chain-20261013/readonly-cli-results.json`:
  100 actual query processes + one independent query, all project file hashes
  unchanged. 4679 ms total in this small Windows fixture, not a scale benchmark.

Acceptance harness corrections were not product passes: the initial CLI duplicate
Base enum failed compilation and was fixed by flattening into the existing group;
Windows PS5 normal stderr was initially misclassified as failure; historical
UTF-8/no-BOM demo source needed explicit decoding; PS5 represents parsed `[]` as
null, which the readonly harness now handles correctly. Final PASS uses real exit
codes and final frozen source, not those earlier failed attempts.
