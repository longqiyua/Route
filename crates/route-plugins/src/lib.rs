//! Route plugin runtime.
//!
//! Provides an in-process event bus and a `Plugin` trait that native Rust
//! plugins implement. WASM-based plugins will be added in a future iteration
//! behind the same `Plugin` trait.
//!
//! # Design
//!
//! - **Event**: strongly-typed enum describing lifecycle events emitted by
//!   route-core / route-basic / route-sync (commit, rollback, sync, etc.).
//! - **PluginContext**: read-only view of repo state passed to plugins when
//!   an event is dispatched.
//! - **Plugin**: trait object that subscribes to events. Implementations
//!   decide which events they care about (return early otherwise).
//! - **EventBus**: synchronous in-process pub/sub. Maintains a list of
//!   registered plugins and dispatches events to all of them. Plugin errors
//!   are collected and surfaced to the caller, but do not stop the pipeline
//!   (a misbehaving plugin should never break a commit).
//! - **DispatchResult**: returned from `EventBus::publish`, summarising how
//!   many plugins succeeded vs failed.

pub mod bus;
pub mod config;
pub mod context;
pub mod events;
pub mod plugins;

pub use bus::{DispatchResult, EventBus};
pub use config::{
    build_bus_from_config, load_config as load_plugin_config, plugins_config_path,
    save_config as save_plugin_config, validate_builtin as validate_plugin_builtin,
    validate_config as validate_plugin_config, BUILTIN_PLUGINS, PluginEntry, PluginsFile,
};
pub use context::PluginContext;
pub use events::{BranchKind, CommitKind, Event, EventKind};
pub use plugins::{
    builtin::{LoggerPlugin, WebhookPlugin, WebhookConfig},
    Plugin,
};
