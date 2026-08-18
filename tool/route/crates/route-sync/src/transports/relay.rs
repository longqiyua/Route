//! Public relay transport — stub for Phase 4.
//!
//! In Phase 4, this will connect to a public relay server to transfer files
//! between machines. For now, it falls back to local copy if destination is local.

use std::path::Path;

use anyhow::Result;

use super::{FileEntry, Transport, TransportType};

pub struct RelayTransport {
    // Phase 4: relay server URL, auth token, etc.
}

impl RelayTransport {
    pub fn new() -> Self {
        Self {}
    }
}

impl Transport for RelayTransport {
    fn send_file(&self, entry: &FileEntry, dest_root: &Path) -> Result<()> {
        // Stub: attempt local copy (works if destination is a local path)
        let dest = dest_root.join(&entry.rel_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&entry.source_abs, &dest)?;
        Ok(())
    }

    fn list_dest(&self, dest_root: &Path) -> Result<Vec<String>> {
        if !dest_root.exists() {
            return Ok(vec![]);
        }
        let mut files = Vec::new();
        walk(dest_root, dest_root, &mut files)?;
        Ok(files)
    }

    fn delete_file(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        let path = dest_root.join(rel_path);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }

    fn mkdir(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        std::fs::create_dir_all(dest_root.join(rel_path))?;
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Relay
    }
}

fn walk(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files)?;
        } else {
            let rel = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(rel);
        }
    }
    Ok(())
}
