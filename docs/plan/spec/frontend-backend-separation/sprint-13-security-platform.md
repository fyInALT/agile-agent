# Sprint 13: Security Hardening & Platform Support

## Metadata

- Sprint ID: `sprint-fbs-013`
- Title: `Security Hardening & Platform Support`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 12: Observability & Operational Readiness](./sprint-12-observability-operational-readiness.md)

## Background

The daemon is now observable and bounded, but it trusts all local processes implicitly. In shared environments (CI runners, pair-programming VMs, multi-user servers), any process on the machine can connect to the daemon, spawn agents, and execute arbitrary code through tool approvals. The `daemon.json` file is world-readable, leaking the WebSocket URL. There is no audit trail of who did what.

Additionally, the entire system has been designed and tested on Linux. Windows has no SIGTERM, different process models, and different filesystem permissions. macOS has its own quirks for ephemeral ports and `~` path resolution.

This sprint addresses the security boundary for v1 (localhost scope) and ensures the system runs correctly on all three major platforms. It also adds disaster recovery for data corruption scenarios.

## Sprint Goal

The daemon has basic security controls (origin validation, file permissions, optional token auth), an audit log, disaster recovery for corrupted data, and passes tests on Linux, macOS, and Windows.

## TDD Approach

Security and platform code must be tested with adversarial and cross-platform assumptions.

1. **Red**: Write tests that simulate attacks (unauthorized connection, oversized input, corrupted files) and assert defenses work.
2. **Green**: Implement security controls and platform abstractions until tests pass.
3. **Refactor**: Extract platform-specific code behind traits; keep core logic platform-agnostic.

Test requirements per story:
- Security tests: attempt unauthorized actions, assert rejection
- Corruption tests: corrupt files, assert graceful recovery
- Platform tests: run the same test suite on Linux, macOS, Windows CI
- All security tests run in isolated temp dirs to prevent real system impact

## Stories

### Story 13.1: Security Hardening — v1 Scope

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement basic security controls for the localhost-only daemon.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.1.1 | Validate WebSocket `Origin` header (reject non-localhost origins) | Todo | - |
| T13.1.2 | Set `0o600` permissions on `daemon.json` | Todo | - |
| T13.1.3 | Add input length limits (text ≤ 1MB, IDs ≤ 256 chars) | Todo | - |
| T13.1.4 | Add optional token auth (`Authorization: Bearer <token>` in handshake) | Todo | - |
| T13.1.5 | Generate random token on daemon startup, store in `daemon.json` | Todo | - |
| T13.1.6 | Implement audit log: `~/.agile-agent/audit.jsonl` | Todo | - |
| T13.1.7 | Log all `agent.spawn`, `agent.stop`, `tool.approve` to audit log | Todo | - |
| T13.1.8 | Write test: non-localhost origin rejected | Todo | - |
| T13.1.9 | Write test: oversized input truncated with warning | Todo | - |
| T13.1.10 | Write test: missing token rejected when auth enabled | Todo | - |

#### Acceptance Criteria

- WebSocket connections from non-localhost origins are rejected with `403`
- `daemon.json` has `0o600` permissions
- Input > 1MB is truncated; warning is logged
- Token auth is disabled by default; when enabled, clients must provide valid token
- Audit log contains timestamp, action, client ID, and result for every sensitive operation
- **Tests**: `origin_localhost_only` — non-localhost rejected; `daemon_json_permissions` — `0o600` verified; `input_too_long_truncated` — 2MB text truncated to 1MB; `token_missing_rejected` — unauthorized connection closed; `audit_log_entries` — spawn/stop/approve all logged

#### Technical Notes

Origin validation is a defense-in-depth measure — the daemon binds to `127.0.0.1` already, but some proxies or tunnels could expose it. Token auth uses a 32-byte random string generated with `rand::thread_rng()`.

---

### Story 13.2: Disaster Recovery

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement recovery mechanisms for corrupted or truncated data files.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.2.1 | Add SHA-256 checksum to `snapshot.json` | Todo | - |
| T13.2.2 | Verify checksum on snapshot read; fail gracefully if mismatch | Todo | - |
| T13.2.3 | Handle truncated `events.jsonl` (crash during write) — truncate to last complete line | Todo | - |
| T13.2.4 | Check disk space before write; enter read-only mode if < 100MB | Todo | - |
| T13.2.5 | Copy snapshot + events to backup dir on graceful shutdown | Todo | - |
| T13.2.6 | Keep last 3 backups, delete older ones | Todo | - |
| T13.2.7 | Write test: checksum mismatch produces graceful error | Todo | - |
| T13.2.8 | Write test: truncated log recovers to last complete line | Todo | - |
| T13.2.9 | Write test: disk full triggers read-only mode | Todo | - |

#### Acceptance Criteria

- Corrupted snapshot is detected via checksum; daemon starts with empty state rather than panicking
- Truncated event log is repaired automatically (last partial line removed)
- Disk full condition is detected; daemon continues operating but stops persisting events
- Backups are created on shutdown and retained for 3 iterations
- **Tests**: `snapshot_checksum_mismatch` — mismatch detected, empty state fallback; `log_truncated_recovery` — partial line removed; `disk_full_readonly` — write rejected, broadcast continues; `backup_retention` — only 3 backups kept

#### Technical Notes

Use `sha2` crate for SHA-256. Disk space check uses `fs2::available_space()` on the events log directory. Read-only mode is a runtime flag, not a config change.

---

### Story 13.3: Multi-Platform Support

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Ensure the daemon and clients work correctly on Linux, macOS, and Windows.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.3.1 | Abstract process spawn logic behind `ProcessSpawner` trait | Todo | - |
| T13.3.2 | Implement Linux process spawn (current behavior) | Todo | - |
| T13.3.3 | Implement Windows process spawn using job objects for cleanup | Todo | - |
| T13.3.4 | Use `dirs::data_dir()` for config paths instead of hardcoded `~/.agile-agent` | Todo | - |
| T13.3.5 | Handle Windows lack of SIGTERM (use Ctrl+C event or graceful shutdown RPC) | Todo | - |
| T13.3.6 | Fix any path separator issues in tests | Todo | - |
| T13.3.7 | Write test: process spawn works on all platforms | Todo | - |
| T13.3.8 | Write test: config directory resolves correctly on all platforms | Todo | - |

#### Acceptance Criteria

- Daemon starts and accepts connections on Linux, macOS, and Windows
- Auto-link works on all platforms
- Graceful shutdown works on all platforms
- All existing tests pass on all platforms
- **Tests**: `spawn_linux`, `spawn_macos`, `spawn_windows` — platform-specific process tests; `config_dir_platform` — `dirs::data_dir()` resolves correctly

#### Technical Notes

Use `cfg` attributes for platform-specific code. Keep platform-specific modules small — most logic stays platform-agnostic. Windows job objects ensure daemon is killed when parent process exits.

---

### Story 13.4: CI Matrix & Cross-Platform Testing

**Priority**: P2
**Effort**: 1 point
**Status**: Backlog

Set up CI to run tests on all three platforms.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.4.1 | Update CI workflow to run on `ubuntu-latest`, `macos-latest`, `windows-latest` | Todo | - |
| T13.4.2 | Ensure `CARGO_BIN_EXE_agent-daemon` is available on all platforms | Todo | - |
| T13.4.3 | Handle platform-specific test timeouts (Windows is slower) | Todo | - |
| T13.4.4 | Document platform-specific setup in `CONTRIBUTING.md` | Todo | - |
| T13.4.5 | Write test: full E2E test passes on all platforms | Todo | - |

#### Acceptance Criteria

- CI runs on all three platforms for every PR
- E2E tests (daemon + CLI + TUI) pass on all platforms
- Documentation includes platform-specific build instructions
- **Tests**: `ci_matrix` — not a code test, but a CI configuration validation

#### Technical Notes

GitHub Actions `matrix` strategy is sufficient. Windows builds may need `vcpkg` for OpenSSL if `tokio-tungstenite` uses native TLS (but v1 uses no TLS, so this is not an issue).

---

### Story 13.5: Protocol Extension Points

**Priority**: P2
**Effort**: 1 point
**Status**: Backlog

Add forward-compatible extension fields to the protocol for future evolution.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T13.5.1 | Add `ext: Option<serde_json::Map>` to `JsonRpcRequest`, `JsonRpcResponse`, `Event` | Todo | - |
| T13.5.2 | Add `capabilities: Vec<String>` to `SessionState` | Todo | - |
| T13.5.3 | Return capabilities in `session.initialize` response | Todo | - |
| T13.5.4 | Reserve `plugin.*` method namespace | Todo | - |
| T13.5.5 | Document extension conventions (ext keys use reverse domain notation) | Todo | - |
| T13.5.6 | Write test: `ext` field round-trips correctly | Todo | - |
| T13.5.7 | Write test: unknown capabilities are ignored by clients | Todo | - |

#### Acceptance Criteria

- `ext` field is present on all message types but optional
- Unknown `ext` keys are preserved during serialization/deserialization
- Capabilities list includes all supported features
- `plugin.*` namespace is rejected with `-32601` until plugin system is implemented
- **Tests**: `ext_roundtrip` — arbitrary JSON in `ext` survives round-trip; `capabilities_listed` — initialize response includes capabilities; `plugin_namespace_rejected` — `plugin.foo` returns `-32601`

#### Technical Notes

Extension keys should use reverse domain notation (e.g., `com.example.myextension`) to avoid collisions. Capabilities enable clients to adapt to daemon features without version negotiation.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Windows process spawn complexity | Medium | High | Use `tokio::process` + job objects; test early on Windows |
| Token auth breaks auto-link | Medium | High | Token auth is opt-in; default remains open |
| Cross-platform CI flakiness | Medium | Medium | Increase timeouts on Windows; retry flaky tests |

## Sprint Deliverables

- `agent/daemon/src/security.rs` — origin validation, token auth, audit log
- `agent/daemon/src/recovery.rs` — checksums, truncation recovery, disk monitoring
- `agent/daemon/src/platform/` — platform-specific process management
- Updated CI workflow with matrix builds
- Protocol extension fields documented

## Dependencies

- [Sprint 12: Observability & Operational Readiness](./sprint-12-observability-operational-readiness.md) — config system and logging must exist for security and recovery features.

## Next Steps

After this sprint, the frontend-backend separation is production-ready. Future work (out of scope):

- **LAN mode**: Bind to `0.0.0.0`, add mTLS authentication
- **Cloud mode**: Daemon as a systemd service, Kubernetes deployment
- **Protocol v2**: Compression, binary frames, batch requests
- **Plugin system**: Third-party extensions via `plugin.*` namespace
