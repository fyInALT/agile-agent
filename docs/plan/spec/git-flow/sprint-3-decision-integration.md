# Sprint 3: Decision Layer Integration

## Sprint Goal
Integrate Git Flow preparation into the decision layer as a new situation type with corresponding actions.

## Duration
3-4 days

## Stories

### Story 3.1: Create TaskPreparationSituation
**Description**: Define new situation type for task preparation phase.

**Tasks**:
- [ ] Create `TaskPreparationSituation` struct in `decision/src/builtin_situations.rs`
- [ ] Define situation fields: task_id, description, current_branch, has_uncommitted
- [ ] Implement `DecisionSituation` trait
- [ ] Register in situation registry

**Acceptance Criteria**:
- Situation properly implements trait
- Contains all necessary context for Git operations
- Registered and discoverable

### Story 3.2: Create Git Preparation Actions
**Description**: Implement decision actions for Git operations.

**Tasks**:
- [ ] Create `PrepareBranchAction` - execute full preparation workflow
- [ ] Create `StashAndCreateBranchAction` - handle uncommitted + create branch
- [ ] Create `RebaseBranchAction` - rebase existing branch
- [ ] Create `ForceCreateBranchAction` - recreate stale branch
- [ ] Register all actions

**Acceptance Criteria**:
- Actions implement `DecisionAction` trait
- Each action has clear purpose and outcome
- Actions are composable

### Story 3.3: Create TaskPreparationClassifier
**Description**: Classifier to detect when task preparation is needed.

**Tasks**:
- [ ] Create classifier that triggers on task assignment
- [ ] Detect when agent is idle with new task but no proper branch
- [ ] Check if current branch matches task metadata
- [ ] Register classifier

**Acceptance Criteria**:
- Triggers when new task assigned without preparation
- Checks branch naming compliance
- Returns `NeedsDecision` with TaskPreparationSituation

### Story 3.4: Implement Preparation Workflow Execution
**Description**: Execute the full preparation workflow in AgentPool.

**Tasks**:
- [ ] Add `execute_task_preparation()` in `agent_pool.rs`
- [ ] Call Git operations in sequence
- [ ] Handle preparation failures gracefully
- [ ] Update agent state after preparation

**Acceptance Criteria**:
- Full workflow executed atomically
- Agent status updated appropriately
- Errors logged and handled

### Story 3.5: Add Preparation Logging
**Description**: Comprehensive logging of preparation events.

**Tasks**:
- [ ] Log `task_preparation.started`
- [ ] Log `task_preparation.branch_created`
- [ ] Log `task_preparation.sync_completed`
- [ ] Log `task_preparation.completed` or `task_preparation.failed`

**Acceptance Criteria**:
- All preparation steps logged
- Logs include relevant context (branch, task_id, agent)
- Logs viewable in TUI/logs

## Technical Notes

- Follow existing decision layer patterns
- Keep preparation logic in decision layer, Git ops in core
- Ensure thread-safe operations

## Dependencies

- Sprint 2 (Git Operations)

## Risks

- Integration complexity with existing flow
- Race conditions in multi-agent scenarios

---

**Sprint Status**: Pending Sprint 2 Completion
