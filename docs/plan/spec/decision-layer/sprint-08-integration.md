# Sprint 8: Integration

## Metadata

- Sprint ID: `decision-sprint-008`
- Title: `Integration`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 8 Tests: T8.1.T1-T8.5.T5 (23 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Complete integration of Decision Layer with existing agile-agent components: AgentPool, Backlog, Kanban, WorkplaceStore, TUI, and observability systems.

## Stories

### Story 8.1a: AgentSlot Extension for Decision Agent

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Extend AgentSlot with Decision Agent support and blocked status.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.1a.1 | Extend `AgentSlotStatus` with BlockedForHumanDecision | Todo | - |
| T8.1a.2 | Add Decision Agent field to AgentSlot | Todo | - |
| T8.1a.3 | Add decision_policy field | Todo | - |
| T8.1a.4 | Implement blocked status setter/getter | Todo | - |
| T8.1a.5 | Write unit tests for slot extension | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.1.T1 | BlockedForHumanDecision status set correctly |
| T8.1.T2 | Blocked task stays with blocked agent (KeepAssigned policy) |

#### Acceptance Criteria

- AgentSlot holds Decision Agent
- BlockedForHumanDecision status defined
- Blocked status setter/getter work

---

### Story 8.1b: AgentPool Blocked Agent Handling

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement blocked agent handling logic in AgentPool.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.1b.1 | Implement `process_agent_blocked()` in AgentPool | Todo | - |
| T8.1b.2 | Implement blocked task policy (Keep/Reassign/MarkWaiting) | Todo | - |
| T8.1b.3 | Implement agent mail notification for blocked | Todo | - |
| T8.1b.4 | Implement `process_human_response()` for clearing blocked | Todo | - |
| T8.1b.5 | Implement decision execution on main agent | Todo | - |
| T8.1b.6 | Write integration tests for blocked flow | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.1.T3 | Blocked task reassigned to idle agent (ReassignIfPossible) |
| T8.1.T4 | Other agents notified via mail |
| T8.1.T5 | Blocked status cleared on human response |
| T8.1.T6 | Decision executed on main agent |

#### Acceptance Criteria

- AgentPool handles blocked state correctly
- Blocked task policy configurable
- Other agents notified when agent blocked

#### Technical Notes

```rust
// Extend AgentSlot in core/src/agent_slot.rs
pub struct AgentSlot {
    // Existing fields...
    
    /// Decision Agent for this slot
    decision_agent: Option<DecisionAgent>,
    
    /// Decision Agent creation policy
    decision_policy: DecisionAgentCreationPolicy,
}

pub enum AgentSlotStatus {
    // Existing statuses...
    
    /// Blocked waiting for human decision
    BlockedForHumanDecision {
        decision_request_id: DecisionRequestId,
        reason: CriticalDecisionReason,
        blocked_at: Instant,
        preliminary_analysis: String,
        options: Vec<ChoiceOption>,
        recommended: Option<String>,
    },
}

impl AgentPool {
    fn process_agent_blocked(&mut self, agent_id: AgentId, request: HumanDecisionRequest) {
        // 1. Set blocked status
        let slot = self.get_slot_mut(&agent_id);
        slot.status = AgentSlotStatus::BlockedForHumanDecision {
            decision_request_id: request.id.clone(),
            reason: request.reason.clone(),
            blocked_at: Instant::now(),
            preliminary_analysis: request.analysis.context_summary.clone(),
            options: request.options.clone(),
            recommended: request.recommendation.clone().map(|r| r.selected),
        };
        
        // 2. Add to human decision queue
        self.human_queue.push(request.clone());
        
        // 3. Send notification to TUI
        self.notify_blocked(agent_id, &request);
        
        // 4. Send mail to other agents
        self.notify_other_agents(agent_id, request);
        
        // 5. Handle blocked task
        self.handle_blocked_task(agent_id);
    }
    
    fn handle_blocked_task(&mut self, agent_id: AgentId) {
        match self.config.blocked_task_policy {
            BlockedTaskPolicy::KeepAssigned => {
                // Task stays with blocked agent
            }
            BlockedTaskPolicy::ReassignIfPossible => {
                // Try to assign to idle agent
                let task_id = self.get_assigned_task(agent_id);
                if let Some(idle_agent) = self.find_idle_agent() {
                    self.reassign_task(task_id, idle_agent);
                }
            }
            BlockedTaskPolicy::MarkWaiting => {
                // Mark task as waiting in backlog
                let task_id = self.get_assigned_task(agent_id);
                self.backlog.mark_waiting(task_id);
            }
        }
    }
    
    fn notify_other_agents(&mut self, blocked_agent: AgentId, request: HumanDecisionRequest) {
        for agent in self.active_agents() {
            if agent.id != blocked_agent {
                self.send_mail(MailMessage {
                    from: blocked_agent.clone(),
                    to: agent.id.clone(),
                    type: MailType::AgentBlocked {
                        agent_id: blocked_agent.clone(),
                        reason: format!("Waiting for human decision: {}", request.decision_type),
                    },
                    priority: MailPriority::Info,
                });
            }
        }
    }
    
    fn process_human_response(&mut self, response: HumanDecisionResponse) {
        // 1. Remove from queue
        let request = self.human_queue.complete(response.request_id.clone());
        
        // 2. Clear blocked status
        let agent_id = request.agent_id.clone();
        self.clear_blocked_status(&agent_id);
        
        // 3. Execute decision
        self.execute_decision(agent_id, response.selection);
        
        // 4. Record in history
        self.human_queue.history.push(response);
    }
}

/// Blocked task policy
pub enum BlockedTaskPolicy {
    /// Keep task assigned to blocked agent
    KeepAssigned,
    
    /// Try to reassign to idle agent
    ReassignIfPossible,
    
    /// Mark task as waiting in backlog
    MarkWaiting,
}
```

---

### Story 8.2: Integration with Backlog and Kanban

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Integrate Decision Layer with task management.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.2.1 | Implement task completion notification | Todo | - |
| T8.2.2 | Implement task failure marking | Todo | - |
| T8.2.3 | Implement next task selection | Todo | - |
| T8.2.4 | Implement PR submission trigger | Todo | - |
| T8.2.5 | Implement story completion detection | Todo | - |
| T8.2.6 | Write integration tests with kanban | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.2.T1 | Task completion moves Kanban to Done |
| T8.2.T2 | Task failure moves Kanban to Failed |
| T8.2.T3 | Next task selected from Todo |
| T8.2.T4 | Story definition loaded correctly |
| T8.2.T5 | Task definition loaded correctly |

#### Acceptance Criteria

- Task completion updates Kanban
- Task failure handled correctly
- Next task selection works

#### Technical Notes

```rust
impl DecisionAgent {
    fn on_task_complete(&mut self, task_id: TaskId) -> Result<()> {
        // 1. Update Kanban status
        let kanban = self.workplace.kanban()?;
        kanban.move_task(task_id, KanbanColumn::Done)?;
        
        // 2. Clear task context
        self.archive_task(&task_id)?;
        
        // 3. Select next task
        self.select_next_task()?;
        
        Ok(())
    }
    
    fn on_task_failed(&mut self, task_id: TaskId) -> Result<()> {
        // 1. Update Kanban status
        let kanban = self.workplace.kanban()?;
        kanban.move_task(task_id, KanbanColumn::Failed)?;
        
        // 2. Annotate with failure reason
        kanban.annotate_task(task_id, "Failed after recovery exhausted")?;
        
        // 3. Select next task
        self.select_next_task()?;
        
        Ok(())
    }
    
    fn select_next_task(&mut self) -> Result<Option<TaskId>> {
        // 1. Get backlog
        let backlog = self.workplace.backlog()?;
        
        // 2. Pick from Todo column
        let next = backlog.pick_from_todo()?;
        
        // 3. Assign to main agent
        if let Some(task_id) = next {
            self.main_agent.assign_task(task_id)?;
        }
        
        Ok(next)
    }
    
    fn load_task_definition(&self, task_id: TaskId) -> Result<TaskDefinition> {
        let backlog = self.workplace.backlog()?;
        backlog.get_task(task_id)
    }
    
    fn load_story_definition(&self, story_id: StoryId) -> Result<StoryDefinition> {
        let backlog = self.workplace.backlog()?;
        backlog.get_story(story_id)
    }
}
```

---

### Story 8.3: Integration with WorkplaceStore

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Integrate Decision Layer persistence with WorkplaceStore.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.3.1 | Create decision persistence directory structure | Todo | - |
| T8.3.2 | Implement Decision Agent persistence integration | Todo | - |
| T8.3.3 | Implement Decision Agent restore on startup | Todo | - |
| T8.3.4 | Implement project rules loading (CLAUDE.md) | Todo | - |
| T8.3.5 | Write integration tests for persistence | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.3.T1 | Decision directory created |
| T8.3.T2 | Decision state persisted |
| T8.3.T3 | Decision agent restored |
| T8.3.T4 | CLAUDE.md rules loaded |

#### Acceptance Criteria

- Decision Agent persists correctly
- Decision Agent restored on startup
- Project rules loaded from workplace

#### Technical Notes

```rust
// Persistence path structure
// ~/.agile-agent/workplaces/{workplace_id}/agents/{agent_id}/decision/
// ├── state.json           # DecisionAgentState
// ├── context_cache.json   # RunningContextCache
// ├── transcript.json      # Decision history
// └── session.json         # Provider session

impl WorkplaceStore {
    fn decision_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agents_path()
            .join(agent_id.to_string())
            .join("decision")
    }
    
    fn ensure_decision_path(&self, agent_id: &AgentId) -> Result<()> {
        let path = self.decision_path(agent_id);
        std::fs::create_dir_all(&path)?;
        Ok(())
    }
    
    fn load_project_rules(&self) -> Result<ProjectRules> {
        // Load from CLAUDE.md/AGENTS.md in project root
        let claude_md = self.project_path().join("CLAUDE.md");
        if claude_md.exists() {
            ProjectRules::from_file(&claude_md)
        } else {
            Ok(ProjectRules::default())
        }
    }
    
    fn restore_decision_agents(&self) -> Result<Vec<DecisionAgent>> {
        let mut agents = Vec::new();
        
        for agent_dir in self.agents_path().read_dir()? {
            let agent_id = AgentId::from_path(agent_dir.path());
            let decision_path = self.decision_path(&agent_id);
            
            if decision_path.exists() {
                let agent = DecisionAgent::restore(&decision_path, self)?;
                agents.push(agent);
            }
        }
        
        Ok(agents)
    }
}
```

---

### Story 8.4: Decision Observability (Metrics, Logs)

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement observability for Decision Layer.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.4.1 | Create `DecisionMetrics` struct | Todo | - |
| T8.4.2 | Implement metrics collection | Todo | - |
| T8.4.3 | Implement decision logging format | Todo | - |
| T8.4.4 | Implement metrics aggregation | Todo | - |
| T8.4.5 | Implement quality tracking | Todo | - |
| T8.4.6 | Implement CLI metrics commands | Todo | - |
| T8.4.7 | Write unit tests for observability | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.4.T1 | Total decisions tracked |
| T8.4.T2 | Success rate calculated |
| T8.4.T3 | Decisions by type tracked |
| T8.4.T4 | Log format valid JSON |
| T8.4.T5 | CLI metrics output valid |

#### Acceptance Criteria

- Metrics collected per decision
- Logs structured and queryable
- Quality tracking works

#### Technical Notes

```rust
pub struct DecisionMetrics {
    /// Total decisions made
    total_decisions: u64,
    
    /// Successful decisions
    successful_decisions: u64,
    
    /// Success rate
    success_rate: f64,
    
    /// Decisions by type
    by_type: HashMap<DecisionType, TypeMetrics>,
    
    /// Average decision duration
    avg_duration_ms: u64,
    
    /// Timeout count
    timeout_count: u64,
    
    /// Human intervention count
    human_intervention_count: u64,
    
    /// Reflection effectiveness
    reflection_effectiveness: f64,
}

impl DecisionAgent {
    fn record_decision(&mut self, record: DecisionRecord) {
        // Update metrics
        self.metrics.total_decisions += 1;
        
        if record.output.is_success() {
            self.metrics.successful_decisions += 1;
        }
        
        // Update type metrics
        let type_metrics = self.metrics.by_type
            .entry(record.decision_type())
            .or_default();
        type_metrics.count += 1;
        type_metrics.total_duration_ms += record.duration_ms;
        
        // Recalculate averages
        self.metrics.recalculate();
    }
}

/// Decision log format
pub struct DecisionLog {
    decision_id: DecisionId,
    agent_id: AgentId,
    task_id: Option<TaskId>,
    timestamp: DateTime<Utc>,
    trigger_status: ProviderStatus,
    output: DecisionOutput,
    engine_type: DecisionEngineType,
    duration_ms: u64,
    success: bool,
    reflection_round: u8,
    retry_count: u8,
    context_size_bytes: usize,
    critical: bool,
    human_requested: bool,
}

/// CLI commands
// agile-agent decision metrics --agent alpha
// agile-agent decision history --agent alpha --period "last 7 days"
// agile-agent decision analyze --quality

pub fn decision_metrics_summary(metrics: &DecisionMetrics) -> String {
    format!(
        "Decision Metrics:\n\
         Total: {}\n\
         Success Rate: {:.1}%\n\
         Avg Duration: {}ms\n\
         Human Interventions: {}\n\
         Reflection Effectiveness: {:.1}%\n\
         \n\
         By Type:\n\
         {}",
        metrics.total_decisions,
        metrics.success_rate * 100,
        metrics.avg_duration_ms,
        metrics.human_intervention_count,
        metrics.reflection_effectiveness * 100,
        metrics.by_type.iter()
            .map(|(t, m)| format!("  {}: {} decisions", t, m.count))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
```

---

### Story 8.5: Cost Optimization Strategies

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Implement cost optimization for decision layer.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T8.5.1 | Implement tiered engine selection | Todo | - |
| T8.5.2 | Implement decision result caching | Todo | - |
| T8.5.3 | Implement context compression for prompts | Todo | - |
| T8.5.4 | Implement reflection policy optimization | Todo | - |
| T8.5.5 | Implement budget tracking | Todo | - |
| T8.5.6 | Write unit tests for optimization | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T8.5.T1 | Complexity threshold works |
| T8.5.T2 | Rule engine used for low complexity |
| T8.5.T3 | LLM engine used for high complexity |
| T8.5.T4 | Cache returns cached decision |
| T8.5.T5 | Budget tracked correctly |

#### Acceptance Criteria

- Cost reduced through tiered engines
- Caching works correctly
- Budget tracked

#### Technical Notes

```rust
/// Tiered decision engine - use cheaper engine for simple decisions
pub struct TieredDecisionEngine {
    /// Rule-based engine (fast, cheap)
    rule_engine: RuleBasedDecisionEngine,
    
    /// LLM engine (expensive, powerful)
    llm_engine: Option<LLMDecisionEngine>,
    
    /// Complexity threshold for LLM
    complexity_threshold: u8,
}

impl DecisionEngine for TieredDecisionEngine {
    fn decide(&mut self, context: DecisionContext) -> Result<DecisionOutput> {
        // Calculate complexity
        let complexity = self.calculate_complexity(&context);
        
        // Low complexity -> Rule engine
        if complexity < self.complexity_threshold {
            return self.rule_engine.decide(context);
        }
        
        // High complexity -> LLM engine
        if let Some(llm) = &mut self.llm_engine {
            return llm.decide(context);
        }
        
        // Fallback to rule engine
        self.rule_engine.decide(context)
    }
    
    fn calculate_complexity(&self, context: &DecisionContext) -> u8 {
        let mut score = 0;
        
        // Many options = complex
        if let ProviderStatus::WaitingForChoice { options } = &context.trigger_status {
            if options.len() > 3 { score += 1; }
        }
        
        // Completion verification = complex
        if context.trigger_status.is_completion() { score += 2; }
        
        // Large context = complex
        if context.running_context.size() > 5000 { score += 1; }
        
        score
    }
}

/// Decision result cache
pub struct DecisionCache {
    cache: HashMap<DecisionCacheKey, DecisionOutput>,
    ttl: Duration,
}

impl DecisionCache {
    fn get(&self, context: &DecisionContext) -> Option<DecisionOutput> {
        let key = DecisionCacheKey::from_context(context);
        self.cache.get(&key).cloned()
    }
    
    fn put(&mut self, context: &DecisionContext, output: DecisionOutput) {
        let key = DecisionCacheKey::from_context(context);
        self.cache.insert(key, output);
    }
}

/// Budget tracking
pub struct DecisionBudget {
    total_budget: f64,
    used: f64,
    warning_threshold: f64,
}

impl DecisionBudget {
    fn check(&self, estimated_cost: f64) -> Result<()> {
        if self.used + estimated_cost > self.total_budget {
            Err(Error::BudgetExceeded)
        } else if self.used + estimated_cost > self.warning_threshold {
            log::warn!("Budget approaching limit: {:.2}/{:.2}", self.used, self.total_budget);
            Ok(())
        } else {
            Ok(())
        }
    }
    
    fn record_usage(&mut self, cost: f64) {
        self.used += cost;
    }
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
│  │ AgentPool                                                        ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐                         ││
│  │  │ Slot A   │ │ Slot B   │ │ Slot C   │                         ││
│  │  │ Main     │ │ Main     │ │ Main     │                         ││
│  │  │ Agent    │ │ Agent    │ │ Agent    │                         ││
│  │  │ ┌──────┐ │ │ ┌──────┐ │ │ ┌──────┐ │                         ││
│  │  │ │Dec   │ │ │ │Dec   │ │ │ │Dec   │ │                         ││
│  │  │ │Agent │ │ │ │Agent │ │ │ │Agent │ │                         ││
│  │  │ └──────┘ │ │ └──────┘ │ │ └──────┘ │                         ││
│  │  └──────────┘ └──────────┘ └──────────┘                         ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ HumanDecisionQueue                                               ││
│  │  - Pending requests                                              ││
│  │  - History                                                       ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Shared Components                                                ││
│  │  - WorkplaceStore                                                ││
│  │  - Backlog                                                       ││
│  │  - Kanban                                                        ││
│  │  - Mail System                                                   ││
│  │  - ProjectRules                                                  ││
│  └─────────────────────────────────────────────────────────────────┘│
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │ Observability                                                    ││
│  │  - DecisionMetrics                                               ││
│  │  - DecisionLog                                                   ││
│  │  - BudgetTracker                                                 ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Integration complexity | Medium | Medium | Incremental integration, tests |
| Blocked state handling | Low | Medium | Clear policy configuration |
| Metrics overhead | Low | Low | Async collection |

## Sprint Deliverables

- `core/src/agent_slot.rs` - Extended with Decision Agent
- `core/src/agent_pool.rs` - Blocked handling
- `core/src/decision_integration.rs` - Integration helpers
- `decision/src/metrics.rs` - Observability
- Integration tests

## Dependencies

- Sprint 1-7: All decision layer components
- Multi-Agent Runtime: AgentPool, AgentSlot
- Kanban Sprint 1-5: Backlog, Kanban

## Final Release Checklist

After Sprint 8, validate:

1. [ ] Decision Agent created for each Main Agent
2. [ ] Provider output classified correctly (Claude/Codex/ACP)
3. [ ] Decision engines work (LLM/CLI/RuleBased)
4. [ ] Context cache respects limits
5. [ ] Human intervention works (blocked state)
6. [ ] Error recovery escalates correctly
7. [ ] Integration with AgentPool works
8. [ ] Integration with Kanban works
9. [ ] Metrics collected
10. [ ] All tests pass

## Project Complete

This is the final sprint. After completion, the Decision Layer is production-ready and integrated with agile-agent.