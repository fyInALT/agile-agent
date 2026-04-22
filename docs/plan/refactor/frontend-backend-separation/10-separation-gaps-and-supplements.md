# Gaps Analysis & Supplementary Tasks

> Status: Draft  
> Date: 2026-04-20  
> Scope: Systematic gap analysis of the frontend-backend separation plan, with supplementary tasks derived from production systems engineering experience

This document audits the existing separation plan for omissions that would create operational pain, security holes, or maintenance debt in a production environment. Every gap is rated by **severity** (P0 = must fix before release, P1 = should fix, P2 = nice to have) and assigned to a sprint.

---

## 1. Audit Methodology

We evaluate coverage across eight engineering dimensions:

| Dimension | Existing Coverage | Gap Level |
|-----------|------------------|-----------|
| Architecture & Protocol | Comprehensive (IMP-01–07) | Low |
| State & Event Streaming | Comprehensive (IMP-04–06) | Low |
| Client Refactor | Comprehensive (IMP-06–07, Sprint 7–9) | Low |
| Testing | Strong (IMP-08, TDD per story) | Low |
| Migration | Strong (IMP-09, Sprint 11) | Low |
| Observability & Diagnostics | None | **Critical** |
| Resource Limits & Backpressure | None | **Critical** |
| Security & Hardening | Mentioned but not specified | **High** |
| Configuration Management | Minimal (CLI args only) | **High** |
| Disaster Recovery | Partial (snapshot, replay) | Medium |
| Multi-Platform Support | None | Medium |
| Developer Experience | None | Medium |

---

## 2. Gap Details & Supplementary Tasks

### 2.1 Observability & Diagnostics (P0)

**Gap**: The daemon is a background process. Without structured logging, metrics, and health checks, operators cannot diagnose failures, performance regressions, or resource leaks.

**Missing coverage**:
- No structured logging strategy for the daemon (log levels, rotation, format)
- No performance metrics (event throughput, connection count, memory usage)
- No health check endpoint (`/health` HTTP endpoint or `session.health` JSON-RPC method)
- No debugging interface (dump current state, list connected clients, force snapshot)
- No log correlation (request IDs propagated across daemon → client)

**Supplementary tasks**:
1. **Structured Logging**: Use `tracing` + `tracing-subscriber` with JSON output option. Include `request_id`, `connection_id`, `agent_id` in every span.
2. **Metrics Endpoint**: Expose Prometheus-compatible metrics on `/metrics` (HTTP, separate from WebSocket). Metrics: `agent_events_total`, `websocket_connections_active`, `websocket_messages_sent_total`, `snapshot_generation_duration_seconds`, `event_log_size_bytes`.
3. **Health Check**: `GET /health` returns `{"status":"ok","version":"1.0.0","connections":3,"agents":2}`. Used by auto-link to verify daemon liveness instead of PID checks.
4. **Debug Commands**: `session.debugDump` JSON-RPC method returns internal state (connections, agent slots, event queue depth) for troubleshooting.
5. **Log Rotation**: Daemon logs rotate daily, retain 7 days. Configurable via `daemon.toml`.

**Assigned to**: New Sprint 12 — Observability & Operational Readiness

---

### 2.2 Resource Limits & Backpressure (P0)

**Gap**: The daemon has no resource boundaries. An unbounded event log, unbounded connections, or unbounded transcript growth will eventually cause OOM or disk exhaustion.

**Missing coverage**:
- No maximum connection limit
- No maximum agent pool size (beyond core's internal limit)
- No event log size cap or rotation
- No transcript size limit in snapshots
- No rate limiting on client requests
- No backpressure on event broadcast (unbounded channels)

**Supplementary tasks**:
1. **Connection Limit**: Max 10 concurrent WebSocket connections. New connections rejected with JSON-RPC error `-32000` + `data.max_connections = 10`.
2. **Event Log Rotation**: When `events.jsonl` exceeds 100MB, rotate to `events-2026-04-20.jsonl` and start a new file. Keep last 5 rotation files.
3. **Snapshot Transcript Pagination**: `SessionState` includes only the last 1000 transcript items by default. Add `session.loadHistory(offset, limit)` method for on-demand loading.
4. **Request Rate Limiting**: Per-connection rate limit: 100 requests/minute. Excess requests receive `-32000` + `data.retry_after`.
5. **Memory Alert**: If daemon RSS exceeds 500MB, log a warning and trigger an emergency snapshot write.

**Assigned to**: New Sprint 12 — Observability & Operational Readiness

---

### 2.3 Security Hardening — v1 Scope (P1)

**Gap**: While v1 is localhost-only, several attack vectors exist in shared-machine environments (CI runners, shared dev servers, pair programming).

**Missing coverage**:
- No WebSocket origin validation
- No CSRF protection (WebSocket lacks same-origin policy)
- daemon.json is world-readable by default
- No input sanitization on protocol params (could inject into logs)
- No connection authentication (any process on the machine can connect)

**Supplementary tasks**:
1. **Origin Validation**: Reject WebSocket connections without `Origin: http://localhost` header (configurable).
2. **daemon.json Permissions**: Set `0o600` permissions on `daemon.json` so only the owner can read the WebSocket URL.
3. **Input Sanitization**: All string params are length-limited (max 1MB for `text`, max 256 chars for IDs). Truncate with warning log.
4. **Connection Token (optional)**: `daemon.json` includes a `token` field. Clients must include `Authorization: Bearer <token>` in the WebSocket handshake. Disabled by default, enabled via config.
5. **Audit Log**: Log all `agent.spawn`, `agent.stop`, `tool.approve` actions with timestamp and client ID to `audit.jsonl`.

**Assigned to**: New Sprint 13 — Security Hardening & Platform Support

---

### 2.4 Configuration Management (P1)

**Gap**: Daemon configuration is limited to CLI arguments. There is no config file, no environment variable support, and no runtime config reloading.

**Missing coverage**:
- No `daemon.toml` or `daemon.yaml` config file
- No environment variable overrides
- No config validation at startup
- No hot reload

**Supplementary tasks**:
1. **Config File**: `~/.agile-agent/daemon.toml` with sections for `[server]`, `[logging]`, `[limits]`, `[security]`.
2. **Env Override**: `AGILE_AGENT_LOG_LEVEL=debug`, `AGILE_AGENT_MAX_CONNECTIONS=20` override config file values.
3. **Validation**: Daemon refuses to start with invalid config (unknown keys, out-of-range values) and prints actionable errors.
4. **Defaults**: Sensible defaults for all fields. Config file is optional — defaults work out of the box.

**Config example**:
```toml
[server]
bind_address = "127.0.0.1"
max_connections = 10

[logging]
level = "info"
format = "json"  # or "pretty"
output = "file"  # or "stderr"
path = "~/.agile-agent/daemon.log"

[limits]
max_event_log_size_mb = 100
max_transcript_items_per_snapshot = 1000
request_rate_limit_per_minute = 100

[security]
origin_validation = true
daemon_json_permissions = 0o600
token_auth = false
```

**Assigned to**: New Sprint 12 — Observability & Operational Readiness

---

### 2.5 Disaster Recovery (P1)

**Gap**: Events log and snapshot files can be corrupted by crashes or disk errors. There is no recovery procedure.

**Missing coverage**:
- No checksum for snapshot files
- No recovery from truncated events.jsonl
- No backup strategy
- No graceful handling of full disk

**Supplementary tasks**:
1. **Snapshot Checksum**: Write `snapshot.json` with a SHA-256 checksum field. Verify on read.
2. **Truncated Log Recovery**: If `events.jsonl` ends with a partial line (crash during write), truncate to the last complete line and log a warning.
3. **Disk Full Handling**: Before write, check available disk space. If < 100MB, log critical error and enter read-only mode (no new events persisted, broadcast only).
4. **Backup on Shutdown**: Copy `snapshot.json` and `events.jsonl` to `~/.agile-agent/backups/<timestamp>/` on graceful shutdown. Keep last 3 backups.

**Assigned to**: New Sprint 13 — Security Hardening & Platform Support

---

### 2.6 Multi-Platform Support (P2)

**Gap**: The plan assumes Linux behavior for process management, signal handling, and filesystem paths.

**Missing coverage**:
- Windows daemon spawn (no fork, different process model)
- Windows signal handling (no SIGTERM)
- macOS filesystem permissions differences
- Cross-platform `daemon.json` path handling

**Supplementary tasks**:
1. **Windows Process Management**: Use `tokio::process` with job objects to ensure daemon cleanup when parent exits.
2. **Windows Signal Alternative**: Use named pipe or Ctrl+C event instead of SIGTERM.
3. **Cross-Platform Paths**: Use `dirs::data_dir()` for config location instead of hardcoded `~/.agile-agent/`.
4. **CI Matrix**: Test on Ubuntu, macOS, and Windows in CI.

**Assigned to**: New Sprint 13 — Security Hardening & Platform Support

---

### 2.7 Developer Experience (P2)

**Gap**: No guidance for developers working on the separated system. How do you debug a daemon? How do you run TUI and daemon in dev mode?

**Missing coverage**:
- No dev-mode daemon (foreground, verbose logging)
- No debugging guide
- No `justfile` / `Makefile` targets for common dev tasks

**Supplementary tasks**:
1. **Dev Mode**: `agent-daemon --foreground --log-level debug` runs in foreground with pretty logs.
2. **Dev Justfile**: `just dev-daemon`, `just dev-tui`, `just test-e2e` commands.
3. **Debugging Guide**: Document attaching `rust-gdb` to daemon, reading `events.jsonl`, using `session.debugDump`.

**Assigned to**: Sprint 11 extension (add Story 11.6)

---

### 2.8 Protocol Extension Points (P2)

**Gap**: The protocol v1 design does not explicitly预留 extension points for v2 features.

**Missing coverage**:
- No extension field in message envelope
- No plugin method namespace
- No capability negotiation beyond version

**Supplementary tasks**:
1. **Extension Field**: Add `ext: Option<serde_json::Value>` to all message types for forward-compatible extensions.
2. **Capability Negotiation**: `session.initialize` response includes `capabilities: ["events", "approvals", "decisions"]` so clients can adapt.
3. **Plugin Namespace**: Reserve `plugin.*` method namespace for third-party extensions.

**Assigned to**: Sprint 1 extension (add to `session.initialize` design)

---

## 3. Sprint Reassignment Summary

| Gap | Severity | New / Existing Sprint | Story |
|-----|----------|----------------------|-------|
| Observability | P0 | **Sprint 12** (new) | Structured logging, metrics, health check, debug commands |
| Resource Limits | P0 | **Sprint 12** (new) | Connection limit, log rotation, pagination, rate limiting |
| Configuration | P1 | **Sprint 12** (new) | Config file, env override, validation, defaults |
| Security | P1 | **Sprint 13** (new) | Origin validation, permissions, token auth, audit log |
| Disaster Recovery | P1 | **Sprint 13** (new) | Checksums, truncated log recovery, disk full, backups |
| Multi-Platform | P2 | **Sprint 13** (new) | Windows process management, cross-platform paths, CI matrix |
| Developer Experience | P2 | Sprint 11 | Add Story 11.6: Dev mode, justfile, debugging guide |
| Protocol Extensions | P2 | Sprint 1 | Add `ext` field and capability negotiation to IMP-02 |

---

## 4. Revised Sprint Roadmap

```
Sprint 1–11: Core separation (unchanged)
Sprint 12: Observability & Operational Readiness (new)
Sprint 13: Security Hardening & Platform Support (new)
```

**Revised total**: 26 weeks (≈ 6.5 months), +5 story points

---

## 5. Risk: Analysis Paralysis

This gap analysis intentionally identifies more work than can be done. The principle is: **document the gap, prioritize ruthlessly, schedule what fits, defer the rest.**

Deferred items (post-release):
- Multi-platform support (if initial release targets Linux only)
- Protocol extension points (if no v2 planned within 6 months)
- Advanced security (token auth) if all users are single-machine single-user

Mandatory items (before release):
- Observability (you cannot ship a background process without logs)
- Resource limits (you cannot ship a process that grows unbounded)
- Configuration management (CLI args are insufficient for production)
