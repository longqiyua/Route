//! Fuzzy string matching using Levenshtein distance.
//!
//! Provides fast approximate matching for code identifiers, function names,
//! and file paths. Used when AI context is too large and we need to find
//! relevant code without sending everything to the LLM.

use crate::SearchResult;

/// Fuzzy string matcher using normalized Levenshtein distance.
#[derive(Debug, Clone)]
pub struct FuzzyMatcher {
    /// Minimum similarity threshold (0.0–1.0). Default: 0.4
    pub threshold: f64,
    /// Maximum results to return. Default: 10
    pub max_results: usize,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self {
            threshold: 0.4,
            max_results: 10,
        }
    }
}

impl FuzzyMatcher {
    /// Create a new fuzzy matcher with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a matcher with a custom threshold.
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            threshold,
            ..Self::default()
        }
    }

    /// Search `candidates` for the best fuzzy matches to `query`.
    ///
    /// Returns results sorted by relevance (highest first), filtered by threshold.
    pub fn fuzzy_search(&self, query: &str, candidates: &[&str]) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = candidates
            .iter()
            .map(|c| {
                let score = self.similarity(query, c);
                SearchResult {
                    text: c.to_string(),
                    score,
                }
            })
            .filter(|r| r.score >= self.threshold)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.max_results);
        results
    }

    /// Compute normalized Levenshtein similarity between two strings.
    ///
    /// Returns a value in [0.0, 1.0] where 1.0 = identical.
    pub fn similarity(&self, a: &str, b: &str) -> f64 {
        if a.is_empty() && b.is_empty() {
            return 1.0;
        }
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let dist = levenshtein_distance(a, b);
        let max_len = a.len().max(b.len()) as f64;
        1.0 - (dist as f64 / max_len)
    }
}

/// Compute Levenshtein distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr_row[j] = std::cmp::min(
                std::cmp::min(
                    curr_row[j - 1] + 1, // insertion
                    prev_row[j] + 1,     // deletion
                ),
                prev_row[j - 1] + cost, // substitution
            );
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let m = FuzzyMatcher::new();
        assert!((m.similarity("hello", "hello") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_no_match() {
        let m = FuzzyMatcher::new();
        assert!(m.similarity("abc", "xyz") < 0.4);
    }

    #[test]
    fn test_fuzzy_search() {
        let m = FuzzyMatcher::new();
        let candidates = &[
            "find_user_by_id",
            "create_user",
            "delete_user",
            "find_all_users",
            "update_email",
        ];
        let results = m.fuzzy_search("find_user", candidates);
        assert!(!results.is_empty());
        assert!(results[0].score >= 0.4);
    }

    #[test]
    fn test_levenshtein() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("", "abc"), 3);
    }
}
