# Sprint 1: Worktree Infrastructure

## Metadata

- Sprint ID: `worktree-sprint-01`
- Title: `Worktree Infrastructure`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: None
- Design Reference: `docs/plan/worktree/worktree-integration-research.md`

## Sprint Goal

Build the WorktreeManager core infrastructure with porcelain parsing and persistence layer. Establish foundational types (WorktreeInfo, WorktreeState, WorktreeCreateOptions) and storage mechanisms that enable agent resume functionality.

## Stories

### Story 1.1: WorktreeInfo and WorktreeError Types

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define core data types for worktree information and error handling.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `core/src/worktree_manager.rs` module file | Todo | - |
| T1.1.2 | Define `WorktreeInfo` struct with all fields | Todo | - |
| T1.1.3 | Define `WorktreeError` enum with thiserror | Todo | - |
| T1.1.4 | Implement `Serialize/Deserialize` for WorktreeInfo | Todo | - |
| T1.1.5 | Write unit tests for WorktreeInfo construction | Todo | - |
| T1.1.6 | Write unit tests for WorktreeError variants | Todo | - |

#### Technical Design

```rust
/// Worktree status information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub is_detached: bool,
    pub is_locked: bool,
    pub lock_reason: Option<String>,
    pub is_prunable: bool,
    pub prune_reason: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    #[error("not a git repository: {0}")]
    NotAGitRepository(PathBuf),
    
    #[error("worktree not found: {0}")]
    WorktreeNotFound(PathBuf),
    
    #[error("git command failed: {0}")]
    GitCommandFailed(String),
    
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("porcelain parse error: {0}")]
    ParseError(String),
}
```

---

### Story 1.2: WorktreeCreateOptions Type

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Define configuration options for worktree creation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Define `WorktreeCreateOptions` struct | Todo | - |
| T1.2.2 | Add path, branch, create_branch, base fields | Todo | - |
| T1.2.3 | Add lock_reason optional field | Todo | - |
| T1.2.4 | Implement Default trait | Todo | - |
| T1.2.5 | Implement builder pattern methods | Todo | - |
| T1.2.6 | Write unit tests for options construction | Todo | - |

#### Technical Design

```rust
/// Worktree creation options
#[derive(Debug, Clone, Default)]
pub struct WorktreeCreateOptions {
    /// Worktree path (relative to repo root or absolute)
    pub path: PathBuf,
    /// Branch name (None means detached HEAD)
    pub branch: Option<String>,
    /// Whether to create new branch (if branch doesn't exist)
    pub create_branch: bool,
    /// Base commit/branch to create from (only valid when create_branch=true)
    pub base: Option<String>,
    /// Lock reason (optional)
    pub lock_reason: Option<String>,
}
```

---

### Story 1.3: WorktreeManager Core Operations

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement the WorktreeManager struct with create, remove, list, and prune operations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Define `WorktreeManager` struct with repo_root, worktrees_dir, prefix | Todo | - |
| T1.3.2 | Implement `WorktreeManager::new()` with git validation | Todo | - |
| T1.3.3 | Implement `WorktreeManager::list()` with git worktree list --porcelain | Todo | - |
| T1.3.4 | Implement `parse_porcelain_output()` for git output parsing | Todo | - |
| T1.3.5 | Implement `WorktreeManager::create()` with branch creation logic | Todo | - |
| T1.3.6 | Implement `WorktreeManager::remove()` with path handling | Todo | - |
| T1.3.7 | Implement `WorktreeManager::prune()` for cleanup | Todo | - |
| T1.3.8 | Implement `run_git_command()` helper with proper error handling | Todo | - |
| T1.3.9 | Write unit tests for list operation | Todo | - |
| T1.3.10 | Write unit tests for create/remove lifecycle | Todo | - |
| T1.3.11 | Write integration tests with real git repository | Todo | - |

#### Acceptance Criteria

- Can create worktree with new branch
- Can list all worktrees with porcelain parsing
- Can remove worktree
- Can prune stale worktree records
- Test coverage > 80%

#### Technical Notes

```bash
# Expected git command outputs
git worktree list --porcelain
worktree /path/to/main
HEAD abc1234...
branch refs/heads/main

worktree /path/to/.worktrees/agent-alpha
HEAD def5678...
branch refs/heads/feature/task-123
```

---

### Story 1.4: WorktreeState Persistent Type

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the persistent worktree state structure for agent resume support.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Create `core/src/worktree_state.rs` module file | Todo | - |
| T1.4.2 | Define `WorktreeState` struct with all fields | Todo | - |
| T1.4.3 | Implement `WorktreeState::new()` constructor | Todo | - |
| T1.4.4 | Implement `touch()` method for timestamp update | Todo | - |
| T1.4.5 | Implement `record_commit()` method | Todo | - |
| T1.4.6 | Implement `exists()` method for path check | Todo | - |
| T1.4.7 | Implement `relative_path()` method | Todo | - |
| T1.4.8 | Implement Serialize/Deserialize with chrono support | Todo | - |
| T1.4.9 | Write unit tests for WorktreeState | Todo | - |

#### Technical Design

```rust
/// Persistent worktree state for agent resume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeState {
    pub worktree_id: String,
    pub path: PathBuf,
    pub branch: Option<String>,
    pub base_commit: String,
    pub task_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_active_at: chrono::DateTime<chrono::Utc>,
    pub preserve_on_completion: bool,
    pub commits: Vec<String>,
    pub head_commit: Option<String>,
    pub has_uncommitted_changes: bool,
}
```

---

### Story 1.5: WorktreeStateStore Implementation

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement the persistence store for worktree states.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.5.1 | Create `core/src/worktree_state_store.rs` module file | Todo | - |
| T1.5.2 | Define `WorktreeStateStore` struct with state_dir | Todo | - |
| T1.5.3 | Implement `WorktreeStateStore::new()` | Todo | - |
| T1.5.4 | Implement `save()` method for agent state file integration | Todo | - |
| T1.5.5 | Implement `load()` method for reading worktree state | Todo | - |
| T1.5.6 | Implement `delete()` method for state removal | Todo | - |
| T1.5.7 | Implement `list_all()` for all agents with worktrees | Todo | - |
| T1.5.8 | Define `WorktreeStateError` enum | Todo | - |
| T1.5.9 | Write unit tests for save/load operations | Todo | - |
| T1.5.10 | Write integration tests with real file system | Todo | - |

#### Storage Location

```
.state/
├── agents/
│   ├── agent_001.json        # Agent state including worktree info
│   ├── agent_002.json
│   └── ...
└── worktrees/
    └── index.json            # Global worktree index (optional)
```

#### Agent State File Structure

```json
{
  "agent_id": "agent_001",
  "codename": "alpha",
  "provider_type": "claude",
  "status": "running",
  "worktree": {
    "worktree_id": "wt-alpha-001",
    "path": "/path/to/repo/.worktrees/agent-alpha",
    "branch": "agent/task-123",
    "base_commit": "abc123...",
    "commits": ["def456...", "ghi789..."],
    "head_commit": "ghi789..."
  }
}
```

---

### Story 1.6: Porcelain Output Parser

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement robust parsing of git worktree list --porcelain output.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.6.1 | Define porcelain parsing state machine | Todo | - |
| T1.6.2 | Parse "worktree" line for path | Todo | - |
| T1.6.3 | Parse "HEAD" line for commit SHA | Todo | - |
| T1.6.4 | Parse "branch" line for branch reference | Todo | - |
| T1.6.5 | Parse "detached" marker | Todo | - |
| T1.6.6 | Parse "locked" with optional reason | Todo | - |
| T1.6.7 | Handle worktree record boundaries (blank lines) | Todo | - |
| T1.6.8 | Write parser unit tests with edge cases | Todo | - |

#### Parsing Rules

```text
Input format:
worktree /path/to/main
HEAD abc1234...
branch refs/heads/main

worktree /path/to/.worktrees/agent-alpha
HEAD def5678...
branch refs/heads/feature/task-123
locked
reason "Agent alpha working"

Output: Vec<WorktreeInfo>
```

---

### Story 1.7: Thread-Safe Git Operations

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Add synchronization for non-thread-safe git operations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.7.1 | Add `git_lock: Mutex<()>` to WorktreeManager | Todo | - |
| T1.7.2 | Document thread-safe operations table | Todo | - |
| T1.7.3 | Implement locking for git merge operations | Todo | - |
| T1.7.4 | Write concurrency tests | Todo | - |

#### Thread Safety Table

| Operation | Thread Safe | Notes |
|-----------|-------------|-------|
| `git worktree add` | ✓ | Can parallel create different worktrees |
| `git worktree remove` | ✓ | Can parallel remove different worktrees |
| `git checkout` | ✗ | Not safe within same worktree |
| `git commit` | ✓ | Independent in different worktrees |
| `git push` | ✓ | Independent operation |
| `git merge` | ✗ | Needs sync within same repo |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Git version incompatibility | Low | Medium | Detect git version, provide fallback for older versions |
| Porcelain output format changes | Low | High | Use git --porcelain stable format, add version detection |
| Worktree corruption | Medium | Medium | Implement repair function, regular prune |
| Concurrent creation conflicts | Low | Medium | Use internal mutex for synchronization |

## Sprint Deliverables

- `core/src/worktree_manager.rs` - WorktreeManager implementation
- `core/src/worktree_state.rs` - WorktreeState struct
- `core/src/worktree_state_store.rs` - Persistence store
- Unit tests for all modules (>80% coverage)
- Integration tests with real git repository

## Dependencies

- Git >= 2.17 installed on system
- `chrono` crate for timestamp handling
- `serde/serde_json` for persistence
- `thiserror` for error definitions

## Module Structure

```
core/src/
├── worktree_manager.rs     # WorktreeManager, WorktreeInfo, WorktreeError
├── worktree_state.rs       # WorktreeState struct
├── worktree_state_store.rs # Persistence operations
└── lib.rs                  # Module exports (updated)
```

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Agent Integration](./sprint-2-agent-integration.md) for AgentPool integration with resume support.