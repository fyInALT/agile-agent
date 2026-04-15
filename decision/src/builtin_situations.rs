//! Built-in situation implementations

use crate::situation::{ChoiceOption, CompletionProgress, DecisionSituation, ErrorInfo};
use crate::situation_registry::SituationRegistry;
use crate::types::{ActionType, SituationType, UrgencyLevel};
use serde::{Deserialize, Serialize};

// Built-in situation type getters (functions instead of const)
pub fn waiting_for_choice() -> SituationType {
    SituationType::new("waiting_for_choice")
}

pub fn claims_completion() -> SituationType {
    SituationType::new("claims_completion")
}

pub fn partial_completion() -> SituationType {
    SituationType::new("partial_completion")
}

pub fn error() -> SituationType {
    SituationType::new("error")
}

// Provider-specific subtypes
pub fn claude_finished() -> SituationType {
    SituationType::with_subtype("finished", "claude")
}

pub fn codex_approval() -> SituationType {
    SituationType::with_subtype("waiting_for_choice", "codex")
}

pub fn acp_permission() -> SituationType {
    SituationType::with_subtype("waiting_for_choice", "acp")
}

/// Situation 1: Waiting for choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitingForChoiceSituation {
    /// Available options
    pub options: Vec<ChoiceOption>,

    /// Permission type (for security check)
    pub permission_type: Option<String>,

    /// Whether this is a critical choice
    pub critical: bool,
}

impl WaitingForChoiceSituation {
    pub fn new(options: Vec<ChoiceOption>) -> Self {
        Self {
            options,
            permission_type: None,
            critical: false,
        }
    }

    pub fn with_permission_type(self, permission_type: impl Into<String>) -> Self {
        Self {
            permission_type: Some(permission_type.into()),
            ..self
        }
    }

    pub fn critical(self) -> Self {
        Self {
            critical: true,
            ..self
        }
    }
}

impl Default for WaitingForChoiceSituation {
    fn default() -> Self {
        Self {
            options: Vec::new(),
            permission_type: None,
            critical: false,
        }
    }
}

impl DecisionSituation for WaitingForChoiceSituation {
    fn situation_type(&self) -> SituationType {
        waiting_for_choice()
    }

    fn implementation_type(&self) -> &'static str {
        "WaitingForChoiceSituation"
    }

    fn requires_human(&self) -> bool {
        self.critical
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.critical {
            UrgencyLevel::High
        } else {
            UrgencyLevel::Low
        }
    }

    fn to_prompt_text(&self) -> String {
        let options_text = self
            .options
            .iter()
            .map(|o| format!("[{}] {}", o.id, o.label))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            "Waiting for choice:\nOptions:\n{}\nPermission type: {}",
            options_text,
            self.permission_type.as_deref().unwrap_or("unknown")
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("select_option"),
            ActionType::new("select_first"),
            ActionType::new("reject_all"),
            ActionType::new("custom_instruction"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 2: Claims completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimsCompletionSituation {
    /// Completion summary
    pub summary: String,

    /// Reflection rounds so far
    pub reflection_rounds: u8,

    /// Maximum reflection rounds
    pub max_reflection_rounds: u8,

    /// Confidence level (0.0-1.0)
    pub confidence: f64,
}

impl ClaimsCompletionSituation {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            reflection_rounds: 0,
            max_reflection_rounds: 2,
            confidence: 0.8,
        }
    }

    pub fn with_reflection_rounds(self, rounds: u8, max: u8) -> Self {
        Self {
            reflection_rounds: rounds,
            max_reflection_rounds: max,
            ..self
        }
    }

    pub fn with_confidence(self, confidence: f64) -> Self {
        Self { confidence, ..self }
    }
}

impl Default for ClaimsCompletionSituation {
    fn default() -> Self {
        Self {
            summary: String::new(),
            reflection_rounds: 0,
            max_reflection_rounds: 2,
            confidence: 0.8,
        }
    }
}

impl DecisionSituation for ClaimsCompletionSituation {
    fn situation_type(&self) -> SituationType {
        claims_completion()
    }

    fn implementation_type(&self) -> &'static str {
        "ClaimsCompletionSituation"
    }

    fn requires_human(&self) -> bool {
        // Requires human if reflection exhausted and low confidence
        self.reflection_rounds >= self.max_reflection_rounds && self.confidence < 0.7
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.confidence < 0.5 {
            UrgencyLevel::Critical
        } else {
            UrgencyLevel::Medium
        }
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Claims completion (round {}):\nSummary: {}\nConfidence: {:.0}%",
            self.reflection_rounds,
            self.summary,
            self.confidence * 100.0
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        if self.reflection_rounds < self.max_reflection_rounds {
            vec![
                ActionType::new("reflect"),
                ActionType::new("confirm_completion"),
            ]
        } else {
            vec![
                ActionType::new("confirm_completion"),
                ActionType::new("request_human"),
            ]
        }
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 3: Partial completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialCompletionSituation {
    pub progress: CompletionProgress,
    pub blocker: Option<String>,
}

impl PartialCompletionSituation {
    pub fn new(progress: CompletionProgress) -> Self {
        Self {
            progress,
            blocker: None,
        }
    }

    pub fn with_blocker(self, blocker: impl Into<String>) -> Self {
        Self {
            blocker: Some(blocker.into()),
            ..self
        }
    }
}

impl Default for PartialCompletionSituation {
    fn default() -> Self {
        Self {
            progress: CompletionProgress::default(),
            blocker: None,
        }
    }
}

impl DecisionSituation for PartialCompletionSituation {
    fn situation_type(&self) -> SituationType {
        partial_completion()
    }

    fn implementation_type(&self) -> &'static str {
        "PartialCompletionSituation"
    }

    fn requires_human(&self) -> bool {
        self.blocker.is_some()
    }

    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Medium
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Partial completion:\nCompleted: {}\nRemaining: {}\nBlocker: {}",
            self.progress.completed_items.join(", "),
            self.progress.remaining_items.join(", "),
            self.blocker.as_deref().unwrap_or("none")
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("continue"),
            ActionType::new("skip_remaining"),
            ActionType::new("request_context"),
        ]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Situation 4: Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSituation {
    pub error: ErrorInfo,
}

impl ErrorSituation {
    pub fn new(error: ErrorInfo) -> Self {
        Self { error }
    }
}

impl Default for ErrorSituation {
    fn default() -> Self {
        Self {
            error: ErrorInfo::new("unknown", "Unknown error"),
        }
    }
}

impl DecisionSituation for ErrorSituation {
    fn situation_type(&self) -> SituationType {
        error()
    }

    fn implementation_type(&self) -> &'static str {
        "ErrorSituation"
    }

    fn requires_human(&self) -> bool {
        !self.error.recoverable || self.error.retry_count >= 3
    }

    fn human_urgency(&self) -> UrgencyLevel {
        if self.error.recoverable {
            UrgencyLevel::Medium
        } else {
            UrgencyLevel::High
        }
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Error (retry {}):\nType: {}\nMessage: {}\nRecoverable: {}",
            self.error.retry_count,
            self.error.error_type,
            self.error.message,
            self.error.recoverable
        )
    }

    fn available_actions(&self) -> Vec<ActionType> {
        if self.error.recoverable && self.error.retry_count < 3 {
            vec![
                ActionType::new("retry"),
                ActionType::new("retry_adjusted"),
                ActionType::new("restart"),
            ]
        } else {
            vec![
                ActionType::new("request_human"),
                ActionType::new("abort"),
            ]
        }
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Initialize registry with built-in situations
pub fn register_situation_builtins(registry: &SituationRegistry) {
    registry.register_default(Box::new(WaitingForChoiceSituation::default()));
    registry.register_default(Box::new(ClaimsCompletionSituation::default()));
    registry.register_default(Box::new(PartialCompletionSituation::default()));
    registry.register_default(Box::new(ErrorSituation::default()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waiting_for_choice_situation_type() {
        let situation = WaitingForChoiceSituation::default();
        assert_eq!(situation.situation_type(), waiting_for_choice());
    }

    #[test]
    fn test_waiting_for_choice_options() {
        let situation = WaitingForChoiceSituation::new(vec![
            ChoiceOption::new("A", "Option A"),
            ChoiceOption::new("B", "Option B"),
        ]);
        assert_eq!(situation.options.len(), 2);
        assert_eq!(situation.options[0].id, "A");
    }

    #[test]
    fn test_waiting_for_choice_critical() {
        let situation = WaitingForChoiceSituation::new(vec![]).critical();
        assert!(situation.requires_human());
        assert_eq!(situation.human_urgency(), UrgencyLevel::High);
    }

    #[test]
    fn test_waiting_for_choice_available_actions() {
        let situation = WaitingForChoiceSituation::default();
        let actions = situation.available_actions();
        assert!(actions.contains(&ActionType::new("select_option")));
        assert!(actions.contains(&ActionType::new("reject_all")));
    }

    #[test]
    fn test_claims_completion_situation_type() {
        let situation = ClaimsCompletionSituation::default();
        assert_eq!(situation.situation_type(), claims_completion());
    }

    #[test]
    fn test_claims_completion_reflection_rounds() {
        let situation =
            ClaimsCompletionSituation::new("Done").with_reflection_rounds(1, 2).with_confidence(0.9);
        assert_eq!(situation.reflection_rounds, 1);
        assert_eq!(situation.max_reflection_rounds, 2);
        assert!(!situation.requires_human()); // High confidence, not exhausted
    }

    #[test]
    fn test_claims_completion_requires_human_when_exhausted() {
        let situation = ClaimsCompletionSituation::new("Done")
            .with_reflection_rounds(2, 2)
            .with_confidence(0.5); // Low confidence
        assert!(situation.requires_human());
    }

    #[test]
    fn test_claims_completion_available_actions_reflect() {
        let situation = ClaimsCompletionSituation::new("Done").with_reflection_rounds(0, 2);
        let actions = situation.available_actions();
        assert!(actions.contains(&ActionType::new("reflect")));
    }

    #[test]
    fn test_claims_completion_available_actions_no_reflect() {
        let situation = ClaimsCompletionSituation::new("Done").with_reflection_rounds(2, 2);
        let actions = situation.available_actions();
        assert!(!actions.contains(&ActionType::new("reflect")));
    }

    #[test]
    fn test_partial_completion_situation_type() {
        let situation = PartialCompletionSituation::default();
        assert_eq!(situation.situation_type(), partial_completion());
    }

    #[test]
    fn test_partial_completion_progress() {
        let progress = CompletionProgress {
            completed_items: vec!["item1".to_string()],
            remaining_items: vec!["item2".to_string()],
            estimated_remaining_minutes: Some(30),
        };
        let situation = PartialCompletionSituation::new(progress);
        assert_eq!(situation.progress.completed_items.len(), 1);
        assert_eq!(situation.progress.remaining_items.len(), 1);
    }

    #[test]
    fn test_partial_completion_blocker() {
        let situation = PartialCompletionSituation::default().with_blocker("Missing dependency");
        assert!(situation.requires_human());
    }

    #[test]
    fn test_error_situation_type() {
        let situation = ErrorSituation::default();
        assert_eq!(situation.situation_type(), error());
    }

    #[test]
    fn test_error_situation_recoverable() {
        let error = ErrorInfo::new("timeout", "Connection timeout").with_retry_count(1);
        let situation = ErrorSituation::new(error);
        assert!(situation.error.recoverable);
        assert!(!situation.requires_human()); // Recoverable and retry count < 3
    }

    #[test]
    fn test_error_situation_unrecoverable() {
        let error = ErrorInfo::new("fatal", "Critical error").unrecoverable();
        let situation = ErrorSituation::new(error);
        assert!(situation.requires_human());
    }

    #[test]
    fn test_error_situation_retry_count_exhausted() {
        let error = ErrorInfo::new("timeout", "Timeout").with_retry_count(3);
        let situation = ErrorSituation::new(error);
        assert!(situation.requires_human());
    }

    #[test]
    fn test_to_prompt_text_format() {
        let situation = WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]);
        let text = situation.to_prompt_text();
        assert!(text.contains("Waiting for choice"));
        assert!(text.contains("[A] Option A"));
    }

    #[test]
    fn test_register_builtins() {
        let registry = SituationRegistry::new();
        register_situation_builtins(&registry);

        assert!(registry.is_registered(&waiting_for_choice()));
        assert!(registry.is_registered(&claims_completion()));
        assert!(registry.is_registered(&partial_completion()));
        assert!(registry.is_registered(&error()));
    }

    #[test]
    fn test_situation_serde() {
        let situation = WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")])
            .with_permission_type("execute");

        let json = serde_json::to_string(&situation).unwrap();
        let parsed: WaitingForChoiceSituation = serde_json::from_str(&json).unwrap();
        assert_eq!(situation.options.len(), parsed.options.len());
        assert_eq!(situation.permission_type, parsed.permission_type);
    }

    #[test]
    fn test_situation_type_getters() {
        assert_eq!(waiting_for_choice().name, "waiting_for_choice");
        assert_eq!(claims_completion().name, "claims_completion");
        assert_eq!(claude_finished().name, "finished");
        assert_eq!(claude_finished().subtype, Some("claude".to_string()));
    }
}