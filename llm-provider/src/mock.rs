//! Mock LLM provider for testing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::models::ModelType;
use crate::provider::{LlmProvider, LlmResponse, LlmStreamChunk, LlmUsage};

/// Configuration for the mock LLM provider.
#[derive(Debug, Clone)]
pub struct MockLlmConfig {
    /// Response to return for all requests
    pub response: String,
    /// Simulated delay in milliseconds (0 for no delay)
    pub delay_ms: u64,
    /// Whether to fail requests
    pub should_fail: bool,
    /// Error message if should_fail is true
    pub error_message: String,
    /// Token count for the response
    pub token_count: usize,
    /// Prompt token count (for usage tracking)
    pub prompt_tokens: u32,
}

impl Default for MockLlmConfig {
    fn default() -> Self {
        Self {
            response: "Mock response".to_string(),
            delay_ms: 0,
            should_fail: false,
            error_message: "Mock error".to_string(),
            token_count: 5,
            prompt_tokens: 10,
        }
    }
}

/// A mock LLM provider for testing.
///
/// This provider can be configured to return specific responses,
/// simulate delays, or fail on demand.
///
/// # Example
///
/// ```rust
/// use agent_llm_provider::mock::MockLlmProvider;
/// use agent_llm_provider::provider::LlmProvider;
/// use agent_llm_provider::models::ModelType;
///
/// let mock = MockLlmProvider::new()
///     .with_response("Hello, world!".to_string())
///     .with_delay(0);
///
/// let response = mock.complete("Say hello").unwrap();
/// assert_eq!(response.content, "Hello, world!");
/// ```
#[derive(Debug, Clone)]
pub struct MockLlmProvider {
    config: Arc<Mutex<MockLlmConfig>>,
    /// Call count for each model type
    call_counts: Arc<Mutex<std::collections::HashMap<ModelType, usize>>>,
    /// Last prompt received
    last_prompt: Arc<Mutex<Option<String>>>,
    /// Last model type used
    last_model: Arc<Mutex<Option<ModelType>>>,
}

impl MockLlmProvider {
    /// Create a new mock provider with default config.
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(MockLlmConfig::default())),
            call_counts: Arc::new(Mutex::new(std::collections::HashMap::new())),
            last_prompt: Arc::new(Mutex::new(None)),
            last_model: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a mock provider with a specific response.
    pub fn with_response(self, response: impl Into<String>) -> Self {
        {
            let mut config = self.config.lock().unwrap();
            config.response = response.into();
        }
        self
    }

    /// Create a mock provider that returns different responses based on model type.
    pub fn with_responses(self, simple: impl Into<String>, thinking: impl Into<String>) -> Self {
        {
            let mut config = self.config.lock().unwrap();
            config.response = format!("SIMPLE:{};;THINKING:{}", simple.into(), thinking.into());
        }
        self
    }

    /// Set the simulated delay in milliseconds.
    pub fn with_delay(self, delay_ms: u64) -> Self {
        {
            let mut config = self.config.lock().unwrap();
            config.delay_ms = delay_ms;
        }
        self
    }

    /// Make the provider fail on all requests.
    pub fn with_error(self, message: impl Into<String>) -> Self {
        {
            let mut config = self.config.lock().unwrap();
            config.should_fail = true;
            config.error_message = message.into();
        }
        self
    }

    /// Set the token count for responses.
    pub fn with_tokens(self, count: usize) -> Self {
        {
            let mut config = self.config.lock().unwrap();
            config.token_count = count;
        }
        self
    }

    /// Get the number of times this provider has been called.
    pub fn call_count(&self, model: ModelType) -> usize {
        self.call_counts
            .lock()
            .unwrap()
            .get(&model)
            .copied()
            .unwrap_or(0)
    }

    /// Get the total number of calls across all models.
    pub fn total_call_count(&self) -> usize {
        self.call_counts.lock().unwrap().values().sum()
    }

    /// Get the last prompt received.
    pub fn last_prompt(&self) -> Option<String> {
        self.last_prompt.lock().unwrap().clone()
    }

    /// Get the last model type used.
    pub fn last_model(&self) -> Option<ModelType> {
        *self.last_model.lock().unwrap()
    }

    /// Reset call counts and last values.
    pub fn reset(&self) {
        self.call_counts.lock().unwrap().clear();
        *self.last_prompt.lock().unwrap() = None;
        *self.last_model.lock().unwrap() = None;
    }

    fn record_call(&self, prompt: &str, model: ModelType) {
        *self.last_prompt.lock().unwrap() = Some(prompt.to_string());
        *self.last_model.lock().unwrap() = Some(model);

        let mut counts = self.call_counts.lock().unwrap();
        *counts.entry(model).or_insert(0) += 1;
    }
}

impl Default for MockLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for MockLlmProvider {
    fn complete(&self, prompt: &str) -> Result<LlmResponse> {
        self.complete_with_model(prompt, ModelType::Simple)
    }

    fn complete_with_model(&self, prompt: &str, model: ModelType) -> Result<LlmResponse> {
        self.record_call(prompt, model);

        let config = self.config.lock().unwrap().clone();

        if config.should_fail {
            return Err(anyhow::anyhow!("{}", config.error_message));
        }

        // Apply delay if configured
        if config.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(config.delay_ms));
        }

        // Determine which response to use
        let response = if config.response.contains(";;") {
            let parts: Vec<&str> = config.response.split(";;").collect();
            let prefix = if matches!(model, ModelType::Thinking) {
                "THINKING:"
            } else {
                "SIMPLE:"
            };
            parts
                .iter()
                .find(|p| p.starts_with(prefix))
                .map(|p| p.strip_prefix(prefix).unwrap_or(p))
                .unwrap_or(&config.response)
                .to_string()
        } else {
            config.response.clone()
        };

        Ok(LlmResponse {
            content: response,
            usage: Some(LlmUsage {
                prompt_tokens: config.prompt_tokens,
                completion_tokens: config.token_count as u32,
                total_tokens: config.prompt_tokens + config.token_count as u32,
            }),
        })
    }

    fn complete_streaming<F>(&self, prompt: &str, callback: F) -> Result<()>
    where
        F: Fn(LlmStreamChunk) + Send + 'static,
    {
        self.complete_streaming_with_model(prompt, ModelType::Simple, callback)
    }

    fn complete_streaming_with_model<F>(
        &self,
        prompt: &str,
        model: ModelType,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(LlmStreamChunk) + Send + 'static,
    {
        self.record_call(prompt, model);

        let config = self.config.lock().unwrap().clone();

        if config.should_fail {
            return Err(anyhow::anyhow!("{}", config.error_message));
        }

        // Apply delay if configured
        if config.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(config.delay_ms));
        }

        // Stream the response word by word, marking last chunk as finished
        let words: Vec<&str> = config.response.split_whitespace().collect();
        let last_index = words.len().saturating_sub(1);
        for (i, word) in words.iter().enumerate() {
            callback(LlmStreamChunk {
                content: if i == 0 {
                    word.to_string()
                } else {
                    format!(" {}", word)
                },
                is_finished: i == last_index,
            });
        }

        Ok(())
    }

    async fn complete_async(&self, prompt: &str) -> Result<LlmResponse> {
        self.complete(prompt)
    }

    async fn complete_async_with_model(
        &self,
        prompt: &str,
        model: ModelType,
    ) -> Result<LlmResponse> {
        self.complete_with_model(prompt, model)
    }
}

/// A simple in-memory mock that just echoes back the prompt.
#[derive(Debug, Clone, Default)]
pub struct EchoMockProvider {
    call_count: Arc<AtomicUsize>,
}

impl EchoMockProvider {
    pub fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl LlmProvider for EchoMockProvider {
    fn complete(&self, prompt: &str) -> Result<LlmResponse> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(LlmResponse {
            content: format!("Echo: {}", prompt),
            usage: Some(LlmUsage {
                prompt_tokens: 5,
                completion_tokens: 5,
                total_tokens: 10,
            }),
        })
    }

    fn complete_with_model(&self, prompt: &str, _model: ModelType) -> Result<LlmResponse> {
        self.complete(prompt)
    }

    fn complete_streaming<F>(&self, prompt: &str, callback: F) -> Result<()>
    where
        F: Fn(LlmStreamChunk) + Send + 'static,
    {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let text = format!("Echo: {}", prompt);
        for (i, c) in text.chars().enumerate() {
            callback(LlmStreamChunk {
                content: c.to_string(),
                is_finished: i == text.len() - 1,
            });
        }
        Ok(())
    }

    fn complete_streaming_with_model<F>(
        &self,
        prompt: &str,
        _model: ModelType,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(LlmStreamChunk) + Send + 'static,
    {
        self.complete_streaming(prompt, callback)
    }

    async fn complete_async(&self, prompt: &str) -> Result<LlmResponse> {
        self.complete(prompt)
    }

    async fn complete_async_with_model(
        &self,
        prompt: &str,
        model: ModelType,
    ) -> Result<LlmResponse> {
        self.complete_with_model(prompt, model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_provider_basic_response() {
        let mock = MockLlmProvider::new().with_response("Hello, world!".to_string());
        let response = mock.complete("Say hello").unwrap();
        assert_eq!(response.content, "Hello, world!");
    }

    #[test]
    fn test_mock_provider_records_last_prompt() {
        let mock = MockLlmProvider::new().with_response("Response".to_string());
        mock.complete("My prompt").unwrap();
        assert_eq!(mock.last_prompt(), Some("My prompt".to_string()));
    }

    #[test]
    fn test_mock_provider_records_last_model() {
        let mock = MockLlmProvider::new().with_response("Response".to_string());
        mock.complete_with_model("Prompt", ModelType::Thinking)
            .unwrap();
        assert_eq!(mock.last_model(), Some(ModelType::Thinking));
    }

    #[test]
    fn test_mock_provider_call_count() {
        let mock = MockLlmProvider::new().with_response("Response".to_string());
        assert_eq!(mock.call_count(ModelType::Simple), 0);

        mock.complete("1").unwrap();
        assert_eq!(mock.call_count(ModelType::Simple), 1);

        mock.complete_with_model("2", ModelType::Thinking).unwrap();
        assert_eq!(mock.call_count(ModelType::Thinking), 1);
        assert_eq!(mock.total_call_count(), 2);
    }

    #[test]
    fn test_mock_provider_error() {
        let mock = MockLlmProvider::new().with_error("Test error");
        let result = mock.complete("Any prompt");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Test error");
    }

    #[test]
    fn test_mock_provider_reset() {
        let mock = MockLlmProvider::new().with_response("Response".to_string());
        mock.complete("1").unwrap();
        mock.complete("2").unwrap();
        assert_eq!(mock.total_call_count(), 2);

        mock.reset();
        assert_eq!(mock.total_call_count(), 0);
        assert_eq!(mock.last_prompt(), None);
    }

    #[test]
    fn test_mock_provider_streaming() {
        let mock = MockLlmProvider::new().with_response("Hello world".to_string());
        let chunks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let chunks_clone = Arc::clone(&chunks);

        mock.complete_streaming("Say something", move |chunk| {
            chunks_clone.lock().unwrap().push(chunk);
        })
        .unwrap();

        let chunks = chunks.lock().unwrap();
        // Should have multiple chunks for "Hello world"
        assert!(chunks.len() > 1);
        // Last chunk should be marked as finished
        assert!(chunks.last().unwrap().is_finished);
    }

    #[test]
    fn test_echo_mock() {
        let echo = EchoMockProvider::new();
        let response = echo.complete("test").unwrap();
        assert_eq!(response.content, "Echo: test");
        assert_eq!(echo.call_count(), 1);
    }

    #[test]
    fn test_mock_provider_different_responses_by_model() {
        let mock =
            MockLlmProvider::new().with_responses("Simple response", "Complex thinking response");

        let simple = mock
            .complete_with_model("prompt", ModelType::Simple)
            .unwrap();
        assert!(simple.content.contains("Simple"));

        let thinking = mock
            .complete_with_model("prompt", ModelType::Thinking)
            .unwrap();
        assert!(thinking.content.contains("Complex"));
    }
}
