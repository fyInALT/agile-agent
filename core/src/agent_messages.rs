use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_runtime::AgentRuntime;
use crate::app::AppState;
use crate::app::TranscriptEntry;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageKind {
    User,
    Assistant,
    Thinking,
    ToolCall,
    Status,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageDirection {
    Inbound,
    Outbound,
    Internal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageChannel {
    Interaction,
    Runtime,
    Tooling,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageEndpointKind {
    Operator,
    Agent,
    Provider,
    Tool,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMessageEndpoint {
    pub kind: AgentMessageEndpointKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMessageEnvelope {
    pub sequence: usize,
    pub direction: AgentMessageDirection,
    pub channel: AgentMessageChannel,
    pub sender: AgentMessageEndpoint,
    pub recipient: AgentMessageEndpoint,
    pub kind: AgentMessageKind,
    pub correlation_id: Option<String>,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMessages {
    pub entries: Vec<AgentMessageEnvelope>,
}

impl AgentMessages {
    pub fn from_runtime_and_app(runtime: &AgentRuntime, state: &AppState) -> Self {
        let captured_at = Utc::now().to_rfc3339();
        let agent_endpoint = AgentMessageEndpoint {
            kind: AgentMessageEndpointKind::Agent,
            id: runtime.agent_id().as_str().to_string(),
        };
        let provider_endpoint = AgentMessageEndpoint {
            kind: AgentMessageEndpointKind::Provider,
            id: runtime.meta().provider_type.label().to_string(),
        };

        let entries = state
            .transcript
            .iter()
            .enumerate()
            .map(|(sequence, entry)| {
                map_entry(
                    entry,
                    sequence,
                    captured_at.clone(),
                    agent_endpoint.clone(),
                    provider_endpoint.clone(),
                )
            })
            .collect();
        Self { entries }
    }
}

fn map_entry(
    entry: &TranscriptEntry,
    sequence: usize,
    captured_at: String,
    agent_endpoint: AgentMessageEndpoint,
    provider_endpoint: AgentMessageEndpoint,
) -> AgentMessageEnvelope {
    match entry {
        TranscriptEntry::User(text) => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Inbound,
            channel: AgentMessageChannel::Interaction,
            sender: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Operator,
                id: "operator".to_string(),
            },
            recipient: agent_endpoint,
            kind: AgentMessageKind::User,
            correlation_id: None,
            summary: text.clone(),
            created_at: captured_at,
        },
        TranscriptEntry::Assistant(text) => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Outbound,
            channel: AgentMessageChannel::Interaction,
            sender: agent_endpoint,
            recipient: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Operator,
                id: "operator".to_string(),
            },
            kind: AgentMessageKind::Assistant,
            correlation_id: None,
            summary: text.clone(),
            created_at: captured_at,
        },
        TranscriptEntry::Thinking(text) => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Runtime,
            sender: agent_endpoint.clone(),
            recipient: agent_endpoint,
            kind: AgentMessageKind::Thinking,
            correlation_id: None,
            summary: text.clone(),
            created_at: captured_at,
        },
        TranscriptEntry::ExecCommand {
            call_id,
            input_preview,
            output_preview,
            success,
            started,
            exit_code,
            duration_ms,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: if *started {
                agent_endpoint.clone()
            } else {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "exec_command".to_string(),
                }
            },
            recipient: if *started {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "exec_command".to_string(),
                }
            } else {
                agent_endpoint
            },
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!(
                "exec_command:{}:{}:{}:{}:{}:{}",
                started,
                success,
                input_preview.as_deref().unwrap_or(""),
                output_preview.as_deref().unwrap_or(""),
                exit_code
                    .map(|value| value.to_string())
                    .as_deref()
                    .unwrap_or(""),
                duration_ms
                    .map(|value| value.to_string())
                    .as_deref()
                    .unwrap_or(""),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::PatchApply {
            call_id,
            summary_preview,
            success,
            started,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: if *started {
                agent_endpoint.clone()
            } else {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "patch_apply".to_string(),
                }
            },
            recipient: if *started {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "patch_apply".to_string(),
                }
            } else {
                agent_endpoint
            },
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!(
                "patch_apply:{}:{}:{}",
                started,
                success,
                summary_preview.as_deref().unwrap_or(""),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::ToolCall {
            name,
            call_id,
            input_preview,
            output_preview,
            started,
            success,
            exit_code,
            duration_ms,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: if *started {
                agent_endpoint.clone()
            } else {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: name.clone(),
                }
            },
            recipient: if *started {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: name.clone(),
                }
            } else {
                agent_endpoint
            },
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!(
                "{}:{}:{}:{}:{}:{}:{}",
                name,
                started,
                success,
                input_preview.as_deref().unwrap_or(""),
                output_preview.as_deref().unwrap_or(""),
                exit_code
                    .map(|value| value.to_string())
                    .as_deref()
                    .unwrap_or(""),
                duration_ms
                    .map(|value| value.to_string())
                    .as_deref()
                    .unwrap_or(""),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::Status(text) => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Outbound,
            channel: AgentMessageChannel::Runtime,
            sender: provider_endpoint,
            recipient: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Operator,
                id: "operator".to_string(),
            },
            kind: AgentMessageKind::Status,
            correlation_id: None,
            summary: text.clone(),
            created_at: captured_at,
        },
        TranscriptEntry::Error(text) => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Outbound,
            channel: AgentMessageChannel::Runtime,
            sender: provider_endpoint,
            recipient: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Operator,
                id: "operator".to_string(),
            },
            kind: AgentMessageKind::Error,
            correlation_id: None,
            summary: text.clone(),
            created_at: captured_at,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::AgentMessageChannel;
    use super::AgentMessageDirection;
    use super::AgentMessageEndpointKind;
    use super::AgentMessageKind;
    use super::AgentMessages;
    use crate::agent_runtime::AgentRuntime;
    use crate::app::AppState;
    use crate::app::TranscriptEntry;
    use crate::provider::ProviderKind;
    use crate::workplace_store::WorkplaceStore;
    use tempfile::TempDir;

    #[test]
    fn projects_transcript_into_message_envelopes() {
        let temp = TempDir::new().expect("tempdir");
        let workplace = WorkplaceStore::for_cwd(temp.path()).expect("workplace");
        let runtime = AgentRuntime::new(&workplace, 1, ProviderKind::Mock);
        let mut state = AppState::new(ProviderKind::Mock);
        state
            .transcript
            .push(TranscriptEntry::Status("hello".to_string()));

        let messages = AgentMessages::from_runtime_and_app(&runtime, &state);

        assert_eq!(messages.entries.len(), 1);
        assert_eq!(messages.entries[0].sequence, 0);
        assert_eq!(messages.entries[0].kind, AgentMessageKind::Status);
        assert_eq!(messages.entries[0].channel, AgentMessageChannel::Runtime);
        assert_eq!(
            messages.entries[0].direction,
            AgentMessageDirection::Outbound
        );
        assert_eq!(
            messages.entries[0].sender.kind,
            AgentMessageEndpointKind::Provider
        );
        assert_eq!(messages.entries[0].summary, "hello");
    }
}
