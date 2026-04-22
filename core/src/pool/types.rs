//! Types for AgentPool domain
//!
//! Contains snapshot types, configuration types, and result types
//! that are used across pool management.

use std::path::PathBuf;

use crate::agent_role::AgentRole;
use crate::agent_runtime::{AgentCodename, AgentId, ProviderType};
use crate::agent_slot::{AgentSlotStatus, TaskId};
use crate::backlog::TaskStatus;
use agent_decision::HumanDecisionTimeoutConfig;

/// Event emitted when an agent becomes blocked
#[derive(Debug, Clone)]
pub struct AgentBlockedEvent {
    /// The blocked agent ID
    pub agent_id: AgentId,
    /// The reason type
    pub reason_type: String,
    /// Human readable description
    pub description: String,
    /// Urgency level
    pub urgency: String,
}

/// Notifier trait for agent blocked events
///
/// Implement this trait to receive notifications when agents become blocked.
/// This enables other agents or systems to react to blocking events.
pub trait AgentBlockedNotifier: Send + Sync {
    /// Called when an agent becomes blocked
    fn on_agent_blocked(&self, event: AgentBlockedEvent);
}

/// No-op notifier that does nothing
#[derive(Debug, Clone, Default)]
pub struct NoOpAgentBlockedNotifier;

impl AgentBlockedNotifier for NoOpAgentBlockedNotifier {
    fn on_agent_blocked(&self, _event: AgentBlockedEvent) {
        // Do nothing
    }
}

/// Snapshot of an agent's status for display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatusSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub role: AgentRole,
    pub status: AgentSlotStatus,
    pub assigned_task_id: Option<TaskId>,
    /// Worktree branch name (if agent has worktree)
    pub worktree_branch: Option<String>,
    /// Whether agent has a worktree
    pub has_worktree: bool,
    /// Whether worktree directory exists on disk
    pub worktree_exists: bool,
}

/// Per-agent task assignment info for visualization
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTaskAssignment {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub task_id: TaskId,
    pub task_status: TaskStatus,
}

/// Snapshot of task queue state for TUI display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskQueueSnapshot {
    /// Total number of tasks in backlog
    pub total_tasks: usize,
    /// Number of tasks ready to be assigned
    pub ready_tasks: usize,
    /// Number of tasks currently running
    pub running_tasks: usize,
    /// Number of tasks completed successfully
    pub completed_tasks: usize,
    /// Number of tasks that failed
    pub failed_tasks: usize,
    /// Number of tasks that are blocked
    pub blocked_tasks: usize,
    /// Tasks assigned to specific agents
    pub agent_assignments: Vec<AgentTaskAssignment>,
    /// Number of idle agents available for assignment
    pub available_agents: usize,
    /// Number of active agents (responding/executing)
    pub active_agents: usize,
}

/// Policy for handling tasks when agent becomes blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum BlockedTaskPolicy {
    /// Task stays assigned to blocked agent
    KeepAssigned,
    /// Reassign task to another idle agent if available
    #[default]
    ReassignIfPossible,
    /// Mark task as waiting in backlog
    MarkWaiting,
}


/// Blocked handling configuration
#[derive(Debug, Clone)]
pub struct BlockedHandlingConfig {
    /// Task policy when agent blocked
    pub task_policy: BlockedTaskPolicy,
    /// Human decision timeout config
    pub timeout_config: HumanDecisionTimeoutConfig,
    /// Notify other agents when blocked
    pub notify_others: bool,
    /// Record blocked history
    pub record_history: bool,
    /// Maximum history entries (0 = unlimited)
    pub max_history_entries: usize,
}

impl Default for BlockedHandlingConfig {
    fn default() -> Self {
        Self {
            task_policy: BlockedTaskPolicy::default(),
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: true,
            record_history: true,
            max_history_entries: 1000,
        }
    }
}

/// Record of agent blocking history
#[derive(Debug, Clone)]
pub struct BlockedHistoryEntry {
    /// Agent ID
    pub agent_id: AgentId,
    /// Blocking reason type
    pub reason_type: String,
    /// Blocking description
    pub description: String,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Whether it was resolved
    pub resolved: bool,
    /// Resolution method
    pub resolution: Option<String>,
}

/// Decision execution result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionExecutionResult {
    /// Selection executed successfully
    Executed { option_id: String },
    /// Recommendation accepted
    AcceptedRecommendation,
    /// Custom instruction sent
    CustomInstruction { instruction: String },
    /// Task skipped
    Skipped,
    /// Operation cancelled
    Cancelled,
    /// Agent not found
    AgentNotFound,
    /// Agent not in blocked state
    NotBlocked,
    /// Task preparation succeeded
    TaskPrepared { branch: String, worktree_path: PathBuf },
    /// Task preparation failed
    PreparationFailed { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_task_policy_default() {
        let policy = BlockedTaskPolicy::default();
        assert_eq!(policy, BlockedTaskPolicy::ReassignIfPossible);
    }

    #[test]
    fn blocked_handling_config_default() {
        let config = BlockedHandlingConfig::default();
        assert!(config.notify_others);
        assert!(config.record_history);
        assert_eq!(config.max_history_entries, 1000);
    }

    #[test]
    fn agent_blocked_event_creation() {
        let event = AgentBlockedEvent {
            agent_id: AgentId::new("agent-001"),
            reason_type: "human_decision".to_string(),
            description: "Waiting for approval".to_string(),
            urgency: "high".to_string(),
        };
        assert_eq!(event.agent_id.as_str(), "agent-001");
        assert_eq!(event.reason_type, "human_decision");
    }

    #[test]
    fn no_op_notifier_does_not_panic() {
        let notifier = NoOpAgentBlockedNotifier::default();
        let event = AgentBlockedEvent {
            agent_id: AgentId::new("test"),
            reason_type: "test".to_string(),
            description: "test".to_string(),
            urgency: "low".to_string(),
        };
        notifier.on_agent_blocked(event); // Should not panic
    }

    #[test]
    fn task_queue_snapshot_defaults() {
        let snapshot = TaskQueueSnapshot {
            total_tasks: 0,
            ready_tasks: 0,
            running_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            blocked_tasks: 0,
            agent_assignments: vec![],
            available_agents: 0,
            active_agents: 0,
        };
        assert_eq!(snapshot.total_tasks, 0);
    }

    #[test]
    fn decision_execution_result_variants() {
        let executed = DecisionExecutionResult::Executed { option_id: "opt-1".to_string() };
        let skipped = DecisionExecutionResult::Skipped;
        let cancelled = DecisionExecutionResult::Cancelled;

        assert!(executed != skipped);
        assert!(skipped != cancelled);
    }
}