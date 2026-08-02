//! Path layout for `.route-basic/` metadata directory.

use std::path::{Path, PathBuf};

/// Name of the Route basic-mode metadata directory.
pub const ROUTE_BASIC_DIR: &str = ".route-basic";

/// Resolves all paths inside `.route-basic/` for a given project root.
#[derive(Debug, Clone)]
pub struct RoutePaths {
    pub project_path: PathBuf,
    pub route_dir: PathBuf,
}

impl RoutePaths {
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        let project_path = project_path.as_ref().to_path_buf();
        let route_dir = project_path.join(ROUTE_BASIC_DIR);
        Self {
            project_path,
            route_dir,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.route_dir.join("config.json")
    }

    pub fn db_path(&self) -> PathBuf {
        self.route_dir.join("db.sqlite")
    }

    pub fn objects_dir(&self) -> PathBuf {
        self.route_dir.join("objects")
    }

    /// Blob path: `objects/<hash前2>/<hash>`
    pub fn blob_path(&self, hash: &str) -> PathBuf {
        let prefix = &hash[..2.min(hash.len())];
        self.objects_dir().join(prefix).join(hash)
    }

    /// Ensure all base directories exist.
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.route_dir)?;
        std::fs::create_dir_all(self.objects_dir())?;
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.config_path().exists() && self.db_path().exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_path_uses_2_char_prefix() {
        let p = RoutePaths::new("/tmp/proj");
        let blob = p.blob_path("abcdef1234");
        assert_eq!(blob, PathBuf::from("/tmp/proj/.route-basic/objects/ab/abcdef1234"));
    }
}
