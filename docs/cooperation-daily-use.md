# Reference/Cooperation daily-use surface audit

Phase: ROUTE_COOPERATION_SURFACE_DAILY_USE_I. Baseline: `cbbb032`.
Initial working tree: only the three preexisting Yuich-related deletions;
these are outside the phase and must remain untouched.

## Pre-code mapping

| Surface | Existing canonical domain path |
|---|---|
| reference.list/get | ReferenceRegistry::read |
| reference.register/refresh | keyed ReferenceRegistry operation journal/CAS/event bridge |
| reference.recover | ReferenceRegistry::recover (previously library-only) |
| cooperation.list/get | ledger-backed resource projections |
| cooperation.register/refresh | register_cooperation_resource / refresh_cooperation_resource |
| cooperation.knowledge.query/record | cooperation_knowledge / record_cooperation_knowledge |
| development.state/events.query | existing commons projections/delta query |
| worker.* | existing worker identity/presence/message methods |

Legacy Reference CLI add/list/show/inspect/refresh already exists. New work is
a minimal Cooperation command group, an actionable Reference recovery command,
and dedicated adapters that never duplicate domain writes or Evidence rules.
Unused Reference RPC helpers were removed during reliability closure: none are
an implemented surface. The current capability list is hand-maintained and
will be derived from dispatch definitions instead.

All newly exposed mutation methods require operation keys through route/1.
PENDING retry must resume the existing domain identity, not create another
operation. Read endpoints must not initialize or rebind identities.

Verification planned: three real process cycles (normal handoff, controlled
failure/recovery, resource change); worker provenance; foreign-project no-write;
100-read stability; E0/E1K/E10K/large-resource measurement. No daily-use claim is
made by this initial audit. Cross-host/model validation remains NOT_RUN.
## Supported human workflow

Run from the project directory. Resource registration never executes its target.
Use a new operation key for a new intent, and the same key and parameters to retry.

```powershell
route init
route reference register ref-notes ./notes.txt --operation-key ref-notes-1
route reference list
route reference show
route cooperation add notes ./notes.txt --kind DOCUMENT --provenance worker-a --operation-key notes-1
route cooperation list
route cooperation show notes
route cooperation knowledge record notes-k1 notes "Declared usage notes" --provenance worker-a --status DECLARED --operation-key notes-k1
route cooperation knowledge show --cooperation-id notes
route cooperation state
route cooperation events --after 0
# Exit, reopen a terminal, and run the read commands again.
route cooperation refresh notes --operation-key notes-refresh-1
route reference observe ref-notes --operation-key ref-refresh-1
# Only when an interrupted Reference operation requires reconciliation:
route reference recover --operation-key recovery-1
```

Existing `reference add`, imported-source proposal `reference refresh`, and
`reference inspect` remain available with their prior semantics. The new
`reference observe` performs bounded availability/fingerprint refresh; it does
not apply an import proposal. Missing local resources can be registered.

Knowledge record supports `--worker`, `--fingerprint`, repeated `--capability`,
repeated `--evidence`, and `--supersedes`. Fingerprint binding is explicit:
copy the value displayed by cooperation show; no manual Route-state editing
or JSON construction is needed. OBSERVED requires existing successful System
TestPass/CheckPass Evidence whose metadata binds project_id, cooperation_id,
and resource_fingerprint. Host claims/messages cannot manufacture that trust.
Positive trusted-Evidence creation is not added by this phase.

## Dedicated route/1 contract

Implemented dispatch generates capability discovery from the same macro arms.
The eleven new methods are:

- reference.list, reference.get, reference.register, reference.refresh,
  reference.recover
- cooperation.list, cooperation.get, cooperation.register, cooperation.refresh
- cooperation.knowledge.query, cooperation.knowledge.record

Reads return arrays or one object. reference.get takes reference_id;
cooperation.get/refresh takes cooperation_id; knowledge.query optionally takes
cooperation_id. Missing get targets return an actionable error, not an invented
record. Dedicated Reference registration currently records the generic OTHER
type; richer existing Reference types remain available through legacy add.

Reference mutations return reference_id, operation_id and global_revision;
recovery returns recovered=true and reset_performed=false. Cooperation mutations
return the canonical event, global_revision and replay flag. A transport retry
returns the stored receipt, with receipt.replay=true, even in a new process.
Changing the parameters with the same key returns IDEMPOTENCY_CONFLICT.

Exact request example exercised by the process tests:

```json
{"protocol":"route/1","request_id":"daily","method":"cooperation.register","context":{},"params":{"cooperation_id":"resource","locator":"resource.txt","kind":"DOCUMENT","provenance":"worker-a","actor_worker_id":"worker-a"},"idempotency_key":"resource"}
```

Actual response structure (generated IDs, timestamps and hashes omitted here):

```text
ok: true
result: { event: { event_id, project_id, sequence, actor_worker_id,
                  payload, previous_hash, hash, ... },
          global_revision, replay: false }
error: null
receipt: { method: cooperation.register, operation_status: COMPLETED,
           idempotency_key: resource, ... }
```

development.state returns result.state and result.stale (from seen_revision);
development.events.query returns result.project_id/global_revision/events.
Provenance is preserved on result knowledge records; querying does not transfer it.

cooperation.discovery.record, cooperation.capability.record and constraint.list
remain unadvertised: no dedicated tested daily-use adapter. Existing generic
event ingress remains subject to the canonical domain and Evidence validator.
Task RPC remains unavailable. No new worker provider or cross-host runtime exists.

## Real process evidence

`tool/route/crates/route-cli/tests/daily_use_e2e.rs` launches the actual Route
binary for every workflow operation. Initialization uses route init. No daily
cycle modifies .route by hand; only the separate malformed-metadata adversarial
test deliberately corrupts a disposable fixture.

| Gate | Actual outcome |
|---|---|
| Normal resume A to B to C | Stable project ID; B consumes A's event delta, sees Reference/resource/knowledge; C retries without a duplicate domain effect |
| Two workers | Distinct worker-a/worker-b registered; B updates its seen revision; A's knowledge provenance remains worker-a |
| Controlled process death | Test-only feature exits process with code 23 after registry domain write, before event completion |
| Recovery | Fresh read reports REFERENCE_RECOVERY_REQUIRED with the CLI action; recover reconciles; retry leaves revision unchanged; next registration succeeds |
| World changed | External file modified, refreshed from a new process; bound declaration stale and capability removed; old record retained; replacement recorded; later removal reports MISSING |
| Foreign project | Independently initialized A and B have different IDs; A registers B as PROJECT; B's complete file/directory bytes and mtimes unchanged |
| 100 reads | Full project file contents, mtimes and directory set identical; zero events, receipts or logical durable growth |
| Mutation replay | All six dedicated mutations tested across fresh processes; changed-parameter retry rejected; canonical registry/ledger/receipt unchanged |
| JSONL | Five dedicated reads on one real JSONL transport; complete snapshot unchanged |
| Adversarial | 256 MiB external file; 2,000-entry directory; missing/changed; credential URI/name; traversal rejection; normalized Windows path; ancestor junction unavailable; corrupt registry preserved |

The crash hook is compiled only with test-utils; normal builds contain no
environment-variable-controlled exit hook. The resource-budget fixture helper
is also test-only, refuses nonempty ledgers and caps seeding at 10,000 events.
Preseeded history is used only for measurement, never as daily-use evidence.

A real failing 100-read test found CLI-wide history telemetry behind otherwise
read-only RPC. RPC now relies on domain events/receipts for mutation auditing;
the transport and dedicated read CLI commands do not append invocation history.
Mutation lock ownership metadata may change during retries; the replay test
compares canonical domain/receipt files, whereas read tests compare everything.

## Performance decision

Initial debug E10K shared-state p95 was 2374.558 ms, peak 44,417,024 bytes:
a real budget miss, not waived. Inspection showed five full parses/hash
verifications of the same ledger per shared-state query. A targeted fix reuses
one verified snapshot for resource/knowledge/worker/presence projections.
This also avoids internally inconsistent revisions between those projections.

Final resource results are recorded in [measurement evidence](cooperation-daily-use-budget.json).
One existing target was reused, no release profile or extra target introduced.
All queries include independent process startup; 20 runs, nearest-rank p50
(10th) and p95 (19th), Windows K32GetProcessMemoryInfo peak working set.
OS: Windows 11 Home Chinese 10.0.26200; CPU: AMD Ryzen 9 7945HX, 16 cores/
32 logical processors; installed RAM: 17,179,869,184 bytes (16 GiB),
OS-visible physical memory: 16,338,731,008 bytes.

Binary is debug with test-utils, SHA-256
`aad8b96904b5a67950ca1fd99c75d55499c48ec83b8ffc593a9fee3304b0cbcf`,
38,375,936 bytes. Build source: cbbb03271e8d3257b26ad7395cb9466463d2c681 plus
this phase's working-tree patch; it is not represented as a clean baseline build.

| Fixture | Durable bytes | Worst query p50/p95 ms | Max peak MiB |
|---|---:|---:|---:|
| E0 | 408 | 145.091 / 159.524 | 14.14 |
| E1K | 567083 | 187.785 / 203.764 | 15.36 |
| E10K | 5697084 | 571.641 / 602.381 | 30.38 |
| LARGE, external 268435456 bytes | 7860 | 167.773 / 218.491 | 15.34 |

Each row's worst query is development.state. Full per-method results, response
bytes and samples are in the JSON. All 320 measured reads preserve durable
bytes/hash/mtime/directory sets. Windows physical I/O bytes: NOT_MEASURED, no
estimate substituted. Event query returns min(100,event_count) events;
Reference/Cooperation lists are empty in E0/E1K/E10K, one entry each in LARGE.
Shared state returns at most 50 recent events. This measures history scaling,
not 10,000 resources or a populated Git working tree.

Current ledger is sufficient for the tested scale. No PackStore/segmentation.
Residual limits: whole-ledger cost remains linear; directory inspection is
metadata-only; large-file head/tail+mtime fingerprint is bounded, not a
cryptographic proof of unchanged interior bytes. No recursive ingestion,
external content copy, arbitrary execution or automatic secret scrubber.
Cross-host and cross-model validation: NOT_RUN.
Full regression initially found an old test that implicitly initialized a fresh
project via event query; it now initializes explicitly and first asserts the
uninitialized read leaves no files. It also exposed Windows access-denied during
concurrent history directory-lock deletion. History append now reuses the existing
OS ownership lock and never removes the lock path or steals based on age.
The four history integrity/concurrency tests pass after this correction. Legacy
crash-left directory locks are not silently deleted or migrated.
## Final regression and scope gate

| Command / gate | Actual result | Exit |
|---|---|---:|
| cargo fmt --all -- --check | PASS | 0 |
| cargo test -p route-core | 22 unit + 1 doc = 23 PASS | 0 |
| cargo test -p route-basic | 393 PASS, 2 subprocess fixture functions ignored by direct runner | 0 |
| cargo test -p route-cli | 60 PASS | 0 |
| cargo test --workspace --exclude route-pyo3 | 597 PASS, 2 fixture functions ignored | 0 |
| cargo test -p route-cli --features test-utils --test daily_use_e2e | 7 PASS; benchmark deliberately separate | 0 |
| daily_use_e2e resource_budget_real_binary -- --ignored --nocapture | 1 PASS; 320 measured independent reads | 0 |
| powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-docs.ps1 | PASS | 0 |
| Parser validation of root scripts/*.ps1 | 4 files PASS | 0 |
| git diff --check | PASS | 0 |

The two ignored basic functions are explicitly spawned by their parent crash/
lock tests, not untested requirements. Standard default CLI/workspace tests do
not compile the feature-only crash/benchmark tests; those are run separately as
shown. Existing unused-variable warnings remain, not new failures.

Scope audit: 14 intended Route files (source, transport tests, docs and compact
benchmark JSON). No generated executable, build/cache/temp, Release, private
material or unknown file included. Synthetic adversarial credential strings
are fixtures, not live credentials. Three preexisting Yuich deletions remain
unstaged and unchanged. Main worktree is read-only; no tag/release/force/history
rewrite or cross-project development performed.

Readiness gate: local daily-use workflows and resource budgets PASS. This is not
a claim that positive OBSERVED creation, cross-host/model operation or complete
Constraint projection has been dogfooded. Git outcome is reported separately
after normal non-force push and remote readback.
