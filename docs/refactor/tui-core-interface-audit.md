# TUI-Core Interface Audit

> Status: Draft  
> Date: 2026-04-20  
> Purpose: Catalog every TUI → Core call and data dependency to design a minimal interface for backend separation

## 1. Executive Summary

`agent-tui` currently has **deep, bidirectional coupling** with `agent-core`. The TUI does not merely "display" core state — it **owns** `RuntimeSession`, `AgentPool`, `EventAggregator`, and `AgentMailbox`, and directly invokes dozens of core methods inside the render loop.

This audit catalogs every dependency so we can:
1. Extract a **minimal TUI-facing API surface** from core
2. Replace direct method calls with **WebSocket messages**
3. Move all state ownership into the daemon

---

## 2. TUI State Ownership (The Core Problem)

`TuiState` (in `tui/src/ui_state.rs`) directly embeds four core runtime objects:

```rust
pub struct TuiState {
    pub session: RuntimeSession,              // owns AppState + AgentRuntime + workplace
    pub agent_pool: Option<AgentPool>,        // owns all agent slots
    pub event_aggregator: EventAggregator,    // owns all provider channels
    pub mailbox: AgentMailbox,                // owns cross-agent mail
    // ... 20+ TUI-only fields (scroll, composer, overlays)
}
```

> **Critical insight**: These four fields represent the **entire runtime state** of the agent system. The TUI is not a view layer — it *is* the runtime controller.

### 2.1 Ownership Flow

```
Provider Threads (core)
    │ mpsc::Sender<ProviderEvent>
    ▼
EventAggregator (owned by TuiState)
    │ try_recv() polled every frame
    ▼
TUI Main Loop (app_loop.rs)
    │ mutates
    ▼
AppState + AgentPool + Mailbox (all inside TuiState)
```

---

## 3. Import Dependency Map

### 3.1 Top-Level Import Frequency (TUI → Core)

| Module | Refs | Nature |
|--------|------|--------|
| `app` | 26 | State + rendering data (`AppState`, `AppStatus`, `LoopPhase`, `TranscriptEntry`) |
| `agent_runtime` | 12 | IDs and meta (`AgentId`, `AgentMeta`, `AgentStatus`, `ProviderSessionId`) |
| `ProviderKind` | 11 | Provider selection |
| `shutdown_snapshot` | 9 | Session persistence |
| `agent_mail` | 8 | Cross-agent communication |
| `runtime_session` | 8 | Session bootstrap and access |
| `workplace_store` | 7 | Workplace resolution |
| `logging` | 6 | Debug/event logging |
| `provider_profile` | 5 | Profile selection overlay |
| `agent_pool` | 4 | Multi-agent coordination |
| `agent_slot` | 4 | Slot status and tasks |
| `agent_role` | 4 | Role display |
| `launch_config` | 4 | Agent spawn configuration |
| `command_bus` | 4 | Slash command parsing |
| `commands` | 3 | Local command handling |
| `task_engine` | 3 | Turn execution |
| `ProviderEvent` | 3 | Event stream matching |
| `event_aggregator` | 1 | Event polling |
| `shared_state` | 2 | Workplace state access |
| `global_config` | 1 | Config store |
| `skills` | 2 | Skill registry |
| `backlog` | 2 | Backlog state |
| `decision_mail` | 2 | Decision requests |

### 3.2 CLI → Core Import Frequency

CLI has **far fewer** dependencies (18 refs vs 140+ in TUI):

| Module | Refs |
|--------|------|
| `agent_runtime` | 3 |
| `logging` | 3 |
| `loop_runner` | 2 |
| `multi_agent_session` | 1 |
| `agent_store` | 1 |
| `session_store` | 1 |
| `backlog_store` | 1 |
| `skills` | 1 |
| `probe` | 1 |
| `workplace_store` | 1 |
| `runtime_mode` | 1 |
| `ProviderKind` | 1 |
| `shutdown_snapshot` | 1 |
| `backlog` | 1 |
| `event_aggregator` | 1 |
| `global_config` | 1 |
| `provider_profile` | 1 |

> CLI is already closer to a "thin client" because it mostly delegates to `loop_runner` and `MultiAgentSession`.

---

## 4. Method Call Taxonomy

### 4.1 Session & Lifecycle (TuiState → RuntimeSession)

| Method | Caller | Purpose |
|--------|--------|---------|
| `RuntimeSession::bootstrap()` | `app_loop::run()` | Create or restore session from cwd |
| `session.quick_shutdown()` | `app_loop` (on quit) | Persist and stop agents |
| `session.was_interrupted()` | `app_loop` | Check if previous session was interrupted |
| `session.app` / `session.app_mut` | `ui_state` | Access AppState |
| `session.workplace` / `session.workplace_mut` | `ui_state` | Access SharedWorkplaceState |

**Daemon equivalent**: `initialize` message returns full `SessionState` snapshot.

### 4.2 Agent Pool Management (TuiState → AgentPool)

| Method | Caller | Purpose |
|--------|--------|---------|
| `pool.spawn_agent()` | `spawn_agent()` | Create new agent slot |
| `pool.spawn_agent_with_profile()` | `spawn_agent_with_profile()` | Spawn with provider profile |
| `pool.spawn_agent_with_launch_config()` | `spawn_agent_with_launch_config()` | Spawn with launch bundle |
| `pool.stop_agent()` | `stop_focused_agent()` | Stop focused agent |
| `pool.pause_agent()` | `pause_focused_agent()` | Pause agent |
| `pool.resume_agent()` | `resume_focused_agent()` | Resume agent |
| `pool.focus_agent()` | `focus_agent()` | Change focused agent |
| `pool.focus_next()` | `focus_next_agent()` | Focus next |
| `pool.focus_previous()` | `focus_previous_agent()` | Focus previous |
| `pool.statuses()` | `agent_statuses()` | Get all agent statuses |
| `pool.active_count()` | (implied) | Know if multi-agent |

**Daemon equivalent**: `send_input` with `/spawn`, `/stop`, `/focus` commands; daemon broadcasts `event` with updated `AgentStatusSnapshot`.

### 4.3 Event Polling (TuiState → EventAggregator)

| Method | Caller | Purpose |
|--------|--------|---------|
| `event_aggregator.poll_all()` | `poll_agent_events()` | Non-blocking poll all channels |
| `event_aggregator.poll_with_timeout()` | `poll_agent_events_with_timeout()` | Blocking poll with timeout |
| `event_aggregator.register()` | `register_agent_channel()` | Add new agent channel |
| `event_aggregator.unregister()` | `unregister_agent_channel()` | Remove agent channel |
| `event_aggregator.agent_count()` | `agent_channel_count()` | Count active channels |

**Daemon equivalent**: WebSocket push. TUI never polls — daemon pushes `event` messages.

### 4.4 Mailbox (TuiState → AgentMailbox)

| Method | Caller | Purpose |
|--------|--------|---------|
| `mailbox.send_mail()` | (scattered) | Send cross-agent message |
| `mailbox.inbox_for()` | `focused_unread_mail_for_prompt()` | Read inbox for display |
| `mailbox.unread_count()` | `focused_unread_mail_count()` | Badge count |
| `mailbox.action_required_count()` | `focused_action_required_count()` | Action badge |
| `mailbox.mark_all_read()` | `mark_focused_mail_read()` | Clear unread |
| `mailbox.process_pending()` | (scattered) | Process pending mail |
| `mailbox.pending_count()` | (scattered) | Count pending |

**Daemon equivalent**: `send_input` with `@agent` mention; daemon routes mail and pushes `event` with `MailReceived`.

### 4.5 Transcript Mutation (TuiState → AppState)

These are the **most granular** and **most frequent** calls. Every `ProviderEvent` delta triggers a TuiState method that mutates the transcript:

| Method | Trigger Event |
|--------|--------------|
| `append_active_assistant_chunk()` | `ProviderEvent::AssistantChunk` |
| `append_active_thinking_chunk()` | `ProviderEvent::ThinkingChunk` |
| `append_status_to_agent_transcript()` | `ProviderEvent::Status` |
| `push_active_exec_started()` | `ProviderEvent::ExecCommandStarted` |
| `append_active_exec_output()` | `ProviderEvent::ExecCommandOutputDelta` |
| `finish_active_exec()` | `ProviderEvent::ExecCommandFinished` |
| `push_active_patch_apply_started()` | `ProviderEvent::PatchApplyStarted` |
| `finish_active_patch_apply()` | `ProviderEvent::PatchApplyFinished` |
| `push_active_web_search_started()` | `ProviderEvent::WebSearchStarted` |
| `finish_active_web_search()` | `ProviderEvent::WebSearchFinished` |
| `push_active_mcp_tool_call_started()` | `ProviderEvent::McpToolCallStarted` |
| `finish_active_mcp_tool_call()` | `ProviderEvent::McpToolCallFinished` |
| `push_active_generic_tool_call_started()` | `ProviderEvent::GenericToolCallStarted` |
| `finish_active_generic_tool_call()` | `ProviderEvent::GenericToolCallFinished` |
| `flush_active_entries_to_transcript()` | End of turn |
| `finalize_active_entries_after_failure()` | `ProviderEvent::Error` |

**Critical observation**: The TUI maintains **"active" partially-built transcript entries** (e.g., an `ExecCommand` that has started but not finished) and flushes them to the permanent transcript when complete. This is complex stateful logic that currently lives in `TuiState`.

**Daemon equivalent**: The daemon owns the transcript building. TUI receives only completed or delta `event` messages and appends them to a local render buffer.

### 4.6 Snapshot / Persistence

| Method | Purpose |
|--------|---------|
| `create_shutdown_snapshot()` | Save full state on quit |
| `create_resume_snapshot()` | Save TUI-specific state |
| `restore_from_resume_snapshot()` | Restore TUI state on reconnect |
| `persist_if_changed()` | Periodic save to disk |

**Daemon equivalent**: Daemon handles all persistence. TUI only needs to restore its local view state (scroll position, composer text) on reconnect.

### 4.7 Overlay State (TUI-only, no core deps)

| Method | Purpose |
|--------|---------|
| `open_provider_overlay()` / `close_provider_overlay()` | Provider selection UI |
| `open_profile_selection_overlay()` / `close_profile_selection_overlay()` | Profile picker |
| `open_launch_config_overlay()` / `close_launch_config_overlay()` | Launch config UI |
| `open_transcript_overlay()` / `close_transcript_overlay()` | Transcript overlay |
| `open_stop_confirmation()` / `is_confirmation_overlay_open()` | Confirmation dialogs |

**No change needed**: These are pure TUI state and remain in `TuiState`.

### 4.8 Provider Event Matching (app_loop.rs)

`app_loop.rs` matches **all 20+ `ProviderEvent` variants** across three different sites:

1. **Main event handler** (~L983): Mutates TuiState based on event type
2. **Overview log builder** (~L1815): Converts events to log messages
3. **Transcript render** (~L1988): Converts events to renderable cells

| ProviderEvent Variant | Handler Count |
|----------------------|---------------|
| `Status` | 3 |
| `AssistantChunk` | 2 |
| `ThinkingChunk` | 2 |
| `ExecCommandStarted` | 3 |
| `ExecCommandFinished` | 2 |
| `ExecCommandOutputDelta` | 2 |
| `GenericToolCallStarted` | 2 |
| `GenericToolCallFinished` | 2 |
| `WebSearchStarted` | 2 |
| `WebSearchFinished` | 2 |
| `ViewImage` | 1 |
| `ImageGenerationFinished` | 1 |
| `McpToolCallStarted` | 2 |
| `McpToolCallFinished` | 2 |
| `PatchApplyStarted` | 2 |
| `PatchApplyOutputDelta` | 1 |
| `PatchApplyFinished` | 2 |
| `SessionHandle` | 2 |
| `Error` | 3 |
| `Finished` | 3 |

---

## 5. Data Types Crossing the Boundary

### 5.1 Types TUI Needs to Deserialize (from daemon events)

These must be serializable and sent over WebSocket:

```rust
// Core state snapshot
struct SessionState {
    app_state: AppState,           // full transcript, input, status
    workplace: SharedWorkplaceState, // backlog, skills
    agents: Vec<AgentStatusSnapshot>, // pool status
    focused_agent_id: Option<AgentId>,
}

// Individual event deltas
enum Event {
    TranscriptItemStarted { item_id, kind, agent_id },
    TranscriptItemDelta { item_id, delta },
    TranscriptItemCompleted { item_id, content },
    AgentStatusChanged { agent_id, old, new },
    AgentSpawned { agent_id, codename, role },
    AgentStopped { agent_id },
    MailReceived { to, from, subject, body },
    DecisionRequired { request_id, situation },
    ToolApprovalRequired { request_id, tool, preview },
    Error { agent_id, message },
}
```

### 5.2 Types TUI Sends (to daemon)

```rust
enum ClientMessage {
    Initialize { workplace: PathBuf },
    SendInput { text: String },
    ApproveTool { request_id: String, allowed: bool },
    ApproveDecision { request_id: String, choice: String },
    SetFocus { agent_id: AgentId },
    StopAgent { agent_id: AgentId },
    PauseAgent { agent_id: AgentId },
    ResumeAgent { agent_id: AgentId },
    Heartbeat,
}
```

### 5.3 Types That Can Stay TUI-Only

These never cross the wire:

- `TextArea`, `TextAreaState` — composer widget state
- `TuiViewState`, `AgentViewState` — scroll and view state
- `TranscriptOverlayState`, `ConfirmationOverlay` — overlay UI state
- `MarkdownStreamCollector` — streaming markdown buffer
- `AppTerminal` — terminal handle

---

## 6. Interface Minimization Strategy

### 6.1 Phase 1: Extract Protocol Types

Create `agent-protocol` crate containing **only** the types needed for TUI-daemon communication:

```
agent-protocol/src/
  lib.rs
  messages.rs       # ClientMessage, ServerMessage
  events.rs         # Event enum
  state.rs          # SessionState, AgentStatusSnapshot
  types.rs          # Minimal re-exports: AgentId, AgentStatus, etc.
```

### 6.2 Phase 2: Move Runtime Ownership to Daemon

Remove from `TuiState`:
- `session: RuntimeSession` → daemon owns
- `agent_pool: Option<AgentPool>` → daemon owns
- `event_aggregator: EventAggregator` → daemon owns
- `mailbox: AgentMailbox` → daemon owns

Keep in `TuiState`:
- All rendering state (scroll, composer, overlays)
- Local view caches
- WebSocket connection handle

### 6.3 Phase 3: Replace Method Calls with Messages

| Current TUI Code | New Message-Based Code |
|------------------|----------------------|
| `state.spawn_agent(...)` | `ws.send(ClientMessage::SendInput { text: "/spawn claude".to_string() })` |
| `state.stop_focused_agent()` | `ws.send(ClientMessage::SendInput { text: "/stop".to_string() })` |
| `state.poll_agent_events()` | `while let Some(msg) = ws.read().await { match msg { ServerMessage::Event(e) => ... } }` |
| `state.app().transcript.clone()` | `session_state.app_state.transcript` (from `Initialize` response) |
| `mailbox.unread_count(id)` | `event` payload includes unread counts |

### 6.4 Phase 4: Consolidate ProviderEvent Handling

Currently TUI matches `ProviderEvent` in **3 separate locations** (main loop, overview, render). After separation:

- Daemon receives `ProviderEvent` from provider threads
- Daemon converts to `Event` and pushes to TUI
- TUI has **one** event handler that updates local render state

---

## 7. Risk Areas

### 7.1 High Risk: Active Transcript Entry Building

The TUI builds partially-completed transcript entries (e.g., an `ExecCommand` that receives `Started`, then multiple `OutputDelta`, then `Finished`). This logic is interleaved with render state.

**Mitigation**: Move the "active entry builder" into the daemon. TUI only receives finalized `TranscriptItem` events or simple deltas.

### 7.2 High Risk: EventAggregator Channel Management

TUI currently registers/unregisters `mpsc::Receiver<ProviderEvent>` channels as agents spawn/stop.

**Mitigation**: Daemon manages all channels internally. TUI has a single WebSocket connection per session.

### 7.3 Medium Risk: Resume Snapshot Serialization

`TuiResumeSnapshot` contains a mix of core state and TUI state.

**Mitigation**: Split into `SessionSnapshot` (daemon) and `ViewSnapshot` (TUI local storage).

### 7.4 Medium Risk: Command Bus / Slash Commands

TUI currently calls `parse_slash_command()` directly and then mutates core state.

**Mitigation**: Send raw text to daemon; daemon parses and executes commands.

---

## 8. Recommended Refactoring Order

1. **Extract `agent-protocol`** with minimal event + state types
2. **Move `EventAggregator` into daemon** — make it broadcast over WebSocket instead of `mpsc`
3. **Extract transcript builder** from TuiState into a core module that produces `Event` stream
4. **Replace `TuiState.session` with WebSocket client** — daemon returns `SessionState` on connect
5. **Replace `TuiState.agent_pool` with event-driven updates**
6. **Replace `TuiState.mailbox` with mail events**
7. **Delete all direct `agent_core` imports from `tui/src`** except protocol types

---

## 9. Appendix: Raw Call Sites

### 9.1 TUI Files with Core Imports

| File | Core Imports |
|------|-------------|
| `app_loop.rs` | `AppState`, `AppStatus`, `LoopPhase`, `TranscriptEntry`, `CommandBus`, `commands`, `logging`, `probe`, `ProviderEvent`, `RuntimeSession`, `task_engine`, `GlobalConfigStore`, `ProfilePersistence`, `agent_mail`, `AgentId`, `WorkplaceStore`, `DecisionRequest`, `ProviderKind`, `SessionHandle` |
| `ui_state.rs` | `AgentMailbox`, `AgentPool`, `AgentRole`, `AgentId`, `AgentMeta`, `AgentStatus`, `ProviderSessionId`, `AppState`, `AppStatus`, `LoopPhase`, `TranscriptEntry`, `EventAggregator`, `logging`, `ProviderEvent`, `ProviderKind`, `RuntimeSession`, `SharedWorkplaceState`, `shutdown_snapshot`, `ExecCommandStatus`, `McpInvocation`, `McpToolCallStatus`, `PatchApplyStatus`, `PatchChange`, `WebSearchAction`, `WorkplaceStore`, `launch_config`, `provider_profile`, `agent_mail` |
| `tui_snapshot.rs` | `AgentMailbox`, `AgentRole`, `AgentCodename`, `AgentId`, `AgentSlotStatus`, `TaskId`, `TranscriptEntry`, `BacklogState`, `AgentLaunchBundle`, `ProviderKind`, `SessionHandle`, `ShutdownReason`, `WorkplaceStore`, `RuntimeSession` |
| `render.rs` | `AgentStatusSnapshot`, `AgentRole`, `TranscriptEntry`, `AppStatus`, `ProviderKind`, `RuntimeSession` |
| `overview_row.rs` | `AgentStatusSnapshot`, `AgentSlotStatus`, `AgentRole`, `AgentCodename`, `AgentId`, `ProviderType` |
| `history_cell.rs` | `TranscriptEntry`, `ExecCommandStatus`, `McpInvocation`, `McpToolCallStatus`, `PatchApplyStatus`, `PatchChange`, `PatchChangeKind`, `WebSearchAction` |
| `resume_overlay.rs` | `ShutdownReason`, `ShutdownSnapshot`, `AgentRuntime`, `BacklogState`, `AgentShutdownSnapshot` |
| `input.rs` | `AppStatus`, `AppState`, `ProviderKind`, `RuntimeSession`, `SkillRegistry` |
| `test_support.rs` | `AgentRuntime`, `WorkplaceId`, `AppState`, `AppStatus`, `ProviderKind`, `RuntimeSession`, `SharedWorkplaceState`, `SkillRegistry`, `WorkplaceStore` |
| `command_runtime.rs` | `ProviderKind`, `provider_capabilities`, `WorkplaceStore` |
| `composer/footer.rs` | `AppStatus`, `LoopPhase` |
| `profile_selection_overlay.rs` | `ProviderProfile`, `CliBaseType` |
| `provider_overlay.rs` | `ProviderKind` |
| `launch_config_overlay.rs` | `LaunchSourceMode`, `ProviderKind` |
| `shell_tests.rs` | `TranscriptEntry`, `logging`, `RunMode`, `ProviderKind`, `WorkplaceStore` |
| `transcript/cells.rs` | `TranscriptEntry` |
| `lib.rs` | `logging`, `RunMode`, `probe`, `WorkplaceStore` |

### 9.2 CLI Files with Core Imports

| File | Core Imports |
|------|-------------|
| `app_runner.rs` | `AgentBootstrapKind`, `AgentRuntime`, `AgentStore`, `AppState`, `backlog_store`, `logging`, `loop_runner`, `MultiAgentSession`, `probe`, `RuntimeMode`, `session_store`, `SkillRegistry`, `WorkplaceStore`, `ProviderKind`, `shutdown_snapshot`, `backlog`, `AgentStatus`, `event_aggregator`, `GlobalConfigStore`, `ProfilePersistence` |
