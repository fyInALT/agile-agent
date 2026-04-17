use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::agent_runtime::AgentRuntime;
use crate::app::AppState;
use crate::app::TranscriptEntry;
use crate::tool_calls::ExecCommandStatus;
use crate::tool_calls::McpToolCallStatus;
use crate::tool_calls::PatchApplyStatus;
use crate::tool_calls::PatchChange;
use crate::tool_calls::WebSearchAction;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageKind {
    User,
    Assistant,
    Thinking,
    ToolCall,
    Decision,
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
            source,
            allow_exploring_group: _,
            input_preview,
            output_preview,
            status,
            exit_code,
            duration_ms,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: if matches!(status, ExecCommandStatus::InProgress) {
                agent_endpoint.clone()
            } else {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "exec_command".to_string(),
                }
            },
            recipient: if matches!(status, ExecCommandStatus::InProgress) {
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
                source.as_deref().unwrap_or(""),
                exec_command_status_label(*status),
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
            changes,
            status,
            output_preview: _,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: if matches!(status, PatchApplyStatus::InProgress) {
                agent_endpoint.clone()
            } else {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "patch_apply".to_string(),
                }
            },
            recipient: if matches!(status, PatchApplyStatus::InProgress) {
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
                "patch_apply:{}:{}",
                patch_apply_status_label(*status),
                summarize_patch_changes(changes),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::WebSearch {
            call_id,
            query,
            action,
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
                    id: "web_search".to_string(),
                }
            },
            recipient: if *started {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: "web_search".to_string(),
                }
            } else {
                agent_endpoint
            },
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!(
                "web_search:{}:{}:{}",
                started,
                query,
                summarize_web_search_action(action),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::ViewImage { call_id, path } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Tool,
                id: "view_image".to_string(),
            },
            recipient: agent_endpoint,
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!("view_image:{path}"),
            created_at: captured_at,
        },
        TranscriptEntry::ImageGeneration {
            call_id,
            revised_prompt,
            result,
            saved_path,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Tool,
                id: "image_generation".to_string(),
            },
            recipient: agent_endpoint,
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!(
                "image_generation:{}:{}:{}",
                revised_prompt.as_deref().unwrap_or(""),
                result.as_deref().unwrap_or(""),
                saved_path.as_deref().unwrap_or(""),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::McpToolCall {
            call_id,
            invocation,
            result_blocks,
            error,
            status,
            is_error: _,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Tooling,
            sender: if matches!(status, McpToolCallStatus::InProgress) {
                agent_endpoint.clone()
            } else {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: format!("{}.{}", invocation.server, invocation.tool),
                }
            },
            recipient: if matches!(status, McpToolCallStatus::InProgress) {
                AgentMessageEndpoint {
                    kind: AgentMessageEndpointKind::Tool,
                    id: format!("{}.{}", invocation.server, invocation.tool),
                }
            } else {
                agent_endpoint
            },
            kind: AgentMessageKind::ToolCall,
            correlation_id: call_id.clone(),
            summary: format!(
                "mcp_tool_call:{}:{}:{}:{}:{}",
                mcp_tool_call_status_label(*status),
                invocation.server,
                invocation.tool,
                error.as_deref().unwrap_or(""),
                result_blocks.len(),
            ),
            created_at: captured_at,
        },
        TranscriptEntry::GenericToolCall {
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
        TranscriptEntry::Decision {
            agent_id,
            situation_type,
            action_type,
            reasoning,
            confidence,
            tier,
        } => AgentMessageEnvelope {
            sequence,
            direction: AgentMessageDirection::Internal,
            channel: AgentMessageChannel::Runtime,
            sender: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Agent,
                id: format!("decision-{}", agent_id),
            },
            recipient: AgentMessageEndpoint {
                kind: AgentMessageEndpointKind::Agent,
                id: agent_id.clone(),
            },
            kind: AgentMessageKind::Decision,
            correlation_id: None,
            summary: format!(
                "decision:{}:{}:{}:{}%:{}",
                tier, situation_type, action_type, confidence, reasoning
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

fn summarize_patch_changes(changes: &[PatchChange]) -> String {
    changes
        .iter()
        .map(|change| format!("{} (+{} -{})", change.path, change.added, change.removed))
        .collect::<Vec<_>>()
        .join("|")
}

fn patch_apply_status_label(status: PatchApplyStatus) -> &'static str {
    match status {
        PatchApplyStatus::InProgress => "in_progress",
        PatchApplyStatus::Completed => "completed",
        PatchApplyStatus::Failed => "failed",
        PatchApplyStatus::Declined => "declined",
    }
}

fn exec_command_status_label(status: ExecCommandStatus) -> &'static str {
    match status {
        ExecCommandStatus::InProgress => "in_progress",
        ExecCommandStatus::Completed => "completed",
        ExecCommandStatus::Failed => "failed",
        ExecCommandStatus::Declined => "declined",
    }
}

fn summarize_web_search_action(action: &Option<WebSearchAction>) -> String {
    match action {
        Some(WebSearchAction::Search { query, queries }) => query
            .clone()
            .or_else(|| queries.as_ref().and_then(|items| items.first().cloned())),
        Some(WebSearchAction::OpenPage { url }) => url.clone(),
        Some(WebSearchAction::FindInPage { url, pattern }) => {
            match (pattern.as_deref(), url.as_deref()) {
                (Some(pattern), Some(url)) => Some(format!("{pattern} in {url}")),
                (Some(pattern), None) => Some(pattern.to_string()),
                (None, Some(url)) => Some(url.to_string()),
                (None, None) => None,
            }
        }
        Some(WebSearchAction::Other) | None => None,
    }
    .unwrap_or_default()
}

fn mcp_tool_call_status_label(status: McpToolCallStatus) -> &'static str {
    match status {
        McpToolCallStatus::InProgress => "in_progress",
        McpToolCallStatus::Completed => "completed",
        McpToolCallStatus::Failed => "failed",
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
