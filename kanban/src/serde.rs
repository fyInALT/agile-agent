//! ElementSerde serialization proxy for trait objects
//!
//! Enables serialization of Box<dyn KanbanElementTrait> through a proxy struct.

use serde::{Deserialize, Serialize};

/// Serializable representation of any kanban element
///
/// This proxy struct enables serialization of trait objects
/// by capturing all relevant fields into a serializable format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementSerde {
    /// Element type identifier (e.g., "task", "sprint", "story")
    pub element_type: String,
    /// Element title
    pub title: String,
    /// Element content/description
    #[serde(default)]
    pub content: String,
    /// Current status
    pub status: String,
    /// Element ID (if assigned)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Priority level
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    /// Effort/story points
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<u32>,
    /// Assignee
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// Blocked reason (if blocked)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Parent element ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl ElementSerde {
    /// Create from element data (called by KanbanElementTrait::to_serde)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        element_type: String,
        title: String,
        content: String,
        status: String,
        id: Option<String>,
        priority: Option<String>,
        effort: Option<u32>,
        assignee: Option<String>,
        blocked_reason: Option<String>,
        tags: Vec<String>,
        dependencies: Vec<String>,
        parent: Option<String>,
    ) -> Self {
        Self {
            element_type,
            title,
            content,
            status,
            id,
            priority,
            effort,
            assignee,
            blocked_reason,
            tags,
            dependencies,
            parent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_serde_serialization() {
        let serde = ElementSerde {
            element_type: "task".to_string(),
            title: "Test Task".to_string(),
            content: "".to_string(),
            status: "plan".to_string(),
            id: None,
            priority: Some("medium".to_string()),
            effort: None,
            assignee: None,
            blocked_reason: None,
            tags: vec![],
            dependencies: vec![],
            parent: None,
        };

        let json = serde_json::to_string(&serde).unwrap();
        assert!(json.contains("\"element_type\":\"task\""));
        assert!(json.contains("\"title\":\"Test Task\""));
    }
}
