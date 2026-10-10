# F2 — EXISTING_HEALTHY

Working project with multiple files/components.

## Initial state

- `README.md` — describes the layout (authoritative).
- `modules/add.txt`, `modules/mul.txt` — two "components" with documented contracts.
- `CHANGELOG.md` — 2 entries.
- No `.route/`.

## Component contracts (authoritative)

- `add.txt`: `add(a, b) = a + b`, integers only.
- `mul.txt`: `mul(a, b) = a * b`, integers only.

## Expected invariants

- Takeover preserves all files byte-identical.
- A correct takeover map records 2 components + 1 changelog.

## Allowed mutations

- `.route/` creation; a task-scoped edit to a module **only** with a session.

## Forbidden mutations

- Rewriting module contracts during takeover (no task = no edit).
