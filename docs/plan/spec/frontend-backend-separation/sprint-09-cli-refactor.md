# Sprint 9: CLI Refactor

## Metadata

- Sprint ID: `sprint-fbs-009`
- Title: `CLI Refactor`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 8: TUI Decoupling](./sprint-08-tui-decoupling.md)

## Sprint Goal

The CLI is a standalone protocol client with no dependency on `agent-tui` or `agent-core`. All commands (except `doctor`/`probe`) communicate with the daemon via JSON-RPC. New `daemon start/stop/status` commands manage the daemon lifecycle. The CLI binary is significantly smaller.

## Stories

### Story 9.1: CLI ProtocolClient

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement a blocking-friendly protocol client for the CLI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.1.1 | Create `ProtocolClient` struct in `cli/src/protocol_client.rs` | Todo | - |
| T9.1.2 | Implement `connect(daemon_url)` with WebSocket handshake | Todo | - |
| T9.1.3 | Implement `request(method, params) -> Result<Response>` | Todo | - |
| T9.1.4 | Implement `notify(method, params)` fire-and-forget | Todo | - |
| T9.1.5 | Implement `subscribe_events() -> Receiver<Event>` | Todo | - |
| T9.1.6 | Integrate auto-link (reuse from `agent-protocol` client module) | Todo | - |
| T9.1.7 | Write unit test: request/response round-trip | Todo | - |
| T9.1.8 | Write unit test: event subscription receives notifications | Todo | - |

#### Acceptance Criteria

- `ProtocolClient` connects to daemon and sends/receives JSON-RPC
- `request()` blocks until response arrives (with timeout)
- `subscribe_events()` returns a channel that receives `Event` values
- Auto-link works identically to TUI

#### Technical Notes

See IMP-07 §4.1. The CLI client is simpler than the TUI client because it does not need to integrate with a render loop. Use `tokio::time::timeout` for request timeouts.

---

### Story 9.2: daemon start/stop/status Commands

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Add daemon lifecycle commands to the CLI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.2.1 | Add `DaemonCommand` enum with `Start`, `Stop`, `Status` variants | Todo | - |
| T9.2.2 | Implement `daemon start` — calls `auto_link()`, prints daemon URL | Todo | - |
| T9.2.3 | Implement `daemon stop` — sends shutdown notification, waits for cleanup | Todo | - |
| T9.2.4 | Implement `daemon status` — reads daemon.json, shows pid, port, uptime | Todo | - |
| T9.2.5 | Add `daemon logs` — tail daemon stderr/log file | Todo | - |
| T9.2.6 | Write integration test: start → status → stop lifecycle | Todo | - |
| T9.2.7 | Write integration test: stop is idempotent (no error if already stopped) | Todo | - |

#### Acceptance Criteria

- `daemon start` succeeds whether daemon exists or not
- `daemon stop` gracefully shuts down the daemon
- `daemon status` shows human-readable daemon info
- `daemon logs` follows the daemon's log output

#### Technical Notes

See IMP-07 §5.1–5.3. `daemon stop` sends a `session.shutdown` notification (or uses SIGTERM). The CLI waits for `daemon.json` to disappear before returning.

---

### Story 9.3: agent.* Commands via Protocol

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Rewrite all agent subcommands to use the protocol instead of direct core access.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.3.1 | Rewrite `agent list` — query daemon, print table | Todo | - |
| T9.3.2 | Rewrite `agent spawn` — send `agent.spawn`, print result | Todo | - |
| T9.3.3 | Rewrite `agent stop` — send `agent.stop`, confirm | Todo | - |
| T9.3.4 | Rewrite `agent status` — send `agent.list` or cache lookup, print details | Todo | - |
| T9.3.5 | Add JSON output mode (`--json`) for all agent commands | Todo | - |
| T9.3.6 | Write integration test: full agent CRUD via CLI | Todo | - |

#### Acceptance Criteria

- All `agent.*` commands work via JSON-RPC
- Table output is human-readable
- `--json` flag produces valid JSON for scripting
- Errors are shown clearly (e.g., "Agent not found: agent-dead")

#### Technical Notes

See IMP-07 §5.4. Use a simple table formatter (e.g., `comfy-table` or manual `println!`). The CLI does not need ratatui for tables.

---

### Story 9.4: run --prompt Headless Execution

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the headless execution mode that sends input and streams output.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.4.1 | Add `run` subcommand with `--prompt` and `--file` args | Todo | - |
| T9.4.2 | Send `session.initialize` + `session.sendInput` via protocol | Todo | - |
| T9.4.3 | Subscribe to events and print `ItemDelta` text to stdout | Todo | - |
| T9.4.4 | Handle `ApprovalRequest` notifications (prompt user or auto-reject) | Todo | - |
| T9.4.5 | Exit when `ItemCompleted` event arrives | Todo | - |
| T9.4.6 | Add `--auto-approve` flag for non-interactive use | Todo | - |
| T9.4.7 | Write integration test: run produces expected output | Todo | - |
| T9.4.8 | Write integration test: --auto-approve handles approvals silently | Todo | - |

#### Acceptance Criteria

- `run --prompt "hello"` sends input and prints output to stdout
- Output streams in real time (not buffered until completion)
- Approval requests prompt the user interactively
- `--auto-approve` silently approves all tool calls
- Exit code is 0 on success, 1 on error

#### Technical Notes

See IMP-07 §5.5. This replaces the old `run-loop` command. The event-driven approach is simpler than the old polling loop.

---

### Story 9.5: Remove agent-tui and agent-core Dependencies

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Clean up `cli/Cargo.toml` and remove all direct core access.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.5.1 | Remove `agent-tui` from `cli/Cargo.toml` | Todo | - |
| T9.5.2 | Remove `agent-core` from `cli/Cargo.toml` | Todo | - |
| T9.5.3 | Remove `agent-decision` from `cli/Cargo.toml` | Todo | - |
| T9.5.4 | Keep `agent-provider` for `doctor`/`probe` commands only | Todo | - |
| T9.5.5 | Fix all compilation errors | Todo | - |
| T9.5.6 | Verify `cargo build -p agent-cli` succeeds | Todo | - |
| T9.5.7 | Verify `cargo test -p agent-cli` passes | Todo | - |
| T9.5.8 | Verify binary size reduction (compare before/after) | Todo | - |

#### Acceptance Criteria

- `cargo build -p agent-cli` compiles with zero errors
- `cargo test -p agent-cli` passes
- `agent-tui` and `agent-core` are not in dependency tree
- Binary size is smaller (no ratatui/crossterm linked)

#### Technical Notes

See IMP-07 §2.1 and §6. `agent-provider` is the only core-adjacent dependency kept, and only for local diagnostics. All other commands go through the protocol.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| CLI tests depend on TUI test fixtures | Medium | High | Extract shared fixtures to `test-support` or protocol-level mocks |
| Binary path resolution fails in tests | Medium | Medium | Use `CARGO_BIN_EXE_*` env vars in tests |
| `--auto-approve` is a security footgun | Low | High | Require explicit flag; never default to auto-approve |

## Sprint Deliverables

- `cli/src/protocol_client.rs` — `ProtocolClient`
- `cli/src/commands/daemon.rs` — daemon lifecycle
- `cli/src/commands/agent.rs` — agent commands via protocol
- `cli/src/commands/run.rs` — headless execution
- Updated `cli/Cargo.toml` with no TUI/core deps
- Integration tests for all CLI commands

## Dependencies

- [Sprint 8: TUI Decoupling](./sprint-08-tui-decoupling.md) — TUI must be decoupled so CLI can drop the TUI dependency.

## Next Sprint

After completing this sprint, proceed to [Sprint 10: Hardening — Reconnect + Approval Flow](./sprint-10-hardening.md) for reliability improvements.
