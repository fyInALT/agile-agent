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

**Total Duration: 10 weeks**

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
```

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
