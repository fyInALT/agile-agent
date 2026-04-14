# Sprint 1: Core Domain Model

## Metadata

- Sprint: `Sprint 1`
- Goal: Establish the core domain types and KanbanElement model
- Status: `Planning`
- Stories: 3
- Estimated Duration: 1-2 days

## Package Structure

```
kanban/                    # New standalone crate
├── Cargo.toml
└── src/
    ├── lib.rs            # Module exports
    ├── error.rs          # KanbanError enum
    └── domain.rs          # Domain types

core/                      # Existing crate
└── src/
    └── lib.rs            # Will depend on kanban crate
```

---

## Story 1.1: Define Core Domain Types

**As a** developer
**I need** core domain types for status, priority, element ID, and element type
**So that** the kanban system has a consistent type system

### Tasks

- [x] **T1.1.1: Create kanban crate structure**
  - Create `kanban/Cargo.toml` with:
    - `name = "agent-kanban"`
    - `edition.workspace = true`
    - `version.workspace = true`
    - Dependencies: `serde`, `serde_json`, `chrono`
  - Create `kanban/src/lib.rs` with module declarations

- [x] **T1.1.2: Add workspace member**
  - Modify `Cargo.toml` workspace to add `"kanban"` to members list

- [x] **T1.1.3: Implement Status enum**
  - File: `kanban/src/domain.rs`
  - Add `Status` enum with variants: `Plan`, `Backlog`, `Blocked`, `Ready`, `Todo`, `InProgress`, `Done`, `Verified`
  - Add `valid_transitions()` method returning valid target states
  - Add `can_transition_to()` method for validation
  - Add `is_terminal()` method (only `Verified` is terminal)
  - Add `serde` serialization with `snake_case` rename

- [x] **T1.1.4: Implement Priority enum**
  - File: `kanban/src/domain.rs`
  - Add `Priority` enum with variants: `Critical`, `High`, `Medium`, `Low`
  - Add `as_str()` method
  - Add `from_str()` method
  - Add `serde` serialization with `lowercase` rename

- [x] **T1.1.5: Implement ElementId type**
  - File: `kanban/src/domain.rs`
  - Add `ElementId` wrapper struct around `String`
  - Add `new(type_, number)` constructor generating format `{type}-{number:03}`
  - Add `parse()` method for deserialization with validation
  - Add `as_str()` method
  - Add `number()` method returning the numeric portion
  - Add `type_()` method returning the `ElementType`
  - Implement `Display` trait
  - Implement `Hash` trait for use in collections

- [x] **T1.1.6: Implement ElementType enum**
  - File: `kanban/src/domain.rs`
  - Add `ElementType` enum with variants: `Sprint`, `Story`, `Task`, `Idea`, `Issue`, `Tips`
  - Add `as_str()` method
  - Add `from_str()` method (accept both "tip" and "tips")

- [x] **T1.1.7: Write unit tests for domain types**
  - File: `kanban/tests/domain.rs`
  - Test `Status.valid_transitions()`
  - Test `Status.can_transition_to()` for valid and invalid transitions
  - Test `ElementId.parse()` with valid and invalid inputs
  - Test `Priority.from_str()`

### Acceptance Criteria

1. All domain types compile without errors
2. Status state machine follows design: `plan → backlog → blocked/ready/todo → in_progress → done → verified`
3. `Verified` is the only terminal state
4. `ElementId` format is validated: `{type}-{number}` e.g. `sprint-001`, `task-042`
5. All unit tests pass

---

## Story 1.2: Implement KanbanElement with All Variants

**As a** developer
**I need** KanbanElement enum with Sprint, Story, Task, Idea, Issue, and Tips variants
**So that** all kanban items are represented consistently

### Tasks

- [x] **T1.2.1: Implement BaseElement struct**
  - File: `kanban/src/domain.rs`
  - Add `BaseElement` struct with all common fields: `id`, `title`, `content`, `keywords`, `status`, `dependencies`, `references`, `parent`, `created_at`, `updated_at`, `priority`, `assignee`, `effort`, `blocked_reason`, `tags`, `status_history`
  - Add `new(type_, title)` constructor initializing with defaults
  - Add `can_transition_to()` method delegating to status
  - Add `transition()` method with validation and history recording
  - Add `serde` defaults for optional fields

- [x] **T1.2.2: Implement element variant structs**
  - File: `kanban/src/domain.rs`
  - Add `Sprint` struct with `#[serde(flatten)] base: BaseElement` and `goal`, `start_date`, `end_date`, `active` fields
  - Add `Story` struct with `#[serde(flatten)] base: BaseElement`
  - Add `Task` struct with `#[serde(flatten)] base: BaseElement`
  - Add `Idea` struct with `#[serde(flatten)] base: BaseElement`
  - Add `Issue` struct with `#[serde(flatten)] base: BaseElement`
  - Add `Tips` struct with `#[serde(flatten)] base: BaseElement`, `target_task`, `agent_id` fields
  - Add constructor methods for each variant

- [x] **T1.2.3: Implement KanbanElement enum**
  - File: `kanban/src/domain.rs`
  - Add `#[serde(tag = "type")]` enum with all variants
  - Add constructor methods: `new_sprint()`, `new_story()`, `new_task()`, `new_idea()`, `new_issue()`, `new_tips()`
  - Add accessor methods: `id()`, `element_type()`, `status()`, `can_transition_to()`, `transition()`, `assignee()`, `dependencies()`, `references()`, `parent()`
  - Add mutable setters: `set_id()`, `set_status()`, `set_updated_at()`, `set_created_at()`, `base_mut()`

- [x] **T1.2.4: Write unit tests for KanbanElement**
  - File: `kanban/tests/element.rs`
  - Test sprint creation with goal
  - Test task creation with parent
  - Test tips creation with target_task and agent_id
  - Test status transition validation
  - Test that Tips has correct target_task reference

### Acceptance Criteria

1. `KanbanElement` serializes to JSON with `"type": "sprint|story|task|idea|issue|tip"` field
2. All variant constructors work correctly
3. `transition()` fails for invalid status changes
4. `parent` relationship is set correctly for Story (→ Sprint) and Task (→ Story)
5. Tips stores `target_task` and `agent_id` correctly
6. All unit tests pass

---

## Story 1.3: Add Status History Tracking

**As a** developer
**I need** status history entries for each element
**So that** cycle time metrics can be calculated

### Tasks

- [x] **T1.3.1: Implement StatusHistoryEntry struct**
  - File: `kanban/src/domain.rs`
  - Add `StatusHistoryEntry` struct with `status: Status` and `entered_at: DateTime<Utc>` fields
  - Add `new(status)` constructor using `Utc::now()`
  - Add `serde` serialization

- [x] **T1.3.2: Integrate status history into BaseElement**
  - File: `kanban/src/domain.rs`
  - Ensure `status_history` is initialized with entry for initial `Plan` status
  - Ensure `transition()` appends new entry to history
  - Add `#[serde(default)]` to allow empty history on deserialization

- [x] **T1.3.3: Write unit tests for status history**
  - File: `kanban/tests/domain.rs`
  - Test that new element has one history entry (Plan)
  - Test that transition adds new history entry
  - Test that history preserves order

### Acceptance Criteria

1. Every element starts with one `StatusHistoryEntry` for `Plan` status
2. Every `transition()` call adds a new entry with current timestamp
3. History entries are serialized/deserialized correctly
4. All unit tests pass

---

## Sprint 1 Completion Checklist

- [x] All 3 stories completed
- [x] All tasks checked off
- [x] Code compiles: `cargo build -p agent-kanban`
- [x] Tests pass: `cargo test -p agent-kanban`
- [x] Code formatted: `cargo fmt -p agent-kanban`
- [x] Committed: `git commit -m "feat(kanban): complete sprint 1 - core domain model"`
