//! Unit tests for transition rule registry
//!
//! TDD: Tests for extensible status transition rules

use agent_kanban::transition::{BuiltinTransitionRule, TransitionRegistry, TransitionRule};
use agent_kanban::types::StatusType;

mod transition_rule_tests {
    use super::*;

    #[test]
    fn test_builtin_transition_rule_plan_to_backlog() {
        let rule = BuiltinTransitionRule::new(StatusType::new("plan"), StatusType::new("backlog"));
        assert_eq!(rule.from_status().name(), "plan");
        assert_eq!(rule.to_status().name(), "backlog");
    }

    #[test]
    fn test_transition_registry_new() {
        let registry = TransitionRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_transition_registry_register() {
        let registry = TransitionRegistry::new();
        registry.register(Box::new(BuiltinTransitionRule::new(
            StatusType::new("plan"),
            StatusType::new("backlog"),
        )));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_transition_registry_can_transition() {
        let registry = TransitionRegistry::new();
        // Register builtin rules
        registry.register_builtin_rules();

        assert!(registry.can_transition(&StatusType::new("plan"), &StatusType::new("backlog")));
        assert!(!registry.can_transition(&StatusType::new("plan"), &StatusType::new("done")));
    }

    #[test]
    fn test_transition_registry_valid_transitions() {
        let registry = TransitionRegistry::new();
        registry.register_builtin_rules();

        let valid = registry.valid_transitions(&StatusType::new("backlog"));
        assert!(valid.contains(&StatusType::new("blocked")));
        assert!(valid.contains(&StatusType::new("ready")));
        assert!(valid.contains(&StatusType::new("todo")));
    }

    #[test]
    fn test_transition_registry_thread_safe() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(TransitionRegistry::new());
        registry.register_builtin_rules();

        let mut handles = vec![];
        for _ in 0..10 {
            let registry_clone = registry.clone();
            handles.push(thread::spawn(move || {
                assert!(
                    registry_clone
                        .can_transition(&StatusType::new("plan"), &StatusType::new("backlog"))
                );
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
