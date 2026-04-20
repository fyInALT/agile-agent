# TUI-Backend Separation Requirements

> Status: Draft
> Date: 2026-04-20
> Target: Decouple `agent-tui` from `agent-core` runtime via per-workspace background daemons communicating over WebSocket

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
| G1 | A background daemon per workspace persists agent sessions independently of any TUI | Must |
| G2 | Multiple TUI clients can connect to the same daemon and view the same session state | Must |
| G3 | A client can disconnect and reconnect without losing agent progress | Must |
| G4 | Multiple workspaces can run simultaneously on the same machine, fully isolated | Must |
| G5 | Human-decision / approval requests are broadcast to all connected clients | Must |
| G6 | Existing single-terminal `agent-cli` mode continues to work (embedded mode) | Should |
| G7 | Third-party clients (IDE plugins, web dashboards) can connect via the same protocol | Could |
| G8 | Session alias makes it easy to connect without memorizing UUIDs | Should |

## 3. Non-Goals

- Multi-user / authentication (single-user daemon, like `codex app-server`)
- Cross-machine networking over the internet (local WebSocket only for v1; LAN considered for v2)
- Real-time collaborative editing (multiple users typing in the same input box)
- Process sandboxing or containerization
- Shared state across workspaces (workspaces are strictly isolated)

## 4. Reference Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Per-Workspace Daemon                               │
│  (one independent process per workplace directory)                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────────┐ │
│  │ Session      │  │ AgentPool    │  │ EventBus     │  │ WebSocket       │ │
│  │ Manager      │  │              │  │              │  │ Server          │ │
│  │              │  │ - agents     │  │ - broadcast  │  │ (localhost)     │ │
│  │ - session    │  │ - slots      │  │ - subscribe  │  │                 │ │
│  │ - workplace  │  │ - backlog    │  │ - persist    │  │ - bidirectional │ │
│  │ - snapshots  │  │              │  │              │  │ - JSON messages │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
         ▲                              ▲                              ▲
         │                              │                              │
    ┌────┴────┐                    ┌────┴────┐                    ┌────┴────┐
    │  TUI #1 │                    │  TUI #2 │                    │  CLI    │
    │ (ratatui)│                   │ (ratatui)│                   │ (headless)
    └─────────┘                    └─────────┘                    └─────────┘

Workspace A Daemon (ws://localhost:50001)  ←── completely isolated ──→  Workspace B Daemon (ws://localhost:50002)
```

### 4.1 Codex App Server Pattern (Primary Reference)

OpenAI's Codex App Server decouples the core engine from all client surfaces via:

- **Transport**: JSON-RPC 2.0 over stdio (default) or WebSocket (`--listen ws://`)
- **Primitives**: `Thread` (session) → `Turn` (work unit) → `Item` (atomic I/O)
- **Bidirectional**: Server can send approval requests; client responds with allow/deny
- **Streaming**: Events stream as JSONL deltas ("started" → delta → "completed")

We adopt the same **bidirectional messaging** model but use **WebSocket over TCP localhost** for flexibility.

### 4.2 ACP-CLI Queue Pattern (Secondary Reference)

`acp-cli` uses a queue-owner model:

- The first process for a session becomes the **queue owner** (holds the agent connection)
- Subsequent processes connect as **queue clients** via IPC
- Sessions auto-resume by matching `(agent, git_root, session_name)`

We adopt the **per-workspace daemon** model: each workspace gets its own daemon process with its own WebSocket port.

## 5. Architecture Requirements

### 5.1 Daemon Process Model — One Per Workspace

A daemon is **not** a global singleton. Instead:

1. **One daemon per workspace**: Each unique working directory gets its own independent `agent-daemon` process.
2. **Full isolation**: Daemons for different workspaces share **no memory, no state, no event bus**. They are completely separate OS processes.
3. **Auto-spawn on demand**: `agent-cli` (or `agent-tui`) checks if a daemon exists for the current workplace; if not, it spawns one.
4. **Session affinity**: All clients connecting to `ws://localhost:<port>` of a given daemon see the same session.
5. **Graceful shutdown**: On SIGTERM, the daemon saves shutdown snapshots and stops all agents cleanly.

**Why per-workspace instead of singleton?**

- Simpler mental model: one directory = one daemon = one session
- No cross-workspace leak risk
- Easier resource accounting and cleanup
- Matches how developers work (each project is independent)

### 5.2 Session Registry & Discovery

Since multiple daemons may run on the same machine, we need a lightweight registry:

**Registry file**: `~/.agile-agent/registry.json`

```json
{
  "version": 1,
  "sessions": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "alias": "api-server",
      "workplace": "/home/user/projects/api-server",
      "pid": 12345,
      "websocket_url": "ws://localhost:50001",
      "created_at": "2026-04-20T10:00:00Z"
    },
    {
      "id": "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
      "alias": "frontend",
      "workplace": "/home/user/projects/frontend",
      "pid": 12346,
      "websocket_url": "ws://localhost:50002",
      "created_at": "2026-04-20T11:30:00Z"
    }
  ]
}
```

**Registry operations**:

| Operation | Trigger |
|-----------|---------|
| Register | Daemon starts successfully |
| Update heartbeat | Daemon writes heartbeat timestamp every 30s |
| Prune stale | `agent-cli daemon list` prunes entries whose heartbeat is older than 2 min |
| Unregister | Daemon shuts down gracefully |

**Alias rules**:

- Alias is optional; if not provided, defaults to the directory basename (e.g., `api-server`)
- Alias must be unique across all running sessions on the machine
- Alias can be renamed via `agent-cli daemon rename <old> <new>`
- Connection can use either UUID or alias: `agent-cli --session api-server`

### 5.3 IPC Protocol — WebSocket

**Transport**: WebSocket over TCP localhost (`ws://localhost:<port>`).

**Port allocation**: Ephemeral port (0) assigned by OS; daemon writes the actual port to the registry. No hardcoded ports.

**Message format**: JSON objects (one per WebSocket text frame). Not JSON-RPC 2.0 — a simpler custom protocol is sufficient and avoids framing overhead.

**Message envelope**:

```json
{
  "msg_type": "event",
  "payload": { ... }
}
```

**Client → Daemon messages**:

| `msg_type` | Purpose | Example payload |
|-----------|---------|-----------------|
| `initialize` | Connect and request full state | `{ "workplace": "/path", "client_id": "tui-1" }` |
| `send_input` | Send user message or slash command | `{ "text": "/approve" }` |
| `approve_tool` | Respond to approval request | `{ "request_id": "req-1", "allowed": true }` |
| `heartbeat` | Keep connection alive | `{}` |
| `set_focus` | Change focused agent (multi-agent) | `{ "agent_id": "agent-1" }` |

**Daemon → Client messages**:

| `msg_type` | Purpose | Example payload |
|-----------|---------|-----------------|
| `session_state` | Full snapshot on connect | `{ "app_state": { ... }, "agents": [ ... ] }` |
| `event` | Incremental event (broadcast) | `{ "type": "transcript_delta", "agent_id": "...", "delta": "..." }` |
| `request_approval` | Server-initiated approval | `{ "request_id": "req-1", "tool": "exec", "command": "rm -rf" }` |
| `error` | Protocol or runtime error | `{ "message": "agent crashed" }` |

**Event streaming model** (mirroring Codex `Item` lifecycle):

```json
{ "msg_type": "event", "payload": { "type": "item_started", "item_id": "item-001", "kind": "assistant_message" }}
{ "msg_type": "event", "payload": { "type": "item_delta", "item_id": "item-001", "delta": { "content": "Let me check" }}}
{ "msg_type": "event", "payload": { "type": "item_completed", "item_id": "item-001", "content": "Let me check the file..." }}
```

### 5.4 TUI Client (`agent-tui` as a thin client)

The TUI becomes a **render-only** client:

1. On startup, connects to the daemon's WebSocket URL (from registry, by alias or UUID).
2. Sends `initialize`; receives `session_state` (full snapshot) and renders it.
3. User input is sent via `send_input`; daemon broadcasts resulting events to all clients.
4. On disconnect (Ctrl-C), the TUI exits cleanly; the daemon continues running agents.
5. On reconnect, the TUI replays missed events from the event log.

### 5.5 CLI Client (`agent-cli` modes)

The CLI gains a `daemon` subcommand and a headless execution mode:

```bash
# Daemon lifecycle
agent-cli daemon start [--alias api-server]     # Start daemon for current workplace
agent-cli daemon stop [--alias api-server]       # Stop daemon
agent-cli daemon list                            # List all running sessions
agent-cli daemon rename <old> <new>              # Rename session alias

# Connect to an existing session by alias
agent-cli --session api-server

# Headless execution (connects to daemon, no TUI)
agent-cli run --prompt "refactor auth module"

# Existing TUI mode still works (auto-starts daemon if not running)
agent-cli
```

### 5.6 Event Bus & Broadcasting

A pub/sub `EventBus` inside **each** daemon:

1. **Per-daemon scope**: Events never cross workspace boundaries.
2. **Broadcast semantics**: Every event is sent to **all connected clients** of that daemon.
3. **Approval routing**: When an agent needs approval, the daemon sends `request_approval` to **all clients**; the first client response wins.
4. **Backpressure**: If a client lags behind, the daemon drops old deltas and sends a full `session_state` snapshot instead.

### 5.7 Multi-Workspace Isolation

**Strict isolation guarantees**:

| Resource | Isolation Level |
|----------|----------------|
| AgentPool | Separate per workspace |
| Backlog | Separate per workspace |
| Skills | Separate per workspace (discovered from each cwd) |
| Transcript | Separate per workspace |
| EventBus | Separate per workspace |
| WebSocket port | Separate per workspace |
| Shutdown snapshot | Separate per workspace |

**No shared state**: There is no global singleton. Two workspaces running on the same machine are identical to two workspaces on different machines.

## 6. Data Flow Scenarios

### 6.1 Two TUIs, Same Workspace

```
TUI #1 ──WebSocket──┐
                     ├──► Workspace A Daemon ──► AgentPool ──► Provider Threads
TUI #2 ──WebSocket──┘      ▲
                             └──── broadcast events ────┘
```

1. TUI #1 and TUI #2 both connect to `ws://localhost:50001` (Workspace A).
2. User types in TUI #1; `send_input` goes to daemon.
3. Agent processes input and generates events.
4. Daemon broadcasts events to both TUI #1 and TUI #2.
5. Both TUIs render the same transcript in real time.

### 6.2 Two Workspaces, Same Machine

```
TUI #3 ──WebSocket──► Workspace B Daemon (ws://localhost:50002)
TUI #4 ──WebSocket──► Workspace C Daemon (ws://localhost:50003)
```

1. Workspace B and C are completely independent OS processes.
2. Events in Workspace B never reach Workspace C.
3. Stopping Workspace B's daemon does not affect Workspace C.

### 6.3 Disconnect and Reconnect

1. TUI #1 disconnects while agent is "Responding".
2. Daemon continues the agent turn to completion.
3. TUI #1 reconnects 5 minutes later via the same WebSocket URL.
4. Daemon sends `session_state` snapshot + replays events from the log.
5. TUI #1 shows the full conversation including everything that happened while disconnected.

### 6.4 Approval from Any Client

1. Agent executes `rm -rf important_dir`.
2. Daemon broadcasts `request_approval` to all connected clients.
3. User sees the approval prompt in **both** TUI #1 and TUI #2.
4. User clicks "Allow" in TUI #2.
5. Daemon receives `approve_tool` from TUI #2, cancels the pending request in TUI #1, and proceeds.

### 6.5 Connect by Alias

```bash
# In terminal 1: start daemon with alias
$ cd ~/projects/api-server
$ agent-cli daemon start --alias api-server
Session "api-server" started at ws://localhost:50001

# In terminal 2: connect by alias
$ agent-cli --session api-server
[TUI connects to ws://localhost:50001]

# In terminal 3: also connect by alias
$ agent-cli --session api-server
[TUI also connects to ws://localhost:50001, both show same state]
```

## 7. Crate Restructuring Plan

### New Crates

| Crate | Responsibility |
|-------|---------------|
| `agent-daemon` (new binary crate) | Per-workspace background daemon: session manager, WebSocket server, event bus |
| `agent-protocol` (new lib crate) | Shared message types (`MsgType`, `Event`, `SessionState`) used by daemon and clients |

### Modified Crates

| Crate | Changes |
|-------|---------|
| `agent-tui` | Strip out `RuntimeSession` ownership; become a thin WebSocket client that renders `SessionState` snapshots and `Event` streams |
| `agent-cli` | Add `daemon` subcommands; `run` mode connects to daemon instead of embedding core directly; `--session <alias>` flag |
| `agent-core` | Expose `RuntimeSession` / `AgentPool` as a library API consumed by `agent-daemon`; retain snapshot / persistence logic |

### Deleted / Moved

- `tui/src/app_loop.rs` session bootstrap logic → `agent-daemon/src/session_manager.rs`
- `core/src/event_aggregator.rs` channel polling → `agent-daemon/src/event_bus.rs` (with WebSocket broadcasting)

## 8. Migration Path

| Phase | Work | Risk |
|-------|------|------|
| 1 | Extract `agent-protocol` with WebSocket message types and event schemas | Low |
| 2 | Build `agent-daemon` binary that hosts a single `RuntimeSession` and serves it over WebSocket | Medium |
| 3 | Refactor `agent-tui` to connect to daemon via WebSocket; keep embedded mode as fallback | Medium |
| 4 | Add event persistence / replay so reconnect works seamlessly | Medium |
| 5 | Implement session registry (`~/.agile-agent/registry.json`) and alias support | Medium |
| 6 | Add multi-client broadcast and approval routing | Medium |
| 7 | Add `agent-cli daemon start/stop/list/rename` commands | Low |
| 8 | Deprecate embedded mode; all CLI commands go through daemon | Low |

## 9. Open Questions

1. **WebSocket library choice?**
   - Recommendation: `tokio-tungstenite` (async, mature, widely used in Rust ecosystem).
2. **Should the daemon use tokio or std threads?**
   - Recommendation: `tokio` runtime for async WebSocket handling + concurrent client management.
3. **Event log storage format?**
   - Append-only JSONL file per session: `~/.agile-agent/sessions/<session_id>/events.jsonl`
4. **How to handle TUI-specific state (scroll position, composer text)?**
   - TUI-specific state stays in the TUI process. Only shared agent state lives in the daemon.
5. **Security: any process on the machine can connect to localhost WebSocket?**
   - Yes, same as Codex App Server. localhost is inherently local-only. Token-based auth can be added in v2.
6. **Port collision?**
   - Bind to port 0 (OS-assigned ephemeral port) and read back the actual port. No hardcoded ports.
7. **Alias uniqueness across machine restarts?**
   - Registry is pruned on read; stale entries (no heartbeat) are ignored. Alias uniqueness is checked only among running sessions.

## 10. Acceptance Criteria

- [ ] `agent-cli daemon start --alias my-project` launches a background WebSocket server
- [ ] `agent-cli daemon list` shows all running sessions with alias, port, and workplace
- [ ] `agent-cli --session my-project` auto-connects to the correct daemon
- [ ] Two `agent-cli` terminals with `--session my-project` show identical transcripts in real time
- [ ] Closing one terminal does not stop the agent; reopening restores the full state
- [ ] Tool approval prompts appear in all connected terminals; approving in one resolves all
- [ ] `agent-cli run --prompt "..."` executes headlessly via the daemon
- [ ] Two different workplace directories can each run their own daemon simultaneously without interference
- [ ] `cargo test --workspace` passes after each phase

## 11. Related Documents

- [Codex App Server Architecture](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md) — JSON-RPC 2.0 protocol reference
- [Agor Architecture](https://agor.live/guide/architecture) — Local-first daemon with executor isolation
- [ACP-CLI Sessions](https://lib.rs/crates/acp-cli) — Queue-owner / queue-client IPC model
- `docs/git-flow-preparation.md` — Existing task preparation pipeline
