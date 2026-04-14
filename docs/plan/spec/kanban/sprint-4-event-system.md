# Sprint 4: Event System

## Metadata

- Sprint: `Sprint 4`
- Goal: Establish the event system for publish/subscribe notifications
- Status: `Planning`
- Stories: 3
- Estimated Duration: 1-2 days
- Dependencies: Sprint 3 (Service Layer)

## Package Structure

```
kanban/                    # agent-kanban crate
├── src/
│   ├── lib.rs            # Module exports
│   ├── error.rs          # KanbanError enum
│   ├── domain.rs         # Domain types
│   ├── repository.rs     # KanbanElementRepository trait
│   ├── file_repository.rs # FileKanbanRepository implementation
│   ├── service.rs        # KanbanService
│   └── events.rs         # KanbanEvent, KanbanEventBus (new in this sprint)
└── tests/
    ├── service.rs        # Service tests
    └── events.rs         # Event tests
```

---

## Story 4.1: Define KanbanEvent Types

**As a** developer
**I need** well-defined event types for kanban changes
**So that** consumers can react to specific events

### Tasks

- [ ] **T4.1.1: Define KanbanEvent enum**
  - File: `kanban/src/events.rs`
  - Add `Created { element_id, element_type }`
  - Add `Updated { element_id, changes }` where changes is `Vec<String>`
  - Add `StatusChanged { element_id, old_status, new_status, changed_by }`
  - Add `Deleted { element_id }`
  - Add `TipAppended { task_id, tip_id, agent_id }`
  - Add `DependencyAdded { element_id, dependency }`
  - Add `DependencyRemoved { element_id, dependency }`
  - Add `Clone` derive (needed for event bus)

- [ ] **T4.1.2: Define KanbanEventSubscriber trait**
  - File: `kanban/src/events.rs`
  - Add trait with `on_event(event: &KanbanEvent)` method
  - Add `Send` bound for thread safety

- [ ] **T4.1.3: Export events from kanban module**
  - File: `kanban/src/lib.rs`
  - Export `KanbanEvent`
  - Export `KanbanEventSubscriber`

- [ ] **T4.1.4: Write unit tests for event types**
  - File: `kanban/tests/events.rs`
  - Test event creation with all variants
  - Test event cloning

### Acceptance Criteria

1. All event variants compile with correct field types
2. Events are `Clone` and `Debug`
3. Subscriber trait has correct signature
4. Events can be created and matched exhaustively
5. All unit tests pass

---

## Story 4.2: Implement KanbanEventBus

**As a** developer
**I need** an event bus for publish/subscribe
**So that** multiple consumers can receive kanban events

### Tasks

- [ ] **T4.2.1: Implement KanbanEventBus struct**
  - File: `kanban/src/events.rs`
  - Add `subscribers: RwLock<Vec<Box<dyn KanbanEventSubscriber + Send>>>` field
  - Add `new()` constructor

- [ ] **T4.2.2: Implement subscribe method**
  - File: `kanban/src/events.rs`
  - Accept `Box<dyn KanbanEventSubscriber + Send>`
  - Store in subscribers list (wrapped in RwLock)

- [ ] **T4.2.3: Implement publish method**
  - File: `kanban/src/events.rs`
  - Accept `KanbanEvent`
  - Iterate through all subscribers
  - Call `on_event()` on each subscriber
  - Use read lock for iteration

- [ ] **T4.2.4: Implement Default trait**
  - File: `kanban/src/events.rs`
  - Implement `Default` for `KanbanEventBus` using `new()`

- [ ] **T4.2.5: Write unit tests for event bus**
  - File: `kanban/tests/events.rs`
  - Test single subscriber receives events
  - Test multiple subscribers all receive events
  - Test subscriber can filter by event type
  - Test thread safety (concurrent subscribe/publish)

### Acceptance Criteria

1. `subscribe()` adds subscriber to list
2. `publish()` delivers event to all subscribers
3. Subscribers are called in order of subscription
4. `RwLock` allows concurrent reads, exclusive writes
5. All unit tests pass

---

## Story 4.3: Integrate Events with KanbanService

**As a** developer
**I need** KanbanService to publish events on all state changes
**So that** consumers (TUI, agents) can react to changes

### Tasks

- [ ] **T4.3.1: Review existing event publishing in service**
  - File: `kanban/src/service.rs`
  - Verify `create_element` publishes `Created` event
  - Verify `update_status` publishes `StatusChanged` event
  - Identify any missing event publications

- [ ] **T4.3.2: Implement append_tip method**
  - File: `kanban/src/service.rs`
  - Validate target is a Task
  - Create Tips element with target_task and agent_id
  - Call `create_element` to save
  - Publish `TipAppended` event

- [ ] **T4.3.3: Write unit tests for event integration**
  - File: `kanban/tests/service.rs`
  - Test `append_tip` creates tip and publishes event
  - Test event contains correct task_id and agent_id
  - Test error when appending tip to non-task element

- [ ] **T4.3.4: Update service tests to verify events**
  - File: `kanban/tests/service.rs`
  - Add test that verifies event is published on create
  - Add test that verifies event is published on status change
  - Use mock subscriber to capture events

### Acceptance Criteria

1. `create_element` publishes `Created` event
2. `update_status` publishes `StatusChanged` event with old/new status and agent
3. `append_tip` validates target is Task, creates Tips, publishes `TipAppended`
4. All events include correct element_id references
5. Event tests pass with mock subscriber verification

---

## Sprint 4 Completion Checklist

- [ ] All 3 stories completed
- [ ] All tasks checked off
- [ ] Code compiles: `cargo build -p agent-kanban`
- [ ] Tests pass: `cargo test -p agent-kanban`
- [ ] Code formatted: `cargo fmt -p agent-kanban`
- [ ] Committed: `git commit -m "feat(kanban): complete sprint 4 - event system"`
