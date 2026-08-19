# 2026-08-19 — Windows `route` NAME_COLLISION Discovery

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta
- **Goal**: Make Route availability detection honest on Windows, where the OS already ships a `route` command.
- **Trigger**: A shell `route` match on Windows is the network-routing utility, not the Route dev system; a naive probe would report a false positive.
- **Source branch/ref**: `fruit` `e30ce43` (probe text in `TOOL.json` and `tool/route/__init__.py`).
- **Development intent**: Never let the OS NAME_COLLISION masquerade as proof of the Route dev system.

## What Route understood

- Availability must be decided from the manifest + the presence of the Route workspace root (`Cargo.toml`) in the tool directory — never from a shell `route` match.

## What changed

- `TOOL.json` `availability_probe` documents the NAME_COLLISION explicitly.
- `tool/route/__init__.py` `probe_availability()` implements the honest probe and sets `runtime_available: false` with a clear `runtime_error`.

## What did not change

- Route core; the probe is host-side discovery only.

## Evidence

- `TOOL.json` and `__init__.py` contents (quoted above).

## Tests

Manifest-based probe returns `source_present` from `Cargo.toml` presence, not from a shell match.

## Failures

None (this is a guard against a future false positive).

## Outcome

NAME_COLLISION documented and guarded.

## Status

CONFIRMED.

## Route learned

Windows tool-name collisions are real; availability probes must be structural, not shell-based.

## Related commits

- fruit: `e30ce43`; sandbox: this record.
