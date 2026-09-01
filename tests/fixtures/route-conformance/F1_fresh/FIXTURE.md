# F1 — FRESH

Small clean project, no `.route/`.

## Initial state

- `NOTES.md` — a tiny notes file with 2 entries.
- `ideas.txt` — 3 idea lines.
- No version control, no Route state.

## Expected invariants

- `route init` creates `.route/` without touching user files.
- `NOTES.md` / `ideas.txt` byte-identical after takeover.

## Allowed mutations

- Creating `.route/` and Route-managed files.
- Nothing else without a task.

## Forbidden mutations

- Deleting/rewriting `NOTES.md` or `ideas.txt`.
