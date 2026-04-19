# Decision Layer Architecture

## Overview

The decision layer has been refactored from a flat structure of 49 files (~26,000 lines) into a layered architecture with 9 logical sub-packages.

## Directory Structure

```
decision/src/
├── lib.rs                    # Root module, unified exports
│
├── core/                     # Layer 1: Core types
│   ├── mod.rs
│   ├── types.rs              # ActionType, SituationType, UrgencyLevel, DecisionEngineType
│   ├── error.rs              # DecisionError
│   ├── context.rs            # DecisionContext, RunningContextCache
│   └── output.rs             # DecisionOutput, DecisionRecord
│
├── model/                    # Layer 2: Business models
│   ├── mod.rs
│   ├── situation/            # DecisionSituation subsystem
│   │   ├── mod.rs
│   │   ├── situation.rs      # DecisionSituation trait
│   │   ├── situation_registry.rs
│   │   └── builtin_situations.rs
│   ├── action/               # DecisionAction subsystem
│   │   ├── mod.rs
│   │   ├── action.rs         # DecisionAction trait
│   │   ├── action_registry.rs
│   │   └── builtin_actions.rs
│   ├── task/                 # Task entity subsystem
│   │   ├── mod.rs
│   │   ├── task.rs           # Task entity, TaskStatus
│   │   ├── task_metrics.rs
│   │   ├── task_metadata.rs
│   │   ├── task_preparation.rs
│   │   └── task_completion.rs
│   └── workflow/             # Workflow subsystem
│   │   ├── mod.rs
│   │   └── workflow.rs       # DecisionProcess, DecisionStage, Condition
│
├── pipeline/                 # Layer 3: Main execution flow (entry point)
│   ├── mod.rs
│   ├── pipeline.rs           # DecisionPipeline (orchestrator)
│   ├── maker.rs              # DecisionMaker trait
│   ├── maker_registry.rs     # DecisionMakerRegistry
│   ├── strategy.rs           # DecisionStrategy, TieredStrategy
│   └── processor.rs          # Pre/Post processors (in pipeline.rs)
│
├── engine/                   # Layer 4: Decision implementations
│   ├── mod.rs
│   ├── engine.rs             # DecisionEngine trait
│   ├── rule_engine.rs        # RuleBasedDecisionEngine (Simple tier)
│   ├── llm_engine.rs         # LLMDecisionEngine (Medium/Complex tier)
│   ├── tiered_engine.rs      # TieredDecisionEngine (complexity-based)
│   ├── cli_engine.rs         # CLIDecisionEngine (Human tier)
│   ├── mock_engine.rs        # MockDecisionEngine (testing)
│   ├── task_engine.rs        # TaskDecisionEngine (task-specific)
│   └── llm_caller.rs         # LLMCaller trait
│
├── classifier/               # Layer 5: Output classification
│   ├── mod.rs
│   ├── classifier.rs         # OutputClassifier trait
│   ├── classifier_registry.rs
│   ├── acp_classifier.rs     # ACP protocol outputs
│   ├── claude_classifier.rs  # Claude outputs
│   └── codex_classifier.rs   # Codex outputs
│
├── provider/                 # Layer 6: LLM provider adaptation
│   ├── mod.rs
│   ├── provider_kind.rs      # ProviderKind (Claude, Codex, etc.)
│   ├── provider_event.rs     # ProviderEvent
│   └── initializer.rs        # Initializer
│
├── state/                    # Layer 7: State management
│   ├── mod.rs
│   ├── lifecycle.rs          # DecisionAgentState, DecisionAgentConfig
│   ├── blocking.rs           # BlockedState, HumanDecisionQueue
│   ├── recovery.rs           # Recovery mechanism
│   ├── git_state.rs          # GitState analysis
│   ├── uncommitted_handler.rs
│   └── commit_boundary.rs
│
├── runtime/                  # Layer 8: Runtime support
│   ├── mod.rs
│   ├── concurrent.rs         # SessionPool, RateLimiter, Arbitrator
│   ├── automation.rs         # AutoChecker, DecisionFilter
│   ├── persistence.rs        # TaskStore, TaskRegistry
│   └── metrics.rs            # DecisionMetrics
│
├── config/                   # Layer 9: Configuration
│   ├── mod.rs
│   ├── yaml_loader.rs        # YAML configuration loading
│   └── prompts/mod.rs        # Prompt templates
│
└── condition.rs              # Shared: Condition expressions
```

## Layer Dependencies

```
Layer 1: Core (types, error, context, output)
         ↓ foundational types
         
Layer 2: Model (situation, action, task, workflow)
         ↓ business models, depends on Core
         
Layer 3: Pipeline (pipeline, maker, strategy)
         ↓ execution flow, depends on Core + Model
         
Layer 4: Engine (rule, llm, tiered, cli, task)
         ↓ decision implementations, depends on Core + Model + Pipeline
         
Layer 5: Classifier (acp, claude, codex)
         ↓ output classification, depends on Core + Model
         
Layer 6: Provider (kind, event, initializer)
         ↓ LLM adaptation, depends on Core
         
Layer 7: State (lifecycle, blocking, recovery, git_state)
         ↓ state management, depends on Core + Model
         
Layer 8: Runtime (concurrent, automation, persistence, metrics)
         ↓ runtime support, depends on Core + Model + State
         
Layer 9: Config (yaml_loader, prompts)
         ↓ configuration, depends on Model
```

## Main Execution Flow

```
┌───────────────────────────────────────────────────────────────────┐
│                     Decision Pipeline                              │
│                                                                    │
│  Input: ProviderOutput (from external LLM/AI provider)            │
│                                                                    │
│  Step 1: Classifier Layer                                         │
│          - OutputClassifier identifies output type                 │
│          - Creates DecisionSituation                               │
│                                                                    │
│  Step 2: Context Building                                         │
│          - DecisionContext from situation + history + metadata     │
│          - RunningContextCache maintains execution history         │
│                                                                    │
│  Step 3: Pipeline.execute()                                       │
│          ┌─────────────────────────────────────────────────────┐  │
│          │ Pre-Processors                                       │  │
│          │ - enrich context (reflection_round, etc.)           │  │
│          └─────────────────────────────────────────────────────┘  │
│          ┌─────────────────────────────────────────────────────┐  │
│          │ Strategy Selection                                   │  │
│          │ - TieredStrategy determines tier:                   │  │
│          │   Simple → RuleBased (rule_engine)                  │  │
│          │   Medium → LLM (llm_engine)                         │  │
│          │   Complex → LLM (llm_engine)                        │  │
│          │   Critical → Human (cli_engine)                     │  │
│          └─────────────────────────────────────────────────────┘  │
│          ┌─────────────────────────────────────────────────────┐  │
│          │ Maker Execution                                      │  │
│          │ - DecisionMaker.make_decision(context, registries)  │  │
│          │ - Engine processes and returns DecisionOutput       │  │
│          └─────────────────────────────────────────────────────┘  │
│          ┌─────────────────────────────────────────────────────┐  │
│          │ Post-Processors                                      │  │
│          │ - validate output (has_actions, confidence)         │  │
│          └─────────────────────────────────────────────────────┘  │
│          ┌─────────────────────────────────────────────────────┐  │
│          │ Recording                                            │  │
│          │ - add to PipelineDecisionRecord history             │  │
│          └─────────────────────────────────────────────────────┘  │
│                                                                    │
│  Output: DecisionOutput                                           │
│          - actions: Vec<DecisionAction>                           │
│          - reasoning: String                                      │
│          - confidence: f64                                        │
└───────────────────────────────────────────────────────────────────┘
```

## Key Types

### Entry Points

- `DecisionPipeline` - Main orchestrator (pipeline layer)
- `TieredDecisionEngine` - Default engine (engine layer)
- `TaskDecisionEngine` - Task-specific decisions (engine layer)

### Core Types

- `DecisionContext` - Input to decision
- `DecisionOutput` - Output from decision
- `DecisionError` - Error handling

### Model Types

- `DecisionSituation` - Trigger condition trait
- `DecisionAction` - Execution action trait
- `Task` - Task entity with lifecycle
- `DecisionProcess` - Workflow stages

### Strategy Types

- `DecisionStrategy` - Maker selection trait
- `DecisionMaker` - Execution trait
- `DecisionTier` - Complexity level (Simple/Medium/Complex/Critical)

## Backward Compatibility

The refactored structure maintains backward compatibility through re-exports in `lib.rs`:

```rust
// Old usage still works:
use agent_decision::{DecisionPipeline, DecisionContext, DecisionOutput};
use agent_decision::{DecisionSituation, DecisionAction};
use agent_decision::{Task, TaskStatus};

// New layered usage also supported:
use agent_decision::pipeline::DecisionPipeline;
use agent_decision::model::situation::DecisionSituation;
use agent_decision::engine::tiered::TieredDecisionEngine;
```

## Statistics

- Total files: 59 (organized in 9 layers)
- Total tests: 770 (all passing)
- Total lines: ~26,000

## Benefits

1. **Clear separation of concerns**: Each layer has a distinct responsibility
2. **Better navigation**: Logical grouping makes it easier to find related code
3. **Reduced coupling**: Layers depend on lower layers, not sideways
4. **Maintainable**: Changes to one layer don't affect others
5. **Testable**: Each layer can be tested independently
6. **Backward compatible**: Existing code continues to work
