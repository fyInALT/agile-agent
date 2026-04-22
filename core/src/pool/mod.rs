//! AgentPool domain modules
//!
//! Provides modular components for pool management:
//! - lifecycle: AgentLifecycleManager for spawn/stop/pause/resume operations
//! - task_assignment: TaskAssignmentCoordinator for assign/auto_assign/complete operations
//! - focus_manager: FocusManager for managing focused agent index
//! - queries: PoolQueries for read-only query operations
//! - decision_executor: DecisionExecutor for executing decision layer outputs
//! - blocked_handler: BlockedHandler for handling blocked agents
//! - decision_coordinator: WorkerDecisionRouter for decision layer state
//! - worktree_coordinator: WorktreeCoordinator for worktree management
//! - worktree_recovery: WorktreeRecovery for orphaned/idle worktree cleanup
//! - event_converter: Event conversion utilities for decision layer
//!
//! Types moved to pool/types.rs:
//! - AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot
//! - BlockedTaskPolicy, BlockedHandlingConfig, BlockedHistoryEntry
//! - DecisionExecutionResult

pub mod blocked_handler;
pub mod decision_coordinator;
pub mod decision_executor;
pub mod decision_interpreter;
pub mod decision_spawner;
// event_converter removed — conversion now handled by agent-events::From impls
pub mod focus_manager;
pub mod lifecycle;
pub mod queries;
pub mod task_assignment;
pub mod types;
pub mod worktree_coordinator;
pub mod worktree_recovery;

// Re-export main types for convenience
pub use blocked_handler::{BlockedHandler, AgentBlockedNotifier, AgentBlockedEvent, NoOpAgentBlockedNotifier};
pub use decision_coordinator::{WorkerDecisionRouter, DecisionAgentStats};
pub use decision_executor::DecisionExecutor;
pub use decision_interpreter::DecisionCommandInterpreter;
pub use decision_spawner::{spawn_decision_agent_for, spawn_decision_agent_with_profile_for, stop_decision_agent_for};

pub use focus_manager::{FocusManager, FocusError};
pub use lifecycle::{AgentLifecycleManager, LifecycleError};
pub use queries::PoolQueries;
pub use task_assignment::{TaskAssignmentCoordinator, AssignmentError};
pub use types::{
    AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot,
    BlockedTaskPolicy, BlockedHandlingConfig, BlockedHistoryEntry,
    DecisionExecutionResult,
};
pub use worktree_coordinator::WorktreeCoordinator;
pub use worktree_recovery::{WorktreeRecovery, WorktreeRecoveryReport, AgentPoolWorktreeError};