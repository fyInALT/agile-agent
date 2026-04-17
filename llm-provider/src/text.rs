//! Text processing utilities using the LLM.

use anyhow::Result;

use crate::models::ModelType;
use crate::provider::LlmProvider;

/// Text processor using LLM for various text operations.
#[derive(Debug, Clone)]
pub struct TextProcessor<P: LlmProvider> {
    provider: P,
}

impl<P: LlmProvider> TextProcessor<P> {
    /// Create a new text processor with the given provider.
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    /// Summarize the given text using the thinking model.
    ///
    /// Returns a concise summary of the input text.
    pub fn summarize(&self, text: &str) -> Result<String> {
        let prompt = format!("Summarize the following text concisely:\n\n{}", text);
        self.provider
            .complete_with_model(&prompt, ModelType::Thinking)
            .map(|r| r.content)
    }

    /// Async version: Summarize text.
    pub async fn summarize_async(&self, text: &str) -> Result<String> {
        let prompt = format!("Summarize the following text concisely:\n\n{}", text);
        self.provider
            .complete_async_with_model(&prompt, ModelType::Thinking)
            .await
            .map(|r| r.content)
    }

    /// Compress text to approximately the given number of tokens using the simple model.
    ///
    /// This is useful for reducing text length while preserving key information.
    pub fn compress(&self, text: &str, max_tokens: usize) -> Result<String> {
        let prompt = format!(
            "Compress the following text to approximately {} tokens, preserving key information:\n\n{}",
            max_tokens, text
        );
        self.provider
            .complete_with_model(&prompt, ModelType::Simple)
            .map(|r| r.content)
    }

    /// Async version: Compress text.
    pub async fn compress_async(&self, text: &str, max_tokens: usize) -> Result<String> {
        let prompt = format!(
            "Compress the following text to approximately {} tokens, preserving key information:\n\n{}",
            max_tokens, text
        );
        self.provider
            .complete_async_with_model(&prompt, ModelType::Simple)
            .await
            .map(|r| r.content)
    }

    /// Extract key points from text using the thinking model.
    ///
    /// Returns a list of key points as separate lines.
    pub fn extract_key_points(&self, text: &str) -> Result<Vec<String>> {
        let prompt = format!(
            "Extract the key points from the following text. Return each point on a separate line:\n\n{}",
            text
        );
        let response = self
            .provider
            .complete_with_model(&prompt, ModelType::Thinking)?;

        Ok(response
            .content
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Async version: Extract key points.
    pub async fn extract_key_points_async(&self, text: &str) -> Result<Vec<String>> {
        let prompt = format!(
            "Extract the key points from the following text. Return each point on a separate line:\n\n{}",
            text
        );
        let response = self
            .provider
            .complete_async_with_model(&prompt, ModelType::Thinking)
            .await?;

        Ok(response
            .content
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    /// Classify the text into a category using the simple model.
    ///
    /// Returns the predicted category.
    pub fn classify(&self, text: &str, categories: &[&str]) -> Result<String> {
        let categories_str = categories.join(", ");
        let prompt = format!(
            "Classify the following text into one of these categories: {}.\n\nText: {}\n\nCategory:",
            categories_str, text
        );
        self.provider
            .complete_with_model(&prompt, ModelType::Simple)
            .map(|r| r.content)
    }

    /// Async version: Classify text.
    pub async fn classify_async(&self, text: &str, categories: &[&str]) -> Result<String> {
        let categories_str = categories.join(", ");
        let prompt = format!(
            "Classify the following text into one of these categories: {}.\n\nText: {}\n\nCategory:",
            categories_str, text
        );
        self.provider
            .complete_async_with_model(&prompt, ModelType::Simple)
            .await
            .map(|r| r.content)
    }

    /// Rewrite text to match a target style using the thinking model.
    pub fn rewrite(&self, text: &str, style: &str) -> Result<String> {
        let prompt = format!(
            "Rewrite the following text in the style of {}:\n\n{}",
            style, text
        );
        self.provider
            .complete_with_model(&prompt, ModelType::Thinking)
            .map(|r| r.content)
    }

    /// Async version: Rewrite text.
    pub async fn rewrite_async(&self, text: &str, style: &str) -> Result<String> {
        let prompt = format!(
            "Rewrite the following text in the style of {}:\n\n{}",
            style, text
        );
        self.provider
            .complete_async_with_model(&prompt, ModelType::Thinking)
            .await
            .map(|r| r.content)
    }

    /// Count approximate tokens in text (rough estimate).
    ///
    /// This is a rough estimate based on word count.
    pub fn estimate_tokens(&self, text: &str) -> usize {
        // Rough estimate: ~4 characters per token on average
        text.chars().count() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockLlmProvider;

    #[test]
    fn estimate_tokens() {
        let mock = MockLlmProvider::new().with_response("x".to_string());
        let processor = TextProcessor::new(mock);

        // Simple test - just verify it doesn't panic
        let tokens = processor.estimate_tokens("Hello world");
        assert!(tokens > 0);
    }
}
