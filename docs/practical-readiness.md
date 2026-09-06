# Practical readiness and development gates

Audit date: 2026-09-06. Current daily-use work on `fruit` is based on
`cbbb032`; the original audit below was based on `4e0e2865`. This is not a release certification or a
replacement for the historical feature matrix. No new runtime is proposed.

## Intended useful outcome

A developer should be able to register useful information, resume a task in
another process, discover what changed, and reuse explicitly qualified
knowledge without losing project identity, history, or control over execution.
Route must save more recovery/context work than it adds in maintenance.
Model definitions and successful serialization alone do not demonstrate this.

## Initial audit findings and closure status

| Priority | Evidence in current code | Practical consequence / acceptance |
|---|---|---|
| P0 | `rpc.rs` listed new methods without dispatch arms | Fixed in this audit: do not advertise them; reject before receipt/state writes. Re-enable individually only with real transport tests. |
| P0 | `ReferenceRegistry::read_unlocked` treated existing blank JSON as a new registry | Fixed: reject empty/truncated files, preserve bytes; missing registry remains a supported first-use case. Legacy Reference callers now propagate corruption errors; see closure evidence. |
| P0 | Knowledge fingerprint comparison used `Option::zip` | Fixed: losing a previously observed fingerprint makes bound knowledge stale; stale capabilities must not remain in the projection. |
| P0 | Reference registry writes and commons events have separate persistence paths | Implemented: durable Reference operation journal, registry CAS, deterministic forward recovery and one keyed ledger event; legacy writes share this boundary. |
| P0 | Knowledge can carry OBSERVED through the generic record path; capability-specific evidence checks are separate | Implemented: one stateful validator under the ledger lock for generic and dedicated ingress; successful System Evidence must bind project/resource/fingerprint. |
| P0 | Cooperation projection inserts records by ID; append lock does not establish all domain uniqueness/supersession rules | Implemented: locked ID/supersession validation; concurrent conflicting replacements have one winner. |
| P0 | Registry and development locks infer staleness from a 30-second age | Implemented: kernel-owned file locks plus PID/nonce metadata; real Windows child holds beyond 30 seconds and exits abruptly, then recovery succeeds. |
| P1 | Reference reads acquire a lock that creates the reference directory | Implemented: no lock acquisition/initialization on Reference reads; Cooperation reads do not initialize or rebind identity. |
| P1 | `material.rs` scans `constraints/`, not Constitution/Protocol/intent | Coverage report implemented: bounded constraints/ppam metadata is PARTIAL; prose and active intent rules are explicitly NOT_PROJECTED, not silently omitted. |
| P1 | Commons loads and rewrites a complete JSON ledger; directory scanning and locator checks need adversarial bounds | Measured in the daily-use batch: E10K p95 602.381 ms and peak 30.38 MiB after eliminating repeated ledger verification; bounded large-file/directory, junction, credential-marker and corruption process tests pass. |
| P1 | New substrate has library tests but no complete CLI/RPC worker handoff | Verified in real processes: A records, B reads delta/reuses, C retries; crash/recovery, external change and foreign-project no-write tests pass. |

## Definition of usable

These are acceptance requirements, not measured results. A capability advances
separately through `DESIGNED`, `LIBRARY_TESTED`, `SURFACE_TESTED`, and
`DAILY_USE_VERIFIED`. Missing evidence is `NOT_RUN`, not an implicit pass.

1. Fresh disposable project: documented commands work without manual state
   edits; errors give a safe recovery action. No change outside the named root.
2. Restart and concurrency: two independent processes see the same ordered
   changes; retries do not duplicate effects; interrupted writes either recover
   or fail closed without erasing history. Test the crash window, not only the
   happy path.
3. Epistemics: declaration, inference and observation remain distinct; cited
   evidence exists; replaced/missing resources invalidate applicable knowledge.
4. Safety: no execution from registration, no implicit foreign-project writes,
   no secret/raw reasoning persistence, no recursive Wiki/Release ingestion.
5. Resource budget: publish binary SHA, machine/RAM, dataset size, elapsed time,
   peak working set, bytes read/written and retained disk growth. Test empty,
   1,000 and 10,000-event histories plus a large external resource. Proposed
   local-interactive target: p95 read <= 1 second, peak <= 128 MiB for metadata
   queries; a measured exception needs explicit review, never silent waiver.
   Compare 100 repeated reads: they must not grow durable state. No background
   polling, full reference copy or extra build target without a stated need.
6. Dogfood: three complete task/resume cycles across process restarts, including
   a failed operation and recovery. Record expected/actual outputs and usable
   limitations. This gate does not imply cross-host or cross-model validation.

## Development rules for the current backlog

- Start each patch with a concrete user scenario, failing regression, canonical
  state owner, allowed paths and failure/retry behavior. No new conceptual
  layers until the existing workflow closes.
- Domain validation belongs below transports. A CLI/RPC adapter may not weaken
  it. Capability discovery lists only implemented dispatch routes.
- Persistence changes require concurrent writer, duplicate replay, restart,
  corruption and failure-injection tests. Preserve historical bytes and schema
  compatibility; never use resetting state as the recovery implementation.
- Reuse one bounded build target; do not start another build against a locked
  target. Build/cache/Release are not source or snapshots. Cleanup requires an
  exact target, an ownership check and appropriate user authority.
- Before merging a completed batch: formatting, route-core/basic/cli tests,
  workspace tests excluding route-pyo3, doc links, PowerShell parsing and diff
  checks. Record exit codes and distinguish targeted from full regression.
- A handoff records changed files, tests actually run, remaining acceptance
  gaps and the next smallest closure step. No phase PASS, commit/push or release
  claim on the strength of library tests alone.

Current disposition: **the exact local Reference/Cooperation workflows below
have daily-use process evidence**; this is not production certification of Route
as a whole. See [daily-use evidence](cooperation-daily-use.md) for commands,
actual request/response shapes, limitations, benchmark fixtures and regression
results. Historical [reliability closure](cooperation-reliability-closure.md)
remains a separate phase.

## Capability-specific readiness

| Capability | DESIGNED | LIBRARY_TESTED | SURFACE_TESTED | DAILY_USE_VERIFIED |
|---|---|---|---|---|
| Reference registry/journal/recovery | YES | YES | YES, dedicated RPC + CLI | YES, local register/resume/crash/recover/retry/refresh |
| CooperationResource | YES | YES | YES, dedicated RPC + CLI | YES, local/external bounded resource and foreign-project no-write |
| CooperationKnowledge | YES | YES | YES, dedicated RPC + CLI; OBSERVED rejection gate | YES for bound DECLARED learning, stale projection and supersession; positive OBSERVED creation through a host is NOT_RUN |
| MultiWorkerSharedLearning | YES | YES | YES, two distinct worker IDs and actual OS processes | YES for same-project delta/reuse with original provenance |
| ConstraintProjection | YES, partial coverage only | YES | NOT_RUN for a dedicated transport | NOT_RUN |
| OS lock ownership | YES | YES, real crash/age tests | Used through domain surfaces | Exercised in local recovery; not a distributed lock |
| Cross-host validation | Protocol design | No new claim | NOT_RUN | NOT_RUN |
| Cross-model validation | Host-neutral design | No new claim | NOT_RUN | NOT_RUN |

Evidence is bounded: E0/E1K/E10K contain history findings, not 10,000 resources;
the benchmark project has no populated Git tree. The large resource is 256 MiB
and remains external. Full per-method metrics and binary/machine identity are
in [budget results](cooperation-daily-use-budget.json). Whole-ledger read cost
still scales linearly. At the tested scale no segmented ledger is warranted.

## Original audit checks (historical; before reliability closure)

- `cargo test -p route-basic --lib`: exit 0, 380 passed, including blank
  registry rejection and missing-resource staleness regressions.
- `cargo test -p route-cli --lib rpc::tests`: exit 0, 11 passed, including
  unimplemented method rejection without state creation.
- `cargo fmt --all -- --check`, `scripts/check-docs.ps1`, parsing root
  `scripts/*.ps1`, and `git diff --check`: exit 0.
- An initial RPC filter against the binary target selected zero tests; it is
  not counted as RPC evidence. The library-target run above is the real check.
- Full workspace regression, resource-budget measurements, and the new
  cross-process substrate acceptance scenarios: NOT_RUN in this audit.
- Existing unfinished RPC helper warnings remain. No commit, push, history
  rewrite, release, or change to Yuich was performed by this audit. Preexisting
  worktree deletions were left untouched.
