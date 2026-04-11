use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::mock_provider;
use crate::probe;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Mock,
    Claude,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Mock => Self::Claude,
            Self::Claude => Self::Mock,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    Status(String),
    AssistantChunk(String),
    Error(String),
    Finished,
}

pub fn default_provider() -> ProviderKind {
    if probe::is_provider_available("claude") {
        ProviderKind::Claude
    } else {
        ProviderKind::Mock
    }
}

pub fn start_provider(
    provider: ProviderKind,
    prompt: String,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    match provider {
        ProviderKind::Mock => start_mock_provider(prompt, event_tx),
        ProviderKind::Claude => crate::providers::claude::start(prompt, event_tx),
    }
}

fn start_mock_provider(prompt: String, event_tx: Sender<ProviderEvent>) -> Result<()> {
    thread::Builder::new()
        .name("agile-agent-mock-provider".to_string())
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
