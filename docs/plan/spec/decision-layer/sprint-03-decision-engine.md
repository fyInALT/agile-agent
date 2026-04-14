# Sprint 3: Decision Engine (Trait-Based)

## Metadata

- Sprint ID: `decision-sprint-003`
- Title: `Decision Engine (Trait-Based)`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14
- Updated: 2026-04-14 (Architecture Evolution)

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 3 Tests: T3.1.T1-T3.6.T8 (34 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Architecture Evolution

Decision engines now:
- Return **DecisionOutput with Vec<Box<dyn DecisionAction>>** (action sequence)
- Use **ActionRegistry** for parsing LLM output
- Use **ConditionExpr** for rule-based engine (expression engine)

## Sprint Goal

Implement decision engines that produce action sequences from decision context, using ActionRegistry for parsing and extensible rule expression engine.

## Stories

### Story 3.1: DecisionEngine Trait (Action-Based)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define DecisionEngine trait that returns action sequences.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Create `DecisionEngine` trait | Todo | - |
| T3.1.2 | Define `decide()` returning DecisionOutput with actions | Todo | - |
| T3.1.3 | Define `build_prompt()` using situation.to_prompt_text() | Todo | - |
| T3.1.4 | Define `parse_response()` using ActionRegistry | Todo | - |
| T3.1.5 | Define session management methods | Todo | - |
| T3.1.6 | Write trait documentation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.1.T1 | engine_type() returns correct type |
| T3.1.T2 | decide() returns DecisionOutput with actions |
| T3.1.T3 | build_prompt() uses situation trait |
| T3.1.T4 | parse_response() uses ActionRegistry |
| T3.1.T5 | session_handle() returns Option |
| T3.1.T6 | is_healthy() returns bool |
| T3.1.T7 | reset() clears state |

#### Acceptance Criteria

- Trait returns action sequences (Vec<Box<dyn DecisionAction>>)
- Prompt built from situation trait
- Response parsed via ActionRegistry

#### Technical Notes

```rust
/// Decision engine trait - returns action sequences
pub trait DecisionEngine: Send + Sync {
    /// Engine type
    fn engine_type(&self) -> DecisionEngineType;
    
    /// Make a decision based on context
    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> Result<DecisionOutput>;
    
    /// Build decision prompt from context
    fn build_prompt(&self, context: &DecisionContext, action_registry: &ActionRegistry) -> String;
    
    /// Parse response to action sequence
    fn parse_response(
        &self,
        response: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> Result<Vec<Box<dyn DecisionAction>>>;
    
    /// Get current session handle
    fn session_handle(&self) -> Option<&SessionHandle>;
    
    /// Check engine health
    fn is_healthy(&self) -> bool;
    
    /// Reset engine state
    fn reset(&mut self) -> Result<()>;
    
    /// Persist session for multi-turn
    fn persist_session(&self, path: &Path) -> Result<()>;
    
    /// Restore session from persistence
    fn restore_session(&mut self, path: &Path) -> Result<()>;
}

/// Decision output - action sequence
pub struct DecisionOutput {
    /// Action sequence to execute
    actions: Vec<Box<dyn DecisionAction>>,
    
    /// Reasoning explanation
    reasoning: String,
    
    /// Confidence level (0.0-1.0)
    confidence: f64,
    
    /// Whether requested human intervention
    human_requested: bool,
}

impl DecisionOutput {
    pub fn new(actions: Vec<Box<dyn DecisionAction>>, reasoning: String, confidence: f64) -> Self {
        Self {
            actions,
            reasoning,
            confidence,
            human_requested: false,
        }
    }
    
    pub fn with_human_request(mut self) -> Self {
        self.human_requested = true;
        self
    }
    
    pub fn actions(&self) -> &[Box<dyn DecisionAction>] {
        &self.actions
    }
    
    pub fn first_action(&self) -> Option<&dyn DecisionAction> {
        self.actions.first().map(|b| b.as_ref())
    }
    
    /// Execute all actions sequentially
    pub fn execute(
        &self,
        context: &DecisionContext,
        agent: &mut MainAgentConnection,
    ) -> Result<Vec<ActionResult>> {
        let mut results = Vec::new();
        for action in &self.actions {
            let result = action.execute(context, agent)?;
            results.push(result);
            
            // Stop if action failed
            if let ActionResult::Failed { .. } = &result {
                break;
            }
        }
        Ok(results)
    }
}

/// Decision engine type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionEngineType {
    LLM { provider: ProviderKind },
    CLI { provider: ProviderKind },
    RuleBased,
    Mock,
    Custom { name: String },
}
```

---

### Story 3.2: LLM Decision Engine

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement LLM-based decision engine with ActionRegistry parsing.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Create `LLMDecisionEngine` struct | Todo | - |
| T3.2.2 | Implement `build_prompt()` using situation trait | Todo | - |
| T3.2.3 | Implement prompt templates for situation types | Todo | - |
| T3.2.4 | Implement `call_llm_with_timeout()` | Todo | - |
| T3.2.5 | Implement `parse_response()` via ActionRegistry | Todo | - |
| T3.2.6 | Implement session persistence | Todo | - |
| T3.2.7 | Write unit tests with mock LLM | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.2.T1 | Prompt contains situation.to_prompt_text() |
| T3.2.T2 | Prompt contains available actions from registry |
| T3.2.T3 | Response parsed to SelectOptionAction |
| T3.2.T4 | Response parsed to ReflectAction |
| T3.2.T5 | Response parsed to ConfirmCompletionAction |
| T3.2.T6 | Timeout handled gracefully |
| T3.2.T7 | Session persisted and restored |

#### Acceptance Criteria

- Prompt uses situation trait for context
- Response parsed via ActionRegistry
- Timeout handled gracefully

#### Technical Notes

```rust
/// LLM decision engine
pub struct LLMDecisionEngine {
    provider: ProviderKind,
    session: Option<SessionHandle>,
    config: DecisionAgentConfig,
}

impl DecisionEngine for LLMDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::LLM { provider: self.provider }
    }
    
    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> Result<DecisionOutput> {
        // 1. Build prompt from situation + available actions
        let prompt = self.build_prompt(&context, action_registry);
        
        // 2. Call LLM with timeout
        let response = self.call_llm_with_timeout(prompt)?;
        
        // 3. Parse response to actions
        let actions = self.parse_response(
            &response,
            context.trigger_situation.as_ref(),
            action_registry,
        )?;
        
        // 4. Calculate confidence from response
        let confidence = self.extract_confidence(&response);
        
        // 5. Extract reasoning
        let reasoning = self.extract_reasoning(&response);
        
        Ok(DecisionOutput::new(actions, reasoning, confidence))
    }
    
    fn build_prompt(&self, context: &DecisionContext, action_registry: &ActionRegistry) -> String {
        format!(
            "You are a decision helper for a development agent.\n\
            \n\
            ## Current Situation\n\
            {}\n\
            \n\
            ## Available Actions\n\
            {}\n\
            \n\
            ## Project Rules\n\
            {}\n\
            \n\
            ## Current Task\n\
            {}\n\
            \n\
            ## Running Context\n\
            {}\n\
            \n\
            ## Instructions\n\
            Select an action from Available Actions.\n\
            Output format:\n\
            {}\n\
            \n\
            Confidence: [0.0-1.0]\n\
            Reasoning: [Brief explanation]",
            context.trigger_situation.to_prompt_text(),
            action_registry.generate_prompt_formats(),
            context.project_rules.summary(),
            context.current_task.map(|t| t.definition()).unwrap_or_default(),
            context.running_context.summary(),
            context.trigger_situation.available_actions()
                .iter()
                .map(|a| action_registry.get(a)
                    .map(|act| act.to_prompt_format())
                    .unwrap_or_else(|| a.name.clone()))
                .collect::<Vec<_>>()
                .join("\nOR\n")
        )
    }
    
    fn parse_response(
        &self,
        response: &str,
        situation: &dyn DecisionSituation,
        action_registry: &ActionRegistry,
    ) -> Result<Vec<Box<dyn DecisionAction>>> {
        // Try each available action type
        for action_type in situation.available_actions() {
            if let Some(action) = action_registry.parse(action_type.clone(), response) {
                return Ok(vec![action]);
            }
        }
        
        // Fallback: parse generic output
        self.parse_generic_response(response)
    }
}
```

---

### Story 3.3: CLI Decision Engine (Independent Session)

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement CLI decision engine with independent provider session.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Create `CLIDecisionEngine` struct | Todo | - |
| T3.3.2 | Implement independent session spawning | Todo | - |
| T3.3.3 | Implement provider thread management | Todo | - |
| T3.3.4 | Implement event channel collection | Todo | - |
| T3.3.5 | Implement output parsing to actions | Todo | - |
| T3.3.6 | Write unit tests for CLI engine | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.3.T1 | CLI session different from main agent |
| T3.3.T2 | Provider thread spawns correctly |
| T3.3.T3 | Events collected until blocked |
| T3.3.T4 | Output parsed to actions |
| T3.3.T5 | Session persists and restores |

#### Acceptance Criteria

- CLI engine uses independent session
- Provider thread managed correctly
- Output parsed via ActionRegistry

#### Technical Notes

```rust
/// CLI decision engine with independent session
pub struct CLIDecisionEngine {
    provider: ProviderKind,
    session: Option<SessionHandle>,
    agent_id: AgentId,
    parent_agent_id: AgentId,
    config: DecisionAgentConfig,
    event_rx: Option<mpsc::Receiver<ProviderEvent>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl DecisionEngine for CLIDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::CLI { provider: self.provider }
    }
    
    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> Result<DecisionOutput> {
        // 1. Spawn or resume provider session
        if self.session.is_none() {
            self.spawn_provider_session()?;
        }
        
        // 2. Build and send prompt
        let prompt = self.build_prompt(&context, action_registry);
        self.send_prompt(prompt)?;
        
        // 3. Collect output until blocked
        let output = self.collect_output()?;
        
        // 4. Parse via ActionRegistry
        let actions = self.parse_response(
            &output,
            context.trigger_situation.as_ref(),
            action_registry,
        )?;
        
        Ok(DecisionOutput::new(actions, "CLI decision".into(), 0.8))
    }
}
```

---

### Story 3.4: Rule-Based Decision Engine with Expression Engine

**Priority**: P1
**Effort**: 4 points
**Status**: Backlog

Implement rule-based engine with condition expression engine.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Create `ConditionExpr` enum (AND/OR/NOT) | Todo | - |
| T3.4.2 | Create `Condition` enum with variants | Todo | - |
| T3.4.3 | Create `ConditionEvaluatorRegistry` | Todo | - |
| T3.4.4 | Create `DecisionRule` with condition + actions | Todo | - |
| T3.4.5 | Implement `RuleBasedDecisionEngine` | Todo | - |
| T3.4.6 | Implement default rules | Todo | - |
| T3.4.7 | Implement custom rule loading from TOML | Todo | - |
| T3.4.8 | Write unit tests for expression engine | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.4.T1 | ConditionExpr::And evaluates correctly |
| T3.4.T2 | ConditionExpr::Or evaluates correctly |
| T3.4.T3 | Condition::StatusType matches |
| T3.4.T4 | Condition::ProjectKeyword matches |
| T3.4.T5 | Condition::Custom evaluates via registry |
| T3.4.T6 | Default rules cover common cases |
| T3.4.T7 | TOML rules loaded correctly |

#### Acceptance Criteria

- Expression engine supports AND/OR/NOT/Custom
- Rules produce action sequences
- Custom condition evaluators can be registered

#### Technical Notes

```rust
/// Condition expression - supports complex logic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConditionExpr {
    /// Single condition
    Single(Condition),
    
    /// AND combination - all must match
    And(Vec<ConditionExpr>),
    
    /// OR combination - any must match
    Or(Vec<ConditionExpr>),
    
    /// NOT - negates inner expression
    Not(ConditionExpr),
}

impl ConditionExpr {
    /// Evaluate against context
    pub fn evaluate(&self, context: &DecisionContext, evaluator_registry: &ConditionEvaluatorRegistry) -> bool {
        match self {
            ConditionExpr::Single(cond) => cond.evaluate(context, evaluator_registry),
            
            ConditionExpr::And(exprs) => exprs.iter()
                .all(|e| e.evaluate(context, evaluator_registry)),
            
            ConditionExpr::Or(exprs) => exprs.iter()
                .any(|e| e.evaluate(context, evaluator_registry)),
            
            ConditionExpr::Not(expr) => !expr.evaluate(context, evaluator_registry),
        }
    }
}

/// Single condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// Situation type matches
    SituationType { type: String },
    
    /// Project rule keyword present
    ProjectKeyword { keyword: String },
    
    /// Story type matches
    StoryType { type: String },
    
    /// Reflection rounds within range
    ReflectionRounds { min: u8, max: u8 },
    
    /// Confidence below threshold
    ConfidenceBelow { threshold: f64 },
    
    /// Time since last action
    TimeSinceLastAction { min_seconds: u64, max_seconds: Option<u64> },
    
    /// Custom condition (extensible)
    Custom { name: String, params: HashMap<String, String> },
}

impl Condition {
    pub fn evaluate(&self, context: &DecisionContext, registry: &ConditionEvaluatorRegistry) -> bool {
        match self {
            Condition::SituationType { type } => {
                context.trigger_situation.situation_type().name == *type
            }
            
            Condition::ProjectKeyword { keyword } => {
                context.project_rules.contains_keyword(keyword)
            }
            
            Condition::StoryType { type } => {
                context.current_story
                    .map(|s| s.story_type() == *type)
                    .unwrap_or(false)
            }
            
            Condition::ReflectionRounds { min, max } => {
                // Extract from situation if ClaimsCompletion
                false // TODO: implement
            }
            
            Condition::ConfidenceBelow { threshold } => {
                // From situation if available
                false
            }
            
            Condition::Custom { name, params } => {
                registry.evaluate(name, context, params)
            }
            
            _ => false,
        }
    }
}

/// Custom condition evaluator registry
pub struct ConditionEvaluatorRegistry {
    evaluators: HashMap<String, Box<dyn ConditionEvaluator>>,
}

pub trait ConditionEvaluator: Send + Sync {
    fn evaluate(&self, context: &DecisionContext, params: &HashMap<String, String>) -> bool;
}

impl ConditionEvaluatorRegistry {
    pub fn new() -> Self {
        Self { evaluators: HashMap::new() }
    }
    
    pub fn register(&mut self, name: impl Into<String>, evaluator: Box<dyn ConditionEvaluator>) {
        self.evaluators.insert(name.into(), evaluator);
    }
    
    pub fn evaluate(&self, name: &str, context: &DecisionContext, params: &HashMap<String, String>) -> bool {
        self.evaluators.get(name)
            .map(|e| e.evaluate(context, params))
            .unwrap_or(false)
    }
}

/// Decision rule - condition + action sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRule {
    pub name: String,
    pub condition: ConditionExpr,
    pub actions: Vec<ActionSpec>,
    pub priority: RulePriority,
}

/// Action specification for rule output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
    pub type: String,
    pub params: HashMap<String, String>,
}

impl ActionSpec {
    pub fn to_action(&self, registry: &ActionRegistry) -> Option<Box<dyn DecisionAction>> {
        // Build action from spec
        // TODO: implement action factory
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RulePriority {
    Low,
    Medium,
    High,
    Critical,
}

/// Rule-based decision engine
pub struct RuleBasedDecisionEngine {
    rules: Vec<DecisionRule>,
    default_rules: Vec<DecisionRule>,
    evaluator_registry: ConditionEvaluatorRegistry,
}

impl RuleBasedDecisionEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            default_rules: Self::builtin_rules(),
            evaluator_registry: ConditionEvaluatorRegistry::new(),
        }
    }
    
    pub fn with_custom_rules(rules: Vec<DecisionRule>) -> Self {
        Self {
            rules,
            default_rules: Self::builtin_rules(),
            evaluator_registry: ConditionEvaluatorRegistry::new(),
        }
    }
    
    pub fn register_evaluator(&mut self, name: impl Into<String>, evaluator: Box<dyn ConditionEvaluator>) {
        self.evaluator_registry.register(name, evaluator);
    }
    
    fn builtin_rules() -> Vec<DecisionRule> {
        vec![
            // Rule: Approve safe reads
            DecisionRule {
                name: "approve-read".into(),
                condition: ConditionExpr::Single(Condition::SituationType { 
                    type: "waiting_for_choice".into() 
                }),
                actions: vec![ActionSpec { 
                    type: "select_first".into(), 
                    params: HashMap::new() 
                }],
                priority: RulePriority::Medium,
            },
            
            // Rule: Reflect on first completion claim
            DecisionRule {
                name: "reflect-first".into(),
                condition: ConditionExpr::And(vec![
                    ConditionExpr::Single(Condition::SituationType { 
                        type: "claims_completion".into() 
                    }),
                    ConditionExpr::Single(Condition::ReflectionRounds { min: 0, max: 1 }),
                ]),
                actions: vec![ActionSpec { 
                    type: "reflect".into(), 
                    params: HashMap::new() 
                }],
                priority: RulePriority::High,
            },
            
            // Rule: Retry on recoverable error
            DecisionRule {
                name: "retry-error".into(),
                condition: ConditionExpr::Single(Condition::SituationType { 
                    type: "error".into() 
                }),
                actions: vec![ActionSpec { 
                    type: "retry".into(), 
                    params: HashMap::new() 
                }],
                priority: RulePriority::Medium,
            },
        ]
    }
    
    fn find_matching_rule(&self, context: &DecisionContext) -> Option<&DecisionRule> {
        // Sort by priority, find first match
        let all_rules: Vec<&DecisionRule> = self.rules.iter()
            .chain(self.default_rules.iter())
            .collect();
        
        // Sort by priority (Critical > High > Medium > Low)
        let sorted = all_rules.into_iter()
            .sorted_by_key(|r| match r.priority {
                RulePriority::Critical => 0,
                RulePriority::High => 1,
                RulePriority::Medium => 2,
                RulePriority::Low => 3,
            });
        
        for rule in sorted {
            if rule.condition.evaluate(context, &self.evaluator_registry) {
                return Some(rule);
            }
        }
        
        None
    }
}

impl DecisionEngine for RuleBasedDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::RuleBased
    }
    
    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> Result<DecisionOutput> {
        let rule = self.find_matching_rule(&context);
        
        if let Some(rule) = rule {
            let actions = rule.actions.iter()
                .filter_map(|spec| spec.to_action(action_registry))
                .collect();
            
            return Ok(DecisionOutput::new(
                actions,
                format!("Rule: {}", rule.name),
                0.9,
            ));
        }
        
        // No matching rule - default action
        Ok(DecisionOutput::new(
            vec![Box::new(CustomInstructionAction::new("Continue with current task"))],
            "No matching rule".into(),
            0.5,
        ))
    }
    
    fn is_healthy(&self) -> bool { true }
    
    fn reset(&mut self) -> Result<()> { Ok(()) }
}
```

**TOML Rule Configuration**:

```toml
[[decision_layer.rules]]
name = "approve-write-safe"
priority = "medium"

[decision_layer.rules.condition]
type = "and"
conditions = [
    { type = "single", condition = { situation_type = { type = "waiting_for_choice" } } },
    { type = "single", condition = { project_keyword = { keyword = "safe_write" } } }
]

[[decision_layer.rules.actions]]
type = "select_option"
params = { option_id = "approved", reason = "Safe write operation" }

[[decision_layer.rules]]
name = "deny-dangerous"
priority = "critical"

[decision_layer.rules.condition]
type = "or"
conditions = [
    { type = "single", condition = { custom = { name = "command_contains", params = { pattern = "rm -rf" } } } },
    { type = "single", condition = { custom = { name = "command_contains", params = { pattern = "sudo" } } } }
]

[[decision_layer.rules.actions]]
type = "select_option"
params = { option_id = "denied", reason = "Dangerous command" }
```

---

### Story 3.5: Mock Decision Engine

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement mock decision engine for testing.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.5.1 | Create `MockDecisionEngine` struct | Todo | - |
| T3.5.2 | Implement configurable mock responses | Todo | - |
| T3.5.3 | Implement decision recording | Todo | - |
| T3.5.4 | Write unit tests for mock engine | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.5.T1 | Returns first option for waiting_for_choice |
| T3.5.T2 | Returns reflect for claims_completion round 0 |
| T3.5.T3 | Returns confirm for claims_completion round >= 2 |
| T3.5.T4 | Returns retry for error |
| T3.5.T5 | History recorded |
| T3.5.T6 | Always healthy |

#### Acceptance Criteria

- Mock returns predefined action sequences
- Decision history recorded
- Useful for testing

#### Technical Notes

```rust
/// Mock decision engine for testing
pub struct MockDecisionEngine {
    history: Vec<DecisionRecord>,
    config: DecisionAgentConfig,
}

impl DecisionEngine for MockDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::Mock
    }
    
    fn decide(
        &mut self,
        context: DecisionContext,
        _action_registry: &ActionRegistry,
    ) -> Result<DecisionOutput> {
        let situation = context.trigger_situation.as_ref();
        let actions = self.get_mock_actions(situation);
        
        let record = DecisionRecord {
            decision_id: DecisionId::new(),
            timestamp: Utc::now(),
            situation_type: situation.situation_type(),
            actions: actions.iter().map(|a| a.action_type()).collect(),
            reasoning: "Mock decision".into(),
            confidence: 0.8,
            engine_type: DecisionEngineType::Mock,
        };
        
        self.history.push(record);
        
        Ok(DecisionOutput::new(actions, "Mock".into(), 0.8))
    }
    
    fn is_healthy(&self) -> bool { true }
    
    fn reset(&mut self) -> Result<()> {
        self.history.clear();
        Ok(())
    }
}

impl MockDecisionEngine {
    fn get_mock_actions(&self, situation: &dyn DecisionSituation) -> Vec<Box<dyn DecisionAction>> {
        match situation.situation_type().name.as_str() {
            "waiting_for_choice" => vec![
                Box::new(SelectOptionAction::new("A", "Mock: first option"))
            ],
            "claims_completion" => {
                // Need to check reflection_rounds from situation data
                vec![Box::new(ReflectAction::new("Mock: please reflect"))]
            },
            "error" => vec![
                Box::new(RetryAction::new("Mock: retry", 1000, false))
            ],
            _ => vec![
                Box::new(CustomInstructionAction::new("Mock: continue"))
            ],
        }
    }
    
    pub fn history(&self) -> &[DecisionRecord] {
        &self.history
    }
}
```

---

### Story 3.6: Tiered Decision Engine

**Priority**: P2
**Effort**: 3 points
**Status**: Backlog

Implement tiered engine that selects engine based on complexity.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.6.1 | Create `TieredDecisionEngine` struct | Todo | - |
| T3.6.2 | Implement complexity calculation | Todo | - |
| T3.6.3 | Implement engine selection logic | Todo | - |
| T3.6.4 | Write unit tests for tiered selection | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.6.T1 | Low complexity → RuleBased |
| T3.6.T2 | High complexity → LLM |
| T3.6.T3 | RuleBased fallback when LLM unavailable |
| T3.6.T4 | Complexity threshold configurable |
| T3.6.T5 | Complexity score calculated correctly |
| T3.6.T6 | LLM timeout → RuleBased fallback |
| T3.6.T7 | Budget check before LLM call |
| T3.6.T8 | Budget tracked correctly |

#### Acceptance Criteria

- Low complexity uses rule-based engine
- High complexity uses LLM engine
- Fallback on timeout/error

#### Technical Notes

```rust
/// Tiered decision engine - selects based on complexity
pub struct TieredDecisionEngine {
    rule_engine: RuleBasedDecisionEngine,
    llm_engine: Option<LLMDecisionEngine>,
    complexity_threshold: u8,
    config: TieredConfig,
}

pub struct TieredConfig {
    /// Complexity threshold for LLM
    complexity_threshold: u8,
    
    /// Always use rule for certain situation types
    always_rule_types: Vec<String>,
    
    /// Budget tracking
    budget: DecisionBudget,
}

impl TieredDecisionEngine {
    fn calculate_complexity(&self, context: &DecisionContext) -> u8 {
        let mut score = 0;
        
        let situation = context.trigger_situation.as_ref();
        let type_name = situation.situation_type().name.as_str();
        
        // Claims completion = high complexity (needs verification)
        if type_name == "claims_completion" {
            score += 2;
        }
        
        // Critical situation = high complexity
        if situation.requires_human() {
            score += 2;
        }
        
        // Large context = more complex
        if context.running_context.size_estimate() > 5000 {
            score += 1;
        }
        
        // Project rules complexity
        if context.project_rules.rules.len() > 10 {
            score += 1;
        }
        
        score
    }
}

impl DecisionEngine for TieredDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::Custom { name: "tiered".into() }
    }
    
    fn decide(
        &mut self,
        context: DecisionContext,
        action_registry: &ActionRegistry,
    ) -> Result<DecisionOutput> {
        let situation = context.trigger_situation.as_ref();
        let type_name = situation.situation_type().name.as_str();
        
        // Check always-rule types
        if self.config.always_rule_types.contains(&type_name.to_string()) {
            return self.rule_engine.decide(context, action_registry);
        }
        
        // Calculate complexity
        let complexity = self.calculate_complexity(&context);
        
        // Low complexity → Rule engine
        if complexity < self.complexity_threshold {
            return self.rule_engine.decide(context, action_registry);
        }
        
        // High complexity → LLM engine (with budget check)
        if let Some(llm) = &mut self.llm_engine {
            // Check budget
            let estimated_cost = self.estimate_llm_cost(&context);
            if !self.config.budget.check(estimated_cost)? {
                // Budget exceeded → use rule engine
                return self.rule_engine.decide(context, action_registry);
            }
            
            // Try LLM
            let result = llm.decide(context.clone(), action_registry);
            
            match result {
                Ok(output) => {
                    self.config.budget.record_usage(estimated_cost);
                    Ok(output)
                }
                Err(_) => {
                    // LLM failed → fallback to rule engine
                    self.rule_engine.decide(context, action_registry)
                }
            }
        } else {
            // No LLM available → use rule engine
            self.rule_engine.decide(context, action_registry)
        }
    }
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| LLM parsing complexity | Medium | Medium | Standardized output format |
| Rule expression complexity | Low | Low | Clear TOML schema |
| Tiered engine selection | Low | Medium | Complexity threshold tuning |

## Sprint Deliverables

- `decision/src/engine.rs` - DecisionEngine trait
- `decision/src/output.rs` - DecisionOutput with actions
- `decision/src/llm_engine.rs` - LLMDecisionEngine
- `decision/src/cli_engine.rs` - CLIDecisionEngine
- `decision/src/rule_engine.rs` - RuleBasedDecisionEngine with ConditionExpr
- `decision/src/condition.rs` - ConditionExpr, Condition, ConditionEvaluatorRegistry
- `decision/src/mock_engine.rs` - MockDecisionEngine
- `decision/src/tiered_engine.rs` - TieredDecisionEngine

## Dependencies

- Sprint 1: DecisionSituation trait, DecisionAction trait, ActionRegistry
- Sprint 2: SituationRegistry (for situation type matching)

## Next Sprint

After completing this sprint, proceed to [Sprint 4: Context Cache](sprint-04-context-cache.md) for running context caching with size limits.