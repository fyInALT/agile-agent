# Multi-Agent TUI Overview Design Document

## Overview

This document describes the Overview display mode design for Multi-Agent TUI. This mode supports the "Overview Agent as primary entry point" multi-agent collaboration workflow, where users primarily interact with the Overview Agent to discuss ideas and requirements, which then decomposes tasks and coordinates other Agents.

## Core Design Principles

- **Overview Agent as Team Lead**: Users primarily converse with the Overview Agent to discuss ideas and requirements. It decomposes tasks and coordinates other Agents.
- **Minimal User Interaction with Worker Agents**: Worker Agents work in parallel, automatically picking up tasks from Kanban.
- **Human Intervention Only When Necessary**: When an Agent encounters a problem it cannot resolve (blocked), users switch to that Agent for direct conversation.
- **Layered Information Display**: Upper section shows all Agents' status overview; lower section displays different levels of information based on focus.

## Layout Structure

```
─────────────────────────────────────────────────────────────────────
◎ OVERVIEW      idle  Coordinating Agent work
● alpha         run   Analyzing code structure (2m30s)
● bravo         idle  Waiting for task
○ charlie       blk   🔶 Waiting for API design confirmation
─────────────────────────────────────────────────────────────────────
[Scroll Log Area - Shows all Agents' simplified output in Overview mode]
[Or Agent transcript area - Shows focused Agent's full output]

─────────────────────────────────────────────────────────────────────
> User input box                                                 [?help]
─────────────────────────────────────────────────────────────────────
```

### Region Description

| Region | Content |
|--------|---------|
| Agent Status List | Top, no borders, line-separated, max 8 lines (configurable) |
| Content Area | Middle: scroll log in Overview mode; focused Agent's transcript otherwise |
| Input Box | Bottom, user input sent to currently focused Agent |

### Agent Status List

- **Fixed N rows** (configurable, default: 8)
  - Purpose: Prevent content area jitter from Agent count changes
  - Config path: `Settings → TUI → Agent List Rows` (range: 3-12)
- **No borders**, distinguished only by line breaks
- **Overview Agent (OVERVIEW) always displays at the top**
- Currently focused Agent row may have highlighted background

#### Empty State

When there are no Agents, display a prompt:
```
─────────────────────────────────────────────────────────────────────
◎ OVERVIEW      idle  Coordinating Agent work
                (empty line)
                (empty line)
                (empty line)
                (empty line)
                (empty line)
                (empty line)
                (empty line)
─────────────────────────────────────────────────────────────────────
Hint: Press Ctrl+N to create a new Agent
```

#### Width Adaptation (No Wrapping, Truncation)

When terminal width decreases, Agent rows do not wrap. Instead, content is truncated from the right:

```
// Normal width
◎ OVERVIEW      idle  Coordinating Agent work

// Width reduced
◎ OVERVIEW  idle  Coordinating A...

// Narrowest width (minimum: indicator + name prefix)
◎ O..
```

Truncation order: Task description → Status → Name (prefix preserved)

## Agent Row Information

Each row displays the following:

```
│ Indicator │ Name    │ Status │ Task Description [+ Duration/Progress] │
```

### Indicators

| Indicator | Meaning |
|-----------|---------|
| `◎` | Overview Agent (OVERVIEW) |
| `●` | Running |
| `○` | Idle |
| `◌` | Stopped |
| `🔶` | Blocked - Requires human intervention |

### Status

| Status | Meaning | Behavior |
|--------|---------|----------|
| `run` | Running | Currently executing a task |
| `idle` | Idle | Waiting for new task, auto-picks from Kanban |
| `blk` | Blocked | Encountered unresolvable problem, requires human intervention |
| `stop` | Stopped | Agent has terminated |

### Task Description

- **Running**: Current task description + elapsed time
- **Idle**: Brief description or "Waiting for task"
- **Blocked**: Problem description
- **Completed**: Brief summary of just-completed task (briefly shown before entering idle)

## Interaction Design

### Focus Switching

| Action | Behavior |
|--------|----------|
| `Tab` | Cycle focus to next Agent |
| `Shift+Tab` | Reverse cycle |
| `1-9` | Directly select Nth Agent (first page) |
| Click | Select clicked Agent |
| `[/]` or `PageUp/PageDown` | Page navigation (when Agents > 8) |

### Filtering and Search

| Action | Behavior |
|--------|----------|
| `f` | Filter to show only blocked agents |
| `r` | Filter to show only running agents |
| `a` | Show all agents |
| `/{name}` | Search and select specified agent |

### Agent Operations

| Action | Behavior |
|--------|----------|
| `Ctrl+N` | Create new Agent |
| `Ctrl+X` | Stop focused Agent (requires confirmation) |

### Command Routing

**Default behavior**: User input is sent to the currently focused Agent

**@ Command**:
- `@alpha hello` → Send to alpha
- `@alpha @bravo you two collaborate` → Broadcast to multiple Agents

## Simplified Output (Overview Mode)

When focus is on OVERVIEW, the lower area displays scrollable simplified output logs from all Agents.

### Output Format

```
[14:32:15] ● alpha: Started analyzing module A code
[14:32:18] ● alpha: Found 3 TODOs
[14:32:20] ● bravo: Received task [TASK-102]
[14:32:25] 🔶 charlie: BLOCKED - API design not confirmed, requires human intervention
[14:32:30] ● alpha: Analysis complete, submitted PR #234
```

### Message Type Prefixes

| Prefix | Type | Meaning |
|--------|------|---------|
| `●` | Progress | Normal progress update |
| `🔶` | Blocked | Issue requiring attention |
| `✓` | Complete | Step completed |
| `⚡` | Quick | Completed in short time |
| `📋` | Task | Task assignment/receipt |

### Timestamps

- Messages within the same minute may omit time
- Can be configured to show full timestamp or relative time

## Simplification Level Configuration

Users can configure output verbosity for each Agent, with three levels:

| Level | Displayed Content |
|-------|-------------------|
| **Full** | All status changes, detailed progress, tool call results |
| **Concise** | Key steps, task start/complete, blocked/error |
| **Minimal** | Only blocked/error and task completion |

### Configuration Location

- **Global Default**: Set default level for all Agents
- **Per-Agent**: Override level for specific Agent
- Config path: `Settings → Agent Output Level`

## State Transitions and Decision Mechanism

### Agent State Transitions

```
          ┌──────┐
          │ start│
          └──┬───┘
             ▼
         ┌───────┐     Task complete     ┌─────┐
    ┌───▶│ idle  │◀─────────────────│ run │
    │    └───┬───┘                   └──┬──┘
    │        │                          │
    │        │ Auto-pick                │ Encounters problem
    │        ▼                          ▼
    │    ┌───────┐                 ┌────┐
    └────│ idle  │                 │blk │
         └───────┘                 └────┘
```

### Decision Mechanism (Future)

- When an Agent enters `idle`, it automatically picks up completable tasks from the Kanban system
- Decision layer (future implementation) determines task priority and assignment
- Users can modify task assignments through the Overview Agent

## Relationship with Existing ViewModes

| ViewMode | Characteristic | Suitable Scenario |
|----------|----------------|-------------------|
| **Focused** | Single Agent full view | Deep interaction with single Agent |
| **Split** | Two Agents side by side | Compare outputs of two Agents |
| **Dashboard** | All Agent cards overview | Quick status overview |
| **Overview** | Upper multi-line + lower scroll log | **Multi-Agent coordination workflow (Overview Agent as primary entry)** |

## Future Extensions

- [ ] Agent classification/grouping display (Dev/Review/System)
- [ ] Collapse inactive Agents
- [ ] Performance optimization for >15 Agents
- [ ] Audio/visual alerts (blocked > X minutes)
- [ ] Task progress percentage display
- [ ] Agent role indicators (PO/SM/Developer)

## Pending Details

1. **Simplified output implementation rules**: Specific content per level
2. **@ Command parsing syntax**: Support `@alpha,bravo` or `@alpha @bravo`
3. **Blocked detail prompt format**: Show "suggested action"
4. **Scroll output buffer size**: How many historical records to retain

---

## Design Status

- [x] Core layout design
- [x] Interaction design
- [x] Simplified output format
- [ ] Configuration system design
- [ ] Implementation planning
