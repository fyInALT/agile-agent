# Git Flow Task Preparation Sprint Specifications

## Overview

This directory contains sprint specifications for implementing Git Flow task preparation in the decision layer. The feature ensures work agents follow proper Git workflow conventions when starting tasks.

## Design Reference

- **Requirements Document**: `docs/plan/git-flow-task-preparation-requirements.md`
- **Problem**: Work agents use generic branches, skip baseline sync, leave uncommitted work
- **Solution**: Decision layer executes task preparation phase with Git Flow operations

## Sprint Sequence

| Sprint | Title | Goal | Priority | Duration |
|--------|-------|------|----------|----------|
| [Sprint 1](./sprint-1-metadata-and-config.md) | Metadata & Config | Task metadata extraction, configuration | P0 | 1-2 days |
| [Sprint 2](./sprint-2-health-assessment.md) | Health Assessment | Workspace health checks | P0 | 1-2 days |
| [Sprint 3](./sprint-3-git-operations.md) | Git Operations | Baseline sync, branch creation, rebase | P0 | 2-3 days |
| [Sprint 4](./sprint-4-decision-integration.md) | Decision Integration | Situation, action, classifier | P0 | 1-2 days |
| [Sprint 5](./sprint-5-polish-and-tests.md) | Polish & Tests | Testing, documentation, optimization | P1 | 1 day |

## Total Duration: 6-10 days

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Git Flow Task Preparation                          │
└─────────────────────────────────────────────────────────────────────┘

Sprint 1: Infrastructure
├── GitFlowConfig (core/src/git_flow_config.rs)
├── TaskMetadata (decision/src/task_metadata.rs)
├── Branch name generator
└── Task type classifier

Sprint 2: Health Assessment  
├── UncommittedChangesInfo (core/src/workspace_health.rs)
├── BranchStatusInfo
├── WorkspaceHealthReport
└── WorktreeValidationResult

Sprint 3: Git Operations
├── BaselineSyncResult (core/src/git_flow_executor.rs)
├── FeatureBranchResult
├── RebaseResult
├── UncommittedHandlingResult
└── GitFlowExecutor

Sprint 4: Decision Layer
├── TaskPreparationSituation (decision/src/builtin_situations.rs)
├── PrepareWorkspaceAction (decision/src/builtin_actions.rs)
├── Task preparation classifier
└── Agent pool execution handler

Sprint 5: Polish
├── Integration tests
├── Error handling refinement
├── Documentation
└── Logging enhancement
```

## Key Files to Create/Modify

### New Files
- `core/src/git_flow_config.rs`
- `core/src/workspace_health.rs`
- `core/src/git_flow_executor.rs`
- `decision/src/task_metadata.rs`

### Modified Files
- `core/src/worktree_manager.rs` - Add new Git operations
- `core/src/agent_pool.rs` - Add preparation action execution
- `decision/src/builtin_situations.rs` - Add TaskPreparationSituation
- `decision/src/builtin_actions.rs` - Add PrepareWorkspaceAction

## Dependencies

- Git >= 2.17
- Existing WorktreeManager infrastructure
- Decision layer framework

## Success Metrics

1. 100% of branches follow naming convention
2. 100% of tasks start from latest main/master
3. Zero incidents of lost uncommitted code
4. Preparation completes in < 10 seconds

---

**Status**: Planning Complete, Implementation Pending
