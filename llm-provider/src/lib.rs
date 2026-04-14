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
pub mod models;
pub mod text;

// Re-export commonly used types
pub use client::LlmClient;
pub use config::ConfigFile;
pub use error::LlmError;
pub use models::{ChatMessage, LlmConfig, ModelType, MessageRole};
pub use text::TextProcessor;
