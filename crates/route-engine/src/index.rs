//! Lightweight keyword index for fast code search.
//!
//! Builds an in-memory inverted index from code identifiers and file paths.
//! Used by the AI context pipeline to quickly find relevant files without
//! scanning the entire project.

use std::collections::HashMap;

/// A lightweight, in-memory keyword index.
#[derive(Debug, Clone)]
pub struct KeywordIndex {
    /// Inverted index: keyword → list of (document_id, count)
    index: HashMap<String, Vec<(String, usize)>>,
    /// Total documents indexed
    doc_count: usize,
}

impl Default for KeywordIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl KeywordIndex {
    /// Create a new empty keyword index.
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            doc_count: 0,
        }
    }

    /// Index a document by its ID and text content.
    ///
    /// Words are tokenized by splitting on non-alphanumeric characters
    /// and lowercased. Common words are excluded.
    pub fn index_document(&mut self, doc_id: &str, content: &str) {
        self.doc_count += 1;
        let words = tokenize(content);
        let mut freq: HashMap<String, usize> = HashMap::new();
        for word in words {
            *freq.entry(word).or_insert(0) += 1;
        }
        for (word, count) in freq {
            self.index.entry(word).or_default().push((doc_id.to_string(), count));
        }
    }

    /// Search for documents matching the query.
    ///
    /// Returns document IDs sorted by total match count (highest first).
    pub fn search(&self, query: &str) -> Vec<String> {
        let words = tokenize(query);
        if words.is_empty() {
            return Vec::new();
        }

        let mut scores: HashMap<String, usize> = HashMap::new();
        for word in &words {
            if let Some(docs) = self.index.get(word) {
                for (doc_id, count) in docs {
                    *scores.entry(doc_id.clone()).or_insert(0) += count;
                }
            }
        }

        let mut results: Vec<(String, usize)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.into_iter().map(|(id, _)| id).collect()
    }

    /// Number of indexed documents.
    pub fn doc_count(&self) -> usize {
        self.doc_count
    }

    /// Number of unique keywords in the index.
    pub fn keyword_count(&self) -> usize {
        self.index.len()
    }
}

/// Tokenize text into lowercase words, filtering out short/common words.
fn tokenize(text: &str) -> Vec<String> {
    // Common English stop words to exclude
    let stop_words: &[&str] = &[
        "the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "shall", "can", "need", "to", "of", "in",
        "for", "on", "with", "at", "by", "from", "as", "into", "through",
        "during", "before", "after", "above", "below", "between", "out",
        "off", "over", "under", "again", "further", "then", "once", "here",
        "there", "when", "where", "why", "how", "all", "each", "every",
        "both", "few", "more", "most", "other", "some", "such", "no", "nor",
        "not", "only", "own", "same", "so", "than", "too", "very", "just",
        "because", "but", "and", "or", "if", "while", "about", "up",
    ];

    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|s| s.len() > 2 && !stop_words.contains(&s.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_and_search() {
        let mut idx = KeywordIndex::new();
        idx.index_document("user.rs", "fn find_user_by_id(id: i32) -> User");
        idx.index_document("auth.rs", "fn login(username: &str, password: &str)");
        idx.index_document("email.rs", "fn send_email(to: &str, subject: &str)");

        let results = idx.search("user");
        assert!(!results.is_empty());
        assert!(results.contains(&"user.rs".to_string()));
    }

    #[test]
    fn test_empty_index() {
        let idx = KeywordIndex::new();
        assert!(idx.search("anything").is_empty());
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("fn find_user_by_id(id: i32) -> User");
        assert!(tokens.contains(&"user".to_string()));
        assert!(tokens.contains(&"find_user_by_id".to_string()));
    }
}