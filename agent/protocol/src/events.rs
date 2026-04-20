//! Event types for daemon → client notifications

use crate::state::{AgentSlotStatus, ItemKind, TranscriptItem};
use serde::{Deserialize, Serialize};

/// An event broadcast by the daemon to all connected clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub seq: u64,
    #[serde(flatten)]
    pub payload: EventPayload,
}

/// The payload of an event, tagged by type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase", content = "data")]
pub enum EventPayload {
    AgentSpawned(AgentSpawnedData),
    AgentStopped(AgentStoppedData),
    AgentStatusChanged(AgentStatusChangedData),
    ItemStarted(ItemStartedData),
    ItemDelta(ItemDeltaData),
    ItemCompleted(ItemCompletedData),
    MailReceived(MailReceivedData),
    ApprovalRequest(ApprovalRequestData),
    ApprovalResponse(ApprovalResponseData),
    Error(ErrorData),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSpawnedData {
    pub agent_id: String,
    pub codename: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentStoppedData {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatusChangedData {
    pub agent_id: String,
    pub status: AgentSlotStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedData {
    pub item_id: String,
    pub kind: ItemKind,
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemDeltaData {
    pub item_id: String,
    pub delta: ItemDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompletedData {
    pub item_id: String,
    pub item: TranscriptItem,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MailReceivedData {
    pub to: String,
    pub from: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorData {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequestData {
    pub request_id: String,
    pub agent_id: String,
    pub title: String,
    pub description: String,
    pub options: Vec<ApprovalOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalResponseData {
    pub request_id: String,
    pub selected_option_id: String,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "text")]
pub enum ItemDelta {
    #[serde(rename = "text")]
    Text(String),
    #[serde(rename = "markdown")]
    Markdown(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_serialization_matches_spec() {
        let event = Event {
            seq: 42,
            payload: EventPayload::AgentSpawned(AgentSpawnedData {
                agent_id: "a1".to_string(),
                codename: "dev".to_string(),
                role: "Developer".to_string(),
            }),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["seq"], 42);
        assert_eq!(json["type"], "agentSpawned");
        assert_eq!(json["data"]["agentId"], "a1");
        assert_eq!(json["data"]["codename"], "dev");
        assert_eq!(json["data"]["role"], "Developer");
    }

    #[test]
    fn item_delta_text_serialization() {
        let delta = ItemDelta::Text("Hello".to_string());
        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "Hello");
    }

    #[test]
    fn event_seq_monotonic_in_constructor() {
        // Events themselves don't enforce monotonicity — the daemon does.
        // This test simply verifies seq is stored correctly.
        let e1 = Event {
            seq: 1,
            payload: EventPayload::Error(ErrorData {
                message: "test".to_string(),
                source: None,
            }),
        };
        assert_eq!(e1.seq, 1);
    }

    #[test]
    fn agent_status_changed_serialization() {
        let event = Event {
            seq: 3,
            payload: EventPayload::AgentStatusChanged(AgentStatusChangedData {
                agent_id: "a1".to_string(),
                status: AgentSlotStatus::Running,
            }),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "agentStatusChanged");
        assert_eq!(json["data"]["status"], "running");
    }

    #[test]
    fn item_started_serialization() {
        let event = Event {
            seq: 5,
            payload: EventPayload::ItemStarted(ItemStartedData {
                item_id: "item-1".to_string(),
                kind: ItemKind::ToolCall,
                agent_id: "a1".to_string(),
            }),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "itemStarted");
        assert_eq!(json["data"]["kind"], "tool_call");
    }
}
