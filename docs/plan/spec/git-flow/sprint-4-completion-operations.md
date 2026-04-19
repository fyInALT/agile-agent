# Sprint 4: Task Completion Git Operations

## Sprint Goal
Implement Git operations for proper task completion: commit enforcement, branch push, and cleanup.

## Duration
2-3 days

## Stories

### Story 4.1: Create TaskCompletionSituation
**Description**: Define situation for task completion Git phase.

**Tasks**:
- [ ] Create `TaskCompletionSituation` struct
- [ ] Fields: task_id, branch_name, uncommitted_files, commit_count
- [ ] Implement `DecisionSituation` trait
- [ ] Register in situation registry

**Acceptance Criteria**:
- Situation captures completion context
- Lists uncommitted files if any
- Available actions clearly defined

### Story 4.2: Create Completion Actions
**Description**: Actions for proper task completion.

**Tasks**:
- [ ] Create `CommitAllAction` - commit all changes with proper message
- [ ] Create `PushBranchAction` - push to origin
- [ ] Create `FinalizeTaskAction` - mark complete, optionally cleanup
- [ ] Register actions

**Acceptance Criteria**:
- Commit action generates conventional commit message
- Push action handles authentication errors
- Finalize action updates task status

### Story 4.3: Implement Conventional Commit Generation
**Description**: Generate proper commit messages from task metadata.

**Tasks**:
- [ ] Create `CommitMessageGenerator` in `decision/src/git_operations.rs`
- [ ] Use task metadata for type, scope, subject
- [ ] Include Co-authored-by footer
- [ ] Add tests

**Acceptance Criteria**:
- Messages follow conventional commits format
- Include task reference
- Include agent attribution

### Story 4.4: Implement Branch Cleanup
**Description**: Clean up branches after task merge/completion.

**Tasks**:
- [ ] Add `delete_merged_branch()` to WorktreeManager
- [ ] Add `cleanup_worktree()` for worktree removal
- [ ] Safety checks before deletion
- [ ] Add tests

**Acceptance Criteria**:
- Only delete merged branches
- Preserve unmerged work
- Log all cleanup operations

## Technical Notes

- Completion should be idempotent (safe to retry)
- Commit messages should be reviewable before commit
- Cleanup should be optional/configurable

## Dependencies

- Sprint 3 (Decision Integration)

## Risks

- Push failures due to permissions
- Accidental branch deletion

---

**Sprint Status**: Pending Sprint 3 Completion
