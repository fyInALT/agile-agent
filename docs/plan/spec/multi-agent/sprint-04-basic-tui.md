# Sprint 4: Basic Multi-Agent TUI

## Metadata

- Sprint ID: `sprint-004`
- Title: `Basic Multi-Agent TUI`
- Duration: 2 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-13
- Depends On: Sprint 1, Sprint 2, Sprint 3

## Sprint Goal

Display all agent states in TUI with status bar, agent switching, and creation controls. User can view and manage multiple agents from the TUI.

## Stories

### Story 4.1: Agent Status Bar

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Create status bar showing all agent statuses.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.1.1 | Design status bar layout with agent indicators | Todo | - |
| T4.1.2 | Implement `render_agent_status_bar()` | Todo | - |
| T4.1.3 | Show focused agent marker | Todo | - |
| T4.1.4 | Show busy/idle/error status per agent | Todo | - |
| T4.1.5 | Show provider type per agent | Todo | - |
| T4.1.6 | Write tests for status bar rendering | Todo | - |

#### Mockup

```
● alpha [claude] ● bravo [codex] ○ charlie [mock]    Ctrl+V 2
```

---

### Story 4.2: Agent Focus Switching

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement keyboard controls to switch focus between agents.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.2.1 | Add Tab/Shift+Tab for next/previous agent | Todo | - |
| T4.2.2 | Add Ctrl+1-9 for direct agent selection | Todo | - |
| T4.2.3 | Update focused_index on key press | Todo | - |
| T4.2.4 | Update transcript view to focused agent | Todo | - |
| T4.2.5 | Update composer to focused agent | Todo | - |
| T4.2.6 | Write tests for focus switching | Todo | - |

#### Acceptance Criteria

- Tab cycles through agents
- Ctrl+N jumps to Nth agent
- Transcript updates to focused agent

---

### Story 4.3: Per-Agent Transcript View

**Priority**: P1
**Effort**: 5 points
**Status**: Backlog

Display transcript for the currently focused agent.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.3.1 | Create transcript cache per agent | Todo | - |
| T4.3.2 | Implement transcript switching on focus change | Todo | - |
| T4.3.3 | Preserve scroll offset per agent | Todo | - |
| T4.3.4 | Preserve follow-tail state per agent | Todo | - |
| T4.3.5 | Update rendering to use focused transcript | Todo | - |
| T4.3.6 | Write tests for transcript switching | Todo | - |

#### Acceptance Criteria

- Transcript switches instantly on focus
- Scroll state preserved per agent
- No rendering glitches on switch

---

### Story 4.4: Agent Creation UI

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Add UI to spawn new agents from TUI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.4.1 | Add Ctrl+N keybinding for new agent | Todo | - |
| T4.4.2 | Create provider selection overlay | Todo | - |
| T4.4.3 | Implement spawn on provider selection | Todo | - |
| T4.4.4 | Focus new agent after spawn | Todo | - |
| T4.4.5 | Write tests for agent creation UI | Todo | - |

#### Mockup

```
┌─────────────────────────────────┐
│ New Agent - Select Provider     │
├─────────────────────────────────┤
│ > Claude                        │
│   Codex                         │
│   Mock                          │
├─────────────────────────────────┤
│ Enter to select, Esc to cancel  │
└─────────────────────────────────┘
```

---

### Story 4.5: Agent Stop UI

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Add UI to stop specific agents from TUI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T4.5.1 | Add Ctrl+X keybinding to stop focused agent | Todo | - |
| T4.5.2 | Show confirmation for stop | Todo | - |
| T4.5.3 | Implement graceful stop | Todo | - |
| T4.5.4 | Update status bar after stop | Todo | - |
| T4.5.5 | Write tests for stop UI | Todo | - |

#### Acceptance Criteria

- Ctrl+X stops focused agent
- Confirmation dialog appears
- Agent marked as stopped

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Transcript switching performance | Medium | Medium | Cache rendered cells per agent |
| Focus switch latency | Low | Low | Pre-render transcripts |

## Sprint Deliverables

- Modified `render.rs` with status bar
- Modified `input.rs` with new keybindings
- Per-agent transcript caching
- Agent creation/stop overlays

## Dependencies

- Sprint 1: AgentPool (for status queries)
- Sprint 2: Provider Threads (for real agent creation)
- Sprint 3: Task Distribution (for task info display)

## Next Sprint

After completing this sprint, proceed to [Sprint 5: Persistence](./sprint-05-persistence.md).