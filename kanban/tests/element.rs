//! Unit tests for KanbanElement

use agent_kanban::domain::{BaseElement, ElementId, ElementType, KanbanElement, Priority, Status};

mod base_element_tests {
    use super::*;

    #[test]
    fn test_base_element_new_defaults() {
        let base = BaseElement::new(ElementType::Task, "Test Task");
        assert_eq!(base.title, "Test Task");
        assert_eq!(base.status, Status::Plan);
        assert_eq!(base.priority, Priority::Medium);
        assert!(base.id.is_none());
        assert!(base.content.is_empty());
        assert!(base.keywords.is_empty());
        assert!(base.dependencies.is_empty());
        assert!(base.references.is_empty());
        assert!(base.parent.is_none());
        assert!(base.assignee.is_none());
        assert!(base.effort.is_none());
        assert!(base.blocked_reason.is_none());
        assert!(base.tags.is_empty());
    }

    #[test]
    fn test_base_element_can_transition() {
        let base = BaseElement::new(ElementType::Task, "Test");
        assert!(base.can_transition_to(&Status::Backlog));
        assert!(!base.can_transition_to(&Status::Done));
    }

    #[test]
    fn test_base_element_transition() {
        let mut base = BaseElement::new(ElementType::Task, "Test");
        let result = base.transition(Status::Backlog);
        assert!(result.is_ok());
        assert_eq!(base.status, Status::Backlog);
        assert_eq!(base.status_history.len(), 2); // Plan + Backlog
    }

    #[test]
    fn test_base_element_invalid_transition() {
        let mut base = BaseElement::new(ElementType::Task, "Test");
        let result = base.transition(Status::Done);
        assert!(result.is_err());
        assert_eq!(base.status, Status::Plan); // Unchanged
    }
}

mod sprint_tests {
    use super::*;

    #[test]
    fn test_new_sprint() {
        let sprint = KanbanElement::new_sprint("Sprint 1", "Complete project");
        match sprint {
            KanbanElement::Sprint(s) => {
                assert_eq!(s.base.title, "Sprint 1");
                assert_eq!(s.base.status, Status::Plan);
                assert_eq!(s.goal, "Complete project");
                assert!(!s.active);
                assert!(s.start_date.is_none());
                assert!(s.end_date.is_none());
            }
            _ => panic!("Expected Sprint variant"),
        }
    }

    #[test]
    fn test_sprint_with_dates() {
        let sprint = KanbanElement::new_sprint_with_dates(
            "Sprint 1",
            "Complete project",
            "2026-04-01",
            "2026-04-14",
        );
        match sprint {
            KanbanElement::Sprint(s) => {
                assert!(s.start_date.is_some());
                assert!(s.end_date.is_some());
                assert!(s.active);
            }
            _ => panic!("Expected Sprint variant"),
        }
    }
}

mod story_tests {
    use super::*;

    #[test]
    fn test_new_story() {
        let story = KanbanElement::new_story("User Story 1", "As a user...");
        match story {
            KanbanElement::Story(s) => {
                assert_eq!(s.base.title, "User Story 1");
                assert_eq!(s.base.status, Status::Plan);
                assert!(s.base.parent.is_none());
            }
            _ => panic!("Expected Story variant"),
        }
    }

    #[test]
    fn test_story_with_parent() {
        let story = KanbanElement::new_story_with_parent(
            "User Story 1",
            "As a user...",
            ElementId::new(ElementType::Sprint, 1),
        );
        match story {
            KanbanElement::Story(s) => {
                assert!(s.base.parent.is_some());
                assert_eq!(s.base.parent.unwrap().type_(), ElementType::Sprint);
            }
            _ => panic!("Expected Story variant"),
        }
    }
}

mod task_tests {
    use super::*;

    #[test]
    fn test_new_task() {
        let task = KanbanElement::new_task("Implement feature X");
        match task {
            KanbanElement::Task(t) => {
                assert_eq!(t.base.title, "Implement feature X");
                assert_eq!(t.base.status, Status::Plan);
            }
            _ => panic!("Expected Task variant"),
        }
    }

    #[test]
    fn test_task_with_parent() {
        let task = KanbanElement::new_task_with_parent(
            "Implement feature X",
            ElementId::new(ElementType::Story, 1),
        );
        match task {
            KanbanElement::Task(t) => {
                assert!(t.base.parent.is_some());
                assert_eq!(t.base.parent.unwrap().type_(), ElementType::Story);
            }
            _ => panic!("Expected Task variant"),
        }
    }
}

mod idea_tests {
    use super::*;

    #[test]
    fn test_new_idea() {
        let idea = KanbanElement::new_idea("New feature idea");
        match idea {
            KanbanElement::Idea(i) => {
                assert_eq!(i.base.title, "New feature idea");
                assert_eq!(i.base.status, Status::Plan);
            }
            _ => panic!("Expected Idea variant"),
        }
    }
}

mod issue_tests {
    use super::*;

    #[test]
    fn test_new_issue() {
        let issue = KanbanElement::new_issue("Bug in login");
        match issue {
            KanbanElement::Issue(i) => {
                assert_eq!(i.base.title, "Bug in login");
                assert!(i.base.priority == Priority::High || i.base.priority == Priority::Medium);
            }
            _ => panic!("Expected Issue variant"),
        }
    }
}

mod tips_tests {
    use super::*;

    #[test]
    fn test_new_tips() {
        let tips = KanbanElement::new_tips(
            "How to debug",
            ElementId::new(ElementType::Task, 1),
            "agent-123",
        );
        match tips {
            KanbanElement::Tips(t) => {
                assert_eq!(t.base.title, "How to debug");
                assert_eq!(t.target_task, ElementId::new(ElementType::Task, 1));
                assert_eq!(t.agent_id, "agent-123");
            }
            _ => panic!("Expected Tips variant"),
        }
    }
}

mod kanban_element_tests {
    use super::*;

    #[test]
    fn test_element_id_accessor() {
        let mut element = KanbanElement::new_task("Test");
        element.set_id(ElementId::new(ElementType::Task, 1));
        assert_eq!(element.id().unwrap().as_str(), "task-001");
    }

    #[test]
    fn test_element_type_accessor() {
        let element = KanbanElement::new_story("Test", "Content");
        assert_eq!(element.element_type(), ElementType::Story);
    }

    #[test]
    fn test_element_status_accessor() {
        let element = KanbanElement::new_idea("Test");
        assert_eq!(element.status(), Status::Plan);
    }

    #[test]
    fn test_element_can_transition() {
        let element = KanbanElement::new_task("Test");
        assert!(element.can_transition_to(&Status::Backlog));
        assert!(!element.can_transition_to(&Status::Done));
    }

    #[test]
    fn test_element_transition() {
        let mut element = KanbanElement::new_task("Test");
        let result = element.transition(Status::Backlog);
        assert!(result.is_ok());
        assert_eq!(element.status(), Status::Backlog);
    }

    #[test]
    fn test_element_invalid_transition() {
        let mut element = KanbanElement::new_task("Test");
        let result = element.transition(Status::Done);
        assert!(result.is_err());
        assert_eq!(element.status(), Status::Plan); // Unchanged
    }

    #[test]
    fn test_assignee_accessor() {
        let element = KanbanElement::new_task("Test");
        assert!(element.assignee().is_none());
    }

    #[test]
    fn test_dependencies_accessor() {
        let element = KanbanElement::new_task("Test");
        assert!(element.dependencies().is_empty());
    }

    #[test]
    fn test_references_accessor() {
        let element = KanbanElement::new_task("Test");
        assert!(element.references().is_empty());
    }

    #[test]
    fn test_parent_accessor() {
        let element = KanbanElement::new_task("Test");
        assert!(element.parent().is_none());
    }

    #[test]
    fn test_set_status() {
        let mut element = KanbanElement::new_task("Test");
        element.set_status(Status::Backlog);
        assert_eq!(element.status(), Status::Backlog);
    }

    #[test]
    fn test_set_id() {
        let mut element = KanbanElement::new_task("Test");
        let id = ElementId::new(ElementType::Task, 5);
        element.set_id(id.clone());
        assert_eq!(element.id().unwrap(), &id);
    }

    #[test]
    fn test_base_mut() {
        let mut element = KanbanElement::new_task("Test");
        element.base_mut().title = "Updated".to_string();
        assert_eq!(element.title(), "Updated");
    }
}

mod serialization_tests {
    use super::*;

    #[test]
    fn test_sprint_serialization() {
        let sprint =
            KanbanElement::new_sprint_with_dates("Sprint 1", "Goal", "2026-04-01", "2026-04-14");
        let json = serde_json::to_string_pretty(&sprint).unwrap();
        assert!(json.contains("\"type\": \"sprint\""));
        assert!(json.contains("\"title\": \"Sprint 1\""));
        assert!(json.contains("\"goal\": \"Goal\""));
    }

    #[test]
    fn test_task_serialization() {
        let task = KanbanElement::new_task_with_parent(
            "Implement X",
            ElementId::new(ElementType::Story, 1),
        );
        let json = serde_json::to_string_pretty(&task).unwrap();
        assert!(json.contains("\"type\": \"task\""));
        assert!(json.contains("\"title\": \"Implement X\""));
    }

    #[test]
    fn test_tips_serialization() {
        let tips =
            KanbanElement::new_tips("Debug tip", ElementId::new(ElementType::Task, 1), "agent-1");
        let json = serde_json::to_string_pretty(&tips).unwrap();
        assert!(json.contains("\"type\": \"tips\""));
        assert!(json.contains("\"target_task\": \"task-001\""));
        assert!(json.contains("\"agent_id\": \"agent-1\""));
    }

    #[test]
    fn test_deserialization() {
        let json = r#"{"type":"story","title":"Test","content":"","keywords":[],"status":"plan","dependencies":[],"references":[],"parent":null,"created_at":"2026-04-13T00:00:00Z","updated_at":"2026-04-13T00:00:00Z","priority":"medium","assignee":null,"effort":null,"blocked_reason":null,"tags":[],"status_history":[{"status":"plan","entered_at":"2026-04-13T00:00:00Z"}]}"#;
        let element: KanbanElement = serde_json::from_str(json).unwrap();
        assert_eq!(element.element_type(), ElementType::Story);
        assert_eq!(element.title(), "Test");
    }

    #[test]
    fn test_roundtrip_serialization() {
        let original =
            KanbanElement::new_tips("Debug tip", ElementId::new(ElementType::Task, 1), "agent-1");
        let json = serde_json::to_string(&original).unwrap();
        let restored: KanbanElement = serde_json::from_str(&json).unwrap();
        assert_eq!(original.title(), restored.title());
        assert_eq!(original.element_type(), restored.element_type());
    }
}
