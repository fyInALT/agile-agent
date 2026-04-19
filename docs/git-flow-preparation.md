# Git Flow Task Preparation Architecture

This document describes the architecture for Git Flow task preparation in agile-agent.

## Overview

Git Flow task preparation ensures work agents follow proper Git workflow conventions when starting tasks. The system:

- Automatically extracts task metadata (branch name, type, summary)
- Analyzes current Git state before task assignment
- Handles uncommitted changes appropriately
- Syncs with baseline branch and creates feature branches
- Provides graceful handling of conflicts and edge cases

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Git Flow Task Preparation                              │
└─────────────────────────────────────────────────────────────────────────────┘

         Task Assignment
                │
                ▼
    ┌───────────────────────┐
    │ TaskStartingSituation  │  (decision/src/builtin_situations.rs)
    │                        │
    │ - task_description     │
    │ - task_id              │
    │ - git_state            │
    │ - task_meta            │
    └───────────────────────┘
                │
                ▼
    ┌───────────────────────┐
    │ TaskPreparationPipeline│  (decision/src/task_preparation.rs)
    │                        │
    │ 1. Extract Metadata    │────► TaskMetadata
    │ 2. Analyze Git State   │────► GitState
    │ 3. Handle Uncommitted  │────► UncommittedAnalysis
    │ 4. Setup Branch        │────► PreparationResult
    └───────────────────────┘
                │
                ├────► Ready: Branch ready, agent starts task
                ├────► NeedsUncommitted: Handle changes first
                ├────► NeedsSync: Rebase required
                ├────► NeedsHuman: Block for user input
                └────► Failed: Report error
                │
                ▼
    ┌───────────────────────┐
    │ PrepareTaskStartAction │  (decision/src/builtin_actions.rs)
    │                        │
    │ - task_meta            │
    │ - pre_actions[]        │
    │ - worktree_path        │
    └───────────────────────┘
                │
                ▼
    ┌───────────────────────┐
    │ GitFlowExecutor       │  (core/src/git_flow_executor.rs)
    │                        │
    │ - check_health()       │
    │ - prepare_for_task()   │
    │ - sync_baseline()      │
    │ - setup_branch()       │
    └───────────────────────┘
```

## Components

### 1. TaskMetadata Extraction

Located in `decision/src/task_metadata.rs`.

Extracts structured metadata from task description:

| Field | Description | Source |
|-------|-------------|--------|
| `task_id` | Task identifier | Backlog or generated |
| `branch_name` | Generated branch name | Pattern-based |
| `task_type` | Feature/Bugfix/Refactor/etc | Keyword classification |
| `summary` | Short description | First N words |
| `priority` | Task priority | Default or extracted |

Branch naming pattern: `<type>/<task-id>-<desc>`

Example:
- Task: "Add user authentication feature"
- Branch: `feature/task-001-add-user-auth`

### 2. GitState Analysis

Located in `decision/src/git_state.rs`.

Analyzes current workspace Git state:

| Field | Description |
|-------|-------------|
| `current_branch` | Current checked out branch |
| `has_uncommitted` | Boolean flag for changes |
| `uncommitted_files` | List of modified files |
| `has_conflicts` | Boolean flag for merge/rebase conflicts |
| `commits_ahead` | Commits ahead of base |
| `commits_behind` | Commits behind base |

### 3. Uncommitted Changes Handler

Located in `decision/src/uncommitted_handler.rs`.

Classifies and handles uncommitted changes:

| Classification | Action |
|----------------|--------|
| CurrentTask | Commit with WIP prefix |
| PreviousTask | Commit with chore prefix |
| Temporary | Discard or stash |
| Unknown | Request human decision |
| Valuable | Commit with appropriate prefix |
| Low Value | Stash or discard |

### 4. Git Flow Executor

Located in `core/src/git_flow_executor.rs`.

Executes Git operations:

| Operation | Description |
|-----------|-------------|
| `prepare_for_task()` | Full preparation workflow |
| `check_health()` | Workspace health assessment |
| `sync_baseline()` | Fetch and sync with origin |
| `setup_branch()` | Create or checkout branch |
| `handle_uncommitted_stash()` | Auto-stash changes |

### 5. Git Flow Configuration

Located in `core/src/git_flow_config.rs`.

Configuration options:

```rust
GitFlowConfig {
    base_branch: "main",              // Base branch (main/master)
    branch_pattern: "<type>/<task-id>-<desc>", // Naming pattern
    auto_sync_baseline: true,         // Auto fetch from origin
    auto_stash_changes: false,        // Auto stash uncommitted
    auto_cleanup_merged: true,        // Cleanup merged branches
    stale_branch_days: 30,            // Stale branch threshold
    enforce_conventional_commits: true, // Enforce commit format
    max_desc_length: 30,              // Max branch description length
}
```

## Task Types and Branch Prefixes

| Type | Prefix | Keywords |
|------|--------|----------|
| Feature | `feature/` | add, implement, create, new, introduce |
| Bugfix | `fix/` | fix, bug, issue, error, resolve |
| Refactor | `refactor/` | refactor, simplify, optimize, clean |
| Docs | `docs/` | document, readme, doc, documentation |
| Test | `test/` | test, testing, spec, coverage |
| Chore | `chore/` | chore, maintenance, cleanup, update |
| Hotfix | `hotfix/` | hotfix, urgent, critical, emergency |

## Preparation Results

| Result | Description | Agent Action |
|--------|-------------|--------------|
| `Ready` | All checks passed, branch ready | Start task immediately |
| `NeedsUncommittedHandling` | Uncommitted changes detected | Execute handling action first |
| `NeedsSync` | Branch behind base or needs rebase | Sync/rebase before task |
| `NeedsHuman` | Requires user intervention | Block agent, show prompt |
| `Failed` | Preparation error occurred | Report error, don't start task |

## Workspace Health Assessment

Health score (0-100) determines readiness:

| Score | Status | Action |
|-------|--------|--------|
| 100 | Healthy | Proceed |
| 80-99 | Minor issues | Proceed with warnings |
| 50-79 | Moderate issues | May block or warn |
| <50 | Critical issues | Block agent |

Health issues categories:

- `UncommittedChanges`: Modified files in working tree
- `BranchStatus`: Branch diverged or behind base
- `WorktreeState`: Worktree issues
- `ConflictState`: Merge/rebase conflicts
- `NetworkIssue`: Fetch/push problems

## Error Handling

| Error | Recovery |
|-------|----------|
| `HealthCheckFailed` | Block agent, report issues |
| `UncommittedChangesNeedHandling` | Stash if auto_stash enabled |
| `BranchCollision` | Auto-suffix branch name |
| `RebaseConflicts` | Block for human resolution |
| `PreparationTimeout` | Use degraded mode |
| `InvalidWorktreePath` | Report error, don't start |

## Integration with Decision Layer

Task preparation integrates with the decision layer via:

1. **TaskStartingSituation**: Sent to decision agent when task assigned
2. **PrepareTaskStartAction**: Executed by agent_pool on decision
3. **AgentPool.trigger_task_preparation()**: Trigger preparation flow
4. **AgentPool.execute_decision_action()**: Handle `prepare_task_start` action

## Usage

Preparation is automatically triggered when:

```rust
// In agent_pool.rs assign_task flow
if let Err(e) = self.trigger_task_preparation(
    &agent_id,
    &request.task_id,
    &request.task_description,
) {
    // Handle preparation failure
}
```

Manual trigger via decision request:

```rust
let situation = TaskStartingSituation::new("Add user authentication")
    .with_task_id("PROJ-123")
    .with_git_state(git_state);

let request = DecisionRequest::new(
    agent_id,
    task_starting(),
    DecisionContext::new(Box::new(situation), agent_id.as_str()),
);
```

## Testing

Run tests:

```bash
# Decision crate tests (includes task preparation)
cargo test -p agent-decision --lib

# Core crate tests (includes git_flow_executor)
cargo test -p agent-core --lib

# Full workspace
cargo test --workspace
```

## Related Documentation

- [Decision Layer Flow](decision-layer-flow.md)
- [Git Flow Task Preparation Requirements](plan/git-flow-task-preparation-requirements.md)
- [Git Flow Sprint Specs](plan/spec/git-flow/)
