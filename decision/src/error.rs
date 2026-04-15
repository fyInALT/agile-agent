//! Decision layer errors

use thiserror::Error;

/// Decision layer error types
#[derive(Debug, Error)]
pub enum DecisionError {
    /// Situation type not found in registry
    #[error("Situation type not found: {0}")]
    SituationNotFound(String),

    /// Action type not found in registry
    #[error("Action type not found: {0}")]
    ActionNotFound(String),

    /// Failed to parse action from output
    #[error("Failed to parse action: {0}")]
    ActionParseError(String),

    /// Failed to parse response
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// Failed to serialize/deserialize decision
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Decision engine error
    #[error("Decision engine error: {0}")]
    EngineError(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    ConfigError(String),

    /// Blocking resolution failed
    #[error("Blocking resolution failed: {0}")]
    BlockingError(String),

    /// Persistence error
    #[error("Persistence error: {0}")]
    PersistenceError(String),

    /// Session pool exhausted
    #[error("Session pool exhausted: {0}")]
    SessionPoolExhausted(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded, waiting at position {0}")]
    RateLimitExceeded(usize),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Result type alias for decision layer
pub type Result<T> = std::result::Result<T, DecisionError>;