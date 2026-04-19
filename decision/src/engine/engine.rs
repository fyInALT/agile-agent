//! Decision engine trait and implementations

use crate::model::action::DecisionAction;
use crate::model::action::action_registry::ActionRegistry;
use crate::core::context::DecisionContext;
use crate::core::output::DecisionOutput;
use crate::model::situation::DecisionSituation;
use crate::core::types::DecisionEngineType;
use std::fmt;

/// Decision engine trait - makes decisions from context
pub trait DecisionEngine: Send + Sync {
    /// Get engine type
    fn engine_type(&self) -> DecisionEngineType;

    /// Make a decision based on context
    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<DecisionOutput>;

    /// Build decision prompt from context
    fn build_prompt(&self, context: &DecisionContext, action_registry: &ActionRegistry) -> String;

    /// Parse response to action sequence
    fn parse_response(
        &self,
        response: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<Vec<Box<dyn DecisionAction>>>;

    /// Get current session handle
    fn session_handle(&self) -> Option<&str>;

    /// Check engine health
    fn is_healthy(&self) -> bool;

    /// Reset engine state
    fn reset(&mut self) -> crate::error::Result<()>;
}

impl fmt::Debug for dyn DecisionEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecisionEngine")
            .field("type", &self.engine_type())
            .field("healthy", &self.is_healthy())
            .finish()
    }
}

/// Session handle for multi-turn decisions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    /// Session ID
    pub session_id: String,

    /// Provider kind
    pub provider: crate::provider::provider_kind::ProviderKind,

    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl SessionHandle {
    pub fn new(
        session_id: impl Into<String>,
        provider: crate::provider::provider_kind::ProviderKind,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            provider,
            created_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_handle_new() {
        let handle = SessionHandle::new("sess-1", crate::provider::provider_kind::ProviderKind::Claude);
        assert_eq!(handle.session_id, "sess-1");
        assert!(handle.provider.is_claude());
    }

    #[test]
    fn test_decision_engine_type_mock() {
        let type_ = DecisionEngineType::Mock;
        assert!(type_.is_mock());
        assert!(!type_.is_llm());
        assert!(!type_.is_cli());
    }

    #[test]
    fn test_decision_engine_type_llm() {
        let type_ = DecisionEngineType::LLM {
            provider: crate::provider::provider_kind::ProviderKind::Claude,
        };
        assert!(type_.is_llm());
        assert!(!type_.is_mock());
    }

    #[test]
    fn test_decision_engine_type_display() {
        let type_ = DecisionEngineType::Mock;
        assert_eq!(format!("{}", type_), "mock");
    }
}
