# TUI-Backend Separation Requirements

> Status: Draft  
> Date: 2026-04-20  
> Target: Decouple `agent-tui` from `agent-core` runtime via a persistent background daemon

## 1. Background & Motivation

Currently `agent-tui` directly embeds `agent-core` runtime via `RuntimeSession`. The TUI owns the `AppState`, `AgentRuntime`, `AgentPool`, and event channels. This monolithic design prevents:

- **Multiple terminals** viewing the same agent session simultaneously
- **Headless operation** — running agents without a TUI attached
- **Session persistence** — closing the terminal kills the agent
- **Remote control** — connecting from another machine or IDE plugin

Codex CLI solves this with `codex app-server`: a background JSON-RPC server that powers the CLI TUI, VS Code extension, web app, and desktop app through a single API. We need a similar daemon-based architecture for `agile-agent`.

## 2. Goals

| ID | Goal | Priority |
|----|------|----------|
| G1 | A background daemon (`agent-daemon`) persists agent sessions independently of any TUI | Must |
| G2 | Multiple TUI clients can connect to the same daemon and view the same session state | Must |
| G3 | A client can disconnect and reconnect without losing agent progress | Must |
| G4 | The daemon supports multiple concurrent sessions (one per workplace) | Must |
| G5 | Human-decision / approval requests are broadcast to all connected clients | Must |
| G6 | Existing single-terminal `agent-cli` mode continues to work (embedded mode) | Should |
| G7 | Third-party clients (IDE plugins, web dashboards) can connect via the same protocol | Could |

## 3. Non-Goals

- Multi-user / authentication (single-user daemon, like `codex app-server`)
- Cross-machine networking (local IPC only for v1)
- Real-time collaborative editing (multiple users typing in the same input box)
- Process sandboxing or containerization

## 4. Reference Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Agent Daemon (agent-daemon)                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐ │
│  │ SessionMgr   │  │ AgentPool    │  │ EventBus     │  │ IPC Server      │ │
│  │              │  │              │  │              │  │ (Unix Socket)   │ │
│  │ - sessions   │  │ - agents     │  │ - broadcast  │  │                 │ │
│  │ - workplace  │  │ - slots      │  │ - subscribe  │  │ - JSON-RPC 2.0  │ │
│  │ - snapshots  │  │ - backlog    │  │ - persist    │  │ - bidirectional │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
         ▲                              ▲                              ▲
         │                              │                              │
    ┌────┴────┐                    ┌────┴────┐                    ┌────┴────┐
    │  TUI #1 │                    │  TUI #2 │                    │  CLI    │
    │ (ratatui)│                   │ (ratatui)│                   │ (headless)
    └─────────┘                    └─────────┘                    └─────────┘
```

### 4.1 Codex App Server Pattern (Primary Reference)

OpenAI's Codex App Server decouples the core engine from all client surfaces via:

- **Transport**: JSON-RPC 2.0 over stdio (default) or WebSocket (`--listen ws://`)
- **Primitives**: `Thread` (session) → `Turn` (work unit) → `Item` (atomic I/O)
- **Bidirectional**: Server can send approval requests; client responds with allow/deny
- **Streaming**: Events stream as JSONL deltas ("started" → delta → "completed")

We adopt the same JSON-RPC 2.0 / JSONL protocol over **Unix domain sockets** for local IPC.

### 4.2 Happy-CLI Daemon Pattern (Secondary Reference)

`happy-cli` uses a background daemon with:

- **Socket.io** for real-time sync
- **Queue IPC server** so the first process owns the agent connection, subsequent processes become queue clients
- **Session persistence and resumption**

We adopt the **queue-owner / queue-client** model for session affinity.

## 5. Architecture Requirements

### 5.1 Daemon Process (`agent-daemon`)

A new crate `agent-daemon` (or a binary within `cli/`) that:

1. **Spawns on demand**: `agent-cli daemon start` launches the daemon in the background; `agent-cli daemon stop` kills it.
2. **Singleton per user**: Only one daemon process per user (`~/.agile-agent/daemon.sock`).
3. **Session ownership**: The daemon is the **single source of truth** for all `RuntimeSession`, `AgentPool`, and `SharedWorkplaceState` instances.
4. **Event persistence**: Transcript events are persisted to disk so clients can replay history on reconnect.
5. **Graceful shutdown**: On SIGTERM, the daemon saves shutdown snapshots and stops all agents cleanly.

### 5.2 IPC Protocol

**Transport**: Unix domain socket at `~/.agile-agent/daemon.sock` (Windows: named pipe).

**Framing**: Newline-delimited JSON (JSONL), each line a valid JSON-RPC 2.0 message.

**Message categories**:

| Direction | Message Type | Example |
|-----------|-------------|---------|
| Client → Daemon | `initialize` | `{ "jsonrpc":"2.0", "id":1, "method":"initialize", "params":{ "workplace":"/path/to/project" } }` |
| Client → Daemon | `send_input` | User message, slash command |
| Client → Daemon | `approve_tool` | Allow/deny a pending tool call |
| Client → Daemon | `subscribe` | Subscribe to agent events |
| Daemon → Client | `event` | Transcript delta, status change, tool output |
| Daemon → Client | `request_approval` | Server-initiated approval request |
| Daemon → Client | `session_state` | Full state snapshot on connect |

**Event streaming model** (mirroring Codex `Item` lifecycle):

```json
{ "jsonrpc": "2.0", "method": "event", "params": {
    "type": "transcript_item",
    "item": {
        "id": "item-001",
        "kind": "assistant_message",
        "status": "started"
    }
}}
{ "jsonrpc": "2.0", "method": "event", "params": {
    "type": "transcript_item_delta",
    "item_id": "item-001",
    "delta": { "content": "Let me check" }
}}
{ "jsonrpc": "2.0", "method": "event", "params": {
    "type": "transcript_item",
    "item": {
        "id": "item-001",
        "kind": "assistant_message",
        "status": "completed",
        "content": "Let me check the file..."
    }
}}
```

### 5.3 Session Manager

The daemon maintains a `SessionManager` that:

1. Maps `workplace_id` → `RuntimeSession` (one session per workplace).
2. Auto-creates a session when a client initializes for a new workplace.
3. Keeps sessions alive for **N minutes** after the last client disconnects (configurable, default 30 min).
4. Saves / restores sessions via `ShutdownSnapshot` on daemon shutdown / startup.
5. Supports named sessions (e.g., `session: "api-refactor"`) so multiple independent sessions can run in the same workplace.

### 5.4 TUI Client (`agent-tui` as a thin client)

The TUI becomes a **render-only** client:

1. On startup, connects to `~/.agile-agent/daemon.sock`.
2. Sends `initialize` with the current working directory.
3. Receives `session_state` (full transcript + agent status) and renders it.
4. User input is sent via `send_input`; daemon broadcasts resulting events to all clients.
5. On disconnect (Ctrl-C), the TUI exits cleanly; the daemon continues running agents.
6. On reconnect, the TUI replays missed events from the event log.

### 5.5 CLI Client (`agent-cli` in headless mode)

The CLI gains a `daemon` subcommand and a headless execution mode:

```bash
# Daemon lifecycle
agent-cli daemon start      # Start background daemon
agent-cli daemon stop       # Stop daemon
agent-cli daemon status     # Show running sessions

# Headless execution (connects to daemon, no TUI)
agent-cli run --prompt "refactor auth module"

# Existing TUI mode still works (auto-starts daemon if not running)
agent-cli
```

### 5.6 Event Bus & Broadcasting

A pub/sub `EventBus` inside the daemon:

1. **Topics per session**: `events/<workplace_id>`.
2. **Broadcast semantics**: Every event is sent to **all connected clients** of that session.
3. **Approval routing**: When an agent needs approval, the daemon sends `request_approval` to **all clients**; the first client response wins.
4. **Backpressure**: If a client lags behind, the daemon drops old deltas and sends a full `session_state` snapshot instead.

## 6. Data Flow Scenarios

### 6.1 Two TUIs, Same Session

```
TUI #1 ──connect──┐
                   ├──► Daemon ──► AgentPool ──► Provider Threads
TUI #2 ──connect──┘      ▲
                           └──── broadcast events ────┘
```

1. TUI #1 and TUI #2 both `initialize` for `/home/user/project`.
2. User types in TUI #1; `send_input` goes to daemon.
3. Agent processes input and generates events.
4. Daemon broadcasts events to both TUI #1 and TUI #2.
5. Both TUIs render the same transcript in real time.

### 6.2 Disconnect and Reconnect

1. TUI #1 disconnects while agent is "Responding".
2. Daemon continues the agent turn to completion.
3. TUI #1 reconnects 5 minutes later.
4. Daemon sends `session_state` snapshot + replays events from the log.
5. TUI #1 shows the full conversation including everything that happened while disconnected.

### 6.3 Approval from Any Client

1. Agent executes `rm -rf important_dir`.
2. Daemon broadcasts `request_approval` to all connected clients.
3. User sees the approval prompt in **both** TUI #1 and TUI #2.
4. User clicks "Allow" in TUI #2.
5. Daemon receives `approve_tool` from TUI #2, cancels the pending request in TUI #1, and proceeds.

## 7. Crate Restructuring Plan

### New Crates

| Crate | Responsibility |
|-------|---------------|
| `agent-daemon` (new binary crate) | Background daemon, session manager, IPC server, event bus |
| `agent-protocol` (new lib crate) | JSON-RPC types, message schemas, event definitions shared between daemon and clients |

### Modified Crates

| Crate | Changes |
|-------|---------|
| `agent-tui` | Strip out `RuntimeSession` ownership; become a thin IPC client that renders `SessionState` snapshots and `Event` streams |
| `agent-cli` | Add `daemon` subcommands; `run` mode connects to daemon instead of embedding core directly |
| `agent-core` | Expose `RuntimeSession` / `AgentPool` as a library API consumed by `agent-daemon`; retain snapshot / persistence logic |

### Deleted / Moved

- `tui/src/app_loop.rs` session bootstrap logic → `agent-daemon/src/session_manager.rs`
- `core/src/event_aggregator.rs` channel polling → `agent-daemon/src/event_bus.rs` (with IPC broadcasting)

## 8. Migration Path

| Phase | Work | Risk |
|-------|------|------|
| 1 | Extract `agent-protocol` with JSON-RPC types and event schemas | Low |
| 2 | Build `agent-daemon` binary that can host a single `RuntimeSession` and serve it over Unix socket | Medium |
| 3 | Refactor `agent-tui` to connect to daemon instead of embedding core; keep embedded mode as fallback | Medium |
| 4 | Add event persistence / replay so reconnect works seamlessly | Medium |
| 5 | Add multi-session support (`SessionManager` with workplace → session map) | Medium |
| 6 | Add multi-client broadcast and approval routing | Medium |
| 7 | Deprecate embedded mode; all CLI commands go through daemon | Low |

## 9. Open Questions

1. **Should the daemon be a separate crate or a `cli` binary feature?**
   - Recommendation: Separate `agent-daemon` crate for clean dependency boundaries.
2. **IPC on Windows?**
   - Phase 1: Unix sockets only. Phase 2: Named pipes or TCP localhost.
3. **Event log storage format?**
   - Append-only JSONL file per session: `~/.agile-agent/sessions/<workplace_id>/events.jsonl`
4. **How to handle TUI-specific state (scroll position, composer text)?**
   - TUI-specific state stays in the TUI process. Only shared agent state lives in the daemon.
5. **Security: any user on the machine can connect to the socket?**
   - Yes, same as Codex App Server. File permissions on the socket can restrict to the owner.

## 10. Acceptance Criteria

- [ ] `agent-cli daemon start` launches a background process
- [ ] `agent-cli` (TUI mode) auto-connects to the daemon; if daemon is not running, it auto-starts it
- [ ] Two `agent-cli` terminals in the same workplace show identical transcripts in real time
- [ ] Closing one terminal does not stop the agent; reopening restores the full state
- [ ] Tool approval prompts appear in all connected terminals; approving in one resolves all
- [ ] `agent-cli run --prompt "..."` executes headlessly via the daemon
- [ ] `cargo test --workspace` passes after each phase

## 11. Related Documents

- [Codex App Server Architecture](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) — JSON-RPC 2.0 protocol reference
- [Agor Architecture](https://agor.live/guide/architecture) — Local-first daemon with executor isolation
- [ACP-CLI Sessions](https://lib.rs/crates/acp-cli) — Queue-owner / queue-client IPC model
- `docs/git-flow-preparation.md` — Existing task preparation pipeline
