# DevelopmentEvent ledger storage I

Status: **ROUTE_LEDGER_STORAGE_I = PASS** on `fruit` (2026-10-15), within the
Windows process-crash and ledger/Evidence recovery scope below. This document
defines the storage contract; it does not extend Route's AI, routing, Evidence,
archive, or project-entry semantics.

## Scope and authority

The canonical DevelopmentEvent history is still the ordered vector of the same
`DevelopmentEvent` values. Event IDs, global sequence numbers, operation-key
deduplication, provenance fields, evidence references, previous hashes and event
hashes retain their existing meaning. The physical writer changed; the domain
model did not.

The following stores remain separate and were not folded into this journal:

- `EvidenceStore` and System/Agent/Manual evidence;
- CheckRun/finalization/process receipts;
- project identity and host binding metadata;
- Basic's SQLite archive and snapshots;
- Reference registry/operation recovery;
- derived Goal, Work, Workflow, Team, Base and Context projections.

Consequently, a journal read is one consistent DevelopmentEvent snapshot. Route
does not claim an atomic snapshot spanning that journal and every separate store.

## Previous format

Legacy projects use `.route/development/ledger.json`, a schema-1/2 JSON envelope.
Every append acquires `.append-lock`, loads and verifies all events, builds the
new event and hash, serializes the whole envelope, flushes a temporary file and
renames it over the ledger. Reads load and verify the whole envelope. This is
safe against readers observing a half replacement, but ordinary append rewrites
all historical bytes and cumulative write work grows quadratically.

Legacy JSON remains readable and writable. It is never upgraded implicitly.

The audited call chain is `CLI/RPC/domain operation → append_event_inner →
DevelopmentAppendLock → storage::load → caller/domain/idempotency validation →
event_hash → ledger.verify → storage::commit_append`. The only canonical writer
in the production domain API is `append_event_inner`; `seed_benchmark_events` is
a test-utils-only bulk fixture initializer. Queries and all domain projections
use `load_ledger_readonly → load_ledger_at → storage::load`.

| Boundary | Source and relevant functions |
|---|---|
| Event ordering, ID, operation key, provenance chain | [development.rs](../tool/route/crates/route-basic/src/development.rs): `append_event_inner`, `same_semantics`, `event_hash`, `DevelopmentLedger::verify` |
| Format, frame, flush, acknowledged head, migration/backup | [storage.rs](../tool/route/crates/route-basic/src/development/storage.rs): `load`, `parse_journal`, `append_frame`, `commit_append`, public storage operations |
| Project/workspace ownership | [project_identity.rs](../tool/route/crates/route-basic/src/project_identity.rs): `load_identity`, `ensure_identity`, `attach`; `development_state_owner` resolves the owner |
| Kernel process lock | [ownership_lock.rs](../tool/route/crates/route-basic/src/ownership_lock.rs): `OwnershipLock::acquire` uses `File::try_lock`, PID/nonce metadata, no age-based takeover |
| Atomic replacement | [constitutive.rs](../tool/route/crates/route-basic/src/constitutive.rs): `write_atomic` writes/syncs a unique sibling temp then renames; no delete-before-replace |
| Separate Evidence authority | [execution.rs](../tool/route/crates/route-basic/src/execution.rs): `EvidenceStore::load/update/save_locked`, `.evidence-write-lock`, version/fingerprint conflict checks |
| Operator-only administrative CLI | [main.rs](../tool/route/crates/route-cli/src/main.rs): `StorageAction`; Worker credential ingress rejects these commands |
| CLI/RPC process acceptance | [ledger_storage.rs](../tool/route/crates/route-cli/tests/ledger_storage.rs): isolated Git migration/restore and feature-gated actual CLI crashes |

## Framed journal v1

New projects use:

```text
.route/development/
  ledger.manifest.json       format binding and small acknowledged-head anchor
  ledger-000001.rlog         sole writable event authority
  ledger.json                schema-255 compatibility fence, not an authority
  ledger.pending.json        initialization/migration recovery metadata
  .append-lock               kernel-owned cross-process writer lock
```

The manifest binds `format=route.development-journal`, version 1, project ID,
ledger ID and the one active segment. V1 deliberately has one segment and no
rotation policy. Adding segmentation requires a separate protocol and migration;
the current reader will fail closed on an unknown segment name.

The journal header contains the same project and ledger identities plus a SHA-256
digest. Each event is stored as:

```text
EVT1 | payload length | sequence | SHA-256(payload) | header SHA-256 | event JSON
COM1 | sequence       | SHA-256(payload)
```

Lengths and integers are fixed-width little-endian fields. The existing event
hash remains the provenance/hash-chain check; the frame digest detects torn or
altered serialization and does not replace that hash chain.

The compatibility fence makes older Route binaries reject the project instead
of treating a missing legacy ledger as an empty history and creating a competing
authority. Old binaries cannot read or write journal v1. New binaries can read
both formats and keep legacy projects on their original writer until migration.

## Commit and recovery protocol

One cross-process kernel file lock orders all writers. Under that lock a writer
replays and verifies the committed history, applies domain and idempotency checks,
then:

1. appends the complete `EVT1` prepare frame;
2. calls `File::sync_all` for the prepare bytes;
3. appends the `COM1` commit trailer;
4. calls `File::sync_all` again;
5. returns success only after step 4.

The commit boundary is the successfully flushed COM1 trailer. Before returning
success, Route also replaces the small manifest with the acknowledged revision
and hash. Every replay checks this lower-bound anchor; truncating a previously
acknowledged record cannot be mistaken for a recoverable tail. Header checksums
prevent a corrupt length from reclassifying committed history as partial EOF.
A retry after an
acknowledgement loss finds the original operation key and returns the original
event. It cannot mint a second event.

An EOF inside the final PREPARE/COMMIT pair is
`INCOMPLETE_TRAILING_RECORD`. Read-only open may briefly retry to avoid racing an
active writer, but never modifies the file. A subsequent writer under the
exclusive lock may truncate only that protocol-proven uncommitted suffix, flush
the truncation, re-verify the committed prefix, and retry. It does not skip or
truncate invalid committed records.

Diagnostics distinguish:

- `INCOMPLETE_TRAILING_RECORD`: EOF in the only final uncommitted frame;
- `CORRUPT_COMMITTED_RECORD`: complete record has invalid digest, JSON or binding;
- `CORRUPT_MIDDLE_RECORD`: invalid framing/order before the committed end;
- `UNKNOWN_COMMIT_STATUS`: an I/O/flush error occurred where the caller cannot
  truthfully infer whether bytes reached the OS/filesystem;
- `CORRUPT_HEADER`, `MISSING_SEGMENT`, `UNKNOWN_FORMAT_VERSION`, and
  `LEGACY_FENCE_*`: format/binding failures that never fall back to empty history.

## Windows durability boundary

Rust documents `File::sync_all` as attempting to synchronize file content and
metadata to disk and as the way to surface errors otherwise lost on close:
<https://doc.rust-lang.org/std/fs/struct.File.html#method.sync_all>.
Microsoft documents `FlushFileBuffers` as writing buffered information for a
file to the device and notes both its per-write cost and the stronger caveats
around caching/hardware:
<https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers>
and
<https://learn.microsoft.com/en-us/windows/win32/fileio/flushing-system-buffered-i-o-data-to-disk>.

Acceptance uses real Windows process termination, handle release/reacquisition,
restart and fault injection. The supported claim is **process-crash consistency
on the tested local Windows filesystem**. Sudden machine power loss, controller
cache behavior, remote shares and other operating systems are not physically
tested: `POWER_LOSS_VERIFIED=NO`.

Manifest and migration publication use a flushed temporary file followed by a
same-directory rename. The current helper does not explicitly flush a directory
handle or request unbuffered/write-through I/O. Therefore no stronger hardware
power-loss promise is made.

## Explicit migration and rollback

```powershell
route-ai storage inspect --json
route-ai storage migrate --json
route-ai storage rollback-migration --json
```

Migration holds the same writer lock and performs these steps:

1. load and fully verify legacy JSON;
2. build a new journal segment independently;
3. replay it and compare the exact ordered events and final head;
4. persist `ledger.legacy.json` as the non-writable recovery source;
5. replace `ledger.json` with the schema-255 old-binary fence;
6. publish the manifest last as the authority cutover.

Before manifest publication, legacy JSON or its verified recovery copy remains
the source of truth. Pending metadata binds the recovery copy's original hash.
The sentinel-only intermediate state fails closed and a repeated migrate resumes
from the recovery copy. Interrupted empty initialization resumes on a writer
retry. After publication, only the
journal receives events. The recovery JSON is not a second writable authority.
Re-migration after rollback reads the live legacy JSON, including new writes.
Recognized prior staging segments are moved to uniquely named recovery files;
unrecognized segments are preserved and rejected. These files are migration
recovery material, not ordinary append overhead or writable authorities.

Rollback is permitted only when journal revision, head and every event still
equal the migration base. Once any post-migration event exists, rollback returns
`ROLLBACK_REFUSED_NEW_EVENTS`; the safe path is to back up/export and retain the
journal. It never discards new facts to make an old binary start.

## Backup and independent restore

```powershell
route-ai storage backup C:\safe\route-ledger-backup
route-ai storage restore C:\safe\route-ledger-backup C:\restore-target
```

Backup runs under the writer lock, verifies the ledger, copies the complete
manifest/segment/fence set (or one legacy JSON file), records length and SHA-256
for every file, plus project ID/revision/head, and rejects a nonempty target.
When Evidence exists, it holds the existing Evidence writer lock and includes
the verified `execution/evidence.json` bytes as a supporting store.
Restore accepts only a directory without existing `.route` state, verifies every
file before publishing, retains the project ID, creates a fresh workspace ID and
then reopens/verifies event count, revision and head in independent staging before
publishing `.route`. File names must match the complete allowed set; traversal,
duplicates, missing files and invalid receipts are rejected before publication.
Event payloads and Evidence IDs/references are unchanged. Restored workspaces
receive no external Host key. Explicit attach is required to regain Worker access.
This command backs up the ledger and Evidence, not source files, Sessions,
CheckRun/process/finalization receipts or all other project metadata. Cross-domain
complete runtime recovery remains PARTIAL; existing game saves remain separate.

## Measured scaling and known trade-off

The reproducible acceptance is the explicitly run ignored test
`development::storage::tests::ledger_storage_performance`, with JSON written to
`Release/local-tools/ledger-storage-20261014/performance.json`. It creates the
legacy and journal fixtures with the same current debug-test binary and same
event drafts at 100, 400 and 1,000 events.

The required property is met: journal append bytes are the new frame only and no
longer grow with the complete historical file. The current writer still replays
and verifies the full journal under the lock before each append, and every reader
does a full verified replay. Therefore:

```text
WRITE_BYTES_SCALING = O(new event bytes)
WRITE_CPU/READ_SCALING = OPEN_LINEAR_FULL_REPLAY
```

Two durability flushes and an additional frame digest can make small debug-build
append latency higher even while write amplification collapses. That trade-off
is reported, not hidden. A sparse verified index/checkpoint may be evaluated in
a later phase, but no cache, daemon, database, segment rotation or weakened
verification is introduced here.

Final same-binary debug measurements (2026-10-15), including initial journal
metadata and every acknowledged-head manifest replacement:

| Events | Legacy serialized bytes written | Journal + metadata bytes written | Legacy append p50/p95 ms | Journal append p50/p95 ms |
|---|---:|---:|---:|---:|
| 100 | 3,506,717 | 111,721 | 11.906 / 17.559 | 12.766 / 18.011 |
| 400 | 55,772,267 | 445,021 | 32.145 / 64.704 | 39.520 / 65.709 |
| 1,000 | 348,493,368 | 1,111,623 | 78.682 / 144.936 | 77.995 / 136.322 |

The 1,000-event result reduces this storage-layer byte count by about 313x.
Journal payload file size is 737,676 B versus legacy 696,621 B: framing improves
write amplification, not logical compactness. Warm full replay p50/p95 at 1,000
events is legacy 58.180 / 69.171 ms and journal 92.160 / 112.524 ms; this read
cost is explicitly open. Counts reflect application bytes passed to persistence,
not OS physical writes. Lock-owner metadata, transport receipts, filesystem
journaling and device amplification are outside this byte count. First-replay
single samples are not cold-cache p50/p95. No physical cache purge was performed.

## Verification map

Final standard release acceptance uses the same packaged binary for both
formats, sequentially without a concurrent benchmark or regression. Binary
SHA-256 is `326a7ee5a85f0d1e55a3b8af576ffd7185b48b09772279c625d9a09b199c0a42`.
Project/workspace identity lengths, progress text and build profile are equal.
This is a bounded local observation, not a controlled performance SLA.

| Release observation | Legacy JSON | Journal |
|---|---:|---:|
| 1,000 normal fact ledger writes | 632,522,959 B | 1,194,685 B |
| Acknowledged-head metadata writes | 0 B | 372,899 B |
| Total measured storage-layer writes | 632,522,959 B | 1,567,584 B |
| First 100 writes p50/p95 | 51.029 / 64.087 ms | 61.447 / 77.092 ms |
| Last 20 writes at 1,000 facts p50/p95 | 97.286 / 107.352 ms | 104.334 / 111.374 ms |
| Fresh enter after attach p50/p95 (3 runs) | 43.599 / 44.375 ms | 43.423 / 44.688 ms |
| Warm enter p50/p95 (100 runs, fresh project) | 45.411 / 51.468 ms | 46.318 / 57.959 ms |
| Enter at 1,000 facts p50/p95 (8 runs) | 86.898 / 109.418 ms | 102.861 / 107.965 ms |
| Sampled working-set maximum at 1,000 facts | 15,978,496 B | 16,818,176 B |
| Startup p50/p95 (3 runs) | 36.361 / 45.004 ms | 44.014 / 45.975 ms |

The normal-fact release write count falls about 403.5x. Each format passes 100
enter, 100 status and 100 handoff reads with zero durable growth. RSS is sampled
about every 2 ms and is a lower bound on peak, not an exact peak measurement.
The earlier overlapping-load release runs are retained but not used in this
final table. Debug full-replay measurements above remain the 100/400/1,000
storage-layer comparison; first replay is a single sample at each scale, not a
cold-cache percentile series. No 10,000-event or physical power-loss claim is made.

The final private package contains one `route-ai.exe`, LICENSE and notices only:
3 files, exe 17,051,136 B (prior baseline 16,917,504 B), ZIP 6,479,408 B (prior
6,437,564 B). The project doorway remains 867 B. There is no new required user
configuration, helper executable or service. Existing projects are not silently
migrated. The current project/runtime history was not used for destructive tests.
All 133 pre-phase working paths still match their initial existence/content hashes.

Production and test implementation is in
`tool/route/crates/route-basic/src/development/storage.rs`. Tests cover real
subprocess crashes A-H, same/different operation-key races, eight independent
writers, concurrent reader/writer, partial tail recovery, committed and middle
corruption, format/header/segment/fence failures, I/O/flush/rename faults,
legacy read/write, explicit migration, rollback refusal, backup/restore, exact
event equivalence and 100 read-only opens with zero byte growth.

CLI integration retains `route-ai attach`, `route-ai enter --json`, RPC, Team,
Institution, Cooperation, Base and the one-ROUTE.md/one-executable product
surface. The journal adds internal `.route` files only; users do not maintain a
second config, helper executable or service.

The real standard CLI acceptance creates an isolated Git project and checks
attach/enter, a Goal, normal facts, migration, restart, further writes, backup,
independent restore, identical Goal state and event history, fresh workspace
identity and explicit re-attachment. The test-utils CLI additionally exits at
four actual RPC append windows and verifies operation receipt retries converge
to the same single event. Test-only failpoints are excluded from the standard
release build.

A separate migration audit copies the retained positive-chain fixture into an
independent directory. All 11 queried projections are identical before/after:
Worker list, Principal bindings, Assistant status, Work list, Cooperation list,
CooperationKnowledge query, Base query, Base detector, Handoff, events and
development state. Revision remains 17, head remains
`82b98791af44e260d52e35e86c1b9d4c59d9638d0359d47c4f230314f629d9a3`.
The fixture contains source-backed knowledge, real historic failed/passed/reuse
Evidence and a reviewed Base chain; it is not a new AI synthesis run.

Additional CLI fault audit confirms staging-crash retains legacy authority,
rollback interrupted after manifest removal can resume, and a manifest from a
different ledger identity is rejected. Audit JSON and fixtures are retained in
ignored `Release/local-tools/ledger-storage-20261014/`.

Full checks have actual exit 0 for fmt, route-core, route-basic, route-cli and
the workspace excluding route-pyo3, plus docs links, PowerShell parsing and diff
whitespace. The workspace basic suite has 495 passed / 18 ignored; test-utils
CLI ledger acceptance has 2 passed / 0 ignored. Explicit performance acceptance
was separately run with exit 0; an ignored marker alone is never counted as PASS.
Sandbox-only runs that failed on Windows access restrictions are retained as
failed attempts; the corresponding outside-sandbox runs passed. Existing unused
variable/import warnings remain outside the phase scope.

One repeat workspace attempt exited 101 because the existing
`run_continuity::actual_route_restart_inspect_cancel_and_keyed_release_no_duplicate`
fixture did not publish its grandchild-ready marker within the unchanged 15-second
deadline. That attempt overlapped release performance measurement. Its log is
retained as `workspace-attempt2-failed.log`; the isolated original test then
passed (2 passed / 1 fixture ignored), and the subsequent complete workspace
without a parallel benchmark passed with exit 0. This is recorded as a transient
readiness failure, not a proved production fix or a waived assertion.

The final file-existence guards use fallible `try_exists` checks: permission and
metadata I/O errors propagate instead of silently selecting empty history.
Append and acknowledged-head errors carry ledger/segment/revision/event context
and direct callers to inspect and retry the same operation key.

Outstanding boundaries remain visible: `ReadScaling=OPEN`,
`CrossDomainSnapshot=PARTIAL`, `POWER_LOSS_VERIFIED=NO`,
`REAL_AI_GOLDEN_PATH=NOT_RUN`, and untested platforms are not certified. Ledger
plus Evidence backup is scoped as described above; it is not a replacement for
complete source/runtime game saves.
