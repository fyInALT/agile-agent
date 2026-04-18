//! Decision Strategy trait and implementations
//!
//! Sprint 10: Flexible strategy-based maker selection.
//!
//! The `DecisionStrategy` trait determines which `DecisionMaker` to use
//! for a given situation. Multiple strategies can be chained or combined.

use crate::context::DecisionContext;
use crate::maker::{DecisionMakerMeta, DecisionMakerType};
use crate::types::UrgencyLevel;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Decision Strategy trait
///
/// Determines which `DecisionMaker` to use for a given situation.
/// Implement this trait to create custom selection strategies.
///
/// # Responsibilities
///
/// 1. Analyze the decision context
/// 2. Select the appropriate maker type
/// 3. Provide fallback options
///
/// # Examples
///
/// ```rust,ignore
/// struct SituationBasedStrategy {
///     mappings: HashMap<String, DecisionMakerType>,
/// }
///
/// impl DecisionStrategy for SituationBasedStrategy {
///     fn select_maker(&self, context: &DecisionContext) -> DecisionMakerType {
///         let situation_name = context.trigger_situation.situation_type().name;
///         self.mappings.get(&situation_name)
///             .cloned()
///             .unwrap_or(DecisionMakerType::llm())
///     }
///
///     fn strategy_name(&self) -> &'static str {
///         "situation_based"
///     }
///
///     fn fallback(&self) -> Option<DecisionMakerType> {
///         Some(DecisionMakerType::rule_based())
///     }
/// }
/// ```
pub trait DecisionStrategy: Send + Sync {
    /// Select the appropriate maker type for the given context
    ///
    /// # Arguments
    ///
    /// * `context` - The decision context to analyze
    /// * `available_makers` - Metadata about available makers
    ///
    /// # Returns
    ///
    /// The selected `DecisionMakerType`, or the default if no suitable maker found.
    fn select_maker(
        &self,
        context: &DecisionContext,
        available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType;

    /// Get the strategy name for logging and debugging
    fn strategy_name(&self) -> &'static str;

    /// Get fallback maker type if primary selection fails
    fn fallback(&self) -> Option<DecisionMakerType>;

    /// Get priority for strategy chaining (higher = more important)
    fn priority(&self) -> u8 {
        100
    }

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn DecisionStrategy>;
}

impl fmt::Debug for dyn DecisionStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecisionStrategy")
            .field("name", &self.strategy_name())
            .field("priority", &self.priority())
            .finish()
    }
}

impl Clone for Box<dyn DecisionStrategy> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Strategy selection result
///
/// Contains the selected maker type and additional metadata
/// about the selection process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySelection {
    /// Selected maker type
    pub maker_type: DecisionMakerType,
    /// Strategy that made the selection
    pub strategy_name: String,
    /// Reason for selection
    pub reason: String,
    /// Fallback chain if primary fails
    pub fallback_chain: Vec<DecisionMakerType>,
}

impl StrategySelection {
    /// Create a new selection result
    pub fn new(
        maker_type: DecisionMakerType,
        strategy_name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            maker_type,
            strategy_name: strategy_name.into(),
            reason: reason.into(),
            fallback_chain: Vec::new(),
        }
    }

    /// Add fallback to the chain
    pub fn with_fallback(self, fallback: DecisionMakerType) -> Self {
        Self {
            fallback_chain: self
                .fallback_chain
                .into_iter()
                .chain(std::iter::once(fallback))
                .collect(),
            ..self
        }
    }

    /// Add multiple fallbacks to the chain
    pub fn with_fallbacks(self, fallbacks: Vec<DecisionMakerType>) -> Self {
        Self {
            fallback_chain: self.fallback_chain.into_iter().chain(fallbacks).collect(),
            ..self
        }
    }

    /// Get next fallback if primary fails
    pub fn next_fallback(&self) -> Option<&DecisionMakerType> {
        self.fallback_chain.first()
    }
}

// ============================================================================
// Built-in Strategy Implementations
// ============================================================================

/// Tiered Strategy
///
/// Selects makers based on situation complexity tiers:
/// - Simple → Rule-based
/// - Medium → LLM
/// - Complex → LLM
/// - Critical → Human
///
/// This is the default strategy matching the existing `TieredDecisionEngine` logic.
#[derive(Debug, Clone, Default)]
pub struct TieredStrategy {
    /// Custom tier mappings (optional)
    tier_mappings: std::collections::HashMap<String, DecisionMakerType>,
}

impl TieredStrategy {
    /// Create new tiered strategy
    pub fn new() -> Self {
        Self {
            tier_mappings: std::collections::HashMap::new(),
        }
    }

    /// Add custom tier mapping
    pub fn with_mapping(self, situation: impl Into<String>, maker: DecisionMakerType) -> Self {
        let mut mappings = self.tier_mappings;
        mappings.insert(situation.into(), maker);
        Self {
            tier_mappings: mappings,
        }
    }

    /// Determine tier from situation
    fn determine_tier(context: &DecisionContext) -> DecisionTier {
        let situation = context.trigger_situation.as_ref();
        let type_name = situation.situation_type().name;

        // Critical: human intervention required (highest priority)
        if situation.requires_human() {
            return DecisionTier::Critical;
        }

        // High urgency also triggers critical tier
        if situation.human_urgency() == UrgencyLevel::Critical {
            return DecisionTier::Critical;
        }

        // Complex: error recovery, partial completion
        if type_name == "error" || type_name == "partial_completion" {
            return DecisionTier::Complex;
        }

        // Claims completion needs verification (medium-high complexity)
        if type_name == "claims_completion" {
            return DecisionTier::Medium;
        }

        // Simple: well-known patterns that rule engine can handle
        if type_name == "waiting_for_choice" || type_name == "agent_idle" {
            return DecisionTier::Simple;
        }

        // Default: Medium
        DecisionTier::Medium
    }

    /// Get maker type for tier
    fn maker_for_tier(tier: DecisionTier) -> DecisionMakerType {
        match tier {
            DecisionTier::Simple => DecisionMakerType::rule_based(),
            DecisionTier::Medium => DecisionMakerType::llm(),
            DecisionTier::Complex => DecisionMakerType::llm(),
            DecisionTier::Critical => DecisionMakerType::human(),
        }
    }
}

/// Decision tier level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionTier {
    /// Tier 1: Simple - use rule-based maker
    Simple,
    /// Tier 2: Medium - Use LLM maker
    Medium,
    /// Tier 3: Complex - Use LLM maker with more context
    Complex,
    /// Tier 4: Critical - Use human maker
    Critical,
}

impl Default for DecisionTier {
    fn default() -> Self {
        DecisionTier::Medium
    }
}

impl DecisionTier {
    /// Get tier number (1-4)
    pub fn tier_number(&self) -> u8 {
        match self {
            DecisionTier::Simple => 1,
            DecisionTier::Medium => 2,
            DecisionTier::Complex => 3,
            DecisionTier::Critical => 4,
        }
    }

    /// Check if requires human
    pub fn requires_human(&self) -> bool {
        matches!(self, DecisionTier::Critical)
    }
}

impl DecisionStrategy for TieredStrategy {
    fn select_maker(
        &self,
        context: &DecisionContext,
        _available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType {
        let situation_type = context.trigger_situation.situation_type().name;

        // Check custom mappings first
        if let Some(custom_maker) = self.tier_mappings.get(&situation_type) {
            return custom_maker.clone();
        }

        // Use tier-based selection
        let tier = Self::determine_tier(context);
        Self::maker_for_tier(tier)
    }

    fn strategy_name(&self) -> &'static str {
        "tiered"
    }

    fn fallback(&self) -> Option<DecisionMakerType> {
        Some(DecisionMakerType::rule_based())
    }

    fn priority(&self) -> u8 {
        100 // Default priority
    }

    fn clone_boxed(&self) -> Box<dyn DecisionStrategy> {
        Box::new(self.clone())
    }
}

/// Situation Mapping Strategy
///
/// Directly maps situation types to maker types.
/// Useful for fine-grained control over maker selection.
#[derive(Debug, Clone)]
pub struct SituationMappingStrategy {
    /// Situation type → Maker type mappings
    mappings: std::collections::HashMap<String, DecisionMakerType>,
    /// Default maker for unmapped situations
    default_maker: DecisionMakerType,
    /// Fallback maker if default fails
    fallback_maker: Option<DecisionMakerType>,
}

impl SituationMappingStrategy {
    /// Create new strategy with mappings
    pub fn new(mappings: std::collections::HashMap<String, DecisionMakerType>) -> Self {
        Self {
            mappings,
            default_maker: DecisionMakerType::llm(),
            fallback_maker: Some(DecisionMakerType::rule_based()),
        }
    }

    /// Create with default maker
    pub fn with_default(
        mappings: std::collections::HashMap<String, DecisionMakerType>,
        default: DecisionMakerType,
    ) -> Self {
        Self {
            mappings,
            default_maker: default,
            fallback_maker: Some(DecisionMakerType::rule_based()),
        }
    }

    /// Add a mapping
    pub fn add_mapping(&mut self, situation: impl Into<String>, maker: DecisionMakerType) {
        self.mappings.insert(situation.into(), maker);
    }

    /// Get mappings
    pub fn mappings(&self) -> &std::collections::HashMap<String, DecisionMakerType> {
        &self.mappings
    }
}

impl Default for SituationMappingStrategy {
    fn default() -> Self {
        let mut mappings = std::collections::HashMap::new();
        // Default mappings matching tiered strategy
        mappings.insert(
            "waiting_for_choice".to_string(),
            DecisionMakerType::rule_based(),
        );
        mappings.insert("agent_idle".to_string(), DecisionMakerType::rule_based());
        mappings.insert("claims_completion".to_string(), DecisionMakerType::llm());
        mappings.insert("error".to_string(), DecisionMakerType::llm());
        mappings.insert("partial_completion".to_string(), DecisionMakerType::llm());

        Self {
            mappings,
            default_maker: DecisionMakerType::llm(),
            fallback_maker: Some(DecisionMakerType::rule_based()),
        }
    }
}

impl DecisionStrategy for SituationMappingStrategy {
    fn select_maker(
        &self,
        context: &DecisionContext,
        _available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType {
        let situation_type = context.trigger_situation.situation_type().name;

        // Check for requires_human override
        if context.trigger_situation.requires_human() {
            // Check if human maker is in mappings, otherwise use default
            if self.mappings.contains_key(&situation_type) {
                // Use the mapped maker only if it's human
                let mapped = self.mappings.get(&situation_type).unwrap();
                if mapped.name == "human" {
                    return mapped.clone();
                }
            }
            return DecisionMakerType::human();
        }

        // Use mappings
        self.mappings
            .get(&situation_type)
            .cloned()
            .unwrap_or_else(|| self.default_maker.clone())
    }

    fn strategy_name(&self) -> &'static str {
        "situation_mapping"
    }

    fn fallback(&self) -> Option<DecisionMakerType> {
        self.fallback_maker.clone()
    }

    fn priority(&self) -> u8 {
        90 // Lower than tiered
    }

    fn clone_boxed(&self) -> Box<dyn DecisionStrategy> {
        Box::new(self.clone())
    }
}

/// Adaptive Strategy
///
/// Adapts maker selection based on decision history and performance.
/// Tracks success rates and adjusts selection over time.
#[derive(Debug, Clone)]
pub struct AdaptiveStrategy {
    /// Base strategy to use for initial selection
    base_strategy: Box<dyn DecisionStrategy>,
    /// Success rate per maker type
    success_rates: std::collections::HashMap<String, f64>,
    /// Minimum samples before adaptation kicks in
    #[allow(dead_code)]
    min_samples: u32,
    /// Adaptation threshold (if success rate below this, try fallback)
    adaptation_threshold: f64,
}

impl AdaptiveStrategy {
    /// Create adaptive strategy with base strategy
    pub fn new(base_strategy: Box<dyn DecisionStrategy>) -> Self {
        Self {
            base_strategy,
            success_rates: std::collections::HashMap::new(),
            min_samples: 5,
            adaptation_threshold: 0.7,
        }
    }

    /// Set minimum samples for adaptation
    pub fn with_min_samples(self, min_samples: u32) -> Self {
        Self {
            min_samples,
            ..self
        }
    }

    /// Set adaptation threshold
    pub fn with_threshold(self, threshold: f64) -> Self {
        Self {
            adaptation_threshold: threshold,
            ..self
        }
    }

    /// Record a decision result
    pub fn record_result(&mut self, maker_type: &DecisionMakerType, success: bool) {
        let key = maker_type.name.clone();
        let current = self.success_rates.get(&key).copied().unwrap_or(0.8);

        // Simple exponential moving average
        let new_rate = if success {
            current * 0.9 + 1.0 * 0.1
        } else {
            current * 0.9 + 0.0 * 0.1
        };

        self.success_rates.insert(key, new_rate);
    }

    /// Get success rate for maker type
    pub fn success_rate(&self, maker_type: &DecisionMakerType) -> f64 {
        self.success_rates
            .get(&maker_type.name)
            .copied()
            .unwrap_or(0.8)
    }
}

impl DecisionStrategy for AdaptiveStrategy {
    fn select_maker(
        &self,
        context: &DecisionContext,
        available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType {
        // Use base strategy for initial selection
        let base_selection = self.base_strategy.select_maker(context, available_makers);

        // Check success rate
        let success_rate = self.success_rate(&base_selection);

        // If below threshold and we have fallback, use fallback
        if success_rate < self.adaptation_threshold {
            if let Some(fallback) = self.base_strategy.fallback() {
                // Check fallback success rate
                let fallback_rate = self.success_rate(&fallback);
                if fallback_rate > success_rate {
                    return fallback;
                }
            }
        }

        base_selection
    }

    fn strategy_name(&self) -> &'static str {
        "adaptive"
    }

    fn fallback(&self) -> Option<DecisionMakerType> {
        self.base_strategy.fallback()
    }

    fn priority(&self) -> u8 {
        110 // Higher than tiered
    }

    fn clone_boxed(&self) -> Box<dyn DecisionStrategy> {
        Box::new(self.clone())
    }
}

/// Composite Strategy
///
/// Combines multiple strategies with priority-based selection.
/// Higher priority strategies are consulted first.
#[derive(Debug, Clone, Default)]
pub struct CompositeStrategy {
    /// Strategy chain (ordered by priority)
    strategies: Vec<Box<dyn DecisionStrategy>>,
    /// Default maker if no strategy matches
    default_maker: DecisionMakerType,
}

impl CompositeStrategy {
    /// Create empty composite strategy
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
            default_maker: DecisionMakerType::llm(),
        }
    }

    /// Add a strategy
    pub fn add_strategy(&mut self, strategy: Box<dyn DecisionStrategy>) {
        // Insert sorted by priority (higher first)
        let priority = strategy.priority();
        let pos = self
            .strategies
            .iter()
            .position(|s| s.priority() < priority)
            .unwrap_or(self.strategies.len());
        self.strategies.insert(pos, strategy);
    }

    /// Get strategies
    pub fn strategies(&self) -> &[Box<dyn DecisionStrategy>] {
        &self.strategies
    }

    /// Set default maker
    pub fn with_default(self, default: DecisionMakerType) -> Self {
        Self {
            default_maker: default,
            ..self
        }
    }
}

impl DecisionStrategy for CompositeStrategy {
    fn select_maker(
        &self,
        context: &DecisionContext,
        available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType {
        // Try each strategy in order
        for strategy in &self.strategies {
            let selection = strategy.select_maker(context, available_makers);

            // Check if the selected maker is actually available
            if available_makers
                .iter()
                .any(|m| m.maker_type.matches(&selection))
            {
                return selection;
            }
        }

        // No strategy matched, use default
        self.default_maker.clone()
    }

    fn strategy_name(&self) -> &'static str {
        "composite"
    }

    fn fallback(&self) -> Option<DecisionMakerType> {
        // Return fallback from first strategy
        self.strategies.first().and_then(|s| s.fallback())
    }

    fn priority(&self) -> u8 {
        50 // Lower priority - usually used as top-level strategy
    }

    fn clone_boxed(&self) -> Box<dyn DecisionStrategy> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_situations::{
        ClaimsCompletionSituation, ErrorSituation, WaitingForChoiceSituation,
    };
    use crate::context::DecisionContext;
    use crate::situation::{ChoiceOption, ErrorInfo};

    fn make_context_waiting_for_choice() -> DecisionContext {
        DecisionContext::new(
            Box::new(WaitingForChoiceSituation::new(vec![ChoiceOption::new(
                "A", "Option A",
            )])),
            "test-agent",
        )
    }

    fn make_context_claims_completion() -> DecisionContext {
        DecisionContext::new(
            Box::new(ClaimsCompletionSituation::new("Task done")),
            "test-agent",
        )
    }

    fn make_context_error() -> DecisionContext {
        DecisionContext::new(
            Box::new(ErrorSituation::new(ErrorInfo::new("test", "Test error"))),
            "test-agent",
        )
    }

    fn make_available_makers() -> Vec<DecisionMakerMeta> {
        vec![
            DecisionMakerMeta::new(DecisionMakerType::rule_based())
                .with_situations(vec![
                    "waiting_for_choice".to_string(),
                    "agent_idle".to_string(),
                ])
                .with_priority(100),
            DecisionMakerMeta::new(DecisionMakerType::llm()).with_priority(90),
            DecisionMakerMeta::new(DecisionMakerType::human())
                .handles_critical()
                .with_priority(80),
        ]
    }

    #[test]
    fn test_strategy_selection_new() {
        let selection = StrategySelection::new(DecisionMakerType::llm(), "test", "test reason");
        assert_eq!(selection.maker_type.name, "llm");
        assert_eq!(selection.strategy_name, "test");
        assert!(selection.fallback_chain.is_empty());
    }

    #[test]
    fn test_strategy_selection_with_fallback() {
        let selection = StrategySelection::new(DecisionMakerType::llm(), "test", "test")
            .with_fallback(DecisionMakerType::rule_based());

        assert_eq!(selection.fallback_chain.len(), 1);
        assert_eq!(selection.next_fallback().unwrap().name, "rule_based");
    }

    #[test]
    fn test_tiered_strategy_simple() {
        let strategy = TieredStrategy::new();
        let context = make_context_waiting_for_choice();
        let makers = make_available_makers();

        let selection = strategy.select_maker(&context, &makers);
        assert_eq!(selection.name, "rule_based");
    }

    #[test]
    fn test_tiered_strategy_medium() {
        let strategy = TieredStrategy::new();
        let context = make_context_claims_completion();
        let makers = make_available_makers();

        let selection = strategy.select_maker(&context, &makers);
        assert_eq!(selection.name, "llm");
    }

    #[test]
    fn test_tiered_strategy_complex() {
        let strategy = TieredStrategy::new();
        let context = make_context_error();
        let makers = make_available_makers();

        let selection = strategy.select_maker(&context, &makers);
        assert_eq!(selection.name, "llm");
    }

    #[test]
    fn test_tiered_strategy_custom_mapping() {
        let strategy =
            TieredStrategy::new().with_mapping("claims_completion", DecisionMakerType::human());

        let context = make_context_claims_completion();
        let makers = make_available_makers();

        let selection = strategy.select_maker(&context, &makers);
        assert_eq!(selection.name, "human");
    }

    #[test]
    fn test_tiered_strategy_fallback() {
        let strategy = TieredStrategy::new();
        assert_eq!(strategy.fallback().unwrap().name, "rule_based");
    }

    #[test]
    fn test_decision_tier_number() {
        assert_eq!(DecisionTier::Simple.tier_number(), 1);
        assert_eq!(DecisionTier::Medium.tier_number(), 2);
        assert_eq!(DecisionTier::Complex.tier_number(), 3);
        assert_eq!(DecisionTier::Critical.tier_number(), 4);
    }

    #[test]
    fn test_situation_mapping_strategy() {
        let strategy = SituationMappingStrategy::default();
        let makers = make_available_makers();

        // Waiting for choice → rule_based
        let ctx1 = make_context_waiting_for_choice();
        assert_eq!(strategy.select_maker(&ctx1, &makers).name, "rule_based");

        // Claims completion → llm
        let ctx2 = make_context_claims_completion();
        assert_eq!(strategy.select_maker(&ctx2, &makers).name, "llm");
    }

    #[test]
    fn test_situation_mapping_strategy_custom() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("custom_situation".to_string(), DecisionMakerType::mock());

        let strategy = SituationMappingStrategy::new(mappings);

        let situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(crate::builtin_situations::WaitingForChoiceSituation::default());
        let context = DecisionContext::new(situation, "test");
        let makers = make_available_makers();

        // Unmapped situation → default (llm)
        assert_eq!(strategy.select_maker(&context, &makers).name, "llm");
    }

    #[test]
    fn test_composite_strategy() {
        let mut composite = CompositeStrategy::new();
        composite.add_strategy(Box::new(TieredStrategy::new()));
        composite.add_strategy(Box::new(SituationMappingStrategy::default()));

        let context = make_context_waiting_for_choice();
        let makers = make_available_makers();

        let selection = composite.select_maker(&context, &makers);
        // Should use higher priority strategy (tiered)
        assert_eq!(selection.name, "rule_based");
    }

    #[test]
    fn test_strategy_clone_boxed() {
        let strategy: Box<dyn DecisionStrategy> = Box::new(TieredStrategy::new());
        let cloned = strategy.clone_boxed();

        assert_eq!(strategy.strategy_name(), cloned.strategy_name());
    }
}
