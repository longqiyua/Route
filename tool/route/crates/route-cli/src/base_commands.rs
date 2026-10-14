//! Trusted-local, explicitly bounded Base surfaces. No AI launch or second store.
use anyhow::{ensure, Context, Result};
use clap::Subcommand;
use serde_json::{json, Value};

#[derive(Subcommand)]
pub enum Action {
    /// Exact component lookup. Returns at most three historical units, never Evidence.
    Query {
        component: String,
        #[arg(long = "environment")]
        environment: Vec<String>,
        #[arg(long)]
        independent: bool,
        #[arg(long)]
        json: bool,
    },
    /// Inspect a current unit, including non-applicable/stale candidates.
    Show { knowledge_id: String },
    /// Bounded candidate/review queue projection (at most three).
    Candidates {},
    /// Read deterministic high-signal extraction input; no synthesis, no writes.
    Packet { goal_id: String },
    /// Queue optional separate extraction; unavailable/busy means DEFERRED, not Goal failure.
    Defer {
        goal_id: String,
        job_id: String,
        #[arg(long)]
        operation_key: String,
    },
    /// Finish an optional job after manual extraction, timeout or unavailable Worker; never validates Knowledge.
    Finish {
        goal_id: String,
        job_id: String,
        #[arg(long)]
        succeeded: bool,
        #[arg(long)]
        operation_key: String,
    },
    /// Import an explicit fixture/manual CooperationKnowledge record; all canonical gates apply.
    Submit {
        path: std::path::PathBuf,
        #[arg(long)]
        operation_key: String,
    },
}
pub fn run(action: Action) -> Result<()> {
    let root = route_basic::project_identity::discover_project_root(&std::env::current_dir()?)
        .context("Route project not found")?;
    let output: Value = match action {
        Action::Query {
            component,
            environment,
            independent,
            ..
        } => json!(route_basic::positive_base::query(
            &root,
            &component,
            &environment,
            independent,
            false
        )?),
        Action::Candidates {} => json!(route_basic::positive_base::query(
            &root,
            "",
            &[],
            false,
            true
        )?),
        Action::Show { knowledge_id } => {
            // Explicit ID lookup is not limited by the three-unit search window.
            let unit = route_basic::cooperation::cooperation_knowledge(&root, None)?
                .into_iter()
                .find(|u| {
                    u.record.knowledge_id == knowledge_id
                        && u.record
                            .lesson
                            .as_ref()
                            .is_some_and(|l| l.positive.is_some())
                })
                .context("Unknown Base unit")?;
            json!({"unit":unit,"trust":"HISTORICAL_KNOWLEDGE_NOT_CURRENT_EVIDENCE"})
        }
        Action::Packet { goal_id } => json!(route_basic::positive_base::packets(&root, &goal_id)?),
        Action::Defer {
            goal_id,
            job_id,
            operation_key,
        } => route_basic::positive_base::defer(&root, &goal_id, &job_id, &operation_key)?,
        Action::Submit {
            path,
            operation_key,
        } => {
            use std::io::Read;
            let mut data = Vec::new();
            std::fs::File::open(path)?
                .take(16_385)
                .read_to_end(&mut data)?;
            ensure!(data.len() <= 16_384, "BASE_IMPORT_TOO_LARGE");
            json!(route_basic::positive_base::submit(
                &root,
                serde_json::from_slice(&data)?,
                &operation_key
            )?)
        }
        Action::Finish {
            goal_id,
            job_id,
            succeeded,
            operation_key,
        } => json!(route_basic::team::broker_action(
            &root,
            &route_basic::principal::CallerContext::trusted_operator(),
            route_basic::team::TeamAction::FinishExtraction {
                goal_id,
                job_id,
                succeeded
            },
            &operation_key
        )?),
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_import_and_readonly_actions_are_explicit() {
        use clap::Parser;
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            action: Action,
        }
        assert!(Cli::try_parse_from([
            "base",
            "query",
            "src/recovery.rs",
            "--environment",
            "Windows",
            "--independent",
            "--json"
        ])
        .is_ok());
        assert!(Cli::try_parse_from(["base", "submit", "candidate.json"]).is_err());
        assert!(
            Cli::try_parse_from(["base", "defer", "goal", "job", "--operation-key", "once"])
                .is_ok()
        );
    }
}
