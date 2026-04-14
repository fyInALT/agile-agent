# Sprint 8: Advanced TUI Modes

## Metadata

- Sprint ID: `sprint-008`
- Title: `Advanced TUI Modes`
- Duration: 2 weeks
- Priority: P2 (Medium)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 4, Sprint 7

## Sprint Goal

Implement multiple TUI view modes (Split, Dashboard, Mail, TaskMatrix) for different workflows. Users can switch views based on their current needs.

## Stories

### Story 8.1: ViewMode State

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Create ViewMode enum and view state management.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.1.1 | Create `ViewMode` enum with 5 modes | Todo | - |
| T8.1.2 | Create `TuiViewState` for mode-specific state | Todo | - |
| T8.1.3 | Add Ctrl+V 1-5 keybindings for mode switch | Todo | - |
| T8.1.4 | Add Ctrl+V Space for quick switch menu | Todo | - |
| T8.1.5 | Write tests for mode switching | Todo | - |

#### ViewMode Enum

```rust
pub enum ViewMode {
    Focused,   // Single agent transcript (default)
    Split,     // Two agents side by side
    Dashboard, // All agents in compact cards
    Mail,      // Mail/communication focus
    TaskMatrix, // Task assignment grid
}
```

---

### Story 8.2: Split View

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement side-by-side view for two agents.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.2.1 | Create split layout renderer | Todo | - |
| T8.2.2 | Implement left/right agent selection | Todo | - |
| T8.2.3 | Add arrow keys for side selection | Todo | - |
| T8.2.4 | Add `s` key for swap | Todo | - |
| T8.2.5 | Add `e` key for equal split | Todo | - |
| T8.2.6 | Handle composer for split mode | Todo | - |
| T8.2.7 | Write tests for split view | Todo | - |

#### Mockup

```
┌─────────────────────────────────────────────────────────────────┐
│ Split View: alpha [claude] | bravo [codex]          Ctrl+V 3    │
├─────────────────────────────────────────────┬───────────────────┤
│ [alpha]                                      │ [bravo]           │
│                                              │                   │
│ › user: Write tests                          │ › user: Write UI  │
│                                              │                   │
│ Working: task-1 (32s)                        │ Idle              │
├─────────────────────────────────────────────┴───────────────────┤
│ ←→ select side  s swap  e equal  Ctrl+V mode                   │
└─────────────────────────────────────────────────────────────────┘
```

---

### Story 8.3: Dashboard View

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement compact cards view for all agents.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.3.1 | Create agent card component | Todo | - |
| T8.3.2 | Layout cards in responsive grid | Todo | - |
| T8.3.3 | Show status per card | Todo | - |
| T8.3.4 | Show task info per card | Todo | - |
| T8.3.5 | Show last activity per card | Todo | - |
| T8.3.6 | Add number keys for card selection | Todo | - |
| T8.3.7 | Write tests for dashboard view | Todo | - |

#### Mockup

```
┌─────────────────────────────────────────────────────────────────┐
│ Agent Dashboard                              Ctrl+V 4           │
├─────────────────────────────────────────────────────────────────┤
│ ┌───────────────┐ ┌───────────────┐ ┌───────────────┐          │
│ │ ● alpha       │ │ ● bravo       │ │ ○ charlie     │          │
│ │ [claude]      │ │ [codex]       │ │ [mock]        │          │
│ │ Working       │ │ Working       │ │ Idle          │          │
│ │ task-1        │ │ task-2        │ │ 3 mails       │          │
│ └───────────────┘ └───────────────┘ └───────────────┘          │
├─────────────────────────────────────────────────────────────────┤
│ n new  x stop selected  r restart  Ctrl+V mode                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### Story 8.4: Mail View

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement view focused on cross-agent communication.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.4.1 | Create mail list component | Todo | - |
| T8.4.2 | Show inbox with unread indicator | Todo | - |
| T8.4.3 | Show mail subject and preview | Todo | - |
| T8.4.4 | Add arrow keys for mail selection | Todo | - |
| T8.4.5 | Add `r` key for reply | Todo | - |
| T8.4.6 | Add `m` key for mark read | Todo | - |
| T8.4.7 | Add compose mail UI | Todo | - |
| T8.4.8 | Write tests for mail view | Todo | - |

---

### Story 8.5: Task Matrix View

**Priority**: P2
**Effort**: 5 points
**Status**: Backlog

Implement task assignment grid view.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.5.1 | Create task matrix layout | Todo | - |
| T8.5.2 | Show tasks as rows | Todo | - |
| T8.5.3 | Show agents as columns | Todo | - |
| T8.5.4 | Show assignment status per cell | Todo | - |
| T8.5.5 | Add arrow keys for task/agent selection | Todo | - |
| T8.5.6 | Add `a` key for assign | Todo | - |
| T8.5.7 | Show task dependencies | Todo | - |
| T8.5.8 | Write tests for task matrix | Todo | - |

---

### Story 8.6: Responsive Layout

**Priority**: P3
**Effort**: 3 points
**Status**: Backlog

Adapt layouts based on terminal width.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.6.1 | Define width thresholds (<80, 80-120, 120-160, >160) | Todo | - |
| T8.6.2 | Adjust dashboard cards per width | Todo | - |
| T8.6.3 | Adjust split ratio per width | Todo | - |
| T8.6.4 | Collapse status bar on narrow | Todo | - |
| T8.6.5 | Write tests for responsive layout | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Rendering performance | Medium | Medium | Cache rendered cells, incremental updates |
| Mode switch latency | Low | Low | Pre-render inactive modes |
| Narrow terminal usability | Medium | Low | Graceful degradation, scroll overflow |

## Sprint Deliverables

- ViewMode enum and state
- Split View renderer
- Dashboard View renderer
- Mail View renderer
- Task Matrix View renderer
- Responsive layout adaptation

## Dependencies

- Sprint 4: Basic TUI, status bar
- Sprint 7: Mail system (for Mail view)

## Next Sprint

After completing this sprint, proceed to [Sprint 9: Kanban System](./sprint-09-kanban.md).