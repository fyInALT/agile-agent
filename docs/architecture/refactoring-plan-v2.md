# Provider/Agent Core Architecture Refactoring Plan V2

> **Version**: V2 (Thorough Refactoring Edition)
> **Premise**: No technical debt is acceptable, no naming compromises, core architecture must be done right.
>
> This document absorbs technical insights from the V1 reflection, but no longer compromises design for "saving effort".

---

## Table of Contents

1. [Refactoring Principles](#1-refactoring-principles)
2. [Domain Model](#2-domain-model)
3. [Type System Renaming](#3-type-system-renaming)
4. [Crate Architecture](#4-crate-architecture)
5. [Worker Aggregate Root](#5-worker-aggregate-root)
6. [WorkerState State Machine](#6-workerstate-state-machine)
7. [DomainEvent Unified Event System](#7-domainevent-unified-event-system)
8. [EventLoop Scheduler](#8-eventloop-scheduler)
9. [Command Side Effect System](#9-command-side-effect-system)
10. [Decision Layer Integration](#10-decision-layer-integration)
11. [Protocol Layer](#11-protocol-layer)
12. [Migration Roadmap](#12-migration-roadmap)

---

## 1. Refactoring Principles

### Principle 1: Naming Is Architecture

Type names are the first documentation of architecture. If a name cannot accurately express the abstraction, developers will write code based on incorrect mental models, producing cascading technical debt.

> `AgentSlot` → Developers will think this is a "container" or "slot", and write penetrative access like `pool.slots[0]`.
>
> `Worker` → Developers will understand this is an entity with state, behavior, and lifecycle, and interact with it through methods.

### Principle 2: Dependency Direction Is Irreversible

```
Domain Layer ←──── Application Layer ←──── Infrastructure Layer
   ↑                              ↑
   │                              │
Shared Kernel                   External Protocol
```

- `agent-events` (shared kernel) depends on no other crate
- `agent-runtime` (domain layer) depends on `agent-events`, not on `agent-daemon`
- `agent-daemon` (application layer) depends on `agent-runtime` and `agent-protocol`
- `agent-provider` (infrastructure) only produces events, consumes no business logic

### Principle 3: Aggregate Root Encapsulation Is Complete

`Worker` is an aggregate root. Its internal state (transcript, state, session) can only be modified through `Worker` methods. Any external code (including `EventLoop`, `DecisionExecutor`, `ProtocolGateway`) cannot directly penetrate internal fields.

### Principle 4: State Calculation Is Pure Function

State change logic should be pure functions: given current state and input event, output new state and side effect commands. Side effects (starting processes, sending protocol events, writing logs) are executed uniformly at boundaries.

### Principle 5: State Machine Is Exhaustive and Explicit

All possible states of the system must be exhaustively represented by the state machine; all state transitions must be explicitly declared. No "implicit states" are allowed (such as using `Option<bool>` to represent three states).

---

## 2. Domain Model

### 2.1 Bounded Contexts

This system is a single bounded context: **Agent Runtime**. It is internally divided into four sub-domains:

| Sub-domain | Responsibility | Aggregate Roots |
|:---|:---|:---|
| **Worker Management** | Worker lifecycle, state machine, history | `Worker`, `WorkerPool` |
| **Event Processing** | Event collection, dispatch, conversion | `EventMux`, `ProtocolGateway` |
| **Decision Making** | Event classification, autonomous decisions, action execution | `DecisionCoordinator` |
| **Behavior Execution** | LLM process startup, protocol parsing | `BehaviorSpawner` |

### 2.2 Core Entities and Value Objects

```
Worker (Aggregate Root)
├── WorkerId (Value Object)
├── Codename (Value Object)
├── Role (Value Object)
├── WorkerState (Value Object, State Machine)
│   ├── Idle
│   ├── Starting
│   ├── Running(RunningPhase)
│   │   ├── Generating
│   │   ├── Thinking
│   │   ├── ExecutingTool { name }
│   │   ├── WaitingForInput
│   │   └── Finishing
│   ├── Paused(PausedReason)
│   │   ├── Blocked { reason }
│   │   ├── BlockedForDecision { state }
│   │   ├── Resting { until, resume_action }
│   │   └── UserRequested { reason }
│   ├── Error { message }
│   └── Stopped { reason }
├── Transcript (Value Object, Immutable Log)
│   └── TranscriptEntry (Value Object)
│       ├── User(String)
│       ├── Assistant(String)
│       ├── Thinking(String)
│       ├── Decision { ... }
│       ├── ExecCommand { ... }
│       ├── PatchApply { ... }
│       └── Error(String)
├── SessionHandle (Value Object)
│   ├── ClaudeSession { session_id }
│   └── CodexThread { thread_id }
└── WorktreeContext (Value Object)
    ├── path: PathBuf
    ├── branch: String
    └── worktree_id: String

WorkerPool (Aggregate Root)
├── workers: Vec<Worker>
├── max_workers: usize
├── focus: FocusManager
└── worktree: WorktreeCoordinator

EventMux (Domain Service)
├── channels: HashMap<WorkerId, Receiver<DomainEvent>>
└── poll_all() → EventBatch

DecisionCoordinator (Domain Service)
├── engines: HashMap<WorkerId, DecisionEngine>
├── classifier: SituationClassifier
└── executor: ActionExecutor

ProtocolGateway (Domain Service)
├── item_tracker: ItemTracker
└── convert(DomainEvent) → Vec<ProtocolEvent>
```

---

## 3. Type System Renaming

### 3.1 Must Rename (Misleading Names)

| Current Name | New Name | Reason |
|:---|:---|:---|
| `AgentSlot` | `Worker` | `Slot` is a container metaphor implying lifelessness and no behavior. `Worker` is an entity that performs tasks. |
| `AgentSlotStatus` | `WorkerState` | `Status` is a read-only property (like HTTP 200). `State` is a complete state machine with transition rules. |
| `AgentPool` | `WorkerPool` | Synchronized rename with `AgentSlot`. `Pool` itself is accurate. |
| `SessionManager` | `Runtime` | `Session` is a business concept (one session). `Runtime` is a system concept (runtime environment). |
| `ProviderEvent` | `DomainEvent` | `Provider` implies the source is the LLM process. In fact Mock, Decision, and System can all produce events. `DomainEvent` is the DDD standard term for all events occurring within the domain. |
| `EventAggregator` | `EventMux` | `Aggregator` implies aggregation calculation. `Mux` (Multiplexer) accurately describes multiplexing behavior. |
| `EventPump` | `ProtocolGateway` | `Pump` is a process metaphor. `Gateway` is an architecture pattern term representing the conversion boundary from internal domain to external protocol. |
| `DecisionAgentSlot` | `DecisionEngine` | `Slot` is again a container metaphor. `DecisionEngine` accurately describes its responsibility. |
| `DecisionAgentStatus` | `EngineState` | Same as above. |
| `DecisionAgentCoordinator` | `DecisionCoordinator` | Remove redundant `Agent`. |
| `AgentMailbox` | `InterWorkerMail` | Clearly indicates communication between Workers, not a Worker's own mailbox. |

### 3.2 Keep Original (Already Accurate)

| Name | Keep Reason |
|:---|:---|
| `AgentId` | Uniformly used throughout the system; `WorkerId` would cause confusion with `DecisionEngine` ID. Keep `AgentId`; in the Worker context it is the Worker's ID. |
| `TranscriptEntry` | Already accurately describes a record in the transcript. |
| `ProviderKind` | Although `BehaviorKind` is more abstract, `ProviderKind` directly corresponds to user-visible CLI tools (claude/codex), keeping it more intuitive. |
| `SessionHandle` | Already accurate. |

### 3.3 Rename Scope Estimate

```
AgentSlot          → Worker          : ~500 references
AgentSlotStatus    → WorkerState     : ~300 references  
AgentPool          → WorkerPool      : ~400 references
SessionManager     → Runtime         : ~200 references
ProviderEvent      → DomainEvent     : ~200 references
EventAggregator    → EventMux        : ~50 references
EventPump          → ProtocolGateway : ~30 references
DecisionAgentSlot  → DecisionEngine  : ~100 references
DecisionAgentCoordinator → DecisionCoordinator : ~50 references
AgentMailbox       → InterWorkerMail : ~30 references
────────────────────────────────────────────────
Total: ~1860 references
```

Using Rust Analyzer's Rename Symbol feature, combined with batch replacement scripts, estimated **2 developers for 1 week** to complete. All test case names are synchronously updated.

---

## 4. Crate Architecture

### 4.1 Target Dependency Graph

```
agent-types                (Basic types: AgentId, ProviderKind, TaskId, WorkplaceId)
    │
    └── agent-protocol     (External protocol: ProtocolEvent, JsonRpc, WebSocket message formats)
            │
            └── agent-tui  (Terminal UI, depends only on protocol)

agent-types
    │
    └── agent-events       (Domain events: DomainEvent definition, including ExecCommandStatus etc.)
            │
            ├── agent-provider    (Process management: Start Claude/Codex/Mock, output DomainEvent)
            │
            ├── agent-runtime     (Core runtime: Worker, WorkerPool, WorkerState, Runtime, EventLoop)
            │       │
            │       └── agent-decision   (Decision layer: depends on runtime's Worker state for classification)
            │
            └── agent-daemon      (Application layer: HTTP service, WebSocket, CLI entry, orchestrates Runtime)
```

### 4.2 Eliminating Circular Dependencies

In the current system, `agent-decision` directly modifies `AgentSlot`'s transcript through `DecisionExecutor`, which is the root of circular dependency:

```
agent-runtime ──► agent-decision (Runtime creates Decision)
    ▲                  │
    │                  ▼
    └────────── DecisionExecutor modifies AgentSlot
```

**Elimination Plan**: `DecisionExecutor` no longer directly modifies `Worker`, but produces `DecisionCommand`, which `Runtime` executes:

```
agent-runtime ──► agent-decision
    ▲                  │
    │                  ▼
    └────────── Runtime receives DecisionCommand and executes
```

```rust
// agent-decision/src/executor.rs

pub enum DecisionCommand {
    AppendTranscript { worker_id: AgentId, entry: TranscriptEntry },
    TransitionState { worker_id: AgentId, state: WorkerState },
    StartBehavior { worker_id: AgentId, prompt: String },
    AssignTask { worker_id: AgentId, task_id: TaskId },
    EscalateToHuman { worker_id: AgentId, request: HumanDecisionRequest },
}

pub struct ActionExecutor;

impl ActionExecutor {
    pub fn execute(
        &self,
        output: &DecisionOutput,
        worker: &Worker,  // Read-only reference, for decision context
    ) -> Vec<DecisionCommand> {
        // Only produces commands, does not modify state
        match output.actions.first() {
            Some(Action::CustomInstruction { instruction }) => vec![
                DecisionCommand::AppendTranscript {
                    worker_id: worker.id().clone(),
                    entry: TranscriptEntry::User(instruction.clone()),
                },
                DecisionCommand::StartBehavior {
                    worker_id: worker.id().clone(),
                    prompt: instruction.clone(),
                },
            ],
            // ... other actions
        }
    }
}
```

This way `agent-decision` only reads `Worker`, does not write to `Worker`, and the dependency direction is unidirectional.

### 4.3 Crate Responsibility Details

#### `agent-types`

- `AgentId`, `Codename`, `Role`, `ProviderKind`, `TaskId`, `WorkplaceId`
- Zero business logic, pure data structures + basic methods

#### `agent-events`

- `DomainEvent` (24 variants, unified domain event definition)
- `ExecCommandStatus`, `PatchApplyStatus`, `McpToolCallStatus` and other execution statuses
- `SessionHandle`
- `DecisionEvent` (decision layer focused event subset, converted through `From<DomainEvent>`)

#### `agent-protocol`

- `ProtocolEvent`, `ProtocolPayload`, `ItemId`, `ItemKind`, `ItemDelta`
- `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`
- Serialization/deserialization implementations
- Shared by `agent-tui` and `agent-daemon`

#### `agent-provider`

- `BehaviorSpawner`
- `ClaudeProcess`, `CodexProcess`, `MockProcess`
- `ProtocolParser` (stream-json / exec-json parsing)
- `LaunchConfig`, `LaunchContext`
- **Only produces `DomainEvent`, consumes no business logic**

#### `agent-runtime`

- `Worker` (Aggregate Root)
- `WorkerPool` (Aggregate Root)
- `WorkerState` (State Machine)
- `Transcript`, `TranscriptEntry`
- `EventMux`
- `Runtime`, `EventLoop`
- `InterWorkerMail`
- `ShutdownSnapshot`, `RuntimeStore`

#### `agent-decision`

- `DecisionEngine`
- `EngineState`
- `Situation`, `SituationClassifier`
- `DecisionOutput`, `Action`, `ActionExecutor`
- `DecisionCommand`
- `TieredDecisionEngine`
- Depends on `agent-runtime`'s `Worker` and `WorkerState` (read-only)

#### `agent-daemon`

- `Daemon` (main struct)
- HTTP/WebSocket server
- CLI argument parsing
- `ProtocolGateway` (`DomainEvent` → `ProtocolEvent` conversion)
- **This is the only orchestration layer that depends on all other crates**

---

## 5. Worker Aggregate Root

### 5.1 Design

```rust
// agent-runtime/src/worker.rs

/// Work unit aggregate root.
///
/// Worker encapsulates all state and behavior of an LLM agent.
/// All state changes must go through Worker methods; external code may never
/// directly modify internal fields.
pub struct Worker {
    id: AgentId,
    codename: Codename,
    role: Role,
    provider: ProviderKind,
    state: WorkerState,
    transcript: Vec<TranscriptEntry>,
    session: Option<SessionHandle>,
    behavior: Option<BehaviorHandle>,
    assigned_task: Option<TaskId>,
    worktree: Option<WorktreeContext>,
    last_activity: Instant,
    last_idle_trigger: Option<Instant>,
}

/// Output after Worker processes an event.
///
/// Contains state change confirmation and a list of commands for the runtime
/// to execute.
pub struct WorkerOutput {
    pub commands: Vec<RuntimeCommand>,
}

impl Worker {
    pub fn new(id: AgentId, codename: Codename, provider: ProviderKind) -> Self;
    
    pub fn id(&self) -> &AgentId;
    pub fn state(&self) -> &WorkerState;
    pub fn transcript(&self) -> &[TranscriptEntry];
    pub fn codename(&self) -> &Codename;
    pub fn role(&self) -> Role;
    pub fn provider(&self) -> ProviderKind;
    pub fn session(&self) -> Option<&SessionHandle>;
    pub fn has_behavior(&self) -> bool;
    pub fn last_activity(&self) -> Instant;
    pub fn assigned_task(&self) -> Option<&TaskId>;
    
    /// Process a domain event.
    ///
    /// This is the core method of Worker. Based on event type, it:
    /// 1. Updates internal state (state, transcript, session, etc.)
    /// 2. Returns commands for the runtime to execute
    ///
    /// This method executes no side effects (does not start processes, send
    /// network messages, or write logs).
    pub fn apply(&mut self, event: &DomainEvent) -> Result<WorkerOutput, WorkerError>;
    
    /// Explicit state transition (guarded by state machine).
    pub fn transition_to(&mut self, new_state: WorkerState) -> Result<(), StateTransitionError>;
    
    /// Bind behavior thread.
    pub fn attach_behavior(&mut self, handle: BehaviorHandle);
    
    /// Clear behavior thread (called after behavior ends).
    pub fn detach_behavior(&mut self);
    
    /// Assign task.
    pub fn assign_task(&mut self, task_id: TaskId);
    
    /// Set session handle.
    pub fn set_session(&mut self, handle: SessionHandle);
    
    /// Check whether idle decision can be triggered.
    pub fn can_trigger_idle_decision(&self, cooldown: Duration) -> bool;
    
    /// Mark that idle decision has been triggered.
    pub fn mark_idle_triggered(&mut self);
}
```

### 5.2 apply Method Implementation Example

```rust
impl Worker {
    pub fn apply(&mut self, event: &DomainEvent) -> Result<WorkerOutput, WorkerError> {
        self.touch_activity();
        
        match event {
            DomainEvent::AssistantChunk { text } => {
                self.transcript.push(TranscriptEntry::Assistant(text.clone()));
                Ok(WorkerOutput {
                    commands: vec![RuntimeCommand::EmitProtocol(
                        ProtocolEvent::item_delta(self.id.clone(), ItemDelta::Text(text.clone()))
                    )],
                })
            }
            
            DomainEvent::ThinkingChunk { text } => {
                self.transcript.push(TranscriptEntry::Thinking(text.clone()));
                Ok(WorkerOutput {
                    commands: vec![RuntimeCommand::EmitProtocol(
                        ProtocolEvent::item_delta(self.id.clone(), ItemDelta::Markdown(text.clone()))
                    )],
                })
            }
            
            DomainEvent::ExecCommandStarted { call_id, input_preview, source } => {
                self.transcript.push(TranscriptEntry::ExecCommand {
                    call_id: call_id.clone(),
                    source: source.clone(),
                    allow_exploring_group: true,
                    input_preview: input_preview.clone(),
                    output_preview: None,
                    status: ExecCommandStatus::InProgress,
                    exit_code: None,
                    duration_ms: None,
                });
                self.transition_to(WorkerState::Running(RunningPhase::ExecutingTool {
                    name: source.clone().unwrap_or_default(),
                }))?;
                Ok(WorkerOutput {
                    commands: vec![RuntimeCommand::EmitProtocol(
                        ProtocolEvent::item_started(self.id.clone(), call_id.clone(), ItemKind::ToolCall)
                    )],
                })
            }
            
            DomainEvent::ExecCommandOutputDelta { delta, .. } => {
                if let Some(TranscriptEntry::ExecCommand { output_preview, status: ExecCommandStatus::InProgress, .. }) = self.transcript.last_mut() {
                    output_preview.get_or_insert_with(String::new).push_str(delta);
                }
                Ok(WorkerOutput {
                    commands: vec![RuntimeCommand::EmitProtocol(
                        ProtocolEvent::item_delta(self.id.clone(), ItemDelta::Text(delta.clone()))
                    )],
                })
            }
            
            DomainEvent::ExecCommandFinished { call_id, output_preview, status, exit_code, duration_ms, source } => {
                if let Some(TranscriptEntry::ExecCommand { output_preview: out, status: st, exit_code: ec, duration_ms: dm, .. }) = self.transcript.last_mut() {
                    *out = output_preview.clone();
                    *st = status.clone();
                    *ec = *exit_code;
                    *dm = *duration_ms;
                }
                self.transition_to(WorkerState::Running(RunningPhase::Generating))?;
                Ok(WorkerOutput {
                    commands: vec![
                        RuntimeCommand::EmitProtocol(ProtocolEvent::item_completed(
                            self.id.clone(), call_id.clone(), /* ... */
                        )),
                        RuntimeCommand::ClassifyForDecision(self.id.clone(), event.clone()),
                    ],
                })
            }
            
            DomainEvent::WorkerFinished => {
                let was_active = self.state.is_running();
                self.transition_to(WorkerState::Idle)?;
                self.detach_behavior();
                
                let mut commands = vec![
                    RuntimeCommand::EmitProtocol(ProtocolEvent::status_changed(
                        self.id.clone(), AgentSlotStatus::Idle
                    )),
                ];
                
                if was_active {
                    commands.push(RuntimeCommand::RequestDecision(
                        self.id.clone(),
                        DecisionRequest::completion(self.id.clone()),
                    ));
                }
                
                Ok(WorkerOutput { commands })
            }
            
            DomainEvent::WorkerFailed { reason } => {
                self.transcript.push(TranscriptEntry::Error(reason.clone()));
                self.transition_to(WorkerState::Error { message: reason.clone() })?;
                self.detach_behavior();
                
                Ok(WorkerOutput {
                    commands: vec![
                        RuntimeCommand::EmitProtocol(ProtocolEvent::error(reason, self.id.clone())),
                        RuntimeCommand::RequestDecision(
                            self.id.clone(),
                            DecisionRequest::error(self.id.clone(), reason.clone()),
                        ),
                    ],
                })
            }
            
            // ... other event branches
            
            _ => Ok(WorkerOutput { commands: vec![] }),
        }
    }
}
```

---

## 6. WorkerState State Machine

### 6.1 State Definition

```rust
// agent-runtime/src/state.rs

/// Worker lifecycle state machine.
///
/// Design principles:
/// - Top-level states exhaust all possible phases of Worker
/// - Sub-states (RunningPhase, PausedReason) refine variants within the same phase
/// - No "implicit states" exist; all states are explicit
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerState {
    /// Idle: waiting for input or task assignment
    Idle,
    
    /// Starting: creating behavior thread
    Starting,
    
    /// Running: interacting with LLM
    Running(RunningPhase),
    
    /// Paused: blocked or waiting, recoverable
    Paused(PausedReason),
    
    /// Error: unrecoverable error occurred, can be manually reset
    Error { message: String },
    
    /// Stopped: terminated, not recoverable (unless Worker is restarted)
    Stopped { reason: String },
}

/// Sub-states of the running phase.
#[derive(Debug, Clone, PartialEq)]
pub enum RunningPhase {
    /// LLM generating text response
    Generating,
    
    /// LLM outputting chain-of-thought
    Thinking,
    
    /// External tool/command executing
    ExecutingTool { name: String },
    
    /// Waiting for user supplementary input
    WaitingForInput,
    
    /// Behavior thread ended, wrapping up
    Finishing,
}

/// Pause reasons.
#[derive(Debug, Clone, PartialEq)]
pub enum PausedReason {
    /// Generic blockage (tool failure, insufficient permissions, etc.)
    Blocked { reason: String },
    
    /// Waiting for decision layer/human confirmation
    BlockedForDecision { state: DecisionState },
    
    /// Rate limit cooldown period
    Resting { until: Instant, resume_action: String },
    
    /// User explicitly paused
    UserRequested { reason: String },
}
```

### 6.2 State Transition Rules

```rust
impl WorkerState {
    /// Check whether state transition is legal.
    ///
    /// All transitions must be explicitly declared. Illegal transitions return errors.
    pub fn can_transition_to(&self, target: &WorkerState) -> bool {
        use WorkerState::*;
        
        match (self, target) {
            // Normal lifecycle
            (Idle, Starting) => true,
            (Starting, Running(RunningPhase::Generating)) => true,
            (Running(RunningPhase::Generating), Running(RunningPhase::Thinking)) => true,
            (Running(RunningPhase::Generating), Running(RunningPhase::ExecutingTool { .. })) => true,
            (Running(RunningPhase::Generating), Running(RunningPhase::WaitingForInput)) => true,
            (Running(RunningPhase::Thinking), Running(RunningPhase::Generating)) => true,
            (Running(RunningPhase::ExecutingTool { .. }), Running(RunningPhase::Generating)) => true,
            (Running(RunningPhase::WaitingForInput), Running(RunningPhase::Generating)) => true,
            (Running(_), Running(RunningPhase::Finishing)) => true,
            (Running(RunningPhase::Finishing), Idle) => true,
            
            // Pause/recovery
            (Running(_), Paused(_)) => true,
            (Paused(PausedReason::Blocked { .. }), Idle) => true,
            (Paused(PausedReason::Blocked { .. }), Running(RunningPhase::Generating)) => true,
            (Paused(PausedReason::BlockedForDecision { .. }), Paused(PausedReason::Resting { .. })) => true,
            (Paused(PausedReason::Resting { .. }), Idle) => true,
            (Paused(PausedReason::UserRequested { .. }), Idle) => true,
            (Paused(PausedReason::UserRequested { .. }), Running(RunningPhase::Generating)) => true,
            
            // Error recovery
            (Error { .. }, Idle) => true,
            (Error { .. }, Stopped { .. }) => true,
            
            // Terminate (any non-terminal state can go to Stopped)
            (Idle, Stopped { .. }) => true,
            (Starting, Stopped { .. }) => true,
            (Running(_), Stopped { .. }) => true,
            (Paused(_), Stopped { .. }) => true,
            (Error { .. }, Stopped { .. }) => true,
            
            // Restart
            (Stopped { .. }, Starting) => true,
            
            // Other transitions illegal
            _ => false,
        }
    }
    
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running(_))
    }
    
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
    
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped { .. })
    }
    
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Paused(reason) if reason.is_blocked())
    }
    
    pub fn is_waiting_for_input(&self) -> bool {
        matches!(self, Self::Running(RunningPhase::WaitingForInput))
    }
    
    pub fn is_paused(&self) -> bool {
        matches!(self, Self::Paused(_))
    }
}

impl PausedReason {
    pub fn is_blocked(&self) -> bool {
        !matches!(self, Self::UserRequested { .. })
    }
}
```

### 6.3 Comparison with Original State Machine

| Dimension | Original Design (`AgentSlotStatus`) | New Design (`WorkerState`) |
|:---|:---|:---|
| Top-level state count | 14 | 6 |
| Sub-state hierarchy | None (all flat) | `RunningPhase`(5) + `PausedReason`(4) |
| Transition rule count | ~40 rules | ~25 rules |
| `is_blocked()` implementation | match 3 states | match `Paused` + `reason.is_blocked()` |
| `is_active()` implementation | match 4 states | match `Running` |
| Error recovery path | `Error → Idle` | `Error → Idle` |
| Terminal state | `Stopped` | `Stopped` |
| New state extensibility | Add top-level state | Add `RunningPhase` or `PausedReason` |

**Key improvement**: `Resting` is no longer an independent state, but `Paused(Resting { .. })`; `WaitingForInput` is no longer an independent state, but `Running(WaitingForInput)`. Semantic hierarchy is clear, but business meaning is fully preserved.

---

## 7. DomainEvent Unified Event System

### 7.1 Unified Event Type

```rust
// agent-events/src/lib.rs

/// Domain event.
///
/// Unified representation of all events in the system. Regardless of whether
/// the event source is:
/// - Claude/Codex process output
/// - Mock simulation
/// - System timer (such as idle timeout)
/// - User input
///
/// All are represented by DomainEvent.
#[derive(Debug, Clone, PartialEq)]
pub enum DomainEvent {
    // ===== Lifecycle =====
    WorkerStarted,
    WorkerFinished,
    WorkerFailed { reason: String },
    SessionAcquired { handle: SessionHandle },
    
    // ===== Streaming output =====
    AssistantChunk { text: String },
    ThinkingChunk { text: String },
    StatusUpdate { text: String },
    
    // ===== Command execution =====
    ExecCommandStarted {
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    },
    ExecCommandOutputDelta {
        call_id: Option<String>,
        delta: String,
    },
    ExecCommandFinished {
        call_id: Option<String>,
        output_preview: Option<String>,
        status: ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    
    // ===== Generic tools =====
    ToolCallStarted {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    },
    ToolCallFinished {
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },
    
    // ===== Code operations =====
    PatchApplyStarted {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    },
    PatchApplyOutputDelta {
        call_id: Option<String>,
        delta: String,
    },
    PatchApplyFinished {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        status: PatchApplyStatus,
    },
    
    // ===== MCP =====
    McpToolCallStarted {
        call_id: Option<String>,
        invocation: McpInvocation,
    },
    McpToolCallFinished {
        call_id: Option<String>,
        invocation: McpInvocation,
        result_blocks: Vec<serde_json::Value>,
        error: Option<String>,
        status: McpToolCallStatus,
        is_error: bool,
    },
    
    // ===== Web search =====
    WebSearchStarted {
        call_id: Option<String>,
        query: String,
    },
    WebSearchFinished {
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    },
    
    // ===== Images =====
    ImageViewRequested {
        call_id: Option<String>,
        path: String,
    },
    ImageGenerationFinished {
        call_id: Option<String>,
        revised_prompt: Option<String>,
        result: Option<String>,
        saved_path: Option<String>,
    },
    
    // ===== System events =====
    IdleTimeout,
}
```

### 7.2 Decision Event Subset

```rust
// agent-events/src/decision.rs

/// Decision layer focused event subset.
///
/// Converted through `From<&DomainEvent>`, filtering out streaming details
/// that the decision layer doesn't care about.
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionEvent {
    Completion,
    Error { message: String },
    ToolFailed { tool_name: String, exit_code: Option<i32> },
    PatchFailed { path: String, status: PatchApplyStatus },
    McpFailed { invocation: String, error: String },
    ApprovalRequired { request_id: String, method: String },
    RateLimited { retry_after: Duration },
    Idle,
}

impl From<&DomainEvent> for Option<DecisionEvent> {
    fn from(event: &DomainEvent) -> Self {
        match event {
            DomainEvent::WorkerFinished => Some(DecisionEvent::Completion),
            DomainEvent::WorkerFailed { reason } => Some(DecisionEvent::Error { message: reason.clone() }),
            DomainEvent::ExecCommandFinished { status, .. } if !status.is_success() => {
                Some(DecisionEvent::ToolFailed { tool_name: "exec".to_string(), exit_code: None })
            }
            DomainEvent::PatchApplyFinished { status: PatchApplyStatus::Failed | PatchApplyStatus::Declined, .. } => {
                Some(DecisionEvent::PatchFailed { path: "".to_string(), status: status.clone() })
            }
            DomainEvent::McpToolCallFinished { is_error: true, error, invocation, .. } => {
                Some(DecisionEvent::McpFailed { 
                    invocation: format!("{:?}", invocation), 
                    error: error.clone().unwrap_or_default() 
                })
            }
            DomainEvent::IdleTimeout => Some(DecisionEvent::Idle),
            _ => None,
        }
    }
}
```

### 7.3 Comparison with Original Three-Layer Events

| Level | Original Design | New Design |
|:---|:---|:---|
| Raw protocol | `agent/provider` parses JSON → internal structure | Unchanged |
| Domain events | `agent/provider::ProviderEvent` + `agent/core` re-export | `agent-events::DomainEvent` (unified) |
| Decision events | `decision::ProviderEvent` (simplified version, manual mapping) | `agent-events::DecisionEvent` (`From` automatic conversion) |
| Protocol broadcast | `agent/protocol::EventPayload` (independent design) | Unchanged |

**Benefits**:
- Delete `core/src/pool/event_converter.rs` (~300 lines of manual mapping code)
- When adding new event variants, only need to add to `DomainEvent`, and decide in `From` whether decision is needed
- Compile-time guarantee: if a new variant needs decision but isn't handled in `From`, decision classification tests will fail

---

## 8. EventLoop Scheduler

### 8.1 Design Goals

`EventLoop` (formerly `tick()`) must satisfy:
1. **Readability**: One glance knows what phases one loop has
2. **Testability**: Each phase can be tested independently
3. **No side effect penetration**: Does not directly modify Worker internal state, indirectly modifies through `Worker::apply()`
4. **Explicit order**: Dependencies between phases are explicitly expressed

### 8.2 Implementation

```rust
// agent-runtime/src/event_loop.rs

/// Event loop.
///
/// Executes every 100ms, the core scheduler of the system.
/// It is not responsible for business logic, only for: collect events →
/// dispatch to Workers → collect commands → execute commands.
pub struct EventLoop {
    state: Mutex<RuntimeState>,
}

struct RuntimeState {
    pool: WorkerPool,
    mux: EventMux,
    decision: DecisionCoordinator,
    protocol: ProtocolGateway,
    mail: InterWorkerMail,
}

impl EventLoop {
    pub async fn tick(&self) -> Result<Vec<ProtocolEvent>, RuntimeError> {
        let mut state = self.state.lock().await;
        let mut broadcast: Vec<ProtocolEvent> = Vec::new();
        
        // ===== Phase 1: Collect =====
        let batch = state.mux.poll_all();
        
        // ===== Phase 2: Process Worker Events =====
        let mut all_commands: Vec<RuntimeCommand> = Vec::new();
        
        for delivery in batch.events {
            if let Some(worker) = state.pool.get_mut(&delivery.worker_id) {
                match worker.apply(&delivery.event) {
                    Ok(output) => all_commands.extend(output.commands),
                    Err(e) => {
                        tracing::warn!(
                            worker_id = %delivery.worker_id,
                            error = %e,
                            "Worker event processing failed"
                        );
                        all_commands.push(RuntimeCommand::EmitProtocol(
                            ProtocolEvent::error(&e.to_string(), delivery.worker_id)
                        ));
                    }
                }
            } else {
                tracing::warn!(worker_id = %delivery.worker_id, "Event for unknown worker");
            }
        }
        
        // ===== Phase 3: Execute Commands =====
        for cmd in all_commands {
            match cmd {
                RuntimeCommand::EmitProtocol(event) => broadcast.push(event),
                
                RuntimeCommand::RequestDecision(worker_id, request) => {
                    if let Err(e) = state.decision.dispatch(&worker_id, request) {
                        tracing::warn!(worker_id = %worker_id, error = %e, "Decision dispatch failed");
                    }
                }
                
                RuntimeCommand::StartBehavior { worker_id, prompt, record_transcript } => {
                    if let Err(e) = Self::start_behavior(&mut state, &worker_id, &prompt, record_transcript) {
                        tracing::warn!(worker_id = %worker_id, error = %e, "Behavior start failed");
                    }
                }
                
                RuntimeCommand::Log { level, message } => {
                    log::log!(level, "{}", message);
                }
            }
        }
        
        // ===== Phase 4: Poll Decision Engines =====
        let decisions = state.decision.poll_completed();
        for (worker_id, outcome) in decisions {
            if let Some(worker) = state.pool.get(&worker_id) {
                let commands = state.decision.executor.execute(worker, &outcome);
                for cmd in commands {
                    match cmd {
                        DecisionCommand::AppendTranscript { entry } => {
                            if let Some(worker) = state.pool.get_mut(&worker_id) {
                                worker.transcript_mut().push(entry);
                            }
                        }
                        DecisionCommand::TransitionState { state: new_state } => {
                            if let Some(worker) = state.pool.get_mut(&worker_id) {
                                if let Err(e) = worker.transition_to(new_state) {
                                    tracing::warn!(error = %e, "Decision state transition failed");
                                }
                            }
                        }
                        DecisionCommand::StartBehavior { prompt } => {
                            if let Err(e) = Self::start_behavior(&mut state, &worker_id, &prompt, false) {
                                tracing::warn!(error = %e, "Decision behavior start failed");
                            }
                        }
                        DecisionCommand::EscalateToHuman { request } => {
                            state.decision.human_queue().push(request);
                        }
                    }
                }
            }
        }
        
        // ===== Phase 5: Idle Check =====
        let idle_workers: Vec<AgentId> = state.pool.workers()
            .filter(|w| w.state().is_idle())
            .filter(|w| w.last_activity().elapsed() > IDLE_TIMEOUT)
            .filter(|w| w.can_trigger_idle_decision(IDLE_DECISION_COOLDOWN))
            .map(|w| w.id().clone())
            .collect();
        
        for worker_id in idle_workers {
            if let Some(worker) = state.pool.get_mut(&worker_id) {
                worker.mark_idle_triggered();
            }
            if let Err(e) = state.decision.dispatch(
                &worker_id,
                DecisionRequest::idle(worker_id.clone()),
            ) {
                tracing::warn!(error = %e, "Idle decision dispatch failed");
            }
        }
        
        // ===== Phase 6: Process Inter-Worker Mail =====
        state.mail.process_pending();
        
        // ===== Phase 7: Cleanup Disconnected Channels =====
        for worker_id in batch.disconnected {
            state.mux.remove(&worker_id);
            if let Some(worker) = state.pool.get_mut(&worker_id) {
                worker.detach_behavior();
                if worker.state().is_running() {
                    if let Err(e) = worker.transition_to(WorkerState::Idle) {
                        tracing::warn!(error = %e, "Post-disconnect transition failed");
                    }
                }
            }
        }
        
        Ok(broadcast)
    }
    
    fn start_behavior(
        state: &mut RuntimeState,
        worker_id: &AgentId,
        prompt: &str,
        record_transcript: bool,
    ) -> Result<(), RuntimeError> {
        let worker = state.pool.get(worker_id)
            .ok_or(RuntimeError::WorkerNotFound)?;
        
        if worker.has_behavior() {
            return Err(RuntimeError::WorkerBusy);
        }
        
        let provider = worker.provider();
        let cwd = worker.worktree().map(|w| w.path.clone())
            .unwrap_or_else(|| state.pool.cwd().clone());
        let session = worker.session().cloned();
        
        let handle = BehaviorSpawner::spawn(provider, prompt, cwd, session)?;
        
        let worker = state.pool.get_mut(worker_id).unwrap();
        worker.attach_behavior(handle.clone());
        state.mux.register(worker_id.clone(), handle.receiver);
        
        if record_transcript {
            worker.transcript_mut().push(TranscriptEntry::User(prompt.to_string()));
        }
        
        Ok(())
    }
}
```

### 8.3 Comparison with Original tick()

| Dimension | Original `tick()` (~1000 lines) | New `EventLoop::tick()` (~150 lines) |
|:---|:---|:---|
| Phase count | None (all logic inline) | 7 explicit Phases |
| Worker state update | Inline match (~400 lines) | `worker.apply(event)` (encapsulated within Worker) |
| Protocol conversion | Inline `pump.process()` | `RuntimeCommand::EmitProtocol` |
| Decision trigger | Inline `classify_event()` + `send_decision_request()` | `RuntimeCommand::RequestDecision` |
| Error handling | Multiple `let _ =` ignoring errors | Unified `match` + `tracing::warn` |
| Testability | Requires constructing complete SessionInner | Each Phase independently testable |

---

## 9. Command Side Effect System

### 9.1 Design

The `apply()` method of Worker does not execute side effects, only returns `RuntimeCommand`. Side effects are uniformly executed in `EventLoop`.

```rust
// agent-runtime/src/command.rs

/// Commands requested by Worker for the runtime to execute after processing
/// events.
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeCommand {
    /// Broadcast protocol event to clients
    EmitProtocol(ProtocolEvent),
    
    /// Request decision layer classification
    RequestDecision(AgentId, DecisionRequest),
    
    /// Start behavior thread
    StartBehavior {
        worker_id: AgentId,
        prompt: String,
        record_transcript: bool,
    },
    
    /// Log
    Log {
        level: LogLevel,
        message: String,
    },
}

/// Commands requested by decision layer for the runtime to execute.
#[derive(Debug, Clone, PartialEq)]
pub enum DecisionCommand {
    /// Append Transcript entry
    AppendTranscript { entry: TranscriptEntry },
    
    /// Transfer Worker state
    TransitionState { state: WorkerState },
    
    /// Start behavior thread
    StartBehavior { prompt: String },
    
    /// Escalate to human decision
    EscalateToHuman { request: HumanDecisionRequest },
}
```

### 9.2 Why Use Command Instead of Effect

| Approach | Advantages | Disadvantages |
|:---|:---|:---|
| **Command (adopted)** | Clear semantics ("command"), Rust idiomatic, serializable | Small Vec allocation |
| **Callback/FnMut** | Zero allocation | Complex lifetimes, not cloneable, difficult to debug |
| **SmallVec** | Stack allocation optimization | Introduces external dependency, complex API |

**Conclusion**: `Vec<RuntimeCommand>` allocation overhead is completely negligible within a 100ms tick cycle (at most dozens of Commands). Code clarity and testability take priority over micro-optimization.

---

## 10. Decision Layer Integration

### 10.1 Eliminating Circular Dependencies

In the original architecture, `agent-decision`'s `DecisionExecutor` directly modified `AgentSlot`, causing circular dependency. In the new architecture:

```
agent-decision ──► agent-runtime (read-only Worker)
    │
    └── Produces DecisionCommand

agent-runtime ──► agent-decision (creates DecisionEngine)
    │
    └── Executes DecisionCommand
```

### 10.2 DecisionEngine Design

```rust
// agent-decision/src/engine.rs

/// Decision engine.
///
/// Each Worker corresponds to one DecisionEngine instance.
/// It receives DomainEvent, makes autonomous decisions, outputs DecisionCommand.
pub struct DecisionEngine {
    id: AgentId,                    // Corresponding Worker ID
    state: EngineState,
    tiered_engine: TieredDecisionEngine,
    mail: DecisionMailBox,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineState {
    Idle,
    Thinking { started_at: Instant },
    Responding,
    Error { message: String },
    Stopped,
}

impl DecisionEngine {
    /// Receive decision request, start background thinking thread.
    pub fn dispatch(&mut self, request: DecisionRequest) -> Result<(), DecisionError>;
    
    /// Poll completed decision results.
    pub fn poll_completed(&mut self) -> Vec<(AgentId, DecisionOutcome)>;
    
    /// Execute decision output, produce command list.
    ///
    /// Note: This method only reads Worker, does not modify Worker state.
    pub fn execute(&self, worker: &Worker, outcome: &DecisionOutcome) -> Vec<DecisionCommand>;
}
```

### 10.3 Classifier Design

```rust
// agent-decision/src/classifier.rs

pub struct SituationClassifier;

impl SituationClassifier {
    pub fn classify(&self, worker: &Worker, event: &DomainEvent) -> ClassifyResult {
        // 1. Convert to decision event (filter out irrelevant streaming events)
        let decision_event: Option<DecisionEvent> = event.into();
        let Some(decision_event) = decision_event else {
            return ClassifyResult::Running;
        };
        
        // 2. Classify based on Worker state and event type
        match (&worker.state, &decision_event) {
            (WorkerState::Running(RunningPhase::Finishing), DecisionEvent::Completion) => {
                ClassifyResult::NeedsDecision {
                    situation: Situation::ClaimsCompletion,
                    context: DecisionContext::new(worker),
                }
            }
            (_, DecisionEvent::Error { message }) => {
                if message.contains("429") || message.contains("rate_limit") {
                    ClassifyResult::NeedsDecision {
                        situation: Situation::RateLimited,
                        context: DecisionContext::new(worker),
                    }
                } else {
                    ClassifyResult::NeedsDecision {
                        situation: Situation::Error,
                        context: DecisionContext::new(worker),
                    }
                }
            }
            (WorkerState::Idle, DecisionEvent::Idle) => {
                ClassifyResult::NeedsDecision {
                    situation: Situation::AgentIdle,
                    context: DecisionContext::new(worker),
                }
            }
            // ... other classification rules
            _ => ClassifyResult::Running,
        }
    }
}
```

---

## 11. Protocol Layer

### 11.1 ProtocolGateway

```rust
// agent-daemon/src/protocol_gateway.rs

/// Protocol gateway.
///
/// Converts internal DomainEvent to external ProtocolEvent.
/// No business logic, pure format conversion.
pub struct ProtocolGateway {
    seq: u64,
    items: ItemTracker,
}

impl ProtocolGateway {
    pub fn convert(&mut self, worker_id: &AgentId, event: &DomainEvent) -> Vec<ProtocolEvent> {
        match event {
            DomainEvent::AssistantChunk { text } => {
                let item_id = self.items.get_or_create(worker_id, None, ItemKind::AssistantOutput);
                vec![ProtocolEvent::item_delta(item_id, ItemDelta::Text(text.clone()))]
            }
            DomainEvent::WorkerFinished => {
                vec![ProtocolEvent::status_changed(worker_id.clone(), AgentSlotStatus::Idle)]
            }
            DomainEvent::WorkerFailed { reason } => {
                vec![
                    ProtocolEvent::error(reason, worker_id.clone()),
                    ProtocolEvent::status_changed(worker_id.clone(), AgentSlotStatus::Blocked),
                ]
            }
            // ... other conversions
            _ => vec![],
        }
    }
}
```

### 11.2 Backward Compatibility

External protocol JSON structure **does not change at all**. `ProtocolGateway` conversion logic ensures clients (TUI, Web UI) need no modifications.

---

## 12. Migration Roadmap

### Phase 0: Infrastructure Preparation (1 week)

**Goal**: Create `agent-events`, promote `ProviderEvent` to `DomainEvent`.

**Steps**:
1. Create `agent-events` crate
2. Define `DomainEvent` (copy from `ProviderEvent`, rename)
3. Define `DecisionEvent` and `From<&DomainEvent>`
4. All crates compile (use type aliases for compatibility)

### Phase 1: Worker Aggregate Root (2 weeks)

**Goal**: Build `Worker` struct (copy from `AgentSlot`, rename fields).

**Steps**:
1. Create `Worker` struct
2. Implement `Worker::apply()` (migrate match branches from `tick()`)
3. Write unit tests for all 24 DomainEvent variants
4. Define `WorkerState` state machine (migrate from `AgentSlotStatus`)
5. State transition rule tests cover all 25 rules

### Phase 2: EventLoop Refactoring (2 weeks)

**Goal**: Split `tick()` into 7 Phases.

**Steps**:
1. Introduce `RuntimeCommand` system
2. `Worker::apply()` returns `RuntimeCommand`
3. `EventLoop` executes `RuntimeCommand`
4. All daemon integration tests pass

### Phase 3: Decision Layer Decoupling (2 weeks)

**Goal**: Introduce `DecisionCommand`.

**Steps**:
1. Introduce `DecisionCommand`
2. `DecisionExecutor` changes to read-only Worker
3. Eliminate write dependency from `agent-decision` → `agent-runtime`
4. Circular dependency elimination verification

### Phase 4: Protocol Layer Migration (1 week)

**Goal**: Separate `ProtocolGateway` from daemon.

**Steps**:
1. `ProtocolGateway` separated from daemon
2. External protocol format unchanged
3. Protocol conversion unit tests

### Phase 5: Type Renaming (1 week)

**Goal**: Execute type renaming.

**Steps**:
1. Batch rename using Rust Analyzer
2. Update all documentation and tests
3. Snapshot format compatibility verification (JSON field names unchanged)

### Phase 6: Cleanup and Optimization (1 week)

**Goal**: Delete obsolete code, full test suite.

**Steps**:
1. Delete obsolete code (`event_converter.rs`, etc.)
2. Full test suite
3. Performance benchmark comparison (ensure no regression)

**Total: 10 weeks**

---

## Conclusion

The core belief of this plan: **Core architecture is not worth compromising on.**

- `AgentSlot` is not Worker → Change it
- `tick()` is not a scheduler → Split it
- `ProviderEvent` is not a domain event → Unify it
- 14 states is not a clear state machine → Reorganize it
- Decision layer directly modifies Worker → Decouple it

Every change has a cost, but the cost of leaving technical debt is higher. A 10-week refactoring investment yields a system with correct naming, clear boundaries, exhaustive state machines, and isolated side effects. This is an architecture that can be maintained for 5+ years.
