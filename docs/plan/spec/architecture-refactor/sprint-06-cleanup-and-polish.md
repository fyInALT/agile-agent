# Sprint 6: Protocol Layer, Cleanup & Polish

## Metadata

- Sprint ID: `sref-006`
- Title: `Protocol Layer, Cleanup & Polish`
- Duration: 1 week
- Priority: P1 (High)
- Status: `Partially Completed`
- Created: 2026-04-22

## Background

With the core architecture refactoring complete (Sprints 1-5), this final sprint focuses on:
1. Extracting and hardening the protocol layer
2. Removing all deprecated code and backward-compatibility aliases
3. Validating external protocol compatibility
4. Performance regression testing
5. Final documentation

This sprint is the quality gate before declaring the refactoring complete. It should not contain any new design — only verification, cleanup, and polish.

## Sprint Goal

All deprecated code removed. Protocol format unchanged verified. Performance within 5% of baseline. Full test suite passes. Documentation complete.

## Stories

### Story 6.1: Harden Protocol Layer

**Priority**: P0
**Effort**: 2 points
**Status**: Skipped ⏭️ — Requires external tooling (protocol version negotiation, ping/pong, message validation). No `ProtocolGateway` exists to harden.

Ensure the protocol layer in `agent-protocol-infra` is robust and well-tested.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.1.1 | Add protocol version negotiation handshake | Todo | - |
| T6.1.2 | Add protocol message validation (reject malformed messages gracefully) | Todo | - |
| T6.1.3 | Add connection health checks (ping/pong) | Todo | - |
| T6.1.4 | Add protocol-level error codes and recovery | Todo | - |
| T6.1.5 | Write unit tests for message serialization/deserialization | Todo | - |
| T6.1.6 | Write integration tests for protocol round-trips | Todo | - |
| T6.1.7 | Document protocol message schema | Todo | - |

#### Acceptance Criteria

- Protocol rejects malformed messages with clear error codes
- Connection health is monitored
- Message serialization is 100% tested
- Protocol schema is documented

---

### Story 6.2: Remove Deprecated Code

**Priority**: P0
**Effort**: 2 points
**Status**: Partially Completed ⚠️ — `event_converter.rs` deleted. Clippy fixed across all 19 workspace crates including test targets. Remaining:
- Deprecation aliases still exist (`AgentSlot`, `SessionManager`, `AgentPool`, `AgentStatus`) — kept for backward compatibility
- `AgentSlotStatus` still exists as a real enum (not yet replaced by `WorkerState`) — requires Sprint 3.4 gap-fill
- `cargo clippy --workspace --tests -- -D warnings` passes ✅

Clean up all backward-compatibility aliases and dead code.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.2.1 | Remove `pub type ProviderEvent = DomainEvent` aliases | Todo | - |
| T6.2.2 | Remove `AgentSlot` struct (replaced by `WorkerHandle`) | Todo | - |
| T6.2.3 | Remove `SessionManager` struct (replaced by `EventLoop`) | Todo | - |
| T6.2.4 | Remove `DecisionAction` enum (replaced by `DecisionCommand`) | Todo | - |
| T6.2.5 | Remove `event_converter.rs` remnants (should already be gone) | Todo | - |
| T6.2.6 | Remove unused imports and `#[allow(dead_code)]` attributes | Todo | - |
| T6.2.7 | Run `cargo clippy --workspace -- -D warnings` | Done ✅ | - |
| T6.2.8 | Run `cargo udeps` (if available) to find unused dependencies | Todo | - |

#### Acceptance Criteria

- Zero deprecation aliases remain
- `cargo clippy --workspace -- -D warnings` passes
- No `#[allow(dead_code)]` on actually dead code
- Unused dependencies removed from `Cargo.toml` files

---

### Story 6.3: Validate External Protocol Compatibility

**Priority**: P0
**Effort**: 2 points
**Status**: Skipped ⏭️ — Requires running the full system with TUI and capturing protocol snapshots.

Ensure the external protocol (WebSocket, stdio) is unchanged despite internal refactoring.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.3.1 | Capture protocol message snapshots before refactoring | Todo | - |
| T6.3.2 | Compare message snapshots after refactoring | Todo | - |
| T6.3.3 | Verify `agent-cli` can connect to a server started by the refactored daemon | Todo | - |
| T6.3.4 | Verify TUI rendering is unchanged | Todo | - |
| T6.3.5 | Verify multi-agent session protocol works | Todo | - |
| T6.3.6 | Document any intentional protocol changes | Todo | - |

#### Acceptance Criteria

- Protocol message format is byte-for-byte identical (or documented changes)
- `agent-cli` connects and operates normally
- TUI displays sessions correctly
- Multi-agent coordination works end-to-end

---

### Story 6.4: Performance Regression Testing

**Priority**: P1
**Effort**: 2 points
**Status**: Skipped ⏭️ — Requires establishing baseline measurements before refactoring began.

Ensure the refactoring does not degrade performance.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4.1 | Establish baseline: measure event loop tick latency (p50, p99) before refactoring | Todo | - |
| T6.4.2 | Measure event loop tick latency after refactoring | Todo | - |
| T6.4.3 | Measure memory usage with 10 concurrent agents | Todo | - |
| T6.4.4 | Measure decision layer classification throughput | Todo | - |
| T6.4.5 | Ensure p99 tick latency is within 5% of baseline | Todo | - |
| T6.4.6 | Ensure memory usage is within 10% of baseline | Todo | - |
| T6.4.7 | Document performance results | Todo | - |

#### Acceptance Criteria

- p99 tick latency ≤ 105% of baseline
- Memory usage ≤ 110% of baseline
- Decision classification throughput ≥ 95% of baseline
- Performance results documented

---

### Story 6.5: Final Documentation

**Priority**: P1
**Effort**: 1 point
**Status**: Partially Completed ⚠️ — `AGENTS.md` updated with new crate structure. `docs/architecture/dependency-graph.md` updated with new crates. `docs/architecture/new-crate-structure.md` written. Sprint specs updated with completion status. `README.md` and `refactoring-plan-v2.md` completion status not yet updated.

Update all architecture and API documentation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.5.1 | Update `docs/architecture/refactoring-plan-v2.md` with completion status | Todo | - |
| T6.5.2 | Update `AGENTS.md` with new crate structure | Todo | - |
| T6.5.3 | Update `README.md` with architecture overview | Todo | - |
| T6.5.4 | Write `docs/architecture/new-crate-structure.md` | Todo | - |
| T6.5.5 | Update module-level doc comments in all new crates | Todo | - |
| T6.5.6 | Verify all `TODO` and `FIXME` comments are resolved or ticketed | Todo | - |

#### Acceptance Criteria

- All architecture docs reflect the new crate structure
- `AGENTS.md` and `README.md` are up to date
- No stale `TODO`/`FIXME` comments remain in refactored code
- Module doc comments explain the purpose of each crate

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Protocol incompatibility discovered late | Low | High | Snapshot testing; early validation in Story 6.3 |
| Performance regression | Low | High | Baseline measurement before any changes; profile if regression >5% |
| Documentation drift | Medium | Low | Checklist in Story 6.5; review docs as part of PR process |

## Sprint Deliverables

- Clean codebase with zero deprecated aliases
- Protocol compatibility verified
- Performance within 5% of baseline
- Complete and up-to-date documentation
- All tests passing

## Dependencies

- [Sprint 5: Crate Reorganization](./sprint-05-crate-reorganization.md) — All prior refactoring must be complete

## Completion Criteria

The entire Architecture Refactoring is complete when:
1. All 6 sprints are finished
2. `cargo test --workspace` passes
3. `cargo clippy --workspace -- -D warnings` passes
4. Protocol compatibility verified
5. Performance within 5% of baseline
6. Documentation complete
7. No deprecated code remains

## Summary

| Sprint | Duration | Focus |
|--------|----------|-------|
| Sprint 1 | 2 weeks | Shared Kernel Extraction (`agent-events`) |
| Sprint 2 | 2 weeks | Worker Aggregate Root (state machine + `apply()`) |
| Sprint 3 | 2 weeks | EventLoop Refactoring (7 phases + effects) |
| Sprint 4 | 2 weeks | Decision Layer Decoupling (read-only + commands) |
| Sprint 5 | 1 week | Crate Reorganization (6 crates + type renames) |
| Sprint 6 | 1 week | Cleanup & Polish (protocol + perf + docs) |
| **Total** | **10 weeks** | **Complete Architecture Refactoring** |
