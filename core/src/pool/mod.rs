//! AgentPool domain modules
//!
//! Provides modular components for pool management:
//! - blocked_handler: BlockedHandler for handling blocked agents
//!
//! Types moved to pool/types.rs:
//! - AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot
//! - BlockedTaskPolicy, BlockedHandlingConfig, BlockedHistoryEntry
//! - DecisionExecutionResult

pub mod blocked_handler;
pub mod types;

// Re-export main types for convenience
pub use blocked_handler::{BlockedHandler, AgentBlockedNotifier, AgentBlockedEvent, NoOpAgentBlockedNotifier};
pub use types::{
    AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot,
    BlockedTaskPolicy, BlockedHandlingConfig, BlockedHistoryEntry,
    DecisionExecutionResult,
};