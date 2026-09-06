//! Process-owned file locks. Age is never evidence of owner death.
//! The kernel releases a dead process's handle; lock files are never unlinked.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Durable replacement shared with transport receipts; never delete-before-rename.
pub fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    crate::constitutive::write_atomic(path, bytes)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    schema: u8,
    pid: u32,
    nonce: String,
    acquired_at: i64,
}

pub struct OwnershipLock {
    file: File,
    metadata: PathBuf,
    nonce: String,
}

impl OwnershipLock {
    pub fn acquire(path: &Path) -> Result<Self> {
        Self::acquire_bounded(path, 400, Duration::from_millis(25))
    }

    fn acquire_bounded(path: &Path, attempts: usize, delay: Duration) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| {
                format!(
                    "opening lock {}; legacy directory locks require explicit recovery",
                    path.display()
                )
            })?;
        for _ in 0..attempts {
            match file.try_lock() {
                Ok(()) => {
                    let metadata = path.with_extension("owner.json");
                    match std::fs::read(&metadata) {
                        Ok(bytes) => {
                            let owner: Owner = serde_json::from_slice(&bytes)
                                .context("corrupt lock owner metadata")?;
                            if owner.schema != 1 || owner.pid == 0 || owner.nonce.is_empty() {
                                bail!("invalid lock owner metadata");
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error.into()),
                    }
                    let nonce = route_core::new_id();
                    let owner = Owner {
                        schema: 1,
                        pid: std::process::id(),
                        nonce: nonce.clone(),
                        acquired_at: route_core::now_millis(),
                    };
                    crate::constitutive::write_atomic(&metadata, &serde_json::to_vec(&owner)?)?;
                    return Ok(Self {
                        file,
                        metadata,
                        nonce,
                    });
                }
                Err(TryLockError::WouldBlock) => std::thread::sleep(delay),
                Err(TryLockError::Error(error)) => return Err(error.into()),
            }
        }
        bail!("lock busy: owner not proven dead; no age-based takeover")
    }
}

impl Drop for OwnershipLock {
    fn drop(&mut self) {
        // Never remove a path or another owner's metadata. Even if metadata
        // was tampered with, closing this handle releases only OUR kernel lock.
        let owned = std::fs::read(&self.metadata)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Owner>(&bytes).ok())
            .is_some_and(|owner| owner.nonce == self.nonce);
        if owned {
            let _ = self.file.unlock();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "subprocess fixture, invoked by process_owner_survives_age_then_crashes"]
    fn owner_process_fixture() {
        let root = std::env::var_os("ROUTE_LOCK_FIXTURE").unwrap();
        let root = PathBuf::from(root);
        let _guard = OwnershipLock::acquire(&root.join("lock")).unwrap();
        std::fs::write(root.join("ready"), b"ready").unwrap();
        std::thread::sleep(Duration::from_secs(31));
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !root.join("release").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::process::exit(17); // Deliberate crash: no Rust guard destructor.
    }

    #[test]
    fn process_owner_survives_age_then_crashes() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ownership_lock::tests::owner_process_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("ROUTE_LOCK_FIXTURE", dir.path())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !dir.path().join("ready").exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "child did not acquire lock"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(30_100));
        assert!(
            OwnershipLock::acquire_bounded(&dir.path().join("lock"), 1, Duration::ZERO).is_err()
        );
        std::fs::write(dir.path().join("release"), b"release").unwrap();
        assert_eq!(child.wait().unwrap().code(), Some(17));
        let recovered = OwnershipLock::acquire(&dir.path().join("lock")).unwrap();
        assert!(
            OwnershipLock::acquire_bounded(&dir.path().join("lock"), 1, Duration::ZERO).is_err()
        );
        drop(recovered);
    }

    #[test]
    fn wrong_metadata_token_cannot_delete_lock_or_release_next_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let first = OwnershipLock::acquire(&path).unwrap();
        let metadata = path.with_extension("owner.json");
        let mut owner: Owner = serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
        owner.nonce = "not-the-guard-token".into();
        crate::constitutive::write_atomic(&metadata, &serde_json::to_vec(&owner).unwrap()).unwrap();
        drop(first);
        assert!(path.exists());
        assert!(metadata.exists());
        let _next = OwnershipLock::acquire(&path).unwrap();
        assert!(OwnershipLock::acquire_bounded(&path, 1, Duration::ZERO).is_err());
    }
    #[test]
    fn old_live_owner_cannot_be_stolen_and_drop_allows_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let guard = OwnershipLock::acquire(&path).unwrap();
        let metadata = path.with_extension("owner.json");
        let mut owner: Owner = serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
        owner.acquired_at = 1; // Equivalent to arbitrarily exceeding the former 30s threshold.
        crate::constitutive::write_atomic(&metadata, &serde_json::to_vec(&owner).unwrap()).unwrap();
        assert!(OwnershipLock::acquire_bounded(&path, 1, Duration::ZERO).is_err());
        drop(guard);
        let next = OwnershipLock::acquire(&path).unwrap();
        drop(next);
        std::fs::write(&metadata, b"{broken").unwrap();
        assert!(OwnershipLock::acquire(&path).is_err());
        assert_eq!(std::fs::read(&metadata).unwrap(), b"{broken");
    }
}
