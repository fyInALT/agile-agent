# Sprint 3: Task Distribution

## Metadata

- Sprint ID: `sprint-003`
- Title: `Task Distribution`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1, Sprint 2

## Sprint Goal

Implement task assignment to specific agents and task completion tracking. Multiple agents can complete tasks concurrently with proper backlog updates.

## Stories

### Story 3.1: Task Assignment

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement assigning tasks to specific agent slots.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Add `assigned_task_id` to AgentSlot | Todo | - |
| T3.1.2 | Implement `AgentPool::assign_task()` | Todo | - |
| T3.1.3 | Validate task exists before assignment | Todo | - |
| T3.1.4 | Validate agent is idle before assignment | Todo | - |
| T3.1.5 | Update backlog status on assignment | Todo | - |
| T3.1.6 | Write unit tests for task assignment | Todo | - |

#### Acceptance Criteria

- Task can be assigned to idle agent
- Assignment updates backlog status
- Cannot assign to busy agent

---

### Story 3.2: Task Completion Tracking

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Track task completion per agent with proper backlog updates.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Create `TaskCompletionResult` enum | Todo | - |
| T3.2.2 | Implement `AgentPool::complete_task()` | Todo | - |
| T3.2.3 | Update backlog task status on completion | Todo | - |
| T3.2.4 | Clear `assigned_task_id` on completion | Todo | - |
| T3.2.5 | Emit `TaskCompleted` event | Todo | - |
| T3.2.6 | Write unit tests for task completion | Todo | - |
| T3.2.7 | Write integration tests for concurrent completion | Todo | - |

#### Technical Notes

```rust
pub enum TaskCompletionResult {
    Success { summary: String, artifacts: Vec<Artifact> },
    Failed { error: String },
    Blocked { reason: String },
    Escalated { escalation: Escalation },
}
```

---

### Story 3.3: Backlog Concurrent Access

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement thread-safe backlog access with Mutex.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Wrap backlog in `Mutex<BacklogState>` | Todo | - |
| T3.3.2 | Implement `BacklogState::assign_task()` | Todo | - |
| T3.3.3 | Implement `BacklogState::complete_task()` | Todo | - |
| T3.3.4 | Implement `BacklogState::list_ready_tasks()` | Todo | - |
| T3.3.5 | Add lock timeout to prevent deadlocks | Todo | - |
| T3.3.6 | Write tests for concurrent backlog access | Todo | - |

#### Acceptance Criteria

- Multiple agents can read backlog simultaneously
- Write operations are serialized
- No deadlock possible

---

### Story 3.4: Task Queue Visualization Helpers

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Create helpers for TUI to display task queue state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Create `TaskQueueSnapshot` struct | Todo | - |
| T3.4.2 | Implement `AgentPool::task_queue_snapshot()` | Todo | - |
| T3.4.3 | Add per-agent assigned task info | Todo | - |
| T3.4.4 | Add pending/unassigned task count | Todo | - |
| T3.4.5 | Write tests for snapshot generation | Todo | - |

#### Acceptance Criteria

- Snapshot shows all task states
- Per-agent assignment visible
- Updated on every change

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Backlock deadlock | Low | High | Short lock duration, timeout |
| Task assignment race | Medium | Medium | Atomic check-and-assign |

## Sprint Deliverables

- Modified `AgentSlot` with task assignment
- Modified `AgentPool` with task methods
- `BacklogState` with Mutex wrapper
- Task queue visualization helpers

## Dependencies

- Sprint 1: AgentSlot, AgentPool
- Sprint 2: Provider Threads (for concurrent completion)

## Next Sprint

After completing this sprint, proceed to [Sprint 4: Basic Multi-Agent TUI](./sprint-04-basic-tui.md).