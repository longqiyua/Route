# Execution Contract I

Route distinguishes a Worker's report from satisfaction of an accepted plan.
AI may propose, execute, and report; Route decides whether the accepted contract
is actually satisfied. This is a bounded local execution contract, not a new
workflow engine. The older `workflow` Reference subtype remains descriptive.

An Operator creates a versioned `WorkflowSpec` under an active DevelopmentIntent.
Each step has a requirement, ordered dependencies, and a `check_id` proof
obligation. Accepted versions are immutable DevelopmentEvents. A Worker can
start, report, fail, conditionally skip, propose a replacement plan, or request
completion, but cannot rewrite a version. An Operator may accept/reject a
`PlanDelta`; acceptance appends a new version and leaves earlier versions and
the proposal queryable. An Operator waiver is recorded as a skip event with
reason and principal provenance. A Worker skip is valid only when the declared
`PATH_EXISTS` condition is observed false. Other skip attempts remain visible
as `BYPASSED` and block completion.

The only qualifying proof for a reported step is a canonical System `CheckPass`
Evidence from the same Intent, with the step's `check_id`, exit code 0, a stable
tested world, a check revision no earlier than the accepted version, and a
fingerprint matching the current project state. A claim or an arbitrary Evidence
reference does not qualify. A later state change leaves the old Evidence intact
but projects its step as `STALE` until a fresh check is run. The whole-project
fingerprint is intentionally conservative: unrelated tracked state changes may
require revalidation.

The domain CompletionGate evaluates every required/conditional step, dependency,
skip, proof, staleness, bypass and existing Work integration boundary. Missing
steps produce `DENIED` with actionable arrays. A passing gate appends a
`CompletionEvaluated` event. A later step report or state change invalidates the
current `COMPLETE` projection; it does not erase history. Successful Intent
session closure also consults this gate. `CONTROLLED` and `FULL_POWER` both
enforce the same completeness rules; full power is not unlimited authority,
retries, or reasoning budget.

The `route/1` methods are `workflow.list`, `workflow.get`, `workflow.status`,
`workflow.complete.check`, `workflow.create`, `workflow.step.start`,
`workflow.step.complete`, `workflow.step.fail`, `workflow.step.skip`,
`workflow.plan_delta.propose`, `workflow.plan_delta.accept`,
`workflow.plan_delta.reject`, and `workflow.complete.request`. Mutations require
a principal, an expected global revision and a durable idempotency key. The
human inspection command is `route workflow status [ID]`.

The executable anti-shortcut tests use a disposable real Route project and
processes: A/B/D proof with C missing is denied; claim-only C is denied;
qualifying C allows completion; a subsequent project mutation makes the proof
stale; fresh checks allow completion again. Library tests cover version
immutability, generic-event rejection, condition/waiver authority and revision
races. This does not establish cross-model operation, remote-host security, or
protection from hostile processes with unrestricted access to the same local
files. `CROSS_MODEL` remains `NOT_RUN`.
