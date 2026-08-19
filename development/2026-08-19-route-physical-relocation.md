# 2026-08-19 — Route Physical Relocation / Standalone Extraction

- **Date**: 2026-08-19 (work committed earlier; recorded now)
- **Route version**: 1.0.0-beta (at time of work: 0.4.0-beta)
- **Goal**: Move Route canonical source so it can be both a standalone repository and a host-embedded tool, without duplication or logical coupling.
- **Trigger**: Route was embedded inside an integration repo; it needed to remain a single canonical source usable standalone.
- **Source branch/ref**: `fruit` commit `e30ce43` "fruit: relocate Route canonical source to tool/route/ as standalone workspace (single source, no logical coupling)"; `bbd0618` "fruit: add tool/ container, Route bundled adapter, discover_bundled_tools(), enforcement tests, history entry".
- **Development intent**: Route must be physically relocatable. Physical path is host context; Route identity is not.

## What Route understood

- A canonical source can live at `tool/route/` inside an embedding host while the same tree is the project root of a standalone repo.
- `TOOL.json` `canonical_source: "."` means the manifest root itself is the source — no references outside `tool/route/`.
- Route must not depend on the host for its source layout.

## What changed

- Route workspace (`Cargo.toml`, `crates/`, `packages/route-py`, `.cargo/config.toml`, `Cargo.lock`, `TOOL.json`) relocated to `tool/route/` as a self-contained tree.
- `__init__.py` at `tool/route/` is a thin discovery reference for hosts (not a copy of Route internals).

## What did not change

- Route crate internals and semantics.
- Any Yuich import into Route code (ownership audit: zero).

## AI Worker role

Performed the physical move under PathGate and DiffGate; verified the relocated workspace still builds.

## Evidence

- `cargo test --workspace` on the relocated `tool/route` tree — PASS.
- `TOOL.json` `migration_status: CONFIRMED` with migration_note describing the standalone workspace.

## Tests

Workspace regression on the relocated tree.

## Failures

None in the move itself; later sandbox/init path issues were unrelated (see release-closure record).

## Outcome

Single canonical Route source; standalone extraction proven by the relocation commit + passing workspace regression.

## Status

CONFIRMED.

## Route learned

Physical location is host context. A canonical source must be relocatable by construction.

## Related commits

- fruit: `e30ce43`, `bbd0618`; main: this promotion lineage.
