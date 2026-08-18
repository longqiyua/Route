//! Hashing utilities: SHA-256 content addressing, ULID generation, short IDs.

use sha2::{Digest, Sha256};
use ulid::Ulid;

/// Compute SHA-256 of raw bytes and return lowercase hex string.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hash_to_hex(&hasher.finalize())
}

/// Convert raw digest bytes to lowercase hex.
pub fn hash_to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Generate a new ULID (26-char lexicographically sortable).
pub fn new_id() -> String {
    Ulid::new().to_string()
}

/// Short ID = first 8 chars of an id (ULID or hash), for human display.
pub fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Compute hash of a file's relative path (normalized) — used as map key.
pub fn path_hash(rel_path: &str) -> String {
    let normalized = rel_path.replace('\\', "/");
    content_hash(normalized.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_deterministic() {
        let h1 = content_hash(b"hello");
        let h2 = content_hash(b"hello");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn short_id_truncates() {
        let id = "01H8XGJWBWBAQ4TPF7A5R6Y7AB";
        assert_eq!(short_id(id), "01H8XGJW");
    }

    #[test]
    fn new_id_is_ulid() {
        let id = new_id();
        assert_eq!(id.len(), 26);
        assert!(id.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
