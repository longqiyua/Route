use anyhow::Result;
use clap::Subcommand;
use route_cli::rpc::invoke_local;
use serde_json::json;

#[derive(Subcommand)]
pub enum Action {
    List,
    Inspect {
        source: String,
    },
    Show {
        id: String,
        version: String,
    },
    Bindings,
    Register {
        source: String,
        #[arg(long)]
        operation_key: String,
    },
    Activate {
        id: String,
        version: String,
        /// Explicit operator grant; package requests are never grants.
        #[arg(long)]
        grant: Vec<String>,
        #[arg(long, default_value_t = 0)]
        order: i32,
        #[arg(long)]
        activated_by: String,
        /// Compare-and-swap revision from institution bindings, or zero for first activation.
        #[arg(long)]
        expected_binding_revision: u64,
        #[arg(long)]
        operation_key: String,
    },
    Deactivate {
        id: String,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        expected_binding_revision: u64,
        #[arg(long)]
        operation_key: String,
    },
    Invoke {
        event: String,
        #[arg(long, default_value = "ON_EVENT")]
        hook: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        worker: Option<String>,
        #[arg(long)]
        correlation_id: String,
        #[arg(long)]
        operation_key: String,
    },
    Replay {
        id: String,
        version: String,
        #[arg(long, default_value_t = 0)]
        after: u64,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
}
fn project_id() -> Result<String> {
    let cwd = std::env::current_dir()?;
    let root = route_basic::discover_project_root(&cwd).unwrap_or(cwd);
    Ok(route_basic::load_identity(&root)?
        .ok_or_else(|| anyhow::anyhow!("run route init first"))?
        .project_id)
}
pub fn run(action: Action) -> Result<()> {
    match action {
        Action::List => invoke_local("institution.list", json!({}), None),
        Action::Inspect { source } => invoke_local(
            "institution.inspect",
            json!({"source_locator":source}),
            None,
        ),
        Action::Show { id, version } => invoke_local(
            "institution.get",
            json!({"institution_id":id,"version":version}),
            None,
        ),
        Action::Bindings => invoke_local("institution.bindings", json!({}), None),
        Action::Register {
            source,
            operation_key,
        } => invoke_local(
            "institution.register",
            json!({"source_locator":source}),
            Some(operation_key),
        ),
        Action::Activate {
            id,
            version,
            grant,
            order,
            activated_by,
            expected_binding_revision,
            operation_key,
        } => invoke_local(
            "institution.activate",
            json!({"institution_id":id,"version":version,"project_id":project_id()?,"grants":grant,"order":order,"activated_by":activated_by,"expected_binding_revision":expected_binding_revision}),
            Some(operation_key),
        ),
        Action::Deactivate {
            id,
            actor,
            expected_binding_revision,
            operation_key,
        } => invoke_local(
            "institution.deactivate",
            json!({"institution_id":id,"project_id":project_id()?,"actor":actor,"expected_binding_revision":expected_binding_revision}),
            Some(operation_key),
        ),
        Action::Invoke {
            event,
            hook,
            expected_revision,
            worker,
            correlation_id,
            operation_key,
        } => invoke_local(
            "institution.invoke",
            json!({"hook":hook,"triggering_event_ref":event,"expected_revision":expected_revision,"requesting_actor":worker,"correlation_id":correlation_id}),
            Some(operation_key),
        ),
        Action::Replay {
            id,
            version,
            after,
            limit,
        } => invoke_local(
            "institution.replay",
            json!({"institution_id":id,"version":version,"after_revision":after,"limit":limit}),
            None,
        ),
    }
}
