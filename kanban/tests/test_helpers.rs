//! Test utilities for kanban crate
//!
//! This module provides shared test infrastructure for integration tests.

use agent_kanban::domain::{ElementId, ElementType, KanbanElement, Status};
use agent_kanban::events::{KanbanEvent, KanbanEventBus, KanbanEventSubscriber};
use agent_kanban::repository::KanbanElementRepository;
use agent_kanban::service::KanbanService;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Test repository with in-memory storage
pub struct TestRepository {
    elements: RwLock<Vec<KanbanElement>>,
    counters: RwLock<HashMap<ElementType, u32>>,
}

impl TestRepository {
    pub fn new() -> Self {
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

/// Event collector for testing - captures all published events
pub struct EventCollector {
    events: RwLock<Vec<KanbanEvent>>,
}

impl EventCollector {
    pub fn new() -> Self {
        EventCollector {
            events: RwLock::new(Vec::new()),
        }
    }

    pub fn get_events(&self) -> Vec<KanbanEvent> {
        self.events.read().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.write().unwrap().clear();
    }
}

impl KanbanEventSubscriber for EventCollector {
    fn on_event(&self, event: &KanbanEvent) {
        self.events.write().unwrap().push(event.clone());
    }
}

/// Wrapper to allow Arc<EventCollector> to be used as subscriber
#[derive(Clone)]
pub struct SharedEventCollector {
    collector: std::sync::Arc<EventCollector>,
}

impl SharedEventCollector {
    pub fn new(collector: std::sync::Arc<EventCollector>) -> Self {
        SharedEventCollector { collector }
    }

    pub fn get_events(&self) -> Vec<KanbanEvent> {
        self.collector.get_events()
    }

    pub fn clear(&self) {
        self.collector.clear()
    }
}

impl KanbanEventSubscriber for SharedEventCollector {
    fn on_event(&self, event: &KanbanEvent) {
        self.collector.on_event(event)
    }
}

/// Create a fully wired KanbanService with TestRepository and real EventBus
pub fn create_test_service() -> (
    KanbanService<TestRepository>,
    Arc<TestRepository>,
    Arc<KanbanEventBus>,
    SharedEventCollector,
) {
    let repo = Arc::new(TestRepository::new());
    let event_bus = Arc::new(KanbanEventBus::new());
    let collector = std::sync::Arc::new(EventCollector::new());
    let shared = SharedEventCollector::new(collector.clone());
    event_bus.subscribe(Box::new(shared.clone()));
    let service = KanbanService::new(repo.clone(), event_bus.clone());
    (service, repo, event_bus, shared)
}
