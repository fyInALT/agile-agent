//! Trait definitions for trait-based kanban architecture
//!
//! Extensible traits replacing fixed enums, following Decision Layer pattern.

use crate::types::{StatusType, ElementTypeIdentifier};

/// KanbanStatus trait - extensible status implementation
///
/// Replaces fixed Status enum with trait-based implementation,
/// enabling custom statuses without modifying core code.
pub trait KanbanStatus: Send + Sync + 'static {
    /// Get the status type identifier
    fn status_type(&self) -> StatusType;

    /// Get the concrete implementation type name (for debugging)
    fn implementation_type(&self) -> &'static str;

    /// Check if this is a terminal status (no further transitions)
    fn is_terminal(&self) -> bool;

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn KanbanStatus>;
}

/// KanbanElementTypeTrait trait - extensible element type implementation
///
/// Replaces fixed ElementType enum with trait-based implementation,
/// enabling custom element types without modifying core code.
pub trait KanbanElementTypeTrait: Send + Sync + 'static {
    /// Get the element type identifier
    fn element_type(&self) -> ElementTypeIdentifier;

    /// Get the concrete implementation type name (for debugging)
    fn implementation_type(&self) -> &'static str;

    /// Get the default status for this element type
    fn default_status(&self) -> StatusType;

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait>;
}

/// KanbanElementTrait trait - extensible kanban element implementation
///
/// Replaces fixed KanbanElement enum with trait-based implementation,
/// enabling custom element types with custom behavior.
pub trait KanbanElementTrait: Send + Sync + 'static {
    /// Get the element ID (if assigned)
    fn id(&self) -> Option<crate::domain::ElementId>;

    /// Get the element type identifier
    fn element_type(&self) -> ElementTypeIdentifier;

    /// Get the current status
    fn status(&self) -> StatusType;

    /// Get the element title
    fn title(&self) -> String;

    /// Get the concrete implementation type name (for debugging)
    fn implementation_type(&self) -> &'static str;

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn KanbanElementTrait>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test implementation for KanbanStatus
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
            self.status_type.name() == "verified"
        }

        fn clone_boxed(&self) -> Box<dyn KanbanStatus> {
            Box::new(TestStatus {
                status_type: self.status_type.clone(),
            })
        }
    }

    #[test]
    fn test_kanban_status_trait() {
        let status = TestStatus {
            status_type: StatusType::new("plan"),
        };
        assert_eq!(status.status_type().name(), "plan");
        assert_eq!(status.implementation_type(), "TestStatus");
        assert!(!status.is_terminal());
    }

    #[test]
    fn test_kanban_status_clone_boxed() {
        let status = TestStatus {
            status_type: StatusType::new("verified"),
        };
        let cloned = status.clone_boxed();
        assert!(cloned.is_terminal());
    }

    /// Test implementation for KanbanElementTypeTrait
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
    fn test_element_type_trait() {
        let elem_type = TestElementType {
            type_id: ElementTypeIdentifier::new("task"),
        };
        assert_eq!(elem_type.element_type().name(), "task");
        assert_eq!(elem_type.implementation_type(), "TestElementType");
        assert_eq!(elem_type.default_status().name(), "plan");
    }
}