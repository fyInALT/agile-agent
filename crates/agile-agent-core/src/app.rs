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
}
