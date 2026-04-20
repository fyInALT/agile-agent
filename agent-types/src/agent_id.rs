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
}