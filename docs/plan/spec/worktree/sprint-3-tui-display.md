# Sprint 3: TUI Display

## Metadata

- Sprint ID: `worktree-sprint-03`
- Title: `TUI Display`
- Duration: 1 week
- Priority: P1
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: [Sprint 2: Agent Integration](./sprint-2-agent-integration.md)
- Design Reference: `docs/plan/worktree/worktree-integration-research.md`

## Sprint Goal

Display worktree status in the Terminal UI, including branch name, worktree path, and worktree existence status. Add TUI commands for pause/resume agent management.

## Stories

### Story 3.1: Agent Status Panel Worktree Display

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Add worktree information to the agent status panel in TUI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Identify agent status panel component in TUI | Todo | - |
| T3.1.2 | Add branch name display to status panel | Todo | - |
| T3.1.3 | Add worktree path display (relative path) | Todo | - |
| T3.1.4 | Add worktree status indicator (active/missing) | Todo | - |
| T3.1.5 | Format branch name with color coding | Todo | - |
| T3.1.6 | Handle missing worktree display gracefully | Todo | - |
| T3.1.7 | Write unit tests for display formatting | Todo | - |

#### Display Layout

```
┌─────────────────────────────────────────┐
│ Agent: alpha [claude]                    │
│ Status: Responding                       │
│ Branch: agent/task-123                   │
│ Worktree: .worktrees/agent-alpha         │
│          ✓ Exists                        │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│ Agent: bravo [codex]                     │
│ Status: Paused                           │
│ Branch: agent/task-456                   │
│ Worktree: .worktrees/agent-bravo         │
│          ⚠ Missing (needs recreate)     │
└─────────────────────────────────────────┘
```

---

### Story 3.2: Worktree Existence Status Indicator

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Show visual indicator for worktree existence status.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Define WorktreeStatus enum (Exists, Missing, Error) | Todo | - |
| T3.2.2 | Implement status check method in AgentSlot | Todo | - |
| T3.2.3 | Add status indicator symbols (✓, ⚠, ✗) | Todo | - |
| T3.2.4 | Color code: green for exists, yellow for missing | Todo | - |
| T3.2.5 | Add tooltip/help text for status meanings | Todo | - |
| T3.2.6 | Write unit tests for status indicators | Todo | - |

#### Status Definitions

| Status | Symbol | Color | Meaning |
|--------|--------|-------|---------|
| Exists | ✓ | Green | Worktree directory present and valid |
| Missing | ⚠ | Yellow | Directory deleted, state exists |
| Error | ✗ | Red | Git error or invalid state |
| None | - | Gray | Agent not using worktree |

---

### Story 3.3: Pause/Resume TUI Commands

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Add TUI keyboard commands for pausing and resuming agents.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Add 'p' key binding for pause focused agent | Todo | - |
| T3.3.2 | Add 'r' key binding for resume focused agent | Todo | - |
| T3.3.3 | Add confirmation dialog for pause command | Todo | - |
| T3.3.4 | Add confirmation dialog for resume command | Todo | - |
| T3.3.5 | Update command help panel with new bindings | Todo | - |
| T3.3.6 | Handle pause/resume errors in TUI | Todo | - |
| T3.3.7 | Show loading spinner during resume operation | Todo | - |
| T3.3.8 | Write unit tests for command handling | Todo | - |

#### Key Bindings

| Key | Action | Description |
|-----|--------|-------------|
| `p` | Pause Agent | Pause focused agent, preserve worktree |
| `r` | Resume Agent | Resume paused agent, verify/recreate worktree |
| `P` | Pause All | Pause all running agents |
| `R` | Resume All | Resume all paused agents |

---

### Story 3.4: Agent Overview List Worktree Column

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Add worktree information column to the agent overview list view.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Identify agent overview list component | Todo | - |
| T3.4.2 | Add "Branch" column to list view | Todo | - |
| T3.4.3 | Add "Worktree" column (short path display) | Todo | - |
| T3.4.4 | Truncate long branch names appropriately | Todo | - |
| T3.4.5 | Handle missing worktree in list display | Todo | - |
| T3.4.6 | Write unit tests for list display | Todo | - |

#### List View Layout

```
┌─────┬──────────┬─────────┬───────────────────┬────────────────┬──────────┐
│ ID  │ Codename │ Provider│ Status            │ Branch         │ Worktree │
├─────┼──────────┼─────────┼───────────────────┼────────────────┼──────────┤
│ 001 │ alpha    │ claude  │ Responding        │ agent/task-123 │ ✓ alpha  │
│ 002 │ bravo    │ codex   │ Paused            │ agent/task-456 │ ⚠ bravo  │
│ 003 │ charlie  │ claude  │ ToolExecuting     │ agent/sprint-1 │ ✓ charlie│
└─────┴──────────┴─────────┴───────────────────┴────────────────┴──────────┘
```

---

### Story 3.5: Worktree Details Popup

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Add popup panel showing detailed worktree information.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.5.1 | Create WorktreeDetailsPopup component | Todo | - |
| T3.5.2 | Display full worktree path | Todo | - |
| T3.5.3 | Display branch name and HEAD commit | Todo | - |
| T3.5.4 | Display creation time and last activity | Todo | - |
| T3.5.5 | Display commits made by agent (count) | Todo | - |
| T3.5.6 | Display uncommitted changes status | Todo | - |
| T3.5.7 | Add 'w' key binding to open popup | Todo | - |
| T3.5.8 | Write unit tests for popup | Todo | - |

#### Popup Content

```
┌─────────────────────────────────────────────────┐
│ Worktree Details - Agent alpha                   │
├─────────────────────────────────────────────────┤
│ ID:          wt-alpha-001                        │
│ Path:        /home/user/repo/.worktrees/alpha    │
│ Branch:      agent/task-123                      │
│ HEAD:        ghi789abc...                        │
│ Base:        abc123def... (main)                 │
│ Created:     2026-04-16 10:00:00                 │
│ Last Active: 2026-04-16 12:30:00                 │
│ Commits:     3                                   │
│ Uncommitted: No                                  │
└─────────────────────────────────────────────────┘
│ Press 'q' or 'Esc' to close                      │
└─────────────────────────────────────────────────┘
```

---

### Story 3.6: Stop Agent with Cleanup Dialog

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Add cleanup option to stop agent confirmation dialog.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.6.1 | Update stop confirmation dialog | Todo | - |
| T3.6.2 | Add "Cleanup worktree?" checkbox | Todo | - |
| T3.6.3 | Add "Preserve worktree" checkbox | Todo | - |
| T3.6.4 | Show worktree branch name in dialog | Todo | - |
| T3.6.5 | Handle cleanup selection in stop command | Todo | - |
| T3.6.6 | Write unit tests for dialog | Todo | - |

#### Dialog Layout

```
┌─────────────────────────────────────────────────┐
│ Stop Agent alpha?                                │
├─────────────────────────────────────────────────┤
│ Worktree: .worktrees/agent-alpha                 │
│ Branch:   agent/task-123                         │
│                                                  │
│ [x] Cleanup worktree (delete branch and files)  │
│ [ ] Preserve worktree (keep for manual review)  │
│                                                  │
│ Warning: Cleanup will remove uncommitted changes │
│                                                  │
│ [Confirm]  [Cancel]                              │
└─────────────────────────────────────────────────┘
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| TUI rendering performance with many agents | Low | Medium | Limit display updates, use caching |
| Key binding conflicts | Low | Low | Review existing bindings, document new ones |
| Resume blocking TUI | Medium | Low | Run resume in background thread |

## Sprint Deliverables

- Modified TUI agent status panel with worktree info
- Worktree existence status indicators
- Pause/Resume keyboard commands
- Agent overview list with worktree columns
- Worktree details popup (optional)
- Stop agent with cleanup dialog

## Dependencies

- Sprint 2: Agent Integration (AgentSlot worktree fields)
- Existing TUI framework (ratatui)
- Key binding system

## Module Structure

```
tui/src/
├── components/
│   ├── agent_status_panel.rs    # Modified for worktree display
│   ├── agent_list_view.rs       # Modified for columns
│   ├── worktree_details_popup.rs # NEW
│   └── stop_confirm_dialog.rs   # Modified for cleanup option
├── handlers/
│   └── keyboard.rs              # Modified for pause/resume bindings
└── render.rs                    # Modified for layout
```

## Next Sprint

After completing this sprint, proceed to [Sprint 4: Advanced Features](./sprint-4-advanced-features.md) for branch management and crash recovery.