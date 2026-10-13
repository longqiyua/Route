//! Durable physical check observations. Evidence remains in the canonical store.
//! An uncertain external execution is NEVER retried merely because its owner died.
use crate::{
    execution::{self, CommandEvidence, EvidenceStore},
    ownership_lock::{atomic_replace, OwnershipLock},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckContract {
    pub argv: Vec<String>,
    pub cwd: String,
    pub executable_hash: String,
    // Only the digest is durable; inherited environment values are never serialized.
    pub environment_hash: String,
    pub policy_hash: String,
    pub obligations: Vec<String>,
    pub deadline_ms: u64,
}
impl CheckContract {
    pub fn approved(
        root: &Path,
        mut argv: Vec<String>,
        mut obligations: Vec<String>,
        policy_hash: String,
    ) -> Result<Self> {
        ensure!(
            !argv.is_empty() && !obligations.is_empty(),
            "INVALID_CHECK_CONTRACT"
        );
        let executable = resolve_executable(root, &argv[0])?;
        let executable_hash = route_core::hash::content_hash_file(&executable)?.0;
        argv[0] = executable.to_string_lossy().into_owned();
        obligations.sort();
        ensure!(
            obligations.windows(2).all(|pair| pair[0] != pair[1]),
            "DUPLICATE_OBLIGATION"
        );
        let mut env: Vec<_> = std::env::vars_os().collect();
        env.sort();
        let mut fields = Vec::new();
        for (name, value) in env {
            fields.push(
                name.into_string()
                    .map_err(|_| anyhow::anyhow!("NON_UTF8_EXECUTION_ENVIRONMENT"))?,
            );
            fields.push(
                value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("NON_UTF8_EXECUTION_ENVIRONMENT"))?,
            );
        }
        let environment_hash =
            route_core::hash::framed_content_hash(fields.iter().map(|s| s.as_bytes()));
        Ok(Self {
            argv,
            cwd: root.canonicalize()?.to_string_lossy().into_owned(),
            executable_hash,
            environment_hash,
            policy_hash,
            obligations,
            deadline_ms: 900_000,
        })
    }
}

// Resolve once to an explicit executable. Scripts need an approved interpreter;
// there is no implicit shell fallback or PATH re-selection at spawn time.
fn resolve_executable(root: &Path, name: &str) -> Result<PathBuf> {
    let requested = Path::new(name);
    let candidates: Vec<_> = if requested.is_absolute() {
        vec![requested.to_path_buf()]
    } else if requested.components().count() > 1 {
        vec![root.join(requested)]
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|directory| {
                let directory = if directory.is_absolute() {
                    directory
                } else {
                    root.join(directory)
                };
                directory.join(requested)
            })
            .collect()
    };
    for candidate in candidates {
        #[cfg(windows)]
        let candidate = if candidate.extension().is_none() {
            candidate.with_extension("exe")
        } else {
            candidate
        };
        if candidate.is_file() {
            return Ok(candidate.canonicalize()?);
        }
    }
    anyhow::bail!("APPROVED_EXECUTABLE_NOT_FOUND")
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckRunState {
    Reserved,
    PossiblyRunning,
    ResultPersisted,
    EvidenceCommitted,
    RecoveryRequired,
    /// Explicit Operator acknowledgement after OS tree termination was proven.
    /// Original key remains non-replayable; only a new approval may observe again.
    LostResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunDiagnostic {
    pub code: String,
    pub parent_termination_confirmed: Option<bool>,
    pub descendants: String,
    pub full_hashes: bool,
    pub next_action: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRun {
    pub run_id: String,
    pub intent_ref: String,
    pub contract: CheckContract,
    pub state: CheckRunState,
    pub owner: String,
    pub attempt: u64,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub world_before: String,
    pub result: Option<CommandEvidence>,
    pub metadata: BTreeMap<String, String>,
    pub evidence_ids: BTreeMap<String, String>,
    pub diagnostic: Option<RunDiagnostic>,
}
fn location(root: &Path, key: &str) -> PathBuf {
    root.join(".route/execution/check-runs")
        .join(format!("{}.json", route_core::sha256_hex(key.as_bytes())))
}
fn persist(path: &Path, run: &CheckRun) -> Result<()> {
    atomic_replace(path, &serde_json::to_vec_pretty(run)?)
}
pub fn inspect(root: &Path, key: &str) -> Result<CheckRun> {
    Ok(serde_json::from_slice(&std::fs::read(location(
        root, key,
    ))?)?)
}
fn ensure_owner(root: &Path, key: &str, run: &CheckRun) -> Result<()> {
    let current = inspect(root, key)?;
    ensure!(
        current.owner == run.owner && current.attempt == run.attempt,
        "CHECKRUN_FENCED"
    );
    Ok(())
}

/// Trusted Operator control plane only. One key is one observation, not a cache key.
/// New policies/worlds/repeated observations use a NEW explicitly approved operation.
/// Same-key uncertainty remains inspectable and must not launch another process.
pub fn execute(root: &Path, intent: &str, key: &str, contract: CheckContract) -> Result<CheckRun> {
    execute_inner(root, intent, key, contract, |_| Ok(()))
}
fn execute_inner(
    root: &Path,
    intent: &str,
    key: &str,
    contract: CheckContract,
    mut seam: impl FnMut(&str) -> Result<()>,
) -> Result<CheckRun> {
    ensure!(!key.is_empty(), "OPERATION_KEY_REQUIRED");
    let mut live_contract = CheckContract::approved(
        root,
        contract.argv.clone(),
        contract.obligations.clone(),
        contract.policy_hash.clone(),
    )?;
    live_contract.deadline_ms = contract.deadline_ms;
    ensure!(
        live_contract == contract && (1..=3_600_000).contains(&contract.deadline_ms),
        "EXECUTION_CONTEXT_CHANGED"
    );
    let path = location(root, key);
    std::fs::create_dir_all(path.parent().unwrap())?;
    // Conservative serial control-plane guard. A fresh key is not permission
    // to sidestep another uncertain execution under the same Goal.
    let _goal_owner = OwnershipLock::acquire(&root.join(".route/execution/.check-run-owner.lock"))?;
    for entry in std::fs::read_dir(path.parent().unwrap())? {
        let candidate = entry?.path();
        let name = candidate.file_name().unwrap().to_string_lossy();
        if candidate != path && name.len() == 69 && name.ends_with(".json") {
            let other: CheckRun = serde_json::from_slice(&std::fs::read(&candidate)?)?;
            ensure!(other.intent_ref != intent || !matches!(other.state, CheckRunState::PossiblyRunning | CheckRunState::RecoveryRequired), "RUN_RECOVERY_REQUIRED: another operation under this Goal has uncertain external execution; new key cannot bypass it");
        }
    }
    let _ownership =
        OwnershipLock::acquire(&path.with_extension("lock")).context("CHECKRUN_ALREADY_OWNED")?;
    let owner = route_core::new_id();
    let mut run: CheckRun = match std::fs::read(&path) {
        Ok(bytes) => {
            let previous: CheckRun = serde_json::from_slice(&bytes).context("CORRUPT_CHECKRUN")?;
            ensure!(
                previous.intent_ref == intent && previous.contract == contract,
                "IDEMPOTENCY_CONFLICT"
            );
            ensure!(!matches!(previous.state, CheckRunState::PossiblyRunning | CheckRunState::RecoveryRequired | CheckRunState::LostResult),
                "CHECKRUN_RESULT_UNCERTAIN: owner lock released but external termination is not proven; inspect run before any new observation");
            ensure!(
                previous.world_before == execution::compute_state_hash(root)?,
                "CHECKRUN_STALE: approve a new observation, never replay stale proof"
            );
            previous
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            ensure!(
                crate::general_work::status(root, intent)?.goal_state
                    == crate::general_work::GoalState::Active,
                "ACTIVE_GOAL_REQUIRED"
            );
            let world_before = execution::compute_state_hash(root)?;
            ensure!(!world_before.is_empty(), "STATE_HASH_UNAVAILABLE");
            let run = CheckRun {
                run_id: route_core::new_id(),
                intent_ref: intent.into(),
                contract,
                state: CheckRunState::Reserved,
                owner: owner.clone(),
                attempt: 1,
                created_at: route_core::now_millis(),
                finished_at: None,
                world_before,
                result: None,
                metadata: BTreeMap::new(),
                evidence_ids: BTreeMap::new(),
                diagnostic: None,
            };
            persist(&path, &run)?;
            run
        }
        Err(e) => return Err(e.into()),
    };
    if run.state != CheckRunState::EvidenceCommitted {
        if run.owner != owner {
            run.owner = owner.clone();
            run.attempt = run.attempt.checked_add(1).context("ATTEMPT_OVERFLOW")?;
            persist(&path, &run)?;
        }
        if run.state == CheckRunState::Reserved {
            let mut live_contract = CheckContract::approved(
                root,
                run.contract.argv.clone(),
                run.contract.obligations.clone(),
                run.contract.policy_hash.clone(),
            )?;
            live_contract.deadline_ms = run.contract.deadline_ms;
            ensure!(
                live_contract == run.contract
                    && (1..=3_600_000).contains(&live_contract.deadline_ms),
                "EXECUTION_CONTEXT_CHANGED"
            );
            ensure!(
                execution::compute_state_hash(root)? == run.world_before,
                "CHECKRUN_STALE"
            );
            seam("reserved")?;
            let team_exists = match crate::team::status(root, intent) {
                Ok(team) => {
                    ensure!(!team.cancelled, "RUN_CANCEL_REQUESTED: not started");
                    true
                }
                Err(error) if error.to_string() == "TEAM_NOT_CONFIGURED" => false,
                Err(error) => return Err(error),
            };
            // Persist uncertainty BEFORE spawn, closing the spawn->started crash window.
            // This may conservatively require investigation even if spawn never happened.
            run.state = CheckRunState::PossiblyRunning;
            persist(&path, &run)?;
            seam("before_spawn")?;
            let mut last_poll = std::time::Instant::now() - std::time::Duration::from_secs(1);
            match execution::observe_command_owned(
                root,
                &run.contract.argv,
                Some(&run.contract.obligations[0]),
                std::time::Duration::from_millis(run.contract.deadline_ms),
                || {
                    if team_exists && last_poll.elapsed() >= std::time::Duration::from_millis(250) {
                        last_poll = std::time::Instant::now();
                        return Ok(crate::team::status(root, intent)?.cancelled);
                    }
                    Ok(false)
                },
                Some(crate::process_supervision::RunIdentity {
                    run_id: run.run_id.clone(),
                    operation_id: key.into(),
                    owner: run.owner.clone(),
                    attempt: run.attempt,
                    team_goal: None,
                }),
            ) {
                Ok((result, metadata)) => {
                    ensure!(
                        metadata.get("state_hash_before") == Some(&run.world_before),
                        "CHECKRUN_WORLD_CHANGED_BEFORE_SPAWN"
                    );
                    seam("after_exit")?;
                    ensure_owner(root, key, &run)?;
                    run.finished_at = Some(route_core::now_millis());
                    run.result = Some(result);
                    run.metadata = metadata.into_iter().collect();
                    run.state = CheckRunState::ResultPersisted;
                    persist(&path, &run)?;
                }
                Err(error) => {
                    ensure_owner(root, key, &run)?;
                    run.state = CheckRunState::RecoveryRequired;
                    let message = error
                        .chain()
                        .map(ToString::to_string)
                        .find(|message| {
                            message.starts_with("RUN_")
                                || message.starts_with("OUTPUT_INCOMPLETE:")
                                || message.starts_with("PROCESS_STATUS_UNKNOWN:")
                                || message.starts_with("CANCEL_STATUS_UNKNOWN:")
                        })
                        .unwrap_or_default();
                    // Only controlled codes, not OS details containing private argv.
                    run.diagnostic = Some(RunDiagnostic {
                        code: message.split(':').next().filter(|s| !s.is_empty()).unwrap_or("EXECUTION_NOT_COMMITTED").into(),
                        parent_termination_confirmed: if message.contains("parent_termination_confirmed=true") || message.contains("direct_parent_exited=true") { Some(true) }
                            else if message.contains("parent_termination_confirmed=false") { Some(false) } else { None },
                        descendants: "UNKNOWN".into(), full_hashes: false,
                        next_action: "Inspect run and owned process; do not automatically retry or kill unknown processes".into(),
                    });
                    persist(&path, &run)?;
                    return Err(error).context("RUN_RECOVERY_REQUIRED");
                }
            }
        }
        seam("before_evidence")?;
        let disk_owner = inspect(root, key)?;
        ensure!(
            disk_owner.owner == run.owner && disk_owner.attempt == run.attempt,
            "CHECKRUN_FENCED"
        );
        let observation = run.result.as_ref().context("CHECKRUN_RESULT_MISSING")?;
        // Preserve an actual FAIL observation, but never promote unstable/changed proof.
        ensure!(
            observation.state_hash == execution::compute_state_hash(root)?,
            "CHECKRUN_STALE"
        );
        for label in &run.contract.obligations {
            let mut metadata: std::collections::HashMap<_, _> =
                run.metadata.clone().into_iter().collect();
            metadata.insert("check_id".into(), label.clone());
            metadata.insert("check_run_id".into(), run.run_id.clone());
            metadata.insert(
                "check_contract_hash".into(),
                route_core::sha256_hex(&serde_json::to_vec(&run.contract)?),
            );
            let evidence_id = execution::commit_command_observation(
                root,
                intent,
                observation,
                metadata,
                &format!("{}:{label}", run.run_id),
            )
            .context("EVIDENCE_COMMIT_FAILED: result persisted; retry exact operation")?;
            run.evidence_ids.insert(label.clone(), evidence_id);
        }
        seam("after_evidence")?;
        // Lock is retained through execution/result/Evidence. No other supported owner
        // can take over; disk owner/attempt check also fences stale internal callbacks.
        let disk = inspect(root, key)?;
        ensure!(
            disk.owner == run.owner && disk.attempt == run.attempt,
            "CHECKRUN_FENCED"
        );
        run.state = CheckRunState::EvidenceCommitted;
        persist(&path, &run)?;
    }
    let observation = run.result.as_ref().context("CHECKRUN_RESULT_MISSING")?;
    ensure!(
        observation.state_hash == execution::compute_state_hash(root)?,
        "CHECKRUN_STALE"
    );
    let store = EvidenceStore::load(root)?;
    for label in &run.contract.obligations {
        let id = run
            .evidence_ids
            .get(label)
            .context("CHECKRUN_EVIDENCE_MISSING")?;
        let evidence = store
            .evidence
            .iter()
            .find(|e| &e.id == id)
            .context("CHECKRUN_EVIDENCE_MISSING")?;
        ensure!(
            evidence.metadata.get("check_run_id") == Some(&run.run_id)
                && !store.has_later_failed_check(evidence),
            "CHECKRUN_PROOF_INVALIDATED"
        );
    }
    Ok(run)
}

pub(crate) fn acknowledge_lost_result(
    root: &Path,
    identity: &crate::process_supervision::RunIdentity,
) -> Result<()> {
    let path = location(root, &identity.operation_id);
    let _lock = OwnershipLock::acquire(&path.with_extension("lock"))?;
    let mut run = inspect(root, &identity.operation_id)?;
    ensure!(
        run.run_id == identity.run_id
            && run.owner == identity.owner
            && run.attempt == identity.attempt,
        "CHECKRUN_FENCED"
    );
    if run.state == CheckRunState::LostResult {
        return Ok(());
    }
    ensure!(
        run.result.is_none()
            && matches!(
                run.state,
                CheckRunState::PossiblyRunning | CheckRunState::RecoveryRequired
            ),
        "RECOVERY_NOT_APPLICABLE"
    );
    run.state = CheckRunState::LostResult;
    persist(&path, &run)
}

pub(crate) fn restore_unlaunched(
    root: &Path,
    identity: &crate::process_supervision::RunIdentity,
) -> Result<()> {
    let path = location(root, &identity.operation_id);
    let _lock = OwnershipLock::acquire(&path.with_extension("lock"))?;
    let mut run = inspect(root, &identity.operation_id)?;
    ensure!(
        run.run_id == identity.run_id
            && run.owner == identity.owner
            && run.attempt == identity.attempt
            && run.result.is_none()
            && matches!(
                run.state,
                CheckRunState::Reserved
                    | CheckRunState::PossiblyRunning
                    | CheckRunState::RecoveryRequired
            ),
        "CHECKRUN_FENCED_OR_NOT_RECOVERABLE"
    );
    run.state = CheckRunState::Reserved;
    persist(&path, &run)
}

#[cfg(test)]
#[path = "check_run_tests.rs"]
mod tests;
