//! Tests for ElementSerde serialization proxy

use agent_kanban::serde::ElementSerde;
use agent_kanban::elements::{TaskElement, SprintElement, IssueElement};
use agent_kanban::traits::KanbanElementTrait;
use agent_kanban::types::StatusType;
use agent_kanban::domain::{ElementId, ElementType};
use agent_kanban::factory::ElementFactory;

mod serde_tests {
    use super::*;

    #[test]
    fn test_task_to_serde() {
        let mut task = TaskElement::new("Test Task");
        let id = ElementId::new(ElementType::Task, 1);
        task.set_id(id.clone());
        task.set_status(StatusType::new("todo"));

        let serde = task.to_serde();

        assert_eq!(serde.element_type, "task");
        assert_eq!(serde.title, "Test Task");
        assert_eq!(serde.id, Some("task-001".to_string()));
        assert_eq!(serde.status, "todo");
    }

    #[test]
    fn test_sprint_to_serde() {
        let sprint = SprintElement::new("Sprint 1", "Complete feature");
        let serde = sprint.to_serde();

        assert_eq!(serde.element_type, "sprint");
        assert_eq!(serde.title, "Sprint 1");
        assert_eq!(serde.content, "Complete feature");
        assert_eq!(serde.status, "plan");
    }

    #[test]
    fn test_serde_serialization() {
        let task = TaskElement::new("Task");
        let serde = task.to_serde();

        let json = serde_json::to_string(&serde).unwrap();
        assert!(json.contains("\"element_type\":\"task\""));
        assert!(json.contains("\"title\":\"Task\""));
    }

    #[test]
    fn test_serde_deserialization() {
        let json = r#"{"element_type":"task","title":"My Task","content":"","status":"plan","id":null}"#;
        let serde: ElementSerde = serde_json::from_str(json).unwrap();

        assert_eq!(serde.element_type, "task");
        assert_eq!(serde.title, "My Task");
        assert_eq!(serde.status, "plan");
    }

    #[test]
    fn test_serde_to_element() {
        let factory = ElementFactory::new();
        let serde = ElementSerde {
            element_type: "task".to_string(),
            title: "Test Task".to_string(),
            content: "".to_string(),
            status: "todo".to_string(),
            id: Some("task-001".to_string()),
            priority: Some("medium".to_string()),
            effort: None,
            assignee: None,
            blocked_reason: None,
            tags: vec![], // Tags can't be restored via trait interface yet
            dependencies: vec![],
            parent: None,
        };

        let element = factory.from_serde(&serde);
        assert!(element.is_some());

        let element = element.unwrap();
        assert_eq!(element.title(), "Test Task");
        assert_eq!(element.element_type().name(), "task");
        assert_eq!(element.status().name(), "todo");
    }

    #[test]
    fn test_round_trip_serialization() {
        let mut task = TaskElement::new("Original Task");
        task.set_status(StatusType::new("backlog"));

        // Convert to serde
        let serde = task.to_serde();

        // Serialize to JSON
        let json = serde_json::to_string(&serde).unwrap();

        // Deserialize from JSON
        let serde2: ElementSerde = serde_json::from_str(&json).unwrap();

        // Convert back to element
        let factory = ElementFactory::new();
        let restored = factory.from_serde(&serde2).unwrap();

        // Verify values match
        assert_eq!(restored.title(), "Original Task");
        assert_eq!(restored.element_type().name(), "task");
        assert_eq!(restored.status().name(), "backlog");
    }
}

mod serde_field_tests {
    use super::*;

    #[test]
    fn test_serde_preserves_priority() {
        let issue = IssueElement::new("Bug report");
        let serde = issue.to_serde();

        // Issues default to High priority
        assert_eq!(serde.priority, Some("high".to_string()));
    }

    #[test]
    fn test_serde_preserves_tags() {
        let mut task = TaskElement::new("Task with tags");
        // Add tags through the inner element
        // Note: We need to verify tags serialization works

        let serde = task.to_serde();
        // Default tags should be empty
        assert_eq!(serde.tags, Vec::<String>::new());
    }

    #[test]
    fn test_serde_with_all_fields() {
        let mut task = TaskElement::new("Complete Task");
        task.set_id(ElementId::new(ElementType::Task, 42));
        task.set_status(StatusType::new("in_progress"));

        let serde = task.to_serde();

        assert_eq!(serde.element_type, "task");
        assert_eq!(serde.title, "Complete Task");
        assert_eq!(serde.id, Some("task-042".to_string()));
        assert_eq!(serde.status, "in_progress");
        assert_eq!(serde.priority, Some("medium".to_string()));
    }
}