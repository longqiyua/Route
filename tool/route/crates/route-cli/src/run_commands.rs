//! Explicit trusted-local process control. Never exposed through Worker RPC.
use anyhow::{Context, Result};
use clap::Subcommand;
use route_basic::process_supervision::Supervisor;

#[derive(Subcommand)]
pub enum Action {
    /// Read local process ownership, uncertainty and safe next action; no repair.
    Inspect { run_id: String },
    /// Cancel only a proven owned Windows Job, with a durable operation key.
    Cancel {
        run_id: String,
        #[arg(long)]
        operation_key: String,
    },
    /// Record an inspection-first recovery decision; never relaunch or invent Evidence.
    Recover {
        run_id: String,
        #[arg(long)]
        operation_key: String,
        /// Explicitly release quarantined capacity only after tree termination is proven.
        #[arg(long)]
        ack_lost_result: bool,
    },
}
pub fn run(action: Action) -> Result<()> {
    let root = route_basic::project_identity::discover_project_root(&std::env::current_dir()?)
        .context("Route project not found")?;
    let supervisor = Supervisor::open(&root)?;
    let inspection = match action {
        Action::Inspect { run_id } => supervisor.inspect(&run_id)?,
        Action::Cancel {
            run_id,
            operation_key,
        } => supervisor.cancel(&run_id, &operation_key)?,
        Action::Recover {
            run_id,
            operation_key,
            ack_lost_result,
        } => {
            if ack_lost_result {
                supervisor.acknowledge_lost_result(&run_id, &operation_key)?
            } else {
                supervisor.recover(&run_id, &operation_key)?
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&inspection)?);
    Ok(())
}
