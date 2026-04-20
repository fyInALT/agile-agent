//! State snapshot types for `session.initialize` response

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SessionState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SessionState {
    pub session_id: String,
    pub alias: String,
    pub server_time: String,
    pub last_event_seq: u64,
    pub app_state: AppStateSnapshot,
    pub agents: Vec<AgentSnapshot>,
    pub workplace: WorkplaceSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_agent_id: Option<String>,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppStateSnapshot {
    pub transcript: Vec<TranscriptItem>,
    pub input: InputState,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct InputState {
    pub text: String,
    pub multiline: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    #[default]
    Idle,
    Running,
    WaitingForApproval,
}

// ---------------------------------------------------------------------------
// Transcript
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TranscriptItem {
    pub id: String,
    pub kind: ItemKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    #[default]
    UserInput,
    AssistantOutput,
    ToolCall,
    ToolResult,
    SystemMessage,
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentSnapshot {
    pub id: String,
    pub codename: String,
    pub role: String,
    pub provider: String,
    pub status: AgentSlotStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentSlotStatus {
    #[default]
    Idle,
    Running,
    Stopped,
    Error,
}

// ---------------------------------------------------------------------------
// Workplace
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorkplaceSnapshot {
    pub id: String,
    pub path: String,
    pub backlog: BacklogSnapshot,
    pub skills: Vec<SkillSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BacklogSnapshot {
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SkillSnapshot {
    pub name: String,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_state_serialization() {
        let state = SessionState {
            session_id: "sess-1".to_string(),
            alias: "test".to_string(),
            server_time: "2026-04-20T14:30:00Z".to_string(),
            last_event_seq: 42,
            app_state: AppStateSnapshot {
                transcript: vec![TranscriptItem {
                    id: "item-1".to_string(),
                    kind: ItemKind::AssistantOutput,
                    agent_id: Some("a1".to_string()),
                    content: "Hello".to_string(),
                    metadata: serde_json::Value::Null,
                    created_at: "2026-04-20T14:30:00Z".to_string(),
                    completed_at: None,
                }],
                input: InputState {
                    text: "".to_string(),
                    multiline: false,
                },
                status: SessionStatus::Idle,
            },
            agents: vec![AgentSnapshot {
                id: "a1".to_string(),
                codename: "dev".to_string(),
                role: "Developer".to_string(),
                provider: "claude".to_string(),
                status: AgentSlotStatus::Idle,
                current_task_id: None,
                uptime_seconds: 120,
            }],
            workplace: WorkplaceSnapshot {
                id: "wp-1".to_string(),
                path: "/home/user/project".to_string(),
                backlog: BacklogSnapshot { items: vec![] },
                skills: vec![SkillSnapshot {
                    name: "rust".to_string(),
                    enabled: true,
                }],
            },
            focused_agent_id: None,
            protocol_version: "1.0.0".to_string(),
            capabilities: vec!["session.initialize".to_string()],
        };
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["session_id"], "sess-1");
        assert_eq!(json["protocol_version"], "1.0.0");
        assert!(json.get("focused_agent_id").is_none());
    }

    #[test]
    fn transcript_item_optional_fields_omitted() {
        let item = TranscriptItem {
            id: "item-1".to_string(),
            kind: ItemKind::UserInput,
            agent_id: None,
            content: "hello".to_string(),
            metadata: serde_json::Value::Null,
            created_at: "2026-04-20T14:30:00Z".to_string(),
            completed_at: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert!(json.get("agent_id").is_none());
        assert!(json.get("completed_at").is_none());
    }

    #[test]
    fn agent_snapshot_serialization() {
        let agent = AgentSnapshot {
            id: "a1".to_string(),
            codename: "claude-dev".to_string(),
            role: "Developer".to_string(),
            provider: "claude".to_string(),
            status: AgentSlotStatus::Running,
            current_task_id: Some("task-1".to_string()),
            uptime_seconds: 300,
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert_eq!(json["status"], "running");
        assert_eq!(json["uptime_seconds"], 300);
    }
}
