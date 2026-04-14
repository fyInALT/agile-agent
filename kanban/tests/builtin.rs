//! Unit tests for builtin trait implementations
//!
//! TDD: Tests for concrete KanbanStatus and KanbanElementTypeTrait implementations

use agent_kanban::builtin::{builtin_statuses_impl, builtin_element_types_impl};
use agent_kanban::traits::{KanbanStatus, KanbanElementTypeTrait};
use agent_kanban::types::{StatusType, ElementTypeIdentifier};

mod builtin_status_tests {
    use super::*;

    #[test]
    fn test_plan_status() {
        let status = builtin_statuses_impl::plan();
        assert_eq!(status.status_type().name(), "plan");
        assert_eq!(status.implementation_type(), "PlanStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_backlog_status() {
        let status = builtin_statuses_impl::backlog();
        assert_eq!(status.status_type().name(), "backlog");
        assert_eq!(status.implementation_type(), "BacklogStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_blocked_status() {
        let status = builtin_statuses_impl::blocked();
        assert_eq!(status.status_type().name(), "blocked");
        assert_eq!(status.implementation_type(), "BlockedStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_ready_status() {
        let status = builtin_statuses_impl::ready();
        assert_eq!(status.status_type().name(), "ready");
        assert_eq!(status.implementation_type(), "ReadyStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_todo_status() {
        let status = builtin_statuses_impl::todo();
        assert_eq!(status.status_type().name(), "todo");
        assert_eq!(status.implementation_type(), "TodoStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_in_progress_status() {
        let status = builtin_statuses_impl::in_progress();
        assert_eq!(status.status_type().name(), "in_progress");
        assert_eq!(status.implementation_type(), "InProgressStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_done_status() {
        let status = builtin_statuses_impl::done();
        assert_eq!(status.status_type().name(), "done");
        assert_eq!(status.implementation_type(), "DoneStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_verified_status() {
        let status = builtin_statuses_impl::verified();
        assert_eq!(status.status_type().name(), "verified");
        assert_eq!(status.implementation_type(), "VerifiedStatus");
        assert!(status.is_terminal());
    }

    #[test]
    fn test_builtin_status_clone_boxed() {
        let status = builtin_statuses_impl::plan();
        let cloned = status.clone_boxed();
        assert_eq!(cloned.status_type().name(), "plan");
        assert_eq!(cloned.implementation_type(), "PlanStatus");
    }

    #[test]
    fn test_all_builtin_statuses() {
        let all = builtin_statuses_impl::all();
        assert_eq!(all.len(), 8);

        let names: Vec<String> = all.iter().map(|s| s.status_type().name().to_string()).collect();
        assert!(names.contains(&"plan".to_string()));
        assert!(names.contains(&"backlog".to_string()));
        assert!(names.contains(&"verified".to_string()));
    }
}

mod builtin_element_type_tests {
    use super::*;

    #[test]
    fn test_sprint_element_type() {
        let elem_type = builtin_element_types_impl::sprint();
        assert_eq!(elem_type.element_type().name(), "sprint");
        assert_eq!(elem_type.implementation_type(), "SprintElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_story_element_type() {
        let elem_type = builtin_element_types_impl::story();
        assert_eq!(elem_type.element_type().name(), "story");
        assert_eq!(elem_type.implementation_type(), "StoryElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_task_element_type() {
        let elem_type = builtin_element_types_impl::task();
        assert_eq!(elem_type.element_type().name(), "task");
        assert_eq!(elem_type.implementation_type(), "TaskElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_idea_element_type() {
        let elem_type = builtin_element_types_impl::idea();
        assert_eq!(elem_type.element_type().name(), "idea");
        assert_eq!(elem_type.implementation_type(), "IdeaElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_issue_element_type() {
        let elem_type = builtin_element_types_impl::issue();
        assert_eq!(elem_type.element_type().name(), "issue");
        assert_eq!(elem_type.implementation_type(), "IssueElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_tips_element_type() {
        let elem_type = builtin_element_types_impl::tips();
        assert_eq!(elem_type.element_type().name(), "tips");
        assert_eq!(elem_type.implementation_type(), "TipsElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_builtin_element_type_clone_boxed() {
        let elem_type = builtin_element_types_impl::task();
        let cloned = elem_type.clone_boxed();
        assert_eq!(cloned.element_type().name(), "task");
        assert_eq!(cloned.implementation_type(), "TaskElementType");
    }

    #[test]
    fn test_all_builtin_element_types() {
        let all = builtin_element_types_impl::all();
        assert_eq!(all.len(), 6);

        let names: Vec<String> = all.iter().map(|t| t.element_type().name().to_string()).collect();
        assert!(names.contains(&"sprint".to_string()));
        assert!(names.contains(&"story".to_string()));
        assert!(names.contains(&"task".to_string()));
        assert!(names.contains(&"idea".to_string()));
        assert!(names.contains(&"issue".to_string()));
        assert!(names.contains(&"tips".to_string()));
    }
}