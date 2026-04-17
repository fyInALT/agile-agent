use thiserror::Error;

use crate::provider::ProviderKind;

/// Errors that can occur during launch config parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("empty input")]
    EmptyInput,

    #[error("empty key in environment variable")]
    EmptyKey,

    #[error("invalid key format: {0}")]
    InvalidKeyFormat(String),

    #[error("no executable found in command fragment")]
    NoExecutableFound,

    #[error("input too long: max {max} bytes, got {actual}")]
    InputTooLong { max: usize, actual: usize },

    #[error("too many environment variables: max {max}, got {actual}")]
    TooManyEnvVars { max: usize, actual: usize },

    #[error("line too long: max {max} bytes, got {actual} at line {line}")]
    LineTooLong {
        max: usize,
        actual: usize,
        line: usize,
    },
}

/// Errors that can occur during launch config validation.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("provider mismatch: selected {:?} but found executable {found}", .selected)]
    ProviderMismatch {
        selected: ProviderKind,
        found: String,
    },

    #[error("reserved argument conflict: {arg} is reserved for provider {provider:?}", arg = .0, provider = .1)]
    ReservedArgumentConflict(String, ProviderKind),

    #[error("mock provider does not support launch config overrides")]
    MockProviderNoOverrides,

    #[error("invalid provider: {0}")]
    InvalidProvider(String),
}

/// Result type for parse operations.
pub type ParseResult<T> = Result<T, ParseError>;

/// Result type for validation operations.
pub type ValidationResult = Result<(), ValidationError>;
