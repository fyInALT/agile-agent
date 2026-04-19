# Git-Flow Task Preparation Sprint Specifications

## Overview

This directory contains sprint specifications for implementing git-flow task preparation in the decision layer. The goal is to ensure work agents follow proper git workflow practices.

## Related Documents

- **Requirements**: `docs/plan/git-flow-task-preparation-requirements.md`
- **Decision Layer Flow**: `docs/decision-layer-flow.md`
- **Worktree Integration**: `docs/plan/spec/worktree/`

## Sprint Sequence

| Sprint | Title | Goal | Priority | Duration |
|--------|-------|------|----------|----------|
| [Sprint 1](./sprint-1-task-meta-extraction.md) | Task Meta Extraction | Extract task metadata and generate branch names | P0 | 2-3 days |
| [Sprint 2](./sprint-2-git-state-analysis.md) | Git State Analysis | Analyze current git state before task start | P0 | 2-3 days |
| [Sprint 3](./sprint-3-branch-sync-creation.md) | Branch Sync & Creation | Sync with main and create task-specific branches | P0 | 3-4 days |
| [Sprint 4](./sprint-4-uncommitted-handling.md) | Uncommitted Changes | Handle uncommitted changes before task start | P0 | 2-3 days |
| [Sprint 5](./sprint-5-integration.md) | Integration | Integrate all components into decision layer | P0 | 3-4 days |
| [Sprint 6](./sprint-6-commit-hygiene.md) | Commit Hygiene | Encourage proper commit practices during work | P1 | 2-3 days |
| [Sprint 7](./sprint-7-task-completion.md) | Task Completion | Git workflow for task completion | P1 | 2-3 days |
| [Sprint 8](./sprint-8-conflict-resolution.md) | Conflict Resolution | Handle merge/rebase conflicts | P1 | 3-4 days |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Decision Layer                            │
│                                                              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐     │
│  │ TaskMeta     │   │ GitState     │   │ BranchSetup  │     │
│  │ Extractor    │──▶│ Analyzer     │──▶│ Action       │     │
│  └──────────────┘   └──────────────┘   └──────────────┘     │
│         │                  │                  │              │
│         ▼                  ▼                  ▼              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐     │
│  │ TaskMeta     │   │ GitState     │   │ TaskStart    │     │
│  │ Situation    │   │ Situation    │   │ Action       │     │
│  └──────────────┘   └──────────────┘   └──────────────┘     │
│                                                              │
│  New Situations & Actions registered in builtin modules      │
└─────────────────────────────────────────────────────────────┘
```

## Key Decisions

1. **Sprint Ordering**: P0 sprints first to deliver MVP functionality
2. **Integration Sprint**: Sprint 5 brings all components together
3. **Incremental Delivery**: Each sprint delivers working functionality
4. **Test Coverage**: Each sprint includes unit and integration tests

## Success Metrics

- All P0 sprints complete: Basic git-flow support operational
- All P1 sprints complete: Full git workflow guidance available
- Test coverage > 80% for all new modules
- No regression in existing decision layer functionality

## Implementation Status

**P0 Sprints (1-5)**: Complete
- Sprint 1: TaskMetaExtractor implemented in `decision/src/task_metadata.rs`
- Sprint 2: GitStateAnalyzer implemented in `decision/src/git_state.rs`
- Sprint 3: BranchSync/Creation implemented in `core/src/git_flow_executor.rs`
- Sprint 4: UncommittedHandler implemented in `decision/src/uncommitted_handler.rs`
- Sprint 5: Integration complete via `agent_pool.rs` and decision layer

**P1 Sprints (6-8)**: Planned
- Sprint 6: Commit hygiene guidance (future work)
- Sprint 7: Task completion flow (future work)
- Sprint 8: Conflict resolution UI (future work)
