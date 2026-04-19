//! Classifier registry with provider dispatch

use crate::classifier::classifier::{ClassifyResult, OutputClassifier};
use crate::provider::provider_event::ProviderEvent;
use crate::provider::provider_kind::ProviderKind;
use crate::model::situation::situation_registry::SituationRegistry;
use crate::core::types::SituationType;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Classifier registry - dispatches to provider-specific classifiers
pub struct ClassifierRegistry {
    /// Per-provider classifiers (thread-safe)
    classifiers: RwLock<HashMap<ProviderKind, Box<dyn OutputClassifier>>>,

    /// Fallback classifier (for unknown providers)
    fallback: Box<dyn OutputClassifier>,

    /// Situation registry (shared reference)
    situation_registry: Arc<SituationRegistry>,
}

impl ClassifierRegistry {
    pub fn new(situation_registry: Arc<SituationRegistry>) -> Self {
        Self {
            classifiers: RwLock::new(HashMap::new()),
            fallback: Box::new(FallbackClassifier),
            situation_registry,
        }
    }

    /// Register classifier for provider
    pub fn register(&self, classifier: Box<dyn OutputClassifier>) {
        self.classifiers
            .write()
            .unwrap()
            .insert(classifier.provider_kind(), classifier);
    }

    /// Classify event
    pub fn classify(&self, event: &ProviderEvent, provider: ProviderKind) -> ClassifyResult {
        let classifiers = self.classifiers.read().unwrap();
        let classifier = classifiers.get(&provider).unwrap_or(&self.fallback);

        match classifier.classify_type(event) {
            Some(situation_type) => {
                let situation = classifier.build_situation(event, &self.situation_registry);
                ClassifyResult::create_needs_decision(situation_type, situation)
            }
            None => ClassifyResult::Running {
                context_update: classifier.extract_context(event),
            },
        }
    }

    /// Get classifier for provider
    pub fn get(&self, provider: ProviderKind) -> Option<Box<dyn OutputClassifier>> {
        self.classifiers
            .read()
            .unwrap()
            .get(&provider)
            .map(|c| c.clone_boxed())
    }

    /// Check if classifier is registered
    pub fn is_registered(&self, provider: ProviderKind) -> bool {
        self.classifiers.read().unwrap().contains_key(&provider)
    }
}

/// Fallback classifier - minimal classification
pub struct FallbackClassifier;

impl OutputClassifier for FallbackClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Unknown
    }

    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            ProviderEvent::Finished { .. } => Some(SituationType::new("claims_completion")),
            ProviderEvent::Error { .. } => Some(SituationType::new("error")),
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

    fn extract_context(&self, _event: &ProviderEvent) -> Option<crate::classifier::ContextUpdate> {
        None
    }

    fn clone_boxed(&self) -> Box<dyn OutputClassifier> {
        Box::new(FallbackClassifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::situation::builtin_situations::register_situation_builtins;

    #[test]
    fn test_classifier_registry_new() {
        let situation_registry = Arc::new(SituationRegistry::new());
        let registry = ClassifierRegistry::new(situation_registry);
        assert!(!registry.is_registered(ProviderKind::Claude));
    }

    #[test]
    fn test_classifier_registry_register() {
        let situation_registry = Arc::new(SituationRegistry::new());
        let registry = ClassifierRegistry::new(situation_registry);
        registry.register(Box::new(FallbackClassifier));
        assert!(registry.is_registered(ProviderKind::Unknown));
    }

    #[test]
    fn test_classifier_registry_classify_fallback() {
        let situation_registry = Arc::new(SituationRegistry::new());
        register_situation_builtins(&situation_registry);
        let registry = ClassifierRegistry::new(situation_registry);

        let event = ProviderEvent::Finished { summary: None };
        let result = registry.classify(&event, ProviderKind::Unknown);

        assert!(result.is_needs_decision());
        if let ClassifyResult::NeedsDecision { situation_type, .. } = result {
            assert_eq!(situation_type.name, "claims_completion");
        }
    }

    #[test]
    fn test_fallback_classifier_provider_kind() {
        let classifier = FallbackClassifier;
        assert_eq!(classifier.provider_kind(), ProviderKind::Unknown);
    }

    #[test]
    fn test_fallback_classifier_finished() {
        let classifier = FallbackClassifier;
        let event = ProviderEvent::Finished { summary: None };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(SituationType::new("claims_completion")));
    }

    #[test]
    fn test_fallback_classifier_error() {
        let classifier = FallbackClassifier;
        let event = ProviderEvent::Error {
            message: "test".to_string(),
            error_type: None,
        };
        let type_ = classifier.classify_type(&event);
        assert_eq!(type_, Some(SituationType::new("error")));
    }

    #[test]
    fn test_fallback_classifier_running() {
        let classifier = FallbackClassifier;
        let event = ProviderEvent::StatusUpdate {
            status: "running".to_string(),
        };
        let type_ = classifier.classify_type(&event);
        assert!(type_.is_none());
    }
}
