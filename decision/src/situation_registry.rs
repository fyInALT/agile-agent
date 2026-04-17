//! Situation registry with thread-safe operations

use crate::situation::DecisionSituation;
use crate::types::SituationType;
use std::collections::HashMap;
use std::sync::RwLock;

/// Situation builder function type
type SituationBuilder = Box<dyn Fn() -> Option<Box<dyn DecisionSituation>> + Send + Sync>;

/// Situation registry - THREAD-SAFE with RwLock
pub struct SituationRegistry {
    /// Registered situation builders (thread-safe)
    builders: RwLock<HashMap<SituationType, SituationBuilder>>,

    /// Default situations (fallback, thread-safe)
    defaults: RwLock<HashMap<SituationType, Box<dyn DecisionSituation>>>,
}

impl SituationRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            builders: RwLock::new(HashMap::new()),
            defaults: RwLock::new(HashMap::new()),
        }
    }

    /// THREAD-SAFE: Register a situation builder
    pub fn register_builder(
        &self,
        type_: SituationType,
        builder: impl Fn() -> Option<Box<dyn DecisionSituation>> + Send + Sync + 'static,
    ) {
        self.builders
            .write()
            .unwrap()
            .insert(type_, Box::new(builder));
    }

    /// THREAD-SAFE: Register a default situation
    pub fn register_default(&self, situation: Box<dyn DecisionSituation>) {
        self.defaults
            .write()
            .unwrap()
            .insert(situation.situation_type(), situation);
    }

    /// THREAD-SAFE: Get situation by type from defaults
    pub fn get(&self, type_: &SituationType) -> Option<Box<dyn DecisionSituation>> {
        self.defaults
            .read()
            .unwrap()
            .get(type_)
            .map(|d| d.clone_boxed())
    }

    /// THREAD-SAFE: Build situation with EXPLICIT FALLBACK CHAIN
    pub fn build(&self, type_: SituationType) -> Box<dyn DecisionSituation> {
        // 1. Try exact type builder
        {
            let builders = self.builders.read().unwrap();
            if let Some(builder) = builders.get(&type_) {
                if let Some(situation) = builder() {
                    return situation;
                }
            }
        }

        // 2. Try base type (without subtype)
        let base_type = type_.base_type();
        if base_type != type_ {
            let builders = self.builders.read().unwrap();
            if let Some(builder) = builders.get(&base_type) {
                if let Some(situation) = builder() {
                    return situation;
                }
            }
        }

        // 3. Try default for exact type
        {
            let defaults = self.defaults.read().unwrap();
            if let Some(default) = defaults.get(&type_) {
                return default.clone_boxed();
            }
        }

        // 4. Try default for base type
        if base_type != type_ {
            let defaults = self.defaults.read().unwrap();
            if let Some(default) = defaults.get(&base_type) {
                return default.clone_boxed();
            }
        }

        // 5. ULTIMATE FALLBACK - create GenericUnknownSituation
        Box::new(GenericUnknownSituation::new(type_.clone()))
    }

    /// THREAD-SAFE: Check if type is registered
    pub fn is_registered(&self, type_: &SituationType) -> bool {
        let builders = self.builders.read().unwrap();
        let defaults = self.defaults.read().unwrap();
        builders.contains_key(type_) || defaults.contains_key(type_)
    }

    /// Get all registered situation types
    pub fn registered_types(&self) -> Vec<SituationType> {
        let builders = self.builders.read().unwrap();
        let defaults = self.defaults.read().unwrap();
        let mut types: Vec<SituationType> = builders.keys().cloned().collect();
        types.extend(defaults.keys().cloned());
        types.sort_by(|a, b| a.name.cmp(&b.name));
        types.dedup();
        types
    }
}

impl Default for SituationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// ULTIMATE FALLBACK: Generic unknown situation
#[derive(Debug, Clone)]
pub struct GenericUnknownSituation {
    detected_type: SituationType,
}

impl GenericUnknownSituation {
    pub fn new(detected_type: SituationType) -> Self {
        Self { detected_type }
    }
}

impl DecisionSituation for GenericUnknownSituation {
    fn situation_type(&self) -> SituationType {
        self.detected_type.clone()
    }

    fn implementation_type(&self) -> &'static str {
        "GenericUnknownSituation"
    }

    fn requires_human(&self) -> bool {
        // Unknown → ALWAYS require human
        true
    }

    fn human_urgency(&self) -> crate::types::UrgencyLevel {
        crate::types::UrgencyLevel::High
    }

    fn to_prompt_text(&self) -> String {
        format!(
            "Unknown situation detected: {}\nRequires human intervention.",
            self.detected_type
        )
    }

    fn available_actions(&self) -> Vec<crate::types::ActionType> {
        vec![crate::types::ActionType::new("request_human")]
    }

    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new() {
        let registry = SituationRegistry::new();
        assert!(!registry.is_registered(&SituationType::new("test")));
    }

    #[test]
    fn test_registry_register_default() {
        let registry = SituationRegistry::new();
        let situation = GenericUnknownSituation::new(SituationType::new("test"));
        registry.register_default(Box::new(situation));

        assert!(registry.is_registered(&SituationType::new("test")));
    }

    #[test]
    fn test_registry_get() {
        let registry = SituationRegistry::new();
        let situation = GenericUnknownSituation::new(SituationType::new("test"));
        registry.register_default(Box::new(situation));

        let retrieved = registry.get(&SituationType::new("test"));
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().situation_type(),
            SituationType::new("test")
        );
    }

    #[test]
    fn test_registry_build_unknown_returns_fallback() {
        let registry = SituationRegistry::new();
        let situation = registry.build(SituationType::new("unknown_type"));

        assert_eq!(situation.implementation_type(), "GenericUnknownSituation");
        assert!(situation.requires_human());
    }

    #[test]
    fn test_registry_build_with_subtype_fallback_to_base() {
        let registry = SituationRegistry::new();
        // Register base type
        let situation = GenericUnknownSituation::new(SituationType::new("waiting_for_choice"));
        registry.register_default(Box::new(situation));

        // Request subtype should fall back to base
        let retrieved = registry.build(SituationType::with_subtype("waiting_for_choice", "codex"));
        assert_eq!(retrieved.situation_type().name, "waiting_for_choice");
    }

    #[test]
    fn test_registry_builder_priority() {
        let registry = SituationRegistry::new();

        // Register builder that returns specific situation
        registry.register_builder(SituationType::new("test"), || {
            Some(Box::new(GenericUnknownSituation::new(SituationType::new(
                "from_builder",
            ))))
        });

        let situation = registry.build(SituationType::new("test"));
        assert_eq!(situation.situation_type().name, "from_builder");
    }

    #[test]
    fn test_generic_unknown_situation() {
        let situation = GenericUnknownSituation::new(SituationType::new("unknown"));
        assert_eq!(situation.situation_type(), SituationType::new("unknown"));
        assert_eq!(situation.implementation_type(), "GenericUnknownSituation");
        assert!(situation.requires_human());
        assert_eq!(situation.human_urgency(), crate::types::UrgencyLevel::High);
    }

    #[test]
    fn test_registry_registered_types() {
        let registry = SituationRegistry::new();
        registry.register_default(Box::new(GenericUnknownSituation::new(SituationType::new(
            "a",
        ))));
        registry.register_default(Box::new(GenericUnknownSituation::new(SituationType::new(
            "b",
        ))));

        let types = registry.registered_types();
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn test_registry_thread_safe_concurrent_read() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(SituationRegistry::new());
        registry.register_default(Box::new(GenericUnknownSituation::new(SituationType::new(
            "test",
        ))));

        let threads: Vec<_> = (0..10)
            .map(|_| {
                let r = registry.clone();
                thread::spawn(move || r.get(&SituationType::new("test")).unwrap())
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }
    }
}
