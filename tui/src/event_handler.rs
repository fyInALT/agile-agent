//! Apply daemon events to `TuiState`.

use agent_protocol::events::*;
use agent_protocol::state::{AgentSnapshot, TranscriptItem};

use crate::protocol_state::ProtocolState;

/// Apply a single daemon event to the TUI state.
///
/// This is pure logic — no async, no I/O — making it trivially testable.
#[allow(dead_code)] // Used in protocol-only mode
pub fn apply_event(state: &mut ProtocolState, event: &Event) {
    match &event.payload {
        EventPayload::AgentSpawned(data) => {
            state.agents.push(AgentSnapshot {
                id: data.agent_id.clone(),
                codename: data.codename.clone(),
                role: data.role.clone(),
                provider: "unknown".to_string(),
                status: agent_protocol::state::AgentSlotStatus::Idle,
                current_task_id: None,
                uptime_seconds: 0,
            });
        }
        EventPayload::AgentStopped(data) => {
            state.agents.retain(|a| a.id != data.agent_id);
            if state.focused_agent_id.as_ref() == Some(&data.agent_id) {
                state.focused_agent_id = None;
            }
        }
        EventPayload::AgentStatusChanged(data) => {
            if let Some(agent) = state.agents.iter_mut().find(|a| a.id == data.agent_id) {
                agent.status = data.status;
            }
        }
        EventPayload::ItemStarted(data) => {
            state.transcript_items.push(TranscriptItem {
                id: data.item_id.clone(),
                kind: data.kind,
                agent_id: Some(data.agent_id.clone()),
                content: String::new(),
                metadata: serde_json::Value::Null,
                created_at: chrono::Utc::now().to_rfc3339(),
                completed_at: None,
            });
        }
        EventPayload::ItemDelta(data) => {
            if let Some(item) = state.transcript_items.iter_mut().find(|i| i.id == data.item_id) {
                match &data.delta {
                    ItemDelta::Text(text) | ItemDelta::Markdown(text) => {
                        item.content.push_str(text);
                    }
                }
            }
        }
        EventPayload::ItemCompleted(data) => {
            if let Some(item) = state.transcript_items.iter_mut().find(|i| i.id == data.item_id) {
                item.content = data.item.content.clone();
                item.metadata = data.item.metadata.clone();
                item.completed_at = Some(chrono::Utc::now().to_rfc3339());
            }
        }
        EventPayload::MailReceived(_) => {
            // Mail display not yet implemented in TUI.
        }
        EventPayload::ApprovalRequest(data) => {
            state.pending_approvals.push(data.clone());
        }
        EventPayload::ApprovalResponse(data) => {
            state.pending_approvals.retain(|a| a.request_id != data.request_id);
        }
        EventPayload::Error(data) => {
            tracing::warn!("daemon error event: {}", data.message);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state() -> ProtocolState {
        ProtocolState::default()
    }

    #[test]
    fn apply_agent_spawned_adds_agent() {
        let mut state = empty_state();
        apply_event(
            &mut state,
            &Event {
                seq: 1,
                payload: EventPayload::AgentSpawned(AgentSpawnedData {
                    agent_id: "a1".to_string(),
                    codename: "alpha".to_string(),
                    role: "Developer".to_string(),
                }),
            },
        );
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.agents[0].id, "a1");
    }

    #[test]
    fn apply_agent_stopped_removes_agent_and_clears_focus() {
        let mut state = empty_state();
        state.agents.push(AgentSnapshot {
            id: "a1".to_string(),
            codename: "alpha".to_string(),
            role: "Developer".to_string(),
            provider: "mock".to_string(),
            status: agent_protocol::state::AgentSlotStatus::Idle,
            current_task_id: None,
            uptime_seconds: 0,
        });
        state.focused_agent_id = Some("a1".to_string());

        apply_event(
            &mut state,
            &Event {
                seq: 1,
                payload: EventPayload::AgentStopped(AgentStoppedData {
                    agent_id: "a1".to_string(),
                    reason: None,
                }),
            },
        );

        assert!(state.agents.is_empty());
        assert!(state.focused_agent_id.is_none());
    }

    #[test]
    fn apply_item_delta_appends_text() {
        let mut state = empty_state();
        state.transcript_items.push(TranscriptItem {
            id: "item-1".to_string(),
            kind: agent_protocol::state::ItemKind::AssistantOutput,
            agent_id: Some("a1".to_string()),
            content: "Hello".to_string(),
            metadata: serde_json::Value::Null,
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
        });

        apply_event(
            &mut state,
            &Event {
                seq: 1,
                payload: EventPayload::ItemDelta(ItemDeltaData {
                    item_id: "item-1".to_string(),
                    delta: ItemDelta::Text(" world".to_string()),
                }),
            },
        );

        assert_eq!(state.transcript_items[0].content, "Hello world");
    }

    #[test]
    fn event_sequence_rebuilds_transcript() {
        let mut state = empty_state();

        // 1. Agent spawns
        apply_event(
            &mut state,
            &Event {
                seq: 1,
                payload: EventPayload::AgentSpawned(AgentSpawnedData {
                    agent_id: "a1".to_string(),
                    codename: "alpha".to_string(),
                    role: "Developer".to_string(),
                }),
            },
        );

        // 2. Item starts
        apply_event(
            &mut state,
            &Event {
                seq: 2,
                payload: EventPayload::ItemStarted(ItemStartedData {
                    item_id: "item-1".to_string(),
                    kind: agent_protocol::state::ItemKind::AssistantOutput,
                    agent_id: "a1".to_string(),
                }),
            },
        );

        // 3. Delta arrives
        apply_event(
            &mut state,
            &Event {
                seq: 3,
                payload: EventPayload::ItemDelta(ItemDeltaData {
                    item_id: "item-1".to_string(),
                    delta: ItemDelta::Text("Hello".to_string()),
                }),
            },
        );

        apply_event(
            &mut state,
            &Event {
                seq: 4,
                payload: EventPayload::ItemDelta(ItemDeltaData {
                    item_id: "item-1".to_string(),
                    delta: ItemDelta::Markdown(" **world**".to_string()),
                }),
            },
        );

        // 4. Item completes
        apply_event(
            &mut state,
            &Event {
                seq: 5,
                payload: EventPayload::ItemCompleted(ItemCompletedData {
                    item_id: "item-1".to_string(),
                    item: TranscriptItem {
                        id: "item-1".to_string(),
                        kind: agent_protocol::state::ItemKind::AssistantOutput,
                        agent_id: Some("a1".to_string()),
                        content: "Hello **world**".to_string(),
                        metadata: serde_json::json!({"final": true}),
                        created_at: String::new(),
                        completed_at: Some(chrono::Utc::now().to_rfc3339()),
                    },
                }),
            },
        );

        // Assertions
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.agents[0].id, "a1");
        assert_eq!(state.transcript_items.len(), 1);
        assert_eq!(state.transcript_items[0].content, "Hello **world**");
        assert_eq!(state.transcript_items[0].metadata, serde_json::json!({"final": true}));
        assert!(state.transcript_items[0].completed_at.is_some());
    }
}
