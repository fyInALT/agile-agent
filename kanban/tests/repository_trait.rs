//! Tests for KanbanElementRepository trait with trait-based types

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Status};
use agent_kanban::error::KanbanError;
use agent_kanban::repository::KanbanElementRepository;
use agent_kanban::types::{ElementTypeIdentifier, StatusType};

/// Mock repository for testing
struct MockRepository {
    elements: std::sync::RwLock<Vec<KanbanElement>>,
    counters: std::sync::RwLock<std::collections::HashMap<ElementType, u32>>,
}

impl MockRepository {
    fn new() -> Self {
        MockRepository {
            elements: std::sync::RwLock::new(Vec::new()),
            counters: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl KanbanElementRepository for MockRepository {
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        let elements = self.elements.read().unwrap();
        Ok(elements.iter().find(|e| e.id() == Some(id)).cloned())
    }

    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(self.elements.read().unwrap().clone())
    }

    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(self
            .elements
            .read()
            .unwrap()
            .iter()
            .filter(|e| e.element_type() == type_)
            .cloned()
            .collect())
    }

    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(self
            .elements
            .read()
            .unwrap()
            .iter()
            .filter(|e| e.status() == status)
            .cloned()
            .collect())
    }

    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(self
            .elements
            .read()
            .unwrap()
            .iter()
            .filter(|e| e.assignee().map(|a| a == assignee).unwrap_or(false))
            .cloned()
            .collect())
    }

    fn list_by_parent(&self, _parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(Vec::new())
    }

    fn list_by_sprint(&self, _sprint_id: &ElementId) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(Vec::new())
    }

    fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        self.list_by_status(Status::Blocked)
    }

    fn save(&self, element: KanbanElement) -> Result<(), KanbanError> {
        let mut elements = self.elements.write().unwrap();
        if let Some(pos) = elements.iter().position(|e| e.id() == element.id()) {
            elements[pos] = element;
        } else {
            elements.push(element);
        }
        Ok(())
    }

    fn delete(&self, id: &ElementId) -> Result<(), KanbanError> {
        self.elements
            .write()
            .unwrap()
            .retain(|e| e.id() != Some(id));
        Ok(())
    }

    fn new_id(&self, type_: ElementType) -> Result<ElementId, KanbanError> {
        let mut counters = self.counters.write().unwrap();
        let next = counters.get(&type_).copied().unwrap_or(0) + 1;
        counters.insert(type_, next);
        Ok(ElementId::new(type_, next))
    }
}

mod repository_trait_tests {
    use super::*;

    #[test]
    fn test_list_by_type_identifier() {
        let repo = MockRepository::new();

        // Create task with ID
        let mut task = KanbanElement::new_task("Task 1");
        task.set_id(repo.new_id(ElementType::Task).unwrap());
        repo.save(task).unwrap();

        // Create story with ID
        let mut story = KanbanElement::new_story("Story 1", "Content");
        story.set_id(repo.new_id(ElementType::Story).unwrap());
        repo.save(story).unwrap();

        // List using trait-based type identifier
        let tasks = repo
            .list_by_type_identifier(&ElementTypeIdentifier::new("task"))
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title(), "Task 1");

        let stories = repo
            .list_by_type_identifier(&ElementTypeIdentifier::new("story"))
            .unwrap();
        assert_eq!(stories.len(), 1);
    }

    #[test]
    fn test_list_by_status_type() {
        let repo = MockRepository::new();

        // Create task (Plan status)
        let mut task = KanbanElement::new_task("Task 1");
        task.set_id(repo.new_id(ElementType::Task).unwrap());
        repo.save(task).unwrap();

        // Create story and transition to Backlog
        let mut story = KanbanElement::new_story("Story 1", "Content");
        story.set_id(repo.new_id(ElementType::Story).unwrap());
        story.transition(Status::Backlog).unwrap();
        repo.save(story).unwrap();

        // List using trait-based status type
        let plan_items = repo.list_by_status_type(&StatusType::new("plan")).unwrap();
        assert_eq!(plan_items.len(), 1);
        assert_eq!(plan_items[0].title(), "Task 1");

        let backlog_items = repo
            .list_by_status_type(&StatusType::new("backlog"))
            .unwrap();
        assert_eq!(backlog_items.len(), 1);
    }

    #[test]
    fn test_new_id_for_type() {
        let repo = MockRepository::new();

        let id1 = repo
            .new_id_for_type(&ElementTypeIdentifier::new("task"))
            .unwrap();
        assert_eq!(id1.as_str(), "task-001");

        let id2 = repo
            .new_id_for_type(&ElementTypeIdentifier::new("story"))
            .unwrap();
        assert_eq!(id2.as_str(), "story-001");

        // Unknown type should return an error (no silent fallback)
        let result = repo.new_id_for_type(&ElementTypeIdentifier::new("custom"));
        assert!(result.is_err());
    }
}
