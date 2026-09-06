//! Recoverable bridge between the existing Reference registry and commons.
//! The pending record is a write-ahead operation, not another truth registry.
use crate::development::{
    append_development_event, DevelopmentEventDraft, DevelopmentEventPayload,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    schema: u8,
    operation_id: String,
    project_id: String,
    before: Option<String>,
    after: String,
    after_hash: String,
    committed: bool,
}
fn path(root: &Path) -> PathBuf {
    root.join(".route/reference/operation.json")
}
fn registry(root: &Path) -> PathBuf {
    crate::constitutive::ReferenceRegistry::path(root)
}
fn load(root: &Path) -> Result<Option<Operation>> {
    match std::fs::read(path(root)) {
        Ok(bytes) => {
            let op: Operation =
                serde_json::from_slice(&bytes).context("corrupt Reference operation")?;
            let identity = crate::project_identity::load_identity(root)?
                .ok_or_else(|| anyhow::anyhow!("Reference operation owner missing"))?;
            if op.schema != 1
                || op.operation_id.is_empty()
                || op.project_id != identity.project_id
                || route_core::sha256_hex(op.after.as_bytes()) != op.after_hash
            {
                bail!("invalid Reference operation identity or hash");
            }
            let _: crate::constitutive::ReferenceRegistry = serde_json::from_str(&op.after)?;
            Ok(Some(op))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub(crate) fn check_read(root: &Path) -> Result<()> {
    if load(root)?.is_some_and(|op| !op.committed) {
        bail!("REFERENCE_RECOVERY_REQUIRED: run `route reference recover --operation-key NEW_RECOVERY_KEY` in this project, then retry; no reset was performed");
    }
    Ok(())
}
fn hash_on_disk(root: &Path) -> Result<Option<String>> {
    match std::fs::read(registry(root)) {
        Ok(bytes) => Ok(Some(route_core::sha256_hex(&bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn persist(root: &Path, op: &Operation) -> Result<()> {
    crate::constitutive::write_atomic(&path(root), &serde_json::to_vec(op)?)
}
pub(crate) fn validate_event(root: &Path, operation_id: &str, hash: &str) -> Result<()> {
    let op = load(root)?
        .ok_or_else(|| anyhow::anyhow!("Reference event requires a reserved operation"))?;
    if op.operation_id != operation_id
        || op.after_hash != hash
        || hash_on_disk(root)?.as_deref() != Some(hash)
    {
        bail!("Reference event does not match durable domain operation");
    }
    Ok(())
}
pub(crate) fn recover_locked(root: &Path) -> Result<()> {
    let Some(mut op) = load(root)? else {
        return Ok(());
    };
    if op.committed {
        return Ok(());
    }
    let current = hash_on_disk(root)?;
    if current != op.before && current.as_deref() != Some(op.after_hash.as_str()) {
        bail!("Reference recovery conflict: preserving journal and registry bytes");
    }
    if current.as_deref() != Some(op.after_hash.as_str()) {
        crate::constitutive::write_atomic(&registry(root), op.after.as_bytes())?;
    }
    failpoint("domain")?;
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: Some(format!("reference_{}", op.operation_id)),
            actor_worker_id: None,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: Some(op.operation_id.clone()),
            source_refs: vec![],
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::ReferenceRegistryCommitted {
                operation_id: op.operation_id.clone(),
                registry_hash: op.after_hash.clone(),
            },
            deduplication_key: Some(format!("reference:{}", op.operation_id)),
        },
    )?;
    failpoint("event")?;
    op.committed = true;
    persist(root, &op)?;
    failpoint("response")?;
    Ok(())
}
pub(crate) fn commit(
    root: &Path,
    expected: Option<&str>,
    after: &[u8],
    operation_id: Option<&str>,
) -> Result<String> {
    recover_locked(root)?;
    let hash = route_core::sha256_hex(after);
    let identity = crate::project_identity::ensure_identity(root)?;
    let op_id = operation_id
        .map(str::to_owned)
        .unwrap_or_else(route_core::new_id);
    if op_id.is_empty()
        || op_id.len() > 128
        || !op_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("invalid Reference operation ID");
    }
    let ledger = crate::development::load_ledger(root)?.0;
    for event in &ledger.events {
        if let DevelopmentEventPayload::ReferenceRegistryCommitted {
            operation_id: id,
            registry_hash,
        } = &event.payload
        {
            if id == &op_id {
                if registry_hash != &hash {
                    bail!("Reference operation idempotency conflict");
                }
                return Ok(hash);
            }
        }
    }
    let before = hash_on_disk(root)?;
    if before.as_deref() == Some(hash.as_str()) {
        return Ok(hash);
    }
    if before.as_deref() != expected {
        bail!("Reference concurrent modification conflict; reload before mutation");
    }
    let op = Operation {
        schema: 1,
        operation_id: op_id,
        project_id: identity.project_id,
        before,
        after: String::from_utf8(after.to_vec())?,
        after_hash: hash.clone(),
        committed: false,
    };
    persist(root, &op)?;
    failpoint("reserved")?;
    recover_locked(root)?;
    Ok(hash)
}

#[cfg(test)]
thread_local! { static FAILURE: std::cell::RefCell<Option<&'static str>> = const { std::cell::RefCell::new(None) }; }
fn failpoint(_window: &str) -> Result<()> {
    #[cfg(feature = "test-utils")]
    if std::env::var("ROUTE_REFERENCE_FAIL_AT").ok().as_deref() == Some(_window) {
        std::process::exit(23);
    }
    #[cfg(test)]
    FAILURE.with(|slot| {
        if slot.borrow().as_deref() == Some(_window) {
            slot.replace(None);
            if std::env::var_os("ROUTE_REFERENCE_CRASH_FIXTURE").is_some() {
                std::process::exit(23);
            }
            bail!("injected Reference crash at {_window}");
        }
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constitutive::ReferenceRegistry;

    #[test]
    #[ignore = "subprocess crash fixture"]
    fn crash_fixture() {
        let root = std::env::var_os("ROUTE_REFERENCE_CRASH_FIXTURE").unwrap();
        let window = std::env::var("ROUTE_REFERENCE_WINDOW").unwrap();
        let window = match window.as_str() {
            "reserved" => "reserved",
            "domain" => "domain",
            "event" => "event",
            "response" => "response",
            _ => panic!("invalid fixture window"),
        };
        FAILURE.with(|slot| slot.replace(Some(window)));
        ReferenceRegistry::new()
            .write_with_operation_id(Path::new(&root), "crash-op")
            .unwrap();
        panic!("fixture did not terminate");
    }

    #[test]
    fn process_restart_recovers_every_persistence_window() {
        for window in ["reserved", "domain", "event", "response"] {
            let dir = tempfile::tempdir().unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "reference_operation::tests::crash_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("ROUTE_REFERENCE_CRASH_FIXTURE", dir.path())
                .env("ROUTE_REFERENCE_WINDOW", window)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(23));
            ReferenceRegistry::recover(dir.path()).unwrap();
            ReferenceRegistry::read(dir.path())
                .unwrap()
                .write_with_operation_id(dir.path(), "crash-op")
                .unwrap();
            assert_eq!(
                crate::development::load_ledger(dir.path())
                    .unwrap()
                    .0
                    .events
                    .len(),
                1
            );
        }
    }
    #[test]
    fn all_crash_windows_recover_once_and_retry_conflicts() {
        for window in ["reserved", "domain", "event", "response"] {
            let dir = tempfile::tempdir().unwrap();
            let reg = ReferenceRegistry::new();
            FAILURE.with(|slot| slot.replace(Some(window)));
            assert!(reg
                .write_with_operation_id(dir.path(), "operation-1")
                .is_err());
            ReferenceRegistry::recover(dir.path()).unwrap();
            let current = ReferenceRegistry::read(dir.path()).unwrap();
            current
                .write_with_operation_id(dir.path(), "operation-1")
                .unwrap();
            let events = crate::development::load_ledger(dir.path())
                .unwrap()
                .0
                .events;
            assert_eq!(events.len(), 1, "window {window}");
            let mut changed = current;
            changed.entries.push(
                crate::constitutive::ReferenceEntry::builder(
                    "another",
                    crate::constitutive::ReferenceType::Document,
                    "file.txt",
                    "fixture",
                )
                .build(),
            );
            assert!(changed
                .write_with_operation_id(dir.path(), "operation-1")
                .is_err());
        }
    }
}
