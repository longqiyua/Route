use anyhow::{ensure, Context, Result};
use clap::Subcommand;
use serde_json::json;
use std::{fs, io::Write, path::PathBuf};
#[derive(Subcommand)]
pub enum Action {
    List,
    Issue {
        worker_id: String,
        #[arg(long)]
        credential_file: PathBuf,
        #[arg(long)]
        operation_key: String,
        #[arg(long)]
        grant: Vec<String>,
    },
    Revoke {
        binding_id: String,
        #[arg(long)]
        operation_key: String,
    },
}
pub fn run(action: Action) -> Result<()> {
    use route_cli::rpc::invoke_local;
    match action {
        Action::List => invoke_local("worker.binding.list", json!({}), None),
        Action::Revoke {
            binding_id,
            operation_key,
        } => invoke_local(
            "worker.binding.revoke",
            json!({"binding_id":binding_id}),
            Some(operation_key),
        ),
        Action::Issue {
            worker_id,
            credential_file,
            operation_key,
            grant,
        } => {
            let root = route_basic::discover_project_root(&std::env::current_dir()?)
                .context("run route init first")?
                .canonicalize()?;
            let parent = credential_file
                .parent()
                .context("credential file needs an existing external parent")?
                .canonicalize()?;
            ensure!(
                !parent.starts_with(&root),
                "credential file must remain outside the shared project"
            );
            let file = parent.join(
                credential_file
                    .file_name()
                    .context("missing credential filename")?,
            );
            let credential = if file.exists() {
                ensure!(
                    fs::symlink_metadata(&file)?.is_file()
                        && !fs::symlink_metadata(&file)?.file_type().is_symlink(),
                    "credential path must be a plain file"
                );
                ensure!(
                    fs::metadata(&file)?.len() == 64,
                    "existing credential file has invalid length; not overwritten"
                );
                fs::read_to_string(&file)?
            } else {
                let secret = route_basic::principal::generate_credential()?;
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut output = options.open(&file)?;
                output.write_all(secret.as_bytes())?;
                output.sync_all()?;
                secret
            };
            ensure!(
                credential.len() == 64 && credential.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid credential file; not overwritten"
            );
            let grants = if grant.is_empty() {
                vec![
                    "work.create_child".to_string(),
                    "work.claim".to_string(),
                    "work.release".to_string(),
                    "work.interrupt".to_string(),
                    "work.finish".to_string(),
                    "worker.message.send".to_string(),
                    "worker.presence.update".to_string(),
                    "development.event.record".to_string(),
                ]
            } else {
                grant
            };
            // Host keeps the file even after an uncertain response, so the same key/hash can be retried.
            invoke_local(
                "worker.binding.issue",
                json!({"worker_id":worker_id,"credential_hash":route_core::sha256_hex(credential.as_bytes()),"grants":grants}),
                Some(operation_key),
            )
        }
    }
}
