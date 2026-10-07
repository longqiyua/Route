# General Work & Assistant I

Route's general-work loop is a local, event-sourced planning and work-tracking
surface. It reuses the global DevelopmentEvent ledger, authenticated Principal
bindings, ChildWork claims, System Evidence, and the versioned Execution
Contract. It does not create another authoritative store. The older `route
goal` project-memory list and analytical `plan.rs` preview remain readable;
neither is silently converted into execution truth.

## Local route/1 flow

1. Initialize a project with `route init`. An Operator calls `goal.create`
   with `expected_revision` and a Goal domain (`PLANNING`, `RESEARCH`,
   `GENERAL`, or `DEVELOPMENT`). The returned ledger event supplies `goal_id`.
2. The Operator calls `workflow.create` for that active Goal, using the Goal ID
   as `intent_ref`. The Workflow has required, conditional, or optional steps,
   dependencies, and named proof obligations. The Operator calls `plan.create`
   with matching `goal_id`, `workflow_id`, and `workflow_version` plus
   assumptions, unknowns, constraints, milestones, risks, and review
   conditions. These are bounded, structured records, not hidden reasoning.
3. Bound Workers discover `work.available`, create `GENERAL` ChildWork under
   the Goal with `goal_id` and optional `workflow_step_id`, self-claim, and
   `work.finish`. Unlike development Work, General Work needs no Git path,
   branch, compiler, or test command. A Worker may record an Observation,
   request a Decision, register a `DRAFT` file Artifact, or record a
   `DRAFT`/`PARTIAL` Outcome. Workers cannot answer Decisions, accept PlanDelta,
   register `FINAL` Artifacts/Outcomes, or close Goals.
   Optional `required_capabilities` on ChildWork are checked against the
   Operator-registered Worker descriptor at claim time; descriptor claims are
   routing metadata, not independently verified competence.
4. An Operator answers pending Decisions. A changed assumption is recorded as
   an Observation with affected step/work IDs. A Worker proposes a Workflow
   PlanDelta, and an Operator accepts or rejects it. An accepted change makes
   Workflow v2 immutable while v1 remains queryable. The Operator then records
   Plan v2 with the accepted `source_delta_id` and addressed Observation IDs.
   Until then, `plan.review` reports the unaddressed or stale-plan condition.
5. Workers register draft Artifacts by relative path and SHA-256. The Operator
   calls `plan.verify_step` only after associated claims are complete and a
   current Artifact or answered Decision exists for that step. Route records
   System CheckPass Evidence with the current project fingerprint and a causal
   revision. This is a **structural** check, not a claim that Route understands
   the business quality of a planning document. A Worker uses that evidence
   ref in `workflow.step.complete`. The Operator integrates all accepted Work
   claims with fresh System Evidence. The Workflow completion gate still
   requires every required/active conditional step and valid dependencies.
6. A Worker calls `workflow.complete.request`. Only after `plan.review` returns
   `PASS` may the Operator register a `FINAL` Artifact and `FINAL` Outcome and
   close the Goal as `SUCCEEDED`. A missing required step, stale fingerprint,
   pending Decision, unaddressed Observation, missing integration, or absent
   final Outcome denies success. `FAILED` and `CANCELLED` remain explicit
   Operator outcomes rather than false success.

All ledger-backed mutations require an idempotency key and the applicable
`expected_revision`. Retry the same request/key after an uncertain response;
do not mint a new key. `plan.verify_step` reconciles an interrupted write of
System Evidence before its corresponding ledger event. The authoritative
read-only endpoints include `goal.list/get/status`, `plan.get/review`,
`work.list/get`, `decision.list/get`, `artifact.list/get`, `outcome.get`,
`observation.list`, and `assistant.status`. `route assistant status [GOAL_ID]`
prints the current obligation count and next actions; `--json` emits the
machine view, and `route assistant review GOAL_ID` emits the completion report.
Read calls do not append to the ledger.

## Trust and limits

The route/1 `--operator` flag is a trusted local-host administrative surface;
never pass it to an untrusted Worker. Worker credentials are scoped to one
project, checked on every mutation, and revocable. Authenticated Workers can
report observations and draft output, not manufacture System proof or final
authority. Artifact paths must stay inside the project; bytes are hashed and
rechecked. A project file change after proof makes the fingerprint stale.

Route does not provide autonomous arbitrary external actions, remote-host
identity isolation, cross-model coordination guarantees, semantic verification
of research conclusions, or a natural-language model runtime in this phase.
The local assistant status is a truthful projection of the machine state, not
a synthetic claim of completion. The scripted two-Worker 30-day product-launch
plan in `tool/route/crates/route-cli/tests/general_work_e2e.rs` exercises a
supplier Day 10→Day 17 change, human budget decision, PlanDelta v1→v2,
required-step denial, restart, proof, integration, and final closure.
