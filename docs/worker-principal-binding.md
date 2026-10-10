# Worker Principal Binding I

Implementation audit, baseline `6d7697089077cf224f15e72bab63605aeab9b959`.
This phase addresses supported-interface identity confusion, not hostile
same-OS-user processes, root/admin compromise or remote authentication.

The actor audit and dated acceptance results below preserve the original phase
baseline. The daily-operation and authority sections describe the current engine,
including the bounded work lifecycle added after that baseline.

## Actor audit before implementation

| Existing entry | Field meaning | Required boundary |
|---|---|---|
| worker.register / Worker metadata | worker_id is subject | Explicit Operator operation; registration is not authentication |
| worker.presence.update | worker_id is actor | Derived Worker caller; redundant payload ID must match |
| worker.message.send | worker_id/from_worker is actor; target_worker is subject | Derive sender, retain recipient |
| development.event.record | actor_worker_id is actor | Derive Worker; restrict externally writable event families |
| cooperation register/refresh/knowledge | actor_worker_id is actor | Worker denied unless explicitly supported; Operator cannot claim a Worker actor |
| institution.activate/deactivate | activated_by/actor labels are actor attribution | Operator-only; label is not a grant or Human authentication |
| institution.invoke | requesting_actor is a Worker reference, not authentication | Operator-only; event remains institution-attributed, never Worker-authored |
| institution registration/effects | institution_id/version is subject/provenance | Existing immutable binding and dedicated domain ingress only |
| task/intent/session/evidence/checkpoint/recovery | Administrative/domain operations | Not inherited by Worker binding |
| raw library entry points | In-process trusted host/internal API | Not authentication; untrusted hosts must use CallerContext boundary |
| future work claim/release | Not implemented | Must accept CallerContext and derive actor when added |

## Selected mechanism

Trusted host generates a 256-bit OS-random credential and keeps it outside shared
project history. Operator binds only its SHA-256 verifier to a Worker and project.
Binding issuance/revocation use the existing canonical event ledger, atomic lock,
revision and idempotency path; no new receipt database. Status output never
returns credentials. Reissue/rotation uses a new binding; revocation is explicit,
not wall-clock expiry. Host supplies credential through inherited environment.

CallerContext constructors distinguish explicit trusted Operator from a verified
Worker. Request fields cannot construct either context or request System or
Institution authority. The fixed internal institution runtime keeps its existing
dedicated provenance path; no external institution credentials are introduced.

Worker authority is a closed allowlist: own messages, presence, bounded findings
and bounded work actions plus public reads. Administrative and institution mutations are
denied. The trusted OS/host must not give an untrusted Worker an unrestricted
Operator launch path; this phase does not establish OS-level isolation.

## Host bootstrap and daily operation

The trusted host owns Worker launch, registration and binding. Worker identity is
not its model, provider, process PID or ExecutionSession. Registration creates a
subject descriptor, not an authenticated actor. Rebinding the same Worker with a
new credential preserves its descriptor and history; revoke old bindings explicitly.

From the initialized project directory, the host can register/select a Worker:

```powershell
'{"protocol":"route/1","request_id":"register-a","method":"worker.register","context":{},"params":{"worker_id":"worker-a"},"idempotency_key":"register-worker-a"}' | route rpc --operator
route worker-binding issue worker-a --credential-file C:\HostPrivate\worker-a.credential --operation-key bind-worker-a
route worker-binding list
```

The host creates the private parent directory beforehand. The credential file must
be outside the shared project; issuance uses create-new, flushes it before binding,
and reuses it on retry rather than overwriting it. Protect the file with host/OS
permissions; do not commit it or copy it into a shared project, logs or prompts.
Windows ACL isolation is not established by this command. An unsuccessful or
uncertain issue leaves the file intact for diagnosis/retry with the same key.

The host supplies `ROUTE_WORKER_CREDENTIAL` only to its Worker Route subprocess.
The following short PowerShell demonstration temporarily sets it and restores the
host's previous value; a production launcher should set the child environment:

```powershell
$previousCredential = $env:ROUTE_WORKER_CREDENTIAL
try {
    $env:ROUTE_WORKER_CREDENTIAL = [IO.File]::ReadAllText('C:\HostPrivate\worker-a.credential')
    '{"protocol":"route/1","request_id":"send-a","method":"worker.message.send","context":{},"params":{"message_type":"NOTICE","content":"Ready"},"idempotency_key":"worker-a-ready"}' | route rpc
} finally {
    $env:ROUTE_WORKER_CREDENTIAL = $previousCredential
}
```

The Worker omits its actor; `target_worker` may identify another registered Worker.
`worker_id`, `from_worker` and `actor_worker_id`, when supplied as actor assertions,
must match the resolved principal. Binding mutation and registration require the
Operator interface. Even `rpc --operator` cannot override an inherited Worker
credential. Direct administrative CLI commands reject a Worker credential.
Use `route worker-binding revoke BINDING_ID --operation-key revoke-binding-id`
from the trusted host, without a Worker environment. Revocation is rechecked for
every JSONL request, before receipt replay, and under the canonical append lock.

For autonomous work, read `work.available` together with
`development.events.query` and its `after_revision` cursor before choosing scope.
Use `work.create_child` under an active Intent when a bounded gap is not already
represented, then `work.claim` before editing. Check existing scope and overlap;
coordinate through `worker.message.send` when another claim intersects your work.
Release work with `work.release`, record an observed interruption with
`work.interrupt`, or complete your own claim with `work.finish` after contributing
and verifying the result. A timeout does not automatically release a claim.

Completion is not integration: `work.integrate` is Operator-only and requires
completed accepted claims, the current state hash and revision, and System
verification evidence. Worker completion does not authorize closing the parent
Intent as success. Work claims confer no filesystem, Git or release authority.
See the [Route Cooperation Protocol](route-cooperation-protocol.md#shared-development-semantics)
for the shared ledger and integration boundary.

## Authority, provenance and compatibility

- Default grants: `worker.message.send`, `worker.presence.update`,
  `development.event.record` (Finding only; not Evidence), `work.create_child`,
  `work.claim`, `work.release`, `work.interrupt`, and `work.finish`.
  These defaults apply to newly issued bindings; existing bindings retain their
  recorded grants. `work.integrate` is not a Worker grant.
- Optional explicit grant: `cooperation.knowledge.record`, retaining existing
  knowledge/evidence validation. Use repeatable `--grant` values to replace defaults.
  No wildcard, administration or institution grant is accepted.
- `CallerContext` is not deserializable. Worker construction requires validating
  the credential against the project ledger; a retained context is revalidated on
  mutation. `trusted_operator()` is for trusted embedding code only.
- Supported untrusted ingress is `route rpc` or the typed `principal::worker_action`
  domain boundary. Legacy low-level Rust APIs are trusted in-process/internal APIs,
  not authentication endpoints or isolation from arbitrary native code.
- Event Worker actor is derived; `caller:worker:WORKER:BINDING` is added by the
  boundary. Caller-prefixed payload source/evidence references are reserved and
  rejected. Operator registration/binding events use `caller:operator` and no Worker
  actor. Institution activation/deactivation labels are normalized to
  `local-operator`; institution transactions retain package/version binding.
- No external Institution or System credentials/constructors exist. Institution
  effects stay typed effects, not Worker actions or Operator grants. Existing
  trusted internal operations are not relabeled as externally authenticated System.
- Bare `route rpc` is read-only without a binding; existing trusted administrative
  hosts must opt into `--operator`. This is an intentional compatibility change.
  Request JSON cannot select Operator, System, Institution or caller scope.
- New operation keys are scoped to the principal/binding. Same key and unchanged
  input replays; changed input conflicts. Worker message IDs are deterministic for
  keyed recovery and timestamps do not make retries new operations. Binding,
  message and presence writes recover from PENDING receipts using the ledger key.
- A pre-binding unscoped receipt produces `LEGACY_IDEMPOTENCY_RECOVERY_REQUIRED`.
  Inspect its outcome and reconcile explicitly before choosing a new operation key;
  do not blindly rename/retry uncertain operations. Historical receipts/events are
  not rewritten or silently executed again.
- Only the SHA-256 verifier is stored in binding events; normal binding list omits
  it. Credential environment is not serialized. Requests containing the current
  credential are rejected before echo/persistence. This is not a general-purpose
  secret detector for arbitrary third-party secrets in user-supplied prose.
- Capabilities `worker.binding.list`, `.issue`, `.revoke` come from real dispatch;
  availability is not a grant. Reads expose shared project metadata, not secrets.

## Security limit and verification

In scope: supported-interface actor spoofing, Worker/Operator/Institution confusion,
revocation, project scoping, keyed replay and forged attribution. Out of scope:
same-OS-user malicious processes able to steal local files/environment/memory,
administrator/root/kernel compromise, hostile network authentication and multi-user
OS isolation. The host must restrict launch/environment access itself. Future
sandbox, separate OS users or authenticated remote transport are not implemented.

Executable coverage: `principal_e2e.rs` tests real process attack cases A–J,
cross-process/restart/rotation, concurrent issue, changed-parameter conflicts,
PENDING binding/message recovery, JSONL revocation, host bootstrap, legacy receipt
preservation and credential non-disclosure. `principal::tests` also checks retained
library contexts after revocation, foreign projects, grants and malformed parameters.

## Acceptance results (2026-09-23)

`ROUTE_WORKER_PRINCIPAL_BINDING_I: PASS` for the trusted-local boundary above.

| Gate | Actual result |
|---|---|
| A / B | Foreign sender and foreign Presence actor rejected |
| C / D | Omitted actor derives A; A can address B without becoming B |
| E / F / G | Operator action, revoked binding (including cached replay), foreign project denied |
| H / I / J | Worker-as-Institution, Institution operation with Worker actor, external System/Operator selection denied |
| Restart | Independent processes retain the binding; rebind retains the Worker identity/history |
| Idempotency | Same-key replay, changed-parameter conflict, concurrent issue winner, binding/message PENDING recovery pass |
| JSONL | Persistent process reauthenticates after external revocation; forged context rejected |
| Secret handling | Bootstrap emits no raw credential; project files/receipts/history scan contains none; current-token reflection rejected |
| Library | 2 new principal tests, including retained revoked context and project/grant checks |
| route-core | 22 unit + 1 doc test passed |
| route-basic | 403 passed; 2 existing subprocess fixtures ignored as direct tests |
| route-cli | 70 passed, including 5 principal process tests |
| Workspace, excluding route-pyo3 | 617 passed, 2 existing fixture ignores; exit 0 |
| Formatting/docs/PowerShell/diff | PASS; all 4 repository PowerShell scripts parse |

Tests use disposable local projects and real Route binaries, not autonomous AI
agents. Existing unrelated test warnings remain; they are not new feature claims.
At that phase's acceptance, the prior identity blocker was CLOSED and autonomous
A/B/C remained NOT_RUN; no claim, assignment or autonomous integration features
were added in that phase. This historical result is not the current work-surface
status; see the [autonomous society audit](autonomous-society-audit.md).
