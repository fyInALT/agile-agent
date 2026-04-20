//! Pure protocol-driven TUI state (zero `agent_core` imports).
//!
//! This is the target state machine for Sprint 8 decoupling.
//! It contains only data types from `agent_protocol` and stdlib.

use agent_protocol::state::{
    AgentSnapshot, SessionState, SessionStatus, TranscriptItem,
};
use std::collections::HashMap;

/// Connection status between TUI and daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Per-agent view state cache (scroll, follow-tail).
#[derive(Debug, Clone, Default)]
pub struct AgentViewState {
    pub scroll_offset: usize,
    pub follow_tail: bool,
    pub last_cell_range: Option<(usize, usize)>,
}

/// Pure render state driven entirely by the daemon event stream.
///
/// This struct intentionally does **not** contain `RuntimeSession`, `AgentPool`,
/// `EventAggregator`, or `AgentMailbox`. All runtime state lives in the daemon.
///
/// Sprint 8 MVP: contains only data types that do not pull in `agent_core`.
/// Overlay widgets and composer are omitted until their decoupling is complete.
#[derive(Debug)]
pub struct ProtocolState {
    // -- connection --
    pub connection_state: ConnectionState,

    // -- session snapshot --
    pub session_id: String,
    pub alias: String,
    pub server_time: String,
    pub last_event_seq: u64,

    // -- app state --
    pub transcript_items: Vec<TranscriptItem>,
    pub input_text: String,
    pub input_multiline: bool,
    pub session_status: SessionStatus,

    // -- agents --
    pub agents: Vec<AgentSnapshot>,
    pub focused_agent_id: Option<String>,

    // -- view / render (lightweight) --
    pub composer_width: u16,
    pub transcript_viewport_height: u16,
    pub transcript_render_width: Option<usize>,
    pub transcript_scroll_offset: usize,
    pub transcript_max_scroll: usize,
    pub transcript_follow_tail: bool,
    pub transcript_rendered_lines: Vec<String>,
    pub busy_started_at: Option<std::time::Instant>,
    pub agent_view_states: HashMap<String, AgentViewState>,
    pub decision_status: Option<String>,
}

impl Default for ProtocolState {
    fn default() -> Self {
        Self {
            connection_state: ConnectionState::default(),
            session_id: String::new(),
            alias: String::new(),
            server_time: String::new(),
            last_event_seq: 0,
            transcript_items: Vec::new(),
            input_text: String::new(),
            input_multiline: false,
            session_status: SessionStatus::default(),
            agents: Vec::new(),
            focused_agent_id: None,
            composer_width: 80,
            transcript_viewport_height: 1,
            transcript_render_width: None,
            transcript_scroll_offset: 0,
            transcript_max_scroll: 0,
            transcript_follow_tail: true,
            transcript_rendered_lines: Vec::new(),
            busy_started_at: None,
            agent_view_states: HashMap::new(),
            decision_status: None,
        }
    }
}

impl ProtocolState {
    /// Bootstrap from an initial `SessionState` snapshot.
    pub fn from_snapshot(snapshot: SessionState) -> Self {
        Self {
            session_id: snapshot.session_id,
            alias: snapshot.alias,
            server_time: snapshot.server_time,
            last_event_seq: snapshot.last_event_seq,
            transcript_items: snapshot.app_state.transcript,
            input_text: snapshot.app_state.input.text.clone(),
            input_multiline: snapshot.app_state.input.multiline,
            session_status: snapshot.app_state.status,
            agents: snapshot.agents,
            focused_agent_id: snapshot.focused_agent_id,
            connection_state: ConnectionState::Connected,
            ..Default::default()
        }
    }

    // -- helpers mirroring old TuiState API used by render.rs --

    pub fn is_multi_agent_mode(&self) -> bool {
        self.agents.len() > 1
    }

    pub fn focused_agent_id(&self) -> Option<&str> {
        self.focused_agent_id.as_deref()
    }

    pub fn agent_statuses(&self) -> Vec<AgentStatusView> {
        self.agents
            .iter()
            .map(|a| AgentStatusView {
                id: a.id.clone(),
                codename: a.codename.clone(),
                role: a.role.clone(),
                status: a.status,
                provider: a.provider.clone(),
            })
            .collect()
    }

    pub fn is_busy(&self) -> bool {
        self.session_status == SessionStatus::Running
    }

    pub fn sync_busy_started_at(&mut self) {
        if self.is_busy() && self.busy_started_at.is_none() {
            self.busy_started_at = Some(std::time::Instant::now());
        } else if !self.is_busy() {
            self.busy_started_at = None;
        }
    }
}

/// Lightweight view of an agent's status for rendering.
#[derive(Debug, Clone)]
pub struct AgentStatusView {
    pub id: String,
    pub codename: String,
    pub role: String,
    pub status: agent_protocol::state::AgentSlotStatus,
    pub provider: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::state::{
        AppStateSnapshot, BacklogSnapshot, InputState, ItemKind, TranscriptItem, WorkplaceSnapshot,
    };

    #[test]
    fn from_snapshot_populates_fields() {
        let snapshot = SessionState {
            session_id: "sess-1".to_string(),
            alias: "test".to_string(),
            server_time: chrono::Utc::now().to_rfc3339(),
            last_event_seq: 0,
            app_state: AppStateSnapshot {
                transcript: vec![TranscriptItem {
                    id: "t1".to_string(),
                    kind: ItemKind::UserInput,
                    agent_id: None,
                    content: "hello".to_string(),
                    metadata: serde_json::Value::Null,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    completed_at: None,
                }],
                input: InputState {
                    text: "typed".to_string(),
                    multiline: false,
                },
                status: SessionStatus::Idle,
            },
            agents: vec![AgentSnapshot {
                id: "a1".to_string(),
                codename: "alpha".to_string(),
                role: "Developer".to_string(),
                provider: "mock".to_string(),
                status: agent_protocol::state::AgentSlotStatus::Idle,
                current_task_id: None,
                uptime_seconds: 0,
            }],
            workplace: WorkplaceSnapshot {
                id: "wp-1".to_string(),
                path: "/tmp".to_string(),
                backlog: BacklogSnapshot { items: vec![] },
                skills: vec![],
            },
            focused_agent_id: Some("a1".to_string()),
            protocol_version: "1.0.0".to_string(),
        };

        let state = ProtocolState::from_snapshot(snapshot);
        assert_eq!(state.session_id, "sess-1");
        assert_eq!(state.agents.len(), 1);
        assert_eq!(state.focused_agent_id, Some("a1".to_string()));
        assert_eq!(state.input_text, "typed");
        assert_eq!(state.connection_state, ConnectionState::Connected);
    }

    #[test]
    fn agent_statuses_reflects_agents() {
        let mut state = ProtocolState::default();
        state.agents.push(AgentSnapshot {
            id: "a1".to_string(),
            codename: "alpha".to_string(),
            role: "Developer".to_string(),
            provider: "mock".to_string(),
            status: agent_protocol::state::AgentSlotStatus::Running,
            current_task_id: None,
            uptime_seconds: 0,
        });

        let statuses = state.agent_statuses();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].codename, "alpha");
    }
}
