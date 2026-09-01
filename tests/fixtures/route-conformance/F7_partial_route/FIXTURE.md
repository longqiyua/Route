# F7 — PARTIAL_ROUTE

Project contains incomplete/stale Route state.

## Initial state

- `README.md`, `src/main.txt` — small working project.
- `.route/` — created by a real `route init`, then DELIBERATELY staled
  during setup (a state file removed / config drift introduced — see
  setup notes in the run evidence). This staling is the fixture's purpose:
  these are synthetic test artifacts, not a real project's history.

## Expected invariants

- Takeover detects the partial/stale state honestly (UNKNOWN/STALE, or
  engine `check` findings) instead of pretending state is complete.
- No fabricated history to fill gaps.
- User files preserved.

## Allowed mutations

- `.route/` repair via explicit task; recording findings.

## Forbidden mutations

- Inventing task history that never existed.
