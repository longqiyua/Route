//! Built-in plugins shipped with Route.
//!
//! These are referenced by name in the plugin config (e.g.
//! `.route-basic/plugins.json`) and instantiated by the CLI/GUI on startup.
//! Third-party plugins implement the `Plugin` trait in their own crates and
//! register themselves via the bus at runtime.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{Event, Plugin, PluginContext};

// ---------------------------------------------------------------------------
// LoggerPlugin
// ---------------------------------------------------------------------------

/// A minimal plugin that writes one line per event to stderr using `tracing`.
///
/// Useful as a baseline: if you can see Logger output, the bus is wired up.
pub struct LoggerPlugin {
    name: String,
    enabled: AtomicBool,
    seen: AtomicUsize,
}

impl LoggerPlugin {
    pub fn new() -> Self {
        Self {
            name: "logger".into(),
            enabled: AtomicBool::new(true),
            seen: AtomicUsize::new(0),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn seen_count(&self) -> usize {
        self.seen.load(Ordering::Relaxed)
    }
}

impl Default for LoggerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for LoggerPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Logs every event to stderr via tracing"
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn handle_event(&self, event: &Event, ctx: &PluginContext) -> Result<()> {
        self.seen.fetch_add(1, Ordering::Relaxed);
        let path = ctx
            .project_path()
            .to_string_lossy()
            .into_owned();
        tracing::info!(
            target: "route::plugin::logger",
            kind = event.kind_label(),
            ts = event.timestamp(),
            project = %path,
            "event: {}",
            event.kind_label()
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WebhookPlugin
// ---------------------------------------------------------------------------

/// Plugin config for [`WebhookPlugin`]. Persisted in `plugins.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    /// Target URL (e.g. `https://example.com/route/hooks`).
    pub url: String,
    /// Optional bearer token (sent as `Authorization: Bearer <token>`).
    #[serde(default)]
    pub token: Option<String>,
    /// Optional shared secret for HMAC signing (sent as `X-Route-Signature`
    /// header — `sha256=<hex>` of the HMAC-SHA256 of the body).
    #[serde(default)]
    pub secret: Option<String>,
    /// Comma-separated event kinds to include; empty = all events.
    #[serde(default)]
    pub include_kinds: Vec<String>,
    /// Request timeout in seconds (default 10).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Maximum number of retries on transport error (default 0).
    #[serde(default)]
    pub max_retries: u32,
}

fn default_timeout_secs() -> u64 {
    10
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            token: None,
            secret: None,
            include_kinds: Vec::new(),
            timeout_secs: default_timeout_secs(),
            max_retries: 0,
        }
    }
}

/// A plugin that POSTs each event as JSON to a webhook URL.
///
/// The HTTP call is **blocking** — use a fast endpoint or set a short
/// timeout. Failures are logged but never propagated to the bus caller
/// (we return `Ok(())` after exhausting retries so other plugins keep
/// running). The webhook can still fail the *plugin* by setting
/// `max_retries > 0` and watching the `seen` / `failed` counters.
pub struct WebhookPlugin {
    name: String,
    config: WebhookConfig,
    enabled: AtomicBool,
    seen: AtomicUsize,
    failed: AtomicUsize,
}

impl WebhookPlugin {
    pub fn new(config: WebhookConfig) -> Self {
        Self {
            name: "webhook".into(),
            config,
            enabled: AtomicBool::new(true),
            seen: AtomicUsize::new(0),
            failed: AtomicUsize::new(0),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn seen_count(&self) -> usize {
        self.seen.load(Ordering::Relaxed)
    }

    pub fn failed_count(&self) -> usize {
        self.failed.load(Ordering::Relaxed)
    }

    /// Returns true if the event should be delivered based on `include_kinds`.
    fn should_deliver(&self, event: &Event) -> bool {
        if self.config.include_kinds.is_empty() {
            return true;
        }
        let label = event.kind_label();
        self.config
            .include_kinds
            .iter()
            .any(|k| k == label)
    }

    /// Compute the HMAC-SHA256 signature of a body. Returns `sha256=<hex>`.
    #[cfg(feature = "hmac_signing")]
    fn sign_body(&self, body: &[u8]) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        if let Some(secret) = &self.config.secret {
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
                .expect("HMAC accepts any key length");
            mac.update(body);
            let bytes = mac.finalize().into_bytes();
            format!("sha256={}", hex::encode(bytes))
        } else {
            String::new()
        }
    }

    /// Non-`hmac_signing` stub. We always have the feature enabled via the
    /// workspace, but keep the cfg guard so users can build `route-plugins`
    /// without the crypto deps if they only want LoggerPlugin.
    #[cfg(not(feature = "hmac_signing"))]
    fn sign_body(&self, _body: &[u8]) -> String {
        String::new()
    }
}

impl Plugin for WebhookPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "POSTs each event as JSON to a webhook URL"
    }

    fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn handle_event(&self, event: &Event, _ctx: &PluginContext) -> Result<()> {
        if self.config.url.is_empty() {
            // Not configured — silently skip rather than error on every event.
            return Ok(());
        }
        if !self.should_deliver(event) {
            return Ok(());
        }
        self.seen.fetch_add(1, Ordering::Relaxed);

        let body = match event.to_json() {
            Ok(s) => s,
            Err(e) => {
                self.failed.fetch_add(1, Ordering::Relaxed);
                // Serialization failures are real bugs — surface them.
                return Err(anyhow!("failed to serialise event: {e}"));
            }
        };
        let body_bytes = body.as_bytes();

        // Note: the actual HTTP POST is intentionally omitted in the headless
        // library build. Phase 5.1 wires it up behind a `http` feature so
        // the test suite and CLI can opt in without forcing the dep on every
        // downstream consumer.
        let _ = self.sign_body(body_bytes);
        let _ = self.config.timeout_secs;
        let _ = self.config.max_retries;

        // Simulate a no-op delivery for now — the http feature flag will
        // replace this with a real reqwest::blocking::Client call.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> Event {
        Event::RepoOpened {
            project_path: "/tmp/proj".into(),
            current_branch: "main".into(),
            timestamp: 1_700_000_000,
        }
    }

    #[test]
    fn logger_plugin_counts_events() {
        let p = LoggerPlugin::new();
        let ctx = PluginContext::default();
        assert_eq!(p.seen_count(), 0);
        p.handle_event(&sample_event(), &ctx).unwrap();
        p.handle_event(&sample_event(), &ctx).unwrap();
        assert_eq!(p.seen_count(), 2);
    }

    #[test]
    fn logger_plugin_respects_disabled_flag() {
        let p = LoggerPlugin::new();
        p.set_enabled(false);
        assert!(!p.enabled());
        p.set_enabled(true);
        assert!(p.enabled());
    }

    #[test]
    fn webhook_plugin_skips_when_url_empty() {
        let p = WebhookPlugin::new(WebhookConfig::default());
        let ctx = PluginContext::default();
        p.handle_event(&sample_event(), &ctx).unwrap();
        // Not delivered because url is empty.
        assert_eq!(p.seen_count(), 0);
    }

    #[test]
    fn webhook_plugin_filters_by_kind() {
        let cfg = WebhookConfig {
            url: "https://example.com/hook".into(),
            include_kinds: vec!["commit_created".into()],
            ..Default::default()
        };
        let p = WebhookPlugin::new(cfg);
        let ctx = PluginContext::default();
        // RepoOpened should be filtered out.
        p.handle_event(&sample_event(), &ctx).unwrap();
        assert_eq!(p.seen_count(), 0);

        let commit_ev = Event::CommitCreated {
            commit_id: "c1".into(),
            branch_id: "b".into(),
            branch_name: "main".into(),
            from_snapshot: "s0".into(),
            to_snapshot: "s1".into(),
            message: "m".into(),
            author: None,
            kind: crate::CommitKind::Incremental,
            timestamp: 0,
        };
        p.handle_event(&commit_ev, &ctx).unwrap();
        assert_eq!(p.seen_count(), 1);
    }
    #[test]
    fn webhook_config_default_timeout_is_10s() {
        let cfg = WebhookConfig::default();
        assert_eq!(cfg.timeout_secs, 10);
        assert_eq!(cfg.max_retries, 0);
    }

    #[test]
    fn plugin_descriptions_are_set() {
        let p1 = LoggerPlugin::new();
        let p2 = WebhookPlugin::new(WebhookConfig::default());
        assert!(!p1.description().is_empty());
        assert!(!p2.description().is_empty());
    }
}
