//! Permission system — controls which operations are allowed based on
//! the current permission level.
//!
//! ## Levels
//!
//! - **Normal** (default for CLI/MCP): All local git operations are
//!   allowed. Remote-configuration and push operations are **blocked**.
//! - **High**: All operations are allowed, including remote config,
//!   push, clone, and config-set for remote URLs. Intended for
//!   advanced users who explicitly opt in.
//!
//! ## Rationale
//!
//! Route's CLI and MCP interfaces are designed for AI agents. Without
//! a permission guard, a misbehaving or hijacked AI could push
//! sensitive data to an attacker-controlled remote. The Normal level
//! prevents this by default. The High level is a conscious opt-in
//! that the user must explicitly enable.
//!
//! ## Frontend integration
//!
//! The GUI always runs at the High level because the human is
//! physically present and can see every push/branch delete. The
//! setting is exposed on the Settings page so the user can toggle
//! the CLI/MCP level.

use serde::{Deserialize, Serialize};

/// Permission level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    /// Normal: local operations allowed, remote operations blocked.
    Normal,
    /// High: all operations allowed.
    High,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl PermissionLevel {
    /// Returns `true` if the given git operation is allowed at this
    /// level.
    pub fn allows(&self, op: GitOperation) -> bool {
        match self {
            PermissionLevel::High => true,
            PermissionLevel::Normal => !op.is_remote_operation(),
        }
    }
}

/// A git operation that can be checked against the permission level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitOperation {
    // --- Remote operations (blocked in Normal mode) ---
    RemoteAdd,
    RemoteRemove,
    Push,
    PushSetUpstream,
    Fetch,
    Pull,
    Clone,
    ConfigSetRemote,

    // --- Local operations (always allowed) ---
    Init,
    Add,
    Reset,
    Commit,
    Revert,
    CherryPick,
    BranchCreate,
    BranchDelete,
    BranchSwitch,
    Merge,
    Rebase,
    RebaseAbort,
    RebaseContinue,
    StashPush,
    StashPop,
    Clean,
    Show,
    Log,
    LogGraph,
    Diff,
    Status,
    TagCreate,
    TagDelete,
    ConfigGet,
    ConfigSetOther,
    Archive,
    Backup,
    Restore,
    Checkpoint,
}

impl GitOperation {
    /// Returns `true` if this operation touches a remote.
    pub fn is_remote_operation(self) -> bool {
        matches!(
            self,
            GitOperation::RemoteAdd
                | GitOperation::RemoteRemove
                | GitOperation::Push
                | GitOperation::PushSetUpstream
                | GitOperation::Fetch
                | GitOperation::Pull
                | GitOperation::Clone
                | GitOperation::ConfigSetRemote
        )
    }

    /// Human-readable name for error messages.
    pub fn label(self) -> &'static str {
        match self {
            GitOperation::RemoteAdd => "remote add",
            GitOperation::RemoteRemove => "remote remove",
            GitOperation::Push => "push",
            GitOperation::PushSetUpstream => "push -u",
            GitOperation::Fetch => "fetch",
            GitOperation::Pull => "pull",
            GitOperation::Clone => "clone",
            GitOperation::ConfigSetRemote => "config set remote",
            GitOperation::Init => "init",
            GitOperation::Add => "add",
            GitOperation::Reset => "reset",
            GitOperation::Commit => "commit",
            GitOperation::Revert => "revert",
            GitOperation::CherryPick => "cherry-pick",
            GitOperation::BranchCreate => "branch create",
            GitOperation::BranchDelete => "branch delete",
            GitOperation::BranchSwitch => "branch switch",
            GitOperation::Merge => "merge",
            GitOperation::Rebase => "rebase",
            GitOperation::RebaseAbort => "rebase abort",
            GitOperation::RebaseContinue => "rebase continue",
            GitOperation::StashPush => "stash push",
            GitOperation::StashPop => "stash pop",
            GitOperation::Clean => "clean",
            GitOperation::Show => "show",
            GitOperation::Log => "log",
            GitOperation::LogGraph => "log graph",
            GitOperation::Diff => "diff",
            GitOperation::Status => "status",
            GitOperation::TagCreate => "tag create",
            GitOperation::TagDelete => "tag delete",
            GitOperation::ConfigGet => "config get",
            GitOperation::ConfigSetOther => "config set",
            GitOperation::Archive => "archive",
            GitOperation::Backup => "backup",
            GitOperation::Restore => "restore",
            GitOperation::Checkpoint => "checkpoint",
        }
    }
}

/// Check if `op` is allowed at the current permission level stored in
/// the Tauri state. Returns `Ok(())` or an error message listing the
/// blocked operation and how to enable High mode.
pub fn check_git_permission(
    level: PermissionLevel,
    op: GitOperation,
) -> Result<(), String> {
    if level.allows(op) {
        Ok(())
    } else {
        Err(format!(
            "Operation '{op}' is blocked at the current permission level.\n\
             Remote operations are restricted in Normal mode.\n\
             To enable: set permission level to 'high' in Settings or call \
             `set_permission_level high`.",
            op = op.label()
        ))
    }
}

/// Serialize the permission level for the frontend.
pub fn permission_level_label(level: PermissionLevel) -> &'static str {
    match level {
        PermissionLevel::Normal => "normal",
        PermissionLevel::High => "high",
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

use tauri::State;
use crate::state::AppState;

/// Get the current permission level.
#[tauri::command]
pub fn get_permission_level(state: State<'_, AppState>) -> String {
    let level = state.permission_level.lock()
        .unwrap_or_else(|p| p.into_inner());
    permission_level_label(*level).to_string()
}

/// Set the permission level. Accepts `"normal"` or `"high"`.
/// Returns the previous level.
#[tauri::command]
pub fn set_permission_level(state: State<'_, AppState>, level: String) -> Result<String, String> {
    let new_level = match level.trim().to_lowercase().as_str() {
        "normal" => PermissionLevel::Normal,
        "high" => PermissionLevel::High,
        other => return Err(format!("invalid permission level: '{other}'. Use 'normal' or 'high'.")),
    };
    let mut guard = state.permission_level.lock()
        .map_err(|e| format!("lock error: {e}"))?;
    let old = permission_level_label(*guard).to_string();
    *guard = new_level;
    Ok(old)
}