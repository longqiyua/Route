// ---------------------------------------------------------------------------
// Types matching Rust DTOs
// ---------------------------------------------------------------------------
//
// We deliberately do NOT import `invoke` from `@tauri-apps/api/core`
// here. All call sites go through `./ipc::safeInvoke` (re-exported
// below) so the bridge is waited for and any "missing IPC" failure
// surfaces as a clean, user-readable error.

export interface BranchDto {
  id: string;
  name: string;
  kind: "main" | "inherited" | "sandbox";
  parent_branch: string | null;
  baseline_snapshot: string | null;
  head_snapshot: string | null;
  created_at: number;
}

export interface SnapshotDto {
  id: string;
  short_id: string;
  manifest_hash: string;
  created_at: number;
  branch_id: string | null;
  branch_name: string | null;
}

export interface CommitDto {
  id: string;
  short_id: string;
  from_snapshot: string;
  to_snapshot: string;
  message: string;
  author: string | null;
  created_at: number;
  branch_id: string;
  branch_name: string;
  kind: "incremental" | "full" | "merge" | "rollback";
  diff_summary: { added: string[]; modified: string[]; removed: string[] } | null;
  /// Who performed the change. "user" by default; "ai:<name>" for AI-driven commits.
  operator: string | null;
  /// Optional long-form note from the operator (used for checkpoint body / AI prompt).
  body: string | null;
  /// True if this commit is a user-marked checkpoint.
  is_checkpoint: boolean;
  /// True if the change came from the AI control channel (CLI / MCP).
  is_ai: boolean;
}

export interface StatusDto {
  project_path: string;
  mode: string;
  current_branch: string;
  branches: BranchDto[];
  latest_snapshot: SnapshotDto | null;
}

export interface TreeNode {
  id: string;
  label: string;
  kind: "root" | "branch" | "snapshot";
  branch_kind?: string;
  snapshot_id?: string;
  commit_id?: string;
  message?: string;
  annotations: string[];
  diff?: string;
  children: TreeNode[];
}

// ---------------------------------------------------------------------------
// API wrappers
//
// We go through `safeInvoke` (see `./ipc`) instead of calling
// `@tauri-apps/api/core::invoke` directly. The bridge can briefly be
// absent during HMR reloads and right after a Tauri host restart, so
// we wait for it to appear and throw a clean error otherwise —
// rather than the cryptic "Cannot read properties of undefined
// (reading 'invoke')" the user was seeing.
// ---------------------------------------------------------------------------

export { isTauriBridgeAvailable, safeInvoke } from "./ipc";
import { safeInvoke } from "./ipc";

export async function pickFolder(): Promise<string | null> {
  return safeInvoke<string | null>("pick_folder");
}

export async function initRepo(path: string): Promise<StatusDto> {
  return safeInvoke<StatusDto>("init_repo", { path });
}

export async function openRepo(path: string): Promise<StatusDto> {
  return safeInvoke<StatusDto>("open_repo", { path });
}

/// Seed a fully-populated example project under `target_dir` and open it
/// as the active repo. Used by the "Try example project" button on the
/// welcome page so the user can see the full feature surface at a glance.
export async function seedExample(targetDir: string): Promise<StatusDto> {
  return safeInvoke<StatusDto>("seed_example", { targetDir });
}

/// Create a brand-new, empty Route workspace in a freshly minted
/// subfolder under the platform's "Route demos" location and open it
/// as the active repo. Used by the "Enter demo mode" button on the
/// welcome page — gives the user a private sandbox where they can
/// poke at the software without touching any real project.
///
/// Return a platform-appropriate directory for the "Try example" flow.
/// The directory is created on disk if missing, so the caller can
/// immediately pass it to `seedExample`.
export async function defaultSeedDir(): Promise<string> {
  return safeInvoke<string>("default_seed_dir");
}

export async function status(): Promise<StatusDto> {
  return safeInvoke<StatusDto>("status");
}

export async function commit(
  message: string,
  author: string | null,
  full: boolean,
  branch: string | null
): Promise<CommitDto> {
  return safeInvoke<CommitDto>("commit", { message, author, full, branch });
}

export async function logCommits(limit: number, branch: string | null): Promise<CommitDto[]> {
  return safeInvoke<CommitDto[]>("log", { limit, branch });
}

export async function rollback(snapshotId: string, reason: string | null): Promise<CommitDto> {
  return safeInvoke<CommitDto>("rollback", { snapshotId, reason });
}

export async function backupToDir(target: string): Promise<string> {
  return safeInvoke<string>("backup_to_dir", { target });
}

export async function branchList(): Promise<BranchDto[]> {
  return safeInvoke<BranchDto[]>("branch_list");
}

export async function branchCreate(
  name: string,
  kind: "main" | "inherited" | "sandbox",
  from: string | null
): Promise<BranchDto> {
  return safeInvoke<BranchDto>("branch_create", { name, kind, from });
}

export async function branchDelete(name: string): Promise<void> {
  return safeInvoke<void>("branch_delete", { name });
}

export async function branchSwitch(name: string): Promise<void> {
  return safeInvoke<void>("branch_switch", { name });
}

/// Merge `source` branch into `target` (defaults to the current branch).
/// route_basic performs a file-level 3-way merge for inherited branches
/// and a 2-way copy for sandbox branches. Sandbox branches cannot be a
/// merge *target* — call `branchCreate` to copy a sandbox's content into
/// a new inherited branch instead.
export async function branchMerge(
  source: string,
  target?: string | null,
): Promise<CommitDto> {
  return safeInvoke<CommitDto>("branch_merge", {
    source,
    target: target ?? null,
  });
}

export async function annotate(commitId: string, text: string): Promise<void> {
  return safeInvoke<void>("annotate", { commitId, text });
}

export async function listAnnotations(commitId: string): Promise<string[]> {
  const list = await safeInvoke<{ id: string; text: string }[]>("list_annotations", { commitId });
  return list.map((a) => a.text);
}

export async function exportData(format: string): Promise<string> {
  return safeInvoke<string>("export_data", { format });
}

/// Export to a file/directory on disk (for ZIP and Folder formats).
export async function exportFile(format: "zip" | "folder", path: string): Promise<string> {
  return safeInvoke<string>("export_file", { format, path });
}

export async function stats(): Promise<Record<string, unknown>> {
  return safeInvoke<Record<string, unknown>>("stats");
}

export async function historyTree(): Promise<TreeNode> {
  return safeInvoke<TreeNode>("history_tree");
}

// ---------------------------------------------------------------------------
// Sync API (route-sync integration)
// ---------------------------------------------------------------------------

export interface ArchiveRuleDto {
  folder_format: string;
  max_snapshots: number | null;
  max_age_days: number | null;
}

/// Transport type — remote transports (webdav, s3, ssh) require credentials.
export type SyncTransport = "local" | "relay" | "server" | "p2p" | "webdav" | "s3" | "ssh";

/// DTO for remote credentials returned from the backend. Secrets are exposed
/// only as `*_set` flags — actual values never leave the Rust side.
export interface CredentialsDto {
  url: string;
  username: string | null;
  password_set: boolean;
  access_key: string | null;
  secret_key_set: boolean;
  bucket: string | null;
  region: string | null;
}

export interface SyncTargetDto {
  name: string;
  source: string;
  destination: string;
  mode: "mirror" | "backup" | "archive";
  transport: SyncTransport;
  conflict: "keep_both" | "skip" | "overwrite";
  archive_rule: ArchiveRuleDto;
  ignore_patterns: string[];
  enabled: boolean;
  credentials: CredentialsDto | null;
}

export interface SyncStatsDto {
  files_scanned: number;
  files_copied: number;
  files_skipped: number;
  files_deleted: number;
  conflicts_resolved: number;
  errors: number;
  bytes_copied: number;
}

export interface SyncResultDto {
  target_name: string;
  mode: string;
  stats: SyncStatsDto;
  timestamp: number;
  error: string | null;
}

export interface SyncStatusDto {
  scheduler_running: boolean;
  targets_count: number;
  enabled_count: number;
}

export interface SyncAddParams {
  name: string;
  source: string;
  destination: string;
  mode: "mirror" | "backup" | "archive";
  transport?: SyncTransport;
  conflict?: "keep_both" | "skip" | "overwrite";
  max_snapshots?: number;
  max_age_days?: number;
  folder_format?: string;
  ignore_patterns?: string[];
  enabled?: boolean;
  // Remote credentials (only used for webdav/s3 transports):
  url?: string;
  username?: string;
  password?: string;
  access_key?: string;
  secret_key?: string;
  bucket?: string;
  region?: string;
}

export async function syncList(): Promise<SyncTargetDto[]> {
  return safeInvoke<SyncTargetDto[]>("sync_list");
}

export async function syncShow(name: string): Promise<SyncTargetDto> {
  return safeInvoke<SyncTargetDto>("sync_show", { name });
}

export async function syncAdd(params: SyncAddParams): Promise<SyncTargetDto> {
  return safeInvoke<SyncTargetDto>("sync_add", { ...params });
}

export async function syncRemove(name: string): Promise<void> {
  return safeInvoke<void>("sync_remove", { name });
}

export async function syncSetEnabled(name: string, enabled: boolean): Promise<void> {
  return safeInvoke<void>("sync_set_enabled", { name, enabled });
}

export async function syncRun(name?: string): Promise<SyncResultDto[]> {
  return safeInvoke<SyncResultDto[]>("sync_run", { name: name ?? null });
}

export async function syncStart(intervalSecs: number): Promise<void> {
  return safeInvoke<void>("sync_start", { intervalSecs });
}

export async function syncStop(): Promise<void> {
  return safeInvoke<void>("sync_stop");
}

export async function syncStatus(): Promise<SyncStatusDto> {
  return safeInvoke<SyncStatusDto>("sync_status");
}

// ---------------------------------------------------------------------------
// Stats API (route-stats integration)
// ---------------------------------------------------------------------------

export interface StatsRangeDto {
  from: number;
  to: number;
}

export interface StatsParams {
  range?: StatsRangeDto | null;
  top_files_limit?: number;
  hourly_buckets?: number;
  daily_buckets?: number;
}

export interface SummaryStats {
  branch_count: number;
  snapshot_count: number;
  commit_count: number;
  annotation_count: number;
  first_commit_at: number | null;
  latest_commit_at: number | null;
  active_days: number;
}

export interface BranchStats {
  name: string;
  kind: string;
  commit_count: number;
  latest_commit_at: number | null;
  head_snapshot: string | null;
}

export type TimelineKind = "hourly" | "daily";

export interface TimelineBucket {
  ts: number;
  kind: TimelineKind;
  count: number;
}

export interface FileStat {
  path: string;
  modifications: number;
  last_seen_at: number;
}

export interface StorageStats {
  blob_count: number;
  total_blob_size: number;
  manifest_count: number;
  logical_size: number;
  dedup_ratio: number;
}

export interface RepoStats {
  project_path: string;
  collected_at: number;
  summary: SummaryStats;
  branches: BranchStats[];
  commits_by_kind: Record<string, number>;
  commits_by_hour: TimelineBucket[];
  commits_by_day: TimelineBucket[];
  top_files: FileStat[];
  storage: StorageStats;
  range: StatsRangeDto | null;
}

export async function statsCollect(params?: StatsParams | null): Promise<RepoStats> {
  return safeInvoke<RepoStats>("stats_collect", { params: params ?? null });
}

export async function statsMarkdown(params?: StatsParams | null): Promise<string> {
  return safeInvoke<string>("stats_markdown", { params: params ?? null });
}

export async function statsJson(params?: StatsParams | null): Promise<string> {
  return safeInvoke<string>("stats_json", { params: params ?? null });
}

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

export interface PluginEntryDto {
  name: string;
  enabled: boolean;
  /** Free-form JSON config. `null` if the plugin has no config. */
  config: unknown;
}

export async function pluginList(): Promise<PluginEntryDto[]> {
  return safeInvoke<PluginEntryDto[]>("plugin_list");
}

export async function pluginShow(name: string): Promise<PluginEntryDto> {
  return safeInvoke<PluginEntryDto>("plugin_show", { name });
}

export async function pluginInstall(
  name: string,
  config?: string | null,
): Promise<PluginEntryDto> {
  return safeInvoke<PluginEntryDto>("plugin_install", {
    name,
    config: config ?? null,
  });
}

export async function pluginRemove(name: string): Promise<void> {
  return safeInvoke<void>("plugin_remove", { name });
}

export async function pluginSetEnabled(name: string, enabled: boolean): Promise<void> {
  return safeInvoke<void>("plugin_set_enabled", { name, enabled });
}

export async function pluginBuiltins(): Promise<string[]> {
  return safeInvoke<string[]>("plugin_builtins");
}

// ---------------------------------------------------------------------------
// Autostart — boot launch, silent start, startup priority
// ---------------------------------------------------------------------------

export interface AutostartConfig {
  enabled: boolean;
  silent: boolean;
  priority: "low" | "normal" | "high";
  close_behavior: "quit" | "hide";
}

export async function autostartGet(): Promise<AutostartConfig> {
  return safeInvoke<AutostartConfig>("autostart_get");
}

export async function autostartSet(params: {
  enabled?: boolean;
  silent?: boolean;
  priority?: "low" | "normal" | "high";
  close_behavior?: "quit" | "hide";
}): Promise<AutostartConfig> {
  return safeInvoke<AutostartConfig>("autostart_set", { ...params });
}

export async function autostartWasSilent(): Promise<boolean> {
  return safeInvoke<boolean>("autostart_was_silent");
}

/// Show the main window and focus it. Paired with the tray "显示" item.
export async function showWindow(): Promise<void> {
  return safeInvoke<void>("show_window");
}

/// Hide the main window. The process keeps running in the background.
export async function hideWindow(): Promise<void> {
  return safeInvoke<void>("hide_window");
}

// ---------------------------------------------------------------------------
// File history & commit diff detail
// ---------------------------------------------------------------------------

export interface FileRevisionDto {
  snapshot_id: string;
  snapshot_short_id: string;
  commit_id: string | null;
  commit_message: string | null;
  blob_hash: string | null;
  created_at: number;
  branch_name: string | null;
}

export interface CommitDiffEntryDto {
  path: string;
  /** "added" | "modified" | "removed" */
  change: string;
  blob_hash: string | null;
  size_bytes: number | null;
}

export async function fileHistory(path: string): Promise<FileRevisionDto[]> {
  return safeInvoke<FileRevisionDto[]>("file_history", { path });
}

export async function restoreFile(
  snapshotId: string,
  path: string,
): Promise<string> {
  return safeInvoke<string>("restore_file", {
    snapshotId,
    path,
  });
}

export async function commitDiffDetail(
  commitId: string,
): Promise<CommitDiffEntryDto[]> {
  return safeInvoke<CommitDiffEntryDto[]>("commit_diff_detail", {
    commitId,
  });
}

// ---------------------------------------------------------------------------
// Working directory status, snapshot diff, tags
// ---------------------------------------------------------------------------

export interface WorkingFileStatusDto {
  path: string;
  /** "added" | "modified" | "removed" */
  change: string;
  current_hash: string | null;
  previous_hash: string | null;
  size_bytes: number | null;
}

export interface SnapshotDiffEntryDto {
  path: string;
  /** "added" | "modified" | "removed" */
  change: string;
  from_hash: string | null;
  to_hash: string | null;
}

export interface TagDto {
  id: string;
  name: string;
  snapshot_id: string;
  snapshot_short_id: string;
  message: string | null;
  created_at: number;
}

export async function workingDirStatus(): Promise<WorkingFileStatusDto[]> {
  return safeInvoke<WorkingFileStatusDto[]>("working_dir_status");
}

export async function diffSnapshots(
  fromSnapshot: string,
  toSnapshot: string,
): Promise<SnapshotDiffEntryDto[]> {
  return safeInvoke<SnapshotDiffEntryDto[]>("diff_snapshots", {
    fromSnapshot,
    toSnapshot,
  });
}

export async function tagList(): Promise<TagDto[]> {
  return safeInvoke<TagDto[]>("tag_list");
}

export async function tagCreate(
  name: string,
  snapshotId: string,
  message: string | null,
): Promise<TagDto> {
  return safeInvoke<TagDto>("tag_create", { name, snapshotId, message });
}

export async function tagDelete(name: string): Promise<void> {
  return safeInvoke<void>("tag_delete", { name });
}

// ---------------------------------------------------------------------------
// Checkpoint, auto-tracking, AI control
// ---------------------------------------------------------------------------

/// A user-marked checkpoint with title and body.
export interface CheckpointDto {
  id: string;
  title: string;
  body: string | null;
  commit_id: string;
  created_at: number;
}

export async function checkpointCreate(
  title: string,
  body: string | null,
): Promise<CommitDto> {
  return safeInvoke<CommitDto>("checkpoint_create", { title, body: body ?? null });
}

export async function checkpointList(): Promise<CheckpointDto[]> {
  return safeInvoke<CheckpointDto[]>("checkpoint_list");
}

export async function checkpointDelete(id: string): Promise<void> {
  return safeInvoke<void>("checkpoint_delete", { id });
}

/// Start background file watching + auto-commit (debounced).
export async function watchStart(debounceMs?: number): Promise<void> {
  return safeInvoke<void>("watch_start", { debounceMs: debounceMs ?? null });
}

export async function watchStop(): Promise<void> {
  return safeInvoke<void>("watch_stop");
}

/// Force the watcher to flush any pending changes immediately. Bound
/// to the workspace "Run" button so the user can persist in-flight
/// edits without waiting for the memory-buffer window.
export async function watchFlush(): Promise<WatchStatusDto> {
  return safeInvoke<WatchStatusDto>("watch_flush");
}

export interface WatchStatusDto {
  running: boolean;
  path: string | null;
  debounce_ms: number;
  pending_changes: number;
}

export async function watchStatus(): Promise<WatchStatusDto> {
  return safeInvoke<WatchStatusDto>("watch_status");
}

/// Undo / Redo: each undo creates a rollback commit; that commit itself can
/// be undone (rolled forward) — the stack is therefore unbounded.
export async function undoLast(): Promise<CommitDto> {
  return safeInvoke<CommitDto>("undo_last");
}

export async function redoLast(): Promise<CommitDto> {
  return safeInvoke<CommitDto>("redo_last");
}

/// Returns true if the in-memory redo stack has at least one entry the
/// user can roll forward to. Cheaper than calling `redoLast` (which
/// mutates state).
export async function canRedo(): Promise<boolean> {
  return safeInvoke<boolean>("can_redo");
}

/// Register an AI operator name. AI commits will record this name and the
/// prompt that triggered the change. Returns the registered operator.
export async function setAiOperator(
  name: string,
  prompt: string,
): Promise<AiOperatorDto> {
  return safeInvoke<AiOperatorDto>("set_ai_operator", { name, prompt });
}

export async function clearAiOperator(): Promise<void> {
  return safeInvoke<void>("clear_ai_operator");
}

export interface AiOperatorDto {
  name: string;
  prompt: string;
  since_ms: number;
}

// ---------------------------------------------------------------------------
// Git mode — Route drives the user's own `git` binary so checkpoints become
// real git commits and branches become real git branches. The backend ONLY
// ever calls `git add` + `git commit` + `git branch` + `git log`. Push and
// remote operations are intentionally NOT exposed — those stay the user's
// job. When git mode is OFF, the app falls back to the built-in route_basic
// version control and these functions are not called.
// ---------------------------------------------------------------------------

/// Result of `git_detect` — tells the settings page whether `git` is on
/// PATH and usable before the user commits to enabling git mode.
export interface GitDetectDto {
  available: boolean;
  /// `git version` output, e.g. "git version 2.43.0". Empty when unavailable.
  version: string;
  /// Human-readable reason when unavailable.
  error: string;
}

/// One row of the git log, shaped to match what the timeline needs.
export interface GitLogEntryDto {
  /// Full 40-char SHA — used as the unique id.
  sha: string;
  /// First-line commit subject.
  message: string;
  /// Author commit timestamp in milliseconds since epoch.
  timestamp_ms: number;
  /// True if the commit carries the `Route-Checkpoint` trailer (created by
  /// the workbench's 打点 button rather than a manual git commit).
  is_checkpoint: boolean;
  /// True if the commit carries `Route-Operator: ai:<name>` (AI-driven).
  is_ai: boolean;
}

/// One local branch. Remote-tracking refs are deliberately not listed.
export interface GitBranchDto {
  name: string;
  current: boolean;
}

/// Working-tree status grouped by change kind. The Git mode counterpart of
/// route_basic's `working_dir_status` (which returns a flat `[{path, change}]`
/// list). The frontend normalizes both into the same `{change, path}` tuples
/// before building the AI commit-message prompt.
export interface GitStatusDto {
  added: string[];
  modified: string[];
  removed: string[];
  untracked: string[];
}

/// Detect whether `git` is installed and runnable. Does not require a
/// project to be open — the settings page calls this on toggle.
export async function gitDetect(): Promise<GitDetectDto> {
  return safeInvoke<GitDetectDto>("git_detect");
}

/// Toggle git mode on the backend so the file watcher commits via git
/// instead of route_basic. Syncs the localStorage flag to the Rust side.
export async function gitModeSet(enabled: boolean): Promise<boolean> {
  return safeInvoke<boolean>("git_mode_set", { enabled });
}

/// Initialize git in the current project (idempotent). Called when the
/// user enables git mode so the first checkpoint doesn't fail.
export async function gitInit(): Promise<string> {
  return safeInvoke<string>("git_init");
}

/// Stage all changes and create a git commit. This is the "打点"
/// (checkpoint) action when git mode is on. Returns the just-created
/// commit as a log entry so the UI can prepend it without a refetch.
export async function gitCommit(
  title: string,
  body: string | null,
): Promise<GitLogEntryDto> {
  return safeInvoke<GitLogEntryDto>("git_commit", { title, body });
}

/// Roll the working tree back to the state of a past commit `<sha>`,
/// recorded as a NEW commit on top of HEAD (non-destructive — commits
/// after `<sha>` stay in history). The Git mode counterpart of route_basic's
/// `rollback`. Returns the just-created rollback commit as a log entry so
/// the timeline can prepend it without a refetch. `<sha>` is the full git
/// SHA the timeline already carries as each entry's `to_snapshot`.
export async function gitRollback(sha: string): Promise<GitLogEntryDto> {
  return safeInvoke<GitLogEntryDto>("git_rollback", { sha });
}

/// Read the git log as a list of timeline entries, newest first. When
/// `branch` is provided, only that branch's log is read (used by the
/// branch tree to render each lane independently).
export async function gitLog(
  limit: number = 200,
  branch?: string | null,
): Promise<GitLogEntryDto[]> {
  return safeInvoke<GitLogEntryDto[]>("git_log", {
    limit,
    branch: branch ?? null,
  });
}

/// Merge `source` into the current branch. Runs `git merge --no-edit --no-ff`.
/// Push / remote operations are intentionally NOT exposed — the user does
/// those themselves outside the app.
export async function gitMerge(source: string): Promise<string> {
  return safeInvoke<string>("git_merge", { source });
}

/// List local branches.
export async function gitBranchList(): Promise<GitBranchDto[]> {
  return safeInvoke<GitBranchDto[]>("git_branch_list");
}

/// Create a new branch at HEAD. Does not switch to it.
export async function gitBranchCreate(name: string): Promise<GitBranchDto> {
  return safeInvoke<GitBranchDto>("git_branch_create", { name });
}

/// Switch the working tree to a different local branch.
export async function gitBranchSwitch(name: string): Promise<void> {
  return safeInvoke<void>("git_branch_switch", { name });
}

/// Current branch name. Empty when the project is not a git repo.
export async function gitCurrentBranch(): Promise<string> {
  return safeInvoke<string>("git_current_branch");
}

/// Read the working-tree status grouped by change kind. The Git mode
/// counterpart of `workingDirStatus`. Returns an empty DTO (not an error)
/// when the project is not a git repo yet.
export async function gitStatus(): Promise<GitStatusDto> {
  return safeInvoke<GitStatusDto>("git_status");
}

/// Return the unified diff of the working tree against HEAD — all unstaged
/// AND staged uncommitted changes as a raw string. Empty when there is
/// nothing to diff or the project is not a git repo. Used for richer AI
/// commit-message prompts that benefit from seeing actual hunks.
export async function gitDiff(): Promise<string> {
  return safeInvoke<string>("git_diff");
}

// ---------------------------------------------------------------------------
// Stash / tag / restore — local-only Git primitives (no network). These
// mirror route-tui's coverage so the GUI backend matches the CLI/TUI.
// Destructive `reset --hard` / `revert` are intentionally NOT exposed;
// `gitRollback` covers the "restore to a past state" semantic
// non-destructively, matching the "low floor / maximum protection" default.
// ---------------------------------------------------------------------------

/// List stash entries as raw `stash@{n}: ...` strings. Empty when there are
/// no stashes or the project isn't a git repo.
export async function gitStashList(): Promise<string[]> {
  return safeInvoke<string[]>("git_stash_list");
}

/// Push a new stash with an optional message. Returns git's output (usually
/// empty). "Nothing to stash" is a no-op success (returns an empty string).
export async function gitStashPush(message?: string | null): Promise<string> {
  return safeInvoke<string>("git_stash_push", { message: message ?? null });
}

/// Pop the top stash. Throws git's conflict output if the pop hits conflicts
/// (the stash is kept by git in that case, so the user can retry).
export async function gitStashPop(): Promise<string> {
  return safeInvoke<string>("git_stash_pop");
}

/// List all tags, sorted as git sorts them. Empty when none.
export async function gitTagList(): Promise<string[]> {
  return safeInvoke<string[]>("git_tag_list");
}

/// Create a tag at HEAD. Annotated (`-a -m`) when `message` is given,
/// lightweight otherwise.
export async function gitTagCreate(
  name: string,
  message?: string | null,
): Promise<void> {
  return safeInvoke<void>("git_tag_create", { name, message: message ?? null });
}

/// Delete a tag. Throws if the tag doesn't exist.
export async function gitTagDelete(name: string): Promise<void> {
  return safeInvoke<void>("git_tag_delete", { name });
}

/// Restore working-tree files from the index (`git restore <paths>`),
/// discarding uncommitted modifications to those tracked files. Untracked
/// files are not affected (they're not in the index).
export async function gitRestore(paths: string[]): Promise<void> {
  return safeInvoke<void>("git_restore", { paths });
}

export async function getAiOperator(): Promise<AiOperatorDto | null> {
  return safeInvoke<AiOperatorDto | null>("get_ai_operator");
}

// ---------------------------------------------------------------------------
// Track configuration + AI summary prompt + .route/index.json
// ---------------------------------------------------------------------------

export interface TrackConfigDto {
  track_all: boolean;
  track_suffixes: string[];
  track_prefixes: string[];
  verify_sha256: boolean;
  memory_buffer_ms: number;
  track_on: { windows: boolean; macos: boolean; linux: boolean };
}

export async function trackGet(): Promise<TrackConfigDto> {
  return safeInvoke<TrackConfigDto>("track_get");
}

export async function trackSet(cfg: TrackConfigDto): Promise<TrackConfigDto> {
  return safeInvoke<TrackConfigDto>("track_set", { cfg });
}

export async function trackSetAll(on: boolean): Promise<TrackConfigDto> {
  return safeInvoke<TrackConfigDto>("track_set_all", { on });
}

export async function trackSetVerifySha256(on: boolean): Promise<TrackConfigDto> {
  return safeInvoke<TrackConfigDto>("track_set_verify_sha256", { on });
}

export async function trackSetMemoryBufferMs(ms: number): Promise<TrackConfigDto> {
  return safeInvoke<TrackConfigDto>("track_set_memory_buffer_ms", { ms });
}

export async function trackSetOn(on: {
  windows: boolean;
  macos: boolean;
  linux: boolean;
}): Promise<TrackConfigDto> {
  return safeInvoke<TrackConfigDto>("track_set_on", { on });
}

/// Built-in system prompt that AI agents should follow when driving
/// the software. The agent is expected to produce a one-sentence
/// intent summary, list the touched files, and surface any conflicts
/// via the CONFLICTS line in the body.
export async function aiSummaryPrompt(): Promise<string> {
  return safeInvoke<string>("ai_summary_prompt");
}

// ---------------------------------------------------------------------------
// AI chat — provider-agnostic one-shot completion primitive.
//
// The backend (`ai_chat` command in route-tauri/src/ai_commands.rs) supports
// three provider families: OpenAI / OpenAI-compatible, Anthropic, and Ollama.
// The HTTP call runs in Rust so the API key never touches the webview and we
// sidestep CORS. Used by the Settings page "test connection" button and as
// the building block any future agent loop will build on.
// ---------------------------------------------------------------------------

/// One chat message. `role` is `system` / `user` / `assistant`.
export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

/// Supported provider identifiers. `openai` also covers any OpenAI-compatible
/// server (DeepSeek, Groq, vLLM, LiteLLM, …) — point `endpoint` at it.
export type AiProvider = "openai" | "anthropic" | "ollama";

/// Send a chat completion request to the configured provider and return the
/// assistant's reply text. Throws the provider's error string on failure.
export async function aiChat(
  provider: AiProvider,
  endpoint: string,
  key: string,
  model: string,
  messages: ChatMessage[],
): Promise<string> {
  return safeInvoke<string>("ai_chat", { provider, endpoint, key, model, messages });
}

/// Regenerate the hidden `.route/index.json` from the current repo
/// state. Returns the absolute path the file was written to.
export async function routeIndexRefresh(): Promise<string> {
  return safeInvoke<string>("route_index_refresh");
}

/// Return the absolute path of the hidden `.route/index.json`, or
/// `null` if it has not been generated yet.
export async function routeIndexPath(): Promise<string | null> {
  return safeInvoke<string | null>("route_index_path");
}

// ---------------------------------------------------------------------------
// 智能取舍 — conflict resolution dialog
// ---------------------------------------------------------------------------

/// One conflict entry surfaced by an AI commit. The `kind` field is
/// the verdict the agent recommends (always "keep_old" when the user
/// has logic the agent is about to overwrite, "keep_ai" when the AI
/// is improving on the user's code, or "keep_both" when both
/// versions should be preserved).
export interface AiConflictDto {
  path: string;
  reason: string;
  /// The agent's recommended verdict: "keep_old" | "keep_ai" | "keep_both".
  recommendation: string;
}

export interface AiConflictReport {
  /// Commit id of the offending change. Empty when the conflict was
  /// surfaced outside of a commit (e.g. by the manual prompt).
  commit_id: string;
  /// When the report comes from parsing a commit body, the body
  /// the agent sent. Empty otherwise.
  body: string;
  conflicts: AiConflictDto[];
}

/// Parse the body of the most recent AI commit on the current branch
/// and return any `CONFLICTS:` entries it contains. The frontend uses
/// this to populate the 智能取舍 dialog.
export async function aiConflictReport(): Promise<AiConflictReport> {
  return safeInvoke<AiConflictReport>("ai_conflict_report");
}

/// Persist a verdict for one conflict (e.g. "keep_old" | "keep_ai" |
/// "keep_both"). Verdicts are stored in the timeline so the user
/// can revisit what they decided months later.
export async function aiConflictResolve(payload: {
  commit_id: string;
  path: string;
  verdict: "keep_old" | "keep_ai" | "keep_both";
  note?: string | null;
}): Promise<void> {
  return safeInvoke<void>("ai_conflict_resolve", { payload });
}

/// Return the persisted verdicts for a commit (path → verdict).
export async function aiConflictList(commitId: string): Promise<
  { path: string; verdict: string; note: string | null; created_at: number }[]
> {
  return safeInvoke<{ path: string; verdict: string; note: string | null; created_at: number }[]>(
    "ai_conflict_list",
    { commitId }
  );
}

// ---------------------------------------------------------------------------
// MCP server configuration
//
// The Tauri app does not run the MCP server — the AI client spawns
// `route-mcp` as a child process. This command locates the binary and
// builds the JSON config snippet the user pastes into their AI client.
// ---------------------------------------------------------------------------

export interface McpConfigDto {
  binary_path: string;
  binary_exists: boolean;
  project_path: string;
  config_snippet: string;
}

/// Fetch the MCP server config (binary path, project path, JSON snippet)
/// for the currently-open project.
export async function mcpGetConfig(): Promise<McpConfigDto> {
  return safeInvoke<McpConfigDto>("mcp_get_config");
}
