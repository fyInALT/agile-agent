# Decision Maker Abstraction

This document describes the new `DecisionMaker` abstraction introduced in Sprint 10,
which provides a flexible and extensible decision-making framework.

## Overview

The decision layer now uses a three-layer architecture:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Decision Layer Architecture                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Layer 1: DecisionPipeline                                      │
│  ─────────────────────────────────────────────                  │
│  - Orchestrates the complete decision flow                      │
│  - Pre-processing (context enrichment)                          │
│  - Post-processing (output validation)                          │
│  - Decision recording and history                               │
│                                                                  │
│  Layer 2: DecisionMakerRegistry + DecisionStrategy              │
│  ─────────────────────────────────────────────                  │
│  - Registry manages decision makers                             │
│  - Strategy selects appropriate maker                            │
│  - Metadata for maker capabilities                              │
│                                                                  │
│  Layer 3: DecisionMaker implementations                         │
│  ─────────────────────────────────────────────                  │
│  - RuleBasedMaker (simple situations)                           │
│  - LLMMaker (medium/complex situations)                         │
│  - HumanMaker (critical situations)                             │
│  - Custom makers (extensible)                                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Core Traits

### DecisionMaker

The core abstraction for executing decisions:

```rust
pub trait DecisionMaker: Send + Sync {
    /// Execute decision logic
    fn make_decision(
        &mut self,
        context: DecisionContext,
        registries: &DecisionRegistries,
    ) -> Result<DecisionOutput>;

    /// Get the maker type identifier
    fn maker_type(&self) -> DecisionMakerType;

    /// Check if the maker is healthy
    fn is_healthy(&self) -> bool;

    /// Reset internal state
    fn reset(&mut self) -> Result<()>;

    /// Clone into boxed trait object
    fn clone_boxed(&self) -> Box<dyn DecisionMaker>;
}
```

### DecisionStrategy

Determines which maker to use for a given situation:

```rust
pub trait DecisionStrategy: Send + Sync {
    /// Select the appropriate maker type
    fn select_maker(
        &self,
        context: &DecisionContext,
        available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType;

    /// Get strategy name
    fn strategy_name(&self) -> &'static str;

    /// Get fallback maker type
    fn fallback(&self) -> Option<DecisionMakerType>;

    /// Get priority for strategy chaining
    fn priority(&self) -> u8;

    /// Clone boxed
    fn clone_boxed(&self) -> Box<dyn DecisionStrategy>;
}
```

### DecisionPreProcessor / DecisionPostProcessor

Hooks for customizing the decision flow:

```rust
pub trait DecisionPreProcessor: Send + Sync {
    fn process(&self, context: &mut DecisionContext) -> Result<()>;
    fn processor_name(&self) -> &'static str;
    fn clone_boxed(&self) -> Box<dyn DecisionPreProcessor>;
}

pub trait DecisionPostProcessor: Send + Sync {
    fn process(&self, output: &mut DecisionOutput) -> Result<()>;
    fn processor_name(&self) -> &'static str;
    fn clone_boxed(&self) -> Box<dyn DecisionPostProcessor>;
}
```

## Built-in Implementations

### Decision Makers

| Maker Type | Purpose | Use Cases |
|------------|---------|-----------|
| `RuleBasedMaker` | Rule-based decisions | Simple situations with known patterns |
| `LLMMaker` | LLM-based decisions | Medium/complex situations requiring reasoning |
| `HumanMaker` | Human intervention | Critical situations requiring approval |
| `MockMaker` | Testing | Unit tests and development |

### Decision Strategies

| Strategy | Purpose | Selection Logic |
|----------|---------|-----------------|
| `TieredStrategy` | Complexity-based selection | Simple → Rule, Medium/Complex → LLM, Critical → Human |
| `SituationMappingStrategy` | Direct situation mapping | Predefined mapping of situations to makers |
| `AdaptiveStrategy` | Performance-based adaptation | Adjusts based on success rates |
| `CompositeStrategy` | Strategy combination | Chains multiple strategies by priority |

### Pre/Post Processors

| Processor | Purpose |
|-----------|---------|
| `ReflectionRoundPreProcessor` | Syncs reflection round metadata |
| `ValidateActionsPostProcessor` | Ensures output has actions |
| `ConfidenceThresholdPostProcessor` | Validates confidence threshold |

## Usage Examples

### Basic Setup

```rust
use agent_decision::maker_registry::DecisionMakerRegistryBuilder;
use agent_decision::maker::{DecisionMakerType, DecisionMakerMeta};
use agent_decision::strategy::TieredStrategy;
use agent_decision::pipeline::{DecisionPipeline, PipelineBuilder};

// 1. Create makers (implement DecisionMaker trait)
let rule_maker = Box::new(RuleBasedMaker::new());
let llm_maker = Box::new(LLMMaker::new(provider));

// 2. Build registry
let registry = DecisionMakerRegistryBuilder::new()
    .with_maker(rule_maker)
    .with_maker(llm_maker)
    .with_strategy(Box::new(TieredStrategy::new()))
    .build();

// 3. Create pipeline
let pipeline = PipelineBuilder::new(Arc::new(registry))
    .with_pre_processor(Box::new(ReflectionRoundPreProcessor))
    .build();

// 4. Make decisions
let context = DecisionContext::new(situation, "agent-1");
let output = pipeline.execute(context)?;
```

### Custom DecisionMaker

```rust
struct CustomDecisionMaker {
    config: MyConfig,
}

impl DecisionMaker for CustomDecisionMaker {
    fn make_decision(
        &mut self,
        context: DecisionContext,
        registries: &DecisionRegistries,
    ) -> Result<DecisionOutput> {
        // Custom decision logic
        let action = self.select_action(&context);
        Ok(DecisionOutput::new(vec![action], "custom reasoning"))
    }

    fn maker_type(&self) -> DecisionMakerType {
        DecisionMakerType::custom("my_custom_maker")
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn reset(&mut self) -> Result<()> {
        Ok(())
    }

    fn clone_boxed(&self) -> Box<dyn DecisionMaker> {
        Box::new(self.clone())
    }
}
```

### Custom Strategy

```rust
struct CustomStrategy {
    mappings: HashMap<String, DecisionMakerType>,
}

impl DecisionStrategy for CustomStrategy {
    fn select_maker(
        &self,
        context: &DecisionContext,
        available_makers: &[DecisionMakerMeta],
    ) -> DecisionMakerType {
        let situation = context.trigger_situation.situation_type().name;
        
        // Custom selection logic
        self.mappings.get(&situation)
            .cloned()
            .unwrap_or(DecisionMakerType::llm())
    }

    fn strategy_name(&self) -> &'static str {
        "custom"
    }

    fn fallback(&self) -> Option<DecisionMakerType> {
        Some(DecisionMakerType::rule_based())
    }

    fn priority(&self) -> u8 {
        100
    }

    fn clone_boxed(&self) -> Box<dyn DecisionStrategy> {
        Box::new(self.clone())
    }
}
```

## Migration Guide

### From TieredDecisionEngine

The existing `TieredDecisionEngine` can be wrapped as a DecisionMaker:

```rust
struct TieredMakerAdapter {
    engine: TieredDecisionEngine,
}

impl DecisionMaker for TieredMakerAdapter {
    fn make_decision(
        &mut self,
        context: DecisionContext,
        registries: &DecisionRegistries,
    ) -> Result<DecisionOutput> {
        // Convert registries to ActionRegistry
        let action_registry = registries.actions();
        self.engine.decide(context, action_registry)
    }

    fn maker_type(&self) -> DecisionMakerType {
        DecisionMakerType::tiered()
    }

    fn is_healthy(&self) -> bool {
        self.engine.is_healthy()
    }

    fn reset(&mut self) -> Result<()> {
        self.engine.reset()
    }

    fn clone_boxed(&self) -> Box<dyn DecisionMaker> {
        Box::new(self.clone())
    }
}
```

### From DecisionEngine trait

The new `DecisionMaker` trait is similar to `DecisionEngine` but with:

1. **Registries bundle**: Instead of passing just `ActionRegistry`, we pass a `DecisionRegistries` bundle containing both actions and situations.

2. **Strategy-based selection**: Instead of hardcoded tier selection, makers are selected by configurable strategies.

3. **Pre/Post processing**: Pipeline supports hooks for context enrichment and output validation.

4. **Better extensibility**: New makers and strategies can be added without modifying existing code.

## Configuration

### Pipeline Configuration

```rust
PipelineConfig {
    enable_pre_processing: true,
    enable_post_processing: true,
    max_history_size: 100,
    enable_recording: true,
    decision_timeout_ms: 30000,
}
```

### Maker Metadata

```rust
DecisionMakerMeta {
    maker_type: DecisionMakerType::llm(),
    supported_situations: vec!["claims_completion", "error"],
    priority: 90,
    handles_critical: false,
    estimated_latency_ms: 500,
}
```

## Related Files

| File | Purpose |
|------|---------|
| `maker.rs` | DecisionMaker trait and types |
| `maker_registry.rs` | Registry and builder |
| `strategy.rs` | DecisionStrategy implementations |
| `pipeline.rs` | DecisionPipeline orchestration |

## Future Extensions

1. **Learning Strategy**: Machine learning-based maker selection
2. **Parallel Execution**: Try multiple makers concurrently
3. **Streaming Output**: Support streaming decision output
4. **Metrics Collection**: Detailed performance metrics
5. **Configuration Files**: YAML/JSON configuration support
