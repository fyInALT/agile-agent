//! EventPump — converts `ProviderEvent`s into protocol `Event`s with sequence numbers.

use agent_core::ProviderEvent;
use agent_protocol::events::*;
use agent_protocol::state::{AgentSlotStatus, ItemKind};
use std::collections::HashMap;

/// Owns event conversion state and assigns monotonic sequence numbers.
pub struct EventPump {
    seq_counter: u64,
    /// Tracks in-flight items per agent: (agent_id, call_id) → item_id.
    current_items: HashMap<(String, Option<String>), String>,
    /// Monotonically increasing item index.
    next_item_index: u64,
}

impl EventPump {
    pub fn new() -> Self {
        Self {
            seq_counter: 0,
            current_items: HashMap::new(),
            next_item_index: 1,
        }
    }

    /// Convert a single `ProviderEvent` into zero or more protocol `Event`s.
    ///
    /// Returns the generated events and their sequence numbers.
    pub fn process(&mut self, agent_id: String, event: ProviderEvent) -> Vec<Event> {
        let mut events = Vec::new();

        match event {
            ProviderEvent::Status(status) => {
                events.push(self.mk_event(EventPayload::AgentStatusChanged(
                    AgentStatusChangedData {
                        agent_id: agent_id.clone(),
                        status: map_status(&status),
                    },
                )));
            }
            ProviderEvent::AssistantChunk(text) => {
                let item_id = self.get_or_create_item(agent_id.clone(), None, ItemKind::AssistantOutput);
                events.push(self.mk_event(EventPayload::ItemDelta(ItemDeltaData {
                    item_id: item_id.clone(),
                    delta: ItemDelta::Text(text),
                })));
            }
            ProviderEvent::ThinkingChunk(text) => {
                let item_id = self.get_or_create_item(agent_id.clone(), None, ItemKind::SystemMessage);
                events.push(self.mk_event(EventPayload::ItemDelta(ItemDeltaData {
                    item_id: item_id.clone(),
                    delta: ItemDelta::Markdown(text),
                })));
            }
            ProviderEvent::ExecCommandStarted { call_id, input_preview: _, source: _ } => {
                let item_id = self.create_item(agent_id.clone(), call_id.clone(), ItemKind::ToolCall);
                events.push(self.mk_event(EventPayload::ItemStarted(ItemStartedData {
                    item_id: item_id.clone(),
                    kind: ItemKind::ToolCall,
                    agent_id: agent_id.clone(),
                })));
            }
            ProviderEvent::ExecCommandFinished { call_id, output_preview, status, exit_code, duration_ms, source } => {
                if let Some(item_id) = self.finish_item(agent_id.clone(), call_id.clone()) {
                    events.push(self.mk_event(EventPayload::ItemCompleted(ItemCompletedData {
                        item_id,
                        item: agent_protocol::state::TranscriptItem {
                            id: format!("item-{}-exec", self.next_item_index),
                            kind: ItemKind::ToolResult,
                            agent_id: Some(agent_id.clone()),
                            content: output_preview.unwrap_or_default(),
                            metadata: serde_json::json!({
                                "type": "exec_command",
                                "status": format!("{:?}", status),
                                "exit_code": exit_code,
                                "duration_ms": duration_ms,
                                "source": source,
                            }),
                            created_at: chrono::Utc::now().to_rfc3339(),
                            completed_at: Some(chrono::Utc::now().to_rfc3339()),
                        },
                    })));
                }
            }
            ProviderEvent::ExecCommandOutputDelta { call_id, delta } => {
                if let Some(item_id) = self.current_item_id(&agent_id, &call_id) {
                    events.push(self.mk_event(EventPayload::ItemDelta(ItemDeltaData {
                        item_id: item_id.clone(),
                        delta: ItemDelta::Text(delta),
                    })));
                }
            }
            ProviderEvent::Error(msg) => {
                events.push(self.mk_event(EventPayload::Error(ErrorData {
                    message: msg,
                    source: Some(agent_id),
                })));
            }
            ProviderEvent::Finished => {
                events.push(self.mk_event(EventPayload::AgentStatusChanged(
                    AgentStatusChangedData {
                        agent_id: agent_id.clone(),
                        status: AgentSlotStatus::Idle,
                    },
                )));
            }
            _ => {
                // Other variants are handled as no-ops for now.
                tracing::debug!("unhandled ProviderEvent variant for {}", agent_id);
            }
        }

        events
    }

    fn mk_event(&mut self, payload: EventPayload) -> Event {
        self.seq_counter += 1;
        Event {
            seq: self.seq_counter,
            payload,
        }
    }

    fn get_or_create_item(
        &mut self,
        agent_id: String,
        call_id: Option<String>,
        kind: ItemKind,
    ) -> String {
        let key = (agent_id, call_id);
        if let Some(id) = self.current_items.get(&key) {
            return id.clone();
        }
        self.create_item(key.0, key.1, kind)
    }

    fn create_item(
        &mut self,
        agent_id: String,
        call_id: Option<String>,
        _kind: ItemKind,
    ) -> String {
        let item_id = format!("item-{}", self.next_item_index);
        self.next_item_index += 1;
        self.current_items
            .insert((agent_id, call_id), item_id.clone());
        item_id
    }

    fn current_item_id(&self, agent_id: &str, call_id: &Option<String>) -> Option<String> {
        self.current_items
            .get(&(agent_id.to_string(), call_id.clone()))
            .cloned()
    }

    fn finish_item(&mut self, agent_id: String, call_id: Option<String>) -> Option<String> {
        self.current_items.remove(&(agent_id, call_id))
    }
}

fn map_status(status: &str) -> AgentSlotStatus {
    match status {
        "idle" => AgentSlotStatus::Idle,
        "running" | "responding" | "thinking" => AgentSlotStatus::Running,
        "stopped" => AgentSlotStatus::Stopped,
        "error" => AgentSlotStatus::Error,
        _ => AgentSlotStatus::Running,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_numbers_are_monotonic() {
        let mut pump = EventPump::new();
        let events = pump.process(
            "a1".to_string(),
            ProviderEvent::AssistantChunk("hello".to_string()),
        );
        assert_eq!(events[0].seq, 1);

        let events2 = pump.process(
            "a1".to_string(),
            ProviderEvent::AssistantChunk("world".to_string()),
        );
        assert_eq!(events2[0].seq, 2);
    }

    #[test]
    fn assistant_chunk_produces_item_delta() {
        let mut pump = EventPump::new();
        let events = pump.process(
            "a1".to_string(),
            ProviderEvent::AssistantChunk("hello".to_string()),
        );

        assert_eq!(events.len(), 1);
        match &events[0].payload {
            EventPayload::ItemDelta(data) => {
                match &data.delta {
                    ItemDelta::Text(t) => assert_eq!(t, "hello"),
                    _ => panic!("expected text delta"),
                }
            }
            _ => panic!("expected ItemDelta"),
        }
    }

    #[test]
    fn exec_command_lifecycle() {
        let mut pump = EventPump::new();

        let started = pump.process(
            "a1".to_string(),
            ProviderEvent::ExecCommandStarted {
                call_id: Some("call-1".to_string()),
                input_preview: Some("ls".to_string()),
                source: None,
            },
        );
        assert_eq!(started.len(), 1);
        assert!(matches!(started[0].payload, EventPayload::ItemStarted(_)));

        let delta = pump.process(
            "a1".to_string(),
            ProviderEvent::ExecCommandOutputDelta {
                call_id: Some("call-1".to_string()),
                delta: "output".to_string(),
            },
        );
        assert_eq!(delta.len(), 1);
        assert!(matches!(delta[0].payload, EventPayload::ItemDelta(_)));

        let finished = pump.process(
            "a1".to_string(),
            ProviderEvent::ExecCommandFinished {
                call_id: Some("call-1".to_string()),
                output_preview: Some("done".to_string()),
                status: agent_core::ExecCommandStatus::Completed,
                exit_code: Some(0),
                duration_ms: Some(100),
                source: None,
            },
        );
        assert_eq!(finished.len(), 1);
        assert!(matches!(finished[0].payload, EventPayload::ItemCompleted(_)));
    }

    #[test]
    fn error_event_produces_error_payload() {
        let mut pump = EventPump::new();
        let events = pump.process(
            "a1".to_string(),
            ProviderEvent::Error("something went wrong".to_string()),
        );
        assert_eq!(events.len(), 1);
        match &events[0].payload {
            EventPayload::Error(data) => {
                assert_eq!(data.message, "something went wrong");
            }
            _ => panic!("expected Error payload"),
        }
    }
}
