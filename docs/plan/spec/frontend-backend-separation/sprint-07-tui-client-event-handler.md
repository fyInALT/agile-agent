# Sprint 7: TUI WebSocket Client + Event Handler

## Metadata

- Sprint ID: `sprint-fbs-007`
- Title: `TUI WebSocket Client + Event Handler`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 6: Event Broadcast + Persistence](./sprint-06-event-broadcast-persistence.md)

## Sprint Goal

The TUI connects to the daemon via WebSocket, receives the initial snapshot, and applies incoming events to rebuild its render state. The TUI can send user input and receive real-time transcript updates. This is the first live end-to-end integration.

## Stories

### Story 7.1: TUI WebSocket Client

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the WebSocket client inside the TUI that connects to the daemon and handles JSON-RPC messaging.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.1.1 | Create `WebSocketClient` struct with `request_tx`, `event_rx` | Todo | - |
| T7.1.2 | Implement `WebSocketClient::connect(daemon_url)` using `tokio_tungstenite` | Todo | - |
| T7.1.3 | Implement read task: parse server messages into `ServerMessage` enum | Todo | - |
| T7.1.4 | Implement write task: serialize and send client requests/notifications | Todo | - |
| T7.1.5 | Implement `call(method, params) -> Response` with request/response correlation | Todo | - |
| T7.1.6 | Implement `notify(method, params)` fire-and-forget | Todo | - |
| T7.1.7 | Write unit test: request/response round-trip with mock WebSocket | Todo | - |
| T7.1.8 | Write unit test: notification does not wait for response | Todo | - |

#### Acceptance Criteria

- Client connects to daemon WebSocket URL successfully
- `call()` returns the correct response for the request ID
- `notify()` sends without waiting
- Server messages are parsed into typed `ServerMessage` variants
- Connection errors are surfaced as `ServerMessage::Error`

#### Technical Notes

See IMP-06 §4. The client uses `tokio::select!` in the read task to handle both incoming messages and outgoing requests. Request/response correlation uses `oneshot` channels stored in a `HashMap<RequestId, Sender>`.

---

### Story 7.2: Auto-Link Integration in TUI

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Wire the shared auto-link logic into the TUI startup flow.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.2.1 | Call `auto_link()` from TUI `main()` before entering event loop | Todo | - |
| T7.2.2 | Display connection progress ("Discovering daemon...") | Todo | - |
| T7.2.3 | Handle auto-link failure with user-friendly error message | Todo | - |
| T7.2.4 | Send `session.initialize` immediately after connect | Todo | - |
| T7.2.5 | Store received `SessionState` in `TuiState::session` | Todo | - |
| T7.2.6 | Write integration test: TUI starts, auto-links, receives snapshot | Todo | - |

#### Acceptance Criteria

- TUI startup automatically discovers or spawns the daemon
- User sees clear progress messages during connection
- Snapshot is received and stored within 2s of connect
- Failure to connect shows actionable error (not panic)

#### Technical Notes

See IMP-06 §4.3. Reuse the shared `auto_link()` from `agent-protocol/src/client/`. TUI-specific UI for connection progress is local to the TUI.

---

### Story 7.3: Event → TuiState Application

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the event handler that applies daemon events to the TUI's render state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.3.1 | Create `event_handler.rs` module with `apply_event(state, event)` | Todo | - |
| T7.3.2 | Implement `AgentSpawned` → add to `TuiState::agents` | Todo | - |
| T7.3.3 | Implement `AgentStopped` → remove from agents, clear focus if needed | Todo | - |
| T7.3.4 | Implement `AgentStatusChanged` → update agent status | Todo | - |
| T7.3.5 | Implement `ItemStarted` → append new transcript item | Todo | - |
| T7.3.6 | Implement `ItemDelta` → append text to in-flight item | Todo | - |
| T7.3.7 | Implement `ItemCompleted` → finalize transcript item | Todo | - |
| T7.3.8 | Write unit test: each event type updates state correctly | Todo | - |
| T7.3.9 | Write unit test: event sequence rebuilds transcript accurately | Todo | - |

#### Acceptance Criteria

- Every `EventPayload` variant has a corresponding state update
- Transcript is byte-for-byte identical to the daemon's transcript after applying all events
- `focused_agent_id` is cleared when the focused agent stops
- State updates are deterministic (same event sequence → same state)

#### Technical Notes

See IMP-06 §5. The event handler is pure logic — no async, no I/O. It takes `&mut TuiState` and `&Event`. This makes it trivially testable.

---

### Story 7.4: Connection State UI

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Add visual indicators for connection state (connecting, connected, reconnecting, disconnected).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.4.1 | Add `ConnectionState` enum to `TuiState` | Todo | - |
| T7.4.2 | Render connection status in status bar (color-coded) | Todo | - |
| T7.4.3 | Show reconnect progress indicator | Todo | - |
| T7.4.4 | Dim or freeze UI when disconnected | Todo | - |
| T7.4.5 | Write unit test: connection state transitions correctly | Todo | - |

#### Acceptance Criteria

- Connected: green indicator, full UI active
- Reconnecting: yellow indicator with retry count
- Disconnected: red indicator, input disabled
- State transitions are visible within one frame (16ms)

---

### Story 7.5: Input Send via Protocol

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Replace direct `AppState` input submission with `session.sendInput` protocol call.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.5.1 | On Enter key, call `client.call("session.sendInput", params)` | Todo | - |
| T7.5.2 | Include `target_agent_id` if focus is set | Todo | - |
| T7.5.3 | Handle response (show error if input rejected) | Todo | - |
| T7.5.4 | Remove direct `app_state.submit_input()` call | Todo | - |
| T7.5.5 | Write integration test: input sent via protocol appears in transcript | Todo | - |

#### Acceptance Criteria

- User input is sent over WebSocket, not directly to core
- Input appears in transcript via event stream (not immediate local append)
- Errors (e.g., no target agent) are shown in status bar

#### Technical Notes

See IMP-06 §6.3. This is the first user action that goes through the protocol instead of direct core access. The transcript update comes back as an `ItemStarted` event.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TUI event loop conflicts with async WebSocket | Medium | High | Use `tokio::select!` with `crossterm` event stream wrapper |
| Event application order mismatches daemon | Low | High | Sequence numbers ensure order; gap detection recovers |
| Large snapshot slows TUI startup | Low | Medium | Async snapshot receive; render initial frame while loading |

## Sprint Deliverables

- `tui/src/websocket_client.rs` — `WebSocketClient`
- `tui/src/event_handler.rs` — `apply_event()`
- Updated `TuiState` with connection state and event-driven render state
- Updated `app_loop.rs` with `tokio::select!` (crossterm + WebSocket)
- Integration tests: TUI connects, receives events, renders updates

## Dependencies

- [Sprint 6: Event Broadcast + Persistence](./sprint-06-event-broadcast-persistence.md) — daemon must broadcast events.

## Next Sprint

After completing this sprint, proceed to [Sprint 8: TUI Decoupling](./sprint-08-tui-decoupling.md) to remove all remaining `agent_core` dependencies from the TUI.
