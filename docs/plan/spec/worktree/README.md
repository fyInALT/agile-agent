# Worktree Integration Sprint Specifications

## Overview

This directory contains sprint specifications for implementing Git Worktree integration in the agile-agent multi-agent system. The worktree solution provides file system isolation for parallel agent development.

## Design Reference

- **Research Document**: `docs/plan/worktree/worktree-integration-research.md`
- **Problem**: Multiple agents modifying the same repository cause file conflicts, branch chaos, and state pollution
- **Solution**: Each agent works in an independent git worktree with its own branch and directory

## Sprint Sequence

| Sprint | Title | Goal | Priority |
|--------|-------|------|----------|
| [Sprint 1](./sprint-1-infrastructure.md) | Infrastructure | WorktreeManager core + persistence layer | P0 |
| [Sprint 2](./sprint-2-agent-integration.md) | Agent Integration | Integrate into AgentPool with resume support | P0 |
| [Sprint 3](./sprint-3-tui-display.md) | TUI Display | Show worktree status in TUI | P1 |
| [Sprint 4](./sprint-4-advanced-features.md) | Advanced Features | Branch management + crash recovery | P1 |

## Key Architecture Components

```
┌─────────────────────────────────────────────────────────────┐
│                        AgentPool                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                  WorktreeManager                     │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │    │
│  │  │ create()    │  │ remove()    │  │ prune()     │  │    │
│  │  │ list()      │  │ status()    │  │ cleanup()   │  │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  │    │
│  └─────────────────────────────────────────────────────┘    │
│                              │                               │
│              ┌───────────────┴───────────────┐              │
│              ▼                               ▼              │
│  ┌──────────────────┐            ┌──────────────────┐      │
│  │   AgentSlot      │            │   AgentSlot      │      │
│  │  (alpha)         │            │  (bravo)         │      │
│  │  cwd: .worktrees │            │  cwd: .worktrees │      │
│  │     /agent-alpha │            │     /agent-bravo │      │
│  │  branch: task/1  │            │  branch: task/2  │      │
│  └──────────────────┘            └──────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

## Core Benefits

1. **Complete Isolation**: Each agent works in independent directory and branch
2. **Seamless Integration**: Providers already support cwd parameter
3. **Resource Efficient**: Shared .git directory, saves disk space
4. **Fast Creation**: O(1) time to create new worktree
5. **Resume Support**: Worktree state persisted per-agent

## New Modules

- `core/src/worktree_manager.rs` - Worktree creation/removal/list operations
- `core/src/worktree_state.rs` - Persistent worktree state struct
- `core/src/worktree_state_store.rs` - Persistence operations

## Dependencies

- Git >= 2.17 (for full worktree feature support)
- No changes needed to existing provider implementations (claude.rs, codex.rs)