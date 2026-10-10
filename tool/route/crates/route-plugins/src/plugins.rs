//! Plugin trait and built-in plugin implementations.

pub mod builtin;

use anyhow::Result;

use crate::{Event, PluginContext};

/// A Route plugin.
///
/// Plugins subscribe to events via [`crate::EventBus::subscribe`]. The bus
/// calls `handle_event` for every event; plugins decide internally which
/// events they care about (returning `Ok(())` immediately for others).
///
/// Implementations must be `Send + Sync` because the bus can be shared across
/// threads. Stateful plugins should use interior mutability (e.g. atomics or
/// `Mutex`-guarded fields).
///
/// # Errors
///
/// Returning `Err` from `handle_event` is non-fatal — the bus collects the
/// error and continues dispatching to other plugins. A misbehaving plugin
/// should never break a commit or rollback.
pub trait Plugin: Send + Sync {
    /// Human-readable plugin name (used in logs and CLI output).
    fn name(&self) -> &str;

    /// Handle an event. See trait docs for error semantics.
    fn handle_event(&self, event: &Event, ctx: &PluginContext) -> Result<()>;

    /// Whether the plugin should receive events. Defaults to `true`.
    fn enabled(&self) -> bool {
        true
    }

    /// Optional: human-readable description for `route plugin list`.
    fn description(&self) -> &str {
        ""
    }
}
