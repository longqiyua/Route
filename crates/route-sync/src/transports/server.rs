//! Custom server transport — stub for Phase 4.

use std::path::Path;

use anyhow::Result;

use super::{FileEntry, Transport, TransportType};

pub struct ServerTransport {
    // Phase 4: server URL, credentials, etc.
}

impl ServerTransport {
    pub fn new() -> Self {
        Self {}
    }
}

impl Transport for ServerTransport {
    fn send_file(&self, entry: &FileEntry, dest_root: &Path) -> Result<()> {
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
        TransportType::Server
    }
}

fn walk(root: &Path, current: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files)?;
        } else {
            let rel = path.strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(rel);
        }
    }
    Ok(())
}
