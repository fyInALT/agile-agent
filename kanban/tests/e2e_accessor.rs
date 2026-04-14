//! Tests for new accessor methods

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Priority, Status};
use chrono::Utc;

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
fn test_set_effort() {
    let mut task = KanbanElement::new_task("Task");
    assert!(task.effort().is_none());

    task.set_effort(5);
    assert_eq!(task.effort(), Some(5));

    task.set_effort(8);
    assert_eq!(task.effort(), Some(8)); // Updated
}

#[test]
fn test_clear_effort() {
    let mut task = KanbanElement::new_task("Task");
    task.set_effort(5);
    assert_eq!(task.effort(), Some(5));

    task.clear_effort();
    assert_eq!(task.effort(), None);
}

#[test]
fn test_kanban_element_keywords_accessor() {
    let task = KanbanElement::new_task("Task");
    assert!(task.keywords().is_empty()); // Default is empty

    // Verify keywords returns a slice we can use
    let keywords = task.keywords();
    assert_eq!(keywords.len(), 0);
}

#[test]
fn test_kanban_element_created_at_accessor() {
    let before = Utc::now();
    let task = KanbanElement::new_task("Task");
    let after = Utc::now();

    let created = task.created_at();
    assert!(created >= &before && created <= &after);
}

#[test]
fn test_kanban_element_updated_at_accessor() {
    let before = Utc::now();
    let task = KanbanElement::new_task("Task");
    let after = Utc::now();

    let updated = task.updated_at();
    assert!(updated >= &before && updated <= &after);
}

#[test]
fn test_kanban_element_base_accessor() {
    let task = KanbanElement::new_task("Task");
    let base = task.base();
    assert_eq!(base.title, "Task");
    assert_eq!(base.status, Status::Plan);
    assert!(base.id.is_none()); // Not set yet
    assert!(base.parent.is_none());
    assert!(base.assignee.is_none());
    assert!(base.effort.is_none());
    assert!(base.keywords.is_empty());
    assert!(base.dependencies.is_empty());
}

#[test]
fn test_kanban_element_base_mut_accessor() {
    let mut task = KanbanElement::new_task("Original");
    task.base_mut().title = "Updated".to_string();
    task.base_mut().assignee = Some("agent-1".to_string());
    task.base_mut().priority = Priority::High;

    assert_eq!(task.title(), "Updated");
    assert_eq!(task.assignee(), Some(&"agent-1".to_string()));
    assert_eq!(task.priority(), Priority::High);
}

#[test]
fn test_tips_accessors() {
    let task_id = ElementId::new(ElementType::Task, 1);
    let tips = KanbanElement::new_tips("This is a helpful tip", task_id.clone(), "agent-1");

    // Tips stores the tip text in title (not content)
    assert_eq!(tips.title(), "This is a helpful tip");
    assert_eq!(tips.element_type(), ElementType::Tips);

    // Tips should have no content (it's stored in title)
    assert_eq!(tips.content(), "");

    // Keywords should be empty
    assert!(tips.keywords().is_empty());

    // Status should be Plan
    assert_eq!(tips.status(), Status::Plan);
}

#[test]
fn test_updated_at_changes_on_status_transition() {
    let mut task = KanbanElement::new_task("Task");
    let original_updated = *task.updated_at();

    // Wait a tiny bit to ensure time passes
    std::thread::sleep(std::time::Duration::from_millis(10));

    task.transition(Status::Backlog).unwrap();
    let new_updated = *task.updated_at();

    assert!(new_updated > original_updated);
}

#[test]
fn test_status_history_initial_state() {
    let task = KanbanElement::new_task("Task");

    // New element should have 1 history entry (the initial Plan status)
    let history = task.status_history();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].status, Status::Plan);
}

#[test]
fn test_status_history_records_transitions() {
    let mut task = KanbanElement::new_task("Task");

    // Transition through several statuses
    task.transition(Status::Backlog).unwrap();
    task.transition(Status::Ready).unwrap();
    task.transition(Status::Todo).unwrap();
    task.transition(Status::InProgress).unwrap();

    let history = task.status_history();
    assert_eq!(history.len(), 5); // Plan + 4 transitions

    assert_eq!(history[0].status, Status::Plan);
    assert_eq!(history[1].status, Status::Backlog);
    assert_eq!(history[2].status, Status::Ready);
    assert_eq!(history[3].status, Status::Todo);
    assert_eq!(history[4].status, Status::InProgress);
}

#[test]
fn test_status_history_invalid_transition_does_not_record() {
    let mut task = KanbanElement::new_task("Task");
    let original_history_len = task.status_history().len();

    // Try an invalid transition
    let result = task.transition(Status::Done);
    assert!(result.is_err());

    // History should not have changed
    assert_eq!(task.status_history().len(), original_history_len);
    assert_eq!(task.status(), Status::Plan);
}

#[test]
fn test_status_history_timestamps_are_set() {
    let mut task = KanbanElement::new_task("Task");

    // Initial history should have a valid timestamp
    let initial_entry = &task.status_history()[0];
    assert!(initial_entry.entered_at <= chrono::Utc::now());

    std::thread::sleep(std::time::Duration::from_millis(10));
    task.transition(Status::Backlog).unwrap();

    let history = task.status_history();
    assert!(history[1].entered_at > history[0].entered_at);
}

#[test]
fn test_add_tag() {
    let mut task = KanbanElement::new_task("Task");
    assert!(task.base().tags.is_empty());

    task.add_tag("bug");
    assert_eq!(task.base().tags, vec!["bug"]);

    task.add_tag("urgent");
    assert_eq!(task.base().tags, vec!["bug", "urgent"]);
}

#[test]
fn test_add_duplicate_tag_is_ignored() {
    let mut task = KanbanElement::new_task("Task");
    task.add_tag("bug");
    task.add_tag("bug"); // Duplicate

    // Should still only have one
    assert_eq!(task.base().tags.len(), 1);
}

#[test]
fn test_remove_tag() {
    let mut task = KanbanElement::new_task("Task");
    task.add_tag("bug");
    task.add_tag("urgent");

    task.remove_tag("bug");
    assert_eq!(task.base().tags, vec!["urgent"]);
}

#[test]
fn test_remove_tag_that_does_not_exist() {
    let mut task = KanbanElement::new_task("Task");
    task.add_tag("bug");

    task.remove_tag("nonexistent"); // Should not panic
    assert_eq!(task.base().tags, vec!["bug"]);
}

#[test]
fn test_add_reference() {
    let mut task = KanbanElement::new_task("Task");
    assert!(task.references().is_empty());

    let ref_id = ElementId::new(ElementType::Story, 1);
    task.add_reference(ref_id.clone());
    assert_eq!(task.references(), &[ref_id]);
}

#[test]
fn test_add_duplicate_reference_is_ignored() {
    let mut task = KanbanElement::new_task("Task");
    let ref_id = ElementId::new(ElementType::Story, 1);
    task.add_reference(ref_id.clone());
    task.add_reference(ref_id); // Duplicate

    assert_eq!(task.base().references.len(), 1);
}

#[test]
fn test_remove_reference() {
    let mut task = KanbanElement::new_task("Task");
    let ref_id = ElementId::new(ElementType::Story, 1);
    task.add_reference(ref_id.clone());

    task.remove_reference(&ref_id);
    assert!(task.references().is_empty());
}

#[test]
fn test_remove_reference_that_does_not_exist() {
    let mut task = KanbanElement::new_task("Task");
    let ref_id = ElementId::new(ElementType::Story, 1);
    task.add_reference(ref_id);

    task.remove_reference(&ElementId::new(ElementType::Task, 999)); // Does not exist
    assert_eq!(task.base().references.len(), 1);
}

#[test]
fn test_block_sets_reason_and_status() {
    let mut task = KanbanElement::new_task("Task");
    assert_eq!(task.status(), Status::Plan);
    assert!(task.base().blocked_reason.is_none());

    // Block requires being in Backlog first
    task.transition(Status::Backlog).unwrap();
    task.block("Waiting on external API").unwrap();

    assert_eq!(task.status(), Status::Blocked);
    assert_eq!(task.base().blocked_reason.as_ref().unwrap(), "Waiting on external API");
}

#[test]
fn test_block_from_invalid_status_fails() {
    let mut task = KanbanElement::new_task("Task");
    // Plan cannot go directly to Blocked
    let result = task.block("Some reason");
    assert!(result.is_err());
    assert_eq!(task.status(), Status::Plan);
}

#[test]
fn test_unblock_clears_reason_and_sets_backlog() {
    let mut task = KanbanElement::new_task("Task");

    // Setup: get to Blocked
    task.transition(Status::Backlog).unwrap();
    task.block("Waiting on external API").unwrap();
    assert_eq!(task.status(), Status::Blocked);
    assert!(task.base().blocked_reason.is_some());

    task.unblock().unwrap();

    assert_eq!(task.status(), Status::Backlog);
    assert!(task.base().blocked_reason.is_none());
}

#[test]
fn test_unblock_from_invalid_status_fails() {
    let mut task = KanbanElement::new_task("Task");
    task.transition(Status::Backlog).unwrap();

    // Can't unblock if not blocked
    let result = task.unblock();
    assert!(result.is_err());
    assert_eq!(task.status(), Status::Backlog);
}
