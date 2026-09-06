# Cooperation reliability closure audit

Phase: ROUTE_COOPERATION_RELIABILITY_CLOSURE_I. Baseline `4e0e2865` with
preexisting uncommitted Reference/Cooperation work, not a clean checkout.

## Pre-code inventory and ownership

Existing changes: Reference schema/persistence in constitutive; Cooperation
models and projections in cooperation; event schema/shared summaries in
development; read-only material projection; library exports; unfinished RPC
helpers and identity checks; tests and readiness/architecture documentation.
The evolution change is a test-fixture adaptation. Existing deletions under
yuich and docs/yuich.md are excluded and preserved.

| Mutation ingress | Canonical owner | Validation / persistence boundary |
|---|---|---|
| ReferenceRegistry write/update/ensure_exists, legacy CLI reference add/remove/import/refresh, learn/promote, context restore | `.route/reference/registry.json` | All must converge on registry write, not raw file replacement |
| Cooperation register/refresh | development ledger | Event append lock; resource state is a projection, not a second store |
| Knowledge/capability record, discovery, human guidance | development ledger | Event append lock; helper prechecks are not sufficient |
| Generic append_development_event, RPC development.event.record | development ledger | Same canonical transition and evidence validator as helpers |
| Reference-related events and unfinished RPC helpers | registry operation + ledger | Must reject unjournaled Reference transition claims |
| EvidenceStore load/save/record; system execution and RPC evidence.record | execution EvidenceStore | Existing System vs Agent/User distinction; generic messages/knowledge cannot stand in for Evidence |
| RPC preflight/completion receipts | `.route/rpc-idempotency.json` | Existing PENDING/COMPLETED semantics; do not add a parallel transport receipt store |

GlobalRevision is the ledger sequence; event and revision commit in one atomic
ledger replacement. Reference needs a durable before/after operation journal
bridging its existing registry to that ledger. Operation keys must survive a
retry; no history reset or separate Cooperation database is needed.

Known unsafe defaults discovered before edits: Reference reads in constitutive
ContextSnapshot, plan, CLI commands; conditional reads in brief/drift/guardian/
memory/evolution; EvidenceStore JSON parse fallback. Context restore writes the
registry directly and must use the same Reference commit boundary.

Lock audit: registry and ledger use age-based directory takeover. RPC uses a
separate directory lock; its write fallback deletes the destination before
rename. Closure should use one reusable OS-lock primitive with owner metadata,
no lock-file deletion/takeover, and atomic replacement for durable data.

## Implemented closure mechanism

Reference uses `operation.json` with owner, operation ID, before/after hashes,
the intended registry bytes and a completion marker. All library write/update/
ensure entry points use this bridge; stale in-memory writers fail CAS rather
than losing another writer's changes. Reads never recover by writing: an
unfinished operation yields `REFERENCE_RECOVERY_REQUIRED`; call
`ReferenceRegistry::recover` explicitly or use the mutation API to reconcile.
Recovery rolls forward exactly once. Historical operation IDs are retained in
the existing ledger; same-ID changed-content retries conflict. Context archive
`registry.json` files are historical material, not active registry writes.

Cooperation is already event-sourced: domain effect and GlobalRevision share
one ledger replacement. Its operation key is the event deduplication key.
Generic RPC event PENDING receipts can safely re-enter that same idempotent
domain boundary, rather than using a second receipt database. Other uncertain
RPC mutations retain their existing recovery-required behavior.

Lock ownership uses Rust standard-library OS file locks and diagnostic
PID/nonce metadata. A live OS handle cannot be displaced by a timestamp. A
dead process releases the kernel lock even without a destructor. Lock files
are never unlinked; malformed metadata and legacy directory locks fail closed.
Legacy directory-lock recovery is deliberately not automatic: age does not
prove its old writer died. This requires Rust 1.89+ for file-lock APIs; validation
here uses Rust 1.95 on Windows. See the [Rust File locking contract](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).

`cooperation::validate_transition` is invoked under the ledger write lock after
replay lookup, covering generic and dedicated library entry points. OBSERVED
requires successful System CheckPass/TestPass Evidence with project_id,
cooperation_id and resource_fingerprint metadata matching the current resource.
Unbound legacy evidence remains stored but cannot establish this elevated claim.
Knowledge is not Evidence; Agent/User records and failed checks cannot promote
a claim. New overlapping observed capability claims require explicit
supersession. Missing/cross-resource/already-superseded targets and duplicate IDs
are rejected under the lock.

Reference/Cooperation inspection uses read-only identity lookup. Missing state
does not initialize directories; copied identity bindings are not rewritten by
inspection. Constraint coverage is explicit: constraints and ppam are bounded
metadata projections (PARTIAL when present); Constitution/Protocol prose and
active intent rules are NOT_PROJECTED, or MISSING for absent source files.

New Reference/Cooperation RPC methods remain unavailable. Obsolete unused
Reference RPC double-write helpers were removed, not exposed. This phase does
not certify daily use or performance budgets.

## Final verification (2026-09-06)

ROUTE_COOPERATION_RELIABILITY_CLOSURE_I: PASS (bounded reliability closure,
not daily-use certification).

| Command | Result | Exit |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | 0 |
| `cargo test -p route-core` | 22 unit + 1 doc test passed | 0 |
| `cargo test -p route-basic` | 393 passed, 2 subprocess fixture entry points ignored by the outer runner | 0 |
| `cargo test -p route-cli` | 54 passed across library, binary and integration suites | 0 |
| `cargo test --workspace --exclude route-pyo3` | 591 passed, 2 fixture entry points ignored | 0 |
| `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check-docs.ps1` | PASS | 0 |
| PowerShell parser validation of `scripts/*.ps1` | PASS | 0 |
| `git diff --check` | PASS | 0 |

The two ignored functions are invoked explicitly by their parent tests as
subprocess fixtures; they deliberately exit with crash codes. They are not
skipped acceptance scenarios. The lock test holds a live child lock beyond
30 seconds, rejects takeover, then verifies recovery after process exit without
destructors. Reference tests terminate children at reservation, durable domain,
durable event and lost-response windows, then reconcile and retry without
duplicate events. In-process tests additionally cover changed-key content
conflicts and stale-writer CAS.

The seven real-process open-society integration tests include generic RPC
evidence rejection/success, PENDING receipt replay, one-winner concurrent
supersession, and legacy Reference read/corruption boundaries. Dedicated new
Reference/Cooperation RPC methods advertised: NONE.

No current regression failure remains. An intermediate Windows sync failure
was CODE_FAILURE (a read-only handle was used for FlushFileBuffers); it was
fixed by syncing a writable handle and the full gates were rerun. Existing
unrelated test warnings remain, not suppressed.

Resource budgets and three-cycle daily-use acceptance: NOT_RUN. Full engine
constraint extraction is not claimed. No history rewriting, main-worktree,
Yuich, tag, release or force-push operation is part of this closure.
