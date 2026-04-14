# Sprint 3: Service Layer

## Metadata

- Sprint: `Sprint 3`
- Goal: Implement the business logic layer with validation and dependency checking
- Status: `Planning`
- Stories: 3
- Estimated Duration: 1-2 days
- Dependencies: Sprint 2 (Repository Layer)

## Package Structure

```
kanban/                    # agent-kanban crate
├── src/
│   ├── lib.rs            # Module exports
│   ├── error.rs          # KanbanError enum
│   ├── domain.rs         # Domain types
│   ├── repository.rs      # KanbanElementRepository trait
│   ├── file_repository.rs # FileKanbanRepository implementation
│   └── service.rs        # KanbanService (new in this sprint)
└── tests/
    └── service.rs        # Service tests
```

---

## Story 3.1: Implement KanbanService - Create/Update Operations

**As a** developer
**I need** a service layer for creating and updating elements
**So that** business logic is centralized and not duplicated

### Tasks

- [x] **T3.1.1: Implement KanbanService struct**
  - File: `kanban/src/service.rs`
  - Add `repository: Arc<R>` field
  - Add `event_bus: Arc<KanbanEventBus>` field
  - Add `new(repository, event_bus)` constructor

- [x] **T3.1.2: Implement create_element**
  - File: `kanban/src/service.rs`
  - Generate new ID using repository
  - Set `created_at` and `updated_at` timestamps
  - Save to repository
  - Publish `KanbanEvent::Created` event
  - Return created element

- [x] **T3.1.3: Implement get_element**
  - File: `kanban/src/service.rs`
  - Delegate to repository `get()`

- [x] **T3.1.4: Implement list_elements and list_by_type**
  - File: `kanban/src/service.rs`
  - Delegate to repository methods

- [x] **T3.1.5: Write unit tests for create operations**
  - File: `kanban/tests/service.rs`
  - Test `create_element` assigns sequential ID
  - Test `create_element` sets timestamps
  - Test `create_element` publishes event
  - Test `get_element` returns saved element
  - Test `list_elements` returns all elements

### Acceptance Criteria

1. `create_element` assigns next sequential ID (sprint-001, sprint-002, etc.)
2. `created_at` and `updated_at` are set to current time
3. Event is published to event bus after creation
4. Service returns the created element with assigned ID
5. All unit tests pass

---

## Story 3.2: Implement Status Transition Validation

**As a** developer
**I need** status transitions to be validated against the state machine
**So that** invalid transitions are rejected

### Tasks

- [x] **T3.2.1: Implement update_status with validation**
  - File: `kanban/src/service.rs`
  - Add `update_status(id, new_status, agent_id)` method
  - Validate element exists
  - Validate transition is valid using `can_transition_to()`
  - Record old status before change
  - Call `element.transition(new_status)`
  - Save updated element
  - Publish `KanbanEvent::StatusChanged` with old_status, new_status, changed_by

- [x] **T3.2.2: Handle transition failures gracefully**
  - File: `kanban/src/service.rs`
  - Return `KanbanError::InvalidStatusTransition` for invalid transitions
  - Ensure error message is descriptive

- [x] **T3.2.3: Write unit tests for status transitions**
  - File: `kanban/tests/service.rs`
  - Test valid transition: Plan → Backlog
  - Test invalid transition: Plan → Done (should fail)
  - Test invalid transition: Verified → Backlog (should fail)
  - Test that event is published on successful transition

### Acceptance Criteria

1. `update_status` validates against `Status.valid_transitions()`
2. Invalid transitions return `KanbanError::InvalidStatusTransition`
3. Valid transitions update element and publish event
4. `changed_by` field records which agent made the change
5. All unit tests pass

---

## Story 3.3: Implement Dependency Checking

**As a** developer
**I need** dependency checking before moving to InProgress or Done
**So that** items cannot be worked on until their dependencies are complete

### Tasks

- [x] **T3.3.1: Implement find_blocking_dependencies**
  - File: `kanban/src/service.rs`
  - Get element's dependencies list
  - For each dependency, check if status is `Done` or `Verified`
  - Return list of IDs that are blocking (not Done/Verified)
  - Handle dangling dependencies (reference to non-existent element)

- [x] **T3.3.2: Add dependency check to status transitions**
  - File: `kanban/src/service.rs`
  - Modify `update_status` to check dependencies when transitioning to `InProgress` or `Done`
  - Return `KanbanError::DependenciesNotMet` with list of blockers if dependencies not satisfied

- [x] **T3.3.3: Implement can_start helper**
  - File: `kanban/src/service.rs`
  - Add `can_start(id)` returning `true` if no blocking dependencies
  - Returns `false` if any dependency is not Done/Verified

- [x] **T3.3.4: Write unit tests for dependency checking**
  - File: `kanban/tests/service.rs`
  - Test element with no dependencies can transition freely
  - Test element with unmet dependency blocks at InProgress
  - Test element with unmet dependency blocks at Done
  - Test element with all dependencies Done/Verified can transition
  - Test dangling dependency returns error
  - Test `can_start` returns correct value

### Acceptance Criteria

1. `find_blocking_dependencies` returns IDs of incomplete dependencies
2. Moving to `InProgress` is blocked if any dependency is not Done/Verified
3. Moving to `Done` is blocked if any dependency is not Done/Verified
4. `can_start` returns `true` only when all dependencies are satisfied
5. Dangling dependency (references non-existent element) returns error
6. All unit tests pass

---

## Sprint 3 Completion Checklist

- [x] All 3 stories completed
- [x] All tasks checked off
- [x] Code compiles: `cargo build -p agent-kanban`
- [x] Tests pass: `cargo test -p agent-kanban`
- [x] Code formatted: `cargo fmt -p agent-kanban`
- [x] Committed: `git commit -m "feat(kanban): complete sprint 3 - service layer"`
