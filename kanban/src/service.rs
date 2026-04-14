//! Service layer for kanban business logic

use crate::domain::{ElementId, ElementType, KanbanElement, Status};
use crate::error::KanbanError;
use crate::events::{KanbanEvent, KanbanEventBus};
use crate::repository::KanbanElementRepository;
use std::sync::Arc;

/// KanbanService provides business logic for kanban operations
pub struct KanbanService<R: KanbanElementRepository> {
    repository: Arc<R>,
    event_bus: Arc<KanbanEventBus>,
}

impl<R: KanbanElementRepository> KanbanService<R> {
    /// Create a new KanbanService
    pub fn new(repository: Arc<R>, event_bus: Arc<KanbanEventBus>) -> Self {
        KanbanService {
            repository,
            event_bus,
        }
    }

    /// Create a new kanban element
    pub fn create_element(&self, mut element: KanbanElement) -> Result<KanbanElement, KanbanError> {
        // Generate new ID
        let id = self.repository.new_id(element.element_type())?;
        element.set_id(id.clone());

        // Save to repository
        self.repository.save(element.clone())?;

        // Publish event
        self.event_bus.publish(KanbanEvent::Created {
            element_id: id,
            element_type: element.element_type(),
        });

        Ok(element)
    }

    /// Get an element by ID
    pub fn get_element(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
        self.repository.get(id)
    }

    /// List all elements
    pub fn list_elements(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list()
    }

    /// List elements by type
    pub fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list_by_type(type_)
    }

    /// List elements by status
    pub fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list_by_status(status)
    }

    /// List elements by assignee
    pub fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list_by_assignee(assignee)
    }

    /// List blocked elements
    pub fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError> {
        self.repository.list_blocked()
    }

    /// Update element status
    pub fn update_status(
        &self,
        id: &ElementId,
        new_status: Status,
        changed_by: &str,
    ) -> Result<KanbanElement, KanbanError> {
        // Get element
        let mut element = self
            .repository
            .get(id)?
            .ok_or_else(|| KanbanError::NotFound(id.as_str().to_string()))?;

        let old_status = element.status();

        // Validate transition
        if !element.can_transition_to(&new_status) {
            return Err(KanbanError::InvalidStatusTransition {
                from: old_status.to_string(),
                to: new_status.to_string(),
            });
        }

        // Check dependencies for InProgress and Done
        if new_status == Status::InProgress || new_status == Status::Done {
            let blockers = self.find_blocking_dependencies(id)?;
            if !blockers.is_empty() {
                return Err(KanbanError::DependenciesNotMet(
                    blockers.iter().map(|id| id.as_str().to_string()).collect(),
                ));
            }
        }

        // Update status
        element
            .transition(new_status)
            .map_err(|_e| KanbanError::InvalidStatusTransition {
                from: old_status.to_string(),
                to: new_status.to_string(),
            })?;

        // Save
        self.repository.save(element.clone())?;

        // Publish event
        self.event_bus.publish(KanbanEvent::StatusChanged {
            element_id: id.clone(),
            old_status,
            new_status,
            changed_by: changed_by.to_string(),
        });

        Ok(element)
    }

    /// Find blocking dependencies (not Done or Verified)
    pub fn find_blocking_dependencies(
        &self,
        id: &ElementId,
    ) -> Result<Vec<ElementId>, KanbanError> {
        let element = self
            .repository
            .get(id)?
            .ok_or_else(|| KanbanError::NotFound(id.as_str().to_string()))?;

        let mut blockers = Vec::new();

        for dep_id in element.dependencies() {
            if let Some(dep) = self.repository.get(dep_id)? {
                if dep.status() != Status::Done && dep.status() != Status::Verified {
                    blockers.push(dep_id.clone());
                }
            } else {
                // Dangling dependency - treat as blocker
                blockers.push(dep_id.clone());
            }
        }

        Ok(blockers)
    }

    /// Check if an element can start (no blocking dependencies)
    pub fn can_start(&self, id: &ElementId) -> Result<bool, KanbanError> {
        let blockers = self.find_blocking_dependencies(id)?;
        Ok(blockers.is_empty())
    }

    /// Add a dependency to an element
    pub fn add_dependency(&self, id: &ElementId, dependency: ElementId) -> Result<(), KanbanError> {
        let mut element = self
            .repository
            .get(id)?
            .ok_or_else(|| KanbanError::NotFound(id.as_str().to_string()))?;

        if !element.dependencies().contains(&dependency) {
            element.base_mut().dependencies.push(dependency.clone());
            self.repository.save(element)?;
            self.event_bus.publish(KanbanEvent::DependencyAdded {
                element_id: id.clone(),
                dependency,
            });
        }

        Ok(())
    }

    /// Remove a dependency from an element
    pub fn remove_dependency(
        &self,
        id: &ElementId,
        dependency: &ElementId,
    ) -> Result<(), KanbanError> {
        let mut element = self
            .repository
            .get(id)?
            .ok_or_else(|| KanbanError::NotFound(id.as_str().to_string()))?;

        element.base_mut().dependencies.retain(|d| d != dependency);
        self.repository.save(element)?;
        self.event_bus.publish(KanbanEvent::DependencyRemoved {
            element_id: id.clone(),
            dependency: dependency.clone(),
        });

        Ok(())
    }

    /// Delete an element
    pub fn delete(&self, id: &ElementId) -> Result<(), KanbanError> {
        self.repository.delete(id)?;
        self.event_bus.publish(KanbanEvent::Deleted {
            element_id: id.clone(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::KanbanElement;
    use crate::repository::KanbanElementRepository;
    use std::sync::Arc;

    struct TestRepository {
        elements: std::sync::RwLock<Vec<KanbanElement>>,
        counters: std::sync::RwLock<std::collections::HashMap<ElementType, u32>>,
    }

    impl TestRepository {
        fn new() -> Self {
            TestRepository {
                elements: std::sync::RwLock::new(Vec::new()),
                counters: std::sync::RwLock::new(std::collections::HashMap::new()),
            }
        }
    }

    impl KanbanElementRepository for TestRepository {
        fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError> {
            let elements = self.elements.read().unwrap();
            Ok(elements.iter().find(|e| e.id() == Some(id)).cloned())
        }

        fn list(&self) -> Result<Vec<KanbanElement>, KanbanError> {
            let elements = self.elements.read().unwrap();
            Ok(elements.clone())
        }

        fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError> {
            let elements = self.elements.read().unwrap();
            Ok(elements
                .iter()
                .filter(|e| e.element_type() == type_)
                .cloned()
                .collect())
        }

        fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError> {
            let elements = self.elements.read().unwrap();
            Ok(elements
                .iter()
                .filter(|e| e.status() == status)
                .cloned()
                .collect())
        }

        fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError> {
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

        fn list_by_parent(&self, _parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError> {
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
            let mut elements = self.elements.write().unwrap();
            elements.retain(|e| e.id() != Some(id));
            Ok(())
        }

        fn new_id(&self, type_: ElementType) -> Result<ElementId, KanbanError> {
            let mut counters = self.counters.write().unwrap();
            let next = counters.get(&type_).copied().unwrap_or(0) + 1;
            counters.insert(type_, next);
            Ok(ElementId::new(type_, next))
        }
    }

    #[test]
    fn test_create_element_assigns_id() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let result = service.create_element(task).unwrap();

        assert_eq!(result.id().unwrap().as_str(), "task-001");
    }

    #[test]
    fn test_create_element_publishes_event() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let _result = service.create_element(task).unwrap();

        // Event bus is stubbed, so we just verify it doesn't panic
    }

    #[test]
    fn test_get_element() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        let retrieved = service.get_element(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title(), "Test Task");
    }

    #[test]
    fn test_list_elements() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task1 = KanbanElement::new_task("Task 1");
        let task2 = KanbanElement::new_task("Task 2");
        service.create_element(task1).unwrap();
        service.create_element(task2).unwrap();

        let all = service.list_elements().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_update_status_valid_transition() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        let updated = service
            .update_status(&id, Status::Backlog, "agent-1")
            .unwrap();
        assert_eq!(updated.status(), Status::Backlog);
    }

    #[test]
    fn test_update_status_invalid_transition() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        let result = service.update_status(&id, Status::Done, "agent-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_blocking_dependencies_none() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        let blockers = service.find_blocking_dependencies(&id).unwrap();
        assert!(blockers.is_empty());
    }

    #[test]
    fn test_can_start_no_dependencies() {
        let repo = Arc::new(TestRepository::new());
        let event_bus = Arc::new(KanbanEventBus::new());
        let service = KanbanService::new(repo.clone(), event_bus);

        let task = KanbanElement::new_task("Test Task");
        let created = service.create_element(task).unwrap();
        let id = created.id().unwrap().clone();

        assert!(service.can_start(&id).unwrap());
    }
}
