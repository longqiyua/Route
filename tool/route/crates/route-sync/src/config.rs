//! Sync configuration — defines sync targets and their settings.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::modes::{ArchiveRule, ConflictResolution, SyncMode};
use crate::transports::TransportType;

/// Remote credentials / endpoint config for WebDAV and S3 transports.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteCredentials {
    /// Endpoint URL.
    /// - WebDAV: e.g. `https://dav.example.com/path/`
    /// - S3: e.g. `https://s3.us-east-1.amazonaws.com` (or MinIO/custom endpoint)
    #[serde(default)]
    pub url: String,
    /// WebDAV Basic auth username.
    #[serde(default)]
    pub username: Option<String>,
    /// WebDAV Basic auth password.
    #[serde(default)]
    pub password: Option<String>,
    /// S3 access key id.
    #[serde(default)]
    pub access_key: Option<String>,
    /// S3 secret access key.
    #[serde(default)]
    pub secret_key: Option<String>,
    /// S3 bucket name.
    #[serde(default)]
    pub bucket: Option<String>,
    /// S3 region.
    #[serde(default)]
    pub region: Option<String>,
}

impl RemoteCredentials {
    pub fn is_empty(&self) -> bool {
        self.url.is_empty()
            && self.username.is_none()
            && self.password.is_none()
            && self.access_key.is_none()
            && self.secret_key.is_none()
            && self.bucket.is_none()
            && self.region.is_none()
    }
}

/// A single sync target — defines source, destination, mode, and transport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTarget {
    /// Unique name for this sync target.
    pub name: String,
    /// Source directory path.
    pub source: PathBuf,
    /// Destination directory path (or remote address for non-local transports).
    pub destination: PathBuf,
    /// Sync mode.
    #[serde(default)]
    pub mode: SyncMode,
    /// Transport type.
    #[serde(default)]
    pub transport: TransportType,
    /// Conflict resolution (for Backup mode).
    #[serde(default)]
    pub conflict: ConflictResolution,
    /// Archive rules (for Archive mode).
    #[serde(default)]
    pub archive_rule: ArchiveRule,
    /// Patterns to ignore (gitignore-style).
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
    /// Whether this target is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Remote credentials / endpoint (for WebDAV / S3 transports).
    #[serde(default)]
    pub credentials: Option<RemoteCredentials>,
}

fn default_true() -> bool {
    true
}

impl Default for SyncTarget {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            source: PathBuf::new(),
            destination: PathBuf::new(),
            mode: SyncMode::default(),
            transport: TransportType::default(),
            conflict: ConflictResolution::default(),
            archive_rule: ArchiveRule::default(),
            ignore_patterns: vec![],
            enabled: true,
            credentials: None,
        }
    }
}

/// Top-level sync configuration — contains all sync targets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncConfig {
    /// All sync targets.
    pub targets: Vec<SyncTarget>,
    /// Default transport for new targets.
    #[serde(default)]
    pub default_transport: TransportType,
}

impl SyncConfig {
    /// Load from a JSON file.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Save to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Find a target by name.
    pub fn find(&self, name: &str) -> Option<&SyncTarget> {
        self.targets.iter().find(|t| t.name == name)
    }

    /// Find a target by name (mutable).
    pub fn find_mut(&mut self, name: &str) -> Option<&mut SyncTarget> {
        self.targets.iter_mut().find(|t| t.name == name)
    }

    /// Add or replace a target.
    pub fn upsert(&mut self, target: SyncTarget) {
        if let Some(existing) = self.find_mut(&target.name) {
            *existing = target;
        } else {
            self.targets.push(target);
        }
    }

    /// Remove a target by name.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.targets.len();
        self.targets.retain(|t| t.name != name);
        self.targets.len() < before
    }
}
