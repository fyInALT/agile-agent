//! Tests for KanbanService with TransitionRegistry

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Status};
use agent_kanban::error::KanbanError;
use agent_kanban::events::KanbanEventBus;
use agent_kanban::repository::KanbanElementRepository;
use agent_kanban::service::KanbanService;
use agent_kanban::transition::TransitionRegistry;
use agent_kanban::types::StatusType;
use std::sync::Arc;

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

    fn list_by_sprint(&self, sprint_id: &ElementId) -> Result<Vec<KanbanElement>, KanbanError> {
        Ok(self
            .elements
            .read()
            .unwrap()
            .iter()
            .filter(|e| e.parent().map(|p| p == sprint_id).unwrap_or(false))
            .cloned()
            .collect())
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

mod service_with_registry_tests {
    use super::*;

    #[test]
    fn test_service_new_with_registry() {
        let repo = Arc::new(MockRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let registry = Arc::new(TransitionRegistry::new());
        registry.register_builtin_rules();

        let service = KanbanService::new_with_registry(repo, event_bus, registry);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        // Valid transition via registry
        let updated = service
            .update_status(&id, Status::Backlog, "agent")
            .unwrap();
        assert_eq!(updated.status(), Status::Backlog);
    }

    #[test]
    fn test_service_registry_validates_transitions() {
        let repo = Arc::new(MockRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let registry = Arc::new(TransitionRegistry::new());
        registry.register_builtin_rules();

        let service = KanbanService::new_with_registry(repo, event_bus, registry);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        // Invalid transition via registry
        let result = service.update_status(&id, Status::Done, "agent");
        assert!(result.is_err());
    }

    #[test]
    fn test_service_update_status_with_type() {
        let repo = Arc::new(MockRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let registry = Arc::new(TransitionRegistry::new());
        registry.register_builtin_rules();

        let service = KanbanService::new_with_registry(repo, event_bus, registry);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        // Valid transition using StatusType
        let updated = service
            .update_status_with_type(&id, StatusType::new("backlog"), "agent")
            .unwrap();
        assert_eq!(updated.status(), Status::Backlog);
    }

    #[test]
    fn test_service_without_registry_uses_enum_validation() {
        let repo = Arc::new(MockRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());

        // Service without registry
        let service = KanbanService::new(repo, event_bus);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        // Valid transition via enum (Plan -> Backlog)
        let updated = service
            .update_status(&id, Status::Backlog, "agent")
            .unwrap();
        assert_eq!(updated.status(), Status::Backlog);

        // Invalid transition via enum (Plan -> Done)
        let result = service.update_status(&id, Status::Done, "agent");
        assert!(result.is_err());
    }
}
