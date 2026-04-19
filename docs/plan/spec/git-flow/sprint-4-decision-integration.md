# Sprint 4: Decision Layer Integration

## Sprint Overview

**Duration**: 1-2 days
**Goal**: Integrate Git Flow task preparation into decision layer
**Priority**: P0 (Critical)

## Stories

### Story 4.1: TaskPreparationSituation

**Description**: Create new situation type for task preparation phase.

**Acceptance Criteria**:
- [ ] `TaskPreparationSituation` struct in `decision/src/builtin_situations.rs`
- [ ] Triggered when agent is assigned a new task
- [ ] Contains workspace health report and recommendations
- [ ] Available actions: prepare_workspace, skip_preparation, prompt_user
- [ ] Unit tests

**Implementation Notes**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPreparationSituation {
    pub task_id: String,
    pub task_description: String,
    pub workspace_health: WorkspaceHealthReport,
    pub suggested_branch: String,
    pub base_commit: String,
    pub requires_user_input: bool,
}

impl DecisionSituation for TaskPreparationSituation {
    fn situation_type(&self) -> SituationType {
        SituationType::new("task_preparation")
    }
    
    fn requires_human(&self) -> bool {
        self.requires_user_input
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            prepare_workspace(),
            skip_preparation(),
            request_human(),
        ]
    }
}
```

### Story 4.2: PrepareWorkspaceAction

**Description**: Create action to execute workspace preparation.

**Acceptance Criteria**:
- [ ] `PrepareWorkspaceAction` struct in `decision/src/builtin_actions.rs`
- [ ] Executes Git Flow preparation workflow
- [ ] Returns preparation result
- [ ] Handles errors gracefully
- [ ] Unit tests

**Implementation Notes**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareWorkspaceAction {
    pub branch_name: String,
    pub handling_policy: UncommittedHandlingPolicy,
    pub sync_baseline: bool,
}

impl DecisionAction for PrepareWorkspaceAction {
    fn action_type(&self) -> ActionType {
        ActionType::new("prepare_workspace")
    }
    
    fn to_prompt_format(&self) -> String {
        format!("PrepareWorkspace: branch={}, policy={}", 
            self.branch_name, self.handling_policy)
    }
}

pub fn prepare_workspace() -> ActionType {
    ActionType::new("prepare_workspace")
}
```

### Story 4.3: Task Preparation Classifier

**Description**: Classifier to detect task preparation situations.

**Acceptance Criteria**:
- [ ] Detect when agent needs task preparation
- [ ] Trigger on: new task assignment, agent idle with no branch
- [ ] Gather workspace health data
- [ ] Create TaskPreparationSituation
- [ ] Unit tests

**Trigger Conditions**:
```text
1. Agent receives new task assignment
2. Agent is idle with no valid feature branch
3. Agent's current branch doesn't match task metadata
4. Agent workspace health score < threshold
```

### Story 4.4: Action Execution Handler

**Description**: Execute prepare_workspace action in agent_pool.

**Acceptance Criteria**:
- [ ] Handle "prepare_workspace" action in `execute_decision_action`
- [ ] Call GitFlowExecutor to perform preparation
- [ ] Update agent's worktree state
- [ ] Log preparation results
- [ ] Handle preparation failures

**Implementation in agent_pool.rs**:
```rust
match action_type.as_str() {
    "prepare_workspace" => {
        // Parse action params
        let params: PrepareWorkspaceParams = parse_params(&params_str);
        
        // Execute preparation via GitFlowExecutor
        let result = self.git_flow_executor.prepare_for_task(
            &params.task_id,
            &params.task_description,
        );
        
        match result {
            Ok(preparation) => {
                // Update agent slot with new worktree info
                slot.set_worktree(preparation.worktree_path, 
                    Some(preparation.branch_name), 
                    preparation.worktree_id);
                
                // Add preparation prompt to transcript
                slot.add_user_message(format!(
                    "Workspace prepared. Branch: {}, Base: {}",
                    preparation.branch_name, preparation.base_commit
                ));
                
                DecisionExecutionResult::Prepared { 
                    branch: preparation.branch_name 
                }
            }
            Err(e) => {
                logging::warn_event(...);
                DecisionExecutionResult::PreparationFailed { 
                    reason: e.to_string() 
                }
            }
        }
    }
}
```

### Story 4.5: Task Start Flow Integration

**Description**: Integrate preparation into task start flow.

**Acceptance Criteria**:
- [ ] Trigger preparation when agent assigned task
- [ ] Block agent until preparation complete
- [ ] Resume agent after successful preparation
- [ ] Handle preparation errors with retry/prompt
- [ ] End-to-end tests

**Flow Diagram**:
```
Task Assignment
       │
       ▼
┌─────────────────────┐
│ Agent → blocked_for │
│ _preparation        │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│ TaskPreparationSit  │
│ sent to decision    │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│ Decision: prepare   │
│ _workspace          │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│ GitFlowExecutor     │
│ executes            │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│ Agent → idle        │
│ (ready to work)     │
└─────────────────────┘
```

## Dependencies

- Sprint 1, 2, 3 (all Git Flow infrastructure)
- Decision layer framework

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Preparation timeout | Add timeout handling |
| Agent stuck blocked | Recovery mechanism |
| Decision errors | Fallback to rule engine |

## Testing Strategy

- Unit tests for situation/action
- Integration tests with decision flow
- End-to-end tests with agent spawning

## Deliverables

1. New situation and action in decision layer
2. Updated agent_pool.rs action execution
3. Integration tests

---

**Sprint Status**: Planned
