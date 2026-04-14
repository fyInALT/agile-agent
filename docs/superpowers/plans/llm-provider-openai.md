# LLM Provider (OpenAI API) Implementation Plan

## Context

The user needs an LLM provider module that supports the OpenAI API for text summarization and simple comprehension tasks. The module must:

- Support async execution internally, with a blocking request interface for other modules
- Support two model tiers: a cheap/fast "simple model" and a more capable "thinking model"
- Be configured via config, allowing users to set model names and API keys
- Be an independent crate (`agent-llm-provider`) for use across the workspace

## Architecture

### Package Structure

```
llm-provider/                      # New crate: agent-llm-provider
├── src/
│   ├── lib.rs                    # Public API exports
│   ├── client.rs                 # OpenAI API client
│   ├── models.rs                 # Model types and configuration
│   ├── error.rs                  # Error types
│   ├── text.rs                   # Text processing utilities
│   └── config.rs                 # Configuration handling
├── Cargo.toml
└── tests/
    └── integration_tests.rs
```

### Design Pattern

The provider follows a **async client with blocking facade** pattern:

- **Internally**: Uses `reqwest` for async HTTP and `tokio` runtime
- **Externally**: Exposes blocking `send()` and `send_streaming()` methods
- **Configuration**: Reads from config file or environment variables

### Key Types

```rust
// models.rs
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub simple_model: String,      // e.g., "gpt-4o-mini"
    pub thinking_model: String,    // e.g., "gpt-4o"
    pub timeout_secs: u64,
}

pub enum ModelType {
    Simple,
    Thinking,
}

// client.rs
pub struct LlmClient {
    config: LlmConfig,
    http_client: reqwest::Client,
}

impl LlmClient {
    // Blocking API for sync callers
    pub fn send(&self, prompt: &str, model: ModelType) -> Result<String>;
    pub fn send_streaming(&self, prompt: &str, model: ModelType, callback: F) -> Result<()>;

    // Async API for advanced users
    pub async fn send_async(&self, prompt: &str, model: ModelType) -> Result<String>;
}

// text.rs - Text processing utilities
pub struct TextProcessor;

impl TextProcessor {
    /// Summarize text using the thinking model
    pub fn summarize(&self, text: &str) -> Result<String>;

    /// Compress text using the simple model
    pub fn compress(&self, text: &str, max_tokens: usize) -> Result<String>;

    /// Extract key points using the thinking model
    pub fn extract_key_points(&self, text: &str) -> Result<Vec<String>>;
}
```

## Implementation Steps

### Step 1: Create the crate structure

Create `llm-provider/Cargo.toml` with dependencies:

```toml
[package]
name = "agent-llm-provider"
version = "0.1.0"
edition = "2024"

[dependencies]
reqwest = { version = "0.12", features = ["json", "stream"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
thiserror = "2"

[dev-dependencies]
tokio-test = "0.4"
```

### Step 2: Implement error types

Create `error.rs` with error types for:
- `ApiError`: OpenAI API errors (rate limit, auth, etc.)
- `NetworkError`: Connection/network issues
- `ConfigError`: Missing or invalid configuration
- `ParseError`: Response parsing errors

### Step 3: Implement model configuration

Create `models.rs` with:

- `LlmConfig` struct with api_key, base_url, simple_model, thinking_model, timeout_secs
- `ModelType` enum
- `LlmRequest` and `LlmResponse` types matching OpenAI API schema
- `ChatMessage` struct for conversation messages

### Step 4: Implement the OpenAI client

Create `client.rs` with:

- `LlmClient::new(config: LlmConfig) -> Self`
- `send(&self, prompt: &str, model: ModelType) -> Result<String>` (blocking)
- `send_streaming(&self, prompt: &str, model: ModelType, callback: F) -> Result<()>` (blocking with callback)
- `async fn send_async(&self, prompt: &str, model: ModelType) -> Result<String>`
- `async fn send_streaming_async(&self, prompt: &str, model: ModelType) -> impl Stream<Item=Result<String>>>`

The blocking methods use `tokio::runtime::Runtime::current()` internally.

### Step 5: Implement text processing utilities

Create `text.rs` with:

- `TextProcessor::new(client: LlmClient) -> Self`
- `summarize(&self, text: &str) -> Result<String>` - uses thinking model
- `compress(&self, text: &str, max_tokens: usize) -> Result<String>` - uses simple model
- `extract_key_points(&self, text: &str) -> Result<Vec<String>>` - uses thinking model

### Step 6: Implement configuration loading

Create `config.rs` with:

- `LlmConfig::from_file(path: &Path) -> Result<Self>`
- `LlmConfig::from_env() -> Result<Self>` - reads from OPENAI_API_KEY, OPENAI_BASE_URL, etc.
- `LlmConfig::default() -> Result<Self>` - uses sensible defaults with env vars

### Step 7: Write tests

Add unit tests in each module and integration tests:

- `test_send_simple_prompt()`
- `test_summarize_text()`
- `test_streaming_response()`
- `test_config_from_env()`

### Step 8: Add to workspace

Update `Cargo.toml`:

```toml
[workspace]
members = [
    "cli",
    "core",
    "kanban",
    "llm-provider",  # Add new crate
    "test-support",
    "tui",
]
```

## Design Decisions

### Why blocking facade with async internals?

Other modules (like kanban) use synchronous patterns. The blocking facade allows them to call `client.send()` without needing to manage an async runtime, while the internal implementation remains fully async.

### Why tokio runtime inside blocking methods?

Uses `tokio::runtime::Handle::current()` to access the existing runtime, or creates a new single-thread runtime for blocking contexts. This pattern is common in Rust crates that provide both sync and async APIs.

### Why two model types?

Simple model (e.g., gpt-4o-mini) for:
- Quick classification
- Basic extraction
- Compression tasks
- Cost-sensitive operations

Thinking model (e.g., gpt-4o) for:
- Summarization
- Complex analysis
- Multi-step reasoning
- Quality-critical tasks

### Why reqwest over ureq?

`reqwest` supports async natively and has better streaming support. Since we need async internals anyway, `reqwest` is the natural choice.

## Verification

After implementation:

1. All unit tests pass: `cargo test -p agent-llm-provider`
2. All doc tests pass: `cargo test --doc -p agent-llm-provider`
3. Code compiles clean with no warnings
4. Integration test with actual API (with valid key) produces expected results

## References

- [OpenAI API Documentation](https://platform.openai.com/docs/api-reference)
- [reqwest crate](https://docs.rs/reqwest/)
- [tokio runtime](https://tokio.rs/tokio/runtime/runtime)
