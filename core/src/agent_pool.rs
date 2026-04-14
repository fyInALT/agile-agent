//! AgentPool for managing multiple agent slots
//!
//! Central coordination structure for multi-agent runtime.

use std::collections::HashMap;

use crate::agent_runtime::{AgentId, AgentCodename, ProviderType, WorkplaceId};
use crate::agent_slot::{AgentSlot, AgentSlotStatus, TaskCompletionResult, TaskId};
use crate::backlog::{BacklogState, TaskStatus};
use crate::provider::ProviderKind;

/// Snapshot of an agent's status for display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatusSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub status: AgentSlotStatus,
    pub assigned_task_id: Option<TaskId>,
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

/// Pool managing multiple agent slots
pub struct AgentPool {
    /// All active agent slots
    slots: Vec<AgentSlot>,
    /// Max concurrent agents (configurable)
    max_slots: usize,
    /// Agent index counter for generating new IDs
    next_agent_index: usize,
    /// Index of the currently focused agent (for TUI)
    focused_slot: usize,
    /// Workplace ID for this pool
    workplace_id: WorkplaceId,
}

impl AgentPool {
    /// Create a new empty agent pool
    pub fn new(workplace_id: WorkplaceId, max_slots: usize) -> Self {
        Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            focused_slot: 0,
            workplace_id,
        }
    }

    /// Get the maximum number of slots
    pub fn max_slots(&self) -> usize {
        self.max_slots
    }

    /// Get the current number of active slots
    pub fn active_count(&self) -> usize {
        self.slots.len()
    }

    /// Check if pool can spawn more agents
    pub fn can_spawn(&self) -> bool {
        self.slots.len() < self.max_slots
    }

    /// Get the next agent index
    pub fn next_agent_index(&self) -> usize {
        self.next_agent_index
    }

    /// Get the focused slot index
    pub fn focused_slot_index(&self) -> usize {
        self.focused_slot
    }

    /// Get workplace ID
    pub fn workplace_id(&self) -> &WorkplaceId {
        &self.workplace_id
    }

    /// Generate a new unique agent ID
    fn generate_agent_id(&mut self) -> AgentId {
        let id = AgentId::new(format!("agent_{:03}", self.next_agent_index));
        self.next_agent_index += 1;
        id
    }

    /// Generate a codename for an agent
    fn generate_codename(index: usize) -> AgentCodename {
        const NAMES: &[&str] = &[
            "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india",
            "juliet", "kilo", "lima", "mike", "november", "oscar", "papa", "quebec", "romeo",
            "sierra", "tango", "uniform", "victor", "whiskey", "xray", "yankee", "zulu",
        ];
        let zero_based = index.saturating_sub(1);
        let base = NAMES[zero_based % NAMES.len()];
        let round = zero_based / NAMES.len();
        let name = if round == 0 {
            base.to_string()
        } else {
            format!("{base}-{}", round + 1)
        };
        AgentCodename::new(name)
    }

    /// Spawn a new agent with specified provider type (mock for foundation)
    ///
    /// Returns the new agent's ID on success, or error if pool is full.
    pub fn spawn_agent(&mut self, provider_kind: ProviderKind) -> Result<AgentId, String> {
        if !self.can_spawn() {
            return Err("Agent pool is full".to_string());
        }

        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        let slot = AgentSlot::new(agent_id.clone(), codename, provider_type);
        self.slots.push(slot);

        // Focus on the newly spawned agent if this is the first one
        if self.slots.len() == 1 {
            self.focused_slot = 0;
        }

        Ok(agent_id)
    }

    /// Stop a specific agent by ID
    ///
    /// Returns the slot index that was stopped.
    pub fn stop_agent(&mut self, agent_id: &AgentId) -> Result<usize, String> {
        let index = self.find_slot_index(agent_id)?;
        let slot = &mut self.slots[index];
        slot.transition_to(AgentSlotStatus::stopped("user requested"))
            .map_err(|e| format!("Failed to stop agent: {}", e))?;
        Ok(index)
    }

    /// Remove a stopped agent from the pool
    ///
    /// Only stopped agents can be removed.
    pub fn remove_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let index = self.find_slot_index(agent_id)?;
        let slot = &self.slots[index];
        if !slot.status().is_terminal() {
            return Err(format!(
                "Cannot remove agent with status {} (must be stopped)",
                slot.status().label()
            ));
        }
        self.slots.remove(index);
        // Adjust focus if necessary
        if self.focused_slot >= self.slots.len() && !self.slots.is_empty() {
            self.focused_slot = self.slots.len() - 1;
        }
        Ok(())
    }

    /// Get all agents with their current status
    pub fn agent_statuses(&self) -> Vec<AgentStatusSnapshot> {
        self.slots
            .iter()
            .map(|slot| AgentStatusSnapshot {
                agent_id: slot.agent_id().clone(),
                codename: slot.codename().clone(),
                provider_type: slot.provider_type(),
                status: slot.status().clone(),
                assigned_task_id: slot.assigned_task_id().cloned(),
            })
            .collect()
    }

    /// Switch focus to a different agent by index
    pub fn focus_agent_by_index(&mut self, index: usize) -> Result<(), String> {
        if index >= self.slots.len() {
            return Err(format!("Invalid focus index {} (only {} agents)", index, self.slots.len()));
        }
        self.focused_slot = index;
        Ok(())
    }

    /// Switch focus to a different agent by ID
    pub fn focus_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let index = self.find_slot_index(agent_id)?;
        self.focus_agent_by_index(index)
    }

    /// Get slot by index
    pub fn get_slot(&self, index: usize) -> Option<&AgentSlot> {
        self.slots.get(index)
    }

    /// Get slot by agent ID
    pub fn get_slot_by_id(&self, agent_id: &AgentId) -> Option<&AgentSlot> {
        self.slots.iter().find(|s| s.agent_id() == agent_id)
    }

    /// Get mutable slot by index
    pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut AgentSlot> {
        self.slots.get_mut(index)
    }

    /// Get mutable slot by agent ID
    pub fn get_slot_mut_by_id(&mut self, agent_id: &AgentId) -> Option<&mut AgentSlot> {
        self.slots.iter_mut().find(|s| s.agent_id() == agent_id)
    }

    /// Get the currently focused slot
    pub fn focused_slot(&self) -> Option<&AgentSlot> {
        self.slots.get(self.focused_slot)
    }

    /// Get the currently focused slot (mutable)
    pub fn focused_slot_mut(&mut self) -> Option<&mut AgentSlot> {
        self.slots.get_mut(self.focused_slot)
    }

    /// Find the index of a slot by agent ID
    fn find_slot_index(&self, agent_id: &AgentId) -> Result<usize, String> {
        self.slots
            .iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))
    }

    /// Assign a task to an idle agent
    pub fn assign_task(&mut self, agent_id: &AgentId, task_id: TaskId) -> Result<(), String> {
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;
        slot.assign_task(task_id)
    }

    /// Assign a task to an idle agent with backlog validation
    ///
    /// Validates that:
    /// - Agent exists and is idle
    /// - Task exists in backlog with Ready status
    /// - Updates backlog status to Running on success
    pub fn assign_task_with_backlog(
        &mut self,
        agent_id: &AgentId,
        task_id: TaskId,
        backlog: &mut BacklogState,
    ) -> Result<(), String> {
        // Validate task exists and is ready
        if !backlog.can_assign_task(task_id.as_str()) {
            return Err(format!(
                "Task {} cannot be assigned (not found or not ready)",
                task_id.as_str()
            ));
        }

        // Assign to agent
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;
        slot.assign_task(task_id.clone())?;

        // Update backlog status
        backlog.start_task(task_id.as_str());

        Ok(())
    }

    /// Complete a task for an agent with backlog update
    ///
    /// Updates backlog status based on completion result:
    /// - Success: task marked Done
    /// - Failure: task marked Failed
    pub fn complete_task_with_backlog(
        &mut self,
        agent_id: &AgentId,
        result: TaskCompletionResult,
        backlog: &mut BacklogState,
    ) -> Result<TaskId, String> {
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;

        // Get assigned task before clearing
        let task_id = slot
            .assigned_task_id()
            .cloned()
            .ok_or_else(|| format!("Agent {} has no assigned task", agent_id.as_str()))?;

        // Update backlog based on result
        match result {
            TaskCompletionResult::Success => {
                backlog.complete_task(task_id.as_str(), Some("Task completed successfully".to_string()));
            }
            TaskCompletionResult::Failure { error } => {
                backlog.fail_task(task_id.as_str(), error);
            }
        }

        // Clear assignment
        slot.clear_task();

        Ok(task_id)
    }

    /// Find an idle agent that can accept a task
    pub fn find_idle_agent(&self) -> Option<&AgentSlot> {
        self.slots.iter().find(|s| *s.status() == AgentSlotStatus::Idle)
    }

    /// Find an idle agent and return its ID for assignment
    pub fn find_idle_agent_id(&self) -> Option<AgentId> {
        self.slots
            .iter()
            .find(|s| *s.status() == AgentSlotStatus::Idle)
            .map(|s| s.agent_id().clone())
    }

    /// Auto-assign a ready task to an available idle agent
    ///
    /// Returns the assigned agent ID if successful.
    pub fn auto_assign_task(
        &mut self,
        backlog: &mut BacklogState,
    ) -> Option<(AgentId, TaskId)> {
        // Find an idle agent
        let agent_id = self.find_idle_agent_id()?;

        // Find a ready task
        let ready_tasks = backlog.ready_tasks();
        let ready_task = ready_tasks.first()?;
        let task_id_str = ready_task.id.clone();
        let task_id = TaskId::new(&task_id_str);

        // Attempt assignment
        self.assign_task_with_backlog(&agent_id, task_id.clone(), backlog)
            .ok()
            .map(|_| (agent_id, task_id))
    }

    /// Check if any agent is active (responding or executing)
    pub fn has_active_agents(&self) -> bool {
        self.slots.iter().any(|s| s.status().is_active())
    }

    /// Count agents by status type
    pub fn count_by_status(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for slot in &self.slots {
            let label = slot.status().label();
            *counts.entry(label).or_insert(0) += 1;
        }
        counts
    }

    /// Generate a snapshot of the task queue state for TUI display
    ///
    /// Combines backlog state with agent pool state for comprehensive view.
    pub fn task_queue_snapshot(&self, backlog: &BacklogState) -> TaskQueueSnapshot {
        let counts = backlog.count_tasks_by_status();

        // Collect agent assignments
        let agent_assignments: Vec<AgentTaskAssignment> = self
            .slots
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
        let available_agents = self
            .slots
            .iter()
            .filter(|s| *s.status() == AgentSlotStatus::Idle)
            .count();
        let active_agents = self.slots.iter().filter(|s| s.status().is_active()).count();

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
    pub fn agents_with_assignments(&self, backlog: &BacklogState) -> Vec<AgentTaskAssignment> {
        self.slots
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_slot::AgentSlotStatus;

    fn make_pool(max_slots: usize) -> AgentPool {
        AgentPool::new(WorkplaceId::new("workplace-001"), max_slots)
    }

    #[test]
    fn pool_new_is_empty() {
        let pool = make_pool(4);
        assert_eq!(pool.active_count(), 0);
        assert!(pool.can_spawn());
        assert_eq!(pool.max_slots(), 4);
    }

    #[test]
    fn spawn_agent_creates_slot() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        assert_eq!(pool.active_count(), 1);
        assert!(pool.get_slot_by_id(&id).is_some());
    }

    #[test]
    fn spawn_multiple_agents_unique_ids() {
        let mut pool = make_pool(4);
        let id1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id2 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        let id3 = pool.spawn_agent(ProviderKind::Codex).unwrap();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_eq!(pool.active_count(), 3);
    }

    #[test]
    fn spawn_until_full_then_fail() {
        let mut pool = make_pool(2);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        let result = pool.spawn_agent(ProviderKind::Codex);
        assert!(result.is_err());
        assert_eq!(pool.active_count(), 2);
    }

    #[test]
    fn stop_agent_marks_stopped() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.stop_agent(&id).unwrap();
        let slot = pool.get_slot_by_id(&id).unwrap();
        assert!(slot.status().is_terminal());
    }

    #[test]
    fn stop_nonexistent_agent_fails() {
        let mut pool = make_pool(4);
        let result = pool.stop_agent(&AgentId::new("agent_999"));
        assert!(result.is_err());
    }

    #[test]
    fn remove_stopped_agent() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.stop_agent(&id).unwrap();
        pool.remove_agent(&id).unwrap();
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn remove_active_agent_fails() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        // Agent is Idle, not stopped
        let result = pool.remove_agent(&id);
        assert!(result.is_err());
    }

    #[test]
    fn agent_statuses_snapshot() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        let statuses = pool.agent_statuses();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].status, AgentSlotStatus::Idle);
    }

    #[test]
    fn focus_agent_by_index() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent_by_index(1).unwrap();
        assert_eq!(pool.focused_slot_index(), 1);
    }

    #[test]
    fn focus_agent_by_id() {
        let mut pool = make_pool(4);
        let _id1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id2 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent(&id2).unwrap();
        assert_eq!(pool.focused_slot_index(), 1);
    }

    #[test]
    fn focus_invalid_index_fails() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let result = pool.focus_agent_by_index(5);
        assert!(result.is_err());
    }

    #[test]
    fn get_slot_by_index() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let slot = pool.get_slot(0).unwrap();
        assert_eq!(slot.agent_id().as_str(), "agent_001");
    }

    #[test]
    fn get_slot_by_id() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let slot = pool.get_slot_by_id(&id).unwrap();
        assert_eq!(slot.agent_id(), &id);
    }

    #[test]
    fn focused_slot() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let focused = pool.focused_slot().unwrap();
        assert_eq!(focused.agent_id().as_str(), "agent_001");
    }

    #[test]
    fn assign_task_to_idle_agent() {
        let mut pool = make_pool(4);
        let id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.assign_task(&id, TaskId::new("task-001")).unwrap();
        let slot = pool.get_slot_by_id(&id).unwrap();
        assert!(slot.assigned_task_id().is_some());
    }

    #[test]
    fn find_idle_agent() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let idle = pool.find_idle_agent().unwrap();
        assert_eq!(idle.status(), &AgentSlotStatus::Idle);
    }

    #[test]
    fn has_active_agents() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        // All agents are Idle initially
        assert!(!pool.has_active_agents());
    }

    #[test]
    fn count_by_status() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        let counts = pool.count_by_status();
        assert_eq!(counts.get("idle"), Some(&2));
    }

    #[test]
    fn codename_generation() {
        let mut pool = make_pool(4);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.spawn_agent(ProviderKind::Codex).unwrap();
        let slot0 = pool.get_slot(0).unwrap();
        let slot1 = pool.get_slot(1).unwrap();
        let slot2 = pool.get_slot(2).unwrap();
        assert_eq!(slot0.codename().as_str(), "alpha");
        assert_eq!(slot1.codename().as_str(), "bravo");
        assert_eq!(slot2.codename().as_str(), "charlie");
    }

    #[test]
    fn remove_adjusts_focus() {
        let mut pool = make_pool(4);
        let _id0 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id1 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent_by_index(1).unwrap();
        pool.stop_agent(&id1).unwrap();
        pool.remove_agent(&id1).unwrap();
        // Focus should adjust to valid index
        assert_eq!(pool.focused_slot_index(), 0);
    }

    // Backlog Integration Tests

    fn make_backlog_with_ready_task() -> BacklogState {
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Test objective".to_string(),
            scope: "Test scope".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });
        backlog
    }

    #[test]
    fn assign_task_with_backlog_updates_status() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task with backlog validation
        let result = pool.assign_task_with_backlog(
            &agent_id,
            TaskId::new("task-001"),
            &mut backlog,
        );
        assert!(result.is_ok());

        // Agent should have task assigned
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_some());

        // Backlog task should be Running
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Running);
    }

    #[test]
    fn assign_task_with_backlog_fails_for_non_ready_task() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Test".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Running, // Already running
            result_summary: None,
        });

        let result = pool.assign_task_with_backlog(
            &agent_id,
            TaskId::new("task-001"),
            &mut backlog,
        );
        assert!(result.is_err());
    }

    #[test]
    fn complete_task_with_backlog_success() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog).unwrap();

        // Complete task successfully
        let completed_task = pool.complete_task_with_backlog(
            &agent_id,
            TaskCompletionResult::Success,
            &mut backlog,
        );
        assert!(completed_task.is_ok());

        // Task should be Done in backlog
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Done);

        // Agent should have no assigned task
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn complete_task_with_backlog_failure() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog).unwrap();

        // Complete task with failure
        let completed_task = pool.complete_task_with_backlog(
            &agent_id,
            TaskCompletionResult::Failure { error: "test error".to_string() },
            &mut backlog,
        );
        assert!(completed_task.is_ok());

        // Task should be Failed in backlog
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Failed);
        assert_eq!(task.result_summary, Some("test error".to_string()));
    }

    #[test]
    fn auto_assign_task_assigns_to_idle_agent() {
        let mut pool = make_pool(2);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Auto-assign should work
        let result = pool.auto_assign_task(&mut backlog);
        assert!(result.is_some());

        let (_agent_id, task_id) = result.unwrap();
        assert_eq!(task_id.as_str(), "task-001");

        // Task should be Running
        let task = backlog.find_task("task-001").unwrap();
        assert_eq!(task.status, crate::backlog::TaskStatus::Running);
    }

    #[test]
    fn auto_assign_task_returns_none_when_no_idle_agents() {
        let mut pool = make_pool(1);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        // Manually mark agent as starting (not idle)
        // Idle -> Starting is valid, then Starting -> Responding
        pool.get_slot_mut_by_id(&agent_id)
            .unwrap()
            .transition_to(AgentSlotStatus::starting())
            .unwrap();
        let mut backlog = make_backlog_with_ready_task();

        let result = pool.auto_assign_task(&mut backlog);
        assert!(result.is_none());
    }

    #[test]
    fn auto_assign_task_returns_none_when_no_ready_tasks() {
        let mut pool = make_pool(2);
        pool.spawn_agent(ProviderKind::Mock).unwrap();
        let backlog = BacklogState::default(); // No tasks

        let result = pool.auto_assign_task(&mut backlog.clone());
        assert!(result.is_none());
    }

    // Task Queue Visualization Tests

    #[test]
    fn task_queue_snapshot_empty_backlog() {
        let pool = make_pool(2);
        let backlog = BacklogState::default();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.total_tasks, 0);
        assert_eq!(snapshot.ready_tasks, 0);
        assert_eq!(snapshot.running_tasks, 0);
        assert_eq!(snapshot.agent_assignments.len(), 0);
    }

    #[test]
    fn task_queue_snapshot_with_tasks() {
        let pool = make_pool(2);
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-002".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 2".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Running,
            result_summary: None,
        });
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-003".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 3".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Done,
            result_summary: Some("Completed".to_string()),
        });

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.total_tasks, 3);
        assert_eq!(snapshot.ready_tasks, 1);
        assert_eq!(snapshot.running_tasks, 1);
        assert_eq!(snapshot.completed_tasks, 1);
    }

    #[test]
    fn task_queue_snapshot_with_agent_assignments() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });

        // Assign task to agent
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog).unwrap();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.agent_assignments.len(), 1);
        assert_eq!(snapshot.agent_assignments[0].task_id.as_str(), "task-001");
        assert_eq!(snapshot.agent_assignments[0].task_status, crate::backlog::TaskStatus::Running);
        assert_eq!(snapshot.running_tasks, 1);
    }

    #[test]
    fn task_queue_snapshot_available_agents_count() {
        let mut pool = make_pool(3);
        let _agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let _agent3 = pool.spawn_agent(ProviderKind::Mock).unwrap();

        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });

        // Assign task to agent2 (agent status stays Idle)
        pool.assign_task_with_backlog(&agent2, TaskId::new("task-001"), &mut backlog).unwrap();

        // Now mark agent2 as starting (not idle)
        pool.get_slot_mut_by_id(&agent2)
            .unwrap()
            .transition_to(AgentSlotStatus::starting())
            .unwrap();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.available_agents, 2); // agent1 and agent3 are idle
        assert_eq!(snapshot.active_agents, 1); // Starting is active
    }

    #[test]
    fn agents_with_assignments_returns_assigned_agents() {
        let mut pool = make_pool(2);
        let agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let _agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = BacklogState::default();
        backlog.push_task(crate::backlog::TaskItem {
            id: "task-001".to_string(),
            todo_id: "todo-001".to_string(),
            objective: "Task 1".to_string(),
            scope: "Test".to_string(),
            constraints: Vec::new(),
            verification_plan: Vec::new(),
            status: crate::backlog::TaskStatus::Ready,
            result_summary: None,
        });

        pool.assign_task_with_backlog(&agent1, TaskId::new("task-001"), &mut backlog).unwrap();

        let assignments = pool.agents_with_assignments(&backlog);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_id, agent1);
    }
}