# Sprint 5: Integration

## Goal

Integrate all P0 components (TaskMeta, GitState, BranchSync, UncommittedHandler) into the decision layer flow.

## Duration

3-4 days

## Stories

### Story 5.1: PrepareTaskStart Orchestrator

**Description**: Create the main orchestrator that combines all preparation steps.

**Acceptance Criteria**:
- Coordinates TaskMeta extraction → GitState analysis → Uncommitted handling → Branch creation
- Handles each step's errors gracefully
- Returns final preparation result

**Implementation**:
```rust
// In decision/src/task_preparation.rs
pub struct TaskPreparationPipeline {
    meta_extractor: TaskMetaExtractor,
    state_analyzer: GitStateAnalyzer,
    uncommitted_handler: UncommittedChangesClassifier,
    branch_setup: BranchSetupAction,
}

impl TaskPreparationPipeline {
    pub fn prepare(&self, request: &TaskPreparationRequest) -> TaskPreparationResult {
        // Step 1: Extract task metadata
        let task_meta = self.meta_extractor.extract(
            &request.task_description,
            request.task_id.as_deref(),
        );
        
        // Step 2: Analyze current git state
        let git_state = self.state_analyzer.analyze(&request.worktree_path)?;
        
        // Step 3: Handle uncommitted changes if present
        if git_state.has_uncommitted {
            let evaluation = self.uncommitted_handler.classify(
                &git_state.uncommitted_files,
                &request.context,
            );
            
            // Create appropriate action
            let uncommitted_action = self.create_uncommitted_action(&evaluation);
            
            // Execute action before proceeding
            // (This happens in AgentPool, not here)
        }
        
        // Step 4: Sync/create branch
        if self.needs_branch_setup(&git_state, &task_meta) {
            self.branch_setup.execute(&request.worktree_path, &task_meta)?;
        }
        
        TaskPreparationResult::Ready {
            task_meta,
            branch_ready: true,
            clean_state: true,
        }
    }
}
```

---

### Story 5.2: TaskPreparationRequest/Result Types

**Description**: Define request and result types for preparation pipeline.

**Acceptance Criteria**:
- Request includes all needed inputs
- Result provides actionable information
- Both are serializable

**Implementation**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPreparationRequest {
    /// Task description from backlog
    pub task_description: String,
    /// Task ID from backlog (if available)
    pub task_id: Option<String>,
    /// Worktree path for the agent
    pub worktree_path: PathBuf,
    /// Agent ID
    pub agent_id: String,
    /// Decision context (existing)
    pub context: DecisionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPreparationResult {
    /// All preparation steps succeeded
    Ready {
        task_meta: TaskMeta,
        branch_ready: bool,
        clean_state: bool,
    },
    
    /// Needs uncommitted changes handling first
    NeedsUncommittedHandling {
        evaluation: UncommittedChangesEvaluation,
        task_meta: TaskMeta,
    },
    
    /// Needs rebase/conflict resolution
    NeedsSync {
        git_state: GitState,
        task_meta: TaskMeta,
    },
    
    /// Needs human intervention
    NeedsHuman {
        reason: String,
        context: Box<dyn DecisionSituation>,
    },
    
    /// Preparation failed
    Failed {
        error: String,
        step: PreparationStep,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationStep {
    MetaExtraction,
    GitStateAnalysis,
    UncommittedHandling,
    BranchSetup,
    FinalVerification,
}
```

---

### Story 5.3: AgentPool Integration

**Description**: Integrate preparation pipeline into AgentPool task assignment flow.

**Acceptance Criteria**:
- Call preparation before task assignment
- Execute uncommitted actions
- Execute branch setup
- Transition agent appropriately

**Implementation**:
```rust
// In core/src/agent_pool.rs
impl AgentPool {
    /// Assign task with git preparation
    pub fn assign_task_with_preparation(
        &mut self,
        agent_id: &AgentId,
        task_description: &str,
        task_id: Option<&str>,
    ) -> Result<TaskAssignmentResult, AgentPoolError> {
        let slot = self.get_slot_by_id(agent_id)?;
        let worktree_path = slot.cwd();
        
        // Create preparation request
        let request = TaskPreparationRequest {
            task_description: task_description.to_string(),
            task_id: task_id.map(|s| s.to_string()),
            worktree_path,
            agent_id: agent_id.as_str().to_string(),
            context: DecisionContext::default(),
        };
        
        // Run preparation pipeline
        let preparation = self.task_preparation_pipeline.prepare(&request)?;
        
        match preparation {
            TaskPreparationResult::Ready { task_meta, .. } => {
                // Agent is ready to start task
                self.start_task_on_agent(agent_id, task_description, &task_meta)?;
                Ok(TaskAssignmentResult::Started)
            }
            
            TaskPreparationResult::NeedsUncommittedHandling { evaluation, task_meta } => {
                // Handle uncommitted changes
                let action = self.create_uncommitted_action(&evaluation);
                self.execute_handle_uncommitted(agent_id, &action)?;
                
                // After handling, proceed with branch setup
                self.setup_branch_for_task(agent_id, &task_meta)?;
                self.start_task_on_agent(agent_id, task_description, &task_meta)?;
                Ok(TaskAssignmentResult::Started)
            }
            
            TaskPreparationResult::NeedsSync { git_state, task_meta } => {
                // Sync branch
                self.sync_branch_for_task(agent_id, &git_state)?;
                self.setup_branch_for_task(agent_id, &task_meta)?;
                self.start_task_on_agent(agent_id, task_description, &task_meta)?;
                Ok(TaskAssignmentResult::Started)
            }
            
            TaskPreparationResult::NeedsHuman { reason, .. } => {
                // Block agent for human decision
                self.block_agent_for_human(agent_id, reason)?;
                Ok(TaskAssignmentResult::Blocked)
            }
            
            TaskPreparationResult::Failed { error, step } => {
                Err(AgentPoolError::PreparationFailed(error, step))
            }
        }
    }
}
```

---

### Story 5.4: Decision Layer Trigger for Task Start

**Description**: Create trigger mechanism for task starting situations.

**Acceptance Criteria**:
- Trigger when new task assigned
- Send preparation context to decision agent
- Receive decision for preparation actions

**Implementation**:
```rust
// In core/src/agent_pool.rs or app_loop.rs
/// Trigger task preparation decision
fn trigger_task_preparation_decision(
    &mut self,
    agent_id: &AgentId,
    task_description: &str,
) -> Result<(), String> {
    // Create TaskStarting situation
    let situation = TaskStartingSituation {
        task_description: task_description.to_string(),
        task_id: None,
        task_meta: None,  // Will be filled by decision layer
        git_state: None,  // Will be filled by decision layer
    };
    
    // Create decision request
    let context = DecisionContext::new(Box::new(situation), agent_id.as_str());
    let request = DecisionRequest::new(
        agent_id.clone(),
        task_starting(),
        context,
    );
    
    // Send to decision agent
    if let Some(sender) = self.decision_mail_senders.get(agent_id) {
        sender.send_request(request)?;
        
        // Block agent for decision
        self.transition_agent_to(agent_id, AgentSlotStatus::blocked_for_decision())?;
    }
    
    Ok(())
}
```

---

### Story 5.5: PrepareTaskStartAction Registration

**Description**: Register the full preparation action.

**Acceptance Criteria**:
- Registered in action registry
- Handles all preparation steps
- Returns proper action format

**Implementation**:
```rust
// In decision/src/builtin_actions.rs
pub fn prepare_task_start() -> ActionType {
    ActionType::new("prepare_task_start")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareTaskStartAction {
    /// Extracted task metadata
    pub task_meta: TaskMeta,
    /// Actions to execute before starting
    pub pre_actions: Vec<PreAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreAction {
    HandleUncommitted(HandleUncommittedAction),
    CreateBranch(CreateTaskBranchAction),
    RebaseToMain,
}

impl DecisionAction for PrepareTaskStartAction {
    fn action_type(&self) -> ActionType {
        prepare_task_start()
    }
    
    fn to_prompt_format(&self) -> String {
        format!(
            "PrepareTaskStart: branch={}\nPre-actions: {:?}",
            self.task_meta.branch_name,
            self.pre_actions.iter().map(|a| match a {
                PreAction::HandleUncommitted(_) => "HandleUncommitted",
                PreAction::CreateBranch(_) => "CreateBranch",
                PreAction::RebaseToMain => "RebaseToMain",
            }).collect::<Vec<_>>()
        )
    }
}
```

---

### Story 5.6: End-to-End Test Scenarios

**Description**: Create comprehensive integration tests.

**Acceptance Criteria**:
- Test clean task start flow
- Test uncommitted → commit → start flow
- Test outdated branch → rebase → start flow
- Test collision → suffix → start flow
- Test error handling scenarios

**Test Scenarios**:
```rust
#[test]
fn test_clean_task_start() {
    // Setup: clean worktree, no uncommitted, on main
    let pool = AgentPool::new_with_worktrees(...);
    let agent_id = pool.spawn_agent_with_worktree(Claude, None, None)?;
    
    // Assign task
    let result = pool.assign_task_with_preparation(&agent_id, 
        "Add user authentication feature", None)?;
    
    assert_eq!(result, TaskAssignmentResult::Started);
    
    // Verify branch created
    let slot = pool.get_slot_by_id(&agent_id)?;
    assert!(slot.branch().starts_with("feature/"));
}

#[test]
fn test_uncommitted_then_start() {
    // Setup: worktree with modified src/main.rs
    // Create some changes
    std::fs::write(worktree_path.join("src/main.rs"), "modified content")?;
    
    let result = pool.assign_task_with_preparation(&agent_id, 
        "Fix login bug", None)?;
    
    assert_eq!(result, TaskAssignmentResult::Started);
    
    // Verify uncommitted was handled (committed or stashed)
    // Verify branch is fix/login-bug
}

#[test]
fn test_outdated_branch_rebase() {
    // Setup: branch behind main by 3 commits
    // Create commits on main, then assign task to agent on old branch
    
    let result = pool.assign_task_with_preparation(&agent_id, 
        "Continue work", Some("task-123"))?;
    
    // Verify rebase happened, then task started
}
```

---

### Story 5.7: Configuration Support

**Description**: Add configuration for preparation behavior.

**Acceptance Criteria**:
- Configurable base branch (main/master)
- Configurable branch naming format
- Configurable auto-commit thresholds
- Persisted in global config

**Implementation**:
```rust
// In core/src/global_config.rs or new file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFlowConfig {
    /// Base branch name (main or master)
    pub base_branch: String,
    
    /// Branch naming format: {type}/{slug} or other
    pub branch_format: String,
    
    /// Auto-commit threshold (significance score)
    pub auto_commit_threshold: u8,
    
    /// Require human approval for risk levels >= this
    pub human_approval_threshold: RiskLevel,
    
    /// Enable/disable git-flow preparation
    pub enabled: bool,
    
    /// Stash by default for uncommitted (vs commit)
    pub stash_by_default: bool,
}

impl Default for GitFlowConfig {
    fn default() -> Self {
        Self {
            base_branch: "main".to_string(),
            branch_format: "{type}/{slug}".to_string(),
            auto_commit_threshold: 50,
            human_approval_threshold: RiskLevel::High,
            enabled: true,
            stash_by_default: false,
        }
    }
}
```

---

## Integration Points

- `decision/src/task_preparation.rs`: New orchestrator module
- `core/src/agent_pool.rs`: Integration with task assignment
- `tui/src/app_loop.rs`: Trigger preparation on task events
- `core/src/global_config.rs`: Configuration

## Dependencies

- Sprint 1-4 all completed
- Existing decision layer flow
- Existing WorktreeManager

## Risks

- Integration complexity (mitigate: incremental integration, extensive tests)
- Breaking existing flows (mitigate: feature flag, fallback to old behavior)

## Definition of Done

- [ ] TaskPreparationPipeline implemented
- [ ] Request/Result types defined
- [ ] AgentPool integration complete
- [ ] Decision trigger mechanism working
- [ ] PrepareTaskStartAction registered
- [ ] End-to-end tests passing
- [ ] Configuration support added
- [ ] Code reviewed
- [ ] Documentation updated
