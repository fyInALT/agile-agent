# Sprint 11: Cleanup + Performance Validation

## Metadata

- Sprint ID: `sprint-fbs-011`
- Title: `Cleanup + Performance Validation`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 10: Hardening](./sprint-10-hardening.md)

## Background

All functional work is done. The daemon is the sole state owner. The TUI and CLI are pure protocol clients. Reconnect, approval, and error handling all work. But the codebase still contains legacy embedded-mode code paths, feature flags, and re-export stubs that were needed during the transition. The `agent-core` crate may have dead code. Documentation still describes the old architecture. No performance benchmarks exist to validate the claim that WebSocket overhead is negligible.

This sprint is the final cleanup and validation. It removes all legacy code, runs performance benchmarks, updates documentation, and prepares the release. This is not optional polish — leaving dead code creates permanent technical debt that future developers will trip over.

## Sprint Goal

All legacy embedded-mode code is removed. The system passes performance benchmarks with multiple concurrent clients. Documentation is updated. The release is ready for deployment. This is the final sprint of the frontend-backend separation.

## TDD Approach

Cleanup removes code, which removes tests. The TDD discipline here is: delete code only after confirming it has no callers.

1. **Red**: Before deleting any module, run the full test suite. All tests must pass — this establishes the baseline.
2. **Green**: Delete the legacy code. Run tests again. If any test fails, the deleted code was still needed — restore and investigate.
3. **Refactor**: After deletion, verify workspace compiles and all tests pass.

Test requirements per story:
- Deletion safety: `cargo test --workspace` passes before and after each removal
- Coverage audit: overall workspace coverage does not drop below 80%
- Performance benchmarks: event latency, snapshot size, memory usage — all measured and documented
- Regression tests: full E2E test suite (daemon + TUI + CLI) passes
- Documentation tests: all code examples in docs compile and run correctly

## Stories

### Story 11.1: Remove Embedded-Mode Fallback

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Delete all code paths that supported running without a daemon.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.1.1 | Remove `embedded-mode` feature from `tui/Cargo.toml` | Todo | - |
| T11.1.2 | Remove `embedded-mode` feature from `cli/Cargo.toml` | Todo | - |
| T11.1.3 | Delete `TuiState::bootstrap()` embedded path | Todo | - |
| T11.1.4 | Delete `AppState::bootstrap()` if only used by embedded TUI | Todo | - |
| T11.1.5 | Remove `--embedded-mode` CLI flag | Todo | - |
| T11.1.6 | Remove embedded-mode conditional compilation (`#[cfg(feature)]`) | Todo | - |
| T11.1.7 | Write compilation check: `cargo build --workspace` succeeds | Todo | - |
| T11.1.8 | Write compilation check: `cargo test --workspace` passes | Todo | - |

#### Acceptance Criteria

- No `embedded-mode` feature exists in any `Cargo.toml`
- No `--embedded-mode` flag in CLI
- `cargo build --workspace` compiles without the feature
- All tests pass
- **Tests**: `no_embedded_feature` — `Cargo.toml` has no `embedded-mode`; `workspace_builds` — `cargo build --workspace` passes; `workspace_tests_pass` — `cargo test --workspace` passes


#### Technical Notes

See IMP-09 §5. This is the point of no return. After this sprint, the daemon is mandatory. Ensure all team members and CI are ready.

---

### Story 11.2: Remove Core Re-Export Stubs

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Clean up `agent-core` re-exports that were created solely for TUI consumption.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.2.1 | Audit `core/src/lib.rs` for re-exports used only by TUI | Todo | - |
| T11.2.2 | Remove unused `pub use` statements | Todo | - |
| T11.2.3 | Make internal modules `pub(crate)` where appropriate | Todo | - |
| T11.2.4 | Verify no downstream crate breaks | Todo | - |
| T11.2.5 | Write compilation check: full workspace builds | Todo | - |

#### Acceptance Criteria

- `core/src/lib.rs` contains only re-exports needed by daemon and other core consumers
- No `pub` items are unused
- Workspace compiles cleanly
- **Tests**: `no_dead_reexports` — `cargo build` succeeds after re-export removal; `core_compiles` — `agent-core` compiles without TUI-only re-exports


---

### Story 11.3: Multi-Client Performance Test

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Validate performance with multiple concurrent TUI and CLI clients.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.3.1 | Write benchmark: 1 daemon + 1 TUI + 1 CLI, 1000 events | Todo | - |
| T11.3.2 | Write benchmark: 1 daemon + 3 TUIs, 1000 events each | Todo | - |
| T11.3.3 | Measure event latency (daemon assigns seq → client receives) | Todo | - |
| T11.3.4 | Measure snapshot size and generation time | Todo | - |
| T11.3.5 | Measure memory usage of daemon under load | Todo | - |
| T11.3.6 | Define performance budget: <5ms event latency, <100MB daemon RSS | Todo | - |
| T11.3.7 | Write benchmark: reconnect with 10,000-event replay | Todo | - |
| T11.3.8 | Document benchmark results and any regressions | Todo | - |

#### Acceptance Criteria

- Event latency (daemon → client) is under 5ms on localhost
- Daemon memory usage is under 100MB with 3 clients and active agents
- Snapshot generation is under 50ms for typical sessions
- Event replay (10,000 events) completes in under 2s
- **Tests**: `latency_under_5ms` — event latency < 5ms; `memory_under_100mb` — daemon RSS < 100MB; `snapshot_under_50ms` — snapshot generation < 50ms; `replay_under_2s` — 10k event replay < 2s


#### Technical Notes

See IMP-08 §6. Use `criterion` for micro-benchmarks and custom integration tests for end-to-end latency. Run benchmarks on CI to detect regressions.

---

### Story 11.4: Documentation Update

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Update all user-facing and developer documentation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.4.1 | Update `README.md` quick start (mention auto-link) | Todo | - |
| T11.4.2 | Update `AGENTS.md` architecture diagram | Todo | - |
| T11.4.3 | Update `docs/refactor/architecture-blueprint.md` if outdated | Todo | - |
| T11.4.4 | Add `docs/plan/spec/frontend-backend-separation/README.md` with sprint summary | Todo | - |
| T11.4.5 | Update CLI help text for new commands | Todo | - |
| T11.4.6 | Write migration guide for users upgrading from embedded mode | Todo | - |

#### Acceptance Criteria

- New users can set up and run with daemon mode from README alone
- Architecture diagrams show daemon + protocol + clients
- Migration guide covers all breaking changes
- **Tests**: `readme_works` — new user can follow README from scratch; `architecture_diagram_updated` — diagram shows daemon + protocol + clients


---

### Story 11.5: Release Notes + Deprecation Notice

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Prepare release notes and communicate changes to users.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.5.1 | Write release notes for v0.11.0 | Todo | - |
| T11.5.2 | List all new commands (`daemon start/stop/status`, `run`) | Todo | - |
| T11.5.3 | List all removed flags (`--embedded-mode`) | Todo | - |
| T11.5.4 | Document known issues and workarounds | Todo | - |
| T11.5.5 | Tag release in git | Todo | - |
| T11.5.6 | Archive implementation specs to `docs/archived/` | Todo | - |

#### Acceptance Criteria

- Release notes are complete and accurate
- Users understand what changed and why
- No undocumented breaking changes
- **Tests**: `release_notes_complete` — all changes documented; `no_undocumented_breaking` — every breaking change listed; `docs_compile` — all code examples in docs run correctly


---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Performance regression vs embedded mode | Low | High | Benchmark both modes before removing embedded; optimize if needed |
| Documentation misses edge cases | Medium | Medium | Have a team member follow README from scratch |
| CI breaks after embedded-mode removal | Medium | High | Update CI matrix before this sprint |

## Sprint Deliverables

- Clean workspace with no embedded-mode code
- Performance benchmark results
- Updated documentation
- Release notes for v0.11.0
- Archived refactor specs

## Dependencies

- [Sprint 10: Hardening](./sprint-10-hardening.md) — system must be stable before final cleanup.

## Next Steps

After this sprint, the frontend-backend separation is complete. Future work (out of scope):

- **LAN mode**: Bind daemon to `0.0.0.0` for multi-machine use
- **Protocol v2**: Compression, binary frames, batch requests
- **Web dashboard**: Browser-based client using the same protocol
- **IDE plugins**: VS Code / JetBrains extensions

---

## Sprint Retrospective Template

At the end of this sprint, the team should discuss:

1. **What went well?** What patterns from this refactor should we keep?
2. **What was hard?** What would we do differently in future architectural refactors?
3. **Technical debt remaining?** Any shortcuts taken that need cleanup?
4. **Protocol stability**: Is the v1 protocol ready for external clients?
