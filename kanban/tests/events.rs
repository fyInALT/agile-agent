//! Unit tests for KanbanEvent types

use agent_kanban::domain::{ElementId, ElementType, Status};
use agent_kanban::events::{KanbanEvent, KanbanEventBus, KanbanEventSubscriber};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn test_created_event() {
    let event = KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Task, 1),
        element_type: ElementType::Task,
    };
    assert_eq!(format!("{}", event), "Created(task-001)");
}

#[test]
fn test_updated_event() {
    let event = KanbanEvent::Updated {
        element_id: ElementId::new(ElementType::Story, 5),
        changes: vec!["title".to_string(), "description".to_string()],
    };
    assert_eq!(format!("{}", event), "Updated(story-005)");
}

#[test]
fn test_status_changed_event() {
    let event = KanbanEvent::StatusChanged {
        element_id: ElementId::new(ElementType::Task, 1),
        old_status: Status::Plan,
        new_status: Status::Backlog,
        changed_by: "agent-1".to_string(),
    };
    assert_eq!(
        format!("{}", event),
        "StatusChanged(task-001: Plan -> Backlog)"
    );
}

#[test]
fn test_deleted_event() {
    let event = KanbanEvent::Deleted {
        element_id: ElementId::new(ElementType::Task, 3),
    };
    assert_eq!(format!("{}", event), "Deleted(task-003)");
}

#[test]
fn test_tip_appended_event() {
    let event = KanbanEvent::TipAppended {
        task_id: ElementId::new(ElementType::Task, 1),
        tip_id: ElementId::new(ElementType::Tips, 2),
        agent_id: "agent-1".to_string(),
    };
    assert_eq!(format!("{}", event), "TipAppended(task=task-001)");
}

#[test]
fn test_dependency_added_event() {
    let event = KanbanEvent::DependencyAdded {
        element_id: ElementId::new(ElementType::Task, 1),
        dependency: ElementId::new(ElementType::Story, 2),
    };
    assert_eq!(
        format!("{}", event),
        "DependencyAdded(task-001 -> story-002)"
    );
}

#[test]
fn test_dependency_removed_event() {
    let event = KanbanEvent::DependencyRemoved {
        element_id: ElementId::new(ElementType::Task, 1),
        dependency: ElementId::new(ElementType::Story, 2),
    };
    assert_eq!(
        format!("{}", event),
        "DependencyRemoved(task-001 -> story-002)"
    );
}

#[test]
fn test_event_clone() {
    let event = KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Task, 1),
        element_type: ElementType::Task,
    };
    let cloned = event.clone();
    assert_eq!(format!("{}", event), format!("{}", cloned));
}

#[test]
fn test_event_debug() {
    let event = KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Task, 1),
        element_type: ElementType::Task,
    };
    let debug = format!("{:?}", event);
    assert!(debug.contains("Created"));
    assert!(debug.contains("task-001"));
}

#[test]
fn test_subscriber_receives_single_event() {
    let bus = KanbanEventBus::new();
    let counter = Arc::new(AtomicUsize::new(0));

    struct SingleCounter(Arc<AtomicUsize>);
    impl KanbanEventSubscriber for SingleCounter {
        fn on_event(&self, _event: &KanbanEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    bus.subscribe(Box::new(SingleCounter(counter.clone())));

    let event = KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Task, 1),
        element_type: ElementType::Task,
    };
    bus.publish(event);

    // Verify the subscriber actually received the event
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_multiple_subscribers_all_receive_events() {
    let bus = KanbanEventBus::new();
    let counter1 = Arc::new(AtomicUsize::new(0));
    let counter2 = Arc::new(AtomicUsize::new(0));

    struct MultiCounter(Arc<AtomicUsize>);
    impl KanbanEventSubscriber for MultiCounter {
        fn on_event(&self, _event: &KanbanEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let sub1 = Box::new(MultiCounter(counter1.clone()));
    let sub2 = Box::new(MultiCounter(counter2.clone()));
    bus.subscribe(sub1);
    bus.subscribe(sub2);

    let event = KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Task, 1),
        element_type: ElementType::Task,
    };
    bus.publish(event);

    // Both subscribers should have received the event
    assert_eq!(counter1.load(Ordering::SeqCst), 1);
    assert_eq!(counter2.load(Ordering::SeqCst), 1);
}

#[test]
fn test_event_bus_debug() {
    let bus = KanbanEventBus::new();
    let debug = format!("{:?}", bus);
    assert!(debug.contains("KanbanEventBus"));
    assert!(debug.contains("subscriber_count"));
}
