# Route AI v1 bounded closure audit

Audit date: 2026-10-09. Scope: `fruit` at `985a31e6a9f62d265fcdae2f9a9af291afa4056e` plus this documentation-only closure change. This is a local, same-host bounded assistant acceptance, **not** a public release certification.

## Decision

`ROUTE_AI_V1_BOUNDED_ASSISTANT_READY = YES` for this precise boundary: Human gives a bounded Goal; Route persists a canonical structured Goal, Plan and versioned Workflow; bound AI Workers self-select and report non-code Work; Decisions and Observations can revise the Plan; independent processes reconstruct state; System proof and integration deny skipped required steps; truthful Outcome and history remain queryable. Route does not invoke a model, judge the semantic quality of an answer, or perform arbitrary external actions. The real AI Host remains responsible for reasoning and external tools.

The first real-user gap was a missing executable Goal→Workflow→Plan intake example. The small [PowerShell golden path](general-work-assistant.md#first-goal-on-a-fresh-project-powershell) now includes authenticated Worker Work/Claim/Finish. The block was extracted from the document, run in a second fresh disposable project, and its Plan read back in a new `route.exe` process. That project reported 0/1 required obligations after a completed claim, correctly withholding System proof. An optional external archive registry write was skipped by this sandbox; local initialization and the Route loop passed.

The actual non-code closure Goal is `goal-a882afcd9bc008dd009740265c40817ba91ff5488b624bf4429e61c83000f07c` in a separate disposable project. Two genuine same-host AI Workers (`closure-worker-alpha`, `closure-worker-beta`) used distinct bound credentials, self-selected Work, registered draft artifacts and finished claims. A third bound identity represented the AI Host's own smoke/synthesis Work. No Human assigned individual Worker tasks. The canonical Observation `observation-0d2efab1f445c2adea2851d9af1d217fc9504ec775a8b23bfeb95b52965fa417` led to accepted PlanDelta `delta-f5ca4ce6821c8f213e6e997878323f383a1b2a7cb54b2a2aad6bcbce4906dd60`: Workflow and Plan v1 remained queryable beside v2. A real Decision about the onboarding blocker was requested and answered without inventing Human release approval.

At revision 35, an authenticated premature `workflow.complete.request` returned `DENIED` despite Worker claims. After six current `plan.verify_step` System proofs, conditional fix-step skip, and integration of eight finished claims, `workflow.complete.check` and `plan.review` returned `PASS` with no missing, stale, failed or unauthorized-bypass items. A FINAL Artifact and FINAL Outcome were recorded; a new process read the Goal `SUCCEEDED`, Workflow/Plan versions `[1,2]`, 7/7 active obligations and `PASS` at revision 67. The disposable project's ledger is the canonical machine evidence; this document is its durable human-readable audit, not a replacement for it.

Current regression after the documentation repair: `cargo test --workspace --exclude route-pyo3 --quiet` exited 0 with **637 passed, 0 failed, 5 ignored**. Focused `route-cli` process tests for Principal Binding, Autonomous Work, Execution Contract and General Work exited 0 with 10 passed. `cargo fmt --all -- --check`, `scripts/check-docs.ps1`, PowerShell parser validation for four scripts, and `git diff --check` passed. The successful Rust commands ran outside the sandbox because this host's sandbox denied Rust `canonicalize` on otherwise readable workspace/temp paths; a sandboxed test attempt accordingly failed for environmental reasons and is not counted as a code regression.

## V1 closure matrix

| Area | Classification | Concrete evidence / limit |
|---|---|---|
| A Project / Workspace identity | COMPLETE | Two fresh `route init` projects; route/1 project/workspace IDs and local status. |
| B Development History | COMPLETE | Hash-chained global events; Goal state and Workflow/Plan v1/v2 recoverable at revision 67. |
| C Evidence / Verification | COMPLETE_WITH_KNOWN_LIMITATION | Six current System CheckPass proofs; `plan.verify_step` is structural, not semantic. |
| D KnownGood / Recovery | COMPLETE_WITH_KNOWN_LIMITATION | [Recovery contract](recovery.md) and independent-process reconstruction; no distributed snapshot guarantee. |
| E Git governance | COMPLETE_WITH_KNOWN_LIMITATION | fruit/origin synchronized at audit baseline; three preexisting Yuich deletions excluded. Main reconciliation is separate. |
| F route/1 | COMPLETE | Live `system.hello` advertised Goal, Plan, Work, Workflow and Assistant; RPC mutations and reads exercised. |
| G Global Event Ledger / GlobalRevision | COMPLETE | Revision 67, immutable PlanDelta v1→v2, current projection without chat transcript. |
| H Worker identity / presence / messages | COMPLETE_WITH_KNOWN_LIMITATION | Distinct authenticated Worker bindings and self-claims; no hostile same-OS-user isolation claim. |
| I Reference / Cooperation | COMPLETE_WITH_KNOWN_LIMITATION | [Daily-use evidence](cooperation-daily-use.md); cross-model `NOT_RUN`. |
| J Institution Runtime | COMPLETE_WITH_KNOWN_LIMITATION | [Institution evidence](open-institution-runtime.md); no Institution Evolution claim. |
| K Principal / Authority | COMPLETE_WITH_KNOWN_LIMITATION | Worker draft/claim versus Operator proof/final authority; local trusted-host boundary. |
| L Autonomous multi-worker work | COMPLETE_WITH_KNOWN_LIMITATION | [Earlier dogfood](autonomous-society-audit.md) plus two genuine self-selecting AI Workers here; same-host only. |
| M Execution Contract | COMPLETE | [Contract](execution-contract.md); live DENIED→PASS without bypass. |
| N General Work Kernel | COMPLETE_WITH_KNOWN_LIMITATION | GENERAL ChildWork, eight finished/integrated claims, artifacts; external actions remain host-owned. |
| O Planning Domain | COMPLETE | Structured Goal, Workflow and Plan v1/v2; Observation addressed by accepted PlanDelta. |
| P Assistant Loop | COMPLETE_WITH_KNOWN_LIMITATION | Real AI Host used `assistant.status` and truthful completion gate; Route itself is not an LLM runtime. |
| Q Restart / continuity | COMPLETE | New processes read Plan, claims, artifact hashes, Outcome and final Goal without chat history. |
| R Documentation / onboarding | COMPLETE | Copyable PowerShell path executed verbatim; docs checker PASS. |
| S Fresh project usability | COMPLETE_WITH_KNOWN_LIMITATION | Fresh local project passed Goal→Plan→Worker progress and restart; optional external archive init skipped in sandbox. |
| T Release/main readiness | PARTIAL | `main...fruit` has 5/15 side commits; main worktree is dirty. Public version metadata and package validation remain release-gate work. |

## What remains—and what does not

No unresolved P0 defect blocks the bounded same-host v1 promise. `fruit` is a viable **v1 candidate** after the documentation-only closure commit is safely synchronized. Stop expanding features; the next work is release governance, not more assistant architecture.

Accepted v1 limitations: `CROSS_MODEL=NOT_RUN`; no cross-host authentication; no same-OS-user hostile-process isolation; no arbitrary external-action certification; full Constraint projection may remain incomplete; endpoint/workspace fingerprints do not claim filesystem snapshot isolation or ABA impossibility; `route-pyo3` is outside the documented non-PyO3 workspace test gate; ignored subprocess fixtures and optional resource probes are not counted as passing tests. Eight current Rust warnings are existing unused-variable/unused-`mut` test-code warnings, not a failed gate. The sandbox's `canonicalize` denial required the actual final artifact/proof operations to run outside that sandbox; a read-only probe verified files and paths, and the canonical gate passed. This is an environment boundary, not evidence of file corruption.

Future-only scope: cross-model/cross-host deployment, Parliament, Market, Reputation, Institution Evolution, PackStore, cloud service, GUI, mobile and voice. None is needed to complete this v1 boundary.

`MAIN_MERGE_READY = NO`: the separate `main` worktree has existing uncommitted files and divergent commits; no merge was attempted. `TAG_READY = NO` and `RELEASE_READY = NO`: the [changelog](../CHANGELOG.md) defers version-metadata alignment to a release gate, and packaging/public claim verification has not been performed. The Human's next decision is whether to authorize a separate main reconciliation and release-gate batch. This audit creates no tag or Release and does not grant that authorization.
