//! Tests for element-context-aware transition rules

use agent_kanban::transition::{TransitionRegistry, TransitionRule, BuiltinTransitionRule};
use agent_kanban::types::StatusType;
use agent_kanban::traits::KanbanElementTrait;
use agent_kanban::elements::{TaskElement, SprintElement};

/// Custom transition rule that only allows Tasks to go to in_progress
struct TaskOnlyInProgressRule {
    from: StatusType,
    to: StatusType,
}

impl TaskOnlyInProgressRule {
    fn new() -> Self {
        Self {
            from: StatusType::new("todo"),
            to: StatusType::new("in_progress"),
        }
    }
}

impl TransitionRule for TaskOnlyInProgressRule {
    fn from_status(&self) -> StatusType {
        self.from.clone()
    }

    fn to_status(&self) -> StatusType {
        self.to.clone()
    }

    fn is_valid_for(&self, element: &dyn KanbanElementTrait) -> bool {
        // Only Task elements can transition to in_progress
        element.element_type().name() == "task"
    }

    fn clone_boxed(&self) -> Box<dyn TransitionRule> {
        Box::new(TaskOnlyInProgressRule::new())
    }
}

mod element_context_tests {
    use super::*;

    #[test]
    fn test_task_can_transition_to_in_progress() {
        let task = TaskElement::new("Test Task");
        let rule = TaskOnlyInProgressRule::new();

        // Task should be allowed to transition
        assert!(rule.is_valid_for(&task));
    }

    #[test]
    fn test_sprint_cannot_transition_to_in_progress() {
        let sprint = SprintElement::new("Sprint 1", "Goal");
        let rule = TaskOnlyInProgressRule::new();

        // Sprint should NOT be allowed to transition to in_progress
        assert!(!rule.is_valid_for(&sprint));
    }

    #[test]
    fn test_registry_with_element_context() {
        let registry = TransitionRegistry::new();
        registry.register(Box::new(TaskOnlyInProgressRule::new()));

        let task = TaskElement::new("Test Task");
        let sprint = SprintElement::new("Sprint 1", "Goal");

        // Registry should check element context
        assert!(registry.can_transition_for(&StatusType::new("todo"), &StatusType::new("in_progress"), &task));
        assert!(!registry.can_transition_for(&StatusType::new("todo"), &StatusType::new("in_progress"), &sprint));
    }

    #[test]
    fn test_default_is_valid_for_returns_true() {
        let rule = BuiltinTransitionRule::new(StatusType::new("plan"), StatusType::new("backlog"));
        let task = TaskElement::new("Test Task");

        // Default implementation should always return true
        assert!(rule.is_valid_for(&task));
    }
}

mod valid_transitions_with_context_tests {
    use super::*;

    #[test]
    fn test_valid_transitions_for_task() {
        let registry = TransitionRegistry::new();
        registry.register(Box::new(TaskOnlyInProgressRule::new()));

        let task = TaskElement::new("Test Task");
        let sprint = SprintElement::new("Sprint", "Goal");

        // For task, todo -> in_progress should be in valid transitions
        let task_transitions = registry.valid_transitions_for(&StatusType::new("todo"), &task);
        assert!(task_transitions.iter().any(|s| s.name() == "in_progress"));

        // For sprint, todo -> in_progress should NOT be in valid transitions
        let sprint_transitions = registry.valid_transitions_for(&StatusType::new("todo"), &sprint);
        assert!(!sprint_transitions.iter().any(|s| s.name() == "in_progress"));
    }
}