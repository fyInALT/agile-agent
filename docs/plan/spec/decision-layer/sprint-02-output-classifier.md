# Sprint 2: Output Classifier (Trait-Based)

## Metadata

- Sprint ID: `decision-sprint-002`
- Title: `Output Classifier (Trait-Based)`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14
- Updated: 2026-04-14 (Architecture Evolution)

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 2 Tests: T2.1.T1-T2.4.T5 (25 tests)
- Provider samples: Collect 12 real samples before implementation (S-CL-001 through S-KM-003)

## Architecture Evolution

Classifiers produce **SituationType** identifiers, then use **SituationRegistry** to build concrete DecisionSituation objects. This enables:
- Adding new provider without modifying situation enum
- Provider-specific situation subtypes (e.g., `finished.claude`, `waiting_for_choice.codex`)
- Custom situation builders per provider

## Sprint Goal

Implement provider-specific output classifiers using SituationType and SituationRegistry pattern.

## Stories

### Story 2.1: OutputClassifier Trait and Registry Integration

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define OutputClassifier trait that integrates with SituationRegistry.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Define `OutputClassifier` trait | Todo | - |
| T2.1.2 | Define `ClassifierRegistry` struct | Todo | - |
| T2.1.3 | Implement registry dispatch by ProviderKind | Todo | - |
| T2.1.4 | Implement situation builder registration | Todo | - |
| T2.1.5 | Write unit tests for trait and registry | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T2.1.T1 | Classifier returns SituationType |
| T2.1.T2 | Classifier builds situation via registry |
| T2.1.T3 | Registry dispatches by provider |
| T2.1.T4 | Unknown provider uses fallback |

#### Acceptance Criteria

- Trait defines classify method returning SituationType
- ClassifierRegistry dispatches by ProviderKind
- Situation building delegated to SituationRegistry

#### Technical Notes

```rust
/// Output classifier trait - produces SituationType
pub trait OutputClassifier: Send + Sync + 'static {
    /// Provider kind this classifier handles
    fn provider_kind(&self) -> ProviderKind;
    
    /// Classify event to situation type (or None for Running)
    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType>;
    
    /// Build situation from event (using registry)
    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn DecisionSituation>>;
    
    /// Extract context data for cache update
    fn extract_context(&self, event: &ProviderEvent) -> Option<ContextUpdate>;
}

/// Context update from event
pub enum ContextUpdate {
    ToolCall(ToolCallRecord),
    FileChange(FileChangeRecord),
    Thinking(String),
    KeyOutput(String),
}

/// Classifier registry - dispatches to provider-specific classifiers
pub struct ClassifierRegistry {
    /// Per-provider classifiers
    classifiers: HashMap<ProviderKind, Box<dyn OutputClassifier>>,
    
    /// Fallback classifier (for unknown providers)
    fallback: Box<dyn OutputClassifier>,
    
    /// Situation registry (shared reference)
    situation_registry: Arc<SituationRegistry>,
}

impl ClassifierRegistry {
    pub fn new(situation_registry: Arc<SituationRegistry>) -> Self {
        Self {
            classifiers: HashMap::new(),
            fallback: Box::new(FallbackClassifier),
            situation_registry,
        }
    }
    
    /// Register classifier for provider
    pub fn register(&mut self, classifier: Box<dyn OutputClassifier>) {
        self.classifiers.insert(classifier.provider_kind(), classifier);
    }
    
    /// Classify event
    pub fn classify(&self, event: &ProviderEvent, provider: ProviderKind) 
        -> ClassifyResult {
        let classifier = self.classifiers.get(&provider)
            .unwrap_or(&self.fallback);
        
        match classifier.classify_type(event) {
            Some(situation_type) => {
                let situation = classifier.build_situation(
                    event,
                    &self.situation_registry,
                );
                
                ClassifyResult::NeedsDecision {
                    situation_type,
                    situation,
                }
            }
            None => ClassifyResult::Running {
                context_update: classifier.extract_context(event),
            },
        }
    }
}

/// Classify result
pub enum ClassifyResult {
    /// Running output - update context
    Running {
        context_update: Option<ContextUpdate>,
    },
    
    /// Needs decision
    NeedsDecision {
        situation_type: SituationType,
        situation: Option<Box<dyn DecisionSituation>>,
    },
}

/// Fallback classifier - minimal classification
pub struct FallbackClassifier;

impl OutputClassifier for FallbackClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Unknown
    }
    
    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            ProviderEvent::Finished => Some(builtin_situations::CLAIMS_COMPLETION.clone()),
            ProviderEvent::Error { .. } => Some(builtin_situations::ERROR.clone()),
            _ => None,
        }
    }
    
    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn DecisionSituation>> {
        let type = self.classify_type(event)?;
        registry.build_from_event(type, event)
    }
    
    fn extract_context(&self, _event: &ProviderEvent) -> Option<ContextUpdate> {
        None
    }
}
```

---

### Story 2.2: Claude Classifier with Situation Builders

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement Claude classifier with Claude-specific situation builders.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Create `ClaudeClassifier` struct | Todo | - |
| T2.2.2 | Implement `classify_type()` for Claude events | Todo | - |
| T2.2.3 | Create `ClaudeFinishedBuilder` for Claude Finished events | Todo | - |
| T2.2.4 | Create `ClaudeErrorBuilder` for Claude Error events | Todo | - |
| T2.2.5 | Register Claude situation builders in registry | Todo | - |
| T2.2.6 | Write unit tests with real Claude samples | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T2.2.T1 | AssistantChunk → Running (no situation) |
| T2.2.T2 | ThinkingChunk → Running (context update) |
| T2.2.T3 | ToolCall → Running (context update) |
| T2.2.T4 | Finished → SituationType::CLAUDE_FINISHED |
| T2.2.T5 | Error → SituationType::ERROR |
| T2.2.T6 | Claude builder extracts summary from transcript |

#### Acceptance Criteria

- Claude events classified correctly
- Claude-specific situation subtype used
- Context extracted for Running events

#### Technical Notes

Based on source code analysis (`core/src/providers/claude.rs`):

```rust
/// Claude classifier
pub struct ClaudeClassifier;

impl OutputClassifier for ClaudeClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Claude
    }
    
    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            // Running events - no situation
            ProviderEvent::AssistantChunk { .. } => None,
            ProviderEvent::ThinkingChunk { .. } => None,
            ProviderEvent::GenericToolCallStarted { .. } => None,
            ProviderEvent::GenericToolCallFinished { .. } => None,
            ProviderEvent::Status { .. } => None,
            ProviderEvent::SessionHandle { .. } => None,
            
            // Finished - Claude-specific subtype
            ProviderEvent::Finished => Some(
                builtin_situations::CLAUDE_FINISHED.clone()
            ),
            
            // Error
            ProviderEvent::Error { .. } => Some(
                builtin_situations::ERROR.clone()
            ),
            
            _ => None,
        }
    }
    
    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn DecisionSituation>> {
        let type = self.classify_type(event)?;
        
        // Use registry builder (registered in initialization)
        registry.build_from_event(type, event)
    }
    
    fn extract_context(&self, event: &ProviderEvent) -> Option<ContextUpdate> {
        match event {
            ProviderEvent::ThinkingChunk { text } => Some(ContextUpdate::Thinking(text.clone())),
            ProviderEvent::AssistantChunk { text } if self.is_key_output(text) => {
                Some(ContextUpdate::KeyOutput(text.clone()))
            }
            ProviderEvent::GenericToolCallStarted { name, input } => {
                Some(ContextUpdate::ToolCall(ToolCallRecord {
                    name: name.clone(),
                    input_preview: Some(input.clone()),
                    output_preview: None,
                    timestamp: Utc::now(),
                    success: true,
                }))
            }
            ProviderEvent::GenericToolCallFinished { name, output } => {
                // Update previous tool call with output
                Some(ContextUpdate::ToolCall(ToolCallRecord {
                    name: name.clone(),
                    input_preview: None,
                    output_preview: Some(output.clone()),
                    timestamp: Utc::now(),
                    success: true,
                }))
            }
            _ => None,
        }
    }
}

/// Claude situation builders - registered in SituationRegistry
pub fn register_claude_builders(registry: &mut SituationRegistry) {
    // Claude Finished builder
    registry.register_builder(
        builtin_situations::CLAUDE_FINISHED.clone(),
        |event: &ProviderEvent| {
            // Claude Finished needs summary extraction from transcript
            // This is ClaimsCompletion situation with Claude-specific data
            Some(Box::new(ClaimsCompletionSituation {
                summary: "Extracted from transcript".to_string(), // Will be filled
                reflection_rounds: 0,
                max_reflection_rounds: 2,
                confidence: 0.0, // Will be calculated
            }))
        },
    );
    
    // Claude Error builder
    registry.register_builder(
        builtin_situations::ERROR.clone(),
        |event: &ProviderEvent| {
            match event {
                ProviderEvent::Error { message } => Some(Box::new(ErrorSituation {
                    error: ErrorInfo {
                        error_type: "claude_error".to_string(),
                        message: message.clone(),
                        recoverable: true,
                        retry_count: 0,
                    },
                })),
                _ => None,
            }
        },
    );
}

/// Key output detection
impl ClaudeClassifier {
    fn is_key_output(&self, text: &str) -> bool {
        // Detect decision-related keywords
        text.contains("完成") || 
        text.contains("finished") ||
        text.contains("done") ||
        text.contains("成功") ||
        text.contains("success")
    }
}
```

**Key Finding**: Claude Code uses `--permission-mode bypassPermissions` (see `claude.rs:169`), so it never returns waiting-for-choice events.

---

### Story 2.3: Codex Classifier with Approval Request Builders

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement Codex classifier with approval request situation builders.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Create `CodexClassifier` struct | Todo | - |
| T2.3.2 | Implement approval request detection | Todo | - |
| T2.3.3 | Create `CodexApprovalBuilder` for approval requests | Todo | - |
| T2.3.4 | Parse `ReviewDecision` options from params | Todo | - |
| T2.3.5 | Register Codex situation builders | Todo | - |
| T2.3.6 | Write unit tests with Codex samples | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T2.3.T1 | execCommandApproval → CODEX_APPROVAL type |
| T2.3.T2 | applyPatchApproval → CODEX_APPROVAL type |
| T2.3.T3 | ReviewDecision options parsed |
| T2.3.T4 | Codex builder creates WaitingForChoiceSituation |
| T2.3.T5 | Dangerous command detected (critical=true) |

#### Acceptance Criteria

- All approval requests detected
- ReviewDecision options parsed correctly
- Critical commands flagged

#### Technical Notes

Based on source code analysis (`../codex/codex-rs/app-server-protocol`):

```rust
/// Codex classifier
pub struct CodexClassifier;

impl OutputClassifier for CodexClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Codex
    }
    
    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            // Approval requests = WaitingForChoice (Codex subtype)
            ProviderEvent::CodexApprovalRequest { method, .. } => {
                match method.as_str() {
                    "execCommandApproval" |
                    "item/commandExecution/requestApproval" |
                    "applyPatchApproval" |
                    "item/patch/requestApproval" |
                    "item/tool/requestUserInput" |
                    "item/permissions/requestApproval" => Some(
                        builtin_situations::CODEX_APPROVAL.clone()
                    ),
                    _ => None,
                }
            }
            
            // Finished
            ProviderEvent::Finished => Some(builtin_situations::CLAIMS_COMPLETION.clone()),
            
            // Error
            ProviderEvent::CodexError { kind, .. } => Some(
                SituationType::with_subtype("error", kind.as_str())
            ),
            
            // Running
            _ => None,
        }
    }
    
    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn DecisionSituation>> {
        let type = self.classify_type(event)?;
        registry.build_from_event(type, event)
    }
    
    fn extract_context(&self, event: &ProviderEvent) -> Option<ContextUpdate> {
        match event {
            ProviderEvent::PatchApplyStarted { path, .. } => Some(ContextUpdate::FileChange(
                FileChangeRecord {
                    path: path.clone(),
                    change_type: ChangeType::Modified,
                    diff_preview: None,
                }
            )),
            _ => None,
        }
    }
}

/// Codex situation builders
pub fn register_codex_builders(registry: &mut SituationRegistry) {
    // Codex approval builder
    registry.register_builder(
        builtin_situations::CODEX_APPROVAL.clone(),
        |event: &ProviderEvent| {
            match event {
                ProviderEvent::CodexApprovalRequest { method, params, .. } => {
                    Some(Box::new(WaitingForChoiceSituation {
                        options: parse_codex_options(method, params),
                        permission_type: Some(method.clone()),
                        critical: is_critical_command(params),
                    }))
                }
                _ => None,
            }
        },
    );
    
    // Codex error builders
    registry.register_builder(
        SituationType::with_subtype("error", "timed_out"),
        |event: &ProviderEvent| {
            Some(Box::new(ErrorSituation {
                error: ErrorInfo {
                    error_type: "timed_out".to_string(),
                    message: "Codex timed out".to_string(),
                    recoverable: true,
                    retry_count: 0,
                },
            }))
        },
    );
}

/// Parse Codex approval options
fn parse_codex_options(method: &str, params: &serde_json::Value) -> Vec<ChoiceOption> {
    match method {
        "execCommandApproval" => vec![
            ChoiceOption { id: "approved".into(), label: "Approve".into(), description: None },
            ChoiceOption { id: "approved_for_session".into(), label: "Approve for session".into(), description: None },
            ChoiceOption { id: "denied".into(), label: "Deny".into(), description: None },
            ChoiceOption { id: "abort".into(), label: "Abort".into(), description: None },
        ],
        "applyPatchApproval" => vec![
            ChoiceOption { id: "approved".into(), label: "Approve patch".into(), description: None },
            ChoiceOption { id: "approved_for_session".into(), label: "Approve for session".into(), description: None },
            ChoiceOption { id: "denied".into(), label: "Deny".into(), description: None },
            ChoiceOption { id: "abort".into(), label: "Abort".into(), description: None },
        ],
        "item/tool/requestUserInput" => {
            // Parse from params
            params.get("options")
                .and_then(|o| o.as_array())
                .map(|arr| arr.iter().filter_map(|v| {
                    v.get("id").and_then(|id| id.as_str())
                        .map(|id| ChoiceOption { 
                            id: id.into(), 
                            label: v.get("label").and_then(|l| l.as_str()).unwrap_or(id).into(),
                            description: None,
                        })
                }).collect())
                .unwrap_or_default()
        },
        _ => vec![
            ChoiceOption { id: "approved".into(), label: "Approve".into(), description: None },
            ChoiceOption { id: "denied".into(), label: "Deny".into(), description: None },
        ],
    }
}

/// Detect critical commands
fn is_critical_command(params: &serde_json::Value) -> bool {
    params.get("command")
        .and_then(|c| c.as_str())
        .map(|cmd| {
            cmd.contains("rm -rf") ||
            cmd.contains("sudo") ||
            cmd.contains("chmod") ||
            cmd.contains("drop table") ||
            cmd.contains("delete from")
        })
        .unwrap_or(false)
}
```

**Key Finding**: Codex is the provider needing most decision intervention with multiple approval request types.

---

### Story 2.4: ACP Classifier (OpenCode/Kimi)

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement ACP classifier for OpenCode and Kimi permission.asked events.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Create `ACPClassifier` struct | Todo | - |
| T2.4.2 | Detect `permission.asked` notification | Todo | - |
| T2.4.3 | Parse permission options (once/always/reject) | Todo | - |
| T2.4.4 | Detect `session.status.idle` completion | Todo | - |
| T2.4.5 | Handle retry status (attempt count) | Todo | - |
| T2.4.6 | Register ACP situation builders | Todo | - |
| T2.4.7 | Write unit tests with ACP samples | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T2.4.T1 | permission.asked → ACP_PERMISSION type |
| T2.4.T2 | Permission options parsed (once/always/reject) |
| T2.4.T3 | session.status.idle → CLAIMS_COMPLETION |
| T2.4.T4 | session.status.busy → Running |
| T2.4.T5 | retry (attempt <= 3) → Running |
| T2.4.T6 | retry (attempt > 3) → ERROR |

#### Acceptance Criteria

- permission.asked detected as WaitingForChoice
- session.status.idle detected as ClaimsCompletion
- Retry exhaustion handled as Error

#### Technical Notes

Based on source code analysis (`../opencode`, `../kimi-cli`):

```rust
/// ACP classifier (OpenCode/Kimi)
pub struct ACPClassifier;

impl OutputClassifier for ACPClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::ACP // Covers both OpenCode and Kimi
    }
    
    fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
        match event {
            // Permission asked = WaitingForChoice (ACP subtype)
            ProviderEvent::ACPNotification { method, params } if method == "permission.asked" => {
                Some(builtin_situations::ACP_PERMISSION.clone())
            }
            
            // Session status idle = Completion
            ProviderEvent::ACPNotification { method, params } if method == "session.status" => {
                let status = params.get("status").and_then(|s| s.as_str()).unwrap_or("busy");
                match status {
                    "idle" => Some(builtin_situations::CLAIMS_COMPLETION.clone()),
                    "retry" => {
                        let attempt = params.get("attempt").and_then(|a| a.as_u64()).unwrap_or(0);
                        if attempt > 3 {
                            Some(SituationType::with_subtype("error", "retry_exhausted"))
                        } else {
                            None // Running
                        }
                    }
                    _ => None, // busy, running
                }
            }
            
            // ACP error
            ProviderEvent::ACPError { code, .. } => Some(
                SituationType::with_subtype("error", code.as_str())
            ),
            
            _ => None,
        }
    }
    
    fn build_situation(
        &self,
        event: &ProviderEvent,
        registry: &SituationRegistry,
    ) -> Option<Box<dyn DecisionSituation>> {
        let type = self.classify_type(event)?;
        registry.build_from_event(type, event)
    }
    
    fn extract_context(&self, event: &ProviderEvent) -> Option<ContextUpdate> {
        match event {
            ProviderEvent::ACPNotification { method, params } if method == "assistant/message" => {
                params.get("text")
                    .map(|t| ContextUpdate::KeyOutput(t.as_str().unwrap_or("").into()))
            }
            _ => None,
        }
    }
}

/// ACP situation builders
pub fn register_acp_builders(registry: &mut SituationRegistry) {
    // ACP permission builder
    registry.register_builder(
        builtin_situations::ACP_PERMISSION.clone(),
        |event: &ProviderEvent| {
            match event {
                ProviderEvent::ACPNotification { params, .. } => {
                    let permission_type = params.get("permission")
                        .and_then(|p| p.as_str())
                        .unwrap_or("unknown");
                    
                    Some(Box::new(WaitingForChoiceSituation {
                        options: vec![
                            ChoiceOption { id: "once".into(), label: "Once".into(), description: None },
                            ChoiceOption { id: "always".into(), label: "Always for session".into(), description: None },
                            ChoiceOption { id: "reject".into(), label: "Reject".into(), description: None },
                        ],
                        permission_type: Some(permission_type.into()),
                        critical: is_critical_permission(permission_type, params),
                    }))
                }
                _ => None,
            }
        },
    );
    
    // ACP completion builder
    registry.register_builder(
        builtin_situations::CLAIMS_COMPLETION.clone(),
        |event: &ProviderEvent| {
            Some(Box::new(ClaimsCompletionSituation {
                summary: "ACP session idle".into(),
                reflection_rounds: 0,
                max_reflection_rounds: 2,
                confidence: 0.8,
            }))
        },
    );
}

/// Detect critical permissions
fn is_critical_permission(permission_type: &str, params: &serde_json::Value) -> bool {
    match permission_type {
        "write" | "edit" => {
            params.get("path")
                .and_then(|p| p.as_str())
                .map(|path| path.contains(".env") || path.contains("credentials"))
                .unwrap_or(false)
        },
        "execute" => {
            params.get("command")
                .and_then(|c| c.as_str())
                .map(|cmd| cmd.contains("rm") || cmd.contains("sudo"))
                .unwrap_or(false)
        },
        _ => false,
    }
}
```

---

### Story 2.5: Classifier Initialization and Registration

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement initialization logic to register all classifiers and builders.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.5.1 | Create `DecisionLayerInitializer` struct | Todo | - |
| T2.5.2 | Implement `initialize_situation_registry()` | Todo | - |
| T2.5.3 | Implement `initialize_classifier_registry()` | Todo | - |
| T2.5.4 | Implement `initialize_action_registry()` | Todo | - |
| T2.5.5 | Write unit tests for initialization | Todo | - |

#### TDD Test Tasks

| Test ID | Definition |
|---------|------------|
| T2.5.T1 | All built-in situations registered |
| T2.5.T2 | All provider classifiers registered |
| T2.5.T3 | All built-in actions registered |
| T2.5.T4 | Registry lookup works for all types |

#### Acceptance Criteria

- All built-in situations, actions, classifiers registered
- Initialization order documented
- Registry ready for use after init

#### Technical Notes

```rust
/// Decision layer initializer
pub struct DecisionLayerInitializer {
    config: DecisionLayerConfig,
}

impl DecisionLayerInitializer {
    /// Initialize complete decision layer
    pub fn initialize(config: DecisionLayerConfig) -> DecisionLayerComponents {
        // 1. Initialize situation registry with built-ins
        let mut situation_registry = SituationRegistry::with_builtins();
        register_claude_builders(&mut situation_registry);
        register_codex_builders(&mut situation_registry);
        register_acp_builders(&mut situation_registry);
        
        // 2. Initialize action registry with built-ins
        let mut action_registry = ActionRegistry::with_builtins();
        // Custom actions can be registered here
        if let Some(custom_actions) = &config.custom_actions {
            for action in custom_actions {
                action_registry.register(action.clone());
            }
        }
        
        // 3. Initialize classifier registry
        let mut classifier_registry = ClassifierRegistry::new(
            Arc::new(situation_registry)
        );
        classifier_registry.register(Box::new(ClaudeClassifier));
        classifier_registry.register(Box::new(CodexClassifier));
        classifier_registry.register(Box::new(ACPClassifier));
        
        // 4. Return components
        DecisionLayerComponents {
            situation_registry: Arc::new(situation_registry),
            action_registry: Arc::new(action_registry),
            classifier_registry: Arc::new(classifier_registry),
        }
    }
}

/// Initialized decision layer components
pub struct DecisionLayerComponents {
    pub situation_registry: Arc<SituationRegistry>,
    pub action_registry: Arc<ActionRegistry>,
    pub classifier_registry: Arc<ClassifierRegistry>,
}
```

---

## Architecture Benefits

| Aspect | Before (Enum) | After (Trait) |
|--------|--------------|---------------|
| New provider support | Modify enum + all match branches | Implement classifier + register |
| Provider-specific data | Fixed fields in enum | Custom situation struct |
| Critical detection | Hardcoded logic | Builder function logic |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Builder registration order | Low | Medium | Clear init order documentation |
| Parser complexity for LLM output | Medium | Medium | Standardized output format |

## Sprint Deliverables

- `decision/src/classifier.rs` - OutputClassifier trait
- `decision/src/classifier_registry.rs` - ClassifierRegistry
- `decision/src/claude_classifier.rs` - ClaudeClassifier + builders
- `decision/src/codex_classifier.rs` - CodexClassifier + builders
- `decision/src/acp_classifier.rs` - ACPClassifier + builders
- `decision/src/initializer.rs` - DecisionLayerInitializer

## Dependencies

- Sprint 1: DecisionSituation trait, SituationRegistry, ActionType

## Next Sprint

After completing this sprint, proceed to [Sprint 3: Decision Engine](sprint-03-decision-engine.md) for decision engine implementations using ActionRegistry.