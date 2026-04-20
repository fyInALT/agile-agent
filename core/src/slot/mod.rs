//! AgentSlot domain modules
//!
//! Provides modular components for agent slot management:
//! - status: AgentSlotStatus enum with transition validation
//! - state_machine: AgentStateMachine trait for explicit state transitions
//! - thread_types: TaskCompletionResult and ThreadOutcome

pub mod status;
pub mod state_machine;
pub mod thread_types;

// Re-export main types for convenience
pub use status::AgentSlotStatus;
pub use state_machine::{AgentStateMachine, DefaultStateMachine};
pub use thread_types::{TaskCompletionResult, ThreadOutcome};