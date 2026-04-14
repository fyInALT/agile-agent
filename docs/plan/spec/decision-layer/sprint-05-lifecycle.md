# Sprint 5: Lifecycle

## Metadata

- Sprint ID: `decision-sprint-005`
- Title: `Decision Agent Lifecycle`
- Duration: 1 week
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 5 Tests: T5.1.T1-T5.4.T4 (18 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Implement Decision Agent creation, destruction, task switching, and session persistence for multi-turn decisions.

## Stories

### Story 5.1: Decision Agent Creation Policies

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement creation timing policies (Eager, Lazy, Configured).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.1.1 | Implement `DecisionAgentCreationPolicy` enum | Todo | - |
| T5.1.2 | Implement Eager creation (at Main Agent spawn) | Todo | - |
| T5.1.3 | Implement Lazy creation (on first blocked) | Todo | - |
| T5.1.4 | Implement Configured creation (from settings) | Todo | - |
| T5.1.5 | Add creation to Main Agent initialization | Todo | - |
| T5.1.6 | Write unit tests for each policy | Todo | - |

#### Acceptance Criteria

- All three policies implemented
- Lazy creation works on blocked event
- Eager creation at Main Agent start

#### Technical Notes

```rust
pub enum DecisionAgentCreationPolicy {
    /// Create immediately when Main Agent spawns
    Eager,
    
    /// Create on first blocked event (recommended)
    Lazy,
    
    /// Follow configuration setting
    Configured,
}

impl MainAgent {
    fn initialize(&mut self) -> Result<()> {
        // Eager: Create Decision Agent immediately
        if self.config.decision_layer.creation_policy == DecisionAgentCreationPolicy::Eager {
            self.decision_agent = Some(DecisionAgent::new(
                self.id.clone(),
                self.config.decision_layer.clone(),
                self.workplace.clone(),
            ));
        }
        
        Ok(())
    }
    
    fn handle_blocked(&mut self, event: ProviderEvent) -> Result<()> {
        // Lazy: Create Decision Agent on first blocked
        if self.decision_agent.is_none() {
            self.decision_agent = Some(DecisionAgent::new(
                self.id.clone(),
                self.config.decision_layer.clone(),
                self.workplace.clone(),
            ));
        }
        
        // Execute decision
        self.decision_agent.as_mut().unwrap().decide(event)?;
    }
}
```

---

### Story 5.2: Decision Agent Destruction and Cleanup

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement destruction timing and cleanup.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.2.1 | Define destruction triggers | Todo | - |
| T5.2.2 | Implement story completion cleanup | Todo | - |
| T5.2.3 | Implement idle timeout cleanup | Todo | - |
| T5.2.4 | Implement manual stop cleanup | Todo | - |
| T5.2.5 | Implement session release | Todo | - |
| T5.2.6 | Implement transcript archival | Todo | - |
| T5.2.7 | Write unit tests for cleanup | Todo | - |

#### Acceptance Criteria

- Cleanup releases provider session
- Transcript archived for review
- All cleanup paths tested

#### Technical Notes

```rust
impl DecisionAgent {
    /// Story completion: clean session but keep transcript
    fn on_story_complete(&mut self) -> Result<()> {
        // 1. Release provider session
        if let Some(session) = &self.session {
            self.provider.close_session(session)?;
        }
        self.session = None;
        
        // 2. Persist transcript for later review
        self.persist_transcript()?;
        
        // 3. Reset state for next story
        self.reflection_rounds = 0;
        self.retry_count = 0;
        self.timeout_count = 0;
        self.context_cache.clear();
        
        Ok(())
    }
    
    /// Idle timeout: full cleanup
    fn on_idle_timeout(&mut self) -> Result<()> {
        // Full cleanup including transcript removal from memory
        self.destroy()?;
        Ok(())
    }
    
    /// Destroy: full cleanup
    fn destroy(&mut self) -> Result<()> {
        // 1. Close session
        if let Some(session) = &self.session {
            self.provider.close_session(session)?;
        }
        
        // 2. Stop provider thread if running
        if let Some(handle) = &self.thread_handle {
            // Signal stop and wait
        }
        
        // 3. Clear state
        self.session = None;
        self.thread_handle = None;
        self.context_cache.clear();
        
        // Keep transcript on disk for review
        
        Ok(())
    }
}

/// Destruction triggers
pub enum DestructionTrigger {
    /// Story completed successfully
    StoryComplete,
    
    /// Main Agent stopped
    MainAgentStopped,
    
    /// Idle timeout exceeded
    IdleTimeout,
    
    /// Manual stop command
    ManualStop,
    
    /// Error requiring full reset
    FatalError,
}
```

**Idle Timeout Configuration**:

```toml
[decision_layer.lifecycle]
# Idle timeout in milliseconds (default: 30 minutes)
idle_timeout_ms = 1800000

# Keep transcript after destruction (default: true)
keep_transcript = true
```

---

### Story 5.3: Task Switching with Context Preservation

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement task switching while preserving Decision Agent state.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.3.1 | Create `TaskDecisionContext` struct | Todo | - |
| T5.3.2 | Implement task decision history storage | Todo | - |
| T5.3.3 | Implement `switch_task()` method | Todo | - |
| T5.3.4 | Implement context isolation per task | Todo | - |
| T5.3.5 | Implement session continuation across tasks | Todo | - |
| T5.3.6 | Write unit tests for task switching | Todo | - |

#### Acceptance Criteria

- Task switching preserves session
- Context isolated per task
- History archived for completed tasks

#### Technical Notes

```rust
impl DecisionAgent {
    /// Task-specific decision context
    task_decision_contexts: HashMap<TaskId, TaskDecisionContext>,
    
    /// Currently active task
    current_task_id: Option<TaskId>,
    
    /// Switch to new task
    fn switch_task(&mut self, new_task: TaskId) -> Result<()> {
        // 1. Archive current task context
        if let Some(current) = &self.current_task_id {
            if let Some(ctx) = self.current_context.take() {
                self.task_decision_contexts.insert(current.clone(), ctx);
            }
        }
        
        // 2. Create or restore new task context
        let new_ctx = self.task_decision_contexts.remove(&new_task)
            .unwrap_or_else(|| TaskDecisionContext::new(new_task.clone()));
        
        self.current_context = Some(new_ctx);
        self.current_task_id = Some(new_task);
        
        // 3. Session continues (same provider, same session)
        // Multi-turn decisions span tasks
        
        Ok(())
    }
    
    /// Get current task context
    fn current_context(&self) -> Option<&TaskDecisionContext> {
        self.current_context.as_ref()
    }
    
    /// Archive completed task
    fn archive_task(&mut self, task_id: &TaskId) -> Result<()> {
        if let Some(ctx) = self.task_decision_contexts.remove(task_id) {
            // Persist to archive
            self.persist_task_history(task_id, ctx)?;
        }
        Ok(())
    }
}

pub struct TaskDecisionContext {
    /// Task ID
    pub task_id: TaskId,
    
    /// Decisions made for this task
    pub decisions: Vec<DecisionRecord>,
    
    /// Reflection rounds for this task
    pub reflection_rounds: u8,
    
    /// Retry count for this task
    pub retry_count: u8,
    
    /// Task start time
    pub started_at: DateTime<Utc>,
    
    /// Task completion time (if completed)
    pub completed_at: Option<DateTime<Utc>>,
}
```

---

### Story 5.4: Session Persistence for Multi-Turn Decisions

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement session persistence across restarts.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T5.4.1 | Create session persistence format | Todo | - |
| T5.4.2 | Implement `persist_session()` method | Todo | - |
| T5.4.3 | Implement `restore_session()` method | Todo | - |
| T5.4.4 | Implement transcript persistence | Todo | - |
| T5.4.5 | Handle session validation on restore | Todo | - |
| T5.4.6 | Write unit tests for persistence | Todo | - |

#### Acceptance Criteria

- Session persists across restarts
- Transcript persisted correctly
- Validation handles stale sessions

#### Technical Notes

```rust
/// Decision Agent persistence structure
#[derive(Serialize, Deserialize)]
pub struct DecisionAgentState {
    /// Decision agent ID
    pub agent_id: AgentId,
    
    /// Parent main agent ID
    pub parent_agent_id: AgentId,
    
    /// Provider session handle
    pub session: Option<SessionHandle>,
    
    /// Current task ID
    pub current_task_id: Option<TaskId>,
    
    /// Task contexts (active)
    pub task_contexts: HashMap<TaskId, TaskDecisionContext>,
    
    /// Configuration
    pub config: DecisionAgentConfig,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
}

impl DecisionAgent {
    fn persist(&self, path: &Path) -> Result<()> {
        let state = DecisionAgentState {
            agent_id: self.id.clone(),
            parent_agent_id: self.parent_agent_id.clone(),
            session: self.session.clone(),
            current_task_id: self.current_task_id.clone(),
            task_contexts: self.task_decision_contexts.clone(),
            config: self.config.clone(),
            created_at: self.created_at,
            last_activity: self.last_activity,
        };
        
        let json = serde_json::to_string(&state)?;
        std::fs::write(path.join("state.json"), json)?;
        
        // Persist context cache
        self.context_cache.persist(path.join("context_cache.json"))?;
        
        // Persist transcript
        self.persist_transcript()?;
        
        Ok(())
    }
    
    fn restore(path: &Path, workplace: &WorkplaceStore) -> Result<Self> {
        let state_json = std::fs::read_to_string(path.join("state.json"))?;
        let state: DecisionAgentState = serde_json::from_str(&state_json)?;
        
        let context_cache = RunningContextCache::restore(
            &path.join("context_cache.json"),
            &state.config,
        )?;
        
        let mut agent = DecisionAgent {
            id: state.agent_id,
            parent_agent_id: state.parent_agent_id,
            session: state.session,
            current_task_id: state.current_task_id,
            task_decision_contexts: state.task_contexts,
            context_cache,
            config: state.config,
            created_at: state.created_at,
            last_activity: state.last_activity,
            // ... other fields
        };
        
        // Validate session is still valid
        if let Some(session) = &agent.session {
            if !agent.validate_session(session)? {
                // Session invalid, reset
                agent.session = None;
            }
        }
        
        Ok(agent)
    }
    
    fn validate_session(&self, session: &SessionHandle) -> Result<bool> {
        // Check if session is still usable with provider
        // Provider-specific validation
        match session {
            SessionHandle::ClaudeSession { session_id } => {
                // Claude: Check if session file exists
                // ...
            }
            SessionHandle::CodexThread { thread_id } => {
                // Codex: Check if thread is valid
                // ...
            }
            _ => Ok(true),
        }
    }
}
```

**Persistence Path Structure**:

```
~/.agile-agent/workplaces/{workplace_id}/agents/{main_agent_id}/decision/
├── state.json           # DecisionAgentState
├── context_cache.json   # RunningContextCache
├── transcript.json      # Decision history transcript
└── session.json         # Provider session info
```

---

## Lifecycle State Diagram

```
Decision Agent Lifecycle:

[Not Created] ──Main Agent blocked──> [Active]
    │                                    │
    │                                    ├── Story complete ──> [Idle] (session released)
    │                                    │                        │
    │                                    │                        ├── New Story ──> [Active]
    │                                    │                        │
    │                                    │                        └── Idle timeout ──> [Destroyed]
    │                                    │
    │                                    ├── Manual stop ──> [Destroyed]
    │                                    │
    │                                    └── Fatal error ──> [Destroyed] (reset)
    │
    └── Eager policy ──> [Active] (Main Agent created)
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Session validation complexity | Medium | Medium | Provider-specific validation |
| Context loss on switch | Low | Medium | Isolate per task, archive completed |
| Persistence file corruption | Low | Medium | Backup on write, validation on restore |

## Sprint Deliverables

- `decision/src/lifecycle.rs` - Creation/destruction policies
- `decision/src/agent.rs` - DecisionAgent lifecycle methods
- Unit tests for all lifecycle paths

## Dependencies

- Sprint 1: Core Types (DecisionAgentConfig)
- Sprint 3: Decision Engine (session management)
- Sprint 4: Context Cache (persistence)

## Next Sprint

After completing this sprint, proceed to [Sprint 6: Human Intervention](./sprint-06-human-intervention.md) for critical decision escalation.