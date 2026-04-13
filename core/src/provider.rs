use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::logging;
use crate::mock_provider;
use crate::probe;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    Mock,
    Claude,
    Codex,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Mock => Self::Claude,
            Self::Claude => Self::Codex,
            Self::Codex => Self::Mock,
        }
    }

    pub fn all() -> [ProviderKind; 3] {
        [
            ProviderKind::Mock,
            ProviderKind::Claude,
            ProviderKind::Codex,
        ]
    }
}

/// Session handle for multi-turn conversation continuity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionHandle {
    ClaudeSession { session_id: String },
    CodexThread { thread_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    Status(String),
    AssistantChunk(String),
    ThinkingChunk(String),
    ToolCallStarted {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    },
    ToolCallFinished {
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        source: Option<String>,
    },
    SessionHandle(SessionHandle),
    Error(String),
    Finished,
}

pub fn default_provider() -> ProviderKind {
    if probe::is_provider_available("claude") {
        ProviderKind::Claude
    } else if probe::is_provider_available("codex") {
        ProviderKind::Codex
    } else {
        ProviderKind::Mock
    }
}

pub fn start_provider(
    provider: ProviderKind,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    logging::debug_event(
        "provider.start",
        "starting provider request",
        serde_json::json!({
            "provider": provider.label(),
            "cwd": cwd.display().to_string(),
            "prompt": prompt,
            "session_handle": format!("{:?}", session_handle),
        }),
    );
    match provider {
        ProviderKind::Mock => start_mock_provider(prompt, event_tx),
        ProviderKind::Claude => {
            crate::providers::claude::start(prompt, cwd, session_handle, event_tx)
        }
        ProviderKind::Codex => {
            crate::providers::codex::start(prompt, cwd, session_handle, event_tx)
        }
    }
}

fn start_mock_provider(prompt: String, event_tx: Sender<ProviderEvent>) -> Result<()> {
    thread::Builder::new()
        .name("agent-mock-provider".to_string())
        .spawn(move || {
            let _ = event_tx.send(ProviderEvent::Status("mock provider started".to_string()));
            for chunk in mock_provider::build_reply_chunks(&prompt) {
                thread::sleep(Duration::from_millis(60));
                if event_tx.send(ProviderEvent::AssistantChunk(chunk)).is_err() {
                    return;
                }
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        })
        .map(|_| ())
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::ProviderEvent;
    use super::ProviderKind;
    use super::start_provider;

    #[test]
    fn mock_provider_emits_assistant_chunks_and_finishes() {
        let (tx, rx) = mpsc::channel();

        start_provider(
            ProviderKind::Mock,
            "hello".to_string(),
            ".".into(),
            None,
            tx,
        )
        .expect("start provider");

        let mut saw_chunk = false;
        let mut saw_finished = false;
        let deadline = std::time::Instant::now() + Duration::from_secs(2);

        while std::time::Instant::now() < deadline {
            let Ok(event) = rx.recv_timeout(Duration::from_millis(250)) else {
                continue;
            };
            match event {
                ProviderEvent::AssistantChunk(_) => saw_chunk = true,
                ProviderEvent::Finished => {
                    saw_finished = true;
                    break;
                }
                ProviderEvent::Status(_)
                | ProviderEvent::ThinkingChunk(_)
                | ProviderEvent::ToolCallStarted { .. }
                | ProviderEvent::ToolCallFinished { .. }
                | ProviderEvent::SessionHandle(_)
                | ProviderEvent::Error(_) => {}
            }
        }

        assert!(
            saw_chunk,
            "mock provider should emit at least one assistant chunk"
        );
        assert!(saw_finished, "mock provider should emit a finished event");
    }
}
