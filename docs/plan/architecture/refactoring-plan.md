# Provider/Agent Actor Architecture Refactoring Plan

> This document proposes a systematic refactoring plan based on the diagnostic conclusions of `provider-agent-actor.md`.
>
> **Goal**: Make the system conform to the 7 software architecture principles, the Actor design pattern, and Domain-Driven Design (DDD), thereby better embracing change.

---

## Table of Contents

1. [Refactoring Philosophy: 7 Principles × Actor × DDD](#1-refactoring-philosophy-7-principles--actor--ddd)
2. [DDD Domain Partitioning (Bounded Contexts)](#2-ddd-domain-partitioning-bounded-contexts)
3. [Type Renaming Scheme](#3-type-renaming-scheme)
4. [Crate Reorganization Plan](#4-crate-reorganization-plan)
5. [Core Refactoring: Separation of Scheduler and Business Logic](#5-core-refactoring-separation-of-scheduler-and-business-logic)
6. [Messaging System Unification: One Type Through Three Layers](#6-messaging-system-unification-one-type-through-three-layers)
7. [State Machine Refactoring: Aggregate Root Cohesion](#7-state-machine-refactoring-aggregate-root-cohesion)
8. [Effect System: Side Effects Made Explicit](#8-effect-system-side-effects-made-explicit)
9. [Post-Refactoring Dependency Diagram](#9-post-refactoring-dependency-diagram)
10. [Migration Roadmap](#10-migration-roadmap)

---

## 1. Refactoring Philosophy: 7 Principles × Actor × DDD

### 1.1 Mapping of the 7 Software Architecture Principles

| Principle | Current Problem | Refactoring Direction |
|:---|:---|:---|
| **S — Single Responsibility** | `tick()` plays scheduler + state updater + decision trigger + protocol converter + mail deliverer | Split into `ActorRuntime` (pure scheduling) + `WorkAgent::handle_message()` (business) + `ProtocolGateway` (conversion) |
| **O — Open/Closed** | Adding a new ProviderEvent requires changing 5 match branches in `tick()` | Introduce `MessageHandler` trait; new event types only need to implement one trait |
| **L — Liskov Substitution** | `AgentPool` has special branch `if provider_kind != ProviderKind::Mock` | Introduce `BehaviorStrategy` trait; `MockStrategy`, `ClaudeStrategy`, `CodexStrategy` are interchangeable |
| **I — Interface Segregation** | `AgentPool` is a 4500+ line "god class" containing lifecycle + decision + task + focus + worktree | Split into `ActorSupervisor`, `DecisionCoordinator`, `TaskRouter`, `FocusManager`, `WorktreeManager` |
| **D — Dependency Inversion** | `agent-core` depends on `agent-provider` (ProviderEvent is defined in provider) | Extract `agent-domain-events` crate; core/provider/decision all depend on it |
| **LoD — Law of Demeter** | `tick()` directly accesses `inner.agent_pool.get_slot_mut_by_id(&agent_id).transcript_mut().last_mut()` | Through `WorkAgent::handle_message(msg)` encapsulate internal state; external code does not penetrate access |
| **CoR — Composition over Reuse** | `AgentPool` inline-implements all coordination logic | Build Supervisor by composing `DecisionCoordinator` + `WorktreeManager` + `MailboxRouter` |

### 1.2 Core Dogmas of the Actor Pattern

The Actor pattern has three inviolable dogmas; the current system partially obeys and partially violates them:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Actor Dogma 1: An Actor is an encapsulation of state + behavior + mailbox  │
│  ─────────────────────────────────────────────────────────────────────────  │
│  ✓ AgentSlot encapsulates state (transcript, status, session)               │
│  ✗ But behavior (how to process each ProviderEvent) is written in tick(),   │
│    not in AgentSlot                                                          │
│  ✗ The mailbox is mpsc::Receiver<ProviderEvent>, but "fetch and process"    │
│    logic is external                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│  Actor Dogma 2: Actors can only communicate through asynchronous messages   │
│  ─────────────────────────────────────────────────────────────────────────  │
│  ✓ ProviderThread only sends events through channel                         │
│  ✓ DecisionAgent receives requests through mail channel                     │
│  ✗ But tick() directly calls `slot.append_transcript()` — a synchronous     │
│    method call. Although single-threaded with no race, this is not Actor    │
│    pattern communication                                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│  Actor Dogma 3: The scheduler is only responsible for fetching messages     │
│    from mailboxes and dispatching them to Actors                            │
│  ─────────────────────────────────────────────────────────────────────────  │
│  ✗ tick() not only dispatches messages but also inline-executes all         │
│    business logic                                                            │
│  → Ideal form: tick() = for each actor { while let Some(msg) = mailbox.recv()│
│                              { actor.handle(msg) } }                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 DDD Strategic Design

The current system lacks explicit **Bounded Context** partitioning. We will partition 5 core domains:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Bounded Context Partitioning                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐      │
│  │  Runtime Context │    │  Process Context │    │ Decision Context │      │
│  │  ─────────────── │    │  ─────────────── │    │  ─────────────── │      │
│  │  Actor lifecycle  │◄──►│  LLM process startup│    │  Autonomous decision │      │
│  │  State machine    │    │  Protocol parsing │◄──►│  Classification/    │      │
│  │  Mailbox management│   │  Event production │    │  execution         │      │
│  │  Aggregate root:  │    │  Aggregate root:  │    │  Aggregate root:   │      │
│  │    WorkAgent      │    │    Behavior       │    │    Decision        │      │
│  └────────┬─────────┘    └──────────────────┘    └────────┬─────────┘      │
│           │                                                │               │
│           └────────────────────┬───────────────────────────┘               │
│                                │                                           │
│                                ▼                                           │
│                     ┌────────────────────┐                                 │
│                     │   Event Context    │                                 │
│                     │  ────────────────  │                                 │
│                     │  ActorMessage      │                                 │
│                     │  (Domain event     │                                 │
│                     │   definitions)     │                                 │
│                     └────────┬───────────┘                                 │
│                                │                                           │
│           ┌────────────────────┼────────────────────┐                     │
│           │                    │                    │                     │
│           ▼                    ▼                    ▼                     │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐        │
│  │ Protocol Context │  │  Coordination    │  │  Persistence     │        │
│  │  ─────────────── │  │  Context         │  │  Context         │        │
│  │  Protocol event   │  │  ActorRuntime    │  │  Snapshot        │        │
│  │  serialization    │  │  tick() scheduler│  │  Store           │        │
│  │  Aggregate root:  │  │  Aggregate root: │  │  Aggregate root: │        │
│  │    Gateway        │  │    Runtime       │  │    Store         │        │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. DDD Domain Partitioning (Bounded Contexts)

### 2.1 Runtime Context

**Core mission**: Manage Actor lifecycle, state, and mailboxes.

**Aggregate roots**:
- `WorkAgent` (formerly `AgentSlot`) — all state and behavior of a single work Actor
- `ActorSupervisor` (formerly the lifecycle portion of `AgentPool`) — Actor creation, stopping, destruction

**Entities**:
- `WorkAgentId` (formerly `AgentId`) — Actor unique identifier
- `ActorLifecycle` (formerly `AgentSlotStatus`) — state machine value object
- `TranscriptJournal` (formerly `transcript: Vec<TranscriptEntry>`) — immutable event log

**Value objects**:
- `Codename` (formerly `AgentCodename`) — display name
- `ProviderKind` — Claude / Codex / Mock
- `ActorRole` (formerly `AgentRole`) — ProductOwner / ScrumMaster / Developer
- `SessionHandle` — multi-turn session handle

**Domain services**:
- `LifecycleValidator` — state transition validation
- `MailboxMultiplexer` (formerly `EventAggregator`) — multiplexed mailbox polling

### 2.2 Process Context

**Core mission**: Start LLM subprocesses, parse their output, produce domain events.

**Aggregate roots**:
- `Behavior` (abstract concept, formerly ProviderThread) — one execution of the LLM process

**Entities**:
- `ClaudeBehavior` — Claude CLI concrete implementation
- `CodexBehavior` — Codex CLI concrete implementation
- `MockBehavior` — simulated implementation

**Value objects**:
- `LaunchSpec` — startup parameter specification
- `ProtocolFrame` — parsed raw protocol frame

**Domain services**:
- `BehaviorSpawner` — create corresponding Behavior according to strategy
- `ProtocolParser` — stream-json / exec-json parsing

### 2.3 Decision Context

**Core mission**: Observe Actor events, make autonomous decisions, send action instructions.

**Aggregate roots**:
- `DecisionActor` (formerly `DecisionAgentSlot`) — decision Actor

**Entities**:
- `Situation` — trigger situation
- `DecisionEngine` — tiered decision engine

**Value objects**:
- `ClassifyResult` — Running / NeedsDecision
- `DecisionOutput` — decision output (containing action list)
- `Confidence` — confidence level

**Domain services**:
- `SituationClassifier` — event classification
- `ActionExecutor` — action execution

### 2.4 Event Context

**Core mission**: Define message types passed between Actors. This is the **Shared Kernel**.

**Value objects**:
- `ActorMessage` (formerly `ProviderEvent`) — complete set of Actor messages
- `MessageEnvelope` — message wrapper with sender identification

### 2.5 Protocol Context

**Core mission**: Convert internal domain events into external protocol events.

**Aggregate roots**:
- `ProtocolGateway` (formerly `EventPump`) — protocol conversion gateway

**Value objects**:
- `ProtocolEvent` (formerly `Event`) — external broadcast event
- `ItemId` — protocol layer item identifier

### 2.6 Coordination Context

**Core mission**: Orchestrate all domains, drive the event loop.

**Aggregate roots**:
- `ActorRuntime` (formerly `SessionManager`) — runtime scheduler

**Domain services**:
- `RuntimeTick` — complete scheduling flow for a single tick
- `EffectRouter` — side effect routing

---

## 3. Type Renaming Scheme

### 3.1 Core Actor Types

| Current Name | New Name | Actor Role | Change Reason |
|:---|:---|:---|:---|
| `AgentSlot` | `WorkAgent` | **Actor** | `Slot` is a container metaphor implying "empty slot"; `Agent` is the role |
| `AgentSlotStatus` | `ActorLifecycle` | **State** | It is a lifecycle state, not "the status of a Slot" |
| `AgentPool` | `ActorSupervisor` | **Supervisor** | Directly reflects the supervisor role in the Actor pattern |
| `AgentId` | `WorkAgentId` | **Identity** | Distinguishes from `DecisionActorId` |
| `AgentCodename` | `ActorCodename` | **Value object** | Generalized, not just for "Agent" |
| `AgentRole` | `ActorRole` | **Value object** | Same as above |
| `SessionManager` | `ActorRuntime` | **Scheduler / Runtime** | `Session` is a business concept; `Runtime` is a technical role |
| `SessionInner` | `RuntimeState` | **Runtime state** | Clearly indicates it is the internal state of the runtime |

### 3.2 Messaging and Communication Types

| Current Name | New Name | Actor Role | Change Reason |
|:---|:---|:---|:---|
| `ProviderEvent` | `ActorMessage` | **Message** | It is a message received by the Actor; not just "an event from the Provider" (the Provider is only one of the message sources) |
| `EventAggregator` | `MailboxMultiplexer` | **Mailbox Mux** | It manages multiple Actor Mailboxes; `EventAggregator` is a procedural name |
| `AgentMailbox` | `InterActorMail` | **Inter-Actor Mail** | Avoid confusion with the Actor's own Mailbox; clearly indicates cross-Actor mail |
| `AgentEvent` | `MailboxDelivery` | **Delivery** | It is a mailbox delivery notification, not a generic "event" |
| `DecisionRequest` | `DecisionDirective` | **Directive** | It is an instruction sent to the DecisionActor; `Request` is too generic |
| `DecisionResponse` | `DecisionOutcome` | **Outcome** | It is the result of a decision; `Response` implies a request/response pattern |

### 3.3 Process and Behavior Types

| Current Name | New Name | Actor Role | Change Reason |
|:---|:---|:---|:---|
| `ProviderThread` | `BehaviorThread` | **Behavior** | It is the "behavior" execution thread of the Actor; `Provider` is the concrete implementation |
| `start_provider` | `spawn_behavior` | **Spawn** | Generalized, not just spawning a provider |
| `ProviderKind` | `BehaviorKind` | **Strategy Key** | It is the kind of Behavior strategy |
| `ProviderCapabilities` | `BehaviorCapabilities` | **Capability** | Same as above |
| `ProviderLaunchContext` | `BehaviorLaunchContext` | **Context** | Same as above |
| `ProviderLLMCaller` | `DecisionLLMCaller` | **LLM Caller** | It is only used in the decision layer; should not be called Provider |

### 3.4 Decision Layer Types

| Current Name | New Name | Actor Role | Change Reason |
|:---|:---|:---|:---|
| `DecisionAgentSlot` | `DecisionActor` | **Child Actor** | `Slot` is a container metaphor; `DecisionAgent` and `WorkAgent` are parallel |
| `DecisionAgentStatus` | `DecisionLifecycle` | **State** | Symmetric with `ActorLifecycle` |
| `DecisionAgentCoordinator` | `DecisionCoordinator` | **Coordinator** | Remove redundant `Agent` |
| `TieredDecisionEngine` | `TieredDecisionEngine` | **Engine** | Keep, name is already accurate |
| `DecisionExecutor` | `ActionExecutor` | **Executor** | It executes Actions, not Decisions |
| `DecisionExecutionResult` | `ActionOutcome` | **Outcome** | Result of action execution |

### 3.5 Protocol and External Types

| Current Name | New Name | Actor Role | Change Reason |
|:---|:---|:---|:---|
| `EventPump` | `ProtocolGateway` | **Gateway** | It is a gateway from domain → protocol; `Pump` is a process metaphor |
| `Event` (protocol) | `ProtocolEvent` | **Protocol Event** | Distinguish from `ActorMessage` |
| `EventPayload` | `ProtocolPayload` | **Payload** | Same as above |
| `SendInputResult` | `DispatchResult` | **Dispatch** | `SendInput` is a concrete business operation; `Dispatch` is a scheduling result |

### 3.6 Complete Naming Comparison Table

```
┌────────────────────────────┬────────────────────────────┐
│         BEFORE             │          AFTER             │
├────────────────────────────┼────────────────────────────┤
│ AgentSlot                  │ WorkAgent                  │
│ AgentSlotStatus            │ ActorLifecycle             │
│ AgentPool                  │ ActorSupervisor            │
│ AgentId                    │ WorkAgentId                │
│ AgentCodename              │ ActorCodename              │
│ AgentRole                  │ ActorRole                  │
│ SessionManager             │ ActorRuntime               │
│ SessionInner               │ RuntimeState               │
│ ProviderEvent              │ ActorMessage               │
│ EventAggregator            │ MailboxMultiplexer         │
│ AgentMailbox               │ InterActorMail             │
│ AgentEvent                 │ MailboxDelivery            │
│ DecisionRequest            │ DecisionDirective          │
│ DecisionResponse           │ DecisionOutcome            │
│ ProviderThread             │ BehaviorThread             │
│ start_provider             │ spawn_behavior             │
│ ProviderKind               │ BehaviorKind               │
│ ProviderCapabilities       │ BehaviorCapabilities       │
│ ProviderLaunchContext      │ BehaviorLaunchContext      │
│ DecisionAgentSlot          │ DecisionActor              │
│ DecisionAgentStatus        │ DecisionLifecycle          │
│ DecisionAgentCoordinator   │ DecisionCoordinator        │
│ DecisionExecutor           │ ActionExecutor             │
│ DecisionExecutionResult    │ ActionOutcome              │
│ EventPump                  │ ProtocolGateway            │
│ Event (protocol)           │ ProtocolEvent              │
│ EventPayload               │ ProtocolPayload            │
│ SendInputResult            │ DispatchResult             │
└────────────────────────────┴────────────────────────────┘
```

---

## 4. Crate Reorganization Plan

### 4.1 Current Crate Dependency Graph (Problem Version)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Current Crate Dependencies (Chaotic)                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   agent-provider ──► agent-types, agent-toolkit                            │
│        ▲                                                                    │
│        │                                                                    │
│   agent-core ──────► agent-provider, agent-decision, agent-types          │
│        ▲                    ▲                                              │
│        │                    │                                              │
│   agent-daemon ────► agent-core, agent-decision, agent-protocol          │
│        │                    │                    ▲                         │
│        │                    └────────────────────┘                         │
│        │                                                                   │
│   agent-tui ───────► agent-core, agent-protocol                            │
│                                                                             │
│   ⚠️ Issue: agent-core depends on agent-provider (inverted)               │
│   ⚠️ Issue: EventPump is in agent-daemon but handles protocol conversion   │
│   ⚠️ Issue: ProviderEvent is in agent-provider but used heavily by         │
│             core/decision                                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Target Crate Dependency Graph (Clean Version)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Target Crate Dependencies (Clean)                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   Layer 1: Shared Kernel (no business logic, pure type definitions)         │
│   ─────────────────────────────────────────────                             │
│                                                                             │
│        agent-domain-events ◄────── agent-types                              │
│              │                                                              │
│              ├── ActorMessage, MessageEnvelope, ActorMessageKind            │
│              ├── WorkAgentId, DecisionActorId                               │
│              ├── ActorCodename, ActorRole, BehaviorKind                     │
│              └── SessionHandle                                              │
│                                                                             │
│   Layer 2: Domain Layer (pure business logic, no IO)                        │
│   ─────────────────────────────────                                         │
│                                                                             │
│        agent-runtime-domain ◄────── agent-domain-events                     │
│              │                                                              │
│              ├── WorkAgent (Aggregate root)                                 │
│              ├── ActorLifecycle (State machine)                             │
│              ├── TranscriptJournal (Value object)                           │
│              ├── ActorSupervisor (Aggregate root)                           │
│              ├── MailboxMultiplexer (Domain service)                        │
│              └── InterActorMail (Domain service)                            │
│                                                                             │
│        agent-decision-domain ◄────── agent-domain-events                    │
│              │                                                              │
│              ├── DecisionActor (Aggregate root)                             │
│              ├── DecisionLifecycle (State machine)                          │
│              ├── Situation, DecisionEngine (Entities)                       │
│              ├── ClassifyResult, DecisionOutput (Value objects)             │
│              └── SituationClassifier, ActionExecutor (Domain services)      │
│                                                                             │
│   Layer 3: Infrastructure Layer (IO, processes, protocols)                  │
│   ───────────────────────────────────                                       │
│                                                                             │
│        agent-behavior-infra ◄────── agent-domain-events                     │
│              │                                                              │
│              ├── BehaviorSpawner (Domain service)                           │
│              ├── ClaudeBehavior, CodexBehavior, MockBehavior (Entities)     │
│              ├── ProtocolParser (Domain service)                            │
│              └── BehaviorLaunchContext, LaunchSpec (Value objects)          │
│                                                                             │
│        agent-protocol-infra ◄────── agent-domain-events                     │
│              │                                                              │
│              ├── ProtocolGateway (Aggregate root)                           │
│              ├── ProtocolEvent, ProtocolPayload (Value objects)             │
│              ├── ItemId, ItemKind (Value objects)                           │
│              └── JsonRpcHandler, WsBroadcaster (Infrastructure)             │
│                                                                             │
│   Layer 4: Application Layer (orchestration, scheduling, runtime)           │
│   ───────────────────────────────────                                       │
│                                                                             │
│        agent-runtime-app ◄────── agent-runtime-domain                       │
│              │                agent-decision-domain                          │
│              │                agent-behavior-infra                           │
│              │                agent-protocol-infra                           │
│              │                                                              │
│              ├── ActorRuntime (Aggregate root)                              │
│              ├── RuntimeState (Value object)                                │
│              ├── RuntimeTick (Domain service)                               │
│              ├── EffectRouter (Domain service)                              │
│              └── ShutdownSnapshot (Value object)                            │
│                                                                             │
│   Layer 5: Interface Layer (UI, CLI, Daemon)                                │
│   ─────────────────────────────────                                         │
│                                                                             │
│        agent-daemon ◄────────── agent-runtime-app, agent-protocol-infra    │
│              │                                                              │
│              ├── HTTP/WebSocket server                                       │
│              ├── CLI argument parsing                                        │
│              └── Main entry main.rs                                          │
│                                                                             │
│        agent-tui ◄───────────── agent-protocol-infra, agent-domain-events   │
│              │                                                              │
│              ├── Terminal UI rendering                                       │
│              ├── User input handling                                         │
│              └── WebSocket client                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Crate Responsibility Specifications

#### `agent-domain-events` (New)

**Responsibility**: Define all message types shared across domains and basic identifiers.

**Contains**:
- `ActorMessage` (formerly `ProviderEvent`)
- `WorkAgentId`, `DecisionActorId`
- `ActorCodename`, `ActorRole`, `BehaviorKind`
- `SessionHandle`
- `MessageEnvelope`

**Principle**: This crate **never contains any business logic**, only type definitions and simple methods (such as `is_running()`, `needs_decision()`). It is the "Shared Kernel" that all domain layers depend on.

#### `agent-runtime-domain` (Extracted from `agent-core`)

**Responsibility**: Pure business logic of the Actor runtime.

**Contains**:
- `WorkAgent` (Aggregate root)
- `ActorSupervisor` (Aggregate root, contains only lifecycle management)
- `ActorLifecycle` (State machine)
- `TranscriptJournal` (Value object)
- `MailboxMultiplexer` (Domain service)
- `InterActorMail` (Domain service)

**Does not contain**:
- Provider process startup (moved to `agent-behavior-infra`)
- Protocol conversion (moved to `agent-protocol-infra`)
- Decision coordination (moved to `agent-decision-domain`)
- HTTP/WebSocket (moved to `agent-daemon`)

#### `agent-decision-domain` (Extracted from `agent-decision`)

**Responsibility**: Pure business logic of autonomous decision making.

**Contains**:
- `DecisionActor` (Aggregate root)
- `DecisionLifecycle` (State machine)
- `Situation`, `DecisionEngine`
- `SituationClassifier`, `ActionExecutor`

**Does not contain**:
- LLM calling infrastructure (remains in `agent-decision`, or moved to `agent-behavior-infra`)

#### `agent-behavior-infra` (Renamed from `agent-provider`)

**Responsibility**: Start and manage LLM subprocesses.

**Contains**:
- `BehaviorSpawner`
- `ClaudeBehavior`, `CodexBehavior`, `MockBehavior`
- `ProtocolParser`
- `BehaviorLaunchContext`

**Principle**: This crate only produces `ActorMessage`, it does not consume them. It is an "event producer".

#### `agent-protocol-infra` (Merged from `agent/protocol` + `EventPump`)

**Responsibility**: External protocol serialization and conversion.

**Contains**:
- `ProtocolGateway` (formerly `EventPump`)
- `ProtocolEvent`, `ProtocolPayload`
- `JsonRpcHandler`, `WsBroadcaster`

**Principle**: Converts internal `ActorMessage` to external `ProtocolEvent` without touching business logic.

#### `agent-runtime-app` (Extracted from `agent-daemon`)

**Responsibility**: Orchestrate all domains, drive the event loop.

**Contains**:
- `ActorRuntime` (formerly `SessionManager`)
- `RuntimeTick` (single tick scheduling flow)
- `EffectRouter` (side effect routing)

**Principle**: This is the "use case layer", coordinating multiple aggregate roots to complete a full user scenario.

---

## 5. Core Refactoring: Separation of Scheduler and Business Logic

### 5.1 Current `tick()` Structure (Problem)

```rust
// agent/daemon/src/session_mgr.rs — Current implementation (pseudocode)
pub async fn tick(&self) -> Result<Vec<Event>> {
    let mut inner = self.inner.lock().await;
    let mut pump = EventPump::new();
    let mut broadcast_events = Vec::new();

    // === 1. Poll Provider events ===
    let poll_result = inner.event_aggregator.poll_all();
    for agent_event in poll_result.events {
        match agent_event {
            AgentEvent::FromProvider { agent_id, event } => {
                // 1a. Update activity timestamp
                if let Some(slot) = inner.agent_pool.get_slot_mut_by_id(&agent_id) {
                    slot.touch_activity();
                }

                // 1b. Update state based on event type (huge match)
                match &event {
                    ProviderEvent::Finished => { /* State→Idle, cleanup thread */ }
                    ProviderEvent::Error(e) => { /* State→Blocked, cleanup thread */ }
                    ProviderEvent::AssistantChunk(c) => { /* Append transcript */ }
                    ProviderEvent::ThinkingChunk(c) => { /* Append transcript */ }
                    ProviderEvent::Status(s) => { /* Append transcript */ }
                    ProviderEvent::ExecCommandStarted { ... } => { /* Append transcript */ }
                    ProviderEvent::ExecCommandOutputDelta { ... } => { /* Append delta */ }
                    ProviderEvent::ExecCommandFinished { ... } => { /* Update transcript */ }
                    // ... 15+ more branches
                }

                // 1c. Decision layer classification
                let classify_result = inner.agent_pool.classify_event(&agent_id, &event);
                if classify_result.is_needs_decision() {
                    let request = DecisionRequest::new(agent_id, ...);
                    inner.agent_pool.send_decision_request(&agent_id, request)?;
                }

                // 1d. Protocol conversion
                broadcast_events.extend(pump.process(agent_id, event));
            }
        }
    }

    // === 2. Cleanup disconnected channels ===
    for disconnected_id in poll_result.disconnected_channels {
        // ... cleanup ...
    }

    // === 3. Idle Agent trigger ===
    // ... 60s check ...

    // === 4. Poll Decision Agents ===
    let decision_responses = inner.agent_pool.poll_decision_agents();
    for (work_agent_id, response) in decision_responses {
        // ... execute decision actions ...
        // ... may start new Provider threads ...
    }

    // === 5. Process mail ===
    inner.mailbox.process_pending();

    Ok(broadcast_events)
}
```

**Problem analysis**:
- `tick()` violates **SRP**: it is simultaneously scheduler, state processor, decision trigger, and protocol converter
- Violates **OCP**: adding a new event type requires modifying match branches in `tick()`
- Violates **LoD**: `tick()` directly penetrates access to `slot.transcript_mut().last_mut()`
- Difficult to test: requires constructing a complete `SessionInner` context

### 5.2 Target Architecture: `MessageHandler` trait + `Effect` System

```rust
// ============================================================
// 5.2.1 Define Effect types (explicit side effects)
// ============================================================

/// Side effects that may be produced after an Actor processes a message.
/// Note: This does not include the Actor's own internal state changes (such as
/// state machine transitions), only side effects that require the external
/// world to execute.
pub enum ActorEffect {
    /// Send event to protocol gateway (external broadcast)
    EmitProtocol(ProtocolEvent),

    /// Send classification request to decision coordinator
    ClassifyForDecision {
        work_agent_id: WorkAgentId,
        message: ActorMessage,
    },

    /// Start Behavior thread (closed loop)
    SpawnBehavior {
        work_agent_id: WorkAgentId,
        prompt: String,
        record_in_transcript: bool,
    },

    /// Send cross-Actor mail
    SendInterActorMail {
        to: WorkAgentId,
        subject: MailSubject,
    },

    /// Log
    Log { level: LogLevel, message: String },

    /// No side effect
    NoOp,
}

// ============================================================
// 5.2.2 Define MessageHandler trait
// ============================================================

/// Actor message processor.
///
/// Each Actor type implements this trait, defining how it handles various
/// message types. This is the core of the Open/Closed Principle: adding a
/// new message type → add a new trait method or default implementation,
/// without modifying scheduler code.
pub trait MessageHandler {
    /// Process a message, returning produced side effects.
    ///
    /// Note: The Actor's internal state (transcript, status, last_activity)
    /// is directly updated within this method (because &mut self), and does
    /// not need to be returned via Effect.
    fn handle_message(&mut self, message: ActorMessage) -> Vec<ActorEffect>;
}

// ============================================================
// 5.2.3 WorkAgent implements MessageHandler
// ============================================================

impl MessageHandler for WorkAgent {
    fn handle_message(&mut self, message: ActorMessage) -> Vec<ActorEffect> {
        self.touch_activity();

        match &message {
            ActorMessage::Finished => self.handle_finished(),
            ActorMessage::Error(err) => self.handle_error(err),
            ActorMessage::AssistantChunk(text) => self.handle_assistant_chunk(text),
            ActorMessage::ThinkingChunk(text) => self.handle_thinking_chunk(text),
            ActorMessage::Status(text) => self.handle_status(text),
            ActorMessage::ExecCommandStarted { call_id, input_preview, source }
                => self.handle_exec_started(call_id, input_preview, source),
            ActorMessage::ExecCommandOutputDelta { call_id, delta }
                => self.handle_exec_delta(call_id, delta),
            ActorMessage::ExecCommandFinished { call_id, output_preview, status, exit_code, duration_ms, source }
                => self.handle_exec_finished(call_id, output_preview, status, exit_code, duration_ms, source),
            // ... other message types
            _ => vec![ActorEffect::NoOp],
        }
    }
}

impl WorkAgent {
    fn handle_finished(&mut self) -> Vec<ActorEffect> {
        if self.lifecycle.is_active() {
            let _ = self.transition_to(ActorLifecycle::idle());
        }
        self.clear_behavior_thread();

        vec![
            ActorEffect::EmitProtocol(ProtocolEvent::agent_status_changed(
                self.id(),
                AgentSlotStatus::Idle,
            )),
        ]
    }

    fn handle_error(&mut self, error: &str) -> Vec<ActorEffect> {
        self.transcript.push(TranscriptEntry::Error(error.to_string()));
        let _ = self.transition_to(ActorLifecycle::blocked(error.to_string()));
        self.clear_behavior_thread();

        vec![
            ActorEffect::EmitProtocol(ProtocolEvent::error(error, self.id())),
            ActorEffect::ClassifyForDecision {
                work_agent_id: self.id().clone(),
                message: ActorMessage::Error(error.to_string()),
            },
        ]
    }

    fn handle_assistant_chunk(&mut self, text: &str) -> Vec<ActorEffect> {
        self.transcript.push(TranscriptEntry::Assistant(text.to_string()));

        vec![
            ActorEffect::EmitProtocol(ProtocolEvent::item_delta(
                self.id(),
                ItemDelta::Text(text.to_string()),
            )),
        ]
    }

    // ... other handlers
}
```

### 5.3 Refactored `RuntimeTick`

```rust
// agent-runtime-app/src/runtime_tick.rs

/// Pure scheduling logic for a single tick.
/// Contains no business processing, only responsible for: fetch messages →
/// dispatch → collect Effects → route Effects.
pub struct RuntimeTick<'a> {
    supervisor: &'a mut ActorSupervisor,
    multiplexer: &'a mut MailboxMultiplexer,
    decision_coordinator: &'a mut DecisionCoordinator,
    protocol_gateway: &'a mut ProtocolGateway,
    inter_actor_mail: &'a mut InterActorMail,
}

impl<'a> RuntimeTick<'a> {
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        // === Step 1: Poll all mailboxes ===
        let delivery = self.multiplexer.poll_all();

        // === Step 2: Dispatch messages to Actors ===
        let mut all_effects: Vec<ActorEffect> = Vec::new();

        for MailboxDelivery::FromBehavior { agent_id, message } in delivery.messages {
            if let Some(agent) = self.supervisor.get_agent_mut(&agent_id) {
                let effects = agent.handle_message(message);
                all_effects.extend(effects);
            }
        }

        // === Step 3: Handle idle triggers ===
        for agent_id in self.supervisor.idle_agent_ids_older_than(Duration::from_secs(60)) {
            if self.supervisor.can_trigger_idle_decision(&agent_id) {
                all_effects.push(ActorEffect::ClassifyForDecision {
                    work_agent_id: agent_id.clone(),
                    message: ActorMessage::AgentIdle,
                });
                self.supervisor.mark_idle_triggered(&agent_id);
            }
        }

        // === Step 4: Poll decision actors ===
        let decision_outcomes = self.decision_coordinator.poll_all();
        for (work_agent_id, outcome) in decision_outcomes {
            let action_effects = self.decision_coordinator.execute_action(
                &work_agent_id,
                &outcome,
            );
            all_effects.extend(action_effects);
        }

        // === Step 5: Route all effects ===
        for effect in all_effects {
            self.route_effect(effect)?;
        }

        // === Step 6: Process inter-actor mail ===
        self.inter_actor_mail.process_pending();

        // === Step 7: Cleanup disconnected channels ===
        for agent_id in delivery.disconnected {
            self.multiplexer.remove_mailbox(&agent_id);
            if let Some(agent) = self.supervisor.get_agent_mut(&agent_id) {
                agent.clear_behavior_thread();
                if agent.lifecycle().is_active() {
                    let _ = agent.transition_to(ActorLifecycle::idle());
                }
            }
        }

        Ok(())
    }

    fn route_effect(&mut self, effect: ActorEffect) -> Result<(), RuntimeError> {
        match effect {
            ActorEffect::EmitProtocol(event) => {
                self.protocol_gateway.emit(event);
            }
            ActorEffect::ClassifyForDecision { work_agent_id, message } => {
                self.decision_coordinator.classify_and_dispatch(&work_agent_id, message)?;
            }
            ActorEffect::SpawnBehavior { work_agent_id, prompt, record_in_transcript } => {
                self.supervisor.spawn_behavior_for(&work_agent_id, &prompt, record_in_transcript)?;
            }
            ActorEffect::SendInterActorMail { to, subject } => {
                self.inter_actor_mail.send(to, subject);
            }
            ActorEffect::Log { level, message } => {
                log(level, message);
            }
            ActorEffect::NoOp => {}
        }
        Ok(())
    }
}
```

### 5.4 Refactoring Benefits

| Principle | Benefit |
|:---|:---|
| **SRP** | `RuntimeTick` only schedules, `WorkAgent` only handles business, `ProtocolGateway` only converts protocols |
| **OCP** | New `ActorMessage` variant → add a `handle_xxx()` method in `WorkAgent`, no need to change `RuntimeTick` |
| **LSP** | `BehaviorStrategy` trait allows `ClaudeStrategy`, `CodexStrategy`, `MockStrategy` to be used interchangeably |
| **ISP** | `ActorSupervisor` only handles lifecycle, `DecisionCoordinator` only handles decisions, `TaskRouter` only handles routing |
| **DIP** | `RuntimeTick` depends on `MessageHandler` trait, not on concrete `WorkAgent` |
| **LoD** | `RuntimeTick` no longer penetrates access to `slot.transcript_mut().last_mut()`, only calls `agent.handle_message(msg)` |
| **CoR** | `ActorSupervisor` is built by composing `DecisionCoordinator` + `WorktreeManager`, not by inlining all logic |

---

## 6. Messaging System Unification: One Type Through Three Layers

### 6.1 Current Problem

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              Current: Triple Representation of ProviderEvent                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   agent/provider/src/provider.rs                                            │
│   ├── ProviderEvent (24 variants, includes ExecCommandStarted/Finished etc)│
│   │                                                                         │
│   │  event_converter.rs manual mapping                                     │
│   ▼                                                                         │
│   decision/src/provider/provider_event.rs                                   │
│   ├── ProviderEvent (15 variants, Claude/Codex/ACP classification naming)  │
│   │                                                                         │
│   │  EventPump::process() manual mapping                                   │
│   ▼                                                                         │
│   agent/protocol/src/events.rs                                              │
│   ├── EventPayload (10 variants, ItemDelta/ItemStarted/AgentStatusChanged) │
│                                                                             │
│   ⚠️ Adding a new event type requires modifying 4 files                    │
│   ⚠️ Two ProviderEvents have the same name but different structures,       │
│      easily confused                                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Target: Unified as `ActorMessage`

```rust
// agent-domain-events/src/actor_message.rs

/// Actor message.
///
/// This is the only message type for communication between Actors. It is:
/// 1. Message from Behavior thread → WorkAgent (formerly ProviderEvent)
/// 2. Instruction carrier from DecisionActor → WorkAgent
/// 3. Event source for protocol gateway → external client
///
/// Reason for renaming from "ProviderEvent" to "ActorMessage":
/// - "Provider" implies the source is the LLM process, but messages can also
///   come from Mock, DecisionActor, or the system
/// - "Event" implies the observer pattern, but here it is the Actor message
///   passing pattern
/// - "Message" accurately reflects the communication primitive of the Actor
///   model
#[derive(Debug, Clone, PartialEq)]
pub enum ActorMessage {
    // ========== Lifecycle messages ==========
    /// Behavior thread started execution
    BehaviorStarted,
    /// Behavior thread ended normally
    BehaviorFinished,
    /// Behavior thread ended abnormally
    BehaviorFailed { reason: String },
    /// Session handle established (multi-turn continuity)
    SessionAcquired { handle: SessionHandle },

    // ========== Streaming output messages ==========
    /// LLM text output fragment
    AssistantChunk { text: String },
    /// LLM chain-of-thought fragment
    ThinkingChunk { text: String },
    /// Status hint
    StatusUpdate { text: String },

    // ========== Tool execution messages ==========
    /// External command started execution
    ExecCommandStarted {
        call_id: Option<String>,
        input_preview: Option<String>,
        source: Option<String>,
    },
    /// External command output fragment
    ExecCommandOutputDelta {
        call_id: Option<String>,
        delta: String,
    },
    /// External command execution complete
    ExecCommandFinished {
        call_id: Option<String>,
        output_preview: Option<String>,
        status: ExecCommandStatus,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },

    // ========== Generic tool messages ==========
    /// Tool call started
    ToolCallStarted {
        name: String,
        call_id: Option<String>,
        input_preview: Option<String>,
    },
    /// Tool call complete
    ToolCallFinished {
        name: String,
        call_id: Option<String>,
        output_preview: Option<String>,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
    },

    // ========== Code operation messages ==========
    /// Patch started applying
    PatchApplyStarted {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
    },
    /// Patch output fragment
    PatchApplyOutputDelta {
        call_id: Option<String>,
        delta: String,
    },
    /// Patch application complete
    PatchApplyFinished {
        call_id: Option<String>,
        changes: Vec<PatchChange>,
        status: PatchApplyStatus,
    },

    // ========== MCP messages ==========
    /// MCP tool call started
    McpToolCallStarted {
        call_id: Option<String>,
        invocation: McpInvocation,
    },
    /// MCP tool call complete
    McpToolCallFinished {
        call_id: Option<String>,
        invocation: McpInvocation,
        result_blocks: Vec<serde_json::Value>,
        error: Option<String>,
        status: McpToolCallStatus,
        is_error: bool,
    },

    // ========== Web search messages ==========
    WebSearchStarted {
        call_id: Option<String>,
        query: String,
    },
    WebSearchFinished {
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    },

    // ========== Image messages ==========
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

    // ========== System messages ==========
    /// Agent has been idle beyond threshold (produced by Runtime, not Behavior)
    AgentIdle,
    /// Heartbeat (used to keep connection alive)
    Heartbeat,
}

impl ActorMessage {
    /// Determine if this message is a "running" message (no decision needed)
    pub fn is_running(&self) -> bool {
        matches!(self,
            Self::AssistantChunk { .. }
            | Self::ThinkingChunk { .. }
            | Self::StatusUpdate { .. }
            | Self::ExecCommandOutputDelta { .. }
            | Self::PatchApplyOutputDelta { .. }
            | Self::Heartbeat
        )
    }

    /// Determine if this message may need decision layer classification
    pub fn may_need_decision(&self) -> bool {
        matches!(self,
            Self::BehaviorFinished
            | Self::BehaviorFailed { .. }
            | Self::ExecCommandFinished { .. }
            | Self::ToolCallFinished { .. }
            | Self::PatchApplyFinished { .. }
            | Self::McpToolCallFinished { .. }
            | Self::WebSearchFinished { .. }
            | Self::AgentIdle
        )
    }

    /// Determine if this message needs protocol broadcast
    pub fn should_broadcast(&self) -> bool {
        !matches!(self, Self::Heartbeat)
    }
}
```

### 6.3 Protocol Conversion: From `ActorMessage` to `ProtocolEvent`

```rust
// agent-protocol-infra/src/gateway.rs

/// Protocol gateway: converts internal ActorMessage to external ProtocolEvent.
///
/// This is a stateless converter (except for the item_id generator),
/// can be tested independently without depending on any business logic.
pub struct ProtocolGateway {
    seq_counter: u64,
    item_tracker: ItemTracker,  // (agent_id, call_id) → item_id
}

impl ProtocolGateway {
    pub fn convert(&mut self, agent_id: &WorkAgentId, message: &ActorMessage) -> Vec<ProtocolEvent> {
        match message {
            ActorMessage::AssistantChunk { text } => {
                let item_id = self.item_tracker.get_or_create(agent_id, None, ItemKind::AssistantOutput);
                vec![ProtocolEvent::item_delta(item_id, ItemDelta::Text(text.clone()))]
            }
            ActorMessage::ThinkingChunk { text } => {
                let item_id = self.item_tracker.get_or_create(agent_id, None, ItemKind::SystemMessage);
                vec![ProtocolEvent::item_delta(item_id, ItemDelta::Markdown(text.clone()))]
            }
            ActorMessage::ExecCommandStarted { call_id, .. } => {
                let item_id = self.item_tracker.create(agent_id, call_id.clone(), ItemKind::ToolCall);
                vec![ProtocolEvent::item_started(item_id, ItemKind::ToolCall, agent_id)]
            }
            ActorMessage::ExecCommandFinished { call_id, output_preview, status, exit_code, duration_ms, .. } => {
                if let Some(item_id) = self.item_tracker.finish(agent_id, call_id.clone()) {
                    vec![ProtocolEvent::item_completed(
                        item_id,
                        TranscriptItem {
                            // ...
                        },
                    )]
                } else {
                    vec![]
                }
            }
            ActorMessage::BehaviorFinished => {
                vec![ProtocolEvent::agent_status_changed(agent_id, AgentSlotStatus::Idle)]
            }
            ActorMessage::BehaviorFailed { reason } => {
                vec![
                    ProtocolEvent::error(reason, agent_id),
                    ProtocolEvent::agent_status_changed(agent_id, AgentSlotStatus::Blocked),
                ]
            }
            // ... other conversions
            _ => vec![],
        }
    }
}
```

---

## 7. State Machine Refactoring: Aggregate Root Cohesion

### 7.1 Current Problem

```rust
// core/src/slot/status.rs
pub enum AgentSlotStatus {
    Idle,
    Starting,
    Responding { started_at: Instant },
    ToolExecuting { tool_name: String },
    Finishing,
    Stopping,
    Stopped { reason: String },
    Error { message: String },
    Blocked { reason: String },
    BlockedForDecision { blocked_state: BlockedState },
    Paused { reason: String },
    WaitingForInput { started_at: Instant },
    Resting { started_at: Instant, blocked_state: BlockedState, on_resume: String },
}
```

**Problems**:
1. Too many states (14), some with overlapping semantics
2. `Blocked` and `BlockedForDecision` distinction is blurry
3. `WaitingForInput` is a sub-state of `Responding`, but promoted to the same level
4. `Resting` contains `blocked_state` and `on_resume`, the state value object carries behavior logic

### 7.2 Target State Machine: `ActorLifecycle`

```rust
// agent-runtime-domain/src/lifecycle.rs

/// Actor lifecycle state.
///
/// Simplification principles:
/// 1. Only retain "runtime phase" states, sub-states expressed through context
/// 2. Merge terminal states
/// 3. Block reasons described by context `BlockedContext`, not distinguished by state
#[derive(Debug, Clone, PartialEq)]
pub enum ActorLifecycle {
    /// Actor created, waiting for task assignment
    Idle,
    /// Starting Behavior thread
    Starting,
    /// Interacting with LLM (includes Responding + WaitingForInput + ToolExecuting)
    /// Sub-state expressed through `InteractionContext`
    Interacting { context: InteractionContext, started_at: Instant },
    /// Wrapping up (Behavior thread ended, cleaning up)
    Finishing,
    /// Paused (recoverable)
    Paused { reason: String },
    /// Stopped (terminal state)
    Stopped { reason: String },
}

/// Sub-state context for the interaction phase.
///
/// This is a "state machine within a state machine" — the main state machine
/// has only 6 states, complex interaction details are placed in context.
#[derive(Debug, Clone, PartialEq)]
pub enum InteractionContext {
    /// LLM generating text
    Generating,
    /// LLM thinking (chain-of-thought)
    Thinking,
    /// Waiting for external tool/command execution
    AwaitingTool { tool_name: String },
    /// Waiting for user input (paused within interaction)
    AwaitingUserInput,
    /// Blocked, needs decision layer intervention
    Blocked { reason: BlockedReason },
}

/// Block reason (value object)
#[derive(Debug, Clone, PartialEq)]
pub enum BlockedReason {
    /// Tool execution failed
    ToolFailed { tool_name: String, error: String },
    /// Needs human decision
    NeedsHumanDecision { request_id: String },
    /// Rate limited
    RateLimited { retry_after: Duration },
    /// Generic error
    Error { message: String },
}
```

### 7.3 State Transition Rules

```rust
impl ActorLifecycle {
    pub fn can_transition_to(&self, target: &Self) -> bool {
        use ActorLifecycle::*;
        match (self, target) {
            // Normal flow
            (Idle, Starting) => true,
            (Starting, Interacting { .. }) => true,
            (Interacting { .. }, Interacting { .. }) => true,  // Free switching between sub-states
            (Interacting { .. }, Finishing) => true,
            (Finishing, Idle) => true,

            // Pause/recovery
            (Interacting { .. }, Paused { .. }) => true,
            (Paused { .. }, Interacting { .. }) => true,
            (Paused { .. }, Idle) => true,

            // Stop (can stop from any non-terminal state)
            (Idle, Stopped { .. }) => true,
            (Starting, Stopped { .. }) => true,
            (Interacting { .. }, Stopped { .. }) => true,
            (Paused { .. }, Stopped { .. }) => true,

            // Other transitions illegal
            _ => false,
        }
    }
}
```

### 7.4 State Machine Simplification Benefits

| Metric | Current | Target | Benefit |
|:---|:---|:---|:---|
| Top-level state count | 14 | 6 | Reduced cognitive burden |
| State transition rule count | 40+ | 12 | Lower error probability |
| `is_active()` etc. complexity | Need to match 6+ states | Match 2 states | Simplified conditions |
| Sub-state expressiveness | None (all flat) | `InteractionContext` nesting | Clear hierarchy |

---

## 8. Effect System: Side Effects Made Explicit

### 8.1 Why an Effect System is Needed

Current `tick()` side effects are **implicit**:
- Calling `pump.process()` → implicitly produces `ProtocolEvent`
- Calling `agent_pool.send_decision_request()` → implicitly produces decision request
- Calling `start_provider_for_agent_inner()` → implicitly starts new thread

**The Effect system** makes side effects **explicit, typed, and testable**.

### 8.2 Effect Type Hierarchy

```rust
// agent-runtime-app/src/effect.rs

/// Side effects produced by an Actor processing a message.
///
/// Key design: Effect is an "immutable description", not an "executable action".
/// Execution is handled by `EffectRouter`. This achieves dependency inversion:
/// - WorkAgent only describes "what I want to do" (produces Effect)
/// - EffectRouter decides "how to do it" (executes Effect)
pub enum ActorEffect {
    // ========== Protocol layer side effects ==========
    EmitProtocol(ProtocolEvent),

    // ========== Decision layer side effects ==========
    ClassifyForDecision {
        work_agent_id: WorkAgentId,
        message: ActorMessage,
    },
    DispatchDecisionDirective {
        work_agent_id: WorkAgentId,
        directive: DecisionDirective,
    },

    // ========== Behavior layer side effects ==========
    SpawnBehavior {
        work_agent_id: WorkAgentId,
        prompt: String,
        record_in_transcript: bool,
    },
    CancelBehavior {
        work_agent_id: WorkAgentId,
    },

    // ========== Coordination layer side effects ==========
    SendInterActorMail {
        to: WorkAgentId,
        mail: InterActorEnvelope,
    },
    BroadcastMail {
        subject: String,
        body: String,
    },

    // ========== Persistence side effects ==========
    PersistSnapshot {
        trigger: SnapshotTrigger,
    },

    // ========== Log side effects ==========
    Log { level: LogLevel, message: String, metadata: Option<serde_json::Value> },

    // ========== Empty side effect ==========
    NoOp,
}

/// Side effect router.
///
/// Routes Effects to their corresponding executors. This is the "interpreter"
/// of the Effect system.
pub struct EffectRouter<'a> {
    behavior_spawner: &'a mut BehaviorSpawner,
    decision_coordinator: &'a mut DecisionCoordinator,
    protocol_gateway: &'a mut ProtocolGateway,
    inter_actor_mail: &'a mut InterActorMail,
    snapshot_store: &'a mut SnapshotStore,
}

impl<'a> EffectRouter<'a> {
    pub fn route(&mut self, effect: ActorEffect) -> Result<(), EffectError> {
        match effect {
            ActorEffect::EmitProtocol(event) => {
                self.protocol_gateway.emit(event);
                Ok(())
            }
            ActorEffect::SpawnBehavior { work_agent_id, prompt, record_in_transcript } => {
                self.behavior_spawner.spawn(&work_agent_id, &prompt, record_in_transcript)
            }
            ActorEffect::ClassifyForDecision { work_agent_id, message } => {
                self.decision_coordinator.classify_and_dispatch(&work_agent_id, message)
            }
            ActorEffect::SendInterActorMail { to, mail } => {
                self.inter_actor_mail.deliver(to, mail);
                Ok(())
            }
            ActorEffect::PersistSnapshot { trigger } => {
                self.snapshot_store.request(trigger);
                Ok(())
            }
            ActorEffect::Log { level, message, metadata } => {
                log::log!(level, "{}", message);
                Ok(())
            }
            ActorEffect::NoOp => Ok(()),
            // ... other routing
        }
    }
}
```

### 8.3 Testing Advantages of the Effect System

```rust
// Testing WorkAgent no longer requires constructing a complete SessionManager!
#[test]
fn work_agent_handles_finished_message() {
    let mut agent = WorkAgent::new(WorkAgentId::new("test-1"), ActorCodename::Alpha);
    agent.transition_to(ActorLifecycle::interacting(InteractionContext::Generating));

    // Send message
    let effects = agent.handle_message(ActorMessage::BehaviorFinished);

    // Assert state changes (internal)
    assert!(agent.lifecycle().is_idle());
    assert!(!agent.has_behavior_thread());

    // Assert side effects (external)
    assert_eq!(effects.len(), 1);
    assert!(matches!(&effects[0],
        ActorEffect::EmitProtocol(ProtocolEvent::AgentStatusChanged { status: AgentSlotStatus::Idle, .. })
    ));
}
```

---

## 9. Post-Refactoring Dependency Diagram

### 9.1 Module-Level Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                         Post-Refactoring Module Dependency Graph                    │
├─────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                      │
│   ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │                        agent-domain-events                                  │   │
│   │              ActorMessage, WorkAgentId, BehaviorKind, SessionHandle          │   │
│   └──────────────┬──────────────────────────────┬───────────────────────────────┘   │
│                  │                              │                                    │
│                  ▼                              ▼                                    │
│   ┌──────────────────────────┐    ┌──────────────────────────┐                      │
│   │   agent-runtime-domain   │    │  agent-decision-domain   │                      │
│   │  ──────────────────────  │    │  ──────────────────────  │                      │
│   │  WorkAgent               │    │  DecisionActor           │                      │
│   │  ActorSupervisor         │    │  DecisionEngine          │                      │
│   │  ActorLifecycle          │    │  SituationClassifier     │                      │
│   │  MailboxMultiplexer      │    │  ActionExecutor          │                      │
│   │  InterActorMail          │    │                          │                      │
│   └──────────────┬───────────┘    └──────────────┬───────────┘                      │
│                  │                               │                                   │
│                  └───────────────┬───────────────┘                                   │
│                                  │                                                   │
│                                  ▼                                                   │
│   ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │                        agent-behavior-infra                                  │   │
│   │  BehaviorSpawner, ClaudeBehavior, CodexBehavior, MockBehavior, ProtocolParser │   │
│   └──────────────┬──────────────────────────────────────────────────────────────┘   │
│                  │                                                                   │
│                  ▼                                                                   │
│   ┌──────────────────────────┐    ┌──────────────────────────┐                      │
│   │  agent-protocol-infra    │    │   agent-runtime-app      │                      │
│   │  ──────────────────────  │    │  ──────────────────────  │                      │
│   │  ProtocolGateway         │    │  ActorRuntime            │                      │
│   │  ProtocolEvent           │    │  RuntimeTick             │                      │
│   │  JsonRpcHandler          │    │  EffectRouter            │                      │
│   │  WsBroadcaster           │    │  ShutdownSnapshot        │                      │
│   └──────────────┬───────────┘    └──────────────┬───────────┘                      │
│                  │                               │                                   │
│                  └───────────────┬───────────────┘                                   │
│                                  │                                                   │
│                                  ▼                                                   │
│   ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │                            agent-daemon                                        │   │
│   │                     HTTP server, WebSocket, main.rs                            │   │
│   └─────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                      │
│   ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │                            agent-tui                                           │   │
│   │                     Terminal UI, WebSocket client                              │   │
│   └─────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                      │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Key Interface Contracts

```rust
// ============================================================
// agent-runtime-domain/src/lib.rs
// ============================================================

/// WorkAgent: Work Actor aggregate root
pub struct WorkAgent {
    // ... private fields ...
}

impl WorkAgent {
    pub fn new(id: WorkAgentId, codename: ActorCodename) -> Self;
    pub fn id(&self) -> &WorkAgentId;
    pub fn lifecycle(&self) -> &ActorLifecycle;
    pub fn transition_to(&mut self, new: ActorLifecycle) -> Result<(), LifecycleError>;
    pub fn handle_message(&mut self, msg: ActorMessage) -> Vec<ActorEffect>;
    pub fn spawn_behavior(&mut self, prompt: &str, context: BehaviorLaunchContext) -> Result<BehaviorHandle, BehaviorError>;
    pub fn cancel_behavior(&mut self);
    pub fn has_behavior_thread(&self) -> bool;
}

/// ActorSupervisor: Supervisor aggregate root
pub struct ActorSupervisor {
    // ... private fields ...
}

impl ActorSupervisor {
    pub fn spawn_agent(&mut self, kind: BehaviorKind) -> Result<WorkAgentId, SupervisorError>;
    pub fn stop_agent(&mut self, id: &WorkAgentId) -> Result<(), SupervisorError>;
    pub fn remove_agent(&mut self, id: &WorkAgentId) -> Result<(), SupervisorError>;
    pub fn get_agent(&self, id: &WorkAgentId) -> Option<&WorkAgent>;
    pub fn get_agent_mut(&mut self, id: &WorkAgentId) -> Option<&mut WorkAgent>;
    pub fn idle_agent_ids(&self) -> Vec<WorkAgentId>;
    pub fn can_trigger_idle_decision(&self, id: &WorkAgentId) -> bool;
}

/// MailboxMultiplexer: Mailbox multiplexer
pub struct MailboxMultiplexer {
    // ... private fields ...
}

impl MailboxMultiplexer {
    pub fn register(&mut self, agent_id: WorkAgentId, receiver: mpsc::Receiver<ActorMessage>);
    pub fn remove_mailbox(&mut self, agent_id: &WorkAgentId);
    pub fn poll_all(&self) -> MailboxDeliveryBatch;
}

// ============================================================
// agent-runtime-app/src/lib.rs
// ============================================================

/// ActorRuntime: Runtime scheduler
pub struct ActorRuntime {
    // ... private fields ...
}

impl ActorRuntime {
    pub async fn bootstrap(cwd: PathBuf, workplace_id: WorkplaceId) -> Result<Self, RuntimeError>;
    pub async fn tick(&self) -> Result<Vec<ProtocolEvent>, RuntimeError>;
    pub async fn dispatch_input(&self, agent_id: WorkAgentId, text: String) -> Result<DispatchResult, RuntimeError>;
    pub async fn snapshot(&self) -> Result<SessionState, RuntimeError>;
}
```

---

## 10. Migration Roadmap

### Phase 0: Infrastructure Preparation (1 week)

**Goal**: Create `agent-events`, promote `ProviderEvent` to `ActorMessage`.

**Steps**:
1. Create `agent-events` crate
2. Move basic types from `agent-types` (`AgentId`, `ProviderKind`, `WorkplaceId`) to `agent-events`
3. Rename `ProviderEvent` to `ActorMessage` in `agent/provider/src/provider.rs`, move to `agent-events`
4. Deprecate simplified `ProviderEvent` in `decision/src/provider/provider_event.rs`, unify to use `ActorMessage`
5. Delete `core/src/pool/event_converter.rs`
6. Update dependencies for all crates, ensure compilation passes

**Verification**: `cargo test --workspace` all pass.

### Phase 1: Split `agent-core` (3-4 weeks)

**Goal**: Split `agent-core` into `agent-runtime-domain` + `agent-behavior-infra`.

**Steps**:
1. Create `agent-runtime-domain` crate, extract:
   - `AgentSlot` → `WorkAgent`
   - Lifecycle portion of `AgentPool` → `ActorSupervisor`
   - `AgentSlotStatus` → `ActorLifecycle`
   - `EventAggregator` → `MailboxMultiplexer`
   - `AgentMailbox` → `InterActorMail`
2. Create `agent-behavior-infra` crate, extract:
   - `agent/provider/src/providers/` → `ClaudeBehavior`, `CodexBehavior`, `MockBehavior`
   - `agent/provider/src/provider_thread.rs` → `BehaviorThread`
   - `agent/provider/src/llm_caller.rs` → `DecisionLLMCaller`
3. Keep `agent-provider` as a facade for behavior infra (backward compatibility)
4. Update `agent-daemon` dependencies

**Verification**: Unit tests from `agent-core` continue to pass in `agent-runtime-domain`.

### Phase 2: Extract `agent-protocol-infra` (1-2 weeks)

**Goal**: Move protocol conversion logic out of `agent-daemon`.

**Steps**:
1. Create `agent-protocol-infra` crate
2. Move `agent/daemon/src/event_pump.rs` → `ProtocolGateway`
3. Keep `agent/protocol/src/events.rs` (it is protocol definition), but `ProtocolGateway` depends on it for conversion
4. Move JSON-RPC handling logic from `agent/protocol/src/methods.rs` to `agent-protocol-infra`

**Verification**: Protocol conversion unit tests all pass.

### Phase 3: Refactor Scheduler (3-4 weeks)

**Goal**: Refactor `tick()` into `RuntimeTick` + `EffectRouter` + `MessageHandler`.

**Steps**:
1. Create `agent-runtime-app` crate
2. Define `ActorEffect` enum
3. Define `MessageHandler` trait
4. Implement `MessageHandler` for `WorkAgent`
5. Implement `EffectRouter`
6. Implement `RuntimeTick` (single tick scheduling logic)
7. Refactor `SessionManager` into `ActorRuntime` (only retain lock management and public APIs)
8. Gradually migrate match branches from `tick()` to `WorkAgent::handle_xxx()` methods

**Verification**: All daemon integration tests pass.

### Phase 4: Type Renaming (1 week)

**Goal**: Execute type renaming.

**Steps**:
1. Batch rename using IDE (Rust Analyzer's Rename Symbol)
2. Update all documentation and tests
3. Publish internal migration guide

**Note**: This step can be gradually completed during Phases 2-4, no need to concentrate.

### Phase 5: State Machine Simplification (1-2 weeks)

**Goal**: Simplify `AgentSlotStatus` (14 states) to `ActorLifecycle` (6 states + `InteractionContext`).

**Steps**:
1. Define new `ActorLifecycle` and `InteractionContext`
2. Update `can_transition_to()` rules
3. Update all state judgment code (`is_active()`, `is_idle()`, etc.)
4. Update snapshot serialization (ensure backward compatibility)

**Verification**: State machine unit tests cover all transition rules.

---

## Conclusion

The core belief of this plan: **Core architecture is not worth compromising on.**

- `AgentSlot` is not a Worker → Change it
- `tick()` is not a scheduler → Split it
- `ProviderEvent` is not a domain event → Unify it
- 14 states is not a clear state machine → Reorganize it
- Decision layer directly modifies Worker → Decouple it

The cost of every change is real, but the cost of leaving technical debt is higher. A 10-week refactoring investment yields a system with correct naming, clear boundaries, exhaustive state machines, and isolated side effects. This is an architecture that can be maintained for 5+ years.
