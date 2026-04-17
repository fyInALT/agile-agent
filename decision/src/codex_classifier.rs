//! Codex classifier

use crate::builtin_situations::{claims_completion, codex_approval, error};
use crate::classifier::{ContextUpdate, OutputClassifier};
use crate::context::{ChangeType, FileChangeRecord};
use crate::provider_event::ProviderEvent;
use crate::provider_kind::ProviderKind;
use crate::situation::{ChoiceOption, ErrorInfo};
use crate::situation_registry::SituationRegistry;
use crate::types::SituationType;

/// Codex classifier
pub struct CodexClassifier;

impl OutputClassifier for CodexClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Codex
    }

    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            // Approval requests = WaitingForChoice (Codex subtype)
            ProviderEvent::CodexApprovalRequest { method, .. } => match method.as_str() {
                "execCommandApproval"
                | "applyPatchApproval"
                | "item/tool/requestUserInput"
                | "item/permissions/requestApproval" => Some(codex_approval()),
                _ => None,
            },

            // Finished
            ProviderEvent::Finished { .. } => Some(claims_completion()),
            ProviderEvent::CodexError { kind, .. } => {
                Some(SituationType::with_subtype("error", kind))
            }
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
            ProviderEvent::CodexPatchApplyStarted { path } => Some(ContextUpdate::file_change(
                FileChangeRecord::new(path.clone(), ChangeType::Modified),
            )),
            _ => None,
        }
    }

    fn clone_boxed(&self) -> Box<dyn OutputClassifier> {
        Box::new(CodexClassifier)
    }
}

/// Register Codex situation builders
pub fn register_codex_builders(registry: &SituationRegistry) {
    // Codex approval builder
    registry.register_builder(codex_approval(), || {
        Some(Box::new(
            crate::builtin_situations::WaitingForChoiceSituation::new(vec![
                ChoiceOption::new("approved", "Approve"),
                ChoiceOption::new("approved_for_session", "Approve for session"),
                ChoiceOption::new("denied", "Deny"),
                ChoiceOption::new("abort", "Abort"),
            ]),
        ))
    });

    // Codex completion builder
    registry.register_builder(claims_completion(), || {
        Some(Box::new(
            crate::builtin_situations::ClaimsCompletionSituation::new("Codex task finished"),
        ))
    });

    // Codex error builder
    registry.register_builder(error(), || {
        Some(Box::new(crate::builtin_situations::ErrorSituation::new(
            ErrorInfo::new("codex_error", "Unknown error"),
        )))
    });
}

/// Parse Codex approval options from method and params
pub fn parse_codex_options(method: &str, params: &serde_json::Value) -> Vec<ChoiceOption> {
    match method {
        "execCommandApproval" => vec![
            ChoiceOption::new("approved", "Approve"),
            ChoiceOption::new("approved_for_session", "Approve for session"),
            ChoiceOption::new("denied", "Deny"),
            ChoiceOption::new("abort", "Abort"),
        ],
        "applyPatchApproval" => vec![
            ChoiceOption::new("approved", "Approve patch"),
            ChoiceOption::new("approved_for_session", "Approve for session"),
            ChoiceOption::new("denied", "Deny"),
            ChoiceOption::new("abort", "Abort"),
        ],
        "item/tool/requestUserInput" => params
            .get("options")
            .and_then(|o| o.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        v.get("id").and_then(|id| id.as_str()).map(|id| {
                            ChoiceOption::new(
                                id,
                                v.get("label").and_then(|l| l.as_str()).unwrap_or(id),
                            )
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => vec![
            ChoiceOption::new("approved", "Approve"),
            ChoiceOption::new("denied", "Deny"),
        ],
    }
}

/// Detect critical commands
pub fn is_critical_command(params: &serde_json::Value) -> bool {
    params
        .get("command")
        .and_then(|c| c.as_str())
        .map(|cmd| {
            cmd.contains("rm -rf")
                || cmd.contains("sudo")
                || cmd.contains("chmod")
                || cmd.contains("drop table")
                || cmd.contains("delete from")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier_registry::ClassifierRegistry;
    use std::sync::Arc;

    #[test]
    fn test_codex_classifier_provider_kind() {
        let classifier = CodexClassifier;
        assert_eq!(classifier.provider_kind(), ProviderKind::Codex);
    }

    #[test]
    fn test_codex_classifier_exec_command_approval() {
        let classifier = CodexClassifier;
        let event = ProviderEvent::CodexApprovalRequest {
            method: "execCommandApproval".to_string(),
            params: serde_json::json!({}),
            request_id: None,
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(codex_approval()));
    }

    #[test]
    fn test_codex_classifier_apply_patch_approval() {
        let classifier = CodexClassifier;
        let event = ProviderEvent::CodexApprovalRequest {
            method: "applyPatchApproval".to_string(),
            params: serde_json::json!({}),
            request_id: None,
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(codex_approval()));
    }

    #[test]
    fn test_codex_classifier_finished() {
        let classifier = CodexClassifier;
        let event = ProviderEvent::Finished { summary: None };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(claims_completion()));
    }

    #[test]
    fn test_codex_classifier_error() {
        let classifier = CodexClassifier;
        let event = ProviderEvent::CodexError {
            kind: "timed_out".to_string(),
            message: "timeout".to_string(),
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(
            type_,
            Some(SituationType::with_subtype("error", "timed_out"))
        );
    }

    #[test]
    fn test_codex_classifier_patch_apply_context() {
        let classifier = CodexClassifier;
        let event = ProviderEvent::CodexPatchApplyStarted {
            path: "/src/main.rs".to_string(),
        };
        let context = classifier.extract_context(&event);
        assert!(matches!(context, Some(ContextUpdate::FileChange(_))));
    }

    #[test]
    fn test_parse_codex_options_exec_command() {
        let options = parse_codex_options("execCommandApproval", &serde_json::json!({}));
        assert_eq!(options.len(), 4);
        assert_eq!(options[0].id, "approved");
    }

    #[test]
    fn test_parse_codex_options_user_input() {
        let params = serde_json::json!({
            "options": [
                {"id": "A", "label": "Option A"},
                {"id": "B", "label": "Option B"}
            ]
        });
        let options = parse_codex_options("item/tool/requestUserInput", &params);
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].id, "A");
    }

    #[test]
    fn test_is_critical_command_rm() {
        let params = serde_json::json!({ "command": "rm -rf /home" });
        assert!(is_critical_command(&params));
    }

    #[test]
    fn test_is_critical_command_sudo() {
        let params = serde_json::json!({ "command": "sudo apt install" });
        assert!(is_critical_command(&params));
    }

    #[test]
    fn test_is_critical_command_safe() {
        let params = serde_json::json!({ "command": "ls -la" });
        assert!(!is_critical_command(&params));
    }

    #[test]
    fn test_register_codex_builders() {
        let registry = SituationRegistry::new();
        register_codex_builders(&registry);
        assert!(registry.is_registered(&codex_approval()));
        assert!(registry.is_registered(&claims_completion()));
    }

    #[test]
    fn test_codex_classifier_in_registry() {
        let situation_registry = Arc::new(SituationRegistry::new());
        crate::builtin_situations::register_situation_builtins(&situation_registry);
        register_codex_builders(&situation_registry);

        let classifier_registry = ClassifierRegistry::new(situation_registry);
        classifier_registry.register(Box::new(CodexClassifier));

        let event = ProviderEvent::CodexApprovalRequest {
            method: "execCommandApproval".to_string(),
            params: serde_json::json!({}),
            request_id: None,
        };
        let result = classifier_registry.classify(&event, ProviderKind::Codex);

        assert!(result.is_needs_decision());
    }
}
