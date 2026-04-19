# Git-Flow Task Preparation Requirements

## Executive Summary

This document defines requirements for enhancing the decision layer to provide comprehensive git workflow guidance for work agents. The goal is to ensure agents follow proper git-flow practices, work on clean branches based on the latest main/master, and maintain good commit hygiene throughout their development work.

## Problem Statement

### Current Issues

1. **Branch Naming Chaos**: Work agents currently create generic `agent/agent_XXX` branches without task context, making it difficult to track what work each branch contains.

2. **Outdated Code Base**: Agents may start working on stale branches that are not synchronized with the latest main/master, leading to merge conflicts and integration issues later.

3. **Uncommitted Work**: Agents frequently leave uncommitted changes at task boundaries, causing state pollution and potential work loss.

4. **Git-Flow Violations**: AI agents (Claude, Codex) often:
   - Forget to commit after completing logical units
   - Skip branch creation for new features
   - Mix multiple unrelated changes in single commits
   - Fail to create meaningful commit messages
   - Don't follow conventional commit conventions

5. **Missing Recovery Handling**: No systematic approach to handle:
   - Merge conflicts during rebase
   - Uncommitted changes before task switch
   - Branch cleanup after task completion

## Goals

1. **Task-Driven Branch Naming**: Each task should have a meaningful branch name derived from task metadata.

2. **Clean Start Guarantee**: Every task should start from a clean state based on the latest main/master.

3. **Commit Hygiene**: Enforce proper commit practices at logical boundaries.

4. **Git-Flow Compliance**: Guide agents through proper feature branch workflow.

5. **Graceful Recovery**: Handle git workflow errors and conflicts systematically.

## Architecture Overview

### Task Preparation Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Decision Layer: Task Preparation                      │
└─────────────────────────────────────────────────────────────────────────┘

  Task Assignment Request
         │
         ▼
  ┌──────────────────────┐
  │ TaskMetaExtractor    │  NEW
  │ - Analyze task       │
  │ - Generate branch    │
  │ - Create summary     │
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────┐
  │ GitStateAnalyzer     │  NEW
  │ - Check current      │
  │ - Detect changes     │
  │ - Validate branch    │
  └──────────────────────┘
         │
         ├─► [Clean State] ─► Proceed to Branch Setup
         │
         ├─► [Uncommitted] ─► EvaluateCommitAction
         │                      │
         │                      ├─► CommitChanges (if valid work)
         │                      └─► StashOrDiscard (if partial/broken)
         │
         ├─► [Outdated Branch] ─► RebaseToMainAction
         │                         │
         │                         ├─► Success ─► Continue
         │                         └─► Conflicts ─► ConflictResolution
         │
         ▼
  ┌──────────────────────┐
  │ BranchSetupAction    │  NEW
  │ - Fetch main/master  │
  │ - Create feature     │
  │ - Checkout new       │
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────┐
  │ TaskStartAction      │
  │ - Send task context  │
  │ - Include git info   │
  │ - Begin development  │
  └──────────────────────┘
```

### New Situations

| Situation | Trigger | Priority | Description |
|-----------|---------|----------|-------------|
| `task_starting` | Task assignment | High | New task about to begin, needs preparation |
| `uncommitted_changes_detected` | Git state check | Medium | Uncommitted work found before task switch |
| `branch_outdated` | Git state check | Medium | Branch behind main/master |
| `merge_conflict` | Rebase failure | High | Conflicts during rebase operation |
| `commit_boundary` | Logical unit complete | Low | Good point to commit changes |
| `task_completion_ready` | Task done, needs git cleanup | Medium | Task complete, prepare for merge/PR |

### New Actions

| Action | Situation | Purpose |
|--------|-----------|---------|
| `prepare_task_start` | `task_starting` | Full preparation pipeline |
| `evaluate_uncommitted` | `uncommitted_changes_detected` | Decide commit/stash/discard |
| `commit_changes` | Various | Create proper commit |
| `rebase_to_main` | `branch_outdated` | Sync with upstream |
| `resolve_conflicts` | `merge_conflict` | Guide conflict resolution |
| `create_feature_branch` | `task_starting` | Create task-specific branch |
| `suggest_commit` | `commit_boundary` | Prompt agent to commit |
| `prepare_pr` | `task_completion_ready` | Ready branch for PR |

## Detailed Requirements

### R1: Task Meta Extraction

**Priority**: P0

Before a work agent begins a new task, the decision layer must extract and generate:

1. **Branch Name**: Derived from task title/description
   - Format: `{type}/{slug}` where type is `feature`, `fix`, `refactor`, `docs`, `test`
   - Slug: Shortened, sanitized task description (max 30 chars)
   - Example: `feature/add-user-auth`, `fix/login-timeout`

2. **Task Summary**: Brief description for commit message reference
   - Max 72 characters
   - Clear, actionable description

3. **Work Type Classification**: Auto-detect from task description
   - `feature`: New functionality
   - `fix`: Bug fixes
   - `refactor`: Code restructuring without behavior change
   - `docs`: Documentation changes
   - `test`: Test additions/modifications
   - `chore`: Maintenance tasks

**Implementation Notes**:
- Use LLM classifier for type detection
- Sanitize branch names (no special chars, spaces)
- Append unique suffix if branch name collision occurs

### R2: Git State Analysis

**Priority**: P0

Before task start, analyze current git state:

1. **Uncommitted Changes Check**
   - Run `git status --porcelain`
   - Classify changes: staged, unstaged, untracked
   - Identify if changes belong to current task or previous work

2. **Branch Status Check**
   - Current branch name
   - Commits ahead/behind main/master
   - Last commit timestamp

3. **Conflict Check**
   - Check for existing conflict markers
   - Detect incomplete merge/rebase

4. **Worktree Health Check**
   - Verify worktree exists and is valid
   - Check for lock files or stale state

### R3: Uncommitted Changes Handling

**Priority**: P0

When uncommitted changes are detected:

**Decision Logic**:
```
IF changes_are_related_to_current_task:
    IF task_is_complete:
        ACTION: commit_with_task_message
    ELSE:
        ACTION: stash_with_description OR commit_wip
ELSE IF changes_are_from_previous_aborted_task:
    IF changes_are_valuable:
        ACTION: commit_with_previous_task_ref
    ELSE:
        ACTION: discard_with_confirmation
ELSE IF changes_are_unknown_or_experimental:
    ACTION: stash_with_timestamp OR request_human_decision
```

**Commit Message Format**:
- Follow Conventional Commits: `<type>(<scope>): <description>`
- Include task reference when available
- Example: `feat(auth): add user login validation [task-123]`

### R4: Branch Synchronization

**Priority**: P0

Ensure agent works on latest code:

1. **Fetch Latest**
   - `git fetch origin main` (or master)
   - Get latest HEAD commit SHA

2. **Rebase Strategy**
   - For existing task branches: `git rebase origin/main`
   - Handle conflicts gracefully (see R5)

3. **New Branch Creation**
   - Create from `origin/main` HEAD
   - Use task-derived name (from R1)

4. **Branch Collision Handling**
   - If branch exists but not checked out: use existing or create with suffix
   - If branch exists and checked out elsewhere: error with suggestion

### R5: Merge Conflict Resolution

**Priority**: P1

When rebase produces conflicts:

1. **Conflict Detection**
   - Parse `git status` for unmerged paths
   - Classify conflict severity (simple vs complex)

2. **Resolution Guidance**
   - For simple conflicts: provide resolution hints to agent
   - For complex conflicts: escalate to human with context

3. **Resolution Actions**
   - `abort_rebase`: Return to pre-rebase state
   - `continue_with_resolution`: Agent resolves conflicts
   - `request_human_help`: Escalate complex conflicts

4. **Post-Resolution Verification**
   - Verify no remaining conflicts
   - Run tests if available
   - Continue rebase after resolution

### R6: Commit Hygiene Enforcement

**Priority**: P1

Encourage proper commit practices:

1. **Commit Boundary Detection**
   - After logical unit completion (file save, test pass)
   - After feature/fix implementation
   - Before major refactoring

2. **Commit Prompt Actions**
   - Send gentle reminder to agent: "Consider committing your changes now"
   - Include suggested commit message based on changes

3. **Pre-Commit Validation**
   - Check for sensitive files (.env, credentials)
   - Verify commit message format
   - Warn about large commits (suggest splitting)

4. **Commit Message Templates**
   - Provide templates based on work type
   - Include Co-authored-by for AI attribution

### R7: Task Completion Git Workflow

**Priority**: P1

When task is complete:

1. **Final Commit Verification**
   - Ensure all changes are committed
   - Verify commit message quality

2. **Branch Status Check**
   - Commits ahead of main
   - No conflicts with main (rebase if needed)

3. **PR Preparation (Optional)**
   - Generate PR title from commits
   - Create PR description template
   - Suggest reviewers if configured

4. **Cleanup Actions**
   - Archive or delete completed feature branch
   - Reset worktree for next task

### R8: Git Hooks Integration

**Priority**: P2

Optional integration with project git hooks:

1. **Pre-Commit Hook Support**
   - Run linting/formatting checks
   - Validate commit message format
   - Block sensitive file commits

2. **Pre-Push Hook Support**
   - Run tests before push
   - Verify branch naming

3. **Hook Failure Handling**
   - Report hook failures to agent
   - Provide guidance for fixes
   - Allow bypass with human approval

## Additional Improvements

### A1: Semantic Branch Naming

Beyond basic naming, support semantic patterns:

- `feature/{issue-id}-{slug}`: Link to issue tracker
- `epic/{epic-name}/{story-name}`: Hierarchical organization
- `hotfix/{severity}-{slug}`: Urgent fixes
- `experiment/{slug}`: Experimental work

### A2: Commit History Preservation

Maintain meaningful commit history:

- Avoid squashing useful intermediate commits
- Keep refactor commits separate from feature commits
- Preserve review-request commits

### A3: Multi-Agent Git Coordination

When multiple agents work on related tasks:

- Detect overlapping file changes
- Warn about potential conflicts
- Coordinate branch merge order
- Sequential PR creation for dependent changes

### A4: Git Workflow Analytics

Track and improve git practices:

- Average commits per task
- Commit message quality score
- Rebase conflict frequency
- Branch lifecycle metrics

### A5: Automatic Changelog Generation

From commit history:

- Group commits by type
- Generate markdown changelog
- Include in PR description
- Update project changelog file

## Implementation Considerations

### Performance

- Git operations should not block agent work
- Use async operations where possible
- Cache git state to avoid repeated checks
- Timeout for long-running git operations

### Error Handling

- Graceful degradation when git unavailable
- Clear error messages for agents
- Recovery paths for each failure mode
- Human escalation for critical failures

### Configuration

Allow project-specific configuration:

- Base branch name (main vs master)
- Branch naming conventions
- Commit message templates
- Hook enforcement level
- PR creation preferences

### Security

- Never expose sensitive git data
- Validate all git operations
- Block dangerous operations (force push to main)
- Require human approval for destructive actions

## Testing Requirements

### Unit Tests

- Task meta extraction logic
- Branch name generation
- Conflict detection
- Commit message validation

### Integration Tests

- Full task preparation flow
- Rebase scenarios
- Multi-agent conflict handling
- PR creation workflow

### Scenario Tests

- Clean task start
- Task start with uncommitted changes
- Task start on outdated branch
- Task completion with PR
- Conflict resolution scenarios

## Success Criteria

1. All tasks start on clean, up-to-date branches
2. No uncommitted work left at task boundaries
3. Branch names clearly indicate task purpose
4. Commit messages follow conventions
5. Merge conflicts handled without human escalation (90% cases)
6. PRs created with proper context (80% of completed tasks)

## Dependencies

- Existing `WorktreeManager` infrastructure
- Decision layer situation/action framework
- Git command execution capability
- Task management system integration

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Git operations slow down agents | Async execution, caching, timeouts |
| Conflict resolution too complex for AI | Human escalation, abort options |
| Over-committing creates noise | Smart commit boundary detection |
| Branch name collisions | Unique suffix appending |
| Breaking existing workflows | Gradual rollout, configuration options |

## Timeline

See sprint specifications in `docs/plan/spec/git-flow-preparation/` for detailed implementation schedule.

## References

- [Git Flow](https://nvie.com/posts/a-successful-git-branching-model/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [Semantic Versioning](https://semver.org/)
- Existing architecture: `docs/decision-layer-flow.md`
- Worktree implementation: `docs/plan/spec/worktree/`
