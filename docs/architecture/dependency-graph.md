# Dependency Graph

## Current Architecture (Post Sprint 5)

```text
                    ┌─────────────────┐
                    │   agent-cli     │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │  agent-daemon   │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────▼───────┐   ┌───────▼───────┐   ┌───────▼───────┐
│  agent-core   │   │ agent-behavior-│   │  agent-worktree│
│               │   │    infra       │   │               │
└───────┬───────┘   └───────┬───────┘   └───────────────┘
        │                   │
        │          ┌────────▼────────┐
        │          │ agent-runtime-  │
        │          │    domain       │
        │          └────────┬────────┘
        │                   │
        │          ┌────────▼────────┐
        │          │  agent-events   │  ← shared kernel
        │          └────────┬────────┘
        │                   │
        │      ┌────────────┼────────────┐
        │      │            │            │
        │ ┌────▼─────┐ ┌────▼─────┐ ┌───▼──────┐
        │ │agent-types│ │agent-toolkit│ │agent-provider│
        │ └──────────┘ └──────────┘ └──────────┘
        │
        │            ┌─────────────────┐
        └────────────►  agent-decision │  ← read-only, no cycles
                     └────────┬────────┘
                              │
                     ┌────────▼────────┐
                     │  agent-events   │  ← shared kernel
                     └─────────────────┘
```

## Direction Rules

1. **`agent-decision` is read-only**: It consumes `agent-events` and produces
   `DecisionCommand` pure data. It has **no** dependency on `agent-core`,
   `agent-daemon`, or any runtime types.

2. **`agent-events` is the shared kernel**: Only depends on `agent-types` and
   `agent-toolkit`. All other crates may depend on it.

3. **`agent-runtime-domain` is pure**: Zero I/O dependencies. Contains
   `WorkerState`, `TranscriptJournal`, `JournalEntry`.

4. **`agent-behavior-infra` depends on `agent-runtime-domain`**: Contains
   `EffectHandler` trait and effect implementations.

5. **No cycles**: `cargo tree --workspace` confirms zero circular dependencies.

6. **Effect flow**: `DecisionCommand` → `DecisionCommandInterpreter` →
   `RuntimeCommand` → `EffectHandler` → side effects.

## Crate Details

| Crate | Depends On | Used By |
|-------|-----------|---------|
| `agent-types` | serde | all |
| `agent-toolkit` | agent-types, serde | agent-events, agent-provider |
| `agent-events` | agent-types, agent-toolkit | agent-decision, agent-provider, agent-core, agent-runtime-domain, agent-behavior-infra |
| `agent-provider` | agent-events, agent-toolkit | agent-core |
| `agent-decision` | agent-events, serde | agent-core |
| `agent-runtime-domain` | agent-events, agent-types, agent-toolkit | agent-core, agent-behavior-infra |
| `agent-behavior-infra` | agent-runtime-domain, agent-events, agent-types | agent-core, agent-daemon |
| `agent-core` | agent-events, agent-decision, agent-provider, agent-worktree, agent-backlog, agent-storage, agent-runtime-domain, agent-behavior-infra | agent-daemon, agent-cli |
| `agent-daemon` | agent-core, agent-protocol | agent-cli |
| `agent-protocol` | serde | agent-daemon |
| `agent-worktree` | git2, serde | agent-core |
| `agent-backlog` | serde | agent-core |
| `agent-storage` | serde | agent-core |
| `agent-tui` | crossterm, ratatui | agent-cli |
| `agent-cli` | agent-daemon, agent-core, agent-tui | — |

## Type Rename Map

| Old Name | New Name | Location |
|----------|----------|----------|
| `AgentSlot` | `WorkerHandle` | `core/src/agent_slot.rs` |
| `AgentPool` | `WorkerPool` | `core/src/agent_pool.rs` |
| `SessionManager` | `EventLoop` | `agent/daemon/src/session_mgr.rs` |
| `FocusManager` | `WorkerFocusManager` | `core/src/pool/focus_manager.rs` |
| `WorktreeCoordinator` | `WorkerWorktreeManager` | `core/src/pool/worktree_coordinator.rs` |
| `ProviderEvent` | `DomainEvent` | `agent/events/src/lib.rs` |
| `DecisionAction` | `DecisionCommand` | `decision/src/command.rs` |
| `AgentStatus` | `WorkerStatus` | `agent/types/src/worker_status.rs` |
