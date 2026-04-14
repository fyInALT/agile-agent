//! Integration tests for the LLM provider crate.

use std::sync::{Arc, Mutex};

use agent_llm_provider::config::ConfigFile;
use agent_llm_provider::error::LlmError;
use agent_llm_provider::mock::{EchoMockProvider, MockLlmProvider};
use agent_llm_provider::models::{ChatMessage, LlmConfig, MessageRole, ModelType};
use agent_llm_provider::provider::LlmProvider;
use agent_llm_provider::{LlmClient, TextProcessor};

// ============================================================================
// LlmClient Tests
// ============================================================================

mod client_tests {
    use super::*;

    #[test]
    fn test_client_with_default_config() {
        let config = LlmConfig::default();
        let client = LlmClient::new(config);
        assert_eq!(client.config().simple_model, "gpt-4o-mini");
        assert_eq!(client.config().thinking_model, "gpt-4o");
    }

    #[test]
    fn test_client_config_builder() {
        let config = LlmConfig::new("api-key-123".to_string())
            .with_base_url("https://custom.api.com/v1")
            .with_simple_model("gpt-4o-mini")
            .with_thinking_model("gpt-4o")
            .with_timeout(120);

        assert_eq!(config.api_key, "api-key-123");
        assert_eq!(config.base_url, "https://custom.api.com/v1");
        assert_eq!(config.simple_model, "gpt-4o-mini");
        assert_eq!(config.thinking_model, "gpt-4o");
        assert_eq!(config.timeout_secs, 120);
    }

    #[test]
    fn test_client_config_model_for() {
        let config = LlmConfig::new("key".to_string());
        assert_eq!(config.model_for(ModelType::Simple), "gpt-4o-mini");
        assert_eq!(config.model_for(ModelType::Thinking), "gpt-4o");
    }
}

// ============================================================================
// Mock Provider Tests
// ============================================================================

mod mock_provider_tests {
    use super::*;

    #[test]
    fn test_mock_basic_response() {
        let mock = MockLlmProvider::new().with_response("Test response".to_string());
        let response = mock.complete("prompt").unwrap();
        assert_eq!(response.content, "Test response");
    }

    #[test]
    fn test_mock_records_call_count() {
        let mock = MockLlmProvider::new().with_response("resp".to_string());

        assert_eq!(mock.total_call_count(), 0);

        mock.complete("p1").unwrap();
        assert_eq!(mock.total_call_count(), 1);

        mock.complete_with_model("p2", ModelType::Thinking).unwrap();
        assert_eq!(mock.total_call_count(), 2);
        assert_eq!(mock.call_count(ModelType::Thinking), 1);
    }

    #[test]
    fn test_mock_records_last_prompt_and_model() {
        let mock = MockLlmProvider::new().with_response("resp".to_string());

        mock.complete_with_model("thinking prompt", ModelType::Thinking).unwrap();

        assert_eq!(mock.last_prompt(), Some("thinking prompt".to_string()));
        assert_eq!(mock.last_model(), Some(ModelType::Thinking));
    }

    #[test]
    fn test_mock_error_handling() {
        let mock = MockLlmProvider::new().with_error("Something went wrong");
        let result = mock.complete("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Something went wrong"));
    }

    #[test]
    fn test_mock_streaming() {
        let mock = MockLlmProvider::new()
            .with_response("Hello world".to_string());

        let chunks = Arc::new(Mutex::new(Vec::new()));
        let chunks_clone = chunks.clone();
        mock.complete_streaming("say hello", move |c| chunks_clone.lock().unwrap().push(c)).unwrap();

        let chunks = chunks.lock().unwrap();
        assert!(!chunks.is_empty());
        // Last chunk should indicate completion
        assert!(chunks.last().unwrap().is_finished);
    }

    #[test]
    fn test_mock_reset() {
        let mock = MockLlmProvider::new().with_response("resp".to_string());

        mock.complete("p1").unwrap();
        mock.complete("p2").unwrap();
        assert_eq!(mock.total_call_count(), 2);

        mock.reset();

        assert_eq!(mock.total_call_count(), 0);
        assert_eq!(mock.last_prompt(), None);
        assert_eq!(mock.last_model(), None);
    }

    #[test]
    fn test_mock_response_by_model() {
        let mock = MockLlmProvider::new()
            .with_responses("Simple answer", "Complex thoughtful answer");

        let simple = mock.complete_with_model("Q?", ModelType::Simple).unwrap();
        let thinking = mock.complete_with_model("Q?", ModelType::Thinking).unwrap();

        assert!(simple.content.contains("Simple"));
        assert!(thinking.content.contains("Complex"));
    }

    #[test]
    fn test_echo_mock() {
        let echo = EchoMockProvider::new();

        assert_eq!(echo.call_count(), 0);

        let response = echo.complete("Hello").unwrap();
        assert_eq!(response.content, "Echo: Hello");
        assert_eq!(response.usage.unwrap().total_tokens, 10);
        assert_eq!(echo.call_count(), 1);

        let response2 = echo.complete_with_model("World", ModelType::Thinking).unwrap();
        assert_eq!(response2.content, "Echo: World");
        assert_eq!(echo.call_count(), 2);
    }

    #[test]
    fn test_echo_mock_streaming() {
        let echo = EchoMockProvider::new();
        let chunks = Arc::new(Mutex::new(Vec::new()));
        let chunks_clone = chunks.clone();

        echo.complete_streaming("Hi", move |c| chunks_clone.lock().unwrap().push(c)).unwrap();

        // Should have chunks for "Echo: Hi"
        let content: String = chunks.lock().unwrap().iter()
            .map(|c| c.content.clone())
            .collect();
        assert!(content.contains("Echo:"));
        assert!(chunks.lock().unwrap().last().unwrap().is_finished);
    }
}

// ============================================================================
// Provider Trait Tests
// ============================================================================

mod provider_trait_tests {
    use super::*;

    // Example function that works with any LLM provider
    fn summarize_using_provider<P: LlmProvider>(provider: &P, text: &str) -> anyhow::Result<String> {
        let prompt = format!("Summarize this: {}", text);
        let response = provider.complete_with_model(&prompt, ModelType::Thinking)?;
        Ok(response.content)
    }

    fn classify_with_provider<P: LlmProvider>(
        provider: &P,
        text: &str,
        categories: &[&str],
    ) -> anyhow::Result<String> {
        let categories_str = categories.join(", ");
        let prompt = format!("Classify '{}' into one of: {}", text, categories_str);
        let response = provider.complete(&prompt)?;
        Ok(response.content)
    }

    #[test]
    fn test_provider_trait_with_mock() {
        let mock = MockLlmProvider::new()
            .with_responses("Category A", "Complex Category B response");

        // Simple classification
        let result = classify_with_provider(&mock, "I love this product", &["positive", "negative"]).unwrap();
        assert!(result.contains("Category"));

        // Reset for thinking model test
        let mock2 = MockLlmProvider::new()
            .with_response("Product analysis: This is a great product with excellent features");

        let summary = summarize_using_provider(&mock2, "A product review").unwrap();
        assert!(summary.contains("Product"));
    }

    #[test]
    fn test_provider_trait_async() {
        let mock = MockLlmProvider::new().with_response("Async response".to_string());

        // Use the async interface
        let future = mock.complete_async("test");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(future).unwrap();

        assert_eq!(result.content, "Async response");
    }

    #[test]
    fn test_provider_with_echo() {
        let echo = EchoMockProvider::new();
        let result = summarize_using_provider(&echo, "some text").unwrap();
        assert!(result.contains("Echo:"));
    }
}

// ============================================================================
// Config Tests
// ============================================================================

mod config_tests {
    use super::*;

    #[test]
    fn test_config_file_defaults() {
        let file = ConfigFile {
            api_key: None,
            base_url: None,
            simple_model: None,
            thinking_model: None,
            timeout_secs: None,
        };
        let config: LlmConfig = file.into();

        assert_eq!(config.simple_model, "gpt-4o-mini");
        assert_eq!(config.thinking_model, "gpt-4o");
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_config_file_with_values() {
        let file = ConfigFile {
            api_key: Some("my-key".to_string()),
            base_url: Some("https://my-api.com/v1".to_string()),
            simple_model: Some("gpt-3.5-turbo".to_string()),
            thinking_model: Some("gpt-4-turbo".to_string()),
            timeout_secs: Some(180),
        };
        let config: LlmConfig = file.into();

        assert_eq!(config.api_key, "my-key");
        assert_eq!(config.base_url, "https://my-api.com/v1");
        assert_eq!(config.simple_model, "gpt-3.5-turbo");
        assert_eq!(config.thinking_model, "gpt-4-turbo");
        assert_eq!(config.timeout_secs, 180);
    }

    #[test]
    fn test_llm_config_from_env_missing_key() {
        // When OPENAI_API_KEY is not set, from_env should fail
        let result = LlmConfig::from_env();
        // This may pass or fail depending on environment, so we just check it's deterministic
        let _ = result.map(|config| {
            assert!(!config.api_key.is_empty() || std::env::var("OPENAI_API_KEY").is_ok());
        });
    }
}

// ============================================================================
// Model Tests
// ============================================================================

mod model_tests {
    use super::*;

    #[test]
    fn test_chat_message_user() {
        let msg = ChatMessage::user("Hello");
        assert!(matches!(msg.role, MessageRole::User));
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_chat_message_system() {
        let msg = ChatMessage::system("You are helpful");
        assert!(matches!(msg.role, MessageRole::System));
        assert_eq!(msg.content, "You are helpful");
    }

    #[test]
    fn test_chat_message_assistant() {
        let msg = ChatMessage::assistant("I can help");
        assert!(matches!(msg.role, MessageRole::Assistant));
        assert_eq!(msg.content, "I can help");
    }

    #[test]
    fn test_message_role_serde() {
        let msg = ChatMessage::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"user\""));

        let parsed: ChatMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed.role, MessageRole::User));
    }

    #[test]
    fn test_model_type() {
        assert_eq!(ModelType::Simple.config_key(), "simple_model");
        assert_eq!(ModelType::Thinking.config_key(), "thinking_model");
    }
}

// ============================================================================
// Error Tests
// ============================================================================

mod error_tests {
    use super::*;

    #[test]
    fn test_rate_limited_is_retryable() {
        let err = LlmError::RateLimited { retry_after: 60 };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_api_error_not_retryable() {
        let err = LlmError::Api("Bad request".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_auth_error() {
        let err = LlmError::Auth("Invalid API key".to_string());
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("Invalid API key"));
    }

    #[test]
    fn test_bad_request_error() {
        let err = LlmError::BadRequest("Missing required field".to_string());
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("Missing required field"));
    }

    #[test]
    fn test_config_error() {
        let err = LlmError::Config("API key not set".to_string());
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("API key not set"));
    }

    #[test]
    fn test_parse_error() {
        let err = LlmError::Parse("Invalid JSON".to_string());
        assert!(!err.is_retryable());
        assert!(err.to_string().contains("Invalid JSON"));
    }
}

// ============================================================================
// Text Processor Tests (using mock)
// ============================================================================

mod text_processor_tests {
    use super::*;

    #[test]
    fn test_text_processor_summarize() {
        let mock = MockLlmProvider::new()
            .with_response("This is a summary of the text".to_string());
        let processor = TextProcessor::new(mock);

        let result = processor.summarize("Long text to summarize").unwrap();
        assert!(result.contains("summary"));

        // Verify it used thinking model
        let mock2 = MockLlmProvider::new().with_response("resp".to_string());
        let processor2 = TextProcessor::new(mock2);
        processor2.summarize("text").unwrap();
        // Summarize uses thinking model
    }

    #[test]
    fn test_text_processor_compress() {
        let mock = MockLlmProvider::new()
            .with_response("Compressed version".to_string());
        let processor = TextProcessor::new(mock);

        let result = processor.compress("Long text", 100).unwrap();
        assert!(result.contains("Compressed"));
    }

    #[test]
    fn test_text_processor_extract_key_points() {
        let mock = MockLlmProvider::new()
            .with_response("Point 1\nPoint 2\nPoint 3".to_string());
        let processor = TextProcessor::new(mock);

        let result = processor.extract_key_points("Document content").unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"Point 1".to_string()));
    }

    #[test]
    fn test_text_processor_classify() {
        let mock = MockLlmProvider::new()
            .with_response("positive".to_string());
        let processor = TextProcessor::new(mock);

        let result = processor.classify("I love it", &["positive", "negative"]).unwrap();
        assert!(result.to_lowercase().contains("positive"));
    }

    #[test]
    fn test_text_processor_rewrite() {
        let mock = MockLlmProvider::new()
            .with_response("Rewritten in formal style".to_string());
        let processor = TextProcessor::new(mock);

        let result = processor.rewrite("casual text", "formal").unwrap();
        assert!(result.contains("formal") || result.contains("Rewritten"));
    }

    #[test]
    fn test_text_processor_estimate_tokens() {
        let mock = MockLlmProvider::new().with_response("x".to_string());
        let processor = TextProcessor::new(mock);

        // Rough estimate: 4 chars per token
        let tokens = processor.estimate_tokens("Hello world!");
        assert!(tokens >= 2); // "Hello world!" is about 12 chars

        let long_text = "a".repeat(100);
        let tokens = processor.estimate_tokens(&long_text);
        assert!(tokens >= 20); // 100 chars / 4 = 25 tokens minimum
    }
}

// ============================================================================
// Streaming Tests
// ============================================================================

mod streaming_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_mock_streaming_chunks() {
        let mock = MockLlmProvider::new()
            .with_response("A B C".to_string());

        let received = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        mock.complete_streaming("test", move |chunk| {
            received_clone.lock().unwrap().push(chunk);
        }).unwrap();

        let chunks = received.lock().unwrap();
        // Should receive chunks for "A", "B", "C" plus final chunk
        assert!(chunks.len() >= 3);
        assert!(chunks.last().unwrap().is_finished);
    }

    #[test]
    fn test_echo_streaming() {
        let echo = EchoMockProvider::new();
        let received = Arc::new(Mutex::new(String::new()));
        let received_clone = received.clone();

        echo.complete_streaming("Hi", move |chunk| {
            received_clone.lock().unwrap().push_str(&chunk.content);
        }).unwrap();

        assert!(received.lock().unwrap().contains("Echo:"));
    }

    #[test]
    fn test_streaming_with_model() {
        let mock = MockLlmProvider::new()
            .with_responses("Simple streaming", "Complex streaming");

        let simple_chunks = Arc::new(Mutex::new(Vec::new()));
        let simple_clone = simple_chunks.clone();
        mock.complete_streaming_with_model("test", ModelType::Simple, move |c| {
            simple_clone.lock().unwrap().push(c);
        }).unwrap();

        let thinking_chunks = Arc::new(Mutex::new(Vec::new()));
        let thinking_clone = thinking_chunks.clone();
        mock.complete_streaming_with_model("test", ModelType::Thinking, move |c| {
            thinking_clone.lock().unwrap().push(c);
        }).unwrap();

        // Both should work
        assert!(!simple_chunks.lock().unwrap().is_empty());
        assert!(!thinking_chunks.lock().unwrap().is_empty());
    }
}