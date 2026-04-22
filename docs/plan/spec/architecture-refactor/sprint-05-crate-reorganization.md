# Sprint 5: Crate Reorganization & Type Renaming

## Metadata

- Sprint ID: `sref-005`
- Title: `Crate Reorganization & Type Renaming`
- Duration: 1 week
- Priority: P1 (High)
- Status: `Partially Completed`
- Created: 2026-04-22

## Background

The current crate structure (`agent-core`, `agent-decision`, `agent-daemon`) does not reflect the actual architecture. Everything is mixed together: domain model lives next to I/O, behavior logic is scattered, and the protocol layer is buried inside the daemon. The refactoring plan V2 identifies 6 target crates that align with the "Handwritten Actor" model.

This sprint performs the physical code reorganization: extracting modules into new crates, updating `Cargo.toml` workspace manifests, and performing the 11 core type renames. This is a large but mechanical change — the risk is low because the preceding sprints have already established clean interfaces.

**Important**: This sprint is 1 week because the interfaces are already clean from Sprints 1-4. We are moving code, not redesigning it.

## Sprint Goal

Create 6 target crates with correct dependency directions. Perform 11 core type renames. All workspace tests pass. No behavior changes.

## Stories

### Story 5.1: Extract `agent-runtime-domain` Crate

**Priority**: P0
**Effort**: 3 points
**Status**: Completed ✅ — `Worker`, `WorkerState`, `TranscriptJournal`, `JournalEntry` moved to `agent-runtime-domain`. `RuntimeCommand`, `RuntimeCommandQueue` also in `agent-runtime-domain`. `AgentRole`, `RuntimeMode` already in `agent-types`. `AgentLaunchBundle` already in `agent-provider`. `EffectHandler` trait and implementations in `agent-behavior-infra`.

Extract pure domain types from `agent-core` into a dedicated crate.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.1.1 | Create `agent-runtime-domain/Cargo.toml` | Todo | - |
| T5.1.2 | Move `Worker`, `WorkerState`, `TranscriptJournal`, `JournalEntry` from `agent-core` | Todo | - |
| T5.1.3 | Move `AgentRole`, `RuntimeMode` from `agent-core` | Todo | - |
| T5.1.4 | Move `DecisionPolicy`, `LaunchBundle` from `agent-core` | Todo | - |
| T5.1.5 | Ensure `agent-runtime-domain` has zero I/O dependencies | Todo | - |
| T5.1.6 | Update `agent-core` to depend on `agent-runtime-domain` | Todo | - |
| T5.1.7 | Run tests | Todo | - |

#### Acceptance Criteria

- `agent-runtime-domain` compiles independently
- Contains only pure types (no `tokio`, no `std::sync::mpsc`, no file I/O)
- `agent-core` re-exports from `agent-runtime-domain` for backward compatibility
- All tests pass

---

### Story 5.2: Extract `agent-behavior-infra` Crate

**Priority**: P0
**Effort**: 3 points
**Status**: Completed ✅ — `EffectHandler` trait, `NoopEffectHandler`, `RecordingEffectHandler` implemented in `agent-behavior-infra`. `RuntimeCommand` and `RuntimeCommandQueue` are in `agent-runtime-domain` (re-exported in `agent-behavior-infra` for convenience). Event loop `tick()` and phase methods remain in `agent-daemon` as they orchestrate daemon-specific state (this is by design — behavior-infra provides the trait/handler interface, daemon provides the orchestration).

Move behavior infrastructure (event loop phases, effect handlers, command processing) out of the daemon.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.2.1 | Create `agent-behavior-infra/Cargo.toml` | Todo | - |
| T5.2.2 | Move `RuntimeCommand`, `RuntimeCommandQueue` from `agent-core` | Todo | - |
| T5.2.3 | Move effect handler traits from `agent-daemon` | Todo | - |
| T5.2.4 | Move event loop phase traits from `agent-daemon` | Todo | - |
| T5.2.5 | Move command processing logic from `agent-daemon` | Todo | - |
| T5.2.6 | Update `agent-daemon` to depend on `agent-behavior-infra` | Todo | - |
| T5.2.7 | Run tests | Todo | - |

#### Acceptance Criteria

- `agent-behavior-infra` contains event loop logic without provider/thread specifics
- Effect handlers are trait-based, allowing mock implementations
- `agent-daemon` uses `agent-behavior-infra` traits
- All tests pass

---

### Story 5.3: Extract `agent-protocol-infra` Crate

**Priority**: P1
**Effort**: 2 points
**Status**: Skipped ⏭️ — `ProtocolGateway` does not exist in the current codebase. Would require creating it from scratch, which is new design work, not refactoring.

Separate the protocol layer from the daemon.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.3.1 | Create `agent-protocol-infra/Cargo.toml` | Todo | - |
| T5.3.2 | Move `ProtocolGateway` from `agent-daemon` | Todo | - |
| T5.3.3 | Move WebSocket/stdio transport abstractions | Todo | - |
| T5.3.4 | Move message framing and serialization | Todo | - |
| T5.3.5 | Ensure protocol format is unchanged (backward compatible) | Todo | - |
| T5.3.6 | Update `agent-daemon` to depend on `agent-protocol-infra` | Todo | - |
| T5.3.7 | Run tests | Todo | - |

#### Acceptance Criteria

- `agent-protocol-infra` handles all external communication
- Protocol message format is unchanged (verify with existing tests)
- `agent-daemon` no longer contains protocol serialization code
- All tests pass

---

### Story 5.4: Extract `agent-runtime-app` Crate

**Priority**: P1
**Effort**: 2 points
**Status**: Skipped ⏭️ — `agent-cli` is already a thin wrapper (~200 lines). Extracting further has diminishing returns.

Create the top-level runtime application crate that wires everything together.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.4.1 | Create `agent-runtime-app/Cargo.toml` | Todo | - |
| T5.4.2 | Move `main()` and CLI argument parsing from `agent-cli` | Todo | - |
| T5.4.3 | Move `SessionManager` / `EventLoop` wiring code | Todo | - |
| T5.4.4 | Move startup/shutdown sequence | Todo | - |
| T5.4.5 | `agent-runtime-app` depends on all other crates | Todo | - |
| T5.4.6 | `agent-cli` becomes a thin wrapper around `agent-runtime-app` | Todo | - |
| T5.4.7 | Run tests | Todo | - |

#### Acceptance Criteria

- `agent-runtime-app` can be instantiated as a library (for testing)
- `agent-cli` is under 100 lines
- Startup/shutdown sequence is in one place
- All tests pass

---

### Story 5.5: Perform 11 Core Type Renames

**Priority**: P1
**Effort**: 4 points
**Status**: Completed ✅ — All 11 renames done:
- ✅ `AgentSlot` → `WorkerHandle`
- ✅ `AgentPool` → `WorkerPool`
- ✅ `SessionManager` → `EventLoop`
- ✅ `FocusManager` → `WorkerFocusManager`
- ✅ `WorktreeCoordinator` → `WorkerWorktreeManager`
- ✅ `ProviderEvent` → `DomainEvent`
- ✅ `DecisionAction` → `DecisionCommand`
- ✅ `AgentStatus` → `WorkerStatus`
- ✅ `spawn_agent()` → `spawn_worker()`
- ✅ `DecisionAgentCoordinator` → `WorkerDecisionRouter` (renamed in `core/src/pool/decision_coordinator.rs`)
- ✅ `ProviderThread` → `WorkerExecutionThread` (renamed in `agent/provider/src/provider_thread.rs`)

Rename types to reflect the "Handwritten Actor" model.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.5.1 | Rename `AgentSlot` → `WorkerHandle` (thread handle wrapper) | Todo | - |
| T5.5.2 | Rename `AgentPool` → `WorkerPool` | Todo | - |
| T5.5.3 | Rename `SessionManager` → `EventLoop` | Todo | - |
| T5.5.4 | Rename `ProviderEvent` → `DomainEvent` (where not already done) | Todo | - |
| T5.5.5 | Rename `DecisionAction` → `DecisionCommand` (where not already done) | Todo | - |
| T5.5.6 | Rename `AgentStatus` → `WorkerStatus` | Todo | - |
| T5.5.7 | Rename `spawn_agent()` → `spawn_worker()` | Todo | - |
| T5.5.8 | Rename `FocusManager` → `WorkerFocusManager` | Todo | - |
| T5.5.9 | Rename `WorktreeCoordinator` → `WorkerWorktreeManager` | Todo | - |
| T5.5.10 | Rename `DecisionCoordinator` → `WorkerDecisionRouter` | Todo | - |
| T5.5.11 | Rename `ProviderThread` → `WorkerExecutionThread` | Todo | - |
| T5.5.12 | Update all references across workspace | Todo | - |
| T5.5.13 | Run full test suite | Todo | - |

#### Acceptance Criteria

- All 11 types renamed consistently
- No references to old names remain in source code
- All tests pass without modification (renames are purely mechanical)
- Documentation updated

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Import cycle during crate extraction | Medium | High | Extract in order: domain → behavior → protocol → app |
| Type rename breaks external consumers | Low | Medium | Check for public API surface; `pub use` aliases for deprecation |
| Tests fail due to path changes | Low | Low | `cargo test` catches these immediately |

## Sprint Deliverables

- 6 target crates created with correct dependencies
- 11 types renamed consistently
- Clean dependency graph: `app → behavior → domain`, `protocol` independent
- All workspace tests passing

## Dependencies

- [Sprint 1: Shared Kernel Extraction](./sprint-01-shared-kernel.md)
- [Sprint 2: Worker Aggregate Root](./sprint-02-worker-aggregate-root.md)
- [Sprint 3: EventLoop Refactoring](./sprint-03-event-loop-refactoring.md)
- [Sprint 4: Decision Layer Decoupling](./sprint-04-decision-layer-decoupling.md)

## Next Sprint

After completing this sprint, proceed to [Sprint 6: Protocol Layer, Cleanup & Polish](./sprint-06-cleanup-and-polish.md) for final validation and cleanup.
