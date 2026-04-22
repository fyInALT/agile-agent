# Sprint 8: WorkerState Synchronization

## Metadata

- Sprint ID: `sref-008`
- Title: `WorkerState Synchronization`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends on: `sref-002` (Worker Aggregate Root), `sref-003` (EventLoop Refactoring)

---

## Background

### The Dual-Status Problem

The multi-agent runtime currently maintains **two parallel state representations** for every agent:

```
┌─────────────────────────────────────────────────────────────┐
│                      AgentSlot                               │
│                                                              │
│  ┌──────────────────────┐    ┌──────────────────────────┐  │
│  │ AgentSlotStatus      │    │ WorkerState              │  │
│  │ (13 variants)        │    │ (6 variants)             │  │
│  │                      │    │                          │  │
│  │ - Idle               │    │ - Starting               │  │
│  │ - Starting           │    │ - Responding             │  │
│  │ - Responding         │    │ - ProcessingTool         │  │
│  │ - ToolExecuting      │    │ - Completed              │  │
│  │ - Finishing          │    │ - Failed                 │  │
│  │ - Stopping           │    │ - Idle                   │  │
│  │ - Stopped            │    │                          │  │
│  │ - Error              │    │                          │  │
│  │ - Blocked            │◄───┤  MISSING!                │  │
│  │ - BlockedForDecision │◄───┤  MISSING!                │  │
│  │ - Paused             │◄───┤  MISSING!                │  │
│  │ - WaitingForInput    │◄───┤  MISSING!                │  │
│  │ - Resting            │◄───┤  MISSING!                │  │
│  └──────────────────────┘    └──────────────────────────┘  │
│           ▲                            ▲                     │
│           │                            │                     │
│     Legacy path                 Dual-write bridge            │
│     (direct mutation)           (Worker::apply)              │
└─────────────────────────────────────────────────────────────┘
```

### Why This Is a Critical Problem

**1. Authority Ambiguity**

`AgentSlot::apply_provider_event_to_worker()` claims "Worker's state becomes the primary authority," but `WorkerState` cannot express 7 of the 13 runtime states. When the legacy path sets `BlockedForDecision` or `Resting`, the `Worker` is completely unaware.

**2. Event Rejection Bugs**

`Worker::apply()` validates events against `WorkerState`. If the legacy path has set `AgentSlotStatus` to `Blocked`, but `WorkerState` is still `Responding`, subsequent provider events that would be valid in `Responding` may be rejected by `Worker::apply()` with `InvalidEventForState`:

```rust
// Scenario:
// 1. Agent is Responding
// 2. Legacy path sets AgentSlotStatus::BlockedForDecision (decision triggered)
// 3. Provider emits AssistantChunk (valid for Responding, invalid for Blocked)
// 4. Worker::apply() sees WorkerState::Responding → accepts event ✓
// 5. But slot status says BlockedForDecision → inconsistent!
//
// Reverse scenario:
// 1. WorkerState::Idle (via apply_provider_event_to_worker)
// 2. Legacy path sets AgentSlotStatus::BlockedForDecision
// 3. Provider emits AssistantChunk
// 4. Worker::apply() sees WorkerState::Idle → rejects event ✗
// 5. Event is lost, transcript incomplete
```

**3. Phase 7 Command Corruption**

Phase 7 collects `RuntimeCommand`s from `Worker::apply()`. If `WorkerState` is out of sync with `AgentSlotStatus`, the commands produced may contradict the slot's actual state:

```rust
// AgentSlotStatus says Blocked (waiting for human)
// WorkerState says Idle (doesn't know about Blocked)
// Phase 7 collects: no commands (Idle produces nothing)
// But maybe it SHOULD produce RequestDecision?
```

**4. Snapshot/Restore Inconsistency**

Shutdown snapshots save `AgentSlotStatus`. On restore, `WorkerState` is recreated from scratch (`Worker::new()`). If `AgentSlotStatus` is `Resting`, the restored `Worker` starts at `Idle`. The agent may immediately try to work instead of waiting for rate limit recovery.

### Root Cause

`WorkerState` was designed as a **domain model** (focused on the agent's lifecycle: start → respond → tool → complete/fail). `AgentSlotStatus` evolved as an **operational model** (handling all edge cases: blocking, pausing, resting, waiting for input).

The domain model is too simple for the operational reality. The dual-write bridge (`apply_provider_event_to_worker`) only works for happy-path events, not for decision-layer-triggered states.

---

## Sprint Goal

Extend `WorkerState` to cover all runtime states, establish a bidirectional synchronization mechanism between `AgentSlotStatus` and `WorkerState`, and ensure `Worker::apply()` never rejects an event due to stale state. All existing tests pass; no behavioral regressions.

---

## Stories

### Story 8.1: Extend WorkerState with Missing Operational States

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Add `Blocked`, `Paused`, `WaitingForInput`, and `Resting` variants to `WorkerState`, plus sub-state distinctions for `Finishing` and `Stopping`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.1.1 | Add `Blocked { reason: String }` variant to `WorkerState` | Todo | - |
| T8.1.2 | Add `Paused { reason: String }` variant to `WorkerState` | Todo | - |
| T8.1.3 | Add `WaitingForInput` variant to `WorkerState` | Todo | - |
| T8.1.4 | Add `Resting { until: Option<DateTime<Utc>> }` variant to `WorkerState` | Todo | - |
| T8.1.5 | Distinguish `Finishing` as a sub-state of `Responding` or a standalone variant | Todo | - |
| T8.1.6 | Distinguish `Stopping` as a transitional variant | Todo | - |
| T8.1.7 | Update `WorkerState::label()` to cover all new variants | Todo | - |
| T8.1.8 | Update `WorkerState::is_active()`, `is_terminal()`, `is_idle()`, `is_failed()` | Todo | - |
| T8.1.9 | Add constructor helpers: `blocked()`, `paused()`, `waiting_for_input()`, `resting()` | Todo | - |
| T8.1.10 | Write unit tests for all new variant construction and classification | Todo | - |

#### Acceptance Criteria

- `WorkerState` has at least 11 variants (up from 6), covering all `AgentSlotStatus` states
- All classification helpers (`is_active`, `is_terminal`, etc.) handle new variants correctly
- `label()` returns human-readable strings for all variants
- New variants are `Clone + PartialEq + Eq` (needed for event sourcing)

#### Technical Notes

```rust
pub enum WorkerState {
    // Existing variants
    Starting,
    Responding { sub: RespondingSubState },
    ProcessingTool { name: String },
    Completed,
    Failed { reason: String },
    Idle,

    // NEW: Operational states previously missing
    /// Agent is blocked awaiting external input or decision
    Blocked { reason: String },

    /// Agent is paused with worktree preserved
    Paused { reason: String },

    /// Agent is waiting for user input within a response
    WaitingForInput,

    /// Agent is resting due to rate limit (HTTP 429)
    Resting { until: Option<DateTime<Utc>> },

    /// Agent is finishing its current work (transitional)
    Finishing,

    /// Agent is being stopped gracefully (transitional)
    Stopping,
}
```

---

### Story 8.2: Update WorkerState Transition Rules

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Extend `can_transition_to()` to support valid transitions into and out of the new operational states.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.2.1 | Define valid transitions for `Blocked` (entry from any active state, exit to Idle/Responding/Starting) | Todo | - |
| T8.2.2 | Define valid transitions for `Paused` (entry from any state, exit to previous state) | Todo | - |
| T8.2.3 | Define valid transitions for `WaitingForInput` (entry from Responding, exit to Responding) | Todo | - |
| T8.2.4 | Define valid transitions for `Resting` (entry from any active state, exit to Idle/Starting) | Todo | - |
| T8.2.5 | Define valid transitions for `Finishing` (entry from Responding/ProcessingTool, exit to Completed/Failed) | Todo | - |
| T8.2.6 | Define valid transitions for `Stopping` (entry from any state, exit to Completed/Failed/Idle) | Todo | - |
| T8.2.7 | Ensure no self-loops (except Paused→Paused if reason changes?) | Todo | - |
| T8.2.8 | Write exhaustive transition matrix tests (all pairs of states) | Todo | - |
| T8.2.9 | Document transition rationale for each new rule | Todo | - |

#### Acceptance Criteria

- Every `AgentSlotStatus → AgentSlotStatus` transition has a corresponding `WorkerState → WorkerState` transition
- Invalid transitions are rejected with descriptive `InvalidTransition` errors
- Transition matrix tests cover all 11×11 = 121 state pairs
- Forward-only progression is maintained where appropriate; recovery paths are explicit

#### Technical Notes

Key design decisions to make:

```rust
// Should Blocked allow self-transition when reason changes?
// Proposal: NO — reason changes should be modelled as Blocked→Blocked
// but can_transition_to disallows self-loops.
// Alternative: add BlockedState sub-structure and transition within it.

// Should Paused remember the previous state?
// Proposal: NO at WorkerState level — AgentSlot can store prev_status separately
// WorkerState should remain a pure state machine, not a history stack.

// Resting → Starting (rate limit recovered, restart work)
// Resting → Idle (rate limit recovered, no pending work)
```

---

### Story 8.3: Implement AgentSlotStatus → WorkerState Mapping

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Create an explicit, testable mapping function from `AgentSlotStatus` to `WorkerState`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.3.1 | Implement `AgentSlotStatus::to_worker_state(&self) -> WorkerState` | Todo | - |
| T8.3.2 | Handle `Blocked { reason }` → `WorkerState::blocked(reason)` | Todo | - |
| T8.3.3 | Handle `BlockedForDecision { blocked_state }` → `WorkerState::blocked(reason_type)` | Todo | - |
| T8.3.4 | Handle `Paused { reason }` → `WorkerState::paused(reason)` | Todo | - |
| T8.3.5 | Handle `WaitingForInput { .. }` → `WorkerState::waiting_for_input()` | Todo | - |
| T8.3.6 | Handle `Resting { started_at, blocked_state, on_resume }` → `WorkerState::resting(until)` | Todo | - |
| T8.3.7 | Handle `Finishing` → `WorkerState::finishing()` | Todo | - |
| T8.3.8 | Handle `Stopping` → `WorkerState::stopping()` | Todo | - |
| T8.3.9 | Write unit tests for every mapping variant | Todo | - |

#### Acceptance Criteria

- Every `AgentSlotStatus` variant maps to exactly one `WorkerState` variant
- Mapping is deterministic and pure (no side effects)
- Information loss is documented (e.g., `BlockedForDecision` rich context → `WorkerState::blocked(reason_type)`)
- All edge cases tested (e.g., `Stopped { reason }` → `Completed` or `Failed` based on reason content)

#### Technical Notes

```rust
impl AgentSlotStatus {
    pub fn to_worker_state(&self) -> WorkerState {
        match self {
            Self::Idle => WorkerState::idle(),
            Self::Starting => WorkerState::starting(),
            Self::Responding { .. } => WorkerState::responding_streaming(),
            Self::ToolExecuting { tool_name } => WorkerState::processing_tool(tool_name),
            Self::Finishing => WorkerState::finishing(),
            Self::Stopping => WorkerState::stopping(),
            Self::Stopped { reason } => {
                // Heuristic: "error" in reason → Failed, otherwise Completed
                if reason.to_lowercase().contains("error")
                    || reason.to_lowercase().contains("fail")
                {
                    WorkerState::failed(reason.clone())
                } else {
                    WorkerState::completed()
                }
            }
            Self::Error { message } => WorkerState::failed(message.clone()),
            Self::Blocked { reason } => WorkerState::blocked(reason.clone()),
            Self::BlockedForDecision { blocked_state } => {
                WorkerState::blocked(blocked_state.reason().reason_type().to_string())
            }
            Self::Paused { reason } => WorkerState::paused(reason.clone()),
            Self::WaitingForInput { .. } => WorkerState::waiting_for_input(),
            Self::Resting { blocked_state, .. } => {
                WorkerState::resting(blocked_state.reason().expires_at())
            }
        }
    }
}
```

---

### Story 8.4: Integrate State Synchronization into AgentSlot

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Modify `AgentSlot::transition_to()` to sync `WorkerState` whenever `AgentSlotStatus` changes.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.4.1 | After `AgentSlot::transition_to()` succeeds, compute target `WorkerState` via mapping | Todo | - |
| T8.4.2 | Attempt `self.worker.state.transition_to(target_worker_state)` | Todo | - |
| T8.4.3 | On sync failure: log warning with both states, but do NOT roll back `AgentSlotStatus` | Todo | - |
| T8.4.4 | Handle the "reverse sync" case: `apply_provider_event_to_worker()` updates `WorkerState`, derive `AgentSlotStatus` | Todo | - |
| T8.4.5 | Ensure `Worker::apply()` never rejects events due to stale state after sync | Todo | - |
| T8.4.6 | Write integration tests: `transition_to(Blocked)` → `WorkerState::Blocked` | Todo | - |
| T8.4.7 | Write integration tests: `apply_provider_event_to_worker()` after `transition_to(Blocked)` | Todo | - |

#### Acceptance Criteria

- Every successful `AgentSlotStatus` transition attempts a corresponding `WorkerState` transition
- Sync failures are logged but non-blocking (operational state is authoritative)
- `Worker::apply()` acceptance rate is 100% for events arriving after any `transition_to()`
- No deadlocks: sync happens within the same `&mut self` borrow, no locking

#### Technical Notes

**Critical design choice**: When `WorkerState` transition fails but `AgentSlotStatus` transition succeeds, which is authoritative?

**Answer**: `AgentSlotStatus` is authoritative. `WorkerState` is a "best-effort domain model." The rationale:
- Legacy code and external triggers set `AgentSlotStatus` directly
- Rolling back would break existing callers
- Logging the mismatch allows developers to fix the mapping

```rust
pub fn transition_to(&mut self, new_status: AgentSlotStatus) -> Result<(), String> {
    // ... existing validation and self.status = new_status ...

    // NEW: Sync WorkerState
    let target_worker_state = new_status.to_worker_state();
    if let Err(e) = self.worker.state().transition_to(target_worker_state.clone()) {
        // Log but do not fail — AgentSlotStatus is authoritative
        logging::warn_event(
            "slot.worker_sync_failed",
            "WorkerState transition failed after AgentSlotStatus change",
            serde_json::json!({
                "agent_id": self.agent_id.as_str(),
                "slot_status": new_status.label(),
                "target_worker_state": format!("{:?}", target_worker_state),
                "error": e.to_string(),
            }),
        );
    } else {
        // Apply the transition (WorkerState is not self-syncing, we need to mutate)
        // NOTE: This requires Worker to expose a mutable state setter or
        //       transition_to must return the new state which we assign.
    }

    Ok(())
}
```

**Problem**: `WorkerState` is currently immutable (transition_to returns `Result<WorkerState, _>` but the caller must assign it). `Worker` stores `state: WorkerState`. We need either:
- `Worker::set_state(new_state)` — direct setter (breaks encapsulation)
- `Worker::transition_state(to) -> Result<(), WorkerError>` — validates and assigns

Preferred: add `Worker::transition_state(to) -> Result<(), WorkerError>`.

---

### Story 8.5: Validate Dual-Write Bridge Consistency

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

After state sync is implemented, verify that the dual-write bridge (`apply_provider_event_to_worker`) never produces `RuntimeCommand`s that contradict the slot's operational state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.5.1 | Create `DualWriteConsistency` test harness | Todo | - |
| T8.5.2 | Test: Agent in `Blocked` status + `Finished` event → what does Worker do? | Todo | - |
| T8.5.3 | Test: Agent in `Resting` status + `AssistantChunk` event → what does Worker do? | Todo | - |
| T8.5.4 | Test: Agent in `Paused` status + `ExecCommandStarted` event → what does Worker do? | Todo | - |
| T8.5.5 | Audit all `RuntimeCommand` produced by `Worker::apply()` for each operational state | Todo | - |
| T8.5.6 | Document expected command matrix: `WorkerState` × `DomainEvent` → `Vec<RuntimeCommand>` | Todo | - |

#### Acceptance Criteria

- Every `AgentSlotStatus` × `DomainEvent` combination produces predictable `RuntimeCommand`s
- No `WorkerError::InvalidEventForState` occurs after a valid `transition_to()`
- Command matrix is documented and tested

#### Technical Notes

The command matrix is the ultimate contract for the dual-write bridge:

```
                    │ Finished │ Error │ AssistantChunk │ ExecCommandStarted │ ...
────────────────────┼──────────┼───────┼────────────────┼────────────────────┼─────
Idle                │  ?       │  ?    │  ?             │  ?                 │
Responding          │ Terminate│ Notify│  []            │  []                │
Blocked             │  ?       │  ?    │  ?             │  ?                 │
Resting             │  ?       │  ?    │  ?             │  ?                 │
```

Some cells may need design decisions (e.g., should a `Finished` event while `Blocked` produce `Terminate` or be ignored?).

---

## Dependency Graph

```
Story 8.1 (Extend WorkerState)
    │
    └──► Story 8.2 (Transition Rules)
              │
              ├──► Story 8.3 (Mapping Function)
              │         │
              │         └──► Story 8.4 (AgentSlot Integration)
              │                   │
              │                   └──► Story 8.5 (Validation)
              │
              └──► Story 8.5 (Transition matrix can be drafted in parallel)
```

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `WorkerState` expansion breaks existing `apply()` assumptions | High | High | Exhaustive test matrix; add `Worker::transition_state()` instead of direct mutation |
| `AgentSlotStatus` → `WorkerState` mapping loses critical context | Medium | Medium | Document information loss; accept that domain model is simpler than operational model |
| Self-loop rule (no `state → same state`) conflicts with reason-only updates | Medium | Medium | Allow reason updates via separate method, not `transition_to()` |
| Shutdown/restore path uses stale `WorkerState` | Medium | High | On restore, reconstruct `WorkerState` from saved `AgentSlotStatus` using mapping |

## Definition of Done

- [ ] `WorkerState` covers all `AgentSlotStatus` variants
- [ ] `AgentSlot::transition_to()` synchronizes `WorkerState` on every call
- [ ] `Worker::apply()` never rejects events due to stale state
- [ ] `Worker::transition_state()` is the only way to mutate `WorkerState` (no direct setters)
- [ ] Transition matrix tests cover all `WorkerState` × `DomainEvent` pairs
- [ ] `cargo clippy --workspace --tests -- -D warnings` passes
- [ ] `cargo test --workspace --lib` passes with zero failures
