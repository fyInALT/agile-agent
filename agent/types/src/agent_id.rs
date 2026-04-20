use serde::{Deserialize, Serialize};

/// Unique identifier for an agent
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Unique identifier for a workplace
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkplaceId(String);

impl WorkplaceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Short codename for an agent (e.g., "alpha", "beta")
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentCodename(String);

impl AgentCodename {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_new_and_as_str() {
        let id = AgentId::new("agent-001");
        assert_eq!(id.as_str(), "agent-001");
    }

    #[test]
    fn workplace_id_new_and_as_str() {
        let id = WorkplaceId::new("workplace-abc");
        assert_eq!(id.as_str(), "workplace-abc");
    }

    #[test]
    fn agent_codename_new_and_as_str() {
        let name = AgentCodename::new("alpha");
        assert_eq!(name.as_str(), "alpha");
    }

    #[test]
    fn agent_id_serialization() {
        let id = AgentId::new("test");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test\"");
        let parsed: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn workplace_id_serialization_roundtrip() {
        let id = WorkplaceId::new("wp-123");
        let json = serde_json::to_string(&id).unwrap();
        let parsed: WorkplaceId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_str(), "wp-123");
    }

    #[test]
    fn agent_codename_serialization_roundtrip() {
        let name = AgentCodename::new("beta");
        let json = serde_json::to_string(&name).unwrap();
        let parsed: AgentCodename = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_str(), "beta");
    }

    #[test]
    fn agent_id_empty_string() {
        let id = AgentId::new("");
        assert_eq!(id.as_str(), "");
    }

    #[test]
    fn agent_id_special_characters() {
        let id = AgentId::new("agent-with-dashes_and_underscores");
        assert_eq!(id.as_str(), "agent-with-dashes_and_underscores");
        let json = serde_json::to_string(&id).unwrap();
        let parsed: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn agent_id_hash_consistency() {
        let id1 = AgentId::new("same-id");
        let id2 = AgentId::new("same-id");
        let id3 = AgentId::new("different-id");

        // Same IDs should hash to same value
        use std::collections::HashSet;
        let set: HashSet<AgentId> = [id1.clone(), id2.clone(), id3].into_iter().collect();
        assert_eq!(set.len(), 2); // "same-id" and "different-id"
    }

    #[test]
    fn workplace_id_equality() {
        let id1 = WorkplaceId::new("wp-1");
        let id2 = WorkplaceId::new("wp-1");
        let id3 = WorkplaceId::new("wp-2");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}