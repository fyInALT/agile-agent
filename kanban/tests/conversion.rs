//! Tests for conversion between old enum types and new trait-based types

use agent_kanban::domain::{Status, ElementType};
use agent_kanban::types::{StatusType, ElementTypeIdentifier};
use std::convert::TryFrom;

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
        assert_eq!(Status::try_from(StatusType::new("plan")).unwrap(), Status::Plan);
        assert_eq!(Status::try_from(StatusType::new("backlog")).unwrap(), Status::Backlog);
        assert_eq!(Status::try_from(StatusType::new("in_progress")).unwrap(), Status::InProgress);
        assert_eq!(Status::try_from(StatusType::new("verified")).unwrap(), Status::Verified);
    }

    #[test]
    fn test_status_from_unknown_status_type_error() {
        // Unknown status types should return an error
        let result = Status::try_from(StatusType::new("custom_unknown"));
        assert!(result.is_err());
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
        assert_eq!(ElementType::try_from(ElementTypeIdentifier::new("sprint")).unwrap(), ElementType::Sprint);
        assert_eq!(ElementType::try_from(ElementTypeIdentifier::new("story")).unwrap(), ElementType::Story);
        assert_eq!(ElementType::try_from(ElementTypeIdentifier::new("task")).unwrap(), ElementType::Task);
        assert_eq!(ElementType::try_from(ElementTypeIdentifier::new("tips")).unwrap(), ElementType::Tips);
    }

    #[test]
    fn test_element_type_from_unknown_identifier_error() {
        // Unknown element types should return an error
        let result = ElementType::try_from(ElementTypeIdentifier::new("custom_unknown"));
        assert!(result.is_err());
    }
}
