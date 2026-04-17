//! # Agent LLM Provider
//!
//! A Rust client library for OpenAI's API with support for multiple model types
//! (simple/thinking) and async/sync interfaces.
//!
//! ## Features
//!
//! - Async internally with blocking facade for easy integration
//! - Support for simple and thinking model tiers
//! - Streaming response support
//! - Text processing utilities (summarize, compress, extract key points)
//! - Configuration via environment variables or config files
//! - Provider trait abstraction for easy testing with mock implementations
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use agent_llm_provider::{LlmClient, LlmConfig, ModelType, TextProcessor};
//!
//! // Create client with explicit config
//! let config = LlmConfig::new("your-api-key".to_string())
//!     .with_simple_model("gpt-4o-mini")
//!     .with_thinking_model("gpt-4o");
//! let client = LlmClient::new(config);
//!
//! // Using with model type
//! let _response = client.send_with_model("Hello?", ModelType::Simple);
//!
//! // Text processing
//! let processor = TextProcessor::new(client);
//! let _summary = processor.summarize("Long text to summarize...");
//! ```
//!
//! ## Using Traits for Testing
//!
//! Use the `LlmProvider` trait to abstract over implementations:
//!
//! ```rust,ignore
//! use agent_llm_provider::provider::{LlmProvider, LlmResponse};
//! use agent_llm_provider::mock::MockLlmProvider;
//! use agent_llm_provider::models::ModelType;
//!
//! // Your code depends on the trait, not the implementation
//! fn do_llm_stuff<P: LlmProvider>(provider: &P) -> LlmResponse {
//!     provider.complete("Hello").unwrap()
//! }
//!
//! // Test with mock
//! let mock = MockLlmProvider::new().with_response("Mock response");
//! let result = do_llm_stuff(&mock);
//! ```
//!
//! ## Configuration
//!
//! Set environment variables:
//! - `OPENAI_API_KEY`: Your API key (required)
//! - `OPENAI_BASE_URL`: API base URL (optional)
//! - `OPENAI_SIMPLE_MODEL`: Simple model name (optional, default: gpt-4o-mini)
//! - `OPENAI_THINKING_MODEL`: Thinking model name (optional, default: gpt-4o)
//! - `OPENAI_TIMEOUT_SECS`: Request timeout (optional, default: 60)

pub mod client;
pub mod config;
pub mod error;
pub mod mock;
pub mod models;
pub mod provider;
pub mod text;

// Re-export commonly used types
pub use client::LlmClient;
pub use config::ConfigFile;
pub use error::LlmError;
pub use models::{ChatMessage, LlmConfig, MessageRole, ModelType};
pub use provider::{LlmProvider, LlmResponse, LlmStreamChunk, LlmUsage};
pub use text::TextProcessor;
