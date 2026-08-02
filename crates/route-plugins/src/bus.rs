//! Event bus — synchronous in-process pub/sub for plugins.

use std::sync::Mutex;

use crate::{Event, Plugin, PluginContext};

/// Outcome of dispatching an event to all registered plugins.
///
/// Plugin failures are non-fatal: the bus collects every error and lets
/// remaining plugins run. The caller can decide whether to log, abort, or
/// ignore.
#[derive(Debug, Clone, Default)]
pub struct DispatchResult {
    pub delivered: usize,
    pub succeeded: usize,
    pub failed: usize,
    /// `(plugin_name, error_message)` pairs for plugins that errored.
    pub errors: Vec<(String, String)>,
}

impl DispatchResult {
    pub fn is_ok(&self) -> bool {
        self.failed == 0
    }
}

/// In-process event bus.
///
/// Holds a list of registered plugins behind a `Mutex` so it can be shared
/// across threads (the bus itself is `Send + Sync`). Dispatch is synchronous
/// — `publish` only returns once every plugin has run. Plugins that need to
/// do I/O should either be fast or push work onto their own background thread.
///
/// # Reentrancy
///
/// A plugin MUST NOT call `EventBus::publish` from within its own
/// `handle_event` — `std::sync::Mutex` is not reentrant and doing so will
/// deadlock. If you need to chain events, return a sentinel from
/// `handle_event` and let the caller republish.
pub struct EventBus {
    plugins: Mutex<Vec<Box<dyn Plugin>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            plugins: Mutex::new(Vec::new()),
        }
    }

    /// Register a plugin. The bus takes ownership.
    pub fn subscribe(&self, plugin: Box<dyn Plugin>) {
        let mut plugins = self.plugins.lock().expect("plugin mutex poisoned");
        plugins.push(plugin);
    }

    /// Number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.lock().expect("plugin mutex poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// List the names of all registered plugins (for diagnostics / CLI).
    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins
            .lock()
            .expect("plugin mutex poisoned")
            .iter()
            .map(|p| p.name().to_string())
            .collect()
    }

    /// Dispatch an event to every enabled plugin.
    ///
    /// Disabled plugins are skipped. Errors are collected but never abort
    /// the dispatch — one broken plugin should not silence the others.
    pub fn publish(&self, event: &Event, ctx: &PluginContext) -> DispatchResult {
        let plugins = self.plugins.lock().expect("plugin mutex poisoned");
        let mut result = DispatchResult {
            delivered: 0,
            succeeded: 0,
            failed: 0,
            errors: Vec::new(),
        };
        for plugin in plugins.iter() {
            if !plugin.enabled() {
                continue;
            }
            result.delivered += 1;
            match plugin.handle_event(event, ctx) {
                Ok(()) => result.succeeded += 1,
                Err(e) => {
                    result.failed += 1;
                    result.errors.push((plugin.name().to_string(), e.to_string()));
                }
            }
        }
        result
    }

    /// Clear all registered plugins (mainly useful for tests).
    pub fn clear(&self) {
        self.plugins.lock().expect("plugin mutex poisoned").clear();
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventKind;
    use anyhow::Result;

    struct CountingPlugin {
        name: String,
        count: std::sync::atomic::AtomicUsize,
        fail: bool,
        enabled: bool,
    }

    impl CountingPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.into(),
                count: std::sync::atomic::AtomicUsize::new(0),
                fail: false,
                enabled: true,
            }
        }
    }

    impl Plugin for CountingPlugin {
        fn name(&self) -> &str {
            &self.name
        }
        fn handle_event(&self, _event: &Event, _ctx: &PluginContext) -> Result<()> {
            self.count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if self.fail {
                Err(anyhow::anyhow!("intentional failure"))
            } else {
                Ok(())
            }
        }
        fn enabled(&self) -> bool {
            self.enabled
        }
    }

    fn sample_event() -> Event {
        Event::RepoOpened {
            project_path: "/tmp".into(),
            current_branch: "main".into(),
            timestamp: 0,
        }
    }

    #[test]
    fn dispatches_to_all_enabled_plugins() {
        let bus = EventBus::new();
        let p1 = CountingPlugin::new("p1");
        let p2 = CountingPlugin::new("p2");
        bus.subscribe(Box::new(p1));
        bus.subscribe(Box::new(p2));

        let ctx = PluginContext::default();
        let r = bus.publish(&sample_event(), &ctx);
        assert_eq!(r.delivered, 2);
        assert_eq!(r.succeeded, 2);
        assert_eq!(r.failed, 0);
    }

    #[test]
    fn disabled_plugins_are_skipped() {
        let bus = EventBus::new();
        let p = CountingPlugin::new("p");
        // Capture the atomic counter via a shared Arc before moving the
        // plugin into the bus (AtomicUsize is not Clone-able).
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        struct DisabledWithCounter {
            inner: CountingPlugin,
            counter: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        }
        impl Plugin for DisabledWithCounter {
            fn name(&self) -> &str {
                self.inner.name()
            }
            fn enabled(&self) -> bool {
                false
            }
            fn handle_event(&self, event: &Event, ctx: &PluginContext) -> Result<()> {
                self.counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.inner.handle_event(event, ctx)
            }
        }
        bus.subscribe(Box::new(DisabledWithCounter {
            inner: p,
            counter: counter.clone(),
        }));

        let ctx = PluginContext::default();
        let r = bus.publish(&sample_event(), &ctx);
        assert_eq!(r.delivered, 0);
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn plugin_failure_does_not_stop_dispatch() {
        let bus = EventBus::new();
        let mut bad = CountingPlugin::new("bad");
        bad.fail = true;
        let good = CountingPlugin::new("good");
        bus.subscribe(Box::new(bad));
        bus.subscribe(Box::new(good));

        let ctx = PluginContext::default();
        let r = bus.publish(&sample_event(), &ctx);
        assert_eq!(r.delivered, 2);
        assert_eq!(r.succeeded, 1);
        assert_eq!(r.failed, 1);
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.errors[0].0, "bad");
        // The good plugin still ran.
        assert!(r.is_ok() == false);
    }

    #[test]
    fn empty_bus_returns_ok() {
        let bus = EventBus::new();
        let ctx = PluginContext::default();
        let r = bus.publish(&sample_event(), &ctx);
        assert_eq!(r.delivered, 0);
        assert!(r.is_ok());
    }

    #[test]
    fn plugin_names_lists_in_registration_order() {
        let bus = EventBus::new();
        bus.subscribe(Box::new(CountingPlugin::new("alpha")));
        bus.subscribe(Box::new(CountingPlugin::new("beta")));
        assert_eq!(bus.plugin_names(), vec!["alpha", "beta"]);
        assert_eq!(bus.len(), 2);
    }

    #[test]
    fn event_kind_distinguishes_variants() {
        let ev1 = Event::RepoOpened {
            project_path: "".into(),
            current_branch: "".into(),
            timestamp: 0,
        };
        let ev2 = Event::CommitCreated {
            commit_id: "".into(),
            branch_id: "".into(),
            branch_name: "".into(),
            from_snapshot: "".into(),
            to_snapshot: "".into(),
            message: "".into(),
            author: None,
            kind: crate::CommitKind::Incremental,
            timestamp: 0,
        };
        assert_eq!(ev1.kind(), EventKind::RepoOpened);
        assert_eq!(ev2.kind(), EventKind::CommitCreated);
        assert_ne!(ev1.kind(), ev2.kind());
    }
}
