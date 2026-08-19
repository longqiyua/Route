# 2026-08-19 — Route v1.0 beta — Constraint / Reference Boundary

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta (display: v1.0 beta)
- **Goal**: Formalize the boundary between binding (`constraints/`) and informative (`references/`) project material on `main`, relocate PPAM to a single canonical source under `constraints/`, unify the public version surface to `1.0.0-beta`, and prove Route standalone (no Yuich) against its own material.
- **Trigger**: User hardening instruction (ROUTE FIRST / MAIN IS THE PRODUCT / CONSTRAINTS x REFERENCES SEPARATION / PPAM AS CONSTRAINT MATERIAL / v1.0 beta).
- **Source branch/ref**: `main` worktree (`route-main-wt`) — prior HEAD `9287998`; this round added `23f1fb4`, `3e1d662`, `08ef5c5`.
- **Development intent**: Route must be understandable and runnable from `main` alone. Material separation must be typed (constraint vs reference) in the AI context, PPAM must live under `constraints/` as ONE canonical source, and every public version surface must say `1.0.0-beta`. No grand architecture, no new DB, no RAG/vector, no Agent framework, no Yuich dependency.

## What Route understood

- Constraint material may restrict a development decision; it is binding. Reference material may inform but may not command; it is data.
- Reference content — even text like `MUST` / `SYSTEM` / `IGNORE PREVIOUS` — never gains constraint tier. Constraint material is only that which Route classifies as CONSTRAINT.
- A constraint can restrict permitted actions but cannot create permission.
- PPAM core semantics must be preserved; directory relocation must not reinterpret PPAM.
- Product release version (`1.0.0-beta`) is decoupled from protocol / Handle / schema revisions.

## Constraint semantics (`constraints/`)

Binding material. Answers: what must be preserved, what must not be done, what rules govern development, what conditions a change must satisfy, what project-specific boundaries exist. Only genuine existing content was moved in — no rule forest invented to fill the directory.

## Reference semantics (`references/`)

Informative material. Answers: what information may help understand the project, what examples/papers/docs/patterns are relevant, what external material may inform a decision. Reference may inform, never command; it can never override a constraint.

## PPAM location

- Previous: PPAM lived under the integration tree / local install (`PPAM/`), outside `main` product layout.
- Now: canonical source at `main:constraints/ppam/` (22 files: 1_Configuration, 2_Request, 3_Prompt, Reference, INDEX, README, 版本规范, bundled vibe-flow skill, etc.). The `4_Expand` third-party layer stays out of `main` (external reference material).
- No duplicate PPAM source exists on `main`. Old paths in `main` were updated; `.gitignore` rule `PPAM/` re-anchored to `/PPAM/` so it no longer masks the canonical `constraints/ppam/`.

## Files changed (main)

- `Cargo.toml` + all workspace `crates/*/Cargo.toml` (incl. `route-ppam`): version `1.0.0-beta`.
- `ROUTE.md`: version table corrected to `1.0.0-beta` for Rust workspace and CLI.
- `README.md`: constraint/reference section (concise, product-oriented).
- `constraints/README.md`, `references/README.md`: semantics documents.
- `constraints/ppam/`: canonical PPAM content (22 files).
- `crates/route-basic/src/material.rs` (new): `MaterialKind` (CONSTRAINT|REFERENCE), `MaterialSource { path, kind, provenance, content_hash }`, `discover_material`, `render_constraints`, `render_references`, `is_doc_file` (only .md/.txt rendered; config-like text stays in discovery + fingerprint).
- `crates/route-basic/src/constitutive.rs`: `ContextSnapshot.material` + fingerprint inclusion; `build_context` renders `## Project Constraints` (binding, full text) and `## Optional References` (preview only).
- `crates/route-basic/src/lib.rs`: module export.
- `.gitignore`: `PPAM/` -> `/PPAM/`.

## Tests

- `cargo check --workspace` — PASS.
- `cargo test --workspace` — PASS (all members, incl. doc-tests).
- `cargo test -p route-basic material` — PASS (4 tests: location classification, empty-when-absent, non-text/internal-dir skipping, render constraint-content vs reference-preview).
- All REAL_OBSERVED on this machine; no fixture masquerading as real.

## Standalone evidence

- Fresh temp project, no Yuich: `route init` -> material files under `constraints/` + `references/` -> `route context` rendered `## Project Constraints` with full `constraints/rules.md` and `## Optional References` with preview of `references/notes.md` -> `route commit` -> `route log` — PASS.
- CLI `route --version` -> `route 1.0.0-beta`.
- `main` tree contains 0 Yuich files; PPAM found only under `constraints/ppam/`.

## Self-dogfood

Route ran against its own `main` material: `route context` classified all `constraints/ppam/*.md` as `## Project Constraints` (binding), no broken paths, no reference/constraint conflict. Version mismatch surfaced in `ROUTE.md` (workspace/CLI still `1.0.0`) and in `route-ppam` Cargo.toml (`0.5.0`) — both fixed (minimal fixes).

## Failures / Recovery

- First `main` commit silently omitted PPAM files: root `.gitignore` `PPAM/` matched `constraints/ppam/` because `core.ignorecase=true` on Windows. Recovered by re-anchoring to `/PPAM/` and adding a follow-up commit.
- PPAM bundled `constraints/ppam/.trae/skills/vibe-flow/SKILL.md` was masked by the `.trae/` IDE-ignore rule; force-added to preserve PPAM completeness.
- `route init` archive dir warning under the sandbox (global archive path outside sandbox) did not block init/commit/log in a temp project.

## Outcome

ROUTE_STANDALONE PASS, CONSTRAINTS/REFERENCES separated, PPAM canonical under `constraints/ppam/` (single source), version surface `1.0.0-beta` across workspace/CLI/route-py/TOOL.json/README/ROUTE.md, workspace tests PASS, self-dogfood PASS with minimal fixes, no Yuich dependency introduced.

## Status

CONFIRMED on `main` (commits `23f1fb4`, `3e1d662`, `08ef5c5`), pending push to `origin/main` (per user instruction).

## Route learned

- Directory-layout normative data must be checked against `core.ignorecase` behavior on Windows; case-insensitive gitignore can silently drop canonical material from commits.
- Version-surface drift hides in per-crate `Cargo.toml` files that don't inherit the workspace version — a release audit must enumerate every crate.
- Rendering only `.md`/`.txt` as constraint text keeps `constraints/` normative without turning it into `config/`.

## Unresolved

- Whether to `git tag` / publish a GitHub Release remains deferred to the user (no auto-tag this round).
- fruit integration path was not re-tested this round (fruit already embeds its own Route via `tool/route/`); promotion continues to require evidence and human review.

## Related commits

- main: `23f1fb4`, `3e1d662`, `08ef5c5` (this round); prior `9287998`.
- sandbox: this record.