# TUI Agent Status Bug Fixes

## Summary

Minimal bug fixes for decision layer status display in TUI. Focus on removing dead code, preventing memory leaks, and ensuring decision status appears in all view modes.

## Identified Bugs

### Bug 1: Dead Code - Global `last_decision_started_at`

**Location**: `core/src/agent_pool.rs:350`

**Issue**: AgentPool has a global `last_decision_started_at` field that:
- Is updated when any decision starts (line 1790)
- Has a getter method (line 1611)
- Is **never queried** for actual UI display
- Is redundant with per-agent timestamps in DecisionAgentSlot

**Fix**: Remove the global field. The per-agent `last_decision_started_at` in DecisionAgentSlot is what's actually used by `agents_with_pending_decisions()` for UI rendering.

**Why**: Per-agent tracking is correct - each agent's decision has independent timing. A global timestamp would show "Analyzing" for ALL agents when ANY one decides, which is wrong behavior.

### Bug 2: Memory Leak - `clear_recent_decision()` Never Called

**Location**: `core/src/decision_agent_slot.rs:751`

**Issue**: `last_decision_started_at` is set when a decision starts but never cleared. While the `has_recent_decision()` check uses elapsed time (1.5s window), the timestamp persists forever in memory.

**Fix**: Call `clear_recent_decision()` after the display window expires. Add cleanup in `poll_decision_agents()` when:
1. Response was received AND
2. More than 1.5s has elapsed since decision started

**Why**: Prevents timestamps from accumulating indefinitely. Minor memory impact but good hygiene.

### Bug 3: Decision Status Missing in Overview View

**Location**: `tui/src/render.rs:328-392` (render_overview_agent_list)

**Issue**: Overview view's agent list shows agent status but doesn't check for pending decisions. Users can't see decision layer activity when coordinating multiple agents.

**Fix**: Before rendering each agent row, check if that agent has a pending decision (from `agents_with_pending_decisions()`). Add a brain emoji indicator similar to status bar.

**Implementation**:
```rust
// In render_overview_agent_list
let pending_decisions = state.agent_pool.as_ref()
    .map(|pool| pool.agents_with_pending_decisions())
    .unwrap_or_default();

for index in &visible {
    // ... existing row formatting ...
    let has_pending_decision = pending_decisions.iter()
        .any(|(id, _)| id == snapshot.agent_id);
    
    if has_pending_decision {
        // Append " 🧠" indicator to row
    }
}
```

### Bug 4: Decision Status Missing in Dashboard Cards

**Location**: `tui/src/render.rs:1990-2078` (render_agent_card)

**Issue**: Dashboard view's agent cards don't show decision layer activity.

**Fix**: Same approach as Overview - check pending decisions and add indicator.

**Implementation**: Add "🧠" to card content when agent has pending decision.

## Implementation Plan

### Phase 1: Core Cleanup (agent_pool.rs, decision_agent_slot.rs)

1. Remove global `last_decision_started_at` field from AgentPool struct
2. Remove the getter method `last_decision_started_at()`
3. Remove the update in `poll_decision_agents()` (line 1790)
4. Update tests that reference the global field

### Phase 2: Memory Leak Fix (agent_pool.rs)

1. In `poll_decision_agents()`, after clearing thinking status, check elapsed time
2. If > 1500ms since decision started, call `clear_recent_decision()`

### Phase 3: Overview View Fix (render.rs, overview_row.rs)

1. Pass pending_decisions to OverviewAgentRow::from_snapshot()
2. Add optional `has_pending_decision` parameter
3. Append " 🧠⠋" indicator when true

### Phase 4: Dashboard View Fix (render.rs)

1. Check pending_decisions in `render_dashboard_cards()`
2. Pass has_pending_decision to `render_agent_card()`
3. Add indicator to card content

## Testing

- Unit tests for `clear_recent_decision()` behavior
- Integration test: spawn agent, trigger decision, verify indicator shows in Overview
- Integration test: spawn agent, trigger decision, verify indicator shows in Dashboard
- Verify existing tests pass after removing global field

## Files Changed

| File | Change |
|------|--------|
| `core/src/agent_pool.rs` | Remove global field, add cleanup call |
| `core/src/decision_agent_slot.rs` | (no changes, method exists) |
| `tui/src/render.rs` | Add decision indicator to Overview and Dashboard |
| `tui/src/overview_row.rs` | Add pending_decision parameter |