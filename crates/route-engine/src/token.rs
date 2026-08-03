//! Token budget estimation and context trimming.
//!
//! Provides a simple token counter that estimates the number of tokens
//! in a given text. Used to control AI context size and fall back to
//! the engine when content exceeds the budget.

/// Token budget manager for AI context window control.
///
/// # Example
///
/// ```rust
/// use route_engine::TokenBudget;
///
/// let budget = TokenBudget::new(4000); // 4K token budget
/// let text = "some very long text...";
///
/// if budget.would_exceed(text) {
///     let trimmed = budget.trim(text);
///     // trimmed fits within budget
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TokenBudget {
    /// Maximum allowed tokens.
    pub max_tokens: usize,
    /// Tokens reserved for system prompt (not counted in user content).
    pub reserved: usize,
}

impl TokenBudget {
    /// Default token budget: 4000 tokens (conservative for most models).
    pub const DEFAULT_MAX: usize = 4000;
    /// Default reserved tokens for system prompt.
    pub const DEFAULT_RESERVED: usize = 1000;

    /// Create a new token budget.
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            reserved: Self::DEFAULT_RESERVED,
        }
    }

    /// Create a budget with default settings.
    pub fn default() -> Self {
        Self::new(Self::DEFAULT_MAX)
    }

    /// Available tokens for user content (max - reserved).
    pub fn available(&self) -> usize {
        self.max_tokens.saturating_sub(self.reserved)
    }

    /// Estimate the token count of a text.
    ///
    /// Uses a simple heuristic: ~1 token per 4 characters for English text,
    /// which is a reasonable approximation for most LLM tokenizers.
    pub fn estimate_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        // Rough estimate: 1 token ≈ 4 chars
        (text.len() + 3) / 4
    }

    /// Check if text would exceed the available token budget.
    pub fn would_exceed(&self, text: &str) -> bool {
        Self::estimate_tokens(text) > self.available()
    }

    /// Trim text to fit within the available token budget.
    ///
    /// Returns the trimmed text with a note about truncation.
    pub fn trim(&self, text: &str) -> String {
        let available = self.available();
        if !self.would_exceed(text) {
            return text.to_string();
        }

        // Calculate how many characters we can keep
        let max_chars = available * 4;
        let mut trimmed = String::with_capacity(max_chars + 50);

        // Keep first 60% and last 40% for context preservation
        let first_part = max_chars * 60 / 100;
        let last_part = max_chars.saturating_sub(first_part);

        trimmed.push_str(&text[..first_part.min(text.len())]);
        trimmed.push_str("\n\n[... truncated ...]\n\n");

        if last_part > 0 && first_part < text.len() {
            let start = text.len().saturating_sub(last_part);
            trimmed.push_str(&text[start..]);
        }

        trimmed
    }

    /// Summarize text to essential context when budget is tight.
    ///
    /// Extracts the first line of each paragraph/section to create
    /// a dense summary. More aggressive than `trim`.
    pub fn summarize(&self, text: &str) -> String {
        if !self.would_exceed(text) {
            return text.to_string();
        }

        let mut summary = String::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                summary.push('\n');
            } else if !trimmed.starts_with(' ') && !trimmed.starts_with('\t') {
                // Section header or first line — keep it
                summary.push_str(trimmed);
                summary.push('\n');
            }
            // Estimate tokens as we go
            if Self::estimate_tokens(&summary) > self.available() {
                summary.push_str("[... context truncated by Route Engine ...]\n");
                break;
            }
        }

        if summary.is_empty() {
            text.chars().take(self.available() * 4).collect()
        } else {
            summary
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let tokens = TokenBudget::estimate_tokens("Hello, world!");
        assert!(tokens > 0);
    }

    #[test]
    fn test_would_exceed() {
        let budget = TokenBudget::new(5000);
        assert!(budget.would_exceed("a".repeat(100_000).as_str()));
        assert!(!budget.would_exceed("short"));
    }

    #[test]
    fn test_trim() {
        let budget = TokenBudget::new(50);
        let long = "a".repeat(1000);
        let trimmed = budget.trim(&long);
        assert!(trimmed.len() < long.len());
        assert!(trimmed.contains("[... truncated ...]"));
    }

    #[test]
    fn test_no_trim_when_under_budget() {
        let budget = TokenBudget::new(4000);
        let short = "short text";
        let trimmed = budget.trim(short);
        assert_eq!(trimmed, short);
    }
}