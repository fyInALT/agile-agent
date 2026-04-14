//! E2E tests for KanbanService event integration
//!
//! These tests verify that the service actually publishes events
//! to the event bus with correct data.

mod test_helpers;
use test_helpers::*;

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Status};
use agent_kanban::events::{KanbanEvent, KanbanEventSubscriber};

#[test]
fn test_create_element_publishes_correct_created_event() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = KanbanElement::new_task("Test Task");
    let created = service.create_element(task).unwrap();
    let task_id = created.id().unwrap().clone();

    // Verify event was published
    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::Created {
            element_id,
            element_type,
        } => {
            assert_eq!(element_id.as_str(), task_id.as_str());
            assert_eq!(*element_type, ElementType::Task);
        }
        other => panic!("Expected Created event, got {:?}", other),
    }
}

#[test]
fn test_update_status_publishes_correct_status_changed_event() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = KanbanElement::new_task("Test Task");
    let created = service.create_element(task).unwrap();
    let task_id = created.id().unwrap().clone();

    collector.clear(); // Clear create event

    // Update status
    let _updated = service
        .update_status(&task_id, Status::Backlog, "agent-1")
        .unwrap();

    // Verify event was published
    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::StatusChanged {
            element_id,
            old_status,
            new_status,
            changed_by,
        } => {
            assert_eq!(element_id.as_str(), task_id.as_str());
            assert_eq!(*old_status, Status::Plan);
            assert_eq!(*new_status, Status::Backlog);
            assert_eq!(changed_by, "agent-1");
        }
        other => panic!("Expected StatusChanged event, got {:?}", other),
    }
}

#[test]
fn test_delete_publishes_deleted_event() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = KanbanElement::new_task("Test Task");
    let created = service.create_element(task).unwrap();
    let task_id = created.id().unwrap().clone();

    collector.clear();

    service.delete(&task_id).unwrap();

    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::Deleted { element_id } => {
            assert_eq!(element_id.as_str(), task_id.as_str());
        }
        other => panic!("Expected Deleted event, got {:?}", other),
    }
}

#[test]
fn test_add_dependency_publishes_dependency_added_event() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    // Create two tasks
    let task1 = service
        .create_element(KanbanElement::new_task("Task 1"))
        .unwrap();
    let task2 = service
        .create_element(KanbanElement::new_task("Task 2"))
        .unwrap();
    let id1 = task1.id().unwrap().clone();
    let id2 = task2.id().unwrap().clone();

    collector.clear();

    service.add_dependency(&id1, id2.clone()).unwrap();

    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::DependencyAdded {
            element_id,
            dependency,
        } => {
            assert_eq!(element_id.as_str(), id1.as_str());
            assert_eq!(dependency.as_str(), id2.as_str());
        }
        other => panic!("Expected DependencyAdded event, got {:?}", other),
    }
}

#[test]
fn test_remove_dependency_publishes_dependency_removed_event() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task1 = service
        .create_element(KanbanElement::new_task("Task 1"))
        .unwrap();
    let task2 = service
        .create_element(KanbanElement::new_task("Task 2"))
        .unwrap();
    let id1 = task1.id().unwrap().clone();
    let id2 = task2.id().unwrap().clone();

    service.add_dependency(&id1, id2.clone()).unwrap();
    collector.clear();

    service.remove_dependency(&id1, &id2).unwrap();

    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::DependencyRemoved {
            element_id,
            dependency,
        } => {
            assert_eq!(element_id.as_str(), id1.as_str());
            assert_eq!(dependency.as_str(), id2.as_str());
        }
        other => panic!("Expected DependencyRemoved event, got {:?}", other),
    }
}

#[test]
fn test_append_tip_publishes_tip_appended_event() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Task"))
        .unwrap();
    let task_id = task.id().unwrap().clone();

    collector.clear();

    let tips = service.append_tip(&task_id, "agent-1", "Helpful tip content").unwrap();

    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::TipAppended {
            task_id: t,
            tip_id,
            agent_id,
        } => {
            assert_eq!(t.as_str(), task_id.as_str());
            assert_eq!(tip_id.as_str(), tips.id().unwrap().as_str());
            assert_eq!(agent_id, "agent-1");
        }
        other => panic!("Expected TipAppended event, got {:?}", other),
    }
}

#[test]
fn test_multiple_subscribers_all_receive_events() {
    use agent_kanban::events::KanbanEventBus;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let event_bus = Arc::new(KanbanEventBus::new());
    let counter1 = Arc::new(AtomicUsize::new(0));
    let counter2 = Arc::new(AtomicUsize::new(0));

    struct MultiCounter(Arc<AtomicUsize>);
    impl KanbanEventSubscriber for MultiCounter {
        fn on_event(&self, _event: &KanbanEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    event_bus.subscribe(Box::new(MultiCounter(counter1.clone())));
    event_bus.subscribe(Box::new(MultiCounter(counter2.clone())));

    event_bus.publish(KanbanEvent::Created {
        element_id: ElementId::new(ElementType::Task, 1),
        element_type: ElementType::Task,
    });

    // Both subscribers should have received exactly 1 event each
    assert_eq!(counter1.load(Ordering::SeqCst), 1);
    assert_eq!(counter2.load(Ordering::SeqCst), 1);
}

#[test]
fn test_service_with_multiple_operations_publishes_all_events() {
    let (service, _repo, _event_bus, collector) = create_test_service();

    let task = service
        .create_element(KanbanElement::new_task("Task"))
        .unwrap();
    let id = task.id().unwrap().clone();

    service
        .update_status(&id, Status::Backlog, "agent-1")
        .unwrap();
    service
        .update_status(&id, Status::Ready, "agent-1")
        .unwrap();
    service.update_status(&id, Status::Todo, "agent-1").unwrap();
    service
        .update_status(&id, Status::InProgress, "agent-1")
        .unwrap();
    service.update_status(&id, Status::Done, "agent-1").unwrap();

    let events = collector.get_events();
    // create + 5 status updates = 6 events
    assert_eq!(events.len(), 6);

    // Verify first is Created
    match &events[0] {
        KanbanEvent::Created { .. } => {}
        other => panic!("Expected Created event first, got {:?}", other),
    }

    // Verify last is StatusChanged to Done
    match &events[5] {
        KanbanEvent::StatusChanged { new_status, .. } => {
            assert_eq!(*new_status, Status::Done);
        }
        other => panic!("Expected StatusChanged event last, got {:?}", other),
    }
}
