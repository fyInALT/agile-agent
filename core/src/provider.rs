use std::path::PathBuf;
use std::sync::mpsc::{Sender, channel};
use std::thread::{self, Builder, JoinHandle};
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::launch_config::context::ProviderLaunchContext;
use crate::logging;
use crate::mock_provider;
use crate::probe;
use crate::provider_thread::ProviderThreadHandle;
use crate::tool_calls::ExecCommandStatus;
use crate::tool_calls::McpInvocation;
use crate::tool_calls::McpToolCallStatus;
use crate::tool_calls::PatchApplyStatus;
use crate::tool_calls::PatchChange;
use crate::tool_calls::WebSearchAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    Mock,
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_slash_passthrough: bool,
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

    pub fn capabilities(self) -> ProviderCapabilities {
        match self {
            Self::Mock => ProviderCapabilities {
                supports_slash_passthrough: false,
            },
            Self::Claude | Self::Codex => ProviderCapabilities {
                supports_slash_passthrough: true,
            },
        }
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
    ExecCommandStarted {
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    },
    ExecCommandFinished {
        call_id: Option<String>,
        output_preview: Option<String>,
        status: ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        source: Option<String>,
    },
    ExecCommandOutputDelta {
        call_id: Option<String>,
        delta: String,
    },
    GenericToolCallStarted {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    },
    GenericToolCallFinished {
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    WebSearchStarted {
        call_id: Option<String>,
        query: String,
    },
    WebSearchFinished {
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    },
    ViewImage {
        call_id: Option<String>,
        path: String,
    },
    ImageGenerationFinished {
        call_id: Option<String>,
        revised_prompt: Option<String>,
        result: Option<String>,
        saved_path: Option<String>,
    },
    PatchApplyOutputDelta {
        call_id: Option<String>,
        delta: String,
    },
    McpToolCallStarted {
        call_id: Option<String>,
        invocation: McpInvocation,
    },
    McpToolCallFinished {
        call_id: Option<String>,
        invocation: McpInvocation,
        result_blocks: Vec<serde_json::Value>,
        error: Option<String>,
        status: McpToolCallStatus,
        is_error: bool,
    },
    PatchApplyStarted {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    },
    PatchApplyFinished {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        status: PatchApplyStatus,
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

/// Start a provider with structured launch context.
pub fn start_provider_with_context(
    context: ProviderLaunchContext,
    prompt: String,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    logging::debug_event(
        "provider.start_with_context",
        "starting provider with launch context",
        serde_json::json!({
            "provider": context.spec.provider.label(),
            "cwd": context.cwd.display().to_string(),
            "prompt": prompt,
            "session_handle": format!("{:?}", context.session_handle),
            "executable": context.spec.resolved_executable_path,
            "extra_args": context.spec.extra_args,
            "env_count": context.spec.effective_env.len(),
        }),
    );

    match context.spec.provider {
        ProviderKind::Mock => start_mock_provider(prompt, event_tx),
        ProviderKind::Claude => {
            crate::providers::claude::start_with_context(context, prompt, event_tx)
        }
        ProviderKind::Codex => {
            crate::providers::codex::start_with_context(context, prompt, event_tx)
        }
    }
}

/// Start a provider with context and full thread lifecycle management.
pub fn start_provider_with_handle_and_context(
    context: ProviderLaunchContext,
    prompt: String,
    thread_name: String,
) -> Result<ProviderThreadHandle> {
    let (keepalive_tx, event_rx) = channel();
    let thread_event_tx = keepalive_tx.clone();

    logging::debug_event(
        "provider.start_threaded_with_context",
        "starting provider thread with launch context",
        serde_json::json!({
            "provider": context.spec.provider.label(),
            "thread_name": thread_name,
            "cwd": context.cwd.display().to_string(),
            "session_handle": format!("{:?}", context.session_handle),
            "executable": context.spec.resolved_executable_path,
        }),
    );

    let handle: JoinHandle<()> = Builder::new().name(thread_name.clone()).spawn(move || {
        run_provider_internal_with_context(context, prompt, thread_event_tx);
    })?;

    Ok(ProviderThreadHandle::new(
        handle,
        event_rx,
        keepalive_tx,
        thread_name,
    ))
}

/// Internal provider runner with context for threaded execution
fn run_provider_internal_with_context(
    context: ProviderLaunchContext,
    prompt: String,
    event_tx: Sender<ProviderEvent>,
) {
    match context.spec.provider {
        ProviderKind::Mock => {
            let _ = event_tx.send(ProviderEvent::Status("mock provider started".to_string()));
            for chunk in mock_provider::build_reply_chunks(&prompt) {
                thread::sleep(Duration::from_millis(60));
                if event_tx.send(ProviderEvent::AssistantChunk(chunk)).is_err() {
                    return;
                }
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        }
        ProviderKind::Claude => {
            if let Err(e) = crate::providers::claude::start_with_context(context, prompt, event_tx)
            {
                logging::warn_event(
                    "provider.claude.start_failed",
                    "Claude provider failed to start",
                    serde_json::json!({ "error": e.to_string() }),
                );
            }
        }
        ProviderKind::Codex => {
            if let Err(e) = crate::providers::codex::start_with_context(context, prompt, event_tx) {
                logging::warn_event(
                    "provider.codex.start_failed",
                    "Codex provider failed to start",
                    serde_json::json!({ "error": e.to_string() }),
                );
            }
        }
    }
}

/// Start a provider with full thread lifecycle management
///
/// Returns a ProviderThreadHandle that includes:
/// - JoinHandle for thread lifecycle
/// - Event receiver for collecting provider events
/// - Graceful shutdown capability
///
/// Use this for multi-agent scenarios where thread management is required.
pub fn start_provider_with_handle(
    provider: ProviderKind,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    thread_name: String,
) -> Result<ProviderThreadHandle> {
    let (keepalive_tx, event_rx) = channel();
    let thread_event_tx = keepalive_tx.clone();

    logging::debug_event(
        "provider.start_threaded",
        "starting provider thread with handle",
        serde_json::json!({
            "provider": provider.label(),
            "thread_name": thread_name,
            "cwd": cwd.display().to_string(),
            "session_handle": format!("{:?}", session_handle),
        }),
    );

    let handle: JoinHandle<()> = Builder::new().name(thread_name.clone()).spawn(move || {
        run_provider_internal(provider, prompt, cwd, session_handle, thread_event_tx);
    })?;

    Ok(ProviderThreadHandle::new(
        handle,
        event_rx,
        keepalive_tx,
        thread_name,
    ))
}

/// Internal provider runner for threaded execution
fn run_provider_internal(
    provider: ProviderKind,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) {
    match provider {
        ProviderKind::Mock => {
            let _ = event_tx.send(ProviderEvent::Status("mock provider started".to_string()));
            for chunk in mock_provider::build_reply_chunks(&prompt) {
                thread::sleep(Duration::from_millis(60));
                if event_tx.send(ProviderEvent::AssistantChunk(chunk)).is_err() {
                    return;
                }
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        }
        ProviderKind::Claude => {
            // Claude provider spawns its own thread internally
            // We call it from here, errors are sent by provider or we log
            if let Err(e) = crate::providers::claude::start(prompt, cwd, session_handle, event_tx) {
                // Provider couldn't start - log error, event_tx was consumed
                logging::warn_event(
                    "provider.claude.start_failed",
                    "Claude provider failed to start",
                    serde_json::json!({ "error": e.to_string() }),
                );
            }
            // Note: Claude spawns its own thread, this outer thread exits quickly
            // The actual events come from Claude's internal thread
        }
        ProviderKind::Codex => {
            // Codex provider spawns its own thread internally
            if let Err(e) = crate::providers::codex::start(prompt, cwd, session_handle, event_tx) {
                logging::warn_event(
                    "provider.codex.start_failed",
                    "Codex provider failed to start",
                    serde_json::json!({ "error": e.to_string() }),
                );
            }
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
                | ProviderEvent::ExecCommandStarted { .. }
                | ProviderEvent::ExecCommandFinished { .. }
                | ProviderEvent::ExecCommandOutputDelta { .. }
                | ProviderEvent::GenericToolCallStarted { .. }
                | ProviderEvent::GenericToolCallFinished { .. }
                | ProviderEvent::WebSearchStarted { .. }
                | ProviderEvent::WebSearchFinished { .. }
                | ProviderEvent::ViewImage { .. }
                | ProviderEvent::ImageGenerationFinished { .. }
                | ProviderEvent::PatchApplyOutputDelta { .. }
                | ProviderEvent::McpToolCallStarted { .. }
                | ProviderEvent::McpToolCallFinished { .. }
                | ProviderEvent::PatchApplyStarted { .. }
                | ProviderEvent::PatchApplyFinished { .. }
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

    #[test]
    fn start_provider_with_handle_returns_thread_handle() {
        use super::start_provider_with_handle;

        let mut handle = start_provider_with_handle(
            ProviderKind::Mock,
            "hello".to_string(),
            ".".into(),
            None,
            "test-mock-thread".to_string(),
        )
        .expect("start provider with handle");

        // Handle should have event receiver
        let rx = handle.event_receiver();

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
                _ => {}
            }
        }

        assert!(saw_chunk, "threaded mock provider should emit chunks");
        assert!(saw_finished, "threaded mock provider should emit finished");

        // Give thread time to finish completely
        std::thread::sleep(Duration::from_millis(100));

        // Thread should finish gracefully
        let result = handle.stop(Duration::from_millis(500));
        assert!(
            matches!(
                result,
                crate::provider_thread::ThreadStopResult::GracefulStop { .. }
                    | crate::provider_thread::ThreadStopResult::AlreadyStopped
            ),
            "thread should finish gracefully or already be stopped, got: {:?}",
            result
        );
    }
}
