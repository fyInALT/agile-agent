# Sprint 10: Scrum Coordination

## Metadata

- Sprint ID: `sprint-010`
- Title: `Scrum Coordination`
- Duration: 2 weeks
- Priority: P3 (Low)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 7, Sprint 9

## Sprint Goal

Implement foundation for Scrum-style coordination with ProductOwner, ScrumMaster, and Developer agent roles. Basic sprint planning and daily standup support.

## Stories

### Story 10.1: Agent Role System

**Priority**: P3
**Effort**: 3 points
**Status**: Backlog

Create agent role enum and role-based behavior hints.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.1.1 | Create `AgentRole` enum (ProductOwner, ScrumMaster, Developer) | Todo | - |
| T10.1.2 | Add `role` field to AgentSlot | Todo | - |
| T10.1.3 | Create role-specific prompt prefixes | Todo | - |
| T10.1.4 | Implement role-based skill filtering | Todo | - |
| T10.1.5 | Write tests for role system | Todo | - |

#### Agent Roles

| Role | Focus | Default Skills |
|------|-------|----------------|
| ProductOwner | Requirements, priorities, backlog grooming | requirements, planning |
| ScrumMaster | Process, blockers, coordination | process, standup |
| Developer | Implementation, testing, delivery | coding, testing |

---

### Story 10.2: Sprint Planning Assistant

**Priority**: P3
**Effort**: 3 points
**Status**: Backlog

Implement sprint planning support for ProductOwner role.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.2.1 | Create `SprintPlanningSession` struct | Todo | - |
| T10.2.2 | Implement story prioritization view | Todo | - |
| T10.2.3 | Implement effort estimation helper | Todo | - |
| T10.2.4 | Implement sprint goal definition | Todo | - |
| T10.2.5 | Implement sprint commitment tracking | Todo | - |
| T10.2.6 | Write tests for sprint planning | Todo | - |

---

### Story 10.3: Daily Standup Generation

**Priority**: P3
**Effort**: 3 points
**Status**: Backlog

Implement daily standup report generation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.3.1 | Create `DailyStandupReport` struct | Todo | - |
| T10.3.2 | Derive from status_history | Todo | - |
| T10.3.3 | Show yesterday's completed tasks | Todo | - |
| T10.3.4 | Show today's planned tasks | Todo | - |
| T10.3.5 | Show blockers | Todo | - |
| T10.3.6 | Generate standup summary | Todo | - |
| T10.3.7 | Write tests for standup generation | Todo | - |

#### Mockup

```
Daily Standup - 2026-04-14

Agent Alpha:
- Yesterday: task-001 (ready→done), task-002 (done)
- Today: task-003 (ready)
- Blockers: none

Agent Bravo:
- Yesterday: story-002 (backlog→in_progress)
- Today: story-002 (in_progress)
- Blockers: story-002 waiting on story-001
```

---

### Story 10.4: Blocker Escalation Flow

**Priority**: P3
**Effort**: 2 points
**Status**: Backlog

Implement blocker detection and escalation for ScrumMaster.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T10.4.1 | Detect blocked agents from status | Todo | - |
| T10.4.2 | Create blocker escalation mail | Todo | - |
| T10.4.3 | Send to ScrumMaster role agent | Todo | - |
| T10.4.4 | Track blocker resolution time | Todo | - |
| T10.4.5 | Write tests for blocker escalation | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Role assignment confusion | Medium | Low | Clear role prompts, visual indicators |
| Standup timing | Low | Low | Manual trigger, not automatic |

## Sprint Deliverables

- `AgentRole` enum and role system
- Sprint planning helpers
- Daily standup generation
- Blocker escalation flow

## Dependencies

- Sprint 7: Mail system (for escalations)
- Sprint 9: Kanban system (for sprint/story/task)

## Next Sprint

After completing this sprint, proceed to [Sprint 11: Integration & Migration](./sprint-11-integration.md).