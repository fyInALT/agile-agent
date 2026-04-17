//! LLM Caller trait for provider integration
//!
//! This trait allows the decision crate to call LLM providers without
//! depending directly on the provider infrastructure. Agent-core implements
//! this trait with actual provider calls.

use crate::error::DecisionError;

/// Trait for calling LLM providers
///
/// Implemented by agent-core with real provider calls.
/// The decision crate uses this to get LLM responses.
pub trait LLMCaller: Send + Sync {
    /// Call the LLM with a prompt and get a response
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to send to the LLM
    /// * `timeout_ms` - Timeout in milliseconds
    ///
    /// # Returns
    ///
    /// The LLM's response text, or an error if the call fails.
    fn call(&self, prompt: &str, timeout_ms: u64) -> Result<String, DecisionError>;

    /// Check if the caller is healthy and ready to make calls
    fn is_healthy(&self) -> bool;

    /// Get the caller's identifier for logging
    fn caller_id(&self) -> &str;
}

/// Mock LLM caller for testing
///
/// Returns predetermined responses based on prompt content.
#[derive(Debug, Clone)]
pub struct MockLLMCaller {
    /// Identifier for logging
    id: String,
}

impl MockLLMCaller {
    /// Create a new mock LLM caller
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl Default for MockLLMCaller {
    fn default() -> Self {
        Self::new("mock-llm")
    }
}

impl LLMCaller for MockLLMCaller {
    fn call(&self, prompt: &str, _timeout_ms: u64) -> Result<String, DecisionError> {
        // Return mock responses based on prompt content
        if prompt.contains("waiting_for_choice") || prompt.contains("Waiting for choice") {
            Ok("ACTION: select_option\nPARAMETERS: {\"option_id\": \"A\"}\nREASONING: First option is typically safest\nCONFIDENCE: 0.85".to_string())
        } else if prompt.contains("claims_completion") || prompt.contains("Claims completion") {
            Ok("ACTION: confirm_completion\nPARAMETERS: {}\nREASONING: Task appears complete based on output\nCONFIDENCE: 0.75".to_string())
        } else if prompt.contains("error") || prompt.contains("Error") {
            Ok("ACTION: retry\nPARAMETERS: {}\nREASONING: Retry may resolve transient error\nCONFIDENCE: 0.70".to_string())
        } else {
            Ok("ACTION: reflect\nPARAMETERS: {}\nREASONING: Need to analyze situation further\nCONFIDENCE: 0.60".to_string())
        }
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn caller_id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_llm_caller_returns_waiting_for_choice_response() {
        let caller = MockLLMCaller::new("test");
        let response = caller.call("waiting_for_choice situation", 1000).unwrap();
        assert!(response.contains("ACTION: select_option"));
    }

    #[test]
    fn mock_llm_caller_returns_claims_completion_response() {
        let caller = MockLLMCaller::new("test");
        let response = caller.call("Claims completion detected", 1000).unwrap();
        assert!(response.contains("ACTION: confirm_completion"));
    }

    #[test]
    fn mock_llm_caller_returns_error_response() {
        let caller = MockLLMCaller::new("test");
        let response = caller.call("Error occurred", 1000).unwrap();
        assert!(response.contains("ACTION: retry"));
    }

    #[test]
    fn mock_llm_caller_returns_reflect_response_for_unknown() {
        let caller = MockLLMCaller::new("test");
        let response = caller.call("unknown situation", 1000).unwrap();
        assert!(response.contains("ACTION: reflect"));
    }

    #[test]
    fn mock_llm_caller_is_healthy() {
        let caller = MockLLMCaller::new("test");
        assert!(caller.is_healthy());
    }

    #[test]
    fn mock_llm_caller_has_id() {
        let caller = MockLLMCaller::new("my-mock");
        assert_eq!(caller.caller_id(), "my-mock");
    }
}
