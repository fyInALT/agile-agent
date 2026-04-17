//! Decision layer initialization

use crate::acp_classifier::ACPClassifier;
use crate::action_registry::ActionRegistry;
use crate::classifier_registry::ClassifierRegistry;
use crate::claude_classifier::ClaudeClassifier;
use crate::codex_classifier::CodexClassifier;
use crate::situation_registry::SituationRegistry;
use std::sync::Arc;

/// Decision layer components after initialization
pub struct DecisionLayerComponents {
    pub situation_registry: Arc<SituationRegistry>,
    pub action_registry: Arc<ActionRegistry>,
    pub classifier_registry: Arc<ClassifierRegistry>,
}

/// Initialize decision layer with all built-in components
pub fn initialize_decision_layer() -> DecisionLayerComponents {
    // 1. Initialize situation registry with builtins
    let situation_registry = Arc::new(SituationRegistry::new());
    crate::builtin_situations::register_situation_builtins(&situation_registry);
    crate::claude_classifier::register_claude_builders(&situation_registry);
    crate::codex_classifier::register_codex_builders(&situation_registry);
    crate::acp_classifier::register_acp_builders(&situation_registry);

    // 2. Initialize action registry with builtins
    let action_registry = Arc::new(ActionRegistry::new());
    crate::builtin_actions::register_action_builtins(&action_registry);

    // 3. Initialize classifier registry
    let classifier_registry = Arc::new(ClassifierRegistry::new(situation_registry.clone()));
    classifier_registry.register(Box::new(ClaudeClassifier));
    classifier_registry.register(Box::new(CodexClassifier));
    classifier_registry.register(Box::new(ACPClassifier));

    DecisionLayerComponents {
        situation_registry,
        action_registry,
        classifier_registry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_situations::{
        acp_permission, claims_completion, claude_finished, codex_approval, error,
        waiting_for_choice,
    };
    use crate::classifier::ClassifyResult;
    use crate::provider_event::ProviderEvent;
    use crate::provider_kind::ProviderKind;

    #[test]
    fn test_initialize_decision_layer() {
        let components = initialize_decision_layer();

        // Check situation registry
        assert!(
            components
                .situation_registry
                .is_registered(&waiting_for_choice())
        );
        assert!(
            components
                .situation_registry
                .is_registered(&claims_completion())
        );
        assert!(
            components
                .situation_registry
                .is_registered(&claude_finished())
        );
        assert!(
            components
                .situation_registry
                .is_registered(&codex_approval())
        );
        assert!(
            components
                .situation_registry
                .is_registered(&acp_permission())
        );
        assert!(components.situation_registry.is_registered(&error()));

        // Check action registry
        assert!(
            components
                .action_registry
                .is_registered(&crate::builtin_actions::select_option())
        );
        assert!(
            components
                .action_registry
                .is_registered(&crate::builtin_actions::reflect())
        );

        // Check classifier registry
        assert!(
            components
                .classifier_registry
                .is_registered(ProviderKind::Claude)
        );
        assert!(
            components
                .classifier_registry
                .is_registered(ProviderKind::Codex)
        );
        assert!(
            components
                .classifier_registry
                .is_registered(ProviderKind::ACP)
        );
    }

    #[test]
    fn test_classifier_claude_finished() {
        let components = initialize_decision_layer();
        let event = ProviderEvent::Finished { summary: None };
        let result = components
            .classifier_registry
            .classify(&event, ProviderKind::Claude);

        assert!(result.is_needs_decision());
        if let ClassifyResult::NeedsDecision { situation_type, .. } = result {
            assert_eq!(situation_type, claude_finished());
        }
    }

    #[test]
    fn test_classifier_codex_approval() {
        let components = initialize_decision_layer();
        let event = ProviderEvent::CodexApprovalRequest {
            method: "execCommandApproval".to_string(),
            params: serde_json::json!({}),
            request_id: None,
        };
        let result = components
            .classifier_registry
            .classify(&event, ProviderKind::Codex);

        assert!(result.is_needs_decision());
        if let ClassifyResult::NeedsDecision { situation_type, .. } = result {
            assert_eq!(situation_type, codex_approval());
        }
    }

    #[test]
    fn test_classifier_acp_permission() {
        let components = initialize_decision_layer();
        let event = ProviderEvent::ACPNotification {
            method: "permission.asked".to_string(),
            params: serde_json::json!({}),
        };
        let result = components
            .classifier_registry
            .classify(&event, ProviderKind::ACP);

        assert!(result.is_needs_decision());
        if let ClassifyResult::NeedsDecision { situation_type, .. } = result {
            assert_eq!(situation_type, acp_permission());
        }
    }

    #[test]
    fn test_classifier_unknown_provider_fallback() {
        let components = initialize_decision_layer();
        let event = ProviderEvent::Finished { summary: None };
        let result = components
            .classifier_registry
            .classify(&event, ProviderKind::Unknown);

        assert!(result.is_needs_decision());
        if let ClassifyResult::NeedsDecision { situation_type, .. } = result {
            assert_eq!(situation_type.name, "claims_completion");
        }
    }

    #[test]
    fn test_situation_registry_build() {
        let components = initialize_decision_layer();
        let situation = components.situation_registry.build(waiting_for_choice());
        assert_eq!(situation.situation_type(), waiting_for_choice());
    }

    #[test]
    fn test_action_registry_get() {
        let components = initialize_decision_layer();
        let action = components
            .action_registry
            .get(&crate::builtin_actions::select_option());
        assert!(action.is_some());
    }
}
