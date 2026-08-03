//! Route Memory — project memory, project structure tracking, causal chain,
//! and conversation tracking.
//!
//! # Design
//!
//! Route Memory is the **always-available knowledge layer** for AI context.
//! It maintains:
//!
//! - **Project Memory** — structured key-value entries about the project
//! - **Project Structure** — snapshot of the current project file tree
//! - **Causal Chain** — ordered log of operations and their effects
//! - **Tiered Memory** — hot (recent) / cold (historical) separation
//! - **Context Pipeline** — assembles memory + structure + causal chain for
//!   AI context injection with token budget management
//! - **Conversation Tracking** — record AI chats, link to snapshots, rollback

pub mod causal;
pub mod context;
pub mod conversation;
pub mod memory;
pub mod structure;

pub use causal::{CausalChain, CausalEntry};
pub use context::{AiContext, ContextConfig, ContextPipeline};
pub use conversation::{ConversationStore, Message, RollbackResult, Session};
pub use memory::{MemoryEntry, MemoryStore, MemoryTier};
pub use structure::{ProjectStructure, StructureSnapshot};