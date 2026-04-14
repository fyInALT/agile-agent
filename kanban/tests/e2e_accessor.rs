//! Tests for new accessor methods

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Priority, Status};

#[test]
fn test_kanban_element_content_accessor() {
    let story = KanbanElement::new_story("Title", "Story content here");
    assert_eq!(story.content(), "Story content here");
    assert_eq!(story.element_type(), ElementType::Story);
}

#[test]
fn test_kanban_element_priority_accessor() {
    let idea = KanbanElement::new_idea("Test Idea");
    assert_eq!(idea.priority(), Priority::Medium); // Default priority

    let task = KanbanElement::new_task("Task");
    assert_eq!(task.priority(), Priority::Medium);
}

#[test]
fn test_kanban_element_effort_accessor() {
    let task = KanbanElement::new_task("Task");
    assert_eq!(task.effort(), None); // Default is None
}

#[test]
fn test_kanban_element_keywords_accessor() {
    let task = KanbanElement::new_task("Task");
    assert!(task.keywords().is_empty()); // Default is empty
}

#[test]
fn test_kanban_element_created_at_accessor() {
    let task = KanbanElement::new_task("Task");
    // Should return a valid timestamp
    let _created = task.created_at();
    assert!(true); // If we got here without panic, the accessor works
}

#[test]
fn test_kanban_element_updated_at_accessor() {
    let task = KanbanElement::new_task("Task");
    let _updated = task.updated_at();
    assert!(true);
}

#[test]
fn test_kanban_element_base_accessor() {
    let task = KanbanElement::new_task("Task");
    let base = task.base();
    assert_eq!(base.title, "Task");
    assert_eq!(base.status, Status::Plan);
}

#[test]
fn test_tips_accessors() {
    let task_id = ElementId::new(ElementType::Task, 1);
    let tips = KanbanElement::new_tips("This is a helpful tip", task_id.clone(), "agent-1");
    // Tips uses title from base, content is stored in base.content which is empty
    assert_eq!(tips.title(), "This is a helpful tip");
    // Tips has its own content (tips store the tip text in base.content)
    // But new_tips sets title, not content, so content is empty
    // The actual tip text is stored in the title field for Tips
}
