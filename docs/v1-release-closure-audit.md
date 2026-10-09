# Route AI v1 release-candidate closure audit

Audit date: 2026-10-10. Feature freeze remained in effect. This is a **local
Windows CLI candidate audit**, not Human approval for a version, `main` merge,
tag, or public Release.

## Source, build, and package

The independently fetched clean `origin/fruit` checkout was
`3af408ab8969fc8914067f9f7a1ccb9606ca543b`; it had no `.route`,
`.route-basic`, Wiki dataset, Release archive, PPL checkout, private ISM
material, target cache, or local Yuich runtime files. The intended tracked
Yuich files were never included in the binary package. The Windows MSVC
toolchain was Cargo 1.95.0 / rustc 1.95.0. A clean-clone
`cargo build --release -p route-cli --locked` exited 0.

The first release binary passed operation smoke but contained absolute Cargo
source paths under the developer home. It was **rejected** and its locally
created staging package removed. The bounded
[`build-release-candidate.ps1`](../scripts/build-release-candidate.ps1) rebuild
used Rust path-prefix remapping in an isolated target; it exited 0. The
resulting `route.exe` is 27,198,976 bytes with SHA-256
`2218293170E0D2A752DB18C4677FB2585231081077FFB91D82BD61220A39A79E`.
It reports `route 1.0.0`. This second build used the same Route code as the
clean checkout plus only release documentation/script changes in the fruit
working tree; it is not a byte-for-byte reproducibility certification.

The local, unpublished ZIP at `Release/packages/route-v1-rc-3af408a-win-x64.zip`
is 10,043,319 bytes, SHA-256
`7617B29DC4B287F9112BE89DF4B07FC1B525B0D5A25B8724334F779BF0FF7449`.
Its nine files are the CLI binary, `LICENSE`, `SECURITY.md`,
`THIRD_PARTY_NOTICES.md`, and five selected operational documents. Archive
inspection found no repository metadata, local runtime state, Wiki/PPL/ISM,
Yuich files, target cache, migration archive, private key/token pattern, or
`C:\Users\...` path in the package. The only large file is the intended
binary. The ZIP was not uploaded or published.

Two independent disposable installs, one after copying the ZIP to another
directory, ran packaged `--version`/`--help`, `init`, `goal.create`,
`workflow.create`, `plan.create`, `plan.get`, `assistant.status`, a fresh-process
Goal read, and `workflow.complete.check`. Both correctly returned `DENIED`
for the incomplete required step. The moved binary hash matched the package
binary. These smoke projects were isolated from the developer checkout.

## Safety gates

From the clean remote checkout: `cargo fmt --all -- --check` exited 0;
`cargo test -p route-core --locked` had 22 unit + 1 doc pass;
`cargo test -p route-basic --locked` had 417 pass / 4 ignored;
`cargo test -p route-cli --locked` had 76 pass / 1 ignored, including Principal
Binding, autonomous Worker, Execution Contract, and General Work end-to-end
tests. `cargo test --workspace --exclude route-pyo3 --locked` exited 0 with
637 pass / 0 fail / 5 ignored across 36 test results. The docs-link check,
PowerShell script parser check, and `git diff --check` passed. Rust tests ran
outside the restrictive Windows filesystem sandbox to avoid its known
canonicalize denial; the commands and source were otherwise unchanged.

`route-pyo3` is experimental and **not shipped** in this Windows CLI package.
Archived Tauri/desktop code and experimental non-Windows targets are not
claimed as v1 deliverables. Reproducible build status: **NOT_CERTIFIED**;
the two release builds intentionally used different compiler path flags.

## Repository and version gates

See the [read-only main reconciliation plan](v1-main-reconciliation-plan.md).
`main` has divergent commits and preexisting dirty user work; no merge was
attempted. Product version is unresolved: Cargo/CLI `1.0.0`, `TOOL.json` and
Python package `1.0.0-beta`, historical docs V0.6 Beta/v1.0 beta, and legacy
npm packages `0.1.0`. Protocol `route/1` is independent and unchanged. The
new [candidate usage document](v1-release-candidate.md) and
[draft notes](v1-release-notes-draft.md) avoid declaring a public version.

The ignored `.route-basic` is local runtime state (about 16.0 GB across 2,723
files), not source or package input; no inner file was written by the earlier
accidental `init`. The ignored `.route` **was** appended by that command:
history sequence 107 records a successful `init`. It is preserved as
canonical local history, not reset or packaged. Neither state is a release
artifact or reason to delete user data.

## Evidence matrix and decision

| Gate | Result |
| --- | --- |
| Source integrity / fruit remote sync | PASS at source SHA above |
| Main reconciliation | REQUIRES_HUMAN; dirty and divergent |
| Version metadata | REQUIRES_HUMAN; product version unresolved |
| Fresh clone / full regression | PASS |
| Release build / checksums | PASS; reproducibility NOT_CERTIFIED |
| Package content / install / move / assistant golden path | PASS for local Windows CLI candidate |
| Candidate docs / known limits / security boundary | PASS for candidate; public version pending |
| Local runtime contamination | PASS; ignored and not packaged |

`FRUIT_V1_CANDIDATE = YES`; `PACKAGE_READY = YES` for local CLI acceptance;
`MAIN_MERGE_READY = NO`; `VERSION_READY = NO`; `TAG_READY = NO`;
`RELEASE_READY = NO`. Recommendation: **do not publish yet**. The next
separately authorized work is main/user-work reconciliation and a Human
product-version decision, followed by a new resolved-source gate before any
tag or public Release. No main modification, merge, tag, remote Release,
force-push, or Yuich change occurred in this phase.
