//! Route shared infrastructure.
//!
//! Provides blob content-addressable storage, SQLite connection management,
//! schema migrations, hashing utilities, and project file scanning.

pub mod hash;
pub mod paths;
pub mod schema;
pub mod storage;

pub use hash::{content_hash, hash_to_hex, new_id, short_id};
pub use paths::RoutePaths;
pub use schema::{SchemaVersion, CURRENT_SCHEMA_VERSION};
pub use storage::{
    now_millis, sha256_hex, BlobStore, DbConnection, ManifestDiff, ProjectScanner, ScanResult,
    TrackFilter,
};
