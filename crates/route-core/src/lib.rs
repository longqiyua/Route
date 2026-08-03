//! Route shared infrastructure.
//!
//! Provides blob content-addressable storage, SQLite connection management,
//! schema migrations, hashing utilities, project file scanning, and the
//! Route Guard protection mechanism.

pub mod guard;
pub mod hash;
pub mod paths;
pub mod schema;
pub mod storage;

pub use guard::{check_protected, check_protected_cwd, ensure_guard_anchor, is_route_active, GuardResult, RouteGuard};
pub use hash::{content_hash, hash_to_hex, new_id, short_id};
pub use paths::RoutePaths;
pub use schema::{SchemaVersion, CURRENT_SCHEMA_VERSION};
pub use storage::{
    now_millis, sha256_hex, BlobStore, DbConnection, ManifestDiff, ProjectScanner, ScanResult,
    TrackFilter,
};
