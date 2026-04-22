# Sprint 7: TUI Effect System Integration

## Metadata

- Sprint ID: `sref-007`
- Title: `TUI Effect System Integration`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-20
- Depends on: `sref-004` (Decision Layer Decoupling), `sref-003` (EventLoop Refactoring)

---

## Background

### The Dual-Path Problem

After Sprint 4 (Decision Layer Decoupling), the daemon's `EventLoop` has a clean two-tier execution model for decision outputs:

```
DecisionOutput
    │
    ├──► translate() ──► Vec<DecisionCommand>
    │                        │
    │                        ├──► interpreter ──► Some(Vec<RuntimeCommand>)
    │                        │                        │
    │                        │                        └──► effect_handler.handle() ──► PURE PATH
    │                        │
    │                        └──► None ──► legacy execute() ──► LEGACY PATH
    │
```

However, the **TUI mode** (`tui/src/app_loop.rs`) completely bypasses this architecture. It calls `AgentPool::execute_decision_action()` directly, which invokes the deprecated `DecisionExecutor::execute()` method that mutates slot state directly:

```
TUI AppLoop
    │
    └──► pool.execute_decision_action() ──► DecisionExecutor::execute()
                                              │
                                              └──► direct slot mutation (legacy)
```

### Why This Is a Problem

| Aspect | Daemon (Pure Path) | TUI (Legacy Path) |
|--------|-------------------|-------------------|
| **Decision execution** | `translate() → interpreter → effect handler` | `execute_decision_action()` direct mutation |
| **Testability** | Effect commands can be captured and asserted | Side effects scattered across slot methods |
| **Observability** | Every command goes through `EffectHandler::handle()` | No central dispatch point |
| **Extensibility** | New commands = add interpreter mapping + effect handler | New commands = modify `DecisionExecutor::execute()` match arms |
| **Consistency risk** | Same decision → same effects (deterministic) | Same decision → may differ due to direct state access |

### Concrete Example: `ApproveAndContinue`

In the daemon's pure path, `ApproveAndContinue` currently returns `None` from the interpreter (intentionally falling back to legacy until a `TransitionState` effect exists). In the TUI, it also goes through legacy `execute()`. **This happens to be consistent today**, but as the pure path expands, the TUI will lag behind.

When we eventually add a `TransitionState { agent_id, new_state }` `RuntimeCommand`, the daemon will use it, but the TUI will still use the legacy path — creating divergent behavior for the same decision.

### Root Cause

The TUI does not have an `EffectHandler`. Its `AppLoop` owns an `AgentPool` directly, not an `EventLoop` with a `SessionInner` + `CompositeEffectHandler`. The TUI's architecture predates the effect system, and retrofitting it requires:

1. A TUI-specific `EffectHandler` implementation that can access TUI state
2. Refactoring `AppLoop`'s decision processing to use `translate()` + `interpreter`
3. Ensuring all `RuntimeCommand` variants have TUI-compatible handlers

---

## Sprint Goal

Build a `TuiEffectHandler` that bridges the pure decision path to the TUI's existing state management, then refactor `AppLoop` to use `translate() + interpreter` instead of `execute_decision_action()`. All existing TUI decision flows must continue to work; the change is architectural, not behavioral.

---

## Stories

### Story 7.1: Audit TUI Decision Execution Surface

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Map every place in `tui/src/app_loop.rs` where decisions are executed, and identify which `RuntimeCommand` variants would be needed to replace each legacy call.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.1.1 | Identify all call sites of `execute_decision_action()` in `app_loop.rs` | Todo | - |
| T7.1.2 | Document the `DecisionExecutionResult` match branches and their side effects | Todo | - |
| T7.1.3 | Map each result variant to required `RuntimeCommand` effect(s) | Todo | - |
| T7.1.4 | Identify gaps where no `RuntimeCommand` variant exists (e.g., transcript mutation) | Todo | - |
| T7.1.5 | Produce a compatibility matrix: `DecisionCommand` × `TUI support` | Todo | - |

#### Acceptance Criteria

- Every `execute_decision_action()` call site is documented with its behavioral contract
- Gaps in `RuntimeCommand` coverage for TUI needs are listed
- No behavioral changes in this story — purely research and documentation

#### Technical Notes

Key call site to audit:
```rust
// tui/src/app_loop.rs:~218
let result = pool.execute_decision_action(&agent_id, output);
```

The subsequent match block handles:
- `Executed { option_id }` → UI transcript update + status bar
- `AcceptedRecommendation` → UI transcript update + status bar
- `CustomInstruction { instruction }` → UI update + `start_multi_agent_provider_request_for_agent()`
- `Skipped` / `Cancelled` → status message
- `TaskPrepared { branch, .. }` → status bar + message
- `PreparationFailed { reason }` → status bar + warning message

---

### Story 7.2: Extend RuntimeCommand for TUI Needs

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Add missing `RuntimeCommand` variants that the TUI needs but the daemon effect system does not yet have.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.2.1 | Add `UpdateTranscript { agent_id, entry }` variant (if transcript mutation is needed) | Todo | - |
| T7.2.2 | Add `TransitionState { agent_id, new_status }` variant for `ApproveAndContinue`/`WakeUp` | Todo | - |
| T7.2.3 | Ensure new variants implement `Serialize + Deserialize` (for future snapshot support) | Todo | - |
| T7.2.4 | Update `CompositeEffectHandler` to include handlers for new variants | Todo | - |
| T7.2.5 | Add unit tests for new command construction and equality | Todo | - |

#### Acceptance Criteria

- All TUI decision side effects can be expressed as `RuntimeCommand` variants
- New variants are pure data (no I/O, no threading)
- `CompositeEffectHandler` compiles with placeholder handlers

#### Technical Notes

```rust
// Proposed new variants
pub enum RuntimeCommand {
    // ... existing variants ...

    /// Update agent's operational status (replaces direct AgentSlotStatus mutation)
    TransitionState {
        agent_id: AgentId,
        new_status: String, // or a dedicated TuiStatus enum
    },

    /// Append entry to agent transcript (for CustomInstruction tracking)
    UpdateTranscript {
        agent_id: AgentId,
        entry: TranscriptEntry,
    },

    /// Update decision status display in TUI status bar
    SetDecisionStatus {
        agent_id: AgentId,
        status: Option<String>,
    },
}
```

---

### Story 7.3: Implement TuiEffectHandler

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Create a TUI-specific effect handler that receives `RuntimeCommand` values and applies them to the TUI's `AppState` / `AgentPool`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.3.1 | Design `TuiEffectHandler` struct holding `&mut AppState` (or equivalent context) | Todo | - |
| T7.3.2 | Implement `EffectHandler` trait for `TuiEffectHandler` | Todo | - |
| T7.3.3 | Implement `NotifyUserHandler` → `app.push_status_message()` | Todo | - |
| T7.3.4 | Implement `TerminateHandler` → `pool.stop_agent()` + UI update | Todo | - |
| T7.3.5 | Implement `SpawnProviderHandler` → `start_multi_agent_provider_request_for_agent()` | Todo | - |
| T7.3.6 | Implement `SendToProviderHandler` → best-effort (or no-op if TUI has no provider input channel) | Todo | - |
| T7.3.7 | Implement `UpdateWorktreeHandler` → `slot.set_worktree()` | Todo | - |
| T7.3.8 | Implement new handler traits: `TransitionStateHandler`, `UpdateTranscriptHandler`, `SetDecisionStatusHandler` | Todo | - |
| T7.3.9 | Write unit tests with `RecordingEffectHandler` verifying command capture | Todo | - |

#### Acceptance Criteria

- `TuiEffectHandler` implements `EffectHandler` trait fully
- Each `RuntimeCommand` variant produces the same TUI side effect as the legacy path
- Handler errors are logged, not panicked
- Thread safety: TUI is single-threaded, but handler should still be `Send + Sync`

#### Technical Notes

The TUI effect handler differs from the daemon in one critical way: the daemon's handlers acquire `Arc<Mutex<SessionInner>>`, while the TUI handler operates on a mutable borrow of `AppState`. This means the TUI handler **cannot** use `blocking_lock()` — it must take `&mut AppState` directly.

```rust
pub struct TuiEffectHandler<'a> {
    state: &'a mut AppState,
}

impl<'a> EffectHandler for TuiEffectHandler<'a> {
    fn handle(&self, command: &RuntimeCommand) -> Result<(), EffectError> {
        // Direct mutation of AppState — no locking needed
    }
}
```

This lifetime-based design is acceptable because `AppLoop` is single-threaded, but it means `TuiEffectHandler` cannot be stored in a struct field (it borrows). The integration point in `AppLoop` will need to create the handler at the decision processing site.

---

### Story 7.4: Refactor AppLoop Decision Processing to Pure Path

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Replace `execute_decision_action()` with `translate() + interpreter + effect_handler` in the TUI's decision polling logic.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.4.1 | Import `DecisionExecutor::translate` and `DecisionCommandInterpreter` in `app_loop.rs` | Todo | - |
| T7.4.2 | Replace `execute_decision_action()` call with `translate()` → `interpreter.interpret()` loop | Todo | - |
| T7.4.3 | Collect `RuntimeCommand`s; if any return `None`, fall back to legacy path with warning log | Todo | - |
| T7.4.4 | Dispatch collected commands through `TuiEffectHandler` | Todo | - |
| T7.4.5 | Handle `CustomInstruction` special case (provider thread restart) via effect handler | Todo | - |
| T7.4.6 | Ensure `output_info` (transcript display metadata) is still produced for UI | Todo | - |
| T7.4.7 | Write integration tests: pure path produces same UI state as legacy path | Todo | - |

#### Acceptance Criteria

- TUI decision polling uses `translate() + interpreter` as primary path
- Pure path commands are dispatched through `TuiEffectHandler`
- If interpreter returns `None` for any command, graceful fallback to legacy path
- All existing TUI decision flows continue to work (no regressions)
- `CustomInstruction` still triggers provider thread restart correctly

#### Technical Notes

The current TUI decision processing (simplified):

```rust
let results = pool.execute_decision_action(&agent_id, output);
for result in results {
    match result {
        CustomInstruction { instruction } => {
            // UI update + provider restart
        }
        // ... other variants ...
    }
}
```

Target architecture:

```rust
let commands = DecisionExecutor::translate(&agent_id, output);
let mut all_interpreted = true;
let mut runtime_cmds = Vec::new();
for cmd in &commands {
    match interpreter.interpret(&agent_id, cmd) {
        Some(mut cmds) => runtime_cmds.append(&mut cmds),
        None => { all_interpreted = false; break; }
    }
}

if all_interpreted && !commands.is_empty() {
    let handler = TuiEffectHandler::new(&mut state);
    for cmd in &runtime_cmds {
        handler.handle(cmd)?;
    }
} else {
    // Legacy fallback
    let results = pool.execute_decision_action(&agent_id, output);
    // ... existing match logic ...
}
```

---

### Story 7.5: Unified Decision Path Testing & Hardening

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Verify that the same decision produces the same end state in both Daemon and TUI modes.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.5.1 | Create `DecisionPathConsistency` test harness that runs decisions against both paths | Todo | - |
| T7.5.2 | Test: `EscalateToHuman` → same transcript + same slot status in both modes | Todo | - |
| T7.5.3 | Test: `TerminateAgent` → same stopped state in both modes | Todo | - |
| T7.5.4 | Test: `CustomInstruction` → same provider thread behavior in both modes | Todo | - |
| T7.5.5 | Test: `ApproveAndContinue` → same idle transition in both modes | Todo | - |
| T7.5.6 | Document any intentional behavioral differences (should be none) | Todo | - |

#### Acceptance Criteria

- All 21 `DecisionCommand` variants have a consistency test or documented exception
- TUI and Daemon produce identical end states for identical inputs
- No regressions in existing TUI or Daemon test suites

---

## Dependency Graph

```
Story 7.1 (Audit)
    │
    ├──► Story 7.2 (Extend RuntimeCommand)
    │         │
    │         └──► Story 7.3 (TuiEffectHandler)
    │                   │
    │                   └──► Story 7.4 (Refactor AppLoop)
    │                             │
    │                             └──► Story 7.5 (Testing)
    │
    └──► Story 7.5 (Test harness design can start in parallel)
```

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| TUI has subtle legacy behavior not expressible as RuntimeCommand | Medium | High | Story 7.1 must be thorough; accept temporary legacy fallback |
| `TuiEffectHandler` lifetime design complicates AppLoop refactoring | Medium | Medium | Use callback-style dispatch instead of stored handler if needed |
| Provider thread restart (`CustomInstruction`) is hard to express as pure effect | High | High | Keep legacy fallback for this variant until SpawnProvider handler is fully capable |

## Definition of Done

- [ ] `TuiEffectHandler` implements all per-variant handler traits
- [ ] `AppLoop` decision polling uses pure path as primary execution mode
- [ ] All 21 `DecisionCommand` variants are either interpreted or have documented legacy fallback
- [ ] TUI and Daemon produce identical states for identical decision outputs
- [ ] `cargo clippy --workspace --tests -- -D warnings` passes
- [ ] `cargo test --workspace --lib` passes with zero failures
