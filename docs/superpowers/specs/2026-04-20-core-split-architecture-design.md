# Core Package Split Architecture Design

## Summary

Split the 71,255-line `agent-core` package into 7 focused crates with clear dependency boundaries and no circular dependencies.

## Current Problem

`agent-core` has grown to **71,255 lines** across 57 modules, making it:
- Hard to understand and navigate
- Slow to compile (single large crate)
- Difficult to test in isolation
- Risky to refactor (everything depends on everything)

### Critical Circular Dependency Found

```
workplace_store → agent_runtime::WorkplaceId
agent_runtime → workplace_store (via workplace_store::WorkplaceStore)
```

This prevents any natural split without introducing a foundation types crate.

## Proposed Architecture

### Crate Dependency Graph

```
                    ┌─────────────────┐
                    │   agent-types   │  ← Foundation (pure types)
                    │   (~500 lines)  │
                    └─────────────────┘
                           │
       ┌───────────────────┼───────────────────┬───────────────────┐
       │                   │                   │                   │
       ▼                   ▼                   ▼                   ▼
┌────────────┐      ┌────────────┐      ┌────────────┐      ┌────────────┐
│agent-toolkit│      │agent-provider│    │agent-worktree│    │agent-backlog│
│(~800 lines)│      │(~10K lines) │      │(~5K lines) │      │(~3K lines) │
│tool_calls  │      │providers    │      │git/worktree│      │tasks/sprint│
└────────────┘      └────────────┘      └────────────┘      └────────────┘
       │                   │                   │                   │
       └───────────────────┼───────────────────┼───────────────────┤
                           │                   │                   │
                           ▼                   ▼                   ▼
                    ┌────────────┐
                    │agent-storage│
                    │(~3K lines) │
                    │persistence │
                    └────────────┘
                           │
                           ▼
                    ┌────────────┐
                    │ agent-core │
                    │(~15K lines)│
                    │coordination│
                    └────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
       ┌───────────┐             ┌───────────┐
       │ agent-cli │             │ agent-tui │
       └───────────┘             └───────────┘
```

### Dependency Flow

```
agent-types ─────────────────────────────────────────────────────────────────┐
                                                                               │
agent-toolkit ← agent-types                                                   │
                                                                               │
agent-provider ← agent-types, agent-toolkit, agent-decision                   │
                                                                               │
agent-worktree ← agent-types                                                   │
                                                                               │
agent-backlog ← agent-types, agent-worktree, agent-kanban                      │
                                                                               │
agent-storage ← agent-types, agent-backlog, agent-provider                     │
                                                                               │
agent-core ← agent-types, agent-toolkit, agent-provider, agent-worktree,      │
             agent-backlog, agent-storage, agent-decision, agent-kanban        │
                                                                               │
agent-cli ← agent-core                                                         │
agent-tui ← agent-core                                                         │
```

## Crate Specifications

### 1. agent-types (~500 lines)

**Purpose:** Foundation crate with pure data types. No implementation dependencies.

**Contents:**
```
agent-types/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── agent_id.rs        # AgentId, WorkplaceId, AgentCodename
    ├── agent_status.rs    # AgentStatus enum
    ├── task_status.rs     # TaskStatus, TodoStatus enums
    ├── provider_type.rs   # ProviderType, ProviderKind enums
    └── task_types.rs      # TaskId, TaskItem, TodoItem structs
```

**Files extracted from core:**
- `agent_runtime.rs` lines 1-80 (AgentId, WorkplaceId, AgentCodename, AgentStatus)
- `backlog.rs` lines 1-60 (TaskStatus, TodoStatus, TodoItem, TaskItem)
- `provider.rs` lines 1-30 (ProviderKind)

**Dependencies:** `serde` only (no anyhow, no chrono, no internal crates)

**API:**
```rust
pub struct AgentId(String);
pub struct WorkplaceId(String);
pub struct AgentCodename(String);
pub enum AgentStatus { Idle, Running, Stopped }
pub enum TaskStatus { Draft, Ready, Running, Verifying, Done, Blocked, Failed }
pub enum TodoStatus { Candidate, Ready, InProgress, Blocked, Done, Dropped }
pub enum ProviderKind { Mock, Claude, Codex }
pub struct TodoItem { id, title, description, priority, status, ... }
pub struct TaskItem { id, todo_id, objective, scope, ... }
```

---

### 2. agent-toolkit (~800 lines)

**Purpose:** Tool call types independent of provider implementation.

**Contents:**
```
agent-toolkit/
├── Cargo.toml
└── src/
    ├── lib.rs
    └── tool_calls.rs      # All tool-related types
```

**Files extracted from core:**
- `tool_calls.rs` (entire file, 71 lines)

**Dependencies:** `agent-types`, `serde`

**API:**
```rust
pub enum PatchChangeKind { Add, Delete, Update }
pub enum PatchApplyStatus { InProgress, Completed, Failed, Declined }
pub enum ExecCommandStatus { InProgress, Completed, Failed, Declined }
pub enum McpToolCallStatus { InProgress, Completed, Failed }
pub struct McpInvocation { server, tool, arguments }
pub enum WebSearchAction { Search, OpenPage, FindInPage, Other }
pub struct PatchChange { path, move_path, kind, diff, added, removed }
```

---

### 3. agent-provider (~10,000 lines)

**Purpose:** Provider execution layer - CLI process management, tool execution, LLM calls.

**Contents:**
```
agent-provider/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── provider.rs            # ProviderEvent, SessionHandle, ProviderCapabilities
    ├── provider_thread.rs     # Thread management (1133 lines)
    ├── mock_provider.rs       # Mock provider for testing
    ├── probe.rs               # Provider detection
    ├── llm_caller.rs          # LLM API caller
    ├── providers/
    │   ├── mod.rs
    │   ├── claude.rs          # Claude CLI (777 lines)
    │   └── codex.rs           # Codex CLI (1320 lines)
    └── launch_config/
        ├── mod.rs
        ├── context.rs         # ProviderLaunchContext
        ├── error.rs
        ├── parser.rs          # Config parsing
        ├── persistence.rs     # Config storage
        ├── redaction.rs       # Secret redaction
        ├── resolver.rs        # Config resolution
        ├── restore.rs         # Restore handling
        ├── spec.rs            # LaunchInputSpec, ResolvedLaunchSpec
        └── validation.rs      # Config validation
```

**Files extracted from core:**
- `provider.rs` (lines 22-150: ProviderKind, ProviderEvent, SessionHandle, ProviderCapabilities)
- `provider_thread.rs` (entire file, 1133 lines)
- `mock_provider.rs` (entire file)
- `probe.rs` (entire file)
- `llm_caller.rs` (entire file)
- `providers/claude.rs` (entire file, 777 lines)
- `providers/codex.rs` (entire file, 1320 lines)
- `launch_config/` (entire directory, 2146 lines)

**Dependencies:** `agent-types`, `agent-toolkit`, `agent-decision`, `anyhow`, `chrono`, `serde`, `shlex`, `tempfile`, `which`

**API:**
```rust
pub enum ProviderEvent { Status, Finished, Error, AssistantChunk, ThinkingChunk, ... }
pub enum SessionHandle { ClaudeSession { session_id }, CodexThread { thread_id } }
pub struct ProviderCapabilities { supports_slash_passthrough: bool }
pub fn start_provider(...) -> ProviderThreadHandle
pub fn detect_provider_kind() -> Option<ProviderKind>
```

---

### 4. agent-worktree (~5,000 lines)

**Purpose:** Git worktree management, workplace state, git flow operations.

**Contents:**
```
agent-worktree/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── worktree_manager.rs       # Git worktree operations (1758 lines)
    ├── worktree_state.rs         # Worktree state model (348 lines)
    ├── worktree_state_store.rs   # State persistence (427 lines)
    ├── workplace_store.rs        # Workplace directory management (928 lines)
    ├── git_flow_executor.rs      # Git workflow executor (~600 lines)
    └── git_flow_config.rs        # Git workflow config (~300 lines)
```

**Files extracted from core:**
- `worktree_manager.rs` (entire file, 1758 lines)
- `worktree_state.rs` (entire file, 348 lines)
- `worktree_state_store.rs` (entire file, 427 lines)
- `workplace_store.rs` (entire file, 928 lines) - move WorkplaceId reference to agent-types
- `git_flow_executor.rs` (entire file)
- `git_flow_config.rs` (entire file)

**Dependencies:** `agent-types`, `anyhow`, `serde`, `chrono`, `git2` (if used)

**API:**
```rust
pub struct WorktreeManager { ... }
pub struct WorktreeState { ... }
pub struct WorkplaceStore { ... }
pub fn create_worktree(...) -> Result<WorktreeConfig>
pub fn remove_worktree(...) -> Result<()>
```

---

### 5. agent-backlog (~3,000 lines)

**Purpose:** Task/backlog domain, sprint planning, standup reports.

**Contents:**
```
agent-backlog/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── backlog.rs           # BacklogState (639 lines)
    ├── backlog_store.rs     # Backlog persistence (~200 lines)
    ├── sprint_planning.rs   # Sprint planning (~400 lines)
    ├── standup_report.rs    # Daily standup (670 lines)
    ├── blocker_escalation.rs # Blocker handling (689 lines)
    ├── task_engine.rs       # Task execution (~300 lines)
    └── task_artifacts.rs    # Task artifacts (~200 lines)
```

**Files extracted from core:**
- `backlog.rs` (lines 60+: BacklogState methods, keep types in agent-types)
- `backlog_store.rs` (entire file)
- `sprint_planning.rs` (entire file)
- `standup_report.rs` (entire file)
- `blocker_escalation.rs` (entire file)
- `task_engine.rs` (entire file)
- `task_artifacts.rs` (entire file)

**Dependencies:** `agent-types`, `agent-worktree`, `agent-kanban`, `anyhow`, `serde`, `chrono`

**API:**
```rust
pub struct BacklogState { todos, tasks }
pub fn load_backlog() -> Result<BacklogState>
pub fn save_backlog(backlog: &BacklogState) -> Result<()>
pub fn run_sprint_planning(...) -> Result<SprintPlan>
```

---

### 6. agent-storage (~3,000 lines)

**Purpose:** Persistence layer - storage utilities, migration, shutdown snapshots.

**Contents:**
```
agent-storage/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── storage.rs               # App data root utilities (10 lines)
    ├── data_migration.rs        # Migration handling (582 lines)
    ├── shutdown_snapshot.rs     # Shutdown state capture (~400 lines)
    ├── persistence_coordinator.rs # Coordination (~300 lines)
    ├── agent_store.rs           # Agent store (~400 lines)
    └── session_store.rs         # Session store (~300 lines)
```

**Files extracted from core:**
- `storage.rs` (entire file)
- `data_migration.rs` (entire file)
- `shutdown_snapshot.rs` (entire file, 777 lines) - types move to agent-types
- `persistence_coordinator.rs` (entire file)
- `agent_store.rs` (entire file)
- `session_store.rs` (entire file)

**Dependencies:** `agent-types`, `agent-backlog`, `agent-provider`, `anyhow`, `serde`, `chrono`

**API:**
```rust
pub fn app_data_root() -> Result<PathBuf>
pub struct ShutdownSnapshot { shutdown_at, workplace_id, agents, backlog, ... }
pub struct AgentShutdownSnapshot { meta, assigned_task_id, was_active, ... }
pub fn load_session(...) -> Result<SessionSnapshot>
pub fn save_session(...) -> Result<()>
```

---

### 7. agent-core (shrinks to ~15,000 lines)

**Purpose:** Coordination layer - agent pool management, runtime, app state.

**Contents:**
```
agent-core/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── agent_pool.rs           # Multi-agent coordination (shrinks)
    ├── agent_slot.rs           # Agent slot runtime (shrinks)
    ├── agent_runtime.rs        # Agent runtime (shrinks, types move to agent-types)
    ├── agent_state.rs          # Agent state (589 lines)
    ├── agent_memory.rs         # Agent memory
    ├── agent_messages.rs       # Agent messages
    ├── agent_transcript.rs     # Agent transcript
    ├── agent_mail.rs           # Cross-agent mail (1026 lines)
    ├── decision_agent_slot.rs  # Decision agent (1473 lines)
    ├── decision_mail.rs        # Decision mail
    ├── app.rs                  # App state (1418 lines)
    ├── runtime_session.rs      # Runtime session (734 lines)
    ├── shared_state.rs         # Shared workplace state (590 lines)
    ├── event_aggregator.rs     # Event aggregation
    ├── runtime_mode.rs         # Runtime mode
    ├── autonomy.rs             # Autonomy config
    ├── loop_runner.rs          # Loop execution
    ├── verification.rs         # Verification
    ├── skills.rs               # Skills registry
    ├── commands.rs             # Command handling
    ├── command_bus/            # Command bus module
    └── multi_agent_session.rs  # Multi-agent session (953 lines)
```

**Dependencies:** 
- `agent-types`, `agent-toolkit`, `agent-provider`, `agent-worktree`, `agent-backlog`, `agent-storage`
- `agent-decision`, `agent-kanban`
- `anyhow`, `chrono`, `serde`, `uuid`

**API:** (unchanged, same public API as current core)

---

## Implementation Order

Phase-by-phase extraction to minimize risk:

### Phase 1: agent-types (lowest risk)
1. Create `agent-types` crate
2. Extract AgentId, WorkplaceId, AgentCodename from agent_runtime.rs
3. Extract TaskStatus, TodoStatus, TodoItem, TaskItem from backlog.rs
4. Extract ProviderKind from provider.rs
5. Update all crates to use agent-types

### Phase 2: agent-toolkit
1. Create `agent-toolkit` crate
2. Move tool_calls.rs entirely
3. Update core and provider to depend on toolkit

### Phase 3: agent-provider
1. Create `agent-provider` crate
2. Move providers/, launch_config/, provider_thread.rs
3. Update core to depend on provider

### Phase 4: agent-worktree
1. Create `agent-worktree` crate
2. Move worktree_manager.rs, worktree_state.rs, workplace_store.rs
3. Update core and backlog to depend on worktree

### Phase 5: agent-backlog
1. Create `agent-backlog` crate
2. Move backlog.rs (excluding types), backlog_store.rs, sprint_planning.rs
3. Update core to depend on backlog

### Phase 6: agent-storage
1. Create `agent-storage` crate
2. Move storage.rs, shutdown_snapshot.rs, persistence_coordinator.rs
3. Update core to depend on storage

### Phase 7: Cleanup
1. Remove extracted files from agent-core
2. Update lib.rs imports
3. Verify all tests pass
4. Update workspace Cargo.toml

---

## Success Criteria

1. All 7 crates compile independently
2. No circular dependencies between crates
3. All 799+ core tests pass after split
4. Total compilation time reduced (can compile crates in parallel)
5. Clear public API for each crate
6. Each crate has single focused purpose

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Circular dependencies | Start with agent-types foundation crate |
| Breaking tests | Run tests after each phase, fix incrementally |
| API changes | Keep same public API, use re-exports |
| Compilation errors | Phase-by-phase extraction, not all at once |
| Missing imports | Comprehensive dependency analysis before extraction |

---

## Post-Split Opportunities

After this split succeeds, agent-core at ~15K lines could be further split:
- **agent-session** (~3K): Runtime session, app state, autonomy
- **agent-coordination** (~5K): Agent pool, agent slot, mail system
- **agent-decision-integration** (~2K): Decision agent slot, decision mail

This would leave agent-core at ~5K lines as pure coordination logic.