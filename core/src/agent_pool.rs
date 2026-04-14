//! AgentPool for managing multiple agent slots
//!
//! Central coordination structure for multi-agent runtime.

use std::collections::HashMap;
use std::sync::Arc;

use crate::agent_runtime::{AgentId, AgentCodename, ProviderType, WorkplaceId};
use crate::agent_slot::{AgentSlot, AgentSlotStatus, TaskId};
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

    /// Find an idle agent that can accept a task
    pub fn find_idle_agent(&self) -> Option<&AgentSlot> {
        self.slots.iter().find(|s| *s.status() == AgentSlotStatus::Idle)
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
        let id1 = pool.spawn_agent(ProviderKind::Mock).unwrap();
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
        let id0 = pool.spawn_agent(ProviderKind::Mock).unwrap();
        let id1 = pool.spawn_agent(ProviderKind::Claude).unwrap();
        pool.focus_agent_by_index(1).unwrap();
        pool.stop_agent(&id1).unwrap();
        pool.remove_agent(&id1).unwrap();
        // Focus should adjust to valid index
        assert_eq!(pool.focused_slot_index(), 0);
    }
}