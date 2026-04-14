# Sprint 2: Repository Layer

## Metadata

- Sprint: `Sprint 2`
- Goal: Establish the repository abstraction and file-based implementation
- Status: `Planning`
- Stories: 3
- Estimated Duration: 1-2 days
- Dependencies: Sprint 1 (Core Domain Model)

## Package Structure

```
kanban/                    # agent-kanban crate
├── src/
│   ├── lib.rs            # Module exports
│   ├── error.rs          # KanbanError enum
│   ├── domain.rs         # Domain types
│   ├── repository.rs     # KanbanElementRepository trait
│   └── file_repository.rs  # FileKanbanRepository implementation
└── tests/
    └── repository.rs     # Repository tests
```

---

## Story 2.1: Define KanbanElementRepository Trait

**As a** developer
**I need** a repository trait for element persistence
**So that** storage implementation can be swapped (file → database) without changing business logic

### Tasks

- [x] **T2.1.1: Define KanbanElementRepository trait**
  - File: `kanban/src/repository.rs`
  - Define trait with `Send + Sync` bounds
  - Define `get(id)` → `Result<Option<KanbanElement>>`
  - Define `list()` → `Result<Vec<KanbanElement>>`
  - Define `list_by_type(type_)` → `Result<Vec<KanbanElement>>`
  - Define `list_by_status(status)` → `Result<Vec<KanbanElement>>`
  - Define `list_by_assignee(assignee)` → `Result<Vec<KanbanElement>>`
  - Define `list_by_parent(parent)` → `Result<Vec<KanbanElement>>`
  - Define `list_blocked()` → `Result<Vec<KanbanElement>>`
  - Define `save(element)` → `Result<()>`
  - Define `delete(id)` → `Result<()>`
  - Define `new_id(type_)` → `Result<ElementId>`

- [x] **T2.1.2: Write unit tests for repository trait**
  - File: `kanban/tests/repository.rs`
  - Write mock tests documenting expected behavior
  - Note: Actual tests require FileKanbanRepository from Story 2.2

### Acceptance Criteria

1. Trait compiles with all method signatures
2. Trait has `Send + Sync` bounds for thread safety
3. Methods return `Result` for error handling
4. Tests compile (may be skipped until Story 2.2)

---

## Story 2.2: Implement FileKanbanRepository

**As a** developer
**I need** a file-based repository implementation
**So that** elements can be persisted to JSON files in the workplace directory

### Tasks

- [x] **T2.2.1: Implement FileKanbanRepository struct**
  - File: `kanban/src/file_repository.rs`
  - Add `base_path`, `index_path`, `elements_path` PathBuf fields
  - Add `counters: RwLock<HashMap<ElementType, u32>>` for ID generation
  - Add `Index` struct for minimal `index.json` format: `{ elements: ["id1", "id2", ...] }`
  - Add `new(base_path)` constructor creating `kanban/` and `kanban/elements/` directories

- [x] **T2.2.2: Implement ID generation**
  - File: `kanban/src/file_repository.rs`
  - Add `load_counters()` to scan existing files and find max IDs
  - Add `new_id(type_)` to increment counter and return `ElementId`
  - Add `element_path(id)` helper to build file path

- [x] **T2.2.3: Implement CRUD operations**
  - File: `kanban/src/file_repository.rs`
  - Implement `get(id)` - read from `{id}.json`
  - Implement `list()` - read all `.json` files, sort by ID
  - Implement `save(element)` - write to `{id}.json`, update index
  - Implement `delete(id)` - remove file, update index
  - Implement `update_index()` - write minimal index

- [x] **T2.2.4: Implement query methods**
  - File: `kanban/src/file_repository.rs`
  - Implement `list_by_type()` - filter by element type
  - Implement `list_by_status()` - filter by status
  - Implement `list_by_assignee()` - filter by assignee
  - Implement `list_by_parent()` - filter by parent ID
  - Implement `list_blocked()` - filter for Blocked status

- [x] **T2.2.5: Write unit tests for FileKanbanRepository**
  - File: `kanban/tests/repository.rs`
  - Test directory creation
  - Test save and get element
  - Test list returns all elements
  - Test list_by_type filters correctly
  - Test list_by_status filters correctly
  - Test list_by_assignee filters correctly
  - Test delete removes file and updates index
  - Test new_id generates sequential IDs

### Acceptance Criteria

1. Repository creates `kanban/elements/` directory on initialization
2. `save()` writes human-readable JSON file: `kanban/elements/{id}.json`
3. `get()` reads and deserializes element from file
4. `list()` returns all elements sorted by ID
5. Query methods filter correctly
6. `index.json` is minimal (ID list only, no metadata)
7. `new_id()` generates sequential IDs per type
8. All unit tests pass with `TempDir`

---

## Story 2.3: Integrate with WorkplaceStore

**As a** developer
**I need** to access kanban storage through WorkplaceStore
**So that** agents can access the kanban from their workplace context

### Tasks

- [x] **T2.3.1: Add kanban directory methods to WorkplaceStore**
  - File: `core/src/workplace_store.rs`
  - Add `kanban_dir()` returning `path.join("kanban")`
  - Add `kanban_elements_dir()` returning `kanban_dir().join("elements")`

- [x] **T2.3.2: Create FileKanbanRepository from WorkplaceStore**
  - File: `kanban/src/lib.rs` or integration module
  - Add `FileKanbanRepository::from_workplace(workplace)` constructor
  - Or add helper function to create repository from workplace path

- [x] **T2.3.3: Write integration tests**
  - File: `kanban/tests/integration.rs`
  - Test creating repository from WorkplaceStore path
  - Test CRUD operations through repository

### Acceptance Criteria

1. `WorkplaceStore` provides `kanban_dir()` and `kanban_elements_dir()`
2. FileKanbanRepository can be created with workplace path
3. Files are stored in correct location: `{workplace}/kanban/elements/{id}.json`
4. Integration tests pass

---

## Sprint 2 Completion Checklist

- [x] All 3 stories completed
- [x] All tasks checked off
- [x] Code compiles: `cargo build -p agent-kanban`
- [x] Tests pass: `cargo test -p agent-kanban`
- [x] Code formatted: `cargo fmt -p agent-kanban`
- [x] Committed: `git commit -m "feat(kanban): complete sprint 2 - repository layer"`
