# Route v1 main reconciliation plan — read-only audit

This is a plan, **not a merge approval or merge result**. `fruit` at
`3af408ab8969fc8914067f9f7a1ccb9606ca543b` and local `main` at
`1c095ce5aae513f0e02c5d6f347427e3851d988e` share merge-base
`0ae87da0e17c853f7d149ecfe49c3fa7fd8ef485`. `origin/main` is
`08ef5c502fb0645310f453844b36cae458600cc8`. Relative to `fruit`,
`main` has five side commits and `fruit` has sixteen. `main` is also one
commit ahead of `origin/main`.

## Decision categories

| Category | Result | Reason |
| --- | --- | --- |
| CLEAN_FAST_FORWARD_POSSIBLE | NO | Both branches have unique commits. |
| CLEAN_MERGE_POSSIBLE | NO | A read-only trivial merge preview reports textual conflicts. |
| SEMANTIC_REVIEW_REQUIRED | YES | Route Rust source moved to `tool/route`; main has distinct beta/release and uncommitted changes. |
| BLOCKED_BY_MAIN_DIRTY_STATE | YES | Main's user work must be preserved, not absorbed silently. |

The five main-only commits contain beta release closure/hardening, canonical
PPAM and bundled skill work, and public-usage/release-surface documentation.
The sixteen fruit-only commits contain workspace relocation, PatchBench,
storage/governance, and the bounded v1 assistant progression. A read-only
trivial merge preview identifies `.gitignore`, `AGENTS.md`, `README.md`,
`ROUTE.md`, and `THIRD_PARTY_NOTICES.md` as textual conflict sites. This
preview is not a full semantic merge analysis. Old root `crates/` paths versus
fruit's `tool/route/crates/` make automatic file matching especially risky.

The separate main worktree was already dirty before this phase: `Cargo.lock`,
`ROUTE.md`, `crates/route-basic/src/game_save.rs`,
`crates/route-basic/src/lib.rs`, `crates/route-cli/Cargo.toml`,
`crates/route-cli/src/commands.rs`, `crates/route-cli/src/main.rs`,
`crates/route-tui/Cargo.toml`, and `crates/route-tui/src/main.rs` are modified;
`crates/route-basic/src/backup.rs`, `self_archive.rs`, `sop.rs`, and
`crates/route-tui/src/lib.rs` are untracked. Of those 13 files, only
`self_archive.rs` matched the corresponding fruit file byte-for-byte in this
audit. Difference alone does not prove a feature is absent or should be
ported. None of this main work was changed, staged, or cleaned here.

Version/release files need explicit Human review: Cargo/CLI report `1.0.0`,
`TOOL.json` and the Python package say `1.0.0-beta`, the old README display
said v1.0 beta, `docs/status.md` says V0.6 Beta, and npm package metadata says
`0.1.0`. The protocol remains `route/1` independently of product version.

## Safe next step (requires separate authorization)

1. Preserve and identify each existing main working-tree change, including
   untracked files, without reset/checkout/stash-and-forget.
2. Decide the public product version and which components are shipped.
3. Reconcile main-only changes against fruit's relocated equivalents in an
   isolated integration worktree; review each semantic conflict and version
   claim. Do not copy the whole main tree over fruit.
4. Run full regression, release build/package audit, and install/move smoke
   from the resolved commit, then obtain Human merge/tag/publication approval.

`MAIN_MERGE_READY = NO`. No merge, tag, or Release was created by this audit.
