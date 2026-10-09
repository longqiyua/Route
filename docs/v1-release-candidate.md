# Route AI v1 release candidate (fruit)

This is a **local Windows CLI candidate**, not a tagged or published release.
Route is a local-first record of project work, evidence, and recovery alongside
Git; it does not replace Git or perform the work for an AI host. Its bounded
assistant promise is: a Human supplies a bounded Goal; Route persists a
structured Workflow and versioned Plan, authenticated Worker work and
evidence, restart continuity, and a truthful Outcome/completion state.

## Install and start

Extract the candidate ZIP into a directory you control. Keep `route.exe`,
`LICENSE`, `THIRD_PARTY_NOTICES.md`, and this document together. On Windows,
open PowerShell in an existing project and run the executable by its absolute
path (or put its directory on `PATH`):

```powershell
& 'C:\path\to\route.exe' --version
& 'C:\path\to\route.exe' --help
& 'C:\path\to\route.exe' init
& 'C:\path\to\route.exe' status
```

`init` creates local Route state in the chosen project. Do not run it from a
directory that you do not intend to initialize. The candidate binary should
work after its folder is moved; it must not depend on a developer checkout.
Keep backups of the project and its `.route` / `.route-basic` state. Do not
manually edit those event stores to repair an interrupted operation.

## First Goal and restart

The [first-Goal PowerShell example](general-work-assistant.md#first-goal-on-a-fresh-project-powershell)
shows the concrete `route/1` `goal.create` → `workflow.create` → `plan.create`
sequence. It uses `route rpc --operator` only for trusted local bootstrap,
registers/binds a Worker, then shows `plan.get` and
`route assistant status GOAL_ID`. Save the returned Goal ID; reopening the
same project and repeating those read commands resumes from persisted state.
`workflow.complete.check` must deny an incomplete required step. A finished
Worker claim alone is not proof of completion; System verification and the
Execution Contract still gate the final Outcome. See
[Work Integration & Verification](work-integration-verification.md) and
[Worker Principal Binding](worker-principal-binding.md).

## Trust boundary and limits

The local Operator is trusted. Keep Worker credentials private, give each
Worker its own binding, and do not expose `--operator` to an untrusted Worker.
The binding is not an isolation boundary against a hostile process running as
the same OS user. Route records structural evidence and completion; it does
not certify the semantic quality of a document or arbitrary external action.
Cross-model operation has not been certified. Cross-host authentication,
cloud-distributed security, arbitrary external-action certification, and a
general autonomous-company/AGI claim are outside this candidate. Tauri GUI
and Python binding are not shipped in this Windows CLI package.

The release version is **not settled**: Cargo and `route.exe --version` say
`1.0.0`, while other product/package metadata and documentation say beta or
V0.6 Beta. `route/1` is the protocol version and is independent of the
product version. No public release or compatibility promise follows from this
candidate document; a Human version decision and main reconciliation remain
release gates.
