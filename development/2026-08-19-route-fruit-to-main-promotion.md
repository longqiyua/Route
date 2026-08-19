# 2026-08-19 — Fruit → Main Promotion Process (Route Canonical Source to Repo Root)

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta
- **Goal**: Promote verified Route-owned changes from the `fruit` integration tree to a standalone `main` repository root — without dragging Yuich or private state along.
- **Trigger**: Release closure — main must be `MAIN_ROUTE_ONLY`.
- **Source branch/ref**: `fruit` `tool/route/**` → `main` repo root (old main @ `0ae87da`).

## What Route understood

- In the Route repository `main` branch, Route is the project root. In an embedding host, Route may live under `tool/route/`. Physical path is host context; Route identity is not.
- Promotion must extract only Route-owned deltas inside `tool/route/**` and map them to the main repo root. Yuich docs/state, Host-only adapters, private logs must not enter `main`.

## What changed

- `main` worktree: removed old-lineage paths (`crates/*` old, `archive/`, `PPAM/` submodule pointer, `document/`, `sandbox/` placeholder, old `Cargo.toml`/`Cargo.lock`).
- Copied the 161 verified Route files from `tool/route/` to the `main` repo root (excluding the host-embedding `tool/route/__init__.py`, which is Yuich-discovery specific).
- Rewrote `README.md` / `ROUTE.md` / `THIRD_PARTY_NOTICES.md` for the converged Route; kept `LICENSE` (AGPL-3.0).

## What did not change

- `tool/route` source (used as the promotion source).
- Yuich private state (never copied to main).

## AI Worker role

Performed the tree mapping under BranchGate (separate worktrees) + DiffGate (verified file lists) + PathGate (only `tool/route/**` extracted).

## Evidence

- `main` worktree `cargo check -p route-cli` — PASS (standalone build proof).
- File-list verification: 161 files copied; `tool/route/__init__.py` intentionally excluded.

## Tests

Standalone `cargo check` in the `main` worktree.

## Failures

None.

## Outcome

Main is Route-only; no Yuich leakage; main tree reflects the verified Route source.

## Status

CONFIRMED locally (pending human review before push).

## Route learned

Branch trees need not be identical; promotion maps host-relative paths (`tool/route/**`) to repo-root paths in the standalone view.

## Related commits

- fruit: `e30ce43`, `bbd0618`; main: closure commit (pending review); sandbox: this record.
