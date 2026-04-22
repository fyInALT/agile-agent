# Sprint 4: Decision Layer Decoupling

## Metadata

- Sprint ID: `sref-004`
- Title: `Decision Layer Decoupling`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Completed`
- Created: 2026-04-22

## Background

Currently, `agent-decision` has bidirectional coupling with `agent-daemon`:
- `agent-daemon` sends `DecisionRequest` to `agent-decision`
- `agent-decision` processes and sends `DecisionAction` back to `agent-daemon`
- `agent-daemon` executes `DecisionAction` (e.g., `CustomInstruction`) which may spawn provider threads, update state, etc.

This creates a cycle: decision layer depends on runtime (to understand events), and runtime depends on decision layer (to execute actions). The fix is to make the decision layer **read-only**: it consumes events, classifies situations, and returns a `DecisionCommand` — a pure data structure describing what should happen. The runtime (EventLoop) executes the command.

This is the most technically challenging sprint because it requires redesigning the action execution path without breaking the idle-agent decision trigger or error recovery flows.

## Sprint Goal

Make `agent-decision` read-only: it returns `DecisionCommand` instead of executing actions directly. The EventLoop interprets commands and executes them. Eliminate `agent-decision` → `agent-daemon` write dependencies. All decision integration tests pass.

## Stories

### Story 4.1: Define `DecisionCommand` Pure Return Type

**Priority**: P0
**Effort**: 3 points
**Status**: Completed ✅ — 21-variant `DecisionCommand` enum with `description()`, `target_agents()`, serde support.

Create a type representing all possible actions the decision layer can recommend, without any execution logic.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Define `DecisionCommand` enum: `EscalateToHuman { reason, context }`, `RetryTool { tool_name, args, max_attempts }`, `SendCustomInstruction { prompt, target_agent }`, `ApproveAndContinue`, `TerminateAgent { reason }`, `SwitchProvider { provider_type }` | Todo | - |
| T4.1.2 | Add `DecisionCommand::description()` for human-readable explanation | Todo | - |
| T4.1.3 | Add `DecisionCommand::target_agents() -> Vec<AgentId>` for routing | Todo | - |
| T4.1.4 | Map each existing `DecisionAction` to `DecisionCommand` | Todo | - |
| T4.1.5 | Write unit tests for command construction and routing | Todo | - |

#### Acceptance Criteria

- `DecisionCommand` covers all existing `DecisionAction` variants
- Commands are pure data — no `async`, no `Send`, no I/O
- Command routing information is statically available
- All existing decision actions have a command equivalent

---

### Story 4.2: Refactor Decision Executor to Return Commands

**Priority**: P0
**Effort**: 5 points
**Status**: Completed ✅ — `DecisionExecutor::translate()` returns pure `Vec<DecisionCommand>`; legacy `execute()` preserved for backward compatibility.

Change the decision execution pipeline from "execute actions" to "produce commands."

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Remove `DecisionAction` execution code from `DecisionExecutor` | Todo | - |
| T4.2.2 | Make `DecisionExecutor::execute()` return `Vec<DecisionCommand>` | Todo | - |
| T4.2.3 | Update `TieredEngine` to return `DecisionCommand` instead of executing | Todo | - |
| T4.2.4 | Update `BlockingEngine` to return `DecisionCommand` | Todo | - |
| T4.2.5 | Update `ConcurrentEngine` to collect commands from all branches | Todo | - |
| T4.2.6 | Update classifier pipeline to produce `DecisionCommand` prototypes | Todo | - |
| T4.2.7 | Write unit tests for each engine variant returning correct commands | Todo | - |

#### Acceptance Criteria

- `DecisionExecutor` no longer has any `async` or I/O code
- All engines return `Vec<DecisionCommand>` instead of executing
- Classification-to-command mapping is explicit and testable
- Engine tests verify command output, not side effects

---

### Story 4.3: Implement Decision Command Interpreter

**Priority**: P0
**Effort**: 4 points
**Status**: Completed ✅ — `DecisionCommandInterpreter` in `daemon/src/decision_interpreter.rs`. Some variants (`SelectOption`, `PrepareTaskStart`) return `None` (not yet supported in new path).

Create an interpreter in the EventLoop that translates `DecisionCommand` into `RuntimeCommand` effects.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Implement `DecisionCommandInterpreter` struct in `daemon/src/` | Todo | - |
| T4.3.2 | Map `EscalateToHuman` → `RuntimeCommand::NotifyUser` + pause agent | Todo | - |
| T4.3.3 | Map `RetryTool` → `RuntimeCommand::SendToProvider` with retry context | Todo | - |
| T4.3.4 | Map `SendCustomInstruction` → `RuntimeCommand::SpawnProvider` with decision prompt | Todo | - |
| T4.3.5 | Map `ApproveAndContinue` → no-op (agent resumes) | Todo | - |
| T4.3.6 | Map `TerminateAgent` → `RuntimeCommand::Terminate` | Todo | - |
| T4.3.7 | Map `SwitchProvider` → `RuntimeCommand::SpawnProvider` with new provider type | Todo | - |
| T4.3.8 | Write integration tests for each command type | Todo | - |
| T4.3.9 | Ensure idle agent trigger flow works end-to-end | Todo | - |

#### Acceptance Criteria

- Each `DecisionCommand` variant maps to one or more `RuntimeCommand`
- Idle agent → decision → custom instruction flow works unchanged
- Error recovery (tool retry) works unchanged
- Human escalation pauses agent and notifies TUI

---

### Story 4.4: Eliminate Circular Dependencies

**Priority**: P1
**Effort**: 3 points
**Status**: Completed ✅ — `agent-decision` Cargo.toml has no `agent-daemon` or `agent-core` dependencies. Dependency graph documented at `docs/architecture/dependency-graph.md`.

Verify and enforce that `agent-decision` has no write-path dependencies on `agent-daemon` or `agent-core` runtime types.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.4.1 | Audit `agent-decision/Cargo.toml` dependencies | Todo | - |
| T4.4.2 | Remove any `agent-daemon` or `agent-core` (runtime parts) dependencies | Todo | - |
| T4.4.3 | Ensure `agent-decision` only depends on `agent-events` and `agent-types` | Todo | - |
| T4.4.4 | Add `cargo-deny` or CI check to prevent future circular deps | Todo | - |
| T4.4.5 | Document the allowed dependency graph in `docs/architecture/dependency-graph.md` | Todo | - |
| T4.4.6 | Write script to verify dependency direction at build time | Todo | - |

#### Acceptance Criteria

- `agent-decision` Cargo.toml has no `agent-daemon` or `agent-core` (non-types) dependency
- `cargo tree` confirms no cycles in dependency graph
- CI fails if circular dependency is introduced
- Dependency graph documentation is up to date

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Decision action semantics changed subtly | Medium | High | Exhaustive integration tests for each action type; preserve existing test assertions |
| Circular dependency hidden in trait bounds | Medium | Medium | Use `cargo-modules` to visualize; add strict CI checks |
| Idle agent flow breaks | Low | High | Dedicated integration test for idle → decision → action pipeline |

## Sprint Deliverables

- `decision/src/command.rs` — `DecisionCommand` enum
- Refactored `DecisionExecutor` returning pure commands
- `daemon/src/decision_interpreter.rs` — Command-to-RuntimeCommand mapping
- Clean dependency graph with `agent-decision` as read-only consumer
- All decision integration tests passing

## Dependencies

- [Sprint 1: Shared Kernel Extraction](./sprint-01-shared-kernel.md) — `DomainEvent` required
- [Sprint 2: Worker Aggregate Root](./sprint-02-worker-aggregate-root.md) — `WorkerState` context needed
- [Sprint 3: EventLoop Refactoring](./sprint-03-event-loop-refactoring.md) — `RuntimeCommand` required

## Next Sprint

After completing this sprint, proceed to [Sprint 5: Crate Reorganization](./sprint-05-crate-reorganization.md) for the physical code reorganization.
