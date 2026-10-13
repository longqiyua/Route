//! Recoverable Operator finalization over existing keyed domain mutations.
//! This journal is progress, not Evidence or a substitute for CompletionGate.
use anyhow::{ensure, Result};
use route_basic::{
    execution,
    ownership_lock::{atomic_replace, OwnershipLock},
    AppendDevelopmentEventResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[cfg(test)]
thread_local! { pub(crate) static CRASH_AT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) }; }
#[cfg(test)]
fn crash_at(point: &str) {
    CRASH_AT.with(|current| {
        if current.borrow().as_deref() == Some(point) {
            std::process::exit(23);
        }
    });
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    domain: String,
    method: String,
    params: Value,
    result: Option<AppendDevelopmentEventResult>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    identity: String,
    goal: String,
    policy: String,
    world: String,
    steps: BTreeMap<String, Step>,
    done: bool,
}
pub(crate) struct Journal {
    path: PathBuf,
    record: Record,
    _owner: OwnershipLock,
}
impl Journal {
    pub fn begin(root: &Path, goal: &str, key: &str, policy: String) -> Result<Self> {
        ensure!(!key.is_empty() && key.len() <= 512, "INVALID_OPERATION_KEY");
        let identity = route_core::sha256_hex(key.as_bytes());
        let path = root
            .join(".route/execution/finalizations")
            .join(format!("{identity}.json"));
        std::fs::create_dir_all(path.parent().unwrap())?;
        let owner = OwnershipLock::acquire(&path.with_extension("lock"))?;
        let world = execution::compute_state_hash(root)?;
        let record = match std::fs::read(&path) {
            Ok(bytes) => {
                let old: Record = serde_json::from_slice(&bytes)?;
                ensure!(
                    old.goal == goal && old.policy == policy,
                    "IDEMPOTENCY_CONFLICT"
                );
                ensure!(
                    old.world == world,
                    "FINALIZATION_WORLD_CHANGED: inspect old operation and approve fresh checks"
                );
                old
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Record {
                identity,
                goal: goal.into(),
                policy,
                world,
                steps: BTreeMap::new(),
                done: false,
            },
            Err(e) => return Err(e.into()),
        };
        let journal = Self {
            path,
            record,
            _owner: owner,
        };
        journal.save()?;
        Ok(journal)
    }
    fn save(&self) -> Result<()> {
        atomic_replace(&self.path, &serde_json::to_vec_pretty(&self.record)?)
    }
    pub fn identity(&self) -> &str {
        &self.record.identity
    }
    pub fn done(&self) -> bool {
        self.record.done
    }
    pub fn recover_closed(&mut self, root: &Path) -> Result<()> {
        let close = self
            .record
            .steps
            .get("goal-close")
            .ok_or_else(|| anyhow::anyhow!("FINALIZATION_CLOSE_NOT_RESERVED"))?;
        ensure!(
            close.domain == "general"
                && close.method == "goal.close"
                && close.params["goal_id"] == self.record.goal
                && close.params["state"] == "SUCCEEDED",
            "FINALIZATION_CLOSE_CONFLICT"
        );
        ensure!(
            self.record
                .steps
                .iter()
                .filter(|(name, _)| name.as_str() != "goal-close")
                .all(|(_, step)| step.result.is_some())
                && ["integrate", "workflow-complete", "outcome"]
                    .iter()
                    .all(|name| self.record.steps.contains_key(*name)),
            "FINALIZATION_PROGRESS_INCOMPLETE"
        );
        route_basic::work::ensure_integrated_before_success(root, &self.record.goal)?;
        ensure!(
            route_basic::general_work::review(root, &self.record.goal)?.completion == "PASS",
            "FINALIZATION_NO_LONGER_VALID"
        );
        let mut params = close.params.clone();
        params.as_object_mut().unwrap().remove("expected_revision");
        // Replay the original canonical close receipt, never invent another close
        // from the mere presence of a SUCCEEDED projection.
        self.action(root, "general", "goal.close", params, "goal-close")?;
        self.finish()
    }
    pub fn finish(&mut self) -> Result<()> {
        self.record.done = true;
        self.save()
    }
    pub fn action(
        &mut self,
        root: &Path,
        domain: &str,
        method: &str,
        mut params: Value,
        name: &str,
    ) -> Result<AppendDevelopmentEventResult> {
        ensure!(
            execution::compute_state_hash(root)? == self.record.world,
            "FINALIZATION_WORLD_CHANGED"
        );
        let request = params.clone();
        if let Some(step) = self.record.steps.get(name) {
            let mut prior = step.params.clone();
            prior.as_object_mut().unwrap().remove("expected_revision");
            ensure!(
                step.domain == domain && step.method == method && prior == request,
                "FINALIZATION_STEP_CONFLICT"
            );
            if let Some(result) = &step.result {
                return Ok(result.clone());
            }
            params = step.params.clone();
        } else {
            params["expected_revision"] =
                serde_json::json!(route_basic::global_development_revision(root)?);
            self.record.steps.insert(
                name.into(),
                Step {
                    domain: domain.into(),
                    method: method.into(),
                    params: params.clone(),
                    result: None,
                },
            );
            self.save()?;
        }
        // Persist exact original params including revision BEFORE invoking canonical
        // idempotent domain ingress. Lost response retries exactly these bytes/key.
        let key = format!(
            "finalize:{}:{}",
            self.record.identity,
            route_core::sha256_hex(name.as_bytes())
        );
        let caller = route_basic::principal::CallerContext::trusted_operator();
        #[cfg(test)]
        crash_at(&format!("before:{method}"));
        let result = match domain {
            "general" => {
                route_basic::general_work::operator_action(root, &caller, method, params, &key)
            }
            "contract" => route_basic::execution_contract::operator_action(
                root, &caller, method, params, &key,
            ),
            "work" => route_basic::work::operator_action(root, &caller, method, params, &key),
            _ => anyhow::bail!("UNKNOWN_FINALIZATION_DOMAIN"),
        }
        .map_err(|e| {
            e.context("FINALIZATION_RECOVERY_REQUIRED: retry same operation; do not invent success")
        })?;
        #[cfg(test)]
        crash_at(&format!("after:{method}"));
        self.record.steps.get_mut(name).unwrap().result = Some(result.clone());
        self.save()?;
        Ok(result)
    }
}
