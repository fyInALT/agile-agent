//! Action registry with thread-safe operations and serialization

use crate::action::DecisionAction;
use crate::types::ActionType;
use std::collections::HashMap;
use std::sync::RwLock;

/// Action parser function type
type ActionParser = Box<dyn Fn(&str) -> Option<Box<dyn DecisionAction>> + Send + Sync>;

/// Action deserializer function type
type ActionDeserializer = Box<dyn Fn(&str) -> Option<Box<dyn DecisionAction>> + Send + Sync>;

/// Action registry - THREAD-SAFE with RwLock and serialization
pub struct ActionRegistry {
    /// Registered actions by type (thread-safe)
    actions: RwLock<HashMap<ActionType, Box<dyn DecisionAction>>>,

    /// Action parsers (parse from LLM output, thread-safe)
    parsers: RwLock<HashMap<ActionType, ActionParser>>,

    /// Action deserializers (for persistence, thread-safe)
    deserializers: RwLock<HashMap<ActionType, ActionDeserializer>>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            actions: RwLock::new(HashMap::new()),
            parsers: RwLock::new(HashMap::new()),
            deserializers: RwLock::new(HashMap::new()),
        }
    }

    /// THREAD-SAFE: Register an action
    pub fn register(&self, action: Box<dyn DecisionAction>) {
        self.actions.write().unwrap().insert(action.action_type(), action);
    }

    /// THREAD-SAFE: Register an action parser
    pub fn register_parser(
        &self,
        type_: ActionType,
        parser: impl Fn(&str) -> Option<Box<dyn DecisionAction>> + Send + Sync + 'static,
    ) {
        self.parsers.write().unwrap().insert(type_, Box::new(parser));
    }

    /// THREAD-SAFE: Register an action deserializer
    pub fn register_deserializer(
        &self,
        type_: ActionType,
        deserializer: impl Fn(&str) -> Option<Box<dyn DecisionAction>> + Send + Sync + 'static,
    ) {
        self.deserializers.write().unwrap().insert(type_, Box::new(deserializer));
    }

    /// THREAD-SAFE: Get action by type
    pub fn get(&self, type_: &ActionType) -> Option<Box<dyn DecisionAction>> {
        self.actions
            .read()
            .unwrap()
            .get(type_)
            .map(|a| a.clone_boxed())
    }

    /// THREAD-SAFE: Parse action from LLM output
    pub fn parse(&self, type_: ActionType, output: &str) -> Option<Box<dyn DecisionAction>> {
        self.parsers
            .read()
            .unwrap()
            .get(&type_)
            .and_then(|parser| parser(output))
    }

    /// Deserialize action from serialized params
    pub fn deserialize(&self, type_: &ActionType, params: &str) -> Option<Box<dyn DecisionAction>> {
        self.deserializers
            .read()
            .unwrap()
            .get(type_)
            .and_then(|deser| deser(params))
    }

    /// THREAD-SAFE: Check if type is registered
    pub fn is_registered(&self, type_: &ActionType) -> bool {
        self.actions.read().unwrap().contains_key(type_)
    }

    /// THREAD-SAFE: Get all registered action types
    pub fn registered_types(&self) -> Vec<ActionType> {
        self.actions
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }

    /// Generate prompt format for all actions
    pub fn generate_prompt_formats(&self) -> String {
        self.actions
            .read()
            .unwrap()
            .values()
            .map(|a| a.to_prompt_format())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_actions::SelectOptionAction;

    #[test]
    fn test_registry_new() {
        let registry = ActionRegistry::new();
        assert!(!registry.is_registered(&ActionType::new("test")));
    }

    #[test]
    fn test_registry_register() {
        let registry = ActionRegistry::new();
        registry.register(Box::new(SelectOptionAction::new("A", "test")));

        assert!(registry.is_registered(&ActionType::new("select_option")));
    }

    #[test]
    fn test_registry_get() {
        let registry = ActionRegistry::new();
        registry.register(Box::new(SelectOptionAction::new("A", "test")));

        let retrieved = registry.get(&ActionType::new("select_option"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().action_type(), ActionType::new("select_option"));
    }

    #[test]
    fn test_registry_parse() {
        let registry = ActionRegistry::new();
        registry.register_parser(ActionType::new("select_option"), |output| {
            // Simple parser for testing
            if output.contains("Selection:") {
                Some(Box::new(SelectOptionAction::new("A", "parsed")))
            } else {
                None
            }
        });

        let parsed = registry.parse(ActionType::new("select_option"), "Selection: [A]");
        assert!(parsed.is_some());
    }

    #[test]
    fn test_registry_registered_types() {
        let registry = ActionRegistry::new();
        registry.register(Box::new(SelectOptionAction::new("A", "test")));

        let types = registry.registered_types();
        assert_eq!(types.len(), 1);
    }

    #[test]
    fn test_registry_thread_safe_concurrent_read() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(ActionRegistry::new());
        registry.register(Box::new(SelectOptionAction::new("A", "test")));

        let threads: Vec<_> = (0..10)
            .map(|_| {
                let r = registry.clone();
                thread::spawn(move || {
                    r.get(&ActionType::new("select_option")).unwrap()
                })
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }
    }

    #[test]
    fn test_generate_prompt_formats() {
        let registry = ActionRegistry::new();
        registry.register(Box::new(SelectOptionAction::new("A", "test")));

        let formats = registry.generate_prompt_formats();
        assert!(!formats.is_empty());
    }
}