//! Pool queries for pure read-only operations
//!
//! Provides PoolQueries that contains pure query methods that don't
//! modify pool state. This module separates read operations from
//! write operations for better clarity.

use std::collections::HashMap;

use crate::agent_runtime::AgentId;
use crate::agent_slot::{AgentSlot, AgentSlotStatus};
use crate::backlog::{BacklogState, TaskStatus};
use crate::pool::{AgentStatusSnapshot, AgentTaskAssignment, TaskQueueSnapshot};

/// PoolQueries - pure read-only query operations
///
/// This struct provides query methods that operate on pool state
/// without modifying it. All methods take slots/backlog as parameters.
pub struct PoolQueries;

impl PoolQueries {
    /// Get agent statuses snapshot
    ///
    /// Returns a snapshot of all agent statuses for UI display.
    pub fn agent_statuses(slots: &[AgentSlot]) -> Vec<AgentStatusSnapshot> {
        slots
            .iter()
            .map(|slot| AgentStatusSnapshot {
                agent_id: slot.agent_id().clone(),
                codename: slot.codename().clone(),
                provider_type: slot.provider_type(),
                role: slot.role(),
                status: slot.status().clone(),
                assigned_task_id: slot.assigned_task_id().cloned(),
                worktree_branch: slot.worktree_branch().cloned(),
                has_worktree: slot.has_worktree(),
                worktree_exists: slot.has_worktree() && slot.cwd().exists(),
            })
            .collect()
    }

    /// Count agents by status
    ///
    /// Returns a map of status labels to counts.
    pub fn count_by_status(slots: &[AgentSlot]) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for slot in slots {
            let label = slot.status().label();
            *counts.entry(label).or_insert(0) += 1;
        }
        counts
    }

    /// Generate task queue snapshot
    ///
    /// Combines backlog state with agent pool state for comprehensive view.
    pub fn task_queue_snapshot(slots: &[AgentSlot], backlog: &BacklogState) -> TaskQueueSnapshot {
        let counts = backlog.count_tasks_by_status();

        // Collect agent assignments
        let agent_assignments: Vec<AgentTaskAssignment> = slots
            .iter()
            .filter_map(|slot| {
                let task_id = slot.assigned_task_id()?;
                let task = backlog.find_task(task_id.as_str())?;
                Some(AgentTaskAssignment {
                    agent_id: slot.agent_id().clone(),
                    codename: slot.codename().clone(),
                    task_id: task_id.clone(),
                    task_status: task.status,
                })
            })
            .collect();

        // Count available and active agents
        let available_agents = slots
            .iter()
            .filter(|s| *s.status() == AgentSlotStatus::Idle)
            .count();
        let active_agents = slots.iter().filter(|s| s.status().is_active()).count();

        TaskQueueSnapshot {
            total_tasks: backlog.tasks.len(),
            ready_tasks: counts.get(&TaskStatus::Ready).copied().unwrap_or(0),
            running_tasks: counts.get(&TaskStatus::Running).copied().unwrap_or(0),
            completed_tasks: counts.get(&TaskStatus::Done).copied().unwrap_or(0),
            failed_tasks: counts.get(&TaskStatus::Failed).copied().unwrap_or(0),
            blocked_tasks: counts.get(&TaskStatus::Blocked).copied().unwrap_or(0),
            agent_assignments,
            available_agents,
            active_agents,
        }
    }

    /// Get agents with their assigned task info
    ///
    /// Returns assignments for agents that have a task assigned.
    pub fn agents_with_assignments(slots: &[AgentSlot], backlog: &BacklogState) -> Vec<AgentTaskAssignment> {
        slots
            .iter()
            .filter_map(|slot| {
                let task_id = slot.assigned_task_id()?;
                let task = backlog.find_task(task_id.as_str())?;
                Some(AgentTaskAssignment {
                    agent_id: slot.agent_id().clone(),
                    codename: slot.codename().clone(),
                    task_id: task_id.clone(),
                    task_status: task.status,
                })
            })
            .collect()
    }

    /// Count idle agents
    pub fn idle_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| *s.status() == AgentSlotStatus::Idle).count()
    }

    /// Count active agents
    pub fn active_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| s.status().is_active()).count()
    }

    /// Count busy agents (not idle)
    pub fn busy_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| !s.status().is_idle()).count()
    }

    /// Find an idle agent slot
    pub fn find_idle_slot(slots: &[AgentSlot]) -> Option<&AgentSlot> {
        slots.iter().find(|s| *s.status() == AgentSlotStatus::Idle)
    }

    /// Find an idle agent ID
    pub fn find_idle_agent_id(slots: &[AgentSlot]) -> Option<AgentId> {
        slots.iter()
            .find(|s| *s.status() == AgentSlotStatus::Idle)
            .map(|s| s.agent_id().clone())
    }

    /// Find a slot by agent ID
    pub fn find_slot_by_id<'a>(slots: &'a [AgentSlot], agent_id: &AgentId) -> Option<&'a AgentSlot> {
        slots.iter().find(|s| s.agent_id() == agent_id)
    }

    /// Find slot index by agent ID
    pub fn find_slot_index(slots: &[AgentSlot], agent_id: &AgentId) -> Option<usize> {
        slots.iter().position(|s| s.agent_id() == agent_id)
    }

    /// Get pending decision count
    ///
    /// Counts agents in BlockedForDecision status.
    pub fn pending_decision_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| s.status().is_blocked_for_human()).count()
    }

    /// Get blocked agent count
    ///
    /// Counts all blocked agents (Blocked, BlockedForDecision, Resting).
    pub fn blocked_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| s.status().is_blocked()).count()
    }

    /// Get error agent count
    pub fn error_count(slots: &[AgentSlot]) -> usize {
        slots.iter().filter(|s| matches!(s.status(), AgentSlotStatus::Error { .. })).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::{AgentId, AgentCodename, ProviderType};
    use crate::agent_slot::{AgentSlot, TaskId};
    use crate::backlog::TaskItem;
    use crate::ProviderKind;

    fn make_slot(agent_id: &str, status: AgentSlotStatus) -> AgentSlot {
        let id = AgentId::new(agent_id);
        let codename = AgentCodename::new("TEST");
        let provider_type = ProviderType::from_provider_kind(ProviderKind::Mock);
        let mut slot = AgentSlot::new(id, codename, provider_type);
        if status != AgentSlotStatus::Idle {
            let _ = slot.transition_to(status);
        }
        slot
    }

    fn make_slot_with_task(agent_id: &str, task_id: &str) -> AgentSlot {
        let mut slot = make_slot(agent_id, AgentSlotStatus::Idle);
        let _ = slot.assign_task(TaskId::new(task_id));
        slot
    }

    fn make_backlog_with_task(task_id: &str, status: TaskStatus) -> BacklogState {
        let mut backlog = BacklogState::default();
        backlog.push_task(TaskItem {
            id: task_id.to_string(),
            todo_id: "todo-1".to_string(),
            objective: "test".to_string(),
            scope: "test".to_string(),
            constraints: vec!["c1".to_string()],
            verification_plan: vec!["v1".to_string()],
            status,
            result_summary: None,
        });
        backlog
    }

    #[test]
    fn agent_statuses_empty() {
        let slots: Vec<AgentSlot> = vec![];
        let statuses = PoolQueries::agent_statuses(&slots);
        assert!(statuses.is_empty());
    }

    #[test]
    fn agent_statuses_with_slots() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::blocked("test")),
        ];
        let statuses = PoolQueries::agent_statuses(&slots);
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].agent_id.as_str(), "agent-1");
        assert_eq!(statuses[1].agent_id.as_str(), "agent-2");
    }

    #[test]
    fn count_by_status() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::Idle),
            make_slot("agent-3", AgentSlotStatus::blocked("test")),
        ];
        let counts = PoolQueries::count_by_status(&slots);
        assert_eq!(counts.get("idle"), Some(&2));
        assert_eq!(counts.get("blocked:test"), Some(&1));
    }

    #[test]
    fn idle_count() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::blocked("test")),
        ];
        assert_eq!(PoolQueries::idle_count(&slots), 1);
        assert_eq!(PoolQueries::busy_count(&slots), 1);
    }

    #[test]
    fn find_idle_slot() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::blocked("test")),
            make_slot("agent-2", AgentSlotStatus::Idle),
        ];
        let found = PoolQueries::find_idle_slot(&slots);
        assert!(found.is_some());
        assert_eq!(found.unwrap().agent_id().as_str(), "agent-2");
    }

    #[test]
    fn find_idle_slot_none() {
        let slots = vec![make_slot("agent-1", AgentSlotStatus::blocked("test"))];
        assert!(PoolQueries::find_idle_slot(&slots).is_none());
    }

    #[test]
    fn find_slot_by_id() {
        let slots = vec![make_slot("agent-1", AgentSlotStatus::Idle)];
        let found = PoolQueries::find_slot_by_id(&slots, &AgentId::new("agent-1"));
        assert!(found.is_some());
    }

    #[test]
    fn find_slot_index() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::Idle),
        ];
        let index = PoolQueries::find_slot_index(&slots, &AgentId::new("agent-2"));
        assert_eq!(index, Some(1));
    }

    #[test]
    fn task_queue_snapshot() {
        let backlog = make_backlog_with_task("task-1", TaskStatus::Running);
        let slots = vec![make_slot_with_task("agent-1", "task-1")];
        let snapshot = PoolQueries::task_queue_snapshot(&slots, &backlog);
        assert_eq!(snapshot.total_tasks, 1);
        assert_eq!(snapshot.running_tasks, 1);
        assert_eq!(snapshot.agent_assignments.len(), 1);
    }

    #[test]
    fn pending_decision_count() {
        let slots = vec![
            make_slot("agent-1", AgentSlotStatus::Idle),
            make_slot("agent-2", AgentSlotStatus::blocked("test")),
        ];
        assert_eq!(PoolQueries::pending_decision_count(&slots), 0);
    }
}