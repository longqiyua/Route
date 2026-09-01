# Protocol Ambiguities (P17) — evidence-first registry

Rule: recorded ONLY with behavioral evidence; standardization is a separate,
later step. Nothing here was resolved "for aesthetic consistency".

## Substantiated this round (4)

### AMB-001 — `task end --result success` not gated on passing evidence

- Evidence: RUN-002, session 01M04EZC5M4P audit — only evidence is
  `system/check_fail` (exit_code=1), yet session status = **Succeeded**.
- Contrast: RUN-005 session 01M04F44AJQD correctly ended **Failed** on
  check_fail (when AI declares failure).
- Divergence: protocol says non-VERIFIED sessions must not be marked
  Succeeded; engine accepted an AI-declared success with zero passing
  evidence. AI-side honesty caught it; engine gate absent.
- Status: OPEN. Minimal fix direction (when user approves): verify-before-
  succeed gate already specified in the MCP contract — engine needs the same
  enforcement for CLI `task end`.

### AMB-002 — `route handoff` cross-project data pollution

- Evidence: F6 setup phase — `route handoff` described the Route repository's
  own architecture instead of the fixture project's state.
- Divergence: handoff doc pulled content from the wrong project scope.
- Status: OPEN. Reproduce before fixing (single observation; per Route
  learning rules one observation is not a rule).

### AMB-003 — `route check` does not cover protocol semantic domains

- Evidence: RUN-007 — `.route/execution/` and root `.route/protocol.md`
  deleted; `route check` still reports Status: OK.
- Divergence: integrity check validates the engine ledger (`.route-basic`)
  but not the protocol-documented `.route/` semantic tree, so "OK" can mask
  a non-conformant state.
- Status: OPEN.

### AMB-004 — memory write surface mismatch

- Evidence: `route memory add` → `unrecognized subcommand` during RUN-001;
  `route memory --help` (re-read this session) offers only
  show/history/refresh/apply/why/supersede/map — no direct write. Writes go
  through `route learn record` → proposals → `memory refresh/apply`.
- Divergence: protocol describes AI memory updates as first-class; engine
  deliberately routes them through proposals (consistent with "never
  auto-applied"), but the naming/surface difference forces AI trial-and-
  error. Documentation should name the canonical write path.
- Status: OPEN (docs-level clarification candidate; no behavior change
  proposed).

## Observed but records lost

The prior conversation (before context compaction) had accumulated a total
count of ~10 in-session divergences; details of the other ~6 were lost with
the conversation context and are NOT reconstructed here — inventing them
would violate the honesty rules. They will re-surface on re-runs; this file
is the authoritative registry going forward.

## Resolved by evidence this round

None. No protocol text was changed to paper over any finding above.
