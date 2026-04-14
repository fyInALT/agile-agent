# Decision Layer Specification (Trait-Based Architecture)

## Overview

This directory contains the Scrum-style breakdown of the decision layer implementation. The decision layer enables autonomous development by monitoring provider outputs and making decisions to keep the development loop running without human intervention.

## Architecture Evolution

This specification adopts **Trait-based architecture** for maximum extensibility:

| Aspect | Traditional (Enum) | This Design (Trait + Registry) |
|--------|-------------------|----------------------------|
| Situation types | Fixed enum (4 types) | DecisionSituation trait (extensible) |
| Decision actions | Fixed enum (5 outputs) | DecisionAction trait (extensible) |
| Blocking reasons | Fixed enum (blocked types) | BlockingReason trait (extensible) |
| Provider events | Direct mapping to status | SituationType + Registry layer |

**Benefits**:
- Adding new situation: implement trait + register (no enum modification)
- Adding new action: implement trait + register (no output modification)
- Adding new blocking reason: implement trait (no AgentSlotStatus modification)
- Adding new provider: implement classifier + register (no core change)

See [Architecture Evolution Proposal](architecture-evolution.md) for detailed rationale.

## Problem Statement

Current agile-agent development loop has blocking points:

| Blocking Scenario | Description | Impact |
|-------------------|-------------|--------|
| Waiting for Choice | Provider returns multiple options | Development interrupted |
| Completion Claim | Provider claims task complete but may not be | Cannot auto-verify |
| Partial Completion | Provider completed part but has more work | Needs manual judgment |
| Error Recovery | Provider errors/gibberish/repetition | Cannot auto-retry |

**Core Problem**: These blocked states prevent truly autonomous development.

## Solution: Decision Layer

A sub-agent dedicated to monitoring main agent outputs and making decisions:

- Monitors provider (Claude/Codex/OpenCode/Kimi) outputs
- Classifies outputs into situation types via SituationRegistry
- Uses configurable decision engine (LLM/CLI/RuleBased/Mock/Tiered)
- Produces action sequences via ActionRegistry
- Persists decision session independently
- Supports human escalation via BlockingReason trait

## Architecture

```
Multi-Agent Runtime with Decision Layer:

┌─────────────────────────────────────────────────────────────────────┐
│  AgentPool                                                           │
│                                                                      │
│  ┌─────────────────────────────┐  ┌─────────────────────────────┐   │
│  │ Main Agent A                │  │ Main Agent B                │   │
│  │ Provider: Claude            │  │ Provider: Codex             │   │
│  │ Session: sess-main-a        │  │ Session: thr-main-b         │   │
│  │ Status: Running             │  │ Status: Blocked             │   │
│  │                             │  │ Reason: HumanDecision       │   │
│  │ ┌─────────────────────────┐ │  │                             │   │
│  │ │ Decision Agent A'       │ │  │ ┌─────────────────────────┐ │   │
│  │ │ Registry: Situation     │ │  │ │ Decision Agent B'       │ │   │
│  │ │ Registry: Action        │ │  │ │ Waiting for human       │ │   │
│  │ │ Engine: Tiered          │ │  │ └─────────────────────────┘ │   │
│  │ └─────────────────────────┘ │  └─────────────────────────────┘   │
│  └─────────────────────────────┘                                    │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Shared Registries                                                ││
│  │  - SituationRegistry (extensible situations)                    ││
│  │  - ActionRegistry (extensible actions)                          ││
│  │  - ClassifierRegistry (provider-specific classifiers)           ││
│  │  - ConditionEvaluatorRegistry (rule expression engine)          ││
│  │  - BlockingReasonRegistry (blocking types)                      ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Concurrent Processing                                            ││
│  │  - DecisionSessionPool (session reuse)                          ││
│  │  - DecisionRateLimiter (API overload prevention)                ││
│  │  - HumanDecisionArbitrator (multi-request handling)             ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

**Key Architecture Points**:

| Aspect | Description |
|--------|-------------|
| Exclusive Ownership | Each Main Agent has one exclusive Decision Agent |
| Trait-Based Types | Situation, Action, Blocking all use trait + registry |
| Provider Independence | Decision Agent provider can differ from Main Agent |
| Session Independence | Decision Agent has separate session |
| Concurrent Support | SessionPool, RateLimiter, Arbitrator for multi-agent |
| Extensibility | Add types by implementing trait + registering |

## Sprints

| Sprint | Name | Stories | Goal |
|--------|------|---------|------|
| [Sprint 1](sprint-01-core-types.md) | Core Types (Trait-Based) | 6 | DecisionSituation, DecisionAction, BlockingReason traits |
| [Sprint 2](sprint-02-output-classifier.md) | Output Classifier (Trait-Based) | 5 | Provider classifiers with SituationRegistry |
| [Sprint 3](sprint-03-decision-engine.md) | Decision Engine (Trait-Based) | 6 | Engines returning action sequences, rule expression engine |
| [Sprint 4](sprint-04-context-cache.md) | Context Cache | 3 | Running context caching with size limits |
| [Sprint 5](sprint-05-lifecycle.md) | Lifecycle | 4 | Decision Agent creation, destruction, task switching |
| [Sprint 6](sprint-06-human-intervention.md) | Human Intervention (Trait-Based) | 5 | HumanDecisionBlocking, priority queue, TUI/CLI |
| [Sprint 7](sprint-07-error-recovery.md) | Error Recovery | 4 | Retry logic, timeout handling, recovery levels |
| [Sprint 8](sprint-08-integration.md) | Integration (With Concurrent) | 6 | AgentPool integration, concurrent processing |

**Total Stories**: 39

## Dependencies

```
Sprint 1 (Core Types - Traits) ─────────────────────────────────────┐
    ↓                                                                │
Sprint 2 (Output Classifier - Registry) ─────────────────────────────┤
    ↓                                                                │
Sprint 3 (Decision Engine - Action Sequences) ───────────────────────┤
    ↓                                                                │
Sprint 4 (Context Cache) ────────────────────────────────────────────┤
    ↓                                                                │
Sprint 5 (Lifecycle) ────────────────────────────────────────────────┤
    ↓                                                                │
Sprint 6 (Human Intervention - BlockingReason) ──────────────────────┤
    ↓                                                                │
Sprint 7 (Error Recovery) ───────────────────────────────────────────┤
    ↓                                                                │
Sprint 8 (Integration + Concurrent) ─────────────────────────────────┘
```

## Stories Summary

### Sprint 1: Core Types (Trait-Based)
- **Story 1.1**: DecisionSituation trait and SituationRegistry
- **Story 1.2**: Built-in situation implementations (4 types)
- **Story 1.3**: DecisionAction trait and ActionRegistry
- **Story 1.4**: Built-in action implementations
- **Story 1.5**: DecisionContext with trait references
- **Story 1.6**: BlockingReason trait and BlockedState

### Sprint 2: Output Classifier (Trait-Based)
- **Story 2.1**: OutputClassifier trait and ClassifierRegistry
- **Story 2.2**: Claude classifier with situation builders
- **Story 2.3**: Codex classifier with approval builders
- **Story 2.4**: ACP classifier (OpenCode/Kimi)
- **Story 2.5**: Classifier initialization and registration

### Sprint 3: Decision Engine (Trait-Based)
- **Story 3.1**: DecisionEngine trait (action-based)
- **Story 3.2**: LLM decision engine
- **Story 3.3**: CLI decision engine (independent session)
- **Story 3.4**: Rule-based engine with expression engine
- **Story 3.5**: Mock decision engine
- **Story 3.6**: Tiered decision engine (complexity-based)

### Sprint 4: Context Cache
- **Story 4.1**: RunningContextCache with size limits
- **Story 4.2**: Context compression and priority retention
- **Story 4.3**: Context persistence and recovery

### Sprint 5: Lifecycle
- **Story 5.1**: Decision Agent creation policies (Eager/Lazy)
- **Story 5.2**: Decision Agent destruction and cleanup
- **Story 5.3**: Task switching with context preservation
- **Story 5.4**: Session persistence for multi-turn decisions

### Sprint 6: Human Intervention (Trait-Based)
- **Story 6.1**: Criticality evaluation integration
- **Story 6.2**: HumanDecisionBlocking implementation
- **Story 6.3**: HumanDecisionQueue with priority
- **Story 6.4a**: TUI decision modal display
- **Story 6.4b**: TUI decision modal interaction
- **Story 6.5**: CLI human decision commands

### Sprint 7: Error Recovery
- **Story 7.1**: RecoveryLevel escalation strategy
- **Story 7.2**: Timeout handling with fallback
- **Story 7.3**: Decision Agent self-error recovery
- **Story 7.4**: Health check and auto-recovery

### Sprint 8: Integration (With Concurrent)
- **Story 8.1**: AgentSlot extension (generic Blocked)
- **Story 8.2**: AgentPool blocked handling
- **Story 8.3**: Integration with Backlog and Kanban
- **Story 8.4**: Integration with WorkplaceStore
- **Story 8.5**: Decision observability
- **Story 8.6**: Concurrent processing (SessionPool, RateLimiter, Arbitrator)

## File Structure

```
docs/plan/spec/decision-layer/
├── README.md                        # This file
├── architecture-evolution.md        # Architecture evolution proposal
├── test-specification.md            # TDD test task definitions
├── sprint-01-core-types.md          # Trait-based core types
├── sprint-02-output-classifier.md   # Trait-based classifiers
├── sprint-03-decision-engine.md     # Action-based engines
├── sprint-04-context-cache.md
├── sprint-05-lifecycle.md
├── sprint-06-human-intervention.md  # BlockingReason trait
├── sprint-07-error-recovery.md
└── sprint-08-integration.md         # With concurrent processing
```

## Test-Driven Development

All implementation follows TDD methodology defined in [test-specification.md](test-specification.md):

1. **Write test first**: Before implementing any feature, define the test case
2. **Make test fail**: Verify the test correctly captures the requirement
3. **Implement minimum**: Write just enough code to pass the test
4. **Refactor**: Clean up implementation while keeping tests passing

Total TDD tests: **150+ tests** across all sprints, defined upfront.

## Target Module Structure

```
decision/                    # agent-decision crate (standalone)
├── Cargo.toml
└── src/
    ├── lib.rs               # Module exports
    ├── error.rs             # DecisionError enum
    ├── types.rs             # SituationType, ActionType, identifiers
    ├── situation.rs         # DecisionSituation trait
    ├── situation_registry.rs # SituationRegistry
    ├── builtin_situations.rs # Built-in situations
    ├── action.rs            # DecisionAction trait
    ├── action_registry.rs   # ActionRegistry
    ├── builtin_actions.rs   # Built-in actions
    ├── output.rs            # DecisionOutput (action sequence)
    ├── context.rs           # DecisionContext, RunningContextCache
    ├── blocking.rs          # BlockingReason trait, BlockedState
    ├── classifier.rs        # OutputClassifier trait
    ├── classifier_registry.rs # ClassifierRegistry
    ├── claude_classifier.rs # Claude classifier + builders
    ├── codex_classifier.rs  # Codex classifier + builders
    ├── acp_classifier.rs    # ACP classifier + builders
    ├── engine.rs            # DecisionEngine trait
    ├── llm_engine.rs        # LLMDecisionEngine
    ├── cli_engine.rs        # CLIDecisionEngine
    ├── rule_engine.rs       # RuleBasedDecisionEngine
    ├── condition.rs         # ConditionExpr, ConditionEvaluatorRegistry
    ├── mock_engine.rs       # MockDecisionEngine
    ├── tiered_engine.rs     # TieredDecisionEngine
    ├── human_blocking.rs    # HumanDecisionBlocking
    ├── human_queue.rs       # HumanDecisionQueue
    ├── human_request.rs     # HumanDecisionRequest/Response
    ├── arbitrator.rs        # HumanDecisionArbitrator
    ├── session_pool.rs      # DecisionSessionPool
    ├── rate_limiter.rs      # DecisionRateLimiter
    ├── recovery.rs          # Error recovery strategies
    ├── prompts.rs           # Decision prompt templates
    ├── initializer.rs       # DecisionLayerInitializer
    └── metrics.rs           # Decision metrics and observability

core/                        # agent-core crate (depends on agent-decision)
└── src/
    ├── decision_integration.rs  # Integration helpers
    └── agent_slot.rs            # Extended with Blocked(BlockedState)
```

## Key Design Decisions

1. **Trait + Registry**: Situation, Action, Blocking all use trait for extensibility
2. **SituationType**: String-based identifier (not enum) for provider-specific subtypes
3. **Action Sequence**: DecisionOutput contains Vec<Box<dyn DecisionAction>>
4. **BlockingReason Trait**: Generic Blocked(BlockedState) in AgentSlotStatus
5. **Expression Engine**: ConditionExpr supports AND/OR/NOT/Custom for rules
6. **Concurrent Support**: SessionPool, RateLimiter, Arbitrator for multi-agent
7. **Provider Flexibility**: Decision Agent can use different provider than Main Agent
8. **Protocol-Level Classification**: Primary detection via provider protocol events
9. **Tiered Decision**: Complexity-based engine selection (RuleBased → LLM)
10. **Human Arbitration**: Sequential/Batched/Parallel strategies for multiple requests

## Provider Decision Trigger Points

Based on source code research:

| Provider | Situation Type Detected | Trigger Event | Classifier |
|----------|------------------------|---------------|------------|
| **Claude** | `finished.claude`, `error` | `Finished`, `Error` | ClaudeClassifier |
| **Codex** | `waiting_for_choice.codex` | `execCommandApproval`, `applyPatchApproval` | CodexClassifier |
| **OpenCode/Kimi (ACP)** | `waiting_for_choice.acp`, `claims_completion` | `permission.asked`, `session.status.idle` | ACPClassifier |

## Extension Points

| What to Add | Where | How |
|------------|-------|-----|
| New situation type | `builtin_situations.rs` or custom module | Implement DecisionSituation trait, register in SituationRegistry |
| New action type | `builtin_actions.rs` or custom module | Implement DecisionAction trait, register in ActionRegistry |
| New blocking reason | custom module | Implement BlockingReason trait, use with BlockedState |
| New provider classifier | custom module | Implement OutputClassifier trait, register in ClassifierRegistry |
| New rule condition | custom module | Implement ConditionEvaluator trait, register in ConditionEvaluatorRegistry |

## Running Tests

```bash
# Build the decision crate
cargo build -p agent-decision

# Run decision tests
cargo test -p agent-decision

# Build core (depends on decision)
cargo build -p agent-core

# Run core tests
cargo test -p agent-core

# Format code
cargo fmt -p agent-decision
cargo fmt -p agent-core
```

## References

- [Architecture Evolution Proposal](architecture-evolution.md)
- [Test Specification](test-specification.md)
- [Decision Layer Idea Document](../../decision-layer-idea.md)
- [Multi-Agent Parallel Runtime Design](../multi-agent-parallel-runtime-design.md)
- [Multi-Agent Sprint Backlog](../multi-agent/backlog.md)
- [Kanban System Specification](../kanban/README.md)
- [Provider Analysis - OpenCode](../../opencode/provider-analysis.md)
- [Provider Analysis - Kimi-CLI](../../kimi-cli/provider-analysis.md)