use serde::{Deserialize, Serialize};

/// Status of a worker in the runtime (protocol-level view).
///
/// This is a coarse-grained status used in external protocol messages.
/// For the full runtime state machine, see `WorkerState` in `agent-runtime-domain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Idle,
    Running,
    Stopped,
}

impl WorkerStatus {
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
        assert_eq!(WorkerStatus::Idle.label(), "idle");
        assert_eq!(WorkerStatus::Running.label(), "running");
        assert_eq!(WorkerStatus::Stopped.label(), "stopped");
    }

    #[test]
    fn agent_status_serialization() {
        let status = WorkerStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }
}