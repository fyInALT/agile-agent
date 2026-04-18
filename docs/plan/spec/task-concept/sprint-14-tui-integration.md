# Sprint 14: TUI Integration

## Metadata

- Sprint ID: `decision-sprint-014`
- Title: `TUI Integration`
- Duration: 1-2 weeks
- Priority: P1 (High)
- Status: `Completed`
- Created: 2026-04-18
- Depends on: `decision-sprint-013` (Decision Engine)

## Reference

See [Task Concept Requirements](../../decision-layer-task-concept-requirements.md) for requirements:
- Integration Requirements: Section 6.1 TUI Integration
- Usability Requirements: Section 5.4

## Sprint Goal

Integrate the task concept into TUI, providing task visibility, history access, and human decision overlays.

## Context

With the DecisionEngine working, users need to:
1. See active tasks and their status
2. View execution history
3. Respond to human decision requests

This sprint adds the UI layer for task management.

## Stories

### Story 14.1: Task Panel Widget

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Create a task panel widget for the dashboard.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T14.1.1 | Create `TaskPanel` widget | Todo | - |
| T14.1.2 | Display active task list | Todo | - |
| T14.1.3 | Show task status and counts | Todo | - |
| T14.1.4 | Add keyboard navigation | Todo | - |
| T14.1.5 | Write unit tests for panel | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T14.1.T1 | Panel renders with tasks |
| T14.1.T2 | Status displayed correctly |
| T14.1.T3 | Reflection count shown |
| T14.1.T4 | Keyboard navigation works |

#### Acceptance Criteria

- Task panel visible in dashboard
- All active tasks displayed
- Status and counts readable

#### Technical Notes

```rust
// In tui/src/task_panel.rs

pub struct TaskPanel {
    tasks: Vec<TaskInfo>,
    selected_index: usize,
}

struct TaskInfo {
    id: TaskId,
    description: String,
    status: TaskStatus,
    reflection_count: usize,
    confirmation_count: usize,
}

impl TaskPanel {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Render task list with status indicators
    }
    
    pub fn handle_key(&mut self, key: KeyEvent) -> TaskPanelCommand {
        match key.code {
            KeyCode::Up => { self.move_up(); TaskPanelCommand::None }
            KeyCode::Down => { self.move_down(); TaskPanelCommand::None }
            KeyCode::Enter => TaskPanelCommand::SelectTask { id: self.selected_task_id() }
            _ => TaskPanelCommand::None
        }
    }
}
```

### Story 14.2: Task Detail View

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Create a detailed view for selected task with history.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T14.2.1 | Create `TaskDetailView` widget | Todo | - |
| T14.2.2 | Display task description and constraints | Todo | - |
| T14.2.3 | Show execution history timeline | Todo | - |
| T14.2.4 | Highlight recent actions | Todo | - |
| T14.2.5 | Add scroll for long history | Todo | - |
| T14.2.6 | Write unit tests for detail view | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T14.2.T1 | Detail view renders task info |
| T14.2.T2 | History shown as timeline |
| T14.2.T3 | Scroll works for long history |
| T14.2.T4 | Actions timestamped correctly |

#### Acceptance Criteria

- Task detail view shows all info
- History visible as timeline
- Scrollable for long history

#### Technical Notes

```rust
pub struct TaskDetailView {
    task: Task,
    history_scroll: usize,
}

impl TaskDetailView {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Split area: top for task info, bottom for history
        
        // Task info section
        // - Description
        // - Constraints
        // - Status
        // - Reflection/Confirmation counts
        
        // History section
        // - Timeline of execution records
        // - Each record: action, timestamp, stage
    }
    
    pub fn scroll_up(&mut self) {
        if self.history_scroll > 0 {
            self.history_scroll -= 1;
        }
    }
    
    pub fn scroll_down(&mut self) {
        self.history_scroll += 1;
    }
}
```

### Story 14.3: Human Decision Overlay Integration

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Connect task decisions to the human decision overlay.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T14.3.1 | Extend `HumanDecisionOverlay` for tasks | Todo | - |
| T14.3.2 | Add task-specific response options | Todo | - |
| T14.3.3 | Pass task context to overlay | Todo | - |
| T14.3.4 | Handle response in decision engine | Todo | - |
| T14.3.5 | Write integration tests | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T14.3.T1 | Overlay shows task context |
| T14.3.T2 | Response options include task-specific |
| T14.3.T3 | Response triggers engine action |
| T14.3.T4 | Overlay dismissed after response |

#### Acceptance Criteria

- Human decision overlay works with tasks
- Task-specific options available
- Response processed by engine

#### Technical Notes

```rust
// Extend existing HumanDecisionOverlay

pub struct TaskDecisionOverlay {
    base: HumanDecisionOverlay,
    task: Task,
    stage: StageId,
}

impl TaskDecisionOverlay {
    pub fn new(request: HumanDecisionRequest, task: Task, stage: StageId) -> Self {
        Self {
            base: HumanDecisionOverlay::new(request),
            task,
            stage,
        }
    }
    
    pub fn handle_response(&self, response: HumanResponse) -> TaskDecisionCommand {
        match response {
            HumanResponse::Approve => TaskDecisionCommand::Approve,
            HumanResponse::Deny { reason } => TaskDecisionCommand::Deny { reason },
            HumanResponse::Custom { feedback } => TaskDecisionCommand::CustomFeedback { feedback },
            HumanResponse::Cancel => TaskDecisionCommand::CancelTask,
        }
    }
}
```

### Story 14.4: Metrics Panel

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Add a metrics panel showing automation statistics.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T14.4.1 | Create `MetricsPanel` widget | Todo | - |
| T14.4.2 | Display automation rate | Todo | - |
| T14.4.3 | Show reflection/confirmation averages | Todo | - |
| T14.4.4 | Display completion rate | Todo | - |
| T14.4.5 | Write unit tests for metrics | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T14.4.T1 | Metrics panel renders |
| T14.4.T2 | Automation rate shown |
| T14.4.T3 | Average reflections shown |

#### Acceptance Criteria

- Metrics visible in dashboard
- Target metrics shown (>80% automation)

### Story 14.5: Keyboard Shortcuts for Tasks

**Priority**: P2
**Effort**: 2 points
**Status**: Backlog

Add keyboard shortcuts for task operations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T14.5.1 | Add Ctrl+D for task detail | Todo | - |
| T14.5.2 | Add Ctrl+R for force reflect | Todo | - |
| T14.5.3 | Add Ctrl+C for force confirm | Todo | - |
| T14.5.4 | Add Ctrl+X for cancel task | Todo | - |
| T14.5.5 | Write tests for shortcuts | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T14.5.T1 | Ctrl+D opens detail view |
| T14.5.T2 | Ctrl+R triggers reflection |
| T14.5.T3 | Ctrl+C triggers confirmation |
| T14.5.T4 | Ctrl+X cancels task |

#### Acceptance Criteria

- Shortcuts work in task context
- Actions executed immediately

## Sprint Deliverables

1. `TaskPanel` widget for dashboard
2. `TaskDetailView` with history
3. Task decision overlay integration
4. `MetricsPanel` for statistics
5. Keyboard shortcuts for tasks
6. Integration tests for TUI

## Sprint Review Checklist

- [x] All tasks completed
- [x] All tests passing
- [x] Task panel visible in dashboard
- [x] Detail view shows history
- [x] Human decision overlay works
- [x] Code reviewed and merged

## Risks and Mitigation

| Risk | Mitigation |
|------|------------|
| TUI layout conflicts | Use existing layout patterns |
| Performance with many tasks | Limit display to active tasks |
| Keybinding conflicts | Use Ctrl+ combinations |

## Next Sprint Preview

Sprint 15 will add metrics and customization:
- Metrics collection implementation
- Custom workflow support
- YAML configuration files