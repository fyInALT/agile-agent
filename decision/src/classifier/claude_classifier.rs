//! Claude classifier

use crate::model::situation::builtin_situations::{claude_finished, error};
use crate::classifier::classifier::{ContextUpdate, OutputClassifier};
use crate::core::context::ToolCallRecord;
use crate::provider::provider_event::ProviderEvent;
use crate::provider::provider_kind::ProviderKind;
use crate::model::situation::situation_registry::SituationRegistry;
use crate::core::types::SituationType;

/// Claude classifier
pub struct ClaudeClassifier;

impl OutputClassifier for ClaudeClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Claude
    }

    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            // Running events - no situation
            ProviderEvent::ClaudeAssistantChunk { .. } => None,
            ProviderEvent::ClaudeThinkingChunk { .. } => None,
            ProviderEvent::ClaudeToolCallStarted { .. } => None,
            ProviderEvent::ClaudeToolCallFinished { success, .. } if *success => None,
            ProviderEvent::StatusUpdate { .. } => None,
            ProviderEvent::SessionHandle { .. } => None,
            ProviderEvent::WebSearchStarted { .. } => None,
            ProviderEvent::ImageGenerationFinished { .. } => None,

            // Web search finished with no action = failed search
            ProviderEvent::WebSearchFinished { action: None, .. } => {
                Some(SituationType::new("web_search_failed"))
            }
            ProviderEvent::WebSearchFinished { .. } => None,

            // Tool call finished with failure
            ProviderEvent::ClaudeToolCallFinished { success: false, name, .. }
                if name == "mcp" =>
            {
                Some(SituationType::with_subtype("error", "mcp_tool_failed"))
            }
            ProviderEvent::ClaudeToolCallFinished { success: false, .. } => {
                Some(error())
            }

            // Finished - Claude-specific subtype
            ProviderEvent::Finished { .. } => Some(claude_finished()),
            ProviderEvent::Error { .. } => Some(error()),
            _ => None,
        }
    }

    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn crate::situation::DecisionSituation>> {
        let type_ = self.classify_type(event)?;
        Some(registry.build(type_))
    }

    fn extract_context(&self, event: &ProviderEvent) -> Option<ContextUpdate> {
        match event {
            ProviderEvent::ClaudeThinkingChunk { text } => {
                Some(ContextUpdate::thinking(text.clone()))
            }
            ProviderEvent::ClaudeAssistantChunk { text } if self.is_key_output(text) => {
                Some(ContextUpdate::key_output(text.clone()))
            }
            ProviderEvent::ClaudeToolCallStarted { name, input } => Some(ContextUpdate::tool_call(
                ToolCallRecord::new(name.clone(), true)
                    .with_input_preview(input.clone().unwrap_or_default()),
            )),
            ProviderEvent::ClaudeToolCallFinished {
                name,
                output,
                success,
                ..
            } => Some(ContextUpdate::tool_call(
                ToolCallRecord::new(name.clone(), *success)
                    .with_output_preview(output.clone().unwrap_or_default()),
            )),
            _ => None,
        }
    }

    fn clone_boxed(&self) -> Box<dyn OutputClassifier> {
        Box::new(ClaudeClassifier)
    }
}

impl ClaudeClassifier {
    /// Check if text is a key output
    fn is_key_output(&self, text: &str) -> bool {
        text.contains("完成")
            || text.contains("finished")
            || text.contains("done")
            || text.contains("成功")
            || text.contains("success")
    }
}

/// Register Claude situation builders
pub fn register_claude_builders(registry: &SituationRegistry) {
    // Claude Finished builder - uses ClaimsCompletionSituation
    registry.register_builder(claude_finished(), || {
        Some(Box::new(
            crate::builtin_situations::ClaimsCompletionSituation::new("Claude session finished")
                .with_confidence(0.8),
        ))
    });

    // Claude Error builder
    registry.register_builder(error(), || {
        Some(Box::new(crate::builtin_situations::ErrorSituation::new(
            crate::model::situation::ErrorInfo::new("claude_error", "Unknown error"),
        )))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::classifier_registry::ClassifierRegistry;
    use std::sync::Arc;

    #[test]
    fn test_claude_classifier_provider_kind() {
        let classifier = ClaudeClassifier;
        assert_eq!(classifier.provider_kind(), ProviderKind::Claude);
    }

    #[test]
    fn test_claude_classifier_assistant_chunk_running() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::ClaudeAssistantChunk {
            text: "hello".to_string(),
        };
        let type_ = classifier.classify_type(&event);
        assert!(type_.is_none());
    }

    #[test]
    fn test_claude_classifier_thinking_chunk_context() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::ClaudeThinkingChunk {
            text: "thinking...".to_string(),
        };
        let context = classifier.extract_context(&event);
        assert!(matches!(context, Some(ContextUpdate::Thinking(_))));
    }

    #[test]
    fn test_claude_classifier_finished() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::Finished {
            summary: Some("done".to_string()),
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(claude_finished()));
    }

    #[test]
    fn test_claude_classifier_error() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::Error {
            message: "timeout".to_string(),
            error_type: None,
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(error()));
    }

    #[test]
    fn test_claude_classifier_tool_call_started() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::ClaudeToolCallStarted {
            name: "Bash".to_string(),
            input: Some("ls".to_string()),
        };
        let context = classifier.extract_context(&event);
        assert!(matches!(context, Some(ContextUpdate::ToolCall(_))));
    }

    #[test]
    fn test_claude_classifier_key_output_detection() {
        let classifier = ClaudeClassifier;
        assert!(classifier.is_key_output("finished"));
        assert!(classifier.is_key_output("done"));
        assert!(classifier.is_key_output("success"));
        assert!(!classifier.is_key_output("random text"));
    }

    #[test]
    fn test_register_claude_builders() {
        let registry = SituationRegistry::new();
        register_claude_builders(&registry);
        assert!(registry.is_registered(&claude_finished()));
        assert!(registry.is_registered(&error()));
    }

    #[test]
    fn test_claude_classifier_in_registry() {
        let situation_registry = Arc::new(SituationRegistry::new());
        crate::builtin_situations::register_situation_builtins(&situation_registry);
        register_claude_builders(&situation_registry);

        let classifier_registry = ClassifierRegistry::new(situation_registry);
        classifier_registry.register(Box::new(ClaudeClassifier));

        let event = ProviderEvent::Finished { summary: None };
        let result = classifier_registry.classify(&event, ProviderKind::Claude);

        assert!(result.is_needs_decision());
    }

    #[test]
    fn web_search_no_results_triggers_decision() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::WebSearchFinished {
            call_id: Some("ws-1".to_string()),
            query: "nonexistent_library_rust".to_string(),
            action: None,
        };
        let result = classifier.classify_type(&event);
        assert_eq!(result, Some(SituationType::new("web_search_failed")));
    }

    #[test]
    fn web_search_with_results_is_running() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::WebSearchFinished {
            call_id: Some("ws-1".to_string()),
            query: "rust".to_string(),
            action: Some(agent_events::WebSearchAction::Search {
                query: Some("rust".to_string()),
                queries: None,
            }),
        };
        let result = classifier.classify_type(&event);
        assert!(result.is_none());
    }

    #[test]
    fn mcp_tool_failure_triggers_error_subtype() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::ClaudeToolCallFinished {
            name: "mcp".to_string(),
            output: Some("connection refused".to_string()),
            success: false,
            result_blocks: None,
        };
        let result = classifier.classify_type(&event);
        assert_eq!(result, Some(SituationType::with_subtype("error", "mcp_tool_failed")));
    }

    #[test]
    fn successful_mcp_tool_is_running() {
        let classifier = ClaudeClassifier;
        let event = ProviderEvent::ClaudeToolCallFinished {
            name: "mcp".to_string(),
            output: Some("ok".to_string()),
            success: true,
            result_blocks: None,
        };
        let result = classifier.classify_type(&event);
        assert!(result.is_none());
    }
}
