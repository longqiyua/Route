//! Route shared infrastructure.
//!
//! Provides blob content-addressable storage, SQLite connection management,
//! schema migrations, hashing utilities, project file scanning, and the
//! Route Guard protection mechanism.

pub mod error;
pub mod guard;
pub mod hash;
pub mod paths;
pub mod safety;
pub mod schema;
pub mod storage;

pub use error::{is_integrity_error, RouteError};
pub use guard::{
    check_protected, check_protected_cwd, ensure_guard_anchor, is_route_active, GuardResult,
    RouteGuard,
};
pub use hash::{content_hash, hash_to_hex, new_id, short_id};
pub use paths::RoutePaths;
pub use safety::{assert_no_symlink_escape, is_plain_file_inside, safe_join, validate_rel_path};
pub use schema::{SchemaVersion, CURRENT_SCHEMA_VERSION};
pub use storage::{
    is_route_temp_file, now_millis, sha256_hex, BlobStore, DbConnection, ManifestDiff,
    ProjectScanner, ScanResult, TrackFilter,
};
