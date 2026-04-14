# Sprint 6: Graceful Shutdown and Restore

## Metadata

- Sprint ID: `sprint-006`
- Title: `Graceful Shutdown and Restore`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1, Sprint 2, Sprint 3, Sprint 5

## Sprint Goal

Implement graceful shutdown with complete state capture and full session restore. Users can quit during active work and resume exactly where they left off.

## Stories

### Story 6.1: ShutdownSnapshot

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Create snapshot structure capturing complete shutdown state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.1.1 | Create `ShutdownSnapshot` struct | Todo | - |
| T6.1.2 | Create `AgentShutdownSnapshot` struct | Todo | - |
| T6.1.3 | Create `ShutdownReason` enum | Todo | - |
| T6.1.4 | Create `ProviderThreadSnapshot` struct | Todo | - |
| T6.1.5 | Create `TaskProgressMarker` struct | Todo | - |
| T6.1.6 | Write tests for snapshot creation | Todo | - |

#### Technical Notes

```rust
pub struct ShutdownSnapshot {
    shutdown_at: String,
    workplace_meta: WorkplaceMeta,
    agents: Vec<AgentShutdownSnapshot>,
    backlog: BacklogState,
    pending_mail: Vec<AgentMail>,
    shutdown_reason: ShutdownReason,
}

pub enum ShutdownReason {
    UserQuit,
    SystemSignal,
    ProviderTimeout,
    CriticalError { error: String },
    CleanExit,
}
```

---

### Story 6.2: Graceful Shutdown Procedure

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Implement multi-phase shutdown with provider signaling.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.2.1 | Implement Phase 1: Signal all providers to finish | Todo | - |
| T6.2.2 | Implement Phase 2: Collect snapshots from each agent | Todo | - |
| T6.2.3 | Implement Phase 3: Wait for threads with timeout | Todo | - |
| T6.2.4 | Implement Phase 4: Persist shutdown snapshot | Todo | - |
| T6.2.5 | Implement Phase 5: Final flush of pending ops | Todo | - |
| T6.2.6 | Implement Phase 6: Mark workplace as cleanly shutdown | Todo | - |
| T6.2.7 | Add shutdown lifecycle logging | Todo | - |
| T6.2.8 | Write tests for shutdown procedure | Todo | - |

#### Shutdown Phases

1. Signal all providers to finish current work
2. Collect final state from each agent
3. Wait for provider threads (with timeout)
4. Persist complete shutdown snapshot
5. Final flush of all pending persistence ops
6. Mark workplace as cleanly shutdown

---

### Story 6.3: Full Session Restore

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Implement restore from shutdown snapshot with resume options.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.3.1 | Check for shutdown snapshot on bootstrap | Todo | - |
| T6.3.2 | Load snapshot if exists | Todo | - |
| T6.3.3 | Restore workplace state from snapshot | Todo | - |
| T6.3.4 | Restore each agent from AgentShutdownSnapshot | Todo | - |
| T6.3.5 | Restore pending mail | Todo | - |
| T6.3.6 | Clear snapshot after successful restore | Todo | - |
| T6.3.7 | Write tests for full restore | Todo | - |
| T6.3.8 | Write tests for corrupted snapshot handling | Todo | - |

---

### Story 6.4: Resume Dialog UI

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Show resume options dialog when restoring from shutdown.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T6.4.1 | Design resume dialog layout | Todo | - |
| T6.4.2 | Show agents that were active at shutdown | Todo | - |
| T6.4.3 | Add [R] Resume all active agents option | Todo | - |
| T6.4.4 | Add [S] Start fresh (keep transcripts) option | Todo | - |
| T6.4.5 | Add [C] Cancel restore, start clean option | Todo | - |
| T6.4.6 | Write tests for resume dialog | Todo | - |

#### Mockup

```
┌─────────────────────────────────────────────────────────────────┐
│ ● Restored Session                                               │
│                                                                  │
│ Previous session had 3 active agents.                            │
│                                                                  │
│ ○ alpha [claude]  - was running task-1 (2 turns completed)      │
│   bravo [codex]   - was idle                                     │
│   charlie [mock]  - was responding (partial output)              │
│                                                                  │
│ [R] Resume all active agents                                     │
│ [S] Start fresh (keep transcripts)                               │
│ [C] Cancel restore, start clean                                  │
│                                                                  │
│ Press R to resume or S to start fresh                            │
└─────────────────────────────────────────────────────────────────┘
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Provider thread won't finish | Medium | Medium | Configurable timeout, force quit after timeout |
| Snapshot corruption | Low | High | Backup snapshot, validation on restore |
| Resume confusion | Medium | Low | Clear dialog with options |

## Sprint Deliverables

- `ShutdownSnapshot` structures
- Graceful shutdown procedure
- Full session restore
- Resume dialog UI

## Dependencies

- Sprint 1: AgentSlot, AgentPool
- Sprint 2: Provider thread management
- Sprint 5: Persistence coordinator

## Next Sprint

After completing this sprint, proceed to [Sprint 7: Cross-Agent Communication](./sprint-07-cross-agent-comm.md).