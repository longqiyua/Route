//! Route CLI — command-line interface for basic mode.

mod commands;
mod git_commands;
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
    /// Undo last commit (step back one snapshot)
    Undo,
    /// Redo last undone commit (step forward one snapshot)
    Redo,
    /// Create a named checkpoint at the current state
    Checkpoint {
        /// Checkpoint title (required)
        #[arg(short, long)]
        title: String,
        /// Optional body text
        #[arg(short, long)]
        body: Option<String>,
    },
    /// Git operations — drive the user's own git binary
    #[command(subcommand)]
    Git(GitAction),
    /// Manage active tracking targets (auto-sync with remote repos)
    #[command(subcommand)]
    Tracking(TrackingAction),
    /// Manage extensions (skills and references for AI pre-injection)
    #[command(subcommand)]
    Extensions(ExtensionAction),
    /// AI operations
    #[command(subcommand)]
    Ai(AiAction),
    /// Show project context for AI injection
    ProjectContext,
    /// Show MCP configuration
    Mcp {
        /// Show config snippet for AI client integration
        #[arg(long)]
        config: bool,
    },
    /// Get or set the permission level (normal|high)
    #[command(subcommand)]
    Permission(PermissionAction),
    /// Route Base operations: context, guard, status
    #[command(subcommand)]
    Base(BaseAction),
    /// Manage conversation sessions (AI chat tracking)
    #[command(subcommand)]
    Conversation(ConversationAction),
}

#[derive(Subcommand)]
enum GitAction {
    /// Initialize a git repository (idempotent)
    Init,
    /// Show working tree status
    Status,
    /// Show commit log
    Log {
        /// Max entries
        #[arg(short, long, default_value = "20")]
        limit: usize,
        /// Show ASCII graph with branch topology
        #[arg(short, long)]
        graph: bool,
        /// Show all branches (not just current)
        #[arg(short, long)]
        all: bool,
    },
    /// Stage all changes and commit
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,
    },
    /// Manage branches
    #[command(subcommand)]
    Branch(GitBranchAction),
    /// Manage remotes
    #[command(subcommand)]
    Remote(GitRemoteAction),
    /// Fetch from a remote
    Fetch {
        /// Remote name (default: origin)
        #[arg(short, long)]
        remote: Option<String>,
    },
    /// Pull from a remote branch (with rebase)
    Pull {
        /// Remote name (default: origin)
        #[arg(short, long)]
        remote: Option<String>,
        /// Branch name (default: current)
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Push to a remote branch
    Push {
        /// Remote name (default: origin)
        #[arg(short, long)]
        remote: Option<String>,
        /// Branch name (default: current)
        #[arg(short, long)]
        branch: Option<String>,
        /// Force push (with lease)
        #[arg(short, long)]
        force: bool,
    },
    /// Show diff (working tree or against a ref)
    Diff {
        /// Target ref (default: HEAD)
        target: Option<String>,
    },
    /// Stage files
    Add {
        /// Paths to stage (empty = all)
        paths: Vec<String>,
    },
    /// Unstage files
    Reset {
        /// Paths to unstage (empty = all)
        paths: Vec<String>,
    },
    /// Stash operations
    #[command(subcommand)]
    Stash(GitStashAction),
    /// Tag operations
    #[command(subcommand)]
    Tag(GitTagAction),
    /// Get/set git config
    Config {
        #[command(subcommand)]
        action: GitConfigAction,
    },
    /// Revert a commit (safe, creates new commit)
    Revert {
        /// Commit SHA to revert
        sha: String,
    },
    /// Cherry-pick commits onto current HEAD
    CherryPick {
        /// Commit SHAs to cherry-pick
        shas: Vec<String>,
        /// Optional commit message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Rebase current branch onto another branch
    Rebase {
        /// Target branch
        target: String,
    },
    /// Abort an in-progress rebase
    RebaseAbort,
    /// Continue a rebase after resolving conflicts
    RebaseContinue,
    /// Clean untracked files
    Clean {
        /// Dry-run (default: true)
        #[arg(long)]
        dry_run: Option<bool>,
        /// Remove untracked directories
        #[arg(short, long)]
        directories: bool,
        /// Actually remove files (default: dry-run)
        #[arg(short, long)]
        force: bool,
    },
    /// Show commit details
    Show {
        /// Commit SHA
        sha: String,
    },
    /// Create a git archive
    Archive {
        /// Output file path
        output: String,
        /// Format: zip, tar, tgz (default: zip)
        #[arg(short, long)]
        format: Option<String>,
        /// Tree-ish ref (default: HEAD)
        #[arg(short, long)]
        treeish: Option<String>,
    },
    /// Clone a remote repository
    Clone {
        /// Remote URL
        url: String,
        /// Target directory
        target: String,
    },
    /// Merge a branch into current
    Merge {
        /// Source branch
        source: String,
    },
    /// Create a safety backup
    Backup,
    /// Restore working tree files
    Restore {
        /// Paths to restore
        paths: Vec<String>,
    },
    /// Set git mode (route.mode config)
    Mode {
        /// Mode string
        mode: String,
    },
    /// Push with upstream (set upstream tracking)
    PushSetUpstream {
        /// Remote name (default: origin)
        #[arg(short, long)]
        remote: Option<String>,
        /// Branch name (default: current)
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Check if rebase is in progress
    RebaseInProgress,
    /// List git backups
    BackupList,
}

#[derive(Subcommand)]
enum GitBranchAction {
    /// List branches
    List,
    /// Create a branch
    Create {
        name: String,
    },
    /// Switch to a branch
    Switch {
        name: String,
    },
    /// Delete a branch
    Delete {
        name: String,
        /// Force delete (even if not merged)
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum GitRemoteAction {
    /// List remotes
    List,
    /// Add a remote
    Add {
        name: String,
        url: String,
    },
    /// Remove a remote
    Remove {
        name: String,
    },
}

#[derive(Subcommand)]
enum GitStashAction {
    /// List stashes
    List,
    /// Push working tree changes to stash
    Push {
        /// Optional message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Pop the top stash
    Pop,
}

#[derive(Subcommand)]
enum GitTagAction {
    /// List tags
    List,
    /// Create a tag at HEAD
    Create {
        name: String,
        /// Optional message (creates annotated tag)
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Delete a tag
    Delete {
        name: String,
    },
}

#[derive(Subcommand)]
enum GitConfigAction {
    /// Get a config value
    Get {
        key: String,
    },
    /// Set a config value
    Set {
        key: String,
        value: String,
        /// Scope: local, global, system (default: local)
        #[arg(short, long)]
        scope: Option<String>,
    },
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

#[derive(Subcommand)]
enum TrackingAction {
    /// List all active tracking targets
    List,
    /// Add a new tracking target
    Add {
        /// Local folder path
        #[arg(short, long)]
        local: String,
        /// Remote URL
        #[arg(short, long)]
        remote: String,
        /// Branch to track
        #[arg(short, long, default_value = "main")]
        branch: String,
        /// Sync interval in seconds
        #[arg(short, long, default_value = "600")]
        interval: u64,
    },
    /// Remove a tracking target
    Remove {
        /// Local folder path
        local: String,
    },
    /// Sync all (or a specific) tracking target now
    Sync {
        /// Specific local folder path to sync
        local: Option<String>,
    },
    /// Show sync history
    History,
}

#[derive(Subcommand)]
enum ExtensionAction {
    /// List all skill files in .route/skills/
    Skills,
    /// List all reference files in .route/references/
    References,
}

#[derive(Subcommand)]
enum AiAction {
    /// Send a chat message to the AI provider
    Chat {
        /// The message to send
        message: String,
        /// Optional system prompt
        #[arg(short, long)]
        system: Option<String>,
        /// Session ID to continue (auto-creates if not provided)
        #[arg(short, long)]
        session: Option<String>,
        /// Create a snapshot before recording the message
        #[arg(short, long)]
        snapshot: bool,
    },
    /// Show AI configuration
    Config,
}

#[derive(Subcommand)]
enum PermissionAction {
    /// Show current permission level
    Status,
    /// Set permission level (normal|high)
    Set {
        /// Permission level: normal or high
        level: String,
    },
}

#[derive(Subcommand)]
enum BaseAction {
    /// Show assembled AI context (project memory + structure + causal chain)
    Context {
        /// Maximum token budget for context (default: 3000)
        #[arg(short, long)]
        max_tokens: Option<usize>,
    },
    /// Show Route Guard protection status
    Guard,
}

#[derive(Subcommand)]
enum ConversationAction {
    /// List all conversation sessions
    List,
    /// Create a new conversation session
    New {
        /// Session title
        title: String,
    },
    /// Show messages in a conversation session
    Show {
        /// Session ID
        session_id: String,
        /// Limit the number of messages shown
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// Record a message in a conversation session
    Record {
        /// Session ID to record into
        session_id: String,
        /// Role: user, ai, or system
        role: String,
        /// Message content
        content: String,
        /// Optional snapshot ID to link to this message
        #[arg(short, long)]
        snapshot: Option<String>,
    },
    /// Rollback a session to a specific message
    Rollback {
        /// Session ID
        session_id: String,
        /// Message ID to rollback to
        message_id: String,
    },
    /// Archive a conversation session
    Archive {
        /// Session ID
        session_id: String,
    },
    /// Delete a conversation session
    Delete {
        /// Session ID
        session_id: String,
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
        Commands::Undo => commands::undo(),
        Commands::Redo => commands::redo(),
        Commands::Checkpoint { title, body } => commands::checkpoint(title, body),
        Commands::Git(action) => match action {
            GitAction::Init => git_commands::init(),
            GitAction::Status => git_commands::status(),
            GitAction::Log { limit, graph, all } => git_commands::log(limit, graph, all),
            GitAction::Commit { message } => git_commands::commit(message),
            GitAction::Branch(b) => match b {
                GitBranchAction::List => git_commands::branch_list(),
                GitBranchAction::Create { name } => git_commands::branch_create(name),
                GitBranchAction::Switch { name } => git_commands::branch_switch(name),
                GitBranchAction::Delete { name, force } => git_commands::branch_delete(name, force),
            },
            GitAction::Remote(r) => match r {
                GitRemoteAction::List => git_commands::remote_list(),
                GitRemoteAction::Add { name, url } => git_commands::remote_add(name, url),
                GitRemoteAction::Remove { name } => git_commands::remote_remove(name),
            },
            GitAction::Fetch { remote } => git_commands::fetch(remote),
            GitAction::Pull { remote, branch } => git_commands::pull(remote, branch),
            GitAction::Push { remote, branch, force } => git_commands::push(remote, branch, force),
            GitAction::Diff { target } => git_commands::diff(target),
            GitAction::Add { paths } => git_commands::add(paths),
            GitAction::Reset { paths } => git_commands::reset(paths),
            GitAction::Stash(s) => match s {
                GitStashAction::List => git_commands::stash_list(),
                GitStashAction::Push { message } => git_commands::stash_push(message),
                GitStashAction::Pop => git_commands::stash_pop(),
            },
            GitAction::Tag(t) => match t {
                GitTagAction::List => git_commands::tag_list(),
                GitTagAction::Create { name, message } => git_commands::tag_create(name, message),
                GitTagAction::Delete { name } => git_commands::tag_delete(name),
            },
            GitAction::Config { action } => match action {
                GitConfigAction::Get { key } => git_commands::config_get(key),
                GitConfigAction::Set { key, value, scope } => git_commands::config_set(key, value, scope),
            },
            GitAction::Revert { sha } => git_commands::revert(sha),
            GitAction::CherryPick { shas, message } => git_commands::cherry_pick(shas, message),
            GitAction::Rebase { target } => git_commands::rebase(target),
            GitAction::RebaseAbort => git_commands::rebase_abort(),
            GitAction::RebaseContinue => git_commands::rebase_continue(),
            GitAction::Clean { dry_run, directories, force } => {
                git_commands::clean(dry_run.unwrap_or(true), directories, force)
            }
            GitAction::Show { sha } => git_commands::show(sha),
            GitAction::Archive { output, format, treeish } => git_commands::archive(output, format, treeish),
            GitAction::Clone { url, target } => git_commands::clone(url, target),
            GitAction::Merge { source } => git_commands::merge(source),
            GitAction::Backup => git_commands::backup(),
            GitAction::Restore { paths } => git_commands::restore(paths),
            GitAction::Mode { mode } => git_commands::set_mode(mode),
            GitAction::PushSetUpstream { remote, branch } => git_commands::push_set_upstream(remote, branch),
            GitAction::RebaseInProgress => git_commands::rebase_in_progress(),
            GitAction::BackupList => git_commands::backup_list(),
        },
        Commands::Tracking(action) => match action {
            TrackingAction::List => commands::tracking_list(),
            TrackingAction::Add { local, remote, branch, interval } => commands::tracking_add(local, remote, branch, interval),
            TrackingAction::Remove { local } => commands::tracking_remove(local),
            TrackingAction::Sync { local } => commands::tracking_sync(local),
            TrackingAction::History => commands::tracking_history(),
        },
        Commands::Extensions(action) => match action {
            ExtensionAction::Skills => commands::extensions_skills(),
            ExtensionAction::References => commands::extensions_references(),
        },
        Commands::Ai(action) => match action {
            AiAction::Chat { message, system, session, snapshot } => {
                commands::ai_chat(message, system, session, snapshot)
            }
            AiAction::Config => commands::ai_config(true),
        },
        Commands::ProjectContext => commands::project_context(),
        Commands::Mcp { config } => commands::mcp_config(config),
        Commands::Permission(action) => match action {
            PermissionAction::Status => commands::permission_status(),
            PermissionAction::Set { level } => commands::permission_set(level),
        },
        Commands::Base(action) => match action {
            BaseAction::Context { max_tokens } => commands::base_context(max_tokens),
            BaseAction::Guard => commands::guard_status(),
        },
        Commands::Conversation(action) => match action {
            ConversationAction::List => commands::conversation_list(),
            ConversationAction::New { title } => commands::conversation_new(&title),
            ConversationAction::Show { session_id, limit } => commands::conversation_show(&session_id, limit),
            ConversationAction::Record { session_id, role, content, snapshot } => commands::conversation_record(&session_id, &role, &content, snapshot),
            ConversationAction::Rollback { session_id, message_id } => commands::conversation_rollback(&session_id, &message_id),
            ConversationAction::Archive { session_id } => commands::conversation_archive(&session_id),
            ConversationAction::Delete { session_id } => commands::conversation_delete(&session_id),
        },
    }
}
