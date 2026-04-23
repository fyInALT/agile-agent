# agile-agent

`agile-agent` is a Rust workspace for building a local autonomous engineering agent on top of existing coding CLIs such as `claude` and `codex`.

It combines a codex-style terminal UI, persisted workplace and agent state, a headless execution loop, a decision layer for ambiguous provider output, and multi-agent coordination with git worktree isolation.

## Status

Implemented:

- interactive TUI substrate with transcript/composer flow, tool rendering, and session restore
- single-agent runtime identity, workplace persistence, and headless autonomous loop
- local skill discovery and structured prompt injection
- trait-based Kanban domain model with shared test support
- multi-agent foundation with role-aware agents and Scrum-style coordination primitives
- decision-layer foundation in `agent-decision` with classifier and engine building blocks
- git worktree isolation for multi-agent development
- launch configuration overlay for agent creation (Ctrl+N)
- Overview mode for multi-agent activity monitoring
- Git Flow task preparation with automatic branch naming and workspace health checks
- multi-agent headless execution mode with `--multi-agent` flag
- modular decision layer architecture (Core → Model → Pipeline → Engine → Classifier → Provider → State → Runtime → Config)
- provider profile system for configurable LLM backends
- daemon-centric architecture with WebSocket JSON-RPC 2.0 protocol
- modular crate structure: types, toolkit, provider, worktree, backlog, storage, protocol, daemon, commands
- OpenAI-backed LLM provider with simple/thinking model tiers
- **Event-sourced Worker aggregate** with `apply(event) -> Vec<RuntimeCommand>` pattern
- **Shared DomainEvent** (24 variants) across provider and decision layers
- **Pure domain model** in `agent-runtime-domain` (Worker, WorkerState, TranscriptJournal)
- **Effect system** in `agent-behavior-infra` (RuntimeCommand, EffectHandler trait)
- **Frontend-backend separation**: TUI and CLI are protocol-only clients
- **Decision layer decoupling**: `agent-decision` is read-only, produces `DecisionCommand`
- **7-phase EventLoop** with RuntimeCommand dispatch

In progress:

- workflow self-improvement
- full parallel multi-agent runtime across TUI execution
- decision layer integration with daemon decision interpreter

Not started yet:

- advanced workflow automation and self-optimization

## Workspace

```text
agile-agent/
├── agent/                 # Core agent crates (modular architecture)
│   ├── daemon/            # WebSocket server, EventLoop, event pump
│   ├── events/            # Shared DomainEvent (24 variants), DecisionEvent
│   ├── protocol/          # JSON-RPC types, events, state snapshots
│   ├── runtime-domain/    # Pure domain types (Worker, WorkerState, TranscriptJournal)
│   ├── behavior-infra/    # Effect system (RuntimeCommand, EffectHandler trait)
│   ├── types/             # Foundation types (AgentId, TaskId, ProviderKind)
│   ├── toolkit/           # Tool call types (PatchChange, ExecCommand)
│   ├── provider/          # Provider execution (Claude, Codex, launch config)
│   ├── worktree/          # Git worktree management
│   ├── backlog/           # Task/backlog domain
│   ├── storage/           # Persistence layer
│   └── commands/          # Slash command system
├── cli/                   # `agile-agent` binary and CLI integration tests
├── core/                  # Runtime engine, WorkerPool, RuntimeSession, verification
├── decision/              # Decision layer, classifiers, engines, DecisionCommand
├── docs/                  # Product specs, sprint specs
├── kanban/                # Trait-based Kanban domain model
├── llm-provider/          # OpenAI client/provider abstraction
├── scripts/               # Developer helper scripts
├── test-support/          # Shared test harnesses and fixtures
└── tui/                   # Terminal UI, rendering, transcript, overlays
```

Workspace crates:

- `agent-daemon`: WebSocket server owning all runtime state via EventLoop
- `agent-events`: Shared DomainEvent (24 variants) and DecisionEvent for event-sourced architecture
- `agent-runtime-domain`: Pure domain types (Worker aggregate root, WorkerState, TranscriptJournal)
- `agent-behavior-infra`: Effect system with RuntimeCommand and EffectHandler trait
- `agent-protocol`: JSON-RPC 2.0 types, events, snapshots, auto-link
- `agent-types`: Foundation types (AgentId, WorkplaceId, TaskId, ProviderKind)
- `agent-toolkit`: Tool call types (PatchChange, ExecCommandStatus, WebSearchAction)
- `agent-provider`: Provider execution layer (Claude, Codex, Mock, launch config)
- `agent-worktree`: Git worktree isolation for multi-agent development
- `agent-backlog`: Task and backlog management
- `agent-storage`: Persistence layer (snapshot, events.jsonl)
- `agent-commands`: Slash command routing and execution
- `agent-cli`: `agile-agent` binary, TUI entrypoint, CLI subcommands (protocol-first)
- `agent-core`: Runtime engine (WorkerPool, RuntimeSession), verification, artifacts
- `agent-decision`: Classifier, tiered decision engine, DecisionCommand (read-only)
- `agent-tui`: Codex-style terminal UI (protocol-only client)
- `agent-kanban`: Extensible Kanban domain model with trait architecture
- `agent-llm-provider`: OpenAI client with simple/thinking model tiers
- `agent-test-support`: Shared test helpers for workspace crates

Key docs:

- `docs/plan/spec/`: implementation-facing product and sprint specs
- `docs/plan/spec/decision-layer/`: decision-layer architecture and sprint specs
- `docs/plan/spec/multi-agent/`: multi-agent sprint specs
- `docs/superpowers/specs/`: superpowers design specs
- `docs/superpowers/plans/`: superpowers implementation plans

## Architecture

### Overview

agile-agent uses a layered crate architecture with event-sourced domain model and frontend-backend separation:

- **Frontend (TUI/CLI)**: Protocol-only clients connecting to daemon via WebSocket JSON-RPC 2.0
- **Daemon (`agent-daemon`)**: Owns all runtime state via EventLoop, serves snapshots and broadcasts events
- **Core (`agent-core`)**: Runtime engine integration layer (WorkerPool, RuntimeSession)
- **Decision Layer (`agent-decision`)**: Read-only decision engine producing DecisionCommand

```text
┌─────────────────────────────────────────────────────────────┐
│                     Frontend Layer                          │
│  ┌──────────────┐  ┌──────────────┐                        │
│  │  agent-cli   │  │  agent-tui   │                        │
│  │  (binary)    │  │ (protocol-   │                        │
│  │              │  │  only client)│                        │
│  └──────┬───────┘  └──────┬───────┘                        │
└─────────┼─────────────────┼──────────────────────────────────┘
          │                 │
          │   WebSocket     │   JSON-RPC 2.0
          ▼                 ▼
┌─────────────────────────────────────────────────────────────┐
│                    Daemon Layer                              │
│              agent-daemon (~95k lines)                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  EventLoop   │  │ Broadcaster  │  │ConnectionManager │  │
│  │ (7 phases,   │  │ (event fan-  │  │ (auth, heartbeat)│  │
│  │  RuntimeCmd) │  │    out)      │  │                  │  │
│  └──────┬───────┘  └──────────────┘  └──────────────────┘  │
└─────────┼───────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│                    Runtime Layer                             │
│              agent-core (~500k lines)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │  WorkerPool  │  │RuntimeSession│  │ EventAggregator  │  │
│  │ (lifecycle, │  │ (bootstrap,  │  │ (multi-source    │  │
│  │   focus)     │  │  persistence)│  │  event poll)     │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │DecisionAgent │  │ WorkerHandle │  │ SharedWorkplace  │  │
│  │Coordinator   │  │ (agent slot) │  │ State            │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────┬───────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┐
    ▼             ▼             ▼
┌───────────┐ ┌──────────┐ ┌──────────────┐
│  agent-   │ │ agent-   │ │ agent-       │
│  decision │ │ events   │ │ behavior-    │
│ (Decision │ │ (Domain  │ │ infra        │
│  Command) │ │  Event)  │ │(EffectSystem)│
└───────────┘ └──────────┘ └──────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────┐
│                    Domain Layer                              │
│  ┌─────────────────┐  ┌──────────────────────────────────┐ │
│  │agent-runtime-   │  │ agent-events                      │ │
│  │domain           │  │ (DomainEvent 24 variants,         │ │
│  │(Worker aggregate│  │  DecisionEvent protocol)          │ │
│  │ root, pure      │  │                                    │ │
│  │ state machine)  │  │                                    │ │
│  └─────────────────┘  └──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┬─────────────┐
    ▼             ▼             ▼             ▼
┌───────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│  agent-   │ │ agent-   │ │ agent-   │ │ agent-   │
│  worktree │ │ backlog  │ │ kanban   │ │ provider │
│ (git ops) │ │(task st) │ │(Scrum)   │ │(Claude/  │
└───────────┘ └──────────┘ └──────────┘ │ Codex)   │
                                         └──────────┘
                  │
                  ▼
         ┌─────────────────┐
         │   agent-types   │
         │ (foundation IDs │
         │  & enums, zero  │
         │   dependencies) │
         └─────────────────┘
```

### Crate Dependency Graph

```text
agent-cli ──┬──► agent-core ──┬──► agent-events
             │                 ├──► agent-runtime-domain
             │                 ├──► agent-behavior-infra
             │                 ├──► agent-types
             │                 ├──► agent-toolkit
             │                 ├──► agent-decision
             │                 ├──► agent-kanban
             │                 ├──► agent-provider
             │                 ├──► agent-worktree
             │                 ├──► agent-backlog
             │                 ├──► agent-storage
             │                 ├──► agent-commands
             │                 └──► agent-llm-provider (via decision)
             │
             ├──► agent-protocol ──► agent-types
             │
             └──► agent-tui ──► agent-protocol
                (protocol-only)

agent-daemon ──► agent-core
             ──► agent-protocol
             ──► agent-events
             ──► agent-types

agent-events ──► agent-types

agent-runtime-domain ──► agent-types

agent-behavior-infra ──► agent-events
                        ──► agent-runtime-domain

agent-provider ──► agent-types
                ──► agent-toolkit
                ──► agent-events

agent-kanban ──► agent-backlog ──► agent-types

agent-worktree ──► agent-types
agent-backlog  ──► agent-types
agent-commands ──► (none within workspace)
agent-storage  ──► agent-types
agent-toolkit  ──► agent-types
agent-protocol ──► agent-types
agent-llm-provider ──► (none)
agent-decision ──► agent-events
agent-test-support ──► agent-core
```

### Foundation Layer

#### `agent-types` — Foundation Types
Zero-dependency crate containing all shared domain primitives. Every other crate depends on this.
- `AgentId`, `WorkplaceId`, `AgentCodename` — strongly-typed string wrappers
- `AgentRole` — `ProductOwner | ScrumMaster | Developer` with prompt prefixes and skills
- `RuntimeMode` — `SingleAgent | MultiAgent`
- `AgentStatus`, `TaskStatus`, `TodoStatus` — lifecycle enums
- `ProviderKind` — `Mock | Claude | Codex`

#### `agent-events` — Shared Domain Events
Event-sourced architecture foundation with 24 domain event variants.
- `DomainEvent` — 24 variants: `WorkerSpawned`, `WorkerStatusChanged`, `TaskAssigned`, `DecisionRequested`, etc.
- `DecisionEvent` — Decision layer protocol events
- Event-driven communication between provider and decision layers

#### `agent-runtime-domain` — Pure Domain Model
Pure types with no external dependencies, forming the domain core.
- `Worker` — Aggregate root with `apply(event) -> Vec<RuntimeCommand>` pattern
- `WorkerState` — State machine with explicit transitions
- `TranscriptJournal`, `JournalEntry` — Transcript persistence types

#### `agent-behavior-infra` — Effect System
Effect handling infrastructure separating pure state transitions from side effects.
- `RuntimeCommand` — Effect enum: spawn provider, send message, update state
- `EffectHandler` trait — Injectable effect execution
- `NoopEffectHandler`, `RecordingEffectHandler` — Test utilities

#### `agent-toolkit` — Tool Call Types
Types for tool invocations and their statuses.
- `ExecCommandStatus`, `PatchApplyStatus`, `McpToolCallStatus`
- `McpInvocation`, `WebSearchAction`, `PatchChange`

#### `agent-commands` — Slash Command Bus
Parses and routes slash commands (`/local`, `/agent`, `/provider`).
- `CommandNamespace` (`Local | Agent | Provider`)
- `CommandInvocation`, `parse_slash_command()` via `shlex`
- Static `COMMAND_SPECS` registry

### Protocol & Provider Layer

#### `agent-protocol` — JSON-RPC 2.0 Wire Protocol
Defines all daemon-client message types. Used by both embedded and protocol modes.
- `JsonRpcMessage`, `JsonRpcRequest`, `JsonRpcResponse`
- `Event` / `EventPayload` — `AgentSpawned`, `ItemDelta`, `ApprovalRequest`, etc.
- `SessionState`, `AppStateSnapshot`, `AgentSnapshot`
- `DaemonConfig` — on-disk `daemon.json` with atomic write and SHA-256 checksum
- `ResolvedWorkplace` — workplace ID derivation via FNV-1a hash

#### `agent-provider` — Provider Execution Layer
Manages CLI provider processes and tool execution.
- `ProviderEvent` — rich enum: `AssistantChunk`, `ThinkingChunk`, `ExecCommandStarted/Finished`, `PatchApplyStarted/Finished`, etc.
- `SessionHandle` — `ClaudeSession { session_id }` | `CodexThread { thread_id }`
- `start_provider()` — spawns named OS threads running the LLM CLI
- `ProviderThreadHandle` — graceful shutdown with timeout, panic detection
- Provider-specific parsers: `providers/claude.rs` (stream-json), `providers/codex.rs`
- **Profile system**: `ProviderProfile`, `ProfileStore`, `${VAR}` interpolation
- **Launch config**: `LaunchInputSpec`, `ResolvedLaunchSpec`, `AgentLaunchBundle`

#### `agent-llm-provider` — OpenAI API Client
Standalone OpenAI client with no workspace dependencies.
- `LlmProvider` trait — `complete()`, `complete_streaming()`, `complete_async()`
- `LlmClient` — `reqwest`-based blocking facade over async internals
- `ModelType` — `Simple` vs `Thinking` tiers
- `MockLlmProvider` / `EchoMockProvider` for tests

### Domain Layer

#### `agent-backlog` — Task & Backlog State
- `BacklogState` — owns `Vec<TodoItem>` + `Vec<TaskItem>`
  - `ready_todos()`, `start_task()`, `complete_task()`, `block_task()` with guarded transitions
- `ThreadSafeBacklog` — `Mutex`-based wrapper with poison recovery

#### `agent-worktree` — Git Worktree Isolation
- `WorktreeManager` — `create()`, `remove()`, `prune()`, `create_for_agent()`
- `WorktreeState` / `WorktreeStateStore` — persistent state for resume
- `GitFlowExecutor` — `prepare_for_task()` with auto branch naming, health checks, baseline sync
- `GitFlowConfig` — branch naming patterns (`<type>/<task-id>-<desc>`), task type classification

#### `agent-kanban` — Trait-Based Kanban Model
Extensible Scrum board domain with trait-based elements.
- `KanbanElementTrait` — identity, content, status, dependencies
- Concrete elements: `Sprint`, `Story`, `Task`, `Idea`, `Issue`, `Tips`
- `KanbanService<R>` — create, update status, manage dependencies
- `KanbanEventBus` — event-driven updates
- `FileKanbanRepository` — JSON file persistence with `index.json`
- `TransitionRule` / `TransitionRegistry` — dependency-aware status transitions

#### `agent-decision` — Tiered Decision Layer (Read-Only)
Autonomous decision-making with situation classification and tiered engine selection. Produces `DecisionCommand` for the daemon to interpret.
- `DecisionPipeline` — main entry: Pre-Processors → Strategy → Maker → Post-Processors
- `TieredDecisionEngine` — routes by complexity:
  - **Simple** → `RuleBasedDecisionEngine`
  - **Medium/Complex** → `LLMDecisionEngine`
  - **Critical** → `CLIDecisionEngine` (human escalation)
- `OutputClassifier` — converts provider events into `ClassifyResult`
- `DecisionCommand` — output enum: `Continue`, `RequestHumanDecision`, `MarkTaskComplete`, `Escalate`, etc.
- `ActionRegistry` / `SituationRegistry` — built-in cases
- `LLMCaller` trait — injectable LLM backend

### Runtime Layer (`agent-core`)

`agent-core` is the coordination heart (~500k lines total). It integrates sub-crates and provides the runtime engine.

#### Worker Aggregate Root (Event-Sourced)
- `Worker` (in `agent-runtime-domain`) — Pure aggregate root: `apply(event) -> Vec<RuntimeCommand>`
- `WorkerState` — 12+ status variants with explicit transition matrix
- State transitions are validated; effects are delegated to `EffectHandler`

#### Runtime Session & Event Aggregation
- `RuntimeSession` — Top-level session with `SharedWorkplaceState` and `WorkerPool`
- `SharedWorkplaceState` — Workplace-wide: `BacklogState`, `SkillRegistry`, `LoopControlFlags`
- `EventAggregator` — Polls all provider channels non-blockingly, merges events

#### Worker Pool (Multi-Agent Coordination)
- `WorkerPool` (alias `AgentPool`) — Vec of `WorkerHandle`s with delegated sub-modules:
  - `WorkerLifecycleManager` — spawn/stop/pause/resume
  - `TaskAssignmentCoordinator` — assign/auto-assign/complete tasks
  - `FocusManager` — TUI focus index
  - `BlockedHandler` — blocked agent config, history, notifier
  - `DecisionAgentCoordinator` — 1:1 decision agent pairing
  - `WorktreeCoordinator` — git worktree bridge
  - `DecisionExecutor` — executes decision layer outputs

#### Worker Handle (Single Agent)
- `WorkerHandle` (alias `AgentSlot`) — Owns one agent's runtime: status, session, transcript, assigned task, provider thread handle, `mpsc::Receiver<ProviderEvent>`, worktree info
- `WorkerSlotStatus` — 12+ variants: `Idle`, `Starting`, `Responding`, `ToolExecuting`, `Finishing`, `Stopping`, `Stopped`, `Error`, `Blocked`, `BlockedForDecision`, `Paused`, `WaitingForInput`, `Resting`

#### Decision Layer Integration
- `DecisionAgentSlot` — Paired decision agent per work agent. Owns `TieredDecisionEngine`, `ActionRegistry`, async provider thread
- `DecisionRequest` / `DecisionResponse` — mpsc channel-based request/response
- `DecisionMail` — Split into `DecisionMailSender` / `DecisionMailReceiver`

#### Provider Profile System
- `ProviderProfile`, `ProfileId`, `ProfileStore`, `ProfilePersistence`
- `CliBaseType` — `Claude | Codex | Custom`

### UI Layer

#### `agent-tui` — Protocol-Only Terminal UI
TUI connects to daemon via WebSocket JSON-RPC 2.0. All runtime state lives in the daemon.
- `app_loop.rs` (~140k lines) — main event loop: render → decision poll → autonomous loop → input → provider drain → multi-agent event poll → idle checks → mail processing
- `render.rs` (~95k lines) — ratatui renderer for all view modes
- `ui_state.rs` (~170k lines) — `TuiState`: event handling, composer, overlays, view state cache
- `view_mode.rs` — Multiple modes: `Overview`, `Focused`, `Split`, `Dashboard`, `Mail`, `TaskMatrix`, `TaskDetail`
- `composer/textarea.rs` — Unicode-aware grapheme-cluster input widget
- `input.rs` — `InputOutcome` enum (~30 variants) mapping keys to semantic actions
- `protocol_app_loop.rs` — async WebSocket event loop
- `protocol_client.rs`, `reconnecting_client.rs`, `websocket_client.rs` — JSON-RPC 2.0 over WebSocket
- `effect_handler.rs` — `TuiEffectHandler` implementing `EffectHandler` trait

#### `agent-cli` — Protocol-First Binary Entry Point
- `main.rs` → app runner
- Subcommands: `doctor`, `agent`, `workplace`, `decision`, `profile`, `resume-last`, `run-loop`, `probe`, `daemon`
- Headless autonomous loop with `LoopGuardrails`
- Protocol client for WebSocket JSON-RPC communication

### Daemon Layer

#### `agent-daemon` — Per-Workspace Daemon (Binary)
The only binary crate under `agent/`. Owns all runtime state via EventLoop and serves JSON-RPC 2.0 over WebSocket.
- `main.rs` — CLI args, `EventLoop` bootstrap, `WebSocketServer`, SIGTERM handling
- `server.rs` — binds `127.0.0.1:0` (ephemeral port)
- `router.rs` — method dispatch table
- `session_mgr.rs` (~95k lines) — owns `RuntimeSession`, `WorkerPool`, `EventAggregator`, worker mailboxes
  - `snapshot()` / `write_snapshot()` — SHA-256 checksum validation
  - 7-phase event loop with `RuntimeCommand` dispatch
- `connection.rs` (~24k lines) — per-connection state machine: localhost origin check, bearer token auth, rate limiter (token bucket), heartbeat timeout (120s), input truncation (1MB)
- `broadcaster.rs` — `EventBroadcaster` fans out to all clients
- `event_pump.rs` — converts `ProviderEvent` → protocol `Event` with monotonic seq numbers
- `lifecycle.rs` — `DaemonLifecycle`: start → accept loop → graceful shutdown → snapshot → rotate backups (keep 3) → remove `daemon.json`
- `handler/` — Request handlers for JSON-RPC methods

### Key Architectural Patterns

#### 1. Event-Sourced Worker Aggregate
The `Worker` aggregate root (in `agent-runtime-domain`) follows the event-sourcing pattern: `apply(event) -> Vec<RuntimeCommand>`. State transitions are pure, and side effects are delegated to `EffectHandler`. This enables:
- Deterministic state reconstruction from event history
- Easy testing with `RecordingEffectHandler`
- Clear separation of concerns

#### 2. Frontend-Backend Separation
TUI and CLI are protocol-only clients connecting to daemon via WebSocket. All runtime state lives in the daemon's EventLoop. This enables:
- Multiple frontend clients connecting to same daemon
- Daemon restart without losing state (via snapshots)
- Protocol-level compatibility guarantees

#### 3. Decision Layer Decoupling
`agent-decision` is read-only — it produces `DecisionCommand`, which is interpreted by `DecisionCommandInterpreter` in the daemon. This prevents the decision layer from directly mutating runtime state.

#### 4. Thread-Based Concurrency with mpsc Channels
Each agent spawns a named OS thread running the LLM CLI process. Communication is one-way: provider thread → main thread via `std::sync::mpsc::Receiver<ProviderEvent>`. The `EventAggregator` polls all channels non-blockingly. Decision agents use a second mpsc pair (`DecisionMail`) for request/response.

#### 5. Explicit State Machine (`WorkerSlotStatus`)
`WorkerSlotStatus` has 12+ variants with an explicitly validated transition matrix. Invalid transitions return `Err(String)`. This prevents illegal state moves.

#### 6. Decision-Agent Pairing (1:1)
Every non-Mock work agent gets a paired `DecisionAgentSlot`. The decision agent owns a `TieredDecisionEngine` and processes `DecisionRequest`s asynchronously in a background thread to avoid blocking the TUI.

#### 7. Worktree Isolation for Parallel Agents
`WorkerPool` can spawn agents with git worktrees. Each agent gets its own branch (`agent/{id}`) and directory. `WorktreeCoordinator` manages state persistence and recreation on resume.

#### 8. Provider Profile System
Named profiles with custom env vars, CLI args, and `${VAR}` interpolation. `WorkerPool` can spawn agents via `spawn_agent_with_profile`, resolving `CliBaseType` → `ProviderKind`.

#### 9. Autonomous Loop with Guardrails
`loop_runner.rs` implements `run_loop()`: pick ready todo → plan task → execute via provider → verify → continue/escalate. `LoopGuardrails` enforce `max_iterations`, `max_continuations_per_task`, `max_verification_failures`.

#### 10. Event-Driven Transcript Updates
`WorkerHandle` maintains transcripts. Tool call entries are updated in-place by `call_id`. The `EventAggregator` collects events from all agents for the TUI loop.

#### 11. Persistence & Crash Recovery
Runtime persists `meta.json`, `state.json`, `transcript.json`, `messages.json`, `memory.json` per agent. `ShutdownSnapshot` captures all agent states on quit. The daemon writes `snapshot.json` with SHA-256 checksums.

#### 12. Trait-Based Extensibility
Multiple crates use object-safe traits with `clone_boxed()`:
- `agent-decision`: `DecisionEngine`, `DecisionMaker`, `DecisionStrategy`, `OutputClassifier`
- `agent-kanban`: `KanbanElementTrait`, `TransitionRule`
- `agent-llm-provider`: `LlmProvider`
- `agent-behavior-infra`: `EffectHandler`

## Features

### Providers

- `claude` via a stream-JSON bridge
- `codex` via an app-server stdio bridge
- `mock` for local development, tests, and headless fallback

**Provider Profiles**: Configure different LLM backends using named profiles. See [docs/profile-configuration.md](docs/profile-configuration.md) for details.

Default provider selection order:

1. `claude`
2. `codex`
3. `mock`

Runtime mode constraints:

- the TUI requires at least one real provider (`claude` or `codex`) and exits early otherwise
- headless `run-loop` can still bootstrap with `mock` for local development and tests

Session continuity is reused when available:

- Claude via `session_id`
- Codex via `thread_id`

### CLI And Execution Modes

- interactive TUI via `cargo run -p agent-cli`
- TUI resume flow via `cargo run -p agent-cli -- resume-last`
- headless autonomous loop via `cargo run -p agent-cli -- run-loop ...`
- agent and workplace inspection via `cargo run -p agent-cli -- agent ...` and `cargo run -p agent-cli -- workplace current`
- decision request inspection via `cargo run -p agent-cli -- decision ...`
- profile management via `cargo run -p agent-cli -- profile list [--verbose]`
- structured environment probing via `cargo run -p agent-cli -- doctor` and `cargo run -p agent-cli -- probe --json`

Current CLI limitations:

- `agent stop` only records a stop request; fully stopping a live agent still requires the TUI session
- `decision` commands are present, but the end-to-end persisted decision workflow is still being wired through

### TUI

The TUI provides:

- a codex-style transcript and composer layout
- multiline editing and paste support
- transcript paging and overlay browsing
- width-aware Markdown rendering for assistant output
- tool output rendering for exec, web, image, and patch events
- patch summaries with file change statistics
- a local skill browser
- multi-agent session state where provider switching creates a new agent identity
- current agent identity in the footer
- Overview mode for monitoring all agents at a glance
- launch configuration overlay for customizing new agents (Ctrl+N)
- git worktree isolation for parallel agent work

Common keybindings:

- `Enter`: submit
- `Ctrl+J`: newline
- `Tab`: cycle to next agent (when multiple agents exist)
- `Ctrl+N`: open launch config overlay to create a new agent
- `Ctrl+P`: toggle/switch provider
- `Ctrl+T`: open transcript overlay
- `$`: open skill browser when the composer is empty
- `Ctrl+C`: quit (or close overlay when open)
- `Alt+1-6`: switch view modes (Focused, Split, Dashboard, Mail, Skills, Overview)
- `Alt+V`: cycle view modes

Preferred namespaced slash commands:

- `/local help`
- `/local status`
- `/local kanban list`
- `/local config get <key>`
- `/local config set <key> <value>`
- `/agent status`
- `/agent <target> status`
- `/agent summary`
- `/provider /status`
- `/provider <target> /status`

Legacy flat commands still accepted:

- `/help`
- `/provider`
- `/skills`
- `/doctor`
- `/backlog`
- `/todo-add <title>`
- `/run-once`
- `/run-loop`
- `/quit`

The TUI requires at least one real provider (`claude` or `codex`) to be installed.

### Decision Layer

The `agent-decision` crate provides:

- provider-output classification by situation type
- rule-based, LLM-backed, CLI, mock, and tiered decision engines
- action and situation registries with built-in cases
- `DecisionCommand` output for daemon interpretation
- human decision request and response types for escalation flows

**Read-only design**: The decision layer does not directly mutate runtime state. It produces `DecisionCommand`, which is interpreted by `DecisionCommandInterpreter` in the daemon.

Current limitation:

- decision layer integration with daemon is in progress; the end-to-end decision UX is still being wired through

### Autonomous Loop

The current loop:

1. picks the next ready todo
2. creates or resumes a task
3. executes through the selected provider
4. continues unfinished work up to the continuation guardrail
5. runs verification
6. marks the task `done`, `failed`, or `blocked`
7. writes task artifacts and escalations when needed

Default headless guardrails:

- `max_iterations = 5`
- `max_continuations_per_task = 3`
- `max_verification_failures = 1`

### Agent Runtime

The runtime treats one `agent` as a first-class object:

- one agent maps to one long-lived runtime identity
- one agent binds to one provider type
- one agent reuses one provider session continuity when available
- switching provider in the TUI creates a new agent instead of mutating the current one

The runtime currently persists:

- `agent_id`
- `codename`
- `workplace_id`
- `provider_type`
- `provider_session_id`
- `created_at`
- `updated_at`
- `status`

On startup from the same working directory, `agile-agent` restores the most recent agent for the derived workplace and reattaches provider session continuity when possible.
When `resume-last` is used, it prefers the current agent's own `state.json` before falling back to older workplace-scoped session files.

### Launch Configuration

When creating a new agent (via Ctrl+N), the launch configuration overlay allows:

- selecting the target provider (claude, codex, or mock)
- configuring environment variable overrides (KEY=VALUE format)
- specifying custom executable paths or command fragments
- previewing parsed configuration before launch

Launch modes:

- `HostDefault`: use the provider's default executable and environment
- `EnvOnly`: use default executable with custom environment variables
- `CommandFragment`: specify full command with executable and arguments

### Git Flow Task Preparation

When agents start new tasks, the decision layer now ensures proper Git workflow:

- **Automatic branch naming**: Branch names follow `<type>/<task-id>-<desc>` convention
- **Baseline sync**: Tasks always start from latest main/master
- **Uncommitted handling**: Changes are classified and handled (commit, stash, discard, or prompt)
- **Workspace health checks**: Health score determines readiness before task start
- **Conflict detection**: Merge/rebase conflicts trigger human intervention

Task types detected from keywords:

| Type | Keywords |
|------|----------|
| Feature | add, implement, create, new |
| Bugfix | fix, bug, issue, error |
| Refactor | refactor, simplify, optimize |
| Docs | document, readme, doc |
| Test | test, testing, spec |

Configuration via `GitFlowConfig` (see `core/src/git_flow_config.rs`).

### Git Worktree Isolation

Agents can operate in isolated git worktrees for parallel development:

- each agent can have its own worktree with a dedicated branch
- worktrees are created and managed automatically by the WorktreeManager
- supports pause/resume of agents with worktree state preservation
- enables safe parallel work on different features without conflicts

### Local Skills

Skills are discovered from:

- `<cwd>/.agile-agent/skills`
- `<cwd>/skills`
- the platform config directory under `agile-agent/skills`

Each skill lives in its own directory and must contain a `SKILL.md`. Enabled skills are injected into the next provider prompt as structured context.

## Quick Start

The examples below use `cargo run -p agent-cli -- ...`; the built binary name is `agile-agent`.

Requirements:

- Rust toolchain
- for the TUI: `claude` or `codex` installed and available on `PATH`
- for headless local experimentation: no real provider is required because `run-loop` can fall back to `mock`
- optional overrides through `AGILE_AGENT_CLAUDE_PATH` or `AGILE_AGENT_CODEX_PATH`

Build:

```bash
cargo build
```

Check provider availability:

```bash
cargo run -p agent-cli -- doctor
```

Launch the TUI:

```bash
cargo run -p agent-cli
```

Resume the most recent saved TUI session:

```bash
cargo run -p agent-cli -- resume-last
```

Inspect the current workplace:

```bash
cargo run -p agent-cli -- workplace current
```

Inspect the current or most recent agent:

```bash
cargo run -p agent-cli -- agent current
```

List known agents in the current workplace, including stopped ones:

```bash
cargo run -p agent-cli -- agent list --all
```

Show detailed status for a specific agent:

```bash
cargo run -p agent-cli -- agent status <agent-id>
```

Spawn a new agent on a specific provider:

```bash
cargo run -p agent-cli -- agent spawn codex
```

Inspect decision requests:

```bash
cargo run -p agent-cli -- decision list
```

Run the autonomous loop headlessly:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5
```

Resume the most recent saved session before the headless loop starts:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5 --resume-last
```

Try the experimental multi-agent headless flag:

```bash
cargo run -p agent-cli -- run-loop --max-iterations 5 --multi-agent
```

Print structured probe output:

```bash
cargo run -p agent-cli -- probe --json
```

Developer checks:

```bash
cargo check
cargo test
```
