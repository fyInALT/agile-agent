//! Repository trait for kanban element persistence

use crate::domain::Status;
use crate::domain::{ElementId, ElementType, KanbanElement};
use crate::error::KanbanError;
use crate::types::{ElementTypeIdentifier, StatusType};

/// KanbanElementRepository defines the interface for element persistence
pub trait KanbanElementRepository: Send + Sync {
    /// Get an element by its ID
    fn get(&self, id: &ElementId) -> Result<Option<KanbanElement>, KanbanError>;

    /// List all elements
    fn list(&self) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by type (enum-based)
    fn list_by_type(&self, type_: ElementType) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by type identifier (trait-based)
    fn list_by_type_identifier(&self, type_id: &ElementTypeIdentifier) -> Result<Vec<KanbanElement>, KanbanError> {
        // Default implementation using conversion
        let type_: ElementType = type_id.clone().into();
        self.list_by_type(type_)
    }

    /// List elements by status (enum-based)
    fn list_by_status(&self, status: Status) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by status type (trait-based)
    fn list_by_status_type(&self, status_type: &StatusType) -> Result<Vec<KanbanElement>, KanbanError> {
        // Default implementation using conversion
        let status: Status = status_type.clone().into();
        self.list_by_status(status)
    }

    /// List elements by assignee
    fn list_by_assignee(&self, assignee: &str) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by parent ID
    fn list_by_parent(&self, parent: &ElementId) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements by sprint ID
    fn list_by_sprint(&self, sprint_id: &ElementId) -> Result<Vec<KanbanElement>, KanbanError>;

    /// List elements that are blocked
    fn list_blocked(&self) -> Result<Vec<KanbanElement>, KanbanError>;

    /// Save an element (insert or update)
    fn save(&self, element: KanbanElement) -> Result<(), KanbanError>;

    /// Delete an element by ID
    fn delete(&self, id: &ElementId) -> Result<(), KanbanError>;

    /// Generate a new ID for the given element type (enum-based)
    fn new_id(&self, type_: ElementType) -> Result<ElementId, KanbanError>;

    /// Generate a new ID for the given element type identifier (trait-based)
    fn new_id_for_type(&self, type_id: &ElementTypeIdentifier) -> Result<ElementId, KanbanError> {
        // Default implementation using conversion
        let type_: ElementType = type_id.clone().into();
        self.new_id(type_)
    }
}