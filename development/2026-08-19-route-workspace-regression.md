# 2026-08-19 — Route Workspace Full Regression + route-py Build

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta
- **Goal**: Prove Route can stand alone: full workspace regression, CLI sanity, and a real `route-py` wheel build.
- **Trigger**: Release closure verification (S10/S11).
- **Source branch/ref**: `fruit` `tool/route` @ `e30ce43`.

## What Route understood

- Full regression must run every workspace member honestly; optional environment-dependent suites are marked `ENVIRONMENT_NOT_RUN`, never silently skipped.
- CLI `--version` should reflect the release version policy.

## What changed

Nothing in this verification step (verification only).

## Evidence

- `cargo test --workspace` — PASS, zero failures. `route-basic`: 323 tests PASS.
- CLI sanity — `route --help` lists the full public command set; `route --version` → `route 1.0.0`.
- `maturin build --release` (packages/route-py) — PASS; wheel `route_vc-0.4.0b0-cp310-abi3-win_amd64.whl` (at time of build, pre-version-bump) generated; `import route` smoke test PASS.
- Existing-project test — PASS (non-empty project: init → status → commit → log; original files preserved).

## Tests

As above, all REAL_OBSERVED on this machine (Windows, Python 3.11.15, maturin 1.14.1).

## Failures

None in the build/test matrix.

## Outcome

ROUTE_STANDALONE PASS; WORKSPACE_REGRESSION PASS; ROUTE_BASIC PASS; ROUTE_PY PASS; CLI_SANITY PASS; EXISTING_PROJECT PASS.

## Status

CONFIRMED.

## Route learned

A release closure needs one real, honest regression run; evidence labels (REAL_OBSERVED vs NOT_RUN) prevent overclaiming.

## Related commits

- fruit: `e30ce43`; sandbox: this record.
