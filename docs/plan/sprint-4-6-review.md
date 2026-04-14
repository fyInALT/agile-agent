# Sprint 4-6 Implementation Review

## Overview

This document summarizes the review of Sprint 4, 5, and 6 implementation against the design specifications.

## Sprint 4: Basic Multi-Agent TUI

### Completed Stories

| Story | Status | Notes |
|-------|--------|-------|
| 4.1 Agent Status Bar | ✅ Complete | `render_agent_status_bar()` implemented in render.rs |
| 4.2 Agent Focus Switching | ✅ Complete | Tab/Shift+Tab, Ctrl+1-9 keybindings in input.rs |
| 4.3 Per-Agent Transcript View | ✅ Complete | AgentViewState with scroll/follow-tail caching |
| 4.4 Agent Creation UI | ✅ Complete | ProviderSelectionOverlay with Ctrl+N |
| 4.5 Agent Stop UI | ✅ Complete | ConfirmationOverlay with Ctrl+X |

### Gap Analysis

1. **Status bar only shows single agent**: `render_agent_status_bar()` shows one agent indicator, not all agents from AgentPool. This is because TUI still uses RuntimeSession (single-agent) instead of MultiAgentSession.

2. **AgentViewState not connected to AgentPool**: The per-agent view state caching exists but isn't used for actual multi-agent transcript switching since we're still in single-agent mode.

## Sprint 5: Persistence

### Completed Stories

| Story | Status | Notes |
|-------|--------|-------|
| 5.1 AgentPersistenceCoordinator | ✅ Complete | PersistenceOp enum, queue(), flush(), force_save() |
| 5.2 Periodic Flush | ✅ Complete | 5-second interval in app_loop.rs |
| 5.3 Per-Agent Directory Isolation | ✅ Complete | Already existed in agent_store.rs |
| 5.4 Restore All Agents | ⚠️ Partial | Only restores single agent, not multi-agent |

### Gap Analysis

1. **Periodic flush not using coordinator queue**: The periodic flush in app_loop.rs calls `persist_if_changed()` directly, not using AgentPersistenceCoordinator's batch queue functionality. This means the coordinator is created but not actively used.

2. **Agent restore is single-agent only**: RuntimeSession.bootstrap() and restore_from_snapshot() only handle one agent. For multi-agent, would need to iterate through snapshot.agents and restore each.

3. **Missing tests for multi-agent restore**: No tests verify that multiple agents are restored correctly from snapshot.

## Sprint 6: Graceful Shutdown and Restore

### Completed Stories

| Story | Status | Notes |
|-------|--------|-------|
| 6.1 ShutdownSnapshot | ✅ Complete | ShutdownSnapshot, AgentShutdownSnapshot, ShutdownReason |
| 6.2 Graceful Shutdown | ⚠️ Partial | Single-agent only, missing multi-phase logic |
| 6.3 Full Session Restore | ⚠️ Partial | Single-agent only, ignores other agents in snapshot |
| 6.4 Resume Dialog UI | ❌ Missing | Not implemented |

### Gap Analysis

1. **Multi-phase shutdown not implemented**: The design describes 6 phases:
   - Phase 1: Signal providers to finish
   - Phase 2: Collect snapshots from each agent
   - Phase 3: Wait for threads with timeout
   - Phase 4: Persist snapshot
   - Phase 5: Final flush
   - Phase 6: Mark workplace shutdown
   
   Current implementation only persists and marks stopped.

2. **ProviderThreadSnapshot placeholder**: In `create_shutdown_snapshot()`, the ProviderThreadSnapshot uses placeholder timestamp "2026-04-14T00:00:00Z" instead of actual thread start time.

3. **restore_from_snapshot ignores other agents**: Only uses `snapshot.agents.first()`, discarding all other agents.

4. **Snapshot not integrated into bootstrap**: RuntimeSession.bootstrap() doesn't check for shutdown snapshot and call restore_from_snapshot. Users can't resume from snapshot automatically.

5. **Resume Dialog UI missing**: No UI dialog to show agents that were active at shutdown and offer resume options.

## Architecture Issues

### Dual Session Types

- **RuntimeSession**: Single-agent session (currently used by TUI)
- **MultiAgentSession**: Multi-agent session with AgentPool (exists but not used by TUI)

The TUI still uses RuntimeSession directly, making all multi-agent features superficial.

### Integration Gaps

| Feature | Exists | Integrated |
|---------|--------|------------|
| AgentPool | ✅ | ❌ Not used by TUI |
| MultiAgentSession | ✅ | ❌ Not used by TUI |
| AgentPersistenceCoordinator | ✅ | ❌ Not used in flush |
| ShutdownSnapshot | ✅ | ❌ Not checked on bootstrap |
| AgentViewState | ✅ | ❌ Not connected to AgentPool |

## Test Quality Analysis

### Missing Test Scenarios

1. **Multi-agent snapshot tests**: No tests for snapshots with multiple agents
2. **Corrupted snapshot handling**: No tests for invalid JSON, missing files
3. **Concurrent persistence**: No tests for multiple agents persisting simultaneously
4. **Resume from interrupted state**: Tests exist but don't verify actual resume behavior

### Potential Bugs

1. **Backlog overwrite**: `restore_from_snapshot` uses snapshot backlog directly, potentially overwriting newer file-based backlog state.

2. **Agent IDs not registered on restore**: When restoring from snapshot, agent IDs may not be registered in workplace meta's agent_ids list.

3. **Snapshot clear race condition**: If restore fails after clear_shutdown_snapshot(), the snapshot is lost.

4. **Coordinator flush_interval test**: Tests modify flush_interval directly which requires `pub` field - this is OK for tests but should be through constructor in real use.

## Recommendations

### Critical Fixes

1. **Integrate shutdown snapshot into bootstrap**: Check for snapshot on startup and restore automatically.

2. **Use AgentPersistenceCoordinator in periodic flush**: Queue operations and batch flush instead of direct persist.

3. **Multi-agent restore**: Iterate through all agents in snapshot, not just first.

4. **Implement resume dialog**: Show which agents were active and offer resume options.

### Architecture Refactoring

1. **Migrate TUI to MultiAgentSession**: Replace RuntimeSession usage in TUI with MultiAgentSession.

2. **Connect AgentViewState to AgentPool**: Use agent pool's focused_index for transcript switching.

3. **Integrate AgentPool into shutdown**: Collect snapshots from all active agents, not just current.

### Test Improvements

1. Add tests for multi-agent snapshot scenarios
2. Add tests for corrupted/incomplete snapshot handling
3. Add integration tests for full shutdown/restore cycle
4. Verify agent IDs registered after restore

## Summary

Sprint 4-6 implementation is **structurally complete** for single-agent mode but **not integrated** for multi-agent mode. The core components exist but aren't connected to the TUI flow. This creates a gap where:

- TUI shows multi-agent UI (status bar, overlays) but operates single-agent
- Shutdown saves multi-agent-ready snapshot but restores single-agent
- Persistence coordinator exists but periodic flush uses direct persist

The next step should be integrating MultiAgentSession into the TUI to enable true multi-agent operation.