# Sprint 3: EventLoop Refactoring

## Metadata

- Sprint ID: `sref-003`
- Title: `EventLoop Refactoring`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-22

## Background

The `SessionManager::tick()` method (in `agent/daemon/src/session_mgr.rs`) is the heart of the system — a 100ms event loop that currently implements all business logic in a single 200+ line function. It handles: polling provider events, idle agent detection, decision agent polling, mailbox processing, thread cleanup, and input dispatch. This monolithic function is the primary obstacle to understanding and modifying the system.

This sprint restructures `tick()` into 7 explicit phases, introduces a `RuntimeCommand` effect system, and makes `Worker::apply()` the central state transition mechanism. The `EventLoop` (renamed from `SessionManager`) becomes an orchestrator that:
1. Collects events from sources
2. Applies them to `Worker` via `apply()`
3. Interprets returned `RuntimeCommand` effects
4. Executes effects via effect handlers

This is the pivotal sprint that makes the "Handwritten Actor" model concrete.

## Sprint Goal

Split `tick()` into 7 explicit phase methods. Introduce `RuntimeCommand` effect type with `Worker::apply()` returning it. Make `EventLoop` interpret and execute effects. All integration tests pass.

## Stories

### Story 3.1: Split `tick()` into 7 Explicit Phases

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Extract each logical section of `tick()` into a dedicated method with a clear contract.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Extract Phase 1: `poll_provider_events()` — drain provider RX channels, convert to `DomainEvent` | Todo | - |
| T3.1.2 | Extract Phase 2: `check_idle_agents()` — trigger decision layer for idle agents (60s timeout / 300s cooldown) | Todo | - |
| T3.1.3 | Extract Phase 3: `poll_decision_agents()` — drain decision agent RX channels, process responses | Todo | - |
| T3.1.4 | Extract Phase 4: `process_mailbox()` — handle `UserInput`, `StopAgent`, `DecisionResult` messages | Todo | - |
| T3.1.5 | Extract Phase 5: `cleanup_completed()` — remove agents in terminal states | Todo | - |
| T3.1.6 | Extract Phase 6: `reconcile_worktrees()` — sync worktree state with worker state | Todo | - |
| T3.1.7 | Extract Phase 7: `dispatch_commands()` — execute `RuntimeCommand` effects | Todo | - |
| T3.1.8 | Write unit tests for each phase in isolation | Todo | - |

#### Acceptance Criteria

- `tick()` body is under 50 lines, delegating to phase methods
- Each phase method has a clear doc comment explaining its contract
- Phase execution order is explicit and documented
- All existing integration tests pass unchanged

---

### Story 3.2: Define `RuntimeCommand` Effect System

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Create a type representing side effects that `Worker::apply()` can request. This decouples state transitions from I/O.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Define `RuntimeCommand` enum: `SpawnProvider { bundle }`, `SendToProvider { event }`, `RequestDecision { situation }`, `NotifyUser { message }`, `UpdateWorktree { path, branch }`, `Terminate { reason }` | Todo | - |
| T3.2.2 | Define `RuntimeCommandQueue` — ordered command buffer | Todo | - |
| T3.2.3 | Implement `Worker::apply()` returning `Result<Vec<RuntimeCommand>, WorkerError>` | Todo | - |
| T3.2.4 | Map each `DomainEvent` variant to appropriate `RuntimeCommand` set | Todo | - |
| T3.2.5 | Write unit tests verifying `apply()` returns expected commands for each event | Todo | - |

#### Acceptance Criteria

- `Worker::apply()` is pure (no I/O, no thread spawning)
- Every event variant maps to at least one `RuntimeCommand`
- `RuntimeCommandQueue` preserves command order
- Effect system is independently unit-testable

---

### Story 3.3: Implement Effect Handlers

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Create the bridge between pure `RuntimeCommand` effects and actual I/O/threads.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Implement `SpawnProviderHandler` — creates provider thread, wires event channel | Todo | - |
| T3.3.2 | Implement `SendToProviderHandler` — sends event to provider TX channel | Todo | - |
| T3.3.3 | Implement `RequestDecisionHandler` — creates decision request, routes to decision agent | Todo | - |
| T3.3.4 | Implement `NotifyUserHandler` — emits user-facing notification via TUI event bus | Todo | - |
| T3.3.5 | Implement `UpdateWorktreeHandler` — delegates to `WorktreeCoordinator` | Todo | - |
| T3.3.6 | Implement `TerminateHandler` — graceful agent shutdown | Todo | - |
| T3.3.7 | Write integration tests for each handler | Todo | - |
| T3.3.8 | Ensure all handlers are idempotent (safe to retry) | Todo | - |

#### Acceptance Criteria

- Each handler is a separate struct with a single `execute()` method
- Handler execution failures are logged, not panicked
- Handlers are idempotent (calling twice is safe)
- Integration tests verify end-to-end effect execution

---

### Story 3.4: Integrate Worker as Primary State Authority

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Switch from `AgentSlot` as primary state to `Worker` as primary state. Remove dual-write.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Replace `AgentSlot` state mutations with `worker.apply()` calls | Todo | - |
| T3.4.2 | Make `AgentSlot` a thin wrapper around `Worker` (data + thread handle only) | Todo | - |
| T3.4.3 | Update `AgentPool` to interact with `Worker` state via `AgentSlot` wrapper | Todo | - |
| T3.4.4 | Update `tick()` phases to use `Worker` methods instead of direct `AgentSlot` field access | Todo | - |
| T3.4.5 | Remove `AgentSlot` event handler code that duplicated `Worker::apply()` logic | Todo | - |
| T3.4.6 | Run full integration test suite | Todo | - |

#### Acceptance Criteria

- `AgentSlot` contains only `Worker`, thread handle, and event channels
- All state transitions go through `Worker::apply()`
- `AgentSlot` tests reduced to thread-lifecycle tests only
- All integration tests pass

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Phase extraction breaks timing assumptions | Medium | High | Integration tests with real timing; add explicit phase ordering invariants |
| Effect handler idempotency bugs | Medium | High | Test double-execution scenarios; deduplicate by command ID |
| Worker::apply() returns wrong commands | Low | High | Property-based testing for command-event mapping |

## Sprint Deliverables

- Refactored `tick()` with 7 phase methods
- `core/src/runtime_command.rs` — `RuntimeCommand` enum and queue
- `daemon/src/effect_handlers/` — One handler per command variant
- `AgentSlot` as thin `Worker` wrapper
- All integration tests passing

## Dependencies

- [Sprint 1: Shared Kernel Extraction](./sprint-01-shared-kernel.md) — `DomainEvent` required
- [Sprint 2: Worker Aggregate Root](./sprint-02-worker-aggregate-root.md) — `Worker` and `WorkerState` required

## Next Sprint

After completing this sprint, proceed to [Sprint 4: Decision Layer Decoupling](./sprint-04-decision-layer-decoupling.md) for separating the decision system from runtime writes.
