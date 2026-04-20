use serde::{Deserialize, Serialize};

/// Status of an agent in the runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Running,
    Stopped,
}

impl AgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_status_labels() {
        assert_eq!(AgentStatus::Idle.label(), "idle");
        assert_eq!(AgentStatus::Running.label(), "running");
        assert_eq!(AgentStatus::Stopped.label(), "stopped");
    }

    #[test]
    fn agent_status_serialization() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }
}