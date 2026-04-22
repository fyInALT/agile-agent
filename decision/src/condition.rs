//! Condition expressions for rule-based decisions

use crate::core::context::DecisionContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Condition expression - supports complex logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionExpr {
    /// Single condition
    Single(Condition),

    /// AND combination - all must match
    And(Vec<ConditionExpr>),

    /// OR combination - any must match
    Or(Vec<ConditionExpr>),

    /// NOT - negates inner expression
    Not(Box<ConditionExpr>),
}

impl ConditionExpr {
    /// Create a single condition
    pub fn single(condition: Condition) -> Self {
        ConditionExpr::Single(condition)
    }

    /// Create an AND combination
    pub fn and(exprs: Vec<ConditionExpr>) -> Self {
        ConditionExpr::And(exprs)
    }

    /// Create an OR combination
    pub fn or(exprs: Vec<ConditionExpr>) -> Self {
        ConditionExpr::Or(exprs)
    }

    /// Create a NOT expression
    #[allow(clippy::should_implement_trait)]
    pub fn not(expr: ConditionExpr) -> Self {
        ConditionExpr::Not(Box::new(expr))
    }

    /// Evaluate against context
    pub fn evaluate(
        &self,
        context: &DecisionContext,
        registry: &ConditionEvaluatorRegistry,
    ) -> bool {
        match self {
            ConditionExpr::Single(cond) => cond.evaluate(context, registry),

            ConditionExpr::And(exprs) => exprs.iter().all(|e| e.evaluate(context, registry)),

            ConditionExpr::Or(exprs) => exprs.iter().any(|e| e.evaluate(context, registry)),

            ConditionExpr::Not(expr) => !expr.evaluate(context, registry),
        }
    }
}

/// Single condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// Situation type matches
    SituationType { type_name: String },

    /// Project rule keyword present
    ProjectKeyword { keyword: String },

    /// Reflection rounds within range
    ReflectionRounds { min: u8, max: u8 },

    /// Confidence below threshold
    ConfidenceBelow { threshold: f64 },

    /// Time since last action in seconds range
    TimeSinceLastAction {
        min_seconds: u64,
        max_seconds: Option<u64>,
    },

    /// Custom condition (extensible)
    Custom {
        name: String,
        params: HashMap<String, String>,
    },
}

impl Condition {
    /// Create a situation type condition
    pub fn situation_type(type_name: impl Into<String>) -> Self {
        Condition::SituationType {
            type_name: type_name.into(),
        }
    }

    /// Create a project keyword condition
    pub fn project_keyword(keyword: impl Into<String>) -> Self {
        Condition::ProjectKeyword {
            keyword: keyword.into(),
        }
    }

    /// Create a reflection rounds condition
    pub fn reflection_rounds(min: u8, max: u8) -> Self {
        Condition::ReflectionRounds { min, max }
    }

    /// Create a confidence below condition
    pub fn confidence_below(threshold: f64) -> Self {
        Condition::ConfidenceBelow { threshold }
    }

    /// Create a custom condition
    pub fn custom(name: impl Into<String>, params: HashMap<String, String>) -> Self {
        Condition::Custom {
            name: name.into(),
            params,
        }
    }

    /// Evaluate against context
    pub fn evaluate(
        &self,
        context: &DecisionContext,
        registry: &ConditionEvaluatorRegistry,
    ) -> bool {
        match self {
            Condition::SituationType { type_name } => {
                context.trigger_situation.situation_type().name == *type_name
            }

            Condition::ProjectKeyword { keyword } => {
                context.project_rules.contains_keyword(keyword)
            }

            Condition::ReflectionRounds { min, max } => {
                let rounds = context.reflection_round();
                rounds >= *min && rounds <= *max
            }

            Condition::ConfidenceBelow { threshold: _ } => {
                // Would need confidence from situation or context
                false
            }

            Condition::TimeSinceLastAction {
                min_seconds: _,
                max_seconds: _,
            } => {
                // Would need timestamp tracking
                false
            }

            Condition::Custom { name, params } => registry.evaluate(name, context, params),
        }
    }
}

/// Custom condition evaluator trait
pub trait ConditionEvaluator: Send + Sync {
    fn evaluate(&self, context: &DecisionContext, params: &HashMap<String, String>) -> bool;
}

/// Condition evaluator registry
pub struct ConditionEvaluatorRegistry {
    evaluators: HashMap<String, Box<dyn ConditionEvaluator>>,
}

impl ConditionEvaluatorRegistry {
    pub fn new() -> Self {
        Self {
            evaluators: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: impl Into<String>, evaluator: Box<dyn ConditionEvaluator>) {
        self.evaluators.insert(name.into(), evaluator);
    }

    pub fn evaluate(
        &self,
        name: &str,
        context: &DecisionContext,
        params: &HashMap<String, String>,
    ) -> bool {
        self.evaluators
            .get(name)
            .map(|e| e.evaluate(context, params))
            .unwrap_or(false)
    }
}

impl Default for ConditionEvaluatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::situation::builtin_situations::WaitingForChoiceSituation;
    use crate::core::context::DecisionContext;

    fn make_context() -> DecisionContext {
        DecisionContext::new(Box::new(WaitingForChoiceSituation::default()), "test-agent")
    }

    #[test]
    fn test_condition_expr_single() {
        let expr = ConditionExpr::single(Condition::situation_type("waiting_for_choice"));
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(expr.evaluate(&ctx, &registry));
    }

    #[test]
    fn test_condition_expr_and() {
        let expr = ConditionExpr::and(vec![
            ConditionExpr::single(Condition::situation_type("waiting_for_choice")),
            ConditionExpr::single(Condition::project_keyword("test")),
        ]);
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(!expr.evaluate(&ctx, &registry)); // No project keyword
    }

    #[test]
    fn test_condition_expr_or() {
        let expr = ConditionExpr::or(vec![
            ConditionExpr::single(Condition::situation_type("waiting_for_choice")),
            ConditionExpr::single(Condition::situation_type("error")),
        ]);
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(expr.evaluate(&ctx, &registry)); // First matches
    }

    #[test]
    fn test_condition_expr_not() {
        let expr = ConditionExpr::not(ConditionExpr::single(Condition::situation_type("error")));
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(expr.evaluate(&ctx, &registry)); // Not error
    }

    #[test]
    fn test_condition_situation_type() {
        let cond = Condition::situation_type("waiting_for_choice");
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(cond.evaluate(&ctx, &registry));
    }

    #[test]
    fn test_condition_situation_type_no_match() {
        let cond = Condition::situation_type("error");
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(!cond.evaluate(&ctx, &registry));
    }

    #[test]
    fn test_condition_project_keyword() {
        let cond = Condition::project_keyword("test_keyword");
        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert!(!cond.evaluate(&ctx, &registry)); // No keyword in default rules
    }

    #[test]
    fn test_condition_custom_evaluator() {
        struct TestEvaluator;
        impl ConditionEvaluator for TestEvaluator {
            fn evaluate(
                &self,
                _context: &DecisionContext,
                params: &HashMap<String, String>,
            ) -> bool {
                params.get("value").map(|v| v == "true").unwrap_or(false)
            }
        }

        let mut registry = ConditionEvaluatorRegistry::new();
        registry.register("test_eval", Box::new(TestEvaluator));

        let ctx = make_context();
        let params = HashMap::from([("value".to_string(), "true".to_string())]);
        let cond = Condition::custom("test_eval", params);

        assert!(cond.evaluate(&ctx, &registry));
    }

    #[test]
    fn test_condition_evaluator_registry() {
        let registry = ConditionEvaluatorRegistry::new();
        let ctx = make_context();
        let params = HashMap::new();

        // Unknown evaluator returns false
        assert!(!registry.evaluate("unknown", &ctx, &params));
    }

    #[test]
    fn test_condition_expr_serde() {
        let expr = ConditionExpr::and(vec![
            ConditionExpr::single(Condition::situation_type("test")),
            ConditionExpr::single(Condition::project_keyword("key")),
        ]);
        let json = serde_json::to_string(&expr).unwrap();
        let parsed: ConditionExpr = serde_json::from_str(&json).unwrap();

        let ctx = make_context();
        let registry = ConditionEvaluatorRegistry::new();
        assert_eq!(
            expr.evaluate(&ctx, &registry),
            parsed.evaluate(&ctx, &registry)
        );
    }
}
