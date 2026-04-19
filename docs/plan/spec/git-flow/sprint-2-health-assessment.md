# Sprint 2: Workspace Health Assessment

## Sprint Overview

**Duration**: 1-2 days
**Goal**: Implement Git workspace health assessment functionality
**Priority**: P0 (Critical)

## Stories

### Story 2.1: Uncommitted Changes Detection

**Description**: Enhance WorktreeManager to detect and classify uncommitted changes.

**Acceptance Criteria**:
- [ ] Function `get_uncommitted_changes_details(worktree_path)` returns detailed info
- [ ] Classifies changes: staged, unstaged, untracked
- [ ] Identifies change categories: code, config, debug artifacts
- [ ] Returns list of affected files with change type
- [ ] Unit tests for various change scenarios

**Implementation Notes**:
```rust
pub struct UncommittedChangesInfo {
    pub has_staged: bool,
    pub has_unstaged: bool,
    pub has_untracked: bool,
    pub staged_files: Vec<FileChange>,
    pub unstaged_files: Vec<FileChange>,
    pub untracked_files: Vec<PathBuf>,
    pub change_category: ChangeCategory,
}

pub enum ChangeCategory {
    CodeChanges,
    ConfigChanges,
    DebugArtifacts,
    Mixed,
    None,
}

pub struct FileChange {
    pub path: PathBuf,
    pub change_type: ChangeType, // Added, Modified, Deleted, Renamed
}
```

### Story 2.2: Branch Status Assessment

**Description**: Assess current branch status relative to main/master.

**Acceptance Criteria**:
- [ ] Function `assess_branch_status(worktree_path)` 
- [ ] Determines: ahead/behind main, merged status, divergence count
- [ ] Identifies if branch is stale (many commits behind)
- [ ] Returns status summary with recommendations
- [ ] Unit tests for various branch states

**Implementation Notes**:
```rust
pub struct BranchStatusInfo {
    pub branch_name: String,
    pub is_merged_to_main: bool,
    pub commits_ahead_main: usize,
    pub commits_behind_main: usize,
    pub last_commit_date: Option<DateTime<Utc>>,
    pub status: BranchHealthStatus,
    pub recommendation: BranchRecommendation,
}

pub enum BranchHealthStatus {
    Healthy,       // At main HEAD or slightly ahead
    NeedsRebase,   // Behind main, needs sync
    Stale,         // Significantly behind main
    Merged,        // Already merged to main
    Orphaned,      // No common ancestor with main
}

pub enum BranchRecommendation {
    Continue,      // Ready to use
    RebaseToMain,  // Should rebase onto main
    Recreate,      // Consider recreating from main
    Cleanup,       // Safe to delete (merged)
    Investigate,   // Needs manual review
}
```

### Story 2.3: Workspace Health Score

**Description**: Calculate overall workspace health score.

**Acceptance Criteria**:
- [ ] Function `calculate_workspace_health(worktree_path)` 
- [ ] Returns score from 0-100
- [ ] Factors: uncommitted changes, branch status, worktree validity
- [ ] Provides detailed health report
- [ ] Unit tests for scoring logic

**Implementation Notes**:
```rust
pub struct WorkspaceHealthReport {
    pub score: u8,  // 0-100
    pub is_ready_for_task: bool,
    pub issues: Vec<HealthIssue>,
    pub warnings: Vec<HealthWarning>,
    pub recommendations: Vec<String>,
}

pub struct HealthIssue {
    pub severity: IssueSeverity, // Critical, Warning, Info
    pub category: IssueCategory,
    pub description: String,
    pub suggested_action: String,
}
```

### Story 2.4: Worktree Validation

**Description**: Validate worktree state before task start.

**Acceptance Criteria**:
- [ ] Function `validate_worktree_state(worktree_path)` 
- [ ] Checks: directory exists, .git link valid, HEAD valid
- [ ] Detects corrupted worktree state
- [ ] Returns validation result with issues
- [ ] Unit tests for various worktree states

**Implementation Notes**:
```rust
pub struct WorktreeValidationResult {
    pub is_valid: bool,
    pub issues: Vec<WorktreeIssue>,
    pub can_be_repaired: bool,
}

pub enum WorktreeIssue {
    DirectoryMissing,
    GitLinkBroken,
    HeadCorrupted,
    BranchNotFound,
    LockConflict,
}
```

## Dependencies

- Sprint 1 (GitFlowConfig, TaskMetadata)
- Existing WorktreeManager

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Git command failures | Graceful error handling |
| Large repo performance | Limit file scan depth |
| Concurrent modifications | Lock mechanisms |

## Testing Strategy

- Unit tests with mock git commands
- Integration tests with real git repo
- Performance tests with large repos

## Deliverables

1. Enhanced `core/src/worktree_manager.rs`
2. New `core/src/workspace_health.rs` module
3. Unit test coverage > 80%

---

**Sprint Status**: Planned
