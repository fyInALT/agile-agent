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
    assert_eq!(task.content(), deserialized.content());
    assert_eq!(task.priority(), deserialized.priority());
    assert_eq!(task.assignee(), deserialized.assignee());
    assert_eq!(task.effort(), deserialized.effort());
}

#[test]
fn test_story_roundtrip_serialization() {
    let story = KanbanElement::new_story("Test Story", "Story content here");
    let json = serde_json::to_string(&story).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(story.title(), deserialized.title());
    assert_eq!(story.content(), deserialized.content());
    assert_eq!(story.element_type(), deserialized.element_type());
    assert_eq!(story.status(), deserialized.status());
    assert_eq!(story.priority(), deserialized.priority());
}

#[test]
fn test_sprint_roundtrip_serialization() {
    let sprint = KanbanElement::new_sprint("Sprint 1", "Goal description");
    let json = serde_json::to_string(&sprint).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(sprint.title(), deserialized.title());
    assert_eq!(sprint.element_type(), deserialized.element_type());
    assert_eq!(sprint.status(), deserialized.status());

    // Check via base() for sprint-specific fields
    match (&sprint, &deserialized) {
        (KanbanElement::Sprint(s1), KanbanElement::Sprint(s2)) => {
            assert_eq!(s1.goal, s2.goal);
            assert_eq!(s1.active, s2.active);
            assert_eq!(s1.start_date, s2.start_date);
            assert_eq!(s1.end_date, s2.end_date);
        }
        _ => panic!("Expected Sprint variant"),
    }
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

    match (&sprint, &deserialized) {
        (KanbanElement::Sprint(s1), KanbanElement::Sprint(s2)) => {
            assert_eq!(s1.goal, s2.goal);
            assert_eq!(s1.active, s2.active);
            assert_eq!(s1.start_date, Some("2024-01-01".to_string()));
            assert_eq!(s1.end_date, Some("2024-01-14".to_string()));
            assert_eq!(s1.start_date, s2.start_date);
            assert_eq!(s1.end_date, s2.end_date);
        }
        _ => panic!("Expected Sprint variant"),
    }
}

#[test]
fn test_idea_roundtrip_serialization() {
    let idea = KanbanElement::new_idea("Test Idea");
    let json = serde_json::to_string(&idea).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(idea.title(), deserialized.title());
    assert_eq!(idea.element_type(), deserialized.element_type());
    assert_eq!(idea.status(), deserialized.status());
    assert_eq!(idea.priority(), deserialized.priority());
}

#[test]
fn test_issue_roundtrip_serialization() {
    let issue = KanbanElement::new_issue("Test Issue");
    let json = serde_json::to_string(&issue).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(issue.title(), deserialized.title());
    assert_eq!(issue.element_type(), deserialized.element_type());
    assert_eq!(issue.status(), deserialized.status());
    // Issues have High priority by default
    assert_eq!(issue.priority(), deserialized.priority());
}

#[test]
fn test_tips_roundtrip_serialization() {
    let task_id = ElementId::new(ElementType::Task, 1);
    let tips = KanbanElement::new_tips("Tip content", task_id.clone(), "agent-1");
    let json = serde_json::to_string(&tips).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(tips.title(), deserialized.title());
    assert_eq!(tips.element_type(), deserialized.element_type());
    assert_eq!(tips.status(), deserialized.status());

    match (&tips, &deserialized) {
        (KanbanElement::Tips(t1), KanbanElement::Tips(t2)) => {
            assert_eq!(t1.target_task, t2.target_task);
            assert_eq!(t1.agent_id, t2.agent_id);
        }
        _ => panic!("Expected Tips variant"),
    }
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

#[test]
fn test_task_with_all_fields_roundtrip() {
    let mut task = KanbanElement::new_task("Full Task");
    task.base_mut().content = "Task description".to_string();
    task.base_mut().priority = Priority::High;
    task.base_mut().assignee = Some("agent-1".to_string());
    task.base_mut().effort = Some(8);
    task.base_mut().keywords = vec!["rust".to_string(), "test".to_string()];

    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(task.title(), deserialized.title());
    assert_eq!(task.content(), deserialized.content());
    assert_eq!(task.priority(), deserialized.priority());
    assert_eq!(task.assignee(), deserialized.assignee());
    assert_eq!(task.effort(), deserialized.effort());
    assert_eq!(task.keywords(), deserialized.keywords());
}

#[test]
fn test_sprint_active_field_serialization() {
    // Test that active field is properly serialized
    let sprint = KanbanElement::new_sprint("Sprint 1", "Goal");
    let json = serde_json::to_string(&sprint).unwrap();

    // Default sprint should have active = false
    assert!(!json.contains("\"active\":true") || json.contains("\"active\":false"));

    let sprint_active =
        KanbanElement::new_sprint_with_dates("Sprint 2", "Goal", "2024-01-01", "2024-01-14");
    // Sprint with dates should be active
    match sprint_active {
        KanbanElement::Sprint(s) => assert!(s.active),
        _ => panic!("Expected Sprint"),
    }
}

#[test]
fn test_status_history_roundtrip_serialization() {
    let mut task = KanbanElement::new_task("Task");

    // Transition through several statuses
    task.transition(Status::Backlog).unwrap();
    task.transition(Status::Ready).unwrap();
    task.transition(Status::Todo).unwrap();

    // Serialize and deserialize
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    // Verify status_history is preserved
    let history = deserialized.status_history();
    assert_eq!(history.len(), 4); // Plan + 3 transitions
    assert_eq!(history[0].status, Status::Plan);
    assert_eq!(history[1].status, Status::Backlog);
    assert_eq!(history[2].status, Status::Ready);
    assert_eq!(history[3].status, Status::Todo);
}

#[test]
fn test_blocked_reason_roundtrip_serialization() {
    let mut task = KanbanElement::new_task("Task");
    task.transition(Status::Backlog).unwrap();
    task.block("Waiting for deps").unwrap();

    // Serialize and deserialize
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    match deserialized {
        KanbanElement::Task(t) => {
            assert_eq!(t.base.blocked_reason.as_ref().unwrap(), "Waiting for deps");
        }
        _ => panic!("Expected Task"),
    }
}

#[test]
fn test_tags_roundtrip_serialization() {
    let mut task = KanbanElement::new_task("Task");
    task.add_tag("bug");
    task.add_tag("urgent");

    // Serialize and deserialize
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.base().tags, vec!["bug", "urgent"]);
}

#[test]
fn test_references_roundtrip_serialization() {
    let mut task = KanbanElement::new_task("Task");
    let ref1 = ElementId::new(ElementType::Story, 1);
    let ref2 = ElementId::new(ElementType::Task, 5);
    task.add_reference(ref1.clone());
    task.add_reference(ref2.clone());

    // Serialize and deserialize
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: KanbanElement = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.references().len(), 2);
}
