//! Route CLI — command-line interface for basic mode.

mod commands;
mod git_commands;
mod plugin_commands;
mod sync_commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

#[derive(Parser)]
#[command(
    name = "route",
    version,
    about = "Route — local-first session-based version management for AI-assisted development"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a Route project in the current directory
    Init {
        /// Initialize in a specific path (defaults to current dir)
        #[arg(long)]
        path: Option<String>,
        /// Scan the project for capabilities, stacks, and docs
        #[arg(long)]
        scan: bool,
    },
    /// Show repository status
    Status {
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
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
    /// Compare two snapshots — see what changed
    Diff {
        /// From snapshot ID
        from: String,
        /// To snapshot ID
        to: String,
    },
    /// Show working directory changes — what would be committed
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
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
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
    /// Verify repository integrity — check for corruption
    Check {
        /// Re-hash every blob and confirm it matches its id (slow)
        #[arg(long)]
        full: bool,
        /// Skip the per-blob on-disk existence check (faster, less thorough)
        #[arg(long)]
        no_blobs: bool,
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// Generate a repair plan from current check findings (PIC bridge)
    RepairPlan {
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// Manage project Constitution — immutable development principles
    Constitution {
        #[command(subcommand)]
        action: ConstitutionAction,
    },
    /// Manage the Protocol — execution playbook (user-versionable)
    Protocol {
        #[command(subcommand)]
        action: ProtocolAction,
    },
    /// Manage Reference registry — external resource descriptions
    Reference {
        #[command(subcommand)]
        action: ReferenceAction,
    },
    /// Manage workflow definitions — reusable task patterns
    Workflow {
        #[command(subcommand)]
        action: WorkflowAction,
    },
    /// Preview a task plan without executing
    Plan {
        /// Task description
        #[arg(short, long)]
        task: String,
        /// Strategy ID to plan under (optional, uses current)
        #[arg(short, long)]
        strategy: Option<String>,
        /// Compare strategies (comma-separated IDs)
        #[arg(long)]
        compare: Option<String>,
        /// Output to stdout
        #[arg(long)]
        stdout: bool,
    },
    /// Generate an agent plan for the current task
    AgentPlan {
        /// Task description
        #[arg(short, long)]
        task: String,
        /// Optional workflow ID
        #[arg(short, long)]
        workflow: Option<String>,
        /// Target format: claude | codex | deepseek | generic
        #[arg(short = 'T', long, default_value = "generic")]
        target: String,
        /// Output to stdout
        #[arg(long)]
        stdout: bool,
    },
    /// Manage project profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Show curator role capabilities
    Curator,
    /// Manage the Effective Development Context and its change history.
    ///
    /// Default (no subcommand): print the current assembled context, with
    /// optional --hash / --metadata filters.
    Context {
        /// Print only the stable fingerprint (SHA-256 hex) of the context.
        #[arg(long)]
        hash: bool,
        /// Print the per-component hashes and version metadata instead of
        /// the full Markdown body.
        #[arg(long)]
        metadata: bool,
        /// Print timeline of recorded context snapshots (oldest first).
        #[arg(long)]
        history: bool,
        /// Show component metadata for a single recorded context hash.
        #[arg(long, value_name = "HASH")]
        show: Option<String>,
        /// Show component-level differences between two context hashes.
        /// Format: "HASH_A,HASH_B" (comma-separated pair).
        #[arg(long, value_name = "HASH_A,HASH_B")]
        diff: Option<String>,
        /// Replay a historical context snapshot (P0: Historical Replay).
        /// Reconstructs the context as it was at the time of the recorded hash.
        #[arg(long, value_name = "HASH")]
        replay: Option<String>,
        /// Target format for replay (claude|codex|generic). Default: generic.
        #[arg(long, value_name = "TARGET")]
        replay_target: Option<String>,
        /// Print replay output to stdout instead of rendering.
        #[arg(long)]
        replay_stdout: bool,
        /// Build a task-scoped context (P5: Task-Scoped Context).
        /// Uses deterministic lexical scoring to select relevant Reference entries.
        #[arg(long, value_name = "TASK")]
        task: Option<String>,
        /// Target format for task-scoped context (claude|codex|generic). Default: generic.
        #[arg(long, value_name = "TARGET")]
        task_target: Option<String>,
        /// Number of top-K references to include in task-scoped context (default: 5).
        #[arg(long, default_value = "5")]
        task_top_k: usize,
        /// Include project memory in context output (P2: Memory Context).
        /// When enabled, the Project Memory section will be appended.
        #[arg(long)]
        memory: bool,
        /// Show detailed explanation of reference selection (P6: Context Explain).
        /// Only valid with --task. Outputs selected/excluded refs, scores, signals,
        /// budget usage, learned confidence, and final context hash.
        #[arg(long)]
        explain: bool,
        /// Output --task context as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// Compile the Effective Context into a file for an external Coding AI
    /// (Claude Code, Codex, ...). The generated file is NOT the source of
    /// truth — `.route/` is. Uses managed block markers so user content is
    /// preserved.
    Apply {
        #[command(subcommand)]
        action: ApplyAction,
    },
    /// Record experience events and generate improvement proposals
    Learn {
        #[command(subcommand)]
        action: LearnAction,
    },
    /// Verified evolution core — candidate-first self-improvement (Observe → Propose → Candidate → Benchmark → Compare → Promote/Reject → Learn).
    ///
    /// Route records, evaluates, promotes, and rolls back. It never runs an
    /// LLM. Harness executes; Route gates. Any promotion must pass the Engine
    /// gate; a candidate can never promote itself.
    Evolve {
        #[command(subcommand)]
        action: EvolveAction,
    },
    /// Emergence Hardening (P13–P23) — experimental capabilities, off by default.
    Emerge {
        #[command(subcommand)]
        action: EmergeAction,
    },
    /// Analyze project structure and generate reference candidates
    Study {
        #[command(subcommand)]
        action: StudyAction,
    },

    /// Apply a study candidate by adding it to the Reference registry.
    StudyApply {
        /// Candidate ID (index from the last `route study` output)
        candidate_id: String,
    },

    /// Self-dogfood: study Route itself and generate improvement proposals.
    ///
    /// Studies the current project, generates pattern proposals, workflow
    /// evolution proposals, and memory proposals — all in 'pending' status.
    /// Does NOT auto-apply anything.
    SelfImprove,
    /// Manage Route's own versioned self-archive.
    ///
    /// Snapshots Route's current standard files into
    /// `Documents/Route/route/versions/` (append-only), lists the history, and
    /// rolls back to a historical version. Works outside any project.
    SelfArchive {
        #[command(subcommand)]
        action: SelfArchiveAction,
    },
    /// Emit cross-project self-evolution input.
    ///
    /// Reads the central archive (`Documents/Route/`) — registry + each
    /// project's save metadata and root info — and emits candidate input for a
    /// new standard. Read-only; never mutates projects or auto-applies.
    SelfEvolve,
    /// Manage AI task sessions — start, execute, verify, and end
    ///
    /// Create a session with `route task begin "<task>" --target <target>`,
    /// track its status, end it, and audit the full causal chain.
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    /// Manage project memory — semantic knowledge layer
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Manage strategy versions — record and restore
    Strategy {
        #[command(subcommand)]
        action: StrategyAction,
    },
    /// Manage strategy experiments — track task results under different strategies
    Experiment {
        #[command(subcommand)]
        action: ExperimentAction,
    },
    /// Manage agent organization memory — recall signals from past executions
    AgentOrg {
        #[command(subcommand)]
        action: AgentOrgAction,
    },
    /// Manage reusable patterns distilled from studies
    Pattern {
        #[command(subcommand)]
        action: PatternAction,
    },
    /// Manage development savepoints — like game save
    Save {
        #[command(subcommand)]
        action: SaveAction,
    },
    /// Trajectory learning — show/diff development trajectories
    Trajectory {
        #[command(subcommand)]
        action: TrajectoryAction,
    },
    /// Manage capability registry — unified view over Reference entries
    Capability {
        #[command(subcommand)]
        action: CapabilityAction,
    },
    /// Discover capabilities in a project path
    Discover {
        /// Path to scan (defaults to current directory)
        path: Option<String>,
    },
    /// Manage capability packs
    Pack {
        #[command(subcommand)]
        action: PackAction,
    },
    /// Manage idea/decision inbox
    Idea {
        #[command(subcommand)]
        action: IdeaAction,
    },
    /// Manage failure library
    Failure {
        #[command(subcommand)]
        action: FailureAction,
    },
    /// Manage development principle candidates
    Principle {
        #[command(subcommand)]
        action: PrincipleAction,
    },
    /// Manage agent role templates
    Agents {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Analyze the impact of a proposed change
    Impact {
        /// Change description
        change: String,
    },
    /// Generate a project health snapshot
    Health {
        /// Show detailed explanations
        #[arg(long)]
        explain: bool,
    },
    /// Show health history
    HealthHistory,
    /// Guardian scan — detect issues in project state
    Guardian {
        #[command(subcommand)]
        action: GuardianAction,
    },
    /// Show next action proposals
    Next {
        /// Max proposals to show
        #[arg(short, long, default_value = "5")]
        limit: usize,
        /// Start a task from a proposal
        #[arg(short = 'S', long)]
        start: Option<String>,
        /// Target AI host for --start: claude | codex
        #[arg(short = 'T', long, default_value = "claude")]
        target: String,
    },
    /// Manage project goals — track what matters
    Goal {
        #[command(subcommand)]
        action: GoalAction,
    },
    /// Manage external saves — save, list, restore, recover a deleted project
    ///
    /// Unlike `route save` (DevelopmentSavepoint inside .route/), this
    /// stores project state outside the project directory for recovery
    /// after deletion, selective partial restore, and learning chains.
    Archive {
        #[command(subcommand)]
        action: ArchiveAction,
    },
    /// Show consolidated project knowledge from all history
    ///
    /// Brain is a DERIVED VIEW (not SSOT) that compresses savepoints,
    /// trajectories, decisions, failures, patterns, goals, guardian findings,
    /// and studies into a compact, task-relevant knowledge base.
    Brain {
        #[command(subcommand)]
        action: BrainAction,
    },
    /// Show project roadmap
    Roadmap,
    /// Detect open loops
    Loops,
    /// Detect drift / decay
    Drift,
    /// Generate a maintenance plan
    Maintain {
        /// Show existing plans
        #[arg(long)]
        list: bool,
        /// Show a specific plan
        #[arg(long)]
        show: Option<String>,
        /// Generate a new plan
        #[arg(long)]
        plan: bool,
        /// Start a plan
        #[arg(short = 'S', long)]
        start: Option<String>,
        /// Run a single maintenance check
        #[arg(long)]
        check: bool,
        /// Target AI host for --start: claude | codex
        #[arg(short = 'T', long, default_value = "claude")]
        target: String,
    },
    /// Generate a project brief — quick overview
    Brief {
        /// Optional task-specific context
        #[arg(short, long)]
        task: Option<String>,
    },
    /// Generate a handoff document for a new AI session
    Handoff,
}

#[derive(Subcommand)]
enum SelfArchiveAction {
    /// Snapshot Route's current standard files as a new version.
    ///
    /// `from` may be a directory (recursive) or a single file.
    Archive {
        /// Source file or directory to snapshot (fetches all files under it).
        #[arg(long)]
        from: Option<PathBuf>,
        /// Why this iteration is happening.
        #[arg(long, default_value = "")]
        message: String,
        /// Route product version to tag the snapshot with.
        #[arg(long, default_value = "")]
        route_version: String,
    },
    /// List the self-archive history (newest last).
    List,
    /// Show a specific archived self-version.
    Show {
        /// Sequence number from `route self-archive list`.
        seq: u64,
        /// Emit as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Roll the archive back to a version.
    ///
    /// Prints the archived files, or writes them under `--out` so the
    /// harness/user can adopt them. Never mutates the repo itself.
    Apply {
        /// Sequence number to roll back to.
        seq: u64,
        /// Directory to write the archived files into.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum TaskAction {
    /// Begin a new execution session.
    ///
    /// Automatically builds task-scoped context, applies it to the target
    /// file, and returns a session ID. Errors if there is already an Active
    /// session unless --concurrent is passed.
    Begin {
        /// Task description (required).
        task: String,
        /// Target AI host: claude | codex | generic (default: claude).
        #[arg(short, long, default_value = "claude")]
        target: String,
        /// Allow concurrent sessions (skip the active-session check).
        #[arg(long)]
        concurrent: bool,
        /// Profile to use (defaults to active profile).
        #[arg(long)]
        profile: Option<String>,
        /// Workflow ID to use (overrides profile's workflow selection).
        #[arg(long)]
        workflow: Option<String>,
    },
    /// Show session status.
    ///
    /// With a session ID, shows that specific session. Without one, lists
    /// all sessions (most recent first).
    Status {
        /// Optional session ID to query.
        id: Option<String>,
    },
    /// Execute a command under the session (P2).
    ///
    /// Spawns the process directly (no shell). Records CheckPass/CheckFail
    /// evidence. stdout/stderr are hashed and truncated for display.
    ///
    /// Example: `route task exec S -- cargo test`
    Exec {
        /// Session ID (or prefix).
        session: String,
        /// Command argv to execute.
        #[arg(last = true, required = true)]
        argv: Vec<String>,
        /// Optional check/probe id for verification policy matching.
        #[arg(long)]
        check_id: Option<String>,
    },
    /// End a session with a result.
    ///
    /// If result is "success", checks the verification policy in the Protocol
    /// before allowing Succeeded status. Automatically triggers learning
    /// analysis (proposal-only, no auto-apply).
    ///
    /// Use --force to override verification failure (sets ForcedUnverified).
    End {
        /// Session ID to end.
        id: String,
        /// Result: success | failed | aborted
        #[arg(short, long)]
        result: String,
        /// Force success despite verification failure (P3).
        /// Sets status to ForcedUnverified — permanent audit marker.
        #[arg(long)]
        force: bool,
    },
    /// Show detailed audit for a session.
    ///
    /// Outputs: context hash, selected refs, AgentPolicy, commit/test/rollback
    /// evidence, final result, learning proposals, and full causal timeline.
    Show {
        /// Session ID.
        id: String,
        /// Show detailed explanation including context explain output.
        #[arg(long)]
        explain: bool,
    },
    /// Verify a session's verification policy (P3).
    ///
    /// Reads the Protocol VerificationPolicy, checks all required evidence
    /// is present and not stale. Returns VERIFIED, FAILED, or INCOMPLETE.
    /// Only System evidence is accepted.
    Verify {
        /// Session ID to verify.
        id: String,
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// One-command entry: start a new task session (P4).
    ///
    /// Atomically completes: begin session → baseline → build task context
    /// → archive → apply host → return session_id + host instructions.
    ///
    /// Example: `route task start "fix rollback corruption" --target claude`
    Start {
        /// Task description (required).
        task: String,
        /// Target AI host: claude | codex | generic (default: claude).
        #[arg(short, long, default_value = "claude")]
        target: String,
        /// Profile to use (defaults to active profile).
        #[arg(long)]
        profile: Option<String>,
        /// Workflow ID to use (overrides profile's workflow selection).
        #[arg(long)]
        workflow: Option<String>,
        /// Strategy ID to use for this task
        #[arg(short, long)]
        strategy: Option<String>,
        /// Create an automatic savepoint before starting the task.
        #[arg(long)]
        savepoint: bool,
        /// Optional parent EvolutionCampaign id this task serves.
        #[arg(long)]
        campaign_id: Option<String>,
    },
    /// Resume a session after restart (P6).
    ///
    /// Checks for drift: context changes, working tree changes, or host
    /// file modifications. Drift is reported but not auto-repaired.
    Resume {
        /// Session ID to resume.
        id: String,
    },
    /// Submit an execution report (AI feedback) for the session (P5).
    ///
    /// Records all observations as AgentFeedback evidence. Does NOT
    /// generate success/failure conclusions. Returns the evidence ids.
    Report {
        /// Session ID.
        id: String,
        /// Observations (free-form text).
        #[arg(short, long)]
        observations: Vec<String>,
    },
    /// Replay the exact context that was active when a session began.
    ///
    /// Read-only: reconstructs the task-scoped context + policy + selected
    /// refs from the session's recorded data. Does NOT restore files or
    /// re-execute the AI.
    Replay {
        /// Session ID to replay.
        id: String,
        /// Override target for replay (claude | codex | generic).
        #[arg(short, long)]
        target: Option<String>,
        /// Output replay to stdout instead of writing to file.
        #[arg(long)]
        stdout: bool,
    },
    /// Inspect what resources a task has bound
    InspectResources {
        /// Session ID
        id: String,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// Show current project memory
    Show,
    /// Show memory history
    History,
    /// Refresh memory from ledger/history (generates proposals)
    Refresh,
    /// Apply a pending memory proposal
    Apply {
        /// Proposal index
        index: usize,
    },
    /// Explain why a topic is the way it is
    Why {
        /// Topic to explain
        topic: String,
    },
    /// Supersede an old memory item with a new one
    Supersede {
        /// ID of the old item to supersede
        old: String,
        /// ID of the new item that replaces it
        new: String,
    },
    /// Show the knowledge map — lightweight relationship graph
    Map {
        /// Optional topic to filter by
        topic: Option<String>,
    },
}

#[derive(Subcommand)]
enum StrategyAction {
    /// Record current strategy as a snapshot
    Record {
        /// Optional label
        #[arg(short, long)]
        label: Option<String>,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List strategy snapshots
    List,
    /// Show a strategy snapshot
    Show {
        /// Snapshot ID (or prefix)
        id: String,
    },
    /// Diff two strategy snapshots
    Diff {
        /// First snapshot ID
        a: String,
        /// Second snapshot ID
        b: String,
    },
    /// Restore a strategy snapshot (preserves current first)
    Restore {
        /// Snapshot ID to restore
        id: String,
    },
    /// Show strategy history timeline
    History,
    /// Fork current strategy into a new named branch
    Fork {
        /// Name for the new strategy (e.g. "taiyi-inspired")
        name: String,
        /// Optional existing strategy ID to fork from (default: current)
        #[arg(long)]
        from: Option<String>,
    },
    /// Use (activate) a strategy
    Use {
        /// Strategy ID to activate
        id: String,
    },
    /// Compare two strategies
    Compare {
        /// First strategy ID
        a: String,
        /// Second strategy ID
        b: String,
    },
    /// Delete a strategy (non-current only)
    Delete {
        /// Strategy ID to delete
        id: String,
    },
}

#[derive(Subcommand)]
enum ExperimentAction {
    /// Show strategy statistics
    Stats,
    /// Suggest best strategy for a task
    Suggest {
        /// Task description
        task: String,
        /// Strategies to consider (comma-separated IDs)
        #[arg(short, long)]
        strategies: String,
    },
    /// Show experiment history
    History,
    /// Show experiments for a specific strategy
    Show {
        /// Strategy ID
        strategy: String,
    },
}

#[derive(Subcommand)]
enum AgentOrgAction {
    /// Show organization experience history
    History,
    /// Explain why a specific session had its agent plan
    Explain {
        /// Session ID or task description
        id: String,
    },
    /// Show recall signals for a task pattern
    Recall {
        /// Task pattern to recall for
        pattern: String,
    },
}

#[derive(Subcommand)]
enum PatternAction {
    /// List all patterns
    List,
    /// Show a specific pattern
    Show { id: String },
    /// Propose a new pattern from a study
    Propose {
        /// Study ID
        #[arg(long)]
        from_study: String,
        /// Pattern name
        #[arg(long)]
        name: String,
        /// Problem this pattern solves
        #[arg(long)]
        problem: String,
        /// Solution approach
        #[arg(long)]
        solution: String,
    },
    /// Apply a pattern proposal
    Apply {
        /// Proposal ID
        id: String,
    },
    /// List pending proposals
    Proposals,
}

#[derive(Subcommand)]
enum SaveAction {
    /// Create a savepoint
    Create {
        /// Name for this savepoint
        name: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List savepoints
    List,
    /// Show a savepoint
    Show { id: String },
    /// Preview restore
    Preview {
        id: String,
        /// Scope: code | strategy | context | all
        #[arg(short, long, default_value = "all")]
        scope: String,
    },
    /// Restore a savepoint
    Restore {
        id: String,
        /// Scope: code | strategy | context | all
        #[arg(short, long, default_value = "all")]
        scope: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// Diff two savepoints
    Diff { a: String, b: String },
    /// Delete a savepoint
    Delete { id: String },
}

#[derive(Subcommand)]
enum TrajectoryAction {
    /// List all development trajectories (newest first)
    List,
    /// Show detailed information about a trajectory
    Show {
        /// Trajectory ID (prefix matching)
        id: String,
    },
    /// Compare two trajectories
    Diff {
        /// First trajectory ID
        a: String,
        /// Second trajectory ID
        b: String,
    },
    /// Analyze a trajectory and generate a reversal analysis (P3)
    Analyze {
        /// Trajectory ID
        id: String,
    },
}

#[derive(Subcommand)]
enum LearnAction {
    /// Show learning dashboard summary
    Status,
    /// Explain why Route thinks something — trace the evidence chain to trajectories/savepoints
    Why {
        /// Target ID (proposal/pattern/trajectory)
        id: String,
        /// Target type: trajectory | pattern | proposal | insight (default: trajectory)
        #[arg(short, long, default_value = "trajectory")]
        #[arg(value_parser = ["trajectory", "pattern", "proposal", "insight"])]
        kind: String,
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// Reject a proposal/pattern (P12)
    Reject {
        /// Target ID
        id: String,
        /// Reason for rejection
        reason: Option<String>,
    },
    /// Supersede an older knowledge item with a newer one
    Supersede {
        /// Target ID to supersede
        target: String,
        /// New ID that supersedes
        by: String,
    },
    /// Downgrade confidence of a knowledge item
    Downgrade {
        /// Target ID
        id: String,
        /// New confidence value (0.0 - 1.0)
        confidence: f64,
    },
    /// Analyze trajectories and generate learning proposals
    Analyze,
    /// Record an experience event (manual or system).
    Record {
        /// Event kind: user_accept|user_reject|rollback|test_pass|test_fail|agent_result|manual_note
        #[arg(short, long)]
        kind: String,
        /// Outcome description (required).
        outcome: String,
        /// Evidence / supporting detail.
        #[arg(short, long)]
        evidence: Option<String>,
        /// Task description this event relates to.
        #[arg(short, long)]
        task: Option<String>,
        /// Scope: project|task-pattern|tool-or-host (default: project).
        #[arg(short, long)]
        scope: Option<String>,
        /// Scope value (required if scope is task-pattern or tool-or-host).
        #[arg(long)]
        scope_value: Option<String>,
        /// Comma-separated tags.
        #[arg(short, long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Provenance (who triggered this).
        #[arg(long)]
        provenance: Option<String>,
    },
    /// Review open learning proposals.
    Review {
        /// Show as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Apply (promote) a learning proposal to an Experience Reference.
    Apply {
        /// Proposal id.
        proposal_id: String,
    },
    /// Reject a learning proposal (original).
    RejectProposal {
        /// Proposal id.
        proposal_id: String,
        /// Reason for rejection.
        #[arg(short, long)]
        reason: String,
    },
    /// Show the full audit trail for a learning lifecycle.
    History {
        /// Claim or proposal id to inspect.
        id: Option<String>,
    },
}

#[derive(Subcommand)]
enum EvolveAction {
    /// Propose a candidate evolution experiment (candidate-first, never edits stable).
    Propose {
        /// Evolution target: project | workflow | context | harness | route
        #[arg(long, value_parser = ["project", "workflow", "context", "harness", "route"])]
        target: String,
        /// Hypothesis (what you expect to improve).
        #[arg(long)]
        hypothesis: String,
        /// Baseline id (the state being compared against).
        #[arg(long)]
        baseline_id: String,
        /// Candidate id (the changed state).
        #[arg(long)]
        candidate_id: String,
        /// Comma-separated changed scopes (concerns the candidate touches).
        #[arg(long, value_delimiter = ',')]
        changed_scope: Vec<String>,
        /// Rationale for the change.
        #[arg(long)]
        rationale: String,
        /// Who/what generated the candidate.
        #[arg(long)]
        generated_by: String,
        /// Reversible save id (required — candidate must be recoverable).
        #[arg(long)]
        reversible_save_id: String,
        /// Reference ids to anchor (comma-separated).
        #[arg(long, value_delimiter = ',')]
        reference_ids: Vec<String>,
        /// Studio preset ids to cross-benchmark across (comma-separated).
        #[arg(long, value_delimiter = ',')]
        studio_ids: Vec<String>,
        /// Optional parent Task session id.
        #[arg(long)]
        task_id: Option<String>,
        /// Optional ExecutionSession id this experiment ran under.
        #[arg(long)]
        session_id: Option<String>,
        /// Optional context hash this experiment was generated against.
        #[arg(long)]
        context_hash: Option<String>,
        /// Optional harness provenance (e.g. "dsh-pic", "generic-claude").
        #[arg(long)]
        harness: Option<String>,
    },
    /// Show an experiment (with --explain for decision rationale).
    Show {
        /// Experiment id.
        id: String,
        /// Explain the causal chain (why / what / baseline / benchmark / metrics / decision / recovery).
        #[arg(long)]
        explain: bool,
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Evaluate a candidate against its benchmark suite (produces a recommendation; never promotes).
    Evaluate {
        /// Experiment id.
        id: String,
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
        /// Harness-measured metric, repeatable: `name=baseline:candidate[:hib]`
        /// (hib = 1 for higher-is-better, 0 for lower-is-better). Adds real
        /// measured evidence to the correctness gate (P5 / P8).
        #[arg(long = "metric", value_delimiter = ',')]
        metric: Vec<String>,
    },
    /// Promote a candidate to KnownGood (gated by the Engine promotion policy).
    Promote {
        /// Experiment id.
        id: String,
        /// Label for the new KnownGood.
        #[arg(long)]
        label: String,
    },
    /// Reject a candidate (stable state untouched).
    Reject {
        /// Experiment id.
        id: String,
        /// Reason for rejection.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Show evolution history (experiments + KnownGood chain).
    History,
    /// Explain an experiment's emergence lineage (P24): parent, mutation,
    /// hypothesis, model/harness/references, novelty, benchmarks, who judged,
    /// why kept/rejected, new knowledge, replication, ablation.
    Explain {
        /// Experiment id.
        id: String,
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Campaign governance (P26–P43): aggregate experiment set + budget + selection.
    Campaign {
        #[command(subcommand)]
        action: CampaignAction,
    },
}

/// Campaign governance subcommands (P26–P43).
#[derive(Subcommand)]
enum CampaignAction {
    /// Create a new campaign (PLANNED).
    Create {
        /// Campaign goal.
        #[arg(long)]
        goal: String,
        /// Campaign scope.
        #[arg(long)]
        scope: String,
        /// Campaign strategy.
        #[arg(long)]
        strategy: String,
        /// Optional parent Task session id this campaign serves.
        #[arg(long)]
        task_id: Option<String>,
    },
    /// Show campaign status (P43 machine-readable with --json).
    Status {
        /// Campaign id.
        id: String,
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Compute the machine-readable next_action for an unattended campaign (P36/P43).
    Next {
        /// Campaign id.
        id: String,
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
        /// Comma-separated capabilities the executing harness declares
        /// (filesystem, shell, web, subagents, code_mode, skills, workflows).
        #[arg(long, value_delimiter = ',')]
        cap: Vec<String>,
    },
    /// Explain/report a campaign (P42; JSON with --json).
    ///
    /// With `--outcome <json>` ingests a harness execution outcome as
    /// UNTRUSTED input — `"success": true` never becomes a trusted PASS by
    /// itself. With `--explain` explains the campaign from persisted data.
    Report {
        /// Campaign id.
        id: String,
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
        /// Explain the campaign from persisted data (P11).
        #[arg(long)]
        explain: bool,
        /// Ingest a harness execution outcome as JSON (UNTRUSTED input).
        #[arg(long)]
        outcome: Option<String>,
    },
}

/// Emergence Hardening (P13–P23) — experimental by default.
#[derive(Subcommand)]
enum EmergeAction {
    /// Show experimental status, blackboard, champion, and store counts.
    Status {
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Explicitly enable experimental emergence capability (off by default).
    Enable,
    /// Append a blackboard event through the Engine (Actor → Event → Projection).
    Event {
        /// Actor (model/agent id).
        #[arg(long)]
        actor: String,
        /// Source: observation | proposal | evidence | decision.
        #[arg(long)]
        source: String,
        /// Confidence 0..1.
        #[arg(long)]
        confidence: f64,
        /// Optional session id.
        #[arg(long)]
        session_id: Option<String>,
        /// Optional task id.
        #[arg(long)]
        task_id: Option<String>,
    },
    /// Show the search budget / stop state of the current evolution run.
    Run {
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Show the benchmark court (independent lifecycle of benchmark cases).
    Court {
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
    /// Show the novelty archive (high-novelty non-winning candidates).
    Novelty {
        /// Output as JSON (machine-readable).
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum ConstitutionAction {
    /// Print the current constitution to stdout
    Show,
    /// Write the default constitution (no-op if one already exists)
    Init,
    /// Print the file path where the constitution lives
    Path,
}

#[derive(Subcommand)]
enum ProtocolAction {
    /// Print the current protocol to stdout
    Show,
    /// Write the default protocol (no-op if one already exists)
    Init,
    /// Mentions protocol version, revision, last-updated timestamp
    Status,
    /// Print the file path where the protocol lives
    Path,
}

#[derive(Subcommand)]
enum ReferenceAction {
    /// Print the full registry (JSON pretty)
    Show,
    /// Register or replace a reference entry
    Add {
        /// Stable unique id, e.g. 'doc-arch'
        id: String,
        /// Entry type: document|repo|skill|cli|executable|mcp|api|workflow|prompt
        #[arg(short = 't', long = "type")]
        type_: String,
        /// Source / path / URL / command name
        #[arg(short, long)]
        source: String,
        /// Optional sub-path or alias within the source
        #[arg(long)]
        path: Option<String>,
        /// One-sentence summary
        #[arg(short, long, default_value = "")]
        description: String,
        /// Capabilities free text
        #[arg(long, default_value = "")]
        capabilities: String,
        /// Constraints / forbidden usages
        #[arg(long, default_value = "")]
        constraints: String,
        /// Optional comma-separated tags
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Whether the entry is enabled (default: true)
        #[arg(long)]
        enabled: Option<bool>,
        /// Human-readable display name
        #[arg(long)]
        name: Option<String>,
        /// Project scope (e.g. frontend, backend, infra)
        #[arg(long)]
        project_scope: Option<String>,
        /// Trust level (high, medium, low)
        #[arg(long)]
        trust: Option<String>,
        /// Entrypoint path or command
        #[arg(long)]
        entrypoint: Option<String>,
    },
    /// Remove a reference entry by id
    Remove {
        id: String,
        /// Force removal even for UserCreated entries
        #[arg(long)]
        force: bool,
    },
    /// List entries, optionally filtered by type
    List {
        /// Filter by type (document, repo, skill, cli, mcp, ...)
        #[arg(short, long)]
        type_: Option<String>,
        /// Also show disabled entries
        #[arg(short, long)]
        show_disabled: bool,
    },
    /// Print the registry path
    Path,
    /// Import a reference entry from an external source.
    ///
    /// Supported sources in v0:
    ///   * Markdown / text file (path ending in `.md` / `.mdx` / `.txt`)
    ///   * A local Route skill file (path ending in `.rs` or `.toml`, or
    ///     a directory containing a `SKILL.md` / `SKILL.toml`)
    ///   * A Git repository URL (`https://...git` or `git@...`)
    ///   * CLI `--help` output: either a path to a saved text file whose
    ///     name contains `help`, or `cli:<cmd>` to actually invoke
    ///     `<cmd> --help` and parse its output
    Import {
        /// Source (file path, git URL, or `cli:<cmd>`).
        source: String,
        /// Explicit id for the imported entry. Defaults to a slug of the
        /// source name.
        #[arg(long)]
        id: Option<String>,
        /// Override the detected type (document/repo/skill/cli).
        #[arg(short = 't', long = "type")]
        type_: Option<String>,
        /// Override the one-sentence summary for sources where none can
        /// be reliably extracted.
        #[arg(short, long)]
        description: Option<String>,
        /// Replace an existing entry with the same id. The default is
        /// to refuse overwriting so user-created entries are safe.
        #[arg(long)]
        replace: bool,
    },
    /// Run the Reference Curator once: analyze the current registry, list
    /// any Unknown entries, report stale Imported ones, and emit a
    /// human-readable proposal list (or JSON if --json). Writes proposals
    /// to `.route/reference/proposals.json` for later apply.
    Review {
        /// Emit JSON output (for tooling) instead of the human report.
        #[arg(long)]
        json: bool,
        /// Do not persist proposals to disk. Only print.
        #[arg(long, alias = "dry-run")]
        dry_run: bool,
    },
    /// Apply a proposal previously generated by `review`.
    Apply {
        /// Proposal id (the stable ULID printed by `review`).
        proposal_id: String,
    },
    /// Re-check imported references against their sources. Always updates
    /// last_checked; only generates update proposals if content_hash
    /// actually changed. No automatic deletion.
    Refresh {
        /// Check only a specific id. Omit for all Imported entries.
        id: Option<String>,
        /// Do not persist result or proposals. Only print.
        #[arg(long, alias = "dry-run")]
        dry_run: bool,
    },
    /// Show detailed info about a reference entry
    Inspect {
        /// Reference entry id
        id: String,
    },
    /// Enable a reference entry
    Enable {
        /// Reference entry id
        id: String,
    },
    /// Disable a reference entry
    Disable {
        /// Reference entry id
        id: String,
    },
}

#[derive(Subcommand)]
enum WorkflowAction {
    /// List all workflows
    List,
    /// Show details of a workflow
    Show {
        /// Workflow id
        id: String,
    },
    /// Import a workflow from a file or URL
    Import {
        /// Source (file path or URL)
        source: String,
        /// Explicit id for the workflow
        #[arg(long)]
        id: Option<String>,
        /// Human-readable name
        #[arg(long)]
        name: Option<String>,
        /// Description
        #[arg(long)]
        description: Option<String>,
    },
    /// Enable a workflow
    Enable {
        /// Workflow id
        id: String,
    },
    /// Disable a workflow
    Disable {
        /// Workflow id
        id: String,
    },
    /// Create a workflow from a study candidate
    FromStudy {
        /// Candidate index from the last study report
        index: usize,
        /// Optional ID override
        #[arg(long)]
        id: Option<String>,
    },
    /// Propose evolving a workflow based on project experience
    Evolve {
        /// Workflow ID to evolve
        id: String,
        /// Reason for the evolution
        #[arg(short, long)]
        reason: String,
        /// Expected effect
        #[arg(short, long)]
        effect: String,
        /// Risk assessment
        #[arg(short, long)]
        risk: String,
    },
    /// Apply a workflow change proposal
    ApplyEvolve {
        /// Proposal ID
        id: String,
    },
    /// List workflow change proposals
    Proposals,
    /// Reset a workflow to upstream or a specific revision
    Reset {
        /// Workflow ID
        id: String,
        /// Reset to upstream or a specific revision number
        #[arg(long)]
        to: String,
    },
    /// Generate workflow evolution proposals from trajectory analysis (P6)
    Learn {
        /// Workflow ID to analyze
        id: String,
    },
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List all profiles
    List,
    /// Show current active profile
    Show,
    /// Use a specific profile
    Use {
        /// Profile id
        id: String,
    },
}

#[derive(Subcommand)]
enum ApplyAction {
    /// Compile the Effective Context into `CLAUDE.md` (Claude Code).
    /// Optionally scoped to a task (P7: Task-Scoped Apply).
    Claude {
        /// Task description for task-scoped context selection.
        #[arg(long, value_name = "TASK")]
        task: Option<String>,
    },
    /// Compile the Effective Context into `AGENTS.md` (OpenAI Codex).
    /// Optionally scoped to a task (P7: Task-Scoped Apply).
    Codex {
        /// Task description for task-scoped context selection.
        #[arg(long, value_name = "TASK")]
        task: Option<String>,
    },
    /// Compile the Effective Context into `.route/generated/context.md`.
    /// Optionally scoped to a task (P7: Task-Scoped Apply).
    Generic {
        /// Task description for task-scoped context selection.
        #[arg(long, value_name = "TASK")]
        task: Option<String>,
    },
    /// Show whether each target is up-to-date with the current context.
    Status,
    /// Run full verification for a target (checks context hash AND
    /// managed block content integrity). Optionally specify a target.
    Verify {
        /// Target to verify (claude|codex|generic). Verifies all if omitted.
        target: Option<String>,
    },
}

#[derive(Subcommand)]
enum StudyAction {
    /// Analyze a project — produce a StudyReport and save to library
    Analyze {
        /// Path to the project (file, directory, or repo root)
        path: String,
    },
    /// List all studies in the library
    List,
    /// Show a specific study from the library
    Show {
        /// Study ID
        id: String,
    },
    /// Diff two studies in the library
    Diff {
        /// First study ID
        a: String,
        /// Second study ID
        b: String,
    },
    /// Compare multiple studies — common patterns, conflicts, unique, learnings
    Compare {
        /// Study IDs to compare
        ids: Vec<String>,
    },
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
    Create { name: String },
    /// Switch to a branch
    Switch { name: String },
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
    Add { name: String, url: String },
    /// Remove a remote
    Remove { name: String },
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
    Delete { name: String },
}

#[derive(Subcommand)]
enum GitConfigAction {
    /// Get a config value
    Get { key: String },
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
    Remove { name: String },
    /// Show details of a sync target
    Show { name: String },
    /// Enable a sync target
    Enable { name: String },
    /// Disable a sync target
    Disable { name: String },
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
    Delete { name: String },
    /// Switch to a branch
    Switch { name: String },
}

#[derive(Subcommand)]
enum PluginAction {
    /// List configured plugins
    List,
    /// Show details of one plugin
    Show { name: String },
    /// Install a built-in plugin (logger | webhook). Optional JSON config.
    /// For webhook: `route plugin install webhook '{"url":"https://..."}'`
    Install {
        name: String,
        /// JSON config for the plugin (string). Optional for logger, required for webhook.
        #[arg(long)]
        config: Option<String>,
    },
    /// Remove a plugin from the config
    Remove { name: String },
    /// Enable a plugin
    Enable { name: String },
    /// Disable a plugin
    Disable { name: String },
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
    Remove { name: String },
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

#[derive(Subcommand)]
enum CapabilityAction {
    /// List all capabilities, optionally filtered by kind
    List {
        /// Filter by kind (knowledge|instruction|tool|skill|workflow|service|runtime)
        #[arg(short, long)]
        kind: Option<String>,
    },
    /// Show details of a capability
    Show {
        /// Capability ID
        id: String,
    },
    /// Inspect a capability — detailed description
    Inspect {
        /// Capability ID
        id: String,
    },
}

#[derive(Subcommand)]
enum PackAction {
    /// Create a new pack
    Create {
        /// Pack name
        name: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Workflow IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        workflows: Vec<String>,
        /// Skill IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        skills: Vec<String>,
        /// Tool IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tools: Vec<String>,
        /// Reference IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        refs: Vec<String>,
        /// Capability IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,
        /// Default strategy ID
        #[arg(long)]
        strategy: Option<String>,
        /// Tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// List all packs
    List,
    /// Show a pack
    Show {
        /// Pack ID
        id: String,
    },
    /// Use (activate) a pack
    Use {
        /// Pack ID
        id: String,
    },
    /// Deactivate a pack
    Deactivate {
        /// Pack ID
        id: String,
    },
    /// Export a pack as JSON
    Export {
        /// Pack ID
        id: String,
    },
    /// Import a pack from JSON
    Import {
        /// JSON file path
        path: String,
    },
    /// Delete a pack
    Delete {
        /// Pack ID
        id: String,
    },
}

#[derive(Subcommand)]
enum IdeaAction {
    /// Add a new idea
    Add {
        /// Idea text
        text: String,
        /// Source (user, ai, system)
        #[arg(short, long, default_value = "user")]
        source: String,
        /// Optional task ID
        #[arg(short, long)]
        task: Option<String>,
        /// Tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// List all ideas, optionally filtered by status
    List {
        /// Filter by status (inbox|considering|accepted|rejected|implemented)
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Show an idea
    Show {
        /// Idea ID
        id: String,
    },
    /// Accept an idea
    Accept {
        /// Idea ID
        id: String,
    },
    /// Reject an idea
    Reject {
        /// Idea ID
        id: String,
    },
    /// Mark an idea as implemented
    Implemented {
        /// Idea ID
        id: String,
    },
    /// Delete an idea
    Delete {
        /// Idea ID
        id: String,
    },
}

#[derive(Subcommand)]
enum FailureAction {
    /// List all failure cases
    List {
        /// Filter by severity (critical|major|minor|cosmetic)
        #[arg(short, long)]
        severity: Option<String>,
    },
    /// Show a failure case
    Show {
        /// Failure case ID
        id: String,
    },
    /// Search failure cases
    Search {
        /// Search query
        query: String,
    },
    /// Resolve a failure case
    Resolve {
        /// Failure case ID
        id: String,
        /// Resolution description
        #[arg(short, long)]
        resolution: String,
    },
    /// Delete a failure case
    Delete {
        /// Failure case ID
        id: String,
    },
    /// Add a new failure case
    Add {
        /// Problem description
        problem: String,
        /// What was attempted
        #[arg(short, long)]
        attempt: String,
        /// Symptoms observed
        #[arg(short, long)]
        symptom: String,
        /// Root cause (if known)
        #[arg(short, long)]
        cause: Option<String>,
        /// Affected scope (comma-separated)
        #[arg(long, value_delimiter = ',')]
        scope: Vec<String>,
        /// Tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Severity (critical|major|minor|cosmetic)
        #[arg(short, long, default_value = "major")]
        severity: String,
    },
}

#[derive(Subcommand)]
enum PrincipleAction {
    /// List all principle candidates, optionally filtered by status
    List {
        /// Filter by status (pending|applied_to_memory|promoted_to_protocol|promoted_to_constitution|rejected)
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Show a principle candidate
    Show {
        /// Candidate ID
        id: String,
    },
    /// Apply a principle to memory
    Apply {
        /// Candidate ID
        id: String,
    },
    /// Promote a principle to protocol
    Promote {
        /// Candidate ID
        id: String,
    },
    /// Reject a principle candidate
    Reject {
        /// Candidate ID
        id: String,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// List role templates
    Templates {
        /// Filter by task pattern
        #[arg(short, long)]
        pattern: Option<String>,
    },
    /// Show a role template
    Show {
        /// Template ID
        id: String,
    },
    /// Record a successful template usage
    RecordSuccess {
        /// Template ID
        id: String,
        /// Session ID
        session: String,
    },
    /// Record a failed template usage
    RecordFailure {
        /// Template ID
        id: String,
        /// Session ID
        session: String,
    },
}

#[derive(Subcommand)]
enum GuardianAction {
    /// Run a guardian scan
    Scan,
    /// List all findings
    Findings {
        /// Filter by status: open | acknowledged | resolved | ignored
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Show a specific finding
    Show {
        /// Finding ID
        id: String,
    },
    /// Resolve a finding
    Resolve {
        /// Finding ID
        id: String,
    },
    /// Ignore a finding
    Ignore {
        /// Finding ID
        id: String,
    },
}

#[derive(Subcommand)]
enum GoalAction {
    /// Add a new goal
    Add {
        /// Goal title
        title: String,
        /// Goal description
        #[arg(short, long)]
        description: Option<String>,
        /// Priority (1-5)
        #[arg(short, long, default_value = "3")]
        priority: u8,
        /// Success criteria
        #[arg(short, long)]
        criteria: Vec<String>,
        /// Tags
        #[arg(short, long)]
        tags: Vec<String>,
    },
    /// List goals
    List {
        /// Filter by status: active | paused | done | abandoned
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Show a goal
    Show {
        /// Goal ID
        id: String,
    },
    /// Update goal status
    Update {
        /// Goal ID
        id: String,
        /// New status: active | paused | done | abandoned
        #[arg(short, long)]
        status: String,
    },
    /// Delete a goal
    Delete {
        /// Goal ID
        id: String,
    },
}

#[derive(Subcommand)]
enum ArchiveAction {
    /// Initialize archive for the current project (creates Original)
    Init,
    /// Create a manual save
    Save {
        /// Reason / caption for this save
        reason: String,
    },
    /// List saves
    List {
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// Show save details
    Show {
        /// Save ID or index
        id: String,
        /// Output as JSON (machine-readable)
        #[arg(long)]
        json: bool,
    },
    /// Diff two saves
    Diff {
        /// Save A (ID or "original")
        a: String,
        /// Save B (ID or "current")
        b: String,
    },
    /// Restore from a save (auto-creates pre-restore save)
    Restore {
        /// Save ID
        id: String,
        /// Restore scope: full | project | route-state | paths=<...>
        #[arg(long, default_value = "full")]
        scope: String,
        /// Specific paths to restore (comma-separated, used with --scope paths)
        #[arg(long)]
        paths: Option<String>,
    },
    /// Recover a deleted project
    Recover {
        /// Project ID
        project_id: String,
        /// Save ID to recover from (default: latest verified)
        #[arg(long)]
        save: Option<String>,
        /// Target path to restore to
        #[arg(long)]
        to: String,
    },
    /// Show recovery options for a deleted project
    RecoverList {
        /// Project ID
        project_id: String,
    },
    /// Check archive invariants
    Check,
    /// Show path history across saves
    PathHistory {
        /// File path to show history for
        path: String,
    },
    /// Delete a save (requires confirmation)
    Delete {
        /// Save ID to delete
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// Delete entire project archive (requires confirmation)
    DeleteProject {
        /// Project ID
        project_id: Option<String>,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum BrainAction {
    /// Show current brain (full view)
    Show,
    /// L0 Brief — the absolute minimum AI needs to know
    Brief,
    /// Refresh brain from all data sources (generates proposals)
    Refresh,
    /// Apply pending brain proposals
    Apply {
        /// Proposal index to apply (or "all")
        index: String,
    },
    /// Show brain history (past versions)
    History,
    /// Explain a brain item with full source chain (L2)
    Explain {
        /// Brain item ID
        id: String,
    },
    /// Task-specific brain view
    For {
        /// Task description
        task: String,
    },
    /// Expand a brain item to show full details
    Expand {
        /// Brain item ID
        id: String,
    },
    /// Show knowledge conflicts
    Conflicts,
    /// Compact brain — rebuild indices without deleting data
    Compact,
    /// Run brain doctor — quality checks
    Doctor,
    /// Diff two brain versions
    Diff {
        /// Brain version A (or "current" for current)
        a: String,
        /// Brain version B (or "current" for current)
        b: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { path, scan } => commands::init(path, scan),
        Commands::Status { json } => commands::status(json),
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
            PluginAction::Install { name, config } => plugin_commands::plugin_install(name, config),
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
        Commands::Checkpoint { title, body, json } => commands::checkpoint(title, body, json),
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
            GitAction::Push {
                remote,
                branch,
                force,
            } => git_commands::push(remote, branch, force),
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
                GitConfigAction::Set { key, value, scope } => {
                    git_commands::config_set(key, value, scope)
                }
            },
            GitAction::Revert { sha } => git_commands::revert(sha),
            GitAction::CherryPick { shas, message } => git_commands::cherry_pick(shas, message),
            GitAction::Rebase { target } => git_commands::rebase(target),
            GitAction::RebaseAbort => git_commands::rebase_abort(),
            GitAction::RebaseContinue => git_commands::rebase_continue(),
            GitAction::Clean {
                dry_run,
                directories,
                force,
            } => git_commands::clean(dry_run.unwrap_or(true), directories, force),
            GitAction::Show { sha } => git_commands::show(sha),
            GitAction::Archive {
                output,
                format,
                treeish,
            } => git_commands::archive(output, format, treeish),
            GitAction::Clone { url, target } => git_commands::clone(url, target),
            GitAction::Merge { source } => git_commands::merge(source),
            GitAction::Backup => git_commands::backup(),
            GitAction::Restore { paths } => git_commands::restore(paths),
            GitAction::Mode { mode } => git_commands::set_mode(mode),
            GitAction::PushSetUpstream { remote, branch } => {
                git_commands::push_set_upstream(remote, branch)
            }
            GitAction::RebaseInProgress => git_commands::rebase_in_progress(),
            GitAction::BackupList => git_commands::backup_list(),
        },
        Commands::Tracking(action) => match action {
            TrackingAction::List => commands::tracking_list(),
            TrackingAction::Add {
                local,
                remote,
                branch,
                interval,
            } => commands::tracking_add(local, remote, branch, interval),
            TrackingAction::Remove { local } => commands::tracking_remove(local),
            TrackingAction::Sync { local } => commands::tracking_sync(local),
            TrackingAction::History => commands::tracking_history(),
        },
        Commands::Extensions(action) => match action {
            ExtensionAction::Skills => commands::extensions_skills(),
            ExtensionAction::References => commands::extensions_references(),
        },
        Commands::Ai(action) => match action {
            AiAction::Chat {
                message,
                system,
                session,
                snapshot,
            } => commands::ai_chat(message, system, session, snapshot),
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
            ConversationAction::Show { session_id, limit } => {
                commands::conversation_show(&session_id, limit)
            }
            ConversationAction::Record {
                session_id,
                role,
                content,
                snapshot,
            } => commands::conversation_record(&session_id, &role, &content, snapshot),
            ConversationAction::Rollback {
                session_id,
                message_id,
            } => commands::conversation_rollback(&session_id, &message_id),
            ConversationAction::Archive { session_id } => {
                commands::conversation_archive(&session_id)
            }
            ConversationAction::Delete { session_id } => commands::conversation_delete(&session_id),
        },
        Commands::Check {
            full,
            no_blobs,
            json,
        } => commands::check(full, no_blobs, json),
        Commands::RepairPlan { json } => commands::repair_plan(json),
        Commands::Constitution { action } => match action {
            ConstitutionAction::Show => commands::constitution_show(),
            ConstitutionAction::Init => commands::constitution_init(),
            ConstitutionAction::Path => commands::constitution_path(),
        },
        Commands::Protocol { action } => match action {
            ProtocolAction::Show => commands::protocol_show(),
            ProtocolAction::Init => commands::protocol_init(),
            ProtocolAction::Status => commands::protocol_status(),
            ProtocolAction::Path => commands::protocol_path(),
        },
        Commands::Reference { action } => match action {
            ReferenceAction::Show => commands::reference_show(),
            ReferenceAction::Add {
                id,
                type_,
                source,
                path,
                description,
                capabilities,
                constraints,
                tags,
                enabled,
                name,
                project_scope,
                trust,
                entrypoint,
            } => commands::reference_add(
                id,
                type_,
                source,
                path,
                description,
                capabilities,
                constraints,
                tags,
                enabled,
                name,
                project_scope,
                trust,
                entrypoint,
            ),
            ReferenceAction::Remove { id, force } => commands::reference_remove(id, force),
            ReferenceAction::List {
                type_,
                show_disabled,
            } => commands::reference_list(type_, show_disabled),
            ReferenceAction::Path => commands::reference_path(),
            ReferenceAction::Import {
                source,
                id,
                type_,
                description,
                replace,
            } => commands::reference_import(source, id, type_, description, replace),
            ReferenceAction::Review { json, dry_run } => commands::reference_review(json, dry_run),
            ReferenceAction::Apply { proposal_id } => commands::reference_apply(proposal_id),
            ReferenceAction::Refresh { id, dry_run } => commands::reference_refresh(id, dry_run),
            ReferenceAction::Inspect { id } => commands::reference_inspect(id),
            ReferenceAction::Enable { id } => commands::reference_enable(id),
            ReferenceAction::Disable { id } => commands::reference_disable(id),
        },
        Commands::Workflow { action } => match action {
            WorkflowAction::List => commands::workflow_list(),
            WorkflowAction::Show { id } => commands::workflow_show(id),
            WorkflowAction::Import {
                source,
                id,
                name,
                description,
            } => commands::workflow_import(source, id, name, description),
            WorkflowAction::Enable { id } => commands::workflow_enable(id),
            WorkflowAction::Disable { id } => commands::workflow_disable(id),
            WorkflowAction::FromStudy { index, id } => commands::workflow_from_study(index, id),
            WorkflowAction::Evolve {
                id,
                reason,
                effect,
                risk,
            } => commands::workflow_evolve(id, reason, effect, risk),
            WorkflowAction::ApplyEvolve { id } => commands::workflow_apply_evolve(id),
            WorkflowAction::Proposals => commands::workflow_list_proposals(),
            WorkflowAction::Reset { id, to } => commands::workflow_reset(id, to),
            WorkflowAction::Learn { id } => commands::workflow_learn_from_trajectories(id),
        },
        Commands::Plan {
            task,
            strategy,
            compare,
            stdout,
        } => commands::plan(task, strategy, compare, stdout),
        Commands::AgentPlan {
            task,
            workflow,
            target,
            stdout,
        } => commands::agent_plan(task, workflow, target, stdout),
        Commands::Profile { action } => match action {
            ProfileAction::List => commands::profile_list(),
            ProfileAction::Show => commands::profile_show(),
            ProfileAction::Use { id } => commands::profile_use(id),
        },
        Commands::Curator => commands::curator(),
        Commands::Context {
            hash,
            metadata,
            history,
            show,
            diff,
            replay,
            replay_target,
            replay_stdout,
            task,
            task_target,
            task_top_k,
            memory,
            explain,
            json,
        } => {
            // P0: Historical Replay takes priority
            if let Some(replay_hash) = replay {
                commands::context_replay(replay_hash, replay_target, replay_stdout)
            } else if let Some(task_str) = task {
                // P6: Context Explain (with --explain flag)
                if explain {
                    commands::context_explain(task_str, task_target, task_top_k)
                } else {
                    // P5: Task-Scoped Context
                    commands::context_task(task_str, task_target, task_top_k, json)
                }
            } else {
                let pair = diff
                    .map(|s| {
                        let mut it = s.splitn(2, ',');
                        let a = it.next().unwrap_or("").to_string();
                        let b = it.next().unwrap_or("").to_string();
                        (a, b)
                    })
                    .and_then(|(a, b)| {
                        if a.is_empty() || b.is_empty() {
                            None
                        } else {
                            Some((a, b))
                        }
                    });
                commands::context_dispatch(hash, metadata, history, show, pair, memory)
            }
        }
        Commands::Apply { action } => match action {
            ApplyAction::Claude { task } => {
                commands::apply_target(route_basic::ApplyTarget::Claude, task.as_deref())
            }
            ApplyAction::Codex { task } => {
                commands::apply_target(route_basic::ApplyTarget::Codex, task.as_deref())
            }
            ApplyAction::Generic { task } => {
                commands::apply_target(route_basic::ApplyTarget::Generic, task.as_deref())
            }
            ApplyAction::Status => commands::apply_status(),
            ApplyAction::Verify { target } => commands::apply_verify(target),
        },
        Commands::Learn { action } => match action {
            // Original Adaptive Learning v0 variants
            LearnAction::Record {
                kind,
                outcome,
                evidence,
                task,
                scope,
                scope_value,
                tags,
                provenance,
            } => commands::learn_record(
                kind,
                outcome,
                evidence,
                task,
                scope,
                scope_value,
                tags,
                provenance,
            ),
            LearnAction::Review { json } => commands::learn_review(json),
            LearnAction::Apply { proposal_id } => commands::learn_apply(proposal_id),
            LearnAction::RejectProposal {
                proposal_id,
                reason,
            } => commands::learn_reject(proposal_id, reason),
            LearnAction::History { id } => commands::learn_history(id),
            // Trajectory Learning v1 variants
            LearnAction::Status => commands::learn_status(),
            LearnAction::Why { id, kind, json } => commands::learn_why(id, kind, json),
            LearnAction::Reject { id, reason } => commands::trajectory_learn_reject(id, reason),
            LearnAction::Supersede { target, by } => commands::learn_supersede(target, by),
            LearnAction::Downgrade { id, confidence } => commands::learn_downgrade(id, confidence),
            // Shared: Analyze dispatches to trajectory learning (P5)
            LearnAction::Analyze => commands::trajectory_learn_analyze(),
        },
        Commands::Study { action } => match action {
            StudyAction::Analyze { path } => commands::study(path),
            StudyAction::List => commands::study_list(),
            StudyAction::Show { id } => commands::study_show(id),
            StudyAction::Diff { a, b } => commands::study_diff(a, b),
            StudyAction::Compare { ids } => commands::study_compare(ids),
        },
        Commands::Emerge { action } => match action {
            EmergeAction::Status { json } => commands::emerge_status(json),
            EmergeAction::Enable => commands::emerge_enable(),
            EmergeAction::Event {
                actor,
                source,
                confidence,
                session_id,
                task_id,
            } => commands::emerge_event(actor, source, confidence, session_id, task_id),
            EmergeAction::Run { json } => commands::emerge_run(json),
            EmergeAction::Court { json } => commands::emerge_court(json),
            EmergeAction::Novelty { json } => commands::emerge_novelty(json),
        },
        Commands::Evolve { action } => match action {
            EvolveAction::Propose {
                target,
                hypothesis,
                baseline_id,
                candidate_id,
                changed_scope,
                rationale,
                generated_by,
                reversible_save_id,
                reference_ids,
                studio_ids,
                task_id,
                session_id,
                context_hash,
                harness,
            } => commands::evolve_propose(
                target,
                hypothesis,
                baseline_id,
                candidate_id,
                changed_scope,
                rationale,
                generated_by,
                reversible_save_id,
                reference_ids,
                studio_ids,
                task_id,
                session_id,
                context_hash,
                harness,
            ),
            EvolveAction::Show { id, explain, json } => commands::evolve_show(id, explain, json),
            EvolveAction::Evaluate { id, json, metric } => {
                commands::evolve_evaluate(id, json, metric)
            }
            EvolveAction::Promote { id, label } => commands::evolve_promote(id, label),
            EvolveAction::Reject { id, reason } => commands::evolve_reject(id, reason),
            EvolveAction::History => commands::evolve_history(),
            EvolveAction::Explain { id, json } => commands::evolve_explain(id, json),
            EvolveAction::Campaign { action } => match action {
                CampaignAction::Create {
                    goal,
                    scope,
                    strategy,
                    task_id,
                } => commands::campaign_create(goal, scope, strategy, task_id),
                CampaignAction::Status { id, json } => commands::campaign_status(id, json),
                CampaignAction::Next { id, json, cap } => commands::campaign_next(id, json, cap),
                CampaignAction::Report {
                    id,
                    json,
                    explain,
                    outcome,
                } => commands::campaign_report(id, json, explain, outcome),
            },
        },
        Commands::StudyApply { candidate_id } => commands::study_apply(candidate_id),
        Commands::SelfImprove => commands::self_improve(),
        Commands::SelfArchive { action } => match action {
            SelfArchiveAction::Archive {
                from,
                message,
                route_version,
            } => {
                let from = from.unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                });
                commands::self_archive_archive(&from, &message, &route_version)
            }
            SelfArchiveAction::List => commands::self_archive_list(),
            SelfArchiveAction::Show { seq, json } => commands::self_archive_show(seq, json),
            SelfArchiveAction::Apply { seq, out } => commands::self_archive_apply(seq, out),
        },
        Commands::SelfEvolve => commands::self_evolve(),
        Commands::Task { action } => match action {
            TaskAction::Begin {
                task,
                target,
                concurrent,
                profile,
                workflow,
            } => commands::task_begin(task, target, concurrent, profile, workflow),
            TaskAction::Status { id } => commands::task_status(id),
            TaskAction::Exec {
                session,
                argv,
                check_id,
            } => commands::task_exec(session, argv, check_id),
            TaskAction::End { id, result, force } => {
                if force {
                    commands::task_force_end(id)
                } else {
                    commands::task_end(id, result)
                }
            }
            TaskAction::Show { id, explain } => commands::task_show(id, explain),
            TaskAction::Verify { id, json } => commands::task_verify(id, json),
            TaskAction::Start {
                task,
                target,
                profile,
                workflow,
                strategy,
                savepoint,
                campaign_id,
            } => commands::task_start(
                task,
                target,
                profile,
                workflow,
                strategy,
                savepoint,
                campaign_id,
            ),
            TaskAction::Resume { id } => commands::task_resume(id),
            TaskAction::Report { id, observations } => commands::task_report(id, observations),
            TaskAction::Replay { id, target, stdout } => commands::task_replay(id, target, stdout),
            TaskAction::InspectResources { id } => commands::task_inspect_resources(id),
        },
        Commands::Memory { action } => match action {
            MemoryAction::Show => commands::memory_show(),
            MemoryAction::History => commands::memory_history(),
            MemoryAction::Refresh => commands::memory_refresh(),
            MemoryAction::Apply { index } => commands::memory_apply(index),
            MemoryAction::Why { topic } => commands::memory_why(topic),
            MemoryAction::Supersede { old, new } => commands::memory_supersede(old, new),
            MemoryAction::Map { topic } => commands::memory_map(topic),
        },
        Commands::Strategy { action } => match action {
            StrategyAction::Record { label, description } => {
                commands::strategy_record(label, description)
            }
            StrategyAction::List => commands::strategy_list(),
            StrategyAction::Show { id } => commands::strategy_show(id),
            StrategyAction::Diff { a, b } => commands::strategy_diff(a, b),
            StrategyAction::Restore { id } => commands::strategy_restore(id),
            StrategyAction::History => commands::strategy_history(),
            StrategyAction::Fork { name, from } => commands::strategy_fork(name, from),
            StrategyAction::Use { id } => commands::strategy_use(id),
            StrategyAction::Compare { a, b } => commands::strategy_compare(a, b),
            StrategyAction::Delete { id } => commands::strategy_delete(id),
        },
        Commands::Experiment { action } => match action {
            ExperimentAction::Stats => commands::experiment_stats(),
            ExperimentAction::Suggest { task, strategies } => {
                commands::experiment_suggest(task, strategies)
            }
            ExperimentAction::History => commands::experiment_history(),
            ExperimentAction::Show { strategy } => commands::experiment_show(strategy),
        },
        Commands::AgentOrg { action } => match action {
            AgentOrgAction::History => commands::agent_org_history(),
            AgentOrgAction::Explain { id } => commands::agent_org_explain(id),
            AgentOrgAction::Recall { pattern } => commands::agent_org_recall(pattern),
        },
        Commands::Pattern { action } => match action {
            PatternAction::List => commands::pattern_list(),
            PatternAction::Show { id } => commands::pattern_show(id),
            PatternAction::Propose {
                from_study,
                name,
                problem,
                solution,
            } => commands::pattern_propose(from_study, name, problem, solution),
            PatternAction::Apply { id } => commands::pattern_apply(id),
            PatternAction::Proposals => commands::pattern_proposals(),
        },
        Commands::Save { action } => match action {
            SaveAction::Create { name, description } => {
                commands::savepoint_create(name, description)
            }
            SaveAction::List => commands::savepoint_list(),
            SaveAction::Show { id } => commands::savepoint_show(id),
            SaveAction::Preview { id, scope } => commands::savepoint_preview(id, scope),
            SaveAction::Restore { id, scope, force } => {
                commands::savepoint_restore(id, scope, force)
            }
            SaveAction::Diff { a, b } => commands::savepoint_diff(a, b),
            SaveAction::Delete { id } => commands::savepoint_delete(id),
        },
        Commands::Capability { action } => match action {
            CapabilityAction::List { kind } => commands::capability_list(kind),
            CapabilityAction::Show { id } => commands::capability_show(id),
            CapabilityAction::Inspect { id } => commands::capability_inspect(id),
        },
        Commands::Trajectory { action } => match action {
            TrajectoryAction::List => commands::trajectory_list(),
            TrajectoryAction::Show { id } => commands::trajectory_show(id),
            TrajectoryAction::Diff { a, b } => commands::trajectory_diff(a, b),
            TrajectoryAction::Analyze { id } => commands::trajectory_analyze(id),
        },
        Commands::Discover { path } => commands::discover(path),
        Commands::Pack { action } => match action {
            PackAction::Create {
                name,
                description,
                workflows,
                skills,
                tools,
                refs,
                capabilities,
                strategy,
                tags,
            } => commands::pack_create(
                name,
                description,
                workflows,
                skills,
                tools,
                refs,
                capabilities,
                strategy,
                tags,
            ),
            PackAction::List => commands::pack_list(),
            PackAction::Show { id } => commands::pack_show(id),
            PackAction::Use { id } => commands::pack_use(id),
            PackAction::Deactivate { id } => commands::pack_deactivate(id),
            PackAction::Export { id } => commands::pack_export(id),
            PackAction::Import { path } => commands::pack_import(path),
            PackAction::Delete { id } => commands::pack_delete(id),
        },
        Commands::Idea { action } => match action {
            IdeaAction::Add {
                text,
                source,
                task,
                tags,
            } => commands::idea_add(text, source, task, tags),
            IdeaAction::List { status } => commands::idea_list(status),
            IdeaAction::Show { id } => commands::idea_show(id),
            IdeaAction::Accept { id } => commands::idea_accept(id),
            IdeaAction::Reject { id } => commands::idea_reject(id),
            IdeaAction::Implemented { id } => commands::idea_implemented(id),
            IdeaAction::Delete { id } => commands::idea_delete(id),
        },
        Commands::Failure { action } => match action {
            FailureAction::List { severity } => commands::failure_list(severity),
            FailureAction::Show { id } => commands::failure_show(id),
            FailureAction::Search { query } => commands::failure_search(query),
            FailureAction::Resolve { id, resolution } => commands::failure_resolve(id, resolution),
            FailureAction::Delete { id } => commands::failure_delete(id),
            FailureAction::Add {
                problem,
                attempt,
                symptom,
                cause,
                scope,
                tags,
                severity,
            } => commands::failure_add(problem, attempt, symptom, cause, scope, tags, severity),
        },
        Commands::Principle { action } => match action {
            PrincipleAction::List { status } => commands::principle_list(status),
            PrincipleAction::Show { id } => commands::principle_show(id),
            PrincipleAction::Apply { id } => commands::principle_apply(id),
            PrincipleAction::Promote { id } => commands::principle_promote(id),
            PrincipleAction::Reject { id } => commands::principle_reject(id),
        },
        Commands::Agents { action } => match action {
            AgentAction::Templates { pattern } => commands::agent_templates(pattern),
            AgentAction::Show { id } => commands::agent_show(id),
            AgentAction::RecordSuccess { id, session } => {
                commands::agent_record_success(id, session)
            }
            AgentAction::RecordFailure { id, session } => {
                commands::agent_record_failure(id, session)
            }
        },
        Commands::Impact { change } => commands::impact(change),
        Commands::Health { explain } => commands::health(explain),
        Commands::HealthHistory => commands::health_history(),
        Commands::Guardian { action } => match action {
            GuardianAction::Scan => commands::guardian_scan(),
            GuardianAction::Findings { status } => commands::guardian_findings(status),
            GuardianAction::Show { id } => commands::guardian_show(id),
            GuardianAction::Resolve { id } => commands::guardian_resolve(id),
            GuardianAction::Ignore { id } => commands::guardian_ignore(id),
        },
        Commands::Next {
            limit,
            start,
            target,
        } => {
            if let Some(proposal_id) = start {
                commands::next_start(proposal_id, target)
            } else {
                commands::next(limit)
            }
        }
        Commands::Goal { action } => match action {
            GoalAction::Add {
                title,
                description,
                priority,
                criteria,
                tags,
            } => commands::goal_add(title, description, priority, criteria, tags),
            GoalAction::List { status } => commands::goal_list(status),
            GoalAction::Show { id } => commands::goal_show(id),
            GoalAction::Update { id, status } => commands::goal_update(id, status),
            GoalAction::Delete { id } => commands::goal_delete(id),
        },
        Commands::Archive { action } => match action {
            ArchiveAction::Init => commands::archive_init(),
            ArchiveAction::Save { reason } => commands::archive_save(reason),
            ArchiveAction::List { json } => commands::archive_list(json),
            ArchiveAction::Show { id, json } => commands::archive_show(id, json),
            ArchiveAction::Diff { a, b } => commands::archive_diff(a, b),
            ArchiveAction::Restore { id, scope, paths } => {
                commands::archive_restore(id, scope, paths)
            }
            ArchiveAction::Recover {
                project_id,
                save,
                to,
            } => commands::archive_recover(project_id, save, to),
            ArchiveAction::RecoverList { project_id } => commands::archive_recover_list(project_id),
            ArchiveAction::Check => commands::archive_check(),
            ArchiveAction::PathHistory { path } => commands::archive_path_history(path),
            ArchiveAction::Delete { id, force } => commands::archive_delete(id, force),
            ArchiveAction::DeleteProject { project_id, force } => {
                commands::archive_delete_project(project_id, force)
            }
        },
        Commands::Brain { action } => match action {
            BrainAction::Show => commands::brain_show(),
            BrainAction::Brief => commands::brain_brief(),
            BrainAction::Refresh => commands::brain_refresh(),
            BrainAction::Apply { index } => commands::brain_apply(index),
            BrainAction::History => commands::brain_history(),
            BrainAction::Explain { id } => commands::brain_explain(id),
            BrainAction::For { task } => commands::brain_for(task),
            BrainAction::Expand { id } => commands::brain_expand(id),
            BrainAction::Conflicts => commands::brain_conflicts(),
            BrainAction::Compact => commands::brain_compact(),
            BrainAction::Doctor => commands::brain_doctor(),
            BrainAction::Diff { a, b } => commands::brain_diff(a, b),
        },
        Commands::Roadmap => commands::roadmap(),
        Commands::Loops => commands::loops(),
        Commands::Drift => commands::drift(),
        Commands::Maintain {
            list,
            show,
            plan,
            start,
            check,
            target,
        } => {
            if let Some(plan_id) = show {
                commands::maintain_show(plan_id)
            } else if let Some(plan_id) = start {
                commands::maintain_start(plan_id, target)
            } else if check {
                commands::maintain_check()
            } else if list {
                commands::maintain_list()
            } else if plan {
                commands::maintain_plan()
            } else {
                commands::maintain_check()
            }
        }
        Commands::Brief { task } => commands::brief(task),
        Commands::Handoff => commands::handoff(),
    };

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            eprintln!("{}", commands::format_user_error(&e));
            process::exit(1);
        }
    }
}
