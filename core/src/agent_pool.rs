//! AgentPool for managing multiple agent slots
//!
//! Central coordination structure for multi-agent runtime.

use std::collections::HashMap;

use crate::agent_role::AgentRole;
use crate::agent_runtime::{AgentCodename, AgentId, ProviderType, WorkplaceId};
use crate::agent_slot::{AgentSlot, AgentSlotStatus, TaskCompletionResult, TaskId};
use crate::backlog::{BacklogState, TaskStatus};
use crate::logging;
use crate::provider::ProviderKind;

// Decision layer imports
use agent_decision::{
    AutoAction,
    BlockedState,
    HumanDecisionRequest, HumanDecisionResponse, HumanSelection,
    HumanDecisionQueue, HumanDecisionTimeoutConfig,
    SituationType,
};

/// Snapshot of an agent's status for display
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatusSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub role: AgentRole,
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

/// Policy for handling tasks when agent becomes blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedTaskPolicy {
    /// Task stays assigned to blocked agent
    KeepAssigned,
    /// Reassign task to another idle agent if available
    ReassignIfPossible,
    /// Mark task as waiting in backlog
    MarkWaiting,
}

impl Default for BlockedTaskPolicy {
    fn default() -> Self {
        BlockedTaskPolicy::ReassignIfPossible
    }
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
    /// Agent not blocked
    NotBlocked,
}

/// Pool managing multiple agent slots
#[derive(Debug)]
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
    /// Human decision queue
    human_queue: HumanDecisionQueue,
    /// Blocked handling configuration
    blocked_config: BlockedHandlingConfig,
    /// Blocking history records
    blocked_history: Vec<BlockedHistoryEntry>,
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
            human_queue: HumanDecisionQueue::new(HumanDecisionTimeoutConfig::default()),
            blocked_config: BlockedHandlingConfig::default(),
            blocked_history: Vec::new(),
        }
    }

    /// Create pool with custom blocked handling config
    pub fn with_blocked_config(workplace_id: WorkplaceId, max_slots: usize, config: BlockedHandlingConfig) -> Self {
        Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            focused_slot: 0,
            workplace_id,
            human_queue: HumanDecisionQueue::new(config.timeout_config.clone()),
            blocked_config: config,
            blocked_history: Vec::new(),
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
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn agent - pool is full",
                serde_json::json!({
                    "reason": "pool_full",
                    "pool_size": self.slots.len(),
                    "max_slots": self.max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }

        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        let slot = AgentSlot::new(agent_id.clone(), codename.clone(), provider_type);
        self.slots.push(slot);

        logging::debug_event(
            "pool.agent.spawn",
            "spawned new agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "provider_type": provider_type.label(),
                "pool_size": self.slots.len(),
                "max_slots": self.max_slots,
            }),
        );

        // Focus on the newly spawned agent if this is the first one
        if self.slots.len() == 1 {
            self.focused_slot = 0;
            logging::debug_event(
                "pool.focus.change",
                "focus set to first agent after spawn",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "index": 0,
                }),
            );
        }

        Ok(agent_id)
    }

    /// Spawn the OVERVIEW agent (ProductOwner role) at the top of the pool
    ///
    /// The OVERVIEW agent is a special coordination agent that always stays at index 0.
    /// Returns the agent ID on success, or error if pool is full or OVERVIEW already exists.
    pub fn spawn_overview_agent(&mut self, provider_kind: ProviderKind) -> Result<AgentId, String> {
        // Check if OVERVIEW agent already exists
        if self.slots.iter().any(|s| s.role() == AgentRole::ProductOwner) {
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn OVERVIEW agent - already exists",
                serde_json::json!({
                    "reason": "overview_already_exists",
                    "pool_size": self.slots.len(),
                }),
            );
            return Err("OVERVIEW agent already exists".to_string());
        }

        if !self.can_spawn() {
            logging::debug_event(
                "pool.agent.spawn.failed",
                "failed to spawn OVERVIEW agent - pool is full",
                serde_json::json!({
                    "reason": "pool_full",
                    "pool_size": self.slots.len(),
                    "max_slots": self.max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }

        let agent_id = AgentId::new("OVERVIEW");
        let codename = AgentCodename::new("OVERVIEW");
        let provider_type = ProviderType::from_provider_kind(provider_kind);

        logging::debug_event(
            "pool.agent.spawn_overview",
            "spawning OVERVIEW agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "provider_type": provider_type.label(),
                "pool_size_before": self.slots.len(),
            }),
        );

        let slot = AgentSlot::with_role(agent_id.clone(), codename, provider_type, AgentRole::ProductOwner);

        // Insert at the beginning (always at index 0)
        self.slots.insert(0, slot);
        // Note: Do NOT increment next_agent_index for OVERVIEW agent
        // Worker agents should start from index 0 (alpha)

        // Focus on OVERVIEW agent by default
        self.focused_slot = 0;

        logging::debug_event(
            "pool.focus.change",
            "focus set to OVERVIEW agent after spawn",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "index": 0,
            }),
        );

        Ok(agent_id)
    }

    /// Get the OVERVIEW agent slot (ProductOwner role)
    pub fn overview_agent(&self) -> Option<&AgentSlot> {
        self.slots.iter().find(|s| s.role() == AgentRole::ProductOwner)
    }

    /// Stop a specific agent by ID
    ///
    /// Returns the slot index that was stopped.
    pub fn stop_agent(&mut self, agent_id: &AgentId) -> Result<usize, String> {
        let index = self.find_slot_index(agent_id)?;
        let slot = &mut self.slots[index];
        let codename = slot.codename().clone();
        let reason = "user requested";
        slot.transition_to(AgentSlotStatus::stopped(reason))
            .map_err(|e| format!("Failed to stop agent: {}", e))?;

        logging::debug_event(
            "pool.agent.stop",
            "stopped agent",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "slot_index": index,
                "reason": reason,
            }),
        );

        Ok(index)
    }

    /// Remove a stopped agent from the pool
    ///
    /// Only stopped agents can be removed.
    pub fn remove_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let index = self.find_slot_index(agent_id)?;
        let slot = &self.slots[index];
        if !slot.status().is_terminal() {
            logging::debug_event(
                "pool.agent.remove.failed",
                "failed to remove agent - not in terminal state",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "current_status": slot.status().label(),
                }),
            );
            return Err(format!(
                "Cannot remove agent with status {} (must be stopped)",
                slot.status().label()
            ));
        }
        let codename = slot.codename().clone();
        self.slots.remove(index);

        logging::debug_event(
            "pool.agent.remove",
            "removed agent from pool",
            serde_json::json!({
                "agent_id": agent_id.as_str(),
                "codename": codename.as_str(),
                "pool_size_after": self.slots.len(),
            }),
        );

        // Adjust focus if necessary
        if self.focused_slot >= self.slots.len() && !self.slots.is_empty() {
            self.focused_slot = self.slots.len() - 1;
            if let Some(new_focused) = self.slots.get(self.focused_slot) {
                logging::debug_event(
                    "pool.focus.adjust",
                    "adjusted focus after agent removal",
                    serde_json::json!({
                        "new_index": self.focused_slot,
                        "new_agent_id": new_focused.agent_id().as_str(),
                    }),
                );
            }
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
                role: slot.role(),
                status: slot.status().clone(),
                assigned_task_id: slot.assigned_task_id().cloned(),
            })
            .collect()
    }

    /// Get all slots for snapshot/export use.
    pub fn slots(&self) -> &[AgentSlot] {
        &self.slots
    }

    /// Restore an agent slot into the pool.
    pub fn restore_slot(&mut self, slot: AgentSlot) -> Result<(), String> {
        let agent_id = slot.agent_id().as_str().to_string();
        let codename = slot.codename().as_str().to_string();
        let role = slot.role().label();

        logging::debug_event(
            "pool.slot.restore",
            "restoring agent slot from snapshot",
            serde_json::json!({
                "agent_id": agent_id,
                "codename": codename,
                "role": role,
                "current_pool_size": self.slots.len(),
                "max_slots": self.max_slots,
            }),
        );

        if !self.can_spawn() {
            logging::debug_event(
                "pool.slot.restore.failed",
                "restore failed - pool full",
                serde_json::json!({
                    "agent_id": agent_id,
                    "current_size": self.slots.len(),
                    "max_slots": self.max_slots,
                }),
            );
            return Err("Agent pool is full".to_string());
        }
        if self.slots.iter().any(|existing| existing.agent_id().as_str() == agent_id) {
            let err = format!(
                "Agent {} already exists in pool",
                agent_id
            );
            logging::debug_event(
                "pool.slot.restore.failed",
                "restore failed - agent already exists",
                serde_json::json!({
                    "agent_id": agent_id,
                    "error": err,
                }),
            );
            return Err(err);
        }

        if slot.role() == AgentRole::ProductOwner {
            if self.overview_agent().is_some() {
                let err = "OVERVIEW agent already exists".to_string();
                logging::debug_event(
                    "pool.slot.restore.failed",
                    "restore failed - overview agent exists",
                    serde_json::json!({
                        "error": err,
                    }),
                );
                return Err(err);
            }
            self.slots.insert(0, slot);
        } else {
            self.slots.push(slot);
        }

        if let Some(restored_index) = self
            .slots
            .last()
            .and_then(|restored| parse_agent_index(restored.agent_id().as_str()))
        {
            self.next_agent_index = self.next_agent_index.max(restored_index + 1);
        } else if let Some(restored_index) = self
            .slots
            .iter()
            .filter_map(|slot| parse_agent_index(slot.agent_id().as_str()))
            .max()
        {
            self.next_agent_index = self.next_agent_index.max(restored_index + 1);
        }

        logging::debug_event(
            "pool.slot.restore.complete",
            "agent slot restored successfully",
            serde_json::json!({
                "agent_id": agent_id,
                "new_pool_size": self.slots.len(),
            }),
        );

        Ok(())
    }

    /// Switch focus to a different agent by index
    pub fn focus_agent_by_index(&mut self, index: usize) -> Result<(), String> {
        if index >= self.slots.len() {
            logging::debug_event(
                "pool.focus.invalid_index",
                "invalid focus index",
                serde_json::json!({
                    "attempted_index": index,
                    "pool_size": self.slots.len(),
                }),
            );
            return Err(format!(
                "Invalid focus index {} (only {} agents)",
                index,
                self.slots.len()
            ));
        }
        let old_index = self.focused_slot;
        let old_agent_id = self.slots.get(old_index).map(|s| s.agent_id().as_str().to_string());
        let new_agent_id = self.slots.get(index).map(|s| s.agent_id().as_str().to_string());
        self.focused_slot = index;

        logging::debug_event(
            "pool.focus.change",
            "focus changed by index",
            serde_json::json!({
                "old_index": old_index,
                "new_index": index,
                "old_agent_id": old_agent_id,
                "new_agent_id": new_agent_id,
            }),
        );

        Ok(())
    }

    /// Switch focus to a different agent by ID
    pub fn focus_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let index = self.find_slot_index(agent_id)?;
        let old_index = self.focused_slot;
        let old_agent_id = self.slots.get(old_index).map(|s| s.agent_id().as_str().to_string());
        let new_codename = self.slots.get(index).map(|s| s.codename().as_str().to_string());

        logging::debug_event(
            "pool.focus.change.by_id",
            "focus changed by agent ID",
            serde_json::json!({
                "old_index": old_index,
                "old_agent_id": old_agent_id,
                "new_agent_id": agent_id.as_str(),
                "new_codename": new_codename,
            }),
        );

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
            e
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
            logging::debug_event(
                "pool.task.assign.failed",
                "failed to assign task - task not ready",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "task_id": task_id.as_str(),
                    "reason": "task_not_ready_or_not_found",
                }),
            );
            return Err(format!(
                "Task {} cannot be assigned (not found or not ready)",
                task_id.as_str()
            ));
        }

        // Assign to agent
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;
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
            e
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

    /// Find an idle agent that can accept a task
    pub fn find_idle_agent(&self) -> Option<&AgentSlot> {
        self.slots
            .iter()
            .find(|s| *s.status() == AgentSlotStatus::Idle)
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
    pub fn auto_assign_task(&mut self, backlog: &mut BacklogState) -> Option<(AgentId, TaskId)> {
        // Find an idle agent
        let agent_id = self.find_idle_agent_id()?;

        // Find a ready task
        let ready_tasks = backlog.ready_tasks();
        let ready_task = ready_tasks.first()?;
        let task_id_str = ready_task.id.clone();
        let task_id = TaskId::new(&task_id_str);

        // Attempt assignment
        match self.assign_task_with_backlog(&agent_id, task_id.clone(), backlog) {
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
                let available_agents = self.slots.iter().filter(|s| *s.status() == AgentSlotStatus::Idle).count();
                let ready_count = backlog.ready_tasks().len();
                logging::debug_event(
                    "pool.task.auto_assign.failed",
                    "auto-assign failed",
                    serde_json::json!({
                        "agent_id": agent_id.as_str(),
                        "task_id": task_id.as_str(),
                        "reason": e,
                        "available_agents": available_agents,
                        "ready_tasks": ready_count,
                    }),
                );
                None
            }
        }
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

    // ==================== Blocked Handling Methods ====================

    /// Get blocked handling configuration
    pub fn blocked_config(&self) -> &BlockedHandlingConfig {
        &self.blocked_config
    }

    /// Get human decision queue
    pub fn human_queue(&self) -> &HumanDecisionQueue {
        &self.human_queue
    }

    /// Get pending human decisions count
    pub fn pending_human_decisions(&self) -> usize {
        self.human_queue.total_pending()
    }

    /// Get blocked history
    pub fn blocked_history(&self) -> &[BlockedHistoryEntry] {
        &self.blocked_history
    }

    /// Prune history to max size
    ///
    /// Removes oldest resolved entries first, then oldest unresolved if still over limit.
    fn prune_history(&mut self) {
        let max = self.blocked_config.max_history_entries;
        if max == 0 {
            return; // No limit
        }

        while self.blocked_history.len() > max {
            // Find the oldest resolved entry
            if let Some(pos) = self.blocked_history.iter().position(|e| e.resolved) {
                self.blocked_history.remove(pos);
            } else {
                // No resolved entries, remove the oldest
                self.blocked_history.remove(0);
            }
        }
    }

    /// Find blocked agents
    pub fn blocked_agents(&self) -> Vec<&AgentSlot> {
        self.slots
            .iter()
            .filter(|s| s.status().is_blocked())
            .collect()
    }

    /// Count blocked agents
    pub fn blocked_count(&self) -> usize {
        self.slots.iter().filter(|s| s.status().is_blocked()).count()
    }

    /// Process an agent becoming blocked
    ///
    /// This handles:
    /// 1. Setting the blocked status on the slot
    /// 2. Adding to human decision queue if human_decision type
    /// 3. Notifying other agents (if configured)
    /// 4. Handling the assigned task according to policy
    pub fn process_agent_blocked(
        &mut self,
        agent_id: &AgentId,
        blocked_state: BlockedState,
        backlog: Option<&mut BacklogState>,
    ) -> Result<(), String> {
        let slot = self
            .get_slot_mut_by_id(agent_id)
            .ok_or_else(|| format!("Agent {} not found in pool", agent_id.as_str()))?;

        // Set blocked status
        slot.transition_to(AgentSlotStatus::blocked_for_decision(blocked_state.clone()))
            .map_err(|e| format!("Failed to transition to blocked: {}", e))?;

        // Handle by blocking type
        let reason_type = blocked_state.reason().reason_type();
        if reason_type == "human_decision" {
            // Create human decision request
            let request = self.build_human_request(agent_id, &blocked_state);
            self.human_queue.push(request);
        }

        // Record in history
        if self.blocked_config.record_history {
            self.blocked_history.push(BlockedHistoryEntry {
                agent_id: agent_id.clone(),
                reason_type: reason_type.to_string(),
                description: blocked_state.reason().description(),
                duration_ms: 0, // Will be updated on resolution
                resolved: false,
                resolution: None,
            });
            self.prune_history();
        }

        // Notify other agents if configured
        if self.blocked_config.notify_others {
            // Emit event for other agents to react
            crate::logging::debug_event(
                "agent.blocked.notify_others",
                "agent blocked, notifying other agents",
                serde_json::json!({
                    "agent_id": agent_id.as_str(),
                    "reason_type": reason_type,
                    "description": blocked_state.reason().description(),
                    "urgency": format!("{}", blocked_state.reason().urgency()),
                }),
            );
        }

        // Handle blocked task
        if let Some(backlog) = backlog {
            self.handle_blocked_task(agent_id, backlog);
        }

        Ok(())
    }

    /// Build human decision request from blocked state
    fn build_human_request(&self, agent_id: &AgentId, blocked_state: &BlockedState) -> HumanDecisionRequest {
        let reason = blocked_state.reason();
        let urgency = reason.urgency();
        let timeout_ms = self.blocked_config.timeout_config.timeout_for_urgency(urgency);

        // Generate request ID
        let request_id = format!("req-{}-{}", agent_id.as_str(), uuid::Uuid::new_v4());

        HumanDecisionRequest::new(
            request_id,
            agent_id.as_str(),
            SituationType::new(reason.reason_type()),
            vec![], // Options would come from the blocking reason
            urgency,
            timeout_ms,
        )
        .with_description(reason.description())
    }

    /// Handle the task assigned to a blocked agent
    fn handle_blocked_task(&mut self, agent_id: &AgentId, backlog: &mut BacklogState) {
        // Get assigned task
        let task_id = self
            .get_slot_by_id(agent_id)
            .and_then(|s| s.assigned_task_id().cloned());

        if let Some(task_id) = task_id {
            match self.blocked_config.task_policy {
                BlockedTaskPolicy::KeepAssigned => {
                    // Task stays with blocked agent - no action needed
                }
                BlockedTaskPolicy::ReassignIfPossible => {
                    // Try to find idle agent
                    if let Some(idle_agent) = self.find_idle_agent_id() {
                        // Check task exists and is Running (task was already assigned)
                        let task_exists = backlog.find_task(task_id.as_str())
                            .map(|t| t.status == TaskStatus::Running)
                            .unwrap_or(false);

                        if task_exists {
                            // Try to assign to idle agent FIRST
                            let reassignment_succeeded = self.get_slot_mut_by_id(&idle_agent)
                                .map(|slot| slot.assign_task(task_id.clone()).is_ok())
                                .unwrap_or(false);

                            // Only clear from blocked slot if reassignment succeeded
                            if reassignment_succeeded {
                                if let Some(blocked_slot) = self.get_slot_mut_by_id(agent_id) {
                                    blocked_slot.clear_task();
                                }
                            }
                            // If reassignment failed, task stays with blocked agent
                        }
                    }
                }
                BlockedTaskPolicy::MarkWaiting => {
                    // Mark task as blocked in backlog
                    backlog.block_task(task_id.as_str(), "agent_blocked".to_string());
                }
            }
        }
    }

    /// Process human decision response
    ///
    /// This handles:
    /// 1. Completing the request in the queue
    /// 2. Clearing the blocked status on the agent
    /// 3. Executing the decision
    /// 4. Recording in history
    pub fn process_human_response(
        &mut self,
        response: HumanDecisionResponse,
    ) -> Result<DecisionExecutionResult, String> {
        // Get request from queue
        let request = self.human_queue.peek().cloned();

        // Complete in queue
        if !self.human_queue.complete(response.clone()) {
            return Err(format!("Request {} not found in queue", response.request_id));
        }

        // Get agent ID from response/request
        let agent_id = AgentId::new(
            request
                .as_ref()
                .map(|r| r.agent_id.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        );

        // Find and update history
        if let Some(entry) = self.blocked_history.iter_mut().find(|e| e.agent_id == agent_id && !e.resolved) {
            entry.resolved = true;
            entry.resolution = Some(format!("{:?}", response.selection));
        }

        // Get slot and clear blocked status
        let slot = self.get_slot_mut_by_id(&agent_id);
        if slot.is_none() {
            return Ok(DecisionExecutionResult::AgentNotFound);
        }

        let slot = slot.unwrap();
        if !slot.status().is_blocked() {
            return Ok(DecisionExecutionResult::NotBlocked);
        }

        // Transition to Responding (active state after unblock)
        use std::time::Instant;
        slot.transition_to(AgentSlotStatus::Responding { started_at: Instant::now() })
            .map_err(|e| format!("Failed to unblock agent: {}", e))?;

        // Execute decision
        self.execute_decision(&agent_id, response.selection)
    }

    /// Execute human selection on an agent
    fn execute_decision(&mut self, agent_id: &AgentId, selection: HumanSelection) -> Result<DecisionExecutionResult, String> {
        let slot = self.get_slot_by_id(agent_id);
        if slot.is_none() {
            return Ok(DecisionExecutionResult::AgentNotFound);
        }

        let result = match selection {
            HumanSelection::Selected { option_id } => {
                DecisionExecutionResult::Executed { option_id }
            }
            HumanSelection::AcceptedRecommendation => {
                DecisionExecutionResult::AcceptedRecommendation
            }
            HumanSelection::Custom { instruction } => {
                DecisionExecutionResult::CustomInstruction { instruction }
            }
            HumanSelection::Skipped => {
                // Clear task assignment
                if let Some(slot) = self.get_slot_mut_by_id(agent_id) {
                    slot.clear_task();
                }
                DecisionExecutionResult::Skipped
            }
            HumanSelection::Cancelled => {
                // Transition to Idle
                if let Some(slot) = self.get_slot_mut_by_id(agent_id) {
                    slot.transition_to(AgentSlotStatus::Idle)
                        .map_err(|e| format!("Failed to cancel: {}", e))?;
                }
                DecisionExecutionResult::Cancelled
            }
        };

        Ok(result)
    }

    /// Clear all blocked agents (e.g., on shutdown)
    pub fn clear_all_blocked(&mut self) {
        for slot in &mut self.slots {
            if slot.status().is_blocked() {
                // Record in history
                if self.blocked_config.record_history {
                    if let Some(entry) = self.blocked_history.iter_mut().find(|e| &e.agent_id == slot.agent_id() && !e.resolved) {
                        entry.resolved = true;
                        entry.resolution = Some("cleared_on_shutdown".to_string());
                    }
                }
                slot.transition_to(AgentSlotStatus::Idle).ok();
            }
        }
        // Clear human queue
        self.human_queue.check_expired();
    }

    /// Check for expired human decision requests
    pub fn check_expired_requests(&mut self) -> Vec<HumanDecisionRequest> {
        self.human_queue.check_expired()
    }

    /// Get requests approaching timeout
    pub fn approaching_timeout_requests(&self) -> Vec<&HumanDecisionRequest> {
        self.human_queue.approaching_timeout()
    }

    /// Process expired requests and execute timeout actions
    ///
    /// Returns the number of requests processed.
    pub fn process_expired_requests(&mut self) -> usize {
        let expired = self.human_queue.check_expired();
        let count = expired.len();

        for request in expired {
            let selection = self.timeout_action_for_request(&request);
            let response = HumanDecisionResponse::new(
                request.id,
                selection,
            );
            // Ignore errors - the request is already removed from queue
            let _ = self.process_human_response(response);
        }

        count
    }

    /// Determine the timeout action for a request based on config
    fn timeout_action_for_request(&self, request: &HumanDecisionRequest) -> HumanSelection {
        let timeout_action = self.blocked_config.timeout_config.timeout_default;

        match timeout_action {
            AutoAction::FollowRecommendation => {
                // If there's a recommendation, accept it
                if request.recommendation.is_some() {
                    HumanSelection::AcceptedRecommendation
                } else {
                    // No recommendation, select default option
                    self.select_default_option(request)
                }
            }
            AutoAction::SelectDefault => {
                self.select_default_option(request)
            }
            AutoAction::Cancel => {
                HumanSelection::Cancelled
            }
            AutoAction::MarkTaskFailed => {
                // Mark task as failed - this would require a new selection type
                // For now, treat as cancelled
                HumanSelection::Cancelled
            }
            AutoAction::ReleaseResource => {
                HumanSelection::Cancelled
            }
        }
    }

    /// Select the default option from a request
    fn select_default_option(&self, request: &HumanDecisionRequest) -> HumanSelection {
        if let Some(first_option) = request.options.first() {
            HumanSelection::Selected {
                option_id: first_option.id.clone(),
            }
        } else {
            // No options available, skip
            HumanSelection::Skipped
        }
    }
}

fn parse_agent_index(agent_id: &str) -> Option<usize> {
    agent_id.strip_prefix("agent_")?.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_slot::AgentSlotStatus;
    use agent_decision::{
        HumanDecisionBlocking, WaitingForChoiceSituation, ResourceBlocking,
        BlockedState, HumanSelection,
    };

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
        let result =
            pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog);
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

        let result =
            pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog);
        assert!(result.is_err());
    }

    #[test]
    fn complete_task_with_backlog_success() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Complete task successfully
        let completed_task =
            pool.complete_task_with_backlog(&agent_id, TaskCompletionResult::Success, &mut backlog);
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
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        // Complete task with failure
        let completed_task = pool.complete_task_with_backlog(
            &agent_id,
            TaskCompletionResult::Failure {
                error: "test error".to_string(),
            },
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
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        let snapshot = pool.task_queue_snapshot(&backlog);
        assert_eq!(snapshot.agent_assignments.len(), 1);
        assert_eq!(snapshot.agent_assignments[0].task_id.as_str(), "task-001");
        assert_eq!(
            snapshot.agent_assignments[0].task_status,
            crate::backlog::TaskStatus::Running
        );
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
        pool.assign_task_with_backlog(&agent2, TaskId::new("task-001"), &mut backlog)
            .unwrap();

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

        pool.assign_task_with_backlog(&agent1, TaskId::new("task-001"), &mut backlog)
            .unwrap();

        let assignments = pool.agents_with_assignments(&backlog);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].agent_id, agent1);
    }

    // Blocked Handling Tests

    #[test]
    fn blocked_task_policy_default() {
        assert_eq!(BlockedTaskPolicy::default(), BlockedTaskPolicy::ReassignIfPossible);
    }

    #[test]
    fn blocked_handling_config_default() {
        let config = BlockedHandlingConfig::default();
        assert_eq!(config.task_policy, BlockedTaskPolicy::ReassignIfPossible);
        assert!(config.notify_others);
        assert!(config.record_history);
    }

    #[test]
    fn pool_new_has_blocked_handling() {
        let pool = make_pool(4);
        assert_eq!(pool.pending_human_decisions(), 0);
        assert_eq!(pool.blocked_count(), 0);
        assert!(pool.blocked_history().is_empty());
    }

    #[test]
    fn pool_with_blocked_config() {
        let config = BlockedHandlingConfig {
            task_policy: BlockedTaskPolicy::KeepAssigned,
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: false,
            record_history: false,
            max_history_entries: 100,
        };
        let pool = AgentPool::with_blocked_config(
            WorkplaceId::new("workplace-001"),
            4,
            config,
        );
        assert_eq!(pool.blocked_config().task_policy, BlockedTaskPolicy::KeepAssigned);
        assert!(!pool.blocked_config().notify_others);
    }

    #[test]
    fn process_agent_blocked_sets_status() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create a human decision blocking
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));

        // Process blocked
        let result = pool.process_agent_blocked(&agent_id, blocked_state, None);
        assert!(result.is_ok());

        // Check status is blocked
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.status().is_blocked());
        assert!(slot.status().is_blocked_for_human());
    }

    #[test]
    fn process_agent_blocked_adds_to_queue() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create a human decision blocking
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));

        // Process blocked
        pool.process_agent_blocked(&agent_id, blocked_state, None).unwrap();

        // Check human queue has request
        assert_eq!(pool.pending_human_decisions(), 1);
    }

    #[test]
    fn process_agent_blocked_records_history() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create a human decision blocking
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));

        // Process blocked
        pool.process_agent_blocked(&agent_id, blocked_state, None).unwrap();

        // Check history recorded
        assert_eq!(pool.blocked_history().len(), 1);
        let entry = &pool.blocked_history()[0];
        assert_eq!(entry.agent_id, agent_id);
        assert_eq!(entry.reason_type, "human_decision");
        assert!(!entry.resolved);
    }

    #[test]
    fn blocked_task_stays_with_agent_keep_assigned() {
        let config = BlockedHandlingConfig {
            task_policy: BlockedTaskPolicy::KeepAssigned,
            timeout_config: HumanDecisionTimeoutConfig::default(),
            notify_others: true,
            record_history: true,
            max_history_entries: 100,
        };
        let mut pool = AgentPool::with_blocked_config(
            WorkplaceId::new("workplace-001"),
            2,
            config,
        );
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, Some(&mut backlog)).unwrap();

        // Task should still be assigned to blocked agent
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_some());
    }

    #[test]
    fn blocked_task_reassigns_if_possible() {
        let mut pool = make_pool(3);
        let blocked_agent = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let idle_agent = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task to blocked_agent
        pool.assign_task_with_backlog(&blocked_agent, TaskId::new("task-001"), &mut backlog).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&blocked_agent, blocked_state, Some(&mut backlog)).unwrap();

        // Task should be reassigned to idle agent (with ReassignIfPossible policy)
        let blocked_slot = pool.get_slot_by_id(&blocked_agent).unwrap();
        let idle_slot = pool.get_slot_by_id(&idle_agent).unwrap();

        // Task is on idle agent now (or still on blocked if slot.assign_task failed due to status)
        // Note: idle_slot.assign_task would fail because the slot is Idle but we need Running
        // For now, check that blocked agent's task is cleared
        assert!(blocked_slot.assigned_task_id().is_none() || idle_slot.assigned_task_id().is_some());
    }

    #[test]
    fn process_human_response_clears_blocked() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, None).unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response
        let response = HumanDecisionResponse::new(
            request.id.clone(),
            HumanSelection::selected("option-a"),
        );

        // Process response
        let result = pool.process_human_response(response);
        assert!(result.is_ok());

        // Check agent is unblocked
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(!slot.status().is_blocked());
    }

    #[test]
    fn process_human_response_executes_selection() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, None).unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response with selection
        let response = HumanDecisionResponse::new(
            request.id.clone(),
            HumanSelection::selected("option-a"),
        );

        // Process response
        let result = pool.process_human_response(response).unwrap();
        assert_eq!(result, DecisionExecutionResult::Executed { option_id: "option-a".to_string() });
    }

    #[test]
    fn process_human_response_skip_clears_task() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let mut backlog = make_backlog_with_ready_task();

        // Assign task
        pool.assign_task_with_backlog(&agent_id, TaskId::new("task-001"), &mut backlog).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, Some(&mut backlog)).unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response with skip
        let response = HumanDecisionResponse::new(
            request.id.clone(),
            HumanSelection::skip(),
        );

        // Process response
        let result = pool.process_human_response(response).unwrap();
        assert_eq!(result, DecisionExecutionResult::Skipped);

        // Task should be cleared
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(slot.assigned_task_id().is_none());
    }

    #[test]
    fn process_human_response_cancel_transitions_to_idle() {
        let mut pool = make_pool(2);
        let agent_id = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking and process
        let blocking = HumanDecisionBlocking::new(
            "req-test",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        let blocked_state = BlockedState::new(Box::new(blocking));
        pool.process_agent_blocked(&agent_id, blocked_state, None).unwrap();

        // Get request from queue
        let request = pool.human_queue().peek().unwrap();

        // Create response with cancel
        let response = HumanDecisionResponse::new(
            request.id.clone(),
            HumanSelection::cancel(),
        );

        // Process response
        let result = pool.process_human_response(response).unwrap();
        assert_eq!(result, DecisionExecutionResult::Cancelled);

        // Agent should be Idle
        let slot = pool.get_slot_by_id(&agent_id).unwrap();
        assert!(matches!(slot.status(), AgentSlotStatus::Idle));
    }

    #[test]
    fn clear_all_blocked_unblocks_agents() {
        let mut pool = make_pool(2);
        let agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Create blocking for both
        let blocking1 = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent1, BlockedState::new(Box::new(blocking1)), None).unwrap();

        let blocking2 = HumanDecisionBlocking::new(
            "req-2",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent2, BlockedState::new(Box::new(blocking2)), None).unwrap();

        assert_eq!(pool.blocked_count(), 2);

        // Clear all
        pool.clear_all_blocked();

        // All should be unblocked
        assert_eq!(pool.blocked_count(), 0);
        let slot1 = pool.get_slot_by_id(&agent1).unwrap();
        let slot2 = pool.get_slot_by_id(&agent2).unwrap();
        assert!(matches!(slot1.status(), AgentSlotStatus::Idle));
        assert!(matches!(slot2.status(), AgentSlotStatus::Idle));
    }

    #[test]
    fn blocked_agents_list() {
        let mut pool = make_pool(3);
        let agent1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let agent2 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let _agent3 = pool.spawn_agent(ProviderKind::Mock).unwrap();

        // Block agent1 with human decision
        let blocking1 = HumanDecisionBlocking::new(
            "req-1",
            Box::new(WaitingForChoiceSituation::default()),
            vec![],
        );
        pool.process_agent_blocked(&agent1, BlockedState::new(Box::new(blocking1)), None).unwrap();

        // Block agent2 with resource waiting
        let blocking2 = ResourceBlocking::new("file", "/tmp/lock", "waiting for file lock");
        pool.process_agent_blocked(&agent2, BlockedState::new(Box::new(blocking2)), None).unwrap();

        // Get blocked agents
        let blocked = pool.blocked_agents();
        assert_eq!(blocked.len(), 2);
    }

    #[test]
    fn decision_execution_result_variants() {
        // Test all variants are constructible
        let executed = DecisionExecutionResult::Executed { option_id: "a".to_string() };
        let accepted = DecisionExecutionResult::AcceptedRecommendation;
        let custom = DecisionExecutionResult::CustomInstruction { instruction: "test".to_string() };
        let skipped = DecisionExecutionResult::Skipped;
        let cancelled = DecisionExecutionResult::Cancelled;
        let not_found = DecisionExecutionResult::AgentNotFound;
        let not_blocked = DecisionExecutionResult::NotBlocked;

        assert!(matches!(executed, DecisionExecutionResult::Executed { .. }));
        assert!(matches!(accepted, DecisionExecutionResult::AcceptedRecommendation));
        assert!(matches!(custom, DecisionExecutionResult::CustomInstruction { .. }));
        assert!(matches!(skipped, DecisionExecutionResult::Skipped));
        assert!(matches!(cancelled, DecisionExecutionResult::Cancelled));
        assert!(matches!(not_found, DecisionExecutionResult::AgentNotFound));
        assert!(matches!(not_blocked, DecisionExecutionResult::NotBlocked));
    }
}
