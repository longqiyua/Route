# F8 — BOOMLESS

Same meaningful task as F4 (fix the inventory defect), with Boom
explicitly unavailable/disabled. Purpose: prove the absence-of-Boom path
is first-class. Note: Boom has NO implementation in any environment today
(see docs/boom.md §29 — protocol-only), so **every** real conformance run
is Boomless; this fixture exists to make that explicit and comparable
against RUN-003 (F4 with identical task).

## Initial state

Identical to F4: `spec.md` + `items.txt` (entry `glue` missing `id`).

## Expected invariants

- Full Intent → Plan → Execute → Evaluate → Save lifecycle works with
  zero Boom involvement.
- No degraded output versus a Boom-present environment may be claimed —
  Boom-present behavior is PROTOCOL_ONLY / NOT_RUN.

## Forbidden mutations

- Same as F4.
