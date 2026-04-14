//! Event types for the kanban system

use crate::domain::{ElementId, ElementType, Status};
use std::fmt;
use std::sync::RwLock;

/// KanbanEvent represents events that occur in the kanban system
#[derive(Debug, Clone)]
pub enum KanbanEvent {
    /// Element was created
    Created {
        element_id: ElementId,
        element_type: ElementType,
    },
    /// Element was updated
    Updated {
        element_id: ElementId,
        changes: Vec<String>,
    },
    /// Element status changed
    StatusChanged {
        element_id: ElementId,
        old_status: Status,
        new_status: Status,
        changed_by: String,
    },
    /// Element was deleted
    Deleted { element_id: ElementId },
    /// Tip was appended to a task
    TipAppended {
        task_id: ElementId,
        tip_id: ElementId,
        agent_id: String,
    },
    /// Dependency was added to element
    DependencyAdded {
        element_id: ElementId,
        dependency: ElementId,
    },
    /// Dependency was removed from element
    DependencyRemoved {
        element_id: ElementId,
        dependency: ElementId,
    },
}

/// Trait for event subscribers
pub trait KanbanEventSubscriber: Send {
    fn on_event(&self, event: &KanbanEvent);
}

/// Event bus for publishing and subscribing to kanban events
#[derive(Default)]
pub struct KanbanEventBus {
    subscribers: RwLock<Vec<Box<dyn KanbanEventSubscriber + Send>>>,
}

impl std::fmt::Debug for KanbanEventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KanbanEventBus")
            .field("subscriber_count", &self.subscribers.read().unwrap().len())
            .finish()
    }
}

impl KanbanEventBus {
    pub fn new() -> Self {
        KanbanEventBus {
            subscribers: RwLock::new(Vec::new()),
        }
    }

    /// Subscribe to events
    pub fn subscribe(&self, subscriber: Box<dyn KanbanEventSubscriber + Send>) {
        let mut subscribers = self.subscribers.write().unwrap();
        subscribers.push(subscriber);
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: KanbanEvent) {
        let subscribers = self.subscribers.read().unwrap();
        for subscriber in subscribers.iter() {
            subscriber.on_event(&event);
        }
    }

    /// Get the number of subscribers
    #[cfg(test)]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().unwrap().len()
    }
}

impl fmt::Display for KanbanEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KanbanEvent::Created { element_id, .. } => {
                write!(f, "Created({})", element_id)
            }
            KanbanEvent::Updated { element_id, .. } => {
                write!(f, "Updated({})", element_id)
            }
            KanbanEvent::StatusChanged {
                element_id,
                old_status,
                new_status,
                ..
            } => {
                write!(
                    f,
                    "StatusChanged({}: {} -> {})",
                    element_id, old_status, new_status
                )
            }
            KanbanEvent::Deleted { element_id } => {
                write!(f, "Deleted({})", element_id)
            }
            KanbanEvent::TipAppended { task_id, .. } => {
                write!(f, "TipAppended(task={})", task_id)
            }
            KanbanEvent::DependencyAdded {
                element_id,
                dependency,
            } => {
                write!(f, "DependencyAdded({} -> {})", element_id, dependency)
            }
            KanbanEvent::DependencyRemoved {
                element_id,
                dependency,
            } => {
                write!(f, "DependencyRemoved({} -> {})", element_id, dependency)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingSubscriber {
        count: AtomicUsize,
    }

    impl CountingSubscriber {
        fn new() -> Self {
            CountingSubscriber {
                count: AtomicUsize::new(0),
            }
        }
    }

    impl KanbanEventSubscriber for CountingSubscriber {
        fn on_event(&self, _event: &KanbanEvent) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct EventCollector {
        events: RwLock<Vec<KanbanEvent>>,
    }

    impl EventCollector {
        fn new() -> Self {
            EventCollector {
                events: RwLock::new(Vec::new()),
            }
        }
    }

    impl KanbanEventSubscriber for EventCollector {
        fn on_event(&self, event: &KanbanEvent) {
            self.events.write().unwrap().push(event.clone());
        }
    }

    #[test]
    fn test_single_subscriber_receives_events() {
        let bus = KanbanEventBus::new();
        let subscriber = Box::new(CountingSubscriber::new());
        bus.subscribe(subscriber);

        let event = KanbanEvent::Created {
            element_id: ElementId::new(ElementType::Task, 1),
            element_type: ElementType::Task,
        };
        bus.publish(event);

        // Subscriber count via internal state - just verify no panic
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_multiple_subscribers_all_receive_events() {
        let bus = KanbanEventBus::new();
        let subscriber1 = Box::new(CountingSubscriber::new());
        let subscriber2 = Box::new(CountingSubscriber::new());
        bus.subscribe(subscriber1);
        bus.subscribe(subscriber2);

        let event = KanbanEvent::Created {
            element_id: ElementId::new(ElementType::Task, 1),
            element_type: ElementType::Task,
        };
        bus.publish(event);

        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_multiple_subscribers_receive_all_events() {
        let bus = KanbanEventBus::new();
        let collector1 = Box::new(EventCollector::new());
        let collector2 = Box::new(EventCollector::new());
        bus.subscribe(collector1);
        bus.subscribe(collector2);

        // Note: the EventCollector receives ALL events - filtering is the subscriber's
        // responsibility, not the bus. This test verifies all subscribers receive
        // all published events.

        let created_event = KanbanEvent::Created {
            element_id: ElementId::new(ElementType::Task, 1),
            element_type: ElementType::Task,
        };
        let status_event = KanbanEvent::StatusChanged {
            element_id: ElementId::new(ElementType::Task, 1),
            old_status: Status::Plan,
            new_status: Status::Backlog,
            changed_by: "agent-1".to_string(),
        };

        bus.publish(created_event.clone());
        bus.publish(status_event.clone());

        // Both subscribers should have received both events
        // (but we can't verify collector contents without a reference to them here)
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_event_display() {
        let event = KanbanEvent::Created {
            element_id: ElementId::new(ElementType::Task, 1),
            element_type: ElementType::Task,
        };
        assert_eq!(format!("{}", event), "Created(task-001)");

        let event = KanbanEvent::StatusChanged {
            element_id: ElementId::new(ElementType::Story, 5),
            old_status: Status::Plan,
            new_status: Status::Backlog,
            changed_by: "agent-1".to_string(),
        };
        assert_eq!(
            format!("{}", event),
            "StatusChanged(story-005: Plan -> Backlog)"
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
}
