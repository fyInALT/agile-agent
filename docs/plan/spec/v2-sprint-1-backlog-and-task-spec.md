# V2 Sprint 1 Backlog and Task Spec

## Metadata

- Sprint: `V2 / Sprint 1`
- Stories covered:
  - `V2-S01`
  - `V2-S02`
  - `V2-S03`

## 1. Purpose

This sprint introduces the first autonomous-loop substrate for V2:

- a persisted backlog
- a task model
- one autonomous loop iteration

The purpose is to prove that `agile-agent` can move from “interactive shell” to “single-step autonomous executor”.

## 2. Scope

### In scope

- backlog storage
- todo/task state model
- backlog inspection
- converting one backlog item into one executable task
- running one loop iteration end to end

### Out of scope

- continuation policy
- completion judge
- verification
- escalation
- multi-iteration loops
- headless mode

## 3. Sprint Goal

The system can select one todo, derive one task, execute it once, and record the result.

## 4. Product Decisions

- Backlog is the source of truth for autonomous work.
- Task generation should be explicit and persisted.
- One successful iteration is enough for this sprint.

## 5. Detailed Execution Checklist

## S1-T01 Add backlog and todo domain models

- Define todo/backlog structs and status enums.
- Keep fields minimal but stable.

## S1-T02 Add task domain model

- Define task structs and task status enums.
- Include objective, scope, constraints, and verification-plan placeholder.

## S1-T03 Add backlog persistence

- Save backlog to local disk.
- Load backlog from local disk.

## S1-T04 Add backlog inspection surface

- Add a basic TUI transcript-visible or panel-visible backlog inspection path.

## S1-T05 Add todo-to-task generation

- Turn one ready todo into one draft/ready task.

## S1-T06 Add one autonomous loop iteration

- Select one todo.
- Generate one task.
- Execute one provider turn.
- Record the result.

## 6. Acceptance

Sprint 1 is done when:

1. Backlog items persist locally.
2. At least one todo can become one task.
3. One autonomous iteration can run from backlog item to provider result.

## 7. Test Plan

- unit tests for backlog/task state transitions
- save/load tests
- one loop smoke test

## 8. Review Demo

1. Show a persisted backlog.
2. Show one todo becoming one task.
3. Run one autonomous iteration.
