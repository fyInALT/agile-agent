//! Method parameters and result types for client → daemon requests

use agent_types::{AgentRole, ProviderKind};
use serde::{Deserialize, Serialize};

// ==========================================================================
// Client method enum (for strongly-typed dispatch)
// ==========================================================================

/// All methods that a client can invoke on the daemon.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientMethod {
    SessionInitialize(InitializeParams),
    SessionHeartbeat,
    SessionSendInput(SendInputParams),
    SessionSetFocus(SetFocusParams),
    AgentSpawn(AgentSpawnParams),
    AgentStop(AgentStopParams),
    AgentList(AgentListParams),
    ToolApprove(ToolApproveParams),
    DecisionRespond(DecisionRespondParams),
}

/// Returns the wire-format method name and serialized params for a method.
pub fn method_name_and_params(method: &ClientMethod) -> (&'static str, serde_json::Value) {
    match method {
        ClientMethod::SessionInitialize(p) => ("session.initialize", serde_json::to_value(p).unwrap()),
        ClientMethod::SessionHeartbeat => ("session.heartbeat", serde_json::json!({})),
        ClientMethod::SessionSendInput(p) => ("session.sendInput", serde_json::to_value(p).unwrap()),
        ClientMethod::SessionSetFocus(p) => ("session.setFocus", serde_json::to_value(p).unwrap()),
        ClientMethod::AgentSpawn(p) => ("agent.spawn", serde_json::to_value(p).unwrap()),
        ClientMethod::AgentStop(p) => ("agent.stop", serde_json::to_value(p).unwrap()),
        ClientMethod::AgentList(p) => ("agent.list", serde_json::to_value(p).unwrap()),
        ClientMethod::ToolApprove(p) => ("tool.approve", serde_json::to_value(p).unwrap()),
        ClientMethod::DecisionRespond(p) => ("decision.respond", serde_json::to_value(p).unwrap()),
    }
}

// ==========================================================================
// Param types
// ==========================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InitializeParams {
    pub client_type: ClientType,
    pub client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Tui,
    Cli,
    Ide,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SendInputParams {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetFocusParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentSpawnParams {
    pub provider: ProviderKind,
    pub role: AgentRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentStopParams {
    pub agent_id: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentListParams {
    #[serde(default)]
    pub include_stopped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolApproveParams {
    pub request_id: String,
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifications: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionRespondParams {
    pub request_id: String,
    pub choice: DecisionChoice,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LoadHistoryParams {
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LoadHistoryResult {
    pub items: Vec<crate::state::TranscriptItem>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ForceSnapshotParams {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionChoice {
    Approve,
    Reject,
    Escalate,
}

// ==========================================================================
// Result types
// ==========================================================================

use crate::state::{AgentSnapshot, SessionState};

pub type InitializeResult = SessionState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SendInputResult {
    pub accepted: bool,
    pub item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetFocusResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_agent_id: Option<String>,
}

pub type AgentSpawnResult = AgentSnapshot;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStopResult {
    pub stopped: bool,
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentListResult {
    pub agents: Vec<AgentSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolApproveResult {
    pub resolved: bool,
    pub request_id: String,
}

pub type DecisionRespondResult = ToolApproveResult;

// ==========================================================================
// Server-initiated notification types
// ==========================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovalRequestParams {
    pub request_id: String,
    pub agent_id: String,
    pub tool: String,
    pub preview: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionRequestParams {
    pub request_id: String,
    pub situation: String,
    pub options: Vec<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HeartbeatAckParams {
    pub server_time: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_types::{AgentRole, ProviderKind};

    #[test]
    fn method_name_mapping() {
        let m = ClientMethod::SessionInitialize(InitializeParams {
            client_type: ClientType::Tui,
            client_version: "0.9.0".to_string(),
            resume_snapshot_id: None,
            protocol_version: None,
        });
        let (name, _) = method_name_and_params(&m);
        assert_eq!(name, "session.initialize");
    }

    #[test]
    fn initialize_params_serialization() {
        let params = InitializeParams {
            client_type: ClientType::Cli,
            client_version: "1.0.0".to_string(),
            resume_snapshot_id: Some("snap-1".to_string()),
            protocol_version: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["client_type"], "cli");
        assert_eq!(json["client_version"], "1.0.0");
        assert_eq!(json["resume_snapshot_id"], "snap-1");
        assert!(json.get("protocol_version").is_none());
    }

    #[test]
    fn agent_spawn_params_serialization() {
        let params = AgentSpawnParams {
            provider: ProviderKind::Claude,
            role: AgentRole::Developer,
            codename: Some("dev-1".to_string()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["provider"], "claude");
        assert_eq!(json["role"], "developer");
        assert_eq!(json["codename"], "dev-1");
    }

    #[test]
    fn decision_choice_serialization() {
        let choice = DecisionChoice::Escalate;
        let json = serde_json::to_string(&choice).unwrap();
        assert_eq!(json, "\"escalate\"");
    }

    #[test]
    fn tool_approve_result_round_trip() {
        let result = ToolApproveResult {
            resolved: true,
            request_id: "apr-1".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ToolApproveResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.resolved);
        assert_eq!(parsed.request_id, "apr-1");
    }
}
