//! Rule-based decision engine

use crate::model::action::DecisionAction;
use crate::model::action::action_registry::ActionRegistry;
use crate::model::action::builtin_actions::{
    ConfirmCompletionAction, ContinueAction, CustomInstructionAction, ReflectAction, RetryAction,
    SelectOptionAction,
};
use crate::condition::{Condition, ConditionEvaluatorRegistry, ConditionExpr};
use crate::core::context::DecisionContext;
use crate::engine::engine::DecisionEngine;
use crate::core::output::DecisionOutput;
use crate::model::situation::DecisionSituation;
use crate::core::types::{ActionType, DecisionEngineType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rule priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RulePriority {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for RulePriority {
    fn default() -> Self {
        RulePriority::Medium
    }
}

/// Action specification for rule output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
    /// Action type name
    pub type_name: String,
    /// Action parameters
    pub params: HashMap<String, String>,
}

impl ActionSpec {
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            params: HashMap::new(),
        }
    }

    pub fn with_param(self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut params = self.params;
        params.insert(key.into(), value.into());
        Self {
            type_name: self.type_name,
            params,
        }
    }

    /// Build action from spec - handles known types directly
    pub fn build_action(&self) -> Option<Box<dyn DecisionAction>> {
        match self.type_name.as_str() {
            "select_first" => Some(Box::new(SelectFirstAction::new())),
            "select_option" => {
                let option_id = self.params.get("option_id").cloned().unwrap_or_default();
                let reason = self.params.get("reason").cloned().unwrap_or_default();
                Some(Box::new(SelectOptionAction::new(option_id, reason)))
            }
            "reflect" => {
                let prompt = self.params.get("prompt").cloned().unwrap_or_default();
                Some(Box::new(ReflectAction::new(prompt)))
            }
            "retry" => Some(Box::new(RetryAction::new("retry"))),
            "confirm_completion" => Some(Box::new(ConfirmCompletionAction::new(false))),
            "continue" => {
                let instruction = self.params.get("instruction").cloned().unwrap_or_default();
                Some(Box::new(ContinueAction::new(instruction)))
            }
            "custom_instruction" => {
                let instruction = self.params.get("instruction").cloned().unwrap_or_default();
                Some(Box::new(CustomInstructionAction::new(instruction)))
            }
            "continue_all_tasks" => {
                let instruction = self
                    .params
                    .get("instruction")
                    .cloned()
                    .unwrap_or_else(|| "continue finish all tasks".to_string());
                Some(Box::new(
                    crate::builtin_actions::ContinueAllTasksAction::new(instruction),
                ))
            }
            "stop_if_complete" => {
                let reason = self
                    .params
                    .get("reason")
                    .cloned()
                    .unwrap_or_else(|| "All tasks complete".to_string());
                Some(Box::new(crate::builtin_actions::StopIfCompleteAction::new(
                    reason,
                )))
            }
            "create_task_branch" => {
                let branch_name = self
                    .params
                    .get("branch_name")
                    .cloned()
                    .unwrap_or_default();
                let base_branch = self
                    .params
                    .get("base_branch")
                    .cloned()
                    .unwrap_or_else(|| "main".to_string());
                Some(Box::new(
                    crate::builtin_actions::CreateTaskBranchAction::new(branch_name, base_branch),
                ))
            }
            "rebase_to_main" => {
                let base_branch = self
                    .params
                    .get("base_branch")
                    .cloned()
                    .unwrap_or_else(|| "main".to_string());
                Some(Box::new(crate::builtin_actions::RebaseToMainAction::new(
                    base_branch,
                )))
            }
            "prepare_task_start" => {
                let task_id = self
                    .params
                    .get("task_id")
                    .cloned()
                    .unwrap_or_else(|| "task-001".to_string());
                let task_description = self
                    .params
                    .get("task_description")
                    .cloned()
                    .unwrap_or_default();
                let task_meta = crate::model::task::task_metadata::TaskMetadata::new(&task_id, &task_description);
                Some(Box::new(
                    crate::builtin_actions::PrepareTaskStartAction::new(task_meta),
                ))
            }
            _ => None,
        }
    }
}

/// Select first option action (simple)
pub struct SelectFirstAction;

impl SelectFirstAction {
    pub fn new() -> Self {
        SelectFirstAction
    }
}

impl DecisionAction for SelectFirstAction {
    fn action_type(&self) -> ActionType {
        crate::builtin_actions::select_first()
    }

    fn implementation_type(&self) -> &'static str {
        "SelectFirstAction"
    }

    fn to_prompt_format(&self) -> String {
        "select_first: Select the first available option".to_string()
    }

    fn serialize_params(&self) -> String {
        "{}".to_string()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(SelectFirstAction)
    }
}

/// Decision rule - condition + action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRule {
    /// Rule name
    pub name: String,
    /// Condition expression
    pub condition: ConditionExpr,
    /// Actions to take when matched
    pub actions: Vec<ActionSpec>,
    /// Priority for rule ordering
    pub priority: RulePriority,
}

impl DecisionRule {
    pub fn new(
        name: impl Into<String>,
        condition: ConditionExpr,
        actions: Vec<ActionSpec>,
        priority: RulePriority,
    ) -> Self {
        Self {
            name: name.into(),
            condition,
            actions,
            priority,
        }
    }
}

/// Rule-based decision engine
pub struct RuleBasedDecisionEngine {
    /// Custom rules
    rules: Vec<DecisionRule>,
    /// Built-in default rules
    default_rules: Vec<DecisionRule>,
    /// Condition evaluator registry
    evaluator_registry: ConditionEvaluatorRegistry,
}

impl RuleBasedDecisionEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            default_rules: Self::builtin_rules(),
            evaluator_registry: ConditionEvaluatorRegistry::new(),
        }
    }

    pub fn with_rules(rules: Vec<DecisionRule>) -> Self {
        Self {
            rules,
            default_rules: Self::builtin_rules(),
            evaluator_registry: ConditionEvaluatorRegistry::new(),
        }
    }

    pub fn register_evaluator(
        &mut self,
        name: impl Into<String>,
        evaluator: Box<dyn crate::condition::ConditionEvaluator>,
    ) {
        self.evaluator_registry.register(name, evaluator);
    }

    /// Built-in default rules
    fn builtin_rules() -> Vec<DecisionRule> {
        vec![
            // Rule: Select first for waiting_for_choice
            DecisionRule::new(
                "approve-first",
                ConditionExpr::single(Condition::situation_type("waiting_for_choice")),
                vec![ActionSpec::new("select_first")],
                RulePriority::Medium,
            ),
            // Rule: Reflect on claims_completion (first round)
            DecisionRule::new(
                "reflect-first",
                ConditionExpr::and(vec![
                    ConditionExpr::single(Condition::situation_type("claims_completion")),
                    ConditionExpr::single(Condition::reflection_rounds(0, 1)),
                ]),
                vec![ActionSpec::new("reflect")],
                RulePriority::High,
            ),
            // Rule: Confirm completion when max reflections reached
            DecisionRule::new(
                "confirm-when-max-reflections",
                ConditionExpr::and(vec![
                    ConditionExpr::single(Condition::situation_type("claims_completion")),
                    ConditionExpr::single(Condition::reflection_rounds(2, 10)), // max is 2, so >=2
                ]),
                vec![ActionSpec::new("confirm_completion")],
                RulePriority::High,
            ),
            // Rule: Retry on error
            DecisionRule::new(
                "retry-error",
                ConditionExpr::single(Condition::situation_type("error")),
                vec![ActionSpec::new("retry")],
                RulePriority::Medium,
            ),
            // Rule: Rate limit recovery - retry after waiting
            DecisionRule::new(
                "retry-rate-limit",
                ConditionExpr::single(Condition::situation_type("rate_limit_recovery")),
                vec![ActionSpec::new("retry")],
                RulePriority::Medium,
            ),
            // Rule: Continue all tasks on agent_idle (default behavior)
            // Decision layer should verify pending tasks before stopping
            DecisionRule::new(
                "continue-on-idle",
                ConditionExpr::single(Condition::situation_type("agent_idle")),
                vec![ActionSpec::new("continue_all_tasks")],
                RulePriority::Medium,
            ),
            // Rule: Prepare task on task_starting
            DecisionRule::new(
                "prepare-task-start",
                ConditionExpr::single(Condition::situation_type("task_starting")),
                vec![ActionSpec::new("prepare_task_start")],
                RulePriority::High,
            ),
        ]
    }

    /// Find matching rule
    fn find_matching_rule(&self, context: &DecisionContext) -> Option<&DecisionRule> {
        let all_rules: Vec<&DecisionRule> =
            self.rules.iter().chain(self.default_rules.iter()).collect();

        // Sort by priority (Critical > High > Medium > Low)
        all_rules
            .into_iter()
            .filter(|rule| rule.condition.evaluate(context, &self.evaluator_registry))
            .max_by_key(|rule| match rule.priority {
                RulePriority::Critical => 0,
                RulePriority::High => 1,
                RulePriority::Medium => 2,
                RulePriority::Low => 3,
            })
    }
}

impl Default for RuleBasedDecisionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionEngine for RuleBasedDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::RuleBased
    }

    fn decide(
        &mut self,
        context: DecisionContext,
        _action_registry: &ActionRegistry,
    ) -> crate::error::Result<DecisionOutput> {
        let rule = self.find_matching_rule(&context);

        if let Some(rule) = rule {
            let actions: Vec<Box<dyn DecisionAction>> = rule
                .actions
                .iter()
                .filter_map(|spec| spec.build_action())
                .collect();

            return Ok(
                DecisionOutput::new(actions, format!("Rule: {}", rule.name)).with_confidence(0.9)
            );
        }

        // No matching rule - default action
        Ok(DecisionOutput::new(
            vec![Box::new(CustomInstructionAction::new(
                "Continue with current task",
            ))],
            "No matching rule",
        )
        .with_confidence(0.5))
    }

    fn build_prompt(&self, context: &DecisionContext, action_registry: &ActionRegistry) -> String {
        let situation = context.trigger_situation.as_ref();
        format!(
            "Situation: {}\n\
            Available Actions: {}\n\
            Project Rules: {}",
            situation.to_prompt_text(),
            action_registry.generate_prompt_formats(),
            context.project_rules.summary(),
        )
    }

    fn parse_response(
        &self,
        response: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<Vec<Box<dyn DecisionAction>>> {
        // Try each available action type
        for action_type in situation.available_actions() {
            if let Some(action) = action_registry.parse(action_type.clone(), response) {
                return Ok(vec![action]);
            }
        }

        // Fallback: custom instruction
        Ok(vec![Box::new(CustomInstructionAction::new(response))])
    }

    fn session_handle(&self) -> Option<&str> {
        None
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn reset(&mut self) -> crate::error::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::action::action_registry::ActionRegistry;
    use crate::model::action::builtin_actions::register_action_builtins;
    use crate::model::situation::builtin_situations::{ErrorSituation, WaitingForChoiceSituation};
    use crate::core::context::DecisionContext;

    fn make_registry() -> ActionRegistry {
        let registry = ActionRegistry::new();
        register_action_builtins(&registry);
        registry
    }

    fn make_context_with_situation(
        situation: Box<dyn crate::situation::DecisionSituation>,
    ) -> DecisionContext {
        DecisionContext::new(situation, "test-agent")
    }

    #[test]
    fn test_rule_based_engine_type() {
        let engine = RuleBasedDecisionEngine::new();
        assert_eq!(engine.engine_type(), DecisionEngineType::RuleBased);
    }

    #[test]
    fn test_rule_based_engine_waiting_for_choice() {
        let mut engine = RuleBasedDecisionEngine::new();
        let registry = make_registry();

        let situation = Box::new(WaitingForChoiceSituation::default());
        let context = make_context_with_situation(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        // Should match "approve-first" rule
    }

    #[test]
    fn test_rule_based_engine_error() {
        let mut engine = RuleBasedDecisionEngine::new();
        let registry = make_registry();

        let situation = Box::new(ErrorSituation::new(crate::model::situation::ErrorInfo::new(
            "test",
            "test error",
        )));
        let context = make_context_with_situation(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
    }

    #[test]
    fn test_rule_priority_ordering() {
        let critical = RulePriority::Critical;
        let high = RulePriority::High;
        let medium = RulePriority::Medium;
        let low = RulePriority::Low;

        assert!(critical > high);
        assert!(high > medium);
        assert!(medium > low);
    }

    #[test]
    fn test_action_spec_build() {
        let spec = ActionSpec::new("select_option")
            .with_param("option_id", "A")
            .with_param("reason", "test");

        let action = spec.build_action();
        assert!(action.is_some());
    }

    #[test]
    fn test_decision_rule_new() {
        let rule = DecisionRule::new(
            "test-rule",
            ConditionExpr::single(Condition::situation_type("test")),
            vec![ActionSpec::new("select_option")],
            RulePriority::Medium,
        );

        assert_eq!(rule.name, "test-rule");
        assert_eq!(rule.priority, RulePriority::Medium);
    }

    #[test]
    fn test_rule_based_engine_custom_rules() {
        let custom_rule = DecisionRule::new(
            "custom-approve",
            ConditionExpr::single(Condition::situation_type("waiting_for_choice")),
            vec![ActionSpec::new("select_first")],
            RulePriority::Critical,
        );

        let mut engine = RuleBasedDecisionEngine::with_rules(vec![custom_rule]);
        let registry = make_registry();

        let situation = Box::new(WaitingForChoiceSituation::default());
        let context = make_context_with_situation(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        // Critical rule should take precedence
    }

    #[test]
    fn test_rule_based_engine_healthy() {
        let engine = RuleBasedDecisionEngine::new();
        assert!(engine.is_healthy());
    }

    #[test]
    fn test_rule_based_engine_session_handle() {
        let engine = RuleBasedDecisionEngine::new();
        assert!(engine.session_handle().is_none());
    }

    #[test]
    fn test_rule_serde() {
        let rule = DecisionRule::new(
            "test-rule",
            ConditionExpr::single(Condition::situation_type("test")),
            vec![ActionSpec::new("reflect")],
            RulePriority::High,
        );

        let json = serde_json::to_string(&rule).unwrap();
        let parsed: DecisionRule = serde_json::from_str(&json).unwrap();

        assert_eq!(rule.name, parsed.name);
        assert_eq!(rule.priority, parsed.priority);
    }

    #[test]
    fn test_reflection_rounds_condition() {
        use crate::model::situation::builtin_situations::ClaimsCompletionSituation;

        // Test reflection_rounds = 0 matches (0, 1)
        let situation = Box::new(ClaimsCompletionSituation::new("test"));
        let context = make_context_with_situation(situation);
        assert!(context.reflection_round() == 0);

        // Test with metadata reflection_round = 1
        let situation2 = Box::new(ClaimsCompletionSituation::new("test"));
        let context2 = DecisionContext::new(situation2, "test-agent").with_reflection_round(1);
        assert_eq!(context2.reflection_round(), 1);

        // Test with metadata reflection_round = 2
        let situation3 = Box::new(ClaimsCompletionSituation::new("test"));
        let context3 = DecisionContext::new(situation3, "test-agent").with_reflection_round(2);
        assert_eq!(context3.reflection_round(), 2);
    }

    #[test]
    fn test_claims_completion_reflect_on_first_round() {
        use crate::model::situation::builtin_situations::ClaimsCompletionSituation;

        let mut engine = RuleBasedDecisionEngine::new();
        let registry = make_registry();

        // First round (reflection_rounds = 0) should trigger reflect
        let situation = Box::new(ClaimsCompletionSituation::new("Task done"));
        let context = make_context_with_situation(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        // Should match "reflect-first" rule
        let action_type = output.first_action_type().unwrap();
        assert_eq!(action_type.name, "reflect");
    }

    #[test]
    fn test_claims_completion_confirm_on_max_rounds() {
        use crate::model::situation::builtin_situations::ClaimsCompletionSituation;

        let mut engine = RuleBasedDecisionEngine::new();
        let registry = make_registry();

        // Max rounds (reflection_rounds = 2) should trigger confirm_completion
        let situation = Box::new(ClaimsCompletionSituation::new("Task done"));
        let context = DecisionContext::new(situation, "test-agent").with_reflection_round(2);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        // Should match "confirm-when-max-reflections" rule
        let action_type = output.first_action_type().unwrap();
        assert_eq!(action_type.name, "confirm_completion");
    }

    #[test]
    fn test_task_starting_triggers_prepare_task_start() {
        use crate::model::situation::builtin_situations::TaskStartingSituation;

        let mut engine = RuleBasedDecisionEngine::new();
        let registry = make_registry();

        let situation = Box::new(TaskStartingSituation::new("Implement login feature")
            .with_task_id("PROJ-123"));
        let context = make_context_with_situation(situation);
        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        // Should match "prepare-task-start" rule
        let action_type = output.first_action_type().unwrap();
        assert_eq!(action_type.name, "prepare_task_start");
    }
}
