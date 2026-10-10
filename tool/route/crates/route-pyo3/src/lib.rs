//! Route Python bindings — PyO3 module.
//!
//! Exposes Route's core functionality (repository, conversation, context)
//! as a native Python module named `route`.
//!
//! # Usage
//!
//! ```python
//! import route
//!
//! # Initialize or open a repository
//! route.init("/path/to/project")
//! status = route.status()
//!
//! # Commit, log, rollback
//! route.commit("feat: add login")
//! commits = route.log(limit=5)
//!
//! # Conversation tracking
//! session = route.conversation_new("chat about refactoring")
//! route.conversation_record(session, "user", "hello")
//! ```

use std::path::PathBuf;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use route_basic::{
    BasicRepository, CommitOptions, CreateBranchOptions, DiffSummary, WorkingFileStatus,
};
use route_memory::ConversationStore;

// ---------------------------------------------------------------------------
// JSON → PyObject converter (since serde_json::Value doesn't impl ToPyObject)
// ---------------------------------------------------------------------------

fn json_to_py<'py>(py: Python<'py>, value: &serde_json::Value) -> PyObject {
    match value {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => b.to_object(py),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_object(py)
            } else if let Some(f) = n.as_f64() {
                f.to_object(py)
            } else {
                py.None()
            }
        }
        serde_json::Value::String(s) => s.to_object(py),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(json_to_py(py, item)).unwrap();
            }
            list.to_object(py)
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (k, v) in obj {
                dict.set_item(k, json_to_py(py, v)).unwrap();
            }
            dict.to_object(py)
        }
    }
}

fn json_result_to_py(value: serde_json::Value) -> PyObject {
    Python::with_gil(|py| json_to_py(py, &value))
}

// ---------------------------------------------------------------------------
// Helper: convert a single commit to a Python dict
// ---------------------------------------------------------------------------

fn commit_to_dict(c: &route_basic::Commit) -> serde_json::Value {
    serde_json::json!({
        "id": c.id,
        "message": c.message,
        "author": c.author,
        "created_at": c.created_at,
        "branch_id": c.branch_id,
        "kind": c.kind.as_str(),
        "from_snapshot": c.from_snapshot,
        "to_snapshot": c.to_snapshot,
        "operator": c.operator,
        "body": c.body,
        "is_checkpoint": c.is_checkpoint,
        "is_ai": c.is_ai,
        "diff_summary": c.diff_summary.as_ref().and_then(|d| {
            serde_json::from_str::<DiffSummary>(d).ok().map(|s| serde_json::json!({
                "added": s.added,
                "modified": s.modified,
                "removed": s.removed,
                "total": s.total(),
                "short": s.short(),
            }))
        }),
    })
}

fn branch_to_dict(b: &route_basic::Branch) -> serde_json::Value {
    serde_json::json!({
        "id": b.id,
        "name": b.name,
        "kind": b.kind.as_str(),
        "created_at": b.created_at,
        "head_snapshot": b.head_snapshot,
        "is_current": false,
    })
}

fn snapshot_to_dict(s: &route_basic::Snapshot) -> serde_json::Value {
    serde_json::json!({
        "id": s.id,
        "manifest_hash": s.manifest_hash,
        "created_at": s.created_at,
    })
}

fn working_file_to_dict(f: &WorkingFileStatus) -> serde_json::Value {
    serde_json::json!({
        "path": f.path,
        "change": f.change,
        "current_hash": f.current_hash,
        "previous_hash": f.previous_hash,
        "size_bytes": f.size_bytes,
    })
}

// ---------------------------------------------------------------------------
// Helpers: result conversion
// ---------------------------------------------------------------------------

fn anyhow_to_pyerr(e: anyhow::Error) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn io_to_pyerr(e: std::io::Error) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

// ---------------------------------------------------------------------------
// Global state (thread-safe, process-wide)
// ---------------------------------------------------------------------------

static GLOBAL_REPO: Lazy<Mutex<Option<BasicRepository>>> = Lazy::new(|| Mutex::new(None));

fn with_repo<F, T>(f: F) -> PyResult<T>
where
    F: FnOnce(&BasicRepository) -> PyResult<T>,
{
    let guard = GLOBAL_REPO
        .lock()
        .map_err(|e| PyRuntimeError::new_err(format!("lock error: {}", e)))?;
    match guard.as_ref() {
        Some(repo) => f(repo),
        None => Err(PyRuntimeError::new_err(
            "No Route repository open. Call route.init(path) or route.open(path) first.",
        )),
    }
}

fn with_repo_mut<F, T>(f: F) -> PyResult<T>
where
    F: FnOnce(&mut BasicRepository) -> PyResult<T>,
{
    let mut guard = GLOBAL_REPO
        .lock()
        .map_err(|e| PyRuntimeError::new_err(format!("lock error: {}", e)))?;
    match guard.as_mut() {
        Some(repo) => f(repo),
        None => Err(PyRuntimeError::new_err(
            "No Route repository open. Call route.init(path) or route.open(path) first.",
        )),
    }
}

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Initialize a Route repository at the given path.
#[pyfunction]
#[pyo3(signature = (path = None))]
fn init(path: Option<String>) -> PyResult<String> {
    let target = path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let repo = BasicRepository::init(&target).map_err(anyhow_to_pyerr)?;
    let _ = route_core::ensure_guard_anchor(&target);
    let path_str = repo.project_path().to_string_lossy().to_string();
    let mut guard = GLOBAL_REPO
        .lock()
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    *guard = Some(repo);
    Ok(path_str)
}

/// Open an existing Route repository.
#[pyfunction]
#[pyo3(signature = (path = None))]
fn open(path: Option<String>) -> PyResult<String> {
    let target = path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let repo = BasicRepository::open(&target).map_err(anyhow_to_pyerr)?;
    let path_str = repo.project_path().to_string_lossy().to_string();
    let mut guard = GLOBAL_REPO
        .lock()
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    *guard = Some(repo);
    Ok(path_str)
}

/// Get repository status as a dict.
#[pyfunction]
fn status() -> PyResult<PyObject> {
    with_repo(|repo| {
        let branches = repo.list_branches().map_err(anyhow_to_pyerr)?;
        let current = repo.get_current_branch_name().map_err(anyhow_to_pyerr)?;

        let branch_list: Vec<serde_json::Value> = branches
            .iter()
            .map(|b| {
                let mut v = branch_to_dict(b);
                v["is_current"] = serde_json::Value::Bool(b.name == current);
                v
            })
            .collect();

        let history = repo.history(1).map_err(anyhow_to_pyerr)?;
        let latest = history.first().map(|s| {
            serde_json::json!({
                "id": s.snapshot.id,
                "created_at": s.snapshot.created_at,
            })
        });

        let result = serde_json::json!({
            "project_path": repo.project_path().to_string_lossy().to_string(),
            "mode": repo.config.mode,
            "current_branch": current,
            "branches": branch_list,
            "latest_snapshot": latest,
        });
        Ok(json_result_to_py(result))
    })
}

/// Commit the current state as a new snapshot.
#[pyfunction]
#[pyo3(signature = (message, *, author = None, force_full = false, branch = None, body = None, is_checkpoint = false, is_ai = false))]
fn commit(
    message: String,
    author: Option<String>,
    force_full: bool,
    branch: Option<String>,
    body: Option<String>,
    is_checkpoint: bool,
    is_ai: bool,
) -> PyResult<PyObject> {
    with_repo_mut(|repo| {
        let commit = repo
            .commit(CommitOptions {
                message,
                author,
                force_full,
                branch,
                operator: Some("user".to_string()),
                body,
                is_checkpoint,
                is_ai,
            })
            .map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(commit_to_dict(&commit)))
    })
}

/// Get commit history.
#[pyfunction]
#[pyo3(signature = (limit = 20, branch = None))]
fn log(limit: usize, branch: Option<String>) -> PyResult<PyObject> {
    with_repo(|repo| {
        let commits = repo
            .list_commits(branch.as_deref(), limit)
            .map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = commits.iter().map(commit_to_dict).collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Rollback to a specific snapshot.
#[pyfunction]
#[pyo3(signature = (snapshot_id, *, reason = None))]
fn rollback(snapshot_id: String, reason: Option<String>) -> PyResult<PyObject> {
    with_repo_mut(|repo| {
        let commit = repo
            .rollback_to(&snapshot_id, reason.as_deref())
            .map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(commit_to_dict(&commit)))
    })
}

/// Show working directory changes (what would be committed).
#[pyfunction]
fn changes() -> PyResult<PyObject> {
    with_repo(|repo| {
        let files = repo.working_dir_status().map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = files.iter().map(working_file_to_dict).collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Diff two snapshots.
#[pyfunction]
fn diff(from: String, to: String) -> PyResult<PyObject> {
    with_repo(|repo| {
        let entries = repo.diff_snapshots(&from, &to).map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "change": e.change,
                    "from_hash": e.from_hash,
                    "to_hash": e.to_hash,
                })
            })
            .collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Undo the last commit.
#[pyfunction]
fn undo() -> PyResult<PyObject> {
    with_repo_mut(|repo| {
        let commit = repo.undo_last().map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(commit_to_dict(&commit)))
    })
}

/// Redo the last undone commit.
#[pyfunction]
fn redo() -> PyResult<PyObject> {
    with_repo_mut(|repo| {
        let commit = repo.redo_last().map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(commit_to_dict(&commit)))
    })
}

/// Get snapshot history.
#[pyfunction]
#[pyo3(signature = (limit = 20))]
fn history(limit: usize) -> PyResult<PyObject> {
    with_repo(|repo| {
        let snapshots = repo.history(limit).map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = snapshots
            .iter()
            .map(|s| {
                let mut v = snapshot_to_dict(&s.snapshot);
                v["branch"] = serde_json::Value::String(s.branch_name.clone().unwrap_or_default());
                v
            })
            .collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Create a checkpoint at the current state.
#[pyfunction]
#[pyo3(signature = (title, *, body = None))]
fn checkpoint(title: String, body: Option<String>) -> PyResult<PyObject> {
    with_repo_mut(|repo| {
        let commit = repo
            .checkpoint_create(&title, body.as_deref(), Some("pyo3"))
            .map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(commit_to_dict(&commit)))
    })
}

/// List all branches.
#[pyfunction]
fn branch_list() -> PyResult<PyObject> {
    with_repo(|repo| {
        let branches = repo.list_branches().map_err(anyhow_to_pyerr)?;
        let current = repo.get_current_branch_name().map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = branches
            .iter()
            .map(|b| {
                let mut v = branch_to_dict(b);
                v["is_current"] = serde_json::Value::Bool(b.name == current);
                v
            })
            .collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Create a new branch.
#[pyfunction]
#[pyo3(signature = (name, *, kind = "inherited", from_branch = None))]
fn branch_create(name: String, kind: &str, from_branch: Option<String>) -> PyResult<PyObject> {
    with_repo(|repo| {
        let branch_kind = match kind {
            "main" => route_basic::BranchKind::Main,
            "inherited" => route_basic::BranchKind::Inherited,
            "sandbox" => route_basic::BranchKind::Sandbox,
            _ => {
                return Err(PyValueError::new_err(
                    "Unknown branch kind. Use: main, inherited, sandbox",
                ))
            }
        };
        let current = repo.get_current_branch_name().map_err(anyhow_to_pyerr)?;
        let branch = repo
            .create_branch(
                &name,
                CreateBranchOptions {
                    kind: branch_kind,
                    from_branch,
                },
                &current,
            )
            .map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(branch_to_dict(&branch)))
    })
}

/// Switch to a branch.
#[pyfunction]
fn branch_switch(name: String) -> PyResult<()> {
    with_repo_mut(|repo| {
        repo.set_current_branch(&name).map_err(anyhow_to_pyerr)?;
        Ok(())
    })
}

/// Delete a branch.
#[pyfunction]
fn branch_delete(name: String) -> PyResult<()> {
    with_repo(|repo| {
        repo.delete_branch(&name).map_err(anyhow_to_pyerr)?;
        Ok(())
    })
}

/// List all tags.
#[pyfunction]
fn tag_list() -> PyResult<PyObject> {
    with_repo(|repo| {
        let tags = repo.tag_list().map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = tags
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "snapshot_id": t.snapshot_id,
                    "created_at": t.created_at,
                    "message": t.message,
                })
            })
            .collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Create a tag at the current HEAD.
#[pyfunction]
#[pyo3(signature = (name, *, message = None))]
fn tag_create(name: String, message: Option<String>) -> PyResult<PyObject> {
    with_repo(|repo| {
        let head = repo.history(1).map_err(anyhow_to_pyerr)?;
        let snapshot_id = head
            .first()
            .map(|s| s.snapshot.id.clone())
            .ok_or_else(|| PyRuntimeError::new_err("No commits yet — cannot tag"))?;
        let tag = repo
            .tag_create(&name, &snapshot_id, message.as_deref())
            .map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(serde_json::json!({
            "id": tag.id,
            "name": tag.name,
            "snapshot_id": tag.snapshot_id,
            "created_at": tag.created_at,
            "message": tag.message,
        })))
    })
}

/// Delete a tag.
#[pyfunction]
fn tag_delete(name: String) -> PyResult<()> {
    with_repo(|repo| {
        repo.tag_delete(&name).map_err(anyhow_to_pyerr)?;
        Ok(())
    })
}

/// Get all snapshots.
#[pyfunction]
fn all_snapshots() -> PyResult<PyObject> {
    with_repo(|repo| {
        let snapshots = repo.all_snapshots().map_err(anyhow_to_pyerr)?;
        let list: Vec<serde_json::Value> = snapshots
            .iter()
            .map(|s| {
                let mut v = snapshot_to_dict(&s.snapshot);
                v["branch"] = serde_json::Value::String(s.branch_name.clone().unwrap_or_default());
                v
            })
            .collect();
        Ok(json_result_to_py(serde_json::json!(list)))
    })
}

/// Get detailed stats.
#[pyfunction]
fn stats() -> PyResult<PyObject> {
    with_repo(|repo| {
        let branches = repo.list_branches().map_err(anyhow_to_pyerr)?;
        let snapshots = repo.all_snapshots().map_err(anyhow_to_pyerr)?;
        let commits = repo.list_commits(None, 10000).map_err(anyhow_to_pyerr)?;

        let main_count = branches
            .iter()
            .filter(|b| b.kind == route_basic::BranchKind::Main)
            .count();
        let inherited_count = branches
            .iter()
            .filter(|b| b.kind == route_basic::BranchKind::Inherited)
            .count();
        let sandbox_count = branches
            .iter()
            .filter(|b| b.kind == route_basic::BranchKind::Sandbox)
            .count();

        let inc = commits
            .iter()
            .filter(|c| c.kind == route_basic::CommitKind::Incremental)
            .count();
        let full = commits
            .iter()
            .filter(|c| c.kind == route_basic::CommitKind::Full)
            .count();
        let rollback = commits
            .iter()
            .filter(|c| c.kind == route_basic::CommitKind::Rollback)
            .count();
        let merge = commits
            .iter()
            .filter(|c| c.kind == route_basic::CommitKind::Merge)
            .count();

        let result = serde_json::json!({
            "branches": {
                "total": branches.len(),
                "main": main_count,
                "inherited": inherited_count,
                "sandbox": sandbox_count,
            },
            "snapshots": snapshots.len(),
            "commits": {
                "total": commits.len(),
                "incremental": inc,
                "full": full,
                "rollback": rollback,
                "merge": merge,
            },
        });
        Ok(json_result_to_py(result))
    })
}

/// Full backup of current HEAD to a target directory.
#[pyfunction]
fn backup(target: String) -> PyResult<String> {
    with_repo(|repo| {
        let target_path = PathBuf::from(&target);
        let result = repo
            .full_backup_to_dir(&target_path)
            .map_err(anyhow_to_pyerr)?;
        Ok(result.to_string_lossy().to_string())
    })
}

/// Attach a text annotation to a commit edge.
#[pyfunction]
fn annotate(commit_id: String, text: String) -> PyResult<PyObject> {
    with_repo(|repo| {
        let annotation = repo
            .add_path_annotation(&commit_id, &text)
            .map_err(anyhow_to_pyerr)?;
        Ok(json_result_to_py(serde_json::json!({
            "id": annotation.id,
            "commit_id": annotation.commit_id,
            "text": annotation.text,
            "created_at": annotation.created_at,
        })))
    })
}

/// List annotations on a commit.
#[pyfunction]
fn annotations(commit_id: String) -> PyResult<PyObject> {
    with_repo(|repo| {
        let list = repo
            .list_path_annotations(&commit_id)
            .map_err(anyhow_to_pyerr)?;
        let result: Vec<serde_json::Value> = list
            .iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "commit_id": a.commit_id,
                    "text": a.text,
                    "created_at": a.created_at,
                })
            })
            .collect();
        Ok(json_result_to_py(serde_json::json!(result)))
    })
}

// ---------------------------------------------------------------------------
// Conversation functions
// ---------------------------------------------------------------------------

fn conversation_store_path(project_path: &PathBuf) -> PathBuf {
    project_path.join(".route").join("conversations.json")
}

fn open_conversation_store(project_path: &PathBuf) -> ConversationStore {
    ConversationStore::with_path(conversation_store_path(project_path))
}

/// Create a new conversation session.
#[pyfunction]
#[pyo3(signature = (title))]
fn conversation_new(title: String) -> PyResult<String> {
    let path = get_project_path()?;
    let mut store = open_conversation_store(&path);
    let session = store.create_session(&title).map_err(io_to_pyerr)?;
    let id = session.id.clone();
    store.save().map_err(io_to_pyerr)?;
    Ok(id)
}

/// List all conversation sessions.
#[pyfunction]
fn conversation_list() -> PyResult<PyObject> {
    let path = get_project_path()?;
    let store = open_conversation_store(&path);
    let sessions = store.list_sessions();
    let list: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "title": s.title,
                "created_at": s.created_at,
                "updated_at": s.updated_at,
                "message_count": s.message_count,
                "tags": s.tags,
                "archived": s.archived,
            })
        })
        .collect();
    Ok(json_result_to_py(serde_json::json!(list)))
}

/// Show messages in a conversation session.
#[pyfunction]
#[pyo3(signature = (session_id, *, limit = None))]
fn conversation_show(session_id: String, limit: Option<usize>) -> PyResult<PyObject> {
    let path = get_project_path()?;
    let store = open_conversation_store(&path);
    let session = store
        .get_session(&session_id)
        .ok_or_else(|| PyRuntimeError::new_err(format!("Session '{}' not found", session_id)))?;

    let msgs = store.session_messages(&session_id);
    let msgs: Vec<&route_memory::Message> = if let Some(l) = limit {
        msgs.iter().rev().take(l).rev().copied().collect()
    } else {
        msgs
    };

    let messages: Vec<serde_json::Value> = msgs
        .iter()
        .map(|m| {
            serde_json::json!({
                "id": m.id,
                "role": m.role,
                "content": m.content,
                "created_at": m.created_at,
                "snapshot_id": m.snapshot_id,
                "parent_id": m.parent_id,
                "rollback_snapshot_id": m.rollback_snapshot_id,
            })
        })
        .collect();

    let result = serde_json::json!({
        "session": {
            "id": session.id,
            "title": session.title,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
            "message_count": session.message_count,
            "tags": session.tags,
            "archived": session.archived,
        },
        "messages": messages,
    });
    Ok(json_result_to_py(result))
}

/// Record a message in a conversation session.
#[pyfunction]
#[pyo3(signature = (session_id, role, content, *, snapshot_id = None))]
fn conversation_record(
    session_id: String,
    role: String,
    content: String,
    snapshot_id: Option<String>,
) -> PyResult<PyObject> {
    let path = get_project_path()?;
    let mut store = open_conversation_store(&path);
    let msg = store
        .add_message(&session_id, &role, &content, snapshot_id)
        .map_err(io_to_pyerr)?
        .ok_or_else(|| PyRuntimeError::new_err(format!("Session '{}' not found", session_id)))?;
    let cloned = msg.clone();
    store.save().map_err(io_to_pyerr)?;
    Ok(json_result_to_py(serde_json::json!({
        "id": cloned.id,
        "role": cloned.role,
        "content": cloned.content,
        "created_at": cloned.created_at,
        "snapshot_id": cloned.snapshot_id,
    })))
}

/// Rollback a conversation session to a specific message.
#[pyfunction]
#[pyo3(signature = (session_id, message_id, *, reason = None))]
fn conversation_rollback(
    session_id: String,
    message_id: String,
    reason: Option<String>,
) -> PyResult<PyObject> {
    let path = get_project_path()?;
    let mut store = open_conversation_store(&path);
    // Pass project_path explicitly to ensure correct repository is opened
    let result =
        store.rollback_to_message(&session_id, &message_id, reason.as_deref(), Some(&path));
    if result.success {
        Ok(json_result_to_py(serde_json::json!({
            "success": true,
            "snapshot_id": result.snapshot_id,
            "messages_removed": result.messages_removed,
        })))
    } else {
        Err(PyRuntimeError::new_err(
            result
                .error
                .unwrap_or_else(|| "Unknown rollback error".to_string()),
        ))
    }
}

/// Archive a conversation session.
#[pyfunction]
fn conversation_archive(session_id: String) -> PyResult<()> {
    let path = get_project_path()?;
    let mut store = open_conversation_store(&path);
    if store.archive_session(&session_id).map_err(io_to_pyerr)? {
        store.save().map_err(io_to_pyerr)?;
        Ok(())
    } else {
        Err(PyRuntimeError::new_err(format!(
            "Session '{}' not found",
            session_id
        )))
    }
}

/// Delete a conversation session.
#[pyfunction]
fn conversation_delete(session_id: String) -> PyResult<()> {
    let path = get_project_path()?;
    let mut store = open_conversation_store(&path);
    if store.delete_session(&session_id).map_err(io_to_pyerr)? {
        Ok(())
    } else {
        Err(PyRuntimeError::new_err(format!(
            "Session '{}' not found",
            session_id
        )))
    }
}

// ---------------------------------------------------------------------------
// Helper: get project path from global repo
// ---------------------------------------------------------------------------

fn get_project_path() -> PyResult<PathBuf> {
    let guard = GLOBAL_REPO
        .lock()
        .map_err(|e| PyRuntimeError::new_err(format!("lock error: {}", e)))?;
    match guard.as_ref() {
        Some(repo) => Ok(repo.project_path().to_path_buf()),
        None => Err(PyRuntimeError::new_err(
            "No Route repository open. Call route.init(path) or route.open(path) first.",
        )),
    }
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

/// Route — lightweight version manager for Vibe Coding (Python bindings).
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Repository operations
    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_function(wrap_pyfunction!(status, m)?)?;
    m.add_function(wrap_pyfunction!(commit, m)?)?;
    m.add_function(wrap_pyfunction!(log, m)?)?;
    m.add_function(wrap_pyfunction!(rollback, m)?)?;
    m.add_function(wrap_pyfunction!(changes, m)?)?;
    m.add_function(wrap_pyfunction!(diff, m)?)?;
    m.add_function(wrap_pyfunction!(undo, m)?)?;
    m.add_function(wrap_pyfunction!(redo, m)?)?;
    m.add_function(wrap_pyfunction!(history, m)?)?;
    m.add_function(wrap_pyfunction!(checkpoint, m)?)?;
    m.add_function(wrap_pyfunction!(stats, m)?)?;
    m.add_function(wrap_pyfunction!(all_snapshots, m)?)?;
    m.add_function(wrap_pyfunction!(backup, m)?)?;
    m.add_function(wrap_pyfunction!(annotate, m)?)?;
    m.add_function(wrap_pyfunction!(annotations, m)?)?;

    // Branch operations
    m.add_function(wrap_pyfunction!(branch_list, m)?)?;
    m.add_function(wrap_pyfunction!(branch_create, m)?)?;
    m.add_function(wrap_pyfunction!(branch_switch, m)?)?;
    m.add_function(wrap_pyfunction!(branch_delete, m)?)?;

    // Tag operations
    m.add_function(wrap_pyfunction!(tag_list, m)?)?;
    m.add_function(wrap_pyfunction!(tag_create, m)?)?;
    m.add_function(wrap_pyfunction!(tag_delete, m)?)?;

    // Conversation tracking
    m.add_function(wrap_pyfunction!(conversation_new, m)?)?;
    m.add_function(wrap_pyfunction!(conversation_list, m)?)?;
    m.add_function(wrap_pyfunction!(conversation_show, m)?)?;
    m.add_function(wrap_pyfunction!(conversation_record, m)?)?;
    m.add_function(wrap_pyfunction!(conversation_rollback, m)?)?;
    m.add_function(wrap_pyfunction!(conversation_archive, m)?)?;
    m.add_function(wrap_pyfunction!(conversation_delete, m)?)?;

    Ok(())
}
