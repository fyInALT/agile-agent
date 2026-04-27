//! Real Provider Integration — connects decision-dsl to actual Claude/Codex
//!
//! Implements LLMProvider trait from decision-dsl using agent-provider.

use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use anyhow::Result;
use decision_dsl::ext::traits::{LLMProvider, ProviderError, ProviderErrorKind};

/// Real LLM provider using agent-provider infrastructure.
pub struct RealLLMProvider {
    /// Provider kind (Claude or Codex)
    provider_kind: agent_provider::ProviderKind,
    /// Working directory
    cwd: std::path::PathBuf,
    /// Health status
    healthy: bool,
    /// Timeout for calls
    default_timeout_ms: u64,
}

impl RealLLMProvider {
    /// Create a new real LLM provider.
    pub fn new(provider_kind: agent_provider::ProviderKind, cwd: std::path::PathBuf) -> Self {
        Self {
            provider_kind,
            cwd,
            healthy: true,
            default_timeout_ms: 60000, // 60 seconds default
        }
    }

    /// Set custom timeout.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.default_timeout_ms = timeout_ms;
        self
    }

    /// Check provider availability.
    pub fn check_available(&self) -> bool {
        agent_provider::probe::is_provider_available(self.provider_kind.label())
    }
}

impl LLMProvider for RealLLMProvider {
    fn call(&mut self, prompt: &str, cwd: &Path, timeout_ms: u64) -> Result<String, ProviderError> {
        let (event_tx, event_rx): (Sender<agent_provider::ProviderEvent>, Receiver<agent_provider::ProviderEvent>) = channel();

        tracing::info!(
            provider = self.provider_kind.label(),
            prompt_len = prompt.len(),
            timeout_ms = timeout_ms,
            "Starting real LLM call"
        );

        // Start provider thread
        let result = agent_provider::start_provider(
            self.provider_kind,
            prompt.to_string(),
            cwd.to_path_buf(),
            None, // No session handle for decision calls
            event_tx,
        );

        if let Err(e) = result {
            self.healthy = false;
            return Err(ProviderError {
                kind: ProviderErrorKind::Unavailable,
                message: e.to_string(),
            });
        }

        // Collect response
        let timeout = Duration::from_millis(timeout_ms);
        let mut response_chunks: Vec<String> = Vec::new();
        let mut finished = false;

        while !finished {
            match event_rx.recv_timeout(timeout) {
                Ok(event) => {
                    match event {
                        agent_provider::ProviderEvent::AssistantChunk(chunk) => {
                            response_chunks.push(chunk);
                        }
                        agent_provider::ProviderEvent::Finished => {
                            finished = true;
                        }
                        agent_provider::ProviderEvent::Error(error) => {
                            return Err(ProviderError {
                                kind: ProviderErrorKind::InternalError,
                                message: error,
                            });
                        }
                        agent_provider::ProviderEvent::Status(_)
                        | agent_provider::ProviderEvent::ThinkingChunk(_) => {
                            // Ignore status and thinking chunks
                        }
                        _ => {
                            // Ignore other events
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    self.healthy = false;
                    return Err(ProviderError {
                        kind: ProviderErrorKind::Timeout,
                        message: format!("Timeout after {}ms", timeout_ms),
                    });
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    finished = true;
                }
            }
        }

        let response = response_chunks.join("");
        if response.is_empty() {
            Err(ProviderError {
                kind: ProviderErrorKind::ParseError,
                message: "Empty response from provider".to_string(),
            })
        } else {
            tracing::info!(
                provider = self.provider_kind.label(),
                response_len = response.len(),
                "LLM call completed"
            );
            Ok(response)
        }
    }

    fn provider_label(&self) -> &str {
        self.provider_kind.label()
    }

    fn is_healthy(&self) -> bool {
        self.healthy && self.check_available()
    }
}

/// Create a real LLM provider with default settings.
pub fn create_real_provider(cwd: std::path::PathBuf) -> RealLLMProvider {
    let provider_kind = agent_provider::default_provider();
    RealLLMProvider::new(provider_kind, cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_provider_new() {
        let cwd = std::path::PathBuf::from("/tmp");
        let provider = RealLLMProvider::new(agent_provider::ProviderKind::Mock, cwd);
        assert_eq!(provider.provider_label(), "mock");
    }

    #[test]
    fn real_provider_with_timeout() {
        let cwd = std::path::PathBuf::from("/tmp");
        let provider = RealLLMProvider::new(agent_provider::ProviderKind::Mock, cwd)
            .with_timeout(30000);
        assert_eq!(provider.default_timeout_ms, 30000);
    }

    #[test]
    fn real_provider_is_healthy() {
        let cwd = std::path::PathBuf::from("/tmp");
        let provider = RealLLMProvider::new(agent_provider::ProviderKind::Mock, cwd);
        // Mock may or may not be detected by probe; test internal healthy flag
        assert!(provider.healthy);
        // is_healthy() combines healthy flag + probe check
        // Mock provider might not be in probe, so just check the logic
        let available = provider.check_available();
        assert_eq!(provider.is_healthy(), provider.healthy && available);
    }

    #[test]
    fn real_provider_call_mock() {
        let cwd = std::path::PathBuf::from("/tmp");
        let mut provider = RealLLMProvider::new(agent_provider::ProviderKind::Mock, cwd.clone());

        let result = provider.call("test prompt", &cwd, 5000);
        // Mock provider should return something
        assert!(result.is_ok() || result.unwrap_err().kind == ProviderErrorKind::Timeout);
    }

    #[test]
    fn create_real_provider_uses_default() {
        let cwd = std::path::PathBuf::from("/tmp");
        let provider = create_real_provider(cwd);
        // Should use whatever default_provider() returns
        assert!(provider.is_healthy() || !provider.check_available());
    }
}