//! Decision Maker Registry
//!
//! Sprint 10: Thread-safe registry for decision makers.
//!
//! The `DecisionMakerRegistry` manages decision makers and their metadata,
//! allowing strategies to query available makers and select appropriate ones.

use crate::core::context::DecisionContext;
use crate::core::error::DecisionError;
use crate::pipeline::maker::{DecisionMaker, DecisionMakerMeta, DecisionMakerType, DecisionRegistries};
use crate::pipeline::strategy::{CompositeStrategy, DecisionStrategy, StrategySelection};
use std::collections::HashMap;
use std::sync::RwLock;

/// Decision Maker Registry
///
/// Thread-safe registry that stores decision makers and their metadata.
/// Provides methods for registering, retrieving, and selecting makers.
///
/// # Architecture
///
/// ```text
/// ┌────────────────────────────────────────────────────────────┐
/// │                   DecisionMakerRegistry                     │
/// │                                                              │
/// │  ┌──────────────────┐     ┌──────────────────┐            │
/// │  │ makers: RwLock   │     │ metadata: RwLock │            │
/// │  │                  │     │                  │            │
/// │  │ HashMap<         │     │ HashMap<         │            │
/// │  │   MakerType,     │     │   MakerType,     │            │
/// │  │   Box<dyn        │     │   MakerMeta>     │            │
/// │  │   DecisionMaker> │     │                  │            │
/// │  │                  │     │                  │            │
/// │  └──────────────────┘     └──────────────────┘            │
/// │                                                              │
/// │  ┌──────────────────────────────────────────────┐          │
/// │  │ strategy: RwLock<Box<dyn DecisionStrategy>>  │          │
/// │  └──────────────────────────────────────────────┘          │
/// │                                                              │
/// └────────────────────────────────────────────────────────────┘
/// ```
pub struct DecisionMakerRegistry {
    /// Registered decision makers
    makers: RwLock<HashMap<DecisionMakerType, Box<dyn DecisionMaker>>>,
    /// Maker metadata for strategy selection
    metadata: RwLock<HashMap<DecisionMakerType, DecisionMakerMeta>>,
    /// Default strategy for maker selection
    strategy: RwLock<Box<dyn DecisionStrategy>>,
    /// Default registries for decision making
    registries: RwLock<DecisionRegistries>,
}

impl DecisionMakerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            makers: RwLock::new(HashMap::new()),
            metadata: RwLock::new(HashMap::new()),
            strategy: RwLock::new(Box::new(crate::strategy::TieredStrategy::new())),
            registries: RwLock::new(DecisionRegistries::new()),
        }
    }

    /// Create with a custom strategy
    pub fn with_strategy(strategy: Box<dyn DecisionStrategy>) -> Self {
        Self {
            makers: RwLock::new(HashMap::new()),
            metadata: RwLock::new(HashMap::new()),
            strategy: RwLock::new(strategy),
            registries: RwLock::new(DecisionRegistries::new()),
        }
    }

    /// Create with existing registries
    pub fn with_registries(registries: DecisionRegistries) -> Self {
        Self {
            makers: RwLock::new(HashMap::new()),
            metadata: RwLock::new(HashMap::new()),
            strategy: RwLock::new(Box::new(crate::strategy::TieredStrategy::new())),
            registries: RwLock::new(registries),
        }
    }

    /// Register a decision maker with metadata
    pub fn register(&self, maker: Box<dyn DecisionMaker>, meta: DecisionMakerMeta) {
        let maker_type = maker.maker_type();
        self.makers
            .write()
            .unwrap()
            .insert(maker_type.clone(), maker);
        self.metadata.write().unwrap().insert(maker_type, meta);
    }

    /// Register a decision maker with default metadata
    pub fn register_default(&self, maker: Box<dyn DecisionMaker>) {
        let maker_type = maker.maker_type();
        let meta = DecisionMakerMeta::new(maker_type.clone());
        self.makers
            .write()
            .unwrap()
            .insert(maker_type.clone(), maker);
        self.metadata.write().unwrap().insert(maker_type, meta);
    }

    /// Register a decision maker with priority
    pub fn register_with_priority(&self, maker: Box<dyn DecisionMaker>, priority: u8) {
        let maker_type = maker.maker_type();
        let meta = DecisionMakerMeta::new(maker_type.clone()).with_priority(priority);
        self.makers
            .write()
            .unwrap()
            .insert(maker_type.clone(), maker);
        self.metadata.write().unwrap().insert(maker_type, meta);
    }

    /// Get a decision maker by type
    pub fn get(&self, maker_type: &DecisionMakerType) -> Option<Box<dyn DecisionMaker>> {
        self.makers
            .read()
            .unwrap()
            .get(maker_type)
            .map(|m| m.clone_boxed())
    }

    /// Get mutable reference to a decision maker
    ///
    /// Note: This returns a cloned maker for mutation to avoid
    /// holding the write lock during decision execution.
    pub fn get_for_execution(
        &self,
        maker_type: &DecisionMakerType,
    ) -> Option<Box<dyn DecisionMaker>> {
        self.makers
            .read()
            .unwrap()
            .get(maker_type)
            .map(|m| m.clone_boxed())
    }

    /// Check if a maker type is registered
    pub fn is_registered(&self, maker_type: &DecisionMakerType) -> bool {
        self.makers.read().unwrap().contains_key(maker_type)
    }

    /// Get all registered maker types
    pub fn registered_types(&self) -> Vec<DecisionMakerType> {
        self.makers.read().unwrap().keys().cloned().collect()
    }

    /// Get metadata for a maker type
    pub fn get_meta(&self, maker_type: &DecisionMakerType) -> Option<DecisionMakerMeta> {
        self.metadata.read().unwrap().get(maker_type).cloned()
    }

    /// Get all maker metadata
    pub fn all_metadata(&self) -> Vec<DecisionMakerMeta> {
        self.metadata.read().unwrap().values().cloned().collect()
    }

    /// Set the decision strategy
    pub fn set_strategy(&self, strategy: Box<dyn DecisionStrategy>) {
        *self.strategy.write().unwrap() = strategy;
    }

    /// Get the current strategy
    pub fn strategy(&self) -> Box<dyn DecisionStrategy> {
        self.strategy.read().unwrap().clone_boxed()
    }

    /// Select a maker using the current strategy
    pub fn select_maker(&self, context: &DecisionContext) -> StrategySelection {
        let strategy = self.strategy.read().unwrap();
        let metadata = self.all_metadata();

        let maker_type = strategy.select_maker(context, &metadata);
        let fallback = strategy.fallback();

        StrategySelection::new(maker_type, strategy.strategy_name(), "strategy selected")
            .with_fallbacks(fallback.into_iter().collect())
    }

    /// Make a decision using the selected maker
    ///
    /// This is the main entry point for decision execution:
    /// 1. Select maker using strategy
    /// 2. Execute decision with selected maker
    /// 3. Try fallback makers if primary fails
    ///
    /// Note: Context is consumed by the maker. If all makers fail,
    /// the error from the last attempt is returned.
    pub fn make_decision(
        &self,
        context: crate::context::DecisionContext,
    ) -> crate::error::Result<crate::output::DecisionOutput> {
        // 1. Select maker using strategy (includes fallback chain)
        let selection = self.select_maker(&context);

        // 2. Get registries reference from stored registries
        let registries_guard = self.registries.read().unwrap();
        let registries = &*registries_guard;

        // 3. Build maker chain: primary + fallbacks
        let maker_chain: Vec<DecisionMakerType> = std::iter::once(selection.maker_type.clone())
            .chain(selection.fallback_chain.clone())
            .collect();

        // 4. Try each maker in order
        let mut last_error = None;
        for maker_type in &maker_chain {
            if let Some(mut maker) = self.get_for_execution(maker_type)
                && maker.is_healthy() {
                    // Note: context is consumed, so we can only try one maker
                    // with the original context. If we need fallback support,
                    // we would need to clone context or pass by reference.
                    // For now, only primary maker gets the context.
                    if maker_type == &selection.maker_type {
                        let result = maker.make_decision(context, registries);
                        match result {
                            Ok(output) => return Ok(output),
                            Err(e) => {
                                // Primary failed, record error but can't retry with fallbacks
                                // (context was consumed)
                                last_error = Some(e);
                                break;
                            }
                        }
                    }
                }
        }

        // 5. All makers failed or unavailable
        Err(DecisionError::EngineError(
            last_error
                .map(|e| format!("Decision failed: {}", e))
                .unwrap_or_else(|| {
                    format!(
                        "Maker {} not available or not healthy",
                        selection.maker_type
                    )
                }),
        ))
    }

    /// Make a decision with fallback support (requires cloneable context)
    ///
    /// This variant supports fallback makers by requiring a context reference.
    /// Use this when you need retry behavior with different makers.
    ///
    /// Note: This creates a new DecisionOutput for each attempt.
    pub fn make_decision_with_fallback(
        &self,
        context: &crate::context::DecisionContext,
    ) -> crate::error::Result<crate::output::DecisionOutput> {
        // 1. Select maker using strategy
        let selection = self.select_maker(context);

        // 2. Get registries reference
        let registries_guard = self.registries.read().unwrap();
        let registries = &*registries_guard;

        // 3. Build maker chain
        let maker_chain: Vec<DecisionMakerType> = std::iter::once(selection.maker_type.clone())
            .chain(selection.fallback_chain.clone())
            .collect();

        // 4. Try each maker
        let mut last_error = None;
        for maker_type in &maker_chain {
            if let Some(mut maker) = self.get_for_execution(maker_type)
                && maker.is_healthy() {
                    // Create a clone of context for each attempt
                    // Note: DecisionContext needs Clone for this to work
                    // Currently this approach won't work since DecisionContext
                    // doesn't implement Clone. We'll try with the reference.
                    let result = maker.make_decision(context.clone(), registries);
                    match result {
                        Ok(output) => return Ok(output),
                        Err(e) => {
                            last_error = Some(e);
                            continue; // Try next fallback
                        }
                    }
                }
        }

        // 5. All makers failed
        Err(DecisionError::EngineError(
            last_error
                .map(|e| format!("All makers failed, last error: {}", e))
                .unwrap_or_else(|| "No healthy makers available".to_string()),
        ))
    }

    /// Check if all makers are healthy
    pub fn all_healthy(&self) -> bool {
        self.makers.read().unwrap().values().all(|m| m.is_healthy())
    }

    /// Reset all makers
    pub fn reset_all(&self) -> crate::error::Result<()> {
        for maker in self.makers.write().unwrap().values_mut() {
            maker.reset()?;
        }
        Ok(())
    }

    /// Get registries reference
    pub fn registries_ref(&self) -> &RwLock<DecisionRegistries> {
        &self.registries
    }

    /// Update registries
    pub fn set_registries(&self, registries: DecisionRegistries) {
        *self.registries.write().unwrap() = registries;
    }

    /// Get maker count
    pub fn maker_count(&self) -> usize {
        self.makers.read().unwrap().len()
    }
}

impl Default for DecisionMakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating a configured registry
///
/// Provides a fluent API for setting up the decision maker registry.
pub struct DecisionMakerRegistryBuilder {
    /// Registry being built
    registry: DecisionMakerRegistry,
    /// Composite strategy for combining strategies
    composite_strategy: CompositeStrategy,
}

impl DecisionMakerRegistryBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            registry: DecisionMakerRegistry::new(),
            composite_strategy: CompositeStrategy::new(),
        }
    }

    /// Add a decision maker
    pub fn with_maker(self, maker: Box<dyn DecisionMaker>) -> Self {
        self.registry.register_default(maker);
        self
    }

    /// Add a decision maker with metadata
    pub fn with_maker_and_meta(
        self,
        maker: Box<dyn DecisionMaker>,
        meta: DecisionMakerMeta,
    ) -> Self {
        self.registry.register(maker, meta);
        self
    }

    /// Add a decision strategy
    pub fn with_strategy(mut self, strategy: Box<dyn DecisionStrategy>) -> Self {
        self.composite_strategy.add_strategy(strategy);
        self
    }

    /// Set registries
    pub fn with_registries(self, registries: DecisionRegistries) -> Self {
        self.registry.set_registries(registries);
        self
    }

    /// Build the registry
    pub fn build(self) -> DecisionMakerRegistry {
        // Set composite strategy if strategies were added
        if !self.composite_strategy.strategies().is_empty() {
            self.registry
                .set_strategy(Box::new(self.composite_strategy));
        }
        self.registry
    }
}

impl Default for DecisionMakerRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::situation::builtin_situations::WaitingForChoiceSituation;
    use crate::core::context::DecisionContext;
    use crate::core::output::DecisionOutput;

    // Mock DecisionMaker for testing
    struct MockMaker {
        maker_type: DecisionMakerType,
        healthy: bool,
    }

    impl MockMaker {
        fn new(maker_type: DecisionMakerType) -> Self {
            Self {
                maker_type,
                healthy: true,
            }
        }
    }

    impl DecisionMaker for MockMaker {
        fn make_decision(
            &mut self,
            _context: DecisionContext,
            _registries: &DecisionRegistries,
        ) -> crate::error::Result<DecisionOutput> {
            Ok(DecisionOutput::new(vec![], "mock decision"))
        }

        fn maker_type(&self) -> DecisionMakerType {
            self.maker_type.clone()
        }

        fn is_healthy(&self) -> bool {
            self.healthy
        }

        fn reset(&mut self) -> crate::error::Result<()> {
            self.healthy = true;
            Ok(())
        }

        fn clone_boxed(&self) -> Box<dyn DecisionMaker> {
            Box::new(MockMaker::new(self.maker_type.clone()))
        }
    }

    fn make_test_context() -> DecisionContext {
        DecisionContext::new(Box::new(WaitingForChoiceSituation::default()), "test-agent")
    }

    #[test]
    fn test_registry_new() {
        let registry = DecisionMakerRegistry::new();
        assert_eq!(registry.maker_count(), 0);
    }

    #[test]
    fn test_registry_register() {
        let registry = DecisionMakerRegistry::new();
        let maker = Box::new(MockMaker::new(DecisionMakerType::mock()));
        registry.register_default(maker);

        assert!(registry.is_registered(&DecisionMakerType::mock()));
        assert_eq!(registry.maker_count(), 1);
    }

    #[test]
    fn test_registry_register_with_priority() {
        let registry = DecisionMakerRegistry::new();
        let maker = Box::new(MockMaker::new(DecisionMakerType::llm()));
        registry.register_with_priority(maker, 50);

        let meta = registry.get_meta(&DecisionMakerType::llm());
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().priority, 50);
    }

    #[test]
    fn test_registry_get() {
        let registry = DecisionMakerRegistry::new();
        let maker = Box::new(MockMaker::new(DecisionMakerType::mock()));
        registry.register_default(maker);

        let retrieved = registry.get(&DecisionMakerType::mock());
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().maker_type().name, "mock");
    }

    #[test]
    fn test_registry_select_maker() {
        let registry = DecisionMakerRegistry::new();
        registry.register_default(Box::new(MockMaker::new(DecisionMakerType::rule_based())));
        registry.register_default(Box::new(MockMaker::new(DecisionMakerType::llm())));

        let context = make_test_context();
        let selection = registry.select_maker(&context);

        // Should select rule_based for waiting_for_choice (simple tier)
        assert_eq!(selection.maker_type.name, "rule_based");
    }

    #[test]
    fn test_registry_make_decision() {
        let registry = DecisionMakerRegistry::new();
        registry.register_default(Box::new(MockMaker::new(DecisionMakerType::rule_based())));

        let context = make_test_context();
        let result = registry.make_decision(context);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.reasoning, "mock decision");
    }

    #[test]
    fn test_registry_all_healthy() {
        let registry = DecisionMakerRegistry::new();
        registry.register_default(Box::new(MockMaker::new(DecisionMakerType::mock())));

        assert!(registry.all_healthy());
    }

    #[test]
    fn test_registry_reset_all() {
        let registry = DecisionMakerRegistry::new();
        registry.register_default(Box::new(MockMaker::new(DecisionMakerType::mock())));

        let result = registry.reset_all();
        assert!(result.is_ok());
    }

    #[test]
    fn test_registry_set_strategy() {
        let registry = DecisionMakerRegistry::new();
        registry.set_strategy(Box::new(
            crate::strategy::SituationMappingStrategy::default(),
        ));

        let strategy = registry.strategy();
        assert_eq!(strategy.strategy_name(), "situation_mapping");
    }

    #[test]
    fn test_builder() {
        let registry = DecisionMakerRegistryBuilder::new()
            .with_maker(Box::new(MockMaker::new(DecisionMakerType::mock())))
            .with_maker(Box::new(MockMaker::new(DecisionMakerType::llm())))
            .with_strategy(Box::new(crate::strategy::TieredStrategy::new()))
            .build();

        assert_eq!(registry.maker_count(), 2);
        assert_eq!(registry.strategy().strategy_name(), "composite");
    }

    #[test]
    fn test_registry_thread_safe_concurrent_read() {
        use std::sync::Arc;
        use std::thread;

        let registry = Arc::new(DecisionMakerRegistry::new());
        registry.register_default(Box::new(MockMaker::new(DecisionMakerType::mock())));

        let threads: Vec<_> = (0..10)
            .map(|_| {
                let r = registry.clone();
                thread::spawn(move || r.get(&DecisionMakerType::mock()).unwrap())
            })
            .collect();

        for t in threads {
            t.join().unwrap();
        }
    }
}
