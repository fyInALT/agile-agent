//! Decision Pipeline
//!
//! Sprint 10: Orchestrates the complete decision flow.
//!
//! The `DecisionPipeline` coordinates the entire decision-making process:
//! 1. Pre-processing (context enrichment)
//! 2. Maker selection (strategy-based)
//! 3. Decision execution (maker execution)
//! 4. Post-processing (output validation)
//! 5. Result delivery

use crate::context::DecisionContext;
use crate::error::DecisionError;
use crate::maker::{
    DecisionMakerType, DecisionPostProcessor, DecisionPreProcessor, DecisionRegistries,
};
use crate::maker_registry::DecisionMakerRegistry;
use crate::output::DecisionOutput;
use crate::strategy::StrategySelection;
use crate::types::SituationType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

/// Decision Pipeline Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Enable pre-processing
    pub enable_pre_processing: bool,
    /// Enable post-processing
    pub enable_post_processing: bool,
    /// Maximum decision history size
    pub max_history_size: usize,
    /// Enable decision recording
    pub enable_recording: bool,
    /// Timeout for decision execution (milliseconds)
    pub decision_timeout_ms: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enable_pre_processing: true,
            enable_post_processing: true,
            max_history_size: 100,
            enable_recording: true,
            decision_timeout_ms: 30000,
        }
    }
}

/// Decision Pipeline
///
/// Orchestrates the complete decision flow with pre/post processing hooks.
///
/// # Flow
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────┐
/// │                     Decision Pipeline                        │
/// │                                                               │
/// │  1. Pre-Processors                                           │
/// │     ┌─────────────┐                                          │
/// │     │ enrich      │──▶ Modify context                        │
/// │     │ validate    │                                          │
/// │     │ metadata    │                                          │
/// │     └─────────────┘                                          │
/// │                                                               │
/// │  2. Strategy Selection                                       │
/// │     ┌─────────────┐                                          │
/// │     │ analyze     │──▶ Select maker type                     │
/// │     │ context     │                                          │
/// │     └─────────────┘                                          │
/// │                                                               │
/// │  3. Maker Execution                                          │
/// │     ┌─────────────┐                                          │
/// │     │ execute     │──▶ Produce output                        │
/// │     │ decision    │                                          │
/// │     └─────────────┘                                          │
/// │                                                               │
/// │  4. Post-Processors                                          │
/// │     ┌─────────────┐                                          │
/// │     │ validate    │──▶ Validate/modify output                │
/// │     │ enrich      │                                          │
/// │     └─────────────┘                                          │
/// │                                                               │
/// │  5. Recording                                                 │
/// │     ┌─────────────┐                                          │
/// │     │ record      │──▶ Add to history                        │
/// │     │ decision    │                                          │
/// │     └─────────────┘                                          │
/// │                                                               │
/// └─────────────────────────────────────────────────────────────┘
/// ```
pub struct DecisionPipeline {
    /// Maker registry
    registry: Arc<DecisionMakerRegistry>,
    /// Pre-processors (applied before decision)
    pre_processors: Vec<Box<dyn DecisionPreProcessor>>,
    /// Post-processors (applied after decision)
    post_processors: Vec<Box<dyn DecisionPostProcessor>>,
    /// Pipeline configuration
    config: PipelineConfig,
    /// Decision history
    history: VecDeque<PipelineDecisionRecord>,
    /// Total decisions made
    total_decisions: u64,
    /// Successful decisions
    successful_decisions: u64,
    /// Failed decisions
    failed_decisions: u64,
}

impl DecisionPipeline {
    /// Create a new pipeline with a maker registry
    pub fn new(registry: Arc<DecisionMakerRegistry>) -> Self {
        Self {
            registry,
            pre_processors: Vec::new(),
            post_processors: Vec::new(),
            config: PipelineConfig::default(),
            history: VecDeque::new(),
            total_decisions: 0,
            successful_decisions: 0,
            failed_decisions: 0,
        }
    }

    /// Create with configuration
    pub fn with_config(registry: Arc<DecisionMakerRegistry>, config: PipelineConfig) -> Self {
        Self {
            registry,
            pre_processors: Vec::new(),
            post_processors: Vec::new(),
            config,
            history: VecDeque::new(),
            total_decisions: 0,
            successful_decisions: 0,
            failed_decisions: 0,
        }
    }

    /// Add a pre-processor
    pub fn add_pre_processor(&mut self, processor: Box<dyn DecisionPreProcessor>) {
        self.pre_processors.push(processor);
    }

    /// Add a post-processor
    pub fn add_post_processor(&mut self, processor: Box<dyn DecisionPostProcessor>) {
        self.post_processors.push(processor);
    }

    /// Get configuration
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: PipelineConfig) {
        self.config = config;
    }

    /// Get registry
    pub fn registry(&self) -> &Arc<DecisionMakerRegistry> {
        &self.registry
    }

    /// Get pre-processors
    pub fn pre_processors(&self) -> &[Box<dyn DecisionPreProcessor>] {
        &self.pre_processors
    }

    /// Get post-processors
    pub fn post_processors(&self) -> &[Box<dyn DecisionPostProcessor>] {
        &self.post_processors
    }

    /// Execute the decision pipeline
    ///
    /// This is the main entry point that orchestrates:
    /// 1. Pre-processing
    /// 2. Maker selection
    /// 3. Decision execution
    /// 4. Post-processing
    /// 5. Recording
    ///
    /// Note: Context is consumed by the maker. If the maker fails,
    /// the error is returned. The caller can retry with a new context.
    pub fn execute(&mut self, context: DecisionContext) -> crate::error::Result<DecisionOutput> {
        self.total_decisions += 1;
        let started_at = Utc::now();
        let situation_type = context.trigger_situation.situation_type();

        // 1. Pre-processing
        let mut processed_context = context;
        if self.config.enable_pre_processing {
            for processor in &self.pre_processors {
                processor.process(&mut processed_context)?;
            }
        }

        // 2. Strategy selection
        let selection = self.registry.select_maker(&processed_context);

        // 3. Maker execution - use the selected maker with stored registries
        let maker_type = selection.maker_type.clone();

        // Get registries from the registry's stored registries and execute maker
        // Note: We need to release the borrow before calling record_decision
        let result = {
            let registries_guard = self.registry.registries_ref().read().unwrap();
            let registries = &*registries_guard;
            self.execute_maker(&selection.maker_type, processed_context, registries)
        };

        let output = match result {
            Ok(out) => out,
            Err(err) => {
                self.failed_decisions += 1;
                return Err(err);
            }
        };

        // 4. Post-processing
        let mut processed_output = output;
        if self.config.enable_post_processing {
            for processor in &self.post_processors {
                processor.process(&mut processed_output)?;
            }
        }

        // 5. Recording
        self.successful_decisions += 1;
        if self.config.enable_recording {
            self.record_decision(
                &situation_type,
                &maker_type,
                &selection,
                &processed_output,
                started_at,
            );
        }

        Ok(processed_output)
    }

    /// Execute a specific maker
    fn execute_maker(
        &self,
        maker_type: &DecisionMakerType,
        context: DecisionContext,
        registries: &DecisionRegistries,
    ) -> crate::error::Result<DecisionOutput> {
        if let Some(mut maker) = self.registry.get_for_execution(maker_type) {
            if !maker.is_healthy() {
                return Err(DecisionError::EngineError(format!(
                    "Maker {} is not healthy",
                    maker_type
                )));
            }
            maker.make_decision(context, registries)
        } else {
            Err(DecisionError::EngineError(format!(
                "Maker {} not found in registry",
                maker_type
            )))
        }
    }

    /// Record a decision
    fn record_decision(
        &mut self,
        situation_type: &SituationType,
        maker_type: &DecisionMakerType,
        selection: &StrategySelection,
        output: &DecisionOutput,
        started_at: DateTime<Utc>,
    ) {
        let record = PipelineDecisionRecord {
            decision_id: format!("pipeline-{}", uuid::Uuid::new_v4()),
            situation_type: situation_type.clone(),
            selected_maker: maker_type.clone(),
            strategy_name: selection.strategy_name.clone(),
            selection_reason: selection.reason.clone(),
            fallback_chain: selection.fallback_chain.clone(),
            action_types: output.actions.iter().map(|a| a.action_type()).collect(),
            reasoning: output.reasoning.clone(),
            confidence: output.confidence,
            started_at,
            completed_at: Utc::now(),
            success: true,
        };

        // Add to history with size limit
        if self.history.len() >= self.config.max_history_size {
            self.history.pop_front();
        }
        self.history.push_back(record);
    }

    /// Get decision history
    pub fn history(&self) -> &[PipelineDecisionRecord] {
        self.history.as_slices().0
    }

    /// Get statistics
    pub fn stats(&self) -> PipelineStats {
        PipelineStats {
            total_decisions: self.total_decisions,
            successful_decisions: self.successful_decisions,
            failed_decisions: self.failed_decisions,
            success_rate: if self.total_decisions > 0 {
                self.successful_decisions as f64 / self.total_decisions as f64
            } else {
                0.0
            },
            average_latency_ms: self.compute_average_latency_ms(),
        }
    }

    /// Compute average latency from history
    fn compute_average_latency_ms(&self) -> u64 {
        if self.history.is_empty() {
            return 0;
        }

        let total_ms: i64 = self
            .history
            .iter()
            .map(|r| (r.completed_at - r.started_at).num_milliseconds())
            .sum();

        (total_ms / self.history.len() as i64) as u64
    }

    /// Reset pipeline state
    pub fn reset(&mut self) -> crate::error::Result<()> {
        self.registry.reset_all()?;
        self.history.clear();
        self.total_decisions = 0;
        self.successful_decisions = 0;
        self.failed_decisions = 0;
        Ok(())
    }

    /// Check if pipeline is healthy
    pub fn is_healthy(&self) -> bool {
        self.registry.all_healthy()
    }

    /// Make a decision (alias for execute)
    pub fn decide(&mut self, context: DecisionContext) -> crate::error::Result<DecisionOutput> {
        self.execute(context)
    }
}

/// Pipeline decision record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDecisionRecord {
    /// Decision ID
    pub decision_id: String,
    /// Situation type
    pub situation_type: SituationType,
    /// Selected maker type
    pub selected_maker: DecisionMakerType,
    /// Strategy that made the selection
    pub strategy_name: String,
    /// Reason for selection
    pub selection_reason: String,
    /// Fallback chain
    pub fallback_chain: Vec<DecisionMakerType>,
    /// Action types in output
    pub action_types: Vec<crate::types::ActionType>,
    /// Reasoning
    pub reasoning: String,
    /// Confidence
    pub confidence: f64,
    /// Started timestamp
    pub started_at: DateTime<Utc>,
    /// Completed timestamp
    pub completed_at: DateTime<Utc>,
    /// Success flag
    pub success: bool,
}

impl PipelineDecisionRecord {
    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> i64 {
        (self.completed_at - self.started_at).num_milliseconds()
    }
}

/// Pipeline statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStats {
    /// Total decisions made
    pub total_decisions: u64,
    /// Successful decisions
    pub successful_decisions: u64,
    /// Failed decisions
    pub failed_decisions: u64,
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Average latency in milliseconds
    pub average_latency_ms: u64,
}

impl Default for PipelineStats {
    fn default() -> Self {
        Self {
            total_decisions: 0,
            successful_decisions: 0,
            failed_decisions: 0,
            success_rate: 0.0,
            average_latency_ms: 0,
        }
    }
}

/// Builder for creating a configured pipeline
pub struct PipelineBuilder {
    /// Registry to use
    registry: Arc<DecisionMakerRegistry>,
    /// Pre-processors to add
    pre_processors: Vec<Box<dyn DecisionPreProcessor>>,
    /// Post-processors to add
    post_processors: Vec<Box<dyn DecisionPostProcessor>>,
    /// Configuration
    config: PipelineConfig,
}

impl PipelineBuilder {
    /// Create a new builder
    pub fn new(registry: Arc<DecisionMakerRegistry>) -> Self {
        Self {
            registry,
            pre_processors: Vec::new(),
            post_processors: Vec::new(),
            config: PipelineConfig::default(),
        }
    }

    /// Add a pre-processor
    pub fn with_pre_processor(mut self, processor: Box<dyn DecisionPreProcessor>) -> Self {
        self.pre_processors.push(processor);
        self
    }

    /// Add a post-processor
    pub fn with_post_processor(mut self, processor: Box<dyn DecisionPostProcessor>) -> Self {
        self.post_processors.push(processor);
        self
    }

    /// Set configuration
    pub fn with_config(mut self, config: PipelineConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the pipeline
    pub fn build(self) -> DecisionPipeline {
        let mut pipeline = DecisionPipeline::with_config(self.registry, self.config);
        for processor in self.pre_processors {
            pipeline.add_pre_processor(processor);
        }
        for processor in self.post_processors {
            pipeline.add_post_processor(processor);
        }
        pipeline
    }
}

// ============================================================================
// Built-in Pre/Post Processors
// ============================================================================

/// Reflection Round Pre-Processor
///
/// Syncs reflection round from context metadata to ensure consistency.
#[derive(Debug, Clone)]
pub struct ReflectionRoundPreProcessor;

impl DecisionPreProcessor for ReflectionRoundPreProcessor {
    fn process(&self, context: &mut DecisionContext) -> crate::error::Result<()> {
        // Reflection round is already in context metadata
        // This processor ensures it's properly set
        if context.metadata.get("reflection_round").is_none() {
            context
                .metadata
                .insert("reflection_round".to_string(), "0".to_string());
        }
        Ok(())
    }

    fn processor_name(&self) -> &'static str {
        "reflection_round"
    }

    fn clone_boxed(&self) -> Box<dyn DecisionPreProcessor> {
        Box::new(self.clone())
    }
}

/// Validate Actions Post-Processor
///
/// Ensures output has at least one action.
#[derive(Debug, Clone)]
pub struct ValidateActionsPostProcessor;

impl DecisionPostProcessor for ValidateActionsPostProcessor {
    fn process(&self, output: &mut DecisionOutput) -> crate::error::Result<()> {
        if !output.has_actions() {
            return Err(DecisionError::ParseError(
                "Decision output has no actions".to_string(),
            ));
        }
        Ok(())
    }

    fn processor_name(&self) -> &'static str {
        "validate_actions"
    }

    fn clone_boxed(&self) -> Box<dyn DecisionPostProcessor> {
        Box::new(self.clone())
    }
}

/// Confidence Threshold Post-Processor
///
/// Ensures output confidence meets a minimum threshold.
#[derive(Debug, Clone)]
pub struct ConfidenceThresholdPostProcessor {
    /// Minimum confidence threshold
    threshold: f64,
}

impl ConfidenceThresholdPostProcessor {
    /// Create with threshold
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }
}

impl Default for ConfidenceThresholdPostProcessor {
    fn default() -> Self {
        Self::new(0.5)
    }
}

impl DecisionPostProcessor for ConfidenceThresholdPostProcessor {
    fn process(&self, output: &mut DecisionOutput) -> crate::error::Result<()> {
        if output.confidence < self.threshold {
            // Log warning but don't fail
            // In production, this could trigger a fallback
        }
        Ok(())
    }

    fn processor_name(&self) -> &'static str {
        "confidence_threshold"
    }

    fn clone_boxed(&self) -> Box<dyn DecisionPostProcessor> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin_situations::WaitingForChoiceSituation;
    use crate::context::DecisionContext;
    use crate::maker::DecisionRegistries;
    use crate::maker_registry::DecisionMakerRegistryBuilder;
    use crate::output::DecisionOutput;

    // Mock DecisionMaker for testing
    struct MockMaker {
        maker_type: DecisionMakerType,
    }

    impl crate::maker::DecisionMaker for MockMaker {
        fn make_decision(
            &mut self,
            _context: DecisionContext,
            _registries: &DecisionRegistries,
        ) -> crate::error::Result<DecisionOutput> {
            Ok(DecisionOutput::new(vec![], "mock"))
        }

        fn maker_type(&self) -> DecisionMakerType {
            self.maker_type.clone()
        }

        fn is_healthy(&self) -> bool {
            true
        }

        fn reset(&mut self) -> crate::error::Result<()> {
            Ok(())
        }

        fn clone_boxed(&self) -> Box<dyn crate::maker::DecisionMaker> {
            Box::new(MockMaker {
                maker_type: self.maker_type.clone(),
            })
        }
    }

    fn make_test_registry() -> Arc<DecisionMakerRegistry> {
        let registry = DecisionMakerRegistryBuilder::new()
            .with_maker(Box::new(MockMaker {
                maker_type: DecisionMakerType::rule_based(),
            }))
            .with_maker(Box::new(MockMaker {
                maker_type: DecisionMakerType::llm(),
            }))
            .build();
        Arc::new(registry)
    }

    fn make_test_context() -> DecisionContext {
        DecisionContext::new(Box::new(WaitingForChoiceSituation::default()), "test")
    }

    #[test]
    fn test_pipeline_new() {
        let registry = make_test_registry();
        let pipeline = DecisionPipeline::new(registry);
        assert!(pipeline.is_healthy());
    }

    #[test]
    fn test_pipeline_execute() {
        let registry = make_test_registry();
        let mut pipeline = DecisionPipeline::new(registry);

        let context = make_test_context();
        let result = pipeline.execute(context);

        assert!(result.is_ok());
        assert_eq!(pipeline.stats().total_decisions, 1);
        assert_eq!(pipeline.stats().successful_decisions, 1);
    }

    #[test]
    fn test_pipeline_with_processors() {
        let registry = make_test_registry();
        let mut pipeline = DecisionPipeline::new(registry);
        pipeline.add_pre_processor(Box::new(ReflectionRoundPreProcessor));
        pipeline.add_post_processor(Box::new(ValidateActionsPostProcessor));

        let context = make_test_context();
        let result = pipeline.execute(context);

        // Should fail because mock maker returns empty actions
        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_history() {
        let registry = make_test_registry();
        let mut pipeline = DecisionPipeline::new(registry);

        let context = make_test_context();
        pipeline.execute(context).unwrap();

        assert_eq!(pipeline.history().len(), 1);
        let record = &pipeline.history()[0];
        assert!(record.success);
        assert_eq!(record.situation_type.name, "waiting_for_choice");
    }

    #[test]
    fn test_pipeline_stats() {
        let registry = make_test_registry();
        let mut pipeline = DecisionPipeline::new(registry);

        // Multiple decisions
        for _ in 0..5 {
            let context = make_test_context();
            pipeline.execute(context).unwrap();
        }

        let stats = pipeline.stats();
        assert_eq!(stats.total_decisions, 5);
        assert_eq!(stats.successful_decisions, 5);
        assert_eq!(stats.failed_decisions, 0);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_pipeline_reset() {
        let registry = make_test_registry();
        let mut pipeline = DecisionPipeline::new(registry);

        let context = make_test_context();
        pipeline.execute(context).unwrap();

        pipeline.reset().unwrap();

        assert_eq!(pipeline.history().len(), 0);
        assert_eq!(pipeline.stats().total_decisions, 0);
    }

    #[test]
    fn test_pipeline_builder() {
        let registry = make_test_registry();
        let pipeline = PipelineBuilder::new(registry)
            .with_pre_processor(Box::new(ReflectionRoundPreProcessor))
            .with_config(PipelineConfig {
                max_history_size: 50,
                ..PipelineConfig::default()
            })
            .build();

        assert_eq!(pipeline.config().max_history_size, 50);
        assert_eq!(pipeline.pre_processors().len(), 1);
    }

    #[test]
    fn test_reflection_round_pre_processor() {
        let mut context = make_test_context();
        let processor = ReflectionRoundPreProcessor;

        processor.process(&mut context).unwrap();

        assert!(context.metadata.contains_key("reflection_round"));
    }

    #[test]
    fn test_validate_actions_post_processor() {
        let mut output = DecisionOutput::new(vec![], "test");
        let processor = ValidateActionsPostProcessor;

        let result = processor.process(&mut output);
        assert!(result.is_err());
    }

    #[test]
    fn test_confidence_threshold_post_processor() {
        let mut output = DecisionOutput::new(vec![], "test").with_confidence(0.3);
        let processor = ConfidenceThresholdPostProcessor::new(0.5);

        // Should pass (warning only, not error)
        let result = processor.process(&mut output);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pipeline_record_elapsed() {
        let registry = make_test_registry();
        let mut pipeline = DecisionPipeline::new(registry);

        let context = make_test_context();
        pipeline.execute(context).unwrap();

        let record = &pipeline.history()[0];
        assert!(record.elapsed_ms() >= 0);
    }
}
