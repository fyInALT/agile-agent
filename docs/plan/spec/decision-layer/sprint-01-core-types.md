# Sprint 1: Core Types (Trait-Based Architecture)

## Metadata

- Sprint ID: `decision-sprint-001`
- Title: `Core Types (Trait-Based)`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14
- Updated: 2026-04-14 (Architecture Evolution)

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 1 Tests: T1.1.T1-T1.6.T6 (36 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Architecture Evolution

This sprint adopts **Trait-based architecture** for extensibility:
- DecisionSituation trait (extensible situations)
- DecisionAction trait (extensible actions)
- Registry pattern (plugin-like registration)

See [Architecture Evolution Proposal](architecture-evolution.md) for rationale.

## Sprint Goal

Establish extensible core types for decision layer using Trait + Registry pattern, enabling future extension without core code modification.

## Stories

### Story 1.1: DecisionSituation Trait and Registry

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Define DecisionSituation trait and SituationRegistry for extensible decision situations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `SituationType` identifier struct | Todo | - |
| T1.1.2 | Define `DecisionSituation` trait | Todo | - |
| T1.1.3 | Create `SituationRegistry` struct | Todo | - |
| T1.1.4 | Implement registry `register()` method | Todo | - |
| T1.1.5 | Implement registry `get()` method | Todo | - |
| T1.1.6 | Implement registry `build_from_event()` method | Todo | - |
| T1.1.7 | Write unit tests for trait and registry | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T1.1.T1 | SituationType creation and comparison |
| T1.1.T2 | Trait methods return correct values |
| T1.1.T3 | Registry stores and retrieves situations |
| T1.1.T4 | Registry builds situation from event |
| T1.1.T5 | Registry handles unknown type gracefully |

#### Acceptance Criteria

- Trait defines all required methods
- Registry supports registration and retrieval
- Unknown types handled gracefully
- Thread-safe registry operations

#### Technical Notes

```rust
/// Situation type identifier - extensible string-based
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SituationType {
    /// Type name (e.g., "waiting_for_choice", "claude_finished")
    name: String,
    
    /// Optional subtype for provider-specific variants
    subtype: Option<String>,
}

impl SituationType {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), subtype: None }
    }
    
    pub fn with_subtype(name: impl Into<String>, subtype: impl Into<String>) -> Self {
        Self { name: name.into(), subtype: Some(subtype.into()) }
    }
}

/// Decision situation trait - extensible
pub trait DecisionSituation: Send + Sync + 'static {
    /// Situation type identifier
    fn situation_type(&self) -> SituationType;
    
    /// Whether requires human escalation
    fn requires_human(&self) -> bool;
    
    /// Human escalation urgency (if requires_human)
    fn human_urgency(&self) -> UrgencyLevel;
    
    /// Serialize for prompt
    fn to_prompt_text(&self) -> String;
    
    /// Available actions for this situation
    fn available_actions(&self) -> Vec<ActionType>;
    
    /// Clone into boxed
    fn clone_boxed(&self) -> Box<dyn DecisionSituation>;
}

/// Urgency level for human intervention
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UrgencyLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Action type identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionType {
    name: String,
}

impl ActionType {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Situation registry - extensible plugin system
pub struct SituationRegistry {
    /// Registered situation builders
    builders: HashMap<SituationType, SituationBuilder>,
    
    /// Default situations (fallback)
    defaults: HashMap<SituationType, Box<dyn DecisionSituation>>,
}

/// Situation builder function type
type SituationBuilder = Box<dyn Fn(&ProviderEvent) -> Option<Box<dyn DecisionSituation>> + Send + Sync>;

impl SituationRegistry {
    pub fn new() -> Self {
        Self {
            builders: HashMap::new(),
            defaults: HashMap::new(),
        }
    }
    
    /// Register a situation builder
    pub fn register_builder(
        &mut self,
        type: SituationType,
        builder: impl Fn(&ProviderEvent) -> Option<Box<dyn DecisionSituation>> + Send + Sync + 'static,
    ) {
        self.builders.insert(type, Box::new(builder));
    }
    
    /// Register a default situation (used when builder fails)
    pub fn register_default(&mut self, situation: Box<dyn DecisionSituation>) {
        self.defaults.insert(situation.situation_type(), situation);
    }
    
    /// Get situation by type
    pub fn get(&self, type: &SituationType) -> Option<&dyn DecisionSituation> {
        self.defaults.get(type).map(|b| b.as_ref())
    }
    
    /// Build situation from provider event
    pub fn build_from_event(&self, type: SituationType, event: &ProviderEvent) 
        -> Option<Box<dyn DecisionSituation>> {
        // Try builder first
        if let Some(builder) = self.builders.get(&type) {
            if let Some(situation) = builder(event) {
                return Some(situation);
            }
        }
        
        // Fall back to default
        self.defaults.get(&type).map(|d| d.clone_boxed())
    }
    
    /// Check if type is registered
    pub fn is_registered(&self, type: &SituationType) -> bool {
        self.builders.contains_key(type) || self.defaults.contains_key(type)
    }
}

/// Built-in situation types (pre-registered)
pub mod builtin_situations {
    use super::*;
    
    /// Situation: Waiting for choice
    pub const WAITING_FOR_CHOICE: SituationType = SituationType::new("waiting_for_choice");
    
    /// Situation: Claims completion
    pub const CLAIMS_COMPLETION: SituationType = SituationType::new("claims_completion");
    
    /// Situation: Partial completion
    pub const PARTIAL_COMPLETION: SituationType = SituationType::new("partial_completion");
    
    /// Situation: Error
    pub const ERROR: SituationType = SituationType::new("error");
    
    /// Provider-specific: Claude finished
    pub const CLAUDE_FINISHED: SituationType = SituationType::with_subtype("finished", "claude");
    
    /// Provider-specific: Codex approval request
    pub const CODEX_APPROVAL: SituationType = SituationType::with_subtype("waiting_for_choice", "codex");
    
    /// Provider-specific: ACP permission asked
    pub const ACP_PERMISSION: SituationType = SituationType::with_subtype("waiting_for_choice", "acp");
}

/// Initialize registry with built-in situations
impl SituationRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        
        // Register built-in situations
        registry.register_default(Box::new(WaitingForChoiceSituation::default()));
        registry.register_default(Box::new(ClaimsCompletionSituation::default()));
        registry.register_default(Box::new(PartialCompletionSituation::default()));
        registry.register_default(Box::new(ErrorSituation::default()));
        
        registry
    }
}
```

---

### Story 1.2: Built-in Situation Implementations

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement four built-in DecisionSituation types.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Implement `WaitingForChoiceSituation` | Todo | - |
| T1.2.2 | Implement `ClaimsCompletionSituation` | Todo | - |
| T1.2.3 | Implement `PartialCompletionSituation` | Todo | - |
| T1.2.4 | Implement `ErrorSituation` | Todo | - |
| T1.2.5 | Implement `ChoiceOption` struct | Todo | - |
| T1.2.6 | Implement `CompletionProgress` struct | Todo | - |
| T1.2.7 | Implement `ErrorType` struct | Todo | - |
| T1.2.8 | Write unit tests for each situation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T1.2.T1 | WaitingForChoice situation_type matches |
| T1.2.T2 | WaitingForChoice options stored correctly |
| T1.2.T3 | ClaimsCompletion reflection_rounds tracked |
| T1.2.T4 | PartialCompletion progress items correct |
| T1.2.T5 | ErrorSituation error_type stored |
| T1.2.T6 | to_prompt_text() output format correct |

#### Acceptance Criteria

- All four situations implement trait correctly
- Situation-specific data stored
- Prompt text format standardized

#### Technical Notes

```rust
/// Situation 1: Waiting for choice
pub struct WaitingForChoiceSituation {
    /// Available options
    options: Vec<ChoiceOption>,
    
    /// Permission type (for security check)
    permission_type: Option<String>,
    
    /// Whether this is a critical choice
    critical: bool,
}

impl DecisionSituation for WaitingForChoiceSituation {
    fn situation_type(&self) -> SituationType {
        builtin_situations::WAITING_FOR_CHOICE.clone()
    }
    
    fn requires_human(&self) -> bool {
        self.critical
    }
    
    fn human_urgency(&self) -> UrgencyLevel {
        if self.critical { UrgencyLevel::High } else { UrgencyLevel::Low }
    }
    
    fn to_prompt_text(&self) -> String {
        let options_text = self.options.iter()
            .map(|o| format!("[{}] {}", o.id, o.label))
            .collect::<Vec<_>>()
            .join("\n");
        
        format!(
            "Waiting for choice:\n\
            Options:\n{}\n\
            Permission type: {}",
            options_text,
            self.permission_type.as_deref().unwrap_or("unknown")
        )
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("select_option"),
            ActionType::new("select_first"),
            ActionType::new("reject_all"),
            ActionType::new("custom_instruction"),
        ]
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Choice option
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// Situation 2: Claims completion
pub struct ClaimsCompletionSituation {
    /// Completion summary
    summary: String,
    
    /// Reflection rounds so far
    reflection_rounds: u8,
    
    /// Maximum reflection rounds
    max_reflection_rounds: u8,
    
    /// Confidence level (0.0-1.0)
    confidence: f64,
}

impl DecisionSituation for ClaimsCompletionSituation {
    fn situation_type(&self) -> SituationType {
        builtin_situations::CLAIMS_COMPLETION.clone()
    }
    
    fn requires_human(&self) -> bool {
        // Requires human if reflection exhausted and low confidence
        self.reflection_rounds >= self.max_reflection_rounds && self.confidence < 0.7
    }
    
    fn human_urgency(&self) -> UrgencyLevel {
        if self.confidence < 0.5 { UrgencyLevel::Critical }
        else { UrgencyLevel::Medium }
    }
    
    fn to_prompt_text(&self) -> String {
        format!(
            "Claims completion (round {}):\n\
            Summary: {}\n\
            Confidence: {:.0}%",
            self.reflection_rounds,
            self.summary,
            self.confidence * 100
        )
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        if self.reflection_rounds < self.max_reflection_rounds {
            vec![
                ActionType::new("reflect"),
                ActionType::new("confirm_completion"),
            ]
        } else {
            vec![
                ActionType::new("confirm_completion"),
                ActionType::new("request_human"),
            ]
        }
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Completion progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionProgress {
    pub completed_items: Vec<String>,
    pub remaining_items: Vec<String>,
    pub estimated_remaining_minutes: Option<u64>,
}

/// Situation 3: Partial completion
pub struct PartialCompletionSituation {
    progress: CompletionProgress,
    blocker: Option<String>,
}

impl DecisionSituation for PartialCompletionSituation {
    fn situation_type(&self) -> SituationType {
        builtin_situations::PARTIAL_COMPLETION.clone()
    }
    
    fn requires_human(&self) -> bool {
        self.blocker.is_some()
    }
    
    fn human_urgency(&self) -> UrgencyLevel {
        UrgencyLevel::Medium
    }
    
    fn to_prompt_text(&self) -> String {
        format!(
            "Partial completion:\n\
            Completed: {}\n\
            Remaining: {}\n\
            Blocker: {}",
            self.progress.completed_items.join(", "),
            self.progress.remaining_items.join(", "),
            self.blocker.as_deref().unwrap_or("none")
        )
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        vec![
            ActionType::new("continue"),
            ActionType::new("skip_remaining"),
            ActionType::new("request_context"),
        ]
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}

/// Error type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    pub error_type: String,
    pub message: String,
    pub recoverable: bool,
    pub retry_count: u8,
}

/// Situation 4: Error
pub struct ErrorSituation {
    error: ErrorInfo,
}

impl DecisionSituation for ErrorSituation {
    fn situation_type(&self) -> SituationType {
        builtin_situations::ERROR.clone()
    }
    
    fn requires_human(&self) -> bool {
        !self.error.recoverable || self.error.retry_count >= 3
    }
    
    fn human_urgency(&self) -> UrgencyLevel {
        if self.error.recoverable { UrgencyLevel::Medium }
        else { UrgencyLevel::High }
    }
    
    fn to_prompt_text(&self) -> String {
        format!(
            "Error (retry {}):\n\
            Type: {}\n\
            Message: {}\n\
            Recoverable: {}",
            self.error.retry_count,
            self.error.error_type,
            self.error.message,
            self.error.recoverable
        )
    }
    
    fn available_actions(&self) -> Vec<ActionType> {
        if self.error.recoverable && self.error.retry_count < 3 {
            vec![
                ActionType::new("retry"),
                ActionType::new("retry_adjusted"),
                ActionType::new("restart"),
            ]
        } else {
            vec![
                ActionType::new("request_human"),
                ActionType::new("abort"),
            ]
        }
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionSituation> {
        Box::new(self.clone())
    }
}
```

---

### Story 1.3: DecisionAction Trait and Registry

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define DecisionAction trait and ActionRegistry for extensible decision actions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Define `DecisionAction` trait | Todo | - |
| T1.3.2 | Define `ActionResult` enum | Todo | - |
| T1.3.3 | Create `ActionRegistry` struct | Todo | - |
| T1.3.4 | Implement registry `register()` method | Todo | - |
| T1.3.5 | Implement registry `parse()` method | Todo | - |
| T1.3.6 | Write unit tests for action trait | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T1.3.T1 | ActionType creation matches |
| T1.3.T2 | Trait methods return correct values |
| T1.3.T3 | Registry stores and retrieves actions |
| T1.3.T4 | Registry parses action from LLM output |
| T1.3.T5 | ActionResult variants work correctly |

#### Acceptance Criteria

- Action trait defines execution interface
- Registry supports parsing from LLM output
- ActionResult tracks execution outcome

#### Technical Notes

```rust
/// Decision action trait - extensible
pub trait DecisionAction: Send + Sync + 'static {
    /// Action type identifier
    fn action_type(&self) -> ActionType;
    
    /// Execute action on main agent
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult>;
    
    /// Serialize for prompt (tells LLM how to output)
    fn to_prompt_format(&self) -> String;
    
    /// Clone into boxed
    fn clone_boxed(&self) -> Box<dyn DecisionAction>;
}

/// Action execution result
#[derive(Debug, Clone)]
pub enum ActionResult {
    /// Action completed successfully
    Success,
    
    /// Action needs follow-up
    NeedsFollowUp {
        next_action: Option<ActionType>,
    },
    
    /// Action delegated to other agent
    Delegated {
        target_agent: AgentId,
    },
    
    /// Action failed
    Failed {
        reason: String,
    },
    
    /// Action requires human confirmation
    NeedsHumanConfirmation {
        message: String,
    },
}

/// Action registry - extensible
pub struct ActionRegistry {
    /// Registered actions by type
    actions: HashMap<ActionType, Box<dyn DecisionAction>>,
    
    /// Action parsers (parse from LLM output)
    parsers: HashMap<ActionType, ActionParser>,
}

/// Action parser function type
type ActionParser = Box<dyn Fn(&str) -> Option<Box<dyn DecisionAction>> + Send + Sync>;

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
            parsers: HashMap::new(),
        }
    }
    
    /// Register an action
    pub fn register(&mut self, action: Box<dyn DecisionAction>) {
        self.actions.insert(action.action_type(), action);
    }
    
    /// Register an action parser
    pub fn register_parser(
        &mut self,
        type: ActionType,
        parser: impl Fn(&str) -> Option<Box<dyn DecisionAction>> + Send + Sync + 'static,
    ) {
        self.parsers.insert(type, Box::new(parser));
    }
    
    /// Get action by type
    pub fn get(&self, type: &ActionType) -> Option<&dyn DecisionAction> {
        self.actions.get(type).map(|b| b.as_ref())
    }
    
    /// Parse action from LLM output
    pub fn parse(&self, type: ActionType, output: &str) -> Option<Box<dyn DecisionAction>> {
        self.parsers.get(&type).and_then(|parser| parser(output))
    }
    
    /// Get all registered action types
    pub fn registered_types(&self) -> Vec<ActionType> {
        self.actions.keys().cloned().collect()
    }
    
    /// Generate prompt format for all actions
    pub fn generate_prompt_formats(&self) -> String {
        self.actions.values()
            .map(|a| a.to_prompt_format())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Built-in action types
pub mod builtin_actions {
    use super::*;
    
    pub const SELECT_OPTION: ActionType = ActionType::new("select_option");
    pub const SELECT_FIRST: ActionType = ActionType::new("select_first");
    pub const REJECT_ALL: ActionType = ActionType::new("reject_all");
    pub const REFLECT: ActionType = ActionType::new("reflect");
    pub const CONFIRM_COMPLETION: ActionType = ActionType::new("confirm_completion");
    pub const CONTINUE: ActionType = ActionType::new("continue");
    pub const RETRY: ActionType = ActionType::new("retry");
    pub const REQUEST_HUMAN: ActionType = ActionType::new("request_human");
    pub const ABORT: ActionType = ActionType::new("abort");
    pub const CUSTOM_INSTRUCTION: ActionType = ActionType::new("custom_instruction");
}

/// Initialize registry with built-in actions
impl ActionRegistry {
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        
        registry.register(Box::new(SelectOptionAction::default()));
        registry.register(Box::new(ReflectAction::default()));
        registry.register(Box::new(ConfirmCompletionAction::default()));
        registry.register(Box::new(ContinueAction::default()));
        registry.register(Box::new(RetryAction::default()));
        registry.register(Box::new(CustomInstructionAction::default()));
        
        // Register parsers
        registry.register_parser(
            builtin_actions::SELECT_OPTION.clone(),
            SelectOptionAction::parse,
        );
        
        registry
    }
}
```

---

### Story 1.4: Built-in Action Implementations

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement built-in DecisionAction types.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Implement `SelectOptionAction` | Todo | - |
| T1.4.2 | Implement `ReflectAction` | Todo | - |
| T1.4.3 | Implement `ConfirmCompletionAction` | Todo | - |
| T1.4.4 | Implement `ContinueAction` | Todo | - |
| T1.4.5 | Implement `RetryAction` | Todo | - |
| T1.4.6 | Implement `CustomInstructionAction` | Todo | - |
| T1.4.7 | Write unit tests for each action | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T1.4.T1 | SelectOptionAction stores option_id |
| T1.4.T2 | SelectOptionAction execute sends to agent |
| T1.4.T3 | ReflectAction generates prompt |
| T1.4.T4 | ConfirmCompletionAction sets submit_pr |
| T1.4.T5 | RetryAction sets cooldown_ms |
| T1.4.T6 | CustomInstructionAction stores text |

#### Acceptance Criteria

- All actions implement trait correctly
- Action execution returns ActionResult
- Prompt format standardized for LLM

#### Technical Notes

```rust
/// Action: Select option
#[derive(Debug, Clone, Default)]
pub struct SelectOptionAction {
    option_id: String,
    reason: String,
}

impl SelectOptionAction {
    pub fn new(option_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self { option_id: option_id.into(), reason: reason.into() }
    }
    
    pub fn parse(output: &str) -> Option<Box<dyn DecisionAction>> {
        // Parse from format: "Selection: [A]\nReason: ..."
        let lines = output.lines().collect::<Vec<_>>();
        if lines.len() < 2 { return None; }
        
        let option_line = lines.iter().find(|l| l.starts_with("Selection:"))?;
        let reason_line = lines.iter().find(|l| l.starts_with("Reason:"))?;
        
        let option_id = option_line.split(':')
            .nth(1)
            .map(|s| s.trim().replace('[', "").replace(']', ""))
            .unwrap_or_default();
        
        let reason = reason_line.split(':')
            .nth(1)
            .map(|s| s.trim())
            .unwrap_or_default()
            .to_string();
        
        Some(Box::new(Self::new(option_id, reason)))
    }
}

impl DecisionAction for SelectOptionAction {
    fn action_type(&self) -> ActionType {
        builtin_actions::SELECT_OPTION.clone()
    }
    
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult> {
        agent.send_selection(self.option_id.clone())?;
        Ok(ActionResult::Success)
    }
    
    fn to_prompt_format(&self) -> String {
        "Selection: [Option ID]\nReason: [Brief explanation]".to_string()
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Reflect
#[derive(Debug, Clone, Default)]
pub struct ReflectAction {
    prompt: String,
}

impl ReflectAction {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self { prompt: prompt.into() }
    }
}

impl DecisionAction for ReflectAction {
    fn action_type(&self) -> ActionType {
        builtin_actions::REFLECT.clone()
    }
    
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult> {
        agent.send_prompt(self.prompt.clone())?;
        Ok(ActionResult::NeedsFollowUp {
            next_action: Some(builtin_actions::CONFIRM_COMPLETION.clone()),
        })
    }
    
    fn to_prompt_format(&self) -> String {
        "Reflect: [Reflection prompt for verification]".to_string()
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Confirm completion
#[derive(Debug, Clone, Default)]
pub struct ConfirmCompletionAction {
    submit_pr: bool,
    next_task: Option<TaskId>,
}

impl DecisionAction for ConfirmCompletionAction {
    fn action_type(&self) -> ActionType {
        builtin_actions::CONFIRM_COMPLETION.clone()
    }
    
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult> {
        if self.submit_pr {
            agent.trigger_pr_submission()?;
        }
        Ok(ActionResult::Success)
    }
    
    fn to_prompt_format(&self) -> String {
        "Confirm: [yes/no]\nSubmitPR: [yes/no]".to_string()
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Continue
#[derive(Debug, Clone, Default)]
pub struct ContinueAction {
    prompt: String,
    focus_items: Vec<String>,
}

impl DecisionAction for ContinueAction {
    fn action_type(&self) -> ActionType {
        builtin_actions::CONTINUE.clone()
    }
    
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult> {
        agent.send_prompt(self.prompt.clone())?;
        Ok(ActionResult::Success)
    }
    
    fn to_prompt_format(&self) -> String {
        "Continue: [Instruction to continue]\nFocus: [Items to focus on]".to_string()
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Retry
#[derive(Debug, Clone, Default)]
pub struct RetryAction {
    prompt: String,
    cooldown_ms: u64,
    adjusted: bool,
}

impl DecisionAction for RetryAction {
    fn action_type(&self) -> ActionType {
        builtin_actions::RETRY.clone()
    }
    
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult> {
        // Apply cooldown
        std::thread::sleep(Duration::from_millis(self.cooldown_ms));
        agent.send_prompt(self.prompt.clone())?;
        Ok(ActionResult::NeedsFollowUp { next_action: None })
    }
    
    fn to_prompt_format(&self) -> String {
        "Retry: [Retry instruction]\nCooldownMs: [milliseconds]".to_string()
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}

/// Action: Custom instruction
#[derive(Debug, Clone, Default)]
pub struct CustomInstructionAction {
    instruction: String,
}

impl DecisionAction for CustomInstructionAction {
    fn action_type(&self) -> ActionType {
        builtin_actions::CUSTOM_INSTRUCTION.clone()
    }
    
    fn execute(&self, context: &DecisionContext, agent: &mut MainAgentConnection) 
        -> Result<ActionResult> {
        agent.send_prompt(self.instruction.clone())?;
        Ok(ActionResult::Success)
    }
    
    fn to_prompt_format(&self) -> String {
        "Custom: [Free-form instruction text]".to_string()
    }
    
    fn clone_boxed(&self) -> Box<dyn DecisionAction> {
        Box::new(self.clone())
    }
}
```

---

### Story 1.5: DecisionContext and RunningContextCache

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define DecisionContext with trait references and RunningContextCache.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.5.1 | Create `DecisionContext` struct | Todo | - |
| T1.5.2 | Add situation reference (Box<dyn DecisionSituation>) | Todo | - |
| T1.5.3 | Create `RunningContextCache` struct | Todo | - |
| T1.5.4 | Implement cache field structures | Todo | - |
| T1.5.5 | Create `ProjectRules` struct | Todo | - |
| T1.5.6 | Write unit tests for context | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T1.5.T1 | DecisionContext stores situation reference |
| T1.5.T2 | DecisionContext stores project rules |
| T1.5.T3 | RunningContextCache fields present |
| T1.5.T4 | ToolCallRecord timestamp set |

#### Acceptance Criteria

- DecisionContext holds trait references
- RunningContextCache defined with all fields
- ProjectRules loads from CLAUDE.md

#### Technical Notes

```rust
/// Decision context - input to decision engine
pub struct DecisionContext {
    /// Current situation (trait reference)
    trigger_situation: Box<dyn DecisionSituation>,
    
    /// Parent main agent ID
    main_agent_id: AgentId,
    
    /// Current task (if assigned)
    current_task: Option<TaskDefinition>,
    
    /// Current story (if assigned)
    current_story: Option<StoryDefinition>,
    
    /// Running context cache
    running_context: RunningContextCache,
    
    /// Project rules from CLAUDE.md
    project_rules: ProjectRules,
    
    /// Decision agent configuration
    config: DecisionAgentConfig,
    
    /// Decision history for this session
    decision_history: Vec<DecisionRecord>,
}

/// Running context cache - collects execution history
pub struct RunningContextCache {
    /// Tool call records (max N entries)
    tool_calls: VecDeque<ToolCallRecord>,
    
    /// File change records (max N entries)
    file_changes: VecDeque<FileChangeRecord>,
    
    /// Thinking summary (rolling)
    thinking_summary: Option<String>,
    
    /// Key outputs (max N entries)
    key_outputs: VecDeque<String>,
}

/// Tool call record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
}

/// File change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChangeRecord {
    pub path: String,
    pub change_type: ChangeType,
    pub diff_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
}

/// Project rules from CLAUDE.md
pub struct ProjectRules {
    /// Raw CLAUDE.md content
    content: String,
    
    /// Extracted rules (key-value)
    rules: HashMap<String, String>,
    
    /// Keywords for rule matching
    keywords: HashSet<String>,
    
    /// Rules that require human confirmation
    requires_human_rules: Vec<String>,
}

impl ProjectRules {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }
    
    pub fn parse(content: &str) -> Result<Self> {
        // Parse markdown, extract rules
        // ...
        Ok(Self { content: content.to_string(), rules: HashMap::new(), keywords: HashSet::new(), requires_human_rules: Vec::new() })
    }
    
    pub fn contains_keyword(&self, keyword: &str) -> bool {
        self.keywords.contains(keyword)
    }
    
    pub fn requires_human_for(&self, action_type: &ActionType) -> bool {
        self.requires_human_rules.iter().any(|r| r.contains(&action_type.name))
    }
}

/// Decision record - history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub decision_id: DecisionId,
    pub timestamp: DateTime<Utc>,
    pub situation_type: SituationType,
    pub actions: Vec<ActionType>,
    pub reasoning: String,
    pub confidence: f64,
    pub engine_type: DecisionEngineType,
}
```

---

### Story 1.6: BlockingReason Trait and AgentSlotStatus

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define BlockingReason trait for extensible blocking states.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.6.1 | Define `BlockingReason` trait | Todo | - |
| T1.6.2 | Create `BlockedState` struct | Todo | - |
| T1.6.3 | Update `AgentSlotStatus` with generic Blocked | Todo | - |
| T1.6.4 | Implement `HumanDecisionBlocking` | Todo | - |
| T1.6.5 | Write unit tests for blocking trait | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T1.6.T1 | BlockingReason trait methods work |
| T1.6.T2 | BlockedState stores reason |
| T1.6.T3 | AgentSlotStatus Blocked variant works |
| T1.6.T4 | HumanDecisionBlocking implements trait |
| T1.6.T5 | Auto resolve action returned |
| T1.6.T6 | Expiration time tracked |

#### Acceptance Criteria

- BlockingReason trait extensible
- BlockedState holds trait reference
- AgentSlotStatus uses generic Blocked

#### Technical Notes

```rust
/// Blocking reason trait - extensible
pub trait BlockingReason: Send + Sync + 'static {
    /// Reason type identifier
    fn reason_type(&self) -> &str;
    
    /// Urgency level
    fn urgency(&self) -> UrgencyLevel;
    
    /// Expiration time (if applicable)
    fn expires_at(&self) -> Option<DateTime<Utc>>;
    
    /// Whether can auto-resolve
    fn can_auto_resolve(&self) -> bool;
    
    /// Auto-resolve action (if can_auto_resolve)
    fn auto_resolve_action(&self) -> Option<AutoAction>;
    
    /// Blocking description for display
    fn description(&self) -> String;
    
    /// Clone into boxed
    fn clone_boxed(&self) -> Box<dyn BlockingReason>;
}

/// Auto-resolve action
#[derive(Debug, Clone)]
pub enum AutoAction {
    FollowRecommendation,
    SelectDefault,
    Cancel,
    MarkTaskFailed,
    ReleaseResource,
}

/// Blocked state - generic wrapper
pub struct BlockedState {
    /// Blocking reason (trait reference)
    reason: Box<dyn BlockingReason>,
    
    /// Blocked start time
    blocked_at: Instant,
    
    /// Blocking context
    context: BlockingContext,
}

impl BlockedState {
    pub fn new(reason: Box<dyn BlockingReason>) -> Self {
        Self {
            reason,
            blocked_at: Instant::now(),
            context: BlockingContext::default(),
        }
    }
    
    pub fn reason(&self) -> &dyn BlockingReason {
        self.reason.as_ref()
    }
    
    pub fn elapsed(&self) -> Duration {
        self.blocked_at.elapsed()
    }
    
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.reason.expires_at() {
            Utc::now() > expires
        } else {
            false
        }
    }
}

/// Blocking context
#[derive(Debug, Clone, Default)]
pub struct BlockingContext {
    pub task_id: Option<TaskId>,
    pub story_id: Option<StoryId>,
    pub additional_info: HashMap<String, String>,
}

/// Agent slot status - generic blocked
#[derive(Debug, Clone)]
pub enum AgentSlotStatus {
    /// Agent is running
    Running,
    
    /// Agent is blocked (generic)
    Blocked(BlockedState),
    
    /// Agent is idle (no task)
    Idle,
    
    /// Agent is stopped
    Stopped,
}

/// Human decision blocking - one implementation
pub struct HumanDecisionBlocking {
    decision_request_id: DecisionRequestId,
    situation: Box<dyn DecisionSituation>,
    options: Vec<ChoiceOption>,
    recommendation: Option<Box<dyn DecisionAction>>,
    expires_at: DateTime<Utc>,
}

impl BlockingReason for HumanDecisionBlocking {
    fn reason_type(&self) -> &str { "human_decision" }
    
    fn urgency(&self) -> UrgencyLevel {
        self.situation.human_urgency()
    }
    
    fn expires_at(&self) -> Option<DateTime<Utc>> {
        Some(self.expires_at)
    }
    
    fn can_auto_resolve(&self) -> bool {
        self.recommendation.is_some()
    }
    
    fn auto_resolve_action(&self) -> Option<AutoAction> {
        if self.recommendation.is_some() {
            Some(AutoAction::FollowRecommendation)
        } else {
            Some(AutoAction::SelectDefault)
        }
    }
    
    fn description(&self) -> String {
        format!("Waiting for human decision: {}", self.situation.situation_type().name)
    }
    
    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone())
    }
}

/// Resource blocking - another implementation (example)
pub struct ResourceBlocking {
    resource_type: String,
    resource_id: String,
    wait_reason: String,
}

impl BlockingReason for ResourceBlocking {
    fn reason_type(&self) -> &str { "resource_waiting" }
    
    fn urgency(&self) -> UrgencyLevel { UrgencyLevel::Low }
    
    fn expires_at(&self) -> Option<DateTime<Utc>> { None }
    
    fn can_auto_resolve(&self) -> bool { true }
    
    fn auto_resolve_action(&self) -> Option<AutoAction> { None }
    
    fn description(&self) -> String {
        format!("Waiting for {}: {}", self.resource_type, self.resource_id)
    }
    
    fn clone_boxed(&self) -> Box<dyn BlockingReason> {
        Box::new(self.clone())
    }
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Trait overhead | Medium | Low | Benchmark, optimize hot paths |
| Registry initialization complexity | Low | Medium | Clear registration order documentation |
| Dynamic dispatch cost | Low | Low | Situation type is stable after init |

## Sprint Deliverables

- `decision/src/types.rs` - SituationType, ActionType, core identifiers
- `decision/src/situation.rs` - DecisionSituation trait
- `decision/src/situation_registry.rs` - SituationRegistry
- `decision/src/builtin_situations.rs` - Four built-in situations
- `decision/src/action.rs` - DecisionAction trait
- `decision/src/action_registry.rs` - ActionRegistry
- `decision/src/builtin_actions.rs` - Built-in actions
- `decision/src/context.rs` - DecisionContext, RunningContextCache
- `decision/src/blocking.rs` - BlockingReason trait, BlockedState

## Dependencies

- None (first sprint)

## Architecture Benefits

After this sprint:
- Adding new situation: implement trait + register (no enum modification)
- Adding new action: implement trait + register (no enum modification)
- Adding new blocking reason: implement trait (no AgentSlotStatus modification)

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Output Classifier](sprint-02-output-classifier.md) for provider-specific classifier implementations using SituationRegistry.