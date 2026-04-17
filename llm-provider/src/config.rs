//! Configuration loading utilities.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::models::LlmConfig;

/// Configuration file format.
#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    /// OpenAI API key
    pub api_key: Option<String>,
    /// Base URL for OpenAI API
    pub base_url: Option<String>,
    /// Simple/fast model name
    pub simple_model: Option<String>,
    /// Thinking/complex model name
    pub thinking_model: Option<String>,
    /// Request timeout in seconds
    pub timeout_secs: Option<u64>,
}

impl ConfigFile {
    /// Load configuration from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        toml::from_str(&contents).map_err(|e| anyhow::anyhow!("Failed to parse config: {}", e))
    }
}

impl From<ConfigFile> for LlmConfig {
    fn from(file: ConfigFile) -> Self {
        let api_key = file
            .api_key
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .unwrap_or_default();
        let base_url = file
            .base_url
            .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        let simple_model = file
            .simple_model
            .or_else(|| std::env::var("OPENAI_SIMPLE_MODEL").ok())
            .unwrap_or_else(|| "gpt-4o-mini".to_string());
        let thinking_model = file
            .thinking_model
            .or_else(|| std::env::var("OPENAI_THINKING_MODEL").ok())
            .unwrap_or_else(|| "gpt-4o".to_string());
        let timeout_secs = file.timeout_secs.unwrap_or(60);

        LlmConfig {
            api_key,
            base_url,
            simple_model,
            thinking_model,
            timeout_secs,
        }
    }
}

impl LlmConfig {
    /// Load configuration from environment variables.
    ///
    /// Environment variables:
    /// - `OPENAI_API_KEY`: API key (required)
    /// - `OPENAI_BASE_URL`: Base URL (optional, defaults to OpenAI)
    /// - `OPENAI_SIMPLE_MODEL`: Simple model name (optional)
    /// - `OPENAI_THINKING_MODEL`: Thinking model name (optional)
    /// - `OPENAI_TIMEOUT_SECS`: Timeout in seconds (optional)
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?;

        Ok(Self {
            api_key,
            base_url: std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            simple_model: std::env::var("OPENAI_SIMPLE_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            thinking_model: std::env::var("OPENAI_THINKING_MODEL")
                .unwrap_or_else(|_| "gpt-4o".to_string()),
            timeout_secs: std::env::var("OPENAI_TIMEOUT_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
        })
    }

    /// Load configuration from a file.
    ///
    /// The file can be TOML or JSON. Environment variables take precedence.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let file = ConfigFile::from_file(path)?;
        Ok(file.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_defaults() {
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
    }

    #[test]
    fn config_file_with_values() {
        let file = ConfigFile {
            api_key: Some("my-key".to_string()),
            base_url: Some("https://custom.api.com/v1".to_string()),
            simple_model: Some("gpt-3.5-turbo".to_string()),
            thinking_model: Some("gpt-4".to_string()),
            timeout_secs: Some(120),
        };
        let config: LlmConfig = file.into();
        assert_eq!(config.api_key, "my-key");
        assert_eq!(config.base_url, "https://custom.api.com/v1");
        assert_eq!(config.simple_model, "gpt-3.5-turbo");
        assert_eq!(config.thinking_model, "gpt-4");
        assert_eq!(config.timeout_secs, 120);
    }
}
