//! Error types for the kanban system

use std::fmt;

/// KanbanError represents errors that can occur in the kanban system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanbanError {
    /// Element not found
    NotFound(String),
    /// Invalid status transition
    InvalidStatusTransition { from: String, to: String },
    /// Dependencies not met
    DependenciesNotMet(Vec<String>),
    /// Repository error
    RepositoryError(String),
    /// Serialization error
    SerializationError(String),
    /// Invalid input
    InvalidInput(String),
    /// Element has dependents (other elements depend on it)
    HasDependents(Vec<String>),
    /// Conversion error (unknown status/type)
    ConversionError(String),
}

impl fmt::Display for KanbanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KanbanError::NotFound(id) => write!(f, "element not found: {}", id),
            KanbanError::InvalidStatusTransition { from, to } => {
                write!(f, "invalid status transition from {} to {}", from, to)
            }
            KanbanError::DependenciesNotMet(deps) => {
                write!(f, "dependencies not met: {}", deps.join(", "))
            }
            KanbanError::RepositoryError(msg) => write!(f, "repository error: {}", msg),
            KanbanError::SerializationError(msg) => write!(f, "serialization error: {}", msg),
            KanbanError::InvalidInput(msg) => write!(f, "invalid input: {}", msg),
            KanbanError::HasDependents(deps) => {
                write!(f, "element has dependents: {}", deps.join(", "))
            }
            KanbanError::ConversionError(msg) => {
                write!(f, "conversion error: {}", msg)
            }
        }
    }
}

impl std::error::Error for KanbanError {}
