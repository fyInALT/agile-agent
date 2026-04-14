//! Error types for the LLM provider.

use thiserror::Error;

/// Errors that can occur when interacting with the LLM provider.
#[derive(Error, Debug)]
pub enum LlmError {
    /// API-related errors (rate limit, auth, etc.)
    #[error("OpenAI API error: {0}")]
    Api(String),

    /// Network or connection issues
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Missing or invalid configuration
    #[error("Configuration error: {0}")]
    Config(String),

    /// Response parsing errors
    #[error("Parse error: {0}")]
    Parse(String),

    /// Rate limiting error with retry-after information
    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    /// Authentication error
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Invalid request error
    #[error("Invalid request: {0}")]
    BadRequest(String),
}

impl LlmError {
    /// Check if this error is retryable (e.g., rate limiting)
    pub fn is_retryable(&self) -> bool {
        matches!(self, LlmError::RateLimited { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_is_retryable() {
        let err = LlmError::RateLimited { retry_after: 60 };
        assert!(err.is_retryable());
    }

    #[test]
    fn api_error_is_not_retryable() {
        let err = LlmError::Api("invalid request".to_string());
        assert!(!err.is_retryable());
    }
}
