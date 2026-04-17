//! ACP classifier (OpenCode/Kimi)

use crate::builtin_situations::{acp_permission, claims_completion, error};
use crate::classifier::{ContextUpdate, OutputClassifier};
use crate::provider_event::ProviderEvent;
use crate::provider_kind::ProviderKind;
use crate::situation::{ChoiceOption, ErrorInfo};
use crate::situation_registry::SituationRegistry;
use crate::types::SituationType;

/// ACP classifier (OpenCode/Kimi)
pub struct ACPClassifier;

impl OutputClassifier for ACPClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::ACP
    }

    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            // Permission asked = WaitingForChoice (ACP subtype)
            ProviderEvent::ACPNotification { method, params } if method == "permission.asked" => {
                Some(acp_permission())
            }

            // Session status
            ProviderEvent::ACPNotification { method, params } if method == "session.status" => {
                let status = params
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("busy");
                match status {
                    "idle" => Some(claims_completion()),
                    "retry" => {
                        let attempt = params.get("attempt").and_then(|a| a.as_u64()).unwrap_or(0);
                        if attempt > 3 {
                            Some(SituationType::with_subtype("error", "retry_exhausted"))
                        } else {
                            None // Running
                        }
                    }
                    _ => None, // busy, running
                }
            }

            ProviderEvent::ACPError { .. } => Some(error()),
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
            ProviderEvent::ACPNotification { method, params } if method == "assistant/message" => {
                params
                    .get("text")
                    .map(|t| ContextUpdate::key_output(t.as_str().unwrap_or("").to_string()))
            }
            _ => None,
        }
    }

    fn clone_boxed(&self) -> Box<dyn OutputClassifier> {
        Box::new(ACPClassifier)
    }
}

/// Register ACP situation builders
pub fn register_acp_builders(registry: &SituationRegistry) {
    // ACP permission builder
    registry.register_builder(acp_permission(), || {
        Some(Box::new(
            crate::builtin_situations::WaitingForChoiceSituation::new(vec![
                ChoiceOption::new("once", "Once"),
                ChoiceOption::new("always", "Always for session"),
                ChoiceOption::new("reject", "Reject"),
            ]),
        ))
    });

    // ACP completion builder
    registry.register_builder(claims_completion(), || {
        Some(Box::new(
            crate::builtin_situations::ClaimsCompletionSituation::new("ACP session idle"),
        ))
    });

    // ACP error builder
    registry.register_builder(error(), || {
        Some(Box::new(crate::builtin_situations::ErrorSituation::new(
            ErrorInfo::new("acp_error", "Unknown error"),
        )))
    });
}

/// Parse ACP permission options
pub fn parse_acp_options(_params: &serde_json::Value) -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("once", "Once"),
        ChoiceOption::new("always", "Always for session"),
        ChoiceOption::new("reject", "Reject"),
    ]
}

/// Detect critical permissions
pub fn is_critical_permission(permission_type: &str, params: &serde_json::Value) -> bool {
    match permission_type {
        "write" | "edit" => params
            .get("path")
            .and_then(|p| p.as_str())
            .map(|path| path.contains(".env") || path.contains("credentials"))
            .unwrap_or(false),
        "execute" => params
            .get("command")
            .and_then(|c| c.as_str())
            .map(|cmd| cmd.contains("rm") || cmd.contains("sudo"))
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier_registry::ClassifierRegistry;
    use std::sync::Arc;

    #[test]
    fn test_acp_classifier_provider_kind() {
        let classifier = ACPClassifier;
        assert_eq!(classifier.provider_kind(), ProviderKind::ACP);
    }

    #[test]
    fn test_acp_classifier_permission_asked() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPNotification {
            method: "permission.asked".to_string(),
            params: serde_json::json!({}),
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(acp_permission()));
    }

    #[test]
    fn test_acp_classifier_session_idle() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPNotification {
            method: "session.status".to_string(),
            params: serde_json::json!({ "status": "idle" }),
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(claims_completion()));
    }

    #[test]
    fn test_acp_classifier_session_busy() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPNotification {
            method: "session.status".to_string(),
            params: serde_json::json!({ "status": "busy" }),
        };
        let type_ = classifier.classify_type(&event);
        assert!(type_.is_none()); // Running
    }

    #[test]
    fn test_acp_classifier_retry_within_limit() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPNotification {
            method: "session.status".to_string(),
            params: serde_json::json!({ "status": "retry", "attempt": 2 }),
        };
        let type_ = classifier.classify_type(&event);
        assert!(type_.is_none()); // Running - attempt <= 3
    }

    #[test]
    fn test_acp_classifier_retry_exhausted() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPNotification {
            method: "session.status".to_string(),
            params: serde_json::json!({ "status": "retry", "attempt": 4 }),
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(
            type_,
            Some(SituationType::with_subtype("error", "retry_exhausted"))
        );
    }

    #[test]
    fn test_acp_classifier_error() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPError {
            code: "timeout".to_string(),
            message: "timeout".to_string(),
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(error()));
    }

    #[test]
    fn test_acp_classifier_assistant_message_context() {
        let classifier = ACPClassifier;
        let event = ProviderEvent::ACPNotification {
            method: "assistant/message".to_string(),
            params: serde_json::json!({ "text": "output" }),
        };
        let context = classifier.extract_context(&event);
        assert!(matches!(context, Some(ContextUpdate::KeyOutput(_))));
    }

    #[test]
    fn test_parse_acp_options() {
        let options = parse_acp_options(&serde_json::json!({}));
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].id, "once");
    }

    #[test]
    fn test_is_critical_permission_env() {
        let params = serde_json::json!({ "path": ".env" });
        assert!(is_critical_permission("write", &params));
    }

    #[test]
    fn test_is_critical_permission_safe() {
        let params = serde_json::json!({ "path": "src/main.rs" });
        assert!(!is_critical_permission("write", &params));
    }

    #[test]
    fn test_register_acp_builders() {
        let registry = SituationRegistry::new();
        register_acp_builders(&registry);
        assert!(registry.is_registered(&acp_permission()));
        assert!(registry.is_registered(&claims_completion()));
    }

    #[test]
    fn test_acp_classifier_in_registry() {
        let situation_registry = Arc::new(SituationRegistry::new());
        crate::builtin_situations::register_situation_builtins(&situation_registry);
        register_acp_builders(&situation_registry);

        let classifier_registry = ClassifierRegistry::new(situation_registry);
        classifier_registry.register(Box::new(ACPClassifier));

        let event = ProviderEvent::ACPNotification {
            method: "permission.asked".to_string(),
            params: serde_json::json!({}),
        };
        let result = classifier_registry.classify(&event, ProviderKind::ACP);

        assert!(result.is_needs_decision());
    }
}
