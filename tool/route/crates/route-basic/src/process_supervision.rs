//! Explicit local-Operator process control. Shared project metadata is NOT kill authority.
//! Windows receipts are authoritative in the local user's Route runtime directory;
//! project copies are informational only. This is not a security boundary against
//! malicious programs already running as the same unrestricted OS user.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::OwnedProcess;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunIdentity {
    pub run_id: String,
    pub operation_id: String,
    pub owner: String,
    pub attempt: u64,
    /// Explicit domain binding; operation-key text is never a type discriminator.
    #[serde(default)]
    pub team_goal: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub creation_time: u64,
    pub executable: String,
    pub executable_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: u8,
    pub project: String,
    pub run: RunIdentity,
    pub nonce: String,
    pub containment: String,
    pub stage: String,
    pub process: Option<ProcessIdentity>,
    pub exit_code: Option<u32>,
    pub cancellation: Option<Cancellation>,
    /// Durable observation of zero active Job members, not merely a missing name.
    #[serde(default)]
    pub tree_termination_confirmed: bool,
    #[serde(default)]
    pub overhead_ms: std::collections::BTreeMap<String, f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cancellation {
    pub operation_key: String,
    pub state: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct Inspection {
    pub receipt: Receipt,
    pub knowledge: String,
    pub active_processes: Option<u32>,
    pub capacity_blocked_by_uncertain_run: bool,
    pub output_after_restart: &'static str,
    pub safe_next_action: &'static str,
}

/// No configurable path comes from a handoff or project file. The runtime store
/// must remain outside the shared checkout. Same-user OS access is trusted,
/// just as for existing local sidecar credentials; sandboxed Workers get no API.
pub struct Supervisor {
    root: PathBuf,
    authority: PathBuf,
}
impl Supervisor {
    pub fn open(root: &Path) -> Result<Self> {
        let profile = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .context("LOCAL_RUNTIME_DIRECTORY_UNAVAILABLE")?;
        Self::at(
            root,
            &PathBuf::from(profile).join("AppData/Local/Route/processes"),
        )
    }
    fn at(root: &Path, base: &Path) -> Result<Self> {
        let root = crate::project_identity::resolve_local_path(root)?;
        ensure!(base.is_absolute(), "RUNTIME_DIRECTORY_NOT_ABSOLUTE");
        let mut ancestor = base;
        while !ancestor.exists() {
            ancestor = ancestor.parent().context("RUNTIME_ANCESTOR_MISSING")?;
        }
        ensure!(
            !crate::project_identity::resolve_local_path(ancestor)?.starts_with(&root),
            "RUNTIME_INSIDE_PROJECT"
        );
        if base.exists() {
            ensure!(
                !crate::project_identity::resolve_local_path(base)?.starts_with(&root),
                "RUNTIME_INSIDE_PROJECT"
            );
        }
        let project = route_core::sha256_hex(root.to_string_lossy().as_bytes());
        Ok(Self {
            root,
            authority: base.join(project),
        })
    }
    fn path(&self, run_id: &str) -> PathBuf {
        self.authority.join(format!(
            "{}.json",
            route_core::sha256_hex(run_id.as_bytes())
        ))
    }
    fn load(&self, run_id: &str) -> Result<Receipt> {
        ensure!(
            std::fs::metadata(self.path(run_id))?.len() <= 65536,
            "MALFORMED_PROCESS_RECEIPT"
        );
        let bytes = std::fs::read(self.path(run_id))?;
        ensure!(bytes.len() <= 65536, "MALFORMED_PROCESS_RECEIPT");
        let receipt: Receipt =
            serde_json::from_slice(&bytes).context("MALFORMED_PROCESS_RECEIPT")?;
        ensure!(
            receipt.schema == 1
                && receipt.run.run_id == run_id
                && receipt.run.attempt > 0
                && receipt.project == self.root.to_string_lossy()
                && receipt.containment == job_name(&self.root, &receipt.run, &receipt.nonce),
            "PROCESS_RECEIPT_SCOPE_MISMATCH"
        );
        Ok(receipt)
    }
    fn save(&self, receipt: &Receipt) -> Result<()> {
        std::fs::create_dir_all(&self.authority)?;
        crate::ownership_lock::atomic_replace(
            &self.path(&receipt.run.run_id),
            &serde_json::to_vec_pretty(receipt)?,
        )
    }
    fn fence(&self, receipt: &Receipt) -> Result<()> {
        let current = self.load(&receipt.run.run_id)?;
        ensure!(
            current.run == receipt.run && current.nonce == receipt.nonce,
            "PROCESS_ATTEMPT_FENCED"
        );
        if receipt.run.team_goal.is_none() {
            match crate::check_run::inspect(&self.root, &receipt.run.operation_id) {
                Ok(run) => ensure!(
                    run.run_id == receipt.run.run_id
                        && run.owner == receipt.run.owner
                        && run.attempt == receipt.run.attempt
                        && matches!(
                            run.state,
                            crate::check_run::CheckRunState::PossiblyRunning
                                | crate::check_run::CheckRunState::RecoveryRequired
                        ),
                    "PROCESS_ATTEMPT_FENCED"
                ),
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
    fn lock(&self, run_id: &str) -> Result<crate::ownership_lock::OwnershipLock> {
        std::fs::create_dir_all(&self.authority)?;
        crate::ownership_lock::OwnershipLock::acquire(&self.path(run_id).with_extension("lock"))
    }
    /// Mutation is explicit and keyed. A recovery observation never creates
    /// Evidence, invents an exit code, relaunches, or releases Team capacity.
    pub fn recover(&self, run_id: &str, operation_key: &str) -> Result<Inspection> {
        ensure!(!operation_key.is_empty(), "OPERATION_KEY_REQUIRED");
        let initial = self.inspect(run_id)?;
        let unlaunched = initial.receipt.stage == "BEFORE_SPAWN"
            && initial.receipt.process.is_none()
            && initial.receipt.run.team_goal.is_none()
            && crate::check_run::inspect(&self.root, &initial.receipt.run.operation_id).is_ok();
        let _goal_guard = if unlaunched {
            Some(crate::ownership_lock::OwnershipLock::acquire(
                &self.root.join(".route/execution/.check-run-owner.lock"),
            )?)
        } else {
            None
        };
        let _lock = self.lock(run_id)?;
        let inspected = self.inspect(run_id)?;
        let path = self.path(run_id).with_file_name(format!(
            "{}.recovery-{}.json",
            route_core::sha256_hex(run_id.as_bytes()),
            route_core::sha256_hex(operation_key.as_bytes())
        ));
        if !path.exists() {
            crate::ownership_lock::atomic_replace(&path, &serde_json::to_vec_pretty(&inspected)?)?;
        }
        if unlaunched {
            ensure!(
                inspected.receipt.stage == "BEFORE_SPAWN"
                    && inspected.receipt.process.is_none()
                    && inspected.active_processes == Some(0),
                "LAUNCH_NOT_PROVEN_ABSENT"
            );
            crate::check_run::restore_unlaunched(&self.root, &inspected.receipt.run)?;
        }
        self.inspect(run_id)
    }
    /// Deliberate Human/Operator acknowledgement; keeps the original run and
    /// lost result intact, releases only a proven empty tree, never starts work.
    pub fn acknowledge_lost_result(&self, run_id: &str, operation_key: &str) -> Result<Inspection> {
        ensure!(!operation_key.is_empty(), "OPERATION_KEY_REQUIRED");
        // Match CheckRun's lock order: control plane -> process receipt.
        // Team-only projects need not have executed a CheckRun yet. This is an
        // explicit mutation, so initialize the lock directory here, not in inspect.
        std::fs::create_dir_all(self.root.join(".route/execution"))?;
        let _goal_lock = crate::ownership_lock::OwnershipLock::acquire(
            &self.root.join(".route/execution/.check-run-owner.lock"),
        )?;
        let _lock = self.lock(run_id)?;
        let inspected = self.inspect(run_id)?;
        ensure!(
            inspected.active_processes == Some(0) && inspected.knowledge == "LOST_RESULT",
            "TREE_NOT_PROVEN_TERMINATED_OR_RESULT_EXISTS"
        );
        let mut confirmed = inspected.receipt.clone();
        confirmed.tree_termination_confirmed = true;
        self.save(&confirmed)?;
        let audit = self.path(run_id).with_file_name(format!(
            "{}.release-{}.json",
            route_core::sha256_hex(run_id.as_bytes()),
            route_core::sha256_hex(operation_key.as_bytes())
        ));
        // Persist the explicit intent before releasing canonical capacity.
        if !audit.exists() {
            crate::ownership_lock::atomic_replace(&audit, &serde_json::to_vec_pretty(&inspected)?)?;
        }
        let identity = &inspected.receipt.run;
        if let Some(goal) = identity.team_goal.as_deref() {
            let view = crate::team::status(&self.root, goal)?;
            let run = view.runs.get(run_id).context("UNKNOWN_TEAM_RUN")?;
            ensure!(
                run.reservation.worker_id == identity.owner,
                "PROCESS_ATTEMPT_FENCED"
            );
            if run.state != crate::team::RunState::LostResult {
                crate::team::broker_action(
                    &self.root,
                    &crate::principal::CallerContext::trusted_operator(),
                    crate::team::TeamAction::Transition {
                        goal_id: goal.into(),
                        run_id: run_id.into(),
                        state: crate::team::RunState::LostResult,
                    },
                    &format!(
                        "run-release:{}",
                        route_core::sha256_hex(operation_key.as_bytes())
                    ),
                )?;
            }
        } else {
            crate::check_run::acknowledge_lost_result(&self.root, identity)?;
        }
        self.inspect(run_id)
    }
    #[cfg(not(windows))]
    pub fn inspect(&self, _run_id: &str) -> Result<Inspection> {
        anyhow::bail!("PROCESS_CONTINUITY_UNSUPPORTED: Windows Job Object implementation only")
    }
    #[cfg(not(windows))]
    pub fn cancel(&self, _run_id: &str, _operation_key: &str) -> Result<Inspection> {
        anyhow::bail!("PROCESS_TREE_CANCEL_UNSUPPORTED: no PID-only fallback")
    }
}
fn job_name(root: &Path, run: &RunIdentity, nonce: &str) -> String {
    format!(
        "Local\\Route.{}.{}",
        route_core::sha256_hex(root.to_string_lossy().as_bytes()),
        route_core::sha256_hex(
            format!(
                "{}:{}:{}:{}:{}",
                run.run_id, run.operation_id, run.owner, run.attempt, nonce
            )
            .as_bytes()
        )
    )
}

#[cfg(all(test, windows))]
mod tests;
