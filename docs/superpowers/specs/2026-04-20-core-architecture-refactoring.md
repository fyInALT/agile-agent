# Core Crate Architecture Refactoring Plan

## Problem Analysis

### Why Files Are Too Large

**agent_pool.rs (5967 lines)**
- **Root Cause**: God Object pattern - one struct managing 10+ distinct responsibilities
- **Symptom**: Single impl block with 67+ public methods
- **Tests**: 2304 lines (38%) - tests are proportional to complexity

**agent_slot.rs (2043 lines)**
- **Root Cause**: Mixed concerns - status enum, slot struct, and multiple domain logic in one file
- **Symptom**: AgentSlotStatus + TaskId + TaskCompletionResult + ThreadOutcome + AgentSlot all together
- **Tests**: 668 lines (33%)

### Architectural Issues

1. **Violation of Single Responsibility Principle**
   - AgentPool handles: lifecycle, worktree, decision agents, profiles, tasks, blocked handling, focus, queries
   - AgentSlot handles: status transitions, task binding, thread management, worktree, transcript, profile

2. **Tight Coupling**
   - AgentPool directly creates/manages WorktreeManager, GitFlowExecutor, ProfileStore
   - Decision agent logic embedded in pool rather than separate coordinator

3. **Missing Abstractions**
   - No "AgentLifecycle" trait/module for spawn/stop/pause/resume
   - No "WorktreeCoordinator" for worktree-specific logic
   - No "DecisionAgentCoordinator" for decision agent lifecycle
   - No "TaskAssigner" for task assignment logic

---

## Proposed Refactoring

### 1. AgentPool Decomposition

**Current**: Single monolithic AgentPool (5967 lines)

**Proposed**: Decompose into focused modules:

```
core/src/
├── agent_pool.rs              (~200 lines) - Pure coordination/facade
├── pool/
│   ├── lifecycle.rs           (~150 lines) - AgentLifecycle trait + impl
│   ├── worktree_coordinator.rs (~300 lines) - WorktreeCoordinator
│   ├── decision_coordinator.rs (~400 lines) - DecisionAgentCoordinator
│   ├── task_assigner.rs        (~150 lines) - TaskAssigner
│   ├── blocked_handler.rs      (~300 lines) - BlockedHandler
│   ├── focus_manager.rs        (~100 lines) - FocusManager
│   ├── profile_manager.rs      (~150 lines) - ProfileManager
│   └── query.rs                (~100 lines) - StatusQuery trait
```

### 2. AgentSlot Decomposition

**Current**: Mixed types in agent_slot.rs (2043 lines)

**Proposed**: Separate domain types:

```
core/src/
├── agent_slot.rs              (~200 lines) - Core AgentSlot struct
├── slot/
│   ├── status.rs              (~350 lines) - AgentSlotStatus + transitions
│   ├── task_binding.rs        (~100 lines) - TaskId + task assignment logic
│   ├── thread_manager.rs      (~150 lines) - ProviderThread handling
│   ├── transcript_manager.rs  (~100 lines) - Transcript operations
│   ├── worktree_binding.rs    (~100 lines) - Worktree reference handling
│   └── recovery.rs            (~100 lines) - Resting state + recovery logic
```

---

## Migration Strategy

### Phase 1: Extract Pure Types (Low Risk)

1. **Extract AgentSlotStatus** → `slot/status.rs`
   - Move enum definition + all status-related methods
   - Update imports in agent_slot.rs
   - Tests: Move status tests to status.rs

2. **Extract TaskId** → `slot/task_binding.rs`
   - TaskId already exists in agent-types, this is duplicate
   - Consider removing from core/src/agent_slot.rs entirely

3. **Extract ThreadOutcome, TaskCompletionResult** → `slot/thread_manager.rs`
   - These are thread-completion related, belongs with thread management

### Phase 2: Extract Coordinator Patterns (Medium Risk)

1. **Create WorktreeCoordinator**
   - Extract: spawn_agent_with_worktree, pause_agent_with_worktree, resume_agent_with_worktree
   - Extract: recover_orphaned_worktrees, auto_cleanup_idle_worktrees
   - Coordinator owns: worktree_manager, worktree_state_store, git_flow_executor

2. **Create DecisionAgentCoordinator**
   - Extract: spawn_decision_agent_for, stop_decision_agent_for, poll_decision_agents
   - Extract: execute_decision_action, classify_event, send_decision_request
   - Coordinator owns: decision_agents HashMap, decision_mail_senders, decision_components

3. **Create BlockedHandler**
   - Extract: process_agent_blocked, blocked_agents, blocked_count
   - Extract: on_agent_blocked, process_human_response, clear_all_blocked
   - Handler owns: blocked_config, blocked_history, blocked_notifier

### Phase 3: Facade Pattern (Low Risk)

After coordinators exist, AgentPool becomes a facade:

```rust
pub struct AgentPool {
    slots: Vec<AgentSlot>,
    max_slots: usize,
    workplace_id: WorkplaceId,
    // Coordinators (delegated behavior)
    lifecycle: AgentLifecycle,
    worktree: Option<WorktreeCoordinator>,
    decision: DecisionAgentCoordinator,
    blocked: BlockedHandler,
    focus: FocusManager,
    profile: Option<ProfileManager>,
}
```

---

## New Module Definitions

### slot/status.rs (~350 lines)

```rust
/// Agent operational status
pub enum AgentSlotStatus {
    Idle, Starting, Responding, ToolExecuting, Finishing,
    Stopping, Stopped, Error, Blocked, Paused,
    BlockedForDecision, WaitingForInput, Resting,
}

impl AgentSlotStatus {
    pub fn idle() -> Self;
    pub fn starting() -> Self;
    pub fn can_transition_to(&self, target: &Self) -> bool;
    pub fn is_active(&self) -> bool;
    pub fn is_blocked(&self) -> bool;
    pub fn label(&self) -> String;
    // ... ~25 status methods
}
```

### pool/worktree_coordinator.rs (~300 lines)

```rust
pub struct WorktreeCoordinator {
    manager: WorktreeManager,
    state_store: WorktreeStateStore,
    git_flow_executor: GitFlowExecutor,
}

impl WorktreeCoordinator {
    pub fn spawn_with_worktree(&mut self, slot: &mut AgentSlot, ...) -> Result<...>;
    pub fn pause_agent(&mut self, slot: &mut AgentSlot) -> Result<...>;
    pub fn resume_agent(&mut self, slot: &mut AgentSlot) -> Result<...>;
    pub fn recover_orphaned(&mut self) -> Vec<...>;
    pub fn cleanup_idle(&mut self) -> Vec<...>;
}
```

### pool/decision_coordinator.rs (~400 lines)

```rust
pub struct DecisionAgentCoordinator {
    agents: HashMap<AgentId, DecisionAgentSlot>,
    mail_senders: HashMap<AgentId, DecisionMailSender>,
    components: DecisionLayerComponents,
}

impl DecisionAgentCoordinator {
    pub fn spawn_for(&mut self, work_agent: &AgentId, ...) -> Result<...>;
    pub fn stop_for(&mut self, work_agent: &AgentId) -> Result<...>;
    pub fn poll(&mut self) -> Vec<(AgentId, DecisionResponse)>;
    pub fn execute_action(&mut self, agent: &AgentId, action: ...) -> Result<...>;
    pub fn classify_event(&self, agent: &AgentId, event: ...) -> ClassifyResult;
}
```

### pool/blocked_handler.rs (~300 lines)

```rust
pub struct BlockedHandler {
    config: BlockedHandlingConfig,
    history: Vec<BlockedHistoryEntry>,
    notifier: Arc<dyn AgentBlockedNotifier>,
    human_queue: HumanDecisionQueue,
}

impl BlockedHandler {
    pub fn process_blocked(&mut self, slot: &mut AgentSlot) -> BlockedState;
    pub fn blocked_agents(&self, slots: &[AgentSlot]) -> Vec<...>;
    pub fn process_human_response(&mut self, ...) -> ...;
    pub fn clear_all(&mut self, slots: &mut [AgentSlot]);
}
```

---

## Expected Results

| File | Before | After | Reduction |
|------|--------|-------|-----------|
| agent_pool.rs | 5967 | ~200 | 96% |
| agent_slot.rs | 2043 | ~200 | 90% |
| New modules | 0 | ~1800 | +1800 |

Total core/src lines: ~26228 → ~26000 (same total, better distribution)

---

## Implementation Order

1. **Week 1**: Phase 1 - Extract types (status.rs, task_binding.rs)
2. **Week 2**: Phase 2.1 - WorktreeCoordinator
3. **Week 3**: Phase 2.2 - DecisionAgentCoordinator
4. **Week 4**: Phase 2.3 - BlockedHandler
5. **Week 5**: Phase 3 - AgentPool facade + FocusManager + ProfileManager

---

## Benefits

1. **Testability**: Each coordinator can be tested independently
2. **Readability**: Files under 500 lines are easier to navigate
3. **Maintainability**: Changes to worktree logic don't affect decision logic
4. **Extensibility**: New coordinators can be added without touching AgentPool
5. **Performance**: Smaller files compile faster (incremental compilation)