//! Route Engine — lightweight search, fuzzy matching, and token budget estimation.
//!
//! # Design
//!
//! Route Engine is the **fallback layer** when AI context exceeds token budget.
//! Instead of sending full project context to the LLM, the engine provides:
//!
//! - **Fuzzy matching** — Levenshtein-based string matching for code search
//! - **Keyword indexing** — lightweight inverted index for fast lookup
//! - **Token budget estimation** — estimate token count of text
//! - **Context trimming** — smart truncation to fit within budget
//!
//! # Usage
//!
//! ```rust
//! use route_engine::{FuzzyMatcher, TokenBudget, SearchResult};
//!
//! let matcher = FuzzyMatcher::new();
//! let results = matcher.fuzzy_search("find_user", &["find_user_by_id", "create_user"]);
//! assert_eq!(results.len(), 2);
//! ```

pub mod fuzzy;
pub mod index;
pub mod token;

pub use fuzzy::FuzzyMatcher;
pub use index::KeywordIndex;
pub use token::TokenBudget;

/// A single search result with relevance score.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    /// The matched text or identifier.
    pub text: String,
    /// Relevance score (0.0 = no match, 1.0 = exact match).
    pub score: f64,
}