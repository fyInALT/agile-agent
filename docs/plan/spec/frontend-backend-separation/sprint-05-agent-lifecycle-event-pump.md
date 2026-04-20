# Sprint 5: Agent Lifecycle + Event Pump

## Metadata

- Sprint ID: `sprint-fbs-005`
- Title: `Agent Lifecycle + Event Pump`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 4: SessionManager + Snapshot](./sprint-04-session-manager-snapshot.md)

## Sprint Goal

Agents can be spawned and stopped through the JSON-RPC protocol. The `EventPump` background task consumes `ProviderEvent`s from `EventAggregator` and converts them to protocol `Event`s with sequence numbers. This is the first time the daemon actively produces events.

## Stories

### Story 5.1: agent.spawn Handler

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the `agent.spawn` method handler that creates a new agent in the pool.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.1.1 | Implement `AgentHandler` struct with `Arc<SessionManager>` | Todo | - |
| T5.1.2 | Parse `AgentSpawnParams` (provider, role, codename) | Todo | - |
| T5.1.3 | Call `AgentPool::spawn()` with validated params | Todo | - |
| T5.1.4 | Return `AgentSnapshot` of the newly created agent | Todo | - |
| T5.1.5 | Validate `ProviderKind` and `AgentRole` enums | Todo | - |
| T5.1.6 | Write integration test: spawn succeeds and returns valid snapshot | Todo | - |
| T5.1.7 | Write integration test: spawn with invalid provider returns `-32602` | Todo | - |
| T5.1.8 | Write integration test: spawn beyond max slots returns `-32000` | Todo | - |

#### Acceptance Criteria

- `agent.spawn` creates a new agent with unique ID and codename
- Response contains a complete `AgentSnapshot`
- Invalid provider/role produces `-32602`
- Pool capacity limits are enforced

#### Technical Notes

See IMP-01 §3.6. The handler delegates directly to `SessionManager::spawn_agent()`. Do not add business logic in the handler — keep it thin.

---

### Story 5.2: agent.stop Handler

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Implement the `agent.stop` method handler for graceful agent shutdown.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.2.1 | Parse `AgentStopParams` (agent_id, force) | Todo | - |
| T5.2.2 | Call `AgentPool::stop()` or `AgentPool::force_stop()` | Todo | - |
| T5.2.3 | Return `AgentStopResult` with stopped confirmation | Todo | - |
| T5.2.4 | Return `-32101` if agent_id does not exist | Todo | - |
| T5.2.5 | Write integration test: stop succeeds for running agent | Todo | - |
| T5.2.6 | Write integration test: stop returns error for unknown agent | Todo | - |

#### Acceptance Criteria

- `agent.stop` gracefully stops the specified agent
- `force: true` sends SIGKILL instead of graceful shutdown
- Unknown `agent_id` produces `-32101`
- Stopped agent is removed from the active pool

#### Technical Notes

See IMP-01 §3.7. The `force` flag is for emergency use only. Default behavior should always be graceful.

---

### Story 5.3: agent.list Handler

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Implement the `agent.list` method handler for querying all agents.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.3.1 | Parse `AgentListParams` (include_stopped) | Todo | - |
| T5.3.2 | Query `AgentPool` for all slots | Todo | - |
| T5.3.3 | Filter stopped agents based on `include_stopped` flag | Todo | - |
| T5.3.4 | Return `AgentListResult` with `Vec<AgentSnapshot>` | Todo | - |
| T5.3.5 | Write integration test: list includes only running agents by default | Todo | - |
| T5.3.6 | Write integration test: list with `include_stopped` shows all agents | Todo | - |

#### Acceptance Criteria

- Default list shows only non-stopped agents
- `include_stopped: true` shows all agents including stopped
- Response is sorted by agent creation time

---

### Story 5.4: EventAggregator Integration

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Wire the existing `EventAggregator` from `agent_core` into the daemon's `SessionManager`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.4.1 | Store `EventAggregator` in `SessionManager` | Todo | - |
| T5.4.2 | Expose `event_rx: mpsc::Receiver<ProviderEvent>` from aggregator | Todo | - |
| T5.4.3 | Register aggregator when agent spawns (add receiver) | Todo | - |
| T5.4.4 | Unregister aggregator when agent stops (remove receiver) | Todo | - |
| T5.4.5 | Write unit test: aggregator receives events from mock provider | Todo | - |
| T5.4.6 | Write unit test: receiver is cleaned up on agent stop | Todo | - |

#### Acceptance Criteria

- `EventAggregator` is accessible from `SessionManager`
- Agent spawn registers a new event receiver
- Agent stop removes the event receiver (no leak)
- Events flow from provider thread → aggregator → `event_rx`

#### Technical Notes

See IMP-04 §4.1. The `EventAggregator` already exists in `agent_core`. Do not rewrite it — just wire it into the daemon's lifecycle.

---

### Story 5.5: EventPump Background Task

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the background task that polls `ProviderEvent`s, converts them to `Event`s, and assigns sequence numbers.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.5.1 | Create `EventPump` struct with `aggregator_rx`, `broadcaster`, `seq_counter` | Todo | - |
| T5.5.2 | Implement `run_event_pump()` async loop | Todo | - |
| T5.5.3 | Implement `EventPumpState` with `current_items: HashMap<AgentId, ItemId>` | Todo | - |
| T5.5.4 | Implement `convert_provider_event()` for all `ProviderEvent` variants | Todo | - |
| T5.5.5 | Spawn EventPump as `tokio::task` during daemon startup | Todo | - |
| T5.5.6 | Write unit test: single provider event converts to correct Event | Todo | - |
| T5.5.7 | Write unit test: sequence numbers are monotonic | Todo | - |
| T5.5.8 | Write unit test: out-of-order deltas are handled gracefully | Todo | - |

#### Acceptance Criteria

- EventPump runs continuously during daemon lifetime
- Every `ProviderEvent` produces at least one `Event`
- `seq` numbers start at `1` and increment by `1` per event
- `current_items` tracks in-flight items correctly across `Started` → `Delta` × N → `Completed`

#### Technical Notes

See IMP-05 §2. This is the **only** place that knows about `ProviderEvent`. Centralizing the conversion here prevents protocol leaks. The pump must handle all `ProviderEvent` variants — missing variants are a protocol bug.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ProviderEvent variant not covered by conversion | Medium | High | Exhaustive match in `convert_provider_event()`; compiler warns on new variants |
| EventPump drops events under load | Low | High | Unbounded channel from aggregator; bounded broadcast channel with overflow detection |
| Agent spawn fails silently | Medium | High | Return explicit errors; log spawn failures |

## Sprint Deliverables

- `agent/daemon/src/handler/agent.rs` — `agent.spawn`, `agent.stop`, `agent.list` handlers
- `agent/daemon/src/event_pump.rs` — `EventPump` and conversion logic
- Updated `SessionManager` with aggregator lifecycle hooks
- Unit and integration tests for agent lifecycle and event conversion

## Dependencies

- [Sprint 4: SessionManager + Snapshot](./sprint-04-session-manager-snapshot.md) — `SessionManager` must exist.

## Next Sprint

After completing this sprint, proceed to [Sprint 6: Event Broadcast + Persistence](./sprint-06-event-broadcast-persistence.md) to broadcast events to clients and persist them to disk.
