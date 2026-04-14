//! Unit tests for trait definitions: KanbanStatus, KanbanElementTypeTrait, KanbanElementTrait
//!
//! TDD: These tests define the expected trait API before implementation

use agent_kanban::traits::{KanbanStatus, KanbanElementTypeTrait, KanbanElementTrait};
use agent_kanban::types::{StatusType, ElementTypeIdentifier, builtin_statuses, builtin_element_types};
use agent_kanban::domain::BaseElement;

mod kanban_status_trait_tests {
    use super::*;

    /// Test struct implementing KanbanStatus for testing
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
    fn test_kanban_status_trait_has_status_type() {
        let status = TestStatus {
            status_type: builtin_statuses::plan(),
        };
        assert_eq!(status.status_type().name(), "plan");
    }

    #[test]
    fn test_kanban_status_trait_has_implementation_type() {
        let status = TestStatus {
            status_type: builtin_statuses::plan(),
        };
        assert_eq!(status.implementation_type(), "TestStatus");
    }

    #[test]
    fn test_kanban_status_trait_has_is_terminal() {
        let verified = TestStatus {
            status_type: builtin_statuses::verified(),
        };
        assert!(verified.is_terminal());

        let plan = TestStatus {
            status_type: builtin_statuses::plan(),
        };
        assert!(!plan.is_terminal());
    }

    #[test]
    fn test_kanban_status_trait_has_clone_boxed() {
        let status = TestStatus {
            status_type: builtin_statuses::backlog(),
        };
        let cloned = status.clone_boxed();
        assert_eq!(cloned.status_type().name(), "backlog");
        assert_eq!(cloned.implementation_type(), "TestStatus");
    }

    #[test]
    fn test_kanban_status_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestStatus>();
    }
}

mod kanban_element_type_trait_tests {
    use super::*;

    /// Test struct implementing KanbanElementTypeTrait for testing
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
            builtin_statuses::plan()
        }

        fn clone_boxed(&self) -> Box<dyn KanbanElementTypeTrait> {
            Box::new(TestElementType {
                type_id: self.type_id.clone(),
            })
        }
    }

    #[test]
    fn test_element_type_trait_has_element_type() {
        let elem_type = TestElementType {
            type_id: builtin_element_types::task(),
        };
        assert_eq!(elem_type.element_type().name(), "task");
    }

    #[test]
    fn test_element_type_trait_has_implementation_type() {
        let elem_type = TestElementType {
            type_id: builtin_element_types::story(),
        };
        assert_eq!(elem_type.implementation_type(), "TestElementType");
    }

    #[test]
    fn test_element_type_trait_has_default_status() {
        let elem_type = TestElementType {
            type_id: builtin_element_types::task(),
        };
        assert_eq!(elem_type.default_status().name(), "plan");
    }

    #[test]
    fn test_element_type_trait_has_clone_boxed() {
        let elem_type = TestElementType {
            type_id: builtin_element_types::sprint(),
        };
        let cloned = elem_type.clone_boxed();
        assert_eq!(cloned.element_type().name(), "sprint");
        assert_eq!(cloned.implementation_type(), "TestElementType");
    }

    #[test]
    fn test_element_type_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestElementType>();
    }
}

mod kanban_element_trait_tests {
    use super::*;
    use agent_kanban::domain::{ElementId, BaseElement, KanbanElement};
    use std::sync::RwLock;

    /// Test wrapper to implement KanbanElementTrait using existing KanbanElement
    struct TestKanbanElement {
        inner: RwLock<KanbanElement>,
    }

    impl KanbanElementTrait for TestKanbanElement {
        fn id(&self) -> Option<ElementId> {
            self.inner.read().unwrap().id().cloned()
        }

        fn element_type(&self) -> ElementTypeIdentifier {
            ElementTypeIdentifier::new(self.inner.read().unwrap().element_type().as_str())
        }

        fn status(&self) -> StatusType {
            StatusType::new(self.inner.read().unwrap().status().to_string().to_lowercase())
        }

        fn title(&self) -> String {
            self.inner.read().unwrap().title().to_string()
        }

        fn implementation_type(&self) -> &'static str {
            "TestKanbanElement"
        }

        fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
            let inner = self.inner.read().unwrap().clone();
            Box::new(TestKanbanElement {
                inner: RwLock::new(inner),
            })
        }
    }

    #[test]
    fn test_kanban_element_trait_has_id() {
        let task = KanbanElement::new_task("Test Task");
        let elem = TestKanbanElement {
            inner: RwLock::new(task),
        };
        // Initially no ID
        assert!(elem.id().is_none());
    }

    #[test]
    fn test_kanban_element_trait_has_element_type() {
        let task = KanbanElement::new_task("Test Task");
        let elem = TestKanbanElement {
            inner: RwLock::new(task),
        };
        assert_eq!(elem.element_type().name(), "task");
    }

    #[test]
    fn test_kanban_element_trait_has_status() {
        let task = KanbanElement::new_task("Test Task");
        let elem = TestKanbanElement {
            inner: RwLock::new(task),
        };
        assert_eq!(elem.status().name(), "plan");
    }

    #[test]
    fn test_kanban_element_trait_has_title() {
        let task = KanbanElement::new_task("My Task Title");
        let elem = TestKanbanElement {
            inner: RwLock::new(task),
        };
        assert_eq!(elem.title(), "My Task Title");
    }

    #[test]
    fn test_kanban_element_trait_has_implementation_type() {
        let task = KanbanElement::new_task("Test Task");
        let elem = TestKanbanElement {
            inner: RwLock::new(task),
        };
        assert_eq!(elem.implementation_type(), "TestKanbanElement");
    }

    #[test]
    fn test_kanban_element_trait_has_clone_boxed() {
        let task = KanbanElement::new_task("Test Task");
        let elem = TestKanbanElement {
            inner: RwLock::new(task),
        };
        let cloned = elem.clone_boxed();
        assert_eq!(cloned.title(), "Test Task");
        assert_eq!(cloned.implementation_type(), "TestKanbanElement");
    }

    #[test]
    fn test_kanban_element_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TestKanbanElement>();
    }
}