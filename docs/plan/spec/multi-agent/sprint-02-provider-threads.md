# Sprint 2: Provider Threads

## Metadata

- Sprint ID: `sprint-002`
- Title: `Provider Threads`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1

## Sprint Goal

Integrate real providers (Claude, Codex) into AgentPool with proper threading. Multiple providers can run simultaneously without blocking.

## Stories

### Story 2.1: Provider Thread Handle

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Create `ProviderThreadHandle` for managing provider thread lifecycle.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Create `ProviderThreadHandle` struct | Todo | - |
| T2.1.2 | Add thread name field for debugging | Todo | - |
| T2.1.3 | Add started_at timestamp field | Todo | - |
| T2.1.4 | Implement graceful thread shutdown | Todo | - |
| T2.1.5 | Write unit tests for thread handle | Todo | - |

#### Technical Notes

```rust
pub struct ProviderThreadHandle {
    handle: std::thread::JoinHandle<()>,
    event_tx: mpsc::Sender<ProviderEvent>,
    thread_name: String,
    started_at: Instant,
}
```

---

### Story 2.2: Provider Thread Spawner

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement `start_provider_thread()` for Claude and Codex.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Create `start_provider_thread()` function | Todo | - |
| T2.2.2 | Configure thread name with agent_id | Todo | - |
| T2.2.3 | Spawn thread with Builder for named threads | Todo | - |
| T2.2.4 | Wire Claude provider through thread spawner | Todo | - |
| T2.2.5 | Wire Codex provider through thread spawner | Todo | - |
| T2.2.6 | Add provider thread lifecycle logging | Todo | - |
| T2.2.7 | Write integration tests for Claude threading | Todo | - |
| T2.2.8 | Write integration tests for Codex threading | Todo | - |

#### Acceptance Criteria

- Provider starts in named thread
- Events flow through channel to correct agent
- Thread join works cleanly on stop

---

### Story 2.3: Thread Safety Model

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Enforce thread safety guarantees: provider threads never mutate shared state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Document thread safety rules in code comments | Todo | - |
| T2.3.2 | Add debug assertions for thread ownership | Todo | - |
| T2.3.3 | Ensure provider only reads cwd, sends events | Todo | - |
| T2.3.4 | Add thread safety tests | Todo | - |

#### Thread Safety Rules

1. Provider threads NEVER directly mutate shared state
2. All state mutations happen in main thread after receiving events
3. Channel communication is the ONLY cross-thread data transfer
4. File persistence uses per-agent directories
5. Backlog uses Mutex for interior mutability

---

### Story 2.4: Graceful Thread Cancellation

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement clean thread cancellation without force-kill.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Drop event_tx to signal thread | Todo | - |
| T2.4.2 | Implement join with timeout | Todo | - |
| T2.4.3 | Log warning if thread doesn't finish in time | Todo | - |
| T2.4.4 | Handle thread panic gracefully | Todo | - |
| T2.4.5 | Write tests for cancellation | Todo | - |

#### Acceptance Criteria

- Thread finishes within timeout on stop
- No resource leaks on cancellation
- Panic is caught and logged

---

### Story 2.5: Multi-Provider Concurrent Execution

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Test and validate concurrent Claude + Codex execution.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.5.1 | Spawn Claude agent and Codex agent simultaneously | Todo | - |
| T2.5.2 | Verify both channels receive events | Todo | - |
| T2.5.3 | Verify no blocking between providers | Todo | - |
| T2.5.4 | Write stress test for concurrent execution | Todo | - |
| T2.5.5 | Write integration test for event ordering | Todo | - |

#### Acceptance Criteria

- Two providers run without blocking each other
- Events arrive in correct channels
- No race conditions or deadlocks

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Provider threading complexity | High | High | Start with mock, incrementally add real providers |
| Thread join timeout | Medium | Medium | Configurable timeout, log warnings |
| Channel buffer overflow | Low | Medium | Adequate buffer size, non-blocking recv |

## Sprint Deliverables

- `core/src/provider_thread.rs` - Provider thread management
- Modified `provider.rs` for thread spawning
- Integration tests for concurrent execution

## Dependencies

- Sprint 1: Foundation (AgentSlot, AgentPool, EventAggregator)

## Next Sprint

After completing this sprint, proceed to [Sprint 3: Task Distribution](./sprint-03-task-distribution.md).