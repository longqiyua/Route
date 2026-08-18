//! Causal chain — ordered log of operations and their effects.
//!
//! Tracks what changes were made to the project, by whom, and what the
//! effects were. Used for audit trail and AI context about recent activity.

/// A single entry in the causal chain.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CausalEntry {
    /// Unique entry ID.
    pub id: String,
    /// What operation was performed (e.g., "commit", "branch_switch", "sync").
    pub action: String,
    /// Who performed it ("user", "ai", "system").
    pub actor: String,
    /// Description of the action.
    pub description: String,
    /// The effect or result of the action.
    pub effect: String,
    /// Timestamp (millis since epoch).
    pub timestamp: i64,
    /// Related tags for filtering.
    pub tags: Vec<String>,
}

/// Ordered causal chain of project operations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CausalChain {
    /// Entries in chronological order (newest first).
    entries: Vec<CausalEntry>,
    /// Maximum number of entries to keep.
    max_entries: usize,
}

impl Default for CausalChain {
    fn default() -> Self {
        Self::new()
    }
}

impl CausalChain {
    /// Default max entries.
    pub const DEFAULT_MAX: usize = 100;

    /// Create a new causal chain.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: Self::DEFAULT_MAX,
        }
    }

    /// Push a new entry to the causal chain.
    pub fn push(
        &mut self,
        action: &str,
        actor: &str,
        description: &str,
        effect: &str,
        tags: Vec<String>,
    ) {
        let id = format!("causal-{}", chrono::Utc::now().timestamp_millis());
        let entry = CausalEntry {
            id,
            action: action.to_string(),
            actor: actor.to_string(),
            description: description.to_string(),
            effect: effect.to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            tags,
        };
        self.entries.insert(0, entry);

        // Trim to max_entries
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Get recent entries (newest first).
    pub fn recent(&self, n: usize) -> Vec<&CausalEntry> {
        self.entries.iter().take(n).collect()
    }

    /// Get all entries.
    pub fn all(&self) -> &[CausalEntry] {
        &self.entries
    }

    /// Get entries filtered by actor.
    pub fn by_actor(&self, actor: &str) -> Vec<&CausalEntry> {
        self.entries.iter().filter(|e| e.actor == actor).collect()
    }

    /// Get entries filtered by action type.
    pub fn by_action(&self, action: &str) -> Vec<&CausalEntry> {
        self.entries.iter().filter(|e| e.action == action).collect()
    }

    /// Format recent causal entries for AI context injection.
    pub fn format_recent(&self, n: usize, max_tokens: usize) -> (String, bool) {
        let mut lines: Vec<String> = Vec::new();
        lines.push("=== Recent Activity (Causal Chain) ===\n".to_string());
        let mut used_tokens = 10;

        for entry in self.recent(n) {
            let line = format!(
                "  [{}] {} / {}: {} → {}",
                entry.action, entry.actor, entry.timestamp, entry.description, entry.effect
            );
            let line_tokens = (line.len() + 3) / 4;
            if used_tokens + line_tokens > max_tokens {
                lines.push("  ... (causal chain truncated)".to_string());
                return (lines.join("\n"), true);
            }
            lines.push(line);
            used_tokens += line_tokens;
        }

        if lines.len() == 1 {
            lines.push("  (no recent activity)".to_string());
        }

        (lines.join("\n"), false)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if chain is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_recent() {
        let mut chain = CausalChain::new();
        chain.push(
            "commit",
            "user",
            "added login feature",
            "snapshot created",
            vec!["feature".to_string()],
        );
        chain.push(
            "sync",
            "system",
            "auto-sync completed",
            "remote updated",
            vec!["sync".to_string()],
        );

        assert_eq!(chain.len(), 2);
        let recent = chain.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].action, "sync");
    }

    #[test]
    fn test_format_empty() {
        let chain = CausalChain::new();
        let (text, _) = chain.format_recent(5, 1000);
        assert!(text.contains("no recent activity"));
    }
}
