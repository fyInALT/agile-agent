# Sprint 3: Auto-Link + Daemon Lifecycle

## Metadata

- Sprint ID: `sprint-fbs-003`
- Title: `Auto-Link + Daemon Lifecycle`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 2: Daemon Skeleton](./sprint-02-daemon-skeleton.md)

## Background

The daemon can now accept WebSocket connections, but clients have no way to find it. A user running `agent-cli` or `agent-tui` in a workplace directory must manually know the WebSocket URL — which changes on every restart because the port is ephemeral. There is no persistence of daemon location, no discovery mechanism, and no graceful shutdown protocol.

This sprint solves the "where is my daemon?" problem. It introduces `daemon.json` as the discovery contract, implements auto-link (discover existing → spawn new if missing), and makes the daemon a well-behaved service that cleans up after itself. Without auto-link, every subsequent sprint would require manual daemon management, blocking all user-facing work.

## Sprint Goal

The daemon can write its configuration to disk on startup and remove it on shutdown. A client can discover an existing daemon or spawn a new one automatically. Graceful shutdown persists a snapshot and cleans up resources. This is the "daemon as a service" milestone.

## TDD Approach

Daemon lifecycle and auto-link involve filesystem I/O and process management — tests must be hermetic and parallel-safe.

1. **Red**: Write tests using `tempfile::TempDir` for isolated filesystem state.
2. **Green**: Implement config I/O, spawn logic, and signal handling until tests pass.
3. **Refactor**: Extract process-spawn utilities; use `CARGO_BIN_EXE_agent-daemon` for binary resolution in tests.

Test requirements per story:
- Filesystem tests in temp dirs (no pollution of real `~/.agile-agent/`)
- Unit tests for `DaemonConfig` serialization and validation
- Integration tests: daemon starts, writes config, stops, cleans up
- Auto-link tests: existing daemon → connect; missing daemon → spawn; stale config → respawn
- Signal tests: send SIGTERM, assert graceful shutdown sequence
- All process-spawn tests have timeouts to prevent hanging CI

## Stories

### Story 3.1: daemon.json Format + I/O

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define and implement the `daemon.json` configuration file that advertises a running daemon to clients.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Define `DaemonConfig` struct (`version`, `pid`, `websocket_url`, `workplace_id`, `alias`, `started_at`, `last_heartbeat`) | Todo | - |
| T3.1.2 | Implement `DaemonConfig::write(path)` with atomic write (temp file + rename) | Todo | - |
| T3.1.3 | Implement `DaemonConfig::read(path)` with validation | Todo | - |
| T3.1.4 | Add schema version field (current: `1`) | Todo | - |
| T3.1.5 | Write unit test: round-trip write → read produces identical config | Todo | - |
| T3.1.6 | Write unit test: corrupted daemon.json is rejected gracefully | Todo | - |
| T3.1.7 | Write unit test: concurrent writes are safe (atomic rename) | Todo | - |

#### Acceptance Criteria

- `daemon.json` is valid JSON with all required fields
- Write is atomic (no half-written files observable)
- Read validates schema version and rejects unknown versions
- `last_heartbeat` is updated every 30s by a background task
- **Tests**: `config_roundtrip` — write → read produces identical config; `config_corrupted` — corrupted file rejected gracefully; `config_atomic_write` — no half-written files


#### Technical Notes

See IMP-03 §3.2. The config lives at `~/.agile-agent/workplaces/<id>/daemon.json`. Use `tokio::fs::write` with a temp file + `rename` for atomicity.

---

### Story 3.2: Daemon Startup Sequence

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the daemon startup: parse args, bootstrap workplace, bind WebSocket, write config, signal readiness.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Add CLI args: `--workplace-id`, `--alias`, `--log-file` | Todo | - |
| T3.2.2 | Resolve workplace from `WorkplaceStore::for_cwd()` | Todo | - |
| T3.2.3 | Bind WebSocket server and capture ephemeral port | Todo | - |
| T3.2.4 | Write `daemon.json` with `pid`, `websocket_url`, `started_at` | Todo | - |
| T3.2.5 | Spawn heartbeat updater task (updates `last_heartbeat` every 30s) | Todo | - |
| T3.2.6 | Spawn WebSocket accept loop | Todo | - |
| T3.2.7 | Write integration test: daemon starts, daemon.json appears, port is reachable | Todo | - |
| T3.2.8 | Write integration test: two daemons on different workplaces get different ports | Todo | - |

#### Acceptance Criteria

- Daemon starts and writes `daemon.json` within 1 second
- `websocket_url` contains the actual assigned port
- `pid` matches the daemon's OS process ID
- Two daemons for different workplaces bind to different ports
- **Tests**: `startup_writes_config` — daemon.json appears within 1s; `startup_unique_port` — two daemons get different ports


#### Technical Notes

See IMP-03 §3.1. The daemon is spawned by the client, not run directly by users. `WorkplaceStore` already exists in `agent-core` — reuse it.

---

### Story 3.3: Graceful Shutdown + Snapshot

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement signal handling, connection drain, agent shutdown, snapshot write, and config cleanup.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Handle `SIGTERM` and `SIGINT` via `tokio::signal` | Todo | - |
| T3.3.2 | Set shutdown flag to stop accepting new connections | Todo | - |
| T3.3.3 | Close all WebSocket connections with code `1001` (Going Away) | Todo | - |
| T3.3.4 | Wait up to 5s for graceful client disconnects | Todo | - |
| T3.3.5 | Write `snapshot.json` with `SessionState` (hardcoded for now) | Todo | - |
| T3.3.6 | Delete `daemon.json` to signal daemon is gone | Todo | - |
| T3.3.7 | Exit with code `0` (clean) or `1` (error) | Todo | - |
| T3.3.8 | Write integration test: SIGTERM triggers full shutdown sequence | Todo | - |

#### Acceptance Criteria

- `SIGTERM` triggers shutdown within 100ms
- All clients receive close frame `1001`
- `daemon.json` is deleted after shutdown
- `snapshot.json` is written with schema version `1`
- No dangling processes or file descriptors
- **Tests**: `shutdown_sigterm` — SIGTERM triggers full sequence; `shutdown_cleanup` — daemon.json deleted after exit; `shutdown_snapshot` — snapshot.json written


#### Technical Notes

See IMP-03 §4. Use `tokio::select!` in the main loop to wait for either the accept loop or the shutdown signal. Snapshot content is hardcoded this sprint — real data in Sprint 4.

---

### Story 3.4: Client Auto-Link

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the client-side logic that discovers or spawns the daemon for the current working directory.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Implement `WorkplaceStore::for_cwd()` (reuse from core) | Todo | - |
| T3.4.2 | Implement `auto_link()` — check daemon.json → validate PID → connect | Todo | - |
| T3.4.3 | Implement stale config detection (`kill -0` PID check) | Todo | - |
| T3.4.4 | Implement daemon spawn: `tokio::process::Command` with binary path resolution | Todo | - |
| T3.4.5 | Implement wait-for-daemon loop (poll daemon.json with timeout) | Todo | - |
| T3.4.6 | Implement exponential backoff on connection failure | Todo | - |
| T3.4.7 | Write integration test: auto-link connects to existing daemon | Todo | - |
| T3.4.8 | Write integration test: auto-link spawns daemon when none exists | Todo | - |
| T3.4.9 | Write integration test: stale config triggers respawn | Todo | - |

#### Acceptance Criteria

- Auto-link succeeds when daemon is already running
- Auto-link spawns daemon when none exists (within 10s)
- Stale config (dead PID) is detected and removed before respawn
- Exponential backoff: 100ms → 200ms → 400ms → ... → 5s max
- Works from both CLI and TUI contexts
- **Tests**: `autolink_existing` — connects to running daemon; `autolink_spawn` — spawns daemon when missing; `autolink_stale` — stale PID triggers respawn


#### Technical Notes

See IMP-06 §4.3 and IMP-07 §4.2. Put auto-link in `agent-protocol/src/client/auto_link.rs` so both CLI and TUI share it. Use `CARGO_BIN_EXE_agent-daemon` env var in tests to find the daemon binary.

---

### Story 3.5: Heartbeat Protocol

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Implement client heartbeat and daemon heartbeat acknowledgment to detect stale connections.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.5.1 | Implement `session.heartbeat` handler in daemon (no-op, update internal timestamp) | Todo | - |
| T3.5.2 | Implement `session.heartbeatAck` notification from daemon | Todo | - |
| T3.5.3 | Implement client-side heartbeat sender (every 30s) | Todo | - |
| T3.5.4 | Implement daemon-side timeout: close connection after 120s without heartbeat | Todo | - |
| T3.5.5 | Write integration test: client heartbeat keeps connection alive | Todo | - |
| T3.5.6 | Write integration test: missing heartbeat triggers disconnect | Todo | - |

#### Acceptance Criteria

- Client sends heartbeat every 30s
- Daemon responds with `heartbeatAck` within 100ms
- Connection closed with `1001` after 120s of silence
- No false disconnects during normal operation
- **Tests**: `heartbeat_keeps_alive` — heartbeat every 30s prevents disconnect; `heartbeat_timeout` — 120s silence triggers close with `1001`


#### Technical Notes

See IMP-01 §3.3 and §4.4. The heartbeat is a Notification (no response expected by JSON-RPC spec, but we send `heartbeatAck` as a courtesy).

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| PID reuse causes false "daemon alive" | Low | High | Include `started_at` in config; compare with process start time |
| Daemon spawn race (two clients spawn simultaneously) | Medium | Medium | File-lock on daemon.json during spawn; second client waits |
| Port exhaustion on busy machines | Very Low | Low | Ephemeral ports (bind to 0); OS manages pool |

## Sprint Deliverables

- `agent/daemon/src/config.rs` — `DaemonConfig` read/write
- `agent/daemon/src/lifecycle.rs` — startup/shutdown/signals
- `agent/protocol/src/client/auto_link.rs` — shared auto-link logic
- `agent/daemon/src/main.rs` — complete startup sequence
- Integration tests: daemon lifecycle, auto-link, heartbeat

## Dependencies

- [Sprint 2: Daemon Skeleton](./sprint-02-daemon-skeleton.md) — WebSocket server and router must exist.

## Next Sprint

After completing this sprint, proceed to [Sprint 4: SessionManager + Snapshot](./sprint-04-session-manager-snapshot.md) to move runtime state ownership into the daemon.
