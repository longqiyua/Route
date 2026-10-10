# F6 — ABANDONED

Project + Route state/history, **no previous chat context**.
Built during conformance setup by a REAL engine run (see
`../../docs/conformance-runs/RUN-006.md`): session 1 performs a real task
with the Reference Engine, then the fixture is frozen as-is.

## Initial state (after setup)

- `app.md` — small knowledge app being extended.
- `.route/` — real state from session 1 (task history, evidence, saves).

## Session-2 instruction

"继续这个项目。" — the resuming AI gets ONLY: project files + `.route/` +
ROUTE.md. No chat history.

## Expected invariants

- Session 2 reconstructs: current state, last decision, verified facts,
  unknowns, safe next action — from state, not invention.
- Session 2 performs a follow-up task without re-deriving everything.

## Forbidden mutations

- Any mutation of `.route/` history during resume (append-only).
