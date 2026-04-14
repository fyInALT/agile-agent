# Sprint 7: Error Recovery

## Metadata

- Sprint ID: `decision-sprint-007`
- Title: `Error Recovery`
- Duration: 1 week
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 7 Tests: T7.1.T1-T7.4.T5 (22 tests)
- Test execution: Write failing tests first, implement minimum, refactor

## Sprint Goal

Implement retry logic, timeout handling, recovery level escalation, and Decision Agent self-error recovery.

## Stories

### Story 7.1: Recovery Level Escalation Strategy

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement tiered recovery escalation when automatic retries fail.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.1.1 | Create `RecoveryLevel` enum | Todo | - |
| T7.1.2 | Implement level determination logic | Todo | - |
| T7.1.3 | Implement level escalation flow | Todo | - |
| T7.1.4 | Implement `handle_recovery()` method | Todo | - |
| T7.1.5 | Write unit tests for escalation | Todo | - |

#### Acceptance Criteria

- All recovery levels implemented
- Escalation follows defined order
- Final fallback to task failure

#### Technical Notes

```rust
pub enum RecoveryLevel {
    /// Level 0: Automatic retry
    AutoRetry,
    
    /// Level 1: Adjusted retry (different prompt)
    AdjustedRetry,
    
    /// Level 2: Switch decision engine
    SwitchEngine,
    
    /// Level 3: Human intervention
    HumanIntervention,
    
    /// Level 4: Task failed
    TaskFailed,
}

impl DecisionAgent {
    fn determine_recovery_level(&self) -> RecoveryLevel {
        if self.retry_count < self.config.max_retries {
            if self.retry_count < 2 {
                RecoveryLevel::AutoRetry
            } else {
                RecoveryLevel::AdjustedRetry
            }
        } else if self.engine_switch_count < 1 {
            RecoveryLevel::SwitchEngine
        } else if !self.human_requested {
            RecoveryLevel::HumanIntervention
        } else {
            RecoveryLevel::TaskFailed
        }
    }
    
    fn handle_recovery(&mut self, error: &DecisionError) -> Result<()> {
        let level = self.determine_recovery_level();
        
        match level {
            RecoveryLevel::AutoRetry => {
                // Cooldown then retry same prompt
                self.retry_with_cooldown()?;
            }
            
            RecoveryLevel::AdjustedRetry => {
                // Different prompt template, more context
                self.retry_with_adjusted_prompt()?;
            }
            
            RecoveryLevel::SwitchEngine => {
                // Switch to different engine type
                self.switch_engine()?;
                self.retry()?;
            }
            
            RecoveryLevel::HumanIntervention => {
                // Pause main agent, request human decision
                self.pause_main_agent();
                self.request_human_decision(error)?;
            }
            
            RecoveryLevel::TaskFailed => {
                // Mark task failed, select next task
                self.mark_task_failed();
                self.select_next_task();
            }
        }
    }
    
    fn retry_with_adjusted_prompt(&mut self) -> Result<()> {
        // Use different prompt template
        self.retry_count += 1;
        
        let prompt = self.build_adjusted_prompt()?;
        self.execute_decision(prompt)?;
    }
    
    fn switch_engine(&mut self) -> Result<()> {
        // Switch engine type (LLM -> RuleBased or vice versa)
        self.engine_switch_count += 1;
        
        match self.engine_type() {
            DecisionEngineType::LLM { .. } => {
                self.engine = Box::new(RuleBasedDecisionEngine::default());
            }
            DecisionEngineType::CLI { .. } => {
                self.engine = Box::new(RuleBasedDecisionEngine::default());
            }
            DecisionEngineType::RuleBased => {
                self.engine = Box::new(LLMDecisionEngine::default());
            }
            DecisionEngineType::Mock => {}
        }
        
        Ok(())
    }
}
```

**Recovery Flow Diagram**:

```
Error occurs
    │
    ▼
Level 0: Auto retry (cooldown + same prompt)
    │ retry_count++
    │
    ▼ (if still fails after 2 retries)
Level 1: Adjusted retry (different prompt, more context)
    │ retry_count++
    │
    ▼ (if max_retries exceeded)
Level 2: Switch engine (LLM → RuleBased)
    │ engine_switch_count++
    │
    ▼ (if switch fails)
Level 3: Human intervention
    │ pause main agent
    │ request human decision
    │
    ▼ (if human timeout or failed)
Level 4: Task failed
    │ mark task failed
    │ select next task
```

---

### Story 7.2: Timeout Handling with Fallback

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement decision timeout handling with fallback strategies.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.2.1 | Implement timeout detection | Todo | - |
| T7.2.2 | Implement `TimeoutFallback` enum | Todo | - |
| T7.2.3 | Implement timeout retry | Todo | - |
| T7.2.4 | Implement fallback decision execution | Todo | - |
| T7.2.5 | Write unit tests for timeout handling | Todo | - |

#### Acceptance Criteria

- Timeout detected correctly
- Fallback strategies work
- Timeout retry limited

#### Technical Notes

```rust
pub enum TimeoutFallback {
    /// Use RuleBased engine
    UseRuleBased,
    
    /// Return default decision (first option)
    DefaultDecision,
    
    /// Request human intervention
    HumanIntervention,
}

impl DecisionAgent {
    fn decide_with_timeout(&mut self, context: DecisionContext) -> Result<DecisionOutput> {
        let timeout = Duration::from_millis(self.config.decision_timeout_ms);
        
        // Wrap decision in timeout
        let result = timeout::run(
            || self.engine.decide(context),
            timeout,
        );
        
        match result {
            Ok(decision) => Ok(decision),
            Err(TimeoutError) => self.handle_decision_timeout(),
        }
    }
    
    fn handle_decision_timeout(&mut self) -> Result<DecisionOutput> {
        self.timeout_count += 1;
        
        // Retry with timeout up to configured limit
        if self.timeout_count <= self.config.timeout_retries {
            // Cooldown and retry
            std::thread::sleep(Duration::from_millis(5000));
            return self.decide_with_timeout(self.current_context()?);
        }
        
        // Fallback based on configuration
        match self.config.timeout_fallback {
            TimeoutFallback::UseRuleBased => {
                let rule_engine = RuleBasedDecisionEngine::default();
                rule_engine.decide(self.current_context()?)
            }
            
            TimeoutFallback::DefaultDecision => {
                // Return safe default
                Ok(self.default_decision())
            }
            
            TimeoutFallback::HumanIntervention => {
                self.request_human_intervention(TimeoutReason)?;
                // Wait for human response (handled elsewhere)
                Err(Error::WaitingForHuman)
            }
        }
    }
    
    fn default_decision(&self) -> DecisionOutput {
        // Safe default based on trigger status
        match &self.current_context()?.trigger_status {
            ProviderStatus::WaitingForChoice { options } => {
                // Select first option (most conservative)
                DecisionOutput::Choice {
                    selected: options.first().map(|o| o.id.clone()).unwrap_or_default(),
                    reason: "Timeout default: first option".to_string(),
                }
            }
            ProviderStatus::ClaimsCompletion { .. } => {
                // Request reflection (conservative)
                DecisionOutput::ReflectionRequest {
                    prompt: "Please verify completion".to_string(),
                }
            }
            ProviderStatus::PartialCompletion { .. } => {
                // Continue (conservative)
                DecisionOutput::ContinueInstruction {
                    prompt: "Continue with current task".to_string(),
                    focus_items: vec![],
                }
            }
            ProviderStatus::Error { .. } => {
                // Retry (conservative)
                DecisionOutput::RetryInstruction {
                    prompt: "Retry the task".to_string(),
                    cooldown_ms: 10000,
                }
            }
        }
    }
}
```

**Timeout Configuration**:

```toml
[decision_layer.timeout]
# Decision timeout in milliseconds
decision_timeout_ms = 60000

# Number of timeout retries before fallback
timeout_retries = 2

# Fallback strategy on timeout exhaustion
fallback = "use_rule_based"  # or "default_decision", "human_intervention"
```

---

### Story 7.3: Decision Agent Self-Error Recovery

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Implement recovery when Decision Agent itself encounters errors.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.3.1 | Create `DecisionAgentError` enum | Todo | - |
| T7.3.2 | Implement `handle_self_error()` method | Todo | - |
| T7.3.3 | Implement session recreation | Todo | - |
| T7.3.4 | Implement context rebuilding | Todo | - |
| T7.3.5 | Implement state reset | Todo | - |
| T7.3.6 | Write unit tests for self-error recovery | Todo | - |

#### Acceptance Criteria

- Decision Agent recovers from internal errors
- Session recreated if lost
- Context rebuilt if corrupted

#### Technical Notes

```rust
pub enum DecisionAgentError {
    /// Engine call failed
    EngineError { message: String },
    
    /// Provider session lost
    SessionLost,
    
    /// Context parsing error
    ContextParseError,
    
    /// Internal state corruption
    InternalError,
}

impl DecisionAgent {
    fn handle_self_error(&mut self, error: DecisionAgentError) -> Result<()> {
        log::warn!("DecisionAgent self-error: {:?}", error);
        
        match error {
            DecisionAgentError::EngineError { message } => {
                // Log and try switching engine
                log::error!("Engine error: {}", message);
                self.switch_engine()?;
            }
            
            DecisionAgentError::SessionLost => {
                // Recreate session
                self.recreate_session()?;
            }
            
            DecisionAgentError::ContextParseError => {
                // Rebuild context from transcript
                self.rebuild_context()?;
            }
            
            DecisionAgentError::InternalError => {
                // Full reset
                self.reset()?;
            }
        }
        
        Ok(())
    }
    
    fn recreate_session(&mut self) -> Result<()> {
        // Close old session if exists
        if let Some(session) = &self.session {
            self.provider.close_session(session)?;
        }
        
        // Create new session
        let cwd = self.get_parent_cwd()?;
        let new_session = self.provider.create_session(&cwd)?;
        self.session = Some(new_session);
        
        // Reset engine state (new session = fresh context)
        self.engine.reset()?;
        
        // Rebuild context from history
        self.rebuild_context()?;
        
        Ok(())
    }
    
    fn rebuild_context(&mut self) -> Result<()> {
        // Rebuild context from transcript history
        let transcript = self.load_transcript()?;
        
        // Extract key events from transcript
        self.context_cache.clear();
        
        for entry in transcript.entries {
            match entry.event {
                ProviderEvent::PatchApplyFinished { .. } => {
                    // Add to file changes
                    self.context_cache.add_file_change(entry.to_file_change());
                }
                ProviderEvent::GenericToolCallFinished { .. } => {
                    // Add to tool calls
                    self.context_cache.add_tool_call(entry.to_tool_call());
                }
                ProviderEvent::ThinkingChunk { .. } => {
                    // Add to thinking
                    self.context_cache.add_thinking(entry.text);
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    fn reset(&mut self) -> Result<()> {
        // Full reset of decision agent state
        self.session = None;
        self.context_cache.clear();
        self.reflection_rounds = 0;
        self.retry_count = 0;
        self.timeout_count = 0;
        self.engine_switch_count = 0;
        self.human_requested = false;
        
        // Reset engine
        self.engine.reset()?;
        
        // Keep transcript for debugging
        
        Ok(())
    }
}
```

---

### Story 7.4: Health Check and Auto-Recovery

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Implement health monitoring and automatic recovery.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T7.4.1 | Create `DecisionAgentHealth` struct | Todo | - |
| T7.4.2 | Implement health metrics calculation | Todo | - |
| T7.4.3 | Implement `is_healthy()` check | Todo | - |
| T7.4.4 | Implement periodic health check | Todo | - |
| T7.4.5 | Implement auto-recovery trigger | Todo | - |
| T7.4.6 | Write unit tests for health checks | Todo | - |

#### Acceptance Criteria

- Health metrics tracked
- Auto-recovery triggers correctly
- Periodic checks work

#### Technical Notes

```rust
pub struct DecisionAgentHealth {
    /// Recent decision success rate
    pub success_rate: f64,
    
    /// Average decision duration
    pub avg_decision_time_ms: u64,
    
    /// Consecutive failures
    pub consecutive_failures: u8,
    
    /// Session status
    pub session_status: SessionHealthStatus,
    
    /// Engine status
    pub engine_status: EngineHealthStatus,
    
    /// Last health check time
    pub last_check: DateTime<Utc>,
}

pub enum SessionHealthStatus {
    Active,
    Stale,
    Lost,
}

pub enum EngineHealthStatus {
    Healthy,
    Degraded,
    Failed,
}

impl DecisionAgent {
    fn health_check(&self) -> DecisionAgentHealth {
        DecisionAgentHealth {
            success_rate: self.calculate_success_rate(),
            avg_decision_time_ms: self.calculate_avg_decision_time(),
            consecutive_failures: self.consecutive_failures,
            session_status: self.session_health(),
            engine_status: self.engine.health_status(),
            last_check: Utc::now(),
        }
    }
    
    fn is_healthy(&self) -> bool {
        let health = self.health_check();
        
        // Health criteria:
        // - Success rate > 70%
        // - Avg decision time < 30 seconds
        // - No more than 3 consecutive failures
        // - Session active
        // - Engine healthy
        
        health.success_rate > 0.7 &&
        health.avg_decision_time_ms < 30000 &&
        health.consecutive_failures < 3 &&
        health.session_status == SessionHealthStatus::Active &&
        health.engine_status == EngineHealthStatus::Healthy
    }
    
    fn auto_recover(&mut self) -> Result<()> {
        if !self.is_healthy() {
            log::warn!("DecisionAgent unhealthy, triggering auto-recovery");
            self.reset()?;
            
            // Recreate session
            self.recreate_session()?;
            
            log::info!("DecisionAgent auto-recovery completed");
        }
        
        Ok(())
    }
    
    fn calculate_success_rate(&self) -> f64 {
        // Calculate from recent decision history
        let recent = self.decision_history.iter().rev().take(10);
        let successes = recent.filter(|r| r.output.is_success()).count();
        successes as f64 / 10.0
    }
    
    fn calculate_avg_decision_time(&self) -> u64 {
        // Calculate from recent decision durations
        let recent = self.decision_history.iter().rev().take(10);
        recent.map(|r| r.duration_ms).sum::<u64>() / 10
    }
    
    fn session_health(&self) -> SessionHealthStatus {
        match &self.session {
            Some(session) => {
                if self.validate_session(session) {
                    SessionHealthStatus::Active
                } else {
                    SessionHealthStatus::Stale
                }
            }
            None => SessionHealthStatus::Lost,
        }
    }
}

/// Periodic health check task
pub fn start_health_check_loop(agent: Arc<Mutex<DecisionAgent>>) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60)); // Check every minute
            
            let mut agent = agent.lock().unwrap();
            if !agent.is_healthy() {
                agent.auto_recover().ok();
            }
        }
    });
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Escalation infinite loop | Low | Medium | Clear TaskFailed termination |
| Session recreation delay | Medium | Low | Lazy recreation, caching |
| Health check overhead | Low | Low | Periodic check (every minute) |

## Sprint Deliverables

- `decision/src/recovery.rs` - Recovery level handling
- `decision/src/health.rs` - Health check and auto-recovery
- `decision/src/error.rs` - DecisionAgentError handling
- Unit tests for all recovery paths

## Dependencies

- Sprint 3: Decision Engine (engine switching)
- Sprint 5: Lifecycle (session recreation)
- Sprint 6: Human Intervention (escalation target)

## Next Sprint

After completing this sprint, proceed to [Sprint 8: Integration](./sprint-08-integration.md) for integration with existing agile-agent components.