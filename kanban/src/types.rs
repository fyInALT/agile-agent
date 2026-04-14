//! Core type identifiers for trait-based kanban architecture
//!
//! String-based identifiers for extensible types, replacing fixed enums.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

/// StatusType - extensible status identifier
///
/// Replaces fixed Status enum with string-based identifier,
/// enabling custom statuses without code modification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StatusType {
    name: String,
}

impl StatusType {
    /// Create a new StatusType from a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
        }
    }

    /// Get the status name
    pub fn name(&self) -> &str {
        &self.name
    }
}

// Custom serialization: serialize as just the string name
impl Serialize for StatusType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.name)
    }
}

// Custom deserialization: deserialize from string
impl<'de> Deserialize<'de> for StatusType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(StatusType::new(s))
    }
}

impl fmt::Display for StatusType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl FromStr for StatusType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(StatusType::new(s))
    }
}

/// ElementTypeIdentifier - extensible element type identifier
///
/// Replaces fixed ElementType enum with string-based identifier,
/// enabling custom element types without code modification.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementTypeIdentifier {
    name: String,
}

impl ElementTypeIdentifier {
    /// Create a new ElementTypeIdentifier from a name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
        }
    }

    /// Get the element type name
    pub fn name(&self) -> &str {
        &self.name
    }
}

// Custom serialization: serialize as just the string name
impl Serialize for ElementTypeIdentifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.name)
    }
}

// Custom deserialization: deserialize from string
impl<'de> Deserialize<'de> for ElementTypeIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(ElementTypeIdentifier::new(s))
    }
}

impl fmt::Display for ElementTypeIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl FromStr for ElementTypeIdentifier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ElementTypeIdentifier::new(s))
    }
}

/// Builtin status types - pre-defined for common kanban workflow
pub mod builtin_statuses {
    use super::StatusType;

    /// Plan status - initial planning phase
    pub fn plan() -> StatusType {
        StatusType::new("plan")
    }

    /// Backlog status - ready to be scheduled
    pub fn backlog() -> StatusType {
        StatusType::new("backlog")
    }

    /// Blocked status - cannot proceed
    pub fn blocked() -> StatusType {
        StatusType::new("blocked")
    }

    /// Ready status - ready to start
    pub fn ready() -> StatusType {
        StatusType::new("ready")
    }

    /// Todo status - scheduled for work
    pub fn todo() -> StatusType {
        StatusType::new("todo")
    }

    /// InProgress status - actively being worked on
    pub fn in_progress() -> StatusType {
        StatusType::new("in_progress")
    }

    /// Done status - completed
    pub fn done() -> StatusType {
        StatusType::new("done")
    }

    /// Verified status - verified and accepted (terminal)
    pub fn verified() -> StatusType {
        StatusType::new("verified")
    }

    /// All builtin statuses
    pub fn all() -> Vec<StatusType> {
        vec![
            plan(),
            backlog(),
            blocked(),
            ready(),
            todo(),
            in_progress(),
            done(),
            verified(),
        ]
    }
}

/// Builtin element type identifiers
pub mod builtin_element_types {
    use super::ElementTypeIdentifier;

    /// Sprint element type
    pub fn sprint() -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("sprint")
    }

    /// Story element type
    pub fn story() -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("story")
    }

    /// Task element type
    pub fn task() -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("task")
    }

    /// Idea element type
    pub fn idea() -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("idea")
    }

    /// Issue element type
    pub fn issue() -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("issue")
    }

    /// Tips element type
    pub fn tips() -> ElementTypeIdentifier {
        ElementTypeIdentifier::new("tips")
    }

    /// All builtin element types
    pub fn all() -> Vec<ElementTypeIdentifier> {
        vec![sprint(), story(), task(), idea(), issue(), tips()]
    }
}

#[cfg(test)]
mod tests {
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
    fn test_status_type_serialization() {
        let status = StatusType::new("in_progress");
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"in_progress\"");

        let parsed: StatusType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, status);
    }

    #[test]
    fn test_status_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(StatusType::new("plan"));
        set.insert(StatusType::new("plan")); // Duplicate
        set.insert(StatusType::new("backlog"));
        assert_eq!(set.len(), 2);
    }

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
    fn test_element_type_serialization() {
        let type_id = ElementTypeIdentifier::new("sprint");
        let json = serde_json::to_string(&type_id).unwrap();
        assert_eq!(json, "\"sprint\"");

        let parsed: ElementTypeIdentifier = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, type_id);
    }

    #[test]
    fn test_element_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ElementTypeIdentifier::new("task"));
        set.insert(ElementTypeIdentifier::new("task")); // Duplicate
        set.insert(ElementTypeIdentifier::new("story"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_builtin_statuses() {
        assert_eq!(builtin_statuses::plan().name(), "plan");
        assert_eq!(builtin_statuses::verified().name(), "verified");
        assert_eq!(builtin_statuses::all().len(), 8);
    }

    #[test]
    fn test_builtin_element_types() {
        assert_eq!(builtin_element_types::task().name(), "task");
        assert_eq!(builtin_element_types::story().name(), "story");
        assert_eq!(builtin_element_types::all().len(), 6);
    }
}