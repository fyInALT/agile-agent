# Git Worktree Integration Research Document

## Metadata

- Date: 2026-04-16
- Project: agile-agent
- Target: Work Agent Git Worktree Isolation Solution
- Status: Draft

---

## 1. Background and Objectives

### 1.1 Problem Statement

In multi-agent parallel development scenarios, multiple work agents operating on the same code repository simultaneously cause the following issues:

1. **File Conflicts**: Multiple agents modifying the same file simultaneously leads to conflicts
2. **Branch Chaos**: Agents switching branches in the same working directory interfere with each other
3. **State Pollution**: One agent's modifications affect other agents' working environment
4. **Commit Isolation Difficulty**: Hard to maintain independent commit history for each agent

### 1.2 Objectives

Create independent git worktrees for each work agent to achieve:

1. **File System Isolation**: Each agent works in an independent directory
2. **Branch Isolation**: Each agent works on an independent branch
3. **Seamless Integration**: codex/claude/opencode can work in worktree without any configuration
4. **Automatic Management**: Worktree creation and cleanup are fully automated

---

## 2. Git Worktree Technical Research

### 2.1 Worktree Basic Concepts

Git worktree allows the same repository to have multiple working directories, each checking out a different branch:

```
Main Repository (main worktree)
├── .git/                     # Main repository's git directory
├── src/
└── ...

Linked worktrees
├── .worktrees/
│   ├── agent-alpha/          # Agent alpha's working directory
│   │   ├── .git              # File pointing to main repository
│   │   └── src/              # Checked out branch content
│   ├── agent-bravo/          # Agent bravo's working directory
│   │   ├── .git
│   │   └── src/
│   └── agent-charlie/
```

### 2.2 Core Git Commands

```bash
# List all worktrees
git worktree list

# Create new worktree (automatically creates branch)
git worktree add -b feature/task-123 .worktrees/agent-alpha

# Create worktree based on existing branch
git worktree add .worktrees/agent-alpha feature/task-123

# Create detached HEAD worktree (for temporary experiments)
git worktree add --detach .worktrees/temp-work

# Remove worktree
git worktree remove .worktrees/agent-alpha

# Clean up deleted worktree records
git worktree prune

# Output in porcelain format (easy for script parsing)
git worktree list --porcelain
```

### 2.3 Key Worktree Features

| Feature | Description | Significance for Agents |
|---------|-------------|------------------------|
| Shared .git directory | All worktrees share the same git repository | Unified commit history, no need for multiple clones |
| Independent working directory | Each worktree has independent file system view | Complete isolation, no file conflicts |
| Branch isolation | Each worktree can check out different branches | Parallel development of different features |
| Lightweight | Creating worktree only takes O(1) time | Fast agent startup |
| Disk efficiency | Shared .git/objects, saves space | Resource efficient |

### 2.4 Porcelain Output Format

```bash
$ git worktree list --porcelain
worktree /path/to/main
HEAD abc1234...
branch refs/heads/main

worktree /path/to/.worktrees/agent-alpha
HEAD def5678...
branch refs/heads/feature/task-123

worktree /path/to/.worktrees/agent-bravo
HEAD 9012345...
detached
```

---

## 3. Architecture Design

### 3.1 Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                        AgentPool                             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                  WorktreeManager                     │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │    │
│  │  │ create()    │  │ remove()    │  │ prune()     │  │    │
│  │  │ list()      │  │ status()    │  │ cleanup()   │  │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  │    │
│  └─────────────────────────────────────────────────────┘    │
│                              │                               │
│              ┌───────────────┴───────────────┐              │
│              ▼                               ▼              │
│  ┌──────────────────┐            ┌──────────────────┐      │
│  │   AgentSlot      │            │   AgentSlot      │      │
│  │  (alpha)         │            │  (bravo)         │      │
│  │  cwd: .worktrees │            │  cwd: .worktrees │      │
│  │     /agent-alpha │            │     /agent-bravo │      │
│  │  branch: task/1  │            │  branch: task/2  │      │
│  └──────────────────┘            └──────────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 WorktreeManager Interface Design

```rust
// core/src/worktree_manager.rs

use std::path::{Path, PathBuf};
use std::process::Command;

/// Worktree status information
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Worktree creation options
#[derive(Debug, Clone)]
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

/// Worktree manager
pub struct WorktreeManager {
    /// Main repository path
    repo_root: PathBuf,
    /// Worktrees storage directory
    worktrees_dir: PathBuf,
    /// Worktree naming prefix
    prefix: String,
}

impl WorktreeManager {
    /// Create new WorktreeManager
    pub fn new(repo_root: PathBuf, prefix: String) -> Result<Self, WorktreeError> {
        // Verify if it's a git repository
        if !repo_root.join(".git").exists() {
            return Err(WorktreeError::NotAGitRepository(repo_root));
        }
        
        let worktrees_dir = repo_root.join(".worktrees");
        
        Ok(Self {
            repo_root,
            worktrees_dir,
            prefix,
        })
    }
    
    /// List all worktrees
    pub fn list(&self) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        // git worktree list --porcelain
        let output = self.run_git_command(&["worktree", "list", "--porcelain"])?;
        self.parse_porcelain_output(&output)
    }
    
    /// Create new worktree
    pub fn create(&self, name: &str, options: WorktreeCreateOptions) -> Result<WorktreeInfo, WorktreeError> {
        let path = self.worktrees_dir.join(name);
        
        // Ensure directory exists
        std::fs::create_dir_all(&self.worktrees_dir)?;
        
        // Build git worktree add command
        let mut args = vec!["worktree", "add"];
        
        if let Some(branch) = &options.branch {
            if options.create_branch {
                args.push("-b");
                args.push(branch);
            }
        } else {
            // detached HEAD
            args.push("--detach");
        }
        
        args.push(path.to_str().unwrap());
        
        if let Some(base) = &options.base {
            args.push(base);
        }
        
        self.run_git_command(&args)?;
        self.get_worktree_info(&path)
    }
    
    /// Remove worktree
    pub fn remove(&self, name: &str) -> Result<(), WorktreeError> {
        let path = self.worktrees_dir.join(name);
        self.run_git_command(&["worktree", "remove", path.to_str().unwrap()])?;
        Ok(())
    }
    
    /// Clean up expired worktree records
    pub fn prune(&self) -> Result<(), WorktreeError> {
        self.run_git_command(&["worktree", "prune"])?;
        Ok(())
    }
    
    /// Create dedicated worktree for agent
    pub fn create_for_agent(&self, agent_id: &str, task_id: &str) -> Result<WorktreeInfo, WorktreeError> {
        let branch_name = format!("{}/{}", self.prefix, task_id);
        let worktree_name = agent_id.to_string();
        
        self.create(&worktree_name, WorktreeCreateOptions {
            path: self.worktrees_dir.join(&worktree_name),
            branch: Some(branch_name),
            create_branch: true,
            base: Some("main".to_string()), // Or get default branch from config
            lock_reason: None,
        })
    }
    
    /// Get worktree information
    fn get_worktree_info(&self, path: &Path) -> Result<WorktreeInfo, WorktreeError> {
        let all = self.list()?;
        all.into_iter()
            .find(|w| w.path == path)
            .ok_or(WorktreeError::WorktreeNotFound(path.to_path_buf()))
    }
    
    /// Run git command
    fn run_git_command(&self, args: &[&str]) -> Result<String, WorktreeError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| WorktreeError::GitCommandFailed(e.to_string()))?;
        
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(WorktreeError::GitCommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }
    
    /// Parse porcelain output
    fn parse_porcelain_output(&self, output: &str) -> Result<Vec<WorktreeInfo>, WorktreeError> {
        // Parse logic...
        todo!()
    }
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
}
```

### 3.3 Integration with AgentSlot

```rust
// Add worktree-related fields in AgentSlot

pub struct AgentSlot {
    // ... existing fields ...
    
    /// Worktree path (if using independent worktree)
    worktree_path: Option<PathBuf>,
    
    /// Worktree branch name
    worktree_branch: Option<String>,
}

impl AgentSlot {
    /// Get agent's working directory
    pub fn cwd(&self) -> PathBuf {
        self.worktree_path.clone().unwrap_or_else(|| {
            // If no worktree, use default directory
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        })
    }
}
```

### 3.4 Integration with AgentPool

```rust
// Integrate WorktreeManager in AgentPool

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
        
        Ok(Self {
            slots: Vec::new(),
            max_slots,
            next_agent_index: 1,
            focused_slot: 0,
            workplace_id,
            worktree_manager: Some(worktree_manager),
            // ...
        })
    }
    
    /// Spawn agent with isolated worktree
    pub fn spawn_agent_with_worktree(
        &mut self,
        provider_kind: ProviderKind,
        task_id: Option<&str>,
    ) -> Result<AgentId, String> {
        if !self.can_spawn() {
            return Err("Agent pool is full".to_string());
        }
        
        let agent_id = self.generate_agent_id();
        let codename = Self::generate_codename(self.next_agent_index - 1);
        
        // Create worktree
        let worktree_info = if let Some(wtm) = &self.worktree_manager {
            let task_ref = task_id.unwrap_or(&codename.0);
            Some(wtm.create_for_agent(agent_id.as_str(), task_ref)?)
        } else {
            None
        };
        
        let mut slot = AgentSlot::new(
            agent_id.clone(),
            codename.clone(),
            ProviderType::from_provider_kind(provider_kind),
        );
        
        // Set worktree path
        if let Some(info) = &worktree_info {
            slot.set_worktree(info.path.clone(), info.branch.clone());
        }
        
        self.slots.push(slot);
        Ok(agent_id)
    }
    
    /// Stop agent and cleanup worktree
    pub fn stop_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let slot_index = self.find_slot_index(agent_id)?;
        let slot = &mut self.slots[slot_index];
        
        // Cleanup worktree
        if let (Some(wtm), Some(path)) = (&self.worktree_manager, &slot.worktree_path) {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .ok_or("Invalid worktree path")?;
            wtm.remove(name)?;
        }
        
        slot.stop("User requested".to_string());
        Ok(())
    }
}
```

### 3.5 Worktree Persistence for Resume (CRITICAL)

**This is a critical design requirement**: Each work agent must persist its worktree information so that after a resume/restart, the agent can continue working in the same worktree.

#### 3.5.1 Problem Statement

When the system restarts or an agent is resumed:
1. The agent needs to know which worktree it was working in
2. The worktree might still exist (graceful shutdown) or need recreation (crash)
3. The agent must restore its working context (branch, uncommitted changes, etc.)

#### 3.5.2 Persistent Worktree State Structure

```rust
// core/src/worktree_state.rs

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persistent worktree state for agent resume
/// Stored in agent's state file alongside other agent metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeState {
    /// Unique identifier for this worktree
    pub worktree_id: String,
    
    /// Absolute path to the worktree directory
    pub path: PathBuf,
    
    /// Branch name (may not exist if worktree was deleted)
    pub branch: Option<String>,
    
    /// Base commit SHA when worktree was created
    pub base_commit: String,
    
    /// Task ID this worktree is associated with
    pub task_id: Option<String>,
    
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last activity timestamp
    pub last_active_at: chrono::DateTime<chrono::Utc>,
    
    /// Whether worktree should be preserved after task completion
    pub preserve_on_completion: bool,
    
    /// Commit SHAs made by this agent in this worktree
    pub commits: Vec<String>,
    
    /// Current HEAD commit SHA
    pub head_commit: Option<String>,
    
    /// Whether there are uncommitted changes
    pub has_uncommitted_changes: bool,
}

impl WorktreeState {
    /// Create new worktree state
    pub fn new(
        worktree_id: String,
        path: PathBuf,
        branch: Option<String>,
        base_commit: String,
        task_id: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            worktree_id,
            path,
            branch,
            base_commit,
            task_id,
            created_at: now,
            last_active_at: now,
            preserve_on_completion: false,
            commits: Vec::new(),
            head_commit: None,
            has_uncommitted_changes: false,
        }
    }
    
    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_active_at = chrono::Utc::now();
    }
    
    /// Record a new commit
    pub fn record_commit(&mut self, commit_sha: String) {
        self.commits.push(commit_sha.clone());
        self.head_commit = Some(commit_sha);
        self.touch();
    }
    
    /// Check if worktree directory still exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
    
    /// Get relative path from repo root
    pub fn relative_path(&self, repo_root: &Path) -> Option<PathBuf> {
        pathdiff::diff_paths(&self.path, repo_root)
    }
}
```

#### 3.5.3 Storage Location

The worktree state is stored in the agent's state file:

```
.state/
├── agents/
│   ├── agent_001.json        # Agent state including worktree info
│   ├── agent_002.json
│   └── ...
└── worktrees/
    ├── index.json             # Global worktree index
    └── agent_001.json         # Dedicated worktree state (optional)
```

**Agent State File Structure**:

```json
{
  "agent_id": "agent_001",
  "codename": "alpha",
  "provider_type": "claude",
  "status": "running",
  "task_id": "task-123",
  "worktree": {
    "worktree_id": "wt-alpha-001",
    "path": "/path/to/repo/.worktrees/agent-alpha",
    "branch": "agent/task-123",
    "base_commit": "abc123...",
    "task_id": "task-123",
    "created_at": "2026-04-16T10:00:00Z",
    "last_active_at": "2026-04-16T12:30:00Z",
    "preserve_on_completion": false,
    "commits": ["def456...", "ghi789..."],
    "head_commit": "ghi789...",
    "has_uncommitted_changes": false
  },
  "session_handle": {
    "type": "claude_session",
    "session_id": "sess-xxx"
  },
  ...
}
```

#### 3.5.4 WorktreeStateStore

```rust
// core/src/worktree_state_store.rs

use std::path::PathBuf;
use std::fs;
use serde_json;

/// Store for persisting and loading worktree states
pub struct WorktreeStateStore {
    /// Base directory for state storage
    state_dir: PathBuf,
}

impl WorktreeStateStore {
    pub fn new(state_dir: PathBuf) -> Self {
        Self { state_dir }
    }
    
    /// Get the agent state file path
    fn agent_state_path(&self, agent_id: &str) -> PathBuf {
        self.state_dir.join("agents").join(format!("{}.json", agent_id))
    }
    
    /// Save worktree state for an agent
    pub fn save(&self, agent_id: &str, state: &WorktreeState) -> Result<(), WorktreeStateError> {
        // Load existing agent state or create new
        let path = self.agent_state_path(agent_id);
        let dir = path.parent().unwrap();
        fs::create_dir_all(dir)?;
        
        let mut agent_state: serde_json::Value = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content)?
        } else {
            serde_json::json!({ "agent_id": agent_id })
        };
        
        // Update worktree field
        agent_state["worktree"] = serde_json::to_value(state)?;
        
        // Write back
        let content = serde_json::to_string_pretty(&agent_state)?;
        fs::write(&path, content)?;
        
        Ok(())
    }
    
    /// Load worktree state for an agent
    pub fn load(&self, agent_id: &str) -> Result<Option<WorktreeState>, WorktreeStateError> {
        let path = self.agent_state_path(agent_id);
        
        if !path.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(&path)?;
        let agent_state: serde_json::Value = serde_json::from_str(&content)?;
        
        if let Some(worktree_value) = agent_state.get("worktree") {
            let state: WorktreeState = serde_json::from_value(worktree_value.clone())?;
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }
    
    /// Delete worktree state (when agent is permanently removed)
    pub fn delete(&self, agent_id: &str) -> Result<(), WorktreeStateError> {
        let path = self.agent_state_path(agent_id);
        if path.exists() {
            // Load and remove worktree field only
            let content = fs::read_to_string(&path)?;
            let mut agent_state: serde_json::Value = serde_json::from_str(&content)?;
            agent_state.as_object_mut().unwrap().remove("worktree");
            
            let content = serde_json::to_string_pretty(&agent_state)?;
            fs::write(&path, content)?;
        }
        Ok(())
    }
    
    /// List all agents with worktree states
    pub fn list_all(&self) -> Result<Vec<(String, WorktreeState)>, WorktreeStateError> {
        let agents_dir = self.state_dir.join("agents");
        if !agents_dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut result = Vec::new();
        for entry in fs::read_dir(&agents_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let content = fs::read_to_string(&path)?;
                let agent_state: serde_json::Value = serde_json::from_str(&content)?;
                
                if let Some(agent_id) = agent_state.get("agent_id").and_then(|v| v.as_str()) {
                    if let Some(worktree_value) = agent_state.get("worktree") {
                        let state: WorktreeState = serde_json::from_value(worktree_value.clone())?;
                        result.push((agent_id.to_string(), state));
                    }
                }
            }
        }
        
        Ok(result)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorktreeStateError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("json error: {0}")]
    JsonError(#[from] serde_json::Error),
}
```

#### 3.5.5 Resume Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Worktree Resume Flow                          │
│                                                                  │
│  1. System Restart / Agent Resume                               │
│     │                                                            │
│     ▼                                                            │
│  2. Load Agent State File                                        │
│     worktree_state_store.load(agent_id)                         │
│     │                                                            │
│     ▼                                                            │
│  3. Check Worktree Existence                                     │
│     worktree_state.exists()?                                     │
│     │                                                            │
│     ├─ YES ──────────────────────────────────────┐              │
│     │                                            │              │
│     │  4a. Verify Worktree Validity              │              │
│     │      - Branch still exists?                │              │
│     │      - HEAD matches stored head_commit?    │              │
│     │                                            │              │
│     │  5a. Resume in Existing Worktree           │              │
│     │      agent_slot.cwd = worktree_state.path  │              │
│     │                                            │              │
│     ▼                                            ▼              │
│  NO ────────────────────────────────────────────▶│              │
│     │                                            │              │
│     │  4b. Recreate Worktree                     │              │
│     │      worktree_manager.create_from_state()  │              │
│     │                                            │              │
│     │  5b. Restore Context                       │              │
│     │      - Re-create branch if needed          │              │
│     │      - Cherry-pick commits if lost         │              │
│     │                                            │              │
│     ▼                                            ▼              │
│  6. Provider Resume                              │              │
│     provider.start(prompt, cwd=worktree_path,   │              │
│                    session_handle=stored_session)│              │
│     │                                            │              │
│     ▼                                            ▼              │
│  7. Agent Continues Work                         │              │
│     - Same files, same branch, same context      │              │
└─────────────────────────────────────────────────────────────────┘
```

#### 3.5.6 Resume Implementation

```rust
// In AgentSlot or AgentPool

/// Resume agent with existing worktree state
pub fn resume_agent(
    &mut self,
    agent_id: &AgentId,
    worktree_state_store: &WorktreeStateStore,
    worktree_manager: &WorktreeManager,
) -> Result<ResumeResult, ResumeError> {
    // 1. Load persisted worktree state
    let worktree_state = worktree_state_store.load(agent_id.as_str())?
        .ok_or(ResumeError::NoWorktreeState(agent_id.clone()))?;
    
    // 2. Check if worktree still exists
    if worktree_state.exists() {
        // Worktree exists - verify and use it
        self.resume_existing_worktree(agent_id, &worktree_state, worktree_manager)?;
        Ok(ResumeResult::ExistingWorktree)
    } else {
        // Worktree doesn't exist - recreate it
        self.recreate_worktree(agent_id, &worktree_state, worktree_manager)?;
        Ok(ResumeResult::RecreatedWorktree)
    }
}

/// Resume in existing worktree
fn resume_existing_worktree(
    &mut self,
    agent_id: &AgentId,
    state: &WorktreeState,
    worktree_manager: &WorktreeManager,
) -> Result<(), ResumeError> {
    // Verify the worktree is valid
    let current_info = worktree_manager.get_worktree_info(&state.path)?;
    
    // Check if branch matches
    if let Some(expected_branch) = &state.branch {
        if current_info.branch.as_ref() != Some(expected_branch) {
            // Branch mismatch - might need to checkout
            log::warn!(
                "Worktree branch mismatch: expected {}, found {}",
                expected_branch,
                current_info.branch.as_deref().unwrap_or("detached")
            );
        }
    }
    
    // Set the worktree path for this agent
    let slot = self.get_slot_mut(agent_id)?;
    slot.set_worktree(state.path.clone(), state.branch.clone());
    
    Ok(())
}

/// Recreate missing worktree
fn recreate_worktree(
    &mut self,
    agent_id: &AgentId,
    state: &WorktreeState,
    worktree_manager: &WorktreeManager,
) -> Result<(), ResumeError> {
    // Try to recreate worktree from stored state
    let branch_name = state.branch.clone()
        .ok_or(ResumeError::MissingBranchInfo)?;
    
    // Check if branch still exists in repo
    let branch_exists = worktree_manager.branch_exists(&branch_name)?;
    
    let options = WorktreeCreateOptions {
        path: state.path.clone(),
        branch: Some(branch_name.clone()),
        create_branch: !branch_exists,  // Only create if doesn't exist
        base: if branch_exists {
            None  // Use existing branch
        } else {
            Some(state.base_commit.clone())  // Recreate from base
        },
        lock_reason: None,
    };
    
    let worktree_name = state.path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(agent_id.as_str());
    
    let info = worktree_manager.create(worktree_name, options)?;
    
    // Update slot with new worktree info
    let slot = self.get_slot_mut(agent_id)?;
    slot.set_worktree(info.path.clone(), info.branch.clone());
    
    // Update persisted state
    // ... (update path if changed, etc.)
    
    Ok(())
}

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
```

#### 3.5.7 Integration with Agent Lifecycle

```rust
// In AgentPool - integrated lifecycle with worktree persistence

impl AgentPool {
    /// Spawn agent with worktree and persist state
    pub fn spawn_agent_with_persistence(
        &mut self,
        provider_kind: ProviderKind,
        task_id: Option<&str>,
    ) -> Result<AgentId, String> {
        // 1. Create worktree
        let agent_id = self.spawn_agent_with_worktree(provider_kind, task_id)?;
        
        // 2. Get worktree info from slot
        let slot = self.get_slot(&agent_id).unwrap();
        let worktree_state = WorktreeState::new(
            slot.worktree_id(),
            slot.worktree_path.clone().unwrap(),
            slot.worktree_branch.clone(),
            worktree_manager.get_base_commit()?,
            task_id,
        );
        
        // 3. Persist worktree state
        self.worktree_state_store.save(agent_id.as_str(), &worktree_state)?;
        
        Ok(agent_id)
    }
    
    /// Pause agent (keep worktree, persist state)
    pub fn pause_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        let slot = self.get_slot_mut(agent_id)?;
        
        // 1. Update worktree state with latest info
        if let Some(path) = &slot.worktree_path {
            let state = self.worktree_state_store.load(agent_id.as_str())?
                .ok_or("No worktree state")?;
            
            // Update state with current HEAD, commits, etc.
            let updated_state = self.update_worktree_state(&state, path)?;
            self.worktree_state_store.save(agent_id.as_str(), &updated_state)?;
        }
        
        // 2. Pause the agent (don't remove worktree)
        slot.pause();
        
        Ok(())
    }
    
    /// Resume paused agent
    pub fn resume_agent(&mut self, agent_id: &AgentId) -> Result<(), String> {
        // 1. Load worktree state and resume
        self.resume_agent_with_worktree(agent_id)?;
        
        // 2. Resume the agent in the worktree
        let slot = self.get_slot_mut(agent_id)?;
        let cwd = slot.cwd();
        
        // 3. Start provider with resumed session
        slot.resume_provider(cwd)?;
        
        Ok(())
    }
    
    /// Stop agent and optionally cleanup worktree
    pub fn stop_agent_with_cleanup(
        &mut self,
        agent_id: &AgentId,
        cleanup_worktree: bool,
    ) -> Result<(), String> {
        let slot = self.get_slot_mut(agent_id)?;
        
        if cleanup_worktree {
            // Remove worktree
            if let (Some(wtm), Some(path)) = (&self.worktree_manager, &slot.worktree_path) {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or("Invalid worktree path")?;
                wtm.remove(name)?;
            }
            
            // Delete persisted state
            self.worktree_state_store.delete(agent_id.as_str())?;
        } else {
            // Keep worktree for later resume - just update state
            if let Some(path) = &slot.worktree_path {
                let state = self.worktree_state_store.load(agent_id.as_str())?
                    .ok_or("No worktree state")?;
                let updated_state = self.update_worktree_state(&state, path)?;
                self.worktree_state_store.save(agent_id.as_str(), &updated_state)?;
            }
        }
        
        slot.stop("Stopped".to_string());
        Ok(())
    }
}
```

#### 3.5.8 Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Store worktree state in agent state file | Simple, atomic reads/writes; worktree is part of agent identity |
| Use absolute paths in storage | Portable across restarts; resolve relative on load |
| Record base_commit | Enables recreation if worktree is lost |
| Track commits made by agent | Useful for review, debugging, and recovery |
| Preserve on pause, cleanup on explicit stop | Default behavior preserves work for resume |

---

## 4. Provider Integration Strategy

### 4.1 Core Principle

**Key Finding**: All providers (Claude, Codex, OpenCode) support `--cwd` or equivalent parameter to specify working directory.

### 4.2 Claude Integration

Claude CLI sets working directory via `current_dir()`:

```rust
// core/src/providers/claude.rs (existing code)
fn run_claude(...) -> Result<()> {
    let mut command = Command::new(&executable);
    command.args(&args);
    command.current_dir(&cwd);  // <-- Sets working directory here
    // ...
}
```

**Conclusion**: Claude already supports this, just pass the correct worktree path.

### 4.3 Codex Integration

Codex CLI also supports `--cwd`:

```rust
// core/src/providers/codex.rs (existing code)
fn run_codex(...) -> Result<()> {
    let mut command = Command::new(&executable);
    // ...
    command.current_dir(&cwd);  // <-- Sets working directory here
    // ...
}
```

**Conclusion**: Codex already supports this, just pass the correct worktree path.

### 4.4 OpenCode Integration

OpenCode uses ACP protocol, specifies cwd when creating session:

```json
// session/new request
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/path/to/worktree"}}
```

```rust
// Future OpenCode provider implementation
fn start_opencode(...) -> Result<()> {
    // cwd parameter in ACP protocol
    let session_params = json!({
        "cwd": worktree_path.to_str().unwrap()
    });
    // ...
}
```

**Conclusion**: OpenCode supports this, pass worktree path in ACP session creation.

### 4.5 Unified Cwd Propagation Path

```
┌──────────────────────────────────────────────────────────────────┐
│                        Data Flow                                  │
│                                                                   │
│  AgentPool.spawn_agent_with_worktree()                            │
│              │                                                    │
│              ▼                                                    │
│  WorktreeManager.create_for_agent()                              │
│              │                                                    │
│              ▼                                                    │
│  AgentSlot.worktree_path = PathBuf::from(".worktrees/agent-001") │
│              │                                                    │
│              ▼                                                    │
│  AgentSlot.cwd() -> PathBuf                                       │
│              │                                                    │
│              ▼                                                    │
│  provider::start(prompt, cwd, ...)                               │
│              │                                                    │
│              ▼                                                    │
│  Command::new().current_dir(&cwd)                                 │
│              │                                                    │
│              ▼                                                    │
│  Claude/Codex/OpenCode runs in worktree directory                 │
└──────────────────────────────────────────────────────────────────┘
```

---

## 5. Branch Management Strategy

### 5.1 Branch Naming Convention

```
agent/{task-id}          # Based on task ID
agent/{agent-codename}   # Based on agent codename
agent/sprint-{n}/{task}  # Based on sprint task
```

### 5.2 Lifecycle

```
┌─────────────────────────────────────────────────────────────────┐
│                    Worktree Lifecycle                            │
│                                                                 │
│  1. Creation                                                    │
│     spawn_agent_with_worktree()                                 │
│     └── git worktree add -b agent/task-123 .worktrees/alpha    │
│                                                                 │
│  2. Usage                                                       │
│     Agent works in worktree                                     │
│     └── Provider uses cwd = .worktrees/alpha                   │
│                                                                 │
│  3. Task Completion                                             │
│     Agent completes task, commits changes                       │
│     └── git push origin agent/task-123                          │
│                                                                 │
│  4. Cleanup                                                     │
│     stop_agent()                                                │
│     └── git worktree remove .worktrees/alpha                    │
│     └── git branch -d agent/task-123 (optional)                 │
└─────────────────────────────────────────────────────────────────┘
```

### 5.3 Branch Completion Strategy

```rust
/// Branch handling options after task completion
pub enum BranchCompletionPolicy {
    /// Keep branch and create PR
    KeepAndCreatePR,
    /// Keep branch for manual review
    KeepForManualReview,
    /// Auto merge to main (only for low-risk tasks)
    AutoMerge { target: String },
    /// Delete branch (experimental tasks)
    Delete,
}

impl WorktreeManager {
    /// Handle task completion
    pub fn complete_task(
        &self,
        worktree_name: &str,
        policy: BranchCompletionPolicy,
    ) -> Result<(), WorktreeError> {
        match policy {
            BranchCompletionPolicy::KeepAndCreatePR => {
                // Keep branch, notify user to create PR
                // Possibly via GitHub CLI or API
            }
            BranchCompletionPolicy::KeepForManualReview => {
                // Only remove worktree, keep branch
                self.remove(worktree_name)?;
            }
            BranchCompletionPolicy::AutoMerge { target } => {
                // Merge to target branch
                // git checkout target && git merge branch
            }
            BranchCompletionPolicy::Delete => {
                // Delete worktree and branch
                self.remove(worktree_name)?;
                // git branch -D branch
            }
        }
        Ok(())
    }
}
```

---

## 6. Concurrency and Safety Considerations

### 6.1 Git Operation Thread Safety

Some Git operations are not thread-safe:

| Operation | Thread Safe | Notes |
|-----------|-------------|-------|
| `git worktree add` | ✓ | Can parallel create different worktrees |
| `git worktree remove` | ✓ | Can parallel remove different worktrees |
| `git checkout` | ✗ | Not safe within same worktree |
| `git commit` | ✓ | Independent in different worktrees |
| `git push` | ✓ | Independent operation |
| `git merge` | ✗ | Needs sync within same repo |

### 6.2 Implementation Recommendation

```rust
use std::sync::Mutex;

pub struct WorktreeManager {
    repo_root: PathBuf,
    worktrees_dir: PathBuf,
    prefix: String,
    /// Mutex for synchronizing non-thread-safe git operations
    git_lock: Mutex<()>,
}
```

### 6.3 Resource Limits

```rust
/// Worktree configuration
pub struct WorktreeConfig {
    /// Maximum number of worktrees
    pub max_worktrees: usize,
    /// Worktree directory name prefix
    pub prefix: String,
    /// Default base branch
    pub default_base_branch: String,
    /// Whether to auto cleanup completed worktrees
    pub auto_cleanup: bool,
    /// Worktree idle timeout (seconds)
    pub idle_timeout_secs: u64,
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            max_worktrees: 10,
            prefix: "agent".to_string(),
            default_base_branch: "main".to_string(),
            auto_cleanup: true,
            idle_timeout_secs: 3600, // 1 hour
        }
    }
}
```

---

## 7. Implementation Plan

### 7.1 Phase 1: Infrastructure (Sprint 1)

**Goal**: Build WorktreeManager core and persistence layer

**Tasks**:
1. Create `core/src/worktree_manager.rs`
   - Implement `create`, `remove`, `list`, `prune` methods
   - Add porcelain output parsing
2. Create `core/src/worktree_state.rs`
   - Define `WorktreeState` struct with all necessary fields
3. Create `core/src/worktree_state_store.rs`
   - Implement save/load operations
   - Integrate with existing agent state file format
4. Add unit tests for all new modules

**Acceptance Criteria**:
- [ ] Can create worktree
- [ ] Can list worktrees
- [ ] Can remove worktree
- [ ] Can save worktree state to disk
- [ ] Can load worktree state from disk
- [ ] Test coverage > 80%

### 7.2 Phase 2: Agent Integration with Persistence (Sprint 2) - CRITICAL

**Goal**: Integrate WorktreeManager into AgentPool with full persistence support

**Tasks**:
1. Modify `AgentSlot` to add worktree fields
   - `worktree_id`, `worktree_path`, `worktree_branch`
2. Modify `AgentPool::spawn_agent` to:
   - Create worktree
   - Persist worktree state immediately
3. Implement `AgentPool::pause_agent`:
   - Update worktree state before pause
   - Keep worktree intact for resume
4. Implement `AgentPool::resume_agent`:
   - Load worktree state from disk
   - Verify worktree exists or recreate
   - Resume provider in correct cwd
5. Modify `AgentPool::stop_agent` to:
   - Support cleanup vs preserve options
   - Delete worktree state if cleanup
6. Update related tests

**Acceptance Criteria**:
- [ ] New agent automatically gets worktree with persisted state
- [ ] Paused agent preserves worktree state
- [ ] Resumed agent continues in same worktree
- [ ] Resumed agent works even if worktree was deleted (recreation)
- [ ] Stopped agent can optionally cleanup worktree
- [ ] Provider runs in correct cwd after resume

### 7.3 Phase 3: TUI Display (Sprint 3)

**Goal**: Display worktree status in TUI

**Tasks**:
1. Display branch name in agent status panel
2. Add worktree path display
3. Add worktree management commands (pause/resume)
4. Show worktree existence status (exists/missing)

**Acceptance Criteria**:
- [ ] TUI shows each agent's branch
- [ ] Can view worktree status from TUI
- [ ] Can pause/resume agent from TUI

### 7.4 Phase 4: Advanced Features (Sprint 4)

**Goal**: Complete branch management and cleanup

**Tasks**:
1. Implement branch completion policy
2. Add auto cleanup for idle worktrees
3. Add PR creation support
4. Implement branch merge flow
5. Add crash recovery: detect orphaned worktree states at startup

**Acceptance Criteria**:
- [ ] Can create PR after task completion
- [ ] Idle worktrees auto cleanup
- [ ] Branch merge flow complete
- [ ] System recovers orphaned worktrees on startup

---

## 8. Risks and Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Too many worktrees causing disk space shortage | Medium | Set max worktree limit, auto cleanup |
| Git version incompatibility | Low | Detect git version, provide fallback |
| Worktree corruption | Medium | Implement repair function, regular prune |
| Branch name conflicts | Low | Use UUID or timestamp as suffix |
| Concurrent creation conflicts | Low | Use internal lock for synchronization |

---

## 9. Alternative Solution Comparison

### 9.1 Git Worktree vs Multiple Repository Clones

| Aspect | Git Worktree | Multiple Clones |
|--------|-------------|-----------------|
| Disk space | Shared .git, saves space | Each has complete .git |
| Creation speed | O(1) | O(n) depends on repo size |
| Sync complexity | Auto sync | Need manual fetch/push |
| Network overhead | None | Need to clone |
| Isolation level | Branch level | Complete isolation |

**Conclusion**: Git Worktree is more suitable for agent scenarios.

### 9.2 Git Worktree vs Subdirectory + Stash

| Aspect | Git Worktree | Subdirectory + Stash |
|--------|-------------|---------------------|
| Branch independence | ✓ Completely independent | ✗ Same branch |
| Parallel development | ✓ Supported | ✗ Need manual stash |
| Conflict risk | None | High |
| Implementation complexity | Medium | Low |

**Conclusion**: Git Worktree provides better isolation.

---

## 10. Summary

### 10.1 Core Advantages

1. **Complete Isolation**: Each agent works in independent directory and branch, zero interference
2. **Seamless Integration**: Existing provider architecture already supports cwd parameter, no modification needed
3. **Resource Efficient**: Shared .git directory, saves disk space and sync time
4. **Fast Creation**: O(1) time to create new worktree
5. **Unified History**: All commits in same repository, easy to manage and track
6. **Resume Support**: Worktree state persisted per-agent, enables seamless resume after restart

### 10.2 Critical Requirement: Worktree Persistence

**Each work agent MUST persist its worktree information to enable resume functionality.**

Key aspects:
- Worktree state stored in agent's state file (`.state/agents/{agent_id}.json`)
- Includes: worktree_id, path, branch, base_commit, commits made, HEAD position
- On resume: load state → verify worktree exists → recreate if missing → continue work
- Pause preserves worktree; explicit stop can optionally cleanup

### 10.3 Implementation Path

```
Phase 1: WorktreeManager Core + Persistence Layer
    ↓
Phase 2: AgentPool Integration with Resume Support (CRITICAL)
    ↓
Phase 3: TUI Display
    ↓
Phase 4: Advanced Features + Crash Recovery
```

### 10.4 Key Code Changes

1. Add new `core/src/worktree_manager.rs`
2. Add new `core/src/worktree_state.rs` - WorktreeState struct
3. Add new `core/src/worktree_state_store.rs` - Persistence operations
4. Modify `core/src/agent_slot.rs` to add worktree fields
5. Modify `core/src/agent_pool.rs` to integrate worktree management with persistence
6. No need to modify `core/src/providers/claude.rs`
7. No need to modify `core/src/providers/codex.rs`
8. Future OpenCode provider needs to pass cwd in ACP session

---

## Appendix A: Command Reference

```bash
# Check git version requirement
git --version  # Recommend >= 2.17

# List all worktrees
git worktree list

# Create worktree (new branch)
git worktree add -b feature/new-feature .worktrees/agent-001

# Create worktree (existing branch)
git worktree add .worktrees/agent-001 feature/existing-branch

# Create detached worktree
git worktree add --detach .worktrees/temp

# Lock worktree (prevent prune)
git worktree lock --reason "Agent working" .worktrees/agent-001

# Unlock worktree
git worktree unlock .worktrees/agent-001

# Remove worktree
git worktree remove .worktrees/agent-001

# Force remove (with uncommitted changes)
git worktree remove --force .worktrees/agent-001

# Clean up expired worktree records
git worktree prune

# View detailed info
git worktree list --porcelain -v
```

## Appendix B: Related Files

- `core/src/agent_pool.rs` - Agent pool management
- `core/src/agent_slot.rs` - Agent slot
- `core/src/provider.rs` - Provider abstraction
- `core/src/providers/claude.rs` - Claude provider
- `core/src/providers/codex.rs` - Codex provider
- `core/src/worktree_manager.rs` - Worktree management (NEW)
- `core/src/worktree_state.rs` - Worktree state struct (NEW)
- `core/src/worktree_state_store.rs` - Worktree persistence (NEW)
- `kanban/src/git_ops.rs` - Git operations (to be extended)

## Appendix C: Reference Links

- [Git Worktree Documentation](https://git-scm.com/docs/git-worktree)
- [Git Worktree Best Practices](https://git-scm.com/book/en/v2/Git-Branching-Branches-in-a-Nutshell)
- [ACP Protocol Spec](../opencode/provider-analysis.md)
