//! AgentSlot domain modules
//!
//! Provides modular components for agent slot management:
//! - status: AgentSlotStatus enum with transition validation
//! - thread_types: TaskCompletionResult and ThreadOutcome

pub mod status;
pub mod thread_types;

// Re-export main types for convenience
pub use status::AgentSlotStatus;
pub use thread_types::{TaskCompletionResult, ThreadOutcome};