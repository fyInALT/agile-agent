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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppState {
    pub transcript: Vec<TranscriptEntry>,
    pub input: String,
    pub status: AppStatus,
    pub should_quit: bool,
}

impl AppState {
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

    pub fn begin_mock_response(&mut self) {
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

    pub fn finish_mock_response(&mut self) {
        self.status = AppStatus::Idle;
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use super::AppStatus;
    use super::TranscriptEntry;

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
        let mut state = AppState::default();
        state.begin_mock_response();
        state.append_assistant_chunk("hello");
        state.append_assistant_chunk(" world");
        state.finish_mock_response();

        assert_eq!(state.status, AppStatus::Idle);
        assert_eq!(
            state.transcript,
            vec![TranscriptEntry::Assistant("hello world".to_string())]
        );
    }
}
