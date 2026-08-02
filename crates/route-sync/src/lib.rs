//! Route directory sync — mirror / backup / archive modes with multiple transports.
//!
//! Modes:
//! - **Mirror**: Complete copy of latest state. Source overwrites target completely.
//! - **Backup**: Append-only. Never deletes. On conflict: keep both (default) or skip.
//! - **Archive**: Strict versioning. Each sync creates a timestamped snapshot folder.
//!
//! Transports:
//! - **Local**: Direct filesystem copy (for local-to-local sync).
//! - **Relay**: Public relay server (default for remote, stubbed for Phase 4).
//! - **Server**: Custom server endpoint (stubbed for Phase 4).
//! - **P2P**: Peer-to-peer direct connection (stubbed for Phase 4).

pub mod config;
pub mod engine;
pub mod modes;
pub mod scheduler;
pub mod transports;

pub use config::{RemoteCredentials, SyncConfig, SyncTarget};
pub use engine::{SyncEngine, SyncResult, SyncStats};
pub use modes::{ArchiveRule, ConflictResolution, SyncMode};
pub use scheduler::{ScheduledJob, Scheduler};
pub use transports::{LocalTransport, S3Transport, Transport, TransportType, WebdavTransport};
