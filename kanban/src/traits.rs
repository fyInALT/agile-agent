//! Trait definitions for trait-based kanban architecture
//!
//! Extensible traits replacing fixed enums, following Decision Layer pattern.

use crate::serde::ElementSerde;
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
    // === Core identity methods ===

    /// Get the element ID (if assigned)
    fn id(&self) -> Option<crate::domain::ElementId>;

    /// Get the element type identifier
    fn element_type(&self) -> ElementTypeIdentifier;

    /// Get the current status
    fn status(&self) -> StatusType;

    /// Get the element title
    fn title(&self) -> String;

    // === Content and metadata ===

    /// Get the element content/description (default: empty string)
    fn content(&self) -> String {
        String::new()
    }

    /// Get the element dependencies (default: empty vec)
    fn dependencies(&self) -> Vec<crate::domain::ElementId> {
        Vec::new()
    }

    /// Get the parent element ID (default: None)
    fn parent(&self) -> Option<crate::domain::ElementId> {
        None
    }

    /// Get the assignee (default: None)
    fn assignee(&self) -> Option<String> {
        None
    }

    /// Get the priority (default: Medium)
    fn priority(&self) -> crate::domain::Priority {
        crate::domain::Priority::Medium
    }

    /// Get the effort/story points (default: None)
    fn effort(&self) -> Option<u32> {
        None
    }

    /// Get the blocked reason (default: None)
    fn blocked_reason(&self) -> Option<String> {
        None
    }

    /// Get the tags (default: empty vec)
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }

    // === Mutation methods ===

    /// Set the element ID
    fn set_id(&mut self, id: crate::domain::ElementId);

    /// Set the status
    fn set_status(&mut self, status: StatusType);

    // === Serialization ===

    /// Convert to ElementSerde for serialization
    fn to_serde(&self) -> ElementSerde {
        ElementSerde::new(
            self.element_type().name().to_string(),
            self.title(),
            self.content(),
            self.status().name().to_string(),
            self.id().map(|id| id.as_str().to_string()),
            Some(self.priority().as_str().to_string()),
            self.effort(),
            self.assignee(),
            self.blocked_reason(),
            self.tags(),
            self.dependencies().iter().map(|d| d.as_str().to_string()).collect(),
            self.parent().map(|p| p.as_str().to_string()),
        )
    }

    // === Debug and utility ===

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