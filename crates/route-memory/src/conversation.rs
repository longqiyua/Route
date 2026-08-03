//! Conversation tracking — record AI chats, link to snapshots, support rollback.
//!
//! # Design
//!
//! Every AI conversation is recorded as a **session** containing **messages**.
//! Each message can be linked to a **project snapshot** (from BasicRepository),
//! enabling rollback to any point in the conversation.
//!
//! # Storage
//!
//! Conversations are stored in `.route/conversations.json` — a lightweight JSON
//! file. This keeps dependencies minimal and data portable.
//!
//! # Rollback
//!
//! Rolling back to a specific message:
//! 1. Finds the snapshot linked to that message
//! 2. Restores the project to that snapshot (via `BasicRepository::rollback_to`)
//! 3. Truncates all messages after the rollback point
//! 4. Creates a new causal chain entry recording the rollback
//!
//! # Usage
//!
//! ```rust
//! use route_memory::ConversationStore;
//!
//! let mut store = ConversationStore::new();
//! let session_id = store.create_session("Refactoring auth").unwrap().id.clone();
//! let msg = store.add_message(&session_id, "user", "Let's refactor the auth module", None).unwrap().unwrap();
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A single conversation session (a chat thread).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID (ULID-based).
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// When the session was created (millis since epoch).
    pub created_at: i64,
    /// When the session was last updated.
    pub updated_at: i64,
    /// Number of messages in this session.
    pub message_count: usize,
    /// Tags for categorization (e.g., "refactor", "feature", "bugfix").
    pub tags: Vec<String>,
    /// Whether this session is archived (hidden from default list).
    pub archived: bool,
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID (ULID-based).
    pub id: String,
    /// Session this message belongs to.
    pub session_id: String,
    /// Role: "user", "ai", or "system".
    pub role: String,
    /// Message content.
    pub content: String,
    /// When the message was created.
    pub created_at: i64,
    /// Optional: linked snapshot ID from BasicRepository.
    /// When set, this message represents a point-in-time snapshot of the project.
    pub snapshot_id: Option<String>,
    /// Optional: previous message ID in the chain (for branching/threading).
    pub parent_id: Option<String>,
    /// Optional: rollback target — if this message was a rollback point, store the
    /// snapshot that was restored to.
    pub rollback_snapshot_id: Option<String>,
}

/// In-memory conversation store, persisted to JSON.
///
/// # Thread safety
///
/// This store is designed for single-threaded use. For concurrent access,
/// wrap in `Mutex`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationStore {
    /// All sessions, keyed by session ID.
    sessions: HashMap<String, Session>,
    /// All messages, keyed by message ID.
    messages: HashMap<String, Message>,
    /// Messages grouped by session_id (ordered list of message IDs).
    session_messages: HashMap<String, Vec<String>>,
    /// File path for persistence (empty means in-memory only).
    #[serde(skip)]
    storage_path: Option<PathBuf>,
    /// Monotonic counter for unique ID generation within same millisecond.
    #[serde(skip)]
    id_counter: u64,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationStore {
    /// Create a new empty conversation store.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            messages: HashMap::new(),
            session_messages: HashMap::new(),
            storage_path: None,
            id_counter: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Persistence
    // -----------------------------------------------------------------------

    /// Set the storage path and load existing data.
    pub fn with_path(path: PathBuf) -> Self {
        let mut store = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Self::new(),
            }
        } else {
            Self::new()
        };
        store.storage_path = Some(path);
        store
    }

    /// Generate a unique ID with timestamp and monotonic counter.
    fn next_id(&mut self, prefix: &str) -> String {
        self.id_counter += 1;
        format!("{}-{}-{}", prefix, chrono::Utc::now().timestamp_millis(), self.id_counter)
    }

    /// Save the store to disk (if a storage path is set).
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(path) = &self.storage_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(self)?;
            std::fs::write(path, json)?;
        }
        Ok(())
    }

    /// Get the storage path.
    pub fn storage_path(&self) -> Option<&Path> {
        self.storage_path.as_deref()
    }

    // -----------------------------------------------------------------------
    // Session CRUD
    // -----------------------------------------------------------------------

    /// Create a new session.
    pub fn create_session(&mut self, title: &str) -> std::io::Result<&Session> {
        let id = self.next_id("conv");
        let now = chrono::Utc::now().timestamp_millis();
        let session = Session {
            id: id.clone(),
            title: title.to_string(),
            created_at: now,
            updated_at: now,
            message_count: 0,
            tags: Vec::new(),
            archived: false,
        };
        self.sessions.insert(id.clone(), session);
        self.session_messages.insert(id.clone(), Vec::new());
        self.save()?;
        Ok(self.sessions.get(&id).unwrap())
    }

    /// Get a session by ID.
    pub fn get_session(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// List all sessions, newest first.
    pub fn list_sessions(&self) -> Vec<&Session> {
        let mut sessions: Vec<&Session> = self.sessions.values().filter(|s| !s.archived).collect();
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        sessions
    }

    /// Update a session's title.
    pub fn update_session_title(&mut self, id: &str, title: &str) -> std::io::Result<bool> {
        if let Some(session) = self.sessions.get_mut(id) {
            session.title = title.to_string();
            session.updated_at = chrono::Utc::now().timestamp_millis();
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Archive a session.
    pub fn archive_session(&mut self, id: &str) -> std::io::Result<bool> {
        if let Some(session) = self.sessions.get_mut(id) {
            session.archived = true;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Delete a session and all its messages.
    pub fn delete_session(&mut self, id: &str) -> std::io::Result<bool> {
        if self.sessions.remove(id).is_some() {
            if let Some(msg_ids) = self.session_messages.remove(id) {
                for msg_id in msg_ids {
                    self.messages.remove(&msg_id);
                }
            }
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Add tags to a session.
    pub fn add_tags(&mut self, session_id: &str, tags: Vec<String>) -> std::io::Result<bool> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            for tag in tags {
                if !session.tags.contains(&tag) {
                    session.tags.push(tag);
                }
            }
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // -----------------------------------------------------------------------
    // Message CRUD
    // -----------------------------------------------------------------------

    /// Add a message to a session.
    ///
    /// Returns the created message, or `None` if the session doesn't exist.
    pub fn add_message(
        &mut self,
        session_id: &str,
        role: &str,
        content: &str,
        snapshot_id: Option<String>,
    ) -> std::io::Result<Option<&Message>> {
        if !self.sessions.contains_key(session_id) {
            return Ok(None);
        }

        let id = self.next_id("msg");
        let now = chrono::Utc::now().timestamp_millis();

        // Find the parent (last message in the session)
        let parent_id = self.session_messages.get(session_id)
            .and_then(|ids| ids.last().cloned());

        let msg = Message {
            id: id.clone(),
            session_id: session_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: now,
            snapshot_id,
            parent_id,
            rollback_snapshot_id: None,
        };

        self.messages.insert(id.clone(), msg);

        // Add to session message list
        if let Some(msg_ids) = self.session_messages.get_mut(session_id) {
            msg_ids.push(id.clone());
        }

        // Update session metadata
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.message_count = self.session_messages.get(session_id).map(|v| v.len()).unwrap_or(0);
            session.updated_at = now;
        }

        self.save()?;
        Ok(self.messages.get(&id))
    }

    /// Get a message by ID.
    pub fn get_message(&self, id: &str) -> Option<&Message> {
        self.messages.get(id)
    }

    /// Get all messages in a session, in chronological order.
    pub fn session_messages(&self, session_id: &str) -> Vec<&Message> {
        self.session_messages
            .get(session_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.messages.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the last N messages in a session.
    pub fn last_messages(&self, session_id: &str, n: usize) -> Vec<&Message> {
        let msgs = self.session_messages(session_id);
        msgs.into_iter().rev().take(n).rev().collect()
    }

    /// Get the total number of messages in a session.
    pub fn message_count(&self, session_id: &str) -> usize {
        self.session_messages
            .get(session_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

// -----------------------------------------------------------------------
// Rollback
// -----------------------------------------------------------------------

/// Rollback result: describes what was done.
#[derive(Debug, Clone, Serialize)]
pub struct RollbackResult {
    /// The session ID.
    pub session_id: String,
    /// The message ID that was rolled back to.
    pub message_id: String,
    /// The snapshot ID that was restored.
    pub snapshot_id: String,
    /// Number of messages removed (those after the rollback point).
    pub messages_removed: usize,
    /// Whether the rollback was successful.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
}

impl ConversationStore {
    /// Rollback a session to a specific message.
    ///
    /// This:
    /// 1. Finds the message and its linked snapshot
    /// 2. Calls `BasicRepository::rollback_to` to restore the project
    /// 3. Truncates all messages after the rollback point
    ///
    /// Returns the rollback result.
    pub fn rollback_to_message(
        &mut self,
        session_id: &str,
        message_id: &str,
        reason: Option<&str>,
    ) -> RollbackResult {
        // Validate session and message
        let msg = match self.messages.get(message_id) {
            Some(m) if m.session_id == session_id => m,
            _ => {
                return RollbackResult {
                    session_id: session_id.to_string(),
                    message_id: message_id.to_string(),
                    snapshot_id: String::new(),
                    messages_removed: 0,
                    success: false,
                    error: Some(format!("Message '{}' not found in session '{}'", message_id, session_id)),
                };
            }
        };

        // Check if message has a snapshot
        let snapshot_id = match &msg.snapshot_id {
            Some(sid) => sid.clone(),
            None => {
                return RollbackResult {
                    session_id: session_id.to_string(),
                    message_id: message_id.to_string(),
                    snapshot_id: String::new(),
                    messages_removed: 0,
                    success: false,
                    error: Some(format!("Message '{}' has no linked snapshot — cannot rollback", message_id)),
                };
            }
        };

        // Perform the rollback via BasicRepository
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        match route_basic::BasicRepository::open(&cwd) {
            Ok(repo) => {
                if let Err(e) = repo.rollback_to_with(
                    &snapshot_id,
                    Some(reason.unwrap_or("rollback from conversation")),
                    route_basic::RollbackOptions {
                        operator: Some("ai".to_string()),
                        body: None,
                        is_ai: true,
                    },
                ) {
                    return RollbackResult {
                        session_id: session_id.to_string(),
                        message_id: message_id.to_string(),
                        snapshot_id,
                        messages_removed: 0,
                        success: false,
                        error: Some(format!("Rollback failed: {}", e)),
                    };
                }
            }
            Err(e) => {
                return RollbackResult {
                    session_id: session_id.to_string(),
                    message_id: message_id.to_string(),
                    snapshot_id,
                    messages_removed: 0,
                    success: false,
                    error: Some(format!("Failed to open repository: {}", e)),
                };
            }
        }

        // Truncate messages after the rollback point
        let removed = self.truncate_after(session_id, message_id);

        // Add a system message recording the rollback
        let rollback_note = format!(
            "🔄 Rolled back to snapshot '{}' (message '{}'). {} messages after this point were removed.",
            snapshot_id, message_id, removed
        );

        let _ = self.add_message(session_id, "system", &rollback_note, None);

        RollbackResult {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
            snapshot_id,
            messages_removed: removed,
            success: true,
            error: None,
        }
    }

    /// Truncate all messages after a given message ID in a session.
    /// Returns the number of messages removed.
    fn truncate_after(&mut self, session_id: &str, message_id: &str) -> usize {
        if let Some(msg_ids) = self.session_messages.get_mut(session_id) {
            // Find the position of the target message
            let pos = msg_ids.iter().position(|id| id == message_id);
            match pos {
                Some(p) => {
                    // Remove all messages after position p
                    let after: Vec<String> = msg_ids.drain(p + 1..).collect();
                    let count = after.len();
                    for id in after {
                        self.messages.remove(&id);
                    }
                    count
                }
                None => 0,
            }
        } else {
            0
        }
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    /// Total number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Total number of messages.
    pub fn message_count_all(&self) -> usize {
        self.messages.len()
    }

    /// Format a session summary for display.
    pub fn format_session_summary(&self, session: &Session) -> String {
        let msgs = self.session_messages(&session.id);
        let last_msg = msgs.last();
        let last_preview = last_msg
            .map(|m| {
                let preview = if m.content.len() > 60 {
                    format!("{}...", &m.content[..60])
                } else {
                    m.content.clone()
                };
                format!("[{}] {}", m.role, preview)
            })
            .unwrap_or_else(|| "(empty)".to_string());

        format!(
            "{:<20}  {:<4} msgs  {}",
            session.title,
            session.message_count,
            last_preview,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let mut store = ConversationStore::new();
        let session = store.create_session("Test session").unwrap();
        assert_eq!(session.title, "Test session");
        assert_eq!(session.message_count, 0);
    }

    #[test]
    fn test_add_message() {
        let mut store = ConversationStore::new();
        let session_id = store.create_session("Test").unwrap().id.clone();
        let msg = store
            .add_message(&session_id, "user", "Hello, AI!", None)
            .unwrap()
            .unwrap();
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello, AI!");
        assert_eq!(store.message_count(&session_id), 1);
    }

    #[test]
    fn test_add_message_with_snapshot() {
        let mut store = ConversationStore::new();
        let session_id = store.create_session("Test").unwrap().id.clone();
        let msg = store
            .add_message(&session_id, "ai", "Here is the refactored code", Some("snap-123".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(msg.snapshot_id, Some("snap-123".to_string()));
    }

    #[test]
    fn test_session_messages_order() {
        let mut store = ConversationStore::new();
        let session_id = store.create_session("Test").unwrap().id.clone();
        store.add_message(&session_id, "user", "msg1", None).unwrap();
        store.add_message(&session_id, "ai", "msg2", None).unwrap();
        store.add_message(&session_id, "user", "msg3", None).unwrap();

        let msgs = store.session_messages(&session_id);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].content, "msg1");
        assert_eq!(msgs[1].content, "msg2");
        assert_eq!(msgs[2].content, "msg3");
    }

    #[test]
    fn test_truncate_after() {
        let mut store = ConversationStore::new();
        let session_id = store.create_session("Test").unwrap().id.clone();
        store.add_message(&session_id, "user", "msg1", None).unwrap();
        let msg2_id = store.add_message(&session_id, "ai", "msg2", None).unwrap().unwrap().id.clone();
        store.add_message(&session_id, "user", "msg3", None).unwrap();

        let removed = store.truncate_after(&session_id, &msg2_id);
        assert_eq!(removed, 1); // only msg3 removed
        assert_eq!(store.message_count(&session_id), 2);
    }

    #[test]
    fn test_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conversations.json");

        // Write
        {
            let mut store = ConversationStore::with_path(path.clone());
            let session_id = store.create_session("Persist test").unwrap().id.clone();
            store.add_message(&session_id, "user", "Hello", None).unwrap();
            store.save().unwrap();
        }

        // Read
        {
            let store = ConversationStore::with_path(path.clone());
            assert_eq!(store.session_count(), 1);
            let sessions = store.list_sessions();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].title, "Persist test");
        }
    }

    #[test]
    fn test_archive_session() {
        let mut store = ConversationStore::new();
        store.create_session("Active").unwrap();
        let session = store.create_session("To archive").unwrap();
        let id = session.id.clone();
        store.archive_session(&id).unwrap();

        let sessions = store.list_sessions();
        assert_eq!(sessions.len(), 1); // only active sessions
        assert_eq!(sessions[0].title, "Active");
    }
}