# Sprint 2: Worker Aggregate Root

## Metadata

- Sprint ID: `sref-002`
- Title: `Worker Aggregate Root`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Completed`
- Created: 2026-04-22

## Background

The current `AgentSlot` (in `core/src/agent_slot.rs`) is a data bag with 16 fields and ad-hoc state mutations scattered across `SessionManager`. There is no single method encapsulating "apply an event to this agent's state." Instead, each event type has its own mutation path through multiple files. This makes reasoning about state changes impossible without reading the entire codebase.

This sprint implements the core of the "Handwritten Actor" model: `Worker` as an aggregate root that encapsulates all mutable state and exposes a single `apply(event) -> Result<(), WorkerError>` method. The `WorkerState` enum provides explicit state machine semantics with compile-time guards for invalid transitions.

**Key insight from the refactoring plan**: We are not adopting a real Actor framework. We are making the handwritten event loop honest by giving the core domain object (Worker) a clear contract.

## Sprint Goal

Implement `Worker` aggregate root with `WorkerState` state machine, `apply()` method for all 24 event variants, and comprehensive unit tests. Existing `AgentSlot` remains functional alongside `Worker` (dual-write during transition).

## Stories

### Story 2.1: Define `WorkerState` State Machine

**Priority**: P0
**Effort**: 3 points
**Status**: Completed ✅ — `WorkerState` with 6 variants + `RespondingSubState` in `agent-runtime-domain`. Note: `AgentSlotStatus` (13-variant) still exists in `agent-core` for backward compatibility.

Replace the implicit state transitions in `AgentSlot` with an explicit enum.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Define `WorkerState` enum: `Starting`, `Responding`, `ProcessingTool`, `Completed`, `Failed`, `Idle` | Todo | - |
| T2.1.2 | Define sub-state for `Responding: { Streaming, WaitingConfirmation }` | Todo | - |
| T2.1.3 | Implement `can_transition_to()` method with explicit guard rules | Todo | - |
| T2.1.4 | Implement `Display` for human-readable state names | Todo | - |
| T2.1.5 | Write unit tests for all valid state transitions | Todo | - |
| T2.1.6 | Write unit tests for all invalid state transitions (should panic or return error) | Todo | - |

#### Acceptance Criteria

- `WorkerState` has 6 top-level variants with `Responding` containing sub-state
- `can_transition_to(Idle, Responding)` returns `false` (no backward jumps)
- `can_transition_to(Starting, Responding)` returns `true`
- `can_transition_to(Starting, Starting)` returns `false` (no self-loops)
- `can_transition_to(ProcessingTool, Failed)` returns `true`
- All transitions have unit test coverage

---

### Story 2.2: Implement `Worker` Aggregate Root

**Priority**: P0
**Effort**: 5 points
**Status**: Completed ✅ — `Worker` struct with `apply(event) -> Result<Vec<RuntimeCommand>, WorkerError>` in `agent-core`.

Create the `Worker` struct as the single authority over all mutable state for one agent.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Define `Worker` struct with fields: `agent_id`, `codename`, `role`, `state: WorkerState`, `transcript: TranscriptJournal`, `worktree: Option<PathBuf>`, `assigned_task_id: Option<TaskId>` | Todo | - |
| T2.2.2 | Implement constructor `Worker::new(id, codename, role)` | Todo | - |
| T2.2.3 | Implement `apply()` method with match over all `DomainEvent` variants | Todo | - |
| T2.2.4 | Lifecycle events update `WorkerState` via validated transitions | Todo | - |
| T2.2.5 | Streaming events append to `TranscriptJournal` | Todo | - |
| T2.2.6 | Tool/MCP/Web events update state to `ProcessingTool` | Todo | - |
| T2.2.7 | Error/Failure events transition to `Failed` state | Todo | - |
| T2.2.8 | Implement `WorkerError` enum for invalid transitions and invariant violations | Todo | - |
| T2.2.9 | Write unit tests for `apply()` on each event category | Todo | - |
| T2.2.10 | Write unit tests for invariant preservation (e.g., cannot assign task to non-Idle worker) | Todo | - |

#### Acceptance Criteria

- `Worker::apply(event)` compiles for all 24 `DomainEvent` variants
- Invalid state transitions return `WorkerError::InvalidTransition { from, to }`
- Worker invariants are maintained after every `apply` call
- 100% event variant coverage in `apply()` tests

---

### Story 2.3: Extract `TranscriptJournal`

**Priority**: P1
**Effort**: 3 points
**Status**: Completed ✅ — `TranscriptJournal` and `JournalEntry` moved to `agent-runtime-domain`.

Move transcript management out of `Worker` into a dedicated type that can be tested independently.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Define `TranscriptJournal` struct with `entries: Vec<JournalEntry>` | Todo | - |
| T2.3.2 | Define `JournalEntry` enum: `UserInput { text }`, `AssistantResponse { chunks }`, `ToolCall { name, args, result }`, `SystemEvent { event }` | Todo | - |
| T2.3.3 | Implement `append(event)` for all event variants | Todo | - |
| T2.3.4 | Implement `last_n(n: usize) -> &[JournalEntry]` for decision context | Todo | - |
| T2.3.5 | Implement `tool_calls() -> Vec<&ToolCallEntry>` for retry extraction | Todo | - |
| T2.3.6 | Implement `to_decision_context() -> DecisionContext` | Todo | - |
| T2.3.7 | Write unit tests for transcript operations | Todo | - |
| T2.3.8 | Write unit tests for `to_decision_context()` formatting | Todo | - |

#### Acceptance Criteria

- `TranscriptJournal` can record all 24 event variants as structured entries
- `last_n(3)` returns the last 3 entries in chronological order
- `to_decision_context()` produces a text summary suitable for LLM prompts
- Tool call retry information is extractable from the journal

---

### Story 2.4: Dual-Write Transition Bridge

**Priority**: P1
**Effort**: 2 points
**Status**: Completed ✅ — `WorkerHandle` (alias `AgentSlot`) contains `Worker` field; events forwarded to `worker.apply()`.

Keep `AgentSlot` functional while introducing `Worker` alongside it. This allows gradual migration without breaking existing features.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Add `worker: Option<Worker>` field to `AgentSlot` | Todo | - |
| T2.4.2 | In `AgentSlot` event handlers, forward events to `worker.apply()` after own processing | Todo | - |
| T2.4.3 | Add `AgentSlot::worker()` accessor for new code | Todo | - |
| T2.4.4 | Ensure all existing `AgentSlot` tests still pass | Todo | - |
| T2.4.5 | Add integration test verifying `AgentSlot` and `Worker` states stay synchronized | Todo | - |

#### Acceptance Criteria

- All existing `AgentSlot` tests pass unchanged
- `Worker` receives the same events as `AgentSlot` in the same order
- `AgentSlot::worker()` returns `Some` when the worker has been initialized
- No production code relies on `Worker` yet (it's shadow-initialized)

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| State machine too rigid | Medium | Medium | Allow explicit `force_transition()` for error recovery paths, logged as warnings |
| `TranscriptJournal` performance with large sessions | Low | Medium | Use ring buffer for entries; implement lazy context generation |
| Dual-write overhead | Low | Low | Worker operations are cheap; remove dual-write in Sprint 3 |

## Sprint Deliverables

- `core/src/worker.rs` — `Worker` aggregate root
- `core/src/worker_state.rs` — `WorkerState` state machine
- `core/src/transcript_journal.rs` — `TranscriptJournal`
- Dual-write bridge in `AgentSlot`
- Comprehensive unit tests for state transitions and invariants

## Dependencies

- [Sprint 1: Shared Kernel Extraction](./sprint-01-shared-kernel.md) — `DomainEvent` must be available in `agent-events`

## Next Sprint

After completing this sprint, proceed to [Sprint 3: EventLoop Refactoring](./sprint-03-event-loop-refactoring.md) for the handwritten event loop cleanup.
