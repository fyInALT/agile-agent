use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMessageRecord {
    pub sequence: usize,
    pub kind: AgentMessageKind,
    pub summary: String,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMessages {
    pub entries: Vec<AgentMessageRecord>,
}

impl AgentMessages {
    pub fn from_app_state(state: &AppState) -> Self {
        let captured_at = Utc::now().to_rfc3339();
        let entries = state
            .transcript
            .iter()
            .enumerate()
            .map(|(sequence, entry)| AgentMessageRecord {
                sequence,
                kind: message_kind(entry),
                summary: message_summary(entry),
                captured_at: captured_at.clone(),
            })
            .collect();
        Self { entries }
    }
}

fn message_kind(entry: &TranscriptEntry) -> AgentMessageKind {
    match entry {
        TranscriptEntry::User(_) => AgentMessageKind::User,
        TranscriptEntry::Assistant(_) => AgentMessageKind::Assistant,
        TranscriptEntry::Thinking(_) => AgentMessageKind::Thinking,
        TranscriptEntry::ToolCall { .. } => AgentMessageKind::ToolCall,
        TranscriptEntry::Status(_) => AgentMessageKind::Status,
        TranscriptEntry::Error(_) => AgentMessageKind::Error,
    }
}

fn message_summary(entry: &TranscriptEntry) -> String {
    match entry {
        TranscriptEntry::User(text)
        | TranscriptEntry::Assistant(text)
        | TranscriptEntry::Thinking(text)
        | TranscriptEntry::Status(text)
        | TranscriptEntry::Error(text) => text.clone(),
        TranscriptEntry::ToolCall {
            name,
            input_preview,
            output_preview,
            success,
            started,
            ..
        } => format!(
            "{}:{}:{}:{}:{}",
            name,
            started,
            success,
            input_preview.as_deref().unwrap_or(""),
            output_preview.as_deref().unwrap_or("")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::AgentMessages;
    use crate::app::AppState;
    use crate::app::TranscriptEntry;
    use crate::provider::ProviderKind;

    #[test]
    fn projects_transcript_into_message_log() {
        let mut state = AppState::new(ProviderKind::Mock);
        state
            .transcript
            .push(TranscriptEntry::Status("hello".to_string()));

        let messages = AgentMessages::from_app_state(&state);

        assert_eq!(messages.entries.len(), 1);
        assert_eq!(messages.entries[0].sequence, 0);
        assert_eq!(messages.entries[0].summary, "hello");
    }
}
