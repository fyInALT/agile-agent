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
| continue-on-idle | agent_idle | continue_all_tasks | Medium |

## Idle State Decision Trigger

When work agents become idle (no active provider output), the decision layer is
automatically triggered to determine whether to continue working or stop.

### Trigger Conditions

1. **Idle Timeout**: Agent in Responding state with no events for `RESPONDING_IDLE_TIMEOUT_SECS` (5s)
2. **Idle Check**: Agent in Idle state for `IDLE_DECISION_TRIGGER_SECS` (60s)

### Trigger Flow

```
Agent becomes idle (no provider output)
         │
         ▼
  ┌──────────────────────┐
  │ check_idle_agents_   │  (every tick)
  │ for_decision()       │
  └──────────────────────┘
         │
         │ elapsed >= IDLE_DECISION_TRIGGER_SECS
         ▼
  ┌──────────────────────┐
  │ trigger_decision_    │
  │ for_idle_agent()     │
  └──────────────────────┘
         │
         ▼
  ┌──────────────────────┐
  │ AgentIdleSituation   │
  │ created and sent     │
  └──────────────────────┘
         │
         ▼
  Decision: continue_all_tasks or stop_if_complete
```

### Decision Logic

- **Default**: `continue_all_tasks` - Agent receives "continue finish all tasks" instruction
- **Stop condition**: Only when decision layer confirms all tasks complete (kanban/backlog empty)

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
| `continue_all_tasks` | idle | User(instruction) | Send "continue finish all tasks" |
| `stop_if_complete` | stopped | - | Stop agent (tasks complete) |

## Git Flow Task Preparation

The decision layer now includes a task preparation phase that ensures proper Git workflow
conventions when agents start new tasks.

### Overview

Before a work agent begins a new task, the decision layer can trigger a preparation phase
that:

1. Extracts task metadata (branch name, type, summary)
2. Analyzes current Git state (uncommitted changes, branch status)
3. Handles uncommitted changes (commit, stash, or prompt user)
4. Syncs with baseline branch (main/master)
5. Creates or switches to task-specific branch

### Trigger Conditions

Task preparation is triggered when:

1. Agent receives a new task assignment
2. Agent is idle with no valid feature branch
3. Agent's current branch doesn't match task metadata
4. Agent workspace health score is below threshold

### Preparation Flow

```
Task Assignment
       │
       ▼
┌─────────────────────┐
│ TaskStarting        │
│ Situation created   │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│ TaskPreparation     │
│ Pipeline            │
│ ├─ Extract metadata │
│ ├─ Analyze Git state│
│ ├─ Handle uncommitted│
│ └─ Setup branch     │
└─────────────────────┘
       │
       ├─► Ready → Agent starts task
       ├─► NeedsUncommitted → Handle changes first
       ├─► NeedsSync → Rebase/sync first
       ├─► NeedsHuman → Block for user input
       └─► Failed → Report error
```

### TaskStartingSituation

- Triggered when agent is assigned a new task
- Contains task description, metadata, and Git state
- Available actions: `prepare_task_start`, `create_task_branch`, `rebase_to_main`, `request_human`

### PrepareTaskStartAction

- Executes full preparation pipeline
- Handles uncommitted changes based on policy
- Creates feature branch from baseline
- Returns preparation result with branch info

### Configuration

Git Flow task preparation uses `GitFlowConfig` in `core/src/git_flow_config.rs`:

```rust
GitFlowConfig {
    base_branch: "main",                // Base branch for sync
    branch_pattern: "<type>/<task-id>-<desc>",  // Branch naming
    auto_sync_baseline: true,           // Auto fetch/sync
    auto_stash_changes: false,          // Auto stash uncommitted
    auto_cleanup_merged: true,          // Cleanup merged branches
    stale_branch_days: 30,              // Stale branch warning threshold
}
```

### Integration Points

| File | Purpose |
|------|---------|
| `decision/src/task_preparation.rs` | Task preparation pipeline orchestrator |
| `decision/src/task_metadata.rs` | Task metadata extraction and branch naming |
| `decision/src/git_state.rs` | Git state analysis (branch, uncommitted, conflicts) |
| `decision/src/uncommitted_handler.rs` | Uncommitted changes classification and handling |
| `core/src/git_flow_executor.rs` | Git operations executor |
| `core/src/git_flow_config.rs` | Git Flow configuration |
| `core/src/agent_pool.rs` | Integration with task assignment flow |

### Error Handling

| Error Condition | Handling |
|-----------------|----------|
| Uncommitted changes requiring user input | Return NeedsHuman, block agent |
| Rebase conflicts detected | Return NeedsHuman for resolution |
| Network failure during sync | Use local baseline, log warning |
| Branch collision detected | Auto-suffix or prompt user |
| Workspace health below threshold | Block agent, report issues |

### Logging Events

Git Flow preparation produces structured logs with prefix `git_flow.preparation.`:

| Event | Description | Key Fields |
|-------|-------------|------------|
| `git_flow.preparation.started` | Preparation started for task | `task_id`, `suggested_branch` |
| `git_flow.preparation.metadata` | Task metadata extracted | `task_id`, `branch`, `task_type` |
| `git_flow.preparation.health_checked` | Workspace health checked | `score`, `issues` |
| `git_flow.preparation.completed` | Preparation completed successfully | `branch`, `base_commit`, `warnings` |

## Built-in Situations

Defined in `decision/src/builtin_situations.rs`:

### WaitingForChoiceSituation

- Requires human input if `critical=true`
- Available actions: `select_option`, `select_first`, `reject_all`, `custom_instruction`

### ClaimsCompletionSituation

- Agent claims task completion, needs verification
- Reflection rounds track verification iterations
- Available actions: `reflect` (if rounds < max), `confirm_completion`, `request_human`

### AgentIdleSituation

- Triggered when agent enters idle state (no active provider output)
- Decision layer determines whether to continue or stop
- Fields: `trigger_reason`, `has_assigned_task`, `idle_duration_secs`
- Available actions: `continue_all_tasks`, `stop_if_complete`, `request_human`
- Tier: Simple (rule engine handles with `continue-on-idle` rule)

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

### ContinueAllTasksAction

Sends instruction to continue working on all pending tasks.

```rust
ContinueAllTasksAction {
    instruction: "continue finish all tasks"
}
```

### StopIfCompleteAction

Instructs agent to stop when all tasks are confirmed complete.

```rust
StopIfCompleteAction {
    reason: "All tasks complete"
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

## Logging Events

The decision layer produces structured JSON logs for debugging. All events use `logging::debug_event`
with the prefix `decision_layer.`.

### Trigger Events

| Event | Description | Key Fields |
|-------|-------------|------------|
| `decision_layer.triggered` | Decision triggered by work agent event | `decision_agent_id`, `work_agent_id`, `situation_type`, `situation_prompt`, `available_actions`, `requires_human` |
| `decision_layer.request_sent` | Request sent to decision agent | `work_agent_id`, `situation_type`, `situation_prompt` |

### Execution Events

| Event | Description | Key Fields |
|-------|-------------|------------|
| `decision_layer.prompt_sent` | Prompt sent to decision engine | `decision_agent_id`, `work_agent_id`, `prompt_length`, `prompt_preview` |
| `decision_layer.engine_response` | Decision engine response received | `decision_agent_id`, `work_agent_id`, `action_types`, `action_params`, `reasoning`, `confidence` |
| `decision_layer.response_sent` | Response sent to work agent | `decision_agent_id`, `work_agent_id`, `status` |

### Action Execution Events

| Event | Description | Key Fields |
|-------|-------------|------------|
| `decision_layer.action_executing` | Starting action execution | `work_agent_id`, `action_type`, `action_params`, `reasoning`, `confidence` |
| `decision_layer.action_completed` | Action completed successfully | `work_agent_id`, `action_type`, additional context varies by action |
| `decision_layer.work_agent_prompt` | Prompt/instruction sent to work agent | `work_agent_id`, `prompt_type`, `prompt`, `agent_status_after` |

### Error and Termination Events

| Event | Description | Key Fields |
|-------|-------------|------------|
| `decision_layer.engine_error` | Decision engine returned error | `decision_agent_id`, `work_agent_id`, `error`, `situation_type` |
| `decision_layer.response_send_failed` | Failed to send response to work agent | `decision_agent_id`, `work_agent_id`, `error` |
| `decision_layer.terminated` | Decision agent stopped | `decision_agent_id`, `reason`, `total_decisions`, `total_errors` |
| `decision_layer.reset` | Decision agent reset from error | `decision_agent_id` |
| `decision_layer.no_decision_agent` | No decision agent for work agent | `work_agent_id` |
| `decision_layer.request_send_failed` | Failed to send request to decision agent | `work_agent_id`, `situation_type`, `error` |
| `decision_layer.unknown_action` | Unknown action type | `work_agent_id`, `action_type`, `action_params` |
| `decision_layer.no_actions` | No actions in decision output | `work_agent_id`, `reasoning` |

### Example Log Flow

```
1. decision_layer.triggered       → Agent finished, needs decision
2. decision_layer.request_sent    → Request queued for decision agent
3. decision_layer.prompt_sent     → Decision engine processing
4. decision_layer.engine_response → Decision made: reflect action
5. decision_layer.response_sent   → Response delivered to work agent
6. decision_layer.action_executing → Executing reflect action
7. decision_layer.work_agent_prompt → "Reflect: Please verify..." sent
```

### Viewing Logs

Logs are written to JSON files in the workplace directory:

```bash
# Find the log file
cat ~/.local/share/agile-agent/workplaces/<workplace-id>/logs/*.json | grep decision_layer

# Pretty print decision layer logs
jq 'select(.event | startswith("decision_layer"))' ~/.local/share/agile-agent/workplaces/*/logs/*.json
```
