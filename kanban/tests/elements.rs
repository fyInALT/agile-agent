//! Tests for concrete KanbanElementTrait implementations

use agent_kanban::domain::ElementId;
use agent_kanban::elements::{
    IdeaElement, IssueElement, SprintElement, StoryElement, TaskElement, TipsElement,
};
use agent_kanban::traits::KanbanElementTrait;
use agent_kanban::types::StatusType;

mod sprint_element_tests {
    use super::*;

    #[test]
    fn test_sprint_element_new() {
        let sprint = SprintElement::new("Sprint 1", "Goal: Complete feature");
        assert_eq!(sprint.title(), "Sprint 1");
        assert_eq!(sprint.element_type().name(), "sprint");
        assert_eq!(sprint.status().name(), "plan");
        assert_eq!(sprint.implementation_type(), "SprintElement");
    }

    #[test]
    fn test_sprint_element_with_id() {
        let mut sprint = SprintElement::new("Sprint 1", "Goal");
        let id = ElementId::new(agent_kanban::domain::ElementType::Sprint, 1);
        sprint.set_id(id.clone());
        assert_eq!(sprint.id(), Some(id));
    }

    #[test]
    fn test_sprint_element_clone_boxed() {
        let sprint = SprintElement::new("Sprint 1", "Goal");
        let cloned = sprint.clone_boxed();
        assert_eq!(cloned.title(), "Sprint 1");
        assert_eq!(cloned.implementation_type(), "SprintElement");
    }
}

mod story_element_tests {
    use super::*;

    #[test]
    fn test_story_element_new() {
        let story = StoryElement::new("Story 1", "Story description");
        assert_eq!(story.title(), "Story 1");
        assert_eq!(story.element_type().name(), "story");
        assert_eq!(story.status().name(), "plan");
        assert_eq!(story.implementation_type(), "StoryElement");
    }

    #[test]
    fn test_story_element_with_parent() {
        let parent_id = ElementId::new(agent_kanban::domain::ElementType::Sprint, 1);
        let story = StoryElement::new_with_parent("Story 1", "Content", parent_id.clone());
        // Parent is stored in base element
        assert_eq!(story.title(), "Story 1");
    }
}

mod task_element_tests {
    use super::*;

    #[test]
    fn test_task_element_new() {
        let task = TaskElement::new("Task 1");
        assert_eq!(task.title(), "Task 1");
        assert_eq!(task.element_type().name(), "task");
        assert_eq!(task.status().name(), "plan");
        assert_eq!(task.implementation_type(), "TaskElement");
    }

    #[test]
    fn test_task_element_with_parent() {
        let parent_id = ElementId::new(agent_kanban::domain::ElementType::Story, 1);
        let task = TaskElement::new_with_parent("Task 1", parent_id.clone());
        assert_eq!(task.title(), "Task 1");
    }

    #[test]
    fn test_task_element_set_status() {
        let mut task = TaskElement::new("Task 1");
        task.set_status(StatusType::new("backlog"));
        assert_eq!(task.status().name(), "backlog");
    }
}

mod idea_element_tests {
    use super::*;

    #[test]
    fn test_idea_element_new() {
        let idea = IdeaElement::new("Idea 1");
        assert_eq!(idea.title(), "Idea 1");
        assert_eq!(idea.element_type().name(), "idea");
        assert_eq!(idea.implementation_type(), "IdeaElement");
    }
}

mod issue_element_tests {
    use super::*;

    #[test]
    fn test_issue_element_new() {
        let issue = IssueElement::new("Issue 1");
        assert_eq!(issue.title(), "Issue 1");
        assert_eq!(issue.element_type().name(), "issue");
        assert_eq!(issue.implementation_type(), "IssueElement");
    }
}

mod tips_element_tests {
    use super::*;

    #[test]
    fn test_tips_element_new() {
        let target_task = ElementId::new(agent_kanban::domain::ElementType::Task, 1);
        let tips = TipsElement::new("Tip content", target_task.clone(), "agent-1");
        assert_eq!(tips.title(), "Tip content");
        assert_eq!(tips.element_type().name(), "tips");
        assert_eq!(tips.implementation_type(), "TipsElement");
    }
}
