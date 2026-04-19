# Sprint 2: Git Operations for Task Preparation

## Sprint Goal
Implement core Git operations needed for task preparation: baseline sync, branch creation, and uncommitted changes handling.

## Duration
3-4 days

## Stories

### Story 2.1: Enhance WorktreeManager with Git Flow Operations
**Description**: Add Git Flow specific operations to WorktreeManager.

**Tasks**:
- [ ] Add `fetch_origin()` method
- [ ] Add `sync_base_branch()` method  
- [ ] Add `get_remote_head()` method
- [ ] Add `is_branch_synced()` method
- [ ] Add unit tests for each method

**Acceptance Criteria**:
- Can fetch from origin
- Can detect if local is behind remote
- Can get latest commit SHA from remote tracking

### Story 2.2: Implement Uncommitted Changes Detection
**Description**: Detect and classify uncommitted changes in worktree.

**Tasks**:
- [ ] Enhance `has_uncommitted_changes()` to return detailed info
- [ ] Create `UncommittedChangesInfo` struct with file list
- [ ] Classify changes as: source code, config, artifacts, sensitive
- [ ] Add tests

**Acceptance Criteria**:
- Returns list of uncommitted files with status (staged/unstaged)
- Can classify change types
- Handles binary files

### Story 2.3: Implement Stash Operations
**Description**: Add stash functionality for preserving uncommitted work.

**Tasks**:
- [ ] Add `stash_changes()` with descriptive message
- [ ] Add `stash_list()` to see existing stashes
- [ ] Add `stash_pop()` for restoration
- [ ] Add tests

**Acceptance Criteria**:
- Can stash with task-related message
- Can list and identify stashes
- Can restore stashed changes

### Story 2.4: Implement Branch Creation from Base
**Description**: Create feature branch from latest base branch.

**Tasks**:
- [ ] Add `create_feature_branch()` method
- [ ] Ensure branch starts from fetched remote HEAD
- [ ] Validate branch doesn't exist elsewhere
- [ ] Handle branch name conflicts
- [ ] Add tests

**Acceptance Criteria**:
- Branch created from origin/main HEAD
- No collision with existing branches
- Proper error handling for conflicts

### Story 2.5: Implement Rebase to Base
**Description**: Rebase existing branch to latest base if needed.

**Tasks**:
- [ ] Add `rebase_to_base()` method
- [ ] Handle rebase conflicts gracefully
- [ ] Abort and rollback on failure
- [ ] Add tests

**Acceptance Criteria**:
- Can rebase to origin/main
- Returns conflict info if conflicts occur
- Safe rollback on failure

## Technical Notes

- Use existing `run_git_command()` infrastructure
- All operations should be atomic where possible
- Log all Git operations for audit trail

## Dependencies

- Sprint 1 (Task Metadata Extraction)

## Risks

- Git operations may fail in various environments
- Rebase conflicts need careful handling

---

**Sprint Status**: Pending Sprint 1 Completion
