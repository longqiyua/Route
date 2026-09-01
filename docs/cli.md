# CLI Reference

This lists the commands that exist in the current V0.6 Beta parser. Run
`route <command> --help` for the authoritative, up-to-date flags.

## Project & State

| Command | Purpose |
|---------|---------|
| `route init [--path <dir>] [--scan]` | Initialize a Route project (creates `.route/` + Original save) |
| `route status [--json]` | Project overview — one screen |
| `route commit -m <msg> [--author] [--full] [--branch]` | Commit current state as a snapshot |
| `route log [-l <n>] [-b <branch>]` | Show commit history |
| `route rollback <snapshot_id> [-r <reason>]` | Rollback to a snapshot |
| `route diff <from> <to>` | Compare two snapshots |
| `route changes` | Show working-directory changes |
| `route undo` | Step back one snapshot |
| `route redo` | Step forward one snapshot |
| `route checkpoint --title <t> [--body <b>]` | Create a named checkpoint |
| `route export [-f json\|markdown\|mermaid\|emacs] [-o <file>]` | Export repository data |
| `route stats` | Repository statistics |
| `route stats-report [-f json\|markdown] [--top] [--hourly] [--daily]` | Detailed stats report |

## Host-neutral RPC

`route rpc` accepts one `route/1` JSON request on stdin and writes one JSON
response to stdout. `route rpc --jsonl` keeps the same process open and handles
one request/response per line. Mutating methods require an idempotency key.

| Method family | Methods |
|---|---|
| Shared state | `development.state`, `development.events.query` |
| Development events | `development.event.record` |
| Workers | `worker.list`, `worker.register`, `worker.presence.update`, `worker.message.send` |
| Existing Route domains | `intent.*`, `evidence.*`, `history.query`, `checkpoint.create`, `recovery.status` |

See [route-cooperation-protocol.md](route-cooperation-protocol.md) for the
envelopes, event types, trust boundary, and complete capability list.

## Branching / Tags / Annotations

| Command | Purpose |
|---------|---------|
| `route branch create\|switch\|list\|delete` | Branch management |
| `route tag list\|create\|delete` | Tag management |
| `route annotate <commit_id> <text>` | Annotate a commit edge |
| `route annotations <commit_id>` | List annotations on a commit |

## Data / IO

| Command | Purpose |
|---------|---------|
| `route sync add\|remove\|list\|show\|enable\|disable\|run\|scheduler` | Sync targets (mirror/backup/archive) |
| `route tracking add\|remove\|list\|sync\|history` | Active tracking targets |
| `route plugin list\|show\|install\|remove\|enable\|disable` | Plugins (logger, webhook) |
| `route backup <target>` | Full backup of current HEAD to an external folder |

## Git Bridge

`route git ...` drives the user's own git binary: status, log, commit, branch,
remote, fetch, pull, push, diff, stage, stash, tag, config, revert,
cherry-pick, rebase, clean, show, archive, clone, merge, backup, restore,
set-mode, push-u, rebase-in-progress, backups, and more.

## Governance

| Command | Purpose |
|---------|---------|
| `route constitution show\|init\|path` | Manage the Constitution |
| `route protocol show\|init\|path` | Manage the Protocol |
| `route reference list\|show\|add\|remove\|import\|enable\|disable\|review\|apply` | Reference registry |
| `route workflow list\|show\|import\|enable\|disable\|create\|propose\|apply\|reset\|evolve` | Workflows |
| `route profile list\|use\|show` | Project profiles |
| `route context ...` | Effective Development Context and its history |
| `route apply --target <claude\|codex\|deepseek\|generic> [--task] [--status] [--verify]` | Compile context into a host file |
| `route check [--json] [--full]` | Verify repository integrity |
| `route repair-plan [--json]` | Generate a repair plan from check findings |

## Tasks & Sessions

| Command | Purpose |
|---------|---------|
| `route task begin "<task>" [--target <host>]` | Begin a session |
| `route task start "<task>" [--target <host>]` | One-command session start |
| `route task status [<session-id>]` | Session status / list |
| `route task exec <session-id> -- <command>` | Execute a command under the session |
| `route task verify <session-id>` | Verify against the Protocol policy |
| `route task end <session-id> --result <success\|failed\|aborted> [--force]` | End a session |
| `route task show <session-id>` | Detailed audit |
| `route task resume <session-id>` | Resume after restart (checks drift) |
| `route task report <session-id>` | Submit an execution report (AI feedback) |
| `route task replay <session-id>` | Replay the context active at session start |
| `route plan` | Preview a task plan |
| `route agent-plan` | Generate an agent plan |
| `route impact` | Analyze the impact of a proposed change |

## Knowledge

| Command | Purpose |
|---------|---------|
| `route memory show\|history\|refresh\|apply\|why\|supersede\|map` | Project memory |
| `route brain` | Consolidated knowledge (derived view, not SSOT) |
| `route strategy record\|list\|show\|diff\|restore\|history\|fork\|use\|compare\|delete\|stats\|suggest` | Strategy snapshots |
| `route experiment history\|show` | Strategy experiments |
| `route agent-org history\|explain\|recall` | Agent organization memory |
| `route pattern list\|show\|propose\|apply\|proposals` | Reusable patterns |
| `route capability list\|show\|inspect` | Capability registry |
| `route discover <path>` | Discover capabilities in a path |
| `route pack create\|list\|show\|use\|deactivate\|export\|import\|delete` | Capability packs |
| `route idea add\|list\|show\|accept\|reject\|implement\|delete` | Idea/decision inbox |
| `route failure add\|list\|show\|search\|resolve\|delete` | Failure library |
| `route principle list\|show\|apply\|promote` | Principle candidates |
| `route agents` | Agent role templates |
| `route trajectory list\|show\|diff\|analyze` | Development trajectories |
| `route roadmap` | Project roadmap |
| `route goal` | Project goals |

## Recovery

| Command | Purpose |
|---------|---------|
| `route archive init` | Initialize archive + Original |
| `route archive save "<reason>"` | Manual external save |
| `route archive list [--json]` | List saves |
| `route archive show <id> [--json]` | Save details |
| `route archive diff <a> <b>` | Compare saves |
| `route archive restore <id> --scope <full\|project\|route-state\|paths> [--paths ...]` | Restore from a save (preview first) |
| `route archive recover <project-id> --to <path> [--save <id>]` | Recover a deleted project |
| `route archive recover-list <project-id>` | Recovery options for a deleted project |
| `route archive check` | Check archive invariants |
| `route archive path-history <path>` | File change history |
| `route archive delete <id> [--force]` | Delete a save (requires confirmation) |
| `route archive delete-project` | Delete entire project archive (requires confirmation) |
| `route save create\|list\|show\|preview\|restore\|diff\|delete` | Development savepoints |

## Health & Maintenance

| Command | Purpose |
|---------|---------|
| `route health` | Project health snapshot |
| `route health-history` | Health history |
| `route guardian scan` | Detect issues |
| `route maintain [--list] [--show <id>] [--plan] [--start <id>] [--check] [--target <host>]` | Maintenance plan |
| `route loops` | Detect open loops |
| `route drift` | Detect drift / decay |
| `route next` | Next action proposals |

## AI Bridge

| Command | Purpose |
|---------|---------|
| `route ai chat <msg>` / `route ai config` | AI operations |
| `route project-context` | Project context for AI injection |
| `route mcp --config` | MCP configuration |
| `route permission status\|set <normal\|high>` | Permission level |
| `route base ...` | Route Base operations (context, guard, status) |
| `route conversation list\|new\|show\|record\|rollback\|archive\|delete` | Conversation tracking |
| `route extension ...` | Skills / references for AI pre-injection |

## Study & Learning

| Command | Purpose |
|---------|---------|
| `route study <path>` | Analyze a project, generate candidates |
| `route study-apply <index>` | Register a study candidate as a Reference |
| `route learn record\|analyze\|review\|apply\|reject\|history\|why` | Experience-driven learning |
| `route self-improve` | Study Route itself (proposals only, no auto-apply) |

## Evolution (EXPERIMENTAL)

| Command | Purpose |
|---------|---------|
| `route evolve propose\|show\|evaluate\|promote\|reject\|history\|explain` | Verified evolution loop |
| `route evolve campaign create\|status\|next\|report` | Campaign governance |
| `route emerge enable\|status\|event\|run\|court\|novelty` | Emergence hardening (EXPERIMENTAL) |

## Docs / Handoff

| Command | Purpose |
|---------|---------|
| `route brief [--task <t>]` | Project brief |
| `route handoff` | Handoff document for a new AI session |
| `route curator` | Curator role capabilities |

> **Note:** flags shown are the common ones. Some commands accept additional
> flags; always check `route <command> --help`. This list reflects the current
> parser and does not invent commands.
