//! Runtime Mode for backward compatibility
//!
//! Provides RuntimeMode enum to support both single-agent and multi-agent modes.

use serde::{Deserialize, Serialize};

/// Runtime mode for agent execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    /// Single agent mode (backward compatible)
    #[default]
    SingleAgent,
    /// Multi-agent mode with concurrent execution
    MultiAgent,
}

impl RuntimeMode {
    /// Check if in single-agent mode
    pub fn is_single_agent(&self) -> bool {
        matches!(self, Self::SingleAgent)
    }

    /// Check if in multi-agent mode
    pub fn is_multi_agent(&self) -> bool {
        matches!(self, Self::MultiAgent)
    }

    /// Get display label for this mode
    pub fn label(&self) -> &'static str {
        match self {
            Self::SingleAgent => "single-agent",
            Self::MultiAgent => "multi-agent",
        }
    }

    /// Get full name for this mode
    pub fn name(&self) -> &'static str {
        match self {
            Self::SingleAgent => "Single Agent",
            Self::MultiAgent => "Multi-Agent",
        }
    }

    /// Switch to multi-agent mode (when spawning second agent)
    pub fn switch_to_multi_agent(&mut self) {
        *self = Self::MultiAgent;
    }

    /// Check if mode allows spawning more agents
    pub fn can_spawn_more(&self, active_agents: usize) -> bool {
        match self {
            Self::SingleAgent => active_agents < 1,
            Self::MultiAgent => true, // Multi-agent mode allows spawning more
        }
    }

    /// Get maximum allowed agents for this mode
    pub fn max_agents(&self) -> usize {
        match self {
            Self::SingleAgent => 1,
            Self::MultiAgent => 10, // Default max for multi-agent
        }
    }

    /// Should use shared workplace state
    pub fn use_shared_state(&self) -> bool {
        matches!(self, Self::MultiAgent)
    }
}

/// Mode transition result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeTransition {
    /// No transition needed
    None,
    /// Transitioned from single to multi
    SingleToMulti,
    /// Invalid transition
    Invalid { reason: String },
}

impl ModeTransition {
    /// Check if transition happened
    pub fn happened(&self) -> bool {
        matches!(self, Self::SingleToMulti)
    }

    /// Check if transition is invalid
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid { .. })
    }
}

/// RuntimeMode helper functions
pub struct ModeHelper;

impl ModeHelper {
    /// Attempt to transition mode when spawning a new agent
    pub fn transition_for_spawn(
        current_mode: &mut RuntimeMode,
        current_agents: usize,
    ) -> ModeTransition {
        match current_mode {
            RuntimeMode::SingleAgent => {
                if current_agents == 0 {
                    // First agent, stay in single-agent mode
                    ModeTransition::None
                } else if current_agents == 1 {
                    // Second agent, switch to multi-agent
                    current_mode.switch_to_multi_agent();
                    ModeTransition::SingleToMulti
                } else {
                    // Can't have more than 1 in single-agent mode
                    ModeTransition::Invalid {
                        reason: "Single-agent mode allows only 1 agent".to_string(),
                    }
                }
            }
            RuntimeMode::MultiAgent => {
                // Already in multi-agent, no transition
                ModeTransition::None
            }
        }
    }

    /// Check if mode transition is needed
    pub fn needs_transition(mode: RuntimeMode, requested_agents: usize) -> bool {
        mode.is_single_agent() && requested_agents > 1
    }

    /// Validate spawn request against current mode
    pub fn validate_spawn(mode: RuntimeMode, current_agents: usize) -> Result<(), String> {
        if current_agents >= mode.max_agents() {
            return Err(format!(
                "Cannot spawn more agents: {} already active, max {} in {:?} mode",
                current_agents,
                mode.max_agents(),
                mode
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_default() {
        let mode = RuntimeMode::default();
        assert_eq!(mode, RuntimeMode::SingleAgent);
    }

    #[test]
    fn mode_is_checks() {
        let single = RuntimeMode::SingleAgent;
        let multi = RuntimeMode::MultiAgent;

        assert!(single.is_single_agent());
        assert!(!single.is_multi_agent());
        assert!(!multi.is_single_agent());
        assert!(multi.is_multi_agent());
    }

    #[test]
    fn mode_labels() {
        assert_eq!(RuntimeMode::SingleAgent.label(), "single-agent");
        assert_eq!(RuntimeMode::MultiAgent.label(), "multi-agent");
        assert_eq!(RuntimeMode::SingleAgent.name(), "Single Agent");
        assert_eq!(RuntimeMode::MultiAgent.name(), "Multi-Agent");
    }

    #[test]
    fn mode_switch() {
        let mut mode = RuntimeMode::SingleAgent;
        mode.switch_to_multi_agent();
        assert_eq!(mode, RuntimeMode::MultiAgent);
    }

    #[test]
    fn mode_can_spawn_more() {
        let single = RuntimeMode::SingleAgent;
        let multi = RuntimeMode::MultiAgent;

        // Single-agent: can spawn 0, not 1
        assert!(single.can_spawn_more(0));
        assert!(!single.can_spawn_more(1));

        // Multi-agent: always can spawn more (up to max)
        assert!(multi.can_spawn_more(0));
        assert!(multi.can_spawn_more(1));
        assert!(multi.can_spawn_more(5));
    }

    #[test]
    fn mode_max_agents() {
        assert_eq!(RuntimeMode::SingleAgent.max_agents(), 1);
        assert_eq!(RuntimeMode::MultiAgent.max_agents(), 10);
    }

    #[test]
    fn mode_use_shared_state() {
        assert!(!RuntimeMode::SingleAgent.use_shared_state());
        assert!(RuntimeMode::MultiAgent.use_shared_state());
    }

    #[test]
    fn mode_serialization() {
        let mode = RuntimeMode::MultiAgent;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"multi_agent\"");
        let parsed: RuntimeMode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, RuntimeMode::MultiAgent);
    }

    #[test]
    fn mode_transition_none() {
        let mut mode = RuntimeMode::SingleAgent;
        let transition = ModeHelper::transition_for_spawn(&mut mode, 0);
        assert_eq!(transition, ModeTransition::None);
        assert_eq!(mode, RuntimeMode::SingleAgent);
    }

    #[test]
    fn mode_transition_single_to_multi() {
        let mut mode = RuntimeMode::SingleAgent;
        let transition = ModeHelper::transition_for_spawn(&mut mode, 1);
        assert_eq!(transition, ModeTransition::SingleToMulti);
        assert_eq!(mode, RuntimeMode::MultiAgent);
    }

    #[test]
    fn mode_transition_invalid() {
        let mut mode = RuntimeMode::SingleAgent;
        let transition = ModeHelper::transition_for_spawn(&mut mode, 2);
        assert!(transition.is_invalid());
        assert_eq!(mode, RuntimeMode::SingleAgent);
    }

    #[test]
    fn mode_transition_multi_agent() {
        let mut mode = RuntimeMode::MultiAgent;
        let transition = ModeHelper::transition_for_spawn(&mut mode, 5);
        assert_eq!(transition, ModeTransition::None);
        assert_eq!(mode, RuntimeMode::MultiAgent);
    }

    #[test]
    fn mode_helper_needs_transition() {
        assert!(ModeHelper::needs_transition(RuntimeMode::SingleAgent, 2));
        assert!(!ModeHelper::needs_transition(RuntimeMode::SingleAgent, 1));
        assert!(!ModeHelper::needs_transition(RuntimeMode::MultiAgent, 10));
    }

    #[test]
    fn mode_helper_validate_spawn() {
        // Single-agent mode
        assert!(ModeHelper::validate_spawn(RuntimeMode::SingleAgent, 0).is_ok());
        assert!(ModeHelper::validate_spawn(RuntimeMode::SingleAgent, 1).is_err());

        // Multi-agent mode
        assert!(ModeHelper::validate_spawn(RuntimeMode::MultiAgent, 0).is_ok());
        assert!(ModeHelper::validate_spawn(RuntimeMode::MultiAgent, 9).is_ok());
        assert!(ModeHelper::validate_spawn(RuntimeMode::MultiAgent, 10).is_err());
    }

    #[test]
    fn mode_transition_happened() {
        let none = ModeTransition::None;
        let transitioned = ModeTransition::SingleToMulti;
        let invalid = ModeTransition::Invalid {
            reason: "test".to_string(),
        };

        assert!(!none.happened());
        assert!(transitioned.happened());
        assert!(!invalid.happened());
    }
}
