//! Unit tests for registry implementations: StatusRegistry, ElementTypeRegistry, ElementRegistry
//!
//! TDD: These tests define the expected registry API before implementation

use agent_kanban::registry::{StatusRegistry, ElementTypeRegistry, ElementRegistry};
use agent_kanban::traits::{KanbanStatus, KanbanElementTypeTrait, KanbanElementTrait};
use agent_kanban::types::{StatusType, ElementTypeIdentifier};
use std::sync::Arc;
use std::thread;

mod status_registry_tests {
    use super::*;

    /// Test implementation of KanbanStatus
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
    fn test_status_registry_new() {
        let registry = StatusRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_status_registry_register() {
        let registry = StatusRegistry::new();
        let status = TestStatus {
            status_type: StatusType::new("custom"),
        };
        registry.register(Box::new(status));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_status_registry_get() {
        let registry = StatusRegistry::new();
        let status = TestStatus {
            status_type: StatusType::new("custom"),
        };
        registry.register(Box::new(status));

        let retrieved = registry.get(&StatusType::new("custom"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().status_type().name(), "custom");
    }

    #[test]
    fn test_status_registry_get_not_found() {
        let registry = StatusRegistry::new();
        let retrieved = registry.get(&StatusType::new("unknown"));
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_status_registry_thread_safe_registration() {
        let registry = Arc::new(StatusRegistry::new());
        let mut handles = vec![];

        // Register from multiple threads
        for i in 0..10 {
            let registry_clone = registry.clone();
            handles.push(thread::spawn(move || {
                let status = TestStatus {
                    status_type: StatusType::new(format!("status-{}", i)),
                };
                registry_clone.register(Box::new(status));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn test_status_registry_thread_safe_retrieval() {
        let registry = Arc::new(StatusRegistry::new());

        // Register initial statuses
        for name in ["plan", "backlog", "done"] {
            let status = TestStatus {
                status_type: StatusType::new(name),
            };
            registry.register(Box::new(status));
        }

        let mut handles = vec![];

        // Retrieve from multiple threads
        for _ in 0..10 {
            let registry_clone = registry.clone();
            handles.push(thread::spawn(move || {
                let retrieved = registry_clone.get(&StatusType::new("plan"));
                assert!(retrieved.is_some());
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

mod element_type_registry_tests {
    use super::*;

    /// Test implementation of KanbanElementTypeTrait
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
    fn test_element_type_registry_new() {
        let registry = ElementTypeRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_element_type_registry_register() {
        let registry = ElementTypeRegistry::new();
        let elem_type = TestElementType {
            type_id: ElementTypeIdentifier::new("custom"),
        };
        registry.register(Box::new(elem_type));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_element_type_registry_get() {
        let registry = ElementTypeRegistry::new();
        let elem_type = TestElementType {
            type_id: ElementTypeIdentifier::new("custom"),
        };
        registry.register(Box::new(elem_type));

        let retrieved = registry.get(&ElementTypeIdentifier::new("custom"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().element_type().name(), "custom");
    }

    #[test]
    fn test_element_type_registry_get_not_found() {
        let registry = ElementTypeRegistry::new();
        let retrieved = registry.get(&ElementTypeIdentifier::new("unknown"));
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_element_type_registry_thread_safe_registration() {
        let registry = Arc::new(ElementTypeRegistry::new());
        let mut handles = vec![];

        // Register from multiple threads
        for i in 0..10 {
            let registry_clone = registry.clone();
            handles.push(thread::spawn(move || {
                let elem_type = TestElementType {
                    type_id: ElementTypeIdentifier::new(format!("type-{}", i)),
                };
                registry_clone.register(Box::new(elem_type));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }

    #[test]
    fn test_element_type_registry_thread_safe_retrieval() {
        let registry = Arc::new(ElementTypeRegistry::new());

        // Register initial element types
        for name in ["task", "story", "sprint"] {
            let elem_type = TestElementType {
                type_id: ElementTypeIdentifier::new(name),
            };
            registry.register(Box::new(elem_type));
        }

        let mut handles = vec![];

        // Retrieve from multiple threads
        for _ in 0..10 {
            let registry_clone = registry.clone();
            handles.push(thread::spawn(move || {
                let retrieved = registry_clone.get(&ElementTypeIdentifier::new("task"));
                assert!(retrieved.is_some());
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

mod element_registry_tests {
    use super::*;
    use agent_kanban::domain::ElementId;

    /// Test implementation of KanbanElementTrait
    struct TestElement {
        id: Option<ElementId>,
        element_type: ElementTypeIdentifier,
        status: StatusType,
        title: String,
    }

    impl KanbanElementTrait for TestElement {
        fn id(&self) -> Option<ElementId> {
            self.id.clone()
        }

        fn element_type(&self) -> ElementTypeIdentifier {
            self.element_type.clone()
        }

        fn status(&self) -> StatusType {
            self.status.clone()
        }

        fn title(&self) -> String {
            self.title.clone()
        }

        fn implementation_type(&self) -> &'static str {
            "TestElement"
        }

        fn clone_boxed(&self) -> Box<dyn KanbanElementTrait> {
            Box::new(TestElement {
                id: self.id.clone(),
                element_type: self.element_type.clone(),
                status: self.status.clone(),
                title: self.title.clone(),
            })
        }
    }

    #[test]
    fn test_element_registry_new() {
        let registry = ElementRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_element_registry_register() {
        let registry = ElementRegistry::new();
        let element = TestElement {
            id: None,
            element_type: ElementTypeIdentifier::new("task"),
            status: StatusType::new("plan"),
            title: "Test Task".to_string(),
        };
        registry.register(Box::new(element));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_element_registry_get_by_id() {
        let registry = ElementRegistry::new();
        let id = ElementId::new(agent_kanban::domain::ElementType::Task, 1);
        let element = TestElement {
            id: Some(id.clone()),
            element_type: ElementTypeIdentifier::new("task"),
            status: StatusType::new("plan"),
            title: "Test Task".to_string(),
        };
        registry.register(Box::new(element));

        let retrieved = registry.get_by_id(&id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title(), "Test Task");
    }

    #[test]
    fn test_element_registry_get_by_id_not_found() {
        let registry = ElementRegistry::new();
        let id = ElementId::new(agent_kanban::domain::ElementType::Task, 1);
        let retrieved = registry.get_by_id(&id);
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_element_registry_list_by_type() {
        let registry = ElementRegistry::new();

        // Register task
        let task = TestElement {
            id: Some(ElementId::new(agent_kanban::domain::ElementType::Task, 1)),
            element_type: ElementTypeIdentifier::new("task"),
            status: StatusType::new("plan"),
            title: "Task 1".to_string(),
        };
        registry.register(Box::new(task));

        // Register story
        let story = TestElement {
            id: Some(ElementId::new(agent_kanban::domain::ElementType::Story, 1)),
            element_type: ElementTypeIdentifier::new("story"),
            status: StatusType::new("plan"),
            title: "Story 1".to_string(),
        };
        registry.register(Box::new(story));

        let tasks = registry.list_by_type(&ElementTypeIdentifier::new("task"));
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title(), "Task 1");

        let stories = registry.list_by_type(&ElementTypeIdentifier::new("story"));
        assert_eq!(stories.len(), 1);
    }

    #[test]
    fn test_element_registry_thread_safe_registration() {
        let registry = Arc::new(ElementRegistry::new());
        let mut handles = vec![];

        // Register from multiple threads
        for i in 0..10 {
            let registry_clone = registry.clone();
            handles.push(thread::spawn(move || {
                let element = TestElement {
                    id: Some(ElementId::new(agent_kanban::domain::ElementType::Task, i)),
                    element_type: ElementTypeIdentifier::new("task"),
                    status: StatusType::new("plan"),
                    title: format!("Task {}", i),
                };
                registry_clone.register(Box::new(element));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
    }
}