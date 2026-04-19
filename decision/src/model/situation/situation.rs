//! Decision situation trait

use crate::core::types::{ActionType, SituationType, UrgencyLevel};
use serde::{Deserialize, Serialize};

/// Decision situation trait - extensible with debug info
pub trait DecisionSituation: Send + Sync + 'static {
    /// Situation type identifier
    fn situation_type(&self) -> SituationType;

    /// Concrete implementation type name (for debugging)
    fn implementation_type(&self) -> &'static str;

    /// Debug representation
    fn debug_info(&self) -> String {
        format!("{} ({})", self.implementation_type(), self.situation_type())
    }

    /// Whether requires human escalation
    fn requires_human(&self) -> bool;

    /// Human escalation urgency (if requires_human)
    fn human_urgency(&self) -> UrgencyLevel;

    /// Serialize for prompt
    fn to_prompt_text(&self) -> String;

    /// Available actions for this situation
    fn available_actions(&self) -> Vec<ActionType>;

    /// Get error info if this situation represents an error (for rate limit detection)
    fn error_info(&self) -> Option<&super::ErrorInfo> {
        None
    }

    /// Serialize parameters for persistence (optional)
    fn serialize_params(&self) -> Option<String> {
        None
    }

    /// Clone into boxed
    fn clone_boxed(&self) -> Box<dyn DecisionSituation>;
}

/// Choice option
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

impl ChoiceOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn with_description(self, description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..self
        }
    }
}

/// Completion progress
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CompletionProgress {
    pub completed_items: Vec<String>,
    pub remaining_items: Vec<String>,
    pub estimated_remaining_minutes: Option<u64>,
}

/// Error info
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub error_type: String,
    pub message: String,
    pub recoverable: bool,
    pub retry_count: u8,
}

impl ErrorInfo {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            recoverable: true,
            retry_count: 0,
        }
    }

    pub fn with_retry_count(self, count: u8) -> Self {
        Self {
            retry_count: count,
            ..self
        }
    }

    pub fn unrecoverable(self) -> Self {
        Self {
            recoverable: false,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choice_option_creation() {
        let opt = ChoiceOption::new("A", "Option A");
        assert_eq!(opt.id, "A");
        assert_eq!(opt.label, "Option A");
        assert_eq!(opt.description, None);
    }

    #[test]
    fn test_choice_option_with_description() {
        let opt = ChoiceOption::new("A", "Option A").with_description("Detailed description");
        assert_eq!(opt.description, Some("Detailed description".to_string()));
    }

    #[test]
    fn test_choice_option_serde() {
        let opt = ChoiceOption::new("A", "Option A").with_description("desc");
        let json = serde_json::to_string(&opt).unwrap();
        let parsed: ChoiceOption = serde_json::from_str(&json).unwrap();
        assert_eq!(opt, parsed);
    }

    #[test]
    fn test_completion_progress_default() {
        let progress = CompletionProgress::default();
        assert!(progress.completed_items.is_empty());
        assert!(progress.remaining_items.is_empty());
        assert_eq!(progress.estimated_remaining_minutes, None);
    }

    #[test]
    fn test_error_info_creation() {
        let err = ErrorInfo::new("timeout", "Connection timed out");
        assert_eq!(err.error_type, "timeout");
        assert_eq!(err.message, "Connection timed out");
        assert!(err.recoverable);
        assert_eq!(err.retry_count, 0);
    }

    #[test]
    fn test_error_info_with_retry_count() {
        let err = ErrorInfo::new("timeout", "Connection timed out").with_retry_count(2);
        assert_eq!(err.retry_count, 2);
    }

    #[test]
    fn test_error_info_unrecoverable() {
        let err = ErrorInfo::new("fatal", "Critical error").unrecoverable();
        assert!(!err.recoverable);
    }

    #[test]
    fn test_error_info_serde() {
        let err = ErrorInfo::new("timeout", "msg").with_retry_count(3);
        let json = serde_json::to_string(&err).unwrap();
        let parsed: ErrorInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);
    }
}
