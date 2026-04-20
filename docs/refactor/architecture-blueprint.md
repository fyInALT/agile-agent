# Architecture Blueprint: TUI-Backend Separation

> Status: Draft  
> Date: 2026-04-20  
> Scope: High-level architectural direction for decoupling `agent-tui` from `agent-core`

---

## 1. Current State Diagnosis

### 1.1 The Core Problem: TUI Is the Runtime

In the current architecture, `agent-tui` does not merely display state — it **owns** the entire runtime. `TuiState` embeds four core runtime objects directly:

```rust
pub struct TuiState {
    pub session: RuntimeSession,           // AppState + AgentRuntime + workplace
    pub agent_pool: Option<AgentPool>,     // All agent slots
    pub event_aggregator: EventAggregator, // All provider channels
    pub mailbox: AgentMailbox,             // Cross-agent mail
    // ... rendering-only fields
}
```

This means:
- **Closing the terminal kills the agent** — there is no persistence of the runtime
- **Only one TUI can exist per session** — the runtime is trapped inside a single process
- **TUI imports 140+ symbols from core** — across 18 files, with deep method call chains
- **`ProviderEvent` is matched in 3 separate locations** — main loop, overview, and render

### 1.2 The Layer Inversion

The current dependency direction is backwards:

```
TUI ──owns──► RuntimeSession ──drives──► Provider Threads
  │                                         ▲
  │                                         │
  └── EventAggregator (mpsc polling) ───────┘
```

The TUI is both the **view** and the **controller**. The event loop runs inside `app_loop.rs`, which directly mutates `AgentPool`, `AppState`, and `Mailbox` on every frame tick.

### 1.3 CLI-TUI Coupling

`agent-cli` depends on `agent-tui` as a **library crate** (`Cargo.toml: agent-tui = { path = "../tui" }`). The CLI is not a separate client — it embeds the TUI library and calls `agent_cli::app_runner::run()`, which in turn calls `agent_tui::run_tui()`.

This means the CLI cannot exist without the TUI codebase, even for headless operations.

### 1.4 Core Bloat

`agent-core` contains ~27,000 lines across 45+ modules. It mixes:
- Runtime primitives (`agent_runtime`, `agent_slot`)
- Pool orchestration (`agent_pool`, `pool/*`)
- Task execution (`task_engine`, `loop_runner`)
- Scrum process (`standup_report`, `blocker_escalation`)
- Configuration (`global_config`, `provider_profile`)
- Infrastructure (`logging`, `workplace_store`)

There is no clear internal boundary — any module can import any other.

---

## 2. Target Architecture

### 2.1 Layer Model

We introduce **four layers** with strict dependency rules:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ LAYER 4: Presentation                                                       │
│ ┌────────────┐  ┌────────────┐  ┌────────────┐                             │
│ │  TUI       │  │  CLI       │  │  IDE / Web │  (future)                   │
│ │  ratatui   │  │  headless  │  │  plugins   │                             │
│ └─────┬──────┘  └─────┬──────┘  └─────┬──────┘                             │
│       │               │               │                                     │
│       └───────────────┴───────────────┘                                     │
│                       │                                                     │
│                       ▼                                                     │
│              ┌────────────────────┐                                         │
│              │  agent-protocol    │  ← Message contract (WebSocket types)   │
│              └────────────────────┘                                         │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ LAYER 3: Service                                                            │
│ ┌─────────────────────────────────────────────────────────────┐             │
│ │  agent-daemon  (one process per workspace directory)        │             │
│ │                                                             │             │
│ │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │             │
│ │  │ SessionMgr   │  │ WebSocket    │  │ EventBus     │     │             │
│ │  │              │  │ Server       │  │              │     │             │
│ │  │ - per-cwd    │  │              │  │ - broadcast  │     │             │
│ │  │ - snapshot   │  │ - 127.0.0.1  │  │ - subscribe  │     │             │
│ │  │ - auto-link  │  │ - ephemeral  │  │ - persist    │     │             │
│ │  └──────────────┘  └──────────────┘  └──────────────┘     │             │
│ └─────────────────────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ LAYER 2: Domain                                                             │
│ ┌─────────────────────────────────────────────────────────────┐             │
│ │  agent-core  (library — consumed by daemon, not by TUI)     │             │
│ │                                                             │             │
│ │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │             │
│ │  │ AgentPool    │  │ AppState     │  │ TaskEngine   │     │             │
│ │  │ AgentSlot    │  │ Backlog      │  │ LoopRunner   │     │             │
│ │  │ AgentRuntime │  │ Mailbox      │  │ Verifier     │     │             │
│ │  └──────────────┘  └──────────────┘  └──────────────┘     │             │
│ └─────────────────────────────────────────────────────────────┘             │
│                                                                             │
│ ┌─────────────────────────────────────────────────────────────┐             │
│ │  agent-decision  │  agent-kanban  │  (domain plugins)       │             │
│ └─────────────────────────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ LAYER 1: Infrastructure                                                     │
│ ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│ │ agent-types  │  │ agent-toolkit│  │ agent-provider│  │ agent-storage│    │
│ │ (identity)   │  │ (tool calls) │  │ (LLM proc)   │  │ (persistence)│    │
│ └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘     │
│ ┌──────────────┐  ┌──────────────┐                                          │
│ │ agent-backlog│  │ agent-worktree│                                         │
│ │ (task model) │  │ (git isolation)│                                        │
│ └──────────────┘  └──────────────┘                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Dependency Rules

| Layer | Can Import From | Cannot Import |
|-------|----------------|---------------|
| Presentation (TUI, CLI) | `agent-protocol` only | `agent-core`, `agent-decision`, etc. |
| Service (daemon) | `agent-core`, `agent-protocol`, all infra | `agent-tui`, `agent-cli` |
| Domain (core, decision, kanban) | All infra | `agent-tui`, `agent-cli`, `agent-daemon` |
| Infrastructure | `agent-types` only | All upper layers |

### 2.3 Per-Workspace Daemon Model

Each working directory gets its own **completely isolated** daemon process:

```
~/projects/api-server/
  ├── .agile-agent/          (workplace storage)
  │   ├── daemon.json        (WebSocket URL, session ID, alias)
  │   ├── events.jsonl       (append-only event log)
  │   └── snapshot.json      (last shutdown snapshot)
  └── src/ ...

~/projects/frontend/
  ├── .agile-agent/
  │   ├── daemon.json        (different port, different session)
  │   ├── events.jsonl
  │   └── snapshot.json
  └── src/ ...
```

**Auto-link flow**:

```
User runs `agent-cli` in ~/projects/api-server
    │
    ▼
WorkplaceStore::for_cwd() resolves workplace ID
    │
    ▼
Check ~/.agile-agent/workplaces/<id>/daemon.json
    │
    ├── Exists + heartbeat fresh ──► Connect to ws://127.0.0.1:<port>
    │
    ├── Exists + heartbeat stale ──► Remove stale, spawn new daemon, connect
    │
    └── Missing ───────────────────► Spawn new daemon, write config, connect
```

### 2.4 Communication Model

**Before**: TUI polls `mpsc::Receiver<ProviderEvent>` every frame.

**After**: Daemon pushes events over WebSocket; TUI receives them asynchronously.

```
Provider Threads
    │
    ▼ mpsc
EventAggregator (inside daemon)
    │
    ▼ convert + broadcast
WebSocket Server ──► TUI #1 (event stream)
              │
              └──► TUI #2 (same event stream)
              │
              └──► CLI    (headless mode)
```

The TUI's render loop becomes:
1. Read events from a local `VecDeque<Event>` (populated by WebSocket reader thread)
2. Apply events to local render state
3. Draw frame

No polling of core channels. No direct method calls into core.

---

## 3. Core Design Principles

### 3.1 Protocol-First Development

Before any code moves, we define the message contract in `agent-protocol`. Both the daemon and the TUI are implemented **against the protocol**, not against each other.

This enables:
- Parallel development of daemon and TUI
- Third-party client implementations (IDE plugins, web dashboards)
- Backward-compatible protocol evolution (versioning)

### 3.2 State Ownership Is Single and Explicit

Every piece of state has exactly one owner:

| State | Owner | Sync Mechanism |
|-------|-------|---------------|
| Transcript, agent status, backlog | Daemon | WebSocket event push |
| Scroll position, composer text | TUI | Local only |
| Provider session handles | Daemon | Internal |
| TUI overlay states | TUI | Local only |
| Shutdown snapshots | Daemon | Filesystem |

### 3.3 Events Over State Sync

Instead of sending the full `AppState` on every change, the daemon sends **events**:

```
Bad:  push full AppState (1MB) every 100ms
Good: push Event::AssistantDelta { content: "He" } (50 bytes)
```

TUI maintains a **local render buffer** that reconstructs the view from the event stream. On reconnect, the daemon sends a `SessionState` snapshot followed by replay of recent events.

### 3.4 Per-Workspace Isolation Is Non-Negotiable

Two workspaces on the same machine must be as isolated as two workspaces on different machines:

- Separate OS processes
- Separate memory spaces
- Separate event buses
- Separate filesystem storage
- No shared singletons, no global locks

This simplifies reasoning, debugging, and resource cleanup.

### 3.5 CLI and TUI Are Peers

After separation, both CLI and TUI are **thin clients** that speak the same protocol:

```
┌────────────┐      ┌────────────┐
│  CLI       │      │  TUI       │
│  (clap)    │      │  (ratatui) │
└─────┬──────┘      └─────┬──────┘
      │                   │
      └───────┬───────────┘
              │ agent-protocol
              ▼
         ┌────────────┐
         │  Daemon    │
         └────────────┘
```

The CLI no longer depends on the TUI crate. It depends only on `agent-protocol`.

---

## 4. Key Architectural Decisions

### 4.1 Why Per-Workspace Daemon Instead of Global Singleton?

| Global Singleton | Per-Workspace Daemon |
|-----------------|----------------------|
| Complex session routing logic | Natural isolation by directory |
| Risk of cross-workspace leaks | Bounded blast radius |
| Global state management overhead | Each daemon is self-contained |
| Harder to debug "which session?" | `ps` shows one process per project |
| Requires session ID in every message | Session is implicit by connection |

A global singleton makes sense for a server product (like Codex App Server in cloud mode). For a local dev tool, per-workspace daemons match how developers already think (one terminal tab = one project).

### 4.2 Why WebSocket Instead of gRPC / IPC / Unix Socket?

| Transport | Pros | Cons |
|-----------|------|------|
| Unix socket | Fast, simple | Platform-specific, no LAN future |
| gRPC | Strong typing | Heavy deps (tonic, prost), code generation |
| HTTP REST | Simple | Polling overhead, poor for streaming |
| **WebSocket** | **Bidirectional, streaming, mature Rust libs, LAN-ready** | **Slightly higher overhead than raw TCP** |

WebSocket gives us:
- Framing built-in (no JSONL parsing needed)
- Async bidirectional push (server → client approvals)
- A clear upgrade path to LAN (bind to `0.0.0.0` instead of `127.0.0.1`)
- Excellent Rust ecosystem (`tokio-tungstenite`)

The overhead is negligible for localhost communication.

### 4.3 Why Extract `agent-protocol` as a Separate Crate?

The protocol crate is the **linchpin** of the entire architecture:

```
        agent-tui ──► agent-protocol ◄── agent-daemon
        agent-cli ──► agent-protocol ◄── agent-daemon
```

Without it, we would have:
- Circular dependencies (daemon needs TUI types, TUI needs daemon types)
- Tight coupling between frontend and backend
- Impossibility of third-party clients

With it:
- Frontend and backend compile independently
- The protocol is documented in one place (the crate itself)
- Versioning is natural (bump `agent-protocol` version)

### 4.4 Why Keep `agent-core` as a Library?

Instead of dissolving `agent-core` entirely, we refactor it into a **domain library** consumed by `agent-daemon`:

- `agent-core` contains the business logic (agent lifecycle, task execution, backlog)
- `agent-daemon` contains the service layer (WebSocket, session management, broadcasting)
- TUI/CLI never touch `agent-core` directly

This preserves the substantial investment in core logic while allowing the service layer to evolve independently.

---

## 5. The Protocol Contract

### 5.1 Message Types

```rust
// agent-protocol/src/messages.rs

// Client → Daemon
pub enum ClientMsg {
    Initialize { client_id: String },
    SendInput { text: String },
    ApproveTool { request_id: String, allowed: bool },
    SetFocus { agent_id: AgentId },
    Heartbeat,
}

// Daemon → Client
pub enum ServerMsg {
    SessionState(SessionState),       // Full snapshot on connect
    Event(Event),                     // Incremental update
    ApprovalRequest(ApprovalRequest), // Server-initiated
    Error { message: String },
}

// Event stream (the core of the protocol)
pub enum Event {
    // Transcript events
    ItemStarted { item_id: String, kind: ItemKind, agent_id: AgentId },
    ItemDelta { item_id: String, delta: ItemDelta },
    ItemCompleted { item_id: String, item: TranscriptItem },

    // Agent lifecycle
    AgentSpawned { agent_id: AgentId, codename: String, role: AgentRole },
    AgentStopped { agent_id: AgentId },
    AgentStatusChanged { agent_id: AgentId, status: AgentSlotStatus },

    // Tool / decision
    ToolApprovalRequired { request_id: String, tool: String, preview: String },
    DecisionRequired { request_id: String, situation: String },

    // Mail
    MailReceived { to: AgentId, from: AgentId, subject: String },
}
```

### 5.2 State Snapshot

On connect, the daemon sends a `SessionState` containing everything the TUI needs to render:

```rust
pub struct SessionState {
    pub session_id: String,
    pub alias: String,
    pub app_state: AppStateSnapshot,        // transcript, input, status
    pub agents: Vec<AgentSnapshot>,         // pool state
    pub workplace: WorkplaceSnapshot,       // backlog, skills
    pub focused_agent_id: Option<AgentId>,
}
```

After the snapshot, only `Event` deltas flow. This is the same pattern used by Codex App Server (`Thread` → `Turn` → `Item`).

---

## 6. Refactoring Roadmap

### Phase 0: Foundation (Protocol & Structure)

**Goal**: Establish the contract and skeleton.

- Create `agent-protocol` crate with message types
- Create `agent-daemon` binary crate skeleton
- Add `tokio` + `tokio-tungstenite` to workspace dependencies
- Define `DaemonConfig` (port, alias, workplace) and JSON serialization
- No runtime logic moved yet

**Success criteria**: `cargo build -p agent-protocol -p agent-daemon` compiles.

### Phase 1: Daemon Skeleton (WebSocket Server)

**Goal**: A daemon that accepts WebSocket connections and echoes messages.

- Daemon binds to ephemeral port, writes `daemon.json` to workplace dir
- Auto-link: CLI/TUI resolves workplace and connects
- WebSocket server accepts connections, maintains client list
- Broadcast a `Heartbeat` to all clients every 30s
- Graceful shutdown on SIGTERM

**Success criteria**: Two `agent-cli` terminals in the same directory both connect to the same daemon and receive heartbeat echoes.

### Phase 2: State Extraction (Move Ownership to Daemon)

**Goal**: Daemon owns `RuntimeSession`; TUI receives snapshot.

- Move `RuntimeSession::bootstrap()` into daemon startup
- Move `AgentPool`, `EventAggregator`, `AgentMailbox` into daemon
- Daemon sends `SessionState` snapshot on WebSocket connect
- TUI renders from snapshot instead of bootstrapping session
- TUI no longer imports `RuntimeSession`, `AgentPool`, `EventAggregator`, `AgentMailbox`

**Success criteria**: TUI opens and shows correct transcript without directly touching `agent-core`.

### Phase 3: Event Streaming (Replace Polling with Push)

**Goal**: All provider events flow through daemon → WebSocket → TUI.

- Daemon converts `ProviderEvent` → `Event` and broadcasts
- TUI receives `Event` messages and updates local render state
- Remove `EventAggregator` from TUI entirely
- Remove all `ProviderEvent` match arms from `app_loop.rs`
- TUI event handler is a single `match event` block

**Success criteria**: Agent executes a command; TUI sees transcript update in real time via WebSocket.

### Phase 4: TUI Thinning (Remove All Direct Core Imports)

**Goal**: TUI depends only on `agent-protocol` and rendering crates.

- Replace all `agent_core::*` imports in `tui/src` with `agent_protocol::*`
- Replace direct method calls (`state.spawn_agent()`, etc.) with `ClientMsg` sends
- Move transcript entry builder logic into daemon
- TUI overlay state (scroll, composer) stays purely local

**Success criteria**: `tui/Cargo.toml` has no `agent-core` dependency.

### Phase 5: CLI Refactor (CLI Becomes a Peer Client)

**Goal**: CLI no longer depends on TUI crate.

- Remove `agent-tui` from `cli/Cargo.toml`
- CLI implements its own `agent-protocol` client
- `agent-cli daemon start/stop/status` commands
- `agent-cli run --prompt` sends `SendInput` and waits for `Finished` event
- `agent-cli` TUI mode spawns a subprocess or connects via WebSocket

**Success criteria**: `cargo test -p agent-cli` passes with no `agent-tui` dependency.

### Phase 6: Cleanup & Hardening

**Goal**: Remove legacy code paths and verify correctness.

- Delete embedded-mode fallback from TUI
- Remove re-export stubs from `core/src/lib.rs` that were created for TUI
- Add reconnect logic (TUI detects disconnect, auto-reconnects, replays events)
- Add event log persistence (`events.jsonl` per workplace)
- Performance test: 2 TUIs + 1 CLI simultaneously, no lag

**Success criteria**: `cargo test --workspace` passes. Manual test: 3 terminals, same workplace, real-time sync.

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Event ordering bugs | Medium | High | Event IDs + sequence numbers; TUI drops out-of-order deltas and requests snapshot |
| Reconnect state drift | Medium | High | Versioned snapshots + event replay buffer; TUI diffs and reconciles |
| Performance regression (WebSocket overhead) | Low | Medium | Benchmark early; localhost WebSocket is ~1-2ms overhead, negligible vs LLM latency |
| Testing complexity (need daemon for TUI tests) | High | Medium | Extract `agent-daemon` test harness; use in-memory WebSocket for unit tests |
| Breaking changes for existing users | Low | High | Maintain embedded-mode fallback during transition; deprecate after 2 releases |
| Port collision on busy machines | Low | Low | Ephemeral port allocation (bind to 0); OS handles it |
| Cross-platform WebSocket issues (Windows) | Medium | Low | Use `tokio-tungstenite` (cross-platform); test on Windows CI |

---

## 8. What This Document Is Not

This blueprint intentionally avoids:

- **Specific function signatures** — those belong in the protocol RFC and implementation specs
- **Database schema** — we use append-only JSONL, no database needed
- **Authentication / security model** — v1 is single-user localhost only
- **Multi-machine / cloud deployment** — out of scope for this phase
- **IDE plugin API** — the protocol is the API; plugins speak WebSocket

These topics will be addressed in follow-up documents after the core separation is complete.

---

## 9. Related Documents

| Document | Purpose |
|----------|---------|
| `tui-backend-separation-requirements.md` | Detailed requirements and acceptance criteria |
| `tui-core-interface-audit.md` | Complete catalog of current TUI→Core dependencies |
| `core-package-analysis.md` | Core module dependency graph and split history |
