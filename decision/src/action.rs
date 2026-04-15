//! Decision action trait

use crate::types::ActionType;

/// Decision action trait - extensible with serialization
pub trait DecisionAction: Send + Sync + 'static {
    /// Action type identifier
    fn action_type(&self) -> ActionType;

    /// Concrete implementation type name (for debugging)
    fn implementation_type(&self) -> &'static str;

    /// Serialize for prompt (tells LLM how to output)
    fn to_prompt_format(&self) -> String;

    /// Serialize parameters to JSON (for persistence)
    fn serialize_params(&self) -> String;

    /// Clone into boxed
    fn clone_boxed(&self) -> Box<dyn DecisionAction>;
}

/// Action execution result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionResult {
    /// Action completed successfully
    Success,

    /// Action needs follow-up
    NeedsFollowUp {
        next_action: Option<ActionType>,
    },

    /// Action delegated to other agent
    Delegated {
        target_agent_id: String,
    },

    /// Action failed
    Failed {
        reason: String,
    },

    /// Action requires human confirmation
    NeedsHumanConfirmation {
        message: String,
    },
}

impl ActionResult {
    pub fn success() -> Self {
        ActionResult::Success
    }

    pub fn needs_followup(next_action: Option<ActionType>) -> Self {
        ActionResult::NeedsFollowUp { next_action }
    }

    pub fn delegated(target_agent_id: impl Into<String>) -> Self {
        ActionResult::Delegated {
            target_agent_id: target_agent_id.into(),
        }
    }

    pub fn failed(reason: impl Into<String>) -> Self {
        ActionResult::Failed {
            reason: reason.into(),
        }
    }

    pub fn needs_human_confirmation(message: impl Into<String>) -> Self {
        ActionResult::NeedsHumanConfirmation {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_result_success() {
        let result = ActionResult::success();
        assert_eq!(result, ActionResult::Success);
    }

    #[test]
    fn test_action_result_needs_followup() {
        let result = ActionResult::needs_followup(Some(ActionType::new("confirm")));
        assert_eq!(
            result,
            ActionResult::NeedsFollowUp {
                next_action: Some(ActionType::new("confirm"))
            }
        );
    }

    #[test]
    fn test_action_result_delegated() {
        let result = ActionResult::delegated("agent-123");
        assert_eq!(
            result,
            ActionResult::Delegated {
                target_agent_id: "agent-123".to_string()
            }
        );
    }

    #[test]
    fn test_action_result_failed() {
        let result = ActionResult::failed("Connection timeout");
        assert_eq!(
            result,
            ActionResult::Failed {
                reason: "Connection timeout".to_string()
            }
        );
    }

    #[test]
    fn test_action_result_needs_human() {
        let result = ActionResult::needs_human_confirmation("Please approve");
        assert_eq!(
            result,
            ActionResult::NeedsHumanConfirmation {
                message: "Please approve".to_string()
            }
        );
    }
}