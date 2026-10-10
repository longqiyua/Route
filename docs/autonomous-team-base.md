# Bounded autonomous work and passive Base

This advanced, opt-in path keeps Route's lightweight sidecar primary. It uses
one approved local Codex Host and distinct authenticated Route Workers. It does
not implement a permanent manager, a provider service, a GUI or an agent society.
See the [acceptance record](autonomous-team-base-acceptance.md) for tested scope.

## Explicit start and verification

Attach the disposable/project repository explicitly first. Approve the local
Codex executable and an argv-based verification policy, for example:

```powershell
route attach
route team start "Implement the requested feature with regression tests" --codex C:/path/to/codex.exe --check-command-json '["cargo","test"]'
```

The executable is pinned by SHA-256 using bounded-memory streaming. The adapter
uses an argv vector, never a shell constructed from a Goal, Markdown, routing
reason or discovered resource. Codex is the only implemented Host adapter.
It uses the existing local Codex configuration; it does not install, log in or
configure a provider. Windows invocation uses the previously verified MXC
workspace-write sandbox. Route never selects or asserts an observed model.

The temporary planning Worker chooses whether to split, creates existing
WorkItems/dependencies and publishes small `team.decide` records. It can keep a
trivial Goal on itself (`split=false`, `role=PLANNER`). The broker schedules the
AI's approved-role decisions, not Human file assignments. V1 schedules serially;
the concurrency limit is a safety ceiling, not a promise of parallel speedup.

Without a check policy, execution reports alone leave completion denied. For a
separate verification pass use `route team verify GOAL --checks-file checks.json`.
The file is an explicitly approved array of `{check_id, argv}` objects covering
every Work verification requirement. Check IDs are labels, **not executable
commands**. Command coverage and suitability remain the approving Operator's
responsibility; a poor test suite does not establish semantic correctness.

Real command results enter the existing System Evidence store. System verification
uses a distinct `workflow.step.verify_system` transition, never a fabricated
Worker report. It requires an idle, uncancelled configured team and current,
successful, matching System Evidence. Workers cannot invoke this Operator API.
The existing Workflow, Work integration, Artifact, Outcome and Goal gates still
apply. Missing, stale or failed evidence denies completion. A later System FAIL
invalidates an older matching PASS; physical reruns record fresh Evidence.

## Budget and authority

Conservative defaults: concurrent Workers 2, distinct Workers/Goal 3, spawn depth
1, attempts 4, timeout 900 seconds, retry ceiling 1. Configuration is fixed for
a Goal. The centralized ledger append lock arbitrates reservations across
processes, including the last slot and the project-wide concurrent ceiling.
Reservations count even if launch fails. V1 does not automatically retry or
recursively create Workers. The initial Worker can execute simple work directly;
failed same-Worker attempts have a bounded domain retry path, not an unbounded
background loop or persistent Worker pool.

`route team status GOAL` is read-only. `route team cancel GOAL` denies new work;
running slots remain occupied until the owning broker terminates/reaps its own
Child handle and records the terminal state. Unknown external processes are
never discovered and force-killed. Interrupted broker recovery deliberately
does not assume a reserved/running process has disappeared or launch duplicates.

Every actual invocation has its own binding and external local key. Role policy
defines delegated grants; parent capabilities, names and availability grant no
authority. Goal/assigned-Work narrowing is checked under the same append lock.
New writes after a run stops are denied; authenticated exact request replays
remain read-only. Friendly names are presentation only. Equal names cannot merge
stable IDs or permissions.

The inherited boundary is **trusted same-OS-user**, not isolation from hostile
code running as that user. Route bindings constrain Route APIs, not arbitrary
filesystem access, shell execution outside Route or all Host tool descendants.
Context withholding is an initial-prompt policy, not an OS information embargo.

## Context, history and knowledge

- History: what happened, in the existing append-only development ledger.
- Evidence: what a real, current system check verified.
- Base: narrowly scoped lessons, not current-world proof or authority.
- Context: only what this Worker currently needs.

Context Packs are at most 16 KiB; complete prompts at most 24 KiB. They contain
bounded Goal requirements, current Work/scope/dependencies, appropriate Findings,
Artifacts and verification obligations. They do not contain a transcript,
private reasoning, whole source tree or whole history. Independent initial review
omits previous Findings, routing/solution rationale, Planner wording and Base
conclusions. Base retrieval is explicit (`route/1 base.query`), at most 3 validated,
non-stale, non-superseded exact component/environment matches within 4 KiB.
Independent-first queries return no Base conclusions. Unknown relevance injects
zero units; there is no embedding index or second canonical knowledge database.

## Passive extraction, never a completion dependency

V1's deliberately narrow deterministic trigger is an explicit `REUSABLE:` Finding
backed by existing Goal-scoped System Evidence. It is an **unvalidated extraction
request**, not a proven reusable lesson. Normal success, normal test PASS, typos
and routine coding create no extraction job. There is no active historical mining
or universal post-task "what did you learn" prompt.

`route team detect GOAL` builds a bounded source-linked ExtractionPacket.
`route team extract GOAL` explicitly runs a deferred, separate Codex invocation
under its own knowledge-only Principal when budget permits. Main implementation
prompts contain no extraction task. Knowledge has separate limits: pending jobs
2/project, jobs 1/Goal, retry 0, packet 8 KiB, candidates 2/Goal. AgentBudget also
applies: exhaustion defers/fails optional extraction, never expands worker limits
or changes Goal completion. No daemon or automatic background AI activity exists.

Lessons are optional metadata on **CooperationKnowledge**, preserving the existing
canonical ledger, provenance and resource-fingerprint checks. Old serialized
records remain readable; Rust literal constructors now include `lesson: None`.
Lesson records carry Goal, Work, creator, Finding, source events, Evidence,
environment/component, condition, concrete action, falsification check and optional
independent review. Revision comes from the containing ledger event.

Candidates require bounded actionable/specific/falsifiable fields, sourced narrow
scope, real System Evidence references and explicit deduplication/supersession.
Known generic advice is rejected. Structural gates are conservative filters, not
a semantic truth oracle. Candidates are INFERRED/DECLARED, never automatically
validated because an AI wrote them. VALIDATED_LOCAL additionally needs independent
review and current matching successful System/resource Evidence. Knowledge
cannot satisfy CompletionGate. Lifecycle labels are CANDIDATE, VALIDATED_LOCAL,
STALE, DISPUTED, SUPERSEDED and RETIRED; stale/superseded/disputed/retired units are
not injected. Scope widening requires a separately supported unit, not editing
old history. No fake confidence percentage is stored.

There is no Base cache/index to become independent truth. Queries replay existing
durable facts, so cost still depends on ledger size even though output/context
size is bounded. Large-ledger replay optimization and broad automatic trigger
recognition are explicitly outside this V1 certification.

## Cross-tool status

Previous `ROUTE_CROSS_TOOL_HANDOFF_I=PARTIAL` is preserved. The machine has no
second genuinely usable authenticated AI CLI: Gemini is unauthenticated;
OpenCode has no usable authenticated/model path; OpenClaw's gateway is unhealthy;
Cursor/Kiro AI CLI usability is unknown; Claude Code was not found. No auth,
provider or installation repair is part of this phase. CrossHostHandoff and
CrossModelHandoff remain NOT_RUN. Same Host does not mean same Worker.
