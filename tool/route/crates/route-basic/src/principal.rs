//! Trusted-local caller binding. Credentials never enter serializable requests/events.
use crate::development::{
    self, AppendDevelopmentEventResult, DevelopmentEvent, DevelopmentEventDraft,
    DevelopmentEventPayload,
};
use anyhow::{anyhow, ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

const WORKER_METHODS: &[&str] = &[
    "worker.message.send",
    "worker.presence.update",
    "development.event.record",
    "cooperation.knowledge.record",
];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerBinding {
    pub binding_id: String,
    pub worker_id: String,
    pub project_id: String,
    pub issued_by: String,
    pub created_at: i64,
    pub status: String,
    pub credential_hash: String,
    pub grants: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum BindingChange {
    Issue { binding: WorkerBinding },
    Revoke { binding_id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BindingTransaction {
    pub request_hash: String,
    pub change: BindingChange,
}

// No Deserialize or external constructor for a Worker principal.
#[derive(Clone)]
enum Principal {
    Operator,
    Worker(WorkerBinding),
    Anonymous,
}
#[derive(Clone)]
pub struct CallerContext {
    principal: Principal,
}
impl CallerContext {
    /// Trusted embedding/CLI launch authority, never selected from request JSON.
    pub fn trusted_operator() -> Self {
        Self {
            principal: Principal::Operator,
        }
    }
    pub fn anonymous() -> Self {
        Self {
            principal: Principal::Anonymous,
        }
    }
    pub fn authenticate(root: &Path, credential: &str) -> Result<Self> {
        ensure!(
            credential.len() == 64 && credential.bytes().all(|b| b.is_ascii_hexdigit()),
            "AUTHENTICATION_DENIED"
        );
        let hash = route_core::sha256_hex(credential.as_bytes());
        let binding = bindings(root)?
            .into_iter()
            .find(|b| b.credential_hash == hash && b.status == "ACTIVE")
            .ok_or_else(|| anyhow!("AUTHENTICATION_DENIED"))?;
        Ok(Self {
            principal: Principal::Worker(binding),
        })
    }
    pub fn scope(&self) -> String {
        match &self.principal {
            Principal::Operator => "operator".into(),
            Principal::Anonymous => "anonymous".into(),
            Principal::Worker(b) => format!("worker:{}:{}", b.worker_id, b.binding_id),
        }
    }
    pub fn worker_id(&self) -> Option<&str> {
        match &self.principal {
            Principal::Worker(b) => Some(&b.worker_id),
            _ => None,
        }
    }
    pub fn require_operator(&self) -> Result<()> {
        ensure!(
            matches!(self.principal, Principal::Operator),
            "OPERATOR_REQUIRED"
        );
        Ok(())
    }
    pub fn authorize(
        &self,
        root: &Path,
        method: &str,
        params: &mut Value,
        mutation: bool,
    ) -> Result<()> {
        ensure!(params.is_object(), "INVALID_PARAMS: expected object");
        for key in [
            "principal",
            "caller",
            "authenticated_principal",
            "operator_identity",
            "system_identity",
            "credential",
            "token",
        ] {
            ensure!(params.get(key).is_none(), "CALLER_CONTEXT_FORBIDDEN");
        }
        // Provenance labels are written by this boundary, never asserted by payload.
        for refs in [params.get("source_refs"), params.get("evidence_refs")] {
            if let Some(Value::Array(refs)) = refs {
                ensure!(
                    !refs
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|r| r.starts_with("caller:")),
                    "CALLER_CONTEXT_FORBIDDEN"
                );
            }
        }
        match &self.principal {
            Principal::Anonymous => ensure!(
                !mutation,
                "AUTHENTICATION_REQUIRED: use a Worker binding or explicit --operator"
            ),
            Principal::Operator => {
                ensure!(
                    !matches!(method, "worker.message.send" | "worker.presence.update"),
                    "WORKER_BINDING_REQUIRED"
                );
                if method == "development.event.record" {
                    ensure!(
                        !matches!(
                            params["payload"]["kind"].as_str(),
                            Some(
                                "WORKER_MESSAGE"
                                    | "WORKER_PRESENCE_UPDATED"
                                    | "WORKER_REGISTERED"
                                    | "WORKER_METADATA_UPDATED"
                            )
                        ),
                        "AUTHORITY_DENIED: dedicated domain ingress required"
                    );
                }
                if method == "institution.activate" {
                    params["activated_by"] = json!("local-operator");
                }
                if method == "institution.deactivate" {
                    params["actor"] = json!("local-operator");
                }
                // Existing local Operator ingress is explicit, not a Worker identity.
                if !matches!(
                    method,
                    "worker.message.send" | "worker.presence.update" | "worker.register"
                ) {
                    ensure!(
                        params.get("actor_worker_id").is_none_or(Value::is_null),
                        "ACTOR_MISMATCH: Operator is not a Worker"
                    );
                }
            }
            Principal::Worker(b) => {
                validate_binding(root, b)?;
                if mutation {
                    ensure!(
                        WORKER_METHODS.contains(&method) && b.grants.iter().any(|g| g == method),
                        "OPERATOR_REQUIRED: binding does not grant this operation"
                    );
                    for key in ["worker_id", "actor_worker_id", "from_worker"] {
                        if let Some(v) = params.get(key) {
                            ensure!(
                                v.is_null() || v.as_str() == Some(&b.worker_id),
                                "ACTOR_MISMATCH"
                            );
                        }
                    }
                    ensure!(
                        params.get("institution_id").is_none()
                            && params.get("activated_by").is_none(),
                        "ACTOR_MISMATCH"
                    );
                    if matches!(
                        method,
                        "development.event.record" | "cooperation.knowledge.record"
                    ) {
                        params["actor_worker_id"] = json!(b.worker_id);
                    } else {
                        params["worker_id"] = json!(b.worker_id);
                        params.as_object_mut().unwrap().remove("from_worker");
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn generate_credential() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| anyhow!("OS credential generation failed"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
pub fn register_worker(
    root: &Path,
    caller: &CallerContext,
    worker_id: Option<String>,
    metadata: development::WorkerMetadata,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.require_operator()?;
    let descriptor = development::WorkerDescriptor {
        worker_id: worker_id
            .unwrap_or_else(|| format!("worker-{}", route_core::sha256_hex(key.as_bytes()))),
        created_at: route_core::now_millis(),
        metadata,
    };
    development::append_development_event(
        root,
        serde_json::from_value(
            json!({"payload":DevelopmentEventPayload::WorkerRegistered{descriptor},"deduplication_key":key,"source_refs":["caller:operator"]}),
        )?,
    )
}
pub(crate) fn project(events: &[DevelopmentEvent]) -> BTreeMap<String, WorkerBinding> {
    let mut out = BTreeMap::new();
    for event in events {
        if let DevelopmentEventPayload::WorkerBinding { transaction } = &event.payload {
            match &transaction.change {
                BindingChange::Issue { binding } => {
                    out.insert(binding.binding_id.clone(), binding.clone());
                }
                BindingChange::Revoke { binding_id } => {
                    if let Some(b) = out.get_mut(binding_id) {
                        b.status = "REVOKED".into();
                    }
                }
            }
        }
    }
    out
}
pub fn bindings(root: &Path) -> Result<Vec<WorkerBinding>> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    Ok(project(&ledger.events)
        .into_values()
        .filter(|b| b.project_id == identity.project_id)
        .collect())
}
fn validate_binding(root: &Path, binding: &WorkerBinding) -> Result<()> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    validate_bound_events(&ledger.events, &identity.project_id, binding)
}
fn validate_bound_events(
    events: &[DevelopmentEvent],
    project_id: &str,
    b: &WorkerBinding,
) -> Result<()> {
    ensure!(
        project_id == b.project_id
            && project(events)
                .get(&b.binding_id)
                .is_some_and(|current| current == b && current.status == "ACTIVE"),
        "AUTHENTICATION_DENIED: stale, revoked or foreign binding"
    );
    Ok(())
}
pub(crate) fn validate_caller(
    events: &[DevelopmentEvent],
    project_id: &str,
    caller: &CallerContext,
    draft: &DevelopmentEventDraft,
) -> Result<()> {
    if let Principal::Worker(b) = &caller.principal {
        validate_bound_events(events, project_id, b)?;
        ensure!(
            draft.actor_worker_id.as_deref() == Some(&b.worker_id),
            "ACTOR_MISMATCH"
        );
        ensure!(
            matches!(
                &draft.payload,
                DevelopmentEventPayload::Finding { .. }
                    | DevelopmentEventPayload::WorkerMessage { .. }
                    | DevelopmentEventPayload::WorkerPresenceUpdated { .. }
                    | DevelopmentEventPayload::CooperationKnowledgeRecorded { .. }
            ),
            "AUTHORITY_DENIED"
        );
    }
    Ok(())
}
pub(crate) fn validate_transition(
    events: &[DevelopmentEvent],
    project_id: &str,
    tx: &BindingTransaction,
) -> Result<()> {
    let state = project(events);
    match &tx.change {
        BindingChange::Issue { binding } => {
            ensure!(
                binding.project_id == project_id,
                "PROJECT_IDENTITY_CONFLICT"
            );
            ensure!(
                !state.contains_key(&binding.binding_id)
                    && !state
                        .values()
                        .any(|b| b.credential_hash == binding.credential_hash),
                "BINDING_CONFLICT"
            );
            ensure!(events.iter().any(|e|matches!(&e.payload,DevelopmentEventPayload::WorkerRegistered{descriptor} if descriptor.worker_id==binding.worker_id)),"UNKNOWN_WORKER");
        }
        BindingChange::Revoke { binding_id } => {
            ensure!(state.contains_key(binding_id), "UNKNOWN_BINDING");
        }
    }
    Ok(())
}
pub fn issue(
    root: &Path,
    caller: &CallerContext,
    worker_id: &str,
    credential_hash: &str,
    grants: Vec<String>,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.require_operator()?;
    ensure!(
        credential_hash.len() == 64 && credential_hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "INVALID_CREDENTIAL_VERIFIER"
    );
    ensure!(
        grants.len() <= 4 && grants.iter().all(|g| WORKER_METHODS.contains(&g.as_str())),
        "AUTHORITY_DENIED"
    );
    let (_, identity, _) = development::load_ledger_readonly(root)?;
    let request_hash = route_core::sha256_hex(&serde_json::to_vec(&json!([
        "issue",
        worker_id,
        credential_hash,
        grants
    ]))?);
    let binding = WorkerBinding {
        binding_id: format!(
            "binding-{}",
            route_core::sha256_hex(format!("{}:{key}", identity.project_id).as_bytes())
        ),
        worker_id: worker_id.into(),
        project_id: identity.project_id,
        issued_by: "local-operator".into(),
        created_at: route_core::now_millis(),
        status: "ACTIVE".into(),
        credential_hash: credential_hash.into(),
        grants,
    };
    append_binding(
        root,
        BindingTransaction {
            request_hash,
            change: BindingChange::Issue { binding },
        },
        key,
    )
}
pub fn revoke(
    root: &Path,
    caller: &CallerContext,
    binding_id: &str,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.require_operator()?;
    append_binding(
        root,
        BindingTransaction {
            request_hash: route_core::sha256_hex(&serde_json::to_vec(&json!([
                "revoke", binding_id
            ]))?),
            change: BindingChange::Revoke {
                binding_id: binding_id.into(),
            },
        },
        key,
    )
}
fn append_binding(
    root: &Path,
    transaction: BindingTransaction,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    let draft = serde_json::from_value(
        json!({"payload":DevelopmentEventPayload::WorkerBinding{transaction},"deduplication_key":key,"source_refs":["caller:operator"]}),
    )?;
    development::append_binding_event(root, draft)
}

/// Actor-sensitive Worker domain entrance shared by RPC/CLI and embedding hosts.
pub fn worker_action(
    root: &Path,
    caller: &CallerContext,
    method: &str,
    mut params: Value,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    caller.authorize(root, method, &mut params, true)?;
    let worker = caller
        .worker_id()
        .ok_or_else(|| anyhow!("WORKER_BINDING_REQUIRED"))?;
    let actor = worker.to_string();
    let obj = params
        .as_object_mut()
        .ok_or_else(|| anyhow!("INVALID_PARAMS"))?;
    obj.remove("worker_id");
    obj.remove("from_worker");
    obj.remove("actor_worker_id");
    let mut draft: DevelopmentEventDraft = match method {
        "worker.message.send" => {
            let input: development::WorkerMessageInput = serde_json::from_value(params)?;
            if let Some(target) = &input.target_worker {
                ensure!(
                    development::worker_descriptors(root)?
                        .iter()
                        .any(|w| &w.worker_id == target),
                    "UNKNOWN_WORKER"
                );
            }
            let message = development::WorkerMessage {
                message_id: input
                    .message_id
                    .unwrap_or_else(|| format!("msg-{}", route_core::sha256_hex(key.as_bytes()))),
                from_worker: actor.clone(),
                target_worker: input.target_worker,
                intent_ref: input.intent_ref.clone(),
                task_ref: input.task_ref.clone(),
                thread_ref: input.thread_ref,
                message_type: input.message_type,
                content: input.content,
                source_refs: input.source_refs,
                requires_response: input.requires_response,
                timestamp: route_core::now_millis(),
            };
            serde_json::from_value(
                json!({"payload":DevelopmentEventPayload::WorkerMessage{message},"intent_ref":input.intent_ref,"task_ref":input.task_ref}),
            )?
        }
        "worker.presence.update" => {
            let presence: development::WorkerPresenceInput = serde_json::from_value(params)?;
            serde_json::from_value(
                json!({"payload":DevelopmentEventPayload::WorkerPresenceUpdated{presence}}),
            )?
        }
        "cooperation.knowledge.record" => {
            if params.get("observed_at").is_none() {
                params["observed_at"] = json!(0);
            }
            let record: crate::CooperationKnowledgeRecord = serde_json::from_value(params)?;
            let refs = record.source_refs.clone();
            let evidence = record.evidence_refs.clone();
            serde_json::from_value(
                json!({"payload":DevelopmentEventPayload::CooperationKnowledgeRecorded{record},"source_refs":refs,"evidence_refs":evidence}),
            )?
        }
        "development.event.record" => {
            let draft: DevelopmentEventDraft = serde_json::from_value(params)?;
            ensure!(
                matches!(draft.payload, DevelopmentEventPayload::Finding { .. }),
                "AUTHORITY_DENIED: only findings are Worker-writable"
            );
            ensure!(
                draft.evidence_refs.is_empty(),
                "AUTHORITY_DENIED: no Evidence fabrication"
            );
            draft
        }
        _ => return Err(anyhow!("AUTHORITY_DENIED")),
    };
    draft.actor_worker_id = Some(actor);
    draft.deduplication_key = Some(key.into());
    draft.source_refs.push(format!("caller:{}", caller.scope()));
    development::append_authenticated_event(root, draft, caller)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusted_context_revalidates_revocation_and_rejects_non_objects() {
        let root = tempfile::tempdir().unwrap();
        let operator = CallerContext::trusted_operator();
        register_worker(
            root.path(),
            &operator,
            Some("a".into()),
            Default::default(),
            "register",
        )
        .unwrap();
        let secret = generate_credential().unwrap();
        issue(
            root.path(),
            &operator,
            "a",
            &route_core::sha256_hex(secret.as_bytes()),
            vec!["worker.message.send".into()],
            "issue",
        )
        .unwrap();
        let caller = CallerContext::authenticate(root.path(), &secret).unwrap();
        assert_eq!(caller.worker_id(), Some("a"));
        assert!(caller
            .authorize(root.path(), "worker.message.send", &mut Value::Null, true)
            .is_err());
        assert!(worker_action(
            root.path(),
            &caller,
            "worker.message.send",
            json!({"from_worker":"b","message_type":"NOTICE","content":"spoof"}),
            "bad"
        )
        .is_err());
        let input = json!({"message_type":"NOTICE","content":"own"});
        let first = worker_action(
            root.path(),
            &caller,
            "worker.message.send",
            input.clone(),
            "once",
        )
        .unwrap();
        assert_eq!(first.event.actor_worker_id.as_deref(), Some("a"));
        assert_eq!(
            first.event.event_id,
            worker_action(
                root.path(),
                &caller,
                "worker.message.send",
                input.clone(),
                "once"
            )
            .unwrap()
            .event
            .event_id
        );
        let id = bindings(root.path()).unwrap()[0].binding_id.clone();
        revoke(root.path(), &operator, &id, "revoke").unwrap();
        // A context retained by an embedding host is not perpetual authority.
        assert!(worker_action(root.path(), &caller, "worker.message.send", input, "once").is_err());
        assert!(CallerContext::authenticate(root.path(), &secret).is_err());
    }

    #[test]
    fn worker_grants_and_provenance_are_not_payload_authority() {
        let root = tempfile::tempdir().unwrap();
        let op = CallerContext::trusted_operator();
        register_worker(
            root.path(),
            &op,
            Some("a".into()),
            Default::default(),
            "register",
        )
        .unwrap();
        let secret = generate_credential().unwrap();
        let hash = route_core::sha256_hex(secret.as_bytes());
        assert!(issue(
            root.path(),
            &op,
            "a",
            &hash,
            vec!["institution.invoke".into()],
            "invalid"
        )
        .is_err());
        issue(
            root.path(),
            &op,
            "a",
            &hash,
            vec!["worker.message.send".into()],
            "issue",
        )
        .unwrap();
        let caller = CallerContext::authenticate(root.path(), &secret).unwrap();
        assert!(caller.require_operator().is_err());
        assert!(caller
            .authorize(root.path(), "worker.presence.update", &mut json!({}), true)
            .is_err());
        assert!(worker_action(root.path(), &caller, "worker.message.send", json!({"message_type":"NOTICE","content":"forged attribution","source_refs":["caller:operator"]}), "forged").is_err());
        let foreign = tempfile::tempdir().unwrap();
        register_worker(
            foreign.path(),
            &op,
            Some("a".into()),
            Default::default(),
            "register",
        )
        .unwrap();
        assert!(caller
            .authorize(foreign.path(), "worker.list", &mut json!({}), false)
            .is_err());
        assert!(CallerContext::anonymous()
            .authorize(root.path(), "worker.register", &mut json!({}), true)
            .is_err());
    }
}
