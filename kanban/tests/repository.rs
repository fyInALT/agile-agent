//! Unit tests for KanbanElementRepository trait

use agent_kanban::domain::{ElementId, ElementType, KanbanElement};
use agent_kanban::repository::KanbanElementRepository;
use std::collections::HashMap;
use std::sync::Arc;

/// Mock repository for testing trait bounds and basic operations
struct MockRepository {
    elements: std::sync::RwLock<Vec<KanbanElement>>,
    counters: std::sync::RwLock<HashMap<ElementType, u32>>,
}

impl MockRepository {
    fn new() -> Self {
        MockRepository {
            elements: std::sync::RwLock::new(Vec::new()),
            counters: std::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl KanbanElementRepository for MockRepository {
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
        status: agent_kanban::domain::Status,
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
        let elements = self.elements.read().unwrap();
        Ok(elements
            .iter()
            .filter(|e| e.status() == agent_kanban::domain::Status::Blocked)
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

mod mock_repository_tests {
    use super::*;

    #[test]
    fn test_save_and_get() {
        let repo = MockRepository::new();
        let element = KanbanElement::new_task("Test Task");
        let id = repo.new_id(ElementType::Task).unwrap();
        let mut element_with_id = element;
        element_with_id.set_id(id.clone());

        repo.save(element_with_id).unwrap();

        let retrieved = repo.get(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title(), "Test Task");
    }

    #[test]
    fn test_list_all() {
        let repo = MockRepository::new();

        let task1 = {
            let mut e = KanbanElement::new_task("Task 1");
            e.set_id(ElementId::new(ElementType::Task, 1));
            e
        };
        let task2 = {
            let mut e = KanbanElement::new_task("Task 2");
            e.set_id(ElementId::new(ElementType::Task, 2));
            e
        };

        repo.save(task1).unwrap();
        repo.save(task2).unwrap();

        let all = repo.list().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_list_by_type() {
        let repo = MockRepository::new();

        let task = {
            let mut e = KanbanElement::new_task("Task");
            e.set_id(ElementId::new(ElementType::Task, 1));
            e
        };
        let story = {
            let mut e = KanbanElement::new_story("Story", "Content");
            e.set_id(ElementId::new(ElementType::Story, 1));
            e
        };

        repo.save(task).unwrap();
        repo.save(story).unwrap();

        let tasks = repo.list_by_type(ElementType::Task).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title(), "Task");

        let stories = repo.list_by_type(ElementType::Story).unwrap();
        assert_eq!(stories.len(), 1);
        assert_eq!(stories[0].title(), "Story");
    }

    #[test]
    fn test_list_by_status() {
        let repo = MockRepository::new();

        let task1 = {
            let mut e = KanbanElement::new_task("Task 1");
            e.set_id(ElementId::new(ElementType::Task, 1));
            e
        };
        let task2 = {
            let mut e = KanbanElement::new_task("Task 2");
            e.set_id(ElementId::new(ElementType::Task, 2));
            e
        };

        repo.save(task1).unwrap();
        repo.save(task2).unwrap();

        let plan = repo
            .list_by_status(agent_kanban::domain::Status::Plan)
            .unwrap();
        assert_eq!(plan.len(), 2);
    }

    #[test]
    fn test_delete() {
        let repo = MockRepository::new();

        let element = {
            let mut e = KanbanElement::new_task("To Delete");
            e.set_id(ElementId::new(ElementType::Task, 1));
            e
        };
        let id = element.id().unwrap().clone();
        repo.save(element).unwrap();

        assert!(repo.get(&id).unwrap().is_some());

        repo.delete(&id).unwrap();

        assert!(repo.get(&id).unwrap().is_none());
    }

    #[test]
    fn test_new_id_generation() {
        let repo = MockRepository::new();

        let id1 = repo.new_id(ElementType::Task).unwrap();
        assert_eq!(id1.as_str(), "task-001");

        let id2 = repo.new_id(ElementType::Task).unwrap();
        assert_eq!(id2.as_str(), "task-002");

        let id3 = repo.new_id(ElementType::Story).unwrap();
        assert_eq!(id3.as_str(), "story-001");
    }

    #[test]
    fn test_update_existing() {
        let repo = MockRepository::new();

        let mut task = KanbanElement::new_task("Original");
        let id = repo.new_id(ElementType::Task).unwrap();
        task.set_id(id.clone());
        repo.save(task).unwrap();

        let mut updated = KanbanElement::new_task("Updated");
        updated.set_id(id.clone());
        repo.save(updated).unwrap();

        let all = repo.list().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title(), "Updated");
    }
}

mod trait_bounds_tests {
    use super::*;

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockRepository>();
        assert_send_sync::<Arc<dyn KanbanElementRepository>>();
    }

    #[test]
    fn test_repository_is_object_safe() {
        fn takes_repo(repo: Arc<dyn KanbanElementRepository>) {
            let _ = repo.list();
        }
        let repo = Arc::new(MockRepository::new());
        takes_repo(repo);
    }
}
