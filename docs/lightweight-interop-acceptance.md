# Lightweight Interop I — acceptance record

ROUTE_LIGHTWEIGHT_INTEROP_I: **PASS**, within the explicitly tested scope below.

Scope: optional local work sidecar, not a replacement terminal or IDE. See
[the contract](lightweight-interop.md) and [repeatable host prompts](../examples/lightweight-interop/README.md).

| Surface | Readiness |
| --- | --- |
| LightweightAttach | DAILY_USE_VERIFIED |
| ZeroConfigGoldenPath | DAILY_USE_VERIFIED |
| GracefulAbsence | DAILY_USE_VERIFIED |
| ExternalWorkerAdapter | DAILY_USE_VERIFIED |
| BoundedHandoff | DAILY_USE_VERIFIED |
| HostIntegration | DAILY_USE_VERIFIED (Codex CLI only) |
| CrossHostHandoff | NOT_RUN |
| LowIntrusionSafety | DAILY_USE_VERIFIED (trusted-local scope) |

## Actual terminal use

Codex CLI 0.162.0-alpha.2, configured model `gpt-6.1-sol`, executed two independent
ephemeral workspace-write sessions in an existing Git test project. This is
same-host fresh-session continuity, not cross-model or cross-terminal certification.

Session A `01a12580-286b-7192-8777-54882f4e95e0` discovered the project, claimed
work, inspected actual Rust source, published a missing UTF-8 boundary-test
finding, and interrupted its claim with a persisted stop reason. Session B
`01a12585-440a-7412-a197-619b5a44c701`, under a separately bound reviewer identity,
received no A transcript or finding in its prompt. It read Route, resumed the
predecessor claim, added a failing Unicode test, reproduced the panic, fixed the
boundary handling, passed two tests, published its result and released the claim.
Shared ledger revisions 13/14 contain A's finding/interruption; 17/18/19 contain
B's resume/result/release. Human context-copy count: **0**. Raw chat and raw CoT
were not required or stored in Route. Host diagnostic streams remain local ignored
test artifacts, not shared Route work records.

The final Goal remains **DENIED**, correctly: a Worker reporting successful tests
does not create System Evidence or satisfy missing structured completion obligations.
The demo tests handoff, not a shortcut to project completion.

The first Windows default-sandbox attempt failed with `helper_unknown_error`;
its exit 0 was not accepted as a successful integration. The supported
invocation-only `-c windows.sandbox=mxc` enabled actual local execution, without
saved configuration changes or bypassing the sandbox. Real use exposed and fixed
restricted Windows path canonicalization and virtualized environment discovery.
WebSocket retries/HTTPS fallback made AI startup take minutes. Only local attach
and record inspection are sub-minute; the full AI cycle is not advertised as such.

Gemini CLI is installed but has no configured authentication. SECOND_HOST and
CROSS_MODEL are NOT_RUN. No credentials or account configuration were changed.
Claude/Cursor integration was not certified. These readiness labels describe
this bounded local scenario, not broad production or hostile multi-user certification.

## Absence and recovery

After real use, both local state directories were moved recoverably out of active
discovery. Shared lookup failed, but Git status, ordinary source editing and
`rustc --test` still worked (two tests passed). An offline file was created.
Both directories were restored. Route recovered revision 19 unchanged and did
not invent a record for the offline edit. No user source or history was deleted.

## Measurements

Final local release build succeeded (6m 58s, exit 0). Executable: **27,398,144
bytes**, SHA256 `7D2E689E6F7B6C81D2A3E487234FA73EE06AB20762065AD2580517463A9AA36A`.
An ignored EXE-only measurement ZIP is **10,069,034 bytes**, not a published or
complete distribution package. The existing public v1.0.0 ZIP was not replaced.
Release attach: **171.91 ms**; identity/status p50/p95: **46.06/61.56 ms**;
handoff p50/p95: **46.63/76.89 ms**. Sampled transient CLI peak working set:
**12,042,240 bytes**. Idle and 100-read durable growth: **0 bytes**, with exact
unchanged file manifests. Attach commands: **1**; manual Route fields: **0**.
Source installation/build and AI startup are not included in attach latency.
The populated real-session handoff also passed using this final release binary:
2,931 UTF-8 bytes, 46.89 ms, revision 19, Goal DENIED, identical state hashes.

`scripts/measure-lightweight-interop.ps1` creates a fresh small project and
separate credential directory. It retains the reported temporary fixture,
samples child-process peak working set, and compares exact file hashes across
idle and 100 reads (50 identity/status + 50 shared handoff).

Debug measurement: binary 42,790,912 bytes; attach 207.06 ms; status p50/p95
93.28/139.56 ms; handoff p50/p95 93.31/138.39 ms; observed transient peak working
set 23,126,016 bytes. Idle growth and 100-read durable growth: **0 bytes**, with
identical file manifests. No persistent background process is installed. Status
here is `sidecar whoami`, not legacy archive-scanning `route status`.
Populated real-session handoff: 2,931 UTF-8 bytes, measured read 80.12 ms.
These are small-fixture Windows observations, not large-ledger latency promises.

Each list is capped at 8, each text at 256 characters plus ellipsis, and the
whole JSON at 65,536 bytes. Output is bounded; ledger replay cost is not indexed
or bounded by this phase. No public release is created by this phase.

## Regression and boundaries

Latest-code regression PASS (exit 0): fmt; route-core, route-basic and route-cli
tests; workspace tests excluding route-pyo3; docs links; PowerShell parser; diff
whitespace. Workspace retained four ignored basic tests and one ignored subprocess
fixture; none failed. No Rust source changed after this final regression.
Focused subprocess tests cover immutable reads, repeated attach, absence/restart,
bounded Unicode, route/1 parity, credential spoofing/escalation, foreign projects,
nested Git discovery and non-execution of Markdown. Existing ignored fixture tests
remain ignored, not silently reported as passes.

Credentials live outside the project; none were found in the actual shared
ledger or ROUTE.md. Common secret patterns are filtered but this is not a general
secret detector. Same-OS-user credential access is a trusted-local boundary,
not hostile-user isolation. Read data is untrusted; attachment grants no shell
execution or Operator role. Existing explicit Evidence workflows remain necessary.

The published v1.0.0 candidate `71cc210db804850fe06ed3719e7f6c80d95d34b4`
remains preserved by its existing tag. Phase base fruit was
`337b80d4857a948579a249df76776439d7ddbf6a`. New commands are development-fruit
capabilities, not claims about the old published binary. Advanced functionality
and the original detailed guide are retained. No main, tag, public Release or
Yuich change is part of this phase; unrelated pre-existing deletions are excluded.
