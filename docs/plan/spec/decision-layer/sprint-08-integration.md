# Sprint 8: Integration (With Concurrent Processing)

## Metadata

- Sprint ID: `decision-sprint-008`
- Title: `Integration`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14
- Updated: 2026-04-14 (Architecture Evolution)

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 8 Tests: T8.1.T1-T8.6.T6 (30 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Architecture Evolution

Added **Concurrent Processing Design**:
- DecisionSessionPool - reuse sessions across agents
- DecisionRateLimiter - prevent API overload
- HumanDecisionArbitrator - handle multiple human requests

## Sprint Goal

Complete integration of Decision Layer with existing agile-agent components, plus concurrent processing support for multi-agent scenarios.

## Stories

### Story 8.1: AgentSlot Extension (Generic Blocked)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Extend AgentSlot to use generic Blocked(BlockedState).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.1.1 | Update AgentSlotStatus with Blocked(BlockedState) | Todo | - |
| T8.1.2 | Add Decision Agent field to AgentSlot | Todo | - |
| T8.1.3 | Implement blocked setter/getter | Todo | - |
| T8.1.4 | Implement is_blocked_for_human helper | Todo | - |
| T8.1.5 | Write unit tests for slot extension | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.1.T1 | BlockedForHumanDecision via Blocked(BlockedState) |
| T8.1.T2 | Blocked task stays with blocked agent |
| T8.1.T3 | is_blocked() returns true |
| T8.1.T4 | blocking_reason() returns trait reference |

#### Acceptance Criteria

- AgentSlot uses generic Blocked(BlockedState)
- BlockingReason trait reference accessible
- Helper methods work correctly

#### Technical Notes

```rust
/// Extended AgentSlot (from Sprint 1 Story 1.6)
pub struct AgentSlot {
    /// Main agent
    main_agent: MainAgent,
    
    /// Decision Agent for this slot
    decision_agent: Option<DecisionAgent>,
    
    /// Decision Agent creation policy
    decision_policy: DecisionAgentCreationPolicy,
    
    /// Current status
    status: AgentSlotStatus,
}

/// Generic status - supports any BlockingReason
pub enum AgentSlotStatus {
    Running,
    Blocked(BlockedState),
    Idle,
    Stopped,
}

impl AgentSlot {
    pub fn is_blocked(&self) -> bool {
        matches!(self.status, AgentSlotStatus::Blocked(_))
    }
    
    pub fn is_blocked_for_human(&self) -> bool {
        match &self.status {
            AgentSlotStatus::Blocked(state) => {
                state.reason().reason_type() == "human_decision"
            }
            _ => false,
        }
    }
    
    pub fn blocking_reason(&self) -> Option<&dyn BlockingReason> {
        match &self.status {
            AgentSlotStatus::Blocked(state) => Some(state.reason()),
            _ => None,
        }
    }
    
    pub fn blocked_elapsed(&self) -> Option<Duration> {
        match &self.status {
            AgentSlotStatus::Blocked(state) => Some(state.elapsed()),
            _ => None,
        }
    }
}
```

---

### Story 8.2: AgentPool Blocked Handling

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement blocked agent handling logic in AgentPool.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.2.1 | Implement `process_agent_blocked()` | Todo | - |
| T8.2.2 | Implement blocked task policy | Todo | - |
| T8.2.3 | Implement agent mail notification | Todo | - |
| T8.2.4 | Implement `process_human_response()` | Todo | - |
| T8.2.5 | Implement decision execution | Todo | - |
| T8.2.6 | Write integration tests | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.2.T1 | Blocked status set correctly |
| T8.2.T2 | Blocked task reassigned (ReassignIfPossible) |
| T8.2.T3 | Other agents notified via mail |
| T8.2.T4 | Status cleared on response |
| T8.2.T5 | Decision executed on main agent |
| T8.2.T6 | Blocked history recorded |

#### Acceptance Criteria

- AgentPool handles blocked state correctly
- Blocked task policy configurable
- Other agents notified when blocked

#### Technical Notes

```rust
impl AgentPool {
    /// Process agent blocked
    fn process_agent_blocked(&mut self, agent_id: AgentId, blocking: Box<dyn BlockingReason>) {
        // 1. Create BlockedState
        let blocked_state = BlockedState::new(blocking);
        
        // 2. Set slot status
        let slot = self.get_slot_mut(&agent_id);
        slot.status = AgentSlotStatus::Blocked(blocked_state.clone());
        
        // 3. Handle by blocking type
        match blocking.reason_type() {
            "human_decision" => {
                let request = self.build_human_request(agent_id, blocked_state);
                self.human_queue.push(request);
                self.notify_blocked(agent_id, &request);
            }
            "resource_waiting" => {
                // Resource blocking - different handling
                self.resource_manager.handle_blocked(agent_id);
            }
            _ => {
                // Unknown blocking type - use default
                self.handle_unknown_blocking(agent_id);
            }
        }
        
        // 4. Send mail to other agents
        self.notify_other_agents(agent_id, blocking.description());
        
        // 5. Handle blocked task
        self.handle_blocked_task(agent_id);
    }
    
    fn handle_blocked_task(&mut self, agent_id: AgentId) {
        match self.config.blocked_task_policy {
            BlockedTaskPolicy::KeepAssigned => {
                // Task stays with blocked agent
            }
            BlockedTaskPolicy::ReassignIfPossible => {
                if let Some(task_id) = self.get_assigned_task(agent_id) {
                    if let Some(idle_agent) = self.find_idle_agent() {
                        self.reassign_task(task_id, idle_agent);
                    }
                }
            }
            BlockedTaskPolicy::MarkWaiting => {
                if let Some(task_id) = self.get_assigned_task(agent_id) {
                    self.backlog.mark_waiting(task_id);
                }
            }
        }
    }
    
    fn process_human_response(&mut self, response: HumanDecisionResponse) {
        // 1. Get request from queue
        let request = self.human_queue.complete(response.clone());
        
        // 2. Get agent
        let agent_id = request.agent_id;
        let slot = self.get_slot_mut(&agent_id);
        
        // 3. Clear blocked status
        slot.status = AgentSlotStatus::Running;
        
        // 4. Execute decision
        self.execute_decision(agent_id, response.selection);
    }
    
    fn execute_decision(&mut self, agent_id: AgentId, selection: HumanSelection) {
        let slot = self.get_slot(&agent_id);
        
        match selection {
            HumanSelection::Selected { option_id } => {
                slot.main_agent.send_selection(option_id)?;
            }
            HumanSelection::AcceptedRecommendation => {
                if let Some(rec) = &slot.blocking_reason().unwrap().recommendation() {
                    rec.action.execute(&context, &mut slot.main_agent)?;
                }
            }
            HumanSelection::Custom { instruction } => {
                slot.main_agent.send_prompt(instruction)?;
            }
            HumanSelection::Skipped => {
                self.skip_current_task(agent_id);
            }
            HumanSelection::Cancelled => {
                slot.main_agent.cancel_current_operation();
            }
        }
    }
}
```

---

### Story 8.3: Integration with Backlog and Kanban

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Integrate Decision Layer with task management.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.3.1 | Implement task completion notification | Todo | - |
| T8.3.2 | Implement task failure marking | Todo | - |
| T8.3.3 | Implement next task selection | Todo | - |
| T8.3.4 | Implement PR submission trigger | Todo | - |
| T8.3.5 | Write integration tests | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.3.T1 | Task completion moves Kanban to Done |
| T8.3.T2 | Task failure moves Kanban to Failed |
| T8.3.T3 | Next task selected from Todo |
| T8.3.T4 | Story definition loaded |
| T8.3.T5 | Task definition loaded |

#### Acceptance Criteria

- Task completion updates Kanban
- Task failure handled correctly
- Next task selection works

---

### Story 8.4: Integration with WorkplaceStore

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Integrate Decision Layer persistence with WorkplaceStore.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.4.1 | Create decision persistence path | Todo | - |
| T8.4.2 | Implement Decision Agent persistence | Todo | - |
| T8.4.3 | Implement Decision Agent restore | Todo | - |
| T8.4.4 | Implement project rules loading | Todo | - |
| T8.4.5 | Write integration tests | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.4.T1 | Decision directory created |
| T8.4.T2 | Decision state persisted |
| T8.4.T3 | Decision agent restored |
| T8.4.T4 | CLAUDE.md rules loaded |

#### Acceptance Criteria

- Decision Agent persists correctly
- Decision Agent restored on startup
- Project rules loaded from workplace

---

### Story 8.5: Decision Observability

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement observability for Decision Layer.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.5.1 | Create DecisionMetrics struct | Todo | - |
| T8.5.2 | Implement metrics collection | Todo | - |
| T8.5.3 | Implement decision logging | Todo | - |
| T8.5.4 | Implement CLI metrics commands | Todo | - |
| T8.5.5 | Write unit tests | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.5.T1 | Total decisions tracked |
| T8.5.T2 | Success rate calculated |
| T8.5.T3 | Decisions by type tracked |
| T8.5.T4 | Log format valid JSON |
| T8.5.T5 | CLI metrics output valid |

#### Acceptance Criteria

- Metrics collected per decision
- Logs structured and queryable
- CLI commands work

---

### Story 8.6: Concurrent Processing (Session Pool + Rate Limiter)

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement concurrent processing support for multi-agent scenarios.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.6.1 | Create `DecisionSessionPool` struct | Todo | - |
| T8.6.2 | Implement session acquire/release | Todo | - |
| T8.6.3 | Create `DecisionRateLimiter` struct | Todo | - |
| T8.6.4 | Implement rate limiting logic | Todo | - |
| T8.6.5 | Create `HumanDecisionArbitrator` struct | Todo | - |
| T8.6.6 | Implement arbitration strategies | Todo | - |
| T8.6.7 | Integrate with AgentPool | Todo | - |
| T8.6.8 | Write unit tests for concurrent handling | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.6.T1 | Session pool acquires session |
| T8.6.T2 | Session pool releases session |
| T8.6.T3 | Pool exhausted error returned |
| T8.6.T4 | Rate limiter blocks over-limit |
| T8.6.T5 | Rate limiter resets after minute |
| T8.6.T6 | Arbitrator pops highest priority |
| T8.6.T7 | Sequential arbitration blocks others |
| T8.6.T8 | Concurrent requests handled correctly |

#### Acceptance Criteria

- Session pool manages sessions per provider
- Rate limiter prevents API overload
- Human decision arbitrator handles multiple requests

#### Technical Notes

```rust
/// Decision session pool - reuse sessions across agents
pub struct DecisionSessionPool {
    /// Available sessions per provider type
    available: HashMap<ProviderKind, VecDeque<SessionHandle>>,
    
    /// Active sessions (agent_id -> session)
    active: HashMap<AgentId, SessionHandle>,
    
    /// Max sessions per provider
    max_per_provider: usize,
    
    /// Session idle timeout
    idle_timeout_ms: u64,
    
    /// Provider factory for creating new sessions
    provider_factory: ProviderFactory,
}

impl DecisionSessionPool {
    pub fn new(config: SessionPoolConfig, provider_factory: ProviderFactory) -> Self {
        Self {
            available: HashMap::new(),
            active: HashMap::new(),
            max_per_provider: config.max_per_provider,
            idle_timeout_ms: config.idle_timeout_ms,
            provider_factory,
        }
    }
    
    /// Acquire session for agent
    pub fn acquire(&mut self, provider: ProviderKind, agent_id: AgentId) -> Result<SessionHandle> {
        // Check if agent already has session
        if let Some(session) = self.active.get(&agent_id) {
            return Ok(session.clone());
        }
        
        // Check available pool
        if let Some(pool) = self.available.get_mut(&provider) {
            // Remove expired sessions
            pool.retain(|s| !s.is_expired());
            
            if let Some(session) = pool.pop_front() {
                self.active.insert(agent_id, session.clone());
                return Ok(session);
            }
        }
        
        // Create new if under limit
        let active_count = self.active.values()
            .filter(|s| s.provider == provider)
            .count();
        
        if active_count < self.max_per_provider {
            let session = self.provider_factory.create_session(provider)?;
            self.active.insert(agent_id, session.clone());
            return Ok(session);
        }
        
        // Pool exhausted - wait or error
        Err(Error::SessionPoolExhausted { provider })
    }
    
    /// Release session back to pool
    pub fn release(&mut self, agent_id: AgentId) {
        if let Some(session) = self.active.remove(&agent_id) {
            // Return to available pool
            self.available.get_mut(&session.provider)
                .map(|pool| {
                    // Don't exceed pool size
                    if pool.len() < self.max_per_provider {
                        pool.push_back(session);
                    }
                });
        }
    }
    
    /// Cleanup expired sessions
    pub fn cleanup_expired(&mut self) {
        for pool in self.available.values_mut() {
            pool.retain(|s| !s.is_expired());
        }
    }
    
    /// Get pool statistics
    pub fn stats(&self) -> SessionPoolStats {
        SessionPoolStats {
            total_active: self.active.len(),
            by_provider: self.available.iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect(),
        }
    }
}

/// Session pool configuration
pub struct SessionPoolConfig {
    /// Max sessions per provider (default: 3)
    pub max_per_provider: usize,
    
    /// Session idle timeout (default: 30 minutes)
    pub idle_timeout_ms: u64,
    
    /// Enable session reuse
    pub reuse_enabled: bool,
}

impl Default for SessionPoolConfig {
    fn default() -> Self {
        Self {
            max_per_provider: 3,
            idle_timeout_ms: 1800000,
            reuse_enabled: true,
        }
    }
}

/// Decision rate limiter
pub struct DecisionRateLimiter {
    /// Requests per minute limit
    requests_per_minute: u32,
    
    /// Current minute counter
    current_count: u32,
    
    /// Minute start time
    minute_start: Instant,
    
    /// Waiting queue
    waiting: VecDeque<AgentId>,
}

impl DecisionRateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        Self {
            requests_per_minute,
            current_count: 0,
            minute_start: Instant::now(),
            waiting: VecDeque::new(),
        }
    }
    
    /// Check if request allowed
    pub fn check(&mut self, agent_id: AgentId) -> RateLimitResult {
        // Reset counter if minute passed
        if self.minute_start.elapsed() > Duration::from_secs(60) {
            self.current_count = 0;
            self.minute_start = Instant::now();
            
            // Process waiting queue
            if let Some(waiting_id) = self.waiting.pop_front() {
                self.current_count += 1;
                return RateLimitResult::Allowed { agent_id: waiting_id };
            }
        }
        
        if self.current_count < self.requests_per_minute {
            self.current_count += 1;
            RateLimitResult::Allowed { agent_id }
        } else {
            self.waiting.push_back(agent_id);
            RateLimitResult::Waiting { position: self.waiting.len() }
        }
    }
    
    /// Get current status
    pub fn status(&self) -> RateLimitStatus {
        RateLimitStatus {
            current_count: self.current_count,
            limit: self.requests_per_minute,
            waiting_count: self.waiting.len(),
            remaining_in_minute: 60 - self.minute_start.elapsed().as_secs(),
        }
    }
}

pub enum RateLimitResult {
    Allowed { agent_id: AgentId },
    Waiting { position: usize },
}

pub struct RateLimitStatus {
    pub current_count: u32,
    pub limit: u32,
    pub waiting_count: usize,
    pub remaining_in_minute: u64,
}

/// Human decision arbitrator - handle multiple human requests
pub struct HumanDecisionArbitrator {
    /// Pending requests
    queue: HumanDecisionQueue,
    
    /// Current request being handled
    current: Option<HumanDecisionRequest>,
    
    /// Arbitration strategy
    strategy: ArbitrationStrategy,
}

pub enum ArbitrationStrategy {
    /// Handle one at a time, block others
    Sequential,
    
    /// Batch similar requests, handle together
    BatchSimilar { similarity_threshold: f64 },
    
    /// Parallel handling (multiple TUI modals - experimental)
    Parallel { max_concurrent: usize },
}

impl HumanDecisionArbitrator {
    pub fn new(strategy: ArbitrationStrategy, queue: HumanDecisionQueue) -> Self {
        Self {
            queue,
            current: None,
            strategy,
        }
    }
    
    /// Submit new request
    pub fn submit(&mut self, request: HumanDecisionRequest) -> ArbitrationResult {
        match &self.strategy {
            ArbitrationStrategy::Sequential => {
                if self.current.is_some() {
                    // Add to queue, wait for current to complete
                    self.queue.push(request);
                    ArbitrationResult::Queued { position: self.queue.total_pending() }
                } else {
                    // Handle immediately
                    self.current = Some(request.clone());
                    ArbitrationResult::Immediate { request }
                }
            }
            
            ArbitrationStrategy::BatchSimilar { similarity_threshold } => {
                // Check if similar to current
                if let Some(current) = &self.current {
                    if self.is_similar(&request, current, *similarity_threshold) {
                        // Batch with current
                        self.queue.push(request);
                        ArbitrationResult::Batched { with: current.id.clone() }
                    } else {
                        self.queue.push(request);
                        ArbitrationResult::Queued { position: self.queue.total_pending() }
                    }
                } else {
                    self.current = Some(request.clone());
                    ArbitrationResult::Immediate { request }
                }
            }
            
            ArbitrationStrategy::Parallel { max_concurrent } => {
                // Allow multiple concurrent (experimental)
                if self.queue.total_pending() < *max_concurrent {
                    ArbitrationResult::Immediate { request }
                } else {
                    self.queue.push(request);
                    ArbitrationResult::Queued { position: self.queue.total_pending() }
                }
            }
        }
    }
    
    /// Complete current request
    pub fn complete(&mut self, response: HumanDecisionResponse) -> Option<HumanDecisionRequest> {
        let request = self.queue.complete(response);
        
        // Move to next in queue
        self.current = self.queue.pop();
        
        request
    }
    
    fn is_similar(&self, a: &HumanDecisionRequest, b: &HumanDecisionRequest, threshold: f64) -> bool {
        // Similar if same situation type and similar options
        a.situation_type == b.situation_type &&
        self.options_similarity(&a.options, &b.options) > threshold
    }
    
    fn options_similarity(&self, a: &[ChoiceOption], b: &[ChoiceOption]) -> f64 {
        if a.is_empty() || b.is_empty() { return 0.0; }
        
        let matching = a.iter()
            .filter(|opt_a| b.iter().any(|opt_b| opt_a.id == opt_b.id))
            .count();
        
        matching as f64 / a.len().max(b.len()) as f64
    }
}

pub enum ArbitrationResult {
    Immediate { request: HumanDecisionRequest },
    Queued { position: usize },
    Batched { with: DecisionRequestId },
}
```

---

## Integration Architecture

```
Multi-Agent Runtime with Decision Layer:

┌─────────────────────────────────────────────────────────────────────┐
│  MultiAgentRuntime                                                   │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ DecisionSessionPool                                              ││
│  │  - Claude sessions: [s1, s2, s3]                                 ││
│  │  - Codex sessions: [t1, t2]                                      ││
│  │  - Acquire/Release management                                    ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ DecisionRateLimiter                                              ││
│  │  - 20 requests/minute limit                                      ││
│  │  - Waiting queue                                                 ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ HumanDecisionArbitrator                                          ││
│  │  - Sequential strategy (default)                                 ││
│  │  - Current request + waiting queue                               ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ AgentPool                                                        ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐                         ││
│  │  │ Slot A   │ │ Slot B   │ │ Slot C   │                         ││
│  │  │ Running  │ │ Blocked  │ │ Running  │                         ││
│  │  │          │ │(human)   │ │          │                         ││
│  │  └──────────┘ └──────────┘ └──────────┘                         ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Shared Components                                                ││
│  │  - SituationRegistry                                             ││
│  │  - ActionRegistry                                                ││
│  │  - ClassifierRegistry                                            ││
│  │  - BlockingReasonRegistry                                        ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Observability                                                    ││
│  │  - DecisionMetrics                                               ││
│  │  - DecisionLog                                                   ││
│  │  - SessionPoolStats                                              ││
│  │  - RateLimitStatus                                               ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Session pool exhaustion | Medium | Medium | Pool size tuning, fallback |
| Rate limiter starvation | Low | Medium | Priority queue in rate limiter |
| Arbitration complexity | Low | Low | Start with Sequential strategy |

## Sprint Deliverables

- `core/src/agent_slot.rs` - Extended with Blocked(BlockedState)
- `core/src/agent_pool.rs` - Blocked handling
- `decision/src/session_pool.rs` - DecisionSessionPool
- `decision/src/rate_limiter.rs` - DecisionRateLimiter
- `decision/src/arbitrator.rs` - HumanDecisionArbitrator
- `decision/src/metrics.rs` - Observability
- Integration tests

## Dependencies

- Sprint 1-7: All decision layer components
- Multi-Agent Runtime: AgentPool, AgentSlot
- Kanban Sprint 1-5: Backlog, Kanban

## Final Release Checklist

After Sprint 8, validate:

1. [ ] SituationRegistry populated with built-in + provider-specific
2. [ ] ActionRegistry populated with built-in actions
3. [ ] ClassifierRegistry dispatches correctly
4. [ ] DecisionSituation trait used throughout
5. [ ] DecisionAction trait used for output
6. [ ] BlockingReason trait used for blocked states
7. [ ] SessionPool manages sessions correctly
8. [ ] RateLimiter prevents overload
9. [ ] HumanDecisionArbitrator handles concurrent requests
10. [ ] Integration with AgentPool works
11. [ ] Integration with Kanban works
12. [ ] Metrics collected
13. [ ] All tests pass

## Project Complete

This is the final sprint. After completion, the Decision Layer is production-ready with extensible trait-based architecture and concurrent processing support.