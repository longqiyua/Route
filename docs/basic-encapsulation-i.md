# Route Basic Encapsulation I

Internal engineering acceptance reference, 2026-10-14. This is a fruit development
profile, not a public Release. Real AI golden-path session acceptance is NOT_RUN:
the installed Codex CLI currently reports `Not logged in`; no other configured
Host was found on PATH. No login or provider setup was performed.

## Product doorway

Normal projects need to recognize only `ROUTE.md` and the Route CLI. The embedded
host-neutral template uses `route-ai enter --json`; explicit `route-ai attach`
bootstraps it once. Dynamic Goal/Work/worker/history/credentials never enter that
file. Existing user files are not replaced, even when unknown or oversized.
`route-ai doorway --template` prints the canonical template without writing it.
Human integration/upgrading of an existing file is explicit; there is no automatic
replacement or managed-block injection.

Version marker `<!-- route:1 -->` identifies the current exact template; known old
doorways/v0 are upgrade-recommended, modified managed files require review, and
unknown markers remain unknown. A marker is never permission or executable input.
The active Route repository itself has a preexisting larger user-maintained
ROUTE.md and generated host files; those original migration/maintenance changes
are preserved. The <=4 KiB hard limit applies to newly emitted basic doorways,
not destructive truncation of an existing user document.

## Installation and Windows collision

Actual pre-phase resolution: both `Get-Command route -All` and `where.exe route`
resolved `C:\Windows\System32\ROUTE.EXE`, the networking utility. Do not overwrite
it, change System32 or assume bare `route` reaches this product.

The internal basic package contains one primary executable named `route-ai.exe`.
It is the existing Route CLI implementation, not a second backend/helper. Install
it once in a user CLI directory you explicitly choose, or invoke its absolute
path. Human PATH configuration is optional installation work, never an action
requested/executed by ROUTE.md. Legacy product route.exe still works by explicit
path. Source development remains available:

```sh
cargo build --manifest-path tool/route/Cargo.toml -p route-cli --release --locked
```

For audited local packages use the existing path-remapping builder followed by
`scripts/package-basic.ps1 -Binary ... -OutputDirectory ...`. The packager creates
only new directories below Release/packages and refuses overwrite. It renames the
program in the package, not the installed system utility. No download, installer,
provider setup, account or daemon is added. Existing release flags remain
opt-level=3, thin LTO, codegen-units=1, strip=true; panic/security/recovery behavior
is not reduced to save size.

## One read-only facade, same state

`enter` selects current Goal/Work, blocked/stale/required-check state and recent
published facts from the existing bounded handoff. Optional relevant Base uses
the existing <=3-unit query and the same common public-data redactor. Worker
identity uses the existing sidecar binding read path, not provisioning. The
response carries route.enter/1, version, project, worker, doorway validation,
bounded continuity, optional Base, omissions and suggested actions. Suggestions
are not permission; all existing Worker/Operator/domain gates still apply.

No new database/cache/second truth store. No persisted copy of handoff, Context,
Base projection, source, binaries, transcripts or chain-of-thought. Output target
is 16384 bytes including newline; implementation enforces that stricter limit
rather than using the 32768-byte ceiling. Optional Base and secondary continuity
rows are dropped deterministically before critical Work. Omissions are explicit;
exceptionally rich required detail yields DETAIL_REQUIRED pointing to the existing
handoff, not malformed/truncated JSON or an invented complete view.

Unattached inspection returns structured ATTACH_REQUIRED, no identity and no
authority. It creates neither state, ROUTE.md nor credentials. A copied doorway
is a discovery boundary even without Git; a private state copy with a different
bound checkout requests REASSOCIATION_REQUIRED without silently rotating it.
Corrupt/redirected state is unavailable, not trusted as an empty successful Goal.
`--independent` withholds Base and prior narrative conclusions for independent
review. Normal absence leaves Git/editor/build/test usable.

## Smaller explicit attach

Basic attach now initializes only existing project identity, ignored domain
ledger and local Worker binding. It does not pre-create the archive SQLite DB,
Constitution/Protocol/reference defaults, host prompt files or a source archive.
Existing .route-basic data is preserved. Advanced archive functionality remains
available through the existing explicit `init` path. `status` on a metadata-only
attachment reuses entry; existing archive status remains unchanged. Status reads
no longer append legacy command-history events.

Credentials remain outside the project under the existing sidecar home. Acceptance
uses an isolated external-to-fixture home to measure exact scoped bytes, not to
change the user's profile or require a configuration field. Ignored Route state
requires no manual editing; no `.gitignore`/hooks/index edits are performed by attach.

## Evidence and scope

Before implementation, the previous verified Base batch was independently
committed as `7b2d1e98952e8c1bd53b0d13b11a056320e49568`. Focused/core/basic/CLI/workspace,
fmt/docs/PowerShell/diff gates were rerun before ordinary fruit push. Fetched local
fruit and origin/fruit matched. All 133 preexisting A paths were excluded.

The real pre-phase release baseline is 31780352 bytes; deterministic minimal ZIP
11463295 bytes, three packaged files. It was built with the same remapped release
profile before linking the new facade, not taken from an older published package
or a debug build. Baseline attach: 14 local files / 178737 local bytes, 794-byte
doorway and one external 64-byte key; no source copies. The 172032-byte SQLite DB
dominated local overhead. Build-cache/debug/history sizes are separate, not added
to package or project-footprint totals.

Focused tests exercise template-only discovery, unmanaged/versioned file safety,
real subprocess A/B structural continuity, copy and local Git clone, 100 readonly
operations, valid bounded JSON, no key echo/Markdown authority, no host files and
a normal rustc build in a clone without Route state. These process fixtures are
not real AI sessions, CrossHost or AIKnowledgeSynthesis certification.

Whole-ledger JSON rewrite/replay remains an existing scalability limitation. This
phase measures persisted bytes/event and does not silently replace canonical
storage with another index or event system. Existing notices are retained in the
internal package; this is not a fresh public third-party licensing certification.

Final measurement/regression/Git receipts are recorded in the acceptance section
below and ignored local artifacts, after actual completion. See also the
[backlog](architecture-improvement-backlog.md), [readiness](practical-readiness.md)
and [Base acceptance](base-positive-chain-i.md).

## Final acceptance — 2026-10-14

ROUTE_BASIC_ENCAPSULATION_I: **PASS, controlled Windows structural/release-profile
scope. REAL_AI_GOLDEN_PATH remains NOT_RUN.**

The project-local ignored state fell from 178737 to 3100 bytes (98.27% reduction),
not by deleting functionality/history, but by deferring archive DB/bootstrap to
explicit advanced use. Including the tracked doorway, fresh Route footprint is
3967 bytes plus the existing external 64-byte local binding key.

| Release-profile measurement | Before | After | Delta |
|---|---:|---:|---:|
| Executable | 31780352 B | 31867904 B | +87552 B / +0.2755% |
| Minimal ZIP | 11463295 B | 11490699 B | +27404 B / +0.2391% |
| Packaged files | 3 | 3 | 0 |
| Primary executables | 1 | 1 | 0 |
| Native import dependencies | 20 | same 20 | 0 new |

Growth is explained by the facade, stable template, root/file safety and binding
projection. No new dependency, feature deletion, panic/compiler-policy change or
new helper executable. The optional no-growth stretch target was not met; the
actual increase is below 5%. Existing Windows UCRT/VCRUNTIME140 prerequisites
remain; this does not certify a clean machine lacking the existing VC runtime.

Exact internal package manifest: `route-ai.exe`, `LICENSE`,
`THIRD_PARTY_NOTICES.md`. ZIP entries match exactly; manifests/receipts remain
outside the package. It includes no source, debug symbols, target cache, private
reports, runtime state, credentials, Wiki, Library, Yuich or developer docs tree.
Alias resolution was tested in an explicitly scoped child-process PATH, with
route-ai resolving to the packaged product while route still resolved to System32.
No session/user/machine PATH was changed; system binary SHA256 stayed identical.

### Performance and growth

These are actual packaged-release process wall times, including startup, in small
controlled Windows projects; cold means first enter per fresh project, not flushed
OS cache. The rich scenario is the retained source-backed Base fixture.

| Operation | Samples | p50 ms | p95 ms |
|---|---:|---:|---:|
| attach | 8 | 72.85 | 87.92 |
| template CLI emission | 8 | 48.75 | 59.56 |
| enter, fresh-project first process | 8 | 51.88 | 64.18 |
| enter, repeated process reads | 100 | 51.64 | 58.70 |
| bounded handoff | 10 | 45.52 | 47.19 |
| enter with Work/Base | 10 | 51.79 | 55.43 |
| existing Context Pack | 10 | 47.21 | 52.40 |
| small fact write | 100 | 56.80 | 66.60 |

100 real enter reads changed no project/host file names or hashes: 0 durable bytes.
Fresh entry stdout was 1409 B, after 100 facts 3061 B, rich entry 4861 B;
Context 3773 B. All are below their unchanged bounds.

100 normal facts grew the canonical ledger by 125576 B, or 1255.76 B/event.
Evidence growth, Base new units/separate store and external host growth were zero.
There were 102 ledger events total (two initial binding/registration events plus
100 facts). A sampled fact appeared once, not repeated in projection/cache logs.
Final largest file was ledger.json, 128275 B; identity 310 B, lock-owner 89 B,
self-ignore 2 B and lock 0 B. No stored Context/handoff/source/binary payload.

Persisted metadata overhead versus 4380 B of fact text was 28.67x, reflecting IDs,
hash-chain/provenance/actor/idempotency metadata, not 28 copies of the fact. The
existing full-file JSON writer remains a physical write-amplification issue:
summing observed post-write ledger sizes estimates 6610117 B rewritten across
100 writes. This is an explicit model, not OS disk-I/O measurement. T5/T6 remain
OPEN/PARTIAL for appropriate future storage work; no canonical migration was
smuggled into this encapsulation phase.

### Requested final report

```text
ROUTE_BASIC_ENCAPSULATION_I: PASS (Windows controlled structural/profile scope)

BASE:
starting HEAD=01952e7ea111d137e270d42140efd8c5d7c9089e
branch=fruit
previous Base commit=7b2d1e98952e8c1bd53b0d13b11a056320e49568
previous Base push=PASS, ordinary fruit push; local/remote equality verified

PRODUCT CONTRACT:
required project-facing files=1 (ROUTE.md)
host-specific files=0 generated by basic attach
manual config fields=0

ROUTE_MD:
bytes=867
lines=22
compat marker=<!-- route:1 -->
dynamic state stored=NO
credentials stored=NO
idempotent=PASS, bytes and mtime unchanged
existing user file preserved=YES, unmanaged/modified/future/oversized preserved
existing active-repo ROUTE.md=5479 B, preexisting protected user file;
                              not silently replaced to satisfy a fresh-template limit

ENTRY:
command=route-ai enter --json
read-only=YES
max bytes=16384 including newline (stricter than 32768 ceiling)
actual bytes=1409 fresh / 3061 after 100 facts / 4861 rich fixture
sections=protocol/version, identity, worker, doorway, Goal/Work/blocked/check
         truth, recent bounded continuity, optional Base, omissions, suggestions
ATTACH_REQUIRED behavior=structured response, no identity/state/key/file creation

WINDOWS COMMAND:
system route.exe collision=CONFIRMED, System32 utility wins normal route lookup
chosen safe invocation=unique packaged route-ai.exe, explicit path or Human-installed CLI directory
system binary modified=NO

PACKAGE:
primary executables=1
file count=3
exe bytes before=31780352
exe bytes after=31867904
zip bytes before=11463295
zip bytes after=11490699
size delta=exe +0.2755%; ZIP +0.2391%; new facade/safety code, no new native imports

FRESH PROJECT:
tracked Route files=1 required/tracked-facing ROUTE.md; no Git index auto-stage
ignored Route files=5
project-local Route bytes=3100 ignored / 3967 including doorway
external local Route bytes=64, isolated existing sidecar-home key
source copies=0
host files=0 generated

100 READS:
durable growth=0 bytes, all project/host file hashes unchanged

100 NORMAL EVENTS:
ledger growth=125576 B / 1255.76 B per event
Evidence growth=0 B
Base growth=0 new units / 0 separate-store bytes
external growth=0 B
total amplification=28.67x stored metadata/fact bytes; sample content occurs once
physical rewrite model=6610117 B, existing whole-ledger rewrite; future T6 issue

GOLDEN PATH:
Human commands=cd project; route-ai attach (after installation)
manual config=0
Human context copies=0
ROUTE.md only discovery=PASS in canonical-template fixture parser/process tests
real AI session=NOT_RUN (current Codex CLI not logged in)

COPY TEST:
Project B inherited identity=NO, including nested child without Git
authority inherited=NO
ATTACH_REQUIRED=YES

CLONE TEST:
credential resurrection=NO
hidden authority=NO
local runtime state copied=NO

GRACEFUL ABSENCE:
Git usable=PASS
build usable=PASS, normal rustc build in fresh clone with no Route state
AI/editor usable=editor/files usable; real AI session NOT_RUN, not invented

SECURITY:
ROUTE.md grants authority=NO
auto download=NO
auto login=NO
secret leakage=NO in tested common-secret/credential fixtures; filter is not a universal classifier
key-material pattern scan=0 hits across 14 owned paths; fixture literals reviewed

PERFORMANCE:
attach p50/p95=72.85/87.92 ms
enter cold p50/p95=51.88/64.18 ms
enter warm p50/p95=51.64/58.70 ms
handoff p50/p95=45.52/47.19 ms
Context p50/p95=47.21/52.40 ms
network/model/background call=NONE in basic entry path

BACKLOG:
BasicEncapsulation=VERIFIED_FIXTURE
SingleProjectDoorway=VERIFIED_FIXTURE
MachineEntryFacade=VERIFIED_FIXTURE
MinimalPackage=VERIFIED_INTERNAL_PROFILE
CrossHost=NOT_RUN
ModelRouting=OPEN/DEFERRED
AIKnowledgeSynthesis=NOT_RUN
DAG=OPEN/DEFERRED

FULL_REGRESSION:
fmt=exit 0
route-core=exit 0 (25 unit + 1 doc)
doorway focused=exit 0 (2 tests)
entry/legacy lightweight interop=exit 0 (2 + 4 tests)
route-basic=exit 0 (482 passed / 16 ignored)
route-cli=exit 0 (97 passed / 5 ignored across selected targets)
workspace excluding route-pyo3=exit 0
docs=exit 0
PowerShell parse=12 root scripts / 0 errors, explicit UTF-8
git diff --check=exit 0
release build before/after=exit 0 / exit 0
package/PE imports/alias resolution=PASS
release 100-read/100-write measurement=PASS
source drift during regression/release acceptance=0 owned source paths
preexisting A paths changed=0 of 133

KNOWN_LIMITATIONS:
Real AI golden path/CrossHost/AIKnowledgeSynthesis NOT_RUN.
Existing user ROUTE.md is not auto-migrated; Human integration may be required.
Alias must be installed by Human or invoked by explicit path; no hidden PATH edit.
Existing Windows VC runtime/UCRT prerequisite unchanged; bare-machine portability NOT_RUN.
Existing notices retained, no new public licensing certification.
Ledger full replay/rewrite and global single-revision snapshot work remain.
Top revision describes handoff; optional current Base/binding use their existing read guarantees.
Latency is small-fixture process timing, not a large-project/RSS certification.

GIT:
phase commit=the feat(route) one-file entry commit containing this report
push=ordinary fruit push only after gates; resolved SHA in final response/local Git receipt
local_SHA/remote_SHA=verified by post-push fetch, not self-referential text in this commit
previous Base SHA=7b2d1e98952e8c1bd53b0d13b11a056320e49568

MAIN_MODIFIED:NO
TAG_CREATED:NO
RELEASE_CREATED:NO (internal ignored package is not a public/GitHub Release)
FORCE_PUSH:NO
YUICH_CHANGED:NO (original in-repo migration deletions preserved)
```

Ignored local receipts: `Release/local-tools/encapsulation-20261014/` contains
baseline-footprint.json, regression.json (source hashes and exit codes),
measurement.json (actual packaged-release data), measurement-debug.json (explicit
preflight, not the release result), package-review.json and final Git receipt.
Exact before/after package manifests are outside their three-file package folders
under Release/packages. They are local evidence, not required files shipped in a
project or package. Both release builds kept the existing safe profile and source
path remapping; no build cache or historical runtime data was deleted.
