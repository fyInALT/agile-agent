# Sprint 12: Observability & Operational Readiness

## Metadata

- Sprint ID: `sprint-fbs-012`
- Title: `Observability & Operational Readiness`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 11: Cleanup + Performance](./sprint-11-cleanup-performance.md)

## Background

The core separation is feature-complete, but the daemon is a black box. It runs in the background with no structured logs, no metrics, no health checks, and no resource boundaries. An operator encountering a hung daemon or memory leak has no diagnostic tools. The event log grows unbounded. Configuration is limited to CLI arguments. These are not optional polish items — they are prerequisites for running the daemon in any production-like environment.

This sprint transforms the daemon from a working prototype into an operable service. It adds structured logging, Prometheus metrics, a health endpoint, resource limits, and a configuration file system. Without these capabilities, the next phase (security hardening, multi-platform support) cannot be validated because there is no visibility into daemon behavior.

## Sprint Goal

The daemon is observable, bounded, and configurable. Operators can query its health, read structured logs, monitor metrics, and tune behavior via config file. The system does not exhaust disk or memory under normal load.

## TDD Approach

Observability code is tested differently from business logic — we test outputs, not states.

1. **Red**: Write tests that assert the presence and format of log output, metrics, and config validation errors.
2. **Green**: Implement logging, metrics, limits, and config until tests pass.
3. **Refactor**: Extract common instrumentation patterns; ensure no panics in metric collection.

Test requirements per story:
- Log capture tests: redirect `tracing` output to a buffer, assert expected lines appear
- Metric tests: scrape the `/metrics` endpoint, assert counters/gauges have expected values
- Config tests: invalid config produces specific validation errors; valid config round-trips
- Limit tests: trigger limits and assert graceful rejection (not panics)
- All tests use isolated temp dirs and reset global state between tests

## Stories

### Story 12.1: Structured Logging & Tracing

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement structured logging for the daemon using `tracing` with JSON and pretty formats.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.1.1 | Add `tracing` and `tracing-subscriber` to `agent-daemon` dependencies | Todo | - |
| T12.1.2 | Initialize subscriber on daemon startup with configurable format | Todo | - |
| T12.1.3 | Add spans for every WebSocket connection (`connection_id`, `client_type`) | Todo | - |
| T12.1.4 | Add spans for every JSON-RPC request (`request_id`, `method`) | Todo | - |
| T12.1.5 | Log all state mutations (agent spawn/stop, input received) at `info` level | Todo | - |
| T12.1.6 | Log all errors at `error` level with context | Todo | - |
| T12.1.7 | Implement log rotation: daily, retain 7 days | Todo | - |
| T12.1.8 | Write test: JSON log output contains expected fields | Todo | - |

#### Acceptance Criteria

- Every log line includes timestamp, level, target, and message
- JSON format includes `connection_id`, `request_id`, `agent_id` when applicable
- Log file rotates daily without dropping messages
- Old logs (> 7 days) are deleted automatically
- **Tests**: `log_json_format` — JSON output matches schema; `log_rotation` — rotation triggers correctly; `log_span_context` — request ID propagated through span

#### Technical Notes

Use `tracing-appender` for non-blocking file output. Use `tracing-subscriber::fmt::layer().json()` for JSON mode. Connection spans should be entered in `Connection::spawn()` and exited on disconnect.

---

### Story 12.2: Metrics & Health Check

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Expose Prometheus-compatible metrics and a health check endpoint.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.2.1 | Add HTTP server on a separate port (`metrics_port = daemon_port + 1`) | Todo | - |
| T12.2.2 | Implement `GET /health` returning JSON status | Todo | - |
| T12.2.3 | Implement `GET /metrics` returning Prometheus text format | Todo | - |
| T12.2.4 | Add counter `agent_events_total` (labeled by event type) | Todo | - |
| T12.2.5 | Add gauge `websocket_connections_active` | Todo | - |
| T12.2.6 | Add histogram `snapshot_generation_duration_seconds` | Todo | - |
| T12.2.7 | Add gauge `event_log_size_bytes` | Todo | - |
| T12.2.8 | Write test: `/health` returns 200 with expected fields | Todo | - |
| T12.2.9 | Write test: `/metrics` contains all defined metrics | Todo | - |

#### Acceptance Criteria

- `/health` returns HTTP 200 with `status`, `version`, `connections`, `agents`
- `/metrics` returns valid Prometheus text format
- Metrics update in real time (no batching delay > 1s)
- Health check is used by auto-link to verify daemon liveness
- **Tests**: `health_ok` — returns 200; `health_degraded` — returns 503 when event pump panics; `metrics_prometheus_format` — parseable by `prometheus-parser`

#### Technical Notes

Use `axum` or `hyper` for the HTTP server. Keep it lightweight — no routing beyond `/health` and `/metrics`. The metrics port is ephemeral (OS-assigned) and written to `daemon.json`.

---

### Story 12.3: Resource Limits & Backpressure

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement hard limits on connections, event log size, request rate, and memory.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.3.1 | Implement max WebSocket connections (default: 10) | Todo | - |
| T12.3.2 | Reject new connections with JSON-RPC error when limit reached | Todo | - |
| T12.3.3 | Implement per-connection request rate limit (default: 100/min) | Todo | - |
| T12.3.4 | Implement event log rotation at 100MB | Todo | - |
| T12.3.5 | Implement snapshot transcript pagination (default: 1000 items) | Todo | - |
| T12.3.6 | Add `session.loadHistory(offset, limit)` method | Todo | - |
| T12.3.7 | Monitor daemon RSS; log warning at 500MB | Todo | - |
| T12.3.8 | Write test: connection limit enforced | Todo | - |
| T12.3.9 | Write test: rate limit returns correct error | Todo | - |
| T12.3.10 | Write test: event log rotates at threshold | Todo | - |

#### Acceptance Criteria

- 11th connection is rejected with error code `-32000` + `data.max_connections`
- Rate-limited requests receive `-32000` + `data.retry_after`
- Event log rotates when size exceeds 100MB; old logs retained for 5 rotations
- Snapshot includes only last 1000 transcript items; older items loadable via `loadHistory`
- Memory warning logged when RSS > 500MB
- **Tests**: `connection_limit_rejected` — 11th connection fails; `rate_limit_triggered` — 101st request/min fails; `log_rotation_threshold` — rotation at 100MB; `memory_warning` — warning logged at threshold

#### Technical Notes

Rate limiting uses a token bucket per connection. Event log rotation checks size before each append. Memory monitoring uses `sysinfo` crate on a 30s interval.

---

### Story 12.4: Configuration Management

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement a TOML configuration file with environment variable overrides.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.4.1 | Define `DaemonConfig` struct with all configurable fields | Todo | - |
| T12.4.2 | Implement TOML parsing with `serde` + `toml` | Todo | - |
| T12.4.3 | Implement env var override (`AGILE_AGENT_<SECTION>_<KEY>`) | Todo | - |
| T12.4.4 | Validate config at startup (unknown keys, out-of-range values) | Todo | - |
| T12.4.5 | Generate default config file on first daemon start | Todo | - |
| T12.4.6 | Reload logging config on `SIGHUP` (optional) | Todo | - |
| T12.4.7 | Write test: valid config parses correctly | Todo | - |
| T12.4.8 | Write test: invalid config produces actionable error | Todo | - |
| T12.4.9 | Write test: env var overrides config file value | Todo | - |

#### Acceptance Criteria

- Config file at `~/.agile-agent/daemon.toml` is optional (defaults work)
- Invalid config prevents daemon startup with clear error message
- Env vars override config file values
- Default config is generated with comments explaining each field
- **Tests**: `config_defaults` — daemon starts without config file; `config_invalid_rejected` — bad port number produces error; `config_env_override` — env var takes precedence

#### Technical Notes

Use `config` crate for layered config (defaults → file → env). Config validation happens before any server binding. Reload on `SIGHUP` only affects logging level, not server binding.

---

### Story 12.5: Debug Commands

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Add JSON-RPC methods for runtime introspection and troubleshooting.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T12.5.1 | Implement `session.debugDump` — returns connections, agents, event queue depth | Todo | - |
| T12.5.2 | Implement `session.forceSnapshot` — triggers immediate snapshot write | Todo | - |
| T12.5.3 | Implement `session.listConnections` — returns all connected clients | Todo | - |
| T12.5.4 | Restrict debug commands to CLI client type (not TUI) | Todo | - |
| T12.5.5 | Write test: `debugDump` returns expected structure | Todo | - |
| T12.5.6 | Write test: debug commands rejected for TUI client type | Todo | - |

#### Acceptance Criteria

- `session.debugDump` returns internal state without exposing secrets
- `session.forceSnapshot` writes snapshot.json immediately
- Debug commands are logged at `warn` level (audit trail)
- TUI clients receive `-32106` (Not supported) for debug commands
- **Tests**: `debug_dump_structure` — response contains expected fields; `debug_tui_rejected` — TUI client gets error; `force_snapshot_writes` — file updated immediately

#### Technical Notes

Debug commands are intended for troubleshooting, not normal operation. They may expose internal details — restrict to trusted clients. Log all debug command invocations for audit.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Metrics HTTP server conflicts with existing port | Low | Medium | Use ephemeral port; include in daemon.json |
| Log rotation loses messages during rotation | Low | High | Use `tracing-appender` which handles this internally |
| Config validation too strict (breaks existing setups) | Medium | Medium | Start with warnings, escalate to errors in next release |

## Sprint Deliverables

- `agent/daemon/src/logging.rs` — structured logging setup
- `agent/daemon/src/metrics.rs` — Prometheus metrics and HTTP server
- `agent/daemon/src/config.rs` — TOML config parsing and validation
- `agent/daemon/src/limits.rs` — resource limit enforcement
- `agent/daemon/src/debug.rs` — debug command handlers
- Updated `daemon.toml` generation
- Integration tests for all observability features

## Dependencies

- [Sprint 11: Cleanup + Performance](./sprint-11-cleanup-performance.md) — core separation must be stable before adding instrumentation.

## Next Sprint

After completing this sprint, proceed to [Sprint 13: Security Hardening & Platform Support](./sprint-13-security-platform.md).
