# Sprint 4-6 Thorough Review Report

## Executive Summary

**Critical Finding**: Sprint 4-6 are **NOT fully completed**. The multi-agent components exist but are **NOT integrated** into the TUI. The TUI still operates in single-agent mode with RuntimeSession, while all multi-agent features (AgentPool, MultiAgentSession, AgentPersistenceCoordinator) are disconnected.

---

## Sprint-by-Sprint Gap Analysis

### Sprint 4: Basic Multi-Agent TUI

| Story | Spec Requirement | Implementation Status | Gap |
|-------|-----------------|----------------------|-----|
| 4.1 Agent Status Bar | "show all agent statuses" | Shows ONE agent only | **Critical**: Only shows focused agent, not all agents |
| 4.2 Agent Focus Switching | Tab/Shift+Tab cycles agents | Pushes status message | **Critical**: No actual focus switching, just message |
| 4.3 Per-Agent Transcript View | Transcript switches on focus | AgentViewState exists but unused | **Critical**: Not connected to actual agents |
| 4.4 Agent Creation UI | Ctrl+N spawns new agent | Overlay opens but no spawn | **Critical**: No actual agent creation |
| 4.5 Agent Stop UI | Ctrl+X stops focused agent | Overlay opens but no stop | **Critical**: No actual agent stop |

**Code Evidence**:
```rust
// tui/src/app_loop.rs:178-188
InputOutcome::FocusNextAgent => {
    // In single-agent mode, this switches provider
    // In multi-agent mode, this would focus next agent
    state.app_mut().push_status_message("focus next (multi-agent: coming soon)");
}
InputOutcome::SpawnAgent => {
    state.open_provider_overlay();
}
// When overlay confirms:
ProviderSelectionCommand::Select(provider) => {
    state.close_provider_overlay();
    // In multi-agent mode, this would spawn a new agent
    state.app_mut().push_status_message(format!(
        "spawn agent with {} (multi-agent: coming soon)", ...
    ));
}
```

**Root Cause**: TUI uses `RuntimeSession` (single-agent) instead of `MultiAgentSession`.

---

### Sprint 5: Persistence

| Story | Spec Requirement | Implementation Status | Gap |
|-------|-----------------|----------------------|-----|
| 5.1 AgentPersistenceCoordinator | Batch persistence with queue | Implemented with tests | **Not integrated**: Never used in app_loop |
| 5.2 Periodic Flush | Coordinator flush on interval | Direct persist_if_changed() | **Critical**: Coordinator queue unused |
| 5.3 Per-Agent Directory | Isolated agent directories | Exists in agent_store.rs | OK - Already existed |
| 5.4 Restore All Agents | Restore all from workplace | Only single agent | **Critical**: snapshot.agents ignored |

**Code Evidence**:
```rust
// tui/src/app_loop.rs - Periodic flush
// Uses persist_if_changed() directly, NOT coordinator
if last_flush.elapsed() >= PERSISTENCE_FLUSH_INTERVAL {
    state.persist_if_changed()?;
    last_flush = Instant::now();
}
```

```rust
// restore_from_snapshot only uses first agent
let (agent_meta, was_active, assigned_task_id) = if let Some(first_agent) = snapshot.agents.first() {
    (first_agent.meta.clone(), first_agent.was_active, ...)
} else { ... };
// Other agents in snapshot.agents are IGNORED!
```

---

### Sprint 6: Graceful Shutdown and Restore

| Story | Spec Requirement | Implementation Status | Gap |
|-------|-----------------|----------------------|-----|
| 6.1 ShutdownSnapshot | Complete shutdown state | Implemented | OK |
| 6.2 Graceful Shutdown | 6-phase shutdown | Single-phase only | **Major**: No thread signaling, no wait |
| 6.3 Full Session Restore | Restore from snapshot | Single agent only | **Critical**: Multi-agent ignored |
| 6.4 Resume Dialog UI | Show active agents, offer options | NOT implemented | **Critical**: Missing entirely |

**Shutdown Phases Missing**:
1. Signal all providers to finish - NOT implemented
2. Collect snapshots from each agent - NOT implemented
3. Wait for threads with timeout - NOT implemented
4. Persist snapshot - Implemented
5. Final flush - Implemented (but without coordinator)
6. Mark workplace shutdown - Implemented

**Resume Dialog Missing**: No UI shows which agents were active at shutdown.

---

## Test Quality Analysis

### Compromised Tests

| Test | File | Issue |
|------|------|-------|
| `tab_shows_focus_next_message_when_idle` | shell_tests.rs | Tests message display, not behavior |
| Provider overlay tests | app_loop.rs | Tests overlay opens, not agent spawn |
| Confirmation overlay tests | app_loop.rs | Tests overlay opens, not agent stop |

**Example of Compromised Test**:
```rust
fn tab_shows_focus_next_message_when_idle() {
    shell.press(KeyCode::Tab, KeyModifiers::NONE);
    let rendered = shell.render_to_string(100, 24);
    // In single-agent mode, Tab shows focus message
    assert!(rendered.contains("focus next"));  // Just checks text, not actual focus!
}
```

### Good Tests

| Test Area | Coverage |
|-----------|----------|
| AgentPool lifecycle | Comprehensive - 25+ tests for spawn/stop/focus |
| ShutdownSnapshot types | Good - 4 tests for snapshot creation |
| RuntimeSession restore | Good - Tests bootstrap restore from snapshot |
| PersistenceCoordinator | Good - 7 tests for queue/flush/force_save |

**Issue**: Tests verify component behavior but NOT integration.

---

## Potential Bugs

### Bug 1: Workplace Mismatch in restore_from_snapshot

```rust
// runtime_session.rs:309-317
let workplace = crate::workplace_store::WorkplaceStore::for_cwd(&launch_cwd)?;  // Line 309
let agent_runtime = AgentRuntime::from_meta(agent_meta, workplace.clone());  // Line 311

let mut workplace_state = SharedWorkplaceState::with_backlog(
    agent_runtime.meta().workplace_id.clone(),  // Uses agent's workplace_id
    backlog,
);
```

**Risk**: If agent_meta.workplace_id differs from WorkplaceStore::for_cwd's workplace_id, mismatch occurs. However, since both derive from same cwd hash, they should match.

### Bug 2: Snapshot Clear Location

```rust
// restore_from_snapshot:369
workplace.clear_shutdown_snapshot()?;
```

**Risk**: Uses `workplace` variable created at line 309, not `self.workplace`. If there's a mismatch in workplace_id, snapshot might not be cleared properly.

### Bug 3: Periodic Flush Race Condition

```rust
// app_loop.rs:54-57
if last_flush.elapsed() >= PERSISTENCE_FLUSH_INTERVAL {
    state.persist_if_changed()?;  // May block during file I/O
    last_flush = Instant::now();
}
```

**Risk**: Direct persist_if_changed() may block TUI rendering during file I/O. Should use coordinator queue for non-blocking persistence.

### Bug 4: Bootstrap Workplace Redundancy

```rust
// bootstrap creates workplace twice:
let workplace = WorkplaceStore::for_cwd(&launch_cwd)?;  // Line 35
// If no snapshot, continue with:
let bootstrap = AgentRuntime::bootstrap_for_cwd(...);  // Creates own workplace at Line 50
```

**Risk**: Inefficient but not buggy - the first workplace is unused if no snapshot.

### Bug 5: Restore Ignores Multiple Agents

```rust
// restore_from_snapshot:296-297
if let Some(first_agent) = snapshot.agents.first() {
    // Only uses first agent!
}
// snapshot.agents.iter().skip(1) - ALL IGNORED
```

**Critical**: Multi-agent restore impossible with current implementation.

---

## Architecture Issues

### Issue 1: Dual Session Types Not Integrated

| Type | Purpose | Used By |
|------|---------|---------|
| RuntimeSession | Single-agent | TUI (actual usage) |
| MultiAgentSession | Multi-agent | Tests only (never integrated) |

**Impact**: All multi-agent code is unreachable from TUI.

### Issue 2: AgentViewState Disconnect

```rust
// ui_state.rs defines:
pub agent_view_states: HashMap<String, AgentViewState>,
pub focused_agent_id: Option<String>,

// But TuiState.session is RuntimeSession (no AgentPool)
// No agents to store view states for!
```

### Issue 3: Status Bar Hardcoded

```rust
// render.rs:107
Span::styled("alpha", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
```

**Bug**: Codename "alpha" is hardcoded, not from AgentPool.

---

## Required Fixes

### P0 - Critical

1. **Integrate MultiAgentSession into TUI**: Replace RuntimeSession with MultiAgentSession
2. **Implement focus switching**: Use AgentPool.focus_agent_by_index()
3. **Implement agent spawn**: Use AgentPool.spawn_agent()
4. **Implement agent stop**: Use AgentPool.stop_agent()
5. **Multi-agent restore**: Iterate all agents in snapshot

### P1 - High

6. **Status bar from AgentPool**: Show all agents, not just one
7. **Use PersistenceCoordinator**: Queue operations, batch flush
8. **Resume Dialog UI**: Show active agents at shutdown

### P2 - Medium

9. **Multi-phase shutdown**: Signal threads, wait with timeout
10. **Thread lifecycle logging**: Track shutdown phases

---

## Test Recommendations

### Add Integration Tests

1. Multi-agent spawn and focus switching
2. Agent status bar shows all agents
3. Shutdown snapshot with multiple agents
4. Restore all agents from snapshot
5. Coordinator queue with periodic flush

### Fix Compromised Tests

1. `tab_shows_focus_next_message_when_idle` → Should test actual focus change
2. Overlay tests → Should test actual agent creation/stopping

---

## Conclusion

Sprint 4-6 have **partial implementations** where:
- **Components exist** (AgentPool, MultiAgentSession, Coordinator, Overlays)
- **Tests pass** (component tests, not integration tests)
- **UI shows features** (keybindings, overlays)
- **But nothing works** (all outcomes push "coming soon" messages)

This creates a facade of completion without actual multi-agent functionality.

**Priority**: Integrate MultiAgentSession into TUI to enable real multi-agent operation.