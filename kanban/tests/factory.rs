//! Tests for ElementFactory

use agent_kanban::factory::ElementFactory;
use agent_kanban::registry::ElementTypeRegistry;
use agent_kanban::types::ElementTypeIdentifier;

mod element_factory_tests {
    use super::*;

    #[test]
    fn test_element_factory_new() {
        let factory = ElementFactory::new();
        assert!(factory.can_create(&ElementTypeIdentifier::new("task")));
        assert!(factory.can_create(&ElementTypeIdentifier::new("story")));
        assert!(factory.can_create(&ElementTypeIdentifier::new("sprint")));
    }

    #[test]
    fn test_element_factory_create_task() {
        let factory = ElementFactory::new();
        let task = factory.create(&ElementTypeIdentifier::new("task"), "Test Task");
        assert!(task.is_some());
        let task = task.unwrap();
        assert_eq!(task.title(), "Test Task");
        assert_eq!(task.element_type().name(), "task");
    }

    #[test]
    fn test_element_factory_create_story() {
        let factory = ElementFactory::new();
        let story = factory.create_with_content(&ElementTypeIdentifier::new("story"), "Story Title", "Story content");
        assert!(story.is_some());
        let story = story.unwrap();
        assert_eq!(story.title(), "Story Title");
        assert_eq!(story.element_type().name(), "story");
    }

    #[test]
    fn test_element_factory_create_sprint() {
        let factory = ElementFactory::new();
        let sprint = factory.create_sprint("Sprint 1", "Goal: Feature");
        assert_eq!(sprint.title(), "Sprint 1");
        assert_eq!(sprint.element_type().name(), "sprint");
    }

    #[test]
    fn test_element_factory_create_unknown_type() {
        let factory = ElementFactory::new();
        let result = factory.create(&ElementTypeIdentifier::new("unknown_type"), "Test");
        assert!(result.is_none());
    }

    #[test]
    fn test_element_factory_with_registry() {
        let registry = ElementTypeRegistry::new();
        let factory = ElementFactory::with_registry(registry);
        // Default factory should still work
        assert!(factory.can_create(&ElementTypeIdentifier::new("task")));
    }
}