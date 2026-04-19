# Sprint 4: Decision Layer Integration

## Sprint Overview

**Duration**: 2-3 days
**Goal**: Integrate Git Flow preparation into decision layer as new situation/action
**Priority**: P0 (Critical)

## Stories

### Story 4.1: TaskPreparationSituation

**Description**: Create new situation type for task preparation.

**Acceptance Criteria**:
- [ ] `TaskPreparationSituation` struct in `decision/src/builtin_situations.rs`
- [ ] Triggered when agent is assigned a new task
- [ ] Contains: task_id, description, metadata, health_report
- [ ] Defines available actions for preparation
- [ ] Unit tests

**Implementation Notes**:
```rust
pub fn task_preparation() -> SituationType {
    SituationType::new("task_preparation")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPreparationSituation {
    pub task_id: String,
    pub task_description: String,
    pub generated_metadata: TaskMetadata,
    pub workspace_health: Option<WorkspaceHealthReport>,
    pub existing_branch: Option<String>,
    pub has_uncommitted: bool,
    pub requires_decision: bool,
}

impl DecisionSituation for TaskPreparationSituation {
    fn situation_type(&self) -> SituationType {
        task_preparation()
    }

    fn requires_human(&self) -> bool {
        // Requires human if conflicts detected or unusual state
        self.workspace_health.as_ref()
            .map(|h| h.score < 50)
            .unwrap_or(false)
    }

    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            prepare_new_branch(),
            reuse_existing_branch(),
            handle_uncommitted_first(),
            request_human_guidance(),
            abort_task_preparation(),
        ]
    }
}
```

### Story 4.2: PrepareNewBranchAction

**Description**: Create action for preparing a new feature branch.

**Acceptance Criteria**:
- [ ] `PrepareNewBranchAction` struct in `decision/src/builtin_actions.rs`
- [ ] Executes Git Flow preparation workflow
- [ ] Creates branch with proper naming
- [ ] Returns preparation result
- [ ] Logs all operations
- [ ] Unit tests

**Implementation Notes**:
```rust
pub fn prepare_new_branch() -> ActionType {
    ActionType::new("prepare_new_branch")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareNewBranchAction {
    pub task_id: String,
    pub branch_name: String,
    pub handle_uncommitted: UncommittedHandlingPolicy,
}

impl DecisionAction for PrepareNewBranchAction {
    fn action_type(&self) -> ActionType {
        prepare_new_branch()
    }

    fn to_prompt_format(&self) -> String {
        format!("PrepareNewBranch: task={}, branch={}", 
            self.task_id, self.branch_name)
    }

    fn serialize_params(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
```

### Story 4.3: AgentPool Task Preparation Hook

**Description**: Add task preparation hook to AgentPool.

**Acceptance Criteria**:
- [ ] AgentPool calls preparation before task assignment
- [ ] Preparation executes Git Flow operations
- [ ] Agent receives preparation result in context
- [ ] Handles preparation failures gracefully
- [ ] Unit tests

**Implementation Notes**:
```rust
impl AgentPool {
    /// Prepare worktree for new task assignment
    pub fn prepare_for_task(
        &mut self,
        agent_id: &AgentId,
        task_id: &str,
        task_description: &str,
    ) -> Result<PreparationResult, GitFlowError> {
        // 1. Get agent's worktree
        let slot = self.get_slot_by_id(agent_id)?;
        let worktree_path = slot.cwd();

        // 2. Execute Git Flow preparation
        let executor = self.get_git_flow_executor()?;
        executor.prepare_for_task(worktree_path, task_id, task_description)
    }

    /// Assign task with Git Flow preparation
    pub fn assign_task_with_preparation(
        &mut self,
        agent_id: &AgentId,
        task: Task,
    ) -> Result<(), AgentPoolError> {
        // Prepare first
        let prep_result = self.prepare_for_task(
            agent_id, 
            task.id.as_str(), 
            &task.description
        )?;

        // Add preparation info to agent context
        let slot = self.get_slot_mut_by_id(agent_id)?;
        slot.add_context_message(format!(
            "Git Flow Preparation Complete:\n- Branch: {}\n- Base: {}\n- Path: {}",
            prep_result.branch_name,
            prep_result.base_commit,
            prep_result.worktree_path.display()
        ));

        // Assign task
        slot.assign_task(task);
        Ok(())
    }
}
```

### Story 4.4: Preparation Context Injection

**Description**: Inject preparation result into agent's initial context.

**Acceptance Criteria**:
- [ ] Agent receives Git Flow context on task start
- [ ] Context includes: branch info, base commit, warnings
- [ ] Context is visible in agent transcript
- [ ] Helps agent understand Git state
- [ ] Unit tests

**Context Message Format**:
```
=== Git Flow Task Preparation ===

Task: PROJ-123 (Add user authentication)
Branch: feature/PROJ-123-add-user-auth
Base Commit: abc123def (origin/main as of 2025-04-19)

Workspace Status:
- Clean worktree (no uncommitted changes)
- Starting from latest main

Notes:
- Follow conventional commit format: feat(auth): ...
- Run tests before claiming completion
- Commit incrementally, not all at once

Ready to begin development.
```

### Story 4.5: Preparation Decision Flow

**Description**: Define decision flow for preparation scenarios.

**Acceptance Criteria**:
- [ ] Rule engine handles preparation situations
- [ ] Automatic decisions for clean workspace
- [ ] Escalates to human for complex scenarios
- [ ] Logs all preparation decisions
- [ ] Integration tests

**Decision Flow**:
```
TaskPreparationSituation received:
│
├─ workspace_health.score >= 80?
│  └─ Yes: Auto-execute PrepareNewBranchAction
│
├─ has_uncommitted AND config.auto_stash?
│  └─ Yes: Auto-stash, then PrepareNewBranchAction
│
├─ existing_branch_at_main_head?
│  └─ Yes: Auto-execute ReuseExistingBranchAction
│
├─ existing_branch_behind_main?
│  └─ Yes: Auto-execute HandleUncommittedFirst → Rebase
│
└─ Otherwise:
│  └─ RequestHumanGuidanceAction
```

**Rule Engine Integration**:
```rust
// In rule_engine.rs
pub fn register_git_flow_rules(registry: &RuleRegistry) {
    registry.register(Rule {
        name: "auto-prepare-clean-workspace",
        condition: |situation| {
            if let Some(s) = situation.downcast::<TaskPreparationSituation>() {
                s.workspace_health.as_ref().map(|h| h.score >= 80).unwrap_or(false)
            } else {
                false
            }
        },
        action: |_| Box::new(PrepareNewBranchAction::default()),
        priority: RulePriority::High,
    });

    registry.register(Rule {
        name: "auto-stash-uncommitted",
        condition: |situation| {
            if let Some(s) = situation.downcast::<TaskPreparationSituation>() {
                s.has_uncommitted && s.config.auto_stash_changes
            } else {
                false
            }
        },
        action: |_| Box::new(HandleUncommittedFirstAction {
            policy: UncommittedHandlingPolicy::AutoStash,
        }),
        priority: RulePriority::High,
    });
}
```

## Dependencies

- Sprint 1-3 (All infrastructure)
- Decision layer framework
- AgentPool task assignment flow

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Preparation delays task start | Timeout with fallback |
| Decision layer overload | Async preparation |
| Context injection breaks agent | Graceful degradation |

## Testing Strategy

- Unit tests for situation/action
- Integration tests with AgentPool
- Decision flow simulation tests
- End-to-end task assignment tests

## Deliverables

1. New situation in `decision/src/builtin_situations.rs`
2. New actions in `decision/src/builtin_actions.rs`
3. AgentPool integration in `core/src/agent_pool.rs`
4. Rule engine updates in `decision/src/rule_engine.rs`
5. Integration tests

---

**Sprint Status**: Planned
