# Open Institution Runtime I

Baseline: `8f5316bc06f318fd567eb787321fabfd0896fad4`, fruit.
Initial audit: local/remote divergence 0/0; only three preexisting Yuich
deletions, excluded. The ownership audit below preceded implementation.

## Pre-implementation ownership audit

ROUTE PROVIDES THE SUBSTRATE, NOT THE SOCIETY.
ROUTE DOES NOT REQUIRE A PARTICULAR SOCIAL ORDER.

| Existing owner | Institution integration boundary |
|---|---|
| DevelopmentEvent hash-chain / GlobalRevision | Canonical durable version, binding and invocation transactions; no additional database or event bus |
| SharedDevelopmentState | Bounded identity/revision/work/worker/ref projections, not entire raw history |
| WorkerIdentity / Presence / WorkerMessage | Reused by reference; institutions cannot acquire a Worker identity or impersonate one |
| ExecutionSession / Intent | Work references and requests only; no second Task ontology, no implicit task creation/failure |
| EvidenceStore | Untouched; institution outputs never Evidence |
| Constraint projection / Constitution / Protocol | Partial existing coverage stays explicit; it cannot confer grants |
| Reference / Cooperation | Read-only relevant IDs and external package locator resolution; no expansion of these domains |
| route/1 | Same dispatch-generated capabilities and durable receipt store |
| route-plugins EventBus | Existing native commit/rollback plugin callbacks, not durable project institutions; no new bus and no unrestricted plugin execution through this adapter |
| OS ownership lock / atomic ledger writes | Reused for institution transitions; generic event ingress must not forge institution transitions |

## Selected bounded implementation

One manifest file (`institution.json` in a directory, or an explicit file),
at most 64 KiB, exact content hash including declarative behavior and configuration.
Source remains external. A new implementation/config requires a new version;
old versions are immutable ledger records pointing to their original source.
Missing/changed old source means unavailable/version mismatch, not silent substitution.

Explicit local operator activation records project ID, exact version/hash,
grants, order and operator attribution. Local CLI/stdio access is the existing
trusted host boundary, not authenticated remote Human identity. Package data
and effects cannot call the operator activation interface or grant themselves
authority. Unknown/missing grants fail closed. Future privileged capabilities
remain representable but non-executable.

Hooks are bounded explicit delivery of a committed event (or replay range).
No daemon, automatic recursive hook loop, new queue or implicit on-disk activation.
Composition uses order then institution ID. All members see the same immutable
revision snapshot; competing recommendations remain distinct conflict records.

One atomic invocation transaction carries typed hook/effect-produced/accepted/
rejected records. This makes crash/retry all-or-nothing without a second journal.
Institution effects accepted here are only shared communication/proposals/
requests, never executed Git/filesystem/shell commands or Evidence.

Replay evaluates an explicitly selected version against a bounded historical
range and an honestly labeled current read-only bounded state snapshot. It does
not claim to reconstruct historical external reality. No production writes,
presence updates, receipts or command telemetry during replay.


## Architecture and author contract

```text
Route Core: Project / State / Event / Evidence / Git / History / Cooperation / Worker
  → InstitutionRuntime
      → free-autonomy-minimal (one optional example)
      → custom declarative packages (implemented)
      → Parliament / Market / other code adapters (FUTURE)
```

Institution != Worker != Subject; existence != authority. Output != fact or
Evidence; agreement != truth or authorization. Nothing requires a social order.
Route retains integrity, provenance, authority validation, recovery and history.

The minimal Society SDK is the public Rust value interface
`InstitutionRuntime::evaluate(&InstitutionPackage, &HookContext) -> Result<Vec<InstitutionEffect>>`
and the strict, language-neutral JSON manifest. Authors supply metadata,
`supported_hooks`, `requested_capabilities`, optional bounded string-map
`configuration` / `configuration_schema`, and declarative `rules`.
Each rule selects `hook`, optionally an event type and
`require_available_work`, then returns typed effects. Configuration is
fingerprinted metadata; it is not an executable expression language.
See the single [example manifest](../examples/institutions/free-autonomy-minimal/institution.json).
Copy it to an author-owned location and choose your own ID/version/behavior.
There is no built-in enum of institution identities.

A directory resolves to `institution.json`; a file locator resolves directly.
RPC registration can alternatively use a registered `cooperation_ref`.
External/foreign sources are read-only: no files are copied into the project.
A future registry or external stdio adapter is representable metadata, but no
such adapter is loaded/executed. The Rust trait itself is not an OS sandbox;
safety today comes from running only the fixed bounded declarative evaluator.

## State, authority and composition

Registration stores immutable metadata, exact SHA-256 of manifest bytes and a
separate configuration hash. Editing whitespace also changes the content hash.
Keep v1's source intact when creating v2; Route does not back up source bytes.
A missing old source cannot be restored by activation alone.

Bindings have explicit project ID, version/hash, configuration hash reference,
operator attribution, timestamp, order, grants and compare-and-swap revision.
Zero bindings are valid. At most 16 may be active. Activation order is ascending
`order`, then institution ID. Every institution receives the same input
GlobalRevision and bounded snapshot. Effects have unique provenance IDs;
different values for the same `conflict_key` retain separate proposals with
`conflict_with` links. No voting, last-writer-wins or automatic conflict resolution.

| Authority | Permitted result |
|---|---|
| OBSERVE | Hook delivery; NO_OP |
| COMMUNICATE | MESSAGE, still institution-attributed rather than WorkerMessage |
| PROPOSE | RECOMMEND, PROPOSE, REQUEST_WORK, OFFER_WORK, SUGGEST_ALLOCATION, REQUEST_REVIEW, REQUEST_ESCALATION |
| REQUEST_DOMAIN_ACTION | REQUEST_ACTION recorded as a request only |
| CHANGE_INSTITUTION_BINDING | Operator interface only; cannot be granted to a package |
| Unknown/future | Denied |

All effects require OBSERVE to receive context, plus their own authority.
Requested capabilities are informational, never grants. Targeted privileged
mutation/impersonation references are rejected. Even an accepted request does
not run a domain operation: a Worker/operator must independently use existing
Route commands and their validation. Institutions never change Intent success,
Worker identity/presence, Evidence or Git through this interface.

The trusted local CLI/stdio operator may set grants. `activated_by` is attribution,
not proof of authenticated Human identity. Do not expose raw stdio as an
untrusted remote service. There is no remote authorization system in this phase.

## Hooks, history and failures

Six hooks are expressible: ON_EVENT, ON_STATE_CHANGE, ON_WORK_AVAILABLE,
ON_WORKER_STATE_CHANGE, ON_CONFLICT, ON_REQUEST. The caller delivers an existing
project event with its expected current revision; unsupported hooks return
NO_EFFECT. There is no daemon/automatic event subscription. A host may poll the
existing event delta and explicitly invoke with a stable operation key, but
must not automatically feed every institution output back into itself.

Context holds project/version/hash, trigger ID/type/revision, current revision,
up to 16 Worker IDs/presences, active Session work references, Reference IDs and
Cooperation IDs per category; existing constraint file references plus explicit
PARTIAL coverage, requester/correlation/causation. No raw event body or entire
history is passed to the evaluator. Bounded output does not eliminate existing
whole-ledger verification cost or unbounded lifetime history accumulation.

Canonical DevelopmentEvent type INSTITUTION contains one atomic transaction.
Lifecycle entries are INSTITUTION_REGISTERED, INSTITUTION_ACTIVATED,
INSTITUTION_DEACTIVATED, INSTITUTION_VERSION_CHANGED, INSTITUTION_HOOK_INVOKED;
individual effect records contain INSTITUTION_EFFECT_PRODUCED and
INSTITUTION_EFFECT_ACCEPTED/REJECTED. One transaction advances GlobalRevision
once, preserving all nested records atomically. Existing development event
queries return these records. Generic event ingress cannot forge them.

Statuses distinguish SUCCESS, NO_EFFECT, DENIED, INVALID_EFFECT,
INSTITUTION_ERROR, INSTITUTION_UNAVAILABLE and VERSION_MISMATCH.
STALE_CONTEXT rejects an outdated invocation before commit; refresh state and
use a new operation key for the changed request. Optional institution failures
are recorded without failing existing sessions. Corrupt canonical ledgers fail
closed; never reset them as recovery. Changed source requires a new version;
a deleted source requires restoring the exact author-owned bytes.

## CLI and route/1 workflow

All commands run in the managed project, using an installed/built Route binary.
No direct edits to `.route` are needed. JSON output supplies IDs/revisions.

```powershell
route institution inspect C:\\packages\\free-autonomy-minimal
route institution register C:\\packages\\free-autonomy-minimal --operation-key register-v1
route institution list
route institution activate free-autonomy-minimal 1.0.0 --grant OBSERVE --grant COMMUNICATE --grant PROPOSE --activated-by local-operator --expected-binding-revision 0 --operation-key activate-v1
route institution bindings
route institution show free-autonomy-minimal 1.0.0
```

Use an event ID and current revision from `development.events.query` /
`development.state` (or their existing CLI equivalents):

```text
route institution invoke EVENT_ID --hook ON_EVENT --expected-revision CURRENT_REVISION --correlation-id review-1 --operation-key invoke-review-1
route institution replay free-autonomy-minimal 1.0.0 --after 0 --limit 1000
route institution deactivate free-autonomy-minimal --actor local-operator --expected-binding-revision BINDING_REVISION --operation-key deactivate-v1
```

For evolution: create a separate v2 manifest, register with a new key, replay v2,
activate v2 using current binding revision, and rollback by activating v1 using
the new binding revision. Inactive definitions/history are never deleted.

Nine dispatch-advertised route/1 methods: institution.list, institution.get,
institution.inspect, institution.register, institution.activate,
institution.deactivate, institution.bindings, institution.invoke, institution.replay.
Mutations require the existing persistent idempotency key; identical retries
return the original result, changed parameters conflict. Register params use
`source_locator` or `cooperation_ref`; get/replay use `institution_id, version`.
Activation uses `project_id, institution_id, version, grants, activated_by,
expected_binding_revision` and optional `order`. Deactivation uses
`project_id, institution_id, actor, expected_binding_revision`. Invoke uses
`hook, triggering_event_ref, expected_revision, correlation_id` and optional
registered Worker `requesting_actor`. CLI derives project ID, not authority.

## Replay and resource contract

Replay accepts `after_revision` and `limit` 1..1000. It evaluates ON_EVENT using
a selected immutable version, historical triggers and the **current** read-only
snapshot/current binding grants. It is a hypothetical comparison, not historical
world reconstruction or permission promotion. Dry-run lifecycle records never
enter production history. CLI/JSONL reads and replay create no receipts,
telemetry, presence updates or files. Repeated replay is deterministic for an
unchanged snapshot/source.

Manifest <=64 KiB; <=32 rules and <=32 total effects per package; <=16 active
bindings; context lists <=16/category; replay <=1000 events. The implementation
reuses the canonical ledger, ownership lock and atomic writes; no second cache,
database, event bus, task system or build target.

Runtime → minimal Society SDK → user/AI-authored packages → bounded evaluation
is implemented for declarative rules. External code/stdio execution, automatic
promotion and autonomous institution evolution remain FUTURE. Parliament,
Market, reputation and unrestricted self-modification are NOT implemented.

## Verification and measured scope

Deterministic library tests cover immutable registration and replay/conflict,
fingerprints, unknown future adapters, missing grants, generic-event forgery,
raw reasoning/unknown fields, stale context, project isolation, no-effect,
oversize/traversal rejection and corruption preservation.

Real independent Route process tests cover custom external registration and
inspection; two registered Workers with independently attributable messages;
shared institution events with no Worker actor; two ordered active packages
with explicit conflict links; restart persistence; v2 replay/promotion/v1
rollback; all four mutations' retry/conflict behavior; concurrent activation;
five JSONL read surfaces; invalid effects, fingerprint drift, corrupt/missing
sources and core usability. A Windows junction-ancestor fixture verifies
refusal without accessing/copying or modifying its target.

[Resource measurements](open-institution-runtime-budget.json) use one reused
debug build target, 1000 canonical findings, 0/1/5 one-effect packages and five
fresh processes per row. Invoke p95 is 235.74/247.80/249.55 ms, peak <=20.20 MiB.
Replay selects one version (not all five), produces 1000 results (~1.41 MB),
p95 <=227.62 ms, peak <=29.72 MiB, zero durable bytes/hash/mtime changes.
Invoke growth for five distinct keys is 16,540/45,010/158,890 bytes respectively;
these are expected history+receipt bytes, not a read leak. Latency includes
startup/transport/ledger validation/persistence, not isolated handler CPU.
Existing whole-ledger cost and lifetime history growth remain limitations.

Reproduce from `tool/route` with the existing CARGO_TARGET_DIR:

```text
cargo test -p route-basic institution_tests --lib
cargo test -p route-cli --test institution_e2e
cargo test -p route-cli --features test-utils --test institution_e2e institution_resource_budget -- --ignored --nocapture
```

No full society, external code sandbox, background delivery, cross-host
authentication or autonomous self-modification is certified by these tests.

## Final regression (2026-09-08)

- Formatting check: exit 0.
- route-core: 23 passed, exit 0 (including doc test).
- route-basic: 401 passed, 2 preexisting ignored, exit 0.
- route-cli: 65 passed, exit 0, including 5 institution process tests.
- Workspace excluding route-pyo3: 610 passed, 2 preexisting ignored, exit 0.
- Final-source resource benchmark: 1 passed, exit 0 (separate explicit run).
- Documentation links/examples: exit 0; all 4 root PowerShell scripts parse.
- Git whitespace check: exit 0; no Yuich deletion staged or restored.

The CooperationResource-only registration regression first caught the missing
optional locator default; it passes after the fix and a complete rebuild.
The final process tests also preserve independent Worker message attribution
when responding to institution effect references. Readiness is
DAILY_USE_VERIFIED only for this bounded local declarative workflow.
