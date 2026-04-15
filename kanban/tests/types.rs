//! Unit tests for new trait-based types: StatusType, ElementTypeIdentifier
//!
//! TDD: These tests define the expected API before implementation

use agent_kanban::types::{
    ElementTypeIdentifier, StatusType, builtin_element_types, builtin_statuses,
};
use std::collections::HashSet;

mod status_type_tests {
    use super::*;

    #[test]
    fn test_status_type_new() {
        let status = StatusType::new("plan");
        assert_eq!(status.name(), "plan");
    }

    #[test]
    fn test_status_type_equality() {
        let s1 = StatusType::new("plan");
        let s2 = StatusType::new("plan");
        let s3 = StatusType::new("backlog");
        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_status_type_hash() {
        let mut set = HashSet::new();
        set.insert(StatusType::new("plan"));
        set.insert(StatusType::new("plan")); // Duplicate
        set.insert(StatusType::new("backlog"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_status_type_serialization() {
        let status = StatusType::new("in_progress");
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let parsed: StatusType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn test_status_type_display() {
        let status = StatusType::new("verified");
        assert_eq!(format!("{}", status), "verified");
    }

    #[test]
    fn test_status_type_from_str() {
        let status: StatusType = "todo".parse().unwrap();
        assert_eq!(status.name(), "todo");
    }

    #[test]
    fn test_builtin_statuses_functions() {
        assert_eq!(builtin_statuses::plan().name(), "plan");
        assert_eq!(builtin_statuses::backlog().name(), "backlog");
        assert_eq!(builtin_statuses::verified().name(), "verified");
        assert_eq!(builtin_statuses::all().len(), 8);
    }
}

mod element_type_identifier_tests {
    use super::*;

    #[test]
    fn test_element_type_new() {
        let type_id = ElementTypeIdentifier::new("task");
        assert_eq!(type_id.name(), "task");
    }

    #[test]
    fn test_element_type_equality() {
        let t1 = ElementTypeIdentifier::new("task");
        let t2 = ElementTypeIdentifier::new("task");
        let t3 = ElementTypeIdentifier::new("story");
        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_element_type_hash() {
        let mut set = HashSet::new();
        set.insert(ElementTypeIdentifier::new("task"));
        set.insert(ElementTypeIdentifier::new("task")); // Duplicate
        set.insert(ElementTypeIdentifier::new("story"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_element_type_serialization() {
        let type_id = ElementTypeIdentifier::new("sprint");
        let json = serde_json::to_string(&type_id).unwrap();
        assert_eq!(json, "\"sprint\"");

        let parsed: ElementTypeIdentifier = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, type_id);
    }

    #[test]
    fn test_element_type_display() {
        let type_id = ElementTypeIdentifier::new("issue");
        assert_eq!(format!("{}", type_id), "issue");
    }

    #[test]
    fn test_element_type_from_str() {
        let type_id: ElementTypeIdentifier = "idea".parse().unwrap();
        assert_eq!(type_id.name(), "idea");
    }

    #[test]
    fn test_builtin_element_types_functions() {
        assert_eq!(builtin_element_types::task().name(), "task");
        assert_eq!(builtin_element_types::story().name(), "story");
        assert_eq!(builtin_element_types::sprint().name(), "sprint");
        assert_eq!(builtin_element_types::all().len(), 6);
    }
}
