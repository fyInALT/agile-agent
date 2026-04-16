//! Provider-based LLM Caller implementation
//!
//! Implements the LLMCaller trait from agent-decision using the
//! provider infrastructure in agent-core.

use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread::Builder;
use std::time::Duration;

use crate::logging;
use crate::provider::{ProviderEvent, ProviderKind};

use agent_decision::error::DecisionError;
use agent_decision::llm_caller::LLMCaller;

/// Provider-based LLM caller
///
/// Uses the provider infrastructure to make real LLM calls.
/// Thread-safe implementation that creates a thread per call.
pub struct ProviderLLMCaller {
    /// Provider kind (Claude or Codex)
    provider_kind: ProviderKind,
    /// Working directory for provider
    cwd: std::path::PathBuf,
    /// Caller identifier
    id: String,
    /// Health status
    healthy: bool,
}

impl ProviderLLMCaller {
    /// Create a new provider-based LLM caller
    pub fn new(provider_kind: ProviderKind, cwd: std::path::PathBuf) -> Self {
        Self {
            provider_kind,
            cwd,
            id: format!("provider-{}", provider_kind.label()),
            healthy: true,
        }
    }

    /// Create with custom identifier
    pub fn with_id(provider_kind: ProviderKind, cwd: std::path::PathBuf, id: impl Into<String>) -> Self {
        Self {
            provider_kind,
            cwd,
            id: id.into(),
            healthy: true,
        }
    }

    /// Get the provider kind
    pub fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    /// Mark as unhealthy
    pub fn mark_unhealthy(&mut self) {
        self.healthy = false;
    }
}

// Note: ProviderLLMCaller doesn't store thread handles or receivers,
// making it thread-safe (Sync). Each call creates its own thread.

impl LLMCaller for ProviderLLMCaller {
    fn call(&self, prompt: &str, timeout_ms: u64) -> Result<String, DecisionError> {
        // Create channel for events
        let (event_tx, event_rx): (Sender<ProviderEvent>, Receiver<ProviderEvent>) = channel();

        let provider_label = self.provider_kind.label();
        let thread_name = format!("{}-decision-call", provider_label);

        logging::debug_event(
            "llm_caller.call",
            "making LLM call",
            serde_json::json!({
                "provider": provider_label,
                "caller_id": self.id,
                "prompt_len": prompt.len(),
                "timeout_ms": timeout_ms,
            }),
        );

        // Clone data needed for the thread
        let cwd = self.cwd.clone();
        let prompt = prompt.to_string();
        let provider_kind = self.provider_kind;

        // Spawn the provider thread
        let handle = Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                run_provider_for_llm_call(provider_kind, prompt, cwd, event_tx);
            })
            .map_err(|e| DecisionError::EngineError(format!("Failed to spawn thread: {}", e)))?;

        // Collect response
        let timeout = Duration::from_millis(timeout_ms);
        let mut response_chunks: Vec<String> = Vec::new();
        let mut finished = false;

        while !finished {
            match event_rx.recv_timeout(timeout) {
                Ok(event) => {
                    match event {
                        ProviderEvent::AssistantChunk(chunk) => {
                            response_chunks.push(chunk);
                        }
                        ProviderEvent::Finished => {
                            finished = true;
                        }
                        ProviderEvent::Error(error) => {
                            return Err(DecisionError::EngineError(format!("Provider error: {}", error)));
                        }
                        ProviderEvent::Status(_) | ProviderEvent::ThinkingChunk(_) => {
                            // Ignore status and thinking chunks for decision making
                        }
                        _ => {
                            // Ignore other events
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Thread didn't finish in time
                    // Clean up by dropping handle (thread will continue but we ignore it)
                    drop(handle);
                    return Err(DecisionError::EngineError(format!(
                        "Timeout waiting for response after {}ms",
                        timeout_ms
                    )));
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Thread finished
                    finished = true;
                }
            }
        }

        // Clean up thread
        drop(handle);

        let response = response_chunks.join("");
        if response.is_empty() {
            Err(DecisionError::EngineError("Empty response from provider".to_string()))
        } else {
            logging::debug_event(
                "llm_caller.response",
                "received LLM response",
                serde_json::json!({
                    "response_len": response.len(),
                }),
            );
            Ok(response)
        }
    }

    fn is_healthy(&self) -> bool {
        self.healthy
    }

    fn caller_id(&self) -> &str {
        &self.id
    }
}

/// Run provider in thread context for LLM call
///
/// This is a simplified provider runner that sends a prompt and collects response.
fn run_provider_for_llm_call(
    provider: ProviderKind,
    prompt: String,
    _cwd: std::path::PathBuf,
    event_tx: Sender<ProviderEvent>,
) {
    // For now, use mock provider logic
    // Full implementation would call actual provider (claude/codex)
    use crate::mock_provider;

    logging::debug_event(
        "llm_caller.run",
        "provider thread started",
        serde_json::json!({
            "provider": provider.label(),
        }),
    );

    // Send status event
    let _ = event_tx.send(ProviderEvent::Status(format!(
        "{} LLM call started",
        provider.label()
    )));

    // Use mock provider to generate response chunks
    // In production, this would call the actual provider via CLI
    for chunk in mock_provider::build_reply_chunks(&prompt) {
        std::thread::sleep(Duration::from_millis(30));
        if event_tx.send(ProviderEvent::AssistantChunk(chunk)).is_err() {
            return;
        }
    }

    // Send finished event
    let _ = event_tx.send(ProviderEvent::Finished);

    logging::debug_event(
        "llm_caller.finished",
        "provider thread finished",
        serde_json::json!({}),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_caller() -> ProviderLLMCaller {
        let temp = TempDir::new().unwrap();
        ProviderLLMCaller::new(ProviderKind::Mock, temp.path().to_path_buf())
    }

    #[test]
    fn test_provider_llm_caller_new() {
        let caller = make_caller();
        assert!(caller.is_healthy());
        assert!(caller.caller_id().contains("mock"));
    }

    #[test]
    fn test_provider_llm_caller_call_returns_response() {
        let caller = make_caller();
        let response = caller.call("test prompt", 5000);
        assert!(response.is_ok());
        assert!(!response.unwrap().is_empty());
    }

    #[test]
    fn test_provider_llm_caller_call_with_timeout() {
        let caller = make_caller();
        // Very short timeout should fail
        let response = caller.call("test prompt", 10);
        assert!(response.is_err());
    }

    #[test]
    fn test_provider_llm_caller_with_custom_id() {
        let temp = TempDir::new().unwrap();
        let caller = ProviderLLMCaller::with_id(
            ProviderKind::Mock,
            temp.path().to_path_buf(),
            "custom-id",
        );
        assert_eq!(caller.caller_id(), "custom-id");
    }
}