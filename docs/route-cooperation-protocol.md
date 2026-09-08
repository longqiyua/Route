# Route Cooperation Protocol — canonical machine interface

`route/1` is Route's host-neutral machine cooperation protocol. It is owned by
Route and is independent of Yuich, MCP, HTTP, a daemon, or a frontend.

## Versioning and transport

Route product version and protocol version are separate compatibility axes.
The current product can be `1.0.0` while supported protocol versions are
`["route/1"]`; a product release does not itself require `route/2`.

The first transport is `route rpc` (one JSON request on stdin, one JSON
response on stdout) and `route rpc --jsonl` (one request/response per line,
sequentially). Stdout contains protocol JSON only. Diagnostics belong to
stderr. This creates no server, port, background process, or authority bypass.

## Envelopes

Every request has mandatory `protocol`, `request_id`, and `method`; `context`
and `params` are objects. Mutating methods require an `idempotency_key`.

```json
{"protocol":"route/1","request_id":"req_01","method":"system.hello","context":{},"params":{"supported_protocols":["route/1"]},"idempotency_key":null}
```

Every response has the same protocol and request id, `ok`, `result`, `error`,
and a receipt. Errors have stable code, human-readable message, retryability,
and non-secret details; Rust/Python stack traces and chain-of-thought never
enter a protocol payload. Unknown top-level fields are rejected in `route/1`
to keep forward compatibility explicit rather than silently ignored.

## Capability and context rules

`system.hello`, `system.capabilities`, and `system.status` describe the
product, selected protocol, compatible versions, feature flags, and only
methods actually connected to Route services. A caller named `yuich` receives
no special behaviour.

Project context accepts `project_id`, `workspace_id`, `scope_id`, and
`root_locator`. Route persists stable `project_id` and distinct per-checkout
`workspace_id` in `.route/project-identity.json`; moving or renaming a
directory preserves the values because they are file-backed, not path-derived.
`route project attach <path>` explicitly shares a project id while retaining a
different workspace id. A supplied `root_locator` must resolve to this
process's discovered project root or the call fails `PROJECT_IDENTITY_CONFLICT`.

## Current route/1 methods

| Capability | Method | Existing Route service |
|---|---|---|
| system inspection | `system.hello`, `system.capabilities`, `system.status` | package metadata + existing status/session state |
| project inspection | `project.inspect`, `project.status` | persisted `.route` identity, git, session state |
| development intent | `intent.list`, `intent.get`, `intent.create`, `intent.close` | existing `ExecutionSession` store and lifecycle |
| evidence | `evidence.record`, `evidence.query` | existing `EvidenceStore` and host-report ingestion; caller evidence is never authoritative |
| history | `history.query` | locked, hash-chained `.route/history` event log |
| checkpoint | `checkpoint.create` | existing `BasicRepository::checkpoint_create` |
| recovery | `recovery.status` | read-only recovery availability/status |
| shared development | `development.state`, `development.events.query`, `development.event.record` | project-scoped append-only `DevelopmentEvent` ledger plus read-only projection of authoritative Route/Git state |
| workers | `worker.list`, `worker.register`, `worker.presence.update`, `worker.message.send` | persistent host/model-neutral worker identity, bounded presence, and project-visible typed messages |

| Reference | `reference.list`, `reference.get`, `reference.register`, `reference.refresh`, `reference.recover` | canonical registry and durable operation journal/CAS/event bridge |
| Cooperation | `cooperation.list`, `cooperation.get`, `cooperation.register`, `cooperation.refresh` | canonical ledger resource operations |
| Shared knowledge | `cooperation.knowledge.query`, `cooperation.knowledge.record` | canonical knowledge projection and unified Evidence validation |

The dispatch definition generates capability discovery; each dedicated adapter
has actual binary transport coverage. See [daily-use commands and evidence](cooperation-daily-use.md).
Read calls do not initialize/rebind identity or append invocation history.
Initialize explicitly with `route init`.

`capability advertised => method actually callable`. Task RPC is intentionally
`NOT_AVAILABLE`: Route has no stable task domain mapping, so no `task.*`
capability is advertised. Mutations reserve their idempotency key in
`.route/rpc-idempotency.json` before invoking the domain operation and commit a
persistent receipt afterward. For domains without safe keyed re-entry, a `PENDING` reservation is fail-closed as
`IDEMPOTENCY_RECOVERY_REQUIRED`, preventing an uncertain retry from creating a
second mutation. Reference and Cooperation dedicated mutations can re-enter their canonical keyed
operations after a PENDING receipt; Reference journal recovery reconciles the
domain/event boundary before replay. Corrupt or unreadable idempotency state
is an explicit error.
The idempotency receipt file is serialized by a cross-process lock. Development
mutations additionally carry the scoped protocol idempotency key into the
project ledger, so a completed retry cannot append the same event twice even
when a different Route process handles it.

## Shared development semantics

`.route/development/ledger.json` is an append-only logical ledger represented
by atomic whole-file commits under a project-local cross-process lock. One
accepted event advances the project-global revision by exactly one. Clients
consume bounded pages with `development.events.query` and
`after_revision`; `development.state` reports whether a supplied
`seen_revision` is stale. A corrupt hash chain or malformed ledger fails
closed.

`Worker` identity is independent of provider, model, host process, execution
session, role, and workspace. Re-registering an existing worker updates its
metadata without replacing its identity. Presence is a last-observed
development projection, not proof of runtime or filesystem truth.

Worker messages are typed (`QUESTION`, `ANSWER`, `NOTICE`, `HELP_REQUEST`,
`HELP_OFFER`, `WARNING`, `PROPOSAL`, `DISAGREEMENT`, `REVIEW_REQUEST`,
`REVIEW_FINDING`, `HANDOFF`) and globally queryable inside the explicitly
shared project. They never enter `EvidenceStore`; a finding, proposal,
agreement, disagreement, or vote cannot promote itself to Evidence. The event
schema accepts bounded operational summaries, not raw chain-of-thought.

## State ownership and compatibility

Route owns Route development intents/sessions, task execution state, evidence,
history, receipts, checkpoints and recovery metadata. A Host owns its own
identity, memory, governance, and unrelated business state. The project/Git
own files and Git objects. The protocol exchanges commands, object refs,
results, and receipts—never mirrored Host databases.

Optional response fields may be added. Existing field semantics and error code
meanings do not change; a breaking semantic change requires a new protocol
major. Receipt is an operation record, not Evidence, though it may later refer
to existing Route evidence/history.

## Distribution and project identity

The CLI is self-contained and can be built/installed from the Route source
checkout (`cargo install --path tool/route/crates/route-cli` from the source
root) and then invoked with `route rpc` from another directory. No Route
source, binary, or database is copied into the managed project. The first
identity-aware inspection creates `.route/project-identity.json`; the file is
portable with the checkout and is therefore stable across rename/move. Each
checkout receives its own `workspace_id`. To intentionally share a project
identity, run `route project attach <existing-project-path>` from the second
checkout; repeated attachment is idempotent and never merges workspaces
silently. A copied identity file is detected by its checkout binding and gets
a fresh workspace id while retaining the project id. Nested working
directories discover the nearest Route state, while
an explicit conflicting `root_locator` fails closed.

## Security boundary

The protocol calls no authority outside existing Route semantics. Invalid
requests and contexts fail closed, unknown methods are structured errors, and
no request can fabricate evidence or overwrite history. `route/1` has no
Yuich dependency, no Yuich RouteHandle, no local Yuich API, no MCP adapter,
no HTTP transport, no component discovery, and no daemon. It also does not
implement parliament, markets, reputation, social simulation, or worker
self-modification; those are possible consumers above Route Core.

## Open institution surfaces

The [bounded institution runtime](open-institution-runtime.md) uses the same
dispatch-generated advertisement, stdio/JSONL transport and persistent
idempotency receipts. Implemented methods are institution.list, institution.get,
institution.inspect, institution.register, institution.activate,
institution.deactivate, institution.bindings, institution.invoke and
institution.replay. Registration/activation/deactivation/invocation are
mutations; the rest are read-only and receipt-free. Typed lifecycle/effect
records are nested atomically inside existing INSTITUTION DevelopmentEvents.
Package capability requests never confer grants. Local host/operator access
owns bindings; this is not authenticated remote Human authorization.
Only declarative evaluation is implemented; no arbitrary package execution.
