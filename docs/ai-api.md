# Route AI API

Route is a version manager for Web Coding projects. It does **not** ship
its own AI — there is no model, no chat, no token budget, no inference
inside the binary. The reason is deliberate: every project has its own
preferred model, and every team has its own policy on which models may
touch their code. Pinning one would be the wrong default.

Instead, Route exposes its version-management primitives through three
identical-shaped interfaces. Any of them can be driven by any AI
agent — a hosted model, a local llama, a "vibecoding" pipeline, the
user's own scripted agent. Pick the one that fits your stack.

| Interface | When to use it |
|---|---|
| **MCP** (`route-mcp` binary, stdio) | Your AI is an MCP-compatible client. This is the most direct path for Claude Desktop, Cursor, Continue, and similar tools. |
| **CLI** (`route ...` subcommands) | Your AI shells out to subprocesses (ReAct-style agents, shell-driving LLMs, CI bots). Robust, no extra daemon. |
| **HTTP** (Tauri webview port-forwarded, or future dedicated server) | Your AI speaks HTTP and runs in a process that can't see the user's local filesystem or shell. |

All three interfaces return the same data. They differ only in framing
(JSON-RPC, argv, JSON over HTTP). Choose the framing that your model
already speaks — the AI's job is the same in all three: call
`route_status` first, call `route_changes` before each `route_commit`,
and never lose a turn.

## What the AI is *allowed* to do

Route treats the AI as a *first-class operator* on the timeline. Every
commit an AI makes is recorded with `is_ai=1` and the agent's name in
the `author` field, so the user can read back months later exactly
which turns were theirs and which were the AI's. The AI never has
silent authority — it can commit, branch, checkpoint, rollback, and
annotate, but it cannot delete history. The user always has the
`route_undo` / `route_rollback` escape hatch.

## The shared mental model

A Route project has:

- one or more **branches** (`main`, `inherited`, or `sandbox`)
- a directed acyclic **commit graph** where each commit is an edge
  between two **snapshot** nodes (the snapshot is a pure file state;
  the commit is the metadata — message, author, kind, optional body)
- a **working directory** that the AI edits, and a **head snapshot** on
  the current branch that the working directory is compared against

A typical AI session looks like:

```
route_status        → "you're on branch X, head is abc123, working dir has 2 changed files"
route_changes       → confirms what would be committed
... AI edits files ...
route_commit        → records the change as a new commit edge
```

If the AI is about to do something risky:

```
route_checkpoint    → "before refactor" — user can roll back from the UI
... AI makes the risky change ...
route_commit        → records the change
```

If the AI is wrong, it can fix itself:

```
route_undo          → step back one head-advancing commit
route_redo          → step forward again
route_rollback      → jump back to any prior snapshot (recorded as its own commit, also undoable)
```

---

## 1. MCP (Model Context Protocol)

The `route-mcp` binary speaks JSON-RPC 2.0 over stdio. One JSON object
per line in, one per line out. The full MCP spec is at
<https://modelcontextprotocol.io>; this server implements only the
`tools` capability, which is all an AI needs to use Route as a tool.

### Wiring it up

**Claude Desktop** — add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "route": {
      "command": "route-mcp",
      "args": []
    }
  }
}
```

The working directory Claude runs in is the directory Route opens as a
repository. Tell the user to `cd` into their project folder before
starting Claude, or use `claude --cwd <path>`.

**Cursor / Continue / other MCP clients** — same idea, point the
client at the `route-mcp` binary and make sure the process's
`cwd` is a Route project (or a subfolder of one).

### Tool list (15 tools)

| Tool | Purpose |
|---|---|
| `route_status` | Current branch, head snapshot, branch list, mode |
| `route_log` | Commit history (newest first, filterable by branch) |
| `route_commit` | Record a new commit (message required) |
| `route_rollback` | Jump back to a prior snapshot (also recorded) |
| `route_undo` | Step back one head-advancing commit |
| `route_redo` | Step forward again |
| `route_checkpoint` | Mark a state the user can roll back to from the UI |
| `route_branches` | List all branches |
| `route_branch_create` | Create a new branch (main / inherited / sandbox) |
| `route_branch_switch` | Switch current branch |
| `route_diff` | Show added/modified/removed files between two snapshots |
| `route_changes` | Show what would be committed right now |
| `route_export` | Export repo as json / markdown / mermaid / emacs |
| `route_annotate` | Tag a commit edge with a short note (e.g. the prompt) |
| `route_read_file` | Read a file from a snapshot (or the working dir) |

### Example: a complete session

```jsonc
// 1) initialize (MCP handshake, no business logic)
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}

// 2) learn where we are
{"jsonrpc":"2.0","id":2,"method":"tools/call",
 "params":{"name":"route_status","arguments":{}}}

// 3) confirm the AI's pending changes match its plan
{"jsonrpc":"2.0","id":3,"method":"tools/call",
 "params":{"name":"route_changes","arguments":{}}}

// 4) record the change
{"jsonrpc":"2.0","id":4,"method":"tools/call",
 "params":{"name":"route_commit",
           "arguments":{"message":"add scroll-margin to anchors",
                        "author":"ai:claude-sonnet-4.5"}}}
```

Tool results are wrapped in the standard MCP `content` array. Errors
come back with `isError: true` and an `_routeCode` integer that maps
to:

| Code | Meaning |
|---|---|
| `-32000` | generic tool failure |
| `-32001` | not a Route repository (wrong cwd) |
| `-32002` | snapshot id / prefix didn't resolve (typo, or ambiguous) |

The AI should treat these like any other tool error: log, retry with
correction, or surface to the user.

---

## 2. CLI

`route` is a single binary that operates on the current directory.
Every MCP tool maps 1:1 to a CLI subcommand. Use the CLI when your
agent shells out to subprocesses.

```bash
# What branch / head am I on?
route status

# What did I just change?
route changes

# Record a change. -m is the user-facing message the timeline will
# show. -a stamps the author (the AI agent's display name).
route commit -m "add scroll-margin to anchors" -a "ai:claude-sonnet-4.5"

# Before a risky change, drop a checkpoint the user can roll back to.
route checkpoint -t "before refactor" -b "splitting the auth middleware into 3 modules"

# I changed my mind / made a mistake — step back one.
route undo

# Jump back to any prior snapshot. Prefix match is fine.
route rollback 01HXYZ... -r "user asked to revert"

# Annotate the most recent commit with the prompt that drove it.
route annotate $(route log -n 1 | awk '/^id/ {print $2}') \
    "user asked: 'clean up the auth middleware'"
```

The full CLI surface is what the `route` binary prints under
`route --help`. The MCP tool list above is a 1:1 map; if a tool
exists, there's a CLI command for it.

### Quoting the agent name

The agent's display name is what the user sees in the timeline. Use a
short, recognisable string. The convention is `ai:<name>`:

- `ai:claude-sonnet-4.5`
- `ai:claude-code-2.0.21`
- `ai:cursor-claude-4.5`
- `ai:continue-deepseek-v3`
- `ai:local-llama-3.3-70b`
- `ai:my-vibecoding-pipeline`

Don't bake the prompt into the agent name — put it in the commit
message's body (`-b` on `commit`, or `route annotate`).

### A note on shelled-out agents

The CLI is blocking and synchronous. Spawn one `route` process per
logical action; don't keep one open across turns (you'll lose
error handling). The CLI is fast (no daemon, no startup cost beyond
the SQLite open) and safe to call in tight loops.

---

## 3. HTTP

The Tauri desktop app exposes the same primitives through its IPC
layer. If your AI runs in a process that cannot see the user's local
shell, you have two options:

### a) Use the desktop app's IPC (recommended for local AIs)

The Tauri app already exposes every command Route has through
`window.__TAURI_INTERNALS__`. From a webview-embedded agent (a
browser extension, a panel in the app itself), invoke them
directly. The TypeScript binding is in
`crates/route-tauri/web/src/api.ts` — every function there is a
1:1 mirror of a CLI/MCP command.

```typescript
import { status, commit, checkpointCreate, rollback } from "./api";

const s = await status();           // { current_branch, branches, head, ... }
await commit("add scroll-margin to anchors", null);   // string message, string? body
await checkpointCreate("before refactor", "splitting auth middleware");
await rollback("01HXYZ...", null);
```

The Tauri commands the frontend wraps are also available to any
in-process Tauri plugin (e.g. a sidecar that ships a small HTTP
listener). The list of `tauri::generate_handler!` registrations in
`crates/route-tauri/src/lib.rs` is the canonical source.

### b) Run a separate HTTP server (for remote / headless AIs)

If you need a real HTTP surface (e.g. a hosted agent talking to a
user's machine over a tunnel), wrap the same `route_basic` and
`route_core` calls in a tiny `axum` / `hyper` server. The route-mcp
binary's `main.rs` is a template: replace the stdio loop with an
HTTP handler and you have a working server. Keep the request/response
shapes identical to the MCP tool results so the AI client code is
portable.

---

## Design rules for the AI

These are the rules Route assumes the AI will follow. They are also
the rules the user's UI assumes, so violating them makes the
timeline confusing. If a stricter mode is needed, set
`is_ai=0` in the operator field — the user can always tell which
turns were the AI's.

1. **Call `route_status` first.** The user may have switched projects,
   rolled back, or have a different branch active than you expect. Never
   guess — read.
2. **Call `route_changes` before `route_commit`.** Confirm what's
   actually different on disk matches what you think you changed. The
   `size_bytes` and `previous_hash` fields let you spot surprise edits
   the user made between your turns.
3. **Drop a `route_checkpoint` before risky changes.** The user can
   roll back from the UI without your involvement. This is a one-line
   insurance policy.
4. **Write the `message` as the user, not as yourself.** The commit
   message is the user's diary. Don't write "AI: refactored auth
   middleware per spec". Write "split auth middleware into verify,
   session, and route guards". The user knows what was asked; they
   don't need the AI's narration.
5. **Use `author = "ai:<name>"` so commits are filterable.** The
   history page has an "AI" filter; it works because every AI commit
   has this prefix. Don't pretend to be the user.
6. **Use `route_annotate` for the prompt, not the message.** The
   prompt goes in the commit body's CONFLICTS / LOGIC sections or in
   an edge annotation. The message stays short and human.
7. **Prefer `route_undo` over `route_rollback` when possible.** Both
   are reversible, but `undo` is a one-step navigation while
   `rollback` is a multi-snapshot jump the user has to manually
   re-climb. Smaller moves are easier to read back.
8. **Never delete history.** `route_rollback` is reversible; nothing
   in the tool surface drops commits. If you think the timeline is
   messy, leave it messy. The user may want to read it back.
9. **Surface failures, don't swallow them.** If `route_commit` fails,
   tell the user *why*. The error message is already localized for
   them. Don't retry blindly — read the message.
10. **Don't call `route_export` on every turn.** It is for the user
    to copy out, not for you to read. The structured JSON in
    `route_status` and `route_log` is your read API.

---

## Worked example: an AI refactor session

```
// AI: "I want to refactor the auth middleware. Before I start, let
//     learn what's there and drop a checkpoint."

→ tools/call route_status
← {
    "current_branch": "main",
    "branches": [{"name":"main","kind":"main","head":"01HXA..."}],
    "head": {"id":"01HXA...","short_id":"01HXA9K2"}
  }

→ tools/call route_checkpoint
   args: {"title":"before auth refactor",
          "body":"splitting verify/session/route guards"}
← {"id":"01HXB...","short_id":"01HXBPVK"}

// AI: edits files, runs tests, ...

→ tools/call route_changes
← {
    "modified": [
      {"path":"src/auth/middleware.rs","size_bytes":4823},
      {"path":"src/auth/mod.rs","size_bytes":312}
    ],
    "added": [],
    "removed": []
  }

→ tools/call route_commit
   args: {"message":"split auth middleware into verify, session, route guards",
          "author":"ai:claude-sonnet-4.5"}
← {"id":"01HXC...","kind":"incremental","from_snapshot":"01HXA...","to_snapshot":"01HXC..."}

// AI: test fails. "Let me step back and try a different approach."

→ tools/call route_undo
← {"undone":"01HXC...","kind":"incremental"}

// AI: edits files, tries again.

→ tools/call route_commit
   args: {"message":"split auth middleware; preserve existing call sites",
          "author":"ai:claude-sonnet-4.5"}
← {"id":"01HXD...","kind":"incremental"}

// AI: annotates the commit with the prompt that drove it, so the
//     user can read back why the AI picked this approach.

→ tools/call route_annotate
   args: {"commit_id":"01HXD...",
          "text":"user asked: 'clean up the auth middleware, but don't break the existing call sites'"}
← {"annotation_id":"01HXD...#1","text":"..."}

// AI: hands control back to the user. "Done. The new structure is in
//     commit 01HXD. If you don't like it, the checkpoint at 01HXB is
//     a one-click rollback from the UI."
```

The user opens the app, sees the AI commits filtered under the "AI"
tab in history, sees the checkpoint starred at 01HXB, and can roll
back to it with one click — or read the prompt that produced each
AI turn by hovering the edge annotation.
