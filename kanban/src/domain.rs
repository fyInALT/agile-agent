//! Core domain types for the kanban system

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

/// Status represents the current state of a kanban element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Plan,
    Backlog,
    Blocked,
    Ready,
    Todo,
    InProgress,
    Done,
    Verified,
}

impl Status {
    /// Returns the valid status transitions from this status
    pub fn valid_transitions(&self) -> Vec<Status> {
        match self {
            Status::Plan => vec![Status::Backlog],
            Status::Backlog => vec![Status::Blocked, Status::Ready, Status::Todo, Status::Plan],
            Status::Blocked => vec![Status::Backlog],
            Status::Ready => vec![Status::Todo, Status::Backlog],
            Status::Todo => vec![Status::InProgress, Status::Ready],
            Status::InProgress => vec![Status::Done, Status::Todo],
            Status::Done => vec![Status::Verified, Status::Todo],
            Status::Verified => vec![], // Terminal state
        }
    }

    /// Checks if transitioning to the target status is valid
    pub fn can_transition_to(&self, target: &Status) -> bool {
        self.valid_transitions().contains(target)
    }

    /// Returns true if this is a terminal status (no further transitions possible)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Status::Verified)
    }
}

/// Priority represents the urgency of a kanban element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Critical => "critical",
            Priority::High => "high",
            Priority::Medium => "medium",
            Priority::Low => "low",
        }
    }

    pub fn from_str(s: &str) -> Option<Priority> {
        match s.to_lowercase().as_str() {
            "critical" => Some(Priority::Critical),
            "high" => Some(Priority::High),
            "medium" => Some(Priority::Medium),
            "low" => Some(Priority::Low),
            _ => None,
        }
    }
}

/// ElementType represents the type of a kanban element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Sprint,
    Story,
    Task,
    Idea,
    Issue,
    Tips,
}

impl ElementType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ElementType::Sprint => "sprint",
            ElementType::Story => "story",
            ElementType::Task => "task",
            ElementType::Idea => "idea",
            ElementType::Issue => "issue",
            ElementType::Tips => "tips",
        }
    }

    pub fn from_str(s: &str) -> Option<ElementType> {
        match s.to_lowercase().as_str() {
            "sprint" => Some(ElementType::Sprint),
            "story" => Some(ElementType::Story),
            "task" => Some(ElementType::Task),
            "idea" => Some(ElementType::Idea),
            "issue" => Some(ElementType::Issue),
            "tips" | "tip" => Some(ElementType::Tips), // Accept both
            _ => None,
        }
    }
}

/// ElementId is a unique identifier for kanban elements
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementId(String);

impl ElementId {
    /// Creates a new ElementId from a type and number
    pub fn new(element_type: ElementType, number: u32) -> Self {
        ElementId(format!("{}-{:03}", element_type.as_str(), number))
    }

    /// Parses an ElementId from a string (e.g., "sprint-001", "task-042")
    pub fn parse(s: &str) -> Result<Self, ElementIdParseError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(ElementIdParseError::InvalidFormat(s.to_string()));
        }

        let type_str = parts[0];
        let num_str = parts[1];

        let _element_type = ElementType::from_str(type_str)
            .ok_or(ElementIdParseError::InvalidType(type_str.to_string()))?;

        let number = num_str
            .parse::<u32>()
            .map_err(|_| ElementIdParseError::InvalidNumber(num_str.to_string()))?;

        Ok(ElementId(format!("{}-{:03}", type_str, number)))
    }

    /// Returns the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the numeric portion of the ID
    pub fn number(&self) -> u32 {
        let parts: Vec<&str> = self.0.split('-').collect();
        parts[1].parse().unwrap_or(0)
    }

    /// Returns the type portion of the ID
    pub fn type_(&self) -> ElementType {
        let parts: Vec<&str> = self.0.split('-').collect();
        ElementType::from_str(parts[0]).unwrap_or(ElementType::Task)
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Hash for ElementId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Serialize for ElementId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ElementId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ElementId::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for ElementId {
    type Err = ElementIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ElementId::parse(s)
    }
}

/// Error type for ElementId parsing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementIdParseError {
    InvalidFormat(String),
    InvalidType(String),
    InvalidNumber(String),
}

impl fmt::Display for ElementIdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ElementIdParseError::InvalidFormat(s) => write!(f, "invalid element ID format: {}", s),
            ElementIdParseError::InvalidType(s) => write!(f, "invalid element type: {}", s),
            ElementIdParseError::InvalidNumber(s) => write!(f, "invalid element number: {}", s),
        }
    }
}

impl std::error::Error for ElementIdParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_transitions() {
        assert!(Status::Plan.can_transition_to(&Status::Backlog));
        assert!(!Status::Plan.can_transition_to(&Status::Done));
    }

    #[test]
    fn test_element_id_parse_and_access() {
        let id = ElementId::parse("sprint-001").unwrap();
        assert_eq!(id.number(), 1);
        assert_eq!(id.type_(), ElementType::Sprint);
    }
}
