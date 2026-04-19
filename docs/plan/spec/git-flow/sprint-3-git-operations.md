# Sprint 3: Git Flow Operations

## Sprint Overview

**Duration**: 2-3 days  
**Goal**: Implement Git Flow operations for task preparation
**Priority**: P0 (Critical)

## Stories

### Story 3.1: Baseline Sync Operation

**Description**: Implement operation to sync local main/master to remote.

**Acceptance Criteria**:
- [ ] Function `sync_baseline(config)` in WorktreeManager
- [ ] Fetches from origin
- [ ] Updates local main/master tracking
- [ ] Returns latest commit SHA
- [ ] Handles network failures gracefully
- [ ] Unit tests

**Implementation Notes**:
```rust
pub struct BaselineSyncResult {
    pub success: bool,
    pub base_branch: String,
    pub base_commit: String,
    pub fetched_commits: usize,
    pub error: Option<String>,
}

impl WorktreeManager {
    pub fn sync_baseline(&self, config: &GitFlowConfig) -> Result<BaselineSyncResult, WorktreeError> {
        // git fetch origin
        // git rev-parse origin/main (or master)
        // return base commit
    }
}
```

### Story 3.2: Feature Branch Creation

**Description**: Create feature branch from latest baseline.

**Acceptance Criteria**:
- [ ] Function `create_feature_branch(metadata, config)` 
- [ ] Creates branch with Git Flow naming
- [ ] Based on latest main/master HEAD
- [ ] Handles existing branch scenarios
- [ ] Validates branch name before creation
- [ ] Unit tests

**Creation Scenarios**:
```
1. New branch (doesn't exist):
   - Create from base commit
   - Return success

2. Existing branch at main HEAD:
   - Reuse existing branch
   - Return success (existing)

3. Existing branch behind main:
   - Option: rebase or recreate
   - Return decision needed

4. Existing branch in other worktree:
   - Cannot checkout (locked)
   - Return error
```

**Implementation Notes**:
```rust
pub struct FeatureBranchResult {
    pub success: bool,
    pub branch_name: String,
    pub base_commit: String,
    pub is_existing: bool,
    pub action_taken: BranchCreationAction,
}

pub enum BranchCreationAction {
    CreatedNew,
    ReusedExisting,
    RebasedExisting,
    RecreatedFromMain,
    FailedWithConflict,
}
```

### Story 3.3: Uncommitted Changes Handling

**Description**: Handle uncommitted changes before task start.

**Acceptance Criteria**:
- [ ] Function `handle_uncommitted(worktree_path, config, policy)` 
- [ ] Implements decision tree for handling
- [ ] Options: commit_wip, stash, discard_artifacts, prompt
- [ ] Returns action taken
- [ ] Safety: never loses code changes
- [ ] Unit tests

**Handling Policy**:
```rust
pub enum UncommittedHandlingPolicy {
    AutoStash,           // Stash all changes automatically
    AutoCommitWip,       // Commit with "WIP: <task-id>" message
    DiscardArtifacts,    // Remove debug files only
    PromptForDecision,   // Ask decision layer
    FailWithError,       // Require manual intervention
}

pub struct UncommittedHandlingResult {
    pub success: bool,
    pub action_taken: UncommittedAction,
    pub stashed_id: Option<String>,  // If stashed
    pub commit_sha: Option<String>,  // If committed
    pub preserved_files: Vec<PathBuf>,
}

pub enum UncommittedAction {
    StashedChanges,
    CommittedWip,
    DiscardedArtifacts,
    NoActionNeeded,
    PromptedUser,
}
```

### Story 3.4: Branch Rebase Operation

**Description**: Rebase existing branch onto latest main.

**Acceptance Criteria**:
- [ ] Function `rebase_to_baseline(worktree_path, config)` 
- [ ] Rebases current branch onto main/master
- [ ] Handles conflict detection
- [ ] Returns rebase result
- [ ] Rollback on failure
- [ ] Unit tests

**Implementation Notes**:
```rust
pub struct RebaseResult {
    pub success: bool,
    pub conflicts_detected: bool,
    pub conflict_files: Vec<PathBuf>,
    pub commits_rebased: usize,
    pub original_head: String,
    pub new_head: String,
}

impl WorktreeManager {
    pub fn rebase_to_baseline(&self, worktree_path: &Path) -> Result<RebaseResult, WorktreeError> {
        // Store original HEAD for rollback
        // git rebase origin/main
        // Check for conflicts
        // Return result
    }
    
    pub fn abort_rebase(&self, worktree_path: &Path) -> Result<(), WorktreeError> {
        // git rebase --abort
    }
}
```

### Story 3.5: Git Operations Executor

**Description**: Unified executor for Git Flow operations.

**Acceptance Criteria**:
- [ ] `GitFlowExecutor` struct orchestrates operations
- [ ] Executes preparation workflow step by step
- [ ] Handles errors and recovery
- [ ] Logs all operations
- [ ] Returns comprehensive result

**Implementation Notes**:
```rust
pub struct GitFlowExecutor {
    worktree_manager: WorktreeManager,
    config: GitFlowConfig,
}

pub struct PreparationResult {
    pub success: bool,
    pub metadata: TaskMetadata,
    pub branch_name: String,
    pub base_commit: String,
    pub worktree_path: PathBuf,
    pub warnings: Vec<String>,
    pub operations_log: Vec<GitOperationLog>,
}

impl GitFlowExecutor {
    pub fn prepare_for_task(&self, task_id: &str, description: &str) -> Result<PreparationResult, GitFlowError> {
        // 1. Extract metadata
        // 2. Sync baseline
        // 3. Check workspace health
        // 4. Handle uncommitted (if needed)
        // 5. Create/rebase branch
        // 6. Return result
    }
}
```

## Dependencies

- Sprint 1 (GitFlowConfig, TaskMetadata)
- Sprint 2 (WorkspaceHealth)
- Existing WorktreeManager

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Rebase conflicts | Detect before rebase, rollback |
| Network failures | Retry with timeout |
| Concurrent branches | Lock mechanism |
| Data loss | Never destructive without backup |

## Testing Strategy

- Unit tests with mock git operations
- Integration tests with real git scenarios
- Conflict simulation tests
- Recovery tests

## Deliverables

1. New `core/src/git_flow_executor.rs`
2. Enhanced `core/src/worktree_manager.rs` with new operations
3. Unit tests > 80% coverage

---

**Sprint Status**: Planned
