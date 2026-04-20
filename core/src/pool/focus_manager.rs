//! Focus manager for managing focused agent in pool
//!
//! Provides FocusManager that manages the focused_slot index and
//! provides methods for focusing by index, focusing by agent ID,
//! and adjusting focus on agent removal.

use crate::agent_runtime::AgentId;
use crate::agent_slot::AgentSlot;
use crate::logging;

/// Error type for focus operations
#[derive(Debug)]
pub enum FocusError {
    /// Invalid focus index
    InvalidIndex { attempted: usize, max: usize },
    /// Agent not found
    AgentNotFound(String),
}

impl std::fmt::Display for FocusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FocusError::InvalidIndex { attempted, max } => {
                write!(f, "Invalid focus index {} (only {} agents)", attempted, max)
            }
            FocusError::AgentNotFound(id) => write!(f, "Agent {} not found", id),
        }
    }
}

impl std::error::Error for FocusError {}

/// Focus manager - manages focused slot index
///
/// This struct tracks which agent slot is currently focused
/// and provides methods to change focus.
pub struct FocusManager {
    /// Currently focused slot index
    focused_slot: usize,
}

impl FocusManager {
    /// Create a new focus manager with focus at index 0
    pub fn new() -> Self {
        Self { focused_slot: 0 }
    }

    /// Create a focus manager with specific initial focus
    pub fn with_index(index: usize) -> Self {
        Self { focused_slot: index }
    }

    /// Get the current focused slot index
    pub fn focused_index(&self) -> usize {
        self.focused_slot
    }

    /// Focus by index
    ///
    /// Returns error if index is out of bounds.
    pub fn focus_by_index(&mut self, slots: &[AgentSlot], index: usize) -> Result<(), FocusError> {
        if index >= slots.len() {
            logging::debug_event(
                "pool.focus.invalid_index",
                "invalid focus index",
                serde_json::json!({
                    "attempted_index": index,
                    "pool_size": slots.len(),
                }),
            );
            return Err(FocusError::InvalidIndex {
                attempted: index,
                max: slots.len(),
            });
        }

        let old_index = self.focused_slot;
        let old_agent_id = slots
            .get(old_index)
            .map(|s| s.agent_id().as_str().to_string());
        let new_agent_id = slots
            .get(index)
            .map(|s| s.agent_id().as_str().to_string());

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

    /// Focus by agent ID
    ///
    /// Finds the slot index for the agent ID and focuses it.
    pub fn focus_agent(&mut self, slots: &[AgentSlot], agent_id: &AgentId) -> Result<(), FocusError> {
        let index = slots.iter()
            .position(|s| s.agent_id() == agent_id)
            .ok_or_else(|| FocusError::AgentNotFound(agent_id.as_str().to_string()))?;

        let old_index = self.focused_slot;
        let old_agent_id = slots
            .get(old_index)
            .map(|s| s.agent_id().as_str().to_string());
        let new_codename = slots
            .get(index)
            .map(|s| s.codename().as_str().to_string());

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

        self.focus_by_index(slots, index)
    }

    /// Adjust focus after agent removal
    ///
    /// If the removed agent was at or before the focused slot,
    /// adjusts focus to valid slot.
    pub fn adjust_on_remove(&mut self, removed_index: usize, slots_len: usize) {
        // If focused slot was beyond the removed index, decrement
        if self.focused_slot > removed_index {
            self.focused_slot -= 1;
        }
        // If focused slot was the removed one, adjust to last valid slot
        else if self.focused_slot == removed_index && slots_len > 0 {
            self.focused_slot = slots_len - 1;
        }
        // If pool is empty, reset to 0
        else if slots_len == 0 {
            self.focused_slot = 0;
        }

        // Validate bounds
        if self.focused_slot >= slots_len && slots_len > 0 {
            self.focused_slot = slots_len - 1;
        }

        logging::debug_event(
            "pool.focus.adjust",
            "adjusted focus after removal",
            serde_json::json!({
                "removed_index": removed_index,
                "new_focused_slot": self.focused_slot,
                "slots_len": slots_len,
            }),
        );
    }

    /// Reset focus to first slot (index 0)
    pub fn reset_to_first(&mut self) {
        self.focused_slot = 0;
    }

    /// Focus on newly spawned agent if first one
    ///
    /// Called after spawning an agent. If this is the first agent,
    /// focus is set to index 0.
    pub fn focus_on_first_spawn(&mut self, slots_len: usize, agent_id: &AgentId) {
        if slots_len == 1 {
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
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_runtime::{AgentId, AgentCodename, ProviderType};
    use crate::agent_slot::AgentSlot;
    use crate::ProviderKind;

    fn make_slot(agent_id: &str) -> AgentSlot {
        let id = AgentId::new(agent_id);
        let codename = AgentCodename::new("TEST");
        let provider_type = ProviderType::from_provider_kind(ProviderKind::Mock);
        AgentSlot::new(id, codename, provider_type)
    }

    #[test]
    fn focus_manager_new() {
        let fm = FocusManager::new();
        assert_eq!(fm.focused_index(), 0);
    }

    #[test]
    fn focus_by_index_success() {
        let slots = vec![make_slot("agent-1"), make_slot("agent-2"), make_slot("agent-3")];
        let mut fm = FocusManager::new();

        let result = fm.focus_by_index(&slots, 2);
        assert!(result.is_ok());
        assert_eq!(fm.focused_index(), 2);
    }

    #[test]
    fn focus_by_index_invalid() {
        let slots = vec![make_slot("agent-1")];
        let mut fm = FocusManager::new();

        let result = fm.focus_by_index(&slots, 5);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FocusError::InvalidIndex { .. }));
    }

    #[test]
    fn focus_agent_success() {
        let slots = vec![make_slot("agent-1"), make_slot("agent-2")];
        let mut fm = FocusManager::new();

        let result = fm.focus_agent(&slots, &AgentId::new("agent-2"));
        assert!(result.is_ok());
        assert_eq!(fm.focused_index(), 1);
    }

    #[test]
    fn focus_agent_not_found() {
        let slots = vec![make_slot("agent-1")];
        let mut fm = FocusManager::new();

        let result = fm.focus_agent(&slots, &AgentId::new("agent-999"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), FocusError::AgentNotFound(_)));
    }

    #[test]
    fn adjust_on_remove_after_focused() {
        // Focused at index 2, remove agent at index 1
        // Focus should decrement to 1
        let mut fm = FocusManager::with_index(2);
        fm.adjust_on_remove(1, 2); // 2 slots remain after removing 1 from 3
        assert_eq!(fm.focused_index(), 1);
    }

    #[test]
    fn adjust_on_remove_focused_agent() {
        // Focused at index 1, remove agent at index 1
        // Focus should move to last valid slot (index 0)
        let mut fm = FocusManager::with_index(1);
        fm.adjust_on_remove(1, 1); // 1 slot remains
        assert_eq!(fm.focused_index(), 0);
    }

    #[test]
    fn adjust_on_remove_before_focused() {
        // Focused at index 2, remove agent at index 0
        // Focus should stay at 2 but since slot was removed, it's now index 1
        let mut fm = FocusManager::with_index(2);
        fm.adjust_on_remove(0, 2); // 2 slots remain after removing index 0
        assert_eq!(fm.focused_index(), 1); // Original index 2 becomes index 1
    }

    #[test]
    fn adjust_on_remove_empty_pool() {
        // Remove last agent, pool becomes empty
        let mut fm = FocusManager::with_index(0);
        fm.adjust_on_remove(0, 0);
        assert_eq!(fm.focused_index(), 0);
    }

    #[test]
    fn reset_to_first() {
        let mut fm = FocusManager::with_index(5);
        fm.reset_to_first();
        assert_eq!(fm.focused_index(), 0);
    }

    #[test]
    fn focus_on_first_spawn_first_agent() {
        let mut fm = FocusManager::new();
        fm.focus_on_first_spawn(1, &AgentId::new("agent-1"));
        assert_eq!(fm.focused_index(), 0);
    }

    #[test]
    fn focus_on_first_spawn_not_first_agent() {
        let mut fm = FocusManager::with_index(2);
        fm.focus_on_first_spawn(3, &AgentId::new("agent-3"));
        assert_eq!(fm.focused_index(), 2); // Should not change
    }
}