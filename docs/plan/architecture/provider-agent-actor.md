# Provider / Agent Architecture

> This document re-examines the core architecture of `agile-agent` for interacting with codex/claude processes, from an **Actor model** perspective.
>
> Core Mission: **Start LLM process → Send requirements → Receive streaming responses → Decide based on responses → Close the loop to complete tasks**.

---

## Table of Contents

1. [Actor Model Overview](#1-actor-model-overview)
2. [Current Architecture Diagnosis](#2-current-architecture-diagnosis)
3. [Core Actors in Detail](#3-core-actors-in-detail)
4. [Messaging System & State Machine](#4-messaging-system--state-machine)
5. [Closed Loop: How the Decision Layer Drives New Interactions](#5-closed-loop-how-the-decision-layer-drives-new-interactions)
6. [Data Flow Panorama](#6-data-flow-panorama)
7. [Issue List & Improvement Directions](#7-issue-list--improvement-directions)
8. [Appendix: Key File Index](#8-appendix-key-file-index)

---

## 1. Actor Model Overview

The core of this system can be strictly mapped to a **manual implementation of the Actor model** (without using frameworks like `actix`, based on `std::sync::mpsc` + `std::thread`):

| Actor Concept | Implementation | Responsibility |
|:---|:---|:---|
| **Actor (Work Agent)** | `AgentSlot` | Holds identity, state, session, history (transcript) |
| **Behavior** | `ProviderThread` | Actually runs the Claude/Codex subprocess, producing event streams |
| **Mailbox** | `mpsc::Receiver<ProviderEvent>` | Each Actor has an exclusive receiver |
| **Message** | `ProviderEvent` (24 variants) | Encapsulates all output from the LLM process |
| **Supervisor** | `AgentPool` | Manages Actor lifecycle, fault recovery, child Actor coordination |
| **Child Actor** | `DecisionAgentSlot` | Paired 1:1 with each work agent, responsible for autonomous decisions |
| **Scheduler** | `SessionManager::tick()` | Single-threaded event loop, serially processes all Actor messages |
| **Protocol Gateway** | `EventPump` | Converts internal ProviderEvent to external protocol::Event |
| **Inter-Actor Mail** | `AgentMailbox` | Cross-Actor Scrum-style messages (task help, blockage notifications, etc.) |

### Key Design Principles

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Principle 1: Single-threaded Ownership                                      │
│  Only the Scheduler (tick() main thread) can modify AgentPool / AgentSlot   │
│  ProviderThread only sends events via mpsc::Sender, never touches shared    │
│  state                                                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│  Principle 2: State Machine Guards                                           │
│  The 14 states of AgentSlotStatus are explicitly validated via              │
│  can_transition_to(). Invalid transitions cannot be caught at compile time  │
│  but will be rejected at runtime                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  Principle 3: Fault Isolation                                                │
│  Decision agents use fallback mutexes; disconnected channels are detected   │
│  and cleaned up                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  Principle 4: Actor Snapshot Persistence                                     │
│  ShutdownSnapshot captures full Actor state for restart (classic Actor      │
│  snapshot pattern)                                                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Current Architecture Diagnosis

Before diving into components, let's face the **root causes of the chaos**:

### 2.1 Overloaded `tick()`

`SessionManager::tick()` currently exceeds **1000 lines**, taking on all of the following responsibilities:

1. Poll all Provider events
2. Update Slot state machine + Transcript
3. Decision layer event classification (classify_event)
4. Idle Agent auto-trigger decisions
5. Poll Decision Agent responses
6. Execute decision actions (may start new Provider threads)
7. Protocol event conversion (EventPump)
8. Cross-Agent mail delivery (Mailbox)
9. Disconnect channel cleanup

**An Actor model scheduler should do only one thing: take messages from mailboxes and dispatch them to the corresponding Actors for processing.**

Current `tick()` not only schedules but also inline-implements all business processing logic, leading to:
- Difficult unit testing (requires constructing a complete SessionManager context)
- Modifying one piece of logic may affect 8 other places
- New developers need to read 1000 lines to understand what one tick does

### 2.2 Multiple Representations of the Same Concept

`ProviderEvent` **has different shapes at three levels**:

| Level | Type | Location | Issue |
|:---|:---|:---|:---|
| Raw process output | `agent_core::ProviderEvent` | `agent/provider/src/provider.rs` | 24 variants, most complete |
| Decision layer input | `agent_decision::provider::ProviderEvent` | `decision/src/provider/provider_event.rs` | Simplified version, missing some variants |
| Protocol broadcast | `agent_protocol::events::EventPayload` | `agent/protocol/src/events.rs` | Completely different structure |

**Chaos manifestation**: `event_converter.rs` must manually map between the core layer and the decision layer. When adding a new event type (e.g., a new MCP event), 4 files need to be modified to connect them.

### 2.3 Blurred Crate Boundaries

Code related to Provider/Agent is scattered across **7 crates**:

```
agent/provider/      ← Process startup, protocol parsing, ProviderEvent definition
agent/daemon/        ← SessionManager, tick(), EventPump, lifecycle
agent/protocol/      ← External protocol events, JSON-RPC, WebSocket
core/src/            ← AgentSlot, AgentPool, EventAggregator, DecisionAgentSlot
decision/src/        ← Decision engine, classifier, Situation/Action registry
agent/types/         ← AgentId, ProviderKind and other basic types
llm-provider/        ← LLM calling abstraction (used by decision layer)
```

**Boundary issues**:
- `ProviderEvent` is defined in `agent/provider`, but heavily used by `core` and `decision`
- `SessionManager` is in `daemon`, but it directly inline-processes Slot state updates that should be done in `core`
- `EventPump` is in `daemon`, but it converts `core`'s ProviderEvent → `protocol`'s Event, logically belonging to the protocol layer

### 2.4 Coupled State Updates and Event Broadcasting

In `tick()`, the processing order for a ProviderEvent is hardcoded:

```
Update Slot state → Decision layer classification → Protocol broadcast
```

This means: **If an event needs to be intercepted by the decision layer and prevent broadcasting, there is no clear extension point.**

---

## 3. Core Actors in Detail

### 3.1 WorkAgent — `AgentSlot`

**File**: `core/src/agent_slot.rs` (~1680 lines)

`AgentSlot` is the atomic Actor unit, encapsulating all mutable state for a single agent:

```rust
pub struct AgentSlot {
    agent_id: AgentId,                    // Unique identity
    codename: AgentCodename,              // Display name: alpha, bravo, charlie...
    provider_type: ProviderType,          // Claude / Codex / Mock
    role: AgentRole,                      // ProductOwner / ScrumMaster / Developer
    status: AgentSlotStatus,              // Current state machine state
    session_handle: Option<SessionHandle>,// Multi-turn conversation handle
    transcript: Vec<TranscriptEntry>,     // Local event journal
    assigned_task_id: Option<TaskId>,     // Backlog linkage
    event_rx: Option<Receiver<ProviderEvent>>, // INBOX
    thread_handle: Option<JoinHandle<()>>,     // Behavior thread handle
    last_activity: Instant,
    decision_policy: DecisionAgentCreationPolicy,
    launch_bundle: Option<AgentLaunchBundle>,
    worktree_path: Option<PathBuf>,       // Isolated filesystem context
    last_idle_trigger_at: Option<Instant>,// Idle decision trigger cooldown
}
```

**Actor characteristics**:
- **Identity**: `agent_id` is globally unique,贯穿 process lifecycle, persistence, protocol broadcast
- **State encapsulation**: All fields are private, external can only modify via `&mut self` methods
- **Journal**: `transcript` is the Actor's local event log, immutable append-only
- **Context**: `worktree_path` provides Actor-level filesystem isolation

#### Lifecycle Methods

```rust
// Bind behavior (start Provider thread)
pub fn set_provider_thread(&mut self, event_rx: Receiver<ProviderEvent>, thread_handle: JoinHandle<()>);

// State transition (guarded by state machine)
pub fn transition_to(&mut self, new_status: AgentSlotStatus) -> Result<(), String>;

// Append journal
pub fn append_transcript(&mut self, entry: TranscriptEntry);
pub fn append_assistant_chunk(&mut self, chunk: &str);
pub fn append_exec_command_output_delta(&mut self, call_id: Option<String>, delta: &str);

// Cleanup behavior (called after Provider thread ends)
pub fn clear_provider_thread(&mut self);
```

---

### 3.2 ProviderThread — Actor Behavior

**Files**: `agent/provider/src/providers/claude.rs`, `codex.rs`, `provider_thread.rs`

ProviderThread is not a Rust struct, but a **running OS thread**. It is the AgentSlot's "behavior" — when the AgentSlot receives user input, it creates and starts this thread, and the thread's lifecycle is completely bound to one LLM call.

#### Claude Process Startup

```rust
// agent/provider/src/providers/claude.rs
Command::new("claude")
    .args(&["-p", "--bare",
            "--output-format", "stream-json",
            "--input-format", "stream-json",
            "--verbose", "--strict-mcp-config",
            "--permission-mode", "bypassPermissions"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
```

- **Input**: JSON payload written to stdin (containing user message + session_id)
- **Output**: JSON Lines stream from stdout, parsed line by line into `ProviderEvent`
- **Session resume**: `--resume <session_id>`

#### Codex Process Startup

```rust
// agent/provider/src/providers/codex.rs
Command::new("codex")
    .args(&["exec",
            "--dangerously-bypass-approvals-and-sandbox",
            "--json"])
    .stdin(Stdio::null())   // exec mode doesn't need stdin
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
```

- **Input**: Prompt as the last CLI argument
- **Output**: JSON Lines stream from stdout
- **Session resume**: `resume <thread_id>`

#### Thread Safety Contract

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ProviderThread NEVER directly modifies shared state                          │
│  It only reads configuration (cwd, prompt, session) and sends events via     │
│  event_tx. All state changes are executed by the Scheduler (tick()) after    │
│  receiving events                                                            │
└─────────────────────────────────────────────────────────────────────────────┘
```

This is the most critical **invariant** of this architecture. Violating it causes data races and unpredictable states.

---

### 3.3 Supervisor — `AgentPool`

**File**: `core/src/agent_pool.rs` (~4578 lines)

`AgentPool` is the Actor supervisor, managing a group of `AgentSlot` (Worker Actors) and a group of `DecisionAgentSlot` (Child Actors).

```rust
pub struct AgentPool {
    slots: Vec<AgentSlot>,                        // Worker Actors
    max_slots: usize,                             // Concurrency limit (default 4)
    next_agent_index: usize,                      // ID generator
    workplace_id: WorkplaceId,
    human_queue: HumanDecisionQueue,              // Human decision queue
    blocked_handler: BlockedHandler,              // Blockage handling
    decision_coordinator: DecisionAgentCoordinator, // Child Actor management
    worktree_coordinator: WorktreeCoordinator,    // Context isolation
    focus_manager: FocusManager,                  // UI focus
    cwd: PathBuf,
    profile_store: Option<ProfileStore>,          // Actor config
}
```

#### Supervisor Responsibilities

| Responsibility | Method | Actor Model Equivalent |
|:---|:---|:---|
| Create Actor | `spawn_agent()` | `ActorSystem.actorOf()` |
| Destroy Actor | `stop_agent()` / `remove_agent()` | Graceful stop + forced removal |
| Cascade lifecycle | `stop_decision_agent_for()` | Supervisor Strategy: Stop Children |
| Fault recovery | `blocked_handler` manages `BlockedState` | Restart/escalation strategy |
| Task assignment | `assign_task()` | Router |
| Worktree isolation | `spawn_agent_with_worktree()` | Actor Context |

#### 1:1 Child Actor Pairing

Every non-Mock WorkAgent automatically creates a paired DecisionAgent:

```rust
pub fn spawn_agent(&mut self, provider_kind: ProviderKind) -> Result<AgentId, String> {
    // ... create AgentSlot ...
    self.slots.push(slot);
    
    // Auto-create paired decision agent
    if provider_kind != ProviderKind::Mock {
        let _ = self.spawn_decision_agent_for(&agent_id);
    }
    Ok(agent_id)
}
```

---

### 3.4 DecisionAgent — Meta-Actor

**File**: `core/src/decision_agent_slot.rs` (~1473 lines)

The DecisionAgent is the **"agent of the agent"** — it observes the WorkAgent's events and autonomously decides the next action.

```rust
pub struct DecisionAgentSlot {
    agent_id: AgentId,                    // Identity: "decision-{work_agent_id}"
    status: DecisionAgentStatus,          // Idle / Thinking / Responding / Error / Stopped
    engine: TieredDecisionEngine,         // Tiered decision engine
    mail_receiver: DecisionMailReceiver,  // INBOX (receives DecisionRequest)
    reflection_round: u32,                // Reflection round count
    pending_reflection_round: Arc<Mutex<Option<u32>>>,
    pending_fallback_response: Arc<Mutex<Option<DecisionResponse>>>,
    decision_count: u32,
    error_count: u32,
    last_activity: Instant,
    last_decision_started_at: Option<Instant>,
}
```

#### Async Processing Model

The DecisionAgent's LLM call is **asynchronous** (in an independent thread), so the Scheduler is never blocked:

```rust
pub fn poll_and_process(&mut self) -> usize {
    if !self.status.is_idle() { return 0; }  // Only one request at a time

    if let Some(request) = self.try_receive_request() {
        self.spawn_async_processing(request);  // Run LLM in background thread
        return 1;
    }
    0
}
```

Background thread:
1. Build prompt (containing situation + context)
2. Call `TieredDecisionEngine::decide()`
3. Return `DecisionResponse` via channel or fallback mutex

---

### 3.5 Scheduler — `SessionManager::tick()`

**File**: `agent/daemon/src/session_mgr.rs`

The Scheduler is the **heart** of the entire Actor system. It is a 100ms-period `tokio::time::interval`, executing one message dispatch per tick.

```rust
// main.rs
let tick_handle = tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        interval.tick().await;
        let events = session_mgr.tick().await.unwrap_or_default();
        for event in events {
            broadcaster.broadcast(event).await;
        }
    }
});
```

The ideal Actor scheduling logic inside `tick()`:

```
FOR EACH Actor:
    WHILE mailbox not empty:
        message = mailbox.recv()
        actor.handle_message(message)   ← This is where business processing should happen
```

But current `tick()` **inline-implements all business processing into the scheduler**, which is the core issue needing refactoring.

---

## 4. Messaging System & State Machine

### 4.1 ProviderEvent — Complete Set of Actor Messages

**Definition**: `agent/provider/src/provider.rs` (24 variants)

| Category | Variant | Meaning | State Machine Impact |
|:---|:---|:---|:---|
| **Streaming output** | `AssistantChunk(String)` | LLM text output | Responding state, append Transcript |
| | `ThinkingChunk(String)` | Chain-of-thought | Same |
| **Lifecycle** | `Status(String)` | Status hint | No state change |
| | `Finished` | Process ended | → Idle, cleanup thread |
| | `Error(msg)` | Process error | → Blocked, cleanup thread |
| | `SessionHandle(h)` | Session ID | Store session_handle |
| **Shell execution** | `ExecCommandStarted { call_id, input_preview, source }` | Start executing command | → ToolExecuting |
| | `ExecCommandOutputDelta { call_id, delta }` | Command output fragment | Append to in-progress entry |
| | `ExecCommandFinished { call_id, output_preview, status, exit_code, duration_ms, source }` | Command complete | → Responding |
| **Generic tools** | `GenericToolCallStarted { name, call_id, input_preview }` | Tool call start | → ToolExecuting |
| | `GenericToolCallFinished { name, call_id, output_preview, success, exit_code, duration_ms }` | Tool call complete | → Responding |
| **Web search** | `WebSearchStarted { call_id, query }` | Start search | → ToolExecuting |
| | `WebSearchFinished { call_id, query, action }` | Search complete | → Responding |
| **Patch** | `PatchApplyStarted { call_id, changes }` | Start applying patch | → ToolExecuting |
| | `PatchApplyOutputDelta { call_id, delta }` | Patch output | Append to entry |
| | `PatchApplyFinished { call_id, changes, status }` | Patch complete | → Responding |
| **MCP** | `McpToolCallStarted { call_id, invocation }` | MCP call start | → ToolExecuting |
| | `McpToolCallFinished { call_id, invocation, result_blocks, error, status, is_error }` | MCP call complete | → Responding |
| **Images** | `ViewImage { call_id, path }` | View image | None |
| | `ImageGenerationFinished { ... }` | Image generation complete | None |

### 4.2 AgentSlotStatus — State Machine

**Definition**: `core/src/slot/status.rs` (14 states)

```
                         ┌─────────────┐
                         │    Idle     │ ◄────────────────────────────┐
                         └──────┬──────┘                              │
                                │ spawn_provider                      │
                                ▼                                     │
                         ┌─────────────┐                              │
    ┌───────────────────►│  Starting   │                              │
    │                    └──────┬──────┘                              │
    │                           │ first event                         │
    │                           ▼                                     │
    │                    ┌─────────────┐      ┌─────────────┐         │
    │               ┌───►│  Responding │◄────►│ToolExecuting│         │
    │               │    └──────┬──────┘      └─────────────┘         │
    │               │           │ stream complete                     │
    │               │           ▼                                     │
    │               │    ┌─────────────┐                              │
    │               └────┤  Finishing  │──────────────────────────────┘
    │                    └─────────────┘
    │
    │  Exception branches
    │
    ├─► Blocked { reason }          ← Error event, user interrupt, decision blockage
    ├─► BlockedForDecision { ... }  ← Requires human/decision layer intervention
    ├─► WaitingForInput { ... }     ← Waiting for user input within Responding
    ├─► Resting { ... }             ← Rate limit cooldown
    ├─► Paused { reason }           ← Explicit pause
    ├─► Error { message }           ← Unrecoverable error
    └─► Stopped { reason }          ← Stopped (terminal state)
```

**State transition rules** (explicitly defined in `can_transition_to()`):

```rust
Idle          → Starting | Blocked | BlockedForDecision
Starting      → Responding | Error
Responding    → ToolExecuting | WaitingForInput | Finishing | Blocked
ToolExecuting → Responding | Error | Blocked
Finishing     → Idle
Blocked       → Idle | Resting | WaitingForInput
BlockedForDecision → Resting | Idle
Error         → Idle | Stopped
Stopped       → Starting
```

### 4.3 EventAggregator — Multiplexed Mailbox Polling

**File**: `core/src/event_aggregator.rs`

```rust
pub struct EventAggregator {
    receivers: HashMap<AgentId, Receiver<ProviderEvent>>,
}
```

`poll_all()` performs non-blocking `try_recv()` on **every Actor's mailbox**:

```rust
pub fn poll_all(&self) -> PollResult {
    for (agent_id, receiver) in &self.receivers {
        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    events.push(AgentEvent::from_provider(agent_id.clone(), event));
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    disconnected.push(agent_id.clone());
                    break;
                }
            }
        }
    }
    PollResult { events, empty_channels, disconnected_channels: disconnected }
}
```

**Key behaviors**:
- `Empty`: Mailbox empty, move to next Actor
- `Disconnected`: ProviderThread has exited (sender dropped), needs cleanup

---

## 5. Closed Loop: How the Decision Layer Drives New Interactions

This is the system's most important **closed control loop**: LLM output triggers a decision, and the decision's result is sent back to the LLM as new input.

### 5.1 Complete Closed Loop Flow

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Claude/Codex │────►│ EventAggregator │────►│  classify_event │
│  ProviderThread│     │   .poll_all()   │     │ NeedsDecision?  │
└─────────────┘     └─────────────────┘     └────────┬────────┘
                                                     │
                           ┌─────────────────────────┘
                           ▼
                    ┌─────────────────┐
                    │ DecisionRequest │
                    │  situation_type │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ DecisionAgentSlot│
                    │  poll_and_process│
                    │  status: Thinking│
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ Background Thread│
                    │ TieredEngine::decide()
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ DecisionResponse │
                    │  custom_instruction│
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ DecisionExecutor │
                    │ execute_decision_action()
                    │ → append TranscriptEntry::User
                    └────────┬────────┘
                             │
                             ▼ DecisionExecutionResult::CustomInstruction
                    ┌─────────────────┐
                    │start_provider_for│
                    │agent_inner()      │
                    └────────┬────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  NEW ProviderThread│
                    │  with instruction  │
                    └─────────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │ EventAggregator │
                    │ .add_receiver() │
                    └─────────────────┘
```

### 5.2 Key Code Path

**Step 1: Event Classification** (in `session_mgr.rs` tick())

```rust
let classify_result = inner.agent_pool.classify_event(&agent_id, &event);
if classify_result.is_needs_decision() {
    let request = DecisionRequest::new(agent_id, situation_type, context);
    inner.agent_pool.send_decision_request(&agent_id, request)?;
}
```

**Step 2: Decision Request Enqueue** (`core/src/agent_pool.rs`)

```rust
pub fn send_decision_request(&self, work_agent_id: &AgentId, request: DecisionRequest)
    -> Result<(), String> {
    if let Some(sender) = self.decision_coordinator.mail_sender_for(work_agent_id) {
        sender.send_request(request).map_err(|e| ...)?;
        Ok(())
    } else {
        Err("no decision agent available".to_string())
    }
}
```

**Step 3: Background Decision** (`core/src/decision_agent_slot.rs`)

```rust
fn spawn_async_processing(&mut self, request: DecisionRequest) {
    self.status = DecisionAgentStatus::thinking_now();
    
    std::thread::spawn(move || {
        let mut engine = TieredDecisionEngine::new(engine_config);
        let result = engine.decide(context, &action_registry);
        let response = match result { Ok(output) => ..., Err(e) => ... };
        
        // Return result via channel
        if let Err(e) = response_tx.send(response.clone()) {
            // Fallback: store in shared mutex if channel fails
            if let Ok(mut guard) = pending_fallback.lock() {
                *guard = Some(response);
            }
        }
    });
}
```

**Step 4: Execute Decision Action** (in `session_mgr.rs` tick())

```rust
let decision_responses = inner.agent_pool.poll_decision_agents();
for (work_agent_id, response) in decision_responses {
    if let Some(output) = response.output() {
        let result = inner.agent_pool.execute_decision_action(&work_agent_id, output);
        
        match result {
            DecisionExecutionResult::CustomInstruction { instruction } => {
                // Closed loop: start new Provider thread
                let _ = Self::start_provider_for_agent_inner(
                    &mut inner, &work_agent_id, &instruction, false
                ).await;
            }
            // ... other action types
        }
    }
}
```

**Note**: `record_user_prompt=false` because `DecisionExecutor` has already appended `TranscriptEntry::User` to the transcript, so it must not be duplicated.

### 5.3 Idle Trigger Closed Loop

In addition to event triggers, the system supports **time-triggered** closed loops:

```rust
// Idle agent check in tick()
for slot in inner.agent_pool.slots() {
    if slot.status().is_idle()
        && slot.last_activity().elapsed().as_secs() >= 60
        && slot.last_idle_trigger_at().map_or(true, |t| t.elapsed().as_secs() >= 300)
    {
        // Send agent_idle decision request
        let request = DecisionRequest::new(agent_id, SituationType::new("agent_idle"), context);
        inner.agent_pool.send_decision_request(&agent_id, request)?;
        slot.set_last_idle_trigger_at(Instant::now());
    }
}
```

This implements the **autonomous loop**: even without any external events, idle Agents actively seek guidance from the decision layer.

---

## 6. Data Flow Panorama

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              CLIENT (TUI / WebSocket / CLI)                          │
│                                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐             │
│  │  JSON-RPC    │  │   WebSocket  │  │   CLI args   │  │   HTTP API   │             │
│  │  session.*   │  │   events     │  │   parse_args │  │   (future)   │             │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘             │
│         └──────────────────┴──────────────────┘                  │                    │
│                            │                                     │                    │
└────────────────────────────┼─────────────────────────────────────┼────────────────────┘
                             │                                     │
                             ▼                                     ▼
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                           DAEMON: SessionManager                                     │
│                    Arc<Mutex<SessionInner>> — Single-threaded scheduling            │
│                                                                                      │
│  ┌─────────────────┐  ┌─────────────┐  ┌─────────────────┐  ┌──────────────────┐   │
│  │ EventAggregator │  │  AgentPool  │  │ DecisionAgents  │  │   EventPump      │   │
│  │  .poll_all()    │  │ classify/   │  │  .poll()        │  │  .process()      │   │
│  │                 │  │ execute     │  │  .execute()     │  │                  │   │
│  └────────┬────────┘  └──────┬──────┘  └────────┬────────┘  └────────┬─────────┘   │
│           │                  │                   │                    │             │
│           │    ┌─────────────┘                   │                    │             │
│           │    │                                 │                    │             │
│           │    ▼                                 │                    │             │
│           │  ┌──────────┐                        │                    │             │
│           └──┤AgentSlot │◄───────────────────────┘                    │             │
│              │-transcript│                                             │             │
│              │-status    │                                             │             │
│              │-session   │                                             │             │
│              └────┬─────┘                                             │             │
│                   │                                                   │             │
│                   │ mpsc::channel                                     │             │
│                   ▼                                                   │             │
│              ┌──────────┐                                             │             │
│              │ event_rx │                                             │             │
│              │(Receiver)│                                             │             │
│              └────┬─────┘                                             │             │
│                   │                                                   │             │
│                   │  ProviderEvent                                    │             │
│                   ▼                                                   │             │
│  ╔═══════════════════════════════════════════════════════════════════════╗         │
│  ║                    PROVIDER THREADS (per agent)                        ║         │
│  ║  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ║         │
│  ║  │ Claude CLI  │  │  Codex CLI  │  │  Mock/Test  │  │ Decision LLM│  ║         │
│  ║  │ stream-json │  │ exec --json │  │  simulated  │  │  caller     │  ║         │
│  ║  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  ║         │
│  ╚═════════╪════════════════╪════════════════╪════════════════╪═════════╝         │
│            └────────────────┴────────────────┴────────────────┘                     │
│                              │                                                       │
│                              │ stdout (JSON Lines)                                   │
│                              ▼                                                       │
│                   ┌─────────────────────┐                                            │
│                   │  protocol::Event    │  ────────────────────────────────────────────┘
│                   │  (JSON broadcast)   │
│                   └─────────────────────┘
└─────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Issue List & Improvement Directions

### 7.1 High Priority: Separation of Scheduler and Business Logic

**Issue**: `tick()` inline-implements all business logic.

**Improvement direction**: Introduce `AgentMessageHandler` trait, letting `AgentSlot` handle its own messages:

```rust
// Ideal architecture
trait ActorMessageHandler {
    fn handle_message(&mut self, message: ProviderEvent) -> Vec<SideEffect>;
}

enum SideEffect {
    BroadcastEvent(protocol::Event),
    SendDecisionRequest(DecisionRequest),
    StartProvider { prompt: String },
    // ...
}

// tick() only handles scheduling
pub async fn tick(&self) -> Result<Vec<Event>> {
    let mut inner = self.inner.lock().await;
    let mut effects = Vec::new();
    
    // 1. Collect all messages
    let events = inner.event_aggregator.poll_all();
    
    // 2. Dispatch to each Actor
    for AgentEvent::FromProvider { agent_id, event } in events {
        if let Some(slot) = inner.agent_pool.get_slot_mut_by_id(&agent_id) {
            let slot_effects = slot.handle_message(event);
            effects.extend(slot_effects);
        }
    }
    
    // 3. Execute side effects uniformly
    for effect in effects {
        match effect {
            SideEffect::BroadcastEvent(ev) => broadcast.push(ev),
            SideEffect::SendDecisionRequest(req) => ...,
            SideEffect::StartProvider { prompt } => ...,
        }
    }
    
    Ok(broadcast)
}
```

### 7.2 High Priority: Unified ProviderEvent Representation

**Issue**: The same event has different type definitions in `core` and `decision`, requiring `event_converter.rs` bridging.

**Improvement direction**:
- Promote `ProviderEvent` to `agent-protocol` or a standalone `agent-events` crate
- Both `agent-core` and `agent-decision` depend on it
- Eliminate `event_converter.rs`

### 7.3 Medium Priority: Crate Boundary Refactoring

**Current unreasonable dependency**:

```
agent-daemon  ──►  agent-core (reasonable)
agent-daemon  ──►  agent-decision (reasonable)
agent-core    ──►  agent-provider (unreasonable: core should not depend on provider)
agent-decision ──► agent-core (partially reasonable, but event converter is a symptom)
```

**Ideal boundary**:

```
agent-protocol     ← All event types, protocol definitions (no business logic)
     │
     ▼
agent-events       ← ProviderEvent definition (shared by core / decision / provider)
     │
     ├──► agent-provider   ← Process startup, protocol parsing (depends only on events)
     ├──► agent-core       ← AgentSlot, AgentPool, state machine (depends on events + protocol)
     └──► agent-decision   ← Decision engine (depends on events + protocol)
              │
              ▼
         agent-daemon       ← SessionManager, EventPump, HTTP/WS service
              │                (depends on all above, is the orchestration layer)
              ▼
         agent-tui          ← UI rendering (depends only on protocol events)
```

### 7.4 Medium Priority: EventPump Location Migration

**Issue**: `EventPump` is in `agent-daemon`, but it converts `core`'s ProviderEvent → `protocol`'s Event, logically belonging to the protocol layer.

**Improvement direction**: Move `EventPump` or at least its conversion logic down to `agent-protocol`.

### 7.5 Low Priority: Mock Provider Special Handling

**Issue**: Multiple places need `if provider_kind != ProviderKind::Mock` special checks.

**Improvement direction**: Encapsulate Mock's differentiated behavior using the Strategy Pattern, making `AgentPool` unaware of ProviderKind.

---

## 8. Appendix: Key File Index

### 8.1 Provider Process Layer

| File | Responsibility |
|:---|:---|
| `agent/provider/src/provider.rs` | `ProviderEvent` definition, generic process startup interface |
| `agent/provider/src/providers/claude.rs` | Claude CLI process wrapper (stream-json protocol) |
| `agent/provider/src/providers/codex.rs` | Codex CLI process wrapper (exec --json protocol) |
| `agent/provider/src/provider_thread.rs` | Provider thread lifecycle, naming, contract documentation |
| `agent/provider/src/llm_caller.rs` | Decision layer LLM caller (implements `agent_decision::LLMCaller`) |
| `agent/provider/src/launch_config/` | Startup config parsing (executable path, args, env vars) |

### 8.2 Core Actor Layer

| File | Responsibility |
|:---|:---|
| `core/src/agent_slot.rs` | **WorkAgent Actor**: state, lifecycle, transcript management |
| `core/src/slot/status.rs` | **State machine**: 14 states and transition rules |
| `core/src/agent_pool.rs` | **Supervisor**: Actor creation/destruction, decision agent coordination, task assignment |
| `core/src/decision_agent_slot.rs` | **DecisionAgent Actor**: async decisions, LLM threads, fallback mechanism |
| `core/src/event_aggregator.rs` | **Mailbox multiplexer**: non-blocking polling of all Actor channels |
| `core/src/agent_mail.rs` | **Cross-Actor mail**: Scrum-style message system |
| `core/src/app.rs` | **AppState**: global behavior state (input, backlog, skill registry) |
| `core/src/pool/event_converter.rs` | ProviderEvent bridging (core → decision) |
| `core/src/pool/decision_coordinator.rs` | Decision agent coordinator (HashMap<AgentId, DecisionAgentSlot>) |
| `core/src/pool/decision_executor.rs` | Decision action executor (custom_instruction, retry, etc.) |

### 8.3 Daemon Scheduling Layer

| File | Responsibility |
|:---|:---|
| `agent/daemon/src/session_mgr.rs` | **Scheduler**: tick() main loop, all business logic orchestration |
| `agent/daemon/src/event_pump.rs` | **Protocol Gateway**: ProviderEvent → protocol::Event conversion |
| `agent/daemon/src/event_log.rs` | Event log persistence |
| `agent/daemon/src/handler/session.rs` | JSON-RPC `session.*` method handling |
| `agent/daemon/src/handler/agent.rs` | JSON-RPC `agent.*` method handling |
| `agent/daemon/src/main.rs` | tick loop + WebSocket/HTTP service startup |

### 8.4 Decision Layer

| File | Responsibility |
|:---|:---|
| `decision/src/provider/initializer.rs` | Decision layer component initialization (Situation/Action/Classifier registries) |
| `decision/src/classifier/classifier_registry.rs` | Event classifier (Running / NeedsDecision) |
| `decision/src/engine/tiered_engine.rs` | **Tiered decision engine**: Rule → LLM → Human Escalation |
| `decision/src/core/context.rs` | `DecisionContext`: trigger situation + agent ID + running context |
| `decision/src/model/situation/` | Built-in situation definitions (waiting_for_choice, error, agent_idle, etc.) |
| `decision/src/model/action/` | Built-in action definitions (custom_instruction, retry, confirm_completion, etc.) |

### 8.5 Protocol Layer

| File | Responsibility |
|:---|:---|
| `agent/protocol/src/events.rs` | External protocol event definitions (ItemDelta, ItemStarted, AgentStatusChanged, etc.) |
| `agent/protocol/src/methods.rs` | JSON-RPC method definitions |
| `agent/protocol/src/state.rs` | Protocol state types (AgentSlotStatus, etc.) |

---

## Summary

The core architecture of `agile-agent` is essentially a **hand-written Actor system**:

- `AgentSlot` is the Actor (identity + state + mailbox)
- `ProviderThread` is the Actor's behavior (running the LLM process in an independent OS thread)
- `AgentPool` is the Supervisor (lifecycle management, child Actor pairing)
- `SessionManager::tick()` is the Scheduler (message dispatcher)
- `ProviderEvent` is the Actor message (24 variants covering all LLM output)
- `DecisionAgent` is the meta-Actor (observing the WorkAgent and making autonomous decisions)

**Architectural strengths**:
1. Single-threaded ownership eliminates data races
2. State machine guards prevent invalid state transitions
3. Decision closed loop enables autonomous operation
4. Actor snapshots support persistence and recovery

**Architectural chaos points**:
1. `tick()` inline-implements too much business logic, violating the principle that a scheduler should "only dispatch"
2. Multiple representations of `ProviderEvent` increase maintenance burden
3. Crate boundaries are blurred and dependency relationships need re-sorting
4. Protocol conversion logic is misplaced (should be in the protocol layer rather than the daemon layer)

**Next action recommendations**:
1. Extract business logic from `tick()` into `AgentSlot::handle_event()` and `AgentPool::handle_decision()`
2. Unify `ProviderEvent` into an independent crate
3. Re-divide crate boundaries so `agent-daemon` becomes a pure orchestration layer
