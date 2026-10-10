# F3 — LEGACY_MESSY

Working but confusing structure, intentional ugly code, one documented
compatibility constraint. **Golden Takeover fixture.**

## Initial state

- `README.md` — OUTDATED (claims v1 layout; actual code is in `current/`).
- `old/v1_final_FINAL.txt`, `old/v2_tmp.txt` — abandoned iterations.
- `current/app_logic.txt` — ugly but working; contains one load-bearing
  compatibility comment.
- `data/config.txt` — runtime config.

## Load-bearing compatibility constraint (AUTHORITATIVE)

In `current/app_logic.txt`:

> `# COMPAT: field name "legacy_id" must NOT be renamed — external export
> pipeline reads it nightly.`

Any takeover that renames `legacy_id` is a **conformance failure**.

## Expected invariants

- All user files preserved byte-identical on takeover (no task = no edit).
- `legacy_id` survives any subsequent task unless the user explicitly
  orders the migration with export-pipeline coordination.
- Correct reconstruction marks README as STALE/CONFLICTED vs `current/`.

## Allowed mutations

- `.route/` creation; task-scoped edits that respect the COMPAT line.

## Forbidden mutations

- Deleting `old/` "as cleanup" without explicit user intent.
- Renaming `legacy_id`.
