# 2026-08-19 — RouteHandle Host-Neutral Validation

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta
- **Goal**: Ensure Route's cooperation surface (Handle / DevelopmentIntent / Outcome / Evidence) is host-neutral — usable by Yuich, Claude/Codex-style agent hosts, custom harnesses, human controllers, and future systems.
- **Trigger**: Release closure requirement — Route must not require any specific Host's private schema.
- **Source branch/ref**: `fruit` `e30ce43` (Handle fields in `TOOL.json`); this session's `TOOL.json` edit (degradation_policy).
- **Development intent**: Yuich integration is a **reference integration**, never a **core dependency**.

## What Route understood

- A host-neutral surface means the schema must not contain Yuich-only required fields.
- `degradation_policy` wording was Yuich-specific ("Yuich falls back…") and had to become generic ("A hosting host falls back…").

## What changed

- `TOOL.json` `degradation_policy` made host-neutral.
- Verified `TOOL.json` Handle fields: `native_handle: true`, `handle_id: "handle:route"`, `handle_protocol_revision_min/max`, capabilities, standalone: true.

## What did not change

- Route core; the Handle contract surface itself.

## AI Worker role

Bounded edit under PathGate; no core change.

## Evidence

- `TOOL.json` diff (degradation_policy wording) reviewed.
- Ownership audit: no Yuich dependency in Route source.

## Tests

Manifest parse + availability probe (README/TOOL docs reflect host-neutral language).

## Failures

None.

## Outcome

RouteHandle host-neutral: YES.

## Status

CONFIRMED.

## Route learned

Host-specific fallback wording in public manifests is a boundary leak; keep it generic.

## Related commits

- fruit: `e30ce43`; this session's `TOOL.json` edit; sandbox: this record.
