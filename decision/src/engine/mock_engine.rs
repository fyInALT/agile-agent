//! Mock decision engine for testing

use crate::model::action::DecisionAction;
use crate::model::action::action_registry::ActionRegistry;
use crate::model::action::builtin_actions::{
    CustomInstructionAction, ReflectAction, RetryAction, SelectOptionAction,
};
use crate::core::context::DecisionContext;
use crate::engine::engine::DecisionEngine;
use crate::core::output::{DecisionOutput, DecisionRecord};
use crate::model::situation::DecisionSituation;
use crate::core::types::DecisionEngineType;

/// Mock decision engine for testing
pub struct MockDecisionEngine {
    /// Decision history
    history: Vec<DecisionRecord>,
}

impl MockDecisionEngine {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    /// Get mock actions for situation
    fn get_mock_actions(&self, situation: &dyn DecisionSituation) -> Vec<Box<dyn DecisionAction>> {
        let type_name = situation.situation_type().name;
        match type_name.as_str() {
            "waiting_for_choice" => {
                // Return first option selection
                vec![Box::new(SelectOptionAction::new("A", "Mock: first option"))]
            }
            "claims_completion" => {
                // Return reflect action for completion
                vec![Box::new(ReflectAction::new("Mock: please reflect"))]
            }
            "error" => {
                // Return retry action for error
                vec![Box::new(
                    RetryAction::new("Mock: retry").with_cooldown(1000),
                )]
            }
            _ => {
                // Default: continue with custom instruction
                vec![Box::new(CustomInstructionAction::new("Mock: continue"))]
            }
        }
    }

    /// Get decision history
    pub fn history(&self) -> &[DecisionRecord] {
        &self.history
    }
}

impl Default for MockDecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionEngine for MockDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::Mock
    }

    fn decide(
        &mut self,
        context: DecisionContext,
        _action_registry: &ActionRegistry,
    ) -> crate::error::Result<DecisionOutput> {
        let situation = context.trigger_situation.as_ref();
        let actions = self.get_mock_actions(situation);

        let output = DecisionOutput::new(actions, "Mock decision").with_confidence(0.8);

        let record = DecisionRecord::new(
            crate::types::generate_id("dec"),
            situation.situation_type(),
            &output,
            DecisionEngineType::Mock,
        );

        self.history.push(record);

        Ok(output)
    }

    fn build_prompt(
        &self,
        _context: &DecisionContext,
        _action_registry: &ActionRegistry,
    ) -> String {
        "Mock prompt".to_string()
    }

    fn parse_response(
        &self,
        response: &str,
        _situation: &dyn DecisionSituation,
        _action_registry: &ActionRegistry,
    ) -> crate::error::Result<Vec<Box<dyn DecisionAction>>> {
        // Mock parsing - return continue action
        Ok(vec![Box::new(CustomInstructionAction::new(response))])
    }

    fn session_handle(&self) -> Option<&str> {
        None
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn reset(&mut self) -> crate::error::Result<()> {
        self.history.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::action::action_registry::ActionRegistry;
    use crate::model::situation::builtin_situations::WaitingForChoiceSituation;
    use crate::core::context::DecisionContext;
    use crate::model::situation::DecisionSituation;

    fn make_context(situation: Box<dyn DecisionSituation>) -> DecisionContext {
        DecisionContext::new(situation, "test-agent")
    }

    #[test]
    fn test_mock_engine_type() {
        let engine = MockDecisionEngine::new();
        assert!(engine.engine_type().is_mock());
    }

    #[test]
    fn test_mock_engine_waiting_for_choice() {
        let mut engine = MockDecisionEngine::new();
        let registry = ActionRegistry::new();
        crate::builtin_actions::register_action_builtins(&registry);

        let situation = Box::new(WaitingForChoiceSituation::default());
        let context = make_context(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        assert_eq!(output.first_action_type().unwrap().name, "select_option");
    }

    #[test]
    fn test_mock_engine_error() {
        let mut engine = MockDecisionEngine::new();
        let registry = ActionRegistry::new();
        crate::builtin_actions::register_action_builtins(&registry);

        let situation = Box::new(crate::builtin_situations::ErrorSituation::new(
            crate::model::situation::ErrorInfo::new("test_error", "test"),
        ));
        let context = make_context(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        assert_eq!(output.first_action_type().unwrap().name, "retry");
    }

    #[test]
    fn test_mock_engine_history() {
        let mut engine = MockDecisionEngine::new();
        let registry = ActionRegistry::new();

        let situation = Box::new(WaitingForChoiceSituation::default());
        let context = make_context(situation);
        engine.decide(context, &registry).unwrap();

        assert_eq!(engine.history().len(), 1);
    }

    #[test]
    fn test_mock_engine_reset() {
        let mut engine = MockDecisionEngine::new();
        let registry = ActionRegistry::new();

        let situation = Box::new(WaitingForChoiceSituation::default());
        let context = make_context(situation);
        engine.decide(context, &registry).unwrap();
        assert_eq!(engine.history().len(), 1);

        engine.reset().unwrap();
        assert_eq!(engine.history().len(), 0);
    }

    #[test]
    fn test_mock_engine_healthy() {
        let engine = MockDecisionEngine::new();
        assert!(engine.is_healthy());
    }

    #[test]
    fn test_mock_engine_session_handle() {
        let engine = MockDecisionEngine::new();
        assert!(engine.session_handle().is_none());
    }
}
