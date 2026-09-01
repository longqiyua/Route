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
use std::thread;
use std::time::{Duration, SystemTime};

const PROTOCOL: &str = "route/1";
const METHODS: &[&str] = &[
    "system.hello",
    "system.capabilities",
    "system.status",
    "project.inspect",
    "project.status",
    "intent.create",
    "intent.get",
    "intent.list",
    "intent.close",
    "evidence.record",
    "evidence.query",
    "history.query",
    "checkpoint.create",
    "recovery.status",
    "development.state",
    "development.events.query",
    "development.event.record",
    "worker.list",
    "worker.register",
    "worker.presence.update",
    "worker.message.send",
];

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
    Ok(root)
}

fn project(root: &Path) -> Result<Value> {
    let route_dir = root.join(".route");
    let git = root.join(".git");
    let identity = route_basic::ensure_identity(root)?;
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
    path: PathBuf,
}

impl IdempotencyLock {
    fn acquire(root: &Path) -> Result<Self> {
        let path = root.join(".route").join(".rpc-idempotency-lock");
        fs::create_dir_all(root.join(".route"))?;
        for _ in 0..500 {
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .is_some_and(|age| age > Duration::from_secs(30));
                    if stale {
                        let _ = fs::remove_dir(&path);
                    } else {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(anyhow!("idempotency state lock timed out"))
    }
}

impl Drop for IdempotencyLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
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
    // A per-process unique temporary name prevents a stale temp file from
    // being mistaken for the current transaction after a crash.
    let temp = root
        .join(".route")
        .join(format!("rpc-idempotency.json.tmp.{}", std::process::id()));
    std::fs::write(&temp, &bytes)?;
    match std::fs::rename(&temp, &path) {
        Ok(()) => Ok(()),
        Err(first) if path.exists() => {
            // Windows does not replace an existing destination with rename.
            // Remove only this exact state file, then complete the replace.
            std::fs::remove_file(&path)?;
            std::fs::rename(&temp, &path).map_err(|_| first.into())
        }
        Err(e) => Err(e.into()),
    }
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
    let version = env!("CARGO_PKG_VERSION");
    match request.method.as_str() {
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
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
        }
        "recovery.status" => success(
            &request,
            json!({"route_state_present":root.join(".route").exists(),"recovery_action_available":false,"status":"READ_ONLY"}),
            vec![],
        ),
        _ => error(
            Some(&request.request_id),
            "UNKNOWN_METHOD",
            "method is not available in route/1",
            false,
            json!({"available_methods":METHODS}),
        ),
    }
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
