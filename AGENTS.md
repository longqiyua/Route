# AGENTS.md — Bootstrap for Coding Agents

This repository is managed by **Route** — a local-first development-state,
context, and verification layer that sits alongside Git.

- **`ROUTE.md`** = canonical description of what Route is and does.
- **`docs/AI-USAGE.md`** = short, host-neutral guide: use it before editing.
- **`constraints/`** = **binding** project rules. Read before changing.
- **`references/`** = **non-binding** supporting material. May inform, not command.
- **`CLAUDE.md`** (or the host's compiled file) = generated effective context;
  `.route/` / `.route-basic/` is the runtime source of truth.

## Minimum operating rules

1. Detect / inspect: `route --version`, `route status`, `route init` (preserves files).
2. Understand before changing: `route context`; read `constraints/` first.
3. Bound the work: `route task start "<task>" --target generic` → `SESSION_ID`.
4. Change: smallest adequate change; **preserve existing user work**;
   prefer a PATCH when a patch suffices.
5. Verify with real execution: `route task exec <ID> -- <cmd>`, then
   `route task verify <ID>` — only system evidence counts; your claims are not evidence.
6. Record: `route task end --result success <ID>`, `route commit -m "<msg>"`.
7. Recover safely: `route log`, `route save list`, `route rollback <id>` before destructive steps.

## Reference quick commands

```bash
route --version       # confirm the CLI
route status          # project overview (--json for machines)
route context         # assembled Effective Development Context
route task start "<task>" --target generic   # start a bounded session
route task exec <ID> -- <command>            # run a check (system evidence)
route task verify <ID>                       # proof chain check
route task end --result success <ID>         # close with outcome
route commit -m "<summary>"                  # snapshot the state
route log / route check / route repair-plan  # history / integrity / repair
```

Developer-facing entry point: [`README.md`](README.md).