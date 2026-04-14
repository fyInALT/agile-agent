# Decision Layer Specification

## Overview

This directory contains the Scrum-style breakdown of the decision layer implementation. The decision layer enables autonomous development by monitoring provider outputs and making decisions to keep the development loop running without human intervention.

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
- Classifies outputs into running vs. blocked states
- Identifies four decision situations
- Uses configurable decision engine (LLM/CLI/RuleBased/Mock)
- Persists decision session independently

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
│  │                             │  │                             │   │
│  │ ┌─────────────────────────┐ │  │ ┌─────────────────────────┐ │   │
│  │ │ Decision Agent A'       │ │  │ │ Decision Agent B'       │ │   │
│  │ │ Provider: Codex (diff)  │ │  │ │ Provider: Claude (diff) │ │   │
│  │ │ Session: sess-dec-a     │ │  │ │ Session: sess-dec-b     │ │   │
│  │ │ [Exclusive to A]        │ │  │ │ [Exclusive to B]        │ │   │
│  │ └─────────────────────────┘ │  │ └─────────────────────────┘ │   │
│  └─────────────────────────────┘  └─────────────────────────────┘   │
│                                                                      │
│  ┌─────────────────────────────┐                                    │
│  │ Main Agent C                │                                    │
│  │ Provider: Kimi              │                                    │
│  │ ┌─────────────────────────┐ │                                    │
│  │ │ Decision Agent C'       │ │                                    │
│  │ │ Provider: Claude        │ │                                    │
│  │ │ [Exclusive to C]        │ │                                    │
│  │ └─────────────────────────┘ │                                    │
│  └─────────────────────────────┘                                    │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Architecture Points**:

| Aspect | Description |
|--------|-------------|
| Exclusive Ownership | Each Main Agent has one exclusive Decision Agent |
| Provider Independence | Decision Agent provider type can differ from Main Agent |
| Session Independence | Decision Agent has separate session |
| Decision Scope | Decision Agent only decides for its parent Main Agent |
| Persistence Separation | Decision transcript stored separately |

## Sprints

| Sprint | Name | Stories | Goal |
|--------|------|---------|------|
| [Sprint 1](sprint-01-core-types.md) | Core Types | 4 | Domain types, ProviderStatus, DecisionOutput |
| [Sprint 2](sprint-02-output-classifier.md) | Output Classifier | 4 | Provider output classification for each provider type |
| [Sprint 3](sprint-03-decision-engine.md) | Decision Engine | 7 | LLM/CLI/RuleBased/Mock engine implementations |
| [Sprint 4](sprint-04-context-cache.md) | Context Cache | 3 | Running context caching with size limits |
| [Sprint 5](sprint-05-lifecycle.md) | Lifecycle | 4 | Decision Agent creation, destruction, task switching |
| [Sprint 6](sprint-06-human-intervention.md) | Human Intervention | 6 | Critical decision escalation to human |
| [Sprint 7](sprint-07-error-recovery.md) | Error Recovery | 4 | Retry logic, timeout handling, recovery levels |
| [Sprint 8](sprint-08-integration.md) | Integration | 6 | Integration with existing agile-agent components |

**Total Stories**: 38 (after splitting oversized tasks)

## Dependencies

```
Sprint 1 (Core Types) ──────────────────────────────────────────────┐
    ↓                                                                │
Sprint 2 (Output Classifier) ───────────────────────────────────────┤
    ↓                                                                │
Sprint 3 (Decision Engine) ──────────────────────────────────────────┤
    ↓                                                                │
Sprint 4 (Context Cache) ────────────────────────────────────────────┤
    ↓                                                                │
Sprint 5 (Lifecycle) ────────────────────────────────────────────────┤
    ↓                                                                │
Sprint 6 (Human Intervention) ───────────────────────────────────────┤
    ↓                                                                │
Sprint 7 (Error Recovery) ───────────────────────────────────────────┤
    ↓                                                                │
Sprint 8 (Integration) ──────────────────────────────────────────────┘
```

## Stories Summary

### Sprint 1: Core Types
- **Story 1.1**: Define ProviderStatus and ProviderOutputType enums
- **Story 1.2**: Define DecisionOutput and DecisionContext structs
- **Story 1.3**: Define DecisionAgentConfig and DecisionEngine enums
- **Story 1.4**: Define CriticalDecisionCriteria and HumanDecisionRequest

### Sprint 2: Output Classifier
- **Story 2.1**: Claude output classifier implementation
- **Story 2.2**: Codex output classifier implementation
- **Story 2.3**: ACP output classifier (OpenCode/Kimi) implementation
- **Story 2.4**: Unified OutputClassifierRegistry

### Sprint 3: Decision Engine
- **Story 3.1**: DecisionEngine trait definition
- **Story 3.2a**: Decision prompt templates
- **Story 3.2b**: LLMDecisionEngine API integration
- **Story 3.3a**: CLI Decision Engine session management
- **Story 3.3b**: CLI Decision Engine provider integration
- **Story 3.4**: RuleBasedDecisionEngine implementation
- **Story 3.5**: MockDecisionEngine for testing

### Sprint 4: Context Cache
- **Story 4.1**: RunningContextCache with size limits
- **Story 4.2**: Context compression and priority retention
- **Story 4.3**: Context persistence and recovery

### Sprint 5: Lifecycle
- **Story 5.1**: Decision Agent creation policies (Eager/Lazy)
- **Story 5.2**: Decision Agent destruction and cleanup
- **Story 5.3**: Task switching with context preservation
- **Story 5.4**: Session persistence for multi-turn decisions

### Sprint 6: Human Intervention
- **Story 6.1**: CriticalDecisionCriteria evaluation
- **Story 6.2**: HumanDecisionQueue implementation
- **Story 6.3**: Human decision notification system
- **Story 6.4a**: TUI decision modal display
- **Story 6.4b**: TUI decision modal interaction
- **Story 6.5**: CLI human decision commands

### Sprint 7: Error Recovery
- **Story 7.1**: RecoveryLevel escalation strategy
- **Story 7.2**: Timeout handling with fallback
- **Story 7.3**: Decision Agent self-error recovery
- **Story 7.4**: Health check and auto-recovery

### Sprint 8: Integration
- **Story 8.1a**: AgentSlot extension for Decision Agent
- **Story 8.1b**: AgentPool blocked agent handling
- **Story 8.2**: Integration with Backlog and Kanban
- **Story 8.3**: Integration with WorkplaceStore
- **Story 8.4**: Decision observability (metrics, logs)
- **Story 8.5**: Cost optimization strategies

## File Structure

```
docs/plan/spec/decision-layer/
├── README.md                        # This file
├── test-specification.md            # TDD test task definitions (130+ tests)
├── sprint-01-core-types.md
├── sprint-02-output-classifier.md
├── sprint-03-decision-engine.md
├── sprint-04-context-cache.md
├── sprint-05-lifecycle.md
├── sprint-06-human-intervention.md
├── sprint-07-error-recovery.md
└── sprint-08-integration.md
```

## Test-Driven Development

All implementation follows TDD methodology defined in [test-specification.md](test-specification.md):

1. **Write test first**: Before implementing any feature, define the test case
2. **Make test fail**: Verify the test correctly captures the requirement
3. **Implement minimum**: Write just enough code to pass the test
4. **Refactor**: Clean up implementation while keeping tests passing

Total TDD tests: **130+ tests** across all sprints, defined upfront.

## Target Module Structure

```
decision/                    # agent-decision crate (standalone)
├── Cargo.toml
└── src/
    ├── lib.rs               # Module exports
    ├── error.rs             # DecisionError enum
    ├── types.rs             # ProviderStatus, DecisionOutput, etc.
    ├── classifier.rs        # OutputClassifier trait
    ├── claude_classifier.rs # Claude-specific classifier
    ├── codex_classifier.rs  # Codex-specific classifier
    ├── acp_classifier.rs    # ACP (OpenCode/Kimi) classifier
    ├── engine.rs            # DecisionEngine trait
    ├── llm_engine.rs        # LLMDecisionEngine
    ├── cli_engine.rs        # CLIDecisionEngine
    ├── rule_engine.rs       # RuleBasedDecisionEngine
    ├── mock_engine.rs       # MockDecisionEngine
    ├── context_cache.rs     # RunningContextCache
    ├── agent.rs             # DecisionAgent struct
    ├── lifecycle.rs         # Creation/destruction policies
    ├── human_queue.rs       # HumanDecisionQueue
    ├── human_request.rs     # HumanDecisionRequest/Response
    ├── recovery.rs          # Error recovery strategies
    ├── prompts.rs           # Decision prompt templates
    └── metrics.rs           # Decision metrics and observability

core/                        # agent-core crate (depends on agent-decision)
└── src/
    ├── decision_integration.rs  # Integration helpers
    └── agent_slot.rs            # Extended with BlockedForHumanDecision
```

## Key Design Decisions

1. **Exclusive Ownership**: Each Main Agent has exactly one Decision Agent (not shared)
2. **Independent Session**: Decision Agent uses separate provider session from Main Agent
3. **Provider Flexibility**: Decision Agent can use different provider type than Main Agent
4. **Protocol-Level Classification**: Primary detection via provider protocol events, not keywords
5. **Tiered Decision Engines**: LLM for complex, RuleBased for simple, Mock for testing
6. **Critical Decision Escalation**: Important decisions escalate to human with blocked agent state
7. **Context Compression**: Running context cached with size limits to control prompt size
8. **Reflection Mechanism**: Up to 2 reflection rounds before final completion verification

## Provider Decision Trigger Points

Based on source code research:

| Provider | Waiting for Choice Trigger | Completion Trigger | Error Trigger |
|----------|---------------------------|-------------------|---------------|
| **Claude** | None (bypassPermissions) | `Finished` event | `Error` event |
| **Codex** | `execCommandApproval`, `applyPatchApproval`, `requestUserInput` | No explicit marker | `timed_out`, `abort` |
| **OpenCode/Kimi (ACP)** | `permission.asked` | `session.status.idle` | No explicit ACP error |

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

- [Decision Layer Idea Document](../../decision-layer-idea.md)
- [Multi-Agent Parallel Runtime Design](../multi-agent-parallel-runtime-design.md)
- [Multi-Agent Sprint Backlog](../multi-agent/backlog.md)
- [Kanban System Specification](../kanban/README.md)
- [Provider Analysis - OpenCode](../../opencode/provider-analysis.md)
- [Provider Analysis - Kimi-CLI](../../kimi-cli/provider-analysis.md)