# Architecture Refactoring Sprint Specifications

This directory contains the sprint specifications for the `agile-agent` architecture refactoring, derived from `docs/architecture/refactoring-plan-v2.md`.

## Overview

The refactoring transforms the current monolithic event loop into a structured "Handwritten Actor" model:
- **Shared Kernel**: Unified event types across all crates (`agent-events`)
- **Worker Aggregate Root**: Explicit state machine with `apply(event)` method
- **EventLoop**: 7 explicit phases with `RuntimeCommand` effect system
- **Decision Layer**: Read-only classification returning pure `DecisionCommand`
- **Crate Reorganization**: 6 target crates with clean dependency directions

## Sprint Index

| Sprint | Title | Duration | Key Deliverable |
|--------|-------|----------|-----------------|
| [Sprint 1](./sprint-01-shared-kernel.md) | Shared Kernel Extraction | 2 weeks | `agent-events` crate with `DomainEvent` + `DecisionEvent` |
| [Sprint 2](./sprint-02-worker-aggregate-root.md) | Worker Aggregate Root | 2 weeks | `Worker` with `WorkerState` state machine and `apply()` |
| [Sprint 3](./sprint-03-event-loop-refactoring.md) | EventLoop Refactoring | 2 weeks | 7-phase `tick()` + `RuntimeCommand` effect system |
| [Sprint 4](./sprint-04-decision-layer-decoupling.md) | Decision Layer Decoupling | 2 weeks | Read-only `DecisionCommand` + interpreter |
| [Sprint 5](./sprint-05-crate-reorganization.md) | Crate Reorganization & Type Renaming | 1 week | 6 target crates + 11 type renames |
| [Sprint 6](./sprint-06-cleanup-and-polish.md) | Protocol Layer, Cleanup & Polish | 1 week | Protocol hardening + performance validation |
| [Sprint 7](./sprint-07-tui-effect-system.md) | TUI Effect System Integration | 2 weeks | `TuiEffectHandler` + pure path in `AppLoop` |
| [Sprint 8](./sprint-08-worker-state-synchronization.md) | WorkerState Synchronization | 2 weeks | Extended `WorkerState` (11 variants) + bidirectional sync |
| [Sprint 9](./sprint-09-decision-event-protocol.md) | DecisionEvent Protocol Enhancement | 1.5 weeks | Specific `DecisionEvent` variants + classifier improvements |

**Total Duration: 15.5 weeks**

## Dependency Graph

```
Sprint 1 (Shared Kernel)
    │
    ▼
Sprint 2 (Worker Aggregate)
    │
    ▼
Sprint 3 (EventLoop)
    │
    ▼
Sprint 4 (Decision Decoupling)
    │
    ▼
Sprint 5 (Crate Reorg)
    │
    ▼
Sprint 6 (Cleanup)
    │
    ▼
Sprint 7 (TUI Effect System) ──┐
    │                          │
    ▼                          │
Sprint 8 (WorkerState Sync)    │
    │                          │
    ▼                          │
Sprint 9 (DecisionEvent) ◄─────┘
```

Sprints 7–9 address critical architecture gaps discovered during the Sprint 4–6 implementation:

- **Sprint 7** closes the TUI/daemon behavioral divergence by giving the TUI its own `EffectHandler`
- **Sprint 8** eliminates the `AgentSlotStatus`/`WorkerState` split-brain problem
- **Sprint 9** restores decision-layer visibility into events that were previously collapsed to generic status updates

## Target Crate Structure

After Sprint 5, the workspace will have:

| Crate | Purpose | Dependencies |
|-------|---------|-------------|
| `agent-events` | Shared kernel: event types, basic types | `serde`, `agent-types` |
| `agent-runtime-domain` | Pure domain model: Worker, state machine | `agent-events` |
| `agent-behavior-infra` | Event loop phases, effect handlers | `agent-runtime-domain` |
| `agent-protocol-infra` | External protocol: WebSocket, stdio | `agent-events` |
| `agent-runtime-app` | Application wiring: main(), startup | All above |
| `agent-cli` | Thin binary wrapper | `agent-runtime-app` |

## Type Renames

| Old Name | New Name |
|----------|----------|
| `AgentSlot` | `WorkerHandle` |
| `AgentPool` | `WorkerPool` |
| `SessionManager` | `EventLoop` |
| `ProviderEvent` | `DomainEvent` |
| `DecisionAction` | `DecisionCommand` |
| `AgentStatus` | `WorkerStatus` |
| `spawn_agent()` | `spawn_worker()` |
| `FocusManager` | `WorkerFocusManager` |
| `WorktreeCoordinator` | `WorkerWorktreeManager` |
| `DecisionCoordinator` | `WorkerDecisionRouter` |
| `ProviderThread` | `WorkerExecutionThread` |

## Related Documents

- [`docs/architecture/refactoring-plan-v2.md`](../architecture/refactoring-plan-v2.md) — Detailed refactoring plan
- [`docs/architecture/refactoring-plan-reflection.md`](../architecture/refactoring-plan-reflection.md) — Critical self-reflection on over-engineering risks
- [`docs/architecture/provider-agent-actor.md`](../architecture/provider-agent-actor.md) — Actor model analysis of current system
