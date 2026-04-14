# Sprint 9: Kanban System

## Metadata

- Sprint ID: `sprint-009`
- Title: `Kanban System`
- Duration: 2 weeks
- Priority: P2 (Medium)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1, Sprint 3

## Sprint Goal

Implement Git-backed Kanban system for multi-agent task management. All agents share a human-readable JSON-based backlog under workplace kanban directory.

## Stories

### Story 9.1: KanbanElement Domain Model

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Create domain model for all kanban element types.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.1.1 | Create `KanbanElement` enum (Sprint, Story, Task, Idea, Issue, Tips) | Todo | - |
| T9.1.2 | Create `ElementId` type with format `{type}-{number}` | Todo | - |
| T9.1.3 | Create `ElementType` enum | Todo | - |
| T9.1.4 | Create `Status` enum with state machine | Todo | - |
| T9.1.5 | Create `Priority` enum | Todo | - |
| T9.1.6 | Implement `KanbanElementExt` trait | Todo | - |
| T9.1.7 | Implement status transition validation | Todo | - |
| T9.1.8 | Write unit tests for domain model | Todo | - |

#### Element Types

| Type | Description | Hierarchy |
|------|-------------|----------|
| `sprint` | Time-boxed iteration | Top-level container |
| `story` | User-facing feature | Child of sprint |
| `task` | Granular work item | Child of story |
| `idea` | Underdeveloped thought | Independent |
| `issue` | Problem to address | Independent |
| `tips` | Note attached to task | Independent |

---

### Story 9.2: FileKanbanRepository

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement file-based repository for kanban elements.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.2.1 | Create `FileKanbanRepository` struct | Todo | - |
| T9.2.2 | Implement `get()` for single element | Todo | - |
| T9.2.3 | Implement `list()` for all elements | Todo | - |
| T9.2.4 | Implement `list_by_type()` | Todo | - |
| T9.2.5 | Implement `list_by_status()` | Todo | - |
| T9.2.6 | Implement `list_by_parent()` | Todo | - |
| T9.2.7 | Implement `save()` for element persistence | Todo | - |
| T9.2.8 | Implement `delete()` for element removal | Todo | - |
| T9.2.9 | Implement `next_id()` for auto-generation | Todo | - |
| T9.2.10 | Write unit tests for repository | Todo | - |

#### Storage Structure

```
~/.agile-agent/workplaces/{workplace_id}/kanban/
├── index.json       # Minimal ID registry
└── elements/
    ├── sprint-001.json
    ├── story-001.json
    ├── task-001.json
    ├── idea-001.json
    └── ...
```

---

### Story 9.3: KanbanService

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement application service for kanban operations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.3.1 | Create `KanbanService` struct | Todo | - |
| T9.3.2 | Implement `create_element()` | Todo | - |
| T9.3.3 | Implement `update_element()` | Todo | - |
| T9.3.4 | Implement `update_status()` with validation | Todo | - |
| T9.3.5 | Implement `append_tip()` for task tips | Todo | - |
| T9.3.6 | Implement `find_blocking_dependencies()` | Todo | - |
| T9.3.7 | Implement `can_start()` dependency check | Todo | - |
| T9.3.8 | Implement `list_by_sprint()` | Todo | - |
| T9.3.9 | Write unit tests for service | Todo | - |

---

### Story 9.4: KanbanEventBus

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Implement event system for kanban change notifications.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.4.1 | Create `KanbanEvent` enum | Todo | - |
| T9.4.2 | Create `KanbanEventBus` struct | Todo | - |
| T9.4.3 | Create `KanbanEventSubscriber` trait | Todo | - |
| T9.4.4 | Implement `subscribe()` | Todo | - |
| T9.4.5 | Implement `publish()` | Todo | - |
| T9.4.6 | Write tests for event bus | Todo | - |

#### KanbanEvent Types

```rust
pub enum KanbanEvent {
    Created { element_id: ElementId, element_type: ElementType },
    Updated { element_id: ElementId, changes: ChangeSummary },
    StatusChanged { element_id: ElementId, old_status: Status, new_status: Status },
    Deleted { element_id: ElementId },
    TipAppended { task_id: ElementId, tip_id: ElementId, agent_id: String },
}
```

---

### Story 9.5: SharedWorkplaceState Integration

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Integrate kanban service with multi-agent runtime.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.5.1 | Add `KanbanService` to `SharedWorkplaceState` | Todo | - |
| T9.5.2 | Add `KanbanEventBus` to `SharedWorkplaceState` | Todo | - |
| T9.5.3 | Implement `AgentSlot::read_kanban()` | Todo | - |
| T9.5.4 | Implement `AgentSlot::update_task_status()` | Todo | - |
| T9.5.5 | Implement `AgentSlot::append_tip()` | Todo | - |
| T9.5.6 | Write integration tests | Todo | - |

---

### Story 9.6: Status History for Cycle Time

**Priority**: P3
**Effort**: 2 points
**Status**: Backlog

Implement status history tracking for metrics.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.6.1 | Add `status_history` field to elements | Todo | - |
| T9.6.2 | Implement status change timestamp logging | Todo | - |
| T9.6.3 | Create cycle time calculation helpers | Todo | - |
| T9.6.4 | Write tests for status history | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Git merge conflicts | Medium | Medium | Each element in separate file, atomic writes |
| Concurrent file writes | Low | Medium | Serialized writes per element type |
| Index staleness | Low | Low | Traverse directory, don't rely on index |

## Sprint Deliverables

- `core/src/kanban/element.rs` - Domain model
- `core/src/kanban/repository.rs` - File repository
- `core/src/kanban/service.rs` - KanbanService
- `core/src/kanban/event.rs` - Event bus
- Integration with SharedWorkplaceState

## Dependencies

- Sprint 1: SharedWorkplaceState
- Sprint 3: Task distribution (use kanban tasks)

## Next Sprint

After completing this sprint, proceed to [Sprint 10: Scrum Coordination](./sprint-10-scrum-coordination.md).