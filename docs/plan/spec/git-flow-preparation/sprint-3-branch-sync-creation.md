# Sprint 3: Branch Sync & Creation

## Goal

Implement branch synchronization with main/master and task-specific branch creation based on TaskMeta.

## Duration

3-4 days

## Stories

### Story 3.1: Extend WorktreeManager with Sync Operations

**Description**: Add branch sync and creation operations to WorktreeManager.

**Acceptance Criteria**:
- Can fetch latest from origin
- Can create branch from specific base
- Can rebase current branch onto main
- Handles branch name collisions

**Implementation**:
```rust
// In core/src/worktree_manager.rs - extend with new methods
impl WorktreeManager {
    /// Fetch latest from origin for base branch
    pub fn fetch_base_branch(&self, base_branch: &str) -> Result<String, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        
        // Fetch from origin
        self.run_git_command_internal(&["fetch", "origin", base_branch])?;
        
        // Get the fetched HEAD SHA
        let sha = self.run_git_command_internal(&["rev-parse", &format!("origin/{}", base_branch)])?;
        Ok(sha.trim().to_string())
    }
    
    /// Create a new branch from base branch HEAD for a task
    pub fn create_task_branch(
        &self,
        worktree_path: &Path,
        branch_name: &str,
        base_branch: &str,
    ) -> Result<(), WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        
        // Check if branch already exists
        if self.branch_exists_internal(branch_name)? {
            // Check if it's checked out elsewhere
            if let Some(location) = self.branch_checkout_location(branch_name)? {
                return Err(WorktreeError::BranchAlreadyCheckedOut(location));
            }
            // Branch exists but not checked out - can checkout
            self.run_git_command_internal_in_worktree(worktree_path, 
                &["checkout", branch_name])?;
        } else {
            // Create new branch from base
            self.run_git_command_internal_in_worktree(worktree_path,
                &["checkout", "-b", branch_name, &format!("origin/{}", base_branch)])?;
        }
        
        Ok(())
    }
    
    /// Rebase worktree branch onto base branch
    pub fn rebase_to_base(
        &self,
        worktree_path: &Path,
        base_branch: &str,
    ) -> Result<RebaseResult, WorktreeError> {
        let _lock = self.git_lock.lock().unwrap();
        
        // Fetch latest first
        self.run_git_command_internal(&["fetch", "origin", base_branch])?;
        
        // Attempt rebase
        let result = self.run_git_command_internal_in_worktree(worktree_path,
            &["rebase", &format!("origin/{}", base_branch)]);
        
        match result {
            Ok(_) => Ok(RebaseResult::Success),
            Err(e) => {
                // Check if it's a conflict error
                if self.has_rebase_conflicts(worktree_path)? {
                    Ok(RebaseResult::Conflicts)
                } else {
                    Err(e)
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseResult {
    Success,
    Conflicts,
    Aborted,
}
```

---

### Story 3.2: BranchSetupAction Implementation

**Description**: Create decision action for branch setup.

**Acceptance Criteria**:
- Registered as new action type
- Uses TaskMeta for branch name
- Uses WorktreeManager for operations
- Returns appropriate result for next step

**Implementation**:
```rust
// In decision/src/builtin_actions.rs - add new action
pub fn create_task_branch() -> ActionType {
    ActionType::new("create_task_branch")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskBranchAction {
    /// Branch name from TaskMeta
    pub branch_name: String,
    /// Base branch (main or master)
    pub base_branch: String,
    /// Whether to rebase if branch exists
    pub rebase_if_needed: bool,
}

impl DecisionAction for CreateTaskBranchAction {
    fn action_type(&self) -> ActionType {
        create_task_branch()
    }
    
    fn to_prompt_format(&self) -> String {
        format!("CreateBranch: {} from {}", self.branch_name, self.base_branch)
    }
    
    fn serialize_params(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}
```

---

### Story 3.3: Branch Name Collision Handling

**Description**: Handle cases where desired branch name already exists.

**Acceptance Criteria**:
- Detect existing branches
- Generate unique suffix if collision
- Provide options: use existing, create with suffix, error if checked out elsewhere

**Implementation**:
```rust
impl BranchNameGenerator {
    /// Generate unique branch name, handling collisions
    pub fn generate_unique(&self, worktree_manager: &WorktreeManager, 
                           task_description: &str, work_type: WorkType) -> Result<String, WorktreeError> {
        let base_name = self.generate(task_description, work_type);
        
        // Check if branch exists
        if !worktree_manager.branch_exists(&base_name)? {
            return Ok(base_name);
        }
        
        // Check if it's checked out elsewhere
        if let Some(location) = worktree_manager.branch_checkout_location(&base_name)? {
            // Cannot use this branch - need alternative
            return self.generate_with_suffix(worktree_manager, &base_name);
        }
        
        // Branch exists but not checked out - could be from previous aborted task
        // Decision: use existing or create new?
        Ok(base_name) // Or return collision info for decision
    }
    
    fn generate_with_suffix(&self, worktree_manager: &WorktreeManager, 
                            base_name: &str) -> Result<String, WorktreeError> {
        for suffix in 2..100 {
            let candidate = format!("{}-{}", base_name, suffix);
            if !worktree_manager.branch_exists(&candidate)? {
                return Ok(candidate);
            }
        }
        Err(WorktreeError::BranchCollision(base_name.to_string()))
    }
}
```

---

### Story 3.4: BranchSetupDecisionContext

**Description**: Create context data for branch setup decision.

**Acceptance Criteria**:
- Includes TaskMeta and GitState
- Provides options for branch setup
- Works with existing decision framework

**Implementation**:
```rust
// In decision/src/context.rs or new file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSetupContext {
    /// Task metadata with desired branch name
    pub task_meta: TaskMeta,
    /// Current git state
    pub git_state: GitState,
    /// Whether rebase is needed
    pub needs_rebase: bool,
    /// Existing branch collision info
    pub collision_info: Option<BranchCollisionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchCollisionInfo {
    pub existing_branch: String,
    pub checked_out_elsewhere: Option<PathBuf>,
    pub suggested_alternative: Option<String>,
}
```

---

### Story 3.5: TaskStarting Situation

**Description**: Create new situation for task start preparation.

**Acceptance Criteria**:
- Registered as new situation type
- Triggers when new task assigned to agent
- Provides branch setup context

**Implementation**:
```rust
// In decision/src/builtin_situations.rs
pub fn task_starting() -> SituationType {
    SituationType::new("task_starting")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStartingSituation {
    /// Task description
    pub task_description: String,
    /// Task ID from backlog (if available)
    pub task_id: Option<String>,
    /// Extracted task metadata
    pub task_meta: Option<TaskMeta>,
    /// Current git state
    pub git_state: Option<GitState>,
}

impl DecisionSituation for TaskStartingSituation {
    fn situation_type(&self) -> SituationType {
        task_starting()
    }
    
    fn requires_human(&self) -> bool {
        // Only if there are conflicts or critical decisions
        self.git_state.as_ref().map(|g| g.has_conflicts).unwrap_or(false)
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("prepare_task_start"),
            ActionType::new("create_task_branch"),
            ActionType::new("rebase_to_main"),
            ActionType::new("request_human"),
        ]
    }
    
    fn to_prompt_text(&self) -> String {
        format!(
            "Task starting: {}\nTask meta: {}\nGit state: {}",
            self.task_description,
            self.task_meta.map(|m| m.branch_name).unwrap_or_default(),
            self.git_state.map(|g| g.current_branch).unwrap_or_default(),
        )
    }
}
```

---

### Story 3.6: Unit Tests

**Description**: Comprehensive unit tests for branch sync and creation.

**Acceptance Criteria**:
- Test fetch and create operations
- Test rebase success and conflict scenarios
- Test collision handling
- Test action serialization

**Test Cases**:
```rust
#[test]
fn test_create_task_branch_from_base() {
    // Setup: create test repo with main branch
    let manager = WorktreeManager::new(repo_path, config);
    
    // Test: create new branch from main
    let result = manager.create_task_branch(worktree_path, "feature/test", "main");
    assert!(result.is_ok());
    
    // Verify branch exists and points to main HEAD
}

#[test]
fn test_branch_collision_handling() {
    let gen = BranchNameGenerator::default();
    
    // Test: existing branch gets suffix
    let unique_name = gen.generate_unique(&manager, "Add auth", WorkType::Feature);
    // Should return feature/add-auth-2 if feature/add-auth exists
}

#[test]
fn test_rebase_result_success() {
    let result = manager.rebase_to_base(worktree_path, "main");
    assert_eq!(result.unwrap(), RebaseResult::Success);
}

#[test]
fn test_rebase_result_conflicts() {
    // Setup: create conflicting changes
    let result = manager.rebase_to_base(worktree_path, "main");
    assert_eq!(result.unwrap(), RebaseResult::Conflicts);
}
```

---

## Integration Points

- `core/src/worktree_manager.rs`: Extend with new methods
- `decision/src/builtin_actions.rs`: Register new action
- `decision/src/builtin_situations.rs`: Register new situation
- `decision/src/context.rs`: Add BranchSetupContext

## Dependencies

- Sprint 1 (TaskMeta) - for branch name generation
- Sprint 2 (GitState) - for state analysis before operations
- Existing WorktreeManager

## Risks

- Rebase conflicts in complex cases (mitigate: Sprint 8 handles resolution)
- Git remote not configured (mitigate: fallback to local main)

## Definition of Done

- [ ] WorktreeManager extended with sync operations
- [ ] CreateTaskBranchAction implemented
- [ ] Branch name collision handling implemented
- [ ] TaskStartingSituation defined
- [ ] Unit tests passing
- [ ] Integration tests with mock repos
- [ ] Code reviewed
