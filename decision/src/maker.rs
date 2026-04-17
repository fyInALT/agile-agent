//! Decision Maker trait and types
//!
//! Sprint 10: Flexible decision-making abstraction.
//!
//! The `DecisionMaker` trait is the core abstraction for executing decisions.
//! Different implementations can be plugged in to support various decision
//! mechanisms (rule-based, LLM-based, human-based, etc.).

use crate::action_registry::ActionRegistry;
use crate::context::DecisionContext;
use crate::output::DecisionOutput;
use crate::situation_registry::SituationRegistry;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Decision Maker type identifier
///
/// Used to identify and select different decision makers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionMakerType {
    /// Maker name (e.g., "rule_based", "llm", "human")
    pub name: String,
    /// Optional subtype for variants
    pub subtype: Option<String>,
}

impl DecisionMakerType {
    /// Create a new maker type
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            subtype: None,
        }
    }

    /// Create a maker type with subtype
    pub fn with_subtype(name: impl Into<String>, subtype: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            subtype: Some(subtype.into()),
        }
    }

    /// Get base type (without subtype)
    pub fn base_type(&self) -> DecisionMakerType {
        if self.subtype.is_some() {
            DecisionMakerType::new(&self.name)
        } else {
            self.clone()
        }
    }

    /// Check if this matches another type
    pub fn matches(&self, other: &DecisionMakerType) -> bool {
        self == other || self.base_type() == other.base_type()
    }

    /// Predefined maker types
    pub fn rule_based() -> Self {
        Self::new("rule_based")
    }

    pub fn llm() -> Self {
        Self::new("llm")
    }

    pub fn human() -> Self {
        Self::new("human")
    }

    pub fn mock() -> Self {
        Self::new("mock")
    }

    pub fn tiered() -> Self {
        Self::new("tiered")
    }

    pub fn custom(name: impl Into<String>) -> Self {
        Self::new(name)
    }
}

impl fmt::Display for DecisionMakerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.subtype {
            Some(subtype) => write!(f, "{}.{}", self.name, subtype),
            None => write!(f, "{}", self.name),
        }
    }
}

impl Default for DecisionMakerType {
    fn default() -> Self {
        Self::rule_based()
    }
}

/// Decision registries bundle
///
/// Contains all registries needed for decision making.
/// This bundle is passed to decision makers to avoid
/// passing multiple parameters.
pub struct DecisionRegistries {
    /// Action registry for action types
    pub actions: ActionRegistry,
    /// Situation registry for building situations
    pub situations: SituationRegistry,
}

impl fmt::Debug for DecisionRegistries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecisionRegistries")
            .field("actions_count", &self.actions.registered_types().len())
            .field("situations_count", &self.situations.registered_types().len())
            .finish()
    }
}

impl DecisionRegistries {
    /// Create a new registries bundle
    pub fn new() -> Self {
        Self {
            actions: ActionRegistry::new(),
            situations: SituationRegistry::new(),
        }
    }

    /// Create with existing registries
    pub fn with_registries(actions: ActionRegistry, situations: SituationRegistry) -> Self {
        Self {
            actions,
            situations,
        }
    }

    /// Get action registry
    pub fn actions(&self) -> &ActionRegistry {
        &self.actions
    }

    /// Get situation registry
    pub fn situations(&self) -> &SituationRegistry {
        &self.situations
    }
}

impl Default for DecisionRegistries {
    fn default() -> Self {
        Self::new()
    }
}

/// Decision Maker trait
///
/// The core abstraction for executing decisions.
/// Implement this trait to create custom decision makers.
///
/// # Responsibilities
///
/// 1. Execute decision logic given context
/// 2. Return a `DecisionOutput` with actions
/// 3. Maintain internal state (if needed)
/// 4. Report health status
///
/// # Examples
///
/// ```rust,ignore
/// struct MyDecisionMaker {
///     config: MyConfig,
/// }
///
/// impl DecisionMaker for MyDecisionMaker {
///     fn make_decision(
///         &mut self,
///         context: DecisionContext,
///         registries: &DecisionRegistries,
///     ) -> crate::error::Result<DecisionOutput> {
///         // Custom decision logic
///         Ok(DecisionOutput::new(vec![], "my decision"))
///     }
///
///     fn maker_type(&self) -> DecisionMakerType {
///         DecisionMakerType::custom("my_maker")
///     }
///
///     fn is_healthy(&self) -> bool {
///         true
///     }
///
///     fn reset(&mut self) -> crate::error::Result<()> {
///         Ok(())
///     }
/// }
/// ```
pub trait DecisionMaker: Send + Sync {
    /// Execute decision logic
    ///
    /// Given a decision context and registries, produce a decision output.
    ///
    /// # Arguments
    ///
    /// * `context` - The decision context containing situation and metadata
    /// * `registries` - Bundle of action and situation registries
    ///
    /// # Returns
    ///
    /// A `DecisionOutput` containing actions to execute, or an error.
    fn make_decision(
        &mut self,
        context: DecisionContext,
        registries: &DecisionRegistries,
    ) -> crate::error::Result<DecisionOutput>;

    /// Get the maker type identifier
    fn maker_type(&self) -> DecisionMakerType;

    /// Check if the maker is healthy and ready to make decisions
    fn is_healthy(&self) -> bool;

    /// Reset internal state
    ///
    /// Called when the maker should start fresh (e.g., for a new task/story).
    fn reset(&mut self) -> crate::error::Result<()>;

    /// Clone into a boxed trait object
    fn clone_boxed(&self) -> Box<dyn DecisionMaker>;
}

impl fmt::Debug for dyn DecisionMaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecisionMaker")
            .field("type", &self.maker_type())
            .field("healthy", &self.is_healthy())
            .finish()
    }
}

impl Clone for Box<dyn DecisionMaker> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Decision Maker metadata
///
/// Contains information about a decision maker's capabilities
/// and configuration. Used for strategy selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionMakerMeta {
    /// Maker type
    pub maker_type: DecisionMakerType,
    /// Supported situation types (empty means all)
    pub supported_situations: Vec<String>,
    /// Priority for default selection
    pub priority: u8,
    /// Whether this maker can handle critical situations
    pub handles_critical: bool,
    /// Estimated latency in milliseconds
    pub estimated_latency_ms: u64,
}

impl DecisionMakerMeta {
    /// Create metadata for a maker
    pub fn new(maker_type: DecisionMakerType) -> Self {
        Self {
            maker_type,
            supported_situations: Vec::new(),
            priority: 100,
            handles_critical: false,
            estimated_latency_ms: 100,
        }
    }

    /// Set supported situations
    pub fn with_situations(self, situations: Vec<String>) -> Self {
        Self {
            supported_situations: situations,
            ..self
        }
    }

    /// Set priority
    pub fn with_priority(self, priority: u8) -> Self {
        Self { priority, ..self }
    }

    /// Enable critical handling
    pub fn handles_critical(self) -> Self {
        Self {
            handles_critical: true,
            ..self
        }
    }

    /// Set estimated latency
    pub fn with_latency(self, latency_ms: u64) -> Self {
        Self {
            estimated_latency_ms: latency_ms,
            ..self
        }
    }

    /// Check if maker supports a situation
    pub fn supports_situation(&self, situation_type: &str) -> bool {
        self.supported_situations.is_empty()
            || self.supported_situations.contains(&situation_type.to_string())
    }
}

impl Default for DecisionMakerMeta {
    fn default() -> Self {
        Self::new(DecisionMakerType::rule_based())
    }
}

/// Decision Pre-Processor trait
///
/// Hook for pre-processing decision context before the maker runs.
/// Use this to enrich context, validate inputs, or modify metadata.
///
/// # Examples
///
/// ```rust,ignore
/// struct AddReflectionRoundProcessor;
///
/// impl DecisionPreProcessor for AddReflectionRoundProcessor {
///     fn process(&self, context: &mut DecisionContext) -> crate::error::Result<()> {
///         // Add reflection round from state
///         context.metadata.insert("reflection_round".to_string(), "1".to_string());
///         Ok(())
///     }
///
///     fn processor_name(&self) -> &'static str {
///         "add_reflection_round"
///     }
/// }
/// ```
pub trait DecisionPreProcessor: Send + Sync {
    /// Process the context before decision
    fn process(&self, context: &mut DecisionContext) -> crate::error::Result<()>;

    /// Get processor name for logging
    fn processor_name(&self) -> &'static str;

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn DecisionPreProcessor>;
}

impl Clone for Box<dyn DecisionPreProcessor> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// Decision Post-Processor trait
///
/// Hook for post-processing decision output after the maker runs.
/// Use this to validate actions, add metadata, or modify output.
///
/// # Examples
///
/// ```rust,ignore
/// struct ValidateActionsProcessor;
///
/// impl DecisionPostProcessor for ValidateActionsProcessor {
///     fn process(&self, output: &mut DecisionOutput) -> crate::error::Result<()> {
///         if !output.has_actions() {
///             return Err(crate::error::DecisionError::NoActions);
///         }
///         Ok(())
///     }
///
///     fn processor_name(&self) -> &'static str {
///         "validate_actions"
///     }
/// }
/// ```
pub trait DecisionPostProcessor: Send + Sync {
    /// Process the output after decision
    fn process(&self, output: &mut DecisionOutput) -> crate::error::Result<()>;

    /// Get processor name for logging
    fn processor_name(&self) -> &'static str;

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn DecisionPostProcessor>;
}

impl Clone for Box<dyn DecisionPostProcessor> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maker_type_new() {
        let mt = DecisionMakerType::new("rule_based");
        assert_eq!(mt.name, "rule_based");
        assert_eq!(mt.subtype, None);
    }

    #[test]
    fn test_maker_type_with_subtype() {
        let mt = DecisionMakerType::with_subtype("llm", "claude");
        assert_eq!(mt.name, "llm");
        assert_eq!(mt.subtype, Some("claude".to_string()));
    }

    #[test]
    fn test_maker_type_display() {
        let mt1 = DecisionMakerType::new("rule_based");
        assert_eq!(format!("{}", mt1), "rule_based");

        let mt2 = DecisionMakerType::with_subtype("llm", "claude");
        assert_eq!(format!("{}", mt2), "llm.claude");
    }

    #[test]
    fn test_maker_type_matches() {
        let mt1 = DecisionMakerType::with_subtype("llm", "claude");
        let mt2 = DecisionMakerType::new("llm");
        let mt3 = DecisionMakerType::new("human");

        assert!(mt1.matches(&mt2));
        assert!(mt2.matches(&mt1));
        assert!(!mt1.matches(&mt3));
    }

    #[test]
    fn test_maker_type_predefined() {
        assert_eq!(DecisionMakerType::rule_based().name, "rule_based");
        assert_eq!(DecisionMakerType::llm().name, "llm");
        assert_eq!(DecisionMakerType::human().name, "human");
        assert_eq!(DecisionMakerType::mock().name, "mock");
        assert_eq!(DecisionMakerType::tiered().name, "tiered");
    }

    #[test]
    fn test_maker_type_serde() {
        let mt = DecisionMakerType::with_subtype("llm", "claude");
        let json = serde_json::to_string(&mt).unwrap();
        let parsed: DecisionMakerType = serde_json::from_str(&json).unwrap();
        assert_eq!(mt, parsed);
    }

    #[test]
    fn test_decision_registries_new() {
        let registries = DecisionRegistries::new();
        assert!(registries.actions().registered_types().is_empty());
        assert!(registries.situations().registered_types().is_empty());
    }

    #[test]
    fn test_maker_meta_new() {
        let meta = DecisionMakerMeta::new(DecisionMakerType::llm());
        assert_eq!(meta.maker_type.name, "llm");
        assert!(meta.supported_situations.is_empty());
        assert_eq!(meta.priority, 100);
        assert!(!meta.handles_critical);
    }

    #[test]
    fn test_maker_meta_with_situations() {
        let meta = DecisionMakerMeta::new(DecisionMakerType::rule_based())
            .with_situations(vec!["waiting_for_choice".to_string()]);
        assert!(meta.supports_situation("waiting_for_choice"));
        assert!(!meta.supports_situation("error"));
    }

    #[test]
    fn test_maker_meta_empty_situations_means_all() {
        let meta = DecisionMakerMeta::new(DecisionMakerType::llm());
        // Empty supported_situations means all situations are supported
        assert!(meta.supports_situation("anything"));
    }

    #[test]
    fn test_maker_meta_handles_critical() {
        let meta = DecisionMakerMeta::new(DecisionMakerType::human()).handles_critical();
        assert!(meta.handles_critical);
    }

    #[test]
    fn test_maker_meta_serde() {
        let meta = DecisionMakerMeta::new(DecisionMakerType::llm())
            .with_priority(50)
            .handles_critical();
        let json = serde_json::to_string(&meta).unwrap();
        let parsed: DecisionMakerMeta = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.maker_type, parsed.maker_type);
        assert_eq!(meta.priority, parsed.priority);
    }
}
