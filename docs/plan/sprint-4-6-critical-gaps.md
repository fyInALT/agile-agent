# Sprint 4-6 Critical Gap Analysis

## Executive Summary

**Status**: Sprint 4-6 implementation is **structurally incomplete**. The multi-agent components exist but are **not integrated into the execution path**. The TUI still operates in single-agent mode with RuntimeSession, while AgentPool provides only UI placeholder agents without actual provider threads.

---

## Gap Analysis

### Gap 1: AgentPool.spawn_agent() Creates Empty Slots

**Current Implementation**:
```rust
// agent_pool.rs:148
let slot = AgentSlot::new(agent_id.clone(), codename, provider_type);
self.slots.push(slot);
```

**AgentSlot::new() creates**:
- status: Idle
- event_rx: None
- thread_handle: None
- transcript: empty Vec

**Required by Design**:
```rust
// design document
AgentSlot::with_thread(
    agent_id,
    codename,
    provider_type,
    event_rx,  // Provider event channel
    thread_handle,  // Actual thread
)
```

**Result**: "Spawned" agents are empty UI placeholders without provider execution.

---

### Gap 2: TUI Uses RuntimeSession, Not MultiAgentSession

**Current Implementation**:
```rust
// app_loop.rs:40
let session = RuntimeSession::bootstrap(launch_cwd, provider::default_provider(), resume_last)?;
let mut state = TuiState::from_session(session);
let mut provider_rx: Option<mpsc::Receiver<ProviderEvent>> = None;  // SINGLE channel
```

**Required by Design**:
```rust
// design document
let session = MultiAgentSession::bootstrap(launch_cwd, provider::default_provider())?;
let mut state = MultiAgentTuiState::from_session(session);

// Poll all agent channels
let agent_events = state.session.poll_events(Duration::from_millis(80));
```

**Result**: TUI has only ONE provider event channel, cannot handle multiple agents.

---

### Gap 3: EventAggregator Not Integrated

**Current**: EventAggregator exists in core/src/event_aggregator.rs but is NOT used in app_loop.rs.

**Required**: app_loop.rs should poll all agent event channels via EventAggregator:
```rust
let poll_result = state.session.event_aggregator().poll_all();
for event in poll_result.events {
    // Process event from any agent
}
```

**Result**: No multi-channel polling; events from multiple agents cannot be received.

---

### Gap 4: Provider Thread Not Spawned Per-Agent

**Current**: provider::start_provider() is called once for the single RuntimeSession.

**Required**: When spawning an agent, need:
1. Create mpsc::channel for events
2. Spawn provider thread with event_tx
3. Store event_rx and thread_handle in AgentSlot
4. Register event_rx with EventAggregator

**Result**: No actual provider execution per-agent.

---

### Gap 5: MultiAgentSession Not Used by TUI

**Current**: MultiAgentSession::bootstrap() exists but app_loop.rs uses RuntimeSession::bootstrap().

**Required**:
```rust
// app_loop.rs should use:
let session = MultiAgentSession::bootstrap(launch_cwd, default_provider, resume_last, max_agents)?;
```

**Result**: Multi-agent session logic (restore, event polling) is unreachable from TUI.

---

## What Works vs What Doesn't

### Works (UI-Level)
- AgentPool can create/spawn/stop/focus agents
- Status bar shows agents from AgentPool
- Tab/Shift+Tab cycles focus (UI-only)
- Ctrl+N opens overlay (creates AgentSlot entry)
- Ctrl+X stops agent (marks slot as stopped)
- AgentViewState caching for scroll positions
- AgentPool tests pass (291 tests)

### Doesn't Work (Execution-Level)
- No provider thread spawns when "spawn agent" is clicked
- No events received from "spawned" agents
- EventAggregator not connected to app_loop
- MultiAgentSession not used by TUI
- AgentSlot has no event_rx, no thread_handle
- "Spawned" agents are empty placeholders

---

## Test Quality Analysis

### Compromised Tests

| Test | Issue |
|------|-------|
| `spawn_agent_creates_pool_and_agent` | Creates AgentSlot but no provider thread |
| `focus_next_agent_cycles_pool` | Tests UI cycling, not real agent execution |
| `stop_focused_agent_marks_stopped` | Marks slot stopped, but no thread to stop |
| All AgentPool tests | Test slot management, not provider execution |

**Root Cause**: Tests verify component behavior (AgentPool state changes) but not integration (actual provider threads, event channels).

### Missing Test Scenarios

1. Provider thread spawning per-agent
2. EventAggregator polling multiple channels
3. Multi-agent concurrent execution
4. Events received from spawned agents
5. MultiAgentSession integration with TUI

---

## Potential Bugs

### Bug 1: AgentPool Disconnected from RuntimeSession

TuiState has BOTH RuntimeSession (single-agent) AND AgentPool (multi-agent placeholders). These are disconnected:
- RuntimeSession's agent_runtime is the REAL provider
- AgentPool's slots are EMPTY placeholders
- User confusion: status bar shows "agents" that aren't running

### Bug 2: Status Bar Shows Wrong State

Status bar displays agents from AgentPool (empty slots) while actual provider runs in RuntimeSession:
- Shows "○ alpha" (idle) while RuntimeSession might be "responding"
- Misleading UI - user sees "spawned agent_002" but nothing is executing

### Bug 3: Focus Switching Does Nothing

Tab cycles AgentPool.focused_slot but:
- All slots are empty (no transcript, no events)
- Switching focus just changes which empty slot is "highlighted"
- No actual transcript switching since slots have no data

---

## Required Fixes

### P0 - Critical Architecture Fix

1. **Replace RuntimeSession with MultiAgentSession in app_loop.rs**
   - Use MultiAgentSession::bootstrap()
   - Use EventAggregator for multi-channel polling

2. **Implement provider thread spawning per-agent**
   - When AgentPool.spawn_agent(), create provider thread
   - Store event_rx and thread_handle in AgentSlot
   - Register with EventAggregator

3. **Connect EventAggregator to app_loop**
   - Poll all channels each frame
   - Route events to correct AgentSlot
   - Update transcript per-agent

### P1 - Integration

4. **MultiAgentSession used by TUI**
   - Replace RuntimeSession in TuiState
   - Update app_loop to use session methods

5. **Per-agent transcript in AgentSlot**
   - Store transcript in AgentSlot, not AppState
   - Render focused agent's transcript

---

## Conclusion

The implementation is a **facade**: multi-agent UI exists but multi-agent execution does not. 

**Key Metrics**:
- Components exist: ✅ AgentPool, AgentSlot, EventAggregator, MultiAgentSession
- Tests pass: ✅ 291 tests
- UI shows agents: ✅ Status bar, overlays
- Provider threads per-agent: ❌ NOT implemented
- EventAggregator in app_loop: ❌ NOT integrated
- MultiAgentSession in TUI: ❌ NOT used

**Recommendation**: Phase 2 implementation requires:
1. Provider thread spawning per-agent
2. EventAggregator integration into app_loop
3. MultiAgentSession replacing RuntimeSession
4. Per-agent transcript storage and rendering