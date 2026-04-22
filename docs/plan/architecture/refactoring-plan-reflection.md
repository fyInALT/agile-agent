# Refactoring Plan Reflection: Honest Design Review and Corrections

> This document is a critical reflection on `refactoring-plan.md`.
>
> Any design plan must pass the test of "how will it be misunderstood" before landing. This document examines over-engineering, misleading statements, and engineering reality conflicts in the original plan one by one, and provides corrected pragmatic directions.

---

## Table of Contents

1. [Reflection 1: "Actor Model" is Conceptual Borrowing, Not a Framework Promise](#reflection-1-actor-model-is-conceptual-borrowing-not-a-framework-promise)
2. [Reflection 2: Cost of 32 Type Renames is Seriously Underestimated](#reflection-2-cost-of-32-type-renames-is-seriously-underestimated)
3. [Reflection 3: Crate Split is Excessive — Module is Sufficient](#reflection-3-crate-split-is-excessive--module-is-sufficient)
4. [Reflection 4: Vec Allocation in Effect System is a Performance Trap](#reflection-4-vec-allocation-in-effect-system-is-a-performance-trap)
5. [Reflection 5: State Machine Simplification Loses Business Semantics](#reflection-5-state-machine-simplification-loses-business-semantics)
6. [Reflection 6: `BehaviorStrategy` Trait is Over-Abstraction](#reflection-6-behaviorstrategy-trait-is-over-abstraction)
7. [Reflection 7: Blind Spot of "Unifying ProviderEvent" — Layering Beats Unification](#reflection-7-blind-spot-of-unifying-providerevent--layering-beats-unification)
8. [Reflection 8: Pragmatic Path for `tick()` Split](#reflection-8-pragmatic-path-for-tick-split)
9. [Reflection 9: DDD Bounded Context Lacks Anti-Corruption Layer](#reflection-9-ddd-bounded-context-lacks-anti-corruption-layer)
10. [Reflection 10: Missing Backward Compatibility and Migration Costs](#reflection-10-missing-backward-compatibility-and-migration-costs)
11. [Corrected Refactoring Principles](#corrected-refactoring-principles)
12. [Corrected Minimum Viable Plan (MVP)](#corrected-minimum-viable-plan-mvp)

---

## Reflection 1: "Actor Model" is Conceptual Borrowing, Not a Framework Promise

### Problem in the Original Plan

The original plan heavily uses terms like "Actor Model", "Supervisor", "Mailbox", "Scheduler", and claims the system "is essentially a handwritten Actor system".

### What Misunderstandings This Causes

**Misunderstanding 1: "Did we use an Actor framework?"**

New developers seeing `ActorSupervisor`, `ActorRuntime`, `MessageHandler` etc. will naturally look for corresponding framework documentation (such as Actix, Akka, Erlang/OTP). When they discover the system only uses `std::sync::mpsc` and `std::thread` to handwrite a message loop, they experience cognitive dissonance: "These names imply framework capabilities, but there are none."

**Misunderstanding 2: "Must I follow strict Actor dogmas?"**

The original plan lists three "inviolable Actor dogmas", the third stating "the scheduler is only responsible for fetching messages from mailboxes and dispatching them to Actors". But in reality:
- `tick()` directly synchronously calling `slot.append_transcript()` is a completely reasonable design
- Under the single-threaded ownership model, synchronous method calls are more efficient and simpler than asynchronous message passing
- Requiring "communication only through messages" would force unnecessary asynchronous complexity and memory allocation

**Misunderstanding 3: "Can this system horizontally scale to distributed?"**

The Actor model (especially Akka/Erlang style) naturally implies distributed capabilities. But this system:
- Concentrates all state in `Arc<Mutex<SessionInner>>`
- All Actors are within the same process
- No location transparency
- No remote message passing

If someone designs distributed expansion based on "Actor model" naming, they head in the wrong direction.

### Engineering Reality

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  This system is not an Actor model implementation, but rather:              │
│                                                                             │
│  "Message-driven single-threaded state machine + background worker threads" │
│                                                                             │
│  More accurate historical analogies:                                        │
│  - Game engine main loop (update/render)                                    │
│  - Browser event loop                                                       │
│  - Redis single-threaded command processing                                 │
│                                                                             │
│  These systems all have:                                                    │
│  - One main thread serially processing events                               │
│  - Background threads doing IO/computation                                  │
│  - Passing results through queues/channels                                  │
│  - But nobody calls them "Actor models"                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Correction Direction

**Do not** wrap the system with Actor framework terminology.

**Should** use more accurate terminology:

| Original Term | Corrected Term | Reason |
|:---|:---|:---|
| Actor | Worker / Agent | No Actor framework, don't pretend there is one |
| Supervisor | Pool / Manager | `Pool` already accurately expresses "managing a group of Workers" |
| Mailbox | Event Channel | `mpsc::channel` is a channel, not an Actor's Mailbox |
| Scheduler | Event Loop / Tick | It is an event loop, same as browser/game engines |
| MessageHandler | EventProcessor | Processing events, not processing Actor messages |
| ActorEffect | Command / Action | Side effects are command pattern, not Actor's Effect |

**The only place to retain Actor pattern reference**: In architecture documents as a **design inspiration source** ("this system borrows ideas from the Actor model: state encapsulation, message passing, fault isolation"), but **not as a naming basis**.

---

## Reflection 2: Cost of 32 Type Renames is Seriously Underestimated

### Problem in the Original Plan

The original plan lists 32 type renames, claiming "type names are documentation". But it does not estimate actual costs.

### Actual Cost Estimate

Take `ProviderEvent` → `ActorMessage` as an example:

```bash
# Count impact scope
$ grep -r "ProviderEvent" --include="*.rs" . | wc -l
# Actual result: ~200+ references

$ grep -r "AgentSlot" --include="*.rs" . | wc -l
# Actual result: ~500+ references

$ grep -r "AgentPool" --include="*.rs" . | wc -l
# Actual result: ~400+ references
```

Total approximately **1500+ references** need modification. Even using IDE batch rename:
- Need to check whether each rename crosses semantic boundaries (e.g., `AgentSlot` used as a plain data structure in some tests)
- Need to update all documentation, comments, READMEs, test names
- Need to update CI/CD scripts, log formats, monitoring metric names
- Team members need to rebuild muscle memory (`Ag<Tab>` no longer completes to `AgentSlot`)

**Conservative estimate: 2-3 developers need 1-2 weeks of full-time work**.

### Deeper Problem: Renaming Doesn't Solve Architecture Problems

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Myth: "If names are right, architecture is clear"                          │
│                                                                             │
│  In reality:                                                                │
│  - `AgentSlot` → `WorkAgent`: better name, but transcript append logic     │
│    is still in tick()                                                       │
│  - `AgentPool` → `ActorSupervisor`: flashier name, but still a 4500-line   │
│    god class                                                                │
│  - `ProviderEvent` → `ActorMessage`: more generic name, but 24 variants    │
│    haven't decreased                                                        │
│                                                                             │
│  Renaming is "lipstick on a pig" — if structure doesn't change, good names │
│ 反而掩盖了坏设计。                                                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Correction Direction

**Strategy: Only rename the few most misleading types, keep compatibility for the rest.**

| Priority | Type | Rename? | Reason |
|:---|:---|:---|:---|
| P0 | `AgentSlot` | ✅ `Worker` | Most misleading; `Slot` implies container rather than entity |
| P0 | `AgentPool` | ❌ Keep | `Pool` is already accurate; low renaming benefit |
| P1 | `ProviderEvent` | ❌ Keep | 200+ references; too high cost; add documentation comments instead |
| P1 | `SessionManager` | ✅ `Runtime` | `Session` is a business concept; `Runtime` is more accurate |
| P2 | `EventPump` | ❌ Keep | Internal type; doesn't affect external understanding |
| P2 | `AgentMailbox` | ❌ Keep | Internal type; extremely low renaming benefit |
| P3 | Other 25 | ❌ All keep | Cost > benefit |

**Rename principle**: Only change when the name **actively misleads** developer understanding.

---

## Reflection 3: Crate Split is Excessive — Module is Sufficient

### Problem in the Original Plan

The original plan proposes creating 6 new crates:
- `agent-domain-events`
- `agent-runtime-domain`
- `agent-decision-domain`
- `agent-behavior-infra`
- `agent-protocol-infra`
- `agent-runtime-app`

### What Problems This Causes

**Problem 1: Compilation Complexity**

In Rust, each crate is an independent compilation unit. Types across crate boundaries cannot use `pub(crate)` visibility; all shared types must be `pub`. This means:
- Internal implementation details are forced to be exposed
- Cannot do compile-time privacy checking
- Cannot use IDE's "restrict to crate" search during refactoring

**Problem 2: Circular Dependency Risk**

```
agent-runtime-domain ──► agent-decision-domain (decision needs Worker state)
agent-decision-domain ──► agent-runtime-domain (decision results modify Worker)
```

In current code, `DecisionExecutor` directly modifies `AgentSlot`'s transcript, meaning there is a bidirectional dependency between Runtime and Decision. If split into two crates, traits or events must be used to decouple, introducing大量间接层。

**Problem 3: IDE and Developer Experience**

- Cross-crate IDE jumps need to parse `Cargo.toml` dependencies, slower than同 crate `mod`
- New developers need to understand 6 crate responsibility boundaries before they can start working
- Each crate needs independent `Cargo.toml`, version management, release process

### Actual Comparison: Crate vs Module

| Dimension | Crate | Module |
|:---|:---|:---|
| Compilation isolation | ✅ Independent compilation | ❌ Same unit compilation |
| Visibility control | ❌ Only pub | ✅ pub(crate) |
| Circular dependency detection | ✅ Compile-time prevention | ⚠️ Requires manual avoidance |
| Developer experience | ⚠️ Slower jumps | ✅ Faster jumps |
| Version management | ❌ Needs independent versions | ✅ Unified version |
| Suitable scenario | Truly independent libraries | Internal modules of the same system |

The various "domains" of this system are actually **tightly coupled**:
- Runtime needs Decision's decision results to start new Behaviors
- Decision needs Runtime's Worker state to do classification
- Protocol needs Runtime's state to generate snapshots

This coupling indicates they **should not be independent crates**, but rather different modules within the same crate.

### Correction Direction

**Do not create new crates; use modules within `agent-core` to partition domains:**

```
core/src/
├── lib.rs                          # Public exports
├── runtime/                        # Runtime module (formerly SessionManager + tick)
│   ├── mod.rs
│   ├── runtime.rs                  # Runtime struct (formerly SessionManager)
│   ├── tick.rs                     # Event loop (formerly tick() split)
│   └── effect.rs                   # Side effect types
├── worker/                         # Worker module (formerly AgentSlot)
│   ├── mod.rs
│   ├── worker.rs                   # Worker struct
│   ├── lifecycle.rs                # State machine (formerly AgentSlotStatus)
│   └── journal.rs                  # Transcript management
├── pool/                           # Pool module (formerly AgentPool lifecycle portion)
│   ├── mod.rs
│   └── pool.rs
├── decision/                       # Decision module (moved from decision crate)
│   ├── mod.rs
│   ├── coordinator.rs
│   ├── engine.rs
│   └── executor.rs
├── protocol/                       # Protocol module (formerly EventPump)
│   ├── mod.rs
│   └── gateway.rs                  # ProtocolGateway (formerly EventPump)
├── behavior/                       # Behavior module (formerly provider startup logic)
│   ├── mod.rs
│   ├── spawner.rs
│   ├── claude.rs
│   ├── codex.rs
│   └── mock.rs
├── event/                          # Event module (unified event definition)
│   ├── mod.rs
│   └── events.rs                   # Unified ProviderEvent definition
├── channel/                        # Channel module (formerly EventAggregator)
│   ├── mod.rs
│   └── multiplexer.rs              # ChannelMultiplexer (formerly EventAggregator)
└── mail/                           # Mail module (formerly AgentMailbox)
    ├── mod.rs
    └── mail.rs
```

**Only split out one new crate**: `agent-protocol` (external protocol definition), because it needs to be shared by `agent-tui` and `agent-daemon`, and should not contain business logic.

---

## Reflection 4: Vec Allocation in Effect System is a Performance Trap

### Problem in the Original Plan

The original plan designs an `Effect` system:

```rust
pub trait MessageHandler {
    fn handle_message(&mut self, message: ActorMessage) -> Vec<ActorEffect>;
}
```

### Performance Analysis

Within a 100ms tick cycle, the system may process:
- 4 Workers × 10 events = 40 events
- Each event averages 1.5 Effects
- Total: 40 × 1.5 = 60 Effects/cycle

This means **600 small Vecs are allocated every second** (10 ticks per second).

In Rust, `Vec::new()` is zero-allocation (doesn't allocate heap memory), but `vec.push()` beyond capacity triggers heap allocation. More critically:
- `Vec<ActorEffect>`'s Drop needs to iterate and destroy each Effect
- If Effect contains `String` or `Vec`, it also triggers secondary allocation
- In high-frequency scenarios (such as streaming output, where each tick may have hundreds of `AssistantChunk`s), this becomes a hotspot

### Lighter Alternatives

**Option A: Callback Style**

```rust
pub trait EventProcessor {
    fn process_event<F>(&mut self, event: ProviderEvent, mut emit: F)
    where
        F: FnMut(Command);
}

// Usage
worker.process_event(event, |cmd| {
    match cmd {
        Command::Broadcast(ev) => broadcaster.emit(ev),
        Command::Classify(id, ev) => decision.classify(id, ev),
        // ...
    }
});
```

**Advantages**:
- No Vec allocation
- Zero-cost abstraction (callback is inlined)
- Cleaner code

**Option B: SmallVec (small array optimization)**

```rust
use smallvec::SmallVec;

type CommandBuffer = SmallVec<[Command; 4]>;

pub trait EventProcessor {
    fn process_event(&mut self, event: ProviderEvent) -> CommandBuffer;
}
```

**Advantages**:
- ≤4 Commands use stack memory, no heap allocation
- Automatically degrades to Vec when exceeding 4
- Most events produce 0-2 Commands, perfect coverage

### Correction Direction

Use **callback style** as primary, `SmallVec` as secondary. Callback style is zero-cost in Rust (`FnMut` trait object or generic parameter can both be inlined by the compiler), and better aligns with Rust idiomatic style.

---

## Reflection 5: State Machine Simplification Loses Business Semantics

### Problem in the Original Plan

The original plan proposes compressing 14 states into 6:

```rust
// Original plan target
enum ActorLifecycle {
    Idle,
    Starting,
    Interacting { context: InteractionContext, started_at: Instant },
    Finishing,
    Paused { reason: String },
    Stopped { reason: String },
}
```

### Lost Semantics

**Loss 1: `BlockedForDecision` vs `Blocked`**

Current code:
```rust
// core/src/slot/status.rs
pub fn is_blocked_for_human(&self) -> bool {
    match self {
        Self::Blocked { reason } => reason.contains("human"),
        Self::BlockedForDecision { blocked_state } => {
            blocked_state.requires_human_input()
        }
        _ => false,
    }
}
```

`BlockedForDecision` is a special state indicating "the decision layer has intervened, waiting for human confirmation". It has `blocked_state` context, containing decision request ID, option list, etc. If merged into `Interacting { context: Blocked { .. } }`:
- UI needs additional `BlockedReason` checks to determine whether to display the decision panel
- Decision recovery logic needs to penetrate two layers of enums
- Snapshot serialization becomes more complex

**Loss 2: UI Semantics of `WaitingForInput`**

Current `WaitingForInput` is an independent state; when the UI sees it, it:
- Displays an input prompt
- Disables the send button (until user input)
- Shows a "waiting for input" animation

If it becomes `Interacting { context: AwaitingUserInput }`:
- UI code needs to extract sub-state from `InteractionContext`
- `is_waiting_for_input()` judgment changes from `matches!(status, WaitingForInput)` to `matches!(lifecycle, Interacting { context: AwaitingUserInput, .. })`
- Complexity increases rather than decreases

**Loss 3: Recovery Logic of `Resting`**

```rust
// Current
AgentSlotStatus::Resting {
    started_at,
    blocked_state,
    on_resume,
}
```

`Resting` indicates "cooldown period after rate limiting", it has an `on_resume` field (action to execute after recovery). If merged into `Paused { reason: "rate_limited".to_string() }`:
- Recovery logic needs additional storage of `on_resume` in other Worker fields
- State machine integrity is broken

### Correction Direction

**Do not simplify the state machine; instead optimize its organization.**

The problem with the current state machine is not "too many states", but rather:
1. State definitions are in `slot/status.rs`, but state transition logic is scattered throughout `tick()`
2. `can_transition_to()` is a huge match, but most rules are repetitive (e.g., "any state can go to Stopped")

**Correct optimization**:

```rust
// core/src/worker/lifecycle.rs

/// Use macros or derive to reduce repetitive code
#[derive(StateMachine)]
pub enum Lifecycle {
    Idle,
    Starting,
    Responding { started_at: Instant },
    ToolExecuting { tool_name: String },
    WaitingForInput { started_at: Instant },
    Blocked { reason: BlockedReason },
    BlockedForDecision { state: DecisionState },
    Resting { until: Instant, resume_action: String },
    Finishing,
    Paused { reason: String },
    Stopped { reason: String },
    Error { message: String },
}

/// Use attribute macros to define transition rules (compile-time expansion to can_transition_to)
#[state_machine]
impl Lifecycle {
    // Normal flow
    #[transition(Idle → Starting)]
    #[transition(Starting → Responding)]
    #[transition(Responding → ToolExecuting)]
    #[transition(ToolExecuting → Responding)]
    #[transition(Responding → Finishing)]
    #[transition(Finishing → Idle)]

    // Block/recovery
    #[transition(any → Blocked)]       // Any state can block
    #[transition(Blocked → Idle)]      // After unblock, return to idle
    #[transition(BlockedForDecision → Resting)]
    #[transition(Resting → Idle)]

    // Pause
    #[transition(any → Paused)]
    #[transition(Paused → Idle)]
    #[transition(Paused → Responding)]

    // Terminate
    #[transition(any → Stopped)]       // Any state can stop
    #[transition(any → Error)]
    #[transition(Error → Idle)]
    #[transition(Stopped → Starting)]  // Restart
}
```

This way:
- State count unchanged (preserves complete business semantics)
- But transition rules go from 40+ line match to 15 lines of declarative annotations
- Readability improved, maintenance cost lowered

---

## Reflection 6: `BehaviorStrategy` Trait is Over-Abstraction

### Problem in the Original Plan

The original plan proposes using a `BehaviorStrategy` trait to eliminate `if provider_kind != ProviderKind::Mock`:

```rust
// Original plan设想
trait BehaviorStrategy {
    fn spawn(&self, prompt: &str, context: BehaviorLaunchContext) -> BehaviorHandle;
    fn capabilities(&self) -> BehaviorCapabilities;
}

struct ClaudeStrategy;
struct CodexStrategy;
struct MockStrategy;
```

### Fundamental Differences Between Mock and Real Providers

```rust
// Real Provider (Claude) spawn flow
fn spawn_claude(prompt, cwd, session) {
    // 1. Build Command (executable path, args, env vars)
    // 2. Start OS process (std::process::Command)
    // 3. Establish stdin/stdout pipes
    // 4. Start reader thread (parse JSONL line by line)
    // 5. Return thread_handle + event_rx
}

// Mock "spawn" flow
fn spawn_mock(prompt) {
    // 1. Build mock reply text
    // 2. Split into chunks
    // 3. Send events directly through channel (no OS process)
    // 4. Immediately send Finished
}
```

Differences:
- Claude/Codex have **real OS process lifecycle** (process startup, signals, exit codes)
- Mock has no process, just in-memory simulation
- Claude needs **session resume** (`--resume session_id`)
- Mock doesn't need sessions
- Claude/Codex have **stderr handling** (error logs, debug info)
- Mock has no stderr

If forcibly unified into a trait:
```rust
trait BehaviorStrategy {
    fn spawn(&self, ...) -> BehaviorHandle;  // Mock implementation: ignores most args
    fn resume(&self, session: SessionHandle) -> BehaviorHandle;  // Mock: panic!()
    fn read_stderr(&self) -> Option<String>;  // Mock: returns None
}
```

This leads to the trait having many "Mock doesn't need but is forced to implement" methods, or Mock implementations filled with `unimplemented!()` and `None`.

### Correction Direction

**Keep explicit `ProviderKind` branches, but centralize the branch point.**

```rust
// core/src/behavior/spawner.rs

pub struct BehaviorSpawner;

impl BehaviorSpawner {
    pub fn spawn(
        &self,
        kind: ProviderKind,
        prompt: &str,
        context: SpawnContext,
    ) -> Result<BehaviorHandle, SpawnError> {
        match kind {
            ProviderKind::Claude => self.spawn_claude(prompt, context),
            ProviderKind::Codex => self.spawn_codex(prompt, context),
            ProviderKind::Mock => self.spawn_mock(prompt),
        }
    }

    fn spawn_claude(&self, prompt: &str, ctx: SpawnContext) -> Result<BehaviorHandle, SpawnError> {
        // Claude-specific logic: session resume, stdin write, stderr read
        ...
    }

    fn spawn_codex(&self, prompt: &str, ctx: SpawnContext) -> Result<BehaviorHandle, SpawnError> {
        // Codex-specific logic: exec mode, arg passing
        ...
    }

    fn spawn_mock(&self, prompt: &str) -> Result<BehaviorHandle, SpawnError> {
        // Mock-specific logic: direct chunk generation, no process
        ...
    }
}
```

**Principle**:
- Differences between different Providers are **real and huge**, don't hide them with a trait
- But centralize branches to `BehaviorSpawner`, rather than scattering them across `AgentPool`, `tick()`, `DecisionExecutor`
- If other places need `is_mock()` checks, it indicates insufficient abstraction; should continue to centralize into `BehaviorSpawner`

---

## Reflection 7: Blind Spot of "Unifying ProviderEvent" — Layering Beats Unification

### Problem in the Original Plan

The original plan proposes unifying `core::ProviderEvent` and `decision::ProviderEvent` into `ActorMessage`, claiming "one type through three layers".

### Actual Layering Needs

Look at the decision layer classifier code:

```rust
// decision/src/classifier/classifier_registry.rs
fn classify_type(&self, event: &ProviderEvent) -> Option<SituationType> {
    match event {
        ProviderEvent::Finished { .. } => Some(SituationType::new("claims_completion")),
        ProviderEvent::Error { .. } => Some(SituationType::new("error")),
        // ... other branches
        _ => None,
    }
}
```

The decision layer only cares about **business-level events**:
- Is it finished? (Finished)
- Is there an error? (Error)
- Does it need approval? (ApprovalRequest)

The decision layer **doesn't care** about streaming details:
- `AssistantChunk` — just a text fragment, meaningless for decisions
- `ExecCommandOutputDelta` — intermediate output, meaningless for decisions
- `PatchApplyOutputDelta` — same as above

If unified into one big enum:
```rust
enum ActorMessage {
    AssistantChunk { text: String },        // Decision layer: ignore
    ThinkingChunk { text: String },         // Decision layer: ignore
    ExecCommandOutputDelta { ... },         // Decision layer: ignore
    ExecCommandFinished { ... },            // Decision layer: care
    Finished,                               // Decision layer: care
    // ... 24 variants total
}
```

The decision layer classifier needs to `match` 5 out of 24 variants, with all others `=> None`. This反而 increases the cognitive burden on the decision layer.

### Rationality of Current Design

The three-layer events in the current design are actually reasonable layering:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Layer 1: Raw Protocol Events (RawProtocolEvent)                            │
│  ─────────────────────────────────────                                      │
│  Location: agent/provider/src/providers/claude.rs, codex.rs                 │
│  Content: Raw structures parsed from JSON Lines                             │
│  Example: { "type": "assistant", "content": "hello" }                        │
├─────────────────────────────────────────────────────────────────────────────┤
│  Layer 2: Domain Events (ProviderEvent / ActorMessage)                      │
│  ─────────────────────────────────────                                      │
│  Location: core/src/ (or unified agent-domain-events)                       │
│  Content: Structured domain events, 24 variants                             │
│  Example: AssistantChunk("hello"), ExecCommandStarted { ... }                │
├─────────────────────────────────────────────────────────────────────────────┤
│  Layer 3: Decision Events (DecisionEvent)                                   │
│  ─────────────────────────────────────                                      │
│  Location: decision/src/                                                    │
│  Content: Abstracted business events, 15 variants                           │
│  Example: Finished, Error, ApprovalRequest, ToolCallFinished                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  Layer 4: Protocol Broadcast Events (ProtocolEvent)                         │
│  ─────────────────────────────────────                                      │
│  Location: agent/protocol/src/events.rs                                     │
│  Content: External JSON-serializable events                                 │
│  Example: ItemDelta { item_id, delta: Text("hello") }                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Layering is reasonable because each layer has different concerns.**

### Correction Direction

**Do not unify into a single layer; instead clarify layers and optimize inter-layer mapping.**

The current problem is that mapping code (`event_converter.rs`) is **manually maintained**. Optimization direction:

```rust
// core/src/event/layers.rs

/// Mapping from domain events to decision events
///
/// Use macros or derive to auto-generate, avoiding manual maintenance
#[derive(EventMap)]
#[map(
    core(ProviderEvent::Finished) => decision(DecisionEvent::Finished),
    core(ProviderEvent::Error { message }) => decision(DecisionEvent::Error { message }),
    core(ProviderEvent::ExecCommandFinished { status, exit_code, .. })
        => decision(DecisionEvent::ToolCallFinished { success: status.is_success(), exit_code }),
    // Other mappings...
)]
pub struct CoreToDecisionMap;
```

Or a simpler solution: **Keep current manual mapping, but add compile-time checks** — if `ProviderEvent` adds a new variant but isn't handled in `event_converter.rs`, compilation fails.

```rust
// Use compile-time checking for non-exhaustive enums
impl ProviderEvent {
    pub fn to_decision_event(&self) -> Option<DecisionEvent> {
        match self {
            Self::Finished => Some(DecisionEvent::Finished),
            Self::Error { message } => Some(DecisionEvent::Error { message: message.clone() }),
            // ... other mappings

            // Compile-time guarantee: if new variant added without handling,
            // this will fail to compile because ProviderEvent is not #[non_exhaustive]
        }
    }
}
```

---

## Reflection 8: Pragmatic Path for `tick()` Split

### Problem in the Original Plan

The original plan proposes completely splitting `tick()` into a pure scheduler:

```rust
// Original plan设想
pub struct RuntimeTick<'a> {
    supervisor: &'a mut ActorSupervisor,
    multiplexer: &'a mut MailboxMultiplexer,
    // ...
}

impl<'a> RuntimeTick<'a> {
    pub fn run(&mut self) -> Result<(), RuntimeError> {
        // 1. Collect messages
        // 2. Dispatch to Actors
        // 3. Collect Effects
        // 4. Route Effects
        // ...
    }
}
```

### Risks of Over-Splitting

**Risk 1: Implicit Dependencies in Event Processing Order**

The processing order in current `tick()` is **intentional**:

```rust
// Current tick() order
1. Update slot transcript        ← Must be done first, because subsequent steps may need latest transcript
2. Decision layer classification  ← Needs to be based on updated state
3. Protocol broadcast            ← Needs to be based on raw events (not updated state)
4. Idle check                    ← Needs to be based on latest state
5. Decision execution            ← May start new Provider, changing state
6. Mail processing               ← Process last to avoid affecting current tick's decisions
```

If processing logic is dispersed into `WorkAgent::handle_message()` and `EffectRouter`, these order dependencies are hidden. For example:
- `WorkAgent::handle_message(AssistantChunk)` produces an `EmitProtocol` Effect
- But protocol broadcast should happen **after all slot updates are complete**, not when the first chunk is processed

**Risk 2: Complexity of Cross-Actor Coordination**

Current `tick()` has大量跨 Actor coordination logic:

```rust
// Current code: one event may affect multiple Actors
if let Some(slot) = inner.agent_pool.get_slot_mut_by_id(&agent_id) {
    slot.transition_to(...)?;
    // Simultaneously notify decision layer
    inner.agent_pool.classify_event(&agent_id, &event);
    // Simultaneously update protocol broadcast
    pump.process(agent_id, event);
}
```

If each Actor only processes its own messages, cross-Actor coordination needs additional mechanisms (such as `ClassifyForDecision` in `Effect`). This introduces new complexity rather than eliminating it.

### Correction Direction

**Do not pursue "pure scheduler"; instead split `tick()` into clear Phases.**

```rust
// core/src/runtime/tick.rs

impl Runtime {
    pub async fn tick(&self) -> Result<Vec<ProtocolEvent>, RuntimeError> {
        let mut state = self.state.lock().await;
        let mut broadcast = Vec::new();

        // ===== Phase 1: Collect =====
        // Collect all external inputs (Provider events, user input, timers)
        let events = state.multiplexer.poll_all();
        let timer_events = state.timer_queue.drain_overdue();
        let user_inputs = state.input_queue.drain();

        // ===== Phase 2: Update Workers =====
        // Update all Worker internal states (transcript, lifecycle)
        for event in &events {
            if let Some(worker) = state.pool.get_worker_mut(&event.agent_id) {
                worker.apply_event(&event.payload)?;
            }
        }

        // ===== Phase 3: Classify =====
        // Classify events that need decisions
        for event in &events {
            if event.payload.may_need_decision() {
                if let Some(worker) = state.pool.get_worker(&event.agent_id) {
                    let classify_result = state.decision.classify(&worker, &event.payload);
                    if classify_result.needs_decision() {
                        state.decision_queue.push(classify_result.into_request());
                    }
                }
            }
        }

        // ===== Phase 4: Execute Decisions =====
        // Execute completed decisions
        let decisions = state.decision.poll_completed();
        for (worker_id, outcome) in decisions {
            let commands = state.decision.executor.execute(&mut state.pool, &worker_id, &outcome);
            for cmd in commands {
                self.execute_command(&mut state, cmd)?;
            }
        }

        // ===== Phase 5: Convert to Protocol =====
        // Convert events to protocol events
        for event in &events {
            if let Some(protocol_events) = state.protocol_gateway.convert(&event) {
                broadcast.extend(protocol_events);
            }
        }

        // ===== Phase 6: Idle Check =====
        // Check idle Workers
        for worker in state.pool.idle_workers() {
            if worker.idle_duration() > IDLE_TIMEOUT {
                state.decision_queue.push(DecisionRequest::agent_idle(worker.id()));
            }
        }

        // ===== Phase 7: Cleanup =====
        // Cleanup disconnected channels
        for agent_id in events.disconnected {
            state.multiplexer.remove(&agent_id);
        }

        // ===== Phase 8: Process Mail =====
        state.mail.process_pending();

        Ok(broadcast)
    }
}
```

**Key improvement**:
- `tick()` is still the orchestrator, but each Phase is an independent function
- There are explicit order dependencies between Phases (e.g., Update → Classify → Execute)
- Each Phase can be tested independently (pass in constructed state, assert output)
- No need for Effect system or MessageHandler trait

---

## Reflection 9: DDD Bounded Context Lacks Anti-Corruption Layer

### Problem in the Original Plan

The original plan partitions 5 Bounded Contexts: Runtime, Process, Decision, Protocol, Coordination.

But DDD Bounded Context is not just code organization; it requires:
1. **Context Map**: Clarify relationships between contexts (partnership, customer-supplier, anti-corruption layer, etc.)
2. **Anti-Corruption Layer**: When two context models are inconsistent, use a translation layer to isolate
3. **Ubiquitous Language**: Each context has its own terminology

The original plan only did point 3 (terminology), without designing points 1 and 2.

### Actual Blurred Boundaries

**Blur 1: Runtime ↔ Decision**

```rust
// core/src/pool/decision_executor.rs
pub fn execute(
    slots: &mut [AgentSlot],        // Directly modifies Runtime data!
    human_queue: &mut HumanDecisionQueue,
    work_agent_id: &AgentId,
    output: &DecisionOutput,
) -> DecisionExecutionResult {
    // ...
    slot.append_transcript(TranscriptEntry::User(instruction));
    // ...
}
```

The decision executor directly modifies Runtime's `AgentSlot`, meaning there is **no anti-corruption layer** between the two contexts. If split into two crates, this direct access becomes a compile error, requiring events or commands for indirect communication.

**Blur 2: Runtime ↔ Protocol**

```rust
// agent/daemon/src/session_mgr.rs
let transcript: Vec<TranscriptItem> = app
    .transcript
    .iter()
    .map(|entry| map_transcript_entry(idx, entry))
    .collect();
```

Runtime directly knows Protocol's `TranscriptItem` format, and vice versa Protocol directly knows Runtime's `TranscriptEntry`. These are not two independent contexts, but tightly coupled modules.

### Correction Direction

**Acknowledge that the current system is a single Bounded Context (Agent Runtime), with sub-domains partitioned by module internally.**

Do not forcibly apply DDD's multi-context model because:
- The system scale is not large enough (~30-40k lines total)
- Coupling between modules is real and necessary
- Introducing anti-corruption layers would add unnecessary indirection

**When the system grows to 100k+ lines, with clear team boundaries, then consider splitting into multiple Bounded Contexts.**

---

## Reflection 10: Missing Backward Compatibility and Migration Costs

### Problem in the Original Plan

The original plan proposes a 6-phase migration roadmap with a total timeline of 12-15 weeks. But it does not consider:

**Omission 1: Snapshot Compatibility**

```rust
// core/src/shutdown_snapshot.rs
pub struct AgentShutdownSnapshot {
    pub meta: AgentMeta,
    pub was_active: bool,
    pub provider_thread_state: Option<ProviderThreadSnapshot>,
    pub transcript: Vec<TranscriptEntry>,
    // ...
}
```

Snapshots use JSON serialization. If `AgentSlotStatus` variant names are renamed (e.g., `BlockedForDecision` → `Interacting`), old snapshot recovery will fail deserialization.

**Omission 2: Protocol Compatibility**

```rust
// agent/protocol/src/events.rs
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EventPayload {
    AgentStatusChanged(AgentStatusChangedData),
    // ...
}
```

External protocol (WebSocket/JSON-RPC) field names cannot be arbitrarily changed, because clients (TUI, Web UI) depend on them.

**Omission 3: Logs and Monitoring**

The system has大量 structured logs:
```rust
logging::debug_event(
    "decision_layer.action_executing",
    "executing decision action on work agent",
    serde_json::json!({ "work_agent_id": ..., "action_type": ... }),
);
```

If type names change, field names in logs also change, requiring monitoring alert rules and log analysis queries to be updated synchronously.

**Omission 4: Team Downtime Cost**

During a 12-15 week refactoring:
- Cannot develop new features (otherwise refactoring and development conflict)
- All PRs need to handle merge conflicts
- Test coverage may temporarily decline

For a team with only 2-3 core developers, this means **nearly 4 months of feature stagnation**.

### Correction Direction

**Adopt the "Strangler Fig Pattern": gradual replacement without stopping work.**

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Strangler Fig Pattern Migration                                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Month 1: Extract Phases                                                   │
│  ─────────────────                                                        │
│  - Don't change any type names                                             │
│  - Split tick() into 8 Phase functions                                     │
│  - Each Phase independently tested                                         │
│  - Can develop new features in parallel                                    │
│                                                                             │
│  Month 2: Centralize Behavior Startup Logic                                │
│  ──────────────────────────────                                            │
│  - Centralize Mock/Claude/Codex startup logic into BehaviorSpawner        │
│  - Eliminate provider_kind branches scattered across AgentPool, tick()    │
│  - Don't change type names or interfaces                                   │
│                                                                             │
│  Month 3: Optimize Event Mapping                                           │
│  ─────────────────────                                                     │
│  - Add compile-time checking for ProviderEvent → DecisionEvent mapping    │
│  - Delete unused branches in event_converter.rs                            │
│  - Can develop new features in parallel                                    │
│                                                                             │
│  Month 4+: Rename as Needed                                                │
│  ───────────────────                                                       │
│  - Rename only 1-2 highest priority types at a time                        │
│  - Use type aliases for backward compatibility (type AgentSlot = Worker;)  │
│  - Evaluate after 3 months: do we really need to continue renaming?        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Corrected Refactoring Principles

Based on the above 10 reflections, the corrected refactoring principles are:

### Principle 1: Split Functions First, Then Modules, Then Crates

```
tick() split → independent functions
    ↓
Function categorization → Module
    ↓
Module stability verified → Consider Crate (if truly needed for cross-crate sharing)
```

### Principle 2: Only Rename "Actively Misleading" Names

| Name | Misleading? | Change? |
|:---|:---|:---|
| `AgentSlot` | High (implies container) | ✅ Change |
| `SessionManager` | Medium (Session is business concept) | ✅ Change |
| `ProviderEvent` | Low (imperfect but widely used) | ❌ Don't change |
| `AgentPool` | Low (Pool is already accurate) | ❌ Don't change |

### Principle 3: No Actor Terminology, Use Accurate Engineering Terms

| Concept | Accurate Term |
|:---|:---|
| Single-threaded event loop | Event Loop |
| Background worker thread | Background Worker Thread |
| State encapsulation | State Encapsulation |
| Message channel | Channel / Queue |
| Supervision/management | Pool Management |

### Principle 4: Layering Beats Unification

Keep `ProviderEvent` (domain layer) and `DecisionEvent` (decision layer) separate, connect them with explicit mapping.

### Principle 5: Strangler Fig Pattern Migration

Refactor without stopping work; change one small piece at a time, keep tests passing.

---

## Corrected Minimum Viable Plan (MVP)

### MVP Goal (2-3 weeks, parallel development possible)

**Don't change type names, don't change crate boundaries, don't introduce new abstractions. Do only one thing: make `tick()` readable.**

### Specific Changes

#### Change 1: tick() Phase Split

```rust
// agent/daemon/src/session_mgr.rs

impl SessionManager {
    pub async fn tick(&self) -> Result<Vec<Event>> {
        let mut inner = self.inner.lock().await;
        let mut pump = EventPump::new();
        let mut broadcast_events = Vec::new();

        // Phase 1: Collect events
        let poll_result = inner.event_aggregator.poll_all();

        // Phase 2: Update Slot states
        Self::phase_update_slots(&mut inner, &poll_result.events)?;

        // Phase 3: Decision layer classification
        let decision_requests = Self::phase_classify_events(&inner, &poll_result.events)?;
        for req in decision_requests {
            inner.agent_pool.send_decision_request(&req.agent_id, req.request)?;
        }

        // Phase 4: Protocol conversion
        Self::phase_convert_to_protocol(&mut pump, &poll_result.events, &mut broadcast_events);

        // Phase 5: Idle check
        Self::phase_check_idle_agents(&mut inner)?;

        // Phase 6: Execute decisions
        Self::phase_execute_decisions(&mut inner).await?;

        // Phase 7: Process mail
        inner.mailbox.process_pending();

        // Phase 8: Cleanup
        Self::phase_cleanup_disconnected(&mut inner, &poll_result.disconnected_channels);

        Ok(broadcast_events)
    }
}
```

#### Change 2: Slot State Update Centralization

```rust
// core/src/agent_slot.rs

impl AgentSlot {
    /// Apply a single ProviderEvent, update internal state.
    /// Returns whether decision layer intervention is needed.
    pub fn apply_event(&mut self, event: &ProviderEvent) -> Result<bool, String> {
        self.touch_activity();

        match event {
            ProviderEvent::Finished => {
                if self.status.is_active() {
                    self.transition_to(AgentSlotStatus::idle())?;
                }
                self.clear_provider_thread();
                Ok(true) // needs decision
            }
            ProviderEvent::Error(err) => {
                self.append_transcript(TranscriptEntry::Error(err.clone()));
                self.transition_to(AgentSlotStatus::blocked(err.clone()))?;
                self.clear_provider_thread();
                Ok(true) // needs decision
            }
            ProviderEvent::AssistantChunk(chunk) => {
                self.append_transcript(TranscriptEntry::Assistant(chunk.clone()));
                Ok(false)
            }
            // ... other branches
        }
    }
}
```

#### Change 3: Behavior Startup Centralization

```rust
// agent/daemon/src/session_mgr.rs or core/src/behavior/spawner.rs

struct BehaviorSpawner;

impl BehaviorSpawner {
    fn spawn_for_agent(
        &self,
        pool: &mut AgentPool,
        agent_id: &AgentId,
        prompt: &str,
        record_user_prompt: bool,
    ) -> Result<(), SpawnError> {
        let slot = pool.get_slot_by_id(agent_id)
            .ok_or(SpawnError::AgentNotFound)?;

        match slot.provider_type().to_provider_kind() {
            Some(ProviderKind::Claude) => self.spawn_claude(pool, agent_id, prompt, record_user_prompt),
            Some(ProviderKind::Codex) => self.spawn_codex(pool, agent_id, prompt, record_user_prompt),
            Some(ProviderKind::Mock) => self.spawn_mock(pool, agent_id, prompt, record_user_prompt),
            None => Err(SpawnError::UnknownProvider),
        }
    }
}
```

### MVP Verification Criteria

1. `tick()` function body reduced from 1000+ lines to under 100 lines
2. Each Phase function independently testable
3. `AgentSlot::apply_event()` covers all event types, with independent unit tests
4. `BehaviorSpawner` eliminates all `provider_kind` matches scattered elsewhere
5. `cargo test --workspace` all pass
6. Snapshot format unchanged (backward compatible)
7. External protocol format unchanged (clients need no modifications)

---

## Conclusion

The problem with the original plan is not "thinking wrong", but "thinking too far".

A good architecture document should:
1. **Honestly face costs**: Renaming 32 types is not something that can be done in "1 week"
2. **Distinguish ideal from reality**: The Actor model is inspiration, not a shackle
3. **Start from the minimum viable plan**: First make `tick()` readable, then consider larger refactoring
4. **Acknowledge the rationality of current design**: Three-layer event separation, 14-state state machine, Mock special handling — these are not bugs, they are features

**The highest realm of architecture is not "perfection", but "just right" — just enough to solve the problem, just enough for the team to understand, just enough for future expansion.**
