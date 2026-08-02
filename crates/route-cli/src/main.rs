//! Route CLI — command-line interface for basic mode.

mod commands;
mod plugin_commands;
mod sync_commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "route",
    version,
    about = "Route — lightweight version manager for Web Coding (basic mode)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a Route basic repository
    Init {
        /// Initialize in a specific path (defaults to current dir)
        #[arg(long)]
        path: Option<String>,
    },
    /// Show repository status
    Status,
    /// Commit current project state as a new snapshot
    Commit {
        /// Commit message (required)
        #[arg(short, long)]
        message: String,
        /// Author (optional)
        #[arg(short, long)]
        author: Option<String>,
        /// Force full snapshot (records kind=full self-loop edge)
        #[arg(long)]
        full: bool,
        /// Branch to commit on (defaults to current)
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Show commit history
    Log {
        /// Max entries
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Filter by branch
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Rollback to a snapshot
    Rollback {
        /// Snapshot ID (or short prefix)
        snapshot_id: String,
        /// Reason for rollback
        #[arg(short, long)]
        reason: Option<String>,
    },
    /// Full backup current HEAD to an external folder
    Backup {
        /// Target directory
        target: String,
    },
    /// Manage branches
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },
    /// Annotate a commit edge with path text (N-N)
    Annotate {
        /// Commit ID
        commit_id: String,
        /// Annotation text
        text: String,
    },
    /// List annotations on a commit
    Annotations {
        /// Commit ID
        commit_id: String,
    },
    /// Export repository data
    Export {
        /// Format: json | markdown | mermaid | emacs
        #[arg(short, long)]
        format: String,
        /// Output file path (defaults to stdout)
        #[arg(short, long)]
        out: Option<String>,
    },
    /// Show repository statistics
    Stats,
    /// Generate a detailed stats report (route-stats integration)
    StatsReport {
        /// Output format: json | markdown
        #[arg(short, long, default_value = "markdown")]
        format: String,
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        out: Option<String>,
        /// Top files limit
        #[arg(long, default_value = "20")]
        top: usize,
        /// Hourly buckets count
        #[arg(long, default_value = "24")]
        hourly: usize,
        /// Daily buckets count
        #[arg(long, default_value = "30")]
        daily: usize,
    },
    /// Manage directory sync targets (mirror / backup / archive)
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
    /// Manage plugins (logger / webhook / ...)
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Manage tags (named snapshot pointers)
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Diff two snapshots by ID (added/modified/removed files)
    Diff {
        /// From snapshot ID
        from: String,
        /// To snapshot ID
        to: String,
    },
    /// Show working-directory changes (what would be committed)
    Changes,
}

#[derive(Subcommand)]
enum SyncAction {
    /// Add a new sync target
    Add {
        /// Unique name for this target
        #[arg(short, long)]
        name: String,
        /// Source directory
        #[arg(short, long)]
        source: String,
        /// Destination directory (or remote address)
        #[arg(short, long)]
        dest: String,
        /// Sync mode: mirror | backup | archive
        #[arg(short, long, default_value = "backup")]
        mode: String,
        /// Transport: local | relay | server | p2p | webdav | s3
        #[arg(short, long)]
        transport: Option<String>,
        /// Conflict resolution (backup mode): keep_both | skip | overwrite
        #[arg(long)]
        conflict: Option<String>,
        /// Archive: max snapshots to keep
        #[arg(long)]
        max_snapshots: Option<usize>,
        /// Archive: max age in days
        #[arg(long)]
        max_age_days: Option<u32>,
        /// Archive: folder name format (chrono format)
        #[arg(long)]
        folder_format: Option<String>,
        /// Ignore pattern (repeatable)
        #[arg(long = "ignore")]
        ignore: Vec<String>,
        /// Create as disabled
        #[arg(long)]
        disable: bool,
        /// Remote URL (WebDAV server URL or S3 endpoint). Required for webdav/s3.
        #[arg(long, env = "ROUTE_REMOTE_URL")]
        url: Option<String>,
        /// WebDAV username
        #[arg(long, env = "ROUTE_REMOTE_USERNAME")]
        username: Option<String>,
        /// WebDAV password
        #[arg(long, env = "ROUTE_REMOTE_PASSWORD")]
        password: Option<String>,
        /// S3 access key
        #[arg(long, env = "ROUTE_REMOTE_ACCESS_KEY")]
        access_key: Option<String>,
        /// S3 secret key
        #[arg(long, env = "ROUTE_REMOTE_SECRET_KEY")]
        secret_key: Option<String>,
        /// S3 bucket name
        #[arg(long, env = "ROUTE_REMOTE_BUCKET")]
        bucket: Option<String>,
        /// S3 region (default: us-east-1)
        #[arg(long, env = "ROUTE_REMOTE_REGION")]
        region: Option<String>,
    },
    /// List all sync targets
    List,
    /// Remove a sync target
    Remove {
        name: String,
    },
    /// Show details of a sync target
    Show {
        name: String,
    },
    /// Enable a sync target
    Enable {
        name: String,
    },
    /// Disable a sync target
    Disable {
        name: String,
    },
    /// Run sync immediately (all enabled targets, or a specific one)
    Run {
        /// Specific target name (optional)
        name: Option<String>,
        /// Show verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Start the scheduler in the foreground (Ctrl+C to stop)
    Start {
        /// Polling interval in seconds
        #[arg(short, long, default_value = "60")]
        interval: u64,
    },
}

#[derive(Subcommand)]
enum BranchAction {
    /// List branches
    List,
    /// Create a new branch
    Create {
        name: String,
        /// Branch kind: main | inherited | sandbox
        #[arg(short, long, default_value = "inherited")]
        kind: String,
        /// Source branch (defaults to current)
        #[arg(long)]
        from: Option<String>,
    },
    /// Delete a branch
    Delete {
        name: String,
    },
    /// Switch to a branch
    Switch {
        name: String,
    },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List configured plugins
    List,
    /// Show details of one plugin
    Show {
        name: String,
    },
    /// Install a built-in plugin (logger | webhook). Optional JSON config.
    /// For webhook: `route plugin install webhook '{"url":"https://..."}'`
    Install {
        name: String,
        /// JSON config for the plugin (string). Optional for logger, required for webhook.
        #[arg(long)]
        config: Option<String>,
    },
    /// Remove a plugin from the config
    Remove {
        name: String,
    },
    /// Enable a plugin
    Enable {
        name: String,
    },
    /// Disable a plugin
    Disable {
        name: String,
    },
}

#[derive(Subcommand)]
enum TagAction {
    /// List all tags
    List,
    /// Create a tag pointing at a snapshot
    Add {
        /// Tag name (e.g. "v1.0")
        name: String,
        /// Snapshot ID to tag
        snapshot_id: String,
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Delete a tag by name
    Remove {
        name: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Init { path } => commands::init(path),
        Commands::Status => commands::status(),
        Commands::Commit {
            message,
            author,
            full,
            branch,
        } => commands::commit(message, author, full, branch),
        Commands::Log { limit, branch } => commands::log(limit, branch),
        Commands::Rollback {
            snapshot_id,
            reason,
        } => commands::rollback(snapshot_id, reason),
        Commands::Backup { target } => commands::backup(target),
        Commands::Branch { action } => match action {
            BranchAction::List => commands::branch_list(),
            BranchAction::Create { name, kind, from } => commands::branch_create(name, kind, from),
            BranchAction::Delete { name } => commands::branch_delete(name),
            BranchAction::Switch { name } => commands::branch_switch(name),
        },
        Commands::Annotate { commit_id, text } => commands::annotate(commit_id, text),
        Commands::Annotations { commit_id } => commands::annotations(commit_id),
        Commands::Export { format, out } => commands::export(format, out),
        Commands::Stats => commands::stats(),
        Commands::StatsReport {
            format,
            out,
            top,
            hourly,
            daily,
        } => commands::stats_report(format, out, top, hourly, daily),
        Commands::Sync { action } => match action {
            SyncAction::Add {
                name,
                source,
                dest,
                mode,
                transport,
                conflict,
                max_snapshots,
                max_age_days,
                folder_format,
                ignore,
                disable,
                url,
                username,
                password,
                access_key,
                secret_key,
                bucket,
                region,
            } => sync_commands::sync_add(
                name,
                source,
                dest,
                mode,
                transport,
                conflict,
                max_snapshots,
                max_age_days,
                folder_format,
                ignore,
                disable,
                url,
                username,
                password,
                access_key,
                secret_key,
                bucket,
                region,
            ),
            SyncAction::List => sync_commands::sync_list(),
            SyncAction::Remove { name } => sync_commands::sync_remove(name),
            SyncAction::Show { name } => sync_commands::sync_show(name),
            SyncAction::Enable { name } => sync_commands::sync_set_enabled(name, true),
            SyncAction::Disable { name } => sync_commands::sync_set_enabled(name, false),
            SyncAction::Run { name, verbose } => sync_commands::sync_run(name, verbose),
            SyncAction::Start { interval } => sync_commands::sync_start(interval),
        },
        Commands::Plugin { action } => match action {
            PluginAction::List => plugin_commands::plugin_list(),
            PluginAction::Show { name } => plugin_commands::plugin_show(name),
            PluginAction::Install { name, config } => {
                plugin_commands::plugin_install(name, config)
            }
            PluginAction::Remove { name } => plugin_commands::plugin_remove(name),
            PluginAction::Enable { name } => plugin_commands::plugin_set_enabled(name, true),
            PluginAction::Disable { name } => plugin_commands::plugin_set_enabled(name, false),
        },
        Commands::Tag { action } => match action {
            TagAction::List => commands::tag_list(),
            TagAction::Add {
                name,
                snapshot_id,
                message,
            } => commands::tag_add(name, snapshot_id, message),
            TagAction::Remove { name } => commands::tag_remove(name),
        },
        Commands::Diff { from, to } => commands::diff_snapshots(from, to),
        Commands::Changes => commands::changes(),
    }
}
