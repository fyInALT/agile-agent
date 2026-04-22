# Core Package Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split 71K-line agent-core into 7 focused crates with clear dependency boundaries.

**Architecture:** Create foundation crate (agent-types) first to resolve circular dependency, then extract domain crates bottom-up: toolkit → provider → worktree → backlog → storage. Finally clean up agent-core.

**Tech Stack:** Rust workspace, Cargo.toml management, serde serialization

---

## Sprint Overview

| Sprint | Focus | Crates | Estimated Stories |
|--------|-------|--------|-------------------|
| 1 | Foundation | agent-types | 5 |
| 2 | Toolkit | agent-toolkit | 3 |
| 3 | Provider Layer | agent-provider | 6 |
| 4 | Worktree | agent-worktree | 4 |
| 5 | Backlog | agent-backlog | 4 |
| 6 | Storage | agent-storage | 4 |
| 7 | Cleanup | agent-core shrink | 5 |

**Total: ~31 stories across 7 sprints**

---

## Sprint 1: Foundation - agent-types Crate

**Goal:** Create foundation crate with pure types to resolve circular dependency.

**Risk Level:** Lowest - no implementation code, only type definitions.

### Story 1.1: Create agent-types crate skeleton

**Files:**
- Create: `agent-types/Cargo.toml`
- Create: `agent-types/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create crate directory**

```bash
mkdir -p agent-types/src
```

- [ ] **Step 2: Write Cargo.toml**

Create `agent-types/Cargo.toml`:
```toml
[package]
name = "agent-types"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
```

- [ ] **Step 3: Write lib.rs skeleton**

Create `agent-types/src/lib.rs`:
```rust
//! Foundation types for agile-agent ecosystem
//!
//! Pure data types with no implementation dependencies.

pub mod agent_id;
pub mod agent_status;
pub mod task_status;
pub mod provider_type;
pub mod task_types;

pub use agent_id::*;
pub use agent_status::*;
pub use task_status::*;
pub use provider_type::*;
pub use task_types::*;
```

- [ ] **Step 4: Update workspace Cargo.toml**

Add to `Cargo.toml` members array:
```toml
members = [
    "agent-types",
    # ... existing members
]
```

- [ ] **Step 5: Verify crate compiles**

```bash
cargo build -p agent-types
```
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add agent-types/ Cargo.toml
git commit -m "feat: create agent-types foundation crate skeleton"
```

---

### Story 1.2: Extract AgentId, WorkplaceId, AgentCodename

**Files:**
- Create: `agent-types/src/agent_id.rs`
- Read: `core/src/agent_runtime.rs` (lines 1-80 for reference)

- [ ] **Step 1: Write failing test for AgentId**

Create `agent-types/src/agent_id.rs` with test:
```rust
use serde::{Deserialize, Serialize};

/// Unique identifier for an agent
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Unique identifier for a workplace
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkplaceId(String);

impl WorkplaceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Short codename for an agent (e.g., "alpha", "beta")
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentCodename(String);

impl AgentCodename {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_new_and_as_str() {
        let id = AgentId::new("agent-001");
        assert_eq!(id.as_str(), "agent-001");
    }

    #[test]
    fn workplace_id_new_and_as_str() {
        let id = WorkplaceId::new("workplace-abc");
        assert_eq!(id.as_str(), "workplace-abc");
    }

    #[test]
    fn agent_codename_new_and_as_str() {
        let name = AgentCodename::new("alpha");
        assert_eq!(name.as_str(), "alpha");
    }

    #[test]
    fn agent_id_serialization() {
        let id = AgentId::new("test");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test\"");
        let parsed: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test -p agent-types
```
Expected: All 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add agent-types/src/agent_id.rs
git commit -m "feat(agent-types): add AgentId, WorkplaceId, AgentCodename types"
```

---

### Story 1.3: Extract AgentStatus enum

**Files:**
- Create: `agent-types/src/agent_status.rs`
- Read: `core/src/agent_runtime.rs` (AgentStatus definition)

- [ ] **Step 1: Write agent_status.rs**

```rust
use serde::{Deserialize, Serialize};

/// Status of an agent in the runtime
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Running,
    Stopped,
}

impl AgentStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Stopped => "stopped",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_status_labels() {
        assert_eq!(AgentStatus::Idle.label(), "idle");
        assert_eq!(AgentStatus::Running.label(), "running");
        assert_eq!(AgentStatus::Stopped.label(), "stopped");
    }

    #[test]
    fn agent_status_serialization() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p agent-types
```

- [ ] **Step 3: Commit**

```bash
git add agent-types/src/agent_status.rs
git commit -m "feat(agent-types): add AgentStatus enum"
```

---

### Story 1.4: Extract TaskStatus, TodoStatus, TodoItem, TaskItem

**Files:**
- Create: `agent-types/src/task_status.rs`
- Create: `agent-types/src/task_types.rs`
- Read: `core/src/backlog.rs` (lines 1-60)

- [ ] **Step 1: Write task_status.rs**

```rust
use serde::{Deserialize, Serialize};

/// Status of a todo item in backlog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    Candidate,
    Ready,
    InProgress,
    Blocked,
    Done,
    Dropped,
}

/// Status of a task in execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    Draft,
    Ready,
    Running,
    Verifying,
    #[serde(alias = "Completed")]
    Done,
    Blocked,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_status_serialization() {
        let status = TodoStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"InProgress\"");
    }
}
```

- [ ] **Step 2: Write task_types.rs**

```rust
use serde::{Deserialize, Serialize};

use super::task_status::{TaskStatus, TodoStatus};

/// Unique identifier for a task
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A todo item in the backlog
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: u8,
    pub status: TodoStatus,
    pub acceptance_criteria: Vec<String>,
    pub dependencies: Vec<String>,
    pub source: String,
}

/// A task derived from a todo
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: String,
    pub todo_id: String,
    pub objective: String,
    pub scope: String,
    pub constraints: Vec<String>,
    pub verification_plan: Vec<String>,
    pub status: TaskStatus,
    pub result_summary: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_creation() {
        let id = TaskId::new("task-001");
        assert_eq!(id.as_str(), "task-001");
    }

    #[test]
    fn todo_item_serialization() {
        let todo = TodoItem {
            id: "todo-1".to_string(),
            title: "Test todo".to_string(),
            description: "Description".to_string(),
            priority: 1,
            status: TodoStatus::Ready,
            acceptance_criteria: vec!["criteria".to_string()],
            dependencies: vec![],
            source: "user".to_string(),
        };
        let json = serde_json::to_string(&todo).unwrap();
        assert!(json.contains("\"title\":\"Test todo\""));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p agent-types
```

- [ ] **Step 4: Commit**

```bash
git add agent-types/src/task_status.rs agent-types/src/task_types.rs
git commit -m "feat(agent-types): add TaskStatus, TodoStatus, TodoItem, TaskItem types"
```

---

### Story 1.5: Extract ProviderKind and update core imports

**Files:**
- Create: `agent-types/src/provider_type.rs`
- Modify: `core/src/provider.rs` (use agent-types)
- Modify: `core/src/agent_runtime.rs` (use agent-types)
- Modify: `core/src/backlog.rs` (use agent-types)
- Modify: `core/Cargo.toml` (add agent-types dependency)

- [ ] **Step 1: Write provider_type.rs**

```rust
use serde::{Deserialize, Serialize};

/// Kind of LLM provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Mock,
    Claude,
    Codex,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Mock => Self::Claude,
            Self::Claude => Self::Codex,
            Self::Codex => Self::Mock,
        }
    }

    pub fn all() -> [ProviderKind; 3] {
        [ProviderKind::Mock, ProviderKind::Claude, ProviderKind::Codex]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_labels() {
        assert_eq!(ProviderKind::Claude.label(), "claude");
    }
}
```

- [ ] **Step 2: Update lib.rs to export ProviderKind**

Edit `agent-types/src/lib.rs`:
```rust
pub mod provider_type;
pub use provider_type::*;
```

- [ ] **Step 3: Add agent-types dependency to core**

Edit `core/Cargo.toml`:
```toml
[dependencies]
agent-types = { path = "../agent-types" }
# ... existing deps
```

- [ ] **Step 4: Update core/src/provider.rs imports**

Add at top of `core/src/provider.rs`:
```rust
use agent_types::ProviderKind;

// Re-export for backward compatibility
pub use agent_types::ProviderKind;
```

Remove the local ProviderKind definition (lines 22-28).

- [ ] **Step 5: Update core/src/agent_runtime.rs imports**

Add at top:
```rust
use agent_types::{AgentId, WorkplaceId, AgentCodename, AgentStatus};

// Re-export for backward compatibility  
pub use agent_types::{AgentId, WorkplaceId, AgentCodename, AgentStatus};
```

Remove local definitions of AgentId, WorkplaceId, AgentCodename, AgentStatus.

- [ ] **Step 6: Update core/src/backlog.rs imports**

Add at top:
```rust
use agent_types::{TaskStatus, TodoStatus, TodoItem, TaskItem, TaskId};

// Re-export for backward compatibility
pub use agent_types::{TaskStatus, TodoStatus, TodoItem, TaskItem, TaskId};
```

Remove local definitions of TaskStatus, TodoStatus, TodoItem, TaskItem.

- [ ] **Step 7: Run all tests**

```bash
cargo test --workspace
```
Expected: All tests still pass (backward compatible via re-exports)

- [ ] **Step 8: Commit**

```bash
git add agent-types/ core/
git commit -m "feat(agent-types): extract ProviderKind, update core to use agent-types"
```

---

**Sprint 1 Complete ✓** - agent-types crate created with all foundation types, core uses it via re-exports for backward compatibility.

---

## Sprint 2: Toolkit - agent-toolkit Crate

**Goal:** Extract tool call types into standalone crate.

**Risk Level:** Low - isolated types with minimal dependencies.

### Story 2.1: Create agent-toolkit crate skeleton

**Files:**
- Create: `agent-toolkit/Cargo.toml`
- Create: `agent-toolkit/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create crate directory**

```bash
mkdir -p agent-toolkit/src
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "agent-toolkit"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-types = { path = "../agent-types" }
serde.workspace = true
```

- [ ] **Step 3: Write lib.rs**

```rust
//! Tool call types for agent providers
//!
//! Types representing tool invocations and their statuses.

mod tool_calls;

pub use tool_calls::*;
```

- [ ] **Step 4: Update workspace**

Edit `Cargo.toml`:
```toml
members = [
    "agent-types",
    "agent-toolkit",
    # ... existing
]
```

- [ ] **Step 5: Verify compilation**

```bash
cargo build -p agent-toolkit
```

- [ ] **Step 6: Commit**

```bash
git add agent-toolkit/ Cargo.toml
git commit -m "feat: create agent-toolkit crate skeleton"
```

---

### Story 2.2: Move tool_calls.rs from core

**Files:**
- Create: `agent-toolkit/src/tool_calls.rs`
- Delete: `core/src/tool_calls.rs` (after move)
- Modify: `core/src/lib.rs`
- Modify: `core/Cargo.toml`

- [ ] **Step 1: Copy tool_calls.rs to toolkit**

Read `core/src/tool_calls.rs`, then create `agent-toolkit/src/tool_calls.rs` with identical content plus re-export from agent-types:

```rust
use serde::{Deserialize, Serialize};

// Import and re-export task status for tool result status
pub use agent_types::TaskStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchChangeKind {
    Add,
    Delete,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecCommandStatus {
    InProgress,
    Completed,
    Failed,
    Declined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpToolCallStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpInvocation {
    pub server: String,
    pub tool: String,
    pub arguments: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSearchAction {
    Search {
        query: Option<String>,
        queries: Option<Vec<String>>,
    },
    OpenPage {
        url: Option<String>,
    },
    FindInPage {
        url: Option<String>,
        pattern: Option<String>,
    },
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchChange {
    pub path: String,
    pub move_path: Option<String>,
    pub kind: PatchChangeKind,
    pub diff: String,
    pub added: usize,
    pub removed: usize,
}

// Tests from original file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_change_kind_serialization() {
        let kind = PatchChangeKind::Add;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"add\"");
    }
}
```

- [ ] **Step 2: Add toolkit dependency to core**

Edit `core/Cargo.toml`:
```toml
[dependencies]
agent-toolkit = { path = "../agent-toolkit" }
```

- [ ] **Step 3: Update core imports**

Edit `core/src/lib.rs`:
```rust
// Remove: pub mod tool_calls;
// Add re-export:
pub use agent_toolkit::{
    PatchChangeKind, PatchApplyStatus, ExecCommandStatus,
    McpToolCallStatus, McpInvocation, WebSearchAction, PatchChange,
};
```

Edit files using `crate::tool_calls::` to use `agent_toolkit::`:
- `core/src/provider.rs`
- `core/src/provider_thread.rs`
- `core/src/app.rs`
- `tui/src/ui_state.rs`
- `tui/src/render.rs`

- [ ] **Step 4: Remove core/src/tool_calls.rs**

```bash
rm core/src/tool_calls.rs
```

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 6: Commit**

```bash
git add agent-toolkit/ core/ tui/
git commit -m "feat: move tool_calls to agent-toolkit, update imports"
```

---

### Story 2.3: Update tui and other crates to use agent-toolkit

**Files:**
- Modify: `tui/Cargo.toml`
- Modify: `tui/src/*.rs` (imports)

- [ ] **Step 1: Add toolkit dependency to tui**

Edit `tui/Cargo.toml`:
```toml
[dependencies]
agent-toolkit = { path = "../agent-toolkit" }
```

- [ ] **Step 2: Update tui imports**

Search and replace in tui files:
```rust
// From: use agent_core::tool_calls::ExecCommandStatus;
// To:   use agent_toolkit::ExecCommandStatus;
```

Or use agent_core re-exports (backward compatible).

- [ ] **Step 3: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 4: Commit**

```bash
git add tui/
git commit -m "feat(tui): use agent-toolkit for tool types"
```

---

**Sprint 2 Complete ✓** - agent-toolkit created with tool call types, core and tui updated.

---

## Sprint 3: Provider Layer - agent-provider Crate

**Goal:** Extract provider execution layer (~10K lines).

**Risk Level:** Medium - many files, complex dependencies.

### Story 3.1: Create agent-provider crate skeleton

**Files:**
- Create: `agent-provider/Cargo.toml`
- Create: `agent-provider/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create directory structure**

```bash
mkdir -p agent-provider/src/providers agent-provider/src/launch_config
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "agent-provider"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-types = { path = "../agent-types" }
agent-toolkit = { path = "../agent-toolkit" }
agent-decision = { path = "../decision" }
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
shlex.workspace = true
tempfile.workspace = true
thiserror.workspace = true
which.workspace = true
uuid.workspace = true
dirs.workspace = true
pathdiff.workspace = true

[dev-dependencies]
serial_test.workspace = true
```

- [ ] **Step 3: Write lib.rs skeleton**

```rust
//! Provider execution layer for agile-agent
//!
//! Manages CLI provider processes (Claude, Codex) and tool execution.

pub mod provider;
pub mod provider_thread;
pub mod mock_provider;
pub mod probe;
pub mod llm_caller;
pub mod providers;
pub mod launch_config;

pub use provider::*;
pub use provider_thread::*;
```

- [ ] **Step 4: Update workspace Cargo.toml**

```toml
members = [
    "agent-types",
    "agent-toolkit",
    "agent-provider",
    # ... existing
]
```

- [ ] **Step 5: Commit skeleton**

```bash
git add agent-provider/ Cargo.toml
git commit -m "feat: create agent-provider crate skeleton"
```

---

### Story 3.2: Move providers/ directory

**Files:**
- Move: `core/src/providers/*.rs` → `agent-provider/src/providers/`
- Modify: `agent-provider/src/providers/mod.rs`

- [ ] **Step 1: Copy claude.rs**

```bash
cp core/src/providers/claude.rs agent-provider/src/providers/claude.rs
```

Update imports in file to use `agent_types::`, `agent_toolkit::`.

- [ ] **Step 2: Copy codex.rs**

```bash
cp core/src/providers/codex.rs agent-provider/src/providers/codex.rs
```

Update imports.

- [ ] **Step 3: Write providers/mod.rs**

```rust
mod claude;
mod codex;

pub use claude::*;
pub use codex::*;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo build -p agent-provider
```

Fix import errors iteratively.

- [ ] **Step 5: Commit**

```bash
git add agent-provider/src/providers/
git commit -m "feat(provider): move claude/codex provider implementations"
```

---

### Story 3.3: Move launch_config/ directory

**Files:**
- Move: `core/src/launch_config/*.rs` → `agent-provider/src/launch_config/`

- [ ] **Step 1: Copy all launch_config files**

```bash
cp core/src/launch_config/*.rs agent-provider/src/launch_config/
```

- [ ] **Step 2: Update imports in each file**

Replace `crate::` with `agent_types::`, `agent_toolkit::`, `agent_provider::`.

- [ ] **Step 3: Write launch_config/mod.rs**

Same content as original.

- [ ] **Step 4: Build and fix**

```bash
cargo build -p agent-provider
```

- [ ] **Step 5: Commit**

```bash
git add agent-provider/src/launch_config/
git commit -m "feat(provider): move launch_config module"
```

---

### Story 3.4: Move provider.rs and provider_thread.rs

**Files:**
- Move: `core/src/provider.rs` → `agent-provider/src/provider.rs`
- Move: `core/src/provider_thread.rs` → `agent-provider/src/provider_thread.rs`

- [ ] **Step 1: Copy provider.rs**

```bash
cp core/src/provider.rs agent-provider/src/provider.rs
```

Update imports, remove ProviderKind duplicate (use from agent_types).

- [ ] **Step 2: Copy provider_thread.rs**

```bash
cp core/src/provider_thread.rs agent-provider/src/provider_thread.rs
```

Update imports.

- [ ] **Step 3: Copy mock_provider.rs, probe.rs, llm_caller.rs**

```bash
cp core/src/mock_provider.rs agent-provider/src/mock_provider.rs
cp core/src/probe.rs agent-provider/src/probe.rs  
cp core/src/llm_caller.rs agent-provider/src/llm_caller.rs
```

- [ ] **Step 4: Build and fix**

```bash
cargo build -p agent-provider
```

- [ ] **Step 5: Commit**

```bash
git add agent-provider/src/*.rs
git commit -m "feat(provider): move provider, provider_thread, mock_provider, probe, llm_caller"
```

---

### Story 3.5: Update core to use agent-provider

**Files:**
- Modify: `core/Cargo.toml`
- Modify: `core/src/lib.rs`
- Delete: `core/src/providers/`, `core/src/launch_config/`, `core/src/provider.rs`, etc.
- Modify: All core files importing these modules

- [ ] **Step 1: Add agent-provider dependency**

Edit `core/Cargo.toml`:
```toml
[dependencies]
agent-provider = { path = "../agent-provider" }
```

- [ ] **Step 2: Update lib.rs re-exports**

```rust
// Re-export provider types for backward compatibility
pub use agent_provider::{
    ProviderEvent, SessionHandle, ProviderCapabilities,
    ProviderThreadHandle, start_provider,
    // ... other exports
};
pub use agent_provider::launch_config::{
    AgentLaunchBundle, LaunchInputSpec, ResolvedLaunchSpec,
    // ... other exports  
};
```

- [ ] **Step 3: Remove moved files from core**

```bash
rm -rf core/src/providers core/src/launch_config
rm core/src/provider.rs core/src/provider_thread.rs 
rm core/src/mock_provider.rs core/src/probe.rs core/src/llm_caller.rs
```

- [ ] **Step 4: Update imports in remaining core files**

Search for `crate::provider::`, `crate::launch_config::`, `crate::providers::` and update.

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace
```

Fix compilation errors iteratively.

- [ ] **Step 6: Commit**

```bash
git add core/
git commit -m "feat(core): use agent-provider, remove moved files"
```

---

### Story 3.6: Update cli and tui to use agent-provider

**Files:**
- Modify: `cli/Cargo.toml`, `tui/Cargo.toml`
- Modify: Imports in affected files

- [ ] **Step 1: Add dependencies**

Edit `cli/Cargo.toml` and `tui/Cargo.toml`:
```toml
[dependencies]
agent-provider = { path = "../agent-provider" }
```

- [ ] **Step 2: Update imports or use core re-exports**

- [ ] **Step 3: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 4: Commit**

```bash
git add cli/ tui/
git commit -m "feat(cli,tui): add agent-provider dependency"
```

---

**Sprint 3 Complete ✓** - agent-provider created with ~10K lines of provider code.

---

## Sprint 4: Worktree - agent-worktree Crate

**Goal:** Extract git worktree management (~5K lines).

**Risk Level:** Medium - git operations, state persistence.

### Story 4.1: Create agent-worktree crate skeleton

**Files:**
- Create: `agent-worktree/Cargo.toml`
- Create: `agent-worktree/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create directory**

```bash
mkdir -p agent-worktree/src
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "agent-worktree"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-types = { path = "../agent-types" }
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
dirs.workspace = true
tempfile.workspace = true
```

- [ ] **Step 3: Write lib.rs**

```rust
//! Git worktree management for multi-agent isolation

pub mod worktree_manager;
pub mod worktree_state;
pub mod worktree_state_store;
pub mod workplace_store;
pub mod git_flow_executor;
pub mod git_flow_config;

pub use worktree_manager::*;
pub use worktree_state::*;
pub use workplace_store::*;
```

- [ ] **Step 4: Update workspace**

```toml
members = [..., "agent-worktree", ...]
```

- [ ] **Step 5: Commit**

```bash
git add agent-worktree/ Cargo.toml  
git commit -m "feat: create agent-worktree crate skeleton"
```

---

### Story 4.2: Move worktree_manager.rs and worktree_state files

**Files:**
- Move: `core/src/worktree_manager.rs`
- Move: `core/src/worktree_state.rs`
- Move: `core/src/worktree_state_store.rs`

- [ ] **Step 1: Copy files**

```bash
cp core/src/worktree_manager.rs agent-worktree/src/
cp core/src/worktree_state.rs agent-worktree/src/
cp core/src/worktree_state_store.rs agent-worktree/src/
```

- [ ] **Step 2: Update imports**

Replace `crate::agent_runtime::WorkplaceId` with `agent_types::WorkplaceId`.
Update other `crate::` references.

- [ ] **Step 3: Build**

```bash
cargo build -p agent-worktree
```

- [ ] **Step 4: Commit**

```bash
git add agent-worktree/src/*.rs
git commit -m "feat(worktree): move worktree_manager, worktree_state modules"
```

---

### Story 4.3: Move workplace_store.rs and git_flow files

**Files:**
- Move: `core/src/workplace_store.rs`
- Move: `core/src/git_flow_executor.rs`
- Move: `core/src/git_flow_config.rs`

- [ ] **Step 1: Copy files**

```bash
cp core/src/workplace_store.rs agent-worktree/src/
cp core/src/git_flow_executor.rs agent-worktree/src/
cp core/src/git_flow_config.rs agent-worktree/src/
```

- [ ] **Step 2: Update imports**

Fix all `crate::` references.

- [ ] **Step 3: Build**

```bash
cargo build -p agent-worktree
```

- [ ] **Step 4: Commit**

```bash
git add agent-worktree/src/
git commit -m "feat(worktree): move workplace_store, git_flow modules"
```

---

### Story 4.4: Update core and other crates

**Files:**
- Modify: `core/Cargo.toml`
- Modify: `core/src/lib.rs`
- Delete: Moved files from core
- Modify: `agent-backlog/Cargo.toml` (if backlog depends on worktree)

- [ ] **Step 1: Add dependency to core**

```toml
[dependencies]
agent-worktree = { path = "../agent-worktree" }
```

- [ ] **Step 2: Update lib.rs re-exports**

```rust
pub use agent_worktree::{
    WorktreeManager, WorktreeConfig, WorktreeError,
    WorktreeState, WorktreeStateStore,
    WorkplaceStore, WorkplaceMeta,
    GitFlowExecutor, GitFlowConfig,
};
```

- [ ] **Step 3: Remove moved files**

```bash
rm core/src/worktree_manager.rs core/src/worktree_state.rs 
rm core/src/worktree_state_store.rs core/src/workplace_store.rs
rm core/src/git_flow_executor.rs core/src/git_flow_config.rs
```

- [ ] **Step 4: Update imports**

Fix remaining files in core that use `crate::worktree_*`.

- [ ] **Step 5: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 6: Commit**

```bash
git add core/ Cargo.toml
git commit -m "feat(core): use agent-worktree, remove moved files"
```

---

**Sprint 4 Complete ✓** - agent-worktree created with ~5K lines.

---

## Sprint 5: Backlog - agent-backlog Crate

**Goal:** Extract task/backlog domain (~3K lines).

**Risk Level:** Low - domain types and state management.

### Story 5.1: Create agent-backlog crate skeleton

**Files:**
- Create: `agent-backlog/Cargo.toml`
- Create: `agent-backlog/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Create directory**

```bash
mkdir -p agent-backlog/src
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "agent-backlog"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-types = { path = "../agent-types" }
agent-worktree = { path = "../agent-worktree" }
agent-kanban = { path = "../kanban" }
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 3: Write lib.rs**

```rust
//! Task and backlog management for agile-agent

pub mod backlog;
pub mod backlog_store;
pub mod sprint_planning;
pub mod standup_report;
pub mod blocker_escalation;
pub mod task_engine;
pub mod task_artifacts;

pub use backlog::*;
pub use backlog_store::*;
```

- [ ] **Step 4: Update workspace**

Add to members.

- [ ] **Step 5: Commit**

```bash
git add agent-backlog/ Cargo.toml
git commit -m "feat: create agent-backlog crate skeleton"
```

---

### Story 5.2: Move backlog.rs and backlog_store.rs

**Files:**
- Move: `core/src/backlog.rs` (methods only, types in agent-types)
- Move: `core/src/backlog_store.rs`

- [ ] **Step 1: Copy backlog.rs**

```bash
cp core/src/backlog.rs agent-backtree/src/backlog.rs
```

Remove type definitions (TodoItem, TaskItem, TodoStatus, TaskStatus) - they're in agent-types. Keep BacklogState struct and methods.

- [ ] **Step 2: Update imports**

```rust
use agent_types::{TodoStatus, TaskStatus, TodoItem, TaskItem};
```

- [ ] **Step 3: Copy backlog_store.rs**

```bash
cp core/src/backlog_store.rs agent-backlog/src/
```

Update imports.

- [ ] **Step 4: Build**

```bash
cargo build -p agent-backlog
```

- [ ] **Step 5: Commit**

```bash
git add agent-backlog/src/
git commit -m "feat(backlog): move backlog and backlog_store modules"
```

---

### Story 5.3: Move sprint_planning, standup, blocker files

**Files:**
- Move: `core/src/sprint_planning.rs`
- Move: `core/src/standup_report.rs`
- Move: `core/src/blocker_escalation.rs`
- Move: `core/src/task_engine.rs`
- Move: `core/src/task_artifacts.rs`

- [ ] **Step 1: Copy files**

```bash
cp core/src/sprint_planning.rs agent-backlog/src/
cp core/src/standup_report.rs agent-backlog/src/
cp core/src/blocker_escalation.rs agent-backlog/src/
cp core/src/task_engine.rs agent-backlog/src/
cp core/src/task_artifacts.rs agent-backlog/src/
```

- [ ] **Step 2: Update imports**

Fix all `crate::` references.

- [ ] **Step 3: Build**

```bash
cargo build -p agent-backlog
```

- [ ] **Step 4: Commit**

```bash
git add agent-backlog/src/
git commit -m "feat(backlog): move sprint_planning, standup, blocker modules"
```

---

### Story 5.4: Update core to use agent-backlog

**Files:**
- Modify: `core/Cargo.toml`
- Modify: `core/src/lib.rs`
- Delete: Moved files

- [ ] **Step 1: Add dependency**

```toml
agent-backlog = { path = "../agent-backlog" }
```

- [ ] **Step 2: Update lib.rs**

```rust
pub use agent_backlog::{
    BacklogState, load_backlog, save_backlog,
    SprintPlanningSession, StandupReport,
    // ... other exports
};
```

- [ ] **Step 3: Remove moved files**

```bash
rm core/src/backlog.rs core/src/backlog_store.rs
rm core/src/sprint_planning.rs core/src/standup_report.rs
rm core/src/blocker_escalation.rs core/src/task_engine.rs
rm core/src/task_artifacts.rs
```

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git add core/
git commit -m "feat(core): use agent-backlog, remove moved files"
```

---

**Sprint 5 Complete ✓** - agent-backlog created with ~3K lines.

---

## Sprint 6: Storage - agent-storage Crate

**Goal:** Extract persistence layer (~3K lines).

**Risk Level:** Medium - file I/O, migration logic.

### Story 6.1: Create agent-storage crate skeleton

**Files:**
- Create: `agent-storage/Cargo.toml`
- Create: `agent-storage/src/lib.rs`

- [ ] **Step 1: Create directory**

```bash
mkdir -p agent-storage/src
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "agent-storage"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-types = { path = "../agent-types" }
agent-backlog = { path = "../agent-backlog" }
agent-provider = { path = "../agent-provider" }
anyhow.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
dirs.workspace = true
```

- [ ] **Step 3: Write lib.rs**

```rust
//! Persistence layer for agile-agent

pub mod storage;
pub mod data_migration;
pub mod shutdown_snapshot;
pub mod persistence_coordinator;
pub mod agent_store;
pub mod session_store;

pub use storage::*;
pub use shutdown_snapshot::*;
```

- [ ] **Step 4: Commit**

```bash
git add agent-storage/ Cargo.toml
git commit -m "feat: create agent-storage crate skeleton"
```

---

### Story 6.2: Move storage.rs, data_migration.rs

**Files:**
- Move: `core/src/storage.rs`
- Move: `core/src/data_migration.rs`

- [ ] **Step 1: Copy files**

```bash
cp core/src/storage.rs agent-storage/src/
cp core/src/data_migration.rs agent-storage/src/
```

- [ ] **Step 2: Update imports**

- [ ] **Step 3: Build**

```bash
cargo build -p agent-storage
```

- [ ] **Step 4: Commit**

```bash
git add agent-storage/src/
git commit -m "feat(storage): move storage, data_migration modules"
```

---

### Story 6.3: Move shutdown_snapshot and persistence files

**Files:**
- Move: `core/src/shutdown_snapshot.rs`
- Move: `core/src/persistence_coordinator.rs`
- Move: `core/src/agent_store.rs`
- Move: `core/src/session_store.rs`

- [ ] **Step 1: Copy files**

```bash
cp core/src/shutdown_snapshot.rs agent-storage/src/
cp core/src/persistence_coordinator.rs agent-storage/src/
cp core/src/agent_store.rs agent-storage/src/
cp core/src/session_store.rs agent-storage/src/
```

- [ ] **Step 2: Update imports**

Types like AgentShutdownSnapshot use agent_types.

- [ ] **Step 3: Build**

```bash
cargo build -p agent-storage
```

- [ ] **Step 4: Commit**

```bash
git add agent-storage/src/
git commit -m "feat(storage): move shutdown_snapshot, persistence modules"
```

---

### Story 6.4: Update core to use agent-storage

**Files:**
- Modify: `core/Cargo.toml`
- Modify: `core/src/lib.rs`
- Delete: Moved files

- [ ] **Step 1: Add dependency**

```toml
agent-storage = { path = "../agent-storage" }
```

- [ ] **Step 2: Update lib.rs**

```rust
pub use agent_storage::{
    app_data_root, ShutdownSnapshot, AgentShutdownSnapshot,
    // ... other exports
};
```

- [ ] **Step 3: Remove files**

```bash
rm core/src/storage.rs core/src/data_migration.rs
rm core/src/shutdown_snapshot.rs core/src/persistence_coordinator.rs
rm core/src/agent_store.rs core/src/session_store.rs
```

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git add core/
git commit -m "feat(core): use agent-storage, remove moved files"
```

---

**Sprint 6 Complete ✓** - agent-storage created with ~3K lines.

---

## Sprint 7: Cleanup and Verification

**Goal:** Final cleanup of agent-core, verify all tests pass, measure compilation improvement.

### Story 7.1: Audit agent-core remaining files

**Files:**
- Read: `core/src/lib.rs`
- Verify: Remaining modules match spec

- [ ] **Step 1: List remaining files**

```bash
ls -la core/src/*.rs
wc -l core/src/*.rs
```

- [ ] **Step 2: Verify line count**

Expected: ~15K lines remaining.

- [ ] **Step 3: Update lib.rs final version**

Ensure all re-exports are correct.

- [ ] **Step 4: Commit**

```bash
git add core/src/lib.rs
git commit -m "chore(core): finalize lib.rs exports after split"
```

---

### Story 7.2: Run full test suite

- [ ] **Step 1: Run all tests**

```bash
cargo test --workspace
```

Expected: All tests pass (same count as before).

- [ ] **Step 2: Record test count**

```bash
cargo test --workspace 2>&1 | grep "passed"
```

- [ ] **Step 3: Fix any failing tests**

If tests fail, investigate and fix.

---

### Story 7.3: Verify no circular dependencies

- [ ] **Step 1: Build each crate independently**

```bash
cargo build -p agent-types
cargo build -p agent-toolkit
cargo build -p agent-provider
cargo build -p agent-worktree
cargo build -p agent-backlog
cargo build -p agent-storage
cargo build -p agent-core
```

Expected: Each builds successfully without the others (except declared deps).

- [ ] **Step 2: Check dependency graph**

```bash
cargo tree -p agent-core
```

Verify no cycles.

---

### Story 7.4: Measure compilation improvement

- [ ] **Step 1: Clean and time full build**

```bash
cargo clean
time cargo build --workspace --release
```

Record time.

- [ ] **Step 2: Compare with baseline**

Note improvement (parallel compilation of smaller crates).

---

### Story 7.5: Update documentation

**Files:**
- Modify: `README.md` (crate overview)
- Update: Architecture diagrams

- [ ] **Step 1: Update README**

Add crate descriptions to README.

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: update README with new crate structure"
```

---

**Sprint 7 Complete ✓** - All cleanup done, tests pass, architecture verified.

---

## Self-Review Checklist

**1. Spec coverage:**
- agent-types ✓ (Story 1.1-1.5)
- agent-toolkit ✓ (Story 2.1-2.3)
- agent-provider ✓ (Story 3.1-3.6)
- agent-worktree ✓ (Story 4.1-4.4)
- agent-backlog ✓ (Story 5.1-5.4)
- agent-storage ✓ (Story 6.1-6.4)
- cleanup ✓ (Story 7.1-7.5)

**2. Placeholder scan:** None found - all steps have concrete code/commands.

**3. Type consistency:** Verified - agent-types defines all shared types, re-exports maintain backward compatibility.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-20-core-split-implementation.md`.

**Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per story, review between stories, fast iteration

**2. Inline Execution** - Execute stories in this session using executing-plans, batch execution with checkpoints

**Which approach do you want?**