//! LLM Provider abstraction trait.
//!
//! This module defines the `LlmProvider` trait that abstracts over different LLM
//! implementations. This allows for easy swapping between real providers (OpenAI)
//! and mock providers for testing.

use anyhow::Result;

use crate::models::ModelType;

/// Response from an LLM provider.
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The generated text content
    pub content: String,
    /// Token usage information (if available)
    pub usage: Option<LlmUsage>,
}

/// Token usage information from an LLM response.
#[derive(Debug, Clone)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// A chunk of content from a streaming response.
#[derive(Debug, Clone)]
pub struct LlmStreamChunk {
    /// The content delta
    pub content: String,
    /// Whether this is the last chunk
    pub is_finished: bool,
}

/// Trait for LLM providers.
///
/// This trait abstracts over different LLM implementations, allowing
/// code to work with any provider that implements this interface.
///
/// # Example
///
/// ```rust
/// use agent_llm_provider::provider::{LlmProvider, LlmResponse};
/// use agent_llm_provider::models::ModelType;
/// use anyhow::Result;
///
/// struct MyService<P: LlmProvider> {
///     provider: P,
/// }
///
/// impl<P: LlmProvider> MyService<P> {
///     pub fn summarize(&self, text: &str) -> Result<LlmResponse> {
///         self.provider.complete_with_model(&format!("Summarize: {}", text), ModelType::Thinking)
///     }
/// }
/// ```
pub trait LlmProvider: Send + Sync {
    /// Send a prompt and get a blocking response.
    ///
    /// Uses the simple model by default.
    fn complete(&self, prompt: &str) -> Result<LlmResponse>;

    /// Send a prompt with a specific model type.
    fn complete_with_model(&self, prompt: &str, model: ModelType) -> Result<LlmResponse>;

    /// Send a streaming request with a callback.
    ///
    /// The callback is called for each content chunk as it's received.
    fn complete_streaming<F>(&self, prompt: &str, callback: F) -> Result<()>
    where
        F: Fn(LlmStreamChunk) + Send + 'static;

    /// Send a streaming request with a specific model type.
    fn complete_streaming_with_model<F>(&self, prompt: &str, model: ModelType, callback: F) -> Result<()>
    where
        F: Fn(LlmStreamChunk) + Send + 'static;

    /// Async version: Send a prompt and get a response.
    fn complete_async(&self, prompt: &str) -> impl std::future::Future<Output = Result<LlmResponse>> + Send;

    /// Async version: Send a prompt with a specific model type.
    fn complete_async_with_model(
        &self,
        prompt: &str,
        model: ModelType,
    ) -> impl std::future::Future<Output = Result<LlmResponse>> + Send;
}
