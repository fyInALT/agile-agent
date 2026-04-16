# Sprint 2: Agent Integration with Persistence

## Metadata

- Sprint ID: `worktree-sprint-02`
- Title: `Agent Integration with Persistence`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: [Sprint 1: Infrastructure](./sprint-1-infrastructure.md)
- Design Reference: `docs/plan/worktree/worktree-integration-research.md`

## Sprint Goal

Integrate WorktreeManager into AgentPool with full persistence support. Enable seamless agent resume by persisting worktree state, verifying worktree existence on resume, and recreating worktrees if missing. This sprint is CRITICAL for the worktree feature - it connects the infrastructure to the agent lifecycle.

## Stories

### Story 2.1: AgentSlot Worktree Fields

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Add worktree-related fields to AgentSlot struct.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Add `worktree_path: Option<PathBuf>` to AgentSlot | Todo | - |
| T2.1.2 | Add `worktree_branch: Option<String>` to AgentSlot | Todo | - |
| T2.1.3 | Add `worktree_id: Option<String>` to AgentSlot | Todo | - |
| T2.1.4 | Implement `set_worktree()` method | Todo | - |
| T2.1.5 | Implement `cwd()` method returning worktree path or default | Todo | - |
| T2.1.6 | Implement `clear_worktree()` method | Todo | - |
| T2.1.7 | Update AgentSlot serialization to include worktree fields | Todo | - |
| T2.1.8 | Write unit tests for AgentSlot worktree methods | Todo | - |

#### Technical Design

```rust
pub struct AgentSlot {
    // ... existing fields ...
    
    /// Worktree path (if using independent worktree)
    worktree_path: Option<PathBuf>,
    
    /// Worktree branch name
    worktree_branch: Option<String>,
    
    /// Worktree unique ID
    worktree_id: Option<String>,
}

impl AgentSlot {
    /// Get agent's working directory
    pub fn cwd(&self) -> PathBuf {
        self.worktree_path.clone().unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
    }
    
    /// Set worktree information
    pub fn set_worktree(&mut self, path: PathBuf, branch: Option<String>) {
        self.worktree_path = Some(path);
        self.worktree_branch = branch;
        self.worktree_id = Some(generate_worktree_id());
    }
    
    /// Clear worktree information
    pub fn clear_worktree(&mut self) {
        self.worktree_path = None;
        self.worktree_branch = None;
        self.worktree_id = None;
    }
}
```

---

### Story 2.2: AgentPool Worktree Integration

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Integrate WorktreeManager into AgentPool for agent spawning.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Add `worktree_manager: Option<WorktreeManager>` to AgentPool | Todo | - |
| T2.2.2 | Add `worktree_state_store: WorktreeStateStore` to AgentPool | Todo | - |
| T2.2.3 | Implement `AgentPool::new_with_worktrees()` constructor | Todo | - |
| T2.2.4 | Implement `spawn_agent_with_worktree()` method | Todo | - |
| T2.2.5 | Implement `create_for_agent()` helper in WorktreeManager | Todo | - |
| T2.2.6 | Implement branch naming logic (agent/{task-id}) | Todo | - |
| T2.2.7 | Implement default branch fallback when no task_id | Todo | - |
| T2.2.8 | Ensure .worktrees directory creation | Todo | - |
| T2.2.9 | Write unit tests for spawn with worktree | Todo | - |
| T2.2.10 | Write integration tests for full lifecycle | Todo | - |

#### Acceptance Criteria

- New agent automatically gets worktree with persisted state
- Agent runs in correct cwd (worktree path)
- Multiple agents can spawn with different worktrees
- Worktree state is persisted immediately on spawn

#### Technical Design

```rust
impl AgentPool {
    pub fn new_with_worktrees(
        workplace_id: WorkplaceId,
        max_slots: usize,
        repo_root: PathBuf,
    ) -> Result<Self, WorktreeError> {
        let worktree_manager = WorktreeManager::new(
            repo_root,
            "agent".to_string(),
        )?;
        
        let state_dir = repo_root.join(".state");
        let worktree_state_store = WorktreeStateStore::new(state_dir);
        
        Ok(Self {
            slots: Vec::new(),
            max_slots,
            // ... other fields ...
            worktree_manager: Some(worktree_manager),
            worktree_state_store,
        })
    }
    
    pub fn spawn_agent_with_worktree(
        &mut self,
        provider_kind: ProviderKind,
        task_id: Option<&str>,
    ) -> Result<AgentId, String> {
        // Create worktree and persist state
    }
}
```

---

### Story 2.3: Agent Pause with Worktree Preservation

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement agent pause that preserves worktree for later resume.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Implement `pause_agent()` in AgentPool | Todo | - |
| T2.3.2 | Update worktree state before pause | Todo | - |
| T2.3.3 | Query current HEAD commit from git | Todo | - |
| T2.3.4 | Record commits made by agent | Todo | - |
| T2.3.5 | Check for uncommitted changes | Todo | - |
| T2.3.6 | Update last_active_at timestamp | Todo | - |
| T2.3.7 | Persist updated state to disk | Todo | - |
| T2.3.8 | Keep worktree intact (no removal) | Todo | - |
| T2.3.9 | Write unit tests for pause operation | Todo | - |

#### Acceptance Criteria

- Paused agent preserves worktree state
- State includes current HEAD, commits, uncommitted status
- Worktree directory remains intact
- Agent can be resumed later

---

### Story 2.4: Agent Resume with Worktree Verification

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement agent resume with worktree existence check and recreation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Implement `resume_agent()` in AgentPool | Todo | - |
| T2.4.2 | Load persisted worktree state from disk | Todo | - |
| T2.4.3 | Check worktree directory existence | Todo | - |
| T2.4.4 | Implement `resume_existing_worktree()` for valid worktree | Todo | - |
| T2.4.5 | Verify branch matches stored state | Todo | - |
| T2.4.6 | Implement `recreate_worktree()` for missing worktree | Todo | - |
| T2.4.7 | Check if branch still exists in repo | Todo | - |
| T2.4.8 | Create worktree from base_commit if branch lost | Todo | - |
| T2.4.9 | Define `ResumeResult` enum (ExistingWorktree, RecreatedWorktree) | Todo | - |
| T2.4.10 | Define `ResumeError` enum | Todo | - |
| T2.4.11 | Start provider with correct cwd after resume | Todo | - |
| T2.4.12 | Write unit tests for resume with existing worktree | Todo | - |
| T2.4.13 | Write unit tests for resume with recreated worktree | Todo | - |
| T2.4.14 | Write integration tests for full resume cycle | Todo | - |

#### Resume Flow

```
1. Load Agent State File
   worktree_state_store.load(agent_id)
   
2. Check Worktree Existence
   worktree_state.exists()?
   
   ├─ YES ────────────────────────────────┐
   │  Verify Worktree Validity            │
   │  - Branch still exists?              │
   │  - HEAD matches stored head_commit?  │
   │                                      │
   │  Resume in Existing Worktree         │
   │  agent_slot.cwd = worktree_state.path│
   │                                      │
   NO ────────────────────────────────────▶│
   │                                      │
   │  Recreate Worktree                   │
   │  worktree_manager.create_from_state()│
   │                                      │
   │  Restore Context                     │
   │  - Re-create branch if needed        │
   │  - Cherry-pick commits if lost       │
   │                                      │
   ▼                                      ▼
6. Provider Resume
   provider.start(prompt, cwd=worktree_path, session_handle=stored_session)
```

#### Technical Design

```rust
pub enum ResumeResult {
    ExistingWorktree,
    RecreatedWorktree,
}

#[derive(Debug, thiserror::Error)]
pub enum ResumeError {
    #[error("no worktree state found for agent: {0}")]
    NoWorktreeState(AgentId),
    
    #[error("missing branch info in worktree state")]
    MissingBranchInfo,
    
    #[error("worktree recreation failed: {0}")]
    RecreationFailed(String),
}

impl AgentPool {
    pub fn resume_agent(
        &mut self,
        agent_id: &AgentId,
    ) -> Result<ResumeResult, ResumeError> {
        // Load, verify, recreate if needed
    }
}
```

---

### Story 2.5: Agent Stop with Worktree Cleanup

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement agent stop with optional worktree cleanup.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.5.1 | Implement `stop_agent_with_cleanup()` in AgentPool | Todo | - |
| T2.5.2 | Add `cleanup_worktree: bool` parameter | Todo | - |
| T2.5.3 | When cleanup=true: remove worktree directory | Todo | - |
| T2.5.4 | When cleanup=true: delete persisted state | Todo | - |
| T2.5.5 | When cleanup=false: update state and keep worktree | Todo | - |
| T2.5.6 | Handle force removal for worktrees with uncommitted changes | Todo | - |
| T2.5.7 | Update AgentSlot status to Stopped | Todo | - |
| T2.5.8 | Clear worktree fields in AgentSlot | Todo | - |
| T2.5.9 | Write unit tests for stop with cleanup | Todo | - |
| T2.5.10 | Write unit tests for stop without cleanup | Todo | - |

#### Acceptance Criteria

- Stopped agent can optionally cleanup worktree
- cleanup=true removes worktree and deletes state
- cleanup=false preserves worktree for potential future resume
- Force removal works for worktrees with uncommitted changes

---

### Story 2.6: Provider Cwd Propagation

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Ensure providers receive correct cwd from agent slot.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.6.1 | Verify Claude provider uses cwd correctly | Todo | - |
| T2.6.2 | Verify Codex provider uses cwd correctly | Todo | - |
| T2.6.3 | Add cwd parameter to provider start methods | Todo | - |
| T2.6.4 | Document cwd propagation path | Todo | - |
| T2.6.5 | Write integration tests for provider cwd | Todo | - |

#### Cwd Propagation Path

```
AgentPool.spawn_agent_with_worktree()
        │
        ▼
WorktreeManager.create_for_agent()
        │
        ▼
AgentSlot.worktree_path = PathBuf::from(".worktrees/agent-001")
        │
        ▼
AgentSlot.cwd() -> PathBuf
        │
        ▼
provider::start(prompt, cwd, ...)
        │
        ▼
Command::new().current_dir(&cwd)
        │
        ▼
Claude/Codex/OpenCode runs in worktree directory
```

#### Key Finding

All providers (Claude, Codex) already support `current_dir()`:
- Claude: `command.current_dir(&cwd)` in existing code
- Codex: `command.current_dir(&cwd)` in existing code
- No provider modifications needed!

---

### Story 2.7: Worktree State Query Methods

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Add methods to query worktree state from git.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.7.1 | Implement `get_current_head()` in WorktreeManager | Todo | - |
| T2.7.2 | Implement `get_commits_since()` for agent commits | Todo | - |
| T2.7.3 | Implement `has_uncommitted_changes()` check | Todo | - |
| T2.7.4 | Implement `branch_exists()` check | Todo | - |
| T2.7.5 | Implement `get_base_commit()` for default branch | Todo | - |
| T2.7.6 | Write unit tests for query methods | Todo | - |

---

### Story 2.8: Integration Tests for Full Lifecycle

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Comprehensive integration tests covering spawn → pause → resume → stop.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.8.1 | Create test fixture with real git repository | Todo | - |
| T2.8.2 | Test spawn with worktree creation | Todo | - |
| T2.8.3 | Test pause preserves worktree | Todo | - |
| T2.8.4 | Test resume with existing worktree | Todo | - |
| T2.8.5 | Test resume recreates missing worktree | Todo | - |
| T2.8.6 | Test stop with cleanup removes worktree | Todo | - |
| T2.8.7 | Test stop without cleanup preserves worktree | Todo | - |
| T2.8.8 | Test multiple agents with different worktrees | Todo | - |
| T2.8.9 | Test provider runs in correct cwd | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Resume state corruption | Medium | High | Validate state on load, have fallback recreation |
| Branch conflicts on recreate | Low | Medium | Use base_commit as fallback, cherry-pick if needed |
| Provider cwd mismatch | Low | High | Integration tests verify cwd propagation |
| Race condition in parallel spawn | Low | Medium | Use mutex for git operations |

## Sprint Deliverables

- Modified `core/src/agent_slot.rs` with worktree fields
- Modified `core/src/agent_pool.rs` with worktree integration
- `core/src/worktree_manager.rs` extended with agent-specific methods
- Resume flow implementation (existing/recreated)
- Stop flow implementation (cleanup/preserve)
- Integration tests for full lifecycle

## Dependencies

- Sprint 1: Infrastructure (WorktreeManager, WorktreeState)
- Existing AgentSlot and AgentPool modules
- Git CLI for worktree operations

## Next Sprint

After completing this sprint, proceed to [Sprint 3: TUI Display](./sprint-3-tui-display.md) for worktree status visualization.