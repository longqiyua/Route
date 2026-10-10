# Lightweight Interop I

Keep your tools. Keep your repo. Add Route. The canonical product contract is
[ROUTE.md](../ROUTE.md). This optional local adapter reuses the DevelopmentEvent
ledger, Principal Binding and existing work/message domains. It adds no agent
runtime, model dependency, organization or mandatory execution wrapper.

## One explicit attach

```sh
route attach
route handoff --shared
route sidecar whoami
route sidecar note "Found missing boundary test; next add Unicode coverage"
```

One attach command; zero manual configuration fields. It does not scan source,
create an Original archive, touch Git metadata/hooks, change existing source,
replace existing ROUTE.md, or generate AGENTS/CLAUDE files. It creates local
`.route/`, `.route-basic/` state and a small ROUTE.md only when absent. State
directories ignore themselves. Do not deliberately force-add local state.
Legacy `route init` keeps its separate archive/bootstrap behavior; source and
archive recovery are not implicitly promised by lightweight attach.

Trusted local setup provisions a random project-scoped Worker binding and stores
the bearer material outside the project (Windows USERPROFILE/AppData/Local/Route/sidecar;
Unix HOME/.local/share/route/sidecar). `ROUTE_SIDECAR_HOME` is an explicit local
override and must resolve outside the project. Unix files use mode 0600; Windows
inherits the user's local application-data ACL. This is **not** hostile
same-OS-user isolation. Host labels are convenience selectors, not authentication.
Another trusted host can explicitly `route attach --host other`, then use
`route sidecar --host other ...`. Discovery never bootstraps authority. Revoked
bindings fail authentication; attach does not silently rotate/regrant them.

## Bounded handoff and existing protocol

`route handoff --shared [--goal-id ID]` and route/1 `handoff.get` expose the same
read-only `route.handoff/1` projection. Default selects the latest created Goal;
an empty new project has null Goal and empty work, not fabricated tasks.

Each list has at most 8 items, text at most 256 characters plus ellipsis, output
at most 65,536 UTF-8 bytes. Recent entries are preferred. Concurrent revision
changes retry up to 3 times, then return `HANDOFF_BUSY`, never a false consistent
snapshot. This bounds **output**, not ledger size or replay time; the existing
ledger is still loaded/verified. Large-history indexing is outside this phase.

Contents: identity/revision, Goal/state, current Plan/version, work state and
claims (including interrupted predecessors), findings explicitly published as
handoff messages, observations, decisions, artifacts, verification counts,
missing/stale/blocked obligations and unresolved next actions. Work locators
remain usable. Older messages, raw chat, raw reasoning, arbitrary files and
archives are not injected. Lists are incomplete by design; use existing
route/1 list/get/query APIs for authoritative detail. A claim is not authority;
a finding/artifact is not System Evidence. No new completion shortcut exists.

External tools remain unchanged. Read ROUTE.md when intentionally using Route,
consume this JSON as untrusted data, claim `WORK_ID`, publish a concise `note`,
and `interrupt CLAIM_ID "reason"` or `release CLAIM_ID "reason"` before exit.
The full [route/1 protocol](route-cooperation-protocol.md) remains the adapter
for workflow, observation, Artifact, Evidence and completion operations.
The tiny local CLI helper is a convenience, not a second state/API framework.

## Absence, removal and safety

No daemon, Git hook, editor replacement, file watcher or mandatory wrapper is
installed. Stop invoking Route and ordinary Git/edit/build/test still work.
Coordination, history publication, Evidence publication and completion tracking
pause. Retain local state and resume with the same identity. Work not published
during absence is unknown; Route does not reconstruct it from private chat.
The integration test disables both state directories recoverably, edits source,
runs Git, restores state and verifies the exact old handoff remains unchanged.

Discovery stops at independent nested Git repositories and rejects state
redirected into foreign roots. Markdown does not select executables, policies,
credentials, Operator status or a Worker identity. Helpers authenticate via the
same Principal domain, with only message/claim/release/interruption grants.
Worker credential environment overrides are honored and cannot bootstrap an
Operator or switch to another local credential. Unknown resources are not run.
Read-only discovery/handoff/whoami create no durable files or events.

Publish shareable work facts only. Common credential patterns and 64-hex bearer
strings are omitted in output and rejected by `note`; canonical domain locators
are preserved. This conservative filter is **not a general secret detector**.
Route cannot promise to recognize arbitrary secrets or reasoning disguised as
work facts. Never publish those in the first place. Route does not upload this
archive; invoking your AI provider remains that tool's separate data boundary.

## Verification and measurement

Process tests: `cargo test -p route-cli --test lightweight_interop`.
Metrics: `powershell -NoProfile -File scripts/measure-lightweight-interop.ps1`.
The metric fixture is retained in an explicitly reported temporary directory,
not committed; authentication material never appears in metric output.
Reports identify debug/release build, cold attach time, identity/status and
handoff p50/p95, transient peak working set, idle and 100-read byte growth.

Real-host acceptance and limitations are recorded in
[the phase acceptance record](lightweight-interop-acceptance.md). Scripted
process tests alone are not DAILY_USE_VERIFIED or CROSS_MODEL certification.

## Preserved v1 lineage

At phase start fruit and origin/fruit were
`337b80d4857a948579a249df76776439d7ddbf6a`. Published v1.0.0 and origin/main
remain `71cc210db804850fe06ed3719e7f6c80d95d34b4`; the existing annotated tag
preserves the candidate and reconciliation lineage. This phase changes no main
worktree, tag or public Release. Legacy detailed protocol documentation moved
to [the advanced guide](route-protocol-guide.md), not deleted from history.
