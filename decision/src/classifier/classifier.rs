//! Output classifier trait and result types

use crate::core::context::{DecisionContext, RunningContextCache};
use crate::provider::provider_event::ProviderEvent;
use crate::model::situation::situation_registry::SituationRegistry;
use crate::core::types::SituationType;
use std::fmt;

/// Context update from classifier
#[derive(Debug, Clone)]
pub enum ContextUpdate {
    /// Thinking text update
    Thinking(String),
    /// Key output text update
    KeyOutput(String),
    /// Tool call record update
    ToolCall(crate::context::ToolCallRecord),
    /// File change record update
    FileChange(crate::context::FileChangeRecord),
}

impl ContextUpdate {
    /// Create thinking context update
    pub fn thinking(text: String) -> Self {
        ContextUpdate::Thinking(text)
    }

    /// Create key output context update
    pub fn key_output(text: String) -> Self {
        ContextUpdate::KeyOutput(text)
    }

    /// Create tool call context update
    pub fn tool_call(record: crate::context::ToolCallRecord) -> Self {
        ContextUpdate::ToolCall(record)
    }

    /// Create file change context update
    pub fn file_change(record: crate::context::FileChangeRecord) -> Self {
        ContextUpdate::FileChange(record)
    }

    /// Apply to running context cache
    pub fn apply(&self, cache: &mut RunningContextCache) {
        match self {
            ContextUpdate::Thinking(text) => cache.update_thinking_summary(text.clone()),
            ContextUpdate::KeyOutput(text) => cache.add_key_output(text.clone()),
            ContextUpdate::ToolCall(record) => cache.add_tool_call(record.clone()),
            ContextUpdate::FileChange(record) => cache.add_file_change(record.clone()),
        }
    }

    /// Apply to decision context
    pub fn apply_to_context(&self, context: &mut DecisionContext) {
        match self {
            ContextUpdate::Thinking(text) => context
                .running_context
                .update_thinking_summary(text.clone()),
            ContextUpdate::KeyOutput(text) => context.running_context.add_key_output(text.clone()),
            ContextUpdate::ToolCall(record) => {
                context.running_context.add_tool_call(record.clone())
            }
            ContextUpdate::FileChange(record) => {
                context.running_context.add_file_change(record.clone())
            }
        }
    }
}

/// Classification result
pub enum ClassifyResult {
    /// Provider is running, no decision needed
    Running {
        context_update: Option<ContextUpdate>,
    },
    /// Provider needs a decision
    NeedsDecision {
        situation_type: SituationType,
        situation: Option<Box<dyn crate::situation::DecisionSituation>>,
    },
}

impl fmt::Debug for ClassifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassifyResult::Running { context_update } => f
                .debug_struct("Running")
                .field("context_update", context_update)
                .finish(),
            ClassifyResult::NeedsDecision { situation_type, .. } => f
                .debug_struct("NeedsDecision")
                .field("situation_type", situation_type)
                .field("situation", &"<dyn DecisionSituation>")
                .finish(),
        }
    }
}

impl ClassifyResult {
    /// Create a running result
    pub fn running(context_update: Option<ContextUpdate>) -> Self {
        ClassifyResult::Running { context_update }
    }

    /// Create a needs decision result
    pub fn create_needs_decision(
        situation_type: SituationType,
        situation: Option<Box<dyn crate::situation::DecisionSituation>>,
    ) -> Self {
        ClassifyResult::NeedsDecision {
            situation_type,
            situation,
        }
    }

    /// Check if this is a running result
    pub fn is_running(&self) -> bool {
        matches!(self, ClassifyResult::Running { .. })
    }

    /// Check if this is a needs decision result
    pub fn is_needs_decision(&self) -> bool {
        matches!(self, ClassifyResult::NeedsDecision { .. })
    }

    /// Get context update if running
    pub fn context_update(&self) -> Option<&ContextUpdate> {
        match self {
            ClassifyResult::Running { context_update } => context_update.as_ref(),
            _ => None,
        }
    }

    /// Get situation type if needs decision
    pub fn situation_type(&self) -> Option<&SituationType> {
        match self {
            ClassifyResult::NeedsDecision { situation_type, .. } => Some(situation_type),
            _ => None,
        }
    }

    /// Get situation if needs decision
    pub fn situation(&self) -> Option<&Box<dyn crate::situation::DecisionSituation>> {
        match self {
            ClassifyResult::NeedsDecision { situation, .. } => situation.as_ref(),
            _ => None,
        }
    }
}

/// Output classifier trait - classifies provider output
pub trait OutputClassifier: Send + Sync {
    /// Get the provider kind this classifier handles
    fn provider_kind(&self) -> crate::provider::provider_kind::ProviderKind;

    /// Classify the event type
    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType>;

    /// Build a situation from the event
    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn crate::situation::DecisionSituation>>;

    /// Extract context update from event
    fn extract_context(&self, event: &ProviderEvent) -> Option<ContextUpdate>;

    /// Clone boxed
    fn clone_boxed(&self) -> Box<dyn OutputClassifier>;
}

impl Clone for Box<dyn OutputClassifier> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_update_thinking() {
        let update = ContextUpdate::thinking("thinking...".to_string());
        assert!(matches!(update, ContextUpdate::Thinking(_)));
    }

    #[test]
    fn test_context_update_key_output() {
        let update = ContextUpdate::key_output("output".to_string());
        assert!(matches!(update, ContextUpdate::KeyOutput(_)));
    }

    #[test]
    fn test_classify_result_running() {
        let result = ClassifyResult::running(None);
        assert!(result.is_running());
        assert!(!result.is_needs_decision());
    }

    #[test]
    fn test_classify_result_needs_decision() {
        let result = ClassifyResult::create_needs_decision(SituationType::new("test"), None);
        assert!(!result.is_running());
        assert!(result.is_needs_decision());
    }

    #[test]
    fn test_context_update_apply() {
        let mut cache = RunningContextCache::default();
        let update = ContextUpdate::thinking("test".to_string());
        update.apply(&mut cache);
        assert!(cache.thinking_summary.is_some());
    }
}
