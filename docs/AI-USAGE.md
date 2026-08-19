# AI Usage — How a Coding AI Uses Route

> **Host-neutral, short, executable.** Any Coding AI — without Yuich, without
> a particular vendor — can read this one file (plus `route --help` where
> needed) and take part in a Route-managed project.
>
> - **Canonical Route behavior** → [ROUTE.md](../ROUTE.md)
> - **Constraints (binding)** → [`constraints/`](../constraints/README.md)
> - **References (non-binding)** → [`references/`](../references/README.md)

---

## AI QUICK START

Copy and paste this block to any Coding AI:

> You are working in a project managed by **Route**, a persistent
> development-state, context, and verification layer that sits alongside Git.
>
> 1. **Detect** — run `route --version`. A project may carry a `.route/` or
>    `.route-basic/` marker.
> 2. **Inspect** — run `route status`. If there is no Route state yet, run
>    `route init` (this preserves existing files; it never rewrites them).
> 3. **Understand before changing** — read `constraints/` (binding, highest
>    priority) first, then relevant `references/` (non-binding). Load context
>    with `route context`.
> 4. **Bound the work** — run `route task start "<task>" --target generic`;
>    note the returned `SESSION_ID`.
> 5. **Small, adequate change** — prefer a PATCH when a patch suffices; preserve
>    existing user work; do not casually rewrite what works.
> 6. **Verify with real execution** — your claims are **not** evidence:
>    `route task exec SESSION_ID -- <command>` records system evidence;
>    `route task verify SESSION_ID` checks the proof chain.
> 7. **Record outcome** — `route task end --result success|failed SESSION_ID`,
>    then `route commit -m "summary"`.
> 8. **Recover safely** — on breakage, inspect KnownGood / savepoints
>    (`route log`, `route save list`, `route rollback <id>`) before any
>    destructive action.

---

## What Route is (one paragraph)

Route is a **local-first, session-based development-state and verification
layer** for AI-assisted work. It records what Git does not: intent, continuity,
tasks, evidence, KnownGood, checkpoints, recovery, and handoff context. It is
**not** a code agent, not a harness, not an LLM, and **not** a Git replacement
(`route commit` snapshots are Route state, not Git history). Route is
**host-independent**: it works with any coding AI and is valuable with or
without Yuich.

---

## Methods of use

- **CLI** — `route <command>` (this is the **universal baseline** below).
- **MCP** — `route-mcp` (config: `route mcp`).
- **Python** — `route-py` binding (`import route`; optional after building).
- **HTTP** — `route-http` REST API (optional).

Pick what the host supports; the CLI path below works everywhere.

---

## Core flow (14 steps, CLI)

```bash
# 1 Detect Route (is the binary available / what version)
route --version

# 2 Inspect project status (integrity, profile, sessions) — --json for machines
route status            # or: route status --json

# 3 Load relevant project context
route context           # assembled Effective Development Context
route context --task "<task>"   # task-scoped view (top-k references)

# 4 Read constraints (binding) — REQUIRED layer
#    constraints/  = what must be preserved / what must not be done

# 5 Read relevant references (non-binding) — informative only
#    references/   = may inform, may NOT command; never beats a constraint

# 6 Understand before changing
route log               # history
route check             # integrity ("Checked ... Status: OK")

# 7 Define a bounded task -> returns SESSION_ID
route task start "<task>" --target generic

# 8 Protect existing user changes (do not rewrite working user files)

# 9 Make the smallest adequate change (PATCH > REPAIR > REFACTOR > RESTRUCTURE)

# 10 Run actual verification (records system evidence)
route task exec SESSION_ID -- <command>   # e.g. route task exec S -- python -m pytest tests/
route task verify SESSION_ID              # VERIFIED / FAILED / INCOMPLETE

# 11 Record outcome
route task end --result success SESSION_ID    # or: --result failed
route commit -m "summary of what was done"

# 12 Update / checkpoint project state (savepoint + snapshot)
route save create      # in-project development savepoint
#    (optional) route archive save   # external save under ROUTE_HOME

# 13 Recover if necessary
route log                                # find a good snapshot
route rollback <SNAPSHOT_ID>             # step back in the project ledger
route save list / route save restore    # savepoint recall
#    (optional) route archive restore / route archive recover

# 14 Never treat your own completion claim as evidence
route task report SESSION_ID -o "<observations>"   # AgentFeedback, NOT proof
```

> **Session discipline:** a session must be **Active** to `exec` / `end`. Start
> one session at a time, or end the previous one
> (`route task end --result failed <id>`) before `task start` a new one.

---

## Verification discipline (evidence, not claims)

- `route task verify` accepts only **system evidence** produced by real
  execution (CheckPass, TestPass, Commit).
- AI self-reports via `route task report` are `AgentFeedback` — reasoning,
  never proof.
- If a Gate reports `INCOMPLETE`, run the missing check with `route task exec`
  and re-verify; do not "declare" success.
- `NOT_RUN` / `UNKNOWN` is an honest state; a fake `PASS` is not.

---

## Route in *other* projects

The Route repo itself uses Route, but **other projects do not copy Route's
source**. A normal project uses one of the real, supported surfaces:

- an **installed CLI** / binary (`route ...`), or
- the **MCP** server, or the **Python** binding, or
- **project-local state** written by Route under the project (`.route/`
  or `.route-basic/`).

You never need to vendor Route source into a target project.

---

## Rough command map (what to explore)

| Goal | Command |
|------|---------|
| Assemble context for an external AI | `route context`, `route apply generic\|claude\|codex` |
| Project rules & resources | `route constitution show`, `route protocol`, `route reference show`, `route workflow` |
| Bounded session | `route task start / status / exec / verify / end` |
| State & recovery | `route save`, `route rollback`, `route checkpoint`, `route archive save\|restore\|recover` |
| Health | `route check`, `route repair-plan`, `route health`, `route guardian` |
| Knowledge / handoff | `route memory`, `route brain`, `route handoff`, `route brief` |

Run `route --help` for the full list; `route <cmd> --help` for any command's
flags. (All commands above exist in v1.0 beta.)

---

## Limits of v1.0 beta

- Experimental and **off by default**: `evolve`, `emerge`.
- User Memory and some `ROUTE_HOME` shared store are protocol-defined but not
  fully surfaced.
- `task exec` spawns processes directly (pass binaries/args, a shell is not
  implied).
- Some archive/recovery paths write outside the project (ROUTE_HOME);
  availability depends on host permissions.

See [ROUTE.md](../ROUTE.md) for the canonical system description.