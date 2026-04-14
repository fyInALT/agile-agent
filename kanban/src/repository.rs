//! Repository trait for kanban element persistence

use crate::domain::Status;
use crate::domain::{ElementId, ElementType, KanbanElement};
use crate::error::KanbanError;

/// KanbanElementRepository defines the interface for element persistence
pub trait KanbanElementRepository: Send + Sync {
    /// Get an element by its ID
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError>;

    /// List all elements
    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by type
    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by status
    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by assignee
    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by parent ID
    fn list_by_parent(&self, parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements that are blocked
    fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError>;

    /// Save an element (insert or update)
    fn save(&self, element: KanbanElement) -> Result<(), KanbanError>;

    /// Delete an element by ID
    fn delete(&self, id: &ElementId) -> Result<(), KanbanError>;

    /// Generate a new ID for the given element type
    fn new_id(&self, type_: ElementType) -> Result<ElementId, KanbanError>;
}
