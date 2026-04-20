# Sprint 10: Hardening — Reconnect + Approval Flow

## Metadata

- Sprint ID: `sprint-fbs-010`
- Title: `Hardening — Reconnect + Approval Flow`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 9: CLI Refactor](./sprint-09-cli-refactor.md)

## Sprint Goal

The system is resilient to network interruptions, daemon restarts, and slow clients. The TUI reconnects automatically with state recovery. Approval and decision requests flow correctly from daemon → TUI overlay → user response → daemon. Error handling gives clear feedback to users.

## Stories

### Story 10.1: TUI Reconnect with Exponential Backoff

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement automatic reconnection when the WebSocket connection drops.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.1.1 | Detect disconnect (WebSocket close, read error, heartbeat timeout) | Todo | - |
| T10.1.2 | Transition `TuiState::connection` to `Reconnecting` | Todo | - |
| T10.1.3 | Implement exponential backoff: 100ms → 200ms → 400ms → ... → 30s max | Todo | - |
| T10.1.4 | Attempt reconnection with `auto_link()` | Todo | - |
| T10.1.5 | Send `session.initialize` with `resume_snapshot_id` on reconnect | Todo | - |
| T10.1.6 | Display reconnect progress to user (attempt count, backoff time) | Todo | - |
| T10.1.7 | Write integration test: disconnect → reconnect succeeds | Todo | - |
| T10.1.8 | Write integration test: max backoff reached, connection retried indefinitely | Todo | - |

#### Acceptance Criteria

- Disconnect is detected within 5s
- Reconnection attempts start at 100ms and double up to 30s
- User sees clear reconnect status in UI
- Reconnection succeeds when daemon becomes available
- No data loss: events resume from correct sequence number

#### Technical Notes

See IMP-06 §7.4 and IMP-04 §7.2. Use `tokio::time::sleep` with `tokio::select!` to allow cancellation on successful reconnect.

---

### Story 10.2: State Recovery on Reconnect

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Ensure the TUI's render state is consistent after reconnect by applying the snapshot and replayed events.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.2.1 | On reconnect, replace `TuiState::session` with new snapshot | Todo | - |
| T10.2.2 | Replace `TuiState::transcript` with snapshot transcript | Todo | - |
| T10.2.3 | Replace `TuiState::agents` with snapshot agents | Todo | - |
| T10.2.4 | Apply replayed events on top of snapshot | Todo | - |
| T10.2.5 | Buffer live events during replay, apply after replay completes | Todo | - |
| T10.2.6 | Write integration test: state after reconnect matches pre-disconnect | Todo | - |
| T10.2.7 | Write integration test: events during replay are not lost | Todo | - |

#### Acceptance Criteria

- Post-reconnect transcript is byte-for-byte identical to pre-disconnect
- No duplicate events (snapshot + replay overlap is handled)
- No missing events (all events between disconnect and reconnect are applied)
- UI does not flicker during recovery

#### Technical Notes

See IMP-05 §5.1 and §5.2. The key is: snapshot first, then replay from `last_event_seq + 1`, then drain buffered live events.

---

### Story 10.3: Approval Request Notification → Overlay

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the full approval flow: daemon sends notification, TUI shows overlay, user responds, daemon receives approval.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.3.1 | TUI: handle `approval.request` notification — push `ApprovalOverlay` | Todo | - |
| T10.3.2 | Render approval overlay with tool name, preview, timeout countdown | Todo | - |
| T10.3.3 | Handle `Y` key (approve) — send `tool.approve` with `allowed: true` | Todo | - |
| T10.3.4 | Handle `N` key (reject) — send `tool.approve` with `allowed: false` | Todo | - |
| T10.3.5 | Handle timeout — auto-reject if user does not respond | Todo | - |
| T10.3.6 | Remove overlay on resolution response | Todo | - |
| T10.3.7 | Write integration test: full approval flow end-to-end | Todo | - |
| T10.3.8 | Write integration test: timeout auto-rejects | Todo | - |

#### Acceptance Criteria

- Approval overlay appears within 100ms of notification
- Tool preview is readable (truncated if too long)
- Timeout countdown updates every second
- `Y`/`N` keys send correct approval response
- Overlay disappears on resolution
- Timeout auto-rejects after configured duration

#### Technical Notes

See IMP-06 §8 and IMP-01 §4.2. The timeout is handled by the daemon, but the TUI should also hide the overlay if the daemon sends a resolution event.

---

### Story 10.4: Decision Request Notification → Overlay

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Implement the decision layer escalation flow in the TUI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.4.1 | TUI: handle `decision.request` notification — push `DecisionOverlay` | Todo | - |
| T10.4.2 | Render decision overlay with situation text and options | Todo | - |
| T10.4.3 | Handle option selection keys (1, 2, 3...) | Todo | - |
| T10.4.4 | Send `decision.respond` with selected choice | Todo | - |
| T10.4.5 | Handle timeout — auto-escalate if no response | Todo | - |
| T10.4.6 | Write integration test: decision flow end-to-end | Todo | - |

#### Acceptance Criteria

- Decision overlay shows situation and numbered options
- User selects option via number keys
- Response is sent to daemon within 100ms
- Timeout auto-escalates after configured duration

---

### Story 10.5: Error Handling + User Feedback

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Improve error visibility in the TUI and CLI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.5.1 | TUI: display JSON-RPC error messages in status bar (not just log) | Todo | - |
| T10.5.2 | TUI: show toast/notification for transient errors | Todo | - |
| T10.5.3 | CLI: print structured error output (method, code, message) | Todo | - |
| T10.5.4 | CLI: exit with non-zero code on daemon errors | Todo | - |
| T10.5.5 | Add error context (which agent, which request) to all error displays | Todo | - |
| T10.5.6 | Write unit test: error formatting is user-friendly | Todo | - |

#### Acceptance Criteria

- Errors are visible to users within 100ms
- Error messages are human-readable (no raw JSON-RPC dumped to screen)
- CLI returns appropriate exit codes
- Error context helps users understand what went wrong

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Reconnect storm (too many retries) | Low | Medium | Exponential backoff with jitter; max 30s interval |
| Approval overlay blocks other UI | Medium | Medium | Modal overlay on top; background events still update |
| Race: approval resolved while overlay closing | Low | Low | Idempotent `tool.approve` handler on daemon |

## Sprint Deliverables

- Updated `tui/src/websocket_client.rs` with reconnect logic
- Updated `tui/src/event_handler.rs` with state recovery
- `tui/src/overlays/approval.rs` — approval overlay
- `tui/src/overlays/decision.rs` — decision overlay
- Updated error handling in both TUI and CLI
- Integration tests: reconnect, approval, decision flows

## Dependencies

- [Sprint 9: CLI Refactor](./sprint-09-cli-refactor.md) — CLI and TUI must both be protocol clients.

## Next Sprint

After completing this sprint, proceed to [Sprint 11: Cleanup + Performance Validation](./sprint-11-cleanup-performance.md) for the final cleanup and release preparation.
