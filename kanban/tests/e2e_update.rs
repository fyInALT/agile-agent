//! Tests for update_element method

mod test_helpers;
use test_helpers::*;

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Priority};
use agent_kanban::events::KanbanEvent;

#[test]
fn test_update_element_title() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Original"))
        .unwrap();
    let id = task.id().unwrap().clone();

    collector.clear();

    let updated = service
        .update_element(&id, Some("New Title"), None, None, None)
        .unwrap();

    assert_eq!(updated.title(), "New Title");
    assert_eq!(collector.get_events().len(), 1);

    match &collector.get_events()[0] {
        KanbanEvent::Updated {
            element_id,
            changes,
        } => {
            assert_eq!(element_id.as_str(), id.as_str());
            assert!(changes.contains(&"title".to_string()));
        }
        _ => panic!("Expected Updated event"),
    }
}

#[test]
fn test_update_element_content() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let story = service
        .create_element(KanbanElement::new_story("Story", "Original content"))
        .unwrap();
    let id = story.id().unwrap().clone();

    collector.clear();

    let updated = service
        .update_element(&id, None, Some("New content"), None, None)
        .unwrap();

    assert_eq!(updated.content(), "New content");
}

#[test]
fn test_update_element_priority() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Task"))
        .unwrap();
    let id = task.id().unwrap().clone();

    collector.clear();

    let updated = service
        .update_element(&id, None, None, Some(Priority::High), None)
        .unwrap();

    assert_eq!(updated.priority(), Priority::High);
}

#[test]
fn test_update_element_assignee() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Task"))
        .unwrap();
    let id = task.id().unwrap().clone();

    collector.clear();

    let updated = service
        .update_element(&id, None, None, None, Some("agent-1"))
        .unwrap();

    assert_eq!(updated.assignee(), Some(&"agent-1".to_string()));
}

#[test]
fn test_update_element_multiple_fields() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Original"))
        .unwrap();
    let id = task.id().unwrap().clone();

    collector.clear();

    let updated = service
        .update_element(
            &id,
            Some("New Title"),
            None,
            Some(Priority::Critical),
            Some("agent-2"),
        )
        .unwrap();

    assert_eq!(updated.title(), "New Title");
    assert_eq!(updated.priority(), Priority::Critical);
    assert_eq!(updated.assignee(), Some(&"agent-2".to_string()));

    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::Updated { changes, .. } => {
            assert_eq!(changes.len(), 3);
            assert!(changes.contains(&"title".to_string()));
            assert!(changes.contains(&"priority".to_string()));
            assert!(changes.contains(&"assignee".to_string()));
        }
        _ => panic!("Expected Updated event"),
    }
}

#[test]
fn test_update_element_no_changes() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Task"))
        .unwrap();
    let id = task.id().unwrap().clone();

    collector.clear();

    // Call with no changes
    let updated = service.update_element(&id, None, None, None, None).unwrap();

    assert_eq!(updated.title(), "Task");
    // No events should be published
    assert!(collector.get_events().is_empty());
}

#[test]
fn test_update_element_not_found() {
    let (service, _repo, _event_bus, _collector) = create_test_service();

    let id = ElementId::new(ElementType::Task, 999);

    let result = service.update_element(&id, Some("Title"), None, None, None);

    assert!(result.is_err());
}
