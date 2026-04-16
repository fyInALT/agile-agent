# Sprint 3: Provider Integration & UI Overlay

## Metadata

- Sprint ID: `launch-config-sprint-03`
- Title: `Provider Integration & UI Overlay`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: Sprint 1, Sprint 2
- Design Reference: `docs/superpowers/specs/2026-04-16-agent-launch-config-and-resume-design.md`

## Sprint Goal

Integrate ResolvedLaunchSpec into provider execution layer, refactor provider startup to consume structured launch configuration, and implement the Launch Config Overlay UI for user input collection with real-time preview.

## Stories

### Story 3.1: ProviderLaunchContext Definition

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define the context struct passed to provider startup functions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.1.1 | Define `ProviderLaunchContext` struct | Todo | - |
| T3.1.2 | Add `spec: ResolvedLaunchSpec` field | Todo | - |
| T3.1.3 | Add `cwd: PathBuf` field | Todo | - |
| T3.1.4 | Add `session_handle: Option<SessionHandle>` field | Todo | - |
| T3.1.5 | Add convenience constructors | Todo | - |
| T3.1.6 | Write unit tests for context construction | Todo | - |

#### Technical Design

```rust
pub struct ProviderLaunchContext {
    pub spec: ResolvedLaunchSpec,
    pub cwd: PathBuf,
    pub session_handle: Option<SessionHandle>,
}

impl ProviderLaunchContext {
    pub fn new(spec: ResolvedLaunchSpec, cwd: PathBuf) -> Self {
        Self {
            spec,
            cwd,
            session_handle: None,
        }
    }
    
    pub fn with_session_handle(mut self, handle: SessionHandle) -> Self {
        self.session_handle = Some(handle);
        self
    }
}
```

---

### Story 3.2: Claude Provider Refactor

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Refactor Claude provider to accept ProviderLaunchContext instead of implicit environment.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.2.1 | Modify `claude::start()` signature to accept `ProviderLaunchContext` | Todo | - |
| T3.2.2 | Use `spec.resolved_executable_path` instead of env lookup | Todo | - |
| T3.2.3 | Build Command with `spec.effective_env` | Todo | - |
| T3.2.4 | Inject `spec.extra_args` before protocol args | Todo | - |
| T3.2.5 | Keep protocol args under provider control | Todo | - |
| T3.2.6 | Update logging to use resolved values | Todo | - |
| T3.2.7 | Write provider integration tests | Todo | - |
| T3.2.8 | Verify backward compatibility | Todo | - |

#### Refactored Signature

```rust
// Old signature (deprecated)
pub fn start(
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()>;

// New signature
pub fn start_with_context(
    context: ProviderLaunchContext,
    prompt: String,
    event_tx: Sender<ProviderEvent>,
) -> Result<()>;
```

#### Execution Changes

```rust
fn run_claude_with_context(
    context: &ProviderLaunchContext,
    prompt: String,
    event_tx: &Sender<ProviderEvent>,
) -> Result<()> {
    let args = build_claude_args(context.session_handle.clone());
    
    // Inject extra args BEFORE protocol args
    let full_args: Vec<String> = context.spec.extra_args.iter()
        .chain(args.iter())
        .cloned()
        .collect();
    
    let mut command = Command::new(&context.spec.resolved_executable_path);
    command.args(&full_args);
    command.current_dir(&context.cwd);
    
    // Use effective_env instead of implicit process environment
    for (key, value) in &context.spec.effective_env {
        command.env(key, value);
    }
    
    command.stdin(Stdio::piped());
    // ...
}
```

---

### Story 3.3: Codex Provider Refactor

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Refactor Codex provider to accept ProviderLaunchContext.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.3.1 | Modify `codex::start()` signature | Todo | - |
| T3.3.2 | Use resolved executable path | Todo | - |
| T3.3.3 | Build Command with effective_env | Todo | - |
| T3.3.4 | Inject extra args | Todo | - |
| T3.3.5 | Keep protocol args under provider control | Todo | - |
| T3.3.6 | Update logging | Todo | - |
| T3.3.7 | Write provider integration tests | Todo | - |
| T3.3.8 | Verify backward compatibility | Todo | - |

---

### Story 3.4: Provider Entry Point Refactor

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Update `core/src/provider.rs` entry points to use ProviderLaunchContext.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.4.1 | Add `start_provider_with_context()` | Todo | - |
| T3.4.2 | Add `start_provider_with_handle_and_context()` | Todo | - |
| T3.4.3 | Keep old functions for backward compatibility | Todo | - |
| T3.4.4 | Generate default context from host env for old callers | Todo | - |
| T3.4.5 | Update internal callers to new signature | Todo | - |
| T3.4.6 | Write integration tests | Todo | - |

#### New Entry Points

```rust
pub fn start_provider_with_context(
    context: ProviderLaunchContext,
    prompt: String,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    match context.spec.provider {
        ProviderKind::Mock => start_mock_provider(prompt, event_tx),
        ProviderKind::Claude => claude::start_with_context(context, prompt, event_tx),
        ProviderKind::Codex => codex::start_with_context(context, prompt, event_tx),
    }
}

pub fn start_provider_with_handle_and_context(
    context: ProviderLaunchContext,
    prompt: String,
    thread_name: String,
) -> Result<ProviderThreadHandle> {
    // ...
}

// Backward compatibility: generate default context
pub fn start_provider(
    provider: ProviderKind,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    let spec = resolve_launch_spec(&LaunchInputSpec::host_default(provider))?;
    let context = ProviderLaunchContext::new(spec, cwd)
        .with_opt_session_handle(session_handle);
    start_provider_with_context(context, prompt, event_tx)
}
```

---

### Story 3.5: Decision Agent LLMCaller Configuration

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Configure decision agent LLMCaller from decision_resolved.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.5.1 | Add `from_resolved_launch_spec()` to LLMCaller config | Todo | - |
| T3.5.2 | Map provider-specific env to LLMCaller settings | Todo | - |
| T3.5.3 | Inject into DecisionAgentSlot on creation | Todo | - |
| T3.5.4 | Update decision crate integration | Todo | - |
| T3.5.5 | Write integration tests | Todo | - |

#### Decision Agent Integration

```rust
impl DecisionAgentSlot {
    pub fn with_launch_bundle(
        work_agent_id: String,
        launch_bundle: &AgentLaunchBundle,
        mail_receiver: DecisionMailReceiver,
        cwd: PathBuf,
        components: &DecisionLayerComponents,
    ) -> Self {
        // Create LLMCaller from decision_resolved
        let caller = create_llm_caller_from_spec(&launch_bundle.decision_resolved);
        
        let mut slot = Self::new(...);
        slot.set_llm_caller(caller);
        slot
    }
}

fn create_llm_caller_from_spec(spec: &ResolvedLaunchSpec) -> Arc<dyn LLMCaller> {
    // Extract API key/base URL from effective_env
    // Create appropriate caller based on provider
}
```

---

### Story 3.6: LaunchConfigOverlay UI Component

**Priority**: P0
**Effort**: 5 points
**Status**: Backlog

Create overlay UI for collecting launch configuration input.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.6.1 | Create `tui/src/launch_config_overlay.rs` | Todo | - |
| T3.6.2 | Define `LaunchConfigOverlayState` struct | Todo | - |
| T3.6.3 | Implement provider display area (read-only) | Todo | - |
| T3.6.4 | Implement Work Agent Config text area | Todo | - |
| T3.6.5 | Implement Decision Agent Config text area | Todo | - |
| T3.6.6 | Implement parse preview area | Todo | - |
| T3.6.7 | Implement error message area | Todo | - |
| T3.6.8 | Implement render function | Todo | - |
| T3.6.9 | Write component tests | Todo | - |

#### UI Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ Launch Configuration                                             │
├─────────────────────────────────────────────────────────────────┤
│ Provider: claude (locked)                                        │
│                                                                  │
│ Work Agent Config:                                               │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ ANTHROPIC_BASE_URL=https://api.example.com                  │ │
│ │ ANTHROPIC_MODEL=claude-opus                                  │ │
│ │                                                              │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│ Decision Agent Config:                                           │
│ ┌─────────────────────────────────────────────────────────────┐ │
│ │ ANTHROPIC_MODEL=claude-haiku                                 │ │
│ │                                                              │ │
│ └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│ Preview:                                                         │
│   Work: provider=claude, exec=/usr/bin/claude, env=2, args=0    │
│   Decision: provider=claude, source=env-only, env=1             │
│                                                                  │
│ [Enter] Confirm  [Esc] Cancel                                   │
└─────────────────────────────────────────────────────────────────┘
```

---

### Story 3.7: Input Parse Real-Time Preview

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Parse input on every keystroke and display preview.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.7.1 | Implement `preview_work_config()` function | Todo | - |
| T3.7.2 | Implement `preview_decision_config()` function | Todo | - |
| T3.7.3 | Call parser on input change | Todo | - |
| T3.7.4 | Display parsed result in preview area | Todo | - |
| T3.7.5 | Handle parse errors gracefully | Todo | - |
| T3.7.6 | Show env count, arg count, executable | Todo | - |
| T3.7.7 | Write preview tests | Todo | - |

#### Preview Implementation

```rust
pub struct ParsePreview {
    pub source_mode: LaunchSourceMode,
    pub env_count: usize,
    pub arg_count: usize,
    pub executable: Option<String>,
    pub error: Option<String>,
}

impl LaunchConfigOverlayState {
    fn update_preview(&mut self) {
        self.work_preview = parse_and_preview(
            self.provider,
            &self.work_config_text,
        );
        self.decision_preview = parse_and_preview(
            self.provider,
            &self.decision_config_text,
        );
    }
}
```

---

### Story 3.8: Error Display and Input Preservation

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Handle parse errors without closing overlay, preserve original input.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.8.1 | Show error message in error area | Todo | - |
| T3.8.2 | Keep overlay open on parse error | Todo | - |
| T3.8.3 | Preserve user input text on error | Todo | - |
| T3.8.4 | Allow user to correct and retry | Todo | - |
| T3.8.5 | Clear error on successful parse | Todo | - |
| T3.8.6 | Write error handling tests | Todo | - |

---

### Story 3.9: Mock Provider Shortcut

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Skip config overlay for Mock provider selection.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.9.1 | Detect Mock provider selection | Todo | - |
| T3.9.2 | Skip launch config overlay entirely | Todo | - |
| T3.9.3 | Create agent immediately with default config | Todo | - |
| T3.9.4 | Write shortcut tests | Todo | - |

#### Implementation

```rust
// In provider selection handling
match selected_provider {
    ProviderKind::Mock => {
        // Skip overlay, create immediately
        state.spawn_agent(ProviderKind::Mock)?;
    }
    ProviderKind::Claude | ProviderKind::Codex => {
        // Open launch config overlay
        state.open_launch_config_overlay(selected_provider);
    }
}
```

---

### Story 3.10: TUI Integration Tests

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Comprehensive TUI integration tests for Ctrl+N flow.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T3.10.1 | Test Ctrl+N opens provider selection | Todo | - |
| T3.10.2 | Test Claude selection opens config overlay | Todo | - |
| T3.10.3 | Test Codex selection opens config overlay | Todo | - |
| T3.10.4 | Test Mock selection skips overlay | Todo | - |
| T3.10.5 | Test valid config creates agent | Todo | - |
| T3.10.6 | Test parse error keeps overlay open | Todo | - |
| T3.10.7 | Test Esc cancels overlay | Todo | - |
| T3.10.8 | Test preview updates on input | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Provider refactor breaks existing callers | Medium | High | Keep old signatures, gradual migration |
| UI overlay responsiveness | Low | Medium | Async/incremental parsing |
| Decision crate integration complexity | Medium | Medium | Early design review with decision crate |

## Sprint Deliverables

- `core/src/provider.rs` refactored with ProviderLaunchContext
- `core/src/providers/claude.rs` refactored
- `core/src/providers/codex.rs` refactored
- `tui/src/launch_config_overlay.rs` - New UI component
- Modified `tui/src/ui_state.rs` - Overlay state management
- Decision agent LLMCaller configuration integration
- Complete TUI integration tests

## Dependencies

- Sprint 1: LaunchInputSpec, ResolvedLaunchSpec, parsers
- Sprint 2: Resolver, persistence
- Existing: Provider crates, Decision crate, TUI framework

## Module Structure

```
core/src/
├── provider.rs           # Modified: ProviderLaunchContext, new entry points
├── providers/
│   ├── claude.rs         # Modified: start_with_context()
│   └── codex.rs          # Modified: start_with_context()
└── launch_config/
    ├── ...

tui/src/
├── launch_config_overlay.rs  # NEW: Overlay UI component
├── ui_state.rs               # Modified: Overlay state
└── provider_overlay.rs       # Existing: Provider selection (unchanged)
```

## Next Sprint

After completing this sprint, proceed to [Sprint 4: Resume Integration & Error Handling](./sprint-4-resume-integration.md).
