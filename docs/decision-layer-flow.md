# Decision Layer Flow

This document describes the complete flow of the decision layer in agile-agent,
including trigger conditions, classification logic, engine selection, and action execution.

## Overview

The decision layer is responsible for handling situations where a work agent needs
external guidance to proceed. It uses a tiered decision engine architecture that
selects the appropriate engine based on situation complexity.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Decision Layer Flow                              │
└─────────────────────────────────────────────────────────────────────────┘

  Provider Event (Finished/Error/etc)
         │
         ▼
  ┌──────────────────────┐
  │ handle_agent_provider │  (app_loop.rs:1740)
  │ _event()              │
  └──────────────────────┘
         │
         ├─► [1] Process event content (transcript, status updates)
         │
         ├─► [2] On Finished: agent → idle
         │
         ▼
  ┌──────────────────────┐
  │ classify_event()     │  (agent_pool.rs:1296)
  │                      │
  │ Find slot → get provider_kind
  │ Convert event → decision event
  │ classifier_registry.classify()
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────────────────┐
  │ ClassifyResult                    │
  │ ├─ Running { context_update }     │ → No decision needed
  │ └─ NeedsDecision { situation_type }│ → Decision required
  └──────────────────────────────────┘
         │
         │ if is_needs_decision()
         ▼
  ┌──────────────────────┐
  │ send_decision_request│  (agent_pool.rs:1330)
  │                      │
  │ Create DecisionRequest
  │ Send via mail sender
  └──────────────────────┘
         │
         ├─► [3] agent → blocked_for_decision
         │
         ▼
  ┌──────────────────────┐
  │ poll_decision_agents │  (app_loop.rs:129, periodic polling)
  │                      │
  │ decision_agent.poll_and_process()
  │   ├─► try_receive_request()
  │   ├─► process_request()
  │   │     └─► TieredDecisionEngine.decide()
  │   │           ├─► select_tier()
  │   │           ├─► select_engine()
  │   │           └─► engine.decide()
  │   └─► send_response()
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────┐
  │ execute_decision_action│  (agent_pool.rs:1414)
  │                       │
  │ Execute based on action_type:
  │ ├─► select_option: Process human_queue
  │ ├─► skip: Skip current task
  │ ├─► request_human: Stay blocked
  │ ├─► custom_instruction: Add user message
  │ ├─► continue: agent → idle
  │ ├─► reflect: Add reflection prompt → idle
  │ ├─► confirm_completion: Confirm completion → idle
  │ ├─► retry: Add retry prompt → idle
  │ └─► unknown: Cancelled
  └──────────────────────┘
```

## Components

### 1. Decision Mail System

Located in `core/src/decision_mail.rs`. Provides thread-safe communication between
work agents and decision agents.

```
┌─────────────────────┐     ┌─────────────────────────────┐
│   Work Agent Slot    │     │   Decision Agent Slot       │
│                      │     │                              │
│  Sends:              │     │  Receives:                   │
│  DecisionRequest     │────▶│ DecisionRequest              │
│                      │     │                              │
│  Receives:           │     │  Sends:                      │
│  DecisionResponse    │◀────│ DecisionResponse             │
└─────────────────────┘     └─────────────────────────────┘
```

### 2. Classifier Registry

Located in `decision/src/classifier_registry.rs`. Dispatches classification to
provider-specific classifiers.

| Provider | Classifier | Finished Event → Situation |
|----------|------------|----------------------------|
| Claude | `ClaudeClassifier` | `claude_finished` |
| Codex | `CodexClassifier` | `claims_completion` |
| Unknown | `FallbackClassifier` | `claims_completion` |

### 3. Tiered Decision Engine

Located in `decision/src/tiered_engine.rs`. Selects engine based on situation complexity.

| Tier | Description | Engine | Situations |
|------|-------------|--------|------------|
| Simple | Well-known patterns | RuleBased | waiting_for_choice |
| Medium | Standard decisions | LLM | claims_completion, claude_finished |
| Complex | Error recovery | LLM | error, partial_completion |
| Critical | Human intervention | CLI | requires_human=true |

### 4. Rule-Based Engine

Located in `decision/src/rule_engine.rs`. Contains built-in rules for simple situations.

| Rule Name | Condition | Action | Priority |
|-----------|-----------|--------|----------|
| approve-first | waiting_for_choice | select_first | Medium |
| reflect-first | claims_completion + reflection_rounds(0,1) | reflect | High |
| retry-error | error | retry | Medium |

## Action Execution

The `execute_decision_action` function in `agent_pool.rs` handles each action type:

| Action | Agent Status | Transcript | Additional Operations |
|--------|--------------|------------|----------------------|
| `select_option` | Depends on human_queue | - | Process HumanDecisionResponse |
| `skip` | idle | - | Skip current task |
| `request_human` | blocked | - | No change (awaiting human) |
| `custom_instruction` | Depends on prior state | User(instruction) | - |
| `continue` | idle | - | - |
| `reflect` | idle | User("Reflect: ...") | Add reflection prompt |
| `confirm_completion` | idle | - | Log completion |
| `retry` | idle | User(prompt) | Add retry prompt |

## Built-in Situations

Defined in `decision/src/builtin_situations.rs`:

### WaitingForChoiceSituation

- Requires human input if `critical=true`
- Available actions: `select_option`, `select_first`, `reject_all`, `custom_instruction`

### ClaimsCompletionSituation

- Agent claims task completion, needs verification
- Reflection rounds track verification iterations
- Available actions: `reflect` (if rounds < max), `confirm_completion`, `request_human`

### ErrorSituation

- Agent encountered an error
- Available actions: `retry`, `request_human`, `abort`

### PartialCompletionSituation

- Task partially complete with blocker
- Available actions: `continue`, `skip`, `request_human`

## Built-in Actions

Defined in `decision/src/builtin_actions.rs`:

### SelectOptionAction

Selects a specific option from choices.

```rust
SelectOptionAction {
    option_id: "A",
    reason: "Best option for the task"
}
```

### ReflectAction

Triggers agent to verify its work.

```rust
ReflectAction {
    prompt: "Please verify your work is complete."
}
```

### ConfirmCompletionAction

Confirms task completion.

```rust
ConfirmCompletionAction {
    submit_pr: false,
    next_task_id: None
}
```

### RetryAction

Retries the previous action with optional cooldown.

```rust
RetryAction {
    prompt: "Retry with adjusted approach",
    cooldown_ms: 1000,
    adjusted: true
}
```

## Flow Example: Agent Finished

1. Claude provider sends `ProviderEvent::Finished`
2. `handle_agent_provider_event()` processes the event
3. Agent status transitions to `idle`
4. `classify_event()` returns `NeedsDecision { situation_type: "claude_finished" }`
5. `send_decision_request()` creates and sends request
6. Agent transitions to `blocked_for_decision`
7. `poll_decision_agents()` receives request
8. `TieredDecisionEngine` selects Medium tier → LLM engine
9. LLM engine returns `ReflectAction` (first round)
10. `execute_decision_action()` adds reflection prompt to transcript
11. Agent transitions to `idle`, ready to process reflection

## Key Implementation Details

### Decision Agent Spawning

Decision agents are spawned for each work agent at creation time
(`spawn_agent()` and `spawn_agent_with_worktree()`). This ensures
every work agent has a paired decision agent ready to handle requests.

```rust
// In spawn_agent() and spawn_agent_with_worktree()
if provider_kind != ProviderKind::Mock {
    self.spawn_decision_agent_for(&agent_id)?;
}
```

### Blocking State Management

When a decision is requested, the agent enters `blocked_for_decision` state.
This prevents the agent from processing new tasks while awaiting decision.

After action execution, most actions transition the agent back to `idle`,
except `request_human` which keeps the agent blocked awaiting human input.

### Mail Channel Architecture

Uses `std::sync::mpsc` channels for thread-safe communication:

- `DecisionMailSender`: Used by work agent to send requests, receive responses
- `DecisionMailReceiver`: Used by decision agent to receive requests, send responses

The split design allows each side to operate independently without blocking.

## Error Handling

| Error Condition | Handling |
|-----------------|----------|
| No decision agent for work agent | Log warning, request fails |
| Decision engine error | Fallback to configured fallback tier |
| Unknown action type | Log warning, return Cancelled |
| Agent not blocked | Return NotBlocked result |

## Configuration

The tiered engine configuration in `TieredEngineConfig`:

```rust
TieredEngineConfig {
    llm_provider: ProviderKind::Claude,
    llm_config: LLMEngineConfig::default(),
    use_cli_for_critical: true,  // Use CLI for human decisions
    fallback_tier: DecisionTier::Medium,  // Fallback on engine failure
}
```

## Related Files

| File | Purpose |
|------|---------|
| `core/src/decision_mail.rs` | Mail channel types |
| `core/src/decision_agent_slot.rs` | Decision agent runtime slot |
| `core/src/agent_pool.rs` | Pool management, request sending, action execution |
| `decision/src/classifier.rs` | Classifier trait and result types |
| `decision/src/classifier_registry.rs` | Provider-specific dispatch |
| `decision/src/claude_classifier.rs` | Claude-specific classification |
| `decision/src/codex_classifier.rs` | Codex-specific classification |
| `decision/src/tiered_engine.rs` | Tiered engine selection |
| `decision/src/rule_engine.rs` | Rule-based decisions |
| `decision/src/builtin_situations.rs` | Built-in situation definitions |
| `decision/src/builtin_actions.rs` | Built-in action implementations |
| `tui/src/app_loop.rs` | Event handling, decision polling |
