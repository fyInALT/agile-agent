//! Unit tests for KanbanService

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Status};
use agent_kanban::events::KanbanEventBus;
use agent_kanban::repository::KanbanElementRepository;
use agent_kanban::service::KanbanService;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

struct TestRepository {
    elements: RwLock<Vec<KanbanElement>>,
    counters: RwLock<HashMap<ElementType, u32>>,
}

impl TestRepository {
    fn new() -> Self {
        TestRepository {
            elements: RwLock::new(Vec::new()),
            counters: RwLock::new(HashMap::new()),
        }
    }
}

impl KanbanElementRepository for TestRepository {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements.iter().find(|e| e.id() == Some(id)).cloned())
    }

    fn list(&self) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements.clone())
    }

    fn list_by_type(
        &self,
        type_: ElementType,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements
            .iter()
            .filter(|e| e.element_type() == type_)
            .cloned()
            .collect())
    }

    fn list_by_status(
        &self,
        status: Status,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements
            .iter()
            .filter(|e| e.status() == status)
            .cloned()
            .collect())
    }

    fn list_by_assignee(
        &self,
        assignee: &str,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements
            .iter()
            .filter(|e| {
                e.assignee()
                    .map(|a| a.as_str() == assignee)
                    .unwrap_or(false)
            })
            .cloned()
            .collect())
    }

    fn list_by_parent(
        &self,
        parent: &ElementId,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements
            .iter()
            .filter(|e| e.parent().map(|p| p == parent).unwrap_or(false))
            .cloned()
            .collect())
    }

    fn list_blocked(&self) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        self.list_by_status(Status::Blocked)
    }

    fn list_by_sprint(
        &self,
        sprint_id: &ElementId,
    ) -> Result<Vec<KanbanElement>, agent_kanban::KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements
            .iter()
            .filter(|e| e.parent().map(|p| p == sprint_id).unwrap_or(false))
            .cloned()
            .collect())
    }

    fn save(&self, element: KanbanElement) -> Result<(), agent_kanban::KanbanError> {
        let mut elements = self.elements.write().unwrap();
        if let Some(pos) = elements.iter().position(|e| e.id() == element.id()) {
            elements[pos] = element;
        } else {
            elements.push(element);
        }
        Ok(())
    }

    fn delete(&self, id: &ElementId) -> Result<(), agent_kanban::KanbanError> {
        let mut elements = self.elements.write().unwrap();
        elements.retain(|e| e.id() != Some(id));
        Ok(())
    }

    fn new_id(&self, type_: ElementType) -> Result<ElementId, agent_kanban::KanbanError> {
        let mut counters = self.counters.write().unwrap();
        let next = counters.get(&type_).copied().unwrap_or(0) + 1;
        counters.insert(type_, next);
        Ok(ElementId::new(type_, next))
    }
}

fn create_service() -> (KanbanService<TestRepository>, Arc<TestRepository>) {
    let repo = Arc::new(TestRepository::new());
    let event_bus = Arc::new(KanbanEventBus::new());
    let service = KanbanService::new(repo.clone(), event_bus);
    (service, repo)
}

mod create_tests {
    use super::*;

    #[test]
    fn test_create_element_assigns_sequential_id() {
        let (service, _repo) = create_service();

        let task1 = service
            .create_element(KanbanElement::new_task("Task 1"))
            .unwrap();
        let task2 = service
            .create_element(KanbanElement::new_task("Task 2"))
            .unwrap();

        assert_eq!(task1.id().unwrap().as_str(), "task-001");
        assert_eq!(task2.id().unwrap().as_str(), "task-002");
    }

    #[test]
    fn test_create_element_sets_status_to_plan() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        assert_eq!(task.status(), Status::Plan);
    }

    #[test]
    fn test_get_element_returns_saved_element() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Test Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        let retrieved = service.get_element(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title(), "Test Task");
    }

    #[test]
    fn test_get_element_not_found() {
        let (service, _repo) = create_service();

        let id = ElementId::new(ElementType::Task, 999);
        let result = service.get_element(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_elements_returns_all() {
        let (service, _repo) = create_service();

        service
            .create_element(KanbanElement::new_task("Task 1"))
            .unwrap();
        service
            .create_element(KanbanElement::new_task("Task 2"))
            .unwrap();
        service
            .create_element(KanbanElement::new_story("Story 1", "Content"))
            .unwrap();

        let all = service.list_elements().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_list_by_type_filters() {
        let (service, _repo) = create_service();

        service
            .create_element(KanbanElement::new_task("Task 1"))
            .unwrap();
        service
            .create_element(KanbanElement::new_task("Task 2"))
            .unwrap();
        service
            .create_element(KanbanElement::new_story("Story 1", "Content"))
            .unwrap();

        let tasks = service.list_by_type(ElementType::Task).unwrap();
        assert_eq!(tasks.len(), 2);

        let stories = service.list_by_type(ElementType::Story).unwrap();
        assert_eq!(stories.len(), 1);
    }

    #[test]
    fn test_list_children_returns_direct_children() {
        let (service, _repo) = create_service();

        // Create a story with tasks
        let story = service
            .create_element(KanbanElement::new_story("Story", "Content"))
            .unwrap();
        let story_id = story.id().unwrap().clone();

        // Create tasks under the story
        let task1 = {
            let mut t = KanbanElement::new_task("Task 1");
            t.base_mut().parent = Some(story_id.clone());
            service.create_element(t).unwrap()
        };
        let task2 = {
            let mut t = KanbanElement::new_task("Task 2");
            t.base_mut().parent = Some(story_id.clone());
            service.create_element(t).unwrap()
        };

        // Get children of the story
        let children = service.list_children(&story_id).unwrap();
        assert_eq!(children.len(), 2);

        // Verify we got the right tasks
        let child_ids: Vec<_> = children.iter().map(|e| e.id().unwrap().as_str()).collect();
        assert!(child_ids.contains(&task1.id().unwrap().as_str()));
        assert!(child_ids.contains(&task2.id().unwrap().as_str()));
    }

    #[test]
    fn test_list_children_returns_empty_when_no_children() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let task_id = task.id().unwrap().clone();

        let children = service.list_children(&task_id).unwrap();
        assert!(children.is_empty());
    }
}

mod status_transition_tests {
    use super::*;

    #[test]
    fn test_valid_transition_plan_to_backlog() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        let updated = service
            .update_status(&id, Status::Backlog, "agent-1")
            .unwrap();
        assert_eq!(updated.status(), Status::Backlog);
    }

    #[test]
    fn test_valid_transition_backlog_to_ready() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        service
            .update_status(&id, Status::Backlog, "agent-1")
            .unwrap();
        let updated = service
            .update_status(&id, Status::Ready, "agent-1")
            .unwrap();
        assert_eq!(updated.status(), Status::Ready);
    }

    #[test]
    fn test_valid_transition_ready_to_todo() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        service
            .update_status(&id, Status::Backlog, "agent-1")
            .unwrap();
        let updated = service
            .update_status(&id, Status::Ready, "agent-1")
            .unwrap();
        assert_eq!(updated.status(), Status::Ready);

        let updated = service.update_status(&id, Status::Todo, "agent-1").unwrap();
        assert_eq!(updated.status(), Status::Todo);
    }

    #[test]
    fn test_valid_transition_todo_to_in_progress() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        // Full path: Plan -> Backlog -> Ready -> Todo -> InProgress
        service
            .update_status(&id, Status::Backlog, "agent-1")
            .unwrap();
        service
            .update_status(&id, Status::Ready, "agent-1")
            .unwrap();
        service.update_status(&id, Status::Todo, "agent-1").unwrap();
        let updated = service
            .update_status(&id, Status::InProgress, "agent-1")
            .unwrap();
        assert_eq!(updated.status(), Status::InProgress);
    }

    #[test]
    fn test_invalid_transition_plan_to_done() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        let result = service.update_status(&id, Status::Done, "agent-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_transition_plan_to_verified() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        let result = service.update_status(&id, Status::Verified, "agent-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_transition_from_verified() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        // Navigate to Verified: Plan -> Backlog -> Ready -> Todo -> InProgress -> Done -> Verified
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
        service
            .update_status(&id, Status::Verified, "agent-1")
            .unwrap();

        // Try to go back to Backlog - should fail
        let result = service.update_status(&id, Status::Backlog, "agent-1");
        assert!(result.is_err());
    }
}

mod dependency_tests {
    use super::*;

    #[test]
    fn test_find_blocking_dependencies_none() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        let blockers = service.find_blocking_dependencies(&id).unwrap();
        assert!(blockers.is_empty());
    }

    #[test]
    fn test_can_start_no_dependencies() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let id = task.id().unwrap().clone();

        assert!(service.can_start(&id).unwrap());
    }

    #[test]
    fn test_dependency_blocks_in_progress() {
        let (service, repo) = create_service();

        // Create a task that will be a dependency
        let dep_task = service
            .create_element(KanbanElement::new_task("Dependency"))
            .unwrap();
        let dep_id = dep_task.id().unwrap().clone();

        // Create main task with dependency
        let mut main_task = KanbanElement::new_task("Main Task");
        main_task.base_mut().dependencies.push(dep_id.clone());
        let main_task = service.create_element(main_task).unwrap();
        let main_id = main_task.id().unwrap().clone();

        // Try to move to InProgress - should fail because dependency is not Done
        let result = service.update_status(&main_id, Status::InProgress, "agent-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_satisfied_allows_in_progress() {
        let (service, repo) = create_service();

        // Create a task that will be a dependency
        let dep_task = service
            .create_element(KanbanElement::new_task("Dependency"))
            .unwrap();
        let dep_id = dep_task.id().unwrap().clone();

        // Complete the dependency: Plan -> Backlog -> Ready -> Todo -> InProgress -> Done
        service
            .update_status(&dep_id, Status::Backlog, "agent-1")
            .unwrap();
        service
            .update_status(&dep_id, Status::Ready, "agent-1")
            .unwrap();
        service
            .update_status(&dep_id, Status::Todo, "agent-1")
            .unwrap();
        service
            .update_status(&dep_id, Status::InProgress, "agent-1")
            .unwrap();
        service
            .update_status(&dep_id, Status::Done, "agent-1")
            .unwrap();

        // Create main task with dependency
        let mut main_task = KanbanElement::new_task("Main Task");
        main_task.base_mut().dependencies.push(dep_id);
        let main_task = service.create_element(main_task).unwrap();
        let main_id = main_task.id().unwrap().clone();

        // Navigate main task to Ready -> Todo
        service
            .update_status(&main_id, Status::Backlog, "agent-1")
            .unwrap();
        service
            .update_status(&main_id, Status::Ready, "agent-1")
            .unwrap();
        service
            .update_status(&main_id, Status::Todo, "agent-1")
            .unwrap();

        // Now InProgress should work
        let result = service.update_status(&main_id, Status::InProgress, "agent-1");
        assert!(result.is_ok());
    }

    #[test]
    fn test_delete_removes_element() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("To Delete"))
            .unwrap();
        let id = task.id().unwrap().clone();

        service.delete(&id).unwrap();

        let result = service.get_element(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_fails_when_element_has_dependents() {
        let (service, _repo) = create_service();

        // Create a dependency
        let dep_task = service
            .create_element(KanbanElement::new_task("Dependency"))
            .unwrap();
        let dep_id = dep_task.id().unwrap().clone();

        // Create main task that depends on dep_task
        let mut main_task = KanbanElement::new_task("Main Task");
        main_task.base_mut().dependencies.push(dep_id.clone());
        let main_task = service.create_element(main_task).unwrap();
        let _main_id = main_task.id().unwrap().clone();

        // Try to delete the dependency - should fail
        let result = service.delete(&dep_id);
        assert!(result.is_err());

        // Verify the dependency still exists
        let dep = service.get_element(&dep_id).unwrap();
        assert!(dep.is_some());
    }

    #[test]
    fn test_delete_succeeds_after_removing_dependency() {
        let (service, _repo) = create_service();

        // Create a dependency
        let dep_task = service
            .create_element(KanbanElement::new_task("Dependency"))
            .unwrap();
        let dep_id = dep_task.id().unwrap().clone();

        // Create main task that depends on dep_task
        let mut main_task = KanbanElement::new_task("Main Task");
        main_task.base_mut().dependencies.push(dep_id.clone());
        let main_task = service.create_element(main_task).unwrap();
        let main_id = main_task.id().unwrap().clone();

        // Remove the dependency from main task
        service.remove_dependency(&main_id, &dep_id).unwrap();

        // Now deleting dep_task should succeed
        service.delete(&dep_id).unwrap();

        // Verify it's gone
        let dep = service.get_element(&dep_id).unwrap();
        assert!(dep.is_none());
    }

    #[test]
    fn test_append_tip_to_task() {
        let (service, _repo) = create_service();

        let task = service
            .create_element(KanbanElement::new_task("Task"))
            .unwrap();
        let task_id = task.id().unwrap().clone();

        let tips = service.append_tip(&task_id, "agent-1", "This is a helpful tip").unwrap();
        assert_eq!(tips.element_type(), ElementType::Tips);
        assert_eq!(tips.title(), "This is a helpful tip");
    }

    #[test]
    fn test_append_tip_to_non_task_fails() {
        let (service, _repo) = create_service();

        let story = service
            .create_element(KanbanElement::new_story("Story", "Content"))
            .unwrap();
        let story_id = story.id().unwrap().clone();

        let result = service.append_tip(&story_id, "agent-1", "This won't work");
        assert!(result.is_err());
    }
}
