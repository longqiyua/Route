# Route Basic Delivery Optimization II

Internal engineering acceptance reference, 2026-10-14. This phase optimizes the
already verified Basic delivery; it adds no backend, model integration, autonomous
routing, daemon, database or Host-specific prompt file. Artifacts under `Release/`
are ignored internal evidence, not a public GitHub Release.

## Result

`ROUTE_BASIC_DELIVERY_OPTIMIZATION_II = PASS` for controlled Windows structural,
release-profile and CLI acceptance. The standard release executable decreased
from 31,867,904 to 16,917,504 bytes (-14,950,400; -46.91%). The three-file ZIP
decreased from 11,490,699 to 6,437,564 bytes (-5,053,135; -43.98%). Commands,
panic semantics, dependencies, native imports and package contents were retained.

The selected release profile is `opt-level="z"`, thin LTO, one codegen unit and
symbol stripping. Cargo explicitly documents that `s`/`z` need empirical comparison
and are not guaranteed to be smaller; this repository therefore records measured
candidates rather than assuming the outcome:
<https://doc.rust-lang.org/cargo/reference/profiles.html#opt-level>.

## True user entry and AI discovery

The entry contract remains one stable `ROUTE.md`, one `route-ai.exe`, and one
machine command: `route-ai enter --json`. The canonical template remains unchanged
at 867 UTF-8 bytes and 22 lines. It contains no Goal, Work, branch, timestamp,
credential, model, token, history, Base unit or generated Context. Existing user
`ROUTE.md` content is still preserved rather than rewritten for a size claim.

There are five distinct cases:

| Case | Verified behavior |
|---|---|
| Human asks an AI to read `ROUTE.md` | The one-screen instruction is documented; the file points to the canonical command. A paid/remote AI session was not run. |
| Host automatically discovers repository instructions | No actual Host was verified to auto-discover the neutral filename. Universal zero-prompt discovery is not advertised. |
| Host does not auto-discover `ROUTE.md` | Human says “读取 ROUTE.md，继续这个项目。” once. |
| Host has no file/shell capability | It cannot perform this integration; normal non-Route project work remains available. |
| `route-ai` is unavailable | No installation, PATH change, login or network call is triggered; ordinary Git/edit/build/test work continues. |

Host-specific discovery bridges remain optional future integration. Basic attach
does not create AGENTS.md, CLAUDE.md, GEMINI.md, `.cursor`, `.claude` or `.codex`.
Only `codex.exe` was present on PATH during this audit and `codex login status`
returned `Not logged in`; no credential was changed and no model credit was spent.
Therefore `REAL_AI_GOLDEN_PATH = NOT_RUN`, not a fixture-derived PASS.

## Entry and project footprint

The existing `route.enter/1` facade was retained without a second API. Fresh output
was 1,409 bytes; the retained rich fixture was 4,861 bytes. Both are valid JSON and
well below the strict 16,384-byte stdout boundary. Required Goal/Work truth is kept
before optional Base/narrative content. Overflow returns a valid `DETAIL_REQUIRED`
projection. Missing, copied or malformed state fails closed; reassociation is
explicit; independent review omits prior Base/findings/decisions/observations.
No secret, authority, AI invocation, network invocation or installation is emitted.

An isolated Git project produced these scoped sizes (Git metadata excluded):

| Stage | tracked doorway | ignored Route state | external key state | ledger |
|---|---:|---:|---:|---:|
| empty/template inspection | 0 B | 0 B | 0 B | 0 B |
| attach | 867 B | 3,081 B | 64 B for that project | 2,699 B |
| first/10/100 enter | 867 B | 3,081 B | unchanged | 2,699 B |
| first fact | 867 B | 4,334 B | unchanged | 3,952 B |
| 100 facts | 867 B | 128,661 B | unchanged | 128,279 B |

The measurement harness attached eight independent projects, so its aggregate Host
directory reached 512 B (8 x 64 B); that is not the external footprint of one project.
There is one required project-facing file, zero Host-specific files, no source copy,
no archive SQLite database, no vector store and no persisted Context cache.

Exactly 100 `enter`, 100 `status`, and 100 bounded `handoff` reads left all project
and external-key file hashes unchanged: durable read growth = 0 B.

## Binary and package audit

Toolchain: rustc 1.95.0 (`x86_64-pc-windows-msvc`, LLVM 22.1.2). Cargo.lock is
85,534 bytes, SHA-256
`C47692A4A4538D846DD0F396A1FE415E22CAABA3B05E8A610A9ED0FE2A0E02C1`.
All builds used `--locked` and the established repository/user path remapping.

| Candidate | executable | ZIP | decision |
|---|---:|---:|---|
| opt=3, thin LTO baseline | 31,867,904 B | 11,490,699 B | replaced after regression comparison |
| opt=s, thin LTO | 18,876,416 B | 7,198,467 B | valid, but larger than z |
| opt=z, thin LTO | 16,917,504 B | 6,437,563 B candidate / 6,437,564 B final rebuild | selected |
| opt=z, fat LTO | 17,651,712 B | 6,597,116 B | rejected: larger and slower build |

The one-byte candidate/final ZIP difference accompanies a separately compiled PE
hash; both archives use the same deterministic entries and timestamps. The final
standard-profile executable SHA-256 is
`8A22F770F2105E5A9CF367DB459A514E447DE9433878BE9E01F2F23010EEEF61`;
the final ZIP SHA-256 is
`2DBD0CB2023DA0123884D5A1B2A6B250A17A52D7DB394DFAE532757F3C3C8EED`.

Each package contains exactly `route-ai.exe`, `LICENSE`, and
`THIRD_PARTY_NOTICES.md`. Help output has the same 104 lines/hash across baseline,
`s`, and `z`. PE section inspection attributes most reduction to `.text`
(25,811,968 -> 12,199,424 raw bytes) and `.rdata`
(5,115,904 -> 3,918,336), not removal of package resources or commands.

The CLI dependency graph has 308 unique normal tree lines and deliberately links
the advanced Route capabilities into the single executable. Large families include
bundled SQLite, ZIP codecs, reqwest/TLS, TUI and Route's core/basic/sync/stats/
plugins/engine/memory crates. No dependency or feature was removed: notably the
currently duplicated reqwest TLS surface was not pruned because changing transport
semantics requires independent compatibility evidence. `cargo-bloat` was absent and
was not silently installed.

All candidates retained the same 20 native imports, including UCRT API sets and
`VCRUNTIME140.dll`. This proves no new native dependency in the compared builds;
it does not prove operation on a clean supported Windows image.
`CLEAN_WINDOWS_INSTALL = NOT_RUN`.

## Performance and storage

The final packaged release measured, with separate CLI processes:

- attach p50/p95: 63.50 / 94.70 ms (baseline run: 84.56 / 107.11 ms)
- first enter after attach p50/p95: 43.72 / 48.47 ms (baseline: 61.63 / 77.52 ms)
- warm enter p50/p95: 46.27 / 51.17 ms (baseline: 54.36 / 62.00 ms)
- 100 fact writes: 6,854.25 ms total; p50/p95 60.82 / 96.96 ms
- sampled child working set: 10,977,280 B maximum during the 100-read series

Timing is a bounded local process measurement, not a statistically controlled lab
benchmark. “Cold” means first process after attach/write; it does not flush the OS
page cache. RSS was sampled about every 2 ms and is a lower bound, not exact peak.
The selected profile shows no measured runtime regression requiring rejection.

For 100 facts, 4,384 payload bytes caused 125,580 B canonical ledger growth:
1,255.8 B/event and 28.65x payload-inclusive metadata growth. This ratio includes
event identity, provenance and hash-chain metadata; it is not 28 copies of content.
Summing each resulting ledger length gives 6,610,301 B estimated rewrite traffic,
not measured physical disk I/O. A separate 400-event bounded run produced 503,280 B
logical growth and 101,937,851 B cumulative rewrite estimate. The implementation
loads/parses and atomically rewrites the whole JSON ledger, so cumulative write work
grows quadratically even though logical ledger growth is linear. No storage format
migration was attempted in this release-delivery phase.

## Windows invocation and safety

Absolute invocation and a child-process-only PATH simulation resolved
`route-ai.exe` to the package in both PowerShell and `cmd.exe`; `route.exe` still
resolved to `C:\Windows\System32\ROUTE.EXE`. Its SHA-256 remained
`79A71EDB01E35DCAC272BFFC400383286C14B8090C61687603FC51AD7999E5C9`.
Current, user and machine PATH were not changed. No installer, automatic login,
credential mutation, model call or public Release was performed.

## Reproducible evidence and limits

The maintained bounded harness is `scripts/measure-basic-delivery.ps1`. Internal
receipts are under `Release/local-tools/delivery-ii-20261014/`; internal packages
are under `Release/packages/delivery-ii-*-20261014*`. Release is Git-ignored.
Historical acceptance reports were not rewritten.

Final regression passed: cargo fmt; route-core (25 unit + 1 doc); route-basic
(482 passed, 16 ignored); route-cli (97 passed, 5 ignored); and the full workspace
excluding route-pyo3. Ignored tests are recorded, not counted as PASS; route-pyo3
is explicitly excluded/not tested. Documentation links, all 13 maintained root
PowerShell scripts, `git diff --check`, final release execution/package/measurement,
secret-pattern scan and all 133 preserved pre-phase working paths also passed.

The follow-up canonical-ledger work is documented in
[DevelopmentEvent ledger storage I](ledger-storage-i.md). It introduces an
explicitly migrated framed journal without changing this delivery phase's binary,
package or ROUTE.md measurements; whole-history read/replay scaling remains open.
