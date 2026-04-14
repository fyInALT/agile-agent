# Kanban System Specification

## Overview

This directory contains the Scrum-style breakdown of the kanban system implementation.

## Sprints

| Sprint | Name | Stories | Goal |
|--------|-------|---------|------|
| [Sprint 1](sprint-1-core-domain-model.md) | Core Domain Model | 3 | Domain types and KanbanElement model |
| [Sprint 2](sprint-2-repository-layer.md) | Repository Layer | 3 | Repository trait and file implementation |
| [Sprint 3](sprint-3-service-layer.md) | Service Layer | 3 | Business logic, validation, dependency checking |
| [Sprint 4](sprint-4-event-system.md) | Event System | 3 | Publish/subscribe event bus |
| [Sprint 5](sprint-5-integration-polish.md) | Integration & Polish | 3 | SharedWorkplaceState integration, polish |

## Dependencies

```
Sprint 1 (Core Domain Model)
    ↓
Sprint 2 (Repository Layer) ────────┐
    ↓                              │
Sprint 3 (Service Layer) ──────────┤
    ↓                              │
Sprint 4 (Event System) ───────────┤
    ↓                              │
Sprint 5 (Integration) ────────────┘
```

## Stories Summary

### Sprint 1: Core Domain Model
- **Story 1.1**: Define Core Domain Types (Status, Priority, ElementId, ElementType)
- **Story 1.2**: Implement KanbanElement with All Variants
- **Story 1.3**: Add Status History Tracking

### Sprint 2: Repository Layer
- **Story 2.1**: Define KanbanElementRepository Trait
- **Story 2.2**: Implement FileKanbanRepository
- **Story 2.3**: Integrate with WorkplaceStore

### Sprint 3: Service Layer
- **Story 3.1**: Implement KanbanService - Create/Update Operations
- **Story 3.2**: Implement Status Transition Validation
- **Story 3.3**: Implement Dependency Checking

### Sprint 4: Event System
- **Story 4.1**: Define KanbanEvent Types
- **Story 4.2**: Implement KanbanEventBus
- **Story 4.3**: Integrate Events with KanbanService

### Sprint 5: Integration & Polish
- **Story 5.1**: Integrate with SharedWorkplaceState
- **Story 5.2**: Add GitOperations Placeholder
- **Story 5.3**: Polish and Missing Accessors

## File Structure

```
docs/plan/spec/kanban/
├── README.md                      # This file
├── sprint-1-core-domain-model.md
├── sprint-2-repository-layer.md
├── sprint-3-service-layer.md
├── sprint-4-event-system.md
└── sprint-5-integration-polish.md
```

## Target Module Structure

```
kanban/                    # agent-kanban crate (standalone)
├── Cargo.toml
└── src/
    ├── lib.rs             # Module exports
    ├── error.rs           # KanbanError enum
    ├── domain.rs          # KanbanElement, Status, Priority, ElementId, etc.
    ├── repository.rs      # KanbanElementRepository trait
    ├── file_repository.rs # FileKanbanRepository implementation
    ├── service.rs         # KanbanService
    ├── events.rs          # KanbanEvent, KanbanEventBus
    └── git_ops.rs         # GitOperations placeholder

core/                      # agent-core crate (depends on agent-kanban)
└── src/
    ├── lib.rs             # Will re-export agent_kanban types
    └── kanban_integration.rs  # Helper to create KanbanService from WorkplaceStore
```

## Running Tests

```bash
# Build the project
cargo build -p agent-kanban

# Run kanban tests
cargo test -p agent-kanban

# Build core (depends on kanban)
cargo build -p agent-core

# Run core tests
cargo test -p agent-core

# Format code
cargo fmt -p agent-kanban
cargo fmt -p agent-core
```

## Key Design Decisions

1. **Clean Architecture**: Separate domain, repository, service, and infrastructure layers
2. **Repository Trait**: Enables future database backend without changing service layer
3. **Event Bus**: Decouples storage updates from UI/external consumers
4. **Status History**: Each transition is logged with timestamp for cycle time tracking
5. **Human-Readable IDs**: Format `{type}-{number}` (e.g., `sprint-001`, `task-042`)
6. **Minimal index.json**: Only stores ID list to minimize merge conflicts
