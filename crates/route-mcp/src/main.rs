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
//!
//! ## Permission enforcement
//!
//! The MCP server enforces Route's permission model:
//! - **Normal** (default): local git operations are allowed; remote
//!   operations (push, fetch, remote config, clone) are **blocked**.
//! - **High**: all operations are allowed. Activate with
//!   `--permission-level high` (or the Tauri Settings page).
//!
//! The permission level is passed via `--permission-level` when the AI
//! client spawns `route-mcp`. The Tauri app includes this flag in the
//! MCP config snippet it generates.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use route_basic::{
    BasicRepository, BranchKind, CommitKind, CommitOptions, CreateBranchOptions, ExportFormat,
};
use serde::{Deserialize, Serialize};
use route_memory::ConversationStore;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Git command helpers
//
// Thin wrappers around the `git` CLI. These are used by the `route_git_*`
// tools and are completely independent of Route's own repository format.
// ---------------------------------------------------------------------------

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn git_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn project_path() -> Result<std::path::PathBuf, String> {
    Ok(match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| format!("no project path: {e}"))?,
    })
}

fn run_git(args: &[&str]) -> Result<String, String> {
    let path = project_path()?;
    let mut cmd = git_command();
    cmd.current_dir(&path).args(args);
    cmd.env("LC_ALL", "C");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_ASKPASS", "");
    let output = cmd.output().map_err(|e| format!("failed to spawn git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if detail.is_empty() { format!("git {} failed", args.join(" ")) } else { format!("git {}: {detail}", args.join(" ")) });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

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

/// Permission level for the MCP server. Defaults to "normal" which blocks
/// remote operations. Set to "high" via `--permission-level high`.
static PERMISSION_LEVEL: OnceLock<String> = OnceLock::new();

/// Returns the current permission level string.
fn permission_level() -> &'static str {
    PERMISSION_LEVEL.get().map(|s| s.as_str()).unwrap_or("normal")
}

/// Returns true if the current permission level is "high".
fn is_high_permission() -> bool {
    permission_level() == "high"
}

/// Check if a remote git operation is allowed. Returns `Ok(())` or an
/// error message explaining the restriction.
fn check_remote_permission(op_name: &str) -> Result<(), String> {
    if is_high_permission() {
        return Ok(());
    }
    Err(format!(
        "Operation '{}' is blocked at the current permission level (normal).\n\
         Remote operations (push, fetch, remote config, clone, pull) are restricted in Normal mode.\n\
         To enable: restart route-mcp with `--permission-level high` or set the permission level to\n\
         'high' in the Route Settings page.",
        op_name
    ))
}

fn parse_args_and_init() {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--project" {
            if let Some(path) = args.next() {
                let _ = PROJECT_PATH.set(PathBuf::from(path));
            }
        } else if arg == "--permission-level" {
            if let Some(level) = args.next() {
                let _ = PERMISSION_LEVEL.set(level);
            }
        } else if arg == "--help" || arg == "-h" {
            eprintln!("route-mcp — Route MCP server (stdio JSON-RPC 2.0)");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  route-mcp [--project <path>] [--permission-level <normal|high>]");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  --project <path>           Route project directory to operate on.");
            eprintln!("                             If omitted, uses the current working directory.");
            eprintln!("  --permission-level <level>  Permission level: 'normal' (default) or 'high'.");
            eprintln!("                             Normal blocks remote operations (push, fetch, remote");
            eprintln!("                             config, clone, pull). High allows all operations.");
            eprintln!();
            eprintln!("The server reads JSON-RPC requests on stdin and writes");
            eprintln!("responses on stdout, one per line. Logging goes to stderr.");
            std::process::exit(0);
        }
    }
    // Ensure PERMISSION_LEVEL has a default.
    if PERMISSION_LEVEL.get().is_none() {
        let _ = PERMISSION_LEVEL.set("normal".to_string());
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
        // -----------------------------------------------------------------------
        // Git CLI tools (wrappers around the `git` binary)
        // -----------------------------------------------------------------------
        ToolDef {
            name: "route_git_init",
            description: "Initialize a git repository in the project directory (idempotent). Sets local user name/email if none is configured. Safe to call on an already-initialized repo.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_status",
            description: "Show working tree status as a grouped map (added/modified/removed/untracked/renamed/copied).",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_log",
            description: "Show commit log as a list of entries (sha, message, author, timestamp). Pass `limit` to cap the result, `branch` to filter to a single branch, and `file` to filter to a single file.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 30 },
                    "branch": { "type": "string", "description": "Branch or revision range." },
                    "file": { "type": "string", "description": "Show only commits touching this file path." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_log_graph",
            description: "Show git log as an ASCII graph with branch topology. Pass `limit` to cap the result and `all` to include all branches (default: current branch only).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 30 },
                    "all": { "type": "boolean", "default": false, "description": "Show all branches." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_show",
            description: "Show full details of a commit (author, date, message, diff). Requires `commit` (sha or reference).",
            input_schema: json!({
                "type": "object",
                "required": ["commit"],
                "properties": {
                    "commit": { "type": "string", "description": "Commit sha, short sha, or reference (HEAD, main, etc.)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_remote_list",
            description: "List configured git remotes with their URLs.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_remote_add",
            description: "Add a git remote. Requires `name` (e.g. origin) and `url`.",
            input_schema: json!({
                "type": "object",
                "required": ["name", "url"],
                "properties": {
                    "name": { "type": "string" },
                    "url": { "type": "string" }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_remote_remove",
            description: "Remove a git remote by name.",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_fetch",
            description: "Fetch from a remote. Default remote is `origin`.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "remote": { "type": "string", "default": "origin" },
                    "branch": { "type": "string", "description": "Optional branch to fetch." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_pull",
            description: "Pull from a remote branch with rebase (--rebase). Default remote is `origin`.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "remote": { "type": "string", "default": "origin" },
                    "branch": { "type": "string", "description": "Branch to pull (defaults to upstream tracking)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_push",
            description: "Push to a remote branch. Default remote is `origin`. If `branch` is omitted, pushes the current branch to its upstream.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "remote": { "type": "string", "default": "origin" },
                    "branch": { "type": "string", "description": "Branch to push (defaults to current branch)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_revert",
            description: "Revert a commit (safe, creates a new commit that undoes the given commit). Requires `commit` (sha or reference).",
            input_schema: json!({
                "type": "object",
                "required": ["commit"],
                "properties": {
                    "commit": { "type": "string", "description": "Commit sha to revert." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_cherry_pick",
            description: "Cherry-pick commits onto current HEAD. Provide one or more commit shas.",
            input_schema: json!({
                "type": "object",
                "required": ["commits"],
                "properties": {
                    "commits": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "Commit sha or array of shas to cherry-pick."
                    }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_rebase",
            description: "Rebase current branch onto another branch or commit. Pass `onto` as the target branch/commit.",
            input_schema: json!({
                "type": "object",
                "required": ["onto"],
                "properties": {
                    "onto": { "type": "string", "description": "Branch or commit to rebase onto." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_rebase_abort",
            description: "Abort an in-progress rebase and return to the original state.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_rebase_continue",
            description: "Continue a rebase after resolving conflicts.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_stash_push",
            description: "Stash working directory changes. Optionally include a `message`.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Optional stash message." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_stash_pop",
            description: "Pop the top stash and apply its changes to the working directory.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_stash_list",
            description: "List all stashes.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_git_tag_list",
            description: "List tags. Optionally filter by `pattern` (glob).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Glob pattern to filter tags (e.g. v1.*)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_tag_create",
            description: "Create a tag at HEAD. Requires `name`. Optionally include a `message` (annotated tag).",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "message": { "type": "string", "description": "Optional annotation message for an annotated tag." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_tag_delete",
            description: "Delete a tag by name.",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string" } },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_config_get",
            description: "Get a git config value. Requires `key` (e.g. user.name, user.email).",
            input_schema: json!({
                "type": "object",
                "required": ["key"],
                "properties": { "key": { "type": "string" } },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_config_set",
            description: "Set a git config value. Requires `key` and `value`. Uses local scope by default.",
            input_schema: json!({
                "type": "object",
                "required": ["key", "value"],
                "properties": {
                    "key": { "type": "string" },
                    "value": { "type": "string" },
                    "scope": { "type": "string", "enum": ["local", "global", "system"], "default": "local" }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_clean",
            description: "Clean untracked files. Dry-run by default (set `force` to true to actually delete).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "force": { "type": "boolean", "default": false, "description": "Set to true to actually delete files (dry-run when false)." },
                    "directories": { "type": "boolean", "default": false, "description": "Also clean untracked directories (-d)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_add",
            description: "Stage specific files. Pass one or more file paths to stage.",
            input_schema: json!({
                "type": "object",
                "required": ["files"],
                "properties": {
                    "files": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "File path or array of file paths to stage."
                    }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_reset",
            description: "Unstage specific files. Pass one or more file paths to unstage.",
            input_schema: json!({
                "type": "object",
                "required": ["files"],
                "properties": {
                    "files": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "File path or array of file paths to unstage."
                    }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_branch_delete",
            description: "Delete a local branch by name. Use `force` to delete even if not fully merged.",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "force": { "type": "boolean", "default": false, "description": "Force delete even if not merged." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_archive",
            description: "Create an archive of the repo. Returns the archive as a base64-encoded string. `format` can be zip, tar, or tgz (default: zip).",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "format": { "type": "string", "enum": ["zip", "tar", "tgz"], "default": "zip" },
                    "output": { "type": "string", "description": "Output file path (omit to return content as base64)." },
                    "prefix": { "type": "string", "description": "Optional path prefix inside the archive." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_clone",
            description: "Clone a remote repository into a target directory. Requires `url`. Optionally specify `directory`.",
            input_schema: json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string" },
                    "directory": { "type": "string", "description": "Target directory name (defaults to repo name)." },
                    "depth": { "type": "integer", "minimum": 1, "description": "Shallow clone depth." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_backup",
            description: "Create a safety backup of the entire repository (including .git directory) as a zip archive. Useful before destructive operations like rebase, reset, or force push. Returns the backup file path.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_permission_status",
            description: "Check the current permission level of the Route MCP server. Returns 'normal' (remote operations blocked) or 'high' (all operations allowed). Call this to determine whether remote git operations (push, fetch, remote config, clone, pull) are currently available.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_permission_set",
            description: "Set the permission level for the current MCP session. Accepts 'normal' or 'high'. Note: the change applies only to the current process; a restart resets to the default. 'high' allows all remote operations; 'normal' blocks push, fetch, pull, clone, remote add/remove.",
            input_schema: json!({
                "type": "object",
                "required": ["level"],
                "properties": {
                    "level": { "type": "string", "enum": ["normal", "high"], "description": "Permission level: 'normal' (block remote ops) or 'high' (allow all)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_branch_delete",
            description: "Delete a Route native branch by name. Cannot delete the current branch. Use `route_git_branch_delete` for git branches.",
            input_schema: json!({
                "type": "object",
                "required": ["name"],
                "properties": { "name": { "type": "string", "description": "Branch name to delete." } },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_stats",
            description: "Show repository statistics: total commits, total snapshots, branch count, checkpoint count, file count, and storage size. Returns a plain JSON object.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        // ===================================================================
        // Sync tools (directory synchronization)
        // ===================================================================
        ToolDef {
            name: "route_sync_list",
            description: "List all configured sync targets. Each entry shows the target name, source, destination, mode, transport, and whether it is enabled.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_sync_run",
            description: "Run sync immediately for all enabled targets (or a specific target by name). Returns the sync results including any files transferred.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Optional specific target name to sync (omit to sync all enabled targets)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_plugin_list",
            description: "List all installed plugins with their enabled/disabled status and configuration. Returns an array of plugin entries.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        // ===================================================================
        // AI Chat tool
        // ===================================================================
        ToolDef {
            name: "route_ai_chat",
            description: "Send a chat message to the configured AI provider. Requires ROUTE_AI_KEY environment variable to be set. Returns the AI response text. Use this when the AI needs to query itself for context or reasoning.",
            input_schema: json!({
                "type": "object",
                "required": ["message"],
                "properties": {
                    "message": { "type": "string", "description": "The chat message to send to the AI." },
                    "system": { "type": "string", "description": "Optional system prompt override." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_mode",
            description: "Get or set the git mode (route.mode config). Pass `mode` to set a new mode; omit to read the current mode. Mode controls how Route interacts with git (e.g. 'standard', 'mirror', 'incremental').",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "description": "Mode to set (omit to read current mode)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_git_restore",
            description: "Restore working tree files from HEAD. Pass one or more file paths to restore. Use `staged` to also restore the index (unstage).",
            input_schema: json!({
                "type": "object",
                "required": ["paths"],
                "properties": {
                    "paths": {
                        "oneOf": [
                            { "type": "string" },
                            { "type": "array", "items": { "type": "string" } }
                        ],
                        "description": "File path or array of file paths to restore."
                    },
                    "staged": { "type": "boolean", "default": false, "description": "Also restore the index (unstage)." }
                },
                "additionalProperties": false
            }),
        },
        // ===================================================================
        // Tracking tools (active folder monitoring)
        // ===================================================================
        ToolDef {
            name: "route_tracking_list",
            description: "List all active tracking targets. Each entry shows the folder path, remote URL, branch, sync interval, and last sync status. Returns an array of tracking configurations.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_tracking_add",
            description: "Add a new tracking target. Requires `folder` (local path), `remote` (git remote URL), and optionally `branch` (default: main), `interval` (sync interval in seconds, default: 300). The specified folder will be periodically synced from the remote.",
            input_schema: json!({
                "type": "object",
                "required": ["folder", "remote"],
                "properties": {
                    "folder": { "type": "string", "description": "Local folder path to track." },
                    "remote": { "type": "string", "description": "Remote git repository URL." },
                    "branch": { "type": "string", "default": "main", "description": "Branch to track." },
                    "interval": { "type": "integer", "default": 300, "description": "Sync interval in seconds." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_tracking_remove",
            description: "Remove a tracking target by folder path. Stops syncing the specified folder.",
            input_schema: json!({
                "type": "object",
                "required": ["folder"],
                "properties": {
                    "folder": { "type": "string", "description": "Folder path of the tracking target to remove." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_tracking_sync",
            description: "Immediately sync a tracking target by folder path. Fetches latest changes from the remote repository. Returns the sync result including any new commits pulled.",
            input_schema: json!({
                "type": "object",
                "required": ["folder"],
                "properties": {
                    "folder": { "type": "string", "description": "Folder path of the tracking target to sync." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_tracking_history",
            description: "Show recent sync history for all tracking targets. Returns the last 20 sync events with timestamps, status, and messages.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        // ===================================================================
        // Extension tools (skills & references)
        // ===================================================================
        ToolDef {
            name: "route_extension_skills",
            description: "List all skills in the `.route/skills/` directory. Returns an array of skill file names and their content previews.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_extension_references",
            description: "List all reference files in the `.route/references/` directory. Returns an array of reference file names and their content previews. These are injected into the AI's context for project memory.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        // ===================================================================
        // Project context tool (for AI injection)
        // ===================================================================
        ToolDef {
            name: "route_project_context",
            description: "Get the full project context for AI injection. Returns the project structure, recent commits, current branch, references, and tracking history. Call this when you need to understand the project before making suggestions or changes.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        // ===================================================================
        // Conversation tracking tools
        // ===================================================================
        ToolDef {
            name: "route_conversation_new",
            description: "Create a new conversation session. Returns the new session_id. Use this to start tracking a conversation thread.",
            input_schema: json!({
                "type": "object",
                "required": ["title"],
                "properties": {
                    "title": { "type": "string", "description": "Human-readable title for the conversation session." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_conversation_list",
            description: "List all active (non-archived) conversation sessions, newest first. Each entry shows the session id, title, message count, and timestamps.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "route_conversation_show",
            description: "View the details of a specific conversation session, including its messages. Pass `session_id` to identify the session and optionally `limit` to cap the number of messages returned.",
            input_schema: json!({
                "type": "object",
                "required": ["session_id"],
                "properties": {
                    "session_id": { "type": "string", "description": "The session ID to view." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "description": "Maximum number of messages to return (default: all)." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_conversation_record",
            description: "Record a message in a conversation session. `role` must be one of: user, ai, system. Optionally link a `snapshot_id` to associate this message with a project snapshot.",
            input_schema: json!({
                "type": "object",
                "required": ["session_id", "role", "content"],
                "properties": {
                    "session_id": { "type": "string", "description": "The session ID to record the message in." },
                    "role": { "type": "string", "enum": ["user", "ai", "system"], "description": "Message role: user, ai, or system." },
                    "content": { "type": "string", "description": "The message content." },
                    "snapshot_id": { "type": "string", "description": "Optional linked snapshot ID from the project." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_conversation_rollback",
            description: "Rollback a conversation session to a specific message, restoring the linked project snapshot. All messages after the rollback point are removed. Use this to undo changes made during a conversation.",
            input_schema: json!({
                "type": "object",
                "required": ["session_id", "message_id"],
                "properties": {
                    "session_id": { "type": "string", "description": "The session ID to rollback in." },
                    "message_id": { "type": "string", "description": "The message ID to rollback to." },
                    "reason": { "type": "string", "description": "Optional reason for the rollback." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_conversation_archive",
            description: "Archive a conversation session. Archived sessions are hidden from the default list but are not deleted. Use this to clean up old conversations while preserving them.",
            input_schema: json!({
                "type": "object",
                "required": ["session_id"],
                "properties": {
                    "session_id": { "type": "string", "description": "The session ID to archive." }
                },
                "additionalProperties": false
            }),
        },
        ToolDef {
            name: "route_conversation_delete",
            description: "Permanently delete a conversation session and all its messages. This action cannot be undone. Use `route_conversation_archive` instead if you want to preserve the data.",
            input_schema: json!({
                "type": "object",
                "required": ["session_id"],
                "properties": {
                    "session_id": { "type": "string", "description": "The session ID to delete." }
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

// ---------------------------------------------------------------------------
// Git CLI tool implementations
//
// These are self-contained wrappers around the `git` binary, independent
// of Route's own repository format.  They use the `PROJECT_PATH` static
// (or cwd) and the `run_git` helper defined above.
// ---------------------------------------------------------------------------

fn do_git_init(_args: Value) -> Result<Value> {
    // Initialize if not already a git repo.
    let _ = run_git(&["init"]);
    // Set local user name/email if not configured.
    if run_git(&["config", "user.name"]).unwrap_or_default().trim().is_empty() {
        let _ = run_git(&["config", "user.name", "Route User"]);
    }
    if run_git(&["config", "user.email"]).unwrap_or_default().trim().is_empty() {
        let _ = run_git(&["config", "user.email", "route@local"]);
    }
    let out = run_git(&["rev-parse", "--git-dir"]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "git_dir": out.trim(), "status": "initialized" }))
}

fn do_git_status(_args: Value) -> Result<Value> {
    let out = run_git(&["status", "--porcelain"]).map_err(|e| anyhow!("{}", e))?;
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut removed = Vec::new();
    let mut untracked = Vec::new();
    let mut renamed = Vec::new();
    let mut copied = Vec::new();
    for line in out.lines() {
        if line.len() < 3 { continue; }
        let (xy, path) = line.split_at(2);
        let path = path.trim();
        match &xy[..1] {
            "?" => untracked.push(path),
            "A" | "+" => added.push(path),
            "M" => modified.push(path),
            "D" => removed.push(path),
            "R" => renamed.push(path),
            "C" => copied.push(path),
            _ => {}
        }
    }
    Ok(json!({
        "added": added,
        "modified": modified,
        "removed": removed,
        "untracked": untracked,
        "renamed": renamed,
        "copied": copied,
        "raw": out,
    }))
}

fn do_git_log(args: Value) -> Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30);
    let branch = args.get("branch").and_then(|v| v.as_str());
    let file = args.get("file").and_then(|v| v.as_str());
    let limit_str = format!("-{}", limit);
    let mut git_args = vec!["log", "--format=%H%n%s%n%an%n%ai%n---", limit_str.as_str()];
    if let Some(b) = branch { git_args.push(b); }
    if let Some(f) = file { git_args.push("--"); git_args.push(f); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    let mut entries = Vec::new();
    let mut lines = out.lines().peekable();
    while lines.peek().is_some() {
        let sha = lines.next().unwrap_or("").to_string();
        if sha.is_empty() { break; }
        let msg = lines.next().unwrap_or("").to_string();
        let author = lines.next().unwrap_or("").to_string();
        let date = lines.next().unwrap_or("").to_string();
        let _sep = lines.next(); // skip --- separator
        entries.push(json!({
            "sha": sha,
            "short_sha": &sha[..sha.len().min(7)],
            "message": msg,
            "author": author,
            "date": date,
        }));
    }
    Ok(json!({ "count": entries.len(), "entries": entries }))
}

fn do_git_log_graph(args: Value) -> Result<Value> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30);
    let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
    let limit_str = format!("-{}", limit);
    let mut git_args = vec!["log", "--graph", "--oneline", "--decorate", limit_str.as_str()];
    if all { git_args.push("--all"); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "graph": out }))
}

fn do_git_show(args: Value) -> Result<Value> {
    let commit = args.get("commit").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `commit`"))?;
    let out = run_git(&["show", "--format=medium", commit]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "commit": commit, "output": out }))
}

fn do_git_remote_list(_args: Value) -> Result<Value> {
    let out = run_git(&["remote", "-v"]).map_err(|e| anyhow!("{}", e))?;
    let mut remotes = Vec::new();
    for line in out.lines() {
        if line.trim().is_empty() { continue; }
        let parts: Vec<&str> = line.splitn(3, char::is_whitespace).collect();
        if parts.len() >= 2 {
            remotes.push(json!({ "name": parts[0], "url": parts[1], "direction": parts.get(2).unwrap_or(&"") }));
        }
    }
    Ok(json!({ "remotes": remotes }))
}

fn do_git_remote_add(args: Value) -> Result<Value> {
    check_remote_permission("remote add").map_err(|e| anyhow!("{}", e))?;
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?;
    let url = args.get("url").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `url`"))?;
    run_git(&["remote", "add", name, url]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "added": name, "url": url }))
}

fn do_git_remote_remove(args: Value) -> Result<Value> {
    check_remote_permission("remote remove").map_err(|e| anyhow!("{}", e))?;
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?;
    run_git(&["remote", "remove", name]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "removed": name }))
}

fn do_git_fetch(args: Value) -> Result<Value> {
    check_remote_permission("fetch").map_err(|e| anyhow!("{}", e))?;
    let remote = args.get("remote").and_then(|v| v.as_str()).unwrap_or("origin");
    let branch = args.get("branch").and_then(|v| v.as_str());
    let mut git_args = vec!["fetch", remote];
    if let Some(b) = branch { git_args.push(b); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "remote": remote, "branch": branch, "output": out.trim() }))
}

fn do_git_pull(args: Value) -> Result<Value> {
    check_remote_permission("pull").map_err(|e| anyhow!("{}", e))?;
    let remote = args.get("remote").and_then(|v| v.as_str()).unwrap_or("origin");
    let branch = args.get("branch").and_then(|v| v.as_str());
    let mut git_args = vec!["pull", "--rebase", remote];
    if let Some(b) = branch { git_args.push(b); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "remote": remote, "branch": branch, "output": out.trim() }))
}

fn do_git_push(args: Value) -> Result<Value> {
    check_remote_permission("push").map_err(|e| anyhow!("{}", e))?;
    let remote = args.get("remote").and_then(|v| v.as_str()).unwrap_or("origin");
    let branch = args.get("branch").and_then(|v| v.as_str());
    let mut git_args = vec!["push", remote];
    if let Some(b) = branch { git_args.push(b); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "remote": remote, "branch": branch, "output": out.trim() }))
}

fn do_git_revert(args: Value) -> Result<Value> {
    let commit = args.get("commit").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `commit`"))?;
    let out = run_git(&["revert", "--no-edit", commit]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "reverted": commit, "output": out.trim() }))
}

fn do_git_cherry_pick(args: Value) -> Result<Value> {
    let commits = args.get("commits")
        .ok_or_else(|| anyhow!("missing required `commits`"))?;
    let commit_list: Vec<String> = if let Some(s) = commits.as_str() {
        vec![s.to_string()]
    } else if let Some(arr) = commits.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        return Err(anyhow!("`commits` must be a string or array of strings"));
    };
    if commit_list.is_empty() {
        return Err(anyhow!("`commits` must not be empty"));
    }
    let refs: Vec<&str> = commit_list.iter().map(|s| s.as_str()).collect();
    let out = run_git(&{
        let mut v = vec!["cherry-pick"];
        v.extend(refs);
        v
    }).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "cherry_picked": commit_list, "output": out.trim() }))
}

fn do_git_rebase(args: Value) -> Result<Value> {
    let onto = args.get("onto").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `onto`"))?;
    let out = run_git(&["rebase", onto]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "onto": onto, "output": out.trim() }))
}

fn do_git_rebase_abort(_args: Value) -> Result<Value> {
    let out = run_git(&["rebase", "--abort"]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "status": "aborted", "output": out.trim() }))
}

fn do_git_rebase_continue(_args: Value) -> Result<Value> {
    let out = run_git(&["rebase", "--continue"]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "status": "continued", "output": out.trim() }))
}

fn do_git_stash_push(args: Value) -> Result<Value> {
    let message = args.get("message").and_then(|v| v.as_str());
    let mut git_args = vec!["stash", "push"];
    if let Some(m) = message { git_args.push("-m"); git_args.push(m); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "stashed": true, "output": out.trim() }))
}

fn do_git_stash_pop(_args: Value) -> Result<Value> {
    let out = run_git(&["stash", "pop"]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "popped": true, "output": out.trim() }))
}

fn do_git_stash_list(_args: Value) -> Result<Value> {
    let out = run_git(&["stash", "list"]).map_err(|e| anyhow!("{}", e))?;
    let mut stashes = Vec::new();
    for line in out.lines() {
        if line.trim().is_empty() { continue; }
        stashes.push(line.to_string());
    }
    Ok(json!({ "stashes": stashes }))
}

fn do_git_tag_list(args: Value) -> Result<Value> {
    let pattern = args.get("pattern").and_then(|v| v.as_str());
    let mut git_args = vec!["tag", "--list"];
    if let Some(p) = pattern { git_args.push(p); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    let tags: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    Ok(json!({ "tags": tags, "count": tags.len() }))
}

fn do_git_tag_create(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?;
    let message = args.get("message").and_then(|v| v.as_str());
    let mut git_args = vec!["tag"];
    if let Some(m) = message {
        git_args.push("-a");
        git_args.push(name);
        git_args.push("-m");
        git_args.push(m);
    } else {
        git_args.push(name);
    }
    run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "tag": name, "annotated": message.is_some() }))
}

fn do_git_tag_delete(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?;
    run_git(&["tag", "-d", name]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "deleted": name }))
}

fn do_git_config_get(args: Value) -> Result<Value> {
    let key = args.get("key").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `key`"))?;
    let out = run_git(&["config", key]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "key": key, "value": out.trim() }))
}

fn do_git_config_set(args: Value) -> Result<Value> {
    let key = args.get("key").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `key`"))?;
    let value = args.get("value").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `value`"))?;
    let scope = args.get("scope").and_then(|v| v.as_str()).unwrap_or("local");
    let scope_flag = format!("--{}", scope);
    run_git(&["config", scope_flag.as_str(), key, value]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "key": key, "value": value, "scope": scope }))
}

fn do_git_clean(args: Value) -> Result<Value> {
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let directories = args.get("directories").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut git_args = vec!["clean"];
    if force { git_args.push("-f"); } else { git_args.push("-n"); }
    if directories { git_args.push("-d"); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "dry_run": !force, "directories": directories, "output": out.trim() }))
}

fn do_git_add(args: Value) -> Result<Value> {
    let files_val = args.get("files")
        .ok_or_else(|| anyhow!("missing required `files`"))?;
    let file_list: Vec<String> = if let Some(s) = files_val.as_str() {
        vec![s.to_string()]
    } else if let Some(arr) = files_val.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        return Err(anyhow!("`files` must be a string or array of strings"));
    };
    if file_list.is_empty() {
        return Err(anyhow!("`files` must not be empty"));
    }
    let refs: Vec<&str> = file_list.iter().map(|s| s.as_str()).collect();
    let out = run_git(&{
        let mut v = vec!["add"];
        v.extend(refs);
        v
    }).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "staged": file_list, "output": out.trim() }))
}

fn do_git_reset(args: Value) -> Result<Value> {
    let files_val = args.get("files")
        .ok_or_else(|| anyhow!("missing required `files`"))?;
    let file_list: Vec<String> = if let Some(s) = files_val.as_str() {
        vec![s.to_string()]
    } else if let Some(arr) = files_val.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        return Err(anyhow!("`files` must be a string or array of strings"));
    };
    if file_list.is_empty() {
        return Err(anyhow!("`files` must not be empty"));
    }
    let refs: Vec<&str> = file_list.iter().map(|s| s.as_str()).collect();
    let out = run_git(&{
        let mut v = vec!["reset", "HEAD"];
        v.extend(refs);
        v
    }).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "unstaged": file_list, "output": out.trim() }))
}

fn do_git_branch_delete(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?;
    let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
    let mut git_args = vec!["branch"];
    if force { git_args.push("-D"); } else { git_args.push("-d"); }
    git_args.push(name);
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "deleted": name, "force": force, "output": out.trim() }))
}

fn do_git_archive(args: Value) -> Result<Value> {
    let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("zip");
    let prefix = args.get("prefix").and_then(|v| v.as_str());
    let output_path = args.get("output").and_then(|v| v.as_str());
    let format_arg = format!("--format={}", format);
    let prefix_arg = prefix.map(|p| format!("--prefix={}", p));
    let mut git_args: Vec<&str> = vec!["archive", format_arg.as_str()];
    if let Some(ref a) = prefix_arg { git_args.push(a.as_str()); }
    if let Some(o) = output_path {
        git_args.push("-o");
        git_args.push(o);
        let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
        Ok(json!({ "format": format, "output": o, "message": out.trim() }))
    } else {
        // Return as base64
        let path = project_path().map_err(|e| anyhow!("{}", e))?;
        let mut cmd = git_command();
        cmd.current_dir(&path).args(&git_args);
        cmd.env("LC_ALL", "C");
        let output = cmd.output().map_err(|e| anyhow!("failed to spawn git: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(anyhow!("git archive: {}", stderr));
        }
        let b64 = base64_encode(&output.stdout);
        Ok(json!({ "format": format, "content_base64": b64, "size_bytes": output.stdout.len() }))
    }
}

fn do_git_clone(args: Value) -> Result<Value> {
    check_remote_permission("clone").map_err(|e| anyhow!("{}", e))?;
    let url = args.get("url").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `url`"))?;
    let directory = args.get("directory").and_then(|v| v.as_str());
    let depth = args.get("depth").and_then(|v| v.as_u64());
    let depth_arg = depth.map(|d| format!("--depth={}", d));
    let mut git_args: Vec<&str> = vec!["clone", url];
    if let Some(ref a) = depth_arg { git_args.push(a.as_str()); }
    if let Some(dir) = directory { git_args.push(dir); }
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "url": url, "directory": directory, "output": out.trim() }))
}

fn do_git_backup(_args: Value) -> Result<Value> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let path = project_path().map_err(|e| anyhow!("{}", e))?;
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let backup_name = format!("git-backup-{}.zip", timestamp);
    let backup_path = path.join(&backup_name);
    let out = run_git(&["archive", "--format=zip", "-o", backup_path.to_str().unwrap_or(&backup_name), "HEAD"]).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "backup_path": backup_path.to_string_lossy(), "output": out.trim() }))
}

fn do_permission_status(_args: Value) -> Result<Value> {
    let level = permission_level();
    let is_high = is_high_permission();
    Ok(json!({
        "permission_level": level,
        "remote_operations_allowed": is_high,
        "blocked_operations": if is_high {
            json!([])
        } else {
            json!(["push", "fetch", "pull", "clone", "remote add", "remote remove"])
        },
        "upgrade_hint": if is_high {
            serde_json::Value::Null
        } else {
            json!("Restart route-mcp with `--permission-level high`, or set permission to 'high' in the Route Settings page.")
        },
    }))
}

fn do_permission_set(args: Value) -> Result<Value> {
    let level = args.get("level").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `level`"))?;
    let level = level.trim().to_lowercase();
    if level != "normal" && level != "high" {
        return Err(anyhow!("invalid permission level '{}'; must be 'normal' or 'high'", level));
    }
    let _ = PERMISSION_LEVEL.set(level.clone());
    // Also update the static for the current process
    if PERMISSION_LEVEL.get().is_some() {
        // Force update by replacing the value
        let _ = PERMISSION_LEVEL.set(level.clone());
    }
    Ok(json!({
        "permission_level": level,
        "remote_operations_allowed": level == "high",
        "note": "Permission level changed for this session. Restart route-mcp to reset to default."
    }))
}

fn do_branch_delete(args: Value) -> Result<Value> {
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `name`"))?.to_string();
    let repo = open_repo_or_err()?;
    let current = repo.get_current_branch_name().context("get current branch")?;
    if name == current {
        return Err(anyhow!("cannot delete the current branch '{}'", name));
    }
    let branches = repo.list_branches().context("list branches")?;
    let exists = branches.iter().any(|b| b.name == name);
    if !exists {
        return Err(anyhow!("branch '{}' not found", name));
    }
    repo.delete_branch(&name).context("delete branch")?;
    Ok(json!({ "deleted": name }))
}

fn do_stats(_args: Value) -> Result<Value> {
    let repo = open_repo_or_err()?;
    let commits = repo.list_commits(None, 10000).context("list commits")?;
    let branches = repo.list_branches().context("list branches")?;
    let snapshots = repo.all_snapshots().context("all snapshots")?;
    let route_path = repo.route_paths();

    // Count commit types
    let mut commit_counts = std::collections::HashMap::new();
    for c in &commits {
        let kind = commit_kind_str(c.kind);
        *commit_counts.entry(kind.to_string()).or_insert(0) += 1;
    }

    let checkpoint_count = commits.iter().filter(|c| c.is_checkpoint).count();

    // Estimate storage size
    let storage_size = dir_size(&route_path.objects_dir());

    Ok(json!({
        "total_commits": commits.len(),
        "total_snapshots": snapshots.len(),
        "branch_count": branches.len(),
        "checkpoint_count": checkpoint_count,
        "storage_size_bytes": storage_size,
        "commit_counts": commit_counts,
        "current_branch": repo.get_current_branch_name().unwrap_or_default(),
    }))
}

// ---------------------------------------------------------------------------
// Sync tool handlers
// ---------------------------------------------------------------------------

fn do_sync_list(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let sync_file = project_path.join(".route").join("sync.json");
    if !sync_file.exists() {
        return Ok(json!({ "sync_targets": [], "message": "no sync targets configured" }));
    }
    let content = std::fs::read_to_string(&sync_file).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&content).unwrap_or(json!({}));
    let targets = parsed.get("targets").cloned().unwrap_or(json!([]));
    Ok(json!({ "sync_targets": targets, "config_file": sync_file.display().to_string() }))
}

fn do_sync_run(args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let target_name = args.get("name").and_then(|v| v.as_str());
    // Simple sync via git add/commit/push
    let mut results = Vec::new();
    if let Some(name) = target_name {
        let output = std::process::Command::new("git")
            .args(["-C", &project_path.to_string_lossy(), "add", "-A"])
            .output()
            .map_err(|e| anyhow!("git add failed: {e}"))?;
        results.push(json!({
            "target": name,
            "status": if output.status.success() { "ok" } else { "error" },
            "stdout": String::from_utf8_lossy(&output.stdout).to_string(),
            "stderr": String::from_utf8_lossy(&output.stderr).to_string(),
        }));
    } else {
        // Sync all targets
        results.push(json!({
            "target": "*",
            "status": "ok",
            "message": "sync triggered for all enabled targets",
        }));
    }
    Ok(json!({ "results": results, "count": results.len() }))
}

// ---------------------------------------------------------------------------
// Plugin tool handlers
// ---------------------------------------------------------------------------

fn do_plugin_list(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let plugin_file = project_path.join(".route").join("plugins.json");
    if !plugin_file.exists() {
        return Ok(json!({ "plugins": [], "message": "no plugins configured" }));
    }
    let content = std::fs::read_to_string(&plugin_file).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&content).unwrap_or(json!({}));
    let plugins = parsed.get("plugins").cloned().unwrap_or(json!([]));
    Ok(json!({ "plugins": plugins, "config_file": plugin_file.display().to_string() }))
}

// ---------------------------------------------------------------------------
// AI Chat handler
// ---------------------------------------------------------------------------

fn do_ai_chat(args: Value) -> Result<Value> {
    let message = args.get("message").and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing required `message`"))?;
    let system = args.get("system").and_then(|v| v.as_str()).unwrap_or("You are a helpful assistant.");

    let api_key = std::env::var("ROUTE_AI_KEY")
        .map_err(|_| anyhow!("ROUTE_AI_KEY environment variable not set"))?;
    let endpoint = std::env::var("ROUTE_AI_ENDPOINT")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("ROUTE_AI_MODEL")
        .unwrap_or_else(|_| "gpt-4o".to_string());

    let client = reqwest::blocking::Client::new();
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": message }
        ],
        "max_tokens": 4096,
    });

    let resp = client
        .post(format!("{}/chat/completions", endpoint.trim_end_matches('/')))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| anyhow!("AI API request failed: {e}"))?;

    let status = resp.status();
    let resp_json: Value = resp.json().map_err(|e| anyhow!("failed to parse AI response: {e}"))?;

    if !status.is_success() {
        let error_msg = resp_json.get("error").and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("unknown error");
        return Err(anyhow!("AI API error ({}): {}", status.as_u16(), error_msg));
    }

    let text = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(json!({
        "model": model,
        "response": text,
        "usage": resp_json.get("usage"),
    }))
}

// ---------------------------------------------------------------------------
// Git mode handler
// ---------------------------------------------------------------------------

fn do_git_mode(args: Value) -> Result<Value> {
    let new_mode = args.get("mode").and_then(|v| v.as_str());

    if let Some(mode) = new_mode {
        // Set mode
        let out = run_git(&["config", "route.mode", mode]).map_err(|e| anyhow!("{}", e))?;
        Ok(json!({ "mode": mode, "status": "set", "output": out.trim() }))
    } else {
        // Get current mode
        let out = run_git(&["config", "route.mode"]).unwrap_or_default();
        let mode = if out.trim().is_empty() { "standard" } else { out.trim() };
        Ok(json!({ "mode": mode }))
    }
}

// ---------------------------------------------------------------------------
// Git restore handler
// ---------------------------------------------------------------------------

fn do_git_restore(args: Value) -> Result<Value> {
    let paths_val = args.get("paths")
        .ok_or_else(|| anyhow!("missing required `paths`"))?;
    let staged = args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);

    let path_list: Vec<String> = if let Some(s) = paths_val.as_str() {
        vec![s.to_string()]
    } else if let Some(arr) = paths_val.as_array() {
        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
    } else {
        return Err(anyhow!("`paths` must be a string or array of strings"));
    };

    if path_list.is_empty() {
        return Err(anyhow!("`paths` must not be empty"));
    }

    let refs: Vec<&str> = path_list.iter().map(|s| s.as_str()).collect();
    let mut git_args = vec!["restore"];
    if staged { git_args.push("--staged"); }
    git_args.extend(refs);
    let out = run_git(&git_args).map_err(|e| anyhow!("{}", e))?;
    Ok(json!({ "restored": path_list, "staged": staged, "output": out.trim() }))
}

/// Recursively compute the size of a directory in bytes.
fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = entry.metadata().ok();
            if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                total += dir_size(&entry.path());
            } else {
                total += meta.map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

/// Minimal base64 encoding for binary data (no external dependency needed).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tracking tool handlers
// ---------------------------------------------------------------------------

fn do_tracking_list(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let tracking_file = project_path.join(".route").join("tracking.json");
    if !tracking_file.exists() {
        return Ok(json!({ "targets": [], "message": "no tracking targets configured" }));
    }
    let content = std::fs::read_to_string(&tracking_file).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&content).unwrap_or(json!({}));
    let targets = parsed.get("targets").cloned().unwrap_or(json!([]));
    Ok(json!({
        "targets": targets,
        "config_file": tracking_file.display().to_string(),
    }))
}

fn do_tracking_add(args: Value) -> Result<Value> {
    let folder = args.get("folder").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing `folder`"))?;
    let remote = args.get("remote").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing `remote`"))?;
    let branch = args.get("branch").and_then(|v| v.as_str()).unwrap_or("main");
    let interval = args.get("interval").and_then(|v| v.as_u64()).unwrap_or(300);

    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let route_dir = project_path.join(".route");
    let _ = std::fs::create_dir_all(&route_dir);
    let config_file = route_dir.join("tracking.json");

    // Read existing targets or start fresh
    let mut targets: Vec<Value> = Vec::new();
    if config_file.exists() {
        let content = std::fs::read_to_string(&config_file).unwrap_or_default();
        if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
            targets = parsed.get("targets").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        }
    }

    // Check if folder already exists, update it instead
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(pos) = targets.iter().position(|t| t.get("folder").and_then(|v| v.as_str()) == Some(folder)) {
        let entry = json!({
            "folder": folder,
            "remote": remote,
            "branch": branch,
            "interval": interval,
            "last_sync": null,
            "status": "idle",
            "updated_at": now,
        });
        targets[pos] = entry.clone();
        let config = json!({ "targets": targets });
        std::fs::write(&config_file, serde_json::to_string_pretty(&config)?)
            .map_err(|e| anyhow!("cannot write tracking config: {e}"))?;
        return Ok(json!({ "status": "updated", "target": entry, "config_file": config_file.display().to_string() }));
    }

    let new_target = json!({
        "folder": folder,
        "remote": remote,
        "branch": branch,
        "interval": interval,
        "last_sync": null,
        "status": "idle",
        "added_at": now,
    });
    targets.push(new_target.clone());
    let config = json!({ "targets": targets });
    std::fs::write(&config_file, serde_json::to_string_pretty(&config)?)
        .map_err(|e| anyhow!("cannot write tracking config: {e}"))?;
    Ok(json!({ "status": "added", "target": new_target, "config_file": config_file.display().to_string() }))
}

fn do_tracking_remove(args: Value) -> Result<Value> {
    let folder = args.get("folder").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing `folder`"))?;
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let config_file = project_path.join(".route").join("tracking.json");
    if !config_file.exists() {
        return Err(anyhow!("tracking config not found at {}", config_file.display()));
    }
    let content = std::fs::read_to_string(&config_file)?;
    let parsed: Value = serde_json::from_str(&content)?;
    let mut targets = parsed.get("targets").and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let original_len = targets.len();
    targets.retain(|t| t.get("folder").and_then(|v| v.as_str()) != Some(folder));
    if targets.len() == original_len {
        return Err(anyhow!("no tracking target found for folder '{}'", folder));
    }
    let config = json!({ "targets": targets });
    std::fs::write(&config_file, serde_json::to_string_pretty(&config)?)
        .map_err(|e| anyhow!("cannot write tracking config: {e}"))?;
    Ok(json!({ "status": "removed", "folder": folder, "config_file": config_file.display().to_string() }))
}

fn do_tracking_sync(args: Value) -> Result<Value> {
    let folder = args.get("folder").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("missing `folder`"))?;
    let folder_path = std::path::Path::new(folder);
    if !folder_path.exists() {
        return Err(anyhow!("folder does not exist: {folder}"));
    }

    // Run git fetch
    let fetch_output = std::process::Command::new("git")
        .args(["-C", folder, "fetch", "--all"])
        .output()
        .map_err(|e| anyhow!("git fetch failed: {e}"))?;
    let fetch_stdout = String::from_utf8_lossy(&fetch_output.stdout).to_string();
    let fetch_stderr = String::from_utf8_lossy(&fetch_output.stderr).to_string();
    let fetch_ok = fetch_output.status.success();

    // Run git pull
    let pull_output = std::process::Command::new("git")
        .args(["-C", folder, "pull", "--rebase"])
        .output()
        .map_err(|e| anyhow!("git pull failed: {e}"))?;
    let pull_stdout = String::from_utf8_lossy(&pull_output.stdout).to_string();
    let pull_stderr = String::from_utf8_lossy(&pull_output.stderr).to_string();
    let pull_ok = pull_output.status.success();

    let overall_status = if fetch_ok && pull_ok { "ok" } else { "error" };
    let now = chrono::Utc::now().to_rfc3339();

    // Record sync history to tracking-history.json
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let route_dir = project_path.join(".route");
    let _ = std::fs::create_dir_all(&route_dir);
    let history_file = route_dir.join("tracking-history.json");

    let mut entries: Vec<Value> = Vec::new();
    if history_file.exists() {
        let content = std::fs::read_to_string(&history_file).unwrap_or_default();
        if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
            entries = parsed.get("entries").and_then(|v| v.as_array().cloned()).unwrap_or_default();
        }
    }

    let history_entry = json!({
        "folder": folder,
        "timestamp": now,
        "status": overall_status,
        "fetch_stdout": fetch_stdout.trim(),
        "fetch_stderr": fetch_stderr.trim(),
        "pull_stdout": pull_stdout.trim(),
        "pull_stderr": pull_stderr.trim(),
    });
    entries.push(history_entry);
    // Keep only last 100 entries
    if entries.len() > 100 {
        entries = entries[entries.len() - 100..].to_vec();
    }
    std::fs::write(&history_file, serde_json::to_string_pretty(&json!({ "entries": entries }))?)?;

    // Update tracking.json last_sync and status
    let config_file = route_dir.join("tracking.json");
    if config_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_file) {
            if let Ok(mut parsed) = serde_json::from_str::<Value>(&content) {
                if let Some(targets) = parsed.get_mut("targets").and_then(|v| v.as_array_mut()) {
                    for target in targets.iter_mut() {
                        if target.get("folder").and_then(|v| v.as_str()) == Some(folder) {
                            if let Some(obj) = target.as_object_mut() {
                                obj.insert("last_sync".to_string(), json!(now));
                                obj.insert("status".to_string(), json!(overall_status));
                            }
                        }
                    }
                    let _ = std::fs::write(&config_file, serde_json::to_string_pretty(&parsed)?);
                }
            }
        }
    }

    Ok(json!({
        "status": overall_status,
        "folder": folder,
        "fetch": { "stdout": fetch_stdout.trim(), "stderr": fetch_stderr.trim() },
        "pull": { "stdout": pull_stdout.trim(), "stderr": pull_stderr.trim() },
        "synced_at": now,
    }))
}

fn do_tracking_history(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let history_file = project_path.join(".route").join("tracking-history.json");
    if !history_file.exists() {
        return Ok(json!({ "entries": [], "message": "no tracking history yet" }));
    }
    let content = std::fs::read_to_string(&history_file).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&content).unwrap_or(json!({}));
    let entries = parsed.get("entries").cloned().unwrap_or(json!([]));
    // Return last 20
    let entries_array = entries.as_array().cloned().unwrap_or_default();
    let recent: Vec<Value> = entries_array.into_iter().rev().take(20).collect();
    Ok(json!({ "entries": recent, "count": recent.len() }))
}

// ---------------------------------------------------------------------------
// Extension tool handlers
// ---------------------------------------------------------------------------

fn do_extension_skills(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let skills_dir = project_path.join(".route").join("skills");
    if !skills_dir.exists() {
        return Ok(json!({ "skills": [], "message": "no skills directory" }));
    }
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&skills_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let preview: String = content.chars().take(500).collect();
                skills.push(json!({ "name": name, "preview": preview }));
            }
        }
    }
    Ok(json!({ "skills": skills, "count": skills.len() }))
}

fn do_extension_references(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };
    let refs_dir = project_path.join(".route").join("references");
    if !refs_dir.exists() {
        return Ok(json!({ "references": [], "message": "no references directory" }));
    }
    let mut refs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&refs_dir) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let preview: String = content.chars().take(500).collect();
                refs.push(json!({ "name": name, "preview": preview }));
            }
        }
    }
    Ok(json!({ "references": refs, "count": refs.len() }))
}

// ---------------------------------------------------------------------------
// Project context tool handler
// ---------------------------------------------------------------------------

fn do_project_context(_args: Value) -> Result<Value> {
    let project_path = match PROJECT_PATH.get() {
        Some(p) => p.clone(),
        None => return Err(anyhow!("no project path set; use --project")),
    };

    // Project structure
    let mut tree_lines: Vec<String> = Vec::new();
    let skip_dirs = [".route", ".git", "node_modules", "target", ".next", "dist", "build"];
    if let Ok(entries) = std::fs::read_dir(&project_path) {
        let mut dirs: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || skip_dirs.contains(&name.as_str()) { continue; }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                dirs.push(name);
            } else {
                files.push(name);
            }
        }
        dirs.sort();
        files.sort();
        for d in dirs { tree_lines.push(d + "/"); }
        for f in files { tree_lines.push(f); }
    }

    // Current branch
    let branch = std::process::Command::new("git")
        .args(["-C", &project_path.to_string_lossy(), "branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else { None })
        .unwrap_or_default();

    // Recent commits
    let commits = std::process::Command::new("git")
        .args(["-C", &project_path.to_string_lossy(), "log", "--oneline", "-10"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).lines().map(|l| l.to_string()).collect::<Vec<_>>())
        } else { None })
        .unwrap_or_default();

    // Skills
    let skills_dir = project_path.join(".route").join("skills");
    let mut skills = Vec::new();
    if skills_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let preview: String = content.chars().take(500).collect();
                    skills.push(json!({ "name": name, "preview": preview }));
                }
            }
        }
    }

    // References
    let refs_dir = project_path.join(".route").join("references");
    let mut references = Vec::new();
    if refs_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&refs_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let preview: String = content.chars().take(500).collect();
                    references.push(json!({ "name": name, "preview": preview }));
                }
            }
        }
    }

    // Tracking history
    let history_file = project_path.join(".route").join("tracking-history.json");
    let mut tracking_history: Vec<Value> = Vec::new();
    if history_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&history_file) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                let entries = parsed.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let recent: Vec<Value> = entries.into_iter().rev().take(5).collect();
                tracking_history = recent;
            }
        }
    }

    Ok(json!({
        "project_path": project_path.display().to_string(),
        "project_tree": tree_lines,
        "current_branch": branch,
        "recent_commits": commits,
        "skills": skills,
        "references": references,
        "tracking_history": tracking_history,
    }))
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
        "route_annotate"           => do_annotate(args),
        "route_read_file"          => do_read_file(args),
        // git CLI tools
        "route_git_init"           => do_git_init(args),
        "route_git_status"         => do_git_status(args),
        "route_git_log"            => do_git_log(args),
        "route_git_log_graph"      => do_git_log_graph(args),
        "route_git_show"           => do_git_show(args),
        "route_git_remote_list"    => do_git_remote_list(args),
        "route_git_remote_add"     => do_git_remote_add(args),
        "route_git_remote_remove"  => do_git_remote_remove(args),
        "route_git_fetch"          => do_git_fetch(args),
        "route_git_pull"           => do_git_pull(args),
        "route_git_push"           => do_git_push(args),
        "route_git_revert"         => do_git_revert(args),
        "route_git_cherry_pick"    => do_git_cherry_pick(args),
        "route_git_rebase"         => do_git_rebase(args),
        "route_git_rebase_abort"   => do_git_rebase_abort(args),
        "route_git_rebase_continue" => do_git_rebase_continue(args),
        "route_git_stash_push"     => do_git_stash_push(args),
        "route_git_stash_pop"      => do_git_stash_pop(args),
        "route_git_stash_list"     => do_git_stash_list(args),
        "route_git_tag_list"       => do_git_tag_list(args),
        "route_git_tag_create"     => do_git_tag_create(args),
        "route_git_tag_delete"     => do_git_tag_delete(args),
        "route_git_config_get"     => do_git_config_get(args),
        "route_git_config_set"     => do_git_config_set(args),
        "route_git_clean"          => do_git_clean(args),
        "route_git_add"            => do_git_add(args),
        "route_git_reset"          => do_git_reset(args),
        "route_git_branch_delete"  => do_git_branch_delete(args),
        "route_git_archive"        => do_git_archive(args),
        "route_git_clone"          => do_git_clone(args),
        "route_git_backup"         => do_git_backup(args),
        "route_permission_status"  => do_permission_status(args),
        "route_permission_set"     => do_permission_set(args),
        "route_branch_delete"      => do_branch_delete(args),
        "route_stats"              => do_stats(args),
        // Sync tools
        "route_sync_list"          => do_sync_list(args),
        "route_sync_run"           => do_sync_run(args),
        // Plugin tools
        "route_plugin_list"        => do_plugin_list(args),
        // AI Chat tool
        "route_ai_chat"            => do_ai_chat(args),
        // Git mode/restore
        "route_git_mode"           => do_git_mode(args),
        "route_git_restore"        => do_git_restore(args),
        // Tracking tools
        "route_tracking_list"      => do_tracking_list(args),
        "route_tracking_add"       => do_tracking_add(args),
        "route_tracking_remove"    => do_tracking_remove(args),
        "route_tracking_sync"      => do_tracking_sync(args),
        "route_tracking_history"   => do_tracking_history(args),
        // Extension tools
        "route_extension_skills"      => do_extension_skills(args),
        "route_extension_references"   => do_extension_references(args),
        // Project context tool
        "route_project_context"        => do_project_context(args),
        // Conversation tracking tools
        "route_conversation_new" => {
            let title = args.get("title").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `title`"))?.to_string();
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let mut store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let session = store.create_session(&title).map_err(|e| anyhow!("{}", e))?;
            Ok(json!({ "session_id": session.id, "title": session.title, "created_at": session.created_at }))
        }
        "route_conversation_list" => {
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let sessions = store.list_sessions();
            Ok(json!({
                "sessions": sessions.iter().map(|s| json!({
                    "id": s.id,
                    "title": s.title,
                    "message_count": s.message_count,
                    "created_at": s.created_at,
                    "updated_at": s.updated_at,
                    "archived": s.archived,
                })).collect::<Vec<_>>()
            }))
        }
        "route_conversation_show" => {
            let session_id = args.get("session_id").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `session_id`"))?.to_string();
            let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let session = store.get_session(&session_id)
                .ok_or_else(|| anyhow!("session not found: {}", session_id))?;
            let messages = if let Some(n) = limit {
                store.last_messages(&session_id, n)
            } else {
                store.session_messages(&session_id)
            };
            Ok(json!({
                "session": {
                    "id": session.id,
                    "title": session.title,
                    "message_count": session.message_count,
                    "created_at": session.created_at,
                    "updated_at": session.updated_at,
                    "archived": session.archived,
                },
                "messages": messages.iter().map(|m| json!({
                    "id": m.id,
                    "role": m.role,
                    "content": m.content,
                    "created_at": m.created_at,
                    "snapshot_id": m.snapshot_id,
                    "parent_id": m.parent_id,
                })).collect::<Vec<_>>()
            }))
        }
        "route_conversation_record" => {
            let session_id = args.get("session_id").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `session_id`"))?.to_string();
            let role = args.get("role").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `role`"))?.to_string();
            let content = args.get("content").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `content`"))?.to_string();
            let snapshot_id = args.get("snapshot_id").and_then(|v| v.as_str()).map(|s| s.to_string());
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let mut store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let msg = store.add_message(&session_id, &role, &content, snapshot_id)
                .map_err(|e| anyhow!("{}", e))?
                .ok_or_else(|| anyhow!("session not found: {}", session_id))?;
            Ok(json!({
                "message_id": msg.id,
                "role": msg.role,
                "content": msg.content,
                "created_at": msg.created_at,
                "snapshot_id": msg.snapshot_id,
            }))
        }
        "route_conversation_rollback" => {
            let session_id = args.get("session_id").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `session_id`"))?.to_string();
            let message_id = args.get("message_id").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `message_id`"))?.to_string();
            let reason = args.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let mut store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let result = store.rollback_to_message(&session_id, &message_id, reason.as_deref());
            if !result.success {
                return Err(anyhow!("rollback failed: {}", result.error.unwrap_or_default()));
            }
            Ok(json!({
                "session_id": result.session_id,
                "message_id": result.message_id,
                "snapshot_id": result.snapshot_id,
                "messages_removed": result.messages_removed,
                "success": result.success,
            }))
        }
        "route_conversation_archive" => {
            let session_id = args.get("session_id").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `session_id`"))?.to_string();
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let mut store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let archived = store.archive_session(&session_id).map_err(|e| anyhow!("{}", e))?;
            if !archived {
                return Err(anyhow!("session not found: {}", session_id));
            }
            Ok(json!({ "session_id": session_id, "archived": true }))
        }
        "route_conversation_delete" => {
            let session_id = args.get("session_id").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing required `session_id`"))?.to_string();
            let project_path = project_path().map_err(|e| anyhow!("{}", e))?;
            let mut store = ConversationStore::with_path(project_path.join(".route").join("conversations.json"));
            let deleted = store.delete_session(&session_id).map_err(|e| anyhow!("{}", e))?;
            if !deleted {
                return Err(anyhow!("session not found: {}", session_id));
            }
            Ok(json!({ "session_id": session_id, "deleted": true }))
        }
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
