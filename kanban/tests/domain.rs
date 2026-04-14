//! Unit tests for core domain types: Status, Priority, ElementId, ElementType

use agent_kanban::domain::{ElementId, ElementType, Priority, Status};

mod status_tests {
    use super::*;

    #[test]
    fn test_valid_transitions_from_plan() {
        let plan = Status::Plan;
        let valid = plan.valid_transitions();
        assert!(valid.contains(&Status::Backlog));
        assert_eq!(valid.len(), 1);
    }

    #[test]
    fn test_valid_transitions_from_backlog() {
        let backlog = Status::Backlog;
        let valid = backlog.valid_transitions();
        assert!(valid.contains(&Status::Blocked));
        assert!(valid.contains(&Status::Ready));
        assert!(valid.contains(&Status::Todo));
        assert!(valid.contains(&Status::Plan)); // Can go back
        assert_eq!(valid.len(), 4);
    }

    #[test]
    fn test_valid_transitions_from_ready() {
        let ready = Status::Ready;
        let valid = ready.valid_transitions();
        assert!(valid.contains(&Status::Todo));
        assert!(valid.contains(&Status::Backlog));
        assert_eq!(valid.len(), 2);
    }

    #[test]
    fn test_valid_transitions_from_todo() {
        let todo = Status::Todo;
        let valid = todo.valid_transitions();
        assert!(valid.contains(&Status::InProgress));
        assert!(valid.contains(&Status::Ready));
        assert_eq!(valid.len(), 2);
    }

    #[test]
    fn test_valid_transitions_from_in_progress() {
        let in_progress = Status::InProgress;
        let valid = in_progress.valid_transitions();
        assert!(valid.contains(&Status::Done));
        assert!(valid.contains(&Status::Todo));
        assert_eq!(valid.len(), 2);
    }

    #[test]
    fn test_valid_transitions_from_done() {
        let done = Status::Done;
        let valid = done.valid_transitions();
        assert!(valid.contains(&Status::Verified));
        assert!(valid.contains(&Status::Todo)); // Reopen
        assert_eq!(valid.len(), 2);
    }

    #[test]
    fn test_valid_transitions_from_verified() {
        let verified = Status::Verified;
        let valid = verified.valid_transitions();
        assert!(valid.is_empty()); // Terminal state
    }

    #[test]
    fn test_can_transition_to_valid() {
        assert!(Status::Plan.can_transition_to(&Status::Backlog));
        assert!(Status::Backlog.can_transition_to(&Status::Ready));
        assert!(Status::Todo.can_transition_to(&Status::InProgress));
        assert!(Status::InProgress.can_transition_to(&Status::Done));
        assert!(Status::Done.can_transition_to(&Status::Verified));
    }

    #[test]
    fn test_can_transition_to_invalid() {
        assert!(!Status::Plan.can_transition_to(&Status::Done));
        assert!(!Status::Plan.can_transition_to(&Status::Verified));
        assert!(!Status::Ready.can_transition_to(&Status::Done));
        assert!(!Status::Verified.can_transition_to(&Status::Backlog)); // Terminal
    }

    #[test]
    fn test_is_terminal() {
        assert!(!Status::Plan.is_terminal());
        assert!(!Status::Backlog.is_terminal());
        assert!(!Status::InProgress.is_terminal());
        assert!(!Status::Done.is_terminal());
        assert!(Status::Verified.is_terminal());
    }

    #[test]
    fn test_status_serialization() {
        let plan = Status::Plan;
        let json = serde_json::to_string(&plan).unwrap();
        assert_eq!(json, "\"plan\"");

        let parsed: Status = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Status::Plan);
    }

    #[test]
    fn test_all_statuses_serialize_correctly() {
        for status in [
            Status::Plan,
            Status::Backlog,
            Status::Blocked,
            Status::Ready,
            Status::Todo,
            Status::InProgress,
            Status::Done,
            Status::Verified,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: Status = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }
}

mod priority_tests {
    use super::*;

    #[test]
    fn test_priority_as_str() {
        assert_eq!(Priority::Critical.as_str(), "critical");
        assert_eq!(Priority::High.as_str(), "high");
        assert_eq!(Priority::Medium.as_str(), "medium");
        assert_eq!(Priority::Low.as_str(), "low");
    }

    #[test]
    fn test_priority_from_str() {
        assert_eq!(Priority::from_str("critical"), Some(Priority::Critical));
        assert_eq!(Priority::from_str("high"), Some(Priority::High));
        assert_eq!(Priority::from_str("medium"), Some(Priority::Medium));
        assert_eq!(Priority::from_str("low"), Some(Priority::Low));
        assert_eq!(Priority::from_str("unknown"), None);
    }

    #[test]
    fn test_priority_case_insensitive() {
        assert_eq!(Priority::from_str("CRITICAL"), Some(Priority::Critical));
        assert_eq!(Priority::from_str("High"), Some(Priority::High));
    }

    #[test]
    fn test_priority_serialization() {
        let critical = Priority::Critical;
        let json = serde_json::to_string(&critical).unwrap();
        assert_eq!(json, "\"critical\"");

        let parsed: Priority = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Priority::Critical);
    }
}

mod element_type_tests {
    use super::*;

    #[test]
    fn test_element_type_as_str() {
        assert_eq!(ElementType::Sprint.as_str(), "sprint");
        assert_eq!(ElementType::Story.as_str(), "story");
        assert_eq!(ElementType::Task.as_str(), "task");
        assert_eq!(ElementType::Idea.as_str(), "idea");
        assert_eq!(ElementType::Issue.as_str(), "issue");
        assert_eq!(ElementType::Tips.as_str(), "tips");
    }

    #[test]
    fn test_element_type_from_str() {
        assert_eq!(ElementType::from_str("sprint"), Some(ElementType::Sprint));
        assert_eq!(ElementType::from_str("story"), Some(ElementType::Story));
        assert_eq!(ElementType::from_str("task"), Some(ElementType::Task));
        assert_eq!(ElementType::from_str("idea"), Some(ElementType::Idea));
        assert_eq!(ElementType::from_str("issue"), Some(ElementType::Issue));
        assert_eq!(ElementType::from_str("tips"), Some(ElementType::Tips));
        assert_eq!(ElementType::from_str("tip"), Some(ElementType::Tips)); // Alias
        assert_eq!(ElementType::from_str("unknown"), None);
    }

    #[test]
    fn test_element_type_case_insensitive() {
        assert_eq!(ElementType::from_str("SPRINT"), Some(ElementType::Sprint));
        assert_eq!(ElementType::from_str("Task"), Some(ElementType::Task));
    }

    #[test]
    fn test_element_type_serialization() {
        let sprint = ElementType::Sprint;
        let json = serde_json::to_string(&sprint).unwrap();
        assert_eq!(json, "\"sprint\"");

        let parsed: ElementType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ElementType::Sprint);
    }
}

mod element_id_tests {
    use super::*;

    #[test]
    fn test_element_id_new() {
        let id = ElementId::new(ElementType::Task, 42);
        assert_eq!(id.as_str(), "task-042");
    }

    #[test]
    fn test_element_id_number() {
        let id = ElementId::new(ElementType::Sprint, 1);
        assert_eq!(id.number(), 1);

        let id = ElementId::new(ElementType::Story, 999);
        assert_eq!(id.number(), 999);
    }

    #[test]
    fn test_element_id_type() {
        let id = ElementId::new(ElementType::Issue, 5);
        assert_eq!(id.type_(), ElementType::Issue);
    }

    #[test]
    fn test_element_id_display() {
        let id = ElementId::new(ElementType::Idea, 7);
        let display = format!("{}", id);
        assert_eq!(display, "idea-007");
    }

    #[test]
    fn test_element_id_hash() {
        use std::collections::HashSet;
        let id1 = ElementId::new(ElementType::Task, 1);
        let id2 = ElementId::new(ElementType::Task, 1);
        let id3 = ElementId::new(ElementType::Task, 2);

        let mut set = HashSet::new();
        set.insert(id1.clone());
        set.insert(id2.clone());
        set.insert(id3.clone());

        assert_eq!(set.len(), 2); // id1 and id2 are equal
        assert!(set.contains(&id1));
        assert!(set.contains(&id3));
    }

    #[test]
    fn test_element_id_parse_valid() {
        let id = ElementId::parse("sprint-001").unwrap();
        assert_eq!(id.type_(), ElementType::Sprint);
        assert_eq!(id.number(), 1);

        let id = ElementId::parse("task-042").unwrap();
        assert_eq!(id.type_(), ElementType::Task);
        assert_eq!(id.number(), 42);
    }

    #[test]
    fn test_element_id_parse_invalid_format() {
        assert!(ElementId::parse("invalid").is_err());
        assert!(ElementId::parse("sprint").is_err());
        assert!(ElementId::parse("sprint-").is_err());
        assert!(ElementId::parse("-001").is_err());
    }

    #[test]
    fn test_element_id_parse_invalid_type() {
        assert!(ElementId::parse("unknown-001").is_err());
    }

    #[test]
    fn test_element_id_parse_invalid_number() {
        assert!(ElementId::parse("sprint-abc").is_err());
        assert!(ElementId::parse("sprint--1").is_err());
    }

    #[test]
    fn test_element_id_serialization() {
        let id = ElementId::new(ElementType::Story, 15);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"story-015\"");

        let parsed: ElementId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn test_element_id_equality() {
        let id1 = ElementId::new(ElementType::Task, 1);
        let id2 = ElementId::new(ElementType::Task, 1);
        let id3 = ElementId::new(ElementType::Task, 2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}
