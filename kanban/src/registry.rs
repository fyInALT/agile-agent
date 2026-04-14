//! Registry implementations for trait-based kanban architecture
//!
//! Thread-safe registries using RwLock for concurrent registration and retrieval.

use crate::traits::{KanbanStatus, KanbanElementTypeTrait};
use crate::types::{StatusType, ElementTypeIdentifier};
use std::collections::HashMap;
use std::sync::RwLock;

/// StatusRegistry - thread-safe registry for KanbanStatus implementations
///
/// Uses RwLock for concurrent registration and retrieval.
/// Supports fallback chain for unknown status types.
pub struct StatusRegistry {
    statuses: RwLock<HashMap<String, Box<dyn KanbanStatus>>>,
}

impl StatusRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            statuses: RwLock::new(HashMap::new()),
        }
    }

    /// Register a status implementation (thread-safe)
    pub fn register(&self, status: Box<dyn KanbanStatus>) {
        let key = status.status_type().name().to_string();
        self.statuses.write().unwrap().insert(key, status);
    }

    /// Get a status by type (thread-safe)
    pub fn get(&self, type_: &StatusType) -> Option<Box<dyn KanbanStatus>> {
        self.statuses
            .read()
            .unwrap()
            .get(type_.name())
            .map(|s| s.clone_boxed())
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.statuses.read().unwrap().is_empty()
    }

    /// Get the number of registered statuses
    pub fn len(&self) -> usize {
        self.statuses.read().unwrap().len()
    }

    /// Check if a status type is registered
    pub fn contains(&self, type_: &StatusType) -> bool {
        self.statuses.read().unwrap().contains_key(type_.name())
    }

    /// List all registered status types
    pub fn list_types(&self) -> Vec<StatusType> {
        self.statuses
            .read()
            .unwrap()
            .keys()
            .map(|k| StatusType::new(k.clone()))
            .collect()
    }
}

impl Default for StatusRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// ElementTypeRegistry - thread-safe registry for KanbanElementTypeTrait implementations
///
/// Uses RwLock for concurrent registration and retrieval.
pub struct ElementTypeRegistry {
    element_types: RwLock<HashMap<String, Box<dyn KanbanElementTypeTrait>>>,
}

impl ElementTypeRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            element_types: RwLock::new(HashMap::new()),
        }
    }

    /// Register an element type implementation (thread-safe)
    pub fn register(&self, element_type: Box<dyn KanbanElementTypeTrait>) {
        let key = element_type.element_type().name().to_string();
        self.element_types.write().unwrap().insert(key, element_type);
    }

    /// Get an element type by identifier (thread-safe)
    pub fn get(&self, type_: &ElementTypeIdentifier) -> Option<Box<dyn KanbanElementTypeTrait>> {
        self.element_types
            .read()
            .unwrap()
            .get(type_.name())
            .map(|t| t.clone_boxed())
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.element_types.read().unwrap().is_empty()
    }

    /// Get the number of registered element types
    pub fn len(&self) -> usize {
        self.element_types.read().unwrap().len()
    }

    /// Check if an element type is registered
    pub fn contains(&self, type_: &ElementTypeIdentifier) -> bool {
        self.element_types.read().unwrap().contains_key(type_.name())
    }

    /// List all registered element type identifiers
    pub fn list_types(&self) -> Vec<ElementTypeIdentifier> {
        self.element_types
            .read()
            .unwrap()
            .keys()
            .map(|k| ElementTypeIdentifier::new(k.clone()))
            .collect()
    }
}

impl Default for ElementTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStatus {
        status_type: StatusType,
    }

    impl KanbanStatus for TestStatus {
        fn status_type(&self) -> StatusType {
            self.status_type.clone()
        }

        fn implementation_type(&self) -> &'static str {
            "TestStatus"
        }

        fn is_terminal(&self) -> bool {
            false
        }

        fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
            Box::new(TestStatus {
                status_type: self.status_type.clone(),
            })
        }
    }

    struct TestElementType {
        type_id: ElementTypeIdentifier,
    }

    impl KanbanElementTypeTrait for TestElementType {
        fn element_type(&self) -> ElementTypeIdentifier {
            self.type_id.clone()
        }

        fn implementation_type(&self) -> &'static str {
            "TestElementType"
        }

        fn default_status(&self) -> StatusType {
            StatusType::new("plan")
        }

        fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
            Box::new(TestElementType {
                type_id: self.type_id.clone(),
            })
        }
    }

    #[test]
    fn test_status_registry_new() {
        let registry = StatusRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_status_registry_register() {
        let registry = StatusRegistry::new();
        registry.register(Box::new(TestStatus {
            status_type: StatusType::new("plan"),
        }));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_status_registry_get() {
        let registry = StatusRegistry::new();
        registry.register(Box::new(TestStatus {
            status_type: StatusType::new("plan"),
        }));
        let retrieved = registry.get(&StatusType::new("plan"));
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_element_type_registry_new() {
        let registry = ElementTypeRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_element_type_registry_register() {
        let registry = ElementTypeRegistry::new();
        registry.register(Box::new(TestElementType {
            type_id: ElementTypeIdentifier::new("task"),
        }));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_element_type_registry_get() {
        let registry = ElementTypeRegistry::new();
        registry.register(Box::new(TestElementType {
            type_id: ElementTypeIdentifier::new("task"),
        }));
        let retrieved = registry.get(&ElementTypeIdentifier::new("task"));
        assert!(retrieved.is_some());
    }
}