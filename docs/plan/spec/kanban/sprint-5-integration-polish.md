# Sprint 5: Integration & Polish

## Metadata

- Sprint: `Sprint 5`
- Goal: Integrate kanban with multi-agent runtime and add remaining polish
- Status: `Planning`
- Stories: 3
- Estimated Duration: 1-2 days
- Dependencies: Sprint 4 (Event System)

## Package Structure

```
kanban/                    # agent-kanban crate (standalone)
├── src/
│   ├── lib.rs            # Module exports
│   ├── error.rs          # KanbanError enum
│   ├── domain.rs         # Domain types
│   ├── repository.rs     # KanbanElementRepository trait
│   ├── file_repository.rs # FileKanbanRepository implementation
│   ├── service.rs        # KanbanService
│   ├── events.rs         # KanbanEvent, KanbanEventBus
│   └── git_ops.rs        # GitOperations placeholder (new in this sprint)
└── tests/
    └── git_ops.rs        # GitOperations tests

core/                      # agent-core crate (depends on agent-kanban)
└── src/
    └── lib.rs            # Will add: pub use agent_kanban::{...}
```

---

## Story 5.1: Integrate with SharedWorkplaceState

**As a** developer
**I need** kanban accessible through SharedWorkplaceState
**So that** multiple agents can access the shared kanban

### Tasks

- [ ] **T5.1.1: Add core re-export of kanban types**
  - File: `core/src/lib.rs`
  - Add `pub use agent_kanban::{KanbanElement, KanbanService, ...}` re-exports
  - This allows `core` users to access kanban types through `agent_core::kanban::*`

- [ ] **T5.1.2: Create integration helper in core**
  - File: `core/src/kanban_integration.rs` (new file)
  - Add helper to create `KanbanService<FileKanbanRepository>` from `WorkplaceStore`
  - Create `FileKanbanRepository` with workplace path
  - Create `KanbanEventBus`
  - Return configured `KanbanService`

- [ ] **T5.1.3: Write integration tests**
  - File: `core/tests/kanban_integration.rs` (new file)
  - Test creating service from WorkplaceStore
  - Test CRUD through SharedWorkplaceState
  - Test event subscription through SharedWorkplaceState

### Acceptance Criteria

1. SharedWorkplaceState owns KanbanService and KanbanEventBus
2. Agents can access kanban via `workplace.kanban()`
3. Event subscription works through SharedWorkplaceState
4. Files are stored in correct location under workplace
5. Integration tests pass (run: `cargo test -p agent-kanban`)

---

## Story 5.2: Add GitOperations Placeholder

**As a** developer
**I need** a GitOperations placeholder for future Git collaboration
**So that** the interface is ready when Git integration is implemented

### Tasks

- [ ] **T5.2.1: Create GitOperations struct**
  - File: `kanban/src/git_ops.rs`
  - Add `GitOperations` struct with `repo_path: PathBuf`
  - Add `new(repo_path)` constructor

- [ ] **T5.2.2: Define GitOperation methods (placeholder)**
  - File: `kanban/src/git_ops.rs`
  - Add `commit_changes(agent_id, message)` → `Result<(), GitError>`
  - Add `fetch_and_rebase(branch)` → `Result<(), GitError>`
  - Add `has_conflicts()` → `bool`

- [ ] **T5.2.3: Define GitError type**
  - File: `kanban/src/git_ops.rs`
  - Add `GitError` struct with `message: String`
  - Implement `Display` trait
  - Implement `Debug` trait

- [ ] **T5.2.4: Export GitOperations from kanban module**
  - File: `kanban/src/lib.rs`
  - Export `GitOperations`

- [ ] **T5.2.5: Write unit tests for GitOperations**
  - File: `kanban/tests/git_ops.rs`
  - Test placeholder methods return appropriate results
  - Test GitError display format

### Acceptance Criteria

1. GitOperations struct exists with correct fields
2. All methods have placeholder implementations (returning Ok/false)
3. GitError has Display and Debug implementations
4. Module exports GitOperations
5. Tests pass (basic placeholder tests)

---

## Story 5.3: Polish and Missing Accessors

**As a** developer
**I need** complete accessor methods on domain types
**So that** service and repository layers can modify elements correctly

### Tasks

- [ ] **T5.3.1: Add base_mut accessor to KanbanElement**
  - File: `kanban/src/domain.rs`
  - Add `base_mut(&mut self) -> &mut BaseElement` to KanbanElement
  - Delegate to variant's base field
  - Used by tests to set dependencies before creation

- [ ] **T5.3.2: Add ElementId.number() method**
  - File: `kanban/src/domain.rs`
  - Add `number(&self) -> u32` to ElementId
  - Parse and return the numeric portion

- [ ] **T5.3.3: Add ElementId.type_() method**
  - File: `kanban/src/domain.rs`
  - Add `type_(&self) -> ElementType` to ElementId
  - Parse and return the type portion

- [ ] **T5.3.4: Verify all trait implementations**
  - Review: `impl Hash for ElementId` works correctly
  - Review: `impl Serialize for ElementId` works correctly
  - Review: `impl<'de> Deserialize<'de> for ElementId` works correctly

- [ ] **T5.3.5: Run full test suite**
  - Run: `cargo test -p agent-kanban`
  - Fix any failing tests
  - Ensure all domain, repository, service, and events tests pass

### Acceptance Criteria

1. `KanbanElement.base_mut()` allows mutable access to BaseElement
2. `ElementId.number()` returns the numeric portion correctly
3. `ElementId.type_()` returns the ElementType correctly
4. All tests pass: `cargo test -p agent-kanban`
5. Code is formatted: `cargo fmt -p agent-kanban`

---

## Sprint 5 Completion Checklist

- [ ] All 3 stories completed
- [ ] All tasks checked off
- [ ] Code compiles: `cargo build -p agent-kanban`
- [ ] Tests pass: `cargo test -p agent-kanban`
- [ ] Code formatted: `cargo fmt -p agent-kanban`
- [ ] Committed: `git commit -m "feat(kanban): complete sprint 5 - integration and polish"`

---

## Kanban System Implementation Complete

After all 5 sprints, the kanban system will have:

| Component | Sprint | Status |
|-----------|--------|--------|
| Domain Types (Status, Priority, ElementId, ElementType) | 1 | Complete |
| KanbanElement (Sprint, Story, Task, Idea, Issue, Tips) | 1 | Complete |
| Status History Tracking | 1 | Complete |
| KanbanElementRepository Trait | 2 | Complete |
| FileKanbanRepository Implementation | 2 | Complete |
| WorkplaceStore Integration | 2 | Complete |
| KanbanService (Create/Update) | 3 | Complete |
| Status Transition Validation | 3 | Complete |
| Dependency Checking | 3 | Complete |
| KanbanEvent Types | 4 | Complete |
| KanbanEventBus (Publish/Subscribe) | 4 | Complete |
| Event Integration with Service | 4 | Complete |
| SharedWorkplaceState Integration | 5 | Complete |
| GitOperations Placeholder | 5 | Complete |
| Polish & Missing Accessors | 5 | Complete |
