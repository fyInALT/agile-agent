# Sprint 5: Persistence

## Metadata

- Sprint ID: `sprint-005`
- Title: `Persistence`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1, Sprint 2, Sprint 3

## Sprint Goal

Implement concurrent persistence for multiple agents with file isolation and periodic flush. All agent states persist correctly on quit and restore on restart.

## Stories

### Story 5.1: AgentPersistenceCoordinator

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Create coordinator for managing persistence operations across agents.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.1.1 | Create `PersistenceOp` enum for operation types | Todo | - |
| T5.1.2 | Create `AgentPersistenceCoordinator` struct | Todo | - |
| T5.1.3 | Implement `queue()` for persistence operations | Todo | - |
| T5.1.4 | Implement `flush()` for batch persistence | Todo | - |
| T5.1.5 | Implement `force_save()` for critical state | Todo | - |
| T5.1.6 | Write unit tests for coordinator | Todo | - |

#### Technical Notes

```rust
pub enum PersistenceOp {
    SaveMeta { agent_id: AgentId, meta: AgentMeta },
    SaveTranscript { agent_id: AgentId, transcript: AgentTranscript },
    SaveState { agent_id: AgentId, state: AgentState },
    SaveMessages { agent_id: AgentId, messages: AgentMessages },
}
```

---

### Story 5.2: Periodic Flush in Event Loop

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Add periodic persistence flush to TUI event loop.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.2.1 | Add flush interval config (default 5s) | Todo | - |
| T5.2.2 | Track last flush timestamp | Todo | - |
| T5.2.3 | Check interval in event loop tick | Todo | - |
| T5.2.4 | Call coordinator flush on interval | Todo | - |
| T5.2.5 | Write tests for periodic flush | Todo | - |

#### Acceptance Criteria

- Flush happens every N seconds
- No blocking on flush
- All queued ops persisted

---

### Story 5.3: Per-Agent Directory Isolation

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Ensure each agent has isolated storage directory.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.3.1 | Create agent directory structure | Todo | - |
| T5.3.2 | Ensure meta.json per agent | Todo | - |
| T5.3.3 | Ensure state.json per agent | Todo | - |
| T5.3.4 | Ensure transcript.json per agent | Todo | - |
| T5.3.5 | Ensure messages.json per agent | Todo | - |
| T5.3.6 | Write tests for file isolation | Todo | - |

#### Directory Structure

```
workplace/
├── meta.json
├── backlog.json
├── agents/
│   ├── agent_001/
│   │   ├── meta.json
│   │   ├── state.json
│   │   ├── transcript.json
│   │   ├── messages.json
│   │   └── memory.json
│   ├── agent_002/
│   │   └── ...
│   └── agent_003/
│       └── ...
```

---

### Story 5.4: Restore All Agents on Bootstrap

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Restore all agent states on workplace bootstrap.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.4.1 | Scan agents/ directory on bootstrap | Todo | - |
| T5.4.2 | Load meta.json for each agent | Todo | - |
| T5.4.3 | Load state.json for each agent | Todo | - |
| T5.4.4 | Load transcript.json for each agent | Todo | - |
| T5.4.5 | Create AgentSlot from loaded data | Todo | - |
| T5.4.6 | Restore session_handle if available | Todo | - |
| T5.4.7 | Write tests for full restore | Todo | - |
| T5.4.8 | Write tests for partial restore (missing files) | Todo | - |

#### Acceptance Criteria

- All agents restore on restart
- Transcript history preserved
- Session continuity preserved

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Concurrent file writes | Medium | High | Per-agent directories, serialized writes per agent |
| Missing files on restore | Medium | Medium | Graceful defaults for missing files |

## Sprint Deliverables

- `core/src/persistence_coordinator.rs`
- Modified event loop with periodic flush
- Per-agent directory structure
- Bootstrap restore logic

## Dependencies

- Sprint 1: AgentSlot structure
- Sprint 2: Thread management
- Sprint 3: Task assignment state

## Next Sprint

After completing this sprint, proceed to [Sprint 6: Graceful Shutdown](./sprint-06-shutdown-restore.md).