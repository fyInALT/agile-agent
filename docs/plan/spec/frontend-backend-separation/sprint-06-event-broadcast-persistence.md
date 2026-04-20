# Sprint 6: Event Broadcast + Persistence

## Metadata

- Sprint ID: `sprint-fbs-006`
- Title: `Event Broadcast + Persistence`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 5: Agent Lifecycle + Event Pump](./sprint-05-agent-lifecycle-event-pump.md)

## Sprint Goal

Events produced by the `EventPump` are broadcast to all connected clients and persisted to an append-only `events.jsonl` log. Clients can detect gaps and request replay. This completes the event streaming infrastructure.

## Stories

### Story 6.1: EventBroadcaster Multi-Client Fan-Out

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the broadcaster that distributes events to all connected WebSocket clients.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.1.1 | Create `EventBroadcaster` struct with `HashMap<ConnectionId, Sender>` | Todo | - |
| T6.1.2 | Implement `register(conn_id, sender)` for new connections | Todo | - |
| T6.1.3 | Implement `unregister(conn_id)` on disconnect | Todo | - |
| T6.1.4 | Implement `broadcast(event)` — sends to all registered clients | Todo | - |
| T6.1.5 | Serialize event once, clone serialized text for each client | Todo | - |
| T6.1.6 | Write unit test: single event reaches all registered clients | Todo | - |
| T6.1.7 | Write unit test: disconnected client does not receive events | Todo | - |
| T6.1.8 | Write unit test: broadcaster survives client channel closure | Todo | - |

#### Acceptance Criteria

- Every connected client receives every event
- Disconnected clients are removed from the broadcast list
- Broadcast is non-blocking (uses `try_send` or unbounded channel)
- Event order is identical for all clients

#### Technical Notes

See IMP-03 §2.5 and IMP-05 §3. Use `mpsc::UnboundedSender` for v1. The broadcaster holds the sender half; each `Connection` task holds the receiver half.

---

### Story 6.2: events.jsonl Append-Only Log

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement persistent storage for events to enable replay after reconnect.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.2.1 | Create `EventLog` struct with file path and handle | Todo | - |
| T6.2.2 | Implement `EventLog::open(workplace_dir)` — create/truncate or append | Todo | - |
| T6.2.3 | Implement `EventLog::append(event)` — atomic line write | Todo | - |
| T6.2.4 | Implement `EventLog::flush()` — sync to disk | Todo | - |
| T6.2.5 | Integrate `EventLog::append()` into `EventPump` (write before broadcast) | Todo | - |
| T6.2.6 | Write unit test: append produces valid JSONL | Todo | - |
| T6.2.7 | Write unit test: file survives crash (flush durability) | Todo | - |

#### Acceptance Criteria

- Each event is written as one JSON line
- File is append-only (no in-place modifications)
- Write happens before broadcast (durability guarantee)
- File can be read line-by-line and deserialized

#### Technical Notes

See IMP-05 §5.3. The file lives at `.agile-agent/events.jsonl`. Use `tokio::fs::write` with `flush().await` for durability. Format: one JSON object per line, no pretty-printing.

---

### Story 6.3: Event Replay from Log

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement reading events back from the log for client reconnect scenarios.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.3.1 | Implement `EventLog::replay_from(from_seq)` — read lines, filter by seq | Todo | - |
| T6.3.2 | Implement `EventLog::replay_range(start_seq, end_seq)` | Todo | - |
| T6.3.3 | Buffer replayed events in memory (do not stream line-by-line) | Todo | - |
| T6.3.4 | Handle corrupted lines gracefully (skip + log warning) | Todo | - |
| T6.3.5 | Write unit test: replay returns events in correct order | Todo | - |
| T6.3.6 | Write unit test: replay_from excludes events before start_seq | Todo | - |
| T6.3.7 | Write unit test: corrupted line is skipped, remaining events returned | Todo | - |

#### Acceptance Criteria

- `replay_from(42)` returns all events with `seq >= 42`
- Events are returned in `seq` order
- Corrupted lines are skipped without crashing
- Replay completes in under 1s for 10,000 events

#### Technical Notes

See IMP-05 §5.3. Replay is used during `session.initialize` with `resume_snapshot_id`. The daemon reads the log, filters, and sends events before entering live broadcast mode.

---

### Story 6.4: Gap Detection + Recovery Protocol

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the client-side gap detection and the daemon-side recovery response.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4.1 | Client: track `last_received_seq` | Todo | - |
| T6.4.2 | Client: detect gap (`next_seq > last_seq + 1`) | Todo | - |
| T6.4.3 | Client: buffer out-of-order events | Todo | - |
| T6.4.4 | Client: send `session.initialize` with `resume_snapshot_id` on gap | Todo | - |
| T6.4.5 | Daemon: on initialize with resume, send snapshot + replayed events | Todo | - |
| T6.4.6 | Client: apply replayed events, then drain buffered queue | Todo | - |
| T6.4.7 | Write integration test: gap detected, replay recovers state | Todo | - |
| T6.4.8 | Write integration test: no gap = normal operation | Todo | - |

#### Acceptance Criteria

- Gap detection triggers within one event cycle
- Recovery completes without duplicate or missing events
- Client state is consistent after recovery
- Normal operation (no gap) has zero recovery overhead

#### Technical Notes

See IMP-01 §6 and IMP-05 §5. The recovery protocol is: snapshot → replayed events → live events. The client must not apply live events until the replay is complete.

---

### Story 6.5: Heartbeat + Lag Detection

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Complete the heartbeat mechanism with lag detection for slow clients.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.5.1 | Add `last_ack_seq` tracking per client connection | Todo | - |
| T6.5.2 | Include `last_received_seq` in client heartbeat payload | Todo | - |
| T6.5.3 | Daemon: compare `last_ack_seq` with `seq_counter` to detect lag | Todo | - |
| T6.5.4 | Daemon: mark lagging clients, drop events for them | Todo | - |
| T6.5.5 | Include `lagging: true` in `heartbeatAck` for lagging clients | Todo | - |
| T6.5.6 | Write integration test: fast client receives all events | Todo | - |
| T6.5.7 | Write integration test: slow client is marked lagging | Todo | - |

#### Acceptance Criteria

- Fast clients receive all events in real time
- Slow clients are detected within 30s (one heartbeat cycle)
- Lagging clients receive `lagging: true` in heartbeat ack
- Client re-syncs automatically when lag is detected

#### Technical Notes

See IMP-05 §6. For v1, unbounded channels are acceptable. Lag detection is a safety net, not the primary mechanism.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| events.jsonl grows unbounded | Medium | Medium | Truncate after snapshot write; monitor file size |
| Replay slows down reconnect | Low | Medium | Benchmark; add pagination if >10s |
| Concurrent append + replay deadlock | Low | High | Use separate read handle; no locks on append-only file |

## Sprint Deliverables

- `agent/daemon/src/broadcaster.rs` — `EventBroadcaster`
- `agent/daemon/src/event_log.rs` — `EventLog` read/write
- Updated `EventPump` with persistence hook
- Gap detection logic in client and daemon
- Integration tests: broadcast, replay, gap recovery

## Dependencies

- [Sprint 5: Agent Lifecycle + Event Pump](./sprint-05-agent-lifecycle-event-pump.md) — events must be produced before they can be broadcast.

## Next Sprint

After completing this sprint, proceed to [Sprint 7: TUI WebSocket Client + Event Handler](./sprint-07-tui-client-event-handler.md) to connect the TUI to the daemon.
