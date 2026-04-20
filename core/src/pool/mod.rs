//! AgentPool domain modules
//!
//! Provides modular components for pool management:
//! - blocked_handler: BlockedHandler for handling blocked agents
//! - decision_coordinator: DecisionAgentCoordinator for decision layer state
//! - worktree_coordinator: WorktreeCoordinator for worktree management
//!
//! Types moved to pool/types.rs:
//! - AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot
//! - BlockedTaskPolicy, BlockedHandlingConfig, BlockedHistoryEntry
//! - DecisionExecutionResult

pub mod blocked_handler;
pub mod decision_coordinator;
pub mod types;
pub mod worktree_coordinator;

// Re-export main types for convenience
pub use blocked_handler::{BlockedHandler, AgentBlockedNotifier, AgentBlockedEvent, NoOpAgentBlockedNotifier};
pub use decision_coordinator::{DecisionAgentCoordinator, DecisionAgentStats};
pub use types::{
    AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot,
    BlockedTaskPolicy, BlockedHandlingConfig, BlockedHistoryEntry,
    DecisionExecutionResult,
};
pub use worktree_coordinator::WorktreeCoordinator;