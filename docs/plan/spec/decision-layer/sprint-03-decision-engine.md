# Sprint 3: Decision Engine

## Metadata

- Sprint ID: `decision-sprint-003`
- Title: `Decision Engine`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 3 Tests: T3.1.T1-T3.5.T6 (26 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Implement decision engines (LLM, CLI, RuleBased, Mock) that produce DecisionOutput from DecisionContext. Each engine has its own session for multi-turn decisions.

## Stories

### Story 3.1: DecisionEngine Trait

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define the DecisionEngine trait that all engines implement.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Create `DecisionEngine` trait | Todo | - |
| T3.1.2 | Define `decide()` method signature | Todo | - |
| T3.1.3 | Define `build_prompt()` helper | Todo | - |
| T3.1.4 | Define `persist_session()` method | Todo | - |
| T3.1.5 | Define `restore_session()` method | Todo | - |
| T3.1.6 | Write trait documentation | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.1.T1 | engine_type() returns correct type |
| T3.1.T2 | decide() takes Context, returns Output |
| T3.1.T3 | build_prompt() generates valid prompt |
| T3.1.T4 | session_handle() returns Option |
| T3.1.T5 | is_healthy() returns bool |
| T3.1.T6 | reset() clears state |

#### Acceptance Criteria

- Trait defines all required methods
- Trait supports async decision making
- Session persistence methods defined

#### Technical Notes

```rust
/// Decision engine trait
pub trait DecisionEngine: Send + Sync {
    /// Engine type
    fn engine_type(&self) -> DecisionEngineType;
    
    /// Make a decision based on context
    fn decide(&mut self, context: DecisionContext) -> Result<DecisionOutput>;
    
    /// Build decision prompt from context
    fn build_prompt(&self, context: &DecisionContext) -> String;
    
    /// Persist session for multi-turn
    fn persist_session(&self, path: &Path) -> Result<()>;
    
    /// Restore session from persistence
    fn restore_session(&mut self, path: &Path) -> Result<()>;
    
    /// Get current session handle
    fn session_handle(&self) -> Option<&SessionHandle>;
    
    /// Check engine health
    fn is_healthy(&self) -> bool;
    
    /// Reset engine state
    fn reset(&mut self) -> Result<()>;
}
```

---

### Story 3.2a: Decision Prompt Templates

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement prompt templates for four decision situations.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2a.1 | Create `DecisionPromptTemplates` struct | Todo | - |
| T3.2a.2 | Implement `choice_prompt()` for WaitingForChoice | Todo | - |
| T3.2a.3 | Implement `reflection_prompt()` for ClaimsCompletion | Todo | - |
| T3.2a.4 | Implement `verification_prompt()` for final DoD check | Todo | - |
| T3.2a.5 | Implement `continue_prompt()` for PartialCompletion | Todo | - |
| T3.2a.6 | Implement `retry_prompt()` for Error situations | Todo | - |
| T3.2a.7 | Write unit tests for prompt templates | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.2.T1 | Choice prompt contains all required sections |
| T3.2.T2 | Reflection prompt contains correct round number |
| T3.2.T3 | Verification prompt for DoD check |
| T3.2.T4 | Continue prompt with focus_items |
| T3.2.T5 | Retry prompt for each ErrorType |

#### Acceptance Criteria

- All four situation prompts implemented
- Prompts contain required sections (project rules, story, context)
- Template format documented

---

### Story 3.2b: LLM Decision Engine API Integration

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement LLM API call integration with response parsing.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2b.1 | Create `LLMDecisionEngine` struct | Todo | - |
| T3.2b.2 | Implement `decide()` with LLM API call | Todo | - |
| T3.2b.3 | Implement response parsing to DecisionOutput | Todo | - |
| T3.2b.4 | Implement timeout handling | Todo | - |
| T3.2b.5 | Implement session persistence | Todo | - |
| T3.2b.6 | Implement health check | Todo | - |
| T3.2b.7 | Write unit tests with mock LLM responses | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.2.T6 | Parse LLM response to Choice output |
| T3.2.T7 | Parse LLM response to ReflectionRequest |
| T3.2.T8 | Parse LLM response to CompletionConfirm |
| T3.2.T9 | Timeout returns error or fallback |
| T3.2.T10 | Session persists and restores |
| T3.2.T11 | Mock LLM responses for testing |

#### Acceptance Criteria

- LLM calls produce valid DecisionOutput
- Session persists correctly
- Timeout handled gracefully

#### Technical Notes

```rust
pub struct LLMDecisionEngine {
    /// Provider to use for LLM calls
    provider: ProviderKind,
    
    /// Current session handle
    session: Option<SessionHandle>,
    
    /// Engine configuration
    config: DecisionAgentConfig,
    
    /// Decision prompt templates
    prompts: DecisionPromptTemplates,
}

impl DecisionEngine for LLMDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::LLM { provider: self.provider }
    }
    
    fn decide(&mut self, context: DecisionContext) -> Result<DecisionOutput> {
        // 1. Build prompt based on trigger status
        let prompt = self.build_prompt(&context);
        
        // 2. Call LLM with timeout
        let response = self.call_llm_with_timeout(prompt)?;
        
        // 3. Parse response to DecisionOutput
        let output = self.parse_response(response, &context.trigger_status)?;
        
        // 4. Persist session
        self.persist_session()?;
        
        Ok(output)
    }
    
    fn build_prompt(&self, context: &DecisionContext) -> String {
        match &context.trigger_status {
            ProviderStatus::WaitingForChoice { options } => {
                self.prompts.choice_prompt(context, options)
            }
            ProviderStatus::ClaimsCompletion { summary, reflection_rounds } => {
                if *reflection_rounds < self.config.max_reflection_rounds {
                    self.prompts.reflection_prompt(context, summary, *reflection_rounds)
                } else {
                    self.prompts.verification_prompt(context, summary)
                }
            }
            ProviderStatus::PartialCompletion { progress } => {
                self.prompts.continue_prompt(context, progress)
            }
            ProviderStatus::Error { error_type } => {
                self.prompts.retry_prompt(context, error_type)
            }
        }
    }
    
    fn call_llm_with_timeout(&self, prompt: String) -> Result<String> {
        // Use timeout from config
        let timeout = Duration::from_millis(self.config.decision_timeout_ms);
        
        // Call provider (Claude/Codex/etc.)
        // ...
    }
}

/// Decision prompt templates
pub struct DecisionPromptTemplates;

impl DecisionPromptTemplates {
    pub fn choice_prompt(&self, context: &DecisionContext, options: &[ChoiceOption]) -> String {
        format!(
            "You are a decision helper for a development agent.\n\
            \n\
            ## Project Rules\n\
            {}\n\
            \n\
            ## Current Story\n\
            {}\n\
            \n\
            ## Running Context Summary\n\
            {}\n\
            \n\
            ## Available Options\n\
            {}\n\
            \n\
            ## Task\n\
            Select the most appropriate option based on project rules and story requirements.\n\
            Output format:\n\
            - Selection: [Option ID]\n\
            - Reason: [Brief explanation]",
            context.project_rules.summary(),
            context.current_story.map(|s| s.definition()).unwrap_or_default(),
            context.running_context.summary(),
            options.iter().map(|o| format!("[{}] {}", o.id, o.label)).collect::<Vec<_>>().join("\n")
        )
    }
    
    pub fn reflection_prompt(&self, context: &DecisionContext, summary: &str, round: u8) -> String {
        format!(
            "The development agent claims to have completed the task.\n\
            Reflection round: {}\n\
            \n\
            ## Claimed Completion\n\
            {}\n\
            \n\
            ## Running Context\n\
            {}\n\
            \n\
            ## Story Definition\n\
            {}\n\
            \n\
            ## Task\n\
            Please reflect:\n\
            1. Are all code files correctly modified?\n\
            2. Are there missing edge cases?\n\
            3. Does it comply with project rules?\n\
            4. Are tests covering new functionality?\n\
            \n\
            If there are gaps, state them explicitly. If truly complete, say 'CONFIRMED COMPLETE'.",
            round,
            summary,
            context.running_context.summary(),
            context.current_story.map(|s| s.definition()).unwrap_or_default()
        )
    }
    
    pub fn verification_prompt(&self, context: &DecisionContext, summary: &str) -> String {
        format!(
            "Final verification after reflection rounds.\n\
            \n\
            ## Completion Content\n\
            {}\n\
            \n\
            ## Story Acceptance Criteria\n\
            {}\n\
            \n\
            ## Running Records\n\
            {}\n\
            \n\
            ## Task\n\
            Verify if Story acceptance criteria are satisfied.\n\
            Output format:\n\
            - Complete: [yes/no]\n\
            - Missing: [list if any]",
            summary,
            context.current_story.map(|s| s.acceptance_criteria()).unwrap_or_default(),
            context.running_context.summary()
        )
    }
    
    pub fn continue_prompt(&self, context: &DecisionContext, progress: &CompletionProgress) -> String {
        format!(
            "The development agent has partially completed.\n\
            \n\
            ## Completed Items\n\
            {}\n\
            \n\
            ## Remaining Items\n\
            {}\n\
            \n\
            ## Task\n\
            Provide instruction to continue:\n\
            'Please continue completing story-xxx remaining parts:\n\
            - Feature A is not yet implemented\n\
            - Feature B is only half done'",
            progress.completed_items.join("\n"),
            progress.remaining_items.join("\n")
        )
    }
    
    pub fn retry_prompt(&self, context: &DecisionContext, error_type: &ErrorType) -> String {
        match error_type {
            ErrorType::Failure { message } => format!(
                "The agent encountered an error: {}\n\
                \n\
                ## Task\n\
                Provide retry instruction:\n\
                'Please retry the task. The previous failure was: {}'",
                message, message
            ),
            ErrorType::Gibberish => "The agent produced nonsensical output.\n\n## Task\nProvide restart instruction: 'Please restart the task from the beginning.'".to_string(),
            ErrorType::Repetition { .. } => "The agent repeated previous output.\n\n## Task\nProvide continuation instruction: 'You seem to have repeated your output. Please continue with new content.'".to_string(),
        }
    }
}
```

---

### Story 3.3a: CLI Decision Engine Session Management

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement independent session management for CLI decision engine.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3a.1 | Create `CLIDecisionEngine` struct | Todo | - |
| T3.3a.2 | Implement `parent_agent_id` reference | Todo | - |
| T3.3a.3 | Implement session creation (independent) | Todo | - |
| T3.3a.4 | Implement session persistence/restore | Todo | - |
| T3.3a.5 | Implement session isolation from main agent | Todo | - |
| T3.3a.6 | Write unit tests for session management | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.3.T1 | CLI session different from main agent |
| T3.3.T2 | parent_agent_id stored correctly |
| T3.3.T7 | New session created for decision |
| T3.3.T8 | Existing session resumed |

#### Acceptance Criteria

- CLI engine uses independent session (not recursive)
- Parent agent ID tracked
- Session isolation verified

---

### Story 3.3b: CLI Decision Engine Provider Integration

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement provider thread spawning and output collection.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3b.1 | Implement provider thread spawning | Todo | - |
| T3.3b.2 | Implement event channel for CLI output | Todo | - |
| T3.3b.3 | Implement `send_prompt()` to provider | Todo | - |
| T3.3b.4 | Implement `collect_output()` until blocked | Todo | - |
| T3.3b.5 | Implement `parse_cli_output()` to DecisionOutput | Todo | - |
| T3.3b.6 | Implement `decide()` via CLI provider | Todo | - |
| T3.3b.7 | Write unit tests for CLI engine | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.3.T3 | Provider thread spawns correctly |
| T3.3.T4 | Events received via channel |
| T3.3.T5 | Output collected until blocked |
| T3.3.T6 | Provider output parsed to DecisionOutput |

#### Acceptance Criteria

- Provider thread spawns correctly
- Events received via channel
- Output parsed to DecisionOutput

#### Technical Notes

```rust
/// CLI decision engine with independent session
pub struct CLIDecisionEngine {
    /// Provider type for decision (can differ from main agent)
    provider: ProviderKind,
    
    /// Independent session handle
    session: Option<SessionHandle>,
    
    /// Decision agent ID
    agent_id: AgentId,
    
    /// Parent main agent ID
    parent_agent_id: AgentId,
    
    /// Configuration
    config: DecisionAgentConfig,
    
    /// Event channel from provider
    event_rx: Option<mpsc::Receiver<ProviderEvent>>,
    
    /// Provider thread handle
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl DecisionEngine for CLIDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::CLI { provider: self.provider }
    }
    
    fn decide(&mut self, context: DecisionContext) -> Result<DecisionOutput> {
        // 1. Build prompt
        let prompt = self.build_prompt(&context);
        
        // 2. Spawn or resume provider session
        if self.session.is_none() {
            self.spawn_provider_session()?;
        }
        
        // 3. Send prompt to provider
        self.send_prompt(prompt)?;
        
        // 4. Collect output until blocked
        let output = self.collect_output()?;
        
        // 5. Parse output to DecisionOutput
        self.parse_cli_output(output, &context.trigger_status)
    }
    
    fn session_handle(&self) -> Option<&SessionHandle> {
        self.session.as_ref()
    }
}

impl CLIDecisionEngine {
    fn spawn_provider_session(&mut self) -> Result<()> {
        // Create new independent session
        // NOT recursive - separate from main agent
        let cwd = self.get_parent_cwd()?;
        
        // Spawn provider thread
        let (event_tx, event_rx) = mpsc::channel();
        let thread = provider::start_provider_thread(
            self.provider,
            self.agent_id.clone(),
            String::new(), // Initial empty prompt
            cwd,
            None, // New session
            event_tx,
        )?;
        
        self.event_rx = Some(event_rx);
        self.thread_handle = Some(thread);
        
        // Wait for session handle
        self.collect_session_handle()?;
        
        Ok(())
    }
    
    fn send_prompt(&self, prompt: String) -> Result<()> {
        // Send prompt via stdin
        // ...
    }
    
    fn collect_output(&mut self) -> Result<String> {
        // Collect events until Finished or WaitingForChoice
        // ...
    }
    
    fn get_parent_cwd(&self) -> Result<PathBuf> {
        // Get working directory from parent agent
        // ...
    }
}

/// Session independence explained:
/// 
/// Main Agent:
///   - provider: claude
///   - session: sess-main-xxx
///   - role: development execution
/// 
/// Decision Agent (CLI engine):
///   - provider: claude (can be different)
///   - session: sess-decision-yyy (INDEPENDENT)
///   - role: decision judgment
/// 
/// Why independent session:
/// 1. Context isolation - decision history doesn't pollute main transcript
/// 2. Role separation - developer vs decision-maker
/// 3. State management - reflection rounds, retry counts independent
/// 4. Debuggability - decision history tracked separately
/// 5. Parallel decisions - multiple agents can decide simultaneously
```

---

### Story 3.4: Rule-Based Decision Engine

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement simple rule-based decision engine for quick decisions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Create `RuleBasedDecisionEngine` struct | Todo | - |
| T3.4.2 | Define decision rules format | Todo | - |
| T3.4.3 | Implement rule matching logic | Todo | - |
| T3.4.4 | Implement default rules | Todo | - |
| T3.4.5 | Implement custom rules loading | Todo | - |
| T3.4.6 | Write unit tests for rule engine | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.4.T1 | Rules match WaitingForChoice status |
| T3.4.T2 | Rules match ClaimsCompletion status |
| T3.4.T3 | Rules match Error status |
| T3.4.T4 | Project keyword matching works |
| T3.4.T5 | Default rules cover common scenarios |
| T3.4.T6 | Custom rules load from file |
| T3.4.T7 | No matching rule returns continue |

#### Acceptance Criteria

- Rules match against context
- Default rules cover common scenarios
- Custom rules loadable from config

#### Technical Notes

```rust
/// Rule-based decision engine (fast, low-cost)
pub struct RuleBasedDecisionEngine {
    /// Decision rules
    rules: Vec<DecisionRule>,
    
    /// Default rules
    default_rules: Vec<DecisionRule>,
}

pub struct DecisionRule {
    /// Rule name
    name: String,
    
    /// Trigger conditions
    conditions: RuleConditions,
    
    /// Decision output when matched
    output: DecisionOutput,
}

pub struct RuleConditions {
    /// Provider status to match
    status: Option<ProviderStatusPattern>,
    
    /// Project rule keywords
    project_rule_keywords: Vec<String>,
    
    /// Story type pattern
    story_type: Option<String>,
}

impl DecisionEngine for RuleBasedDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::RuleBased
    }
    
    fn decide(&mut self, context: DecisionContext) -> Result<DecisionOutput> {
        // Find matching rule
        for rule in &self.rules {
            if rule.matches(&context) {
                return Ok(rule.output.clone());
            }
        }
        
        // Fallback to default rules
        for rule in &self.default_rules {
            if rule.matches(&context) {
                return Ok(rule.output.clone());
            }
        }
        
        // No rule matched - return continue instruction
        Ok(DecisionOutput::ContinueInstruction {
            prompt: "Continue with current task.".to_string(),
            focus_items: vec![],
        })
    }
}

impl DecisionRule {
    fn matches(&self, context: &DecisionContext) -> bool {
        // Check status pattern
        if let Some(pattern) = &self.conditions.status {
            if !pattern.matches(&context.trigger_status) {
                return false;
            }
        }
        
        // Check project rule keywords
        for keyword in &self.conditions.project_rule_keywords {
            if !context.project_rules.contains_keyword(keyword) {
                return false;
            }
        }
        
        true
    }
}

/// Default rules
impl RuleBasedDecisionEngine {
    pub fn default_rules() -> Vec<DecisionRule> {
        vec![
            // Rule: Always approve first read file
            DecisionRule {
                name: "approve-read".to_string(),
                conditions: RuleConditions {
                    status: Some(ProviderStatusPattern::WaitingForChoice { 
                        permission_type: "read".to_string() 
                    }),
                    project_rule_keywords: vec![],
                    story_type: None,
                },
                output: DecisionOutput::Choice {
                    selected: "once".to_string(),
                    reason: "Reading files is safe".to_string(),
                },
            },
            
            // Rule: Reflection on first completion claim
            DecisionRule {
                name: "reflect-first".to_string(),
                conditions: RuleConditions {
                    status: Some(ProviderStatusPattern::ClaimsCompletion { 
                        reflection_rounds: 0 
                    }),
                    project_rule_keywords: vec![],
                    story_type: None,
                },
                output: DecisionOutput::ReflectionRequest {
                    prompt: "Please reflect on whether you truly completed the task.".to_string(),
                },
            },
            
            // Rule: Retry on error
            DecisionRule {
                name: "retry-error".to_string(),
                conditions: RuleConditions {
                    status: Some(ProviderStatusPattern::Error),
                    project_rule_keywords: vec![],
                    story_type: None,
                },
                output: DecisionOutput::RetryInstruction {
                    prompt: "Please retry the task.".to_string(),
                    cooldown_ms: 10000,
                },
            },
        ]
    }
}
```

**Rule Configuration File**:

```toml
[[decision_layer.rules]]
name = "approve-write-safe"
conditions.status = "waiting_for_choice"
conditions.permission_type = "write"
conditions.project_rule_keywords = ["safe_write"]
output.type = "choice"
output.selected = "once"
output.reason = "Safe write operation"

[[decision_layer.rules]]
name = "deny-dangerous"
conditions.status = "waiting_for_choice"
conditions.permission_type = "exec"
conditions.command_contains = "rm -rf"
output.type = "choice"
output.selected = "reject"
output.reason = "Dangerous command"
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
| T3.5.2 | Implement configurable mock outputs | Todo | - |
| T3.5.3 | Implement decision recording | Todo | - |
| T3.5.4 | Write unit tests for mock engine | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T3.5.T1 | Returns first option for choice |
| T3.5.T2 | Reflection for rounds < 2 |
| T3.5.T3 | Completion for rounds >= 2 |
| T3.5.T4 | Retry on Error status |
| T3.5.T5 | History recorded for each decision |
| T3.5.T6 | is_healthy() always true |

#### Acceptance Criteria

- Mock returns predefined outputs
- Decision history recorded
- Useful for testing decision layer

#### Technical Notes

```rust
/// Mock decision engine for testing
pub struct MockDecisionEngine {
    /// Predefined outputs by situation
    outputs: HashMap<ProviderStatusPattern, DecisionOutput>,
    
    /// Decision history (for test verification)
    history: Vec<DecisionRecord>,
    
    /// Current configuration
    config: DecisionAgentConfig,
}

impl DecisionEngine for MockDecisionEngine {
    fn engine_type(&self) -> DecisionEngineType {
        DecisionEngineType::Mock
    }
    
    fn decide(&mut self, context: DecisionContext) -> Result<DecisionOutput> {
        // Record decision
        let record = DecisionRecord {
            decision_id: DecisionId::new(),
            timestamp: Utc::now(),
            trigger_status: context.trigger_status.clone(),
            output: self.get_mock_output(&context.trigger_status),
            engine_type: DecisionEngineType::Mock,
        };
        
        self.history.push(record);
        
        Ok(record.output.clone())
    }
    
    fn reset(&mut self) -> Result<()> {
        self.history.clear();
        Ok(())
    }
    
    fn is_healthy(&self) -> bool {
        true // Mock is always healthy
    }
}

impl MockDecisionEngine {
    fn get_mock_output(&self, status: &ProviderStatus) -> DecisionOutput {
        match status {
            ProviderStatus::WaitingForChoice { options } => {
                // Always select first option
                DecisionOutput::Choice {
                    selected: options.first().map(|o| o.id.clone()).unwrap_or_default(),
                    reason: "Mock: first option".to_string(),
                }
            }
            ProviderStatus::ClaimsCompletion { reflection_rounds, .. } => {
                if *reflection_rounds < 2 {
                    DecisionOutput::ReflectionRequest {
                        prompt: "Mock: please reflect".to_string(),
                    }
                } else {
                    DecisionOutput::CompletionConfirm {
                        submit_pr: true,
                        next_task: None,
                    }
                }
            }
            ProviderStatus::PartialCompletion { .. } => {
                DecisionOutput::ContinueInstruction {
                    prompt: "Mock: continue".to_string(),
                    focus_items: vec!["Mock focus".to_string()],
                }
            }
            ProviderStatus::Error { .. } => {
                DecisionOutput::RetryInstruction {
                    prompt: "Mock: retry".to_string(),
                    cooldown_ms: 1000,
                }
            }
        }
    }
    
    pub fn history(&self) -> &[DecisionRecord] {
        &self.history
    }
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| LLM API rate limits | Medium | Medium | Rate limiting, fallback to RuleBased |
| CLI session spawning delay | Low | Low | Lazy creation policy |
| Prompt template effectiveness | Medium | Medium | Iterative refinement based on outcomes |

## Sprint Deliverables

- `decision/src/engine.rs` - DecisionEngine trait
- `decision/src/llm_engine.rs` - LLMDecisionEngine
- `decision/src/cli_engine.rs` - CLIDecisionEngine
- `decision/src/rule_engine.rs` - RuleBasedDecisionEngine
- `decision/src/mock_engine.rs` - MockDecisionEngine
- `decision/src/prompts.rs` - Decision prompt templates

## Dependencies

- Sprint 1: Core Types (DecisionOutput, DecisionContext)
- Sprint 2: Output Classifier (ProviderStatus)

## Next Sprint

After completing this sprint, proceed to [Sprint 4: Context Cache](./sprint-04-context-cache.md) for running context caching with size limits.