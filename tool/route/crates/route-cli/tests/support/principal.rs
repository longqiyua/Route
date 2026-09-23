//! Trusted fixture host: real operator binding RPC, opaque credentials held only in memory.
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
};
pub fn command(root: &Path, method: &str, params: &Value) -> Command {
    let worker = match method {
        "worker.message.send" | "worker.presence.update" => params["worker_id"].as_str(),
        "development.event.record" | "cooperation.knowledge.record" => {
            params["actor_worker_id"].as_str()
        }
        _ => None,
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_route"));
    command
        .args(["rpc", "--operator"])
        .env_remove("ROUTE_WORKER_CREDENTIAL");
    if let Some(worker) = worker {
        static TOKENS: OnceLock<Mutex<BTreeMap<(String, String), String>>> = OnceLock::new();
        let mut tokens = TOKENS.get_or_init(Default::default).lock().unwrap();
        let key = (root.to_string_lossy().into_owned(), worker.to_string());
        let credential=tokens.entry(key).or_insert_with(|| {
            let secret=route_basic::principal::generate_credential().unwrap();
            let mut issue=Command::new(env!("CARGO_BIN_EXE_route")).args(["rpc","--operator"]).env_remove("ROUTE_WORKER_CREDENTIAL").current_dir(root).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
            issue.stdin.take().unwrap().write_all(json!({"protocol":"route/1","request_id":"fixture-bind","method":"worker.binding.issue","context":{},"params":{"worker_id":worker,"credential_hash":route_core::sha256_hex(secret.as_bytes()),"grants":["worker.message.send","worker.presence.update","development.event.record","cooperation.knowledge.record"]},"idempotency_key":format!("fixture-binding-{worker}")}).to_string().as_bytes()).unwrap();
            let output=issue.wait_with_output().unwrap();let response:Value=serde_json::from_slice(&output.stdout).unwrap();assert_eq!(response["ok"],true,"fixture binding failed: {response}");secret
        });
        command.env("ROUTE_WORKER_CREDENTIAL", credential);
    }
    command
}
