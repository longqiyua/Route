# Autonomous Team and Passive Base I — acceptance

Phase gate: **ROUTE_AUTONOMOUS_TEAM_BASE_I: PASS**, within the single-real-Host
and explicitly scoped readiness below. Real AI knowledge extraction is NOT_RUN;
healthy no-signal completion does not require manufacturing a lesson.

## Real Host and Goals

Codex CLI `0.162.0-alpha.2`, pinned executable SHA-256
`3553cd6e7df5a093d8cb8301cd8088a57e0971aba71ddbe0e67f7f44a15cdf68`.
Observed model: UNKNOWN; configured model is not provider-attested observation.
No second Host/model is claimed. No provider configuration/authentication repair
was attempted. Cross-tool previous result remains PARTIAL; CrossHostHandoff and
CrossModelHandoff remain NOT_RUN for the documented external availability blocker.

Disposable nontrivial Goal: implement a compact duration parser with ASCII-only
integer terms, strictly descending `h/m/s/ms`, checked u64 arithmetic, rejection
of malformed/Unicode/overflow input and regression tests, without dependencies.
Goal ID: `goal-f61b4a26a4578963edc8814267f4825ef1ca13a8bab58b288d833769ff23af6e`.

The actual Planner chose SPLIT and created two WorkItems:

1. Implement parser and regression tests.
2. Independently review correctness and coverage, dependent on item 1.

| Worker ID | Temporary display role | Actual Host | Work |
| --- | --- | --- | --- |
| worker-01M4K4H0BE1BY6A5F25JPV23PQ | Planner | Codex | Decide/decompose/route |
| worker-01M4K4PAV24KCCYA54SAVXH3WX | Implementer | Codex | work-3cf5b83967d38909e569f5d7f08db2ec2f83eab0e0c2ddd0d0cf2ffb58143fb3 |
| worker-01M4K4WM7NKTGE5BQKB4J9TQV8 | IndependentReviewer | Codex | work-b022f713c36488815fb07db00caa46ab32ac9a76b690dee8a4c7adcea02705ef |

All three separate invocations exited 0. Distinct bindings/principals, not names,
establish identity. Routing records used DECLARED capability labels, not fabricated
observations. Reasons were short structured explanations: keep single-file code
and tests together, then use a distinct dependent reviewer for grammar/overflow.

Human Worker assignments=0; Human file assignments=0; Human synchronization
instructions=0. Worker count=3, reused=0, peak concurrent actual invocations=1,
maximum depth=1, attempts=3, runtime budget denials=0. Safety ceiling was 2 concurrent,
3 distinct Workers, 4 attempts and 900-second timeout; no parallel speedup claim.

Actual initial Context Pack bytes: Planner 1,344; Implementer 2,065; Reviewer 2,087.
Prompt bytes: 2,988 / 3,112 / 3,133. Base units injected=0. The fresh project had
no unrelated Goal history; unscoped control-plane events were not injected as
history. No raw transcript, private reasoning or extraction task was included.
Initial reviewer input withheld prior Findings, routing rationale and Base
conclusions. Review Finding: `evt_01M4K5197NM7AC2WA6AMPVQ71T`; source unchanged.

The first worker-only run correctly left completion DENIED: no accepted Plan or
System Evidence yet. The control-plane verification closure was then implemented
and exercised against that same actual delivered work, without reassigning it.
Seven explicitly approved checks ran current Cargo tests, independent boundary
assertions and verification of distinct reviewer publication. All exit codes=0,
state_stable=true; existing Work integration and CompletionGate passed.
Final Goal state=SUCCEEDED, completion=PASS. This is not a process-exit shortcut.

Independent executable assertions exercised over 2,000 additional valid duration
cases plus invalid grammar, Unicode, zero-order and overflow boundaries. The AI's
own Cargo suite contained 3 test functions. Validation artifacts stay in ignored
`Release/acceptance/`; they are not published packages or canonical knowledge.

## Do not split acceptance

A second real Goal requested only `is_even(u32)` and a test for 0, 1, 2.
Goal: `goal-08061c9f31a15a70f7fe44d8ff7248399b7289095ca54909c5ca6811f68843f5`.
Worker: `worker-01M4K5GAECDYKSSJX6PQH71Z3J`.

The AI chose `split=false`, `role=PLANNER`, reason `DO_NOT_SPLIT: One pure function
and a three-case unit test; one Worker can implement and verify directly.`
It created, claimed, implemented and finished its own single WorkItem. No second
Worker was started. Initial context=936 bytes, prompt=2,861 bytes; process exit=0.
An explicitly approved Cargo argv policy automatically produced two current
System checks, integrated work and closed Goal SUCCEEDED/PASS in the same start
workflow. Human assignments/sync=0, Workers=1, peak=1, depth=0, attempts=1.

## Passive Base truth

Both actual Goals encountered no published high-signal reusable issue. Triggers=0,
real candidates=0, extraction calls=0, queue peak=0; Knowledge Worker=NOT_RUN for
the healthy no-signal path. No lesson was manufactured. Main task prompts did
not ask for lessons. Base canonical source remains CooperationKnowledge events;
there is no separate index/database. A known generic candidate is rejected by a
clearly synthetic validator fixture, never entered as real project knowledge.

Candidate/source/scope/dedup/stale/lifecycle/backpressure tests are synthetic
deterministic fixtures, not real AI knowledge-extraction certification. Current
System Evidence overrides older matching PASS; knowledge IDs cannot serve as
Evidence. Optional extraction failure leaves Goal state unchanged. See
[the exact contract and limitations](autonomous-team-base.md).

## Control-plane measurements

20 samples per interface, including Windows CLI launch and ledger I/O; no AI/model
calls. Measurements are from the completed duration fixture, not a large-history
benchmark. No embeddings/index exists, so Base index rebuild is NOT_APPLICABLE.

| Read | p50 ms | p95 ms |
| --- | ---: | ---: |
| Routing/status projection | 46.62 | 108.37 |
| Implementer context | 165.53 | 176.89 |
| Independent reviewer context | 174.69 | 221.63 |
| Knowledge detector | 47.90 | 66.08 |
| Base query | 48.06 | 54.93 |

100 reads: durable growth=0 bytes; complete local state file hashes unchanged.
Actual nontrivial Host process spawn calls took 32.65 / 37.43 / 57.61 ms; trivial
spawn took 33.28 ms. These are OS spawn only, not full broker/model startup.
Executable SHA verification, binding provisioning and context preparation are
additional costs; they are not included in those numbers. The current adapter
separately emits streamed executable-pin verification and OS spawn timings.
Model/network startup and inference latency were not independently measured.

## Security and failure acceptance

Agent attacks A-G: distinct-worker limit, depth limit, bounded retries, two actual
processes racing for one slot, owned-child cancellation/reaping, grant escalation
denial and display-name collision have focused tests. Timeout kills/reaps only
the broker's own Child handle. Running cancellation retains its slot until reap.
Authenticated Goal-scope/stopped-write/replay tests cover task-local narrowing.

Knowledge attacks A-H: generic advice rejection, missing source Evidence denial,
broader scope denial, current FAIL overriding prior PASS, stale retrieval exclusion,
failed optional job leaving Goal unchanged, 100 synthetic ordinary-success
Findings yielding no extraction/lessons, and explicit dedup/supersession are
test fixtures (not 100 independently executed AI Goals). The existing
cross-process supersession/concurrent append tests also remain passing. Synthetic
fixtures are not counted as real lessons or real Knowledge Worker invocations.

## Readiness scope

| Capability | Readiness |
| --- | --- |
| AutonomousWorkDecomposition | DAILY_USE_VERIFIED, single Codex Host |
| AgentBudget | DAILY_USE_VERIFIED limits; attack cases TEST_VERIFIED |
| WorkerBroker | DAILY_USE_VERIFIED; native cancel/timeout TEST_VERIFIED |
| ExplainableRouting | DAILY_USE_VERIFIED, DECLARED reasons |
| RoleSpecificContext | DAILY_USE_VERIFIED |
| IndependentReviewContext | DAILY_USE_VERIFIED |
| PassiveRouteBase | TEST_VERIFIED; real no-signal path verified; extraction NOT_RUN |
| KnowledgeCandidateFiltering | TEST_VERIFIED, not semantic certification |
| KnowledgeBackpressure | TEST_VERIFIED; actual queue peak=0 |
| KnowledgeHerdingDefense | Initial blind review DAILY_USE_VERIFIED; Base exclusion TEST_VERIFIED |
| AutonomousTeamSingleHost | DAILY_USE_VERIFIED |
| CrossHostHandoff | NOT_RUN |
| CrossModelHandoff | NOT_RUN |

Knowledge Worker is a separate invocation/Principal when explicitly requested
with a supported signal. Real invocation=NOT_RUN; packet bound=8 KiB, jobs=1/Goal,
pending=2/project, retry=0, candidates=2/Goal. Optional failure never closes/fails a
Goal. No claim is made for actual AI extraction quality without a real signal.

Known limitations: conservative serial scheduler, same-OS-user trusted boundary,
no OS confinement of arbitrary Host tools/descendants, no model diversity, no
automatic historical mining, bounded output but history-dependent ledger replay,
and approved verification commands are not a semantic oracle for requirements.

## Final regression

Final source: fmt check PASS; route-core PASS (22 unit + 1 doc test); route-basic
PASS (431 passed, 4 ignored subprocess fixtures); route-cli PASS (including native
cancellation/timeout and independent process integration); full workspace excluding
route-pyo3 PASS, exit 0. Documentation links, PowerShell parsing and diff checks
PASS. Eight existing route-basic unused-variable warnings remain; they are not new
failures. Focused Agent/knowledge fixtures and real Host dogfoods are distinguished
above. Release fixtures remain ignored/untracked, credentials stay external, and
the three preexisting unrelated Yuich deletions are excluded from this change.
