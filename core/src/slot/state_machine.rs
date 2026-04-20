//! State machine abstraction for agent slot status transitions
//!
//! Provides AgentStateMachine trait that defines state transition rules
//! and DefaultStateMachine implementation that uses AgentSlotStatus::can_transition_to.
//! This module makes the state machine concept explicit and provides
//! utility methods like allowed_transitions() for UI and debugging.

use crate::agent_slot::{AgentSlot, AgentSlotStatus};

/// Trait defining agent state machine behavior
///
/// This trait abstracts the state transition logic, making it explicit
/// and allowing for alternative implementations (e.g., for testing).
pub trait AgentStateMachine {
    /// Check if transition from one status to another is valid
    fn can_transition(&self, from: &AgentSlotStatus, to: &AgentSlotStatus) -> bool;

    /// Execute transition with side effects
    ///
    /// Returns error if transition is not valid.
    fn transition(&self, slot: &mut AgentSlot, to: AgentSlotStatus) -> Result<(), String>;

    /// Get all allowed transitions from a given status
    ///
    /// Useful for UI to show available actions.
    fn allowed_transitions(&self, from: &AgentSlotStatus) -> Vec<AgentSlotStatus>;

    /// Check if status is terminal (cannot transition except to restart)
    fn is_terminal(&self, status: &AgentSlotStatus) -> bool;

    /// Check if status can accept task assignment
    fn can_accept_task(&self, status: &AgentSlotStatus) -> bool;
}

/// Default state machine implementation
///
/// Uses AgentSlotStatus::can_transition_to for transition validation.
pub struct DefaultStateMachine;

impl DefaultStateMachine {
    /// Create a new default state machine
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentStateMachine for DefaultStateMachine {
    fn can_transition(&self, from: &AgentSlotStatus, to: &AgentSlotStatus) -> bool {
        from.can_transition_to(to)
    }

    fn transition(&self, slot: &mut AgentSlot, to: AgentSlotStatus) -> Result<(), String> {
        slot.transition_to(to)
    }

    fn allowed_transitions(&self, from: &AgentSlotStatus) -> Vec<AgentSlotStatus> {
        // Generate all possible target statuses and filter valid ones
        let all_targets = Self::all_statuses();
        all_targets.into_iter().filter(|to| self.can_transition(from, to)).collect()
    }

    fn is_terminal(&self, status: &AgentSlotStatus) -> bool {
        status.is_terminal()
    }

    fn can_accept_task(&self, status: &AgentSlotStatus) -> bool {
        status.is_idle()
    }
}

impl DefaultStateMachine {
    /// Generate all possible status variants for transition checking
    fn all_statuses() -> Vec<AgentSlotStatus> {
        use std::time::Instant;
        use chrono::Utc;
        use agent_decision::BlockedState;
        use agent_decision::blocking::RateLimitBlockedReason;

        // Helper to create test BlockedState
        fn test_blocked_state() -> BlockedState {
            // Use RateLimitBlockedReason which is simpler to construct
            let rate_limit = RateLimitBlockedReason::new(Utc::now());
            BlockedState::new(Box::new(rate_limit))
        }

        vec![
            AgentSlotStatus::Idle,
            AgentSlotStatus::Starting,
            AgentSlotStatus::Responding { started_at: Instant::now() },
            AgentSlotStatus::ToolExecuting { tool_name: "test".to_string() },
            AgentSlotStatus::Finishing,
            AgentSlotStatus::Stopping,
            AgentSlotStatus::Stopped { reason: "test".to_string() },
            AgentSlotStatus::Error { message: "test".to_string() },
            AgentSlotStatus::Blocked { reason: "test".to_string() },
            AgentSlotStatus::BlockedForDecision { blocked_state: test_blocked_state() },
            AgentSlotStatus::Paused { reason: "test".to_string() },
            AgentSlotStatus::WaitingForInput { started_at: Instant::now() },
            AgentSlotStatus::Resting { started_at: Utc::now(), blocked_state: test_blocked_state(), on_resume: false },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_slot::AgentSlot;
    use crate::agent_runtime::{AgentId, AgentCodename, ProviderType};
    use crate::ProviderKind;
    use std::time::Instant;

    fn make_slot(status: AgentSlotStatus) -> AgentSlot {
        let id = AgentId::new("test-agent");
        let codename = AgentCodename::new("TEST");
        let provider_type = ProviderType::from_provider_kind(ProviderKind::Mock);
        let mut slot = AgentSlot::new(id, codename, provider_type);
        if status != AgentSlotStatus::Idle {
            let _ = slot.transition_to(status);
        }
        slot
    }

    #[test]
    fn default_state_machine_new() {
        let sm = DefaultStateMachine::new();
        assert!(sm.can_transition(&AgentSlotStatus::Idle, &AgentSlotStatus::Starting));
    }

    #[test]
    fn can_transition_idle_to_starting() {
        let sm = DefaultStateMachine::new();
        assert!(sm.can_transition(&AgentSlotStatus::Idle, &AgentSlotStatus::Starting));
        assert!(!sm.can_transition(&AgentSlotStatus::Idle, &AgentSlotStatus::Responding { started_at: Instant::now() }));
    }

    #[test]
    fn transition_executes_on_slot() {
        let sm = DefaultStateMachine::new();
        let mut slot = make_slot(AgentSlotStatus::Idle);
        let result = sm.transition(&mut slot, AgentSlotStatus::Starting);
        assert!(result.is_ok());
        assert_eq!(*slot.status(), AgentSlotStatus::Starting);
    }

    #[test]
    fn transition_fails_for_invalid() {
        let sm = DefaultStateMachine::new();
        let mut slot = make_slot(AgentSlotStatus::Idle);
        let result = sm.transition(&mut slot, AgentSlotStatus::Responding { started_at: Instant::now() });
        assert!(result.is_err());
    }

    #[test]
    fn allowed_transitions_from_idle() {
        let sm = DefaultStateMachine::new();
        let transitions = sm.allowed_transitions(&AgentSlotStatus::Idle);
        // Idle can go to: Starting, Blocked, BlockedForDecision, Stopped, Paused
        // Plus Idle itself (same status is always valid)
        assert!(transitions.len() >= 6);

        // Check some specific transitions
        assert!(transitions.contains(&AgentSlotStatus::Starting));
        assert!(transitions.iter().any(|s| matches!(s, AgentSlotStatus::Blocked { .. })));
        assert!(transitions.contains(&AgentSlotStatus::Idle));

        // Check invalid transitions are not included
        assert!(!transitions.iter().any(|s| matches!(s, AgentSlotStatus::Responding { .. })));
    }

    #[test]
    fn allowed_transitions_from_stopped() {
        let sm = DefaultStateMachine::new();
        let transitions = sm.allowed_transitions(&AgentSlotStatus::stopped("test"));
        // Stopped can only go to Starting (restart) or stay Stopped
        assert!(transitions.len() >= 2);
        assert!(transitions.contains(&AgentSlotStatus::Starting));
    }

    #[test]
    fn is_terminal_check() {
        let sm = DefaultStateMachine::new();
        assert!(sm.is_terminal(&AgentSlotStatus::stopped("test")));
        assert!(!sm.is_terminal(&AgentSlotStatus::Idle));
        assert!(!sm.is_terminal(&AgentSlotStatus::Starting));
    }

    #[test]
    fn can_accept_task_check() {
        let sm = DefaultStateMachine::new();
        assert!(sm.can_accept_task(&AgentSlotStatus::Idle));
        assert!(!sm.can_accept_task(&AgentSlotStatus::Starting));
        assert!(!sm.can_accept_task(&AgentSlotStatus::blocked("test")));
    }
}