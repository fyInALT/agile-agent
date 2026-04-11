use crate::provider::ProviderKind;

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
    Status(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppState {
    pub transcript: Vec<TranscriptEntry>,
    pub input: String,
    pub selected_provider: ProviderKind,
    pub status: AppStatus,
    pub should_quit: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            transcript: Vec::new(),
            input: String::new(),
            selected_provider: ProviderKind::Mock,
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
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use super::AppStatus;
    use super::TranscriptEntry;
    use crate::provider::ProviderKind;

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
        assert_eq!(state.selected_provider, ProviderKind::Mock);
    }
}
