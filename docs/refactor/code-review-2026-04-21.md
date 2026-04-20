# Code Review: Refactored Pool Modules

Date: 2026-04-21
Branch: review/refactor-bugs

## Summary

Reviewed all refactored pool modules after extensive code extraction work. Found and fixed 2 critical bugs.

## Bugs Fixed

### 1. event_converter.rs (commit `75fb4cb`)

**Issues found:**
- `PatchApplyStarted`: path was hardcoded empty string - should extract first change path
- `WebSearchFinished`: incorrectly mapped to "running" - should be "websearch completed"
- `McpToolCallStarted`: input was None - should include `server:tool` for context

**Fix:** Extract path from changes, distinguish started/finished status, include server:tool in input

**Added:** 24 comprehensive tests covering all ProviderEvent variants

### 2. decision_executor.rs (commit `d25755a`, combined with `11eb8d3`)

**Critical bug in `process_human_response_internal`:**
- Line 572: Used `find_by_agent_id(response.request_id)` - WRONG
- `response.request_id` is the request's UUID, not the agent_id
- Lookup always returned None, slot status never transitioned
- Human decisions were silently ignored

**Fix:** Pass `work_agent_id` from caller context to correctly find slot

## Modules Reviewed (No Bugs Found)

### pool/decision_spawner.rs
- Correct implementation
- Proper error handling for non-existent agents
- Tests cover basic scenarios

### pool/decision_coordinator.rs
- Correct state management
- Proper HashMap operations
- `remove_agent` correctly removes both agent and mail sender

### pool/task_assignment.rs
- Clean implementation
- Proper backlog validation
- Good test coverage

### pool/lifecycle.rs
- Correct lifecycle operations
- **Potential issue:** `unwrap()` on manager/state_store (Lines 260-261, 371-372, 435-436)
- **Potential issue:** `focused_slot` may be invalid after remove if slots empty (Line 719)

### pool/worktree_recovery.rs
- Correct recovery logic
- **Missing:** No tests at all (230 lines, 0 tests)
- **Potential issue:** Same `unwrap()` calls

### slot/state_machine.rs
- Correct state transitions
- Good test coverage (9 tests)
- **Note:** `is_blocked()` does not include `Resting` state

## Potential Improvements

1. **Add tests for worktree_recovery.rs** - currently no tests
2. **Add tests for lifecycle.rs** - only 2 basic error display tests
3. **Replace `unwrap()` with proper error handling** in:
   - lifecycle.rs: Lines 260-261, 371-372, 435-436
   - worktree_recovery.rs: Lines 88-89, 177, 206-208
4. **Consider including `Resting` in `is_blocked()`** - resting is rate limit escalation
5. **Fix focused_slot edge case** when pool becomes empty after remove

## Test Coverage Summary

| Module | Lines | Tests | Status |
|--------|-------|-------|--------|
| event_converter.rs | 511 | 24 | Good |
| decision_executor.rs | 656 | 2 | Minimal |
| decision_spawner.rs | 146 | 4 | Good |
| decision_coordinator.rs | 349 | 8 | Good |
| task_assignment.rs | 405 | 10 | Good |
| lifecycle.rs | 847 | 2 | Minimal |
| worktree_recovery.rs | 230 | 0 | Missing |
| focus_manager.rs | 310 | 10 | Good |
| queries.rs | 302 | 9 | Good |
| state_machine.rs | 200 | 9 | Good |

## Commits on review/refactor-bugs

```
d25755a fix(core): correct agent lookup in process_human_response_internal
75fb4cb fix(core): improve event_converter and add comprehensive tests
```

All 581 tests pass after fixes.