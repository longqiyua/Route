//! Route MCP server — Model Context Protocol bridge for any external AI.
//!
//! This is intentionally **not** an AI. Route has no opinion about which
//! model should drive it. Instead, the `route-mcp` binary exposes the
//! Route version-management primitives as MCP `tools`, so that any
//! MCP-compatible client (Claude Desktop, Cursor, Continue, a local
//! llama-server with an MCP shim, the user's own "vibecoding" agent
//! stack, etc.) can call Route just like it would call any other tool.
//!
//! The protocol is JSON-RPC 2.0 over stdio, which is the standard MCP
//! transport. One JSON object per line on stdin, one per line on stdout.
//! See <https://modelcontextprotocol.io> for the full spec — this
//! implementation deliberately covers only the `tools` capability,
//! which is the only thing Route needs to be useful as a tool.
//!
//! Tool naming follows the convention `route_<verb>_<noun>`. A tool
//! called by an AI is conceptually a *version-management operation*,
//! not a chat turn. The AI's responsibility is to decide when to call
//! the tool, and to embed the result back into its own reasoning.
//! Route is purely the system of record.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use route_basic::{
    BasicRepository, BranchKind, CommitKind, CommitOptions, CreateBranchOptions, ExportFormat,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Project path resolution
//
// The MCP server is spawned by an external AI client (Claude Desktop,
// Cursor, a vibecoding agent, etc.) whose working directory is almost
// never the Route project directory. To bridge this gap the Tauri app
// passes `--project <path>` when it generates the AI client config
// snippet, and we store it in a process-wide OnceLock so every
// `open_repo_or_err` call uses the same project. If the flag is absent
// we fall back to the current directory, which keeps `route-mcp` usable
// from the shell for development.
// ---------------------------------------------------------------------------

static PROJECT_PATH: OnceLock<PathBuf> = OnceLock::new();

fn parse_args_and_init() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--project" {
            if let Some(path) = args.next() {
                let _ = PROJECT_PATH.set(PathBuf::from(path));
            }
        } else if arg == "--help" || arg == "-h" {
            eprintln!("route-mcp — Route MCP server (stdio JSON-RPC 2.0)");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  route-mcp [--project <path>]");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  --project <path>  Route project directory to operate on.");
            eprintln!("                    If omitted, uses the current working directory.");
            eprintln!();
            eprintln!("The server reads JSON-RPC requests on stdin and writes");
            eprintln!("responses on stdout, one per line. Logging goes to stderr.");
            std::process::exit(0);
        }
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 envelopes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl RpcResponse {
    fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }
    fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// Standard JSON-RPC error codes, plus a Route-specific range (3xxx) for
// tool failures so the AI can disambiguate a transport problem from a
// "the repo is in a bad state" problem.
const RPC_ERR_PARSE: i32 = -32700;
const RPC_ERR_INVALID: i32 = -32600;
const RPC_ERR_METHOD: i32 = -32601;
const RPC_ERR_PARAMS: i32 = -32602;
const RPC_ERR_TOOL: i32 = -32000; // generic tool failure
#[allow(dead_code)]
const RPC_ERR_REPO: i32 = -32001; // repo not found / not a route repo
const RPC_ERR_SNAPSHOT: i32 = -32002; // snapshot id doesn't resolve

// ---------------------------------------------------------------------------
// MCP tool registry
// ---------------------------------------------------------------------------

struct ToolDef {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

fn tool_registry() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "route_status",
            description: "Show repository status: current branch, branch list, head snapshot id, mode, and the working-directory path. Call this first whenever an AI session opens — it tells the AI what the user is actually working on and where the head is. Returns a plain JSON object.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_log",
            description: "Return the commit history as a JSON array (newest first). Each entry has the commit id, short id, message, author, kind (incremental / full / merge / rollback / checkpoint), branch, from_snapshot, to_snapshot, and created_at. Pass `limit` to cap the result; pass `branch` to filter to a single branch.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 50 },
                    "branch": { "type": "string", "description": "Filter to a single branch name. Omit to use the current branch." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_commit",
            description: "Commit the current working-directory state as a new snapshot. The `message` is required and should describe what changed in a way the user can read back later. `author` defaults to \"user\". `full` forces a full-snapshot kind (self-loop edge) instead of a normal snapshot. Returns the new commit's id, short id, branch, kind, and created_at.",
            input_schema: json!({
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": { "type": "string", "description": "What changed and why. The user will read this in the timeline." },
                    "author": { "type": "string", "description": "Defaults to \"user\". Use the AI agent's display name." },
                    "full": { "type": "boolean", "default": false, "description": "Force a full-snapshot kind (self-loop edge)." },
                    "branch": { "type": "string", "description": "Branch to commit on. Omit to use the current branch." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_rollback",
            description: "Roll the working directory back to a previous snapshot. The snapshot id may be a full ULID or a unique short prefix. A rollback is itself recorded as a new commit (kind=rollback), so the operation is reversible. Use this when the AI's edits were wrong, or when the user explicitly asks to revert. Always confirm with the user before calling this on a non-AI-authored change.",
            input_schema: json!({
                "type": "object",
                "required": ["snapshot_id"],
                "properties": {
                    "snapshot_id": { "type": "string", "description": "Snapshot id (full ULID or unique short prefix)." },
                    "reason": { "type": "string", "description": "Why the rollback was made. Recorded as the commit message." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_undo",
            description: "Undo the most recent head-advancing commit on the current branch. Returns the snapshot id that was undone. This is the inverse of `route_redo`; together they implement step-back / step-forward navigation. Note: rollbacks themselves are undoable, so the AI can recover from a mis-aimed rollback.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_redo",
            description: "Redo the most recently undone head-advancing commit. Returns the snapshot id that was re-applied. Will fail if there is nothing to redo.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_checkpoint",
            description: "Create a named checkpoint at the current state. Checkpoints are first-class commit nodes (is_checkpoint=1) that the user can later roll back to from the UI. Use this when the AI is about to attempt a risky change and the user might want a one-click undo later.",
            input_schema: json!({
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": { "type": "string", "description": "Short label, e.g. \"before refactor\"." },
                    "body": { "type": "string", "description": "Optional longer description." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_branches",
            description: "List branches. Returns an array of objects with name, kind (main | inherited | sandbox), and the head snapshot id (or null for empty).",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_branch_create",
            description: "Create a new branch. `kind` is one of: main, inherited, sandbox. `from` is the source branch name (defaults to current). The new branch starts with the same head as the source branch.",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "kind": { "type": "string", "enum": ["main", "inherited", "sandbox"], "default": "inherited" },
                    "from": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_branch_switch",
            description: "Switch to an existing branch. The working directory is left untouched — the AI is expected to read the new head's snapshot from `route_status` after the switch.",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_merge",
            description: "Merge `source` branch into the current branch (or `target` if specified). Performs a file-level 3-way merge for inherited branches (base / ours / theirs) and a 2-way copy for sandbox branches. Sandbox branches cannot be a merge target — copy their content into a new inherited branch first. The merge is recorded as a new commit (kind=merge). If files conflict, theirs (source) wins and the conflict paths are listed in the commit message. Always confirm with the user before merging.",
            input_schema: json!({
                "type": "object",
                "required": ["source"],
                "properties": {
                    "source": { "type": "string", "description": "Branch name to merge from." },
                    "target": { "type": "string", "description": "Branch name to merge into (defaults to current)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_diff",
            description: "Diff two snapshots by id. Returns added / modified / removed file paths. Pass the same id for both to inspect a single snapshot's contents.",
            input_schema: json!({
                "type": "object",
                "required": ["from", "to"],
                "properties": {
                    "from": { "type": "string" },
                    "to": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_changes",
            description: "Show what would be committed if the AI called `route_commit` right now. Returns the list of files in the working directory that differ from the head snapshot. Use this to confirm intent before committing.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_export",
            description: "Export the repository data. `format` is one of: json, markdown, mermaid, emacs. The output is returned as a single string field `text`.",
            input_schema: json!({
                "type": "object",
                "required": ["format"],
                "properties": {
                    "format": { "type": "string", "enum": ["json", "markdown", "mermaid", "emacs"] }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_annotate",
            description: "Attach a short text annotation to a commit edge. Useful for tagging AI-authored changes with the prompt that produced them, so the timeline can show *why* the change happened months later.",
            input_schema: json!({
                "type": "object",
                "required": ["commit_id", "text"],
                "properties": {
                    "commit_id": { "type": "string" },
                    "text": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_read_file",
            description: "Read a file's contents at a specific snapshot. Pass the snapshot id (omit to read the working directory) and a relative path. Returns the file's text. Useful for letting the AI inspect a previous version of a file without having to checkout.",
            input_schema: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string" },
                    "snapshot_id": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn open_repo_or_err() -> Result<BasicRepository> {
    // Use --project path if provided; otherwise fall back to cwd so the
    // binary still works when invoked directly from a project directory.
    let path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => std::env::current_dir().context("get current directory")?,
    };
    BasicRepository::open(&path).with_context(|| {
        format!(
            "Not a Route repository at '{}'. Run `route init` first, or pass --project <path>.",
            path.display()
        )
    })
}

fn resolve_snapshot_id(repo: &BasicRepository, prefix: &str) -> Result<String> {
    let snapshots = repo.all_snapshots().context("list snapshots")?;
    let mut matches = Vec::new();
    for s in &snapshots {
        if s.snapshot.id.starts_with(prefix) {
            matches.push(s.snapshot.id.clone());
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(anyhow!("[{}] no snapshot matches prefix '{}'", RPC_ERR_SNAPSHOT, prefix)),
        _ => Err(anyhow!("[{}] ambiguous prefix '{}': {} candidates", RPC_ERR_SNAPSHOT, prefix, matches.len())),
    }
}

fn commit_kind_str(k: CommitKind) -> &'static str {
    match k {
        CommitKind::Incremental => "incremental",
        CommitKind::Full => "full",
        CommitKind::Merge => "merge",
        CommitKind::Rollback => "rollback",
    }
}

// ---------------------------------------------------------------------------
// Tool implementations
//
// Every `do_*` function returns a JSON value to embed in the tool
// result. The `handle_tools_call` function wraps tool errors in the
// standard MCP `isError: true` shape so the AI sees a structured
// failure, not a transport-level one.
// ---------------------------------------------------------------------------

fn do_status(_args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let branches = repo.list_branches().context("list branches")?;
    let current = repo.get_current_branch_name().context("current branch")?;
    let history = repo.history(1).context("history")?;
    let head = history.first().map(|h| json!({
        "id": h.snapshot.id,
        "short_id": route_core::short_id(&h.snapshot.id),
        "created_at": h.snapshot.created_at,
    }));
    Ok(json!({
        "project_path": repo.project_path().display().to_string(),
        "mode": repo.config.mode,
        "current_branch": current,
        "branches": branches.iter().map(|b| json!({
            "name": b.name,
            "kind": b.kind.as_str(),
            "head": b.head_snapshot,
        })).collect::<Vec<_>>(),
        "head": head,
    }))
}

fn do_log(args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or(50);
    let branch = args.get("branch").and_then(|v| v.as_str()).map(|s| s.to_string());
    let branch_filter = branch.as_deref();
    let commits = repo.list_commits(branch_filter, limit).context("list commits")?;
    let entries: Vec<Value> = commits.iter().map(|c| json!({
        "id": c.id,
        "short_id": route_core::short_id(&c.id),
        "message": c.message,
        "author": c.author,
        "kind": commit_kind_str(c.kind),
        "branch_id": c.branch_id,
        "from_snapshot": c.from_snapshot,
        "to_snapshot": c.to_snapshot,
        "created_at": c.created_at,
    })).collect();
    Ok(json!({ "count": entries.len(), "branch": branch, "entries": entries }))
}

fn do_commit(args: Value) -> Result<Value> {
    let message = args.get("message").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `message`"))?
        .to_string();
    if message.trim().is_empty() {
        return Err(anyhow!("`message` must not be empty"));
    }
    let author = args.get("author").and_then(|v| v.as_str()).map(|s| s.to_string());
    let full = args.get("full").and_then(|v| v.as_bool()).unwrap_or(false);
    let branch = args.get("branch").and_then(|v| v.as_str()).map(|s| s.to_string());

    let repo = open_repo_or_err()?;
    let mut opts = CommitOptions {
        message: message.clone(),
        author: author.clone(),
        force_full: full,
        branch: branch.clone(),
        ..Default::default()
    };
    if let Some(b) = branch.as_deref() { opts.branch = Some(b.to_string()); }
    if let Some(a) = author.as_deref() { opts.author = Some(a.to_string()); }

    let c = repo.commit(opts).context("commit")?;
    Ok(json!({
        "id": c.id,
        "short_id": route_core::short_id(&c.id),
        "branch_id": c.branch_id,
        "kind": commit_kind_str(c.kind),
        "from_snapshot": c.from_snapshot,
        "to_snapshot": c.to_snapshot,
        "created_at": c.created_at,
        "message": c.message,
        "author": c.author,
    }))
}

fn do_rollback(args: Value) -> Result<Value> {
    let prefix = args.get("snapshot_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `snapshot_id`"))?
        .to_string();
    let reason = args.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
    let repo = open_repo_or_err()?;
    let id = resolve_snapshot_id(&repo, &prefix)?;
    let c = repo.rollback_to(&id, reason.as_deref()).context("rollback")?;
    Ok(json!({
        "rolled_back_to": id,
        "short_id": route_core::short_id(&id),
        "rollback_commit": c.id,
        "kind": commit_kind_str(c.kind),
        "message": c.message,
    }))
}

fn do_undo(_args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let c = repo.undo_last().context("undo")?;
    Ok(json!({
        "undone": c.id,
        "short_id": route_core::short_id(&c.id),
        "kind": commit_kind_str(c.kind),
    }))
}

fn do_redo(_args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let c = repo.redo_last().context("redo")?;
    Ok(json!({
        "redone": c.id,
        "short_id": route_core::short_id(&c.id),
        "kind": commit_kind_str(c.kind),
    }))
}

fn do_checkpoint(args: Value) -> Result<Value> {
    let title = args.get("title").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `title`"))?
        .to_string();
    if title.trim().is_empty() { return Err(anyhow!("`title` must not be empty")); }
    let body = args.get("body").and_then(|v| v.as_str()).map(|s| s.to_string());
    let repo = open_repo_or_err()?;
    let c = repo.checkpoint_create(&title, body.as_deref(), None).context("checkpoint")?;
    Ok(json!({
        "id": c.id,
        "short_id": route_core::short_id(&c.id),
        "title": title,
        "body": body,
    }))
}

fn do_branches(_args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let branches = repo.list_branches().context("list branches")?;
    Ok(json!({
        "branches": branches.iter().map(|b| json!({
            "name": b.name,
            "kind": b.kind.as_str(),
            "head": b.head_snapshot,
        })).collect::<Vec<_>>()
    }))
}

fn do_branch_create(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?.to_string();
    let kind_str = args.get("kind").and_then(|v| v.as_str()).unwrap_or("inherited");
    let kind = match kind_str {
        "main" => BranchKind::Main,
        "sandbox" => BranchKind::Sandbox,
        _ => BranchKind::Inherited,
    };
    let from = args.get("from").and_then(|v| v.as_str()).map(|s| s.to_string());
    let repo = open_repo_or_err()?;
    let current = repo.get_current_branch_name().context("current branch")?;
    let mut opts = CreateBranchOptions { kind, from_branch: from.clone() };
    if let Some(f) = from.as_deref() { opts.from_branch = Some(f.to_string()); }
    let b = repo.create_branch(&name, opts, &current).context("create branch")?;
    Ok(json!({ "created": b.name, "kind": b.kind.as_str(), "head": b.head_snapshot }))
}

fn do_branch_switch(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?.to_string();
    let repo = open_repo_or_err()?;
    repo.set_current_branch(&name).with_context(|| format!("switch to {name}"))?;
    Ok(json!({ "switched_to": name }))
}

fn do_merge(args: Value) -> Result<Value> {
    let source = args.get("source").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `source`"))?.to_string();
    let target = args.get("target").and_then(|v| v.as_str()).map(|s| s.to_string());
    let repo = open_repo_or_err()?;
    let c = repo.merge(&source, target.as_deref()).context("merge")?;
    Ok(json!({
        "merge_commit": c.id,
        "short_id": route_core::short_id(&c.id),
        "kind": commit_kind_str(c.kind),
        "message": c.message,
        "source": source,
        "target": target,
    }))
}

fn do_diff(args: Value) -> Result<Value> {
    let from = args.get("from").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `from`"))?.to_string();
    let to = args.get("to").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `to`"))?.to_string();
    let repo = open_repo_or_err()?;
    let from_id = resolve_snapshot_id(&repo, &from)?;
    let to_id = resolve_snapshot_id(&repo, &to)?;
    let diff = repo.diff_snapshots(&from_id, &to_id).context("diff snapshots")?;
    let mut added = Vec::new(); let mut modified = Vec::new(); let mut removed = Vec::new();
    for e in &diff {
        let entry = json!({ "path": e.path, "from_hash": e.from_hash, "to_hash": e.to_hash });
        match e.change.as_str() {
            "added" => added.push(entry),
            "modified" => modified.push(entry),
            "removed" => removed.push(entry),
            _ => {}
        }
    }
    Ok(json!({
        "from": from_id,
        "to": to_id,
        "added": added,
        "modified": modified,
        "removed": removed,
    }))
}

fn do_changes(_args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let changes = repo.working_dir_status().context("working dir status")?;
    let mut added = Vec::new(); let mut modified = Vec::new(); let mut removed = Vec::new();
    for w in &changes {
        let entry = json!({
            "path": w.path,
            "current_hash": w.current_hash,
            "previous_hash": w.previous_hash,
            "size_bytes": w.size_bytes,
        });
        match w.change.as_str() {
            "added" => added.push(entry),
            "modified" => modified.push(entry),
            "removed" => removed.push(entry),
            _ => {}
        }
    }
    Ok(json!({
        "added": added,
        "modified": modified,
        "removed": removed,
    }))
}

fn do_export(args: Value) -> Result<Value> {
    let format = args.get("format").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `format`"))?;
    let fmt = match format {
        "json" => ExportFormat::Json,
        "markdown" | "md" => ExportFormat::Markdown,
        "mermaid" => ExportFormat::Mermaid,
        "emacs" => ExportFormat::EmacsOrg,
        other => return Err(anyhow!("unsupported format '{}'", other)),
    };
    let repo = open_repo_or_err()?;
    let ctx = repo.build_export_context().context("build export context")?;
    let exporter = route_basic::DefaultExporters::for_format(fmt)
        .ok_or_else(|| anyhow!("no exporter for format '{}'", format))?;
    let mut buf: Vec<u8> = Vec::new();
    exporter.export(&ctx, &mut buf).context("export")?;
    let text = String::from_utf8(buf).context("export produced non-utf8 output")?;
    Ok(json!({ "format": format, "text": text }))
}

fn do_annotate(args: Value) -> Result<Value> {
    let commit_id = args.get("commit_id").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `commit_id`"))?.to_string();
    let text = args.get("text").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `text`"))?.to_string();
    let repo = open_repo_or_err()?;
    let a = repo.add_path_annotation(&commit_id, &text).context("annotate")?;
    Ok(json!({ "annotation_id": a.id, "commit_id": commit_id, "text": text }))
}

fn do_read_file(args: Value) -> Result<Value> {
    let path = args.get("path").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `path`"))?.to_string();
    let snapshot_id = args.get("snapshot_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let repo = open_repo_or_err()?;

    if let Some(prefix) = snapshot_id {
        let id = resolve_snapshot_id(&repo, &prefix)?;
        let files = repo.resolve_snapshot_files(&id).context("resolve snapshot files")?;
        let hash = files.get(&path)
            .ok_or_else(|| anyhow!("file '{}' is not present in snapshot {}", path, route_core::short_id(&id)))?;
        let blob_path = repo.route_paths().blob_path(hash);
        let text = std::fs::read_to_string(&blob_path)
            .with_context(|| format!("read blob {} (path {})", hash, blob_path.display()))?;
        Ok(json!({ "path": path, "snapshot_id": id, "blob_hash": hash, "text": text }))
    } else {
        // Read from the working directory directly.
        let p = repo.project_path().join(&path);
        let text = std::fs::read_to_string(&p)
            .with_context(|| format!("read working-dir file {}", p.display()))?;
        Ok(json!({ "path": path, "text": text }))
    }
}

fn dispatch_tool(name: &str, args: Value) -> Result<Value> {
    match name {
        "route_status"        => do_status(args),
        "route_log"           => do_log(args),
        "route_commit"        => do_commit(args),
        "route_rollback"      => do_rollback(args),
        "route_undo"          => do_undo(args),
        "route_redo"          => do_redo(args),
        "route_checkpoint"    => do_checkpoint(args),
        "route_branches"      => do_branches(args),
        "route_branch_create" => do_branch_create(args),
        "route_branch_switch" => do_branch_switch(args),
        "route_merge"         => do_merge(args),
        "route_diff"          => do_diff(args),
        "route_changes"       => do_changes(args),
        "route_export"        => do_export(args),
        "route_annotate"      => do_annotate(args),
        "route_read_file"     => do_read_file(args),
        other => Err(anyhow!("unknown tool: {}", other)),
    }
}

// ---------------------------------------------------------------------------
// MCP method dispatch
// ---------------------------------------------------------------------------

const SERVER_INFO: &str = "route-mcp";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

fn handle_initialize(id: Value) -> RpcResponse {
    RpcResponse::ok(id, json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": { "name": SERVER_INFO, "version": SERVER_VERSION },
        "capabilities": { "tools": {} },
        "instructions":
            "Route is a version manager for Web Coding projects. \
             Call `route_status` first to learn the current branch and head. \
             Then call `route_changes` to see what would be committed. \
             When the AI edits files, finish with `route_commit` and a clear message. \
             Before risky edits, call `route_checkpoint` so the user can roll back from the UI. \
             If a user says \"undo that\" or \"go back\", call `route_rollback` (not `route_undo`, which only steps the head). \
             Do not include AI reasoning in `route_commit`'s `message` — write the message as if you are the user describing the change. \
             Do not call `route_export` on every turn; it is for the user, not for you."
    }))
}

fn handle_tools_list(id: Value) -> RpcResponse {
    let tools: Vec<Value> = tool_registry().iter().map(|t| json!({
        "name": t.name,
        "description": t.description,
        "inputSchema": t.input_schema,
    })).collect();
    RpcResponse::ok(id, json!({ "tools": tools }))
}

fn handle_tools_call(id: Value, params: Value) -> RpcResponse {
    let name = match params.get("name").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return RpcResponse::err(id, RPC_ERR_PARAMS, "missing `name`"),
    };
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    match dispatch_tool(&name, args) {
        Ok(content) => RpcResponse::ok(id, json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&content).unwrap_or_else(|_| content.to_string()) }],
            "isError": false,
        })),
        Err(e) => {
            // If the error message embeds a structured code, surface it
            // to the AI as a tool error (not a transport error) so the
            // AI can decide to retry, ask the user, or skip the call.
            let msg = e.to_string();
            let (code, clean) = if let Some(rest) = msg.strip_prefix('[') {
                if let Some(end) = rest.find(']') {
                    let code_str = &rest[..end];
                    if let Ok(n) = code_str.parse::<i32>() {
                        (n, rest[end + 1..].trim_start_matches(' ').to_string())
                    } else {
                        (RPC_ERR_TOOL, msg)
                    }
                } else { (RPC_ERR_TOOL, msg) }
            } else { (RPC_ERR_TOOL, msg) };
            RpcResponse::ok(id, json!({
                "content": [{ "type": "text", "text": clean }],
                "isError": true,
                "_routeCode": code,
            }))
        }
    }
}

fn handle_request(req: RpcRequest) -> Option<RpcResponse> {
    let id = req.id.clone().unwrap_or(Value::Null);
    if req.jsonrpc != "2.0" {
        return Some(RpcResponse::err(id, RPC_ERR_INVALID, "jsonrpc must be \"2.0\""));
    }
    match req.method.as_str() {
        "initialize" => Some(handle_initialize(id)),
        "notifications/initialized" => None, // client→server notification, no reply
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => Some(handle_tools_call(id, req.params)),
        "ping" => Some(RpcResponse::ok(id, json!({}))),
        other => Some(RpcResponse::err(id, RPC_ERR_METHOD, format!("method not found: {}", other))),
    }
}

// ---------------------------------------------------------------------------
// stdio loop
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    // Parse --project <path> before anything else so the first
    // `open_repo_or_err` call (which may come from `initialize`'s
    // status check) resolves to the right project.
    parse_args_and_init();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut reader = stdin.lock();

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).context("read stdin")?;
        if n == 0 { break; } // EOF
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        let req: RpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = RpcResponse::err(Value::Null, RPC_ERR_PARSE, format!("parse error: {e}"));
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
                continue;
            }
        };
        if let Some(resp) = handle_request(req) {
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}
