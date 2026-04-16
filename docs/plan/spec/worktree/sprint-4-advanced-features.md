# Sprint 4: Advanced Features

## Metadata

- Sprint ID: `worktree-sprint-04`
- Title: `Advanced Features`
- Duration: 2 weeks
- Priority: P1
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: [Sprint 3: TUI Display](./sprint-3-tui-display.md)
- Design Reference: `docs/plan/worktree/worktree-integration-research.md`

## Sprint Goal

Complete branch management with completion policies, implement auto cleanup for idle worktrees, add PR creation support, and implement crash recovery to detect and handle orphaned worktree states at startup.

## Stories

### Story 4.1: Branch Completion Policy

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement handling of task completion with branch management policies.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Define `BranchCompletionPolicy` enum | Todo | - |
| T4.1.2 | Implement `KeepAndCreatePR` policy | Todo | - |
| T4.1.3 | Implement `KeepForManualReview` policy | Todo | - |
| T4.1.4 | Implement `AutoMerge` policy with target branch | Todo | - |
| T4.1.5 | Implement `Delete` policy for experimental tasks | Todo | - |
| T4.1.6 | Add `complete_task()` method to WorktreeManager | Todo | - |
| T4.1.7 | Integrate with task completion flow in AgentPool | Todo | - |
| T4.1.8 | Write unit tests for all policies | Todo | - |

#### Technical Design

```rust
/// Branch handling options after task completion
pub enum BranchCompletionPolicy {
    /// Keep branch and create PR
    KeepAndCreatePR,
    /// Keep branch for manual review (remove worktree only)
    KeepForManualReview,
    /// Auto merge to target branch (low-risk tasks only)
    AutoMerge { target: String },
    /// Delete branch entirely (experimental tasks)
    Delete,
}

impl WorktreeManager {
    pub fn complete_task(
        &self,
        worktree_name: &str,
        policy: BranchCompletionPolicy,
    ) -> Result<TaskCompletionResult, WorktreeError> {
        match policy {
            BranchCompletionPolicy::KeepAndCreatePR => {
                // Keep branch, trigger PR creation workflow
            }
            BranchCompletionPolicy::KeepForManualReview => {
                self.remove(worktree_name)?;
            }
            BranchCompletionPolicy::AutoMerge { target } => {
                // Checkout target, merge branch, cleanup
            }
            BranchCompletionPolicy::Delete => {
                self.remove(worktree_name)?;
                // Delete branch
            }
        }
    }
}
```

---

### Story 4.2: PR Creation Integration

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Support automatic PR creation after task completion.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Implement `create_pr()` method using gh CLI | Todo | - |
| T4.2.2 | Generate PR title from task description | Todo | - |
| T4.2.3 | Generate PR body template with agent info | Todo | - |
| T4.2.4 | Handle gh CLI not installed gracefully | Todo | - |
| T4.2.5 | Handle PR creation failure with fallback | Todo | - |
| T4.2.6 | Store PR URL in task completion result | Todo | - |
| T4.2.7 | Write unit tests for PR creation | Todo | - |

#### PR Template

```markdown
## Summary
{task_description}

## Changes
- {list of commits from agent}

## Test Plan
{test_plan from task}

---
🤖 Created by agent {agent_codename} ({agent_id})
```

---

### Story 4.3: Idle Worktree Auto Cleanup

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement automatic cleanup of idle worktrees based on timeout.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Add `idle_timeout_secs` to WorktreeConfig | Todo | - |
| T4.3.2 | Track last activity timestamp per worktree | Todo | - |
| T4.3.3 | Implement `cleanup_idle_worktrees()` method | Todo | - |
| T4.3.4 | Check idle worktrees on periodic timer | Todo | - |
| T4.3.5 | Warn before cleanup (TUI notification) | Todo | - |
| T4.3.6 | Preserve worktrees with uncommitted changes | Todo | - |
| T4.3.7 | Update worktree state before cleanup | Todo | - |
| T4.3.8 | Write unit tests for idle cleanup | Todo | - |

#### Configuration

```rust
pub struct WorktreeConfig {
    pub max_worktrees: usize,
    pub prefix: String,
    pub default_base_branch: String,
    pub auto_cleanup: bool,
    pub idle_timeout_secs: u64,  // Default: 3600 (1 hour)
}
```

---

### Story 4.4: Branch Merge Flow

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Implement branch merge workflow for AutoMerge policy.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.4.1 | Implement `merge_branch()` method | Todo | - |
| T4.4.2 | Handle merge conflicts gracefully | Todo | - |
| T4.4.3 | Validate merge safety (no conflicts detected) | Todo | - |
| T4.4.4 | Update worktree state after merge | Todo | - |
| T4.4.5 | Delete branch after successful merge | Todo | - |
| T4.4.6 | Notify user of merge result | Todo | - |
| T4.4.7 | Write unit tests for merge flow | Todo | - |

#### Merge Safety Checks

1. Check for uncommitted changes in target branch
2. Verify no conflicts between branches
3. Run fast-forward merge if possible
4. Create merge commit if needed
5. Verify merge success before cleanup

---

### Story 4.5: Crash Recovery - Orphan Detection

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Detect and handle orphaned worktree states at system startup.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.5.1 | Implement `detect_orphaned_worktrees()` at startup | Todo | - |
| T4.5.2 | Compare persisted states with actual worktrees | Todo | - |
| T4.5.3 | Identify worktree states with missing directories | Todo | - |
| T4.5.4 | Identify worktrees not in persisted states | Todo | - |
| T4.5.5 | Implement recovery options (recreate, cleanup, manual) | Todo | - |
| T4.5.6 | Add startup hook for orphan detection | Todo | - |
| T4.5.7 | Log orphan detection results | Todo | - |
| T4.5.8 | Write unit tests for orphan detection | Todo | - |
| T4.5.9 | Write integration tests for crash scenarios | Todo | - |

#### Detection Flow

```
System Startup
      │
      ▼
Load all worktree states from .state/agents/
      │
      ▼
List all actual worktrees with git worktree list
      │
      ▼
Compare and categorize:
      │
      ├─ Orphaned States: state exists, worktree missing
      │     → Option: recreate or delete state
      │
      ├─ Unknown Worktrees: worktree exists, no state
      │     → Option: create state or remove worktree
      │
      └─ Healthy: both exist and match
      │     → No action needed
      │
      ▼
Report to user / auto-recover based on config
```

---

### Story 4.6: Crash Recovery - Auto Recreation

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Automatically recreate missing worktrees from persisted state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.6.1 | Implement `auto_recreate_orphaned()` | Todo | - |
| T4.6.2 | Check if branch still exists | Todo | - |
| T4.6.3 | Recreate from base_commit if branch lost | Todo | - |
| T4.6.4 | Cherry-pick commits if they exist elsewhere | Todo | - |
| T4.6.5 | Update state with new worktree info | Todo | - |
| T4.6.6 | Add recovery logging | Todo | - |
| T4.6.7 | Write unit tests for auto recreation | Todo | - |

#### Recreation Priority

1. If branch exists: create worktree on existing branch
2. If branch missing but commits reachable: recreate branch and worktree
3. If everything lost: recreate from base_commit, mark as "recovery needed"

---

### Story 4.7: Resource Limits and Validation

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement resource limits and validation for worktree operations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.7.1 | Implement max_worktrees limit check | Todo | - |
| T4.7.2 | Add disk space check before creation | Todo | - |
| T4.7.3 | Validate branch name format | Todo | - |
| T4.7.4 | Prevent duplicate worktree names | Todo | - |
| T4.7.5 | Add warning when approaching limits | Todo | - |
| T4.7.6 | Write unit tests for validation | Todo | - |

#### Limits

```rust
pub const MAX_WORKTREES: usize = 10;
pub const MIN_DISK_SPACE_MB: u64 = 100;
pub const BRANCH_NAME_PATTERN: &str = "^[a-zA-Z0-9/_-]+$";
```

---

### Story 4.8: Worktree Repair Function

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Implement repair function for corrupted worktrees.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.8.1 | Implement `repair_worktree()` method | Todo | - |
| T4.8.2 | Detect corrupted .git file | Todo | - |
| T4.8.3 | Repair broken worktree links | Todo | - |
| T4.8.4 | Run git worktree prune | Todo | - |
| T4.8.5 | Handle partial corruption | Todo | - |
| T4.8.6 | Add repair command to CLI | Todo | - |
| T4.8.7 | Write unit tests for repair | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Crash recovery complexity | Medium | High | Incremental testing, clear logging |
| Merge conflicts in AutoMerge | Medium | Medium | Safety checks, fallback to manual review |
| PR creation failures | Low | Low | Fallback to manual PR, graceful handling |
| Disk space exhaustion | Low | High | Pre-creation check, auto cleanup |

## Sprint Deliverables

- BranchCompletionPolicy implementation
- PR creation support (gh CLI integration)
- Idle worktree auto cleanup
- Crash recovery with orphan detection
- Auto recreation of missing worktrees
- Resource limits and validation
- Worktree repair function

## Dependencies

- Sprint 3: TUI Display (for user notifications)
- GitHub CLI (gh) for PR creation
- Git >= 2.17 for all worktree operations

## Module Structure

```
core/src/
├── worktree_manager.rs      # Extended with completion, merge, repair
├── worktree_state.rs        # Extended with recovery fields
├── worktree_state_store.rs  # Extended with orphan detection
├── worktree_config.rs       # NEW - configuration and limits
└── crash_recovery.rs        # NEW - startup recovery logic
```

## CLI Commands

```bash
# Repair worktrees
agent worktree repair

# List orphaned states
agent worktree list --orphans

# Cleanup idle worktrees
agent worktree cleanup --idle

# Complete task with policy
agent complete <agent-id> --policy=keep-pr
agent complete <agent-id> --policy=delete
```

## Completion

After completing this sprint, the worktree integration feature is fully implemented. Proceed to integration testing and documentation.