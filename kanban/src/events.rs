//! Event types for the kanban system

use crate::domain::{ElementId, ElementType, Status};
use std::fmt;

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

/// Simple event bus for publishing events
#[derive(Debug, Default)]
pub struct KanbanEventBus {
    // Placeholder - will be expanded in Sprint 4 with full pub/sub
}

impl KanbanEventBus {
    pub fn new() -> Self {
        KanbanEventBus::default()
    }

    pub fn subscribe(&self, _subscriber: Box<dyn KanbanEventSubscriber + Send>) {
        // Placeholder - will be implemented in Sprint 4
    }

    pub fn publish(&self, _event: KanbanEvent) {
        // Placeholder - will be implemented in Sprint 4
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
