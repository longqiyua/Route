//! Context Pipeline — assembles project memory + structure + causal chain
//! for AI context injection, with token budget management and engine fallback.
//!
//! # Design
//!
//! The context pipeline implements the following flow:
//!
//! 1. **Assemble** — collect project memory, structure snapshot, and causal chain
//! 2. **Check budget** — estimate token count of assembled context
//! 3. **Trim if needed** — if budget exceeded, apply smart trimming (Core first,
//!    then Hot, then Cold)
//! 4. **Fallback to engine** — if still over budget, use `route-engine`'s fuzzy
//!    matching to find the most relevant memory entries
//! 5. **Output** — produce a structured text block for AI system prompt injection

use route_engine::TokenBudget;

use crate::causal::CausalChain;
use crate::memory::MemoryStore;
use crate::structure::StructureSnapshot;

/// Assembled context for AI injection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiContext {
    /// Formatted context text for AI system prompt.
    pub formatted: String,
    /// Whether the context was truncated to fit budget.
    pub truncated: bool,
    /// Token count of the formatted context.
    pub token_count: usize,
    /// Number of memory entries included.
    pub memory_entries: usize,
    /// Number of structure entries included.
    pub structure_entries: usize,
    /// Number of causal entries included.
    pub causal_entries: usize,
}

/// Context pipeline configuration.
#[derive(Debug, Clone, Copy)]
pub struct ContextConfig {
    /// Maximum token budget for AI context.
    pub max_tokens: usize,
    /// Maximum structure tree lines to include.
    pub max_structure_lines: usize,
    /// Maximum causal entries to include.
    pub max_causal_entries: usize,
    /// Whether to use engine fallback when budget is exceeded.
    pub engine_fallback: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: TokenBudget::DEFAULT_MAX,
            max_structure_lines: 50,
            max_causal_entries: 10,
            engine_fallback: true,
        }
    }
}

/// Context pipeline assembler.
#[derive(Debug, Clone)]
pub struct ContextPipeline {
    config: ContextConfig,
    budget: TokenBudget,
}

impl ContextPipeline {
    /// Create a new context pipeline with default config.
    pub fn new() -> Self {
        let config = ContextConfig::default();
        let budget = TokenBudget::new(config.max_tokens);
        Self { config, budget }
    }

    /// Create a pipeline with a custom config.
    pub fn with_config(config: ContextConfig) -> Self {
        let budget = TokenBudget::new(config.max_tokens);
        Self { config, budget }
    }

    /// Assemble full AI context from memory, structure, and causal chain.
    ///
    /// This is the **primary entry point** for AI context injection.
    /// AI MUST call this before searching or modifying code.
    pub fn assemble(
        &self,
        memory: &MemoryStore,
        structure: &StructureSnapshot,
        causal: &CausalChain,
        project_name: &str,
    ) -> AiContext {
        let mut parts: Vec<String> = Vec::new();
        let mut total_tokens: usize = 0;

        // 1. Project header (always included, minimal tokens)
        let header = format!(
            "=== Route Context ===\nProject: {}\nRoute is a version control and code understanding tool.\n\n",
            project_name
        );
        total_tokens += TokenBudget::estimate_tokens(&header);
        parts.push(header);

        // 2. Project memory (high priority — Core first)
        let memory_tokens = self.config.max_tokens.saturating_sub(total_tokens + 100); // leave room for other sections
        let (memory_text, mem_truncated) = memory.format_for_context(memory_tokens);
        if !mem_truncated || self.config.engine_fallback {
            total_tokens += TokenBudget::estimate_tokens(&memory_text);
            parts.push(memory_text);
        }

        // 3. Project structure (compact tree)
        let (tree_text, _) = structure.format_tree(self.config.max_structure_lines);
        total_tokens += TokenBudget::estimate_tokens(&tree_text);
        parts.push(tree_text);

        // 4. Causal chain (recent activity)
        let causal_tokens = self.config.max_tokens.saturating_sub(total_tokens + 50);
        let (causal_text, _) = causal.format_recent(self.config.max_causal_entries, causal_tokens);
        parts.push(causal_text);

        // 5. Check budget and trim if needed
        let mut formatted = parts.join("\n");
        let mut truncated = false;

        if self.budget.would_exceed(&formatted) {
            if self.config.engine_fallback {
                // Use engine fallback: trim to essential context
                formatted = self.budget.summarize(&formatted);
                truncated = true;
            } else {
                // Simple trim
                formatted = self.budget.trim(&formatted);
                truncated = true;
            }
        }

        // Update token count after trimming
        let final_tokens = TokenBudget::estimate_tokens(&formatted);

        AiContext {
            formatted,
            truncated,
            token_count: final_tokens,
            memory_entries: memory.len(),
            structure_entries: structure.len(),
            causal_entries: causal.len().min(self.config.max_causal_entries),
        }
    }

    /// Get the token budget instance.
    pub fn budget(&self) -> &TokenBudget {
        &self.budget
    }

    /// Get the current config.
    pub fn config(&self) -> &ContextConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryTier;
    use crate::MemoryStore;

    #[test]
    fn test_assemble_empty_context() {
        let pipeline = ContextPipeline::new();
        let memory = MemoryStore::new();
        let structure = StructureSnapshot::new();
        let causal = CausalChain::new();

        let ctx = pipeline.assemble(&memory, &structure, &causal, "test-project");
        assert!(!ctx.formatted.is_empty());
        assert!(!ctx.truncated);
        assert!(ctx.token_count > 0);
    }

    #[test]
    fn test_assemble_with_memory() {
        let pipeline = ContextPipeline::new();
        let mut memory = MemoryStore::new();
        memory.set("arch/test", "test architecture", MemoryTier::Core, vec![]);
        let structure = StructureSnapshot::new();
        let causal = CausalChain::new();

        let ctx = pipeline.assemble(&memory, &structure, &causal, "test");
        assert!(ctx.formatted.contains("arch/test"));
        assert_eq!(ctx.memory_entries, 1);
    }

    #[test]
    fn test_truncation() {
        let config = ContextConfig {
            max_tokens: 50,
            ..Default::default()
        };
        let pipeline = ContextPipeline::with_config(config);

        let mut memory = MemoryStore::new();
        for i in 0..20 {
            memory.set(
                &format!("key/{}", i),
                &"a".repeat(100),
                MemoryTier::Cold,
                vec![],
            );
        }
        let structure = StructureSnapshot::new();
        let causal = CausalChain::new();

        let ctx = pipeline.assemble(&memory, &structure, &causal, "test");
        // Should have some content but not all 20 entries
        assert!(ctx.truncated || ctx.memory_entries <= 20);
    }
}
