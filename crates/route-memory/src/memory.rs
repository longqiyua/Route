//! Structured project memory entries.
//!
//! Memory entries are the primary data source for AI context. Each entry
//! stores a key-value pair with metadata (tier, timestamp, tags).

use std::collections::HashMap;

/// Memory tier: controls retention and priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MemoryTier {
    /// Core memory — always included in context. Immutable project facts.
    Core,
    /// Hot memory — recent changes, active decisions.
    Hot,
    /// Cold memory — historical data, older decisions.
    Cold,
    /// Archived — project-level memory, only loaded on demand.
    Archived,
}

impl MemoryTier {
    /// Priority order for context inclusion (lower = more important).
    pub fn priority(&self) -> u8 {
        match self {
            Self::Core => 0,
            Self::Hot => 1,
            Self::Cold => 2,
            Self::Archived => 3,
        }
    }
}

/// A single memory entry in the project memory store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    /// Unique key for this memory (e.g., "architecture/layer", "decision/use-axum").
    pub key: String,
    /// The memory content.
    pub value: String,
    /// Memory tier for retention priority.
    pub tier: MemoryTier,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Timestamp (millis since epoch).
    pub created_at: i64,
    /// Last updated timestamp.
    pub updated_at: i64,
}

/// In-memory project memory store.
///
/// This is the **primary data source** for AI context. AI MUST read memory
/// entries before searching or modifying code.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryStore {
    /// All memory entries, keyed by their unique key.
    entries: HashMap<String, MemoryEntry>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    /// Create a new empty memory store.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Insert or update a memory entry.
    pub fn set(&mut self, key: &str, value: &str, tier: MemoryTier, tags: Vec<String>) {
        let now = chrono::Utc::now().timestamp_millis();
        let entry = MemoryEntry {
            key: key.to_string(),
            value: value.to_string(),
            tier,
            tags,
            created_at: self.entries.get(key).map(|e| e.created_at).unwrap_or(now),
            updated_at: now,
        };
        self.entries.insert(key.to_string(), entry);
    }

    /// Get a memory entry by key.
    pub fn get(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.get(key)
    }

    /// Remove a memory entry.
    pub fn remove(&mut self, key: &str) -> bool {
        self.entries.remove(key).is_some()
    }

    /// Get all entries sorted by priority (Core first, then Hot, Cold, Archived).
    pub fn all_sorted(&self) -> Vec<&MemoryEntry> {
        let mut entries: Vec<&MemoryEntry> = self.entries.values().collect();
        entries.sort_by_key(|e| e.tier.priority());
        entries
    }

    /// Get entries by tier.
    pub fn by_tier(&self, tier: MemoryTier) -> Vec<&MemoryEntry> {
        self.entries.values().filter(|e| e.tier == tier).collect()
    }

    /// Get entries matching a tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Search memory entries by key prefix.
    pub fn search_by_prefix(&self, prefix: &str) -> Vec<&MemoryEntry> {
        self.entries
            .values()
            .filter(|e| e.key.starts_with(prefix))
            .collect()
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Format memory as a structured text block for AI context injection.
    ///
    /// Respects a maximum token budget. Returns (formatted_text, was_truncated).
    pub fn format_for_context(&self, max_tokens: usize) -> (String, bool) {
        let mut lines: Vec<String> = Vec::new();
        let mut used_tokens = 0;

        // Estimate: 1 token ≈ 4 chars overhead per line
        lines.push("=== Project Memory (Route) ===\n".to_string());
        used_tokens += 8;

        for entry in self.all_sorted() {
            let line = format!(
                "[{}] {}: {}",
                match entry.tier {
                    MemoryTier::Core => "CORE",
                    MemoryTier::Hot => "HOT",
                    MemoryTier::Cold => "COLD",
                    MemoryTier::Archived => "ARCH",
                },
                entry.key,
                entry.value,
            );

            let line_tokens = (line.len() + 3) / 4;
            if used_tokens + line_tokens > max_tokens {
                lines.push("[... memory truncated by Route Engine ...]".to_string());
                return (lines.join("\n"), true);
            }

            lines.push(line);
            used_tokens += line_tokens;
        }

        (lines.join("\n"), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let mut store = MemoryStore::new();
        store.set("architecture/layers", "clean architecture", MemoryTier::Core, vec!["arch".to_string()]);
        let entry = store.get("architecture/layers");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, "clean architecture");
    }

    #[test]
    fn test_priority_sorting() {
        let mut store = MemoryStore::new();
        store.set("cold-key", "cold", MemoryTier::Cold, vec![]);
        store.set("core-key", "core", MemoryTier::Core, vec![]);
        store.set("hot-key", "hot", MemoryTier::Hot, vec![]);

        let sorted = store.all_sorted();
        assert_eq!(sorted[0].key, "core-key");
        assert_eq!(sorted[1].key, "hot-key");
        assert_eq!(sorted[2].key, "cold-key");
    }

    #[test]
    fn test_format_for_context() {
        let mut store = MemoryStore::new();
        store.set("test/key", "test value", MemoryTier::Core, vec![]);
        let (text, truncated) = store.format_for_context(1000);
        assert!(!text.is_empty());
        assert!(!truncated);
        assert!(text.contains("test/key"));
    }
}