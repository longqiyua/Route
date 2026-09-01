//! Route CLI library — exposes CLI command functions for reuse by other crates.

pub mod commands;
pub mod git_commands;
pub mod plugin_commands;
/// Host-neutral `route/1` parser/dispatcher; transport lives in the binary.
pub mod rpc;
pub mod sync_commands;
