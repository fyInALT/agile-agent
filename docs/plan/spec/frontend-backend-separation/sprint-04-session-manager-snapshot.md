# Sprint 4: SessionManager + Snapshot

## Metadata

- Sprint ID: `sprint-fbs-004`
- Title: `SessionManager + Snapshot`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 3: Auto-Link + Lifecycle](./sprint-03-auto-link-lifecycle.md)

## Sprint Goal

The daemon owns the runtime state. `SessionManager` encapsulates `AppState`, `AgentPool`, `EventAggregator`, and `Mailbox` — all previously owned by `TuiState`. The `session.initialize` handler returns a real `SessionState` snapshot built from live data. This is the "state migration begins" milestone.

## Stories

### Story 4.1: SessionManager Struct + Bootstrap

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Create the `SessionManager` struct and its bootstrap logic, mirroring the current `TuiState::bootstrap()`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Create `SessionManager` struct with `app_state`, `agent_pool`, `event_aggregator`, `mailbox`, `runtime` fields | Todo | - |
| T4.1.2 | Implement `SessionManager::bootstrap(workplace)` — load or create `AppState` | Todo | - |
| T4.1.3 | Initialize `AgentRuntime` from config | Todo | - |
| T4.1.4 | Initialize `AgentPool` with runtime | Todo | - |
| T4.1.5 | Initialize `EventAggregator` and `Mailbox` | Todo | - |
| T4.1.6 | Wrap mutable state in `Arc<RwLock<>>` | Todo | - |
| T4.1.7 | Write unit test: `bootstrap()` produces valid SessionManager | Todo | - |
| T4.1.8 | Write unit test: concurrent reads do not block each other | Todo | - |

#### Acceptance Criteria

- `SessionManager::bootstrap()` succeeds with a valid workplace
- All core objects (`AppState`, `AgentPool`, etc.) are initialized
- State is behind `Arc<RwLock<>>` for shared access
- Multiple concurrent reads complete without deadlock

#### Technical Notes

See IMP-04 §4.1. This is a direct extraction from `TuiState::bootstrap()` in the current TUI. Do not refactor the core logic — just move it. The `RwLock` pattern replaces the current `&mut TuiState` access in the TUI event loop.

---

### Story 4.2: Core-to-Protocol Mapping Functions

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the adapter functions that translate `agent_core` domain types into `agent_protocol` wire types.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Implement `into_transcript_item()` — `TranscriptEntry` → `TranscriptItem` | Todo | - |
| T4.2.2 | Implement `into_agent_snapshot()` — `AgentSlot` → `AgentSnapshot` | Todo | - |
| T4.2.3 | Implement `map_item_kind()` — core item kind → protocol item kind | Todo | - |
| T4.2.4 | Implement `map_session_status()` — core status → protocol status | Todo | - |
| T4.2.5 | Implement `map_backlog()` and `map_skills()` | Todo | - |
| T4.2.6 | Handle `serde_json::Value` for opaque metadata fields | Todo | - |
| T4.2.7 | Write unit tests for each mapping function | Todo | - |
| T4.2.8 | Write unit test: mapping is lossless for known fields | Todo | - |

#### Acceptance Criteria

- Every `agent_core` type used in snapshots has a corresponding mapping function
- Mappings are deterministic (same input → same output)
- Timestamps are ISO 8601 strings
- `AgentRole` and `ProviderKind` use string representations matching protocol spec

#### Technical Notes

See IMP-04 §4.1. These functions are the **only** place that knows about both `agent_core` and `agent_protocol`. They must be simple — no business logic, just field copying and type conversion.

---

### Story 4.3: Snapshot Assembly

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement `SessionManager::snapshot()` that assembles the full `SessionState` from all core objects.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Acquire read locks on `app_state` and `agent_pool` | Todo | - |
| T4.3.2 | Map transcript entries to `TranscriptItem` vector | Todo | - |
| T4.3.3 | Map agent slots to `AgentSnapshot` vector | Todo | - |
| T4.3.4 | Assemble `WorkplaceSnapshot` from workplace data | Todo | - |
| T4.3.5 | Set `last_event_seq` from `seq_counter` | Todo | - |
| T4.3.6 | Include `protocol_version` from `agent_protocol::PROTOCOL_VERSION` | Todo | - |
| T4.3.7 | Write unit test: snapshot contains all expected fields | Todo | - |
| T4.3.8 | Write unit test: snapshot size is reasonable (benchmark) | Todo | - |

#### Acceptance Criteria

- Snapshot contains all fields defined in IMP-01 §5.1
- `last_event_seq` reflects the current sequence counter
- Snapshot generation completes in under 50ms for typical sessions
- Empty sessions produce valid snapshots with zero-length arrays

#### Technical Notes

See IMP-04 §4.1 and IMP-02 §5. Snapshot generation must be fast because it blocks the handler. If the transcript is very large, consider pagination (future sprint).

---

### Story 4.4: session.initialize Returns Real Snapshot

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Wire `SessionManager` into the `session.initialize` handler so clients receive live data instead of hardcoded stubs.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.4.1 | Add `Arc<SessionManager>` to `SessionHandler` | Todo | - |
| T4.4.2 | Call `session_mgr.snapshot().await` in `session.initialize` handler | Todo | - |
| T4.4.3 | Handle `resume_snapshot_id` parameter (stub: ignore for now) | Todo | - |
| T4.4.4 | Set connection state to `Initialized` only after snapshot is sent | Todo | - |
| T4.4.5 | Write integration test: client receives snapshot matching daemon state | Todo | - |
| T4.4.6 | Write integration test: snapshot reflects agent spawn/stop changes | Todo | - |

#### Acceptance Criteria

- `session.initialize` returns a snapshot built from live `SessionManager` state
- Snapshot reflects agents spawned via `AgentPool`
- Snapshot reflects transcript changes via `AppState`
- Connection is not marked `Initialized` until the snapshot is successfully serialized

#### Technical Notes

See IMP-03 §2.5 and IMP-04 §4.4. The `SessionManager` is shared via `Arc` across all handlers. This is the first time real core state flows over the protocol.

---

### Story 4.5: Snapshot Persistence on Shutdown

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Implement `snapshot.json` write during graceful shutdown.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.5.1 | Define `SnapshotFile` struct with `version`, `session_id`, `written_at`, `last_event_seq`, `state` | Todo | - |
| T4.5.2 | Implement `SessionManager::write_snapshot(path)` | Todo | - |
| T4.5.3 | Call `write_snapshot()` during shutdown sequence | Todo | - |
| T4.5.4 | Write unit test: snapshot file round-trips correctly | Todo | - |

#### Acceptance Criteria

- `snapshot.json` is written atomically during shutdown
- File contains schema version `1` and a valid `SessionState`
- `written_at` is ISO 8601 timestamp
- File is readable and deserializable after write

#### Technical Notes

See IMP-03 §4.2. Snapshot persistence enables session restore after daemon restart. Full restore logic (reading snapshot on startup) comes in Sprint 8.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| `RwLock` contention under load | Medium | Medium | Profile before optimizing; use `read()` for snapshots |
| Core type changes break mappings | Medium | High | Mapping tests fail fast; update mappers only |
| Snapshot too large for WebSocket frame | Low | Medium | Benchmark; add pagination if needed |

## Sprint Deliverables

- `agent/daemon/src/session_mgr.rs` — `SessionManager` struct and methods
- `agent/daemon/src/mapper.rs` — core-to-protocol mapping functions
- Updated `session.initialize` handler returning live snapshots
- `snapshot.json` write on shutdown
- Unit and integration tests for all mapping and snapshot logic

## Dependencies

- [Sprint 3: Auto-Link + Lifecycle](./sprint-03-auto-link-lifecycle.md) — daemon startup/shutdown must work.

## Next Sprint

After completing this sprint, proceed to [Sprint 5: Agent Lifecycle + Event Pump](./sprint-05-agent-lifecycle-event-pump.md) to enable agent spawn/stop through the protocol and begin event streaming.
