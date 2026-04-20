//! Task assignment coordinator for managing task assignments
//!
//! Provides TaskAssignmentCoordinator that coordinates task assignment
//! operations between agents and backlog. This module extracts task
//! assignment methods from AgentPool to improve execution flow clarity.

use crate::agent_runtime::AgentId;
use crate::agent_slot::{AgentSlot, AgentSlotStatus, TaskCompletionResult, TaskId};
use crate::backlog::BacklogState;
use crate::logging;

/// Error type for task assignment operations
#[derive(Debug)]
pub enum AssignmentError {
    /// Agent not found in pool
    AgentNotFound(String),
    /// Task not found or not ready
    TaskNotReady(String),
    /// Agent not idle
    AgentNotIdle(String),
    /// Agent has no assigned task
    NoAssignedTask(String),
    /// Slot transition failed
    SlotTransitionError(String),
}

impl std::fmt::Display for AssignmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssignmentError::AgentNotFound(id) => write!(f, "Agent {} not found in pool", id),
            AssignmentError::TaskNotReady(id) => write!(f, "Task {} cannot be assigned (not found or not ready)", id),
            AssignmentError::AgentNotIdle(id) => write!(f, "Agent {} is not idle", id),
            AssignmentError::NoAssignedTask(id) => write!(f, "Agent {} has no assigned task", id),
            AssignmentError::SlotTransitionError(msg) => write!(f, "Slot transition failed: {}", msg),
        }
    }
}

impl std::error::Error for AssignmentError {}

/// Task assignment coordinator - coordinates task assignment operations
///
/// This struct provides task assignment methods that operate on slots and backlog.
/// It's a pure coordination module with no internal state.
pub struct TaskAssignmentCoordinator;

impl TaskAssignmentCoordinator {
    /// Assign a task to an agent (simple)
    ///
    /// Basic assignment without backlog validation.
    pub fn assign(
        slots: &mut [AgentSlot],
        agent_id: &AgentId,
        task_id: TaskId,
    ) -> Result<(), AssignmentError> {
        let slot = slots.iter_mut()
            .find(|s| s.agent_id() == agent_id)
            .ok_or_else(|| AssignmentError::AgentNotFound(agent_id.as_str().to_string()))?;

        let codename = slot.codename().clone();
        slot.assign_task(task_id.clone()).map_err(|e| {
            logging::debug_event(
                "pool.task.assign.failed",
                "failed to assign task",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "codename": codename.as_str(),
                    "task_id": task_id.as_str(),
                    "reason": e,
                }),
            );
            AssignmentError::SlotTransitionError(e)
        })?;

        logging::debug_event(
            "pool.task.assign",
            "assigned task to agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "task_id": task_id.as_str(),
            }),
        );

        Ok(())
    }

    /// Assign a task to an agent with backlog validation
    ///
    /// Validates that task exists and is ready before assignment.
    /// Updates backlog status to Running on success.
    pub fn assign_with_backlog(
        slots: &mut [AgentSlot],
        agent_id: &AgentId,
        task_id: TaskId,
        backlog: &mut BacklogState,
    ) -> Result<(), AssignmentError> {
        // Validate task exists and is ready
        if !backlog.can_assign_task(task_id.as_str()) {
            logging::debug_event(
                "pool.task.assign.failed",
                "failed to assign task - task not ready",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "task_id": task_id.as_str(),
                    "reason": "task_not_ready_or_not_found",
                }),
            );
            return Err(AssignmentError::TaskNotReady(task_id.as_str().to_string()));
        }

        // Assign to agent
        let slot = slots.iter_mut()
            .find(|s| s.agent_id() == agent_id)
            .ok_or_else(|| AssignmentError::AgentNotFound(agent_id.as_str().to_string()))?;

        let codename = slot.codename().clone();
        slot.assign_task(task_id.clone()).map_err(|e| {
            logging::debug_event(
                "pool.task.assign.failed",
                "failed to assign task",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "codename": codename.as_str(),
                    "task_id": task_id.as_str(),
                    "reason": e,
                }),
            );
            AssignmentError::SlotTransitionError(e)
        })?;

        // Update backlog status
        backlog.start_task(task_id.as_str());

        logging::debug_event(
            "pool.task.assign",
            "assigned task with backlog update",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "task_id": task_id.as_str(),
                "old_status": "ready",
                "new_status": "running",
            }),
        );

        Ok(())
    }

    /// Complete a task and update backlog
    ///
    /// Updates backlog status based on completion result.
    pub fn complete(
        slots: &mut [AgentSlot],
        agent_id: &AgentId,
        result: TaskCompletionResult,
        backlog: &mut BacklogState,
    ) -> Result<TaskId, AssignmentError> {
        let slot = slots.iter_mut()
            .find(|s| s.agent_id() == agent_id)
            .ok_or_else(|| AssignmentError::AgentNotFound(agent_id.as_str().to_string()))?;

        // Get assigned task before clearing
        let task_id = slot
            .assigned_task_id()
            .cloned()
            .ok_or_else(|| AssignmentError::NoAssignedTask(agent_id.as_str().to_string()))?;

        let codename = slot.codename().clone();

        // Update backlog based on result
        match &result {
            TaskCompletionResult::Success => {
                backlog.complete_task(
                    task_id.as_str(),
                    Some("Task completed successfully".to_string()),
                );
            }
            TaskCompletionResult::Failure { error } => {
                backlog.fail_task(task_id.as_str(), error.clone());
            }
        }

        // Clear assignment
        slot.clear_task();

        logging::debug_event(
            "pool.task.complete",
            "completed task",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "task_id": task_id.as_str(),
                "result": match result {
                    TaskCompletionResult::Success => "success",
                    TaskCompletionResult::Failure { .. } => "failure",
                },
                "old_status": "running",
                "new_status": match result {
                    TaskCompletionResult::Success => "done",
                    TaskCompletionResult::Failure { .. } => "failed",
                },
            }),
        );

        Ok(task_id)
    }

    /// Find an idle agent in the pool
    pub fn find_idle(slots: &[AgentSlot]) -> Option<&AgentSlot> {
        slots.iter().find(|s| *s.status() == AgentSlotStatus::Idle)
    }

    /// Find an idle agent ID
    pub fn find_idle_id(slots: &[AgentSlot]) -> Option<AgentId> {
        slots.iter()
            .find(|s| *s.status() == AgentSlotStatus::Idle)
            .map(|s| s.agent_id().clone())
    }

    /// Auto-assign a ready task to an idle agent
    ///
    /// Returns the assigned (agent_id, task_id) pair if successful.
    pub fn auto_assign(
        slots: &mut [AgentSlot],
        backlog: &mut BacklogState,
    ) -> Option<(AgentId, TaskId)> {
        // Find an idle agent
        let agent_id = Self::find_idle_id(slots)?;

        // Find a ready task
        let ready_tasks = backlog.ready_tasks();
        let ready_task = ready_tasks.first()?;
        let task_id = TaskId::new(&ready_task.id);

        // Attempt assignment
        match Self::assign_with_backlog(slots, &agent_id, task_id.clone(), backlog) {
            Ok(()) => {
                logging::debug_event(
                    "pool.task.auto_assign",
                    "auto-assigned task to idle agent",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "task_id": task_id.as_str(),
                    }),
                );
                Some((agent_id, task_id))
            }
            Err(e) => {
                logging::debug_event(
                    "pool.task.auto_assign.failed",
                    "failed to auto-assign task",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "task_id": task_id.as_str(),
                        "error": e.to_string(),
                    }),
                );
                None
            }
        }
    }

    /// Count agents by status
    pub fn count_by_status(slots: &[AgentSlot]) -> std::collections::HashMap<String, usize> {
        let mut counts = std::collections::HashMap::new();
        for slot in slots {
            let status_label = slot.status().label();
            *counts.entry(status_label).or_insert(0) += 1;
        }
        counts
    }

    /// Count idle agents
    pub fn idle_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| *s.status() == AgentSlotStatus::Idle).count()
    }

    /// Count busy agents (working, blocked, etc.)
    pub fn busy_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| !s.status().is_idle()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::AgentId;
    use crate::agent_slot::AgentSlot;
    use crate::ProviderKind;

    fn make_slot(agent_id: &str, status: AgentSlotStatus) -> AgentSlot {
        let id = AgentId::new(agent_id);
        let codename = crate::agent_runtime::AgentCodename::new("TEST");
        let provider_type = crate::agent_runtime::ProviderType::from_provider_kind(ProviderKind::Mock);
        let mut slot = AgentSlot::new(id, codename, provider_type);
        if status != AgentSlotStatus::Idle {
            let _ = slot.transition_to(status);
        }
        slot
    }

    #[test]
    fn assignment_error_display() {
        let err = AssignmentError::AgentNotFound("test-agent".to_string());
        assert!(err.to_string().contains("test-agent"));

        let err = AssignmentError::TaskNotReady("task-1".to_string());
        assert!(err.to_string().contains("task-1"));
    }

    #[test]
    fn find_idle_empty_slots() {
        let slots: Vec<AgentSlot> = vec![];
        assert!(TaskAssignmentCoordinator::find_idle(&slots).is_none());
        assert!(TaskAssignmentCoordinator::find_idle_id(&slots).is_none());
    }

    #[test]
    fn find_idle_single_idle_slot() {
        let slots = vec![make_slot("agent-1", AgentSlotStatus::Idle)];
        let found = TaskAssignmentCoordinator::find_idle(&slots);
        assert!(found.is_some());
        assert_eq!(found.unwrap().agent_id().as_str(), "agent-1");

        let id = TaskAssignmentCoordinator::find_idle_id(&slots);
        assert!(id.is_some());
        assert_eq!(id.unwrap().as_str(), "agent-1");
    }

    #[test]
    fn find_idle_no_idle_slots() {
        // Use Blocked status which can be transitioned from Idle
        let slots = vec![make_slot("agent-1", AgentSlotStatus::blocked("test"))];
        assert!(TaskAssignmentCoordinator::find_idle(&slots).is_none());
    }

    #[test]
    fn count_by_status() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::Idle),
            make_slot("agent-3", AgentSlotStatus::blocked("test")),
        ];
        let counts = TaskAssignmentCoordinator::count_by_status(&slots);
        assert_eq!(counts.get("idle"), Some(&2));
        // Blocked label includes reason: "blocked:test"
        assert_eq!(counts.get("blocked:test"), Some(&1));
    }

    #[test]
    fn idle_count() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::blocked("test")),
            make_slot("agent-3", AgentSlotStatus::Idle),
        ];
        assert_eq!(TaskAssignmentCoordinator::idle_count(&slots), 2);
        assert_eq!(TaskAssignmentCoordinator::busy_count(&slots), 1);
    }

    #[test]
    fn assign_fails_for_missing_agent() {
        let mut slots = vec![make_slot("agent-1", AgentSlotStatus::Idle)];
        let task_id = TaskId::new("task-1");
        let result = TaskAssignmentCoordinator::assign(&mut slots, &AgentId::new("agent-2"), task_id);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AssignmentError::AgentNotFound(_)));
    }

    #[test]
    fn assign_success_for_idle_agent() {
        let mut slots = vec![make_slot("agent-1", AgentSlotStatus::Idle)];
        let task_id = TaskId::new("task-1");
        let result = TaskAssignmentCoordinator::assign(&mut slots, &AgentId::new("agent-1"), task_id.clone());
        assert!(result.is_ok());
        assert_eq!(slots[0].assigned_task_id(), Some(&task_id));
    }

    #[test]
    fn complete_fails_for_missing_agent() {
        let mut slots = vec![make_slot("agent-1", AgentSlotStatus::Idle)];
        let result = TaskAssignmentCoordinator::complete(
            &mut slots,
            &AgentId::new("agent-2"),
            TaskCompletionResult::Success,
            &mut BacklogState::default(),
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AssignmentError::AgentNotFound(_)));
    }

    #[test]
    fn complete_fails_for_no_assigned_task() {
        let mut slots = vec![make_slot("agent-1", AgentSlotStatus::Idle)];
        let result = TaskAssignmentCoordinator::complete(
            &mut slots,
            &AgentId::new("agent-1"),
            TaskCompletionResult::Success,
            &mut BacklogState::default(),
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AssignmentError::NoAssignedTask(_)));
    }
}