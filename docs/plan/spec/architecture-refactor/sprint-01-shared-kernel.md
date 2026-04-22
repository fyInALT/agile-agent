# Sprint 1: Shared Kernel Extraction

## Metadata

- Sprint ID: `sref-001`
- Title: `Shared Kernel Extraction`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-22

## Background

The `agile-agent` codebase currently suffers from a fractured event type system. `ProviderEvent` is defined in `agent/provider/src/provider.rs`, yet it is heavily used by `agent-core`, `agent-decision`, and `agent-daemon`. Worse, `agent-decision` maintains its own simplified `ProviderEvent` (15 variants), requiring a manual `event_converter.rs` (~300 lines) to bridge the two. Adding a new event variant today requires modifying four files.

This sprint establishes the foundation for the entire refactoring by extracting all shared types into a single `agent-events` crate. This crate becomes the "shared kernel" of the system — depended upon by all other crates, but depending on none.

**Key constraint**: All existing tests must continue to pass. We use type aliases and re-exports to maintain backward compatibility during the transition.

## Sprint Goal

Create the `agent-events` shared kernel crate containing unified `DomainEvent` (24 variants) and `DecisionEvent` (via `From<&DomainEvent>`). Eliminate `core/src/pool/event_converter.rs`. All workspace tests pass with zero regressions.

## Stories

### Story 1.1: Create `agent-events` Crate

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Create a new crate that will house all event definitions and basic types shared across the system.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `agent-events/Cargo.toml` with dependencies on `agent-types`, `agent-toolkit`, `serde` | Todo | - |
| T1.1.2 | Define crate public API in `agent-events/src/lib.rs` | Todo | - |
| T1.1.3 | Move `SessionHandle` from `agent/provider` to `agent-events` | Todo | - |
| T1.1.4 | Move execution status types (`ExecCommandStatus`, `PatchApplyStatus`, `McpToolCallStatus`) to `agent-events` | Todo | - |
| T1.1.5 | Ensure `agent-events` compiles independently | Todo | - |

#### Acceptance Criteria

- `agent-events` crate compiles without errors
- No circular dependencies introduced
- Crate has zero business logic — only type definitions and simple methods

---

### Story 1.2: Define Unified `DomainEvent`

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Define the single source of truth for all domain events. This enum replaces both `agent/provider::ProviderEvent` and the `ProviderEvent` re-export in `agent-core`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Define `DomainEvent` enum with all 24 variants (lifecycle, streaming, tools, MCP, web search, images, system) | Todo | - |
| T1.2.2 | Implement `Debug`, `Clone`, `PartialEq` for `DomainEvent` | Todo | - |
| T1.2.3 | Add helper methods: `is_running()`, `may_need_decision()`, `should_broadcast()` | Todo | - |
| T1.2.4 | Write unit tests for each variant construction | Todo | - |
| T1.2.5 | Write unit tests for helper methods | Todo | - |
| T1.2.6 | Add doc comments explaining each variant's semantics | Todo | - |

#### Acceptance Criteria

- `DomainEvent` covers all 24 variants from the original `ProviderEvent`
- Helper methods correctly classify event types
- 100% variant coverage in tests

#### Technical Notes

```rust
pub enum DomainEvent {
    // Lifecycle
    WorkerStarted,
    WorkerFinished,
    WorkerFailed { reason: String },
    SessionAcquired { handle: SessionHandle },
    // Streaming
    AssistantChunk { text: String },
    ThinkingChunk { text: String },
    StatusUpdate { text: String },
    // Tools, MCP, Web search, Images...
}
```

---

### Story 1.3: Define `DecisionEvent` with Automatic Conversion

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Create the decision-layer-focused event subset with automatic conversion from `DomainEvent`. This replaces the manual mapping in `event_converter.rs`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Define `DecisionEvent` enum (Completion, Error, ToolFailed, PatchFailed, McpFailed, ApprovalRequired, RateLimited, Idle) | Todo | - |
| T1.3.2 | Implement `From<&DomainEvent> for Option<DecisionEvent>` | Todo | - |
| T1.3.3 | Handle edge cases: `ExecCommandFinished` with non-success status, `PatchApplyFinished` with Failed/Declined, `McpToolCallFinished` with `is_error: true` | Todo | - |
| T1.3.4 | Write unit tests for all conversion paths | Todo | - |
| T1.3.5 | Write unit tests for "ignored" events (streaming chunks, deltas) that produce `None` | Todo | - |

#### Acceptance Criteria

- All decision-relevant `DomainEvent` variants convert correctly
- Streaming/delta events produce `None`
- Conversion is compile-time checked: adding a new `DomainEvent` variant without handling it in `From` causes a compile error

---

### Story 1.4: Migrate Existing Code to `agent-events`

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Update all crates to depend on `agent-events` instead of defining their own event types. Use type aliases to maintain backward compatibility.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Add `agent-events` dependency to `agent-provider`, `agent-core`, `agent-decision`, `agent-daemon` | Todo | - |
| T1.4.2 | Add type alias `pub type ProviderEvent = agent_events::DomainEvent;` in `agent/provider/src/lib.rs` for backward compat | Todo | - |
| T1.4.3 | Re-export `DomainEvent` from `agent-core` as `ProviderEvent` for internal code | Todo | - |
| T1.4.4 | Delete `core/src/pool/event_converter.rs` entirely | Todo | - |
| T1.4.5 | Update `agent-decision` to use `DecisionEvent` via `From` instead of its own `ProviderEvent` | Todo | - |
| T1.4.6 | Update all classifier code to use `DomainEvent` | Todo | - |
| T1.4.7 | Fix all compilation errors across workspace | Todo | - |
| T1.4.8 | Run full test suite: `cargo test --workspace` | Todo | - |

#### Acceptance Criteria

- `cargo test --workspace` passes with zero failures
- `event_converter.rs` no longer exists
- No duplicate `ProviderEvent` definitions remain
- All crates compile without warnings

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Type alias confusion during migration | Medium | Medium | Use deprecation attributes on aliases; clear migration guide |
| DecisionEvent misses edge case | Medium | High | Comprehensive unit tests for all conversion paths |
| Crate dependency cycles | Low | High | `agent-events` must have zero dependencies on other project crates |

## Sprint Deliverables

- `agent-events/` — New shared kernel crate
- `agent-events/src/lib.rs` — `DomainEvent`, `DecisionEvent`, `From` impls
- Deleted: `core/src/pool/event_converter.rs`
- All workspace tests passing

## Dependencies

None (this is the foundation sprint for the refactoring effort).

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Worker Aggregate Root](./sprint-02-worker-aggregate-root.md) for the core domain model extraction.
