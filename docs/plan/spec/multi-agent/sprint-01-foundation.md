# Sprint 1: Foundation

## Metadata

- Sprint ID: `sprint-001`
- Title: `Foundation`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-13

## Sprint Goal

Establish core data structures for multi-agent runtime without changing existing TUI behavior. All components are tested with mock providers, ready for real provider integration in Sprint 2.

## Stories

### Story 1.1: AgentSlot and AgentSlotStatus

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the `AgentSlot` struct representing a single agent's runtime slot.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `AgentId` type with unique identifier logic | Todo | - |
| T1.1.2 | Create `AgentCodename` type (alpha, bravo, charlie...) | Todo | - |
| T1.1.3 | Create `AgentSlotStatus` enum with all states | Todo | - |
| T1.1.4 | Create `AgentSlot` struct with all fields | Todo | - |
| T1.1.5 | Implement `AgentSlot::new()` constructor | Todo | - |
| T1.1.6 | Implement status transition methods | Todo | - |
| T1.1.7 | Write unit tests for AgentSlot | Todo | - |
| T1.1.8 | Write unit tests for status transitions | Todo | - |

#### Acceptance Criteria

- `AgentSlot` can be created with unique ID and codename
- Status transitions follow valid state machine
- All transitions are tested with edge cases

#### Technical Notes

```rust
pub struct AgentSlot {
    agent_id: AgentId,
    codename: AgentCodename,
    provider_type: ProviderType,
    status: AgentSlotStatus,
    session_handle: Option<SessionHandle>,
    transcript: Vec<TranscriptEntry>,
    assigned_task_id: Option<TaskId>,
    event_rx: Option<mpsc::Receiver<ProviderEvent>>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    last_activity: Instant,
}

pub enum AgentSlotStatus {
    Idle,
    Starting,
    Responding { started_at: Instant },
    ToolExecuting { tool_name: String },
    Finishing,
    Stopped { reason: String },
    Error { message: String },
}
```

---

### Story 1.2: AgentPool

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the `AgentPool` struct for managing multiple agent slots.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Create `AgentPool` struct with slots vector | Todo | - |
| T1.2.2 | Implement `AgentPool::new()` with max_slots config | Todo | - |
| T1.2.3 | Implement `AgentPool::spawn_agent()` for mock provider | Todo | - |
| T1.2.4 | Implement `AgentPool::stop_agent()` | Todo | - |
| T1.2.5 | Implement `AgentPool::agent_statuses()` snapshot | Todo | - |
| T1.2.6 | Implement `AgentPool::focus_agent()` switch | Todo | - |
| T1.2.7 | Implement `AgentPool::get_slot()` by index and ID | Todo | - |
| T1.2.8 | Write unit tests for spawn/stop lifecycle | Todo | - |
| T1.2.9 | Write unit tests for focus switching | Todo | - |
| T1.2.10 | Write unit tests for status snapshots | Todo | - |

#### Acceptance Criteria

- AgentPool can spawn up to max_slots agents
- Each agent gets unique ID and codename
- Stop marks agent as stopped cleanly
- Focus switching works without errors

---

### Story 1.3: SharedWorkplaceState

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Extract shared state from `AppState` into `SharedWorkplaceState`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Create `SharedWorkplaceState` struct | Todo | - |
| T1.3.2 | Move `workplace_id` from AppState to SharedWorkplaceState | Todo | - |
| T1.3.3 | Move `backlog` from AppState to SharedWorkplaceState | Todo | - |
| T1.3.4 | Move `skills` from AppState to SharedWorkplaceState | Todo | - |
| T1.3.5 | Move loop control flags to SharedWorkplaceState | Todo | - |
| T1.3.6 | Add `Arc<SharedWorkplaceState>` wrapper type | Todo | - |
| T1.3.7 | Write unit tests for shared state | Todo | - |
| T1.3.8 | Refactor existing AppState to use SharedWorkplaceState | Todo | - |

#### Acceptance Criteria

- SharedWorkplaceState contains all shared fields
- AppState reduced to per-agent state
- Existing tests pass after refactor

---

### Story 1.4: EventAggregator

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement non-blocking multi-channel polling for provider events.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Create `AgentEvent` enum wrapping ProviderEvent | Todo | - |
| T1.4.2 | Create `EventAggregator` struct | Todo | - |
| T1.4.3 | Implement `EventAggregator::add_receiver()` | Todo | - |
| T1.4.4 | Implement `EventAggregator::remove_receiver()` | Todo | - |
| T1.4.5 | Implement `EventAggregator::poll_all()` non-blocking | Todo | - |
| T1.4.6 | Implement `EventAggregator::poll_with_timeout()` | Todo | - |
| T1.4.7 | Write unit tests with mock channels | Todo | - |
| T1.4.8 | Write unit tests for timeout behavior | Todo | - |

#### Acceptance Criteria

- Polls all channels without blocking
- Returns events tagged with agent_id
- Timeout prevents infinite wait

#### Technical Notes

```rust
pub enum AgentEvent {
    FromAgent { agent_id: AgentId, event: ProviderEvent },
    StatusChanged { agent_id: AgentId, old_status: AgentSlotStatus, new_status: AgentSlotStatus },
    TaskCompleted { agent_id: AgentId, task_id: TaskId, result: TaskCompletionResult },
    AgentError { agent_id: AgentId, error: String },
    ThreadFinished { agent_id: AgentId, outcome: ThreadOutcome },
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Status state machine complexity | Medium | High | Document all valid transitions, test edge cases |
| Channel polling performance | Low | Medium | Use try_recv, avoid tight spin loops |

## Sprint Deliverables

- `core/src/agent_slot.rs` - AgentSlot implementation
- `core/src/agent_pool.rs` - AgentPool implementation  
- `core/src/shared_state.rs` - SharedWorkplaceState
- `core/src/event_aggregator.rs` - EventAggregator
- Unit tests for all components

## Dependencies

None (foundation sprint).

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Provider Threads](./sprint-02-provider-threads.md) for real provider integration.