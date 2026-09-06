//! Route Cooperation Protocol `route/1` stdio transport.
//!
//! This module is a transport adapter only.  It exposes existing Route state
//! through stable JSON envelopes and never grants authority beyond the domain
//! services it invokes.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

const PROTOCOL: &str = "route/1";
// Discovery and dispatch share one definition; an unimplemented name cannot leak.
macro_rules! route_methods {
    ($request:ident, $root:ident, $version:ident; $( $($name:literal)|+ => $body:expr ),+ $(,)?) => {
        const METHODS: &[&str] = &[$($($name),+),+];
        fn invoke_method($request: Request, $root: PathBuf) -> Value {
            let $version = env!("CARGO_PKG_VERSION");
            match $request.method.as_str() {
                $($($name)|+ => $body,)+
                _ => error(Some(&$request.request_id), "UNKNOWN_METHOD",
                    "method is not available in route/1", false, json!({"available_methods": METHODS})),
            }
        }
    };
}

#[derive(Debug, Deserialize)]
struct Request {
    protocol: String,
    request_id: String,
    method: String,
    #[serde(default)]
    context: Value,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    idempotency_key: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

fn error(
    request_id: Option<&str>,
    code: &str,
    message: impl Into<String>,
    retryable: bool,
    details: Value,
) -> Value {
    json!({"protocol": PROTOCOL, "request_id": request_id, "ok": false,
        "result": Value::Null, "error": {"code": code, "message": message.into(),
        "retryable": retryable, "details": details},
        "receipt": {"method": Value::Null, "operation_status": "REJECTED", "warnings": []}})
}

fn success(request: &Request, result: Value, refs: Vec<String>) -> Value {
    json!({"protocol": PROTOCOL, "request_id": request.request_id, "ok": true,
        "result": result, "error": Value::Null,
        "receipt": {"receipt_id": format!("rpc:{}", request.request_id), "method": request.method,
          "project_refs": [], "affected_object_refs": refs, "operation_status": "COMPLETED",
          "created_at": route_core::now_millis(), "evidence_history_refs": [],
          "idempotency_key": request.idempotency_key, "warnings": []}})
}

fn root_from_context(context: &Value) -> std::result::Result<PathBuf, (&'static str, String)> {
    let cwd = std::env::current_dir().map_err(|e| ("INTERNAL_ERROR", e.to_string()))?;
    let root = route_basic::discover_project_root(&cwd).unwrap_or_else(|| cwd.clone());
    let supplied = context.get("root_locator").and_then(Value::as_str);
    if let Some(supplied_root) = supplied {
        let candidate = PathBuf::from(supplied_root)
            .canonicalize()
            .map_err(|_| ("PROJECT_NOT_FOUND", "root_locator does not resolve".into()))?;
        let actual = root
            .canonicalize()
            .map_err(|e| ("INTERNAL_ERROR", e.to_string()))?;
        if candidate != actual {
            return Err((
                "PROJECT_IDENTITY_CONFLICT",
                "root_locator conflicts with this Route process project".into(),
            ));
        }
    }
    validate_context_identity(&root, context)?;
    Ok(root)
}

fn validate_context_identity(
    root: &Path,
    context: &Value,
) -> std::result::Result<(), (&'static str, String)> {
    if context.get("project_id").is_none() && context.get("workspace_id").is_none() {
        return Ok(());
    }

    let identity = route_basic::load_identity(root)
        .map_err(|e| ("PROJECT_IDENTITY_UNAVAILABLE", e.to_string()))?
        .ok_or_else(|| {
            (
                "PROJECT_IDENTITY_UNAVAILABLE",
                "run route init first".into(),
            )
        })?;
    for (field, expected) in [
        ("project_id", identity.project_id.as_str()),
        ("workspace_id", identity.workspace_id.as_str()),
    ] {
        let Some(value) = context.get(field) else {
            continue;
        };
        let Some(supplied) = value.as_str() else {
            return Err((
                "PROJECT_IDENTITY_CONFLICT",
                format!("{field} must be a string matching this Route process project"),
            ));
        };
        if supplied != expected {
            return Err((
                "PROJECT_IDENTITY_CONFLICT",
                format!("{field} conflicts with this Route process project"),
            ));
        }
    }
    Ok(())
}

fn project(root: &Path) -> Result<Value> {
    let route_dir = root.join(".route");
    let git = root.join(".git");
    let identity = route_basic::load_identity(root)?
        .ok_or_else(|| anyhow!("project identity is missing; run route init first; no automatic initialization was performed"))?;
    let sessions = route_basic::session_status(root, None).unwrap_or_default();
    let active = sessions
        .iter()
        .find(|s| matches!(s.status, route_basic::SessionStatus::Active));
    Ok(
        json!({"resolved_root": root.to_string_lossy(), "project_id": identity.project_id,
      "workspace_id": identity.workspace_id, "root_locator": root.to_string_lossy(),
      "route_state_present": route_dir.exists(), "git_present": git.exists(),
      "identity_status": "STABLE_PERSISTED", "ambiguity": false,
      "current_intent": active.map(|s| json!({"intent_ref": s.id, "summary": s.task, "status": "ACTIVE"})),
      "applicable_capabilities": METHODS}),
    )
}

fn idem_path(root: &Path) -> PathBuf {
    root.join(".route").join("rpc-idempotency.json")
}

struct IdempotencyLock {
    _guard: route_basic::ownership_lock::OwnershipLock,
}
impl IdempotencyLock {
    fn acquire(root: &Path) -> Result<Self> {
        fs::create_dir_all(root.join(".route"))?;
        Ok(Self {
            _guard: route_basic::ownership_lock::OwnershipLock::acquire(
                &root.join(".route/.rpc-idempotency-lock"),
            )?,
        })
    }
}

fn load_idem(root: &Path) -> Result<BTreeMap<String, Value>> {
    let path = idem_path(root);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|e| anyhow!("IDEMPOTENCY_STATE_UNREADABLE: {e}"))?;
    serde_json::from_str(&text).map_err(|e| anyhow!("CORRUPTED_IDEMPOTENCY_STATE: {e}"))
}

fn write_idem(root: &Path, all: &BTreeMap<String, Value>) -> Result<()> {
    std::fs::create_dir_all(root.join(".route"))?;
    let path = idem_path(root);
    let bytes = serde_json::to_vec_pretty(all)?;
    route_basic::ownership_lock::atomic_replace(&path, &bytes)
}

fn fingerprint(r: &Request) -> String {
    route_core::sha256_hex(
        serde_json::to_string(
            &json!({"protocol":r.protocol,"method":r.method,"context":r.context,"params":r.params}),
        )
        .unwrap_or_default()
        .as_bytes(),
    )
}
#[derive(Debug)]
enum IdempotencyDecision {
    Proceed,
    Replay(Value),
}

fn preflight_idempotency(root: &Path, r: &Request) -> Result<IdempotencyDecision> {
    let _lock = IdempotencyLock::acquire(root)?;
    let key = r
        .idempotency_key
        .as_ref()
        .ok_or_else(|| anyhow!("idempotency key is required"))?;
    let scoped = format!("{}:{}:{}", r.protocol, r.method, key);
    let mut all = load_idem(root)?;
    if let Some(old) = all.get(&scoped) {
        if old.get("fingerprint").and_then(Value::as_str) != Some(&fingerprint(r)) {
            return Ok(IdempotencyDecision::Replay(error(
                Some(&r.request_id),
                "IDEMPOTENCY_CONFLICT",
                "idempotency key was used with a different semantic request",
                false,
                json!({}),
            )));
        }
        match old.get("status").and_then(Value::as_str) {
            Some("PENDING")
                if matches!(
                    r.method.as_str(),
                    "development.event.record"
                        | "reference.register"
                        | "reference.refresh"
                        | "reference.recover"
                        | "cooperation.register"
                        | "cooperation.refresh"
                        | "cooperation.knowledge.record"
                ) =>
            {
                // This domain has a durable operation key under its write lock.
                // Re-entry either commits once or returns the existing event.
                return Ok(IdempotencyDecision::Proceed);
            }
            Some("PENDING") => {
                return Ok(IdempotencyDecision::Replay(error(
                    Some(&r.request_id),
                    "IDEMPOTENCY_RECOVERY_REQUIRED",
                    "the previous mutation has an uncertain outcome; recovery is required",
                    false,
                    json!({"idempotency_key":key}),
                )))
            }
            Some("COMPLETED") => {
                let mut response = old.get("response").cloned().ok_or_else(|| {
                    anyhow!("CORRUPTED_IDEMPOTENCY_STATE: completed record has no response")
                })?;
                response["request_id"] = json!(r.request_id);
                response["receipt"]["replay"] = json!(true);
                return Ok(IdempotencyDecision::Replay(response));
            }
            Some(other) => {
                return Ok(IdempotencyDecision::Replay(error(
                    Some(&r.request_id),
                    "IDEMPOTENCY_RECOVERY_REQUIRED",
                    format!("idempotency record is in unrecoverable status {other}"),
                    false,
                    json!({"idempotency_key":key}),
                )))
            }
            None => return Err(anyhow!("CORRUPTED_IDEMPOTENCY_STATE: record has no status")),
        }
    }

    // Reserve before touching the domain.  If the process dies after the
    // domain mutation but before the final receipt write, retries see PENDING
    // and cannot blindly execute the operation a second time.
    all.insert(
        scoped,
        json!({"fingerprint":fingerprint(r),"status":"PENDING","created_at":route_core::now_millis()}),
    );
    write_idem(root, &all)?;
    Ok(IdempotencyDecision::Proceed)
}
fn persist_idem(root: &Path, r: &Request, response: &Value) -> Result<()> {
    let Some(key) = r.idempotency_key.as_ref() else {
        return Ok(());
    };
    let _lock = IdempotencyLock::acquire(root)?;
    let scoped = format!("{}:{}:{}", r.protocol, r.method, key);
    let mut all = load_idem(root)?;
    all.insert(scoped, json!({"fingerprint":fingerprint(r),"response":response,"created_at":route_core::now_millis(),"status":"COMPLETED"}));
    write_idem(root, &all)
}

fn is_mutation(method: &str) -> bool {
    matches!(
        method,
        "intent.create"
            | "intent.close"
            | "evidence.record"
            | "checkpoint.create"
            | "development.event.record"
            | "worker.register"
            | "worker.presence.update"
            | "worker.message.send"
            | "reference.register"
            | "reference.refresh"
            | "reference.recover"
            | "cooperation.register"
            | "cooperation.refresh"
            | "cooperation.discovery.record"
            | "cooperation.knowledge.record"
            | "cooperation.capability.record"
    )
}

fn domain_deduplication_key(request: &Request) -> String {
    format!(
        "{}:{}:{}",
        request.protocol,
        request.method,
        request.idempotency_key.as_deref().unwrap_or_default()
    )
}

fn complete_mutation(root: &Path, request: &Request, result: Value, refs: Vec<String>) -> Value {
    let out = success(request, result, refs);
    match persist_idem(root, request, &out) {
        Ok(()) => out,
        Err(_) => error(
            Some(&request.request_id),
            "INTERNAL_ERROR",
            "mutation receipt persistence failed",
            true,
            json!({}),
        ),
    }
}

fn validate_mutation_request(request: &Request) -> Option<Value> {
    let invalid = |message: &str| {
        error(
            Some(&request.request_id),
            "INVALID_PARAMS",
            message,
            false,
            json!({}),
        )
    };
    match request.method.as_str() {
        "intent.create"
            if request
                .params
                .get("objective")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty()) =>
        {
            Some(invalid("objective is required"))
        }
        "intent.close"
            if request
                .params
                .get("intent_ref")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty()) =>
        {
            Some(invalid("intent_ref is required"))
        }
        "reference.register"
            if missing_string(&request.params, "reference_id")
                || missing_string(&request.params, "locator") =>
        {
            Some(invalid("reference_id and locator are required"))
        }
        "reference.refresh" if missing_string(&request.params, "reference_id") => {
            Some(invalid("reference_id is required"))
        }
        "cooperation.register"
            if missing_string(&request.params, "cooperation_id")
                || missing_string(&request.params, "locator") =>
        {
            Some(invalid("cooperation_id and locator are required"))
        }
        "cooperation.refresh" if missing_string(&request.params, "cooperation_id") => {
            Some(invalid("cooperation_id is required"))
        }
        "cooperation.discovery.record"
            if missing_string(&request.params, "discovery_id")
                || missing_string(&request.params, "cooperation_id") =>
        {
            Some(invalid("discovery_id and cooperation_id are required"))
        }
        "cooperation.knowledge.record"
            if missing_string(&request.params, "knowledge_id")
                || missing_string(&request.params, "cooperation_id") =>
        {
            Some(invalid("knowledge_id and cooperation_id are required"))
        }
        "cooperation.capability.record"
            if missing_string(&request.params, "knowledge_id")
                || missing_string(&request.params, "cooperation_id")
                || missing_string(&request.params, "capability") =>
        {
            Some(invalid(
                "knowledge_id, cooperation_id and capability are required",
            ))
        }
        "intent.close"
            if request
                .params
                .get("result")
                .and_then(Value::as_str)
                .is_some_and(|v| !matches!(v, "success" | "failed" | "aborted")) =>
        {
            Some(invalid("result must be success, failed, or aborted"))
        }
        "evidence.record"
            if request
                .params
                .get("intent_ref")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty()) =>
        {
            Some(invalid("intent_ref is required"))
        }
        "evidence.record"
            if request
                .params
                .get("observation")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty()) =>
        {
            Some(invalid("observation is required"))
        }
        "checkpoint.create"
            if request
                .params
                .get("title")
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty()) =>
        {
            Some(invalid("title is required"))
        }
        "worker.presence.update" | "worker.message.send"
            if request
                .params
                .get("worker_id")
                .and_then(Value::as_str)
                .is_none_or(|value| value.trim().is_empty()) =>
        {
            Some(invalid("worker_id is required"))
        }
        _ => None,
    }
}

fn missing_string(value: &Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(Value::as_str)
        .is_none_or(|text| text.trim().is_empty())
}

// Reference/Cooperation adapters remain unavailable until their transport gates pass.
fn dispatch(request: Request) -> Value {
    if request.protocol != PROTOCOL {
        return error(
            Some(&request.request_id),
            "UNSUPPORTED_PROTOCOL",
            "Route does not support the requested protocol",
            false,
            json!({"supported_protocols":[PROTOCOL]}),
        );
    }
    if !request.extra.is_empty() {
        return error(
            Some(&request.request_id),
            "INVALID_REQUEST",
            "unknown top-level fields are not supported in route/1",
            false,
            json!({"fields": request.extra.keys().collect::<Vec<_>>() }),
        );
    }
    if !request.context.is_object() || !request.params.is_object() {
        return error(
            Some(&request.request_id),
            "INVALID_PARAMS",
            "context and params must be objects",
            false,
            json!({}),
        );
    }
    // Reject unimplemented methods before resolving state or reserving a
    // durable idempotency receipt. Capability discovery is a callable contract.
    if !METHODS.contains(&request.method.as_str()) {
        return error(
            Some(&request.request_id),
            "UNKNOWN_METHOD",
            "method is not available in route/1",
            false,
            json!({"available_methods": METHODS}),
        );
    }
    let root = match root_from_context(&request.context) {
        Ok(v) => v,
        Err((code, message)) => {
            return error(Some(&request.request_id), code, message, false, json!({}))
        }
    };
    if is_mutation(&request.method) {
        if let Some(invalid) = validate_mutation_request(&request) {
            return invalid;
        }
        if request.idempotency_key.is_none() {
            return error(
                Some(&request.request_id),
                "INVALID_PARAMS",
                "idempotency_key is required for mutation methods",
                false,
                json!({}),
            );
        }
        match preflight_idempotency(&root, &request) {
            Ok(IdempotencyDecision::Proceed) => {}
            Ok(IdempotencyDecision::Replay(response)) => return response,
            Err(e) => {
                let text = e.to_string();
                let code = if text.starts_with("CORRUPTED_IDEMPOTENCY_STATE") {
                    "CORRUPTED_IDEMPOTENCY_STATE"
                } else if text.starts_with("IDEMPOTENCY_STATE_UNREADABLE") {
                    "IDEMPOTENCY_STATE_UNREADABLE"
                } else {
                    "INTERNAL_ERROR"
                };
                return error(
                    Some(&request.request_id),
                    code,
                    "idempotency state unavailable",
                    false,
                    json!({}),
                );
            }
        }
    }
    invoke_method(request, root)
}

route_methods! { request, root, version;
        "reference.list" => surface_response(&root, &request, | | surface_reference_list(&root, &request)),
        "reference.get" => surface_response(&root, &request, | | surface_reference_get(&root, &request)),
        "reference.register" => surface_response(&root, &request, | | surface_reference_register(&root, &request)),
        "reference.refresh" => surface_response(&root, &request, | | surface_reference_refresh(&root, &request)),
        "reference.recover" => surface_response(&root, &request, | | surface_reference_recover(&root, &request)),
        "cooperation.list" => surface_response(&root, &request, | | surface_cooperation_list(&root, &request)),
        "cooperation.get" => surface_response(&root, &request, | | surface_cooperation_get(&root, &request)),
        "cooperation.register" => surface_response(&root, &request, | | surface_cooperation_register(&root, &request)),
        "cooperation.refresh" => surface_response(&root, &request, | | surface_cooperation_refresh(&root, &request)),
        "cooperation.knowledge.query" => surface_response(&root, &request, | | surface_cooperation_knowledge_query(&root, &request)),
        "cooperation.knowledge.record" => surface_response(&root, &request, | | surface_cooperation_knowledge_record(&root, &request)),
        "system.hello" => {
            let offered = request
                .params
                .get("supported_protocols")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if !offered.is_empty() && !offered.iter().any(|v| v == PROTOCOL) {
                return error(
                    Some(&request.request_id),
                    "UNSUPPORTED_PROTOCOL",
                    "no mutually supported protocol",
                    false,
                    json!({"supported_protocols":[PROTOCOL]}),
                );
            }
            success(
                &request,
                json!({"route_product_version":version,"selected_protocol":PROTOCOL,"route_protocol_versions":[PROTOCOL],"capabilities":METHODS,"feature_flags":{"jsonl_sequential":true,"mutations":true},"runtime":{"transport":"stdio"}}),
                vec![],
            )
        },
        "system.capabilities" => success(
            &request,
            json!({"protocol":PROTOCOL,"capabilities":METHODS,"advertised_methods_callable":true}),
            vec![],
        ),
        "system.status" => success(
            &request,
            match project(&root) {
                Ok(value) => {
                    json!({"route_product_version":version,"protocols":[PROTOCOL],"project":value})
                }
                Err(e) => {
                    return error(
                        Some(&request.request_id),
                        "PROJECT_IDENTITY_UNAVAILABLE",
                        e.to_string(),
                        true,
                        json!({}),
                    )
                }
            },
            vec![],
        ),
        "project.inspect" | "project.status" => match project(&root) {
            Ok(value) => success(&request, value, vec![]),
            Err(e) => error(
                Some(&request.request_id),
                "PROJECT_IDENTITY_UNAVAILABLE",
                e.to_string(),
                true,
                json!({}),
            ),
        },
        "intent.list" => {
            let list = route_basic::session_status(&root, None).unwrap_or_default().into_iter().map(|s| json!({"intent_ref":s.id,"summary":s.task,"status":format!("{:?}",s.status)})).collect::<Vec<_>>();
            success(&request, json!({"intents":list}), vec![])
        },
        "intent.get" => {
            let id = match request.params.get("intent_ref").and_then(Value::as_str) {
                Some(v) => v,
                None => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        "intent_ref is required",
                        false,
                        json!({}),
                    )
                }
            };
            match route_basic::session_status(&root, Some(id))
                .ok()
                .and_then(|mut xs| xs.pop())
            {
                Some(s) => success(
                    &request,
                    json!({"intent_ref":s.id,"summary":s.task,"status":format!("{:?}",s.status)}),
                    vec![id.to_string()],
                ),
                None => error(
                    Some(&request.request_id),
                    "INTENT_NOT_FOUND",
                    "intent_ref does not exist",
                    false,
                    json!({"intent_ref":id}),
                ),
            }
        },
        "intent.create" => {
            let objective = match request.params.get("objective").and_then(Value::as_str) {
                Some(v) if !v.trim().is_empty() => v,
                _ => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        "objective is required",
                        false,
                        json!({}),
                    )
                }
            };
            match route_basic::begin_session(
                &root,
                objective,
                route_basic::ApplyTarget::Generic,
                true,
                None,
            ) {
                Ok(s) => {
                    let out = success(
                        &request,
                        json!({"intent_ref":s.id,"session_ref":s.id,"status":"ACTIVE"}),
                        vec![s.id.clone()],
                    );
                    match persist_idem(&root, &request, &out) {
                        Ok(()) => out,
                        Err(_) => error(
                            Some(&request.request_id),
                            "INTERNAL_ERROR",
                            "mutation receipt persistence failed",
                            true,
                            json!({}),
                        ),
                    }
                }
                Err(e) => error(
                    Some(&request.request_id),
                    "OPERATION_CONFLICT",
                    e.to_string(),
                    true,
                    json!({}),
                ),
            }
        },
        "intent.close" => {
            let id = match request.params.get("intent_ref").and_then(Value::as_str) {
                Some(v) => v,
                _ => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        "intent_ref is required",
                        false,
                        json!({}),
                    )
                }
            };
            let result = request
                .params
                .get("result")
                .and_then(Value::as_str)
                .unwrap_or("aborted");
            match route_basic::end_session(&root, id, result) {
                Ok(s) => {
                    let out = success(
                        &request,
                        json!({"intent_ref":s.id,"status":format!("{:?}",s.status)}),
                        vec![s.id.clone()],
                    );
                    match persist_idem(&root, &request, &out) {
                        Ok(()) => out,
                        Err(_) => error(
                            Some(&request.request_id),
                            "INTERNAL_ERROR",
                            "mutation receipt persistence failed",
                            true,
                            json!({}),
                        ),
                    }
                }
                Err(e) => error(
                    Some(&request.request_id),
                    "INVALID_STATE_TRANSITION",
                    e.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "evidence.record" => {
            let intent = match request.params.get("intent_ref").and_then(Value::as_str) {
                Some(v) => v,
                _ => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        "intent_ref is required",
                        false,
                        json!({}),
                    )
                }
            };
            let observation = match request.params.get("observation").and_then(Value::as_str) {
                Some(v) if !v.trim().is_empty() => v,
                _ => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        "observation is required",
                        false,
                        json!({}),
                    )
                }
            };
            let report = route_basic::HostReport {
                session_id: intent.to_string(),
                agent_roles_used: vec![],
                actions: vec![],
                verification_requested: vec![],
                observations: vec![observation.to_string()],
            };
            match route_basic::ingest_host_report(&root, report) {
                Ok(ids) => {
                    let out = success(
                        &request,
                        json!({"evidence_refs":ids,"classification":"AGENT_FEEDBACK_ONLY","caller_evidence_class_accepted":false}),
                        ids.clone(),
                    );
                    match persist_idem(&root, &request, &out) {
                        Ok(()) => out,
                        Err(_) => error(
                            Some(&request.request_id),
                            "INTERNAL_ERROR",
                            "mutation receipt persistence failed",
                            true,
                            json!({}),
                        ),
                    }
                }
                Err(e) => error(
                    Some(&request.request_id),
                    "INVALID_STATE_TRANSITION",
                    e.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "evidence.query" => {
            let session = request.params.get("intent_ref").and_then(Value::as_str);
            let store = route_basic::EvidenceStore::load(&root).unwrap_or_default();
            let values = store
                .evidence
                .into_iter()
                .filter(|e| session.map(|s| s == e.session_id).unwrap_or(true))
                .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
                .collect::<Vec<_>>();
            success(&request, json!({"evidence":values}), vec![])
        },
        "history.query" => match route_basic::load_route_history(&root) {
            Ok(events) => {
                let values = events
                    .into_iter()
                    .map(|event| {
                        json!({
                            "event_ref": event.hash,
                            "sequence": event.sequence,
                            "operation": event.operation,
                            "success": event.success,
                            "at": event.timestamp,
                        })
                    })
                    .collect::<Vec<_>>();
                success(&request, json!({"events":values}), vec![])
            }
            Err(e) => error(
                Some(&request.request_id),
                "HISTORY_UNAVAILABLE",
                e.to_string(),
                true,
                json!({}),
            ),
        },
        "checkpoint.create" => {
            let title = match request.params.get("title").and_then(Value::as_str) {
                Some(v) if !v.trim().is_empty() => v,
                _ => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        "title is required",
                        false,
                        json!({}),
                    )
                }
            };
            let body = request.params.get("body").and_then(Value::as_str);
            match route_basic::BasicRepository::open(&root)
                .and_then(|repo| repo.checkpoint_create(title, body, Some("route/1")))
            {
                Ok(commit) => {
                    let out = success(
                        &request,
                        json!({"checkpoint_ref":commit.id,"title":title}),
                        vec![commit.id.clone()],
                    );
                    match persist_idem(&root, &request, &out) {
                        Ok(()) => out,
                        Err(_) => error(
                            Some(&request.request_id),
                            "INTERNAL_ERROR",
                            "mutation receipt persistence failed",
                            true,
                            json!({}),
                        ),
                    }
                }
                Err(e) => error(
                    Some(&request.request_id),
                    "NOT_READY",
                    e.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "development.state" => {
            let recent_limit = request
                .params
                .get("recent_limit")
                .and_then(Value::as_u64)
                .unwrap_or(50) as usize;
            let seen_revision = request.params.get("seen_revision").and_then(Value::as_u64);
            match route_basic::shared_development_state(&root, recent_limit) {
                Ok(state) => {
                    let stale = seen_revision
                        .map(|seen| {
                            route_basic::development_view_is_stale(seen, state.global_revision)
                        })
                        .unwrap_or(false);
                    success(
                        &request,
                        json!({"state": state, "seen_revision": seen_revision, "stale": stale}),
                        vec![],
                    )
                }
                Err(error_value) => error(
                    Some(&request.request_id),
                    "DEVELOPMENT_STATE_UNAVAILABLE",
                    error_value.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "development.events.query" => {
            let after_revision = request
                .params
                .get("after_revision")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let limit = request
                .params
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(100) as usize;
            match route_basic::query_development_events(&root, after_revision, limit) {
                Ok(page) => success(&request, json!(page), vec![]),
                Err(error_value) => error(
                    Some(&request.request_id),
                    "DEVELOPMENT_EVENTS_UNAVAILABLE",
                    error_value.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "development.event.record" => {
            let mut draft = match serde_json::from_value::<route_basic::DevelopmentEventDraft>(
                request.params.clone(),
            ) {
                Ok(value) => value,
                Err(error_value) => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        error_value.to_string(),
                        false,
                        json!({}),
                    )
                }
            };
            if matches!(
                &draft.payload,
                route_basic::DevelopmentEventPayload::WorkerRegistered { .. }
                    | route_basic::DevelopmentEventPayload::WorkerMetadataUpdated { .. }
                    | route_basic::DevelopmentEventPayload::WorkerPresenceUpdated { .. }
                    | route_basic::DevelopmentEventPayload::WorkerMessage { .. }
            ) {
                return error(
                    Some(&request.request_id),
                    "INVALID_PARAMS",
                    "worker lifecycle, presence and messages require their dedicated methods",
                    false,
                    json!({}),
                );
            }
            draft.deduplication_key = Some(domain_deduplication_key(&request));
            match route_basic::append_development_event(&root, draft) {
                Ok(appended) => {
                    let event_ref = appended.event.event_id.clone();
                    complete_mutation(&root, &request, json!(appended), vec![event_ref])
                }
                Err(error_value) => error(
                    Some(&request.request_id),
                    "DEVELOPMENT_EVENT_REJECTED",
                    error_value.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "worker.list" => match (
            route_basic::worker_descriptors(&root),
            route_basic::worker_presences(&root),
        ) {
            (Ok(workers), Ok(presences)) => success(
                &request,
                json!({"workers": workers, "presences": presences}),
                vec![],
            ),
            (Err(error_value), _) | (_, Err(error_value)) => error(
                Some(&request.request_id),
                "WORKER_STATE_UNAVAILABLE",
                error_value.to_string(),
                false,
                json!({}),
            ),
        },
        "worker.register" => {
            let worker_id = request
                .params
                .get("worker_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let metadata = match serde_json::from_value::<route_basic::WorkerMetadata>(
                request
                    .params
                    .get("metadata")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            ) {
                Ok(value) => value,
                Err(error_value) => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        error_value.to_string(),
                        false,
                        json!({}),
                    )
                }
            };
            match route_basic::register_worker(
                &root,
                worker_id,
                metadata,
                Some(domain_deduplication_key(&request)),
            ) {
                Ok(appended) => {
                    let worker_ref = appended.event.actor_worker_id.clone().unwrap_or_default();
                    complete_mutation(&root, &request, json!(appended), vec![worker_ref])
                }
                Err(error_value) => error(
                    Some(&request.request_id),
                    "WORKER_REGISTRATION_REJECTED",
                    error_value.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "worker.presence.update" => {
            let worker_id = request.params["worker_id"].as_str().unwrap_or_default();
            let mut value = request.params.clone();
            value
                .as_object_mut()
                .map(|object| object.remove("worker_id"));
            let presence = match serde_json::from_value::<route_basic::WorkerPresenceInput>(value) {
                Ok(value) => value,
                Err(error_value) => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        error_value.to_string(),
                        false,
                        json!({}),
                    )
                }
            };
            match route_basic::update_worker_presence(
                &root,
                worker_id,
                presence,
                Some(domain_deduplication_key(&request)),
            ) {
                Ok(appended) => complete_mutation(
                    &root,
                    &request,
                    json!(appended),
                    vec![worker_id.to_string()],
                ),
                Err(error_value) => error(
                    Some(&request.request_id),
                    "WORKER_PRESENCE_REJECTED",
                    error_value.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "worker.message.send" => {
            let worker_id = request.params["worker_id"].as_str().unwrap_or_default();
            let mut value = request.params.clone();
            value
                .as_object_mut()
                .map(|object| object.remove("worker_id"));
            let message = match serde_json::from_value::<route_basic::WorkerMessageInput>(value) {
                Ok(value) => value,
                Err(error_value) => {
                    return error(
                        Some(&request.request_id),
                        "INVALID_PARAMS",
                        error_value.to_string(),
                        false,
                        json!({}),
                    )
                }
            };
            match route_basic::send_worker_message(
                &root,
                worker_id,
                message,
                Some(domain_deduplication_key(&request)),
            ) {
                Ok(appended) => {
                    let event_ref = appended.event.event_id.clone();
                    complete_mutation(&root, &request, json!(appended), vec![event_ref])
                }
                Err(error_value) => error(
                    Some(&request.request_id),
                    "WORKER_MESSAGE_REJECTED",
                    error_value.to_string(),
                    false,
                    json!({}),
                ),
            }
        },
        "recovery.status" => success(
            &request,
            json!({"route_state_present":root.join(".route").exists(),"recovery_action_available":false,"status":"READ_ONLY"}),
            vec![],
        ),
}

fn surface_response(
    root: &Path,
    request: &Request,
    operation: impl FnOnce() -> Result<Value>,
) -> Value {
    match operation() {
        Ok(value) if is_mutation(&request.method) => {
            complete_mutation(root, request, value, vec![])
        }
        Ok(value) => success(request, value, vec![]),
        Err(e) => {
            let message = format!("{e:#}");
            let lower = message.to_ascii_lowercase();
            let code = if message.contains("REFERENCE_RECOVERY_REQUIRED") {
                "REFERENCE_RECOVERY_REQUIRED"
            } else if message.contains("SECRET_METADATA_REJECTED") {
                "SECRET_METADATA_REJECTED"
            } else if lower.contains("evidence") {
                "EVIDENCE_REQUIRED"
            } else if lower.contains("stale") || lower.contains("resource_changed") {
                "RESOURCE_CHANGED"
            } else if lower.contains("parsing")
                || lower.contains("corrupt")
                || lower.contains("hash chain")
            {
                "CORRUPTED_STATE"
            } else {
                "DOMAIN_REJECTED"
            };
            let next = match code {
                "EVIDENCE_REQUIRED" => " Supply successful System TestPass/CheckPass Evidence bound to project_id, cooperation_id and resource_fingerprint; declarations are not Evidence.",
                "RESOURCE_CHANGED" => " Refresh the resource, inspect stale knowledge, then record a revalidated replacement.",
                _ => "",
            };
            error(
                Some(&request.request_id),
                code,
                format!("{message}.{next} No automatic reset was performed."),
                false,
                json!({}),
            )
        }
    }
}
fn text_param(r: &Request, name: &str) -> String {
    r.params[name].as_str().unwrap_or_default().to_owned()
}
fn optional_param(r: &Request, name: &str) -> Option<String> {
    r.params[name].as_str().map(str::to_owned)
}
fn strings_param(r: &Request, name: &str) -> Result<Vec<String>> {
    Ok(serde_json::from_value(
        r.params.get(name).cloned().unwrap_or_else(|| json!([])),
    )?)
}
fn surface_reference_list(root: &Path, _: &Request) -> Result<Value> {
    Ok(json!(route_basic::ReferenceRegistry::read(root)?.entries))
}
fn surface_reference_get(root: &Path, r: &Request) -> Result<Value> {
    let registry = route_basic::ReferenceRegistry::read(root)?;
    Ok(json!(registry
        .get(&text_param(r, "reference_id"))
        .ok_or_else(|| anyhow!(
            "reference not found; use route reference list"
        ))?))
}
fn surface_reference_recover(root: &Path, _: &Request) -> Result<Value> {
    route_basic::ReferenceRegistry::recover(root)?;
    Ok(json!({"recovered":true,"reset_performed":false}))
}
fn surface_reference_register(root: &Path, r: &Request) -> Result<Value> {
    reference_mutation(root, r, false)
}
fn surface_reference_refresh(root: &Path, r: &Request) -> Result<Value> {
    reference_mutation(root, r, true)
}
fn reference_mutation(root: &Path, r: &Request, refresh: bool) -> Result<Value> {
    use route_basic::{ReferenceEntry, ReferenceRegistry, ReferenceType};
    let operation_id = route_core::sha256_hex(domain_deduplication_key(r).as_bytes());
    // A journal-backed operation may have completed before its transport receipt.
    // Reconcile first and return the original event revision, never upsert twice.
    ReferenceRegistry::recover(root)?;
    if let Some(revision) = ReferenceRegistry::operation_revision(root, &operation_id)? {
        return Ok(
            json!({"reference_id":text_param(r,"reference_id"),"operation_id":operation_id,"global_revision":revision}),
        );
    }
    let id = text_param(r, "reference_id");
    let mut registry = ReferenceRegistry::read(root)?;
    let mut entry = if refresh {
        registry
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("reference not found; use route reference list"))?
    } else {
        anyhow::ensure!(
            registry.get(&id).is_none(),
            "reference already exists; refresh it or choose another id"
        );
        ReferenceEntry::builder(
            &id,
            ReferenceType::Other,
            text_param(r, "locator"),
            text_param(r, "description"),
        )
        .build()
    };
    route_basic::cooperation::validate_public_metadata(&id)?;
    route_basic::cooperation::validate_public_metadata(&entry.description)?;
    let (locator, availability, fingerprint, _) = route_basic::cooperation::inspect_locator(
        root,
        &entry.source,
        &route_basic::CooperationKind("UNKNOWN".into()),
    )?;
    entry.source = locator;
    entry.availability = serde_json::from_value(json!(availability))?;
    entry.fingerprint = fingerprint.map(|v| v.value);
    registry.upsert(entry);
    registry.write_with_operation_id(root, &operation_id)?;
    Ok(
        json!({"reference_id":id,"operation_id":operation_id,"global_revision":ReferenceRegistry::operation_revision(root,&operation_id)?}),
    )
}
fn surface_cooperation_list(root: &Path, _: &Request) -> Result<Value> {
    Ok(json!(route_basic::cooperation_resources(root)?))
}
fn surface_cooperation_get(root: &Path, r: &Request) -> Result<Value> {
    Ok(json!(route_basic::cooperation_resource(
        root,
        &text_param(r, "cooperation_id")
    )?
    .ok_or_else(|| anyhow!(
        "cooperation resource not found; use route cooperation list"
    ))?))
}
fn surface_cooperation_register(root: &Path, r: &Request) -> Result<Value> {
    Ok(json!(route_basic::register_cooperation_resource(
        root,
        text_param(r, "cooperation_id"),
        text_param(r, "locator"),
        route_basic::CooperationKind(optional_param(r, "kind").unwrap_or_else(|| "UNKNOWN".into())),
        text_param(r, "provenance"),
        optional_param(r, "description"),
        strings_param(r, "declared_capabilities")?,
        strings_param(r, "related_reference_ids")?,
        strings_param(r, "related_constraint_ids")?,
        optional_param(r, "actor_worker_id"),
        Some(domain_deduplication_key(r))
    )?))
}
fn surface_cooperation_refresh(root: &Path, r: &Request) -> Result<Value> {
    Ok(json!(route_basic::refresh_cooperation_resource(
        root,
        &text_param(r, "cooperation_id"),
        optional_param(r, "actor_worker_id"),
        Some(domain_deduplication_key(r))
    )?))
}
fn surface_cooperation_knowledge_query(root: &Path, r: &Request) -> Result<Value> {
    Ok(json!(route_basic::cooperation_knowledge(
        root,
        r.params["cooperation_id"].as_str()
    )?))
}
fn surface_cooperation_knowledge_record(root: &Path, r: &Request) -> Result<Value> {
    let mut params = r.params.clone();
    if let Some(object) = params.as_object_mut() {
        object.remove("actor_worker_id");
    }
    if params.get("observed_at").is_none() {
        params["observed_at"] = json!(0);
    }
    let record = serde_json::from_value::<route_basic::CooperationKnowledgeRecord>(params)?;
    Ok(json!(route_basic::record_cooperation_knowledge(
        root,
        record,
        optional_param(r, "actor_worker_id"),
        Some(domain_deduplication_key(r))
    )?))
}

/// Human CLI commands use precisely the same transport and domain path.
pub fn invoke_local(method: &str, params: Value, key: Option<String>) -> Result<()> {
    let response = handle(
        &json!({"protocol":PROTOCOL,"request_id":"cli","method":method,
        "context":{},"params":params,"idempotency_key":key})
        .to_string(),
    );
    if response["ok"] != true {
        return Err(anyhow!("{}", response["error"]));
    }
    println!("{}", serde_json::to_string_pretty(&response["result"])?);
    Ok(())
}

fn handle(raw: &str) -> Value {
    match serde_json::from_str::<Request>(raw) {
        Ok(r)
            if !r.request_id.trim().is_empty()
                && !r.method.trim().is_empty()
                && !r.protocol.trim().is_empty() =>
        {
            dispatch(r)
        }
        Ok(_) => error(
            None,
            "INVALID_REQUEST",
            "protocol, request_id and method are required",
            false,
            json!({}),
        ),
        Err(_) => error(
            None,
            "INVALID_REQUEST",
            "malformed JSON request",
            false,
            json!({}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn request(method: &str, params: Value) -> String {
        json!({"protocol":"route/1","request_id":"test-request","method":method,
               "context":{},"params":params,"idempotency_key":null})
        .to_string()
    }

    #[test]
    fn g01_hello_negotiates_route_one_for_generic_host() {
        let v = handle(&request(
            "system.hello",
            json!({"client_name":"test-host","supported_protocols":["route/1"]}),
        ));
        assert_eq!(v["ok"], true);
        assert_eq!(v["result"]["selected_protocol"], "route/1");
        assert_eq!(v["request_id"], "test-request");
    }

    #[test]
    fn golden_errors_are_structured_and_no_traceback() {
        for raw in ["{", &request("unknown.method", json!({})), &json!({"protocol":"route/9","request_id":"bad","method":"system.hello","context":{},"params":{}}).to_string()] {
            let v = handle(raw);
            assert_eq!(v["ok"], false);
            assert!(v["error"]["code"].is_string());
            assert!(!v.to_string().to_lowercase().contains("traceback"));
        }
    }

    #[test]
    fn advertised_methods_have_dispatchable_semantics() {
        assert!(METHODS.contains(&"system.hello"));
        assert!(METHODS.contains(&"intent.create"));
        assert!(METHODS.contains(&"evidence.record"));
        assert!(!METHODS.iter().any(|m| m.contains("yuich")));
    }

    #[test]
    fn unfinished_substrate_methods_are_not_advertised_or_persisted() {
        let dir = tempdir().unwrap();
        for method in [
            "cooperation.discovery.record",
            "cooperation.capability.record",
            "constraint.list",
        ] {
            assert!(!METHODS.contains(&method));
            let raw = json!({"protocol":"route/1", "request_id":"unsupported",
                "method":method, "context":{"root_locator":dir.path()},
                "params":{}, "idempotency_key":"must-not-reserve"})
            .to_string();
            let response = handle(&raw);
            assert_eq!(response["error"]["code"], "UNKNOWN_METHOD");
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn explicit_project_conflict_fails_closed() {
        let bad = std::env::temp_dir().join("route-rpc-no-such-project");
        let raw = json!({"protocol":"route/1","request_id":"conflict","method":"project.inspect",
            "context":{"root_locator":bad},"params":{}})
        .to_string();
        let v = handle(&raw);
        assert_eq!(v["ok"], false);
        assert!(matches!(
            v["error"]["code"].as_str(),
            Some("PROJECT_NOT_FOUND") | Some("PROJECT_IDENTITY_CONFLICT")
        ));
    }

    #[test]
    fn explicit_project_and_workspace_identity_match_persisted_identity() {
        let dir = tempdir().unwrap();
        let identity = route_basic::ensure_identity(dir.path()).unwrap();
        let context = json!({
            "project_id": identity.project_id,
            "workspace_id": identity.workspace_id,
        });

        assert!(validate_context_identity(dir.path(), &context).is_ok());
    }

    #[test]
    fn explicit_project_or_workspace_identity_conflict_fails_closed() {
        let dir = tempdir().unwrap();
        let identity = route_basic::ensure_identity(dir.path()).unwrap();
        for context in [
            json!({"project_id":"different-project"}),
            json!({"workspace_id":"different-workspace"}),
            json!({"project_id":identity.project_id,"workspace_id":42}),
        ] {
            let error = validate_context_identity(dir.path(), &context).unwrap_err();
            assert_eq!(error.0, "PROJECT_IDENTITY_CONFLICT");
        }
    }

    fn mutation_request(method: &str, key: &str, params: Value) -> Request {
        Request {
            protocol: PROTOCOL.to_string(),
            request_id: "reservation-test".to_string(),
            method: method.to_string(),
            context: json!({}),
            params,
            idempotency_key: Some(key.to_string()),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn idempotency_reserves_before_mutation_and_replays_durable_receipt() {
        let dir = tempdir().unwrap();
        let req = mutation_request("intent.create", "reservation-key", json!({"objective":"x"}));
        assert!(matches!(
            preflight_idempotency(dir.path(), &req).unwrap(),
            IdempotencyDecision::Proceed
        ));
        let pending = load_idem(dir.path()).unwrap();
        assert_eq!(pending.values().next().unwrap()["status"], "PENDING");

        let response = success(&req, json!({"intent_ref":"i1"}), vec!["i1".into()]);
        persist_idem(dir.path(), &req, &response).unwrap();
        match preflight_idempotency(dir.path(), &req).unwrap() {
            IdempotencyDecision::Replay(v) => {
                assert_eq!(v["receipt"]["receipt_id"], "rpc:reservation-test");
                assert_eq!(v["receipt"]["replay"], true);
            }
            IdempotencyDecision::Proceed => panic!("completed key must replay"),
        }
    }

    #[test]
    fn corrupt_idempotency_state_fails_closed() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".route")).unwrap();
        std::fs::write(idem_path(dir.path()), b"{broken").unwrap();
        let req = mutation_request("intent.create", "corrupt-key", json!({"objective":"x"}));
        let err = preflight_idempotency(dir.path(), &req)
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("CORRUPTED_IDEMPOTENCY_STATE"));
    }

    #[test]
    fn pending_retries_are_recovery_required_not_reexecution() {
        let dir = tempdir().unwrap();
        let req = mutation_request("intent.create", "pending-key", json!({"objective":"x"}));
        assert!(matches!(
            preflight_idempotency(dir.path(), &req).unwrap(),
            IdempotencyDecision::Proceed
        ));
        match preflight_idempotency(dir.path(), &req).unwrap() {
            IdempotencyDecision::Replay(v) => {
                assert_eq!(v["error"]["code"], "IDEMPOTENCY_RECOVERY_REQUIRED")
            }
            IdempotencyDecision::Proceed => panic!("pending key must not reexecute"),
        }
    }

    #[test]
    fn receipt_schema_contains_stable_operation_fields() {
        let req = mutation_request("intent.create", "receipt-key", json!({"objective":"x"}));
        let v = success(&req, json!({"intent_ref":"i1"}), vec!["i1".into()]);
        assert_eq!(v["protocol"], PROTOCOL);
        assert_eq!(v["request_id"], "reservation-test");
        assert_eq!(v["receipt"]["method"], "intent.create");
        assert_eq!(v["receipt"]["operation_status"], "COMPLETED");
        assert_eq!(v["receipt"]["affected_object_refs"][0], "i1");
        assert_eq!(v["receipt"]["idempotency_key"], "receipt-key");
    }
}

pub fn serve(jsonl: bool) -> Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if jsonl {
        for line in io::stdin().lock().lines() {
            let line = line.unwrap_or_default();
            if line.trim().is_empty() {
                continue;
            }
            writeln!(out, "{}", serde_json::to_string(&handle(&line))?)?;
            out.flush()?;
        }
    } else {
        let mut raw = String::new();
        io::stdin().read_to_string(&mut raw)?;
        writeln!(out, "{}", serde_json::to_string(&handle(&raw))?)?;
    }
    Ok(())
}
