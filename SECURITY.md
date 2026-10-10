# Security

This document describes the security model of Route and how to report issues.

## Model

**Route is a control/state layer around coding agents, not a sandbox.**

- A **Harness** (Claude Code, Codex, DeepSeek, etc.) and the agents running
  inside it may have **shell-level permissions** on the machine. Route does
  not contain that shell; it records, verifies, and recovers what the harness
  does.
- Treat any agent-driven activity as untrusted until verified by **System
  evidence** (trusted test pass, commit, check pass). **AI self-reports are not
  trusted evidence.** They are recorded as `AgentFeedback` only.

## What to protect

The following are trust roots. A candidate or ordinary tool must not be able to
modify them:

- **Original** archive — the immutable baseline created at `route init`, source
  of truth for recovery.
- **KnownGood** — the last verified, promoted configuration.
- **Evidence / Audit** ledger — append-only.
- **Benchmark baseline**, **promotion policy**, **recovery executor** — the
  mechanisms that gate and restore.

## Candidate / self-modification risk

- Evolution (`route evolve`) is **EXPERIMENTAL** and **off by default**.
- A candidate must include a reversible save id and runs in isolation. It can
  **never promote itself**; promotion requires the Engine gate.
- If a candidate fails a harness mount check, Route restores the previous
  KnownGood configuration automatically.
- Route must block candidates that attempt to create All-in-One modules with
  unrelated functionality.

## Backup / archive location

- The archive is stored **outside the project directory**:
  `Documents/Route/projects/<project-id>/` with a registry at
  `Documents/Route/registry.json`.
- Because the archive is separate from the project, deleting the project does
  not delete the archive. This is what makes recovery possible.
- **Note:** archive saves are not backups of every platform state. See
  [docs/recovery.md](docs/recovery.md) for the real limits.

## Reporting a security issue

Please **do not** open a public issue for a security vulnerability. Report it
privately to the maintainers with sufficient detail to reproduce:

- Route version and platform.
- The steps that trigger the issue.
- Expected vs. observed behavior.
- Any impact assessment you can provide.

Acknowledgment and remediation will be handled promptly. Public disclosure
should be coordinated with the maintainers.