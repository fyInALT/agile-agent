//! Model types and API data structures.

use serde::{Deserialize, Serialize};

/// Configuration for the LLM provider.
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// OpenAI API key
    pub api_key: String,
    /// Base URL for the OpenAI API (default: https://api.openai.com/v1)
    pub base_url: String,
    /// Model for simple/fast tasks
    pub simple_model: String,
    /// Model for complex/thinking tasks
    pub thinking_model: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl LlmConfig {
    /// Create a new config with the given API key and default settings.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
            simple_model: "gpt-4o-mini".to_string(),
            thinking_model: "gpt-4o".to_string(),
            timeout_secs: 60,
        }
    }

    /// Set the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the simple model.
    pub fn with_simple_model(mut self, model: impl Into<String>) -> Self {
        self.simple_model = model.into();
        self
    }

    /// Set the thinking model.
    pub fn with_thinking_model(mut self, model: impl Into<String>) -> Self {
        self.thinking_model = model.into();
        self
    }

    /// Set the timeout in seconds.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Get the model identifier for a model type.
    pub fn model_for(&self, model_type: ModelType) -> &str {
        match model_type {
            ModelType::Simple => &self.simple_model,
            ModelType::Thinking => &self.thinking_model,
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self::new(std::env::var("OPENAI_API_KEY").unwrap_or_default())
    }
}

/// Type of model to use for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelType {
    /// Simple/fast model for basic tasks
    Simple,
    /// Thinking model for complex tasks
    Thinking,
}

impl ModelType {
    /// Get the config key name for this model type.
    pub fn config_key(&self) -> &'static str {
        match self {
            ModelType::Simple => "simple_model",
            ModelType::Thinking => "thinking_model",
        }
    }
}

/// A chat message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
}

impl ChatMessage {
    /// Create a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    /// Create a new system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    /// Create a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Role of a chat message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Request body for the OpenAI chat completions API.
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl ChatRequest {
    /// Create a new chat request with the given model and messages.
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
            stream: None,
        }
    }

    /// Enable streaming.
    pub fn with_streaming(mut self) -> Self {
        self.stream = Some(true);
        self
    }

    /// Set temperature (0-2).
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp.clamp(0.0, 2.0));
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }
}

/// Response from the OpenAI chat completions API.
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

/// A single completion choice.
#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: ChatMessage,
    #[serde(rename = "finish_reason")]
    pub finish_reason: String,
}

/// Token usage information.
#[derive(Debug, Deserialize)]
pub struct Usage {
    #[serde(rename = "prompt_tokens")]
    pub prompt_tokens: u32,
    #[serde(rename = "completion_tokens")]
    pub completion_tokens: u32,
    #[serde(rename = "total_tokens")]
    pub total_tokens: u32,
}

/// Streaming response chunk from OpenAI API.
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub index: usize,
    #[serde(rename = "delta")]
    pub delta: Delta,
    #[serde(rename = "finish_reason")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Delta {
    #[serde(rename = "content")]
    pub content: Option<String>,
    #[serde(rename = "role")]
    pub role: Option<String>,
}

/// Error response from OpenAI API.
#[derive(Debug, Deserialize)]
pub struct ApiErrorResponse {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: Option<String>,
    #[serde(rename = "param")]
    pub param: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_user() {
        let msg = ChatMessage::user("hello");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "hello");
    }

    #[test]
    fn chat_request_with_streaming() {
        let req = ChatRequest::new("gpt-4", vec![ChatMessage::user("hi")])
            .with_streaming();
        assert_eq!(req.stream, Some(true));
    }

    #[test]
    fn model_type_config_key() {
        assert_eq!(ModelType::Simple.config_key(), "simple_model");
        assert_eq!(ModelType::Thinking.config_key(), "thinking_model");
    }

    #[test]
    fn config_model_for() {
        let config = LlmConfig::new("key".to_string())
            .with_simple_model("gpt-3.5-turbo")
            .with_thinking_model("gpt-4");

        assert_eq!(config.model_for(ModelType::Simple), "gpt-3.5-turbo");
        assert_eq!(config.model_for(ModelType::Thinking), "gpt-4");
    }
}
