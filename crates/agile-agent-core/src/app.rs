use crate::provider::ProviderKind;
use crate::provider::SessionHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AppStatus {
    #[default]
    Idle,
    Responding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptEntry {
    User(String),
    Assistant(String),
    Thinking(String),
    ToolCall {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
        output_preview: Option<String>,
        success: bool,
        started: bool,
    },
    Status(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub transcript: Vec<TranscriptEntry>,
    pub input: String,
    pub selected_provider: ProviderKind,
    pub claude_session_id: Option<String>,
    pub codex_thread_id: Option<String>,
    pub status: AppStatus,
    pub should_quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            transcript: Vec::new(),
            input: String::new(),
            selected_provider: ProviderKind::Mock,
            claude_session_id: None,
            codex_thread_id: None,
            status: AppStatus::Idle,
            should_quit: false,
        }
    }
}

impl AppState {
    pub fn new(selected_provider: ProviderKind) -> Self {
        Self {
            selected_provider,
            ..Self::default()
        }
    }

    pub fn request_quit(&mut self) {
        self.should_quit = true;
    }

    pub fn insert_char(&mut self, ch: char) {
        self.input.push(ch);
    }

    pub fn backspace(&mut self) {
        self.input.pop();
    }

    pub fn take_input(&mut self) -> Option<String> {
        if self.input.is_empty() {
            return None;
        }

        Some(std::mem::take(&mut self.input))
    }

    pub fn push_user_message(&mut self, text: String) {
        self.transcript.push(TranscriptEntry::User(text));
    }

    pub fn begin_provider_response(&mut self) {
        self.status = AppStatus::Responding;
        self.transcript
            .push(TranscriptEntry::Assistant(String::new()));
    }

    pub fn append_assistant_chunk(&mut self, chunk: &str) {
        match self.transcript.last_mut() {
            Some(TranscriptEntry::Assistant(text)) => text.push_str(chunk),
            _ => self
                .transcript
                .push(TranscriptEntry::Assistant(chunk.to_string())),
        }
    }

    pub fn append_thinking_chunk(&mut self, chunk: &str) {
        match self.transcript.last_mut() {
            Some(TranscriptEntry::Thinking(text)) => text.push_str(chunk),
            _ => self
                .transcript
                .push(TranscriptEntry::Thinking(chunk.to_string())),
        }
    }

    pub fn push_tool_call_started(
        &mut self,
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    ) {
        self.transcript.push(TranscriptEntry::ToolCall {
            name,
            call_id,
            input_preview,
            output_preview: None,
            success: true,
            started: true,
        });
    }

    pub fn push_tool_call_finished(
        &mut self,
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
    ) {
        // Find the matching started tool call and update it
        for entry in self.transcript.iter_mut().rev() {
            if let TranscriptEntry::ToolCall {
                name: existing_name,
                call_id: existing_call_id,
                started: true,
                ..
            } = entry
            {
                if *existing_name == name && (call_id.is_none() || existing_call_id == &call_id) {
                    *entry = TranscriptEntry::ToolCall {
                        name,
                        call_id,
                        input_preview: None,
                        output_preview,
                        success,
                        started: false,
                    };
                    return;
                }
            }
        }
        // If not found, add as a finished entry
        self.transcript.push(TranscriptEntry::ToolCall {
            name,
            call_id,
            input_preview: None,
            output_preview,
            success,
            started: false,
        });
    }

    pub fn finish_provider_response(&mut self) {
        self.status = AppStatus::Idle;
    }

    pub fn toggle_provider(&mut self) {
        self.selected_provider = self.selected_provider.next();
    }

    pub fn push_status_message(&mut self, text: impl Into<String>) {
        self.transcript.push(TranscriptEntry::Status(text.into()));
    }

    pub fn push_error_message(&mut self, text: impl Into<String>) {
        self.transcript.push(TranscriptEntry::Error(text.into()));
    }

    pub fn current_session_handle(&self) -> Option<SessionHandle> {
        match self.selected_provider {
            ProviderKind::Mock => None,
            ProviderKind::Claude => {
                self.claude_session_id
                    .as_ref()
                    .map(|session_id| SessionHandle::ClaudeSession {
                        session_id: session_id.clone(),
                    })
            }
            ProviderKind::Codex => {
                self.codex_thread_id
                    .as_ref()
                    .map(|thread_id| SessionHandle::CodexThread {
                        thread_id: thread_id.clone(),
                    })
            }
        }
    }

    pub fn apply_session_handle(&mut self, handle: SessionHandle) {
        match handle {
            SessionHandle::ClaudeSession { session_id } => {
                self.claude_session_id = Some(session_id);
            }
            SessionHandle::CodexThread { thread_id } => {
                self.codex_thread_id = Some(thread_id);
            }
        }
    }

    /// Clear the session handle to start a fresh conversation
    pub fn clear_session(&mut self) {
        self.claude_session_id = None;
        self.codex_thread_id = None;
        self.transcript.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use super::AppStatus;
    use super::TranscriptEntry;
    use crate::provider::ProviderKind;
    use crate::provider::SessionHandle;

    #[test]
    fn take_input_clears_buffer() {
        let mut state = AppState::default();
        state.insert_char('h');
        state.insert_char('i');

        let submitted = state.take_input();

        assert_eq!(submitted, Some("hi".to_string()));
        assert!(state.input.is_empty());
    }

    #[test]
    fn append_assistant_chunk_updates_last_assistant_message() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.begin_provider_response();
        state.append_assistant_chunk("hello");
        state.append_assistant_chunk(" world");
        state.finish_provider_response();

        assert_eq!(state.status, AppStatus::Idle);
        assert_eq!(
            state.transcript,
            vec![TranscriptEntry::Assistant("hello world".to_string())]
        );
    }

    #[test]
    fn toggle_provider_switches_between_mock_and_claude() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.toggle_provider();
        assert_eq!(state.selected_provider, ProviderKind::Claude);
        state.toggle_provider();
        assert_eq!(state.selected_provider, ProviderKind::Codex);
        state.toggle_provider();
        assert_eq!(state.selected_provider, ProviderKind::Mock);
    }

    #[test]
    fn session_handles_are_stored_per_provider() {
        let mut state = AppState::new(ProviderKind::Mock);
        state.apply_session_handle(SessionHandle::ClaudeSession {
            session_id: "s1".to_string(),
        });
        state.apply_session_handle(SessionHandle::CodexThread {
            thread_id: "t1".to_string(),
        });

        state.selected_provider = ProviderKind::Claude;
        assert_eq!(
            state.current_session_handle(),
            Some(SessionHandle::ClaudeSession {
                session_id: "s1".to_string()
            })
        );

        state.selected_provider = ProviderKind::Codex;
        assert_eq!(
            state.current_session_handle(),
            Some(SessionHandle::CodexThread {
                thread_id: "t1".to_string()
            })
        );
    }
}
