//! Tests for extended KanbanElementTrait accessors

use agent_kanban::elements::{SprintElement, StoryElement, TaskElement, IssueElement, TipsElement};
use agent_kanban::traits::KanbanElementTrait;
use agent_kanban::types::StatusType;
use agent_kanban::domain::{ElementId, ElementType, Priority};

mod trait_accessor_tests {
    use super::*;

    #[test]
    fn test_task_element_content() {
        let task = TaskElement::new("Task 1");
        // Default content should be empty string
        assert_eq!(task.content(), "");
    }

    #[test]
    fn test_story_element_content() {
        let story = StoryElement::new("Story 1", "Story description here");
        assert_eq!(story.content(), "Story description here");
    }

    #[test]
    fn test_sprint_element_content() {
        let sprint = SprintElement::new("Sprint 1", "Goal: Complete feature");
        // Sprint goal is stored as content
        assert_eq!(sprint.content(), "Goal: Complete feature");
    }

    #[test]
    fn test_task_element_dependencies() {
        let task = TaskElement::new("Task 1");
        // Default dependencies should be empty
        assert_eq!(task.dependencies(), Vec::<ElementId>::new());
    }

    #[test]
    fn test_task_element_parent() {
        let parent_id = ElementId::new(ElementType::Story, 1);
        let task = TaskElement::new_with_parent("Task 1", parent_id.clone());
        assert_eq!(task.parent(), Some(parent_id));
    }

    #[test]
    fn test_task_element_no_parent() {
        let task = TaskElement::new("Task 1");
        assert_eq!(task.parent(), None);
    }

    #[test]
    fn test_task_element_priority() {
        let task = TaskElement::new("Task 1");
        // Default priority should be Medium
        assert_eq!(task.priority(), Priority::Medium);
    }

    #[test]
    fn test_issue_element_priority() {
        let issue = IssueElement::new("Issue 1");
        // Issues default to High priority
        assert_eq!(issue.priority(), Priority::High);
    }

    #[test]
    fn test_task_element_assignee() {
        let task = TaskElement::new("Task 1");
        // Default assignee should be None
        assert_eq!(task.assignee(), None);
    }

    #[test]
    fn test_task_element_effort() {
        let task = TaskElement::new("Task 1");
        // Default effort should be None
        assert_eq!(task.effort(), None);
    }

    #[test]
    fn test_task_element_blocked_reason() {
        let task = TaskElement::new("Task 1");
        // Default blocked_reason should be None
        assert_eq!(task.blocked_reason(), None);
    }

    #[test]
    fn test_task_element_tags() {
        let task = TaskElement::new("Task 1");
        // Default tags should be empty
        assert_eq!(task.tags(), Vec::<String>::new());
    }

    #[test]
    fn test_element_mutation_set_status() {
        let mut task = TaskElement::new("Task 1");
        task.set_status(StatusType::new("backlog"));
        assert_eq!(task.status().name(), "backlog");
    }

    #[test]
    fn test_element_mutation_set_id() {
        let mut task = TaskElement::new("Task 1");
        let id = ElementId::new(ElementType::Task, 42);
        task.set_id(id.clone());
        assert_eq!(task.id(), Some(id));
    }
}

mod sprint_specific_tests {
    use super::*;

    #[test]
    fn test_sprint_goal_accessor() {
        let sprint = SprintElement::new("Sprint 1", "Complete authentication feature");
        assert_eq!(sprint.goal(), "Complete authentication feature");
    }

    #[test]
    fn test_sprint_dates() {
        let sprint = SprintElement::new_with_dates(
            "Sprint 1",
            "Goal",
            "2024-01-01",
            "2024-01-14"
        );
        assert_eq!(sprint.start_date(), Some("2024-01-01".to_string()));
        assert_eq!(sprint.end_date(), Some("2024-01-14".to_string()));
    }
}

mod tips_specific_tests {
    use super::*;

    #[test]
    fn test_tips_target_task() {
        let target_task = ElementId::new(ElementType::Task, 1);
        let tips = TipsElement::new("Tip content", target_task.clone(), "agent-1");
        assert_eq!(tips.target_task(), target_task);
    }

    #[test]
    fn test_tips_agent_id() {
        let target_task = ElementId::new(ElementType::Task, 1);
        let tips = TipsElement::new("Tip content", target_task, "claude-agent");
        assert_eq!(tips.agent_id(), "claude-agent");
    }
}