# Architecture Evolution Proposal

## Problem Analysis

Current architecture has several rigidity points that hinder iterative evolution:

| Problem | Current Design | Impact | Risk Level |
|---------|---------------|--------|------------|
| ProviderStatus enum | Fixed 4 statuses | Adding new situation requires 5+ file changes | High |
| Human Intervention coupling | Embedded in AgentSlotStatus | AgentPool must understand decision concepts | Medium |
| Decision Rules strategy | Fixed condition types | Cannot add new conditions dynamically | Medium |
| Decision Output types | Fixed 5 outputs | Adding new output type breaks parsing | Medium |
| Concurrent processing | Not designed | Multi-agent parallel decisions undefined | High |
| Provider Event mapping | Direct to status | New provider events require status enum change | Medium |

---

## Proposed Improvements

### 1. Status as Trait, Not Enum

**Current**:
```rust
pub enum ProviderStatus {
    WaitingForChoice { ... },
    ClaimsCompletion { ... },
    PartialCompletion { ... },
    Error { ... },
}
```

**Proposed**:
```rust
/// Decision situation trait - extensible
pub trait DecisionSituation: Send + Sync {
    /// Situation type identifier
    fn situation_type(&self) -> SituationType;
    
    /// Whether requires human escalation
    fn requires_human(&self) -> bool;
    
    /// Serialize for prompt
    fn to_prompt_text(&self) -> String;
    
    /// Available actions for this situation
    fn available_actions(&self) -> Vec<DecisionAction>;
}

/// Situation type registry
pub struct SituationRegistry {
    situations: HashMap<SituationType, Box<dyn DecisionSituation>>,
}

impl SituationRegistry {
    pub fn register(&mut self, situation: Box<dyn DecisionSituation>) {
        self.situations.insert(situation.situation_type(), situation);
    }
    
    pub fn get(&self, type: &SituationType) -> Option<&dyn DecisionSituation> {
        self.situations.get(type).map(|b| b.as_ref())
    }
}
```

**Benefit**: Adding new situation only requires implementing trait, registering. No enum modification.

---

### 2. Provider Event → Situation Mapping Layer

**Current**:
```
Provider Event → Classifier → ProviderStatus (enum)
```

**Proposed**:
```
Provider Event → Classifier → SituationType (string) → SituationRegistry → DecisionSituation
```

```rust
/// Classifier produces situation type, not full status
pub trait OutputClassifier {
    /// Classify event to situation type
    fn classify(&self, event: &ProviderEvent) -> Option<SituationType>;
    
    /// Build situation from event details
    fn build_situation(&self, event: &ProviderEvent, registry: &SituationRegistry) 
        -> Option<Box<dyn DecisionSituation>>;
}

/// Claude classifier
pub struct ClaudeClassifier;

impl OutputClassifier for ClaudeClassifier {
    fn classify(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            ProviderEvent::Finished { .. } => Some(SituationType::new("claude_finished")),
            ProviderEvent::Error { .. } => Some(SituationType::new("error")),
            _ => None, // Running
        }
    }
    
    fn build_situation(&self, event: &ProviderEvent, registry: &SituationRegistry) 
        -> Option<Box<dyn DecisionSituation>> {
        let type = self.classify(event)?;
        registry.build_from_event(type, event)
    }
}
```

**Benefit**: New provider events only need classifier update, no situation enum change.

---

### 3. Human Intervention as Independent Queue System

**Current**:
```rust
pub enum AgentSlotStatus {
    BlockedForHumanDecision { ... },
}
```

**Proposed**:
```rust
/// Blocking reason trait - extensible
pub trait BlockingReason: Send + Sync {
    fn reason_type(&self) -> &str;
    fn urgency(&self) -> UrgencyLevel;
    fn expires_at(&self) -> Option<DateTime<Utc>>;
    fn can_auto_resolve(&self) -> bool;
    fn auto_resolve_action(&self) -> Option<AutoAction>;
}

/// Blocked state - generic
pub struct BlockedState {
    reason: Box<dyn BlockingReason>,
    blocked_at: Instant,
    context: BlockingContext,
}

/// Agent slot status - generic blocked
pub enum AgentSlotStatus {
    Running,
    Blocked(BlockedState),
    Idle,
    Stopped,
}

/// Human decision is ONE type of blocking reason
pub struct HumanDecisionBlocking {
    decision_request: HumanDecisionRequest,
}

impl BlockingReason for HumanDecisionBlocking {
    fn reason_type(&self) -> &str { "human_decision" }
    fn urgency(&self) -> UrgencyLevel { self.decision_request.urgency }
    fn expires_at(&self) -> Option<DateTime<Utc>> { Some(self.decision_request.expires_at) }
    fn can_auto_resolve(&self) -> bool { true }
    fn auto_resolve_action(&self) -> Option<AutoAction> { 
        Some(AutoAction::FollowRecommendation)
    }
}

/// Resource waiting is another blocking reason
pub struct ResourceBlocking {
    resource_type: String,
    resource_id: String,
}

impl BlockingReason for ResourceBlocking {
    fn reason_type(&self) -> &str { "resource_waiting" }
    fn urgency(&self) -> UrgencyLevel { UrgencyLevel::Low }
    fn expires_at(&self) -> Option<DateTime<Utc>> { None }
    fn can_auto_resolve(&self) -> bool { true }
    fn auto_resolve_action(&self) -> Option<AutoAction> { None }
}
```

**Benefit**: AgentPool only understands "Blocked", not specific blocking reasons. New blocking types implement trait.

---

### 4. Decision Rules as Expression Engine

**Current**:
```rust
pub struct RuleConditions {
    status: Option<ProviderStatusPattern>,
    project_rule_keywords: Vec<String>,
    story_type: Option<String>,
}
```

**Proposed**:
```rust
/// Rule condition expression - supports AND/OR/comparison
pub enum ConditionExpr {
    /// Single condition
    Single(Condition),
    
    /// AND combination
    And(Vec<ConditionExpr>),
    
    /// OR combination
    Or(Vec<ConditionExpr>),
    
    /// NOT
    Not(ConditionExpr),
}

pub enum Condition {
    /// Status type matches
    StatusType { type: SituationType },
    
    /// Project rule keyword
    ProjectKeyword { keyword: String },
    
    /// Story type
    StoryType { type: String },
    
    /// Time since last action
    TimeSinceAction { min_seconds: u64, max_seconds: u64 },
    
    /// Reflection rounds
    ReflectionRounds { min: u8, max: u8 },
    
    /// Custom condition (extensible)
    Custom { name: String, params: HashMap<String, String> },
}

/// Custom condition evaluator registry
pub struct ConditionEvaluatorRegistry {
    evaluators: HashMap<String, Box<dyn ConditionEvaluator>>,
}

pub trait ConditionEvaluator {
    fn evaluate(&self, context: &DecisionContext, params: &HashMap<String, String>) -> bool;
}
```

**Benefit**: Conditions can be combined, new condition types can be registered.

---

### 5. Decision Output as Action Sequence

**Current**:
```rust
pub enum DecisionOutput {
    Choice { selected: String, reason: String },
    ReflectionRequest { prompt: String },
    CompletionConfirm { submit_pr: bool, next_task: Option<TaskId> },
    ContinueInstruction { prompt: String, focus_items: Vec<String> },
    RetryInstruction { prompt: String, cooldown_ms: u64 },
}
```

**Proposed**:
```rust
/// Decision action - atomic action
pub trait DecisionAction: Send + Sync {
    fn action_type(&self) -> &str;
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgent) -> Result<ActionResult>;
    fn to_prompt_text(&self) -> String;
}

/// Decision output - sequence of actions
pub struct DecisionOutput {
    actions: Vec<Box<dyn DecisionAction>>,
    reasoning: String,
    confidence: f64,
}

/// Select option action
pub struct SelectOptionAction {
    option_id: String,
    reason: String,
}

impl DecisionAction for SelectOptionAction {
    fn action_type(&self) -> &str { "select_option" }
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgent) -> Result<ActionResult> {
        agent.send_selection(self.option_id)?;
        Ok(ActionResult::Success)
    }
}

/// Reflect action
pub struct ReflectAction {
    prompt: String,
}

impl DecisionAction for ReflectAction {
    fn action_type(&self) -> &str { "reflect" }
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgent) -> Result<ActionResult> {
        agent.send_prompt(self.prompt)?;
        Ok(ActionResult::NeedsFollowUp)
    }
}

/// Delegate action (new - can delegate to other agent)
pub struct DelegateAction {
    target_agent: AgentId,
    task_description: String,
}

impl DecisionAction for DelegateAction {
    fn action_type(&self) -> &str { "delegate" }
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgent) -> Result<ActionResult> {
        agent.delegate_to(self.target_agent, &self.task_description)?;
        Ok(ActionResult::Delegated)
    }
}

/// Action registry - extensible
pub struct ActionRegistry {
    actions: HashMap<String, Box<dyn DecisionAction>>,
}
```

**Benefit**: Output is action sequence, new action types can be registered.

---

### 6. Concurrent Decision Processing Design

**Current**: Not specified

**Proposed**:
```rust
/// Decision session pool - reuse sessions
pub struct DecisionSessionPool {
    /// Available sessions per provider type
    available: HashMap<ProviderKind, VecDeque<SessionHandle>>,
    
    /// Active sessions (in use)
    active: HashMap<AgentId, SessionHandle>,
    
    /// Max sessions per provider
    max_per_provider: usize,
    
    /// Session idle timeout
    idle_timeout_ms: u64,
}

impl DecisionSessionPool {
    pub fn acquire(&mut self, provider: ProviderKind, agent_id: AgentId) -> Result<SessionHandle> {
        // Check available pool
        if let Some(pool) = self.available.get_mut(&provider) {
            if let Some(session) = pool.pop_front() {
                self.active.insert(agent_id, session.clone());
                return Ok(session);
            }
        }
        
        // Create new if under limit
        let active_count = self.active.values().filter(|s| s.provider == provider).count();
        if active_count < self.max_per_provider {
            let session = self.create_session(provider)?;
            self.active.insert(agent_id, session.clone());
            return Ok(session);
        }
        
        // Wait for available
        Err(Error::SessionPoolExhausted)
    }
    
    pub fn release(&mut self, agent_id: AgentId) {
        if let Some(session) = self.active.remove(&agent_id) {
            self.available.get_mut(&session.provider)
                .map(|pool| pool.push_back(session));
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
}

/// Human decision arbitration - when multiple agents request
pub struct HumanDecisionArbitrator {
    /// Pending requests by priority
    queue: PriorityQueue<HumanDecisionRequest>,
    
    /// Current request being handled
    current: Option<HumanDecisionRequest>,
    
    /// Arbitration strategy
    strategy: ArbitrationStrategy,
}

pub enum ArbitrationStrategy {
    /// Handle one at a time, block others
    Sequential,
    
    /// Batch similar requests, handle together
    BatchSimilar,
    
    /// Parallel handling (multiple TUI modals)
    Parallel,
}
```

**Benefit**: Explicit concurrent handling, resource pooling, rate limiting.

---

## Architecture Comparison

| Aspect | Current | Proposed | Evolution Capability |
|--------|---------|----------|---------------------|
| Situation types | Enum (fixed) | Trait (extensible) | Add without code change |
| Blocking reasons | Enum embedded | Trait (extensible) | Add new blocking types |
| Decision actions | Enum (fixed) | Trait (extensible) | Add new action types |
| Rule conditions | Fixed fields | Expression engine | Complex conditions, custom evaluators |
| Provider events | Direct mapping | Registry layer | New events without enum change |
| Concurrency | Undefined | Session pool, rate limiter | Explicit resource management |

---

## Implementation Strategy

### Phase 1: Core Trait Abstractions (Sprint 1 Update)

1. Convert ProviderStatus → DecisionSituation trait
2. Convert DecisionOutput → DecisionAction trait
3. Create SituationRegistry and ActionRegistry

### Phase 2: Blocking State Abstraction (Sprint 6 Update)

1. Convert BlockedForHumanDecision → BlockingReason trait
2. Create BlockedState struct
3. Update AgentSlotStatus to generic Blocked

### Phase 3: Rule Expression Engine (Sprint 3 Update)

1. Convert RuleConditions → ConditionExpr
2. Create ConditionEvaluatorRegistry
3. Update TOML rule format

### Phase 4: Concurrent Processing (New Sprint)

Add explicit concurrent handling design.

---

## Trade-offs

| Choice | Benefit | Cost |
|--------|---------|------|
| Trait over Enum | Extensible | More complex, dynamic dispatch |
| Registry pattern | Plugin-like | Runtime registration needed |
| Expression engine | Flexible rules | Parsing complexity |
| Session pool | Resource efficiency | Pool management overhead |

---

## Recommendation

**High priority**: Situation/Action/Blocking trait abstractions - these are core extension points.

**Medium priority**: Rule expression engine - improves rule flexibility.

**Should add**: Concurrent processing design - critical for multi-agent.

This proposal should be reviewed before Sprint 1 implementation begins.