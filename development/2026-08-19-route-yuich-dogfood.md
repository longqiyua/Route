# 2026-08-19 — Route × Yuich Dogfood (Reference)

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta
- **Goal**: Preserve the Yuich×Route cooperation proof as a **reference integration**, not a release dependency.
- **Trigger**: Release closure — record what actually happened without letting it gate the release.
- **Source branch/ref**: `fruit`; Yuich side `yuich/mrs.py`; Route side `tool/route/`.
- **Development intent**: Yuich is a Host / dogfood participant / Route user / feedback source. Route learns only development-side experience (intent clarity, context sufficiency, worker scope, verification, Host cooperation) — never Yuich's private SubjectHistory.

## What happened (real facts)

- Yuich's development loop was upgraded to a continuous self-host dogfood system (first automated inspection P31/P32).
- The first real dogfood run passed all verification legs: Yuich full regression, Capacity regression, **Route workspace tests (`cargo test --workspace`)**, and **route-py native build (`maturin build --release`)**.
- End-to-end `dev` session flow validated: start → update → inspect → verify → finish → status.

## What changed

Yuich side only (`yuich/mrs.py`, docs). Route side unchanged by this integration.

## Evidence

- Dogfood verification legs passed (documented in `docs/yuich.md` session records).

## Outcome

Route × Yuich dogfood referenced: YES. Route is valuable without Yuich; Yuich may make better use of Route.

## Status

REFERENCE_INTEGRATION (not a core dependency).

## Route learned

Dogfooding Route from a Host is a legitimate feedback source; it must not leak Host-private state into Route.

## Related commits

- fruit: `e30ce43`, `bbd0618`; sandbox: this record.
