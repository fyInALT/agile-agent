# 09 — Migration Plan: Backward-Compatible Rollout

> Status: Draft ✅ DECIDED  
> Date: 2026-04-20  
> Scope: Phased rollout, embedded-mode fallback, deprecation timeline, cutover criteria

This document defines how the TUI-backend separation is rolled out without breaking existing users. It is the bridge between implementation and deployment.

---

## 1. Migration Philosophy

**Never break users.**

The current architecture works. The new architecture is better. The transition must be:
- **Gradual**: Each phase is independently deployable.
- **Reversible**: If a phase has issues, rollback to the previous phase.
- **Transparent**: Users do not need to change their workflow unless they want to.

---

## 2. Feature Flag Strategy

### 2.1 Compile-Time Flag

Use a Cargo feature to control which mode is compiled:

```toml
# tui/Cargo.toml
[features]
default = ["embedded-mode"]
embedded-mode = ["agent-core", "agent-decision", "agent-kanban"]
daemon-mode = []
```

**Phase 1–5**: Default is `embedded-mode`. Daemon mode is opt-in.
**Phase 6**: Default switches to `daemon-mode`. Embedded mode is deprecated.
**Phase 7**: `embedded-mode` feature is removed.

### 2.2 Runtime Flag

For binaries that support both modes:

```bash
agent-cli --daemon-mode    # Use daemon (even if embedded is default)
agent-cli --embedded-mode  # Use embedded (even if daemon is default)
```

The `auto-link` logic detects the mode preference:
- If `--daemon-mode`, always connect to daemon (spawn if needed).
- If `--embedded-mode`, never connect to daemon.
- If neither, follow the compile-time default.

---

## 3. Rollout Phases

### Phase 0: Protocol & Daemon Skeleton (Weeks 1–2)

**What**: `agent-protocol` crate and `agent-daemon` binary exist. No production use yet.

**User impact**: None. Daemon is not spawned automatically.

**Testing**: Unit tests for protocol types. Integration tests for daemon skeleton.

**Rollback**: Remove `agent-daemon` binary. No other changes.

### Phase 1: Dual-State TUI (Weeks 3–4)

**What**: TUI can connect to daemon and receive snapshots, but still owns `RuntimeSession` in embedded mode. Daemon mode is opt-in via `--daemon-mode`.

**User impact**: None by default. Users who opt in see the TUI connect to a daemon.

**Testing**: 
- Embedded mode: All existing tests pass.
- Daemon mode: Manual test — TUI connects, shows snapshot, basic interactions work.

**Rollback**: Remove `--daemon-mode` flag. TUI reverts to pure embedded.

### Phase 2: Daemon-First TUI (Weeks 5–6)

**What**: Default mode switches to daemon. TUI no longer bootstraps its own session. `--embedded-mode` flag preserves old behavior.

**User impact**:
- First run: Slightly slower startup (daemon spawn + connect).
- Subsequent runs: Same speed (daemon already running).
- Closing TUI no longer kills the agent (this is the key user-visible improvement).

**Testing**:
- Full test suite in daemon mode.
- Embedded mode smoke test.

**Rollback**: Change default feature back to `embedded-mode`.

### Phase 3: CLI Refactor (Weeks 7–8)

**What**: CLI no longer depends on `agent-tui` or `agent-core`. All commands use protocol.

**User impact**:
- `agent-cli` binary is smaller (no ratatui/crossterm).
- `agent-cli daemon start/stop/status` commands available.
- `agent-cli session` launches TUI as subprocess.

**Testing**:
- All CLI commands tested against daemon.
- `doctor` and `probe` still work locally.

**Rollback**: Revert CLI to embedded-mode implementation.

### Phase 4: Embedded Mode Deprecation (Weeks 9–10)

**What**: `--embedded-mode` flag shows a deprecation warning. Documentation encourages daemon mode.

**User impact**: Warning message on `--embedded-mode` use. No functional change.

**Code change**:

```rust
if use_embedded {
    eprintln!("WARNING: --embedded-mode is deprecated and will be removed in v0.11.0.");
    eprintln!("         Please report any issues with daemon mode at <issue tracker>.");
    run_embedded().await?;
}
```

### Phase 5: Embedded Mode Removal (Week 11+)

**What**: Remove `embedded-mode` feature and all embedded-mode code paths.

**User impact**: `--embedded-mode` flag is no longer recognized. Users who relied on it must upgrade.

**Code changes**:
- Delete `TuiState::bootstrap()` embedded path.
- Delete `agent-core` dependency from `agent-tui`.
- Delete `agent-tui` dependency from `agent-cli`.
- Remove `embedded-mode` Cargo feature.

**Testing**: Full test suite. No embedded fallback.

---

## 4. Cutover Criteria

Each phase has explicit criteria before proceeding:

| Phase | Entry Criteria | Exit Criteria |
|-------|---------------|---------------|
| 0 | Architecture spec approved | `cargo test -p agent-protocol -p agent-daemon` passes |
| 1 | Phase 0 complete | Manual test: TUI connects to daemon, shows correct state |
| 2 | Phase 1 complete | `cargo test --workspace` passes in daemon mode; no regressions in embedded mode |
| 3 | Phase 2 complete | All CLI commands work via protocol; E2E tests pass |
| 4 | Phase 3 complete; 1 week of no critical bugs | Deprecation warning implemented; docs updated |
| 5 | Phase 4 complete; 2 weeks of no embedded-mode bug reports | `cargo test --workspace` passes; no embedded code remains |

---

## 5. Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| Daemon crashes on startup | Keep embedded fallback until daemon is proven stable |
| Port conflicts on busy machines | Use ephemeral ports; check `daemon.json` for conflicts |
| Users confused by new `daemon start` step | Auto-link: daemon starts automatically when needed |
| Performance regression (WebSocket latency) | Benchmark early; localhost WebSocket is ~1ms |
| Old session snapshots incompatible | Version `snapshot.json` schema; migrate on load |
| TUI reconnects too aggressively | Exponential backoff: 100ms → 200ms → 400ms → ... → 30s max |

---

## 6. Communication Plan

### 6.1 Release Notes

Each phase is documented in release notes:

**v0.9.0** (Phase 0–1):
> **New**: Experimental daemon mode (`agent-cli --daemon-mode`). The daemon runs agent sessions in a background process, allowing you to close the TUI without stopping the agent.
> **Note**: Daemon mode is opt-in. The default behavior is unchanged.

**v0.10.0** (Phase 2–3):
> **Changed**: Daemon mode is now the default. The TUI automatically connects to a background daemon.
> **New**: `agent-cli daemon start/stop/status` commands.
> **Deprecated**: `--embedded-mode` flag. Will be removed in v0.11.0.

**v0.11.0** (Phase 5):
> **Removed**: Embedded mode. All sessions run through the daemon.
> **Migration**: If you were using `--embedded-mode`, simply remove the flag.

### 6.2 Documentation Updates

- `README.md`: Update quick start to mention auto-link behavior.
- `AGENTS.md`: Update architecture diagram.
- `docs/refactor/`: Move implementation docs to `docs/archived/` after Phase 5.

---

## 7. Post-Migration Cleanup

After Phase 5, the following cleanup tasks remain:

1. **Remove `TuiResumeSnapshot`** from `agent-core`: The daemon writes `snapshot.json` directly.
2. **Remove `EventAggregator` re-exports** from `agent-core/src/lib.rs` that were created for TUI.
3. **Consolidate `agent-core`**: With TUI no longer depending on it, some modules that were split out (e.g., `standup_report`) can be reconsidered.
4. **Archive refactor docs**: Move `docs/refactor/` to `docs/archived/refactor-2026-Q2/`.
5. **Update CI**: Remove `embedded-mode` feature from test matrix.

---

## 8. Rollback Procedure

If a critical bug is found in daemon mode after Phase 2:

1. **Immediate**: Users can add `--embedded-mode` to their command to bypass the daemon.
2. **Short-term**: Revert the default feature to `embedded-mode` in a patch release.
3. **Long-term**: Fix the bug and re-release with daemon mode as default.

The compile-time feature flag makes this rollback a one-line change:

```toml
# tui/Cargo.toml
[features]
-default = ["daemon-mode"]
+default = ["embedded-mode"]
 daemon-mode = []
 embedded-mode = ["agent-core", "agent-decision", "agent-kanban"]
```
