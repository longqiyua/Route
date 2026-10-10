//! Hashing utilities: SHA-256 content addressing, ULID generation, short IDs.

use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::Path;
use ulid::Ulid;

/// Compute SHA-256 of raw bytes and return lowercase hex string.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hash_to_hex(&hasher.finalize())
}

/// Compute a file's SHA-256 digest with bounded memory.
///
/// The buffer is fixed regardless of file size, so snapshotting multi-GB
/// datasets does not require allocating a `Vec` as large as the file.
pub fn content_hash_file(path: &Path) -> io::Result<(String, u64)> {
    const BUFFER_SIZE: usize = 256 * 1024;

    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut size = 0_u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size += read as u64;
    }
    Ok((hash_to_hex(&hasher.finalize()), size))
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
    fn file_hash_matches_in_memory_hash_across_many_chunks() {
        let path = std::env::temp_dir().join(format!("route-hash-{}.bin", new_id()));
        let content = vec![0x5a; 3 * 256 * 1024 + 17];
        std::fs::write(&path, &content).unwrap();
        let (hash, size) = content_hash_file(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(hash, content_hash(&content));
        assert_eq!(size, content.len() as u64);
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
