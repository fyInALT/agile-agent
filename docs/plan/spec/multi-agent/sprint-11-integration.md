# Sprint 11: Integration & Migration

## Metadata

- Sprint ID: `sprint-011`
- Title: `Integration & Migration`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1-10

## Sprint Goal

Complete backward compatibility, migration path from V2, and full integration testing. Multi-agent is opt-in with seamless upgrade from single-agent.

## Stories

### Story 11.1: RuntimeMode Enum

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Create RuntimeMode for backward compatibility.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.1.1 | Create `RuntimeMode` enum (SingleAgent, MultiAgent) | Todo | - |
| T11.1.2 | Default to SingleAgent mode | Todo | - |
| T11.1.3 | Add `--multi-agent` CLI flag | Todo | - |
| T11.1.4 | Switch mode when spawning second agent | Todo | - |
| T11.1.5 | Write tests for mode switching | Todo | - |

---

### Story 11.2: Data Migration

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Migrate existing single-agent data to multi-agent format.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.2.1 | Detect legacy single-agent data | Todo | - |
| T11.2.2 | Move meta.json to agents/agent_001/ | Todo | - |
| T11.2.3 | Move state.json to agents/agent_001/ | Todo | - |
| T11.2.4 | Move transcript.json to agents/agent_001/ | Todo | - |
| T11.2.5 | Create workplace-level meta.json | Todo | - |
| T11.2.6 | Preserve existing backlog | Todo | - |
| T11.2.7 | Write migration tests | Todo | - |
| T11.2.8 | Write rollback tests | Todo | - |

---

### Story 11.3: Headless Multi-Agent CLI

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Add headless CLI commands for multi-agent operations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.3.1 | Add `run-loop --multi-agent` command | Todo | - |
| T11.3.2 | Add `agent spawn <provider>` command | Todo | - |
| T11.3.3 | Add `agent stop <agent-id>` command | Todo | - |
| T11.3.4 | Add `agent list --all` command | Todo | - |
| T11.3.5 | Add `agent status <agent-id>` command | Todo | - |
| T11.3.6 | Write tests for headless commands | Todo | - |

---

### Story 11.4: Integration Testing

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Comprehensive integration tests for multi-agent runtime.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.4.1 | Test 2-agent concurrent execution | Todo | - |
| T11.4.2 | Test 5-agent concurrent execution | Todo | - |
| T11.4.3 | Test shutdown/restore cycle | Todo | - |
| T11.4.4 | Test concurrent persistence | Todo | - |
| T11.4.5 | Test task assignment/completion | Todo | - |
| T11.4.6 | Test mail delivery | Todo | - |
| T11.4.7 | Test kanban concurrent access | Todo | - |
| T11.4.8 | Write stress tests | Todo | - |

---

### Story 11.5: Documentation Update

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Update project documentation for multi-agent features.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T11.5.1 | Update README.md with multi-agent section | Todo | - |
| T11.5.2 | Update CLAUDE.md with multi-agent guidance | Todo | - |
| T11.5.3 | Create multi-agent user guide | Todo | - |
| T11.5.4 | Document migration steps | Todo | - |
| T11.5.5 | Document CLI commands | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Migration data loss | Low | High | Backup before migration, rollback support |
| Performance degradation | Medium | Medium | Profile and optimize critical paths |
| Backward compatibility break | Low | High | Extensive single-agent tests |

## Sprint Deliverables

- RuntimeMode with backward compatibility
- Data migration logic
- Headless multi-agent CLI
- Comprehensive integration tests
- Updated documentation

## Dependencies

- All previous sprints (Sprint 1-10)

## Final Release Checklist

After Sprint 11, validate:

1. [ ] Single-agent mode works unchanged
2. [ ] Multi-agent mode opt-in via flag
3. [ ] Migration from V2 seamless
4. [ ] All integration tests pass
5. [ ] Documentation complete
6. [ ] Performance acceptable (<10% overhead)

## Project Complete

This is the final sprint. After completion, the multi-agent parallel runtime is production-ready.