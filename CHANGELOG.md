# Changelog

All notable changes are user-visible unless noted. This project follows
[Semantic Versioning](https://semver.org/).

## V0.6 Beta — 2026-08-15

Productization and documentation freeze. No new core features were added in
this milestone; the goal was to make Route understandable, usable, and
publishable. The codebase remains at workspace version `1.0.0`; the
version-metadata mismatch is deferred to the final Release Gate.

### Added

- `ROUTE.md` — a single canonical guide for humans and agents (what Route is,
  mental model, safety invariants, CLI/document index).
- `docs/` — a documentation set: `concepts`, `architecture`, `recovery`,
  `evolution`, `harness`, `references`, `quickstart`, `cli`, `status`.
- `examples/basic/` — minimal real on-disk formats for Constitution, Protocol,
  and a Reference entry that Route can actually parse.
- `ROADMAP.md` — short-term roadmap only.
- `SECURITY.md` — security model and vulnerability reporting.
- `scripts/check-docs.ps1` — lightweight doc validation (links, examples,
  CLI-command existence).

### Changed

- `README.md` rewritten for first-time visitors: one-liner, why, 30-second
  mental model, key capabilities, quickstart, harness compatibility, status.
- Terminology is now consistent across the repo (see the Glossary in
  `ROUTE.md`): save/snapshot/archive, rollback/recovery/restore,
  stable/known-good/original, agent/harness/model.
- Experimental features (`evolve`, `emerge`, campaign) are now explicitly
  labeled `EXPERIMENTAL` and documented as off by default.
- `AGENTS.md` / `CLAUDE.md` are re-affirmed as generated working context
  (`route apply` output), not documentation; `ROUTE.md` is the canonical
  source and they must not drift from it.

### Safety

- The recovery model is documented with its real limits: file-granularity
  restore, unbounded archive growth (no auto-GC), Windows-primary platform.
- `archive save/list` is described as a save mechanism, not as a guarantee of
  "disaster recovery" — the `recover` flow is `Beta` and gated.
- AI self-reports are explicitly not trusted as verification evidence; only
  System evidence (TestPass/Commit/CheckPass) is accepted.

### Experimental

- `route evolve` — candidate-first, gated self-improvement. Off by default.
- `route emerge` — hardening layer on top of evolution. Off by default.
- `route evolve campaign` — experiment-set governance with budgets.
- `route-tui`, `route-pyo3` — experimental interfaces.

### Known Limitations

- Selective restore is at file granularity.
- Archive storage grows with each save; there is no automatic retention/GC.
- Windows is the primary platform; Linux/macOS are experimental.
- `unknown`-typed Reference entries are never exported until upgraded.
- The `deepseek` / `generic` apply targets are `Beta`; sub-agent/MCP/skill
  behavior is host-dependent.
- Cross-platform and non-core surfaces (HTTP, sync, plugins, packages) are
  `Partial`.

---

## 1.0.0 — 2026-08-14

First stable release (pre-productization).

### Core Features

- **Project Initialization**: `route init` creates `.route/` with config,
  profiles, and auto-creates Original archive save in `Documents/Route/`.
- **Task Lifecycle**: `route task start/exec/verify/end` — session-based AI
  task management with evidence capture.
- **Snapshots & Rollback**: `route commit`, `route rollback`.
- **Archive Save & Restore**: `route archive save/list/show/restore/recover`.
- **AI-Assisted Repair**: `RecoveryEngine` with selective restore, auto-verify,
  and RepairAttempt/Succeeded/Failed events.
- **Constitution / Protocol / Reference**: three-layer context system.
- **Context Integration**: `route apply --target <claude|codex|generic>`.
- **Study / Learning / Memory / Guardian / Maintain / Trajectory**.
- **Verification**: System evidence is the only trusted verification source.

### Safety

- Restore and repair operations default to preview first.
- Original archive save is immutable.
- `latest_verified` only accepts System evidence.
- Auto-save before destructive operations.
- Atomic file writes with hash verification.
- Local-first: all data stored on the local machine.

### CLI

Core commands: `init`, `status`, `task`, `archive`, `commit`, `rollback`,
`apply`, `learn`, `study`, `memory`, `guardian`, `maintain`, `why`.

Advanced commands: `constitution`, `protocol`, `reference`, `workflow`,
`profile`, `strategy`, `context`, `check`, `diff`, `health`, `discover`,
`idea`, `failure`, `principle`, `pattern`, `capability`.

### Packaging

- Workspace version unified to 1.0.0.
- Binary: `route` (CLI), `route-mcp` (MCP server), `route-tui` (terminal UI).
- Python bindings in `route-pyo3` (experimental).
- Windows as primary platform.