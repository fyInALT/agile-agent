//! Full integration tests for the kanban system
//!
//! These tests use real FileKanbanRepository and real file storage
//! to verify the entire system works end-to-end.

use agent_kanban::FileKanbanRepository;
use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Priority, Status};
use agent_kanban::events::{KanbanEvent, KanbanEventBus, KanbanEventSubscriber};
use agent_kanban::repository::KanbanElementRepository;
use agent_kanban::service::KanbanService;
use std::sync::{Arc, RwLock};

/// Full integration test repository that wraps FileKanbanRepository
/// but also tracks events for verification
struct IntegrationRepository {
    repo: FileKanbanRepository,
    events: RwLock<Vec<KanbanEvent>>,
}

impl IntegrationRepository {
    fn new(path: &std::path::Path) -> Result<Self, agent_kanban::KanbanError> {
        Ok(IntegrationRepository {
            repo: FileKanbanRepository::new(path)?,
            events: RwLock::new(Vec::new()),
        })
    }

    fn record_event(&self, event: KanbanEvent) {
        self.events.write().unwrap().push(event);
    }

    fn get_events(&self) -> Vec<KanbanEvent> {
        self.events.read().unwrap().clone()
    }

    fn clear_events(&self) {
        self.events.write().unwrap().clear();
    }
}

impl KanbanElementRepository for IntegrationRepository {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.get(id)
    }

    fn list(&self) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list()
    }

    fn list_by_type(
        &self,
        type_: ElementType,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list_by_type(type_)
    }

    fn list_by_status(
        &self,
        status: Status,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list_by_status(status)
    }

    fn list_by_assignee(
        &self,
        assignee: &str,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list_by_assignee(assignee)
    }

    fn list_by_parent(
        &self,
        parent: &ElementId,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list_by_parent(parent)
    }

    fn list_by_sprint(
        &self,
        sprint_id: &ElementId,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list_by_sprint(sprint_id)
    }

    fn list_blocked(&self) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.repo.list_blocked()
    }

    fn save(&self, element: KanbanElement) -> Result<(), agent_kanban::KanbanError> {
        self.repo.save(element)
    }

    fn delete(&self, id: &ElementId) -> Result<(), agent_kanban::KanbanError> {
        self.repo.delete(id)
    }

    fn new_id(&self, type_: ElementType) -> Result<ElementId, agent_kanban::KanbanError> {
        self.repo.new_id(type_)
    }
}

/// Event collector for integration tests - uses Arc for shared state
struct EventCollector {
    events: Arc<RwLock<Vec<KanbanEvent>>>,
}

impl Clone for EventCollector {
    fn clone(&self) -> Self {
        EventCollector {
            events: self.events.clone(),
        }
    }
}

impl EventCollector {
    fn new() -> Self {
        EventCollector {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<KanbanEvent> {
        self.events.read().unwrap().clone()
    }

    fn clear(&self) {
        self.events.write().unwrap().clear();
    }
}

impl KanbanEventSubscriber for EventCollector {
    fn on_event(&self, event: &KanbanEvent) {
        self.events.write().unwrap().push(event.clone());
    }
}

fn create_integration_service(
    temp_dir: &tempfile::TempDir,
) -> Result<
    (
        KanbanService<IntegrationRepository>,
        Arc<IntegrationRepository>,
        Arc<KanbanEventBus>,
        EventCollector,
    ),
    agent_kanban::KanbanError,
> {
    let path = temp_dir.path().join("kanban");
    let repo = Arc::new(IntegrationRepository::new(&path)?);
    let event_bus = Arc::new(KanbanEventBus::new());
    let collector = EventCollector::new();
    event_bus.subscribe(Box::new(collector.clone()));
    let service = KanbanService::new(repo.clone(), event_bus.clone());
    Ok((service, repo, event_bus, collector))
}

// ============================================================================
// Full System Integration Tests
// ============================================================================

#[test]
fn test_full_kanban_workflow() {
    let temp = tempfile::TempDir::new().unwrap();
    let (service, repo, _event_bus, collector) = create_integration_service(&temp).unwrap();

    // 1. Create a Sprint
    let sprint = service
        .create_element(KanbanElement::new_sprint("Sprint 1", "Deliver v1.0"))
        .unwrap();
    let sprint_id = sprint.id().unwrap().clone();
    assert_eq!(sprint.title(), "Sprint 1");

    // 2. Create Stories under the Sprint
    let story1 = {
        let mut s = KanbanElement::new_story("User Login", "As a user I can login");
        s.base_mut().parent = Some(sprint_id.clone());
        service.create_element(s).unwrap()
    };
    let story1_id = story1.id().unwrap().clone();

    let story2 = {
        let mut s = KanbanElement::new_story("User Logout", "As a user I can logout");
        s.base_mut().parent = Some(sprint_id.clone());
        service.create_element(s).unwrap()
    };
    let _story2_id = story2.id().unwrap().clone();

    // 3. Create Tasks under the Stories
    let task1 = {
        let mut t = KanbanElement::new_task("Implement login form");
        t.base_mut().parent = Some(story1_id.clone());
        service.create_element(t).unwrap()
    };
    let task1_id = task1.id().unwrap().clone();

    // 4. Verify Sprint contains all children
    let sprint_children = service.list_by_sprint(&sprint_id).unwrap();
    assert_eq!(sprint_children.len(), 2); // 2 stories directly under sprint (task is under story)

    // 5. Verify Story contains its Tasks (using repository directly)
    let story1_children = repo.list_by_parent(&story1_id).unwrap();
    assert_eq!(story1_children.len(), 1);
    assert_eq!(story1_children[0].title(), "Implement login form");

    // 6. Update Task through status transitions
    service
        .update_status(&task1_id, Status::Backlog, "agent-1")
        .unwrap();
    service
        .update_status(&task1_id, Status::Ready, "agent-1")
        .unwrap();
    service
        .update_status(&task1_id, Status::Todo, "agent-1")
        .unwrap();

    let updated_task = service.update_status(&task1_id, Status::InProgress, "agent-1");
    assert!(updated_task.is_ok());

    // 7. Complete the task
    service
        .update_status(&task1_id, Status::Done, "agent-1")
        .unwrap();

    // 8. Verify events were published (1 sprint + 2 stories + 1 task + 5 status changes = 9)
    let events = collector.get_events();
    assert_eq!(events.len(), 9);

    // 9. Files should exist on disk
    let elements_dir = temp.path().join("kanban").join("elements");
    assert!(
        elements_dir
            .join(format!("{}.json", sprint_id.as_str()))
            .exists()
    );
    assert!(
        elements_dir
            .join(format!("{}.json", task1_id.as_str()))
            .exists()
    );
}

#[test]
fn test_full_dependency_chain() {
    let temp = tempfile::TempDir::new().unwrap();
    let (service, _repo, _event_bus, _collector) = create_integration_service(&temp).unwrap();

    // Create a chain: task1 -> task2 -> task3
    let task1 = service
        .create_element(KanbanElement::new_task("Design"))
        .unwrap();
    let task1_id = task1.id().unwrap().clone();

    let task2 = {
        let mut t = KanbanElement::new_task("Implementation");
        t.base_mut().parent = Some(task1_id.clone());
        service.create_element(t).unwrap()
    };
    let task2_id = task2.id().unwrap().clone();

    let task3 = {
        let mut t = KanbanElement::new_task("Testing");
        t.base_mut().parent = Some(task2_id.clone());
        service.create_element(t).unwrap()
    };
    let task3_id = task3.id().unwrap().clone();

    // Add dependencies: task2 depends on task1, task3 depends on task2
    service.add_dependency(&task2_id, task1_id.clone()).unwrap();
    service
        .add_dependency(&task3_id.clone(), task2_id.clone())
        .unwrap();

    // Try to move task3 to InProgress - should fail because task2 is not done
    let result = service.update_status(&task3_id, Status::InProgress, "agent-1");
    assert!(result.is_err());

    // Complete task1
    service
        .update_status(&task1_id, Status::Backlog, "agent-1")
        .unwrap();
    service
        .update_status(&task1_id, Status::Ready, "agent-1")
        .unwrap();
    service
        .update_status(&task1_id, Status::Todo, "agent-1")
        .unwrap();
    service
        .update_status(&task1_id, Status::InProgress, "agent-1")
        .unwrap();
    service
        .update_status(&task1_id, Status::Done, "agent-1")
        .unwrap();

    // task2 can now move to InProgress
    service
        .update_status(&task2_id.clone(), Status::Backlog, "agent-1")
        .unwrap();
    service
        .update_status(&task2_id.clone(), Status::Ready, "agent-1")
        .unwrap();
    service
        .update_status(&task2_id.clone(), Status::Todo, "agent-1")
        .unwrap();
    let result = service.update_status(&task2_id, Status::InProgress, "agent-1");
    assert!(result.is_ok());

    // But task3 still cannot move because task2 is not done
    let result = service.update_status(&task3_id, Status::InProgress, "agent-1");
    assert!(result.is_err());
}

#[test]
fn test_full_file_persistence() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("kanban");

    // Create and save elements
    let (service, _repo, _event_bus, _collector) = create_integration_service(&temp).unwrap();

    let task = service
        .create_element(KanbanElement::new_task("Persistent Task"))
        .unwrap();
    let task_id = task.id().unwrap().clone();

    // Drop the service
    drop(service);
    drop(_repo);

    // Create a new service pointing to the same path
    let (service2, _repo2, _event_bus2, _collector2) = create_integration_service(&temp).unwrap();

    // Verify the element still exists
    let retrieved = service2.get_element(&task_id).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().title(), "Persistent Task");

    // Verify ID counter continued
    let new_task = service2
        .create_element(KanbanElement::new_task("Another Task"))
        .unwrap();
    assert_eq!(new_task.id().unwrap().as_str(), "task-002");

    // Verify element files exist
    let task_file = path
        .join("elements")
        .join(format!("{}.json", task_id.as_str()));
    assert!(task_file.exists());

    // Verify index.json exists
    let index_file = path.join("index.json");
    assert!(index_file.exists());
}

#[test]
fn test_full_append_tip_workflow() {
    let temp = tempfile::TempDir::new().unwrap();
    let (service, _repo, _event_bus, collector) = create_integration_service(&temp).unwrap();

    let task = service
        .create_element(KanbanElement::new_task("Complex Task"))
        .unwrap();
    let task_id = task.id().unwrap().clone();

    collector.clear();

    // Append multiple tips
    let tip1 = service.append_tip(&task_id, "agent-1", "First tip content").unwrap();
    let tip1_id = tip1.id().unwrap().clone();

    let tip2 = service.append_tip(&task_id, "agent-2", "Second tip content").unwrap();
    let tip2_id = tip2.id().unwrap().clone();

    // Verify tip was created correctly
    assert_eq!(tip1.element_type(), ElementType::Tips);
    assert_eq!(tip1.title(), "First tip content");

    // Verify tips can be retrieved by type
    let all_tips = service.list_by_type(ElementType::Tips).unwrap();
    assert_eq!(all_tips.len(), 2);

    // Verify each tip has correct agent_id and target_task via base()
    for tip in &all_tips {
        match tip {
            KanbanElement::Tips(t) => {
                assert_eq!(t.target_task, task_id);
                assert!(t.agent_id == "agent-1" || t.agent_id == "agent-2");
            }
            _ => panic!("Expected Tips variant"),
        }
    }

    // Verify specific tip can be retrieved
    let retrieved_tip1 = service.get_element(&tip1_id).unwrap();
    assert!(retrieved_tip1.is_some());
    assert_eq!(retrieved_tip1.unwrap().title(), "First tip content");

    let retrieved_tip2 = service.get_element(&tip2_id).unwrap();
    assert!(retrieved_tip2.is_some());
    assert_eq!(retrieved_tip2.unwrap().title(), "Second tip content");

    // Verify events were published
    let events = collector.get_events();
    assert_eq!(events.len(), 2);

    match &events[0] {
        KanbanEvent::TipAppended {
            task_id: t,
            tip_id,
            agent_id,
        } => {
            assert_eq!(t.as_str(), task_id.as_str());
            assert_eq!(tip_id.as_str(), tip1_id.as_str());
            assert_eq!(agent_id, "agent-1");
        }
        _ => panic!("Expected TipAppended event"),
    }
}

#[test]
fn test_full_update_and_query() {
    let temp = tempfile::TempDir::new().unwrap();
    let (service, _repo, _event_bus, collector) = create_integration_service(&temp).unwrap();

    // Create tasks with different properties
    let task1 = service
        .create_element(KanbanElement::new_task("Bug Fix"))
        .unwrap();
    let task1_id = task1.id().unwrap().clone();

    let task2 = service
        .create_element(KanbanElement::new_task("Feature"))
        .unwrap();
    let _task2_id = task2.id().unwrap().clone();

    collector.clear();

    // Update task1
    service
        .update_element(
            &task1_id,
            Some("Critical Bug Fix"),
            Some("Fix the login bug"),
            Some(Priority::Critical),
            Some("agent-1"),
            None,
        )
        .unwrap();

    // Verify events
    let events = collector.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        KanbanEvent::Updated { changes, .. } => {
            assert!(changes.contains(&"title".to_string()));
            assert!(changes.contains(&"content".to_string()));
            assert!(changes.contains(&"priority".to_string()));
            assert!(changes.contains(&"assignee".to_string()));
        }
        _ => panic!("Expected Updated event"),
    }

    // Query by assignee
    let assigned_to_agent1 = service.list_by_assignee("agent-1").unwrap();
    assert_eq!(assigned_to_agent1.len(), 1);
    assert_eq!(assigned_to_agent1[0].title(), "Critical Bug Fix");

    // Query by status
    let plan_tasks = service.list_by_status(Status::Plan).unwrap();
    // task1 updated to InProgress (via transition), task2 still in Plan
    // Actually we didn't transition task1, so both should be in Plan initially
    assert_eq!(plan_tasks.len(), 2);

    // Query by type
    let tasks = service.list_by_type(ElementType::Task).unwrap();
    assert_eq!(tasks.len(), 2);
}

#[test]
fn test_full_delete_removes_from_disk() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("kanban");
    let (service, _repo, _event_bus, _collector) = create_integration_service(&temp).unwrap();

    let task = service
        .create_element(KanbanElement::new_task("To Delete"))
        .unwrap();
    let task_id = task.id().unwrap().clone();

    // Verify file exists
    let task_file = path
        .join("elements")
        .join(format!("{}.json", task_id.as_str()));
    assert!(task_file.exists());

    // Delete
    service.delete(&task_id).unwrap();

    // Verify file is gone
    assert!(!task_file.exists());

    // Verify element is gone from repository
    assert!(service.get_element(&task_id).unwrap().is_none());
}

#[test]
fn test_full_concurrent_event_publishing() {
    use std::sync::Arc;

    let temp = tempfile::TempDir::new().unwrap();
    let (service, _repo, event_bus, _collector) = create_integration_service(&temp).unwrap();

    // Create multiple subscribers
    let counter1 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    struct AtomicCounter(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl KanbanEventSubscriber for AtomicCounter {
        fn on_event(&self, _event: &KanbanEvent) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    event_bus.subscribe(Box::new(AtomicCounter(counter1.clone())));
    event_bus.subscribe(Box::new(AtomicCounter(counter2.clone())));

    // Create multiple elements
    for i in 0..10 {
        let task = service
            .create_element(KanbanElement::new_task(&format!("Task {}", i)))
            .unwrap();
        service
            .update_status(&task.id().unwrap(), Status::Backlog, "agent-1")
            .unwrap();
    }

    // Both counters should have received all events
    // 10 creates + 10 status updates = 20 events each
    assert_eq!(counter1.load(std::sync::atomic::Ordering::SeqCst), 20);
    assert_eq!(counter2.load(std::sync::atomic::Ordering::SeqCst), 20);
}
