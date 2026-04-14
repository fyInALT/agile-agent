//! Roundtrip serialization tests for all element types
//!
//! Tests that elements can be serialized to JSON and deserialized back
//! without losing any data.

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Priority, Status};

#[test]
fn test_task_roundtrip_serialization() {
    let task = KanbanElement::new_task("Test Task");
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(task.title(), deserialized.title());
    assert_eq!(task.status(), deserialized.status());
    assert_eq!(task.element_type(), deserialized.element_type());
}

#[test]
fn test_story_roundtrip_serialization() {
    let story = KanbanElement::new_story("Test Story", "Story content here");
    let json = serde_json::to_string(&story).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(story.title(), deserialized.title());
    assert_eq!(story.element_type(), deserialized.element_type());
}

#[test]
fn test_sprint_roundtrip_serialization() {
    let sprint = KanbanElement::new_sprint("Sprint 1", "Goal description");
    let json = serde_json::to_string(&sprint).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(sprint.title(), deserialized.title());
    assert_eq!(sprint.element_type(), deserialized.element_type());
}

#[test]
fn test_sprint_with_dates_roundtrip_serialization() {
    let sprint = KanbanElement::new_sprint_with_dates(
        "Sprint 1",
        "Goal description",
        "2024-01-01",
        "2024-01-14",
    );
    let json = serde_json::to_string(&sprint).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(sprint.title(), deserialized.title());
    assert_eq!(sprint.element_type(), deserialized.element_type());
}

#[test]
fn test_idea_roundtrip_serialization() {
    let idea = KanbanElement::new_idea("Test Idea");
    let json = serde_json::to_string(&idea).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(idea.title(), deserialized.title());
    assert_eq!(idea.element_type(), deserialized.element_type());
}

#[test]
fn test_issue_roundtrip_serialization() {
    let issue = KanbanElement::new_issue("Test Issue");
    let json = serde_json::to_string(&issue).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(issue.title(), deserialized.title());
    assert_eq!(issue.element_type(), deserialized.element_type());
}

#[test]
fn test_tips_roundtrip_serialization() {
    let task_id = ElementId::new(ElementType::Task, 1);
    let tips = KanbanElement::new_tips("Tip content", task_id.clone(), "agent-1");
    let json = serde_json::to_string(&tips).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(tips.title(), deserialized.title());
    assert_eq!(tips.element_type(), deserialized.element_type());
}

#[test]
fn test_task_with_status_set() {
    let mut task = KanbanElement::new_task("Complex Task");
    task.set_status(Status::InProgress);

    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(task.title(), deserialized.title());
    assert_eq!(task.status(), deserialized.status());
    assert_eq!(task.element_type(), deserialized.element_type());
}

#[test]
fn test_element_id_roundtrip_serialization() {
    let id = ElementId::new(ElementType::Task, 42);
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: ElementId = serde_json::from_str(&json).unwrap();

    assert_eq!(id.as_str(), deserialized.as_str());
    assert_eq!(id.type_(), deserialized.type_());
    assert_eq!(id.number(), deserialized.number());
}

#[test]
fn test_status_roundtrip_serialization() {
    for status in [
        Status::Plan,
        Status::Backlog,
        Status::Ready,
        Status::Todo,
        Status::InProgress,
        Status::Done,
        Status::Verified,
        Status::Blocked,
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: Status = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }
}

#[test]
fn test_priority_roundtrip_serialization() {
    for priority in [
        Priority::Critical,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ] {
        let json = serde_json::to_string(&priority).unwrap();
        let deserialized: Priority = serde_json::from_str(&json).unwrap();
        assert_eq!(priority, deserialized);
    }
}
