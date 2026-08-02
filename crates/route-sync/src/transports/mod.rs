//! Transport layer — how files are moved between source and destination.

pub mod local;
pub mod relay;
pub mod server;
pub mod ssh;
pub mod webdav;
pub mod s3;

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::RemoteCredentials;
pub use local::LocalTransport;
pub use relay::RelayTransport;
pub use s3::S3Transport;
pub use server::ServerTransport;
pub use ssh::SshTransport;
pub use webdav::WebdavTransport;

/// Type of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportType {
    /// Direct filesystem copy (local-to-local).
    Local,
    /// Public relay server (default for remote sync).
    Relay,
    /// Custom server endpoint.
    Server,
    /// Peer-to-peer direct connection.
    P2P,
    /// WebDAV remote storage.
    Webdav,
    /// S3-compatible remote storage (AWS S3, MinIO, R2, etc.).
    S3,
    /// SSH / SCP. Requires OpenSSH on PATH. Auth is delegated to the
    /// user's ssh-agent / ~/.ssh/config; no private keys are handled
    /// in-process. Default port 22; configurable in the URL.
    Ssh,
}

impl Default for TransportType {
    fn default() -> Self {
        TransportType::Relay
    }
}

impl TransportType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportType::Local => "local",
            TransportType::Relay => "relay",
            TransportType::Server => "server",
            TransportType::P2P => "p2p",
            TransportType::Webdav => "webdav",
            TransportType::S3 => "s3",
            TransportType::Ssh => "ssh",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "local" => Some(TransportType::Local),
            "relay" => Some(TransportType::Relay),
            "server" => Some(TransportType::Server),
            "p2p" => Some(TransportType::P2P),
            "webdav" => Some(TransportType::Webdav),
            "s3" => Some(TransportType::S3),
            "ssh" | "scp" => Some(TransportType::Ssh),
            _ => None,
        }
    }
}

/// A file entry to be transferred.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path (forward-slash).
    pub rel_path: String,
    /// Absolute source path.
    pub source_abs: std::path::PathBuf,
    /// File size in bytes.
    pub size: u64,
}

/// Transport trait — abstracts how files are moved.
pub trait Transport: Send + Sync {
    /// Send a file from source to the destination's relative path.
    fn send_file(&self, entry: &FileEntry, dest_root: &Path) -> Result<()>;

    /// List files currently at the destination.
    fn list_dest(&self, dest_root: &Path) -> Result<Vec<String>>;

    /// Delete a file at the destination (for mirror mode).
    fn delete_file(&self, rel_path: &str, dest_root: &Path) -> Result<()>;

    /// Create a directory at the destination.
    fn mkdir(&self, rel_path: &str, dest_root: &Path) -> Result<()>;

    /// Transport type.
    fn transport_type(&self) -> TransportType;
}

/// Create a transport instance for the given type, with optional remote credentials.
pub fn create_transport(tt: TransportType, creds: Option<&RemoteCredentials>) -> Box<dyn Transport> {
    match tt {
        TransportType::Local => Box::new(LocalTransport),
        TransportType::Relay => Box::new(RelayTransport::new()),
        TransportType::Server => Box::new(ServerTransport::new()),
        TransportType::P2P => Box::new(RelayTransport::new()), // P2P falls back to relay stub for now
        TransportType::Webdav => {
            let creds = creds.cloned().unwrap_or_default();
            Box::new(WebdavTransport::new(creds))
        }
        TransportType::S3 => {
            let creds = creds.cloned().unwrap_or_default();
            Box::new(S3Transport::new(creds))
        }
        TransportType::Ssh => {
            let creds = creds.cloned().unwrap_or_default();
            Box::new(SshTransport::new(creds))
        }
    }
}
