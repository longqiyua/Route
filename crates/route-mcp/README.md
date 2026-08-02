# route-mcp

> Model Context Protocol server for the Route version manager.

`route-mcp` exposes Route's version-management primitives as MCP
`tools`, so any MCP-compatible AI client (Claude Desktop, Cursor,
Continue, a local llama with an MCP shim, your own agent) can call
Route just like any other tool. The binary is a small JSON-RPC 2.0
server over stdio. It is intentionally minimal — there is no AI
inside the binary, no token budget, no inference. Route is the
system of record; the model is the operator.

See [docs/ai-api.md](../docs/ai-api.md) for the full tool
reference and design rules. This file is a quick start.

## Build

```bash
cargo build -p route-mcp --release
# binary at target/release/route-mcp(.exe)
```

## Wire it up

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "route": {
      "command": "/absolute/path/to/route-mcp",
      "args": []
    }
  }
}
```

The MCP server's working directory is whatever Claude starts it with.
Make sure the user has `cd`'d into a Route project (one that has
`.route-basic/` inside) before starting Claude. If you need to pin
the repo path, wrap the binary in a tiny shell script that `cd`'s
first:

```sh
#!/bin/sh
cd /Users/me/projects/my-app
exec /usr/local/bin/route-mcp "$@"
```

### Cursor / Continue / other MCP clients

Point the client at the `route-mcp` binary. Same caveats about
working directory.

## What it exposes

15 tools, all under the `route_` namespace:

- `route_status`, `route_log`, `route_changes`, `route_diff`
- `route_commit`, `route_checkpoint`, `route_rollback`, `route_undo`, `route_redo`
- `route_branches`, `route_branch_create`, `route_branch_switch`
- `route_annotate`, `route_read_file`
- `route_export`

Each tool returns a JSON value embedded in the standard MCP
`{ content: [{ type: "text", text: "..." }], isError: false }` shape.
Errors come back with `isError: true` and an `_routeCode` field for
structured handling.

## Test it by hand

`route-mcp` reads one JSON object per line from stdin and writes one
per line to stdout. You can drive it from a shell:

```bash
$ echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
  | route-mcp
{"jsonrpc":"2.0","id":1,"result":{"capabilities":{"tools":{}},"serverInfo":{"name":"route-mcp","version":"0.4.0-beta"},...}}

$ echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | route-mcp
{"jsonrpc":"2.0","id":2,"result":{"tools":[{...},{...},...]}}
```

## Why this is its own crate

The Route CLI is a user-facing tool; it has to be ergonomic for
humans (short flags, helpful errors, colored output). The MCP server
is an *AI* tool; it has to be ergonomic for models (consistent
shapes, structured error codes, explicit semantics). Splitting them
lets the two evolve independently without either polluting the
other's interface.

If you want to add a new tool:

1. Add the case in `dispatch_tool` in [src/main.rs](src/main.rs).
2. Add the entry in `tool_registry` (description + JSON schema).
3. Add a `do_<name>` function that takes the parsed arguments and
   returns a `serde_json::Value` (or an `anyhow::Error`).
4. Mirror it as a CLI subcommand in `crates/route-cli/src/main.rs`
   so the two surfaces stay in lockstep.
