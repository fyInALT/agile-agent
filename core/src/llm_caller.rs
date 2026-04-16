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
/// Calls the actual provider (Claude/Codex) for decision making.
/// Falls back to mock provider if the real provider is unavailable.
fn run_provider_for_llm_call(
    provider: ProviderKind,
    prompt: String,
    cwd: std::path::PathBuf,
    event_tx: Sender<ProviderEvent>,
) {
    logging::debug_event(
        "llm_caller.run",
        "provider thread started",
        serde_json::json!({
            "provider": provider.label(),
            "prompt_len": prompt.len(),
        }),
    );

    // Keep a copy for fallback
    let prompt_for_fallback = prompt.clone();

    // Try to start the actual provider
    let result = match provider {
        ProviderKind::Mock => {
            // Mock provider: use inline implementation
            use crate::mock_provider;
            let _ = event_tx.send(ProviderEvent::Status("mock provider started".to_string()));
            for chunk in mock_provider::build_reply_chunks(&prompt) {
                std::thread::sleep(Duration::from_millis(30));
                if event_tx.send(ProviderEvent::AssistantChunk(chunk)).is_err() {
                    return;
                }
            }
            let _ = event_tx.send(ProviderEvent::Finished);
            Ok(())
        }
        ProviderKind::Claude => {
            crate::providers::claude::start(prompt, cwd, None, event_tx)
        }
        ProviderKind::Codex => {
            crate::providers::codex::start(prompt, cwd, None, event_tx)
        }
    };

    // Provider takes ownership of event_tx, so we can't use it in fallback
    // Instead, if provider fails to start, we just log and return (provider sends Error event internally)
    if let Err(e) = result {
        logging::warn_event(
            "llm_caller.provider_failed",
            "provider failed to start",
            serde_json::json!({
                "provider": provider.label(),
                "error": e.to_string(),
            }),
        );
        // Note: We can't fallback to mock here because event_tx was consumed by the provider
        // The provider should have sent an Error event before returning Err
    }

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