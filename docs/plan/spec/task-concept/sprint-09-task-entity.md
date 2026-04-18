# Sprint 09: Task Entity Foundation

## Metadata

- Sprint ID: `decision-sprint-009`
- Title: `Task Entity Foundation`
- Duration: 1-2 weeks
- Priority: P0 (Critical)
- Status: `Completed`
- Created: 2026-04-18
- Depends on: `decision-sprint-008` (Integration)

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for detailed requirements:
- FR-001: Task Structure Definition
- FR-002: Task Status Enumeration
- FR-003: Task Status Transitions

## Sprint Goal

Establish the Task entity as the core unit of work tracking in the decision layer, enabling lifecycle management and state transitions with full audit capability.

## Context

The decision layer currently lacks a structured task concept. Decisions are made without tracking:
- What task is being executed
- How many reflection cycles have occurred
- How many completion confirmations have been attempted
- The execution history and decision trail

This sprint introduces the Task entity to serve as the foundation for all future decision workflow enhancements.

## Stories

### Story 9.1: Task Structure Definition

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the Task struct with all required fields for lifecycle tracking.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.1.1 | Define `TaskId` type (UUID-based) | Todo | - |
| T9.1.2 | Create `Task` struct with core fields | Todo | - |
| T9.1.3 | Add `constraints` field for task boundaries | Todo | - |
| T9.1.4 | Add reflection/confirmation count fields | Todo | - |
| T9.1.5 | Add timestamps (created_at, updated_at) | Todo | - |
| T9.1.6 | Implement `Serialize/Deserialize` for persistence | Todo | - |
| T9.1.7 | Write unit tests for Task creation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T9.1.T1 | TaskId generation is unique |
| T9.1.T2 | Task created with correct defaults |
| T9.1.T3 | Task serialization/deserialization works |
| T9.1.T4 | Task constraints stored correctly |
| T9.1.T5 | Timestamps set on creation |

#### Acceptance Criteria

- Task struct defined with all FR-001 required fields
- TaskId is unique and serializable
- Task can be serialized to JSON for persistence
- Unit tests pass with >90% coverage

#### Technical Notes

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskId(String); // UUID-based

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub description: String,
    pub constraints: Vec<String>,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub reflection_count: usize,
    pub confirmation_count: usize,
    pub max_reflection_rounds: usize,
    pub retry_count: usize,
}

impl Task {
    pub fn new(description: String, constraints: Vec<String>) -> Self {
        Self {
            id: TaskId::generate(),
            description,
            constraints,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            reflection_count: 0,
            confirmation_count: 0,
            max_reflection_rounds: 2, // default
            retry_count: 0,
        }
    }
}
```

### Story 9.2: Task Status Enumeration

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define all possible task states as an enumeration.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.2.1 | Define `TaskStatus` enum with all states | Todo | - |
| T9.2.2 | Add display method for TUI rendering | Todo | - |
| T9.2.3 | Implement `Serialize/Deserialize` | Todo | - |
| T9.2.4 | Write unit tests for all status values | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T9.2.T1 | All status variants defined |
| T9.2.T2 | Status display returns readable text |
| T9.2.T3 | Status serialization works |

#### Acceptance Criteria

- TaskStatus enum covers all FR-002 states
- Each status has display text for TUI
- Serialization works correctly

#### Technical Notes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Reflecting,
    PendingConfirmation,
    NeedsHumanDecision,
    Paused,
    Completed,
    Cancelled,
}

impl TaskStatus {
    pub fn display(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::InProgress => "In Progress",
            Self::Reflecting => "Reflecting",
            Self::PendingConfirmation => "Awaiting Confirmation",
            Self::NeedsHumanDecision => "Needs Decision",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }
}
```

### Story 9.3: Task Status Transitions

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Define valid state transitions and implement transition validation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.3.1 | Define transition rules table | Todo | - |
| T9.3.2 | Implement `Task::transition_to()` | Todo | - |
| T9.3.3 | Add validation for invalid transitions | Todo | - |
| T9.3.4 | Log transition in execution history | Todo | - |
| T9.3.5 | Update timestamps on transition | Todo | - |
| T9.3.6 | Write unit tests for all valid transitions | Todo | - |
| T9.3.7 | Write unit tests for invalid transitions | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T9.3.T1 | Pending → InProgress works |
| T9.3.T2 | InProgress → Reflecting works |
| T9.3.T3 | InProgress → PendingConfirmation works |
| T9.3.T4 | Reflecting → InProgress works |
| T9.3.T5 | Reflecting → NeedsHumanDecision works (limit reached) |
| T9.3.T6 | PendingConfirmation → Completed works |
| T9.3.T7 | PendingConfirmation → Reflecting works (rejected) |
| T9.3.T8 | Invalid transition returns error |
| T9.3.T9 | Transition updates timestamp |

#### Acceptance Criteria

- All FR-003 transitions implemented
- Invalid transitions return clear error
- Each transition logged (preparation for FR-014)
- Timestamps updated on state change

#### Technical Notes

```rust
#[derive(Debug, Clone, thiserror::Error)]
pub enum TransitionError {
    #[error("Invalid transition from {from} to {to}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },
}

impl Task {
    pub fn transition_to(&mut self, new_status: TaskStatus) -> Result<(), TransitionError> {
        if !self.is_valid_transition(new_status) {
            return Err(TransitionError::InvalidTransition {
                from: self.status,
                to: new_status,
            });
        }
        
        self.status = new_status;
        self.updated_at = Utc::now();
        // Log transition (will be implemented in Sprint 12)
        Ok(())
    }
    
    fn is_valid_transition(&self, new_status: TaskStatus) -> bool {
        match (self.status, new_status) {
            (TaskStatus::Pending, TaskStatus::InProgress) => true,
            (TaskStatus::InProgress, TaskStatus::Reflecting) => true,
            (TaskStatus::InProgress, TaskStatus::PendingConfirmation) => true,
            (TaskStatus::InProgress, TaskStatus::Paused) => true,
            (TaskStatus::InProgress, TaskStatus::Cancelled) => true,
            (TaskStatus::Reflecting, TaskStatus::InProgress) => true,
            (TaskStatus::Reflecting, TaskStatus::NeedsHumanDecision) => true,
            (TaskStatus::PendingConfirmation, TaskStatus::Completed) => true,
            (TaskStatus::PendingConfirmation, TaskStatus::Reflecting) => true,
            (TaskStatus::NeedsHumanDecision, TaskStatus::InProgress) => true,
            (TaskStatus::NeedsHumanDecision, TaskStatus::Cancelled) => true,
            (TaskStatus::Paused, TaskStatus::InProgress) => true,
            (TaskStatus::Paused, TaskStatus::Cancelled) => true,
            (_, TaskStatus::Cancelled) => true, // Any → Cancelled
            _ => false,
        }
    }
}
```

### Story 9.4: Task Helper Methods

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement helper methods for task state queries.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.4.1 | Implement `is_active()` method | Todo | - |
| T9.4.2 | Implement `is_complete()` method | Todo | - |
| T9.4.3 | Implement `needs_reflection()` check | Todo | - |
| T9.4.4 | Implement `can_continue()` method | Todo | - |
| T9.4.5 | Write unit tests for helpers | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T9.4.T1 | is_active() true for InProgress/Reflecting |
| T9.4.T2 | is_complete() true for Completed |
| T9.4.T3 | needs_reflection() checks count vs limit |
| T9.4.T4 | can_continue() checks appropriate states |

#### Acceptance Criteria

- Helper methods correctly query task state
- Methods used by decision engine in later sprints

#### Technical Notes

```rust
impl Task {
    pub fn is_active(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress | TaskStatus::Reflecting)
    }
    
    pub fn is_complete(&self) -> bool {
        self.status == TaskStatus::Completed
    }
    
    pub fn needs_reflection(&self) -> bool {
        self.reflection_count < self.max_reflection_rounds
    }
    
    pub fn can_continue(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress | TaskStatus::Reflecting)
    }
}
```

## Sprint Deliverables

1. `Task` struct with full lifecycle fields
2. `TaskId` unique identifier type
3. `TaskStatus` enum with all states
4. Validated state transitions with error handling
5. Helper methods for state queries
6. Unit tests with >90% coverage

## Sprint Review Checklist

- [x] All tasks completed
- [x] All tests passing
- [x] Task entity can be created and serialized
- [x] Status transitions validated
- [x] Code reviewed and merged
- [x] Documentation updated

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| Transition rules too restrictive | Start with minimal valid transitions, extend later |
| Missing states discovered later | Status enum is extensible, add new variants |
| Persistence format changes | Use stable serialization format (JSON) |

## Next Sprint Preview

Sprint 10 will build on this foundation to define:
- DecisionStage and StageTransition
- Condition evaluation system
- DecisionAction types
- DecisionProcess workflow definition