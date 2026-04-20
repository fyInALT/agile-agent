# Sprint 8: TUI Decoupling

## Metadata

- Sprint ID: `sprint-fbs-008`
- Title: `TUI Decoupling`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 7: TUI WebSocket Client + Event Handler](./sprint-07-tui-client-event-handler.md)

## Background

The TUI now connects to the daemon and receives events, but it still carries the baggage of direct `agent_core` dependencies. `TuiState` retains fields for `RuntimeSession`, `AgentPool`, `EventAggregator`, and `Mailbox` — all dead weight that is no longer used since the TUI relies on the protocol. The `Cargo.toml` still lists `agent-core`, `agent-decision`, and `agent-kanban` as dependencies. Compilation succeeds only because these types are still imported, not because they are needed.

This sprint is surgical removal. Every `use agent_core::*` statement is deleted. Every direct method call on core objects is replaced with a protocol call. `TuiState` shrinks to a pure render state machine. The `agent-tui` crate becomes a thin presentation layer that depends only on `agent-protocol` and rendering libraries. This is the decoupling endgame.

## Sprint Goal

The TUI contains zero `agent_core` imports. All runtime state ownership is in the daemon. `TuiState` is a pure render state machine. `cargo build -p agent-tui` succeeds with no `agent-core` dependency in `Cargo.toml`.

## TDD Approach

Decoupling is deletion work — the primary risk is accidentally removing something still in use. Tests are the safety net.

1. **Red**: Before deleting any import, write a test that exercises the code path through the new protocol-based route. Ensure the test passes.
2. **Green**: Delete the core import and its usage; verify the test still passes (now via protocol).
3. **Refactor**: Clean up dead code, re-exports, and conditional compilation.

Test requirements per story:
- Compilation gate: `cargo build -p agent-tui` must succeed after each import removal
- Dependency audit: `cargo tree -p agent-tui` shows no `agent-core`/`agent-decision`/`agent-kanban`
- Functional parity tests: every TUI feature works via protocol (spawn, stop, input, focus switch)
- Mock daemon tests: TUI operates correctly against a mock daemon (no real core needed)
- Integration tests: full TUI + daemon end-to-end for all user workflows

## Stories

### Story 8.1: Remove RuntimeSession from TuiState

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Delete the `session: RuntimeSession` field and all code that directly accesses it.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.1.1 | Remove `session` field from `TuiState` | Todo | - |
| T8.1.2 | Replace `session.app_state` access with `TuiState::transcript` (from events) | Todo | - |
| T8.1.3 | Replace `session.workplace` access with `SessionState::workplace` (from snapshot) | Todo | - |
| T8.1.4 | Replace `session.config` access with protocol calls or snapshot data | Todo | - |
| T8.1.5 | Update `render.rs` to use snapshot data instead of `session` | Todo | - |
| T8.1.6 | Write compilation check: `cargo build -p agent-tui` with `session` removed | Todo | - |

#### Acceptance Criteria

- `RuntimeSession` is not imported in any TUI source file
- `TuiState` has no `session` field
- All previous `session` usages are replaced with protocol-driven equivalents
- TUI compiles and renders correctly
- **Tests**: `no_runtime_session_import` — grep confirms no `RuntimeSession` in TUI; `tui_compiles` — `cargo build -p agent-tui` succeeds


#### Technical Notes

See IMP-04 §2 and §3. `RuntimeSession` was the deepest coupling point. Removing it is the single biggest step in decoupling.

---

### Story 8.2: Remove AgentPool from TuiState

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Delete the `agent_pool: Option<AgentPool>` field and replace agent operations with protocol calls.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.2.1 | Remove `agent_pool` field from `TuiState` | Todo | - |
| T8.2.2 | Replace `agent_pool.spawn()` with `client.call("agent.spawn", ...)` | Todo | - |
| T8.2.3 | Replace `agent_pool.stop()` with `client.call("agent.stop", ...)` | Todo | - |
| T8.2.4 | Replace `agent_pool.statuses()` with `TuiState::agents` (from events) | Todo | - |
| T8.2.5 | Update agent overview rendering to use `AgentSnapshot` | Todo | - |
| T8.2.6 | Write integration test: spawn/stop via protocol updates TUI correctly | Todo | - |

#### Acceptance Criteria

- `AgentPool` is not imported in any TUI source file
- Agent spawn/stop go through WebSocket protocol calls
- Agent status is derived from `Event` stream, not direct pool queries
- **Tests**: `spawn_via_protocol` — Ctrl+S sends `agent.spawn`; `stop_via_protocol` — Ctrl+Q sends `agent.stop`; `no_agent_pool_import` — grep confirms no `AgentPool` in TUI


#### Technical Notes

See IMP-06 §7. The `Ctrl+S` spawn keybinding now sends a protocol request. The response confirms spawn; the `AgentSpawned` event updates the UI.

---

### Story 8.3: Remove EventAggregator from TuiState

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Delete the `event_aggregator` field. The TUI no longer polls provider channels directly.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.3.1 | Remove `event_aggregator` field from `TuiState` | Todo | - |
| T8.3.2 | Remove all `event_aggregator.poll()` calls from `app_loop.rs` | Todo | - |
| T8.3.3 | Remove `AgentEvent` import (replaced by `Event` from protocol) | Todo | - |
| T8.3.4 | Verify no `mpsc::Receiver<ProviderEvent>` usage remains | Todo | - |
| T8.3.5 | Write compilation check: no `EventAggregator` references | Todo | - |

#### Acceptance Criteria

- `EventAggregator` is not imported in any TUI source file
- No polling of provider channels in the TUI event loop
- All events arrive via WebSocket `Event` notifications
- **Tests**: `no_event_aggregator_import` — grep confirms no `EventAggregator` in TUI; `events_only_via_websocket` — no `mpsc::Receiver<ProviderEvent>` usage


#### Technical Notes

See IMP-04 §2. This is a straightforward deletion — the WebSocket event stream replaces the aggregator entirely.

---

### Story 8.4: Remove Mailbox from TuiState

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Delete the `mailbox` field. Cross-agent mail is handled by the daemon.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.4.1 | Remove `mailbox` field from `TuiState` | Todo | - |
| T8.4.2 | Replace mail display with `MailReceived` event handler | Todo | - |
| T8.4.3 | Remove `AgentMailbox` import | Todo | - |
| T8.4.4 | Write compilation check: no mailbox references | Todo | - |

#### Acceptance Criteria

- `AgentMailbox` is not imported in any TUI source file
- Mail notifications come via `Event::MailReceived`
- **Tests**: `no_mailbox_import` — grep confirms no `Mailbox` in TUI; `mail_via_event` — `MailReceived` updates UI


---

### Story 8.5: Remove agent-core Dependency from Cargo.toml

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Clean up `Cargo.toml` and fix any remaining indirect imports.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.5.1 | Remove `agent-core` from `tui/Cargo.toml` | Todo | - |
| T8.5.2 | Remove `agent-decision` from `tui/Cargo.toml` | Todo | - |
| T8.5.3 | Remove `agent-kanban` from `tui/Cargo.toml` | Todo | - |
| T8.5.4 | Fix any remaining compilation errors from removed dependencies | Todo | - |
| T8.5.5 | Verify `cargo build -p agent-tui` succeeds | Todo | - |
| T8.5.6 | Verify `cargo test -p agent-tui` succeeds | Todo | - |
| T8.5.7 | Update `AGENTS.md` if TUI dependencies are documented there | Todo | - |

#### Acceptance Criteria

- `cargo build -p agent-tui` compiles with zero errors
- `cargo test -p agent-tui` passes all tests
- `Cargo.toml` has no `agent-core`, `agent-decision`, or `agent-kanban` dependencies
- `agent-protocol` is the only internal agent crate dependency
- **Tests**: `cargo_build_succeeds` — `cargo build -p agent-tui` passes; `cargo_test_passes` — `cargo test -p agent-tui` passes; `dependency_audit` — `cargo tree` shows no core deps


#### Technical Notes

See IMP-06 §2.1. This is the final validation that decoupling is complete. Any remaining compilation errors indicate missed imports.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Hidden transitive dependency on core | Medium | High | `cargo tree -p agent-tui` to audit; fix at each layer |
| TUI test breakage from removed types | Medium | Medium | Update test mocks to use protocol types |
| Re-export stubs in core hide imports | Low | High | Search for `pub use` in `core/src/lib.rs` that TUI relied on |

## Sprint Deliverables

- `TuiState` with only protocol and local render fields
- `tui/Cargo.toml` with no core dependencies
- Clean compilation and test suite
- Updated documentation

## Dependencies

- [Sprint 7: TUI WebSocket Client + Event Handler](./sprint-07-tui-client-event-handler.md) — TUI must already receive events before core can be removed.

## Next Sprint

After completing this sprint, proceed to [Sprint 9: CLI Refactor](./sprint-09-cli-refactor.md) to make the CLI an independent protocol client.
