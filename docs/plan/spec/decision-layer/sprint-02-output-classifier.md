# Sprint 2: Output Classifier

## Metadata

- Sprint ID: `decision-sprint-002`
- Title: `Output Classifier`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-14

## TDD Reference

See [Test Specification](test-specification.md) for detailed TDD test tasks:
- Sprint 2 Tests: T2.1.T1-T2.4.T5 (25 tests)
- Provider samples: Collect 12 real samples before implementation (S-CL-001 through S-KM-003)

## Sprint Goal

Implement provider-specific output classifiers to detect decision trigger points based on actual provider protocols (Claude stream-json, Codex App Server Protocol, ACP for OpenCode/Kimi).

## Stories

### Story 2.1: Claude Output Classifier

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement classifier for Claude Code stream-json protocol.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Create `ClaudeOutputClassifier` struct | Todo | - |
| T2.1.2 | Implement `classify()` method for Claude events | Todo | - |
| T2.1.3 | Handle `AssistantChunk` → Running | Todo | - |
| T2.1.4 | Handle `ThinkingChunk` → Running | Todo | - |
| T2.1.5 | Handle `GenericToolCallStarted/Finished` → Running | Todo | - |
| T2.1.6 | Handle `Finished` event → ClaimsCompletion | Todo | - |
| T2.1.7 | Handle `Error` event → Error | Todo | - |
| T2.1.8 | Handle `SessionHandle` → Running (info) | Todo | - |
| T2.1.9 | Write unit tests with real Claude event samples | Todo | - |

#### Acceptance Criteria

- Claude events classified correctly
- No waiting-for-choice detection (bypassPermissions)
- Finished event triggers ClaimsCompletion
- Error event triggers Error status

#### Technical Notes

Based on source code analysis (`core/src/providers/claude.rs`):

```rust
/// Claude Code output classifier
pub struct ClaudeOutputClassifier;

impl OutputClassifier for ClaudeOutputClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Claude
    }
    
    fn classify(&self, event: &ProviderEvent) -> ProviderOutputType {
        match event {
            // Running events - collect but don't act
            ProviderEvent::AssistantChunk { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::ThinkingChunk { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::GenericToolCallStarted { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::GenericToolCallFinished { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::Status { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::SessionHandle { .. } => ProviderOutputType::Running { event: event.clone() },
            
            // Finished - claims completion (Situation 2)
            ProviderEvent::Finished => ProviderOutputType::Finished {
                status: ProviderStatus::ClaimsCompletion {
                    summary: String::new(), // Will be filled from transcript analysis
                    reflection_rounds: 0,
                },
            },
            
            // Error - Situation 4
            ProviderEvent::Error { message } => ProviderOutputType::Finished {
                status: ProviderStatus::Error {
                    error_type: ErrorType::Failure { message: message.clone() },
                },
            },
            
            _ => ProviderOutputType::Running { event: event.clone() },
        }
    }
    
    fn extract_options(&self, _event: &ProviderEvent) -> Option<Vec<ChoiceOption>> {
        // Claude uses --permission-mode bypassPermissions
        // No waiting-for-choice events expected
        None
    }
    
    fn extract_completion_summary(&self, event: &ProviderEvent) -> Option<String> {
        match event {
            ProviderEvent::AssistantChunk { text } => Some(text.clone()),
            _ => None,
        }
    }
}
```

**Key Finding**: Claude Code uses `--permission-mode bypassPermissions` (see `claude.rs:169`), so it never returns waiting-for-choice events. Decision layer only handles:
- ClaimsCompletion (from `Finished`)
- Error (from `Error`)

---

### Story 2.2: Codex Output Classifier

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Implement classifier for Codex App Server Protocol.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Create `CodexOutputClassifier` struct | Todo | - |
| T2.2.2 | Implement `classify()` method for Codex requests | Todo | - |
| T2.2.3 | Handle `execCommandApproval` → WaitingForChoice | Todo | - |
| T2.2.4 | Handle `applyPatchApproval` → WaitingForChoice | Todo | - |
| T2.2.5 | Handle `item/tool/requestUserInput` → WaitingForChoice | Todo | - |
| T2.2.6 | Handle `item/permissions/requestApproval` → WaitingForChoice | Todo | - |
| T2.2.7 | Parse `ReviewDecision` response options | Todo | - |
| T2.2.8 | Write unit tests with Codex request samples | Todo | - |

#### Acceptance Criteria

- All approval requests detected as WaitingForChoice
- ReviewDecision options parsed correctly
- Codex is the provider needing most decision intervention

#### Technical Notes

Based on source code analysis (`../codex/codex-rs/app-server-protocol`):

```rust
/// Codex output classifier
pub struct CodexOutputClassifier;

impl OutputClassifier for CodexOutputClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Codex
    }
    
    fn classify(&self, event: &ProviderEvent) -> ProviderOutputType {
        // Codex uses ServerRequest pattern for approvals
        match event {
            // Approval requests = WaitingForChoice (Situation 1)
            ProviderEvent::CodexApprovalRequest { method, params } => {
                self.classify_approval_request(method, params)
            }
            
            // Running events
            ProviderEvent::AssistantChunk { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::ThinkingChunk { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::GenericToolCallStarted { .. } => ProviderOutputType::Running { event: event.clone() },
            
            // Finished
            ProviderEvent::Finished => ProviderOutputType::Finished {
                status: ProviderStatus::ClaimsCompletion {
                    summary: String::new(),
                    reflection_rounds: 0,
                },
            },
            
            // Error
            ProviderEvent::Error { message } => ProviderOutputType::Finished {
                status: ProviderStatus::Error {
                    error_type: ErrorType::Failure { message: message.clone() },
                },
            },
            
            _ => ProviderOutputType::Running { event: event.clone() },
        }
    }
    
    fn classify_approval_request(&self, method: &str, params: &Value) -> ProviderOutputType {
        match method {
            // Command execution approval
            "execCommandApproval" |
            "item/commandExecution/requestApproval" => {
                ProviderOutputType::Finished {
                    status: ProviderStatus::WaitingForChoice {
                        options: vec![
                            ChoiceOption { id: "approved".to_string(), label: "Approve".to_string() },
                            ChoiceOption { id: "approved_for_session".to_string(), label: "Approve for session".to_string() },
                            ChoiceOption { id: "denied".to_string(), label: "Deny".to_string() },
                            ChoiceOption { id: "abort".to_string(), label: "Abort".to_string() },
                        ],
                    },
                }
            }
            
            // File modification approval
            "applyPatchApproval" |
            "item/fileChange/requestApproval" => {
                ProviderOutputType::Finished {
                    status: ProviderStatus::WaitingForChoice {
                        options: vec![
                            ChoiceOption { id: "approved".to_string(), label: "Approve".to_string() },
                            ChoiceOption { id: "denied".to_string(), label: "Deny".to_string() },
                            ChoiceOption { id: "abort".to_string(), label: "Abort".to_string() },
                        ],
                    },
                }
            }
            
            // User input request
            "item/tool/requestUserInput" => {
                ProviderOutputType::Finished {
                    status: ProviderStatus::WaitingForChoice {
                        options: vec![
                            ChoiceOption { id: "input".to_string(), label: "Provide input".to_string() },
                        ],
                    },
                }
            }
            
            // Permission request
            "item/permissions/requestApproval" => {
                ProviderOutputType::Finished {
                    status: ProviderStatus::WaitingForChoice {
                        options: vec![
                            ChoiceOption { id: "approved".to_string(), label: "Approve".to_string() },
                            ChoiceOption { id: "denied".to_string(), label: "Deny".to_string() },
                        ],
                    },
                }
            }
            
            _ => ProviderOutputType::Running { event: ProviderEvent::Status(format!("Unknown Codex request: {}", method)) },
        }
    }
    
    fn extract_options(&self, event: &ProviderEvent) -> Option<Vec<ChoiceOption>> {
        match event {
            ProviderEvent::CodexApprovalRequest { method, .. } => {
                Some(self.get_approval_options(method))
            }
            _ => None,
        }
    }
    
    fn get_approval_options(&self, method: &str) -> Vec<ChoiceOption> {
        // Based on ReviewDecision type from Codex protocol
        match method {
            "execCommandApproval" => vec![
                ChoiceOption { id: "approved", label: "Approve" },
                ChoiceOption { id: "approved_for_session", label: "Approve for session" },
                ChoiceOption { id: "approved_execpolicy_amendment", label: "Approve with policy change" },
                ChoiceOption { id: "denied", label: "Deny" },
                ChoiceOption { id: "abort", label: "Abort" },
            ],
            "applyPatchApproval" => vec![
                ChoiceOption { id: "approved", label: "Approve" },
                ChoiceOption { id: "denied", label: "Deny" },
                ChoiceOption { id: "abort", label: "Abort" },
            ],
            _ => vec![],
        }
    }
}
```

**Codex ServerRequest Types** (from `ServerRequest.ts:18`):

| Method | Trigger | Options |
|--------|---------|---------|
| `execCommandApproval` | Command execution | approved, approved_for_session, denied, abort |
| `applyPatchApproval` | File modification | approved, denied, abort |
| `item/commandExecution/requestApproval` | V2 command | Same as execCommandApproval |
| `item/fileChange/requestApproval` | V2 file change | Same as applyPatchApproval |
| `item/tool/requestUserInput` | User input needed | Custom input |
| `item/permissions/requestApproval` | Permission | approved, denied |

---

### Story 2.3: ACP Output Classifier (OpenCode/Kimi)

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement unified classifier for ACP protocol used by OpenCode and Kimi-CLI.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Create `ACPOutputClassifier` struct | Todo | - |
| T2.3.2 | Implement `classify()` method for ACP notifications | Todo | - |
| T2.3.3 | Handle `permission.asked` → WaitingForChoice | Todo | - |
| T2.3.4 | Handle `session.status.idle` → ClaimsCompletion | Todo | - |
| T2.3.5 | Handle `session.status.retry` → Error if exhausted | Todo | - |
| T2.3.6 | Parse permission request patterns | Todo | - |
| T2.3.7 | Write unit tests with ACP notification samples | Todo | - |

#### Acceptance Criteria

- `permission.asked` detected as WaitingForChoice
- `session.status.idle` detected as ClaimsCompletion
- Retry exhaustion detected as Error
- Works for both OpenCode and Kimi-CLI

#### Technical Notes

Based on source code analysis (`../opencode/packages/opencode/src/permission/index.ts`, `../opencode/packages/opencode/src/session/status.ts`):

```rust
/// ACP protocol output classifier (OpenCode/Kimi-CLI)
pub struct ACPOutputClassifier;

impl OutputClassifier for ACPOutputClassifier {
    fn provider_kind(&self) -> ProviderKind {
        // Works for both OpenCode and Kimi
        ProviderKind::OpenCode // or ProviderKind::Kimi
    }
    
    fn classify(&self, event: &ProviderEvent) -> ProviderOutputType {
        match event {
            // ACP notification
            ProviderEvent::ACPNotification { method, params } => {
                self.classify_acp_notification(method, params)
            }
            
            // ACP error
            ProviderEvent::ACPError { code, message } => ProviderOutputType::Finished {
                status: ProviderStatus::Error {
                    error_type: ErrorType::Failure { 
                        message: format!("{}: {}", code, message) 
                    },
                },
            },
            
            // Running events
            ProviderEvent::AssistantChunk { .. } => ProviderOutputType::Running { event: event.clone() },
            ProviderEvent::ThinkingChunk { .. } => ProviderOutputType::Running { event: event.clone() },
            
            _ => ProviderOutputType::Running { event: event.clone() },
        }
    }
    
    fn classify_acp_notification(&self, method: &str, params: &Value) -> ProviderOutputType {
        match method {
            // Permission asked = WaitingForChoice (Situation 1)
            "permission.asked" => ProviderOutputType::Finished {
                status: ProviderStatus::WaitingForChoice {
                    options: self.parse_permission_options(params),
                },
            },
            
            // Session status
            "session.status" => {
                let status_type = params.get("status")
                    .and_then(|s| s.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("busy");
                
                match status_type {
                    // Idle = ClaimsCompletion (Situation 2)
                    "idle" => ProviderOutputType::Finished {
                        status: ProviderStatus::ClaimsCompletion {
                            summary: String::new(),
                            reflection_rounds: 0,
                        },
                    },
                    
                    // Retry exhausted = Error (Situation 4)
                    "retry" => {
                        let attempt = params.get("status")
                            .and_then(|s| s.get("attempt"))
                            .and_then(|a| a.as_u64())
                            .unwrap_or(0);
                        
                        if attempt > 3 {
                            ProviderOutputType::Finished {
                                status: ProviderStatus::Error {
                                    error_type: ErrorType::Failure {
                                        message: "Retry exhausted".to_string(),
                                    },
                                },
                            }
                        } else {
                            ProviderOutputType::Running { event: ProviderEvent::Status(format!("Retry attempt {}", attempt)) }
                        }
                    }
                    
                    // Busy = Running
                    "busy" => ProviderOutputType::Running { event: ProviderEvent::Status("ACP busy".to_string()) },
                    
                    _ => ProviderOutputType::Running { event: ProviderEvent::Status(format!("ACP status: {}", status_type)) },
                }
            },
            
            // Other = Running
            _ => ProviderOutputType::Running { event: ProviderEvent::Status(format!("ACP notification: {}", method)) },
        }
    }
    
    fn parse_permission_options(&self, params: &Value) -> Vec<ChoiceOption> {
        // ACP permission.asked format:
        // { "permission": "write", "patterns": [...], "always": [...] }
        vec![
            ChoiceOption { id: "once".to_string(), label: "Approve once".to_string() },
            ChoiceOption { id: "always".to_string(), label: "Always approve".to_string() },
            ChoiceOption { id: "reject".to_string(), label: "Reject".to_string() },
        ]
    }
    
    fn extract_options(&self, event: &ProviderEvent) -> Option<Vec<ChoiceOption>> {
        match event {
            ProviderEvent::ACPNotification { method, .. } if method == "permission.asked" => {
                Some(self.parse_permission_options(&serde_json::Value::Null))
            }
            _ => None,
        }
    }
}
```

**ACP Permission Event Format** (from `permission/index.ts:43-61`):

```typescript
// permission.asked params
{
  id: PermissionID,
  sessionID: SessionID,
  permission: string,       // "write", "edit", "exec"
  patterns: string[],       // File paths
  metadata: object,
  always: string[],         // "always allow" patterns
  tool?: { messageID, callID }
}

// Reply types
type Reply = "once" | "always" | "reject"
```

---

### Story 2.4: Unified OutputClassifierRegistry

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Create registry for provider-specific classifiers with fallback support.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Create `OutputClassifier` trait | Todo | - |
| T2.4.2 | Create `OutputClassifierRegistry` struct | Todo | - |
| T2.4.3 | Register Claude classifier | Todo | - |
| T2.4.4 | Register Codex classifier | Todo | - |
| T2.4.5 | Register ACP classifier for OpenCode | Todo | - |
| T2.4.6 | Register ACP classifier for Kimi | Todo | - |
| T2.4.7 | Implement fallback classifier for unknown providers | Todo | - |
| T2.4.8 | Write unit tests for registry dispatch | Todo | - |

#### Acceptance Criteria

- Registry dispatches to correct classifier by provider
- Unknown providers handled gracefully
- All classifiers implement OutputClassifier trait

#### Technical Notes

```rust
/// Output classifier trait
pub trait OutputClassifier: Send + Sync {
    /// Provider kind this classifier handles
    fn provider_kind(&self) -> ProviderKind;
    
    /// Classify provider event
    fn classify(&self, event: &ProviderEvent) -> ProviderOutputType;
    
    /// Extract choice options (if WaitingForChoice)
    fn extract_options(&self, event: &ProviderEvent) -> Option<Vec<ChoiceOption>>;
    
    /// Extract completion summary (if ClaimsCompletion)
    fn extract_completion_summary(&self, event: &ProviderEvent) -> Option<String>;
}

/// Registry for provider-specific classifiers
pub struct OutputClassifierRegistry {
    /// Provider-specific classifiers
    classifiers: HashMap<ProviderKind, Box<dyn OutputClassifier>>,
    
    /// Fallback classifier for unknown providers
    fallback: Box<dyn OutputClassifier>,
}

impl OutputClassifierRegistry {
    pub fn new() -> Self {
        let mut classifiers = HashMap::new();
        
        // Register provider-specific classifiers
        classifiers.insert(ProviderKind::Claude, Box::new(ClaudeOutputClassifier));
        classifiers.insert(ProviderKind::Codex, Box::new(CodexOutputClassifier));
        classifiers.insert(ProviderKind::OpenCode, Box::new(ACPOutputClassifier));
        classifiers.insert(ProviderKind::Kimi, Box::new(ACPOutputClassifier));
        
        Self {
            classifiers,
            fallback: Box::new(FallbackClassifier),
        }
    }
    
    pub fn classify(&self, provider: ProviderKind, event: &ProviderEvent) -> ProviderOutputType {
        match self.classifiers.get(&provider) {
            Some(classifier) => classifier.classify(event),
            None => self.fallback.classify(event),
        }
    }
    
    pub fn extract_options(&self, provider: ProviderKind, event: &ProviderEvent) -> Option<Vec<ChoiceOption>> {
        match self.classifiers.get(&provider) {
            Some(classifier) => classifier.extract_options(event),
            None => self.fallback.extract_options(event),
        }
    }
}

/// Fallback classifier for unknown providers
pub struct FallbackClassifier;

impl OutputClassifier for FallbackClassifier {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Mock // Placeholder
    }
    
    fn classify(&self, event: &ProviderEvent) -> ProviderOutputType {
        match event {
            ProviderEvent::Finished => ProviderOutputType::Finished {
                status: ProviderStatus::ClaimsCompletion {
                    summary: String::new(),
                    reflection_rounds: 0,
                },
            },
            ProviderEvent::Error { message } => ProviderOutputType::Finished {
                status: ProviderStatus::Error {
                    error_type: ErrorType::Failure { message: message.clone() },
                },
            },
            _ => ProviderOutputType::Running { event: event.clone() },
        }
    }
    
    fn extract_options(&self, _event: &ProviderEvent) -> Option<Vec<ChoiceOption>> {
        None
    }
    
    fn extract_completion_summary(&self, _event: &ProviderEvent) -> Option<String> {
        None
    }
}
```

---

## Provider Decision Trigger Summary

Based on source code research:

| Provider | Waiting for Choice | Completion | Error |
|----------|-------------------|------------|-------|
| **Claude** | None (bypassPermissions) | `Finished` event | `Error` event |
| **Codex** | `execCommandApproval`, `applyPatchApproval`, `requestUserInput` | No explicit marker | `timed_out`, `abort` |
| **ACP (OpenCode/Kimi)** | `permission.asked` | `session.status.idle` | No explicit ACP error |

**Key Insights**:

1. **Claude**: Least decision intervention needed - only claims completion and errors
2. **Codex**: Most decision intervention - many approval requests
3. **ACP**: Standardized protocol with clear waiting markers

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Protocol changes in providers | Low | Medium | Sample collection, version detection |
| Missing edge cases | Medium | Low | Comprehensive test samples |
| Classifier performance | Low | Low | Benchmark tests |

## Sprint Deliverables

- `decision/src/classifier.rs` - OutputClassifier trait
- `decision/src/claude_classifier.rs` - Claude classifier
- `decision/src/codex_classifier.rs` - Codex classifier
- `decision/src/acp_classifier.rs` - ACP classifier
- Unit tests with real provider event samples

## Dependencies

- Sprint 1: Core Types (ProviderStatus, ProviderOutputType)

## Next Sprint

After completing this sprint, proceed to [Sprint 3: Decision Engine](./sprint-03-decision-engine.md) for decision engine implementations.