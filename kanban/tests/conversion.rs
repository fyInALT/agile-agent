//! Tests for conversion between old enum types and new trait-based types

use agent_kanban::domain::{Status, ElementType};
use agent_kanban::types::{StatusType, ElementTypeIdentifier};

mod status_conversion_tests {
    use super::*;

    #[test]
    fn test_status_to_status_type() {
        assert_eq!(Status::Plan.to_status_type().name(), "plan");
        assert_eq!(Status::Backlog.to_status_type().name(), "backlog");
        assert_eq!(Status::InProgress.to_status_type().name(), "in_progress");
        assert_eq!(Status::Verified.to_status_type().name(), "verified");
    }

    #[test]
    fn test_status_from_status_type() {
        assert_eq!(Status::from(StatusType::new("plan")), Status::Plan);
        assert_eq!(Status::from(StatusType::new("backlog")), Status::Backlog);
        assert_eq!(Status::from(StatusType::new("in_progress")), Status::InProgress);
        assert_eq!(Status::from(StatusType::new("verified")), Status::Verified);
    }

    #[test]
    fn test_status_from_unknown_status_type_fallback() {
        // Unknown status types should fall back to Plan
        assert_eq!(Status::from(StatusType::new("custom_unknown")), Status::Plan);
    }

    #[test]
    fn test_status_as_str() {
        assert_eq!(Status::Plan.as_str(), "plan");
        assert_eq!(Status::InProgress.as_str(), "in_progress");
        assert_eq!(Status::Verified.as_str(), "verified");
    }
}

mod element_type_conversion_tests {
    use super::*;

    #[test]
    fn test_element_type_to_identifier() {
        assert_eq!(ElementType::Sprint.to_element_type_identifier().name(), "sprint");
        assert_eq!(ElementType::Story.to_element_type_identifier().name(), "story");
        assert_eq!(ElementType::Task.to_element_type_identifier().name(), "task");
        assert_eq!(ElementType::Tips.to_element_type_identifier().name(), "tips");
    }

    #[test]
    fn test_element_type_from_identifier() {
        assert_eq!(ElementType::from(ElementTypeIdentifier::new("sprint")), ElementType::Sprint);
        assert_eq!(ElementType::from(ElementTypeIdentifier::new("story")), ElementType::Story);
        assert_eq!(ElementType::from(ElementTypeIdentifier::new("task")), ElementType::Task);
        assert_eq!(ElementType::from(ElementTypeIdentifier::new("tips")), ElementType::Tips);
    }

    #[test]
    fn test_element_type_from_unknown_identifier_fallback() {
        // Unknown element types should fall back to Task
        assert_eq!(ElementType::from(ElementTypeIdentifier::new("custom_unknown")), ElementType::Task);
    }
}