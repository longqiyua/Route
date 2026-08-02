//! BasicRepository — main entry point for basic mode operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Result};
use route_core::{
    content_hash, new_id, now_millis as _now, BlobStore, DbConnection, ManifestDiff,
    ProjectScanner, RoutePaths, TrackFilter,
};
use route_plugins::{Event, EventBus, PluginContext};
use rusqlite::params;

use crate::models::{
    AiPrompt, Branch, BranchKind, Commit, CommitKind, CommitPathAnnotation, DiffSummary, RepoConfig,
    Snapshot, SnapshotWithBranch, TrackConfig, TrackOs,
};

fn now_millis() -> i64 {
    _now()
}

/// Hydrate a `Commit` from a SQLite row in the standard commits schema
/// (id, from_snapshot, to_snapshot, message, author, created_at, branch_id,
/// kind, diff_summary, operator, body, is_checkpoint, is_ai).
///
/// Centralizing this here keeps the `get_commit` / `list_commits` /
/// `rollback_to` queries in lockstep when columns are added.
fn row_to_commit(r: &rusqlite::Row<'_>) -> rusqlite::Result<Commit> {
    let kind_str: String = r.get(7)?;
    Ok(Commit {
        id: r.get(0)?,
        from_snapshot: r.get(1)?,
        to_snapshot: r.get(2)?,
        message: r.get(3)?,
        author: r.get(4)?,
        created_at: r.get(5)?,
        branch_id: r.get(6)?,
        kind: CommitKind::from_str(&kind_str).unwrap_or(CommitKind::Incremental),
        diff_summary: r.get(8)?,
        operator: r.get(9)?,
        body: r.get(10)?,
        is_checkpoint: r.get::<_, i64>(11)? != 0,
        is_ai: r.get::<_, i64>(12)? != 0,
    })
}

/// Options for committing a new snapshot.
#[derive(Debug, Clone, Default)]
pub struct CommitOptions {
    pub message: String,
    pub author: Option<String>,
    /// Force a full snapshot (records kind=full self-loop edge).
    pub force_full: bool,
    /// Branch to commit on; defaults to current branch.
    pub branch: Option<String>,
    /// Operator identity stamp. `None` defaults to "user".
    /// Pass "ai:<name>" for AI-driven changes.
    pub operator: Option<String>,
    /// Long-form note (checkpoint body / AI prompt embedding).
    pub body: Option<String>,
    /// If true, this commit is recorded as a user-marked checkpoint.
    pub is_checkpoint: bool,
    /// If true, the change came from the AI control channel.
    pub is_ai: bool,
}

/// Options for a rollback. Used by `rollback_to_with` to attribute the
/// rollback to a specific operator (e.g. an AI agent).
#[derive(Debug, Clone, Default)]
pub struct RollbackOptions {
    /// Operator identity. Defaults to "user".
    pub operator: Option<String>,
    /// Optional long-form note (e.g. the AI prompt that triggered the undo).
    pub body: Option<String>,
    /// True if the rollback came from the AI control channel.
    pub is_ai: bool,
}

/// Options for creating a branch.
#[derive(Debug, Clone, Default)]
pub struct CreateBranchOptions {
    pub kind: BranchKind,
    /// For inherited/sandbox: parent branch name (defaults to current).
    pub from_branch: Option<String>,
}

/// Main repository handle.
pub struct BasicRepository {
    pub paths: RoutePaths,
    pub db: DbConnection,
    pub config: RepoConfig,
    /// Optional event bus. Set once via `set_event_bus`; subsequent reads
    /// go through `emit_event` which no-ops when the bus is unset.
    event_bus: OnceLock<Arc<EventBus>>,
    /// In-memory redo stack. Each entry is a `(from_snapshot, to_snapshot)`
    /// pair representing a previously-undone edit that the user can roll
    /// forward to. Cleared on any new head-advancing commit.
    ///
    /// This is intentionally in-memory only — across restarts the user
    /// cannot redo, which matches git's behavior. The DB still preserves
    /// the full edit / rollback history for inspection.
    redo_stack: Mutex<Vec<(String, String)>>,
    /// When true, all `commit` / `working_dir_status` operations use
    /// the non-parallel `ProjectScanner` instead of the default
    /// parallel walker. The parallel walker spawns worker threads
    /// with the 1 MB default Windows stack, which trips
    /// STATUS_STACK_OVERFLOW on the test runner. Production code
    /// keeps this at `false` for the speed.
    use_serial_scanner: bool,
}

/// A single file revision in history (returned by `file_history`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileRevision {
    pub snapshot_id: String,
    pub commit_id: Option<String>,
    pub commit_message: Option<String>,
    pub blob_hash: Option<String>,
    pub created_at: i64,
    pub branch_name: Option<String>,
}

/// One file entry in a commit's diff (returned by `commit_diff_detail`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommitDiffEntry {
    pub path: String,
    pub change: String, // "added" | "modified" | "removed"
    pub blob_hash: Option<String>,
    pub size_bytes: Option<u64>,
}

/// One entry in the working-directory status (returned by `working_dir_status`).
/// Compares current files on disk against the current branch's HEAD snapshot.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkingFileStatus {
    pub path: String,
    pub change: String, // "added" | "modified" | "removed"
    /// Current blob hash on disk (None for removed files).
    pub current_hash: Option<String>,
    /// Previous blob hash in HEAD snapshot (None for added files).
    pub previous_hash: Option<String>,
    pub size_bytes: Option<u64>,
}

/// One entry in a two-snapshot diff (returned by `diff_snapshots`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotDiffEntry {
    pub path: String,
    pub change: String, // "added" | "modified" | "removed"
    pub from_hash: Option<String>,
    pub to_hash: Option<String>,
}

impl BasicRepository {
    /// Attach an event bus. Once set, the bus cannot be replaced (matches
    /// the typical lifecycle: CLI/GUI constructs the bus once at startup).
    /// Returns `Err` if a bus is already attached.
    pub fn set_event_bus(&self, bus: Arc<EventBus>) -> Result<()> {
        self.event_bus
            .set(bus)
            .map_err(|_| anyhow!("event bus already attached"))
    }

    /// Whether an event bus is attached.
    pub fn has_event_bus(&self) -> bool {
        self.event_bus.get().is_some()
    }

    /// Dispatch an event to the bus (if attached). Errors are logged but
    /// never propagated — a misbehaving plugin must not break core ops.
    fn emit_event(&self, event: Event) {
        if let Some(bus) = self.event_bus.get() {
            let ctx = PluginContext::new(&self.paths.project_path);
            let result = bus.publish(&event, &ctx);
            if !result.is_ok() {
                tracing::warn!(
                    target: "route::events",
                    delivered = result.delivered,
                    failed = result.failed,
                    "plugin dispatch had failures: {:?}",
                    result.errors
                );
            }
        }
    }

    /// Initialize a new basic-mode repository in the given project path.
    pub fn init(project_path: impl AsRef<Path>) -> Result<Self> {
        let paths = RoutePaths::new(&project_path);
        if paths.is_initialized() {
            return Err(anyhow!("Route basic repository already initialized at {}", paths.route_dir.display()));
        }
        paths.ensure_dirs()?;

        // Create DB
        let db = DbConnection::open(&paths.db_path())?;

        // Create main branch
        let main_branch_id = new_id();
        {
            let conn = db.lock();
            conn.execute(
                "INSERT INTO branches(id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at)
                 VALUES(?1, 'main', 'main', NULL, NULL, NULL, ?2)",
                params![main_branch_id, now_millis()],
            )?;
        }

        let config = RepoConfig {
            version: 1,
            project_path: paths.project_path.to_string_lossy().to_string(),
            mode: "basic".to_string(),
            created_at: now_millis(),
            main_branch_id: main_branch_id.clone(),
            track: TrackConfig::default(),
        };

        // Write config.json
        let config_json = serde_json::to_string_pretty(&config)?;
        std::fs::write(paths.config_path(), config_json)?;

        Ok(Self {
            paths,
            db,
            config,
            event_bus: OnceLock::new(),
            redo_stack: Mutex::new(Vec::new()),
            use_serial_scanner: false,
        })
    }

    /// Open an existing basic-mode repository.
    pub fn open(project_path: impl AsRef<Path>) -> Result<Self> {
        let paths = RoutePaths::new(&project_path);
        if !paths.is_initialized() {
            return Err(anyhow!(
                "Not a Route basic repository: {}. Run `route init --mode basic` first.",
                paths.route_dir.display()
            ));
        }
        let db = DbConnection::open(&paths.db_path())?;
        let config_json = std::fs::read_to_string(paths.config_path())?;
        let mut config: RepoConfig = serde_json::from_str(&config_json)?;
        // Older configs (before TrackConfig was added) lack the field.
        // The `#[serde(default)]` annotation on the field already fills
        // it in, but be explicit so a freshly-initialised `RepoConfig`
        // round-trips correctly.
        let _ = &mut config.track;
        Ok(Self {
            paths,
            db,
            config,
            event_bus: OnceLock::new(),
            redo_stack: Mutex::new(Vec::new()),
            use_serial_scanner: false,
        })
    }

    /// Force all subsequent `commit` / `working_dir_status` operations
    /// to use the non-parallel `ProjectScanner`. This avoids the worker
    /// thread spawn that the parallel walker does, which trips
    /// STATUS_STACK_OVERFLOW on Windows when the call chain is deep.
    /// Mainly used by tests and by the example seeder.
    pub fn with_serial_scanner(mut self) -> Self {
        self.use_serial_scanner = true;
        self
    }

    /// Read the current track configuration.
    pub fn track_config(&self) -> &TrackConfig {
        &self.config.track
    }

    /// Update the track configuration in memory and on disk. Returns
    /// the saved `TrackConfig`. The next `commit` / `working_dir_status`
    /// call will use the new rules.
    pub fn set_track_config(&mut self, cfg: TrackConfig) -> Result<TrackConfig> {
        self.config.track = cfg;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json)?;
        Ok(self.config.track.clone())
    }

    /// Convenience: enable or disable "track all files" without
    /// losing the existing suffix / prefix lists.
    pub fn set_track_all(&mut self, on: bool) -> Result<()> {
        self.config.track.track_all = on;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json)?;
        Ok(())
    }

    /// Convenience: toggle SHA-256 verification. When on, every
    /// commit additionally records the SHA-256 of every tracked file
    /// in the manifest so an external system can verify the bytes.
    pub fn set_verify_sha256(&mut self, on: bool) -> Result<()> {
        self.config.track.verify_sha256 = on;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json)?;
        Ok(())
    }

    /// Convenience: set the memory-buffer debounce (in milliseconds).
    /// 0 disables the buffer (every change is committed immediately).
    pub fn set_memory_buffer_ms(&mut self, ms: u64) -> Result<()> {
        self.config.track.memory_buffer_ms = ms;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json)?;
        Ok(())
    }

    /// Update the OSes on which tracking is active. All three are
    /// enabled by default; the user can deselect any subset.
    pub fn set_track_on(&mut self, on: TrackOs) -> Result<()> {
        self.config.track.track_on = on;
        let json = serde_json::to_string_pretty(&self.config)?;
        std::fs::write(self.paths.config_path(), json)?;
        Ok(())
    }

    /// The system prompt the software shows to AI agents. Returned as
    /// a string so the frontend can surface it in the AI control panel
    /// or the AI control API.
    pub fn ai_summary_prompt() -> &'static str {
        AiPrompt::summary_prompt()
    }

    /// Public façade for the scanner, used by the index module and
    /// by external callers that need a custom TrackFilter.
    pub fn make_scanner(&self) -> ProjectScanner {
        self.scanner()
    }

    /// Public read-only access to the underlying `RoutePaths`. The
    /// conflict module and the index module both need this so they
    /// can locate `.route/conflicts/`, `.route/index.json`, etc.
    pub fn route_paths(&self) -> &RoutePaths {
        &self.paths
    }

    /// Parse the most recent AI commit on the current branch and
    /// return any conflicts surfaced by its body. Returns an empty
    /// report when the user has not done any AI work yet.
    pub fn latest_ai_conflict_report(&self) -> Result<crate::ai_conflict::AiConflictReport> {
        let commits = self.list_commits(None, 50)?;
        let ai = commits.into_iter().find(|c| c.is_ai);
        match ai {
            Some(c) => crate::ai_conflict::report_for_commit(&self.db.lock(), &c),
            None => Ok(crate::ai_conflict::AiConflictReport {
                commit_id: String::new(),
                body: String::new(),
                conflicts: Vec::new(),
            }),
        }
    }

    /// Persist a verdict for one conflict. Mirrors
    /// `crate::ai_conflict::record_verdict` but takes the lock for the
    /// caller so the IPC layer doesn't need to know about `DbConnection`.
    pub fn record_conflict_verdict(
        &self,
        commit_id: &str,
        path: &str,
        verdict: &str,
        note: Option<&str>,
    ) -> Result<crate::ai_conflict::AiConflictVerdict> {
        crate::ai_conflict::record_verdict(&self.paths, &self.db.conn, commit_id, path, verdict, note)
    }

    /// Return the verdicts recorded for a commit. See
    /// `crate::ai_conflict::list_verdicts`.
    pub fn list_conflict_verdicts(
        &self,
        commit_id: &str,
    ) -> Result<Vec<crate::ai_conflict::AiConflictVerdict>> {
        crate::ai_conflict::list_verdicts(&self.db.conn, commit_id)
    }

    fn scanner(&self) -> ProjectScanner {
        let s = ProjectScanner::new(&self.paths.project_path);
        // Translate the project's TrackConfig into a TrackFilter. If
        // the current OS is not in the active set, install a filter
        // that rejects everything so the scanner is a no-op.
        let filter = if !self.config.track.track_on.is_active_now() {
            Some(TrackFilter {
                track_all: false,
                suffixes: vec![],
                prefixes: vec![],
            })
        } else {
            Some(TrackFilter {
                track_all: self.config.track.track_all,
                suffixes: self
                    .config
                    .track
                    .track_suffixes
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .collect(),
                prefixes: self.config.track.track_prefixes.clone(),
            })
        };
        let s = s.with_track_filter(filter);
        if self.use_serial_scanner {
            s.with_serial_walker()
        } else {
            s
        }
    }

    /// Open or initialize a basic-mode repository.
    pub fn open_or_init(project_path: impl AsRef<Path>) -> Result<Self> {
        let paths = RoutePaths::new(&project_path);
        if paths.is_initialized() {
            Self::open(project_path)
        } else {
            Self::init(project_path)
        }
    }

    pub fn project_path(&self) -> &Path {
        &self.paths.project_path
    }

    // -----------------------------------------------------------------------
    // Branches
    // -----------------------------------------------------------------------

    pub fn list_branches(&self) -> Result<Vec<Branch>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at
             FROM branches ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let kind_str: String = row.get(2)?;
            Ok(Branch {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: BranchKind::from_str(&kind_str).unwrap_or(BranchKind::Main),
                parent_branch: row.get(3)?,
                baseline_snapshot: row.get(4)?,
                head_snapshot: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_branch(&self, name: &str) -> Result<Branch> {
        let conn = self.db.lock();
        let kind_str: String = conn.query_row(
            "SELECT kind FROM branches WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )?;
        let kind = BranchKind::from_str(&kind_str).unwrap_or(BranchKind::Main);
        let row = conn.query_row(
            "SELECT id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at
             FROM branches WHERE name = ?1",
            params![name],
            |row| {
                Ok(Branch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind,
                    parent_branch: row.get(3)?,
                    baseline_snapshot: row.get(4)?,
                    head_snapshot: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(row)
    }

    pub fn get_branch_by_id(&self, id: &str) -> Result<Branch> {
        let conn = self.db.lock();
        let kind_str: String = conn.query_row(
            "SELECT kind FROM branches WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        let kind = BranchKind::from_str(&kind_str).unwrap_or(BranchKind::Main);
        let row = conn.query_row(
            "SELECT id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at
             FROM branches WHERE id = ?1",
            params![id],
            |row| {
                Ok(Branch {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    kind,
                    parent_branch: row.get(3)?,
                    baseline_snapshot: row.get(4)?,
                    head_snapshot: row.get(5)?,
                    created_at: row.get(6)?,
                })
            },
        )?;
        Ok(row)
    }

    /// Create a new branch.
    pub fn create_branch(
        &self,
        name: &str,
        opts: CreateBranchOptions,
        current_branch_name: &str,
    ) -> Result<Branch> {
        if self.get_branch(name).is_ok() {
            return Err(anyhow!("Branch already exists: {}", name));
        }
        let parent_name = opts
            .from_branch
            .as_deref()
            .unwrap_or(current_branch_name);
        let parent = self.get_branch(parent_name)?;

        let new_id = new_id();
        let created = now_millis();
        let parent_head = parent.head_snapshot.clone();
        let (baseline, head) = match opts.kind {
            BranchKind::Main => (None, None),
            BranchKind::Inherited => (parent_head.clone(), parent_head.clone()),
            BranchKind::Sandbox => (None, parent_head.clone()),
        };

        // For sandbox: copy parent's HEAD manifest into a new snapshot (deep copy semantics
        // for state isolation — blobs are shared via content addressing, no duplication needed).
        let head_snapshot = match opts.kind {
            BranchKind::Sandbox => {
                if let Some(parent_head_id) = parent_head {
                    let parent_snap = self.get_snapshot(&parent_head_id)?;
                    // New snapshot referencing same manifest — blobs dedupe automatically.
                    Some(self.create_snapshot_record(&parent_snap.manifest_hash)?)
                } else {
                    None
                }
            }
            _ => head,
        };

        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO branches(id, name, kind, parent_branch, baseline_snapshot, head_snapshot, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id,
                name,
                opts.kind.as_str(),
                match opts.kind {
                    BranchKind::Main => None,
                    _ => Some(parent.id),
                },
                baseline,
                head_snapshot,
                created,
            ],
        )?;

        drop(conn);

        // Emit BranchCreated.
        self.emit_event(Event::BranchCreated {
            name: name.to_string(),
            kind: match opts.kind {
                BranchKind::Main => route_plugins::BranchKind::Main,
                BranchKind::Inherited => route_plugins::BranchKind::Inherited,
                BranchKind::Sandbox => route_plugins::BranchKind::Sandbox,
            },
            parent_branch: match opts.kind {
                BranchKind::Main => None,
                _ => Some(parent.name.clone()),
            },
            baseline_snapshot: baseline.clone(),
            timestamp: created,
        });

        self.get_branch(name)
    }

    pub fn delete_branch(&self, name: &str) -> Result<()> {
        if name == "main" {
            return Err(anyhow!("Cannot delete main branch"));
        }
        let branch = self.get_branch(name)?;
        let deleted_ts = now_millis();
        let conn = self.db.lock();
        // Cascade delete will remove commits and annotations
        conn.execute("DELETE FROM branches WHERE id = ?1", params![branch.id])?;
        drop(conn);

        // Emit BranchDeleted.
        self.emit_event(Event::BranchDeleted {
            name: name.to_string(),
            timestamp: deleted_ts,
        });
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Snapshots & manifests
    // -----------------------------------------------------------------------

    pub fn get_snapshot(&self, id: &str) -> Result<Snapshot> {
        let conn = self.db.lock();
        let row = conn.query_row(
            "SELECT id, manifest_hash, created_at FROM snapshots WHERE id = ?1",
            params![id],
            |r| {
                Ok(Snapshot {
                    id: r.get(0)?,
                    manifest_hash: r.get(1)?,
                    created_at: r.get(2)?,
                })
            },
        )?;
        Ok(row)
    }

    pub fn get_manifest(&self, hash: &str) -> Result<crate::models::Manifest> {
        let conn = self.db.lock();
        let content: String = conn.query_row(
            "SELECT content FROM manifests WHERE hash = ?1",
            params![hash],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Resolve a snapshot's full file state, walking inherited branch baseline if needed.
    pub fn resolve_snapshot_files(&self, snapshot_id: &str) -> Result<HashMap<String, String>> {
        let snap = self.get_snapshot(snapshot_id)?;
        let mut manifest = self.get_manifest(&snap.manifest_hash)?;

        // If snapshot is the baseline of an inherited branch, merge parent's resolved state.
        let conn = self.db.lock();
        let inherited: Option<(String, String)> = conn
            .query_row(
                "SELECT b.parent_branch, b.id FROM branches b
                 WHERE b.baseline_snapshot = ?1 AND b.kind = 'inherited'",
                params![snapshot_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .ok();
        drop(conn);

        if let Some((parent_branch_id, _)) = inherited {
            let parent = self.get_branch_by_id(&parent_branch_id)?;
            if let Some(parent_head) = parent.head_snapshot {
                // Guard against infinite recursion. An inherited branch is
                // created with `baseline = parent.head` and `head =
                // parent.head` (the same snapshot id on both branches), so
                // resolving that snapshot and then looking up the parent
                // finds the same id again. Without this guard the
                // recursion blows the stack — which is what crashed the
                // Tauri app the first time the user clicked "Try example
                // project" on Windows. Once the inherited branch makes
                // any commit, its head advances past the baseline and the
                // guard is a no-op.
                if parent_head == snapshot_id {
                    // Same id on both sides — nothing to merge in.
                } else {
                    let parent_files = self.resolve_snapshot_files(&parent_head)?;
                    // Child manifest overrides parent
                    for (k, v) in parent_files {
                        manifest.entry(k).or_insert(v);
                    }
                }
            }
        }

        Ok(manifest)
    }

    /// Internal: create a snapshot record (no manifest resolution).
    fn create_snapshot_record(&self, manifest_hash: &str) -> Result<String> {
        let id = new_id();
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO snapshots(id, manifest_hash, created_at) VALUES(?1, ?2, ?3)",
            params![id, manifest_hash, now_millis()],
        )?;
        Ok(id)
    }

    /// Internal: store manifest JSON, return its hash.
    fn store_manifest(&self, manifest: &HashMap<String, String>) -> Result<String> {
        let content = serde_json::to_string(manifest)?;
        let hash = content_hash(content.as_bytes());
        let conn = self.db.lock();
        conn.execute(
            "INSERT OR IGNORE INTO manifests(hash, content) VALUES(?1, ?2)",
            params![hash, content],
        )?;
        Ok(hash)
    }

    // -----------------------------------------------------------------------
    // Commit (the core innovation — metadata on edge)
    // -----------------------------------------------------------------------

    /// Commit current project state as a new snapshot + edge.
    pub fn commit(&self, opts: CommitOptions) -> Result<Commit> {
        let branch_name = opts
            .branch
            .clone()
            .unwrap_or_else(|| self.get_current_branch_name().unwrap_or("main".into()));
        let branch = self.get_branch(&branch_name)?;

        let scan = self.scanner().scan()?;
        let prev_files = match &branch.head_snapshot {
            Some(head) => self.resolve_snapshot_files(head)?,
            None => HashMap::new(),
        };

        let diff = ManifestDiff::compute(&prev_files, &scan.files);

        // Store blobs for changed files
        let blob_store = BlobStore::new(self.paths.clone());
        for (rel, abs) in &scan.absolute_paths {
            if diff.added.contains(rel) || diff.modified.contains(rel) {
                let bytes = std::fs::read(abs)?;
                let hash = blob_store.store(&bytes)?;
                let conn = self.db.lock();
                conn.execute(
                    "INSERT OR IGNORE INTO blobs(hash, size, created) VALUES(?1, ?2, ?3)",
                    params![hash, bytes.len() as i64, now_millis()],
                )?;
            }
        }

        let manifest_hash = self.store_manifest(&scan.files)?;
        let new_snapshot = self.create_snapshot_record(&manifest_hash)?;

        let kind = if opts.force_full {
            CommitKind::Full
        } else {
            CommitKind::Incremental
        };

        let diff_summary = DiffSummary {
            added: diff.added.clone(),
            modified: diff.modified.clone(),
            removed: diff.removed.clone(),
        };

        let commit_id = new_id();
        let from_snapshot = branch.head_snapshot.clone().unwrap_or_else(|| new_snapshot.clone());
        // For force_full: edge is a self-loop on current head (records the full-backup event).
        // Otherwise: edge goes from head to new snapshot.
        let to_snapshot = if opts.force_full && branch.head_snapshot.is_some() {
            from_snapshot.clone()
        } else {
            new_snapshot.clone()
        };

        // Default the operator to "user" for any direct user commit.
        let operator = opts.operator.clone().or_else(|| Some("user".to_string()));
        let is_checkpoint = opts.is_checkpoint;
        let is_ai = opts.is_ai;
        let body = opts.body.clone();

        let commit_ts = now_millis();
        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO commits(
                    id, from_snapshot, to_snapshot, message, author, created_at,
                    branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                 )
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    commit_id,
                    from_snapshot,
                    to_snapshot,
                    opts.message,
                    opts.author,
                    commit_ts,
                    branch.id,
                    kind.as_str(),
                    diff_summary.to_json(),
                    operator,
                    body,
                    is_checkpoint as i64,
                    is_ai as i64,
                ],
            )?;
            // Update branch HEAD only for incremental (not for self-loop full backup)
            if !opts.force_full {
                conn.execute(
                    "UPDATE branches SET head_snapshot = ?1 WHERE id = ?2",
                    params![to_snapshot, branch.id],
                )?;
            }
        }

        // Any new head-advancing commit diverges the linear history; the
        // user can no longer safely redo. Drop the in-memory redo stack.
        // Checkpoints and force_full backups don't advance head, so they
        // intentionally preserve any pending redo.
        if !opts.force_full && !is_checkpoint {
            self.redo_stack.lock().unwrap().clear();
        }

        // Emit SnapshotCreated (only when a new snapshot node was actually
        // created — force_full self-loops reuse the existing head).
        if !(opts.force_full && branch.head_snapshot.is_some()) {
            self.emit_event(Event::SnapshotCreated {
                snapshot_id: new_snapshot.clone(),
                branch_id: branch.id.clone(),
                branch_name: branch.name.clone(),
                manifest_hash,
                timestamp: commit_ts,
            });
        }

        // Emit CommitCreated.
        self.emit_event(Event::CommitCreated {
            commit_id: commit_id.clone(),
            branch_id: branch.id.clone(),
            branch_name: branch.name.clone(),
            from_snapshot,
            to_snapshot,
            message: opts.message.clone(),
            author: opts.author.clone(),
            kind: match kind {
                CommitKind::Incremental => route_plugins::CommitKind::Incremental,
                CommitKind::Full => route_plugins::CommitKind::Full,
                CommitKind::Merge => route_plugins::CommitKind::Merge,
                CommitKind::Rollback => route_plugins::CommitKind::Rollback,
            },
            timestamp: commit_ts,
        });

        // Refresh the hidden `.route/index.json` so AI agents see the
        // new state without having to re-scan the project. Errors are
        // swallowed — the index is a derivative artefact, not a
        // source of truth.
        if let Ok(idx_path) = crate::index::write_index(self) {
            let _ = idx_path;
        }

        self.get_commit(&commit_id)
    }

    /// Merge `source_branch` into `target_branch` (defaults to current).
    ///
    /// File-level 3-way merge when source is `Inherited` (merge base =
    /// source.baseline_snapshot); 2-way "theirs wins" when source is
    /// `Sandbox` (sandbox semantics = take what's in the sandbox). `Main`
    /// as source uses 3-way against the target's baseline when available.
    ///
    /// `Sandbox` cannot be a target — it's a terminal scratch space. The
    /// merged manifest is written to the project directory and then a
    /// single `CommitKind::Merge` edge is recorded on the target branch.
    /// Conflicts (both sides changed the same file) resolve to "theirs"
    /// and are listed in the commit message so the user can review.
    pub fn merge(
        &self,
        source_branch: &str,
        target_branch: Option<&str>,
    ) -> Result<Commit> {
        let target_name = match target_branch {
            Some(t) => t.to_string(),
            None => self.get_current_branch_name()?,
        };
        if source_branch == target_name {
            return Err(anyhow!("cannot merge a branch into itself"));
        }
        let source = self.get_branch(source_branch)?;
        let target = self.get_branch(&target_name)?;
        if target.kind == BranchKind::Sandbox {
            return Err(anyhow!("sandbox branches cannot be merge targets"));
        }

        let source_head = source.head_snapshot.as_ref()
            .ok_or_else(|| anyhow!("source branch '{}' has no commits", source_branch))?;
        let target_head = target.head_snapshot.as_ref()
            .ok_or_else(|| anyhow!("target branch '{}' has no commits", target_name))?;

        let theirs = self.resolve_snapshot_files(source_head)?;
        let ours = self.resolve_snapshot_files(target_head)?;

        // Merge base selection — only Inherited stores a baseline (the fork
        // point from its parent). Sandbox has no baseline, so we fall back
        // to a 2-way merge. Main as source has no parent either.
        let base = match source.kind {
            BranchKind::Inherited => source
                .baseline_snapshot
                .as_ref()
                .and_then(|id| self.resolve_snapshot_files(id).ok())
                .unwrap_or_default(),
            _ => HashMap::new(),
        };

        // 3-way file merge. For each path in the union of ours ∪ theirs ∪ base:
        //   - ours == base, theirs != base  → take theirs (they changed it)
        //   - theirs == base, ours != base  → keep ours (we changed it)
        //   - both != base                  → conflict → take theirs, record
        //   - both == base                  → keep (unchanged)
        // Files only in theirs → added. Files only in ours → kept.
        // Files in base but neither side → removed.
        let mut merged: HashMap<String, String> = HashMap::new();
        let mut conflicts: Vec<String> = Vec::new();
        let all_paths: std::collections::BTreeSet<String> = ours
            .keys()
            .chain(theirs.keys())
            .chain(base.keys())
            .cloned()
            .collect();
        for path in all_paths {
            let o = ours.get(&path);
            let t = theirs.get(&path);
            let b = base.get(&path);
            match (o, t, b) {
                (None, None, _) => {} // removed by both — stay removed
                (Some(o), Some(t), Some(b)) if o == b && t != b => {
                    // only theirs changed → take theirs
                    merged.insert(path, t.clone());
                }
                (Some(o), Some(t), Some(b)) if t == b && o != b => {
                    // only ours changed → keep ours
                    merged.insert(path, o.clone());
                }
                (Some(o), Some(t), _) if o == t => {
                    // same on both sides
                    merged.insert(path, o.clone());
                }
                (Some(_), Some(t), _) => {
                    // both changed differently → conflict, theirs wins
                    conflicts.push(path.clone());
                    merged.insert(path, t.clone());
                }
                (Some(o), None, Some(b)) if o == b => {
                    // theirs removed, ours unchanged → remove
                }
                (Some(o), None, _) => {
                    // theirs removed, ours changed (or only ours) → keep ours
                    merged.insert(path, o.clone());
                }
                (None, Some(t), _) => {
                    // only theirs has it → add
                    merged.insert(path, t.clone());
                }
            }
        }

        // Write the merged state to the project directory so the scanner
        // sees it when `commit` runs. This reuses the existing apply path.
        self.apply_files_to_project(&merged)?;

        let conflict_note = if conflicts.is_empty() {
            String::new()
        } else {
            format!("\n\nConflicts (took theirs): {}", conflicts.join(", "))
        };

        self.commit(CommitOptions {
            message: format!("merge '{}' into '{}'{}", source_branch, target_name, conflict_note),
            author: None,
            force_full: false,
            branch: Some(target_name.clone()),
            operator: Some("user".to_string()),
            body: None,
            is_checkpoint: false,
            is_ai: false,
        })
    }

    pub fn get_commit(&self, id: &str) -> Result<Commit> {
        let conn = self.db.lock();
        let row = conn.query_row(
            "SELECT id, from_snapshot, to_snapshot, message, author, created_at, branch_id, kind, diff_summary,
                    operator, body, is_checkpoint, is_ai
             FROM commits WHERE id = ?1",
            params![id],
            |r| row_to_commit(r),
        )?;
        Ok(row)
    }

    /// List commits on a branch (descending by time).
    pub fn list_commits(&self, branch_name: Option<&str>, limit: usize) -> Result<Vec<Commit>> {
        let conn = self.db.lock();
        let mut sql = String::from(
            "SELECT c.id, c.from_snapshot, c.to_snapshot, c.message, c.author, c.created_at,
                    c.branch_id, c.kind, c.diff_summary, c.operator, c.body, c.is_checkpoint, c.is_ai
             FROM commits c",
        );
        let mut params_vec: Vec<String> = Vec::new();
        if let Some(name) = branch_name {
            sql.push_str(" JOIN branches b ON c.branch_id = b.id WHERE b.name = ?1");
            params_vec.push(name.to_string());
        }
        sql.push_str(" ORDER BY c.created_at DESC LIMIT ?N");
        let limit_str = limit.to_string();
        let mut stmt = conn.prepare(&sql.replace("?N", &limit_str))?;
        let rows = if let Some(name) = branch_name {
            stmt.query_map(params![name], |r| row_to_commit(r))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            stmt.query_map([], |r| row_to_commit(r))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Path annotations (N-N edge text — innovation)
    // -----------------------------------------------------------------------

    pub fn add_path_annotation(&self, commit_id: &str, text: &str) -> Result<CommitPathAnnotation> {
        let id = new_id();
        let created = now_millis();
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO commit_path_annotations(id, commit_id, text, created_at) VALUES(?1, ?2, ?3, ?4)",
            params![id, commit_id, text, created],
        )?;
        drop(conn);

        // Emit AnnotationAdded.
        self.emit_event(Event::AnnotationAdded {
            commit_id: commit_id.to_string(),
            text: text.to_string(),
            timestamp: created,
        });

        Ok(CommitPathAnnotation {
            id,
            commit_id: commit_id.to_string(),
            text: text.to_string(),
            created_at: created,
        })
    }

    pub fn list_path_annotations(&self, commit_id: &str) -> Result<Vec<CommitPathAnnotation>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, commit_id, text, created_at FROM commit_path_annotations
             WHERE commit_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![commit_id], |r| {
            Ok(CommitPathAnnotation {
                id: r.get(0)?,
                commit_id: r.get(1)?,
                text: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn delete_path_annotation(&self, annotation_id: &str) -> Result<()> {
        let conn = self.db.lock();
        conn.execute(
            "DELETE FROM commit_path_annotations WHERE id = ?1",
            params![annotation_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // File-level history & restore (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Return the revision history of a single file path across all
    /// snapshots that contain it. Newest first. Revisions where the file
    /// was absent are skipped (use `commit_diff_detail` to see removals).
    pub fn file_history(&self, rel_path: &str) -> Result<Vec<FileRevision>> {
        // Walk all snapshots, resolve their manifest (including inherited
        // baselines), and record any that contain `rel_path`.
        let snaps = self.all_snapshots()?;
        let mut out = Vec::new();
        for swb in snaps {
            let snap = &swb.snapshot;
            let manifest = self.get_manifest(&snap.manifest_hash)?;
            // Inherited baseline merge: also check parent's resolved state.
            let blob_hash = if let Some(h) = manifest.get(rel_path) {
                Some(h.clone())
            } else {
                // Maybe inherited from parent baseline.
                let conn = self.db.lock();
                let parent_blob: Option<String> = conn
                    .query_row(
                        "SELECT b.parent_branch FROM branches b
                         WHERE b.baseline_snapshot = ?1 AND b.kind = 'inherited'",
                        params![snap.id],
                        |_| Ok(()),
                    )
                    .ok()
                    .and_then(|_: ()| None);
                drop(conn);
                // For simplicity we skip cross-branch inherited blobs in
                // history listing; the common case (main branch) is covered.
                parent_blob
            };
            if let Some(hash) = blob_hash {
                // Find the commit that produced this snapshot (to_snapshot = snap.id).
                let conn = self.db.lock();
                let commit_row: Option<(String, String, i64)> = conn
                    .query_row(
                        "SELECT c.id, c.message, c.created_at FROM commits c
                         WHERE c.to_snapshot = ?1
                         ORDER BY c.created_at DESC LIMIT 1",
                        params![snap.id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .ok();
                drop(conn);
                let (commit_id, commit_message, created_at) =
                    commit_row.unwrap_or((String::new(), String::new(), snap.created_at));
                out.push(FileRevision {
                    snapshot_id: snap.id.clone(),
                    commit_id: if commit_id.is_empty() {
                        None
                    } else {
                        Some(commit_id)
                    },
                    commit_message: if commit_message.is_empty() {
                        None
                    } else {
                        Some(commit_message)
                    },
                    blob_hash: Some(hash),
                    created_at,
                    branch_name: swb.branch_name.clone(),
                });
            }
        }
        // Sort newest first.
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    /// Restore a single file from a snapshot's blob into the working
    /// directory. Returns the absolute path that was written.
    pub fn restore_file_from_snapshot(
        &self,
        snapshot_id: &str,
        rel_path: &str,
    ) -> Result<PathBuf> {
        let files = self.resolve_snapshot_files(snapshot_id)?;
        let hash = files
            .get(rel_path)
            .ok_or_else(|| anyhow!("File '{}' not present in snapshot {}", rel_path, snapshot_id))?;
        let blob_store = BlobStore::new(self.paths.clone());
        let bytes = blob_store.read(hash)?;
        let target = self.paths.project_path.join(rel_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, &bytes)?;
        Ok(target)
    }

    // -----------------------------------------------------------------------
    // Commit diff detail (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Full diff detail for a commit: list of changed files with blob
    /// hashes and sizes. For "added"/"modified" files, `blob_hash` points
    /// to the new content in the `to_snapshot`. For "removed" files,
    /// `blob_hash` is `None`.
    pub fn commit_diff_detail(&self, commit_id: &str) -> Result<Vec<CommitDiffEntry>> {
        let commit = self.get_commit(commit_id)?;
        let diff = commit
            .diff_summary
            .as_deref()
            .map(DiffSummary::from_json)
            .unwrap_or_default();

        let to_manifest = self
            .get_snapshot(&commit.to_snapshot)
            .ok()
            .and_then(|s| self.get_manifest(&s.manifest_hash).ok())
            .unwrap_or_default();

        // blobs table for size lookup
        let conn = self.db.lock();
        let mut out = Vec::new();
        for path in &diff.added {
            let hash = to_manifest.get(path).cloned();
            let size = hash
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h));
            out.push(CommitDiffEntry {
                path: path.clone(),
                change: "added".into(),
                blob_hash: hash,
                size_bytes: size,
            });
        }
        for path in &diff.modified {
            let hash = to_manifest.get(path).cloned();
            let size = hash
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h));
            out.push(CommitDiffEntry {
                path: path.clone(),
                change: "modified".into(),
                blob_hash: hash,
                size_bytes: size,
            });
        }
        for path in &diff.removed {
            out.push(CommitDiffEntry {
                path: path.clone(),
                change: "removed".into(),
                blob_hash: None,
                size_bytes: None,
            });
        }
        drop(conn);
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Working directory status (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Compute the working-directory status: which files have been added,
    /// modified, or removed on disk compared to the current branch's HEAD
    /// snapshot. Useful for showing "what would be committed" before commit.
    pub fn working_dir_status(&self) -> Result<Vec<WorkingFileStatus>> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;

        let prev_files = match &branch.head_snapshot {
            Some(head) => self.resolve_snapshot_files(head)?,
            None => HashMap::new(),
        };

        let scan = self.scanner().scan()?;
        let diff = ManifestDiff::compute(&prev_files, &scan.files);

        let conn = self.db.lock();
        let mut out = Vec::new();
        for path in &diff.added {
            let hash = scan.files.get(path).cloned();
            let size = hash
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h))
                .or_else(|| {
                    scan.absolute_paths
                        .get(path)
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len())
                });
            out.push(WorkingFileStatus {
                path: path.clone(),
                change: "added".into(),
                current_hash: hash,
                previous_hash: None,
                size_bytes: size,
            });
        }
        for path in &diff.modified {
            let current = scan.files.get(path).cloned();
            let previous = prev_files.get(path).cloned();
            let size = current
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h))
                .or_else(|| {
                    scan.absolute_paths
                        .get(path)
                        .and_then(|p| std::fs::metadata(p).ok())
                        .map(|m| m.len())
                });
            out.push(WorkingFileStatus {
                path: path.clone(),
                change: "modified".into(),
                current_hash: current,
                previous_hash: previous,
                size_bytes: size,
            });
        }
        for path in &diff.removed {
            let previous = prev_files.get(path).cloned();
            let size = previous
                .as_deref()
                .and_then(|h| Self::blob_size(&conn, h));
            out.push(WorkingFileStatus {
                path: path.clone(),
                change: "removed".into(),
                current_hash: None,
                previous_hash: previous,
                size_bytes: size,
            });
        }
        drop(conn);
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Diff between two snapshots (Phase 5 capability extension)
    // -----------------------------------------------------------------------

    /// Diff two arbitrary snapshots by their IDs. Resolves inherited branch
    /// baselines via `resolve_snapshot_files`. Returns added/modified/removed
    /// entries with from/to blob hashes.
    pub fn diff_snapshots(
        &self,
        from_snapshot: &str,
        to_snapshot: &str,
    ) -> Result<Vec<SnapshotDiffEntry>> {
        let from_files = self.resolve_snapshot_files(from_snapshot)?;
        let to_files = self.resolve_snapshot_files(to_snapshot)?;
        let diff = ManifestDiff::compute(&from_files, &to_files);

        let mut out = Vec::new();
        for path in &diff.added {
            out.push(SnapshotDiffEntry {
                path: path.clone(),
                change: "added".into(),
                from_hash: None,
                to_hash: to_files.get(path).cloned(),
            });
        }
        for path in &diff.modified {
            out.push(SnapshotDiffEntry {
                path: path.clone(),
                change: "modified".into(),
                from_hash: from_files.get(path).cloned(),
                to_hash: to_files.get(path).cloned(),
            });
        }
        for path in &diff.removed {
            out.push(SnapshotDiffEntry {
                path: path.clone(),
                change: "removed".into(),
                from_hash: from_files.get(path).cloned(),
                to_hash: None,
            });
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Tags (named snapshot pointers)
    // -----------------------------------------------------------------------

    /// Create a tag pointing at a snapshot. Returns Err if name already
    /// exists or snapshot doesn't exist.
    pub fn tag_create(
        &self,
        name: &str,
        snapshot_id: &str,
        message: Option<&str>,
    ) -> Result<crate::models::Tag> {
        // Validate snapshot exists
        self.get_snapshot(snapshot_id)?;

        let id = new_id();
        let ts = now_millis();
        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO tags(id, name, snapshot_id, message, created_at)
                 VALUES(?1, ?2, ?3, ?4, ?5)",
                params![id, name, snapshot_id, message, ts],
            )?;
        }
        Ok(crate::models::Tag {
            id,
            name: name.to_string(),
            snapshot_id: snapshot_id.to_string(),
            message: message.map(|s| s.to_string()),
            created_at: ts,
        })
    }

    /// List all tags, newest first.
    pub fn tag_list(&self) -> Result<Vec<crate::models::Tag>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, snapshot_id, message, created_at
             FROM tags ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(crate::models::Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                snapshot_id: r.get(2)?,
                message: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Look up a tag by name.
    pub fn tag_get(&self, name: &str) -> Result<crate::models::Tag> {
        let conn = self.db.lock();
        let row = conn.query_row(
            "SELECT id, name, snapshot_id, message, created_at
             FROM tags WHERE name = ?1",
            params![name],
            |r| {
                Ok(crate::models::Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    snapshot_id: r.get(2)?,
                    message: r.get(3)?,
                    created_at: r.get(4)?,
                })
            },
        )?;
        Ok(row)
    }

    /// Delete a tag by name. Returns Err if not found.
    pub fn tag_delete(&self, name: &str) -> Result<()> {
        let conn = self.db.lock();
        let affected = conn.execute("DELETE FROM tags WHERE name = ?1", params![name])?;
        if affected == 0 {
            Err(anyhow!("Tag '{}' not found", name))
        } else {
            Ok(())
        }
    }

    /// Look up a blob's size from the `blobs` table.
    fn blob_size(conn: &rusqlite::Connection, hash: &str) -> Option<u64> {
        conn.query_row(
            "SELECT size FROM blobs WHERE hash = ?1",
            params![hash],
            |r| r.get::<_, i64>(0),
        )
        .ok()
        .map(|n| n as u64)
    }

    // -----------------------------------------------------------------------
    // Rollback (new rollback edge, history preserved)
    // -----------------------------------------------------------------------

    /// Rollback to a target snapshot, recording a new `kind=rollback` edge.
    ///
    /// Returns the new rollback commit. To attribute the rollback to a
    /// particular operator (e.g. an AI agent), pass `operator`, `body`, and
    /// `is_ai` through `RollbackOptions`. Defaults: `operator="user"`,
    /// `is_ai=false`.
    pub fn rollback_to(&self, snapshot_id: &str, reason: Option<&str>) -> Result<Commit> {
        self.rollback_to_with(snapshot_id, reason, RollbackOptions::default())
    }

    /// Like `rollback_to` but with explicit attribution fields.
    pub fn rollback_to_with(
        &self,
        snapshot_id: &str,
        reason: Option<&str>,
        opts: RollbackOptions,
    ) -> Result<Commit> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;
        let current_head = branch
            .head_snapshot
            .clone()
            .ok_or_else(|| anyhow!("Branch has no HEAD to rollback from"))?;

        // Emit RollbackStarted before mutating project files.
        let started_ts = now_millis();
        self.emit_event(Event::RollbackStarted {
            target_snapshot: snapshot_id.to_string(),
            reason: reason.map(|s| s.to_string()),
            timestamp: started_ts,
        });

        let files = self.resolve_snapshot_files(snapshot_id)?;
        self.apply_files_to_project(&files)?;

        let operator = opts.operator.unwrap_or_else(|| "user".to_string());
        let body = opts.body;
        let is_ai = opts.is_ai;

        let commit_id = new_id();
        let rollback_ts = now_millis();
        let message = reason.unwrap_or("Rollback").to_string();
        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO commits(
                    id, from_snapshot, to_snapshot, message, author, created_at,
                    branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                 )
                 VALUES(?1, ?2, ?3, ?4, NULL, ?5, ?6, 'rollback', NULL, ?7, ?8, 0, ?9)",
                params![
                    commit_id,
                    current_head,
                    snapshot_id,
                    message,
                    rollback_ts,
                    branch.id,
                    operator,
                    body,
                    is_ai as i64,
                ],
            )?;
            conn.execute(
                "UPDATE branches SET head_snapshot = ?1 WHERE id = ?2",
                params![snapshot_id, branch.id],
            )?;
        }

        // Emit RollbackCompleted, then CommitCreated (the rollback creates a
        // new kind=rollback commit edge in the DB, so downstream plugins
        // should see it just like a regular commit).
        self.emit_event(Event::RollbackCompleted {
            new_commit_id: commit_id.clone(),
            target_snapshot: snapshot_id.to_string(),
            timestamp: rollback_ts,
        });
        self.emit_event(Event::CommitCreated {
            commit_id: commit_id.clone(),
            branch_id: branch.id.clone(),
            branch_name: branch_name.clone(),
            from_snapshot: current_head.clone(),
            to_snapshot: snapshot_id.to_string(),
            message: message.clone(),
            author: None,
            kind: route_plugins::CommitKind::Rollback,
            timestamp: rollback_ts,
        });

        self.get_commit(&commit_id)
    }

    /// Apply a manifest to the project: delete files not in manifest, write all manifest files.
    fn apply_files_to_project(&self, files: &HashMap<String, String>) -> Result<()> {
        let blob_store = BlobStore::new(self.paths.clone());

        // Delete files present in project but not in target manifest
        let scan = self.scanner().scan()?;
        for rel in scan.files.keys() {
            if !files.contains_key(rel) {
                let abs = self.paths.project_path.join(rel);
                let _ = std::fs::remove_file(&abs);
            }
        }

        // Write target files
        for (rel, hash) in files {
            let abs = self.paths.project_path.join(rel);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            blob_store.copy_to(hash, &abs)?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Checkpoint (user-marked restore point)
    // -----------------------------------------------------------------------

    /// Mark a checkpoint at the current head. Does not modify files or
    /// advance the branch. The commit edge is a self-loop on head with
    /// `is_checkpoint=1`, the title in `message`, and the body in `body`.
    ///
    /// Returns the new checkpoint commit.
    pub fn checkpoint_create(
        &self,
        title: &str,
        body: Option<&str>,
        operator: Option<&str>,
    ) -> Result<Commit> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;
        let head = branch
            .head_snapshot
            .clone()
            .ok_or_else(|| anyhow!("Branch has no HEAD — commit something before checkpointing"))?;

        let commit_id = new_id();
        let ts = now_millis();
        let op = operator
            .map(|s| s.to_string())
            .unwrap_or_else(|| "user".to_string());

        {
            let conn = self.db.lock();
            conn.execute(
                "INSERT INTO commits(
                    id, from_snapshot, to_snapshot, message, author, created_at,
                    branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                 )
                 VALUES(?1, ?2, ?2, ?3, NULL, ?4, ?5, 'incremental', NULL, ?6, ?7, 1, 0)",
                params![
                    commit_id,
                    head,
                    title,
                    ts,
                    branch.id,
                    op,
                    body.map(|s| s.to_string()),
                ],
            )?;
        }

        self.emit_event(Event::CommitCreated {
            commit_id: commit_id.clone(),
            branch_id: branch.id.clone(),
            branch_name: branch_name.clone(),
            from_snapshot: head.clone(),
            to_snapshot: head.clone(),
            message: title.to_string(),
            author: None,
            kind: route_plugins::CommitKind::Incremental,
            timestamp: ts,
        });

        self.get_commit(&commit_id)
    }

    // -----------------------------------------------------------------------
    // Undo / Redo (operate on the redo_stack)
    // -----------------------------------------------------------------------

    /// Undo the most recent non-rollback commit on the current branch.
    ///
    /// Pushes `(from_snapshot, to_snapshot)` of that commit onto the
    /// redo stack so `redo_last` can roll it forward again, then issues
    /// a rollback to the commit's `from_snapshot`.
    ///
    /// Returns the rollback commit that was created.
    pub fn undo_last(&self) -> Result<Commit> {
        let branch_name = self.get_current_branch_name()?;
        let head = self
            .get_branch(&branch_name)?
            .head_snapshot
            .clone()
            .ok_or_else(|| anyhow!("Branch has no HEAD"))?;

        let commits = self.list_commits(Some(&branch_name), 50)?;

        // Newest non-rollback commit whose from_snapshot is not the
        // current head. The `from_snapshot` check skips commits that
        // have already been undone (their from_snapshot now equals
        // HEAD, so rolling back to it would be a no-op).
        let target = commits
            .iter()
            .filter(|c| c.kind != CommitKind::Rollback)
            .find(|c| c.from_snapshot != head)
            .ok_or_else(|| anyhow!("Nothing to undo"))?;

        // Record the redo pair BEFORE performing the rollback so the
        // state machine is consistent even if the rollback fails midway.
        {
            let mut stack = self.redo_stack.lock().unwrap();
            stack.push((target.from_snapshot.clone(), target.to_snapshot.clone()));
        }

        self.rollback_to_with(
            &target.from_snapshot,
            Some("Undo"),
            RollbackOptions {
                operator: Some("user".to_string()),
                body: None,
                is_ai: false,
            },
        )
    }

    /// Redo the most recently undone commit. Pops the top of the redo
    /// stack and rolls forward from the current head to the popped
    /// `to_snapshot`. Returns the rollback commit that was created.
    pub fn redo_last(&self) -> Result<Commit> {
        let next = {
            let mut stack = self.redo_stack.lock().unwrap();
            stack.pop()
        };
        let (_from, to) = next.ok_or_else(|| anyhow!("Nothing to redo"))?;

        self.rollback_to_with(
            &to,
            Some("Redo"),
            RollbackOptions {
                operator: Some("user".to_string()),
                body: None,
                is_ai: false,
            },
        )
    }

    /// Current size of the in-memory redo stack. Useful for the UI to
    /// enable / disable the redo button.
    pub fn redo_stack_len(&self) -> usize {
        self.redo_stack.lock().unwrap().len()
    }

    /// Whether the redo stack has at least one entry the user can redo.
    pub fn can_redo(&self) -> bool {
        self.redo_stack_len() > 0
    }

    // -----------------------------------------------------------------------
    // Full backup to external folder
    // -----------------------------------------------------------------------

    /// Copy all files at current HEAD to an external target folder.
    /// Records a kind=full self-loop edge in the DB.
    pub fn full_backup_to_dir(&self, target_dir: &Path) -> Result<PathBuf> {
        let branch_name = self.get_current_branch_name().unwrap_or_else(|_| "main".to_string());
        let branch = self.get_branch(&branch_name)?;
        let head = branch
            .head_snapshot
            .ok_or_else(|| anyhow!("Branch has no HEAD — commit something first"))?;

        let files = self.resolve_snapshot_files(&head)?;
        let blob_store = BlobStore::new(self.paths.clone());

        std::fs::create_dir_all(target_dir)?;
        for (rel, hash) in &files {
            let abs = target_dir.join(rel);
            if let Some(parent) = abs.parent() {
                std::fs::create_dir_all(parent)?;
            }
            blob_store.copy_to(hash, &abs)?;
        }

        // Record kind=full self-loop edge
        let commit_id = new_id();
        let conn = self.db.lock();
        conn.execute(
            "INSERT INTO commits(id, from_snapshot, to_snapshot, message, author, created_at, branch_id, kind, diff_summary)
             VALUES(?1, ?2, ?2, ?3, NULL, ?4, ?5, 'full', NULL)",
            params![
                commit_id,
                head,
                format!("Full backup to {}", target_dir.display()),
                now_millis(),
                branch.id,
            ],
        )?;
        drop(conn);

        Ok(target_dir.to_path_buf())
    }

    // -----------------------------------------------------------------------
    // File-based exports (ZIP / Folder)
    // -----------------------------------------------------------------------

    /// Build the ExportContext with all branches, snapshots, commits, and annotations.
    pub fn build_export_context(&self) -> Result<crate::export::ExportContext> {
        let branches = self.list_branches()?;
        let snapshots = self.all_snapshots()?;
        let commits = self.list_commits(None, 10000)?;

        let mut annotations = std::collections::HashMap::new();
        for c in &commits {
            let anns = self.list_path_annotations(&c.id).unwrap_or_default();
            annotations.insert(c.id.clone(), anns);
        }

        let project_name = self
            .project_path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "route-project".to_string());

        Ok(crate::export::ExportContext {
            project_name,
            project_path: self.project_path().to_string_lossy().to_string(),
            branches,
            snapshots: snapshots.into_iter().map(|s| s.snapshot).collect(),
            commits,
            annotations,
        })
    }

    /// Export the full repository (metadata + all blobs + all manifests) as a ZIP archive.
    pub fn export_zip(&self, output_path: &Path) -> Result<PathBuf> {
        use std::io::Write;

        let ctx = self.build_export_context()?;
        let blob_store = BlobStore::new(self.paths.clone());

        // Collect all blob hashes from DB
        let blob_hashes: Vec<String> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash FROM blobs ORDER BY hash")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };

        // Collect all manifest hashes
        let manifest_hashes: Vec<String> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash FROM manifests ORDER BY hash")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };

        let file = std::fs::File::create(output_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 1. Text exports
        let text_exporters = crate::DefaultExporters::all();
        for exporter in &text_exporters {
            let mut buf = Vec::new();
            exporter.export(&ctx, &mut buf)?;
            let name = match exporter.format() {
                crate::export::ExportFormat::Json => "metadata.json",
                crate::export::ExportFormat::Markdown => "report.md",
                crate::export::ExportFormat::Mermaid => "mindmap.mmd",
                crate::export::ExportFormat::EmacsOrg => "report.org",
                _ => continue,
            };
            zip.start_file(name, options)?;
            zip.write_all(&buf)?;
        }

        // 2. Manifests
        for hash in &manifest_hashes {
            let content: String = {
                let conn = self.db.lock();
                conn.query_row(
                    "SELECT content FROM manifests WHERE hash = ?1",
                    params![hash],
                    |r| r.get(0),
                )?
            };
            zip.start_file(format!("manifests/{hash}.json"), options)?;
            zip.write_all(content.as_bytes())?;
        }

        // 3. Blobs
        for hash in &blob_hashes {
            let bytes = blob_store.read(hash)?;
            zip.start_file(format!("blobs/{hash}"), options)?;
            zip.write_all(&bytes)?;
        }

        // 4. README
        zip.start_file("README.txt", options)?;
        let readme = format!(
            "Route export archive\n\
             Project: {}\n\
             Path: {}\n\
             Branches: {}\n\
             Snapshots: {}\n\
             Commits: {}\n\
             Blobs: {}\n\
             Manifests: {}\n\
             Generated: {}\n",
            ctx.project_name,
            ctx.project_path,
            ctx.branches.len(),
            ctx.snapshots.len(),
            ctx.commits.len(),
            blob_hashes.len(),
            manifest_hashes.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );
        zip.write_all(readme.as_bytes())?;

        zip.finish()?;
        Ok(output_path.to_path_buf())
    }

    /// Export the full repository as a folder structure on disk.
    pub fn export_folder(&self, output_dir: &Path) -> Result<PathBuf> {
        let ctx = self.build_export_context()?;
        let blob_store = BlobStore::new(self.paths.clone());

        std::fs::create_dir_all(output_dir)?;
        let manifests_dir = output_dir.join("manifests");
        let blobs_dir = output_dir.join("blobs");
        std::fs::create_dir_all(&manifests_dir)?;
        std::fs::create_dir_all(&blobs_dir)?;

        // 1. Text exports
        let text_exporters = crate::DefaultExporters::all();
        for exporter in &text_exporters {
            let mut buf = Vec::new();
            exporter.export(&ctx, &mut buf)?;
            let name = match exporter.format() {
                crate::export::ExportFormat::Json => "metadata.json",
                crate::export::ExportFormat::Markdown => "report.md",
                crate::export::ExportFormat::Mermaid => "mindmap.mmd",
                crate::export::ExportFormat::EmacsOrg => "report.org",
                _ => continue,
            };
            std::fs::write(output_dir.join(name), &buf)?;
        }

        // 2. Manifests
        let manifest_entries: Vec<(String, String)> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash, content FROM manifests")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };
        for (hash, content) in &manifest_entries {
            std::fs::write(manifests_dir.join(format!("{hash}.json")), content)?;
        }

        // 3. Blobs
        let blob_hashes: Vec<String> = {
            let conn = self.db.lock();
            let mut stmt = conn.prepare("SELECT hash FROM blobs ORDER BY hash")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            out
        };
        for hash in &blob_hashes {
            let bytes = blob_store.read(hash)?;
            std::fs::write(blobs_dir.join(hash), &bytes)?;
        }

        // 4. README
        let readme = format!(
            "Route export folder\n\
             Project: {}\n\
             Path: {}\n\
             Branches: {}\n\
             Snapshots: {}\n\
             Commits: {}\n\
             Blobs: {}\n\
             Manifests: {}\n\
             Generated: {}\n",
            ctx.project_name,
            ctx.project_path,
            ctx.branches.len(),
            ctx.snapshots.len(),
            ctx.commits.len(),
            blob_hashes.len(),
            manifest_entries.len(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
        );
        std::fs::write(output_dir.join("README.txt"), readme)?;

        Ok(output_dir.to_path_buf())
    }

    // -----------------------------------------------------------------------
    // Current branch tracking (HEAD)
    // -----------------------------------------------------------------------

    /// Get the current branch name. Stored in `meta` table as `current_branch`.
    pub fn get_current_branch_name(&self) -> Result<String> {
        let conn = self.db.lock();
        let name: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'current_branch'",
                [],
                |r| r.get(0),
            )
            .ok();
        drop(conn);
        Ok(name.unwrap_or_else(|| "main".to_string()))
    }

    pub fn set_current_branch(&self, name: &str) -> Result<()> {
        // Validate branch exists
        self.get_branch(name)?;
        let conn = self.db.lock();
        conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES('current_branch', ?1)",
            params![name],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // History (snapshot chain from HEAD backwards)
    // -----------------------------------------------------------------------

    pub fn history(&self, limit: usize) -> Result<Vec<SnapshotWithBranch>> {
        let branch_name = self.get_current_branch_name()?;
        let branch = self.get_branch(&branch_name)?;
        let mut out = Vec::new();
        let mut current_id = branch.head_snapshot;
        let mut visited = std::collections::HashSet::new();

        while let Some(id) = current_id {
            if out.len() >= limit || visited.contains(&id) {
                break;
            }
            visited.insert(id.clone());
            let snap = self.get_snapshot(&id)?;

            // Find the commit that produced this snapshot (incoming edge on current branch)
            let conn = self.db.lock();
            let producer: Option<Commit> = conn
                .query_row(
                    "SELECT id, from_snapshot, to_snapshot, message, author, created_at,
                            branch_id, kind, diff_summary, operator, body, is_checkpoint, is_ai
                     FROM commits WHERE to_snapshot = ?1 AND branch_id = ?2
                     ORDER BY created_at DESC LIMIT 1",
                    params![id, branch.id],
                    |r| row_to_commit(r),
                )
                .ok();
            drop(conn);

            out.push(SnapshotWithBranch {
                snapshot: snap.clone(),
                branch_id: Some(branch.id.clone()),
                branch_name: Some(branch.name.clone()),
            });

            current_id = producer.map(|p| p.from_snapshot);
        }

        Ok(out)
    }

    /// Get all snapshots with their owning branch (via latest producing commit).
    pub fn all_snapshots(&self) -> Result<Vec<SnapshotWithBranch>> {
        let conn = self.db.lock();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.manifest_hash, s.created_at, b.id, b.name
             FROM snapshots s
             LEFT JOIN commits c ON c.to_snapshot = s.id
             LEFT JOIN branches b ON b.id = c.branch_id
             GROUP BY s.id
             ORDER BY s.created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SnapshotWithBranch {
                snapshot: Snapshot {
                    id: r.get(0)?,
                    manifest_hash: r.get(1)?,
                    created_at: r.get(2)?,
                },
                branch_id: r.get(3)?,
                branch_name: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_repo() -> (TempDir, BasicRepository) {
        let tmp = TempDir::new().unwrap();
        let repo = BasicRepository::init(tmp.path()).unwrap();
        (tmp, repo)
    }

    #[test]
    fn init_creates_main_branch() {
        let (_tmp, repo) = setup_repo();
        let branches = repo.list_branches().unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].name, "main");
        assert_eq!(branches[0].kind, BranchKind::Main);
    }

    #[test]
    fn open_reopens_existing() {
        let tmp = TempDir::new().unwrap();
        BasicRepository::init(tmp.path()).unwrap();
        let repo = BasicRepository::open(tmp.path()).unwrap();
        assert_eq!(repo.config.mode, "basic");
    }

    #[test]
    fn commit_creates_snapshot_and_edge() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();

        let commit = repo
            .commit(CommitOptions {
                message: "initial".into(),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(commit.kind, CommitKind::Incremental);
        assert_eq!(commit.message, "initial");

        let history = repo.history(10).unwrap();
        assert!(!history.is_empty());
    }

    #[test]
    fn rollback_creates_rollback_edge() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // Rollback to v1
        let rollback = repo.rollback_to(&c1.to_snapshot, Some("back to v1")).unwrap();
        assert_eq!(rollback.kind, CommitKind::Rollback);

        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
    }

    #[test]
    fn full_backup_to_dir_copies_files() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub").join("b.txt"), b"world").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let backup_dir = tmp.path().join("backup");
        repo.full_backup_to_dir(&backup_dir).unwrap();

        assert!(backup_dir.join("a.txt").exists());
        assert!(backup_dir.join("sub").join("b.txt").exists());
    }

    #[test]
    fn path_annotations_n_to_n() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        let commit = repo
            .commit(CommitOptions {
                message: "init".into(),
                ..Default::default()
            })
            .unwrap();

        let a1 = repo.add_path_annotation(&commit.id, "first note").unwrap();
        let _a2 = repo.add_path_annotation(&commit.id, "second note").unwrap();

        let list = repo.list_path_annotations(&commit.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].text, "first note");
        assert_eq!(list[1].text, "second note");

        repo.delete_path_annotation(&a1.id).unwrap();
        let list = repo.list_path_annotations(&commit.id).unwrap();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn export_zip_produces_archive() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let zip_path = tmp.path().join("export.zip");
        repo.export_zip(&zip_path).unwrap();

        assert!(zip_path.exists());
        assert!(zip_path.metadata().unwrap().len() > 0);

        // Verify ZIP contents by reading it back
        let file = std::fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut has_metadata = false;
        let mut has_readme = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            if name == "metadata.json" {
                has_metadata = true;
            }
            if name == "README.txt" {
                has_readme = true;
            }
        }
        assert!(has_metadata, "ZIP should contain metadata.json");
        assert!(has_readme, "ZIP should contain README.txt");
    }

    #[test]
    fn export_folder_produces_files() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world").unwrap();
        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let export_dir = tmp.path().join("export-folder");
        repo.export_folder(&export_dir).unwrap();

        assert!(export_dir.exists());
        assert!(export_dir.join("metadata.json").exists());
        assert!(export_dir.join("report.md").exists());
        assert!(export_dir.join("mindmap.mmd").exists());
        assert!(export_dir.join("README.txt").exists());
        assert!(export_dir.join("manifests").exists());
        assert!(export_dir.join("blobs").exists());
        // At least 2 blobs (one for each file)
        let blob_count = std::fs::read_dir(export_dir.join("blobs")).unwrap().count();
        assert!(blob_count >= 2, "expected >= 2 blobs, got {blob_count}");
    }

    // -----------------------------------------------------------------------
    // Event bus integration
    // -----------------------------------------------------------------------

    /// A test plugin that records every event it sees.
    struct RecordingPlugin {
        seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl route_plugins::Plugin for RecordingPlugin {
        fn name(&self) -> &str {
            "recorder"
        }
        fn handle_event(
            &self,
            event: &Event,
            _ctx: &route_plugins::PluginContext,
        ) -> anyhow::Result<()> {
            self.seen
                .lock()
                .unwrap()
                .push(event.kind_label().to_string());
            Ok(())
        }
    }

    #[test]
    fn event_bus_emits_on_commit() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();

        // Without a bus attached, no events fire — sanity check.
        assert!(!repo.has_event_bus());

        let bus = std::sync::Arc::new(route_plugins::EventBus::new());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(Box::new(RecordingPlugin {
            seen: seen.clone(),
        }));
        repo.set_event_bus(bus).unwrap();
        assert!(repo.has_event_bus());

        repo.commit(CommitOptions {
            message: "initial".into(),
            ..Default::default()
        })
        .unwrap();

        let recorded = seen.lock().unwrap().clone();
        assert!(
            recorded.iter().any(|k| k == "snapshot_created"),
            "expected snapshot_created event, got {:?}",
            recorded
        );
        assert!(
            recorded.iter().any(|k| k == "commit_created"),
            "expected commit_created event, got {:?}",
            recorded
        );
    }

    #[test]
    fn event_bus_emits_on_rollback() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "second".into(),
            ..Default::default()
        })
        .unwrap();

        // Attach bus after the initial commits — we only want rollback events.
        let bus = std::sync::Arc::new(route_plugins::EventBus::new());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(Box::new(RecordingPlugin {
            seen: seen.clone(),
        }));
        repo.set_event_bus(bus).unwrap();

        repo.rollback_to(&c1.from_snapshot, Some("test rollback"))
            .unwrap();

        let recorded = seen.lock().unwrap().clone();
        assert!(recorded.iter().any(|k| k == "rollback_started"));
        assert!(recorded.iter().any(|k| k == "rollback_completed"));
        assert!(recorded.iter().any(|k| k == "commit_created"));
    }

    #[test]
    fn event_bus_emits_on_branch_lifecycle() {
        let (_tmp, repo) = setup_repo();

        let bus = std::sync::Arc::new(route_plugins::EventBus::new());
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(Box::new(RecordingPlugin {
            seen: seen.clone(),
        }));
        repo.set_event_bus(bus).unwrap();

        repo.create_branch(
            "feature",
            CreateBranchOptions {
                kind: BranchKind::Inherited,
                from_branch: Some("main".into()),
            },
            "main",
        )
        .unwrap();
        repo.delete_branch("feature").unwrap();

        let recorded = seen.lock().unwrap().clone();
        assert!(recorded.iter().any(|k| k == "branch_created"));
        assert!(recorded.iter().any(|k| k == "branch_deleted"));
    }

    #[test]
    fn event_bus_set_twice_errors() {
        let (_tmp, repo) = setup_repo();
        let bus1 = std::sync::Arc::new(route_plugins::EventBus::new());
        let bus2 = std::sync::Arc::new(route_plugins::EventBus::new());
        repo.set_event_bus(bus1).unwrap();
        assert!(repo.set_event_bus(bus2).is_err());
    }

    // -----------------------------------------------------------------------
    // Working directory status
    // -----------------------------------------------------------------------

    #[test]
    fn working_dir_status_detects_changes() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "init".into(),
            ..Default::default()
        })
        .unwrap();

        // Modify a.txt, add c.txt, delete b.txt
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"new").unwrap();
        std::fs::remove_file(tmp.path().join("b.txt")).unwrap();

        let status = repo.working_dir_status().unwrap();
        let added: Vec<_> = status.iter().filter(|s| s.change == "added").collect();
        let modified: Vec<_> = status.iter().filter(|s| s.change == "modified").collect();
        let removed: Vec<_> = status.iter().filter(|s| s.change == "removed").collect();

        assert_eq!(added.len(), 1);
        assert_eq!(added[0].path, "c.txt");
        assert!(added[0].current_hash.is_some());
        assert!(added[0].previous_hash.is_none());

        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].path, "a.txt");
        assert!(modified[0].current_hash.is_some());
        assert!(modified[0].previous_hash.is_some());
        assert_ne!(
            modified[0].current_hash,
            modified[0].previous_hash
        );

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].path, "b.txt");
        assert!(removed[0].current_hash.is_none());
        assert!(removed[0].previous_hash.is_some());
    }

    #[test]
    fn working_dir_status_empty_when_clean() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        repo.commit(CommitOptions {
            message: "init".into(),
            ..Default::default()
        })
        .unwrap();

        let status = repo.working_dir_status().unwrap();
        assert!(status.is_empty(), "expected empty status, got {:?}", status);
    }

    // -----------------------------------------------------------------------
    // Snapshot diff
    // -----------------------------------------------------------------------

    #[test]
    fn diff_snapshots_computes_changes() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"keep").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        std::fs::write(tmp.path().join("c.txt"), b"new").unwrap();
        std::fs::remove_file(tmp.path().join("b.txt")).unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "second".into(),
                ..Default::default()
            })
            .unwrap();

        let diff = repo
            .diff_snapshots(&c1.to_snapshot, &c2.to_snapshot)
            .unwrap();
        let added: Vec<_> = diff.iter().filter(|e| e.change == "added").collect();
        let modified: Vec<_> = diff.iter().filter(|e| e.change == "modified").collect();
        let removed: Vec<_> = diff.iter().filter(|e| e.change == "removed").collect();

        assert_eq!(added.len(), 1);
        assert_eq!(added[0].path, "c.txt");
        assert!(added[0].from_hash.is_none());
        assert!(added[0].to_hash.is_some());

        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].path, "a.txt");
        assert_ne!(modified[0].from_hash, modified[0].to_hash);

        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].path, "b.txt");
        assert!(removed[0].from_hash.is_some());
        assert!(removed[0].to_hash.is_none());
    }

    // -----------------------------------------------------------------------
    // Tags
    // -----------------------------------------------------------------------

    #[test]
    fn tag_lifecycle() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let commit = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();

        // Create
        let tag = repo
            .tag_create("v1.0", &commit.to_snapshot, Some("release 1.0"))
            .unwrap();
        assert_eq!(tag.name, "v1.0");
        assert_eq!(tag.snapshot_id, commit.to_snapshot);
        assert_eq!(tag.message.as_deref(), Some("release 1.0"));

        // List
        let tags = repo.tag_list().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "v1.0");

        // Get
        let fetched = repo.tag_get("v1.0").unwrap();
        assert_eq!(fetched.id, tag.id);

        // Duplicate name rejected
        assert!(repo.tag_create("v1.0", &commit.to_snapshot, None).is_err());

        // Nonexistent snapshot rejected
        assert!(repo
            .tag_create("bad", "nonexistent-snapshot-id", None)
            .is_err());

        // Delete
        repo.tag_delete("v1.0").unwrap();
        assert!(repo.tag_list().unwrap().is_empty());

        // Deleting again errors
        assert!(repo.tag_delete("v1.0").is_err());
    }

    // -----------------------------------------------------------------------
    // Checkpoint — user-defined marker on the current HEAD
    // -----------------------------------------------------------------------

    #[test]
    fn checkpoint_creates_self_loop_with_metadata() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "first".into(),
                ..Default::default()
            })
            .unwrap();

        // Capture HEAD before checkpointing.
        let head_before = repo
            .get_branch("main")
            .unwrap()
            .head_snapshot
            .clone()
            .expect("branch must have a HEAD after a commit");

        let cp = repo
            .checkpoint_create("Pre-refactor marker", Some("Refactoring foo.rs next"), Some("user"))
            .unwrap();

        // Self-loop edge.
        assert_eq!(cp.from_snapshot, cp.to_snapshot);
        assert_eq!(cp.to_snapshot, head_before);
        assert!(cp.is_checkpoint, "checkpoint flag must be set");
        assert!(!cp.is_ai, "checkpoint is a user action, is_ai must be false");
        assert_eq!(cp.message, "Pre-refactor marker");
        assert_eq!(cp.body.as_deref(), Some("Refactoring foo.rs next"));
        assert_eq!(cp.operator.as_deref(), Some("user"));

        // The checkpoint does not advance HEAD.
        let head_after = repo.get_branch("main").unwrap().head_snapshot.unwrap();
        assert_eq!(head_after, head_before, "checkpoint must not change HEAD");

        // And it lives in the same snapshot we committed to.
        assert_eq!(cp.to_snapshot, c1.to_snapshot);
    }

    #[test]
    fn checkpoint_default_operator_is_user() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "first".into(),
            ..Default::default()
        })
        .unwrap();

        let cp = repo
            .checkpoint_create("mark", None, None)
            .unwrap();
        assert_eq!(cp.operator.as_deref(), Some("user"));
    }

    #[test]
    fn checkpoint_on_empty_branch_errors() {
        let (_tmp, repo) = setup_repo();
        // No commit yet — main branch has no HEAD.
        let result = repo.checkpoint_create("too early", None, None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.to_lowercase().contains("no head") || msg.to_lowercase().contains("commit something"),
            "expected HEAD-required error, got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Operator / body / is_ai on regular commits
    // -----------------------------------------------------------------------

    #[test]
    fn commit_persists_operator_body_and_ai_flag() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();

        // Default user commit.
        let user_commit = repo
            .commit(CommitOptions {
                message: "user edit".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(user_commit.operator.as_deref(), Some("user"));
        assert!(!user_commit.is_ai);
        assert!(!user_commit.is_checkpoint);
        assert!(user_commit.body.is_none());

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();

        // AI-driven commit (the CLI / MCP would set this up).
        let ai_commit = repo
            .commit(CommitOptions {
                message: "ai edit".into(),
                operator: Some("ai:claude".into()),
                body: Some("Refactor foo() to use a HashMap".into()),
                is_ai: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(ai_commit.operator.as_deref(), Some("ai:claude"));
        assert!(ai_commit.is_ai);
        assert!(!ai_commit.is_checkpoint);
        assert_eq!(
            ai_commit.body.as_deref(),
            Some("Refactor foo() to use a HashMap")
        );

        // Both should be persisted (we re-open the repo to prove the
        // columns survive a round-trip through SQLite).
        drop(repo);
        let repo2 = BasicRepository::open(tmp.path()).unwrap();
        let commits = repo2.list_commits(Some("main"), 10).unwrap();
        assert_eq!(commits.len(), 2);
        let by_id: std::collections::HashMap<_, _> =
            commits.iter().map(|c| (c.id.clone(), c.clone())).collect();
        let u = by_id.get(&user_commit.id).unwrap();
        assert_eq!(u.operator.as_deref(), Some("user"));
        assert!(!u.is_ai);
        let a = by_id.get(&ai_commit.id).unwrap();
        assert_eq!(a.operator.as_deref(), Some("ai:claude"));
        assert!(a.is_ai);
        assert_eq!(
            a.body.as_deref(),
            Some("Refactor foo() to use a HashMap")
        );
    }

    // -----------------------------------------------------------------------
    // Undo / Redo state machine
    // -----------------------------------------------------------------------

    #[test]
    fn undo_restores_previous_file_state_and_pushes_redo_pair() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();

        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        let _c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();

        // Pre-conditions.
        assert!(!repo.can_redo(), "redo stack should start empty");
        assert_eq!(repo.redo_stack_len(), 0);

        // Undo: file should be v1, redo stack should have 1 entry.
        let undo = repo.undo_last().unwrap();
        assert_eq!(undo.kind, CommitKind::Rollback);
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
        assert!(repo.can_redo());
        assert_eq!(repo.redo_stack_len(), 1);
    }

    #[test]
    fn redo_replays_the_undone_change() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.undo_last().unwrap();
        assert!(repo.can_redo());
        assert_eq!(repo.redo_stack_len(), 1);

        // Redo: file should be v2 again, redo stack should be empty.
        let redo = repo.redo_last().unwrap();
        assert_eq!(redo.kind, CommitKind::Rollback);
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v2");
        assert!(!repo.can_redo());
        assert_eq!(repo.redo_stack_len(), 0);
    }

    #[test]
    fn undo_with_nothing_to_undo_errors() {
        let (_tmp, repo) = setup_repo();
        // No commits at all — main branch has no head, undo must error.
        let result = repo.undo_last();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.to_lowercase().contains("no head"),
            "expected HEAD-required error, got: {msg}"
        );
    }

    #[test]
    fn redo_with_nothing_to_redo_errors() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        // Never undone — redo must error cleanly.
        assert!(repo.redo_last().is_err());
    }

    #[test]
    fn undo_skips_rollback_commits_when_finding_target() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let _c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        let c2 = repo
            .commit(CommitOptions {
                message: "v2".into(),
                ..Default::default()
            })
            .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v3").unwrap();
        let c3 = repo
            .commit(CommitOptions {
                message: "v3".into(),
                ..Default::default()
            })
            .unwrap();

        // First undo: walk newest-first. c3 is the newest non-rollback
        // whose from != head (head = v3, c3.from = v2). Undo it.
        let u1 = repo.undo_last().unwrap();
        assert_eq!(u1.kind, CommitKind::Rollback);
        assert_eq!(u1.to_snapshot, c3.from_snapshot, "first undo should roll back to c3's from (v2)");
        // And the content is now v2.
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v2");

        // Second undo: the just-produced rollback (u1) is skipped (it's
        // a rollback AND its from equals the current head, so it would
        // be a no-op anyway). The next non-rollback with from != head
        // is c2 (from = v1). Undo it.
        let u2 = repo.undo_last().unwrap();
        assert_eq!(u2.kind, CommitKind::Rollback);
        assert_eq!(u2.to_snapshot, c2.from_snapshot, "second undo should roll back to c2's from (v1)");
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
        // The c3 commit is still in the DB; we just undid the work that
        // came after it.
        assert!(repo.get_commit(&c3.id).is_ok());
    }

    #[test]
    fn undo_on_first_commit_errors() {
        // The very first commit on a new branch is a self-loop edge
        // (from = to = the new snapshot, because there was no prior
        // head to transition from). There's no earlier state to roll
        // back to, so undo must error.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        let c1 = repo
            .commit(CommitOptions {
                message: "v1".into(),
                ..Default::default()
            })
            .unwrap();

        // The first commit on a new branch is a self-loop.
        assert_eq!(c1.from_snapshot, c1.to_snapshot);

        // Undo must fail cleanly with "Nothing to undo" because no
        // non-rollback commit has a from_snapshot different from HEAD.
        let result = repo.undo_last();
        assert!(result.is_err());
    }

    #[test]
    fn undo_works_after_two_commits_then_chains() {
        // Two real (non-self-loop) commits: c2.from ≠ c2.to, so the
        // second commit is undoable.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        // First undo: undoes c2, restoring v1.
        repo.undo_last().unwrap();
        let content = std::fs::read_to_string(tmp.path().join("a.txt")).unwrap();
        assert_eq!(content, "v1");
        assert!(repo.can_redo());

        // Second undo: c2 is now a no-op (its from = current head), so
        // the filter skips it. But c1 is a self-loop (from = to), so
        // it's also skipped (from = current head == v1). With both
        // commits filtered out, undo must error cleanly.
        let result = repo.undo_last();
        assert!(
            result.is_err(),
            "after undoing everything undoable, undo must fail"
        );
    }

    #[test]
    fn new_commit_after_undo_clears_redo_stack() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.undo_last().unwrap();
        assert!(repo.can_redo(), "undo should leave something on the redo stack");

        // A new head-advancing commit diverges the linear history — the
        // user can no longer safely redo (rolling forward would land on
        // a snapshot that's no longer in the chain). The repo drops the
        // redo stack as a defensive measure.
        std::fs::write(tmp.path().join("a.txt"), b"v3").unwrap();
        repo.commit(CommitOptions {
            message: "v3 (after undo)".into(),
            ..Default::default()
        })
        .unwrap();

        assert!(
            !repo.can_redo(),
            "redo stack must be cleared by a new head-advancing commit"
        );
        assert_eq!(repo.redo_stack_len(), 0);

        // And redo itself must now error cleanly.
        assert!(repo.redo_last().is_err());
    }

    #[test]
    fn checkpoint_after_undo_preserves_redo_stack() {
        // A checkpoint is NOT a head-advancing commit (it's a self-loop
        // on the current head), so it must leave the redo stack intact.
        // The user can still redo after checkpointing.
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            ..Default::default()
        })
        .unwrap();

        repo.undo_last().unwrap();
        assert!(repo.can_redo());

        repo.checkpoint_create("pre-redo", None, None).unwrap();
        assert!(
            repo.can_redo(),
            "checkpoint must NOT clear the redo stack"
        );
    }

    #[test]
    fn undo_attributes_to_user_operator() {
        let (tmp, repo) = setup_repo();
        std::fs::write(tmp.path().join("a.txt"), b"v1").unwrap();
        repo.commit(CommitOptions {
            message: "v1".into(),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"v2").unwrap();
        repo.commit(CommitOptions {
            message: "v2".into(),
            operator: Some("ai:claude".into()),
            body: Some("refactor".into()),
            is_ai: true,
            ..Default::default()
        })
        .unwrap();

        let undo = repo.undo_last().unwrap();
        // The undo itself is performed by the user (button click), even
        // if what was undone was an AI change.
        assert_eq!(undo.operator.as_deref(), Some("user"));
        assert!(!undo.is_ai);
    }
}
