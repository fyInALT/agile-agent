//! OpenAI API client implementation.

use std::time::Duration;

use anyhow::Result;
use futures_util::StreamExt;
use reqwest::Client;
use tokio::runtime::Handle;

use crate::error::LlmError;
use crate::models::{
    ChatMessage, ChatRequest, ChatResponse, LlmConfig, ModelType, StreamChunk,
};

type StdResult<T, E> = std::result::Result<T, E>;

/// OpenAI API client for making LLM requests.
#[derive(Debug, Clone)]
pub struct LlmClient {
    config: LlmConfig,
    http_client: Client,
}

impl LlmClient {
    /// Create a new LLM client with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("reqwest client should build");
        Self {
            config,
            http_client,
        }
    }

    /// Create a new client from environment variables.
    pub fn from_env() -> Result<Self> {
        let config = LlmConfig::default();
        if config.api_key.is_empty() {
            return Err(LlmError::Config("OPENAI_API_KEY not set".to_string()).into());
        }
        Ok(Self::new(config))
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &LlmConfig {
        &self.config
    }

    /// Send a simple prompt and get a blocking response.
    ///
    /// Uses the simple model by default.
    pub fn send(&self, prompt: &str) -> Result<String> {
        self.send_with_model(prompt, ModelType::Simple)
    }

    /// Send a prompt with a specific model type.
    pub fn send_with_model(&self, prompt: &str, model: ModelType) -> Result<String> {
        let handle = Handle::current();
        handle.block_on(self.send_async_with_model(prompt, model))
    }

    /// Send a streaming request with a callback for each chunk.
    ///
    /// The callback is called for each content chunk as it's received.
    pub fn send_streaming<F>(&self, prompt: &str, callback: F) -> Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        self.send_streaming_with_model(prompt, ModelType::Simple, callback)
    }

    /// Send a streaming request with a specific model type.
    pub fn send_streaming_with_model<F>(&self, prompt: &str, model: ModelType, callback: F) -> Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        let handle = Handle::current();
        handle.block_on(self.send_streaming_async_with_model(prompt, model, callback))
    }

    /// Async version: Send a prompt and get a response.
    pub async fn send_async(&self, prompt: &str) -> Result<String> {
        self.send_async_with_model(prompt, ModelType::Simple).await
    }

    /// Async version: Send a prompt with a specific model type.
    pub async fn send_async_with_model(&self, prompt: &str, model: ModelType) -> Result<String> {
        let message = ChatMessage::user(prompt);
        let request = ChatRequest::new(self.config.model_for(model), vec![message]);

        let response = self.execute_request(request).await?;

        Ok(response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    /// Async version: Send a streaming request and process chunks via callback.
    pub async fn send_streaming_async<F>(&self, prompt: &str, callback: F) -> Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        self.send_streaming_async_with_model(prompt, ModelType::Simple, callback)
            .await
    }

    /// Async version: Send a streaming request with specific model type.
    pub async fn send_streaming_async_with_model<F>(
        &self,
        prompt: &str,
        model: ModelType,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(String) + Send + 'static,
    {
        let message = ChatMessage::user(prompt);
        let request = ChatRequest::new(self.config.model_for(model), vec![message])
            .with_streaming();

        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(LlmError::Network)?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow::anyhow!("HTTP error: {}", status));
        }

        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    // Parse SSE lines
                    for line in text.lines() {
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                return Ok(());
                            }
                            if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                                if let Some(content) = &chunk.choices.first().and_then(|c| c.delta.content.as_ref()) {
                                    callback(content.to_string());
                                }
                            }
                        }
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Network error: {}", e)),
            }
        }

        Ok(())
    }

    async fn execute_request(&self, request: ChatRequest) -> StdResult<ChatResponse, LlmError> {
        let url = format!("{}/chat/completions", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(LlmError::Network)?;

        let status = response.status();

        if status.is_success() {
            response
                .json::<ChatResponse>()
                .await
                .map_err(|e| LlmError::Parse(e.to_string()))
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(LlmError::Api(format!("HTTP {}: {}", status, error_text)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_client_new() {
        let config = LlmConfig::new("test-key".to_string());
        let client = LlmClient::new(config);
        assert_eq!(client.config.api_key, "test-key");
    }
}
