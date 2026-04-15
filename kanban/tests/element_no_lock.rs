//! Tests for elements without RwLock overhead

use agent_kanban::domain::{ElementId, ElementType, Priority};
use agent_kanban::elements::{IssueElement, SprintElement, TaskElement};
use agent_kanban::traits::KanbanElementTrait;
use agent_kanban::types::StatusType;

/// Test that elements can be created and accessed without panic from unwrap
mod no_lock_panic_tests {
    use super::*;

    #[test]
    fn test_sprint_access_no_panic() {
        let sprint = SprintElement::new("Sprint 1", "Goal");
        // All accessor calls should not panic
        let _ = sprint.id();
        let _ = sprint.title();
        let _ = sprint.content();
        let _ = sprint.status();
        let _ = sprint.priority();
        let _ = sprint.dependencies();
        let _ = sprint.tags();
    }

    #[test]
    fn test_task_access_no_panic() {
        let task = TaskElement::new("Task 1");
        let _ = task.id();
        let _ = task.title();
        let _ = task.content();
        let _ = task.status();
        let _ = task.priority();
    }

    #[test]
    fn test_issue_access_no_panic() {
        let issue = IssueElement::new("Issue 1");
        let _ = issue.id();
        let _ = issue.title();
        let _ = issue.content();
        let _ = issue.status();
        assert_eq!(issue.priority(), Priority::High);
    }

    #[test]
    fn test_element_mutation_no_panic() {
        let mut task = TaskElement::new("Task 1");
        task.set_status(StatusType::new("backlog"));
        assert_eq!(task.status().name(), "backlog");

        let mut sprint = SprintElement::new("Sprint", "Goal");
        let id = ElementId::new(ElementType::Sprint, 1);
        sprint.set_id(id.clone());
        assert_eq!(sprint.id(), Some(id));
    }
}

/// Test cloning boxed elements without lock overhead
mod clone_tests {
    use super::*;

    #[test]
    fn test_clone_boxed_task() {
        let task = TaskElement::new("Original Task");
        let cloned = task.clone_boxed();
        assert_eq!(cloned.title(), "Original Task");
        assert_eq!(cloned.implementation_type(), "TaskElement");
    }

    #[test]
    fn test_clone_boxed_sprint() {
        let sprint = SprintElement::new("Sprint 1", "Goal text");
        let cloned = sprint.clone_boxed();
        assert_eq!(cloned.title(), "Sprint 1");
        assert_eq!(cloned.content(), "Goal text");
    }
}
