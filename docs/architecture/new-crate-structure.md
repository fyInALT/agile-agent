# New Crate Structure (Post Architecture Refactoring)

> **Status**: Active — Sprints 1–5 complete, Sprint 6 cleanup in progress.
> **Last updated**: 2026-04-20

## Overview

The architecture refactoring (see [`refactoring-plan-v2.md`](./refactoring-plan-v2.md)) reorganized the codebase from 3 large crates (`agent-core`, `agent-decision`, `agent-daemon`) into a layered architecture with clear dependency directions and 6 target crates.

## Target Crate Architecture

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                              agent-cli                                  │
│                     (thin entrypoint, protocol client)                  │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
┌─────────────────────────────────────────────────────────────────────────┐
│                            agent-daemon                                 │
│         (WebSocket server, EventLoop, DecisionCommandInterpreter)       │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
        ┌───────────────────────────┼───────────────────────────┐
        │                           │                           │
┌───────▼────────┐        ┌────────▼────────┐        ┌────────▼────────┐
│  agent-core    │        │ agent-behavior- │        │  agent-worktree │
│                │        │    infra        │        │                 │
│ WorkerPool,    │        │                 │        │ Git worktree    │
│ AppState,      │        │ EffectHandler   │        │ isolation       │
│ RuntimeSession │        │ trait, command  │        │                 │
└───────┬────────┘        └────────┬────────┘        └─────────────────┘
        │                          │
        │               ┌──────────▼──────────┐
        │               │  agent-runtime-     │
        │               │     domain          │
        │               │                     │
        │               │ WorkerState,        │
        │               │ Worker (TODO),      │
        │               │ TranscriptJournal,  │
        │               │ RuntimeCommand      │
        │               │ (TODO), JournalEntry│
        │               └──────────┬──────────┘
        │                          │
        │               ┌──────────▼──────────┐
        │               │    agent-events     │
        │               │   (shared kernel)   │
        │               │                     │
        │               │ DomainEvent (24)    │
        │               │ DecisionEvent       │
        │               └──────────┬──────────┘
        │                          │
        │        ┌─────────────────┼─────────────────┐
        │        │                 │                 │
┌───────▼────────▼──────┐ ┌────────▼────────┐ ┌─────▼──────┐
│     agent-provider    │ │   agent-types   │ │ agent-toolkit│
│  (Claude, Codex, Mock)│ │  (AgentId, etc) │ │(ExecCommand)│
└───────────────────────┘ └─────────────────┘ └────────────┘
```

## Crate Responsibilities

### `agent-events` — Shared Kernel

- **Depends on**: `agent-types`, `agent-toolkit`
- **Used by**: All other crates
- **Contains**:
  - `DomainEvent` (24 variants) — unified event type
  - `DecisionEvent` (8 variants) — decision-layer focused subset
  - `From<&DomainEvent> for Option<DecisionEvent>` automatic conversion
  - `SessionHandle`, execution status types

### `agent-runtime-domain` — Pure Domain Model

- **Depends on**: `agent-types`, `agent-events`, `agent-toolkit`
- **Used by**: `agent-core`, `agent-behavior-infra`
- **Contains**:
  - `WorkerState` (6-variant state machine with `RespondingSubState`)
  - `TranscriptJournal`, `JournalEntry`
- **TODO**: Move `Worker` aggregate root here (currently in `agent-core`)
- **TODO**: Move `RuntimeCommand` here (currently in `agent-behavior-infra`)

### `agent-behavior-infra` — Behavior Infrastructure

- **Depends on**: `agent-types`, `agent-events`, `agent-runtime-domain`
- **Used by**: `agent-core`, `agent-daemon`
- **Contains**:
  - `EffectHandler` trait
  - `NoopEffectHandler`, `RecordingEffectHandler`
  - `EffectError`
- **TODO**: Event loop phase traits and command processing logic still in `agent-daemon`

### `agent-core` — Runtime Engine

- **Depends on**: `agent-events`, `agent-decision`, `agent-provider`, `agent-worktree`, `agent-backlog`, `agent-storage`, `agent-runtime-domain`, `agent-behavior-infra`
- **Used by**: `agent-daemon`, `agent-cli`
- **Contains**:
  - `WorkerPool` (alias `AgentPool`)
  - `WorkerHandle` (alias `AgentSlot`)
  - `RuntimeSession`, `AppState`
  - `EventAggregator`
  - `DecisionExecutor` (legacy + `translate()` pure function)
- **TODO**: `Worker` aggregate root should move to `agent-runtime-domain`

### `agent-decision` — Decision Layer

- **Depends on**: `agent-events`, `serde`
- **Used by**: `agent-core`
- **Contains**:
  - `DecisionCommand` (21-variant pure enum)
  - `DecisionExecutor::translate()` — pure `DecisionOutput → Vec<DecisionCommand>`
  - Classifiers, engines, situations
- **Key invariant**: Zero dependencies on `agent-core` or `agent-daemon`

### `agent-daemon` — Application Layer

- **Depends on**: `agent-core`, `agent-protocol`
- **Used by**: `agent-cli`
- **Contains**:
  - `EventLoop` (alias `SessionManager`)
  - HTTP/WebSocket server
  - `DecisionCommandInterpreter`
  - Event pump, broadcaster

### Other Crates (unchanged)

| Crate | Responsibility |
|-------|---------------|
| `agent-types` | Foundation types (`AgentId`, `ProviderKind`, `TaskId`, `WorkerStatus` (was `AgentStatus`)) |
| `agent-toolkit` | Tool call types (`PatchChange`, `ExecCommandStatus`) |
| `agent-provider` | Provider execution (`ClaudeProcess`, `CodexProcess`, `MockProcess`, `LaunchConfig`) |
| `agent-worktree` | Git worktree isolation |
| `agent-backlog` | Task and backlog management |
| `agent-storage` | Persistence layer |
| `agent-protocol` | JSON-RPC types, events, snapshots |
| `agent-commands` | Command bus and slash command system |
| `agent-tui` | Terminal UI (protocol-only) |
| `agent-cli` | Binary entrypoints |
| `agent-kanban` | Trait-based Kanban domain model |
| `agent-llm-provider` | OpenAI client |

## Dependency Direction Rules

1. **`agent-events` is the shared kernel**: Depends on nothing except `agent-types` and `agent-toolkit`. All other crates may depend on it.
2. **`agent-decision` is read-only**: Consumes `agent-events`, produces `DecisionCommand`. No dependency on `agent-core` or `agent-daemon`.
3. **`agent-runtime-domain` is pure**: Zero I/O dependencies. Only types and pure functions.
4. **`agent-behavior-infra` depends on `agent-runtime-domain`**: For `WorkerState`, `RuntimeCommand` (after move).
5. **`agent-daemon` is the only orchestration layer**: It depends on all other crates and wires them together.
6. **No cycles**: `cargo tree --workspace` confirms zero circular dependencies.

## Type Rename Status

| Old Name | New Name | Status |
|----------|----------|--------|
| `AgentSlot` | `WorkerHandle` | ✅ Done (alias preserved) |
| `AgentPool` | `WorkerPool` | ✅ Done (alias preserved) |
| `SessionManager` | `EventLoop` | ✅ Done (alias preserved) |
| `FocusManager` | `WorkerFocusManager` | ✅ Done |
| `WorktreeCoordinator` | `WorkerWorktreeManager` | ✅ Done |
| `ProviderEvent` | `DomainEvent` | ✅ Done (alias preserved) |
| `DecisionAction` | `DecisionCommand` | ✅ Done |
| `AgentStatus` | `WorkerStatus` | ✅ Done (alias preserved) |
| `spawn_agent()` | `spawn_worker()` | ✅ Done (deprecated wrappers) |
| `DecisionAgentCoordinator` | `WorkerDecisionRouter` | ⏳ Pending |
| `ProviderThread` | `WorkerExecutionThread` | ⏳ Pending |

## Known Gaps

1. **`Worker` not in `agent-runtime-domain`**: The aggregate root is still in `agent-core`.
2. **`RuntimeCommand` not in `agent-runtime-domain`**: The effect enum is in `agent-behavior-infra`, creating a cross-dependency issue for moving `Worker`.
3. **`AgentSlotStatus` still exists**: The old 13-variant enum coexists with the new 6-variant `WorkerState`.
4. **`agent-protocol-infra` not extracted**: `ProtocolGateway` does not exist in the codebase; this story was skipped.
5. **`agent-runtime-app` not extracted**: `agent-cli` is already a thin wrapper; this story was skipped.
6. **Protocol hardening (Sprint 6.1)**: Not started — requires external tooling.
7. **Performance regression testing (Sprint 6.4)**: Not started — requires baseline measurements.
