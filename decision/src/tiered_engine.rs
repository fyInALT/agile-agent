//! Tiered decision engine
//!
//! Sprint 3.6: Complexity-based engine selection.
//! Uses RuleBased for simple situations, LLM for complex, CLI for critical.

use crate::action::DecisionAction;
use crate::action_registry::ActionRegistry;
use crate::cli_engine::CLIDecisionEngine;
use crate::context::DecisionContext;
use crate::engine::DecisionEngine;
use crate::llm_caller::LLMCaller;
use crate::llm_engine::{LLMDecisionEngine, LLMEngineConfig};
use crate::mock_engine::MockDecisionEngine;
use crate::output::DecisionOutput;
use crate::provider_kind::ProviderKind;
use crate::rule_engine::RuleBasedDecisionEngine;
use crate::situation::DecisionSituation;
use crate::types::{DecisionEngineType, SituationType};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Tier level for engine selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionTier {
    /// Tier 1: Simple - use RuleBased engine
    Simple,

    /// Tier 2: Medium - use LLM engine
    Medium,

    /// Tier 3: Complex - use LLM engine with more context
    Complex,

    /// Tier 4: Critical - use CLI engine (human decision)
    Critical,
}

impl Default for DecisionTier {
    fn default() -> Self {
        DecisionTier::Simple
    }
}

impl DecisionTier {
    /// Determine tier from situation complexity
    pub fn from_situation(situation: &dyn DecisionSituation) -> Self {
        let type_name = situation.situation_type().name;

        // Critical: human intervention required (highest priority)
        if situation.requires_human() {
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

/// Tiered engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredEngineConfig {
    /// Provider for LLM engine
    pub llm_provider: ProviderKind,

    /// LLM engine config
    pub llm_config: LLMEngineConfig,

    /// Use CLI for critical tier
    pub use_cli_for_critical: bool,

    /// Fallback tier when engine fails
    pub fallback_tier: DecisionTier,
}

impl Default for TieredEngineConfig {
    fn default() -> Self {
        Self {
            llm_provider: ProviderKind::Claude,
            llm_config: LLMEngineConfig::default(),
            use_cli_for_critical: true,
            fallback_tier: DecisionTier::Medium,
        }
    }
}

/// Tiered decision engine - complexity-based engine selection
pub struct TieredDecisionEngine {
    /// Configuration
    config: TieredEngineConfig,

    /// Rule-based engine (Tier 1)
    rule_engine: RuleBasedDecisionEngine,

    /// LLM engine (Tier 2-3)
    llm_engine: LLMDecisionEngine,

    /// CLI engine (Tier 4)
    cli_engine: Option<CLIDecisionEngine>,

    /// Mock engine for testing (reserved for future use)
    #[allow(dead_code)]
    mock_engine: MockDecisionEngine,

    /// Decision history
    history: Vec<TieredDecisionRecord>,

    /// Healthy flag
    healthy: bool,
}

/// Record of tiered decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredDecisionRecord {
    /// Decision ID
    pub id: String,

    /// Situation type
    pub situation_type: SituationType,

    /// Selected tier
    pub tier: DecisionTier,

    /// Engine used
    pub engine_type: DecisionEngineType,

    /// Action selected
    pub action_type: String,

    /// Confidence
    pub confidence: f64,

    /// Timestamp
    pub timestamp: String,
}

impl TieredDecisionEngine {
    /// Create new tiered decision engine
    pub fn new(config: TieredEngineConfig) -> Self {
        let llm_provider = config.llm_provider;
        let llm_config = config.llm_config.clone();
        let use_cli_for_critical = config.use_cli_for_critical;

        Self {
            config,
            rule_engine: RuleBasedDecisionEngine::new(),
            llm_engine: LLMDecisionEngine::with_config(llm_provider, llm_config),
            cli_engine: if use_cli_for_critical {
                Some(CLIDecisionEngine::new(llm_provider))
            } else {
                None
            },
            mock_engine: MockDecisionEngine::new(),
            history: Vec::new(),
            healthy: true,
        }
    }

    /// Create with default configuration
    pub fn with_provider(provider: ProviderKind) -> Self {
        let config = TieredEngineConfig {
            llm_provider: provider,
            ..TieredEngineConfig::default()
        };
        Self::new(config)
    }

    /// Set custom LLM caller for the LLM engine
    ///
    /// This allows injecting a real provider caller instead of using the mock.
    pub fn set_llm_caller(&mut self, caller: Arc<dyn LLMCaller>) {
        self.llm_engine.set_llm_caller(caller);
    }

    /// Select tier for situation
    fn select_tier(&self, situation: &dyn DecisionSituation) -> DecisionTier {
        DecisionTier::from_situation(situation)
    }

    /// Select engine for tier
    fn select_engine(&mut self, tier: DecisionTier) -> &mut dyn DecisionEngine {
        match tier {
            DecisionTier::Simple => &mut self.rule_engine,
            DecisionTier::Medium => &mut self.llm_engine,
            DecisionTier::Complex => &mut self.llm_engine,
            DecisionTier::Critical => {
                if let Some(cli) = &mut self.cli_engine {
                    cli as &mut dyn DecisionEngine
                } else {
                    // Fallback to LLM if CLI not configured
                    &mut self.llm_engine
                }
            }
        }
    }

    /// Make decision with fallback (reserved for future use)
    #[allow(dead_code)]
    fn decide_with_fallback(
        &mut self,
        situation_type: SituationType,
        tier: DecisionTier,
        action_registry: &ActionRegistry,
        engine: &mut dyn DecisionEngine,
    ) -> crate::error::Result<DecisionOutput> {
        // Build a fresh context from situation type
        let registry = crate::situation_registry::SituationRegistry::new();
        crate::builtin_situations::register_situation_builtins(&registry);

        let situation = registry.build(situation_type.clone());
        let context = DecisionContext::new(situation, "tiered-agent");

        match engine.decide(context, action_registry) {
            Ok(output) => return Ok(output),
            Err(e) => {
                // Try fallback tier
                let fallback_tier = self.config.fallback_tier;
                if fallback_tier != tier {
                    let fallback_engine = self.select_engine(fallback_tier);

                    let situation2 = registry.build(situation_type.clone());
                    let context2 = DecisionContext::new(situation2, "tiered-agent");
                    return fallback_engine.decide(context2, action_registry);
                }
                return Err(e);
            }
        }
    }

    /// Record tiered decision
    fn record_decision(
        &mut self,
        situation_type: SituationType,
        tier: DecisionTier,
        engine_type: DecisionEngineType,
        output: &DecisionOutput,
    ) {
        let record = TieredDecisionRecord {
            id: format!("dec-{}", uuid::Uuid::new_v4()),
            situation_type,
            tier,
            engine_type,
            action_type: output
                .first_action_type()
                .map(|a| a.name)
                .unwrap_or_default(),
            confidence: output.confidence,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        self.history.push(record);
    }

    /// Get decision history
    pub fn history(&self) -> &[TieredDecisionRecord] {
        &self.history
    }

    /// Get statistics by tier
    pub fn tier_stats(&self) -> TierStatistics {
        let mut stats = TierStatistics::default();

        for record in &self.history {
            match record.tier {
                DecisionTier::Simple => stats.simple_count += 1,
                DecisionTier::Medium => stats.medium_count += 1,
                DecisionTier::Complex => stats.complex_count += 1,
                DecisionTier::Critical => stats.critical_count += 1,
            }
        }

        stats.total = self.history.len() as u64;
        stats
    }
}

/// Tier statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TierStatistics {
    /// Total decisions
    pub total: u64,

    /// Simple tier count
    pub simple_count: u64,

    /// Medium tier count
    pub medium_count: u64,

    /// Complex tier count
    pub complex_count: u64,

    /// Critical tier count
    pub critical_count: u64,
}

impl TierStatistics {
    /// Get percentage for tier
    pub fn percentage(&self, tier: DecisionTier) -> f64 {
        if self.total == 0 {
            return 0.0;
        }

        let count = match tier {
            DecisionTier::Simple => self.simple_count,
            DecisionTier::Medium => self.medium_count,
            DecisionTier::Complex => self.complex_count,
            DecisionTier::Critical => self.critical_count,
        };

        count as f64 / self.total as f64 * 100.0
    }
}

impl DecisionEngine for TieredDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        // Report as Custom since it uses multiple engines
        DecisionEngineType::Custom {
            name: "tiered".to_string(),
        }
    }

    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<DecisionOutput> {
        // 1. Determine tier from situation
        let tier = self.select_tier(context.trigger_situation.as_ref());
        let situation_type = context.trigger_situation.situation_type();

        // Bug fix: Get llm_provider before calling select_engine to avoid borrow issues
        let llm_provider = self.config.llm_provider;

        // 2. Get engine type for this tier
        let engine_type = match tier {
            DecisionTier::Simple => DecisionEngineType::RuleBased,
            DecisionTier::Medium => DecisionEngineType::LLM {
                provider: llm_provider,
            },
            DecisionTier::Complex => DecisionEngineType::LLM {
                provider: llm_provider,
            },
            DecisionTier::Critical => DecisionEngineType::CLI {
                provider: llm_provider,
            },
        };

        // 3. Select engine and try decision with fallback
        // Bug fix: Implement proper fallback mechanism
        // Note: We can't clone context, so we pass it directly to primary engine
        // If primary fails, we rebuild a minimal context for fallback
        let engine = self.select_engine(tier);
        let output_result = engine.decide(context, action_registry);

        let output = match output_result {
            Ok(out) => out,
            Err(e) => {
                // Try fallback tier if configured and different from current
                let fallback_tier = self.config.fallback_tier;
                if fallback_tier != tier {
                    // Rebuild minimal context for fallback (since context was consumed)
                    let registry = crate::situation_registry::SituationRegistry::new();
                    crate::builtin_situations::register_situation_builtins(&registry);
                    let fallback_situation = registry.build(situation_type.clone());
                    let fallback_context =
                        DecisionContext::new(fallback_situation, "tiered-fallback");

                    // Select fallback engine
                    let fallback_engine = self.select_engine(fallback_tier);
                    let fallback_engine_type = match fallback_tier {
                        DecisionTier::Simple => DecisionEngineType::RuleBased,
                        DecisionTier::Medium | DecisionTier::Complex => DecisionEngineType::LLM {
                            provider: llm_provider,
                        },
                        DecisionTier::Critical => DecisionEngineType::CLI {
                            provider: llm_provider,
                        },
                    };

                    // Try fallback
                    match fallback_engine.decide(fallback_context, action_registry) {
                        Ok(fallback_output) => {
                            // Record with fallback info
                            self.record_decision(
                                situation_type,
                                fallback_tier,
                                fallback_engine_type,
                                &fallback_output,
                            );
                            return Ok(fallback_output);
                        }
                        Err(fallback_err) => {
                            // Both failed - return original error with context
                            return Err(crate::error::DecisionError::EngineError(format!(
                                "Primary engine failed: {}. Fallback engine also failed: {}",
                                e, fallback_err
                            )));
                        }
                    }
                } else {
                    // No fallback available
                    return Err(e);
                }
            }
        };

        // 4. Record decision
        self.record_decision(situation_type, tier, engine_type, &output);

        Ok(output)
    }

    fn build_prompt(&self, context: &DecisionContext, action_registry: &ActionRegistry) -> String {
        // Use LLM engine for prompt building (most detailed)
        self.llm_engine.build_prompt(context, action_registry)
    }

    fn parse_response(
        &self,
        response: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> crate::error::Result<Vec<Box<dyn DecisionAction>>> {
        // Use rule engine parsing (most flexible)
        self.rule_engine
            .parse_response(response, situation, action_registry)
    }

    fn session_handle(&self) -> Option<&str> {
        self.llm_engine.session_handle()
    }

    fn is_healthy(&self) -> bool {
        self.healthy && self.rule_engine.is_healthy() && self.llm_engine.is_healthy()
    }

    fn reset(&mut self) -> crate::error::Result<()> {
        self.rule_engine.reset()?;
        self.llm_engine.reset()?;
        if let Some(cli) = &mut self.cli_engine {
            cli.reset()?;
        }
        self.history.clear();
        self.healthy = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_registry::ActionRegistry;
    use crate::builtin_actions::register_action_builtins;
    use crate::builtin_situations::{
        ClaimsCompletionSituation, ErrorSituation, WaitingForChoiceSituation,
    };
    use crate::context::DecisionContext;
    use crate::situation::{ChoiceOption, ErrorInfo};

    fn make_test_registry() -> ActionRegistry {
        let registry = ActionRegistry::new();
        register_action_builtins(&registry);
        registry
    }

    fn make_simple_choice_context() -> DecisionContext {
        let situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );
        DecisionContext::new(situation, "test-agent")
    }

    fn make_medium_choice_context() -> DecisionContext {
        let situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(ClaimsCompletionSituation::new("Test completion"));
        DecisionContext::new(situation, "test-agent")
    }

    fn make_error_context() -> DecisionContext {
        let situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(ErrorSituation::new(ErrorInfo::new("test", "Test error")));
        DecisionContext::new(situation, "test-agent")
    }

    #[test]
    fn test_decision_tier_from_simple() {
        let situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );
        let tier = DecisionTier::from_situation(situation.as_ref());
        assert_eq!(tier, DecisionTier::Simple);
    }

    #[test]
    fn test_decision_tier_from_medium() {
        // Claims completion is Medium tier
        let situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(ClaimsCompletionSituation::new("Test"));
        let tier = DecisionTier::from_situation(situation.as_ref());
        assert_eq!(tier, DecisionTier::Medium);
    }

    #[test]
    fn test_decision_tier_from_error() {
        let situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(ErrorSituation::new(ErrorInfo::new("test", "Test error")));
        let tier = DecisionTier::from_situation(situation.as_ref());
        assert_eq!(tier, DecisionTier::Complex);
    }

    #[test]
    fn test_decision_tier_number() {
        assert_eq!(DecisionTier::Simple.tier_number(), 1);
        assert_eq!(DecisionTier::Medium.tier_number(), 2);
        assert_eq!(DecisionTier::Complex.tier_number(), 3);
        assert_eq!(DecisionTier::Critical.tier_number(), 4);
    }

    #[test]
    fn test_decision_tier_requires_human() {
        assert!(!DecisionTier::Simple.requires_human());
        assert!(!DecisionTier::Medium.requires_human());
        assert!(!DecisionTier::Complex.requires_human());
        assert!(DecisionTier::Critical.requires_human());
    }

    #[test]
    fn test_tiered_engine_new() {
        let engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        assert!(matches!(
            engine.engine_type(),
            DecisionEngineType::Custom { .. }
        ));
    }

    #[test]
    fn test_tiered_engine_config_default() {
        let config = TieredEngineConfig::default();
        assert!(config.use_cli_for_critical);
        assert_eq!(config.llm_provider, ProviderKind::Claude);
    }

    #[test]
    fn test_tiered_select_tier() {
        let engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);

        // WaitingForChoice is Simple tier
        let simple_situation: Box<dyn crate::situation::DecisionSituation> = Box::new(
            WaitingForChoiceSituation::new(vec![ChoiceOption::new("A", "Option A")]),
        );
        assert_eq!(
            engine.select_tier(simple_situation.as_ref()),
            DecisionTier::Simple
        );

        // ClaimsCompletion is Medium tier
        let medium_situation: Box<dyn crate::situation::DecisionSituation> =
            Box::new(ClaimsCompletionSituation::new("Test"));
        assert_eq!(
            engine.select_tier(medium_situation.as_ref()),
            DecisionTier::Medium
        );
    }

    #[test]
    fn test_tiered_decide_simple() {
        let mut engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        let context = make_simple_choice_context();
        let registry = make_test_registry();

        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        assert_eq!(engine.history().len(), 1);
        assert_eq!(engine.history()[0].tier, DecisionTier::Simple);
    }

    #[test]
    fn test_tiered_decide_medium() {
        let mut engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        let context = make_medium_choice_context();
        let registry = make_test_registry();

        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        assert_eq!(engine.history()[0].tier, DecisionTier::Medium);
    }

    #[test]
    fn test_tiered_decide_complex() {
        let mut engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        let context = make_error_context();
        let registry = make_test_registry();

        let output = engine.decide(context, &registry).unwrap();

        assert!(output.has_actions());
        assert_eq!(engine.history()[0].tier, DecisionTier::Complex);
    }

    #[test]
    fn test_tiered_history() {
        let mut engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        let registry = make_test_registry();

        engine
            .decide(make_simple_choice_context(), &registry)
            .unwrap();
        engine
            .decide(make_medium_choice_context(), &registry)
            .unwrap();
        engine.decide(make_error_context(), &registry).unwrap();

        assert_eq!(engine.history().len(), 3);
    }

    #[test]
    fn test_tiered_stats() {
        let mut engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        let registry = make_test_registry();

        engine
            .decide(make_simple_choice_context(), &registry)
            .unwrap();
        engine
            .decide(make_simple_choice_context(), &registry)
            .unwrap();
        engine
            .decide(make_medium_choice_context(), &registry)
            .unwrap();

        let stats = engine.tier_stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.simple_count, 2);
        assert_eq!(stats.medium_count, 1);
    }

    #[test]
    fn test_tier_stats_percentage() {
        let stats = TierStatistics {
            total: 10,
            simple_count: 4,
            medium_count: 3,
            complex_count: 2,
            critical_count: 1,
        };

        assert_eq!(stats.percentage(DecisionTier::Simple), 40.0);
        assert_eq!(stats.percentage(DecisionTier::Medium), 30.0);
    }

    #[test]
    fn test_tiered_reset() {
        let mut engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        let registry = make_test_registry();

        engine
            .decide(make_simple_choice_context(), &registry)
            .unwrap();
        engine.reset().unwrap();

        assert!(engine.history().is_empty());
        assert!(engine.is_healthy());
    }

    #[test]
    fn test_tiered_engine_healthy() {
        let engine = TieredDecisionEngine::with_provider(ProviderKind::Claude);
        assert!(engine.is_healthy());
    }

    #[test]
    fn test_tiered_engine_config_serde() {
        let config = TieredEngineConfig {
            llm_provider: ProviderKind::Claude,
            llm_config: LLMEngineConfig {
                timeout_seconds: 60,
                max_retries: 3,
                temperature: 0.5,
                max_tokens: 1000,
                persist_session: false,
            },
            use_cli_for_critical: true,
            fallback_tier: DecisionTier::Medium,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: TieredEngineConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.llm_provider, parsed.llm_provider);
        assert_eq!(config.use_cli_for_critical, parsed.use_cli_for_critical);
    }

    #[test]
    fn test_tiered_decision_record() {
        let record = TieredDecisionRecord {
            id: "dec-001".to_string(),
            situation_type: SituationType::new("waiting_for_choice"),
            tier: DecisionTier::Simple,
            engine_type: DecisionEngineType::RuleBased,
            action_type: "select_option".to_string(),
            confidence: 0.9,
            timestamp: "2026-04-15T10:00:00Z".to_string(),
        };

        assert_eq!(record.tier, DecisionTier::Simple);
        assert_eq!(record.confidence, 0.9);
    }
}
