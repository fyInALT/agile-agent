# 02 — agent-protocol Crate Design

> Status: Draft ✅ DECIDED  
> Date: 2026-04-20  
> Scope: Rust types, crate layout, serialization strategy, type-safe JSON-RPC

This document defines the `agent-protocol` crate — the **single source of truth** for all messages exchanged between daemon and clients. Every type in this crate is `Serialize + DeserializeOwned` and maps directly to the wire format defined in IMP-01.

---

## 1. Crate Structure

```
agent/protocol/
├── Cargo.toml
└── src/
    ├── lib.rs              # Re-exports, version constant
    ├── jsonrpc.rs          # JSON-RPC 2.0 envelope types (generic, wire-agnostic)
    ├── methods.rs          # Method enum + per-method param/result types
    ├── events.rs           # Event enum + EventData types
    ├── state.rs            # SessionState, TranscriptItem, AgentSnapshot
    └── client.rs           # Shared WebSocket client abstraction (optional v1)
```

**Dependencies** (`Cargo.toml`):

```toml
[package]
name = "agent-protocol"
version = "1.0.0"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
agent-types = { path = "../types" }   # AgentRole, ProviderKind only
```

**No `tokio`, no `tungstenite`, no `agent-core`**. This crate is pure data. Transport concerns live in `agent-daemon` and `agent-tui`.

---

## 2. Core Type Design

### 2.1 Design Goal: Type Safety Without Stringly-Typed APIs

A naive JSON-RPC implementation uses `String` for method names and `serde_json::Value` for params. This is brittle — a typo in a method name is a runtime error, not a compile-time error.

Instead, we use **strongly-typed method enums** where every variant carries its param type:

```rust
// agent-protocol/src/methods.rs

pub enum ClientMethod {
    SessionInitialize(InitializeParams),
    SessionHeartbeat,
    SessionSendInput(SendInputParams),
    SessionSetFocus(SetFocusParams),
    AgentSpawn(AgentSpawnParams),
    AgentStop(AgentStopParams),
    AgentList(AgentListParams),
    ToolApprove(ToolApproveParams),
    DecisionRespond(DecisionRespondParams),
}
```

The `ClientMethod` enum serializes to the wire-format method name via a custom `Serialize` impl:

```rust
// Wire: {"method":"session.initialize","params":{"clientType":"tui"}}
// Rust: ClientMethod::SessionInitialize(InitializeParams { client_type: ClientType::Tui, .. })
```

This gives us:
- **Compile-time correctness**: You cannot construct a `SessionSendInput` with `AgentSpawnParams`.
- **Exhaustive matching**: `match method { SessionInitialize(p) => ..., AgentSpawn(p) => ... }` — the compiler ensures every method is handled.
- **Self-documenting**: The type system encodes the protocol spec.

### 2.2 JSON-RPC Envelope (Generic)

```rust
// agent-protocol/src/jsonrpc.rs

use serde::{Deserialize, Serialize};

/// The top-level message. Transport-agnostic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
    Error(JsonRpcErrorResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,        // always "2.0"
    pub id: RequestId,          // String, not integer
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: RequestId,
    pub error: JsonRpcError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
}
```

**Why `serde_json::Value` for params/result?** Because the envelope is generic. Typed params live in `methods.rs`. The flow is:

1. Receive raw JSON → deserialize to `JsonRpcMessage`
2. If it's a Request, inspect `method` string
3. Parse `params` into the concrete param type for that method
4. Route to handler

This two-step deserialization is clean and avoids the "one giant enum with 50 variants" anti-pattern.

### 2.3 Request ID Type

`RequestId` is a `String`, not an integer. This allows clients to generate UUID-style IDs:

```rust
let id = RequestId::String(format!("req-{}", uuid::Uuid::new_v4()));
```

The daemon echoes the same `RequestId` back in the Response. No ID generation logic on the server side.

---

## 3. Method Types

### 3.1 Param Types (Client → Daemon)

Every method has a dedicated param struct:

```rust
// agent-protocol/src/methods.rs

use serde::{Deserialize, Serialize};
use agent_types::{AgentRole, ProviderKind};

// session.initialize
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub client_type: ClientType,
    pub client_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
    Tui,
    Cli,
    Ide,
}

// session.sendInput
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendInputParams {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_agent_id: Option<String>,
}

// session.setFocus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFocusParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

// agent.spawn
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpawnParams {
    pub provider: ProviderKind,
    pub role: AgentRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codename: Option<String>,
}

// agent.stop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStopParams {
    pub agent_id: String,
    #[serde(default)]
    pub force: bool,
}

// agent.list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListParams {
    #[serde(default)]
    pub include_stopped: bool,
}

// tool.approve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApproveParams {
    pub request_id: String,
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifications: Option<serde_json::Value>,
}

// decision.respond
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRespondParams {
    pub request_id: String,
    pub choice: DecisionChoice,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionChoice {
    Approve,
    Reject,
    Escalate,
}
```

### 3.2 Result Types (Daemon → Client)

```rust
// agent-protocol/src/methods.rs

use crate::state::{AgentSnapshot, SessionState};

// session.initialize → SessionState
// session.sendInput → SendInputResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendInputResult {
    pub accepted: bool,
    pub item_id: String,
}

// session.setFocus → SetFocusResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFocusResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_agent_id: Option<String>,
}

// agent.spawn → AgentSnapshot
// agent.stop → AgentStopResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStopResult {
    pub stopped: bool,
    pub agent_id: String,
}

// agent.list → AgentListResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListResult {
    pub agents: Vec<AgentSnapshot>,
}

// tool.approve → ToolApproveResult
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolApproveResult {
    pub resolved: bool,
    pub request_id: String,
}

// decision.respond → DecisionRespondResult
pub type DecisionRespondResult = ToolApproveResult; // Same shape
```

### 3.3 Method Name ↔ Variant Mapping

The `ClientMethod` enum uses a custom `Serialize`/`Deserialize` implementation to map to wire-format method names:

```rust
impl Serialize for ClientMethod {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (method, params) = match self {
            ClientMethod::SessionInitialize(p) => ("session.initialize", serde_json::to_value(p).unwrap()),
            ClientMethod::SessionHeartbeat => ("session.heartbeat", serde_json::Value::Null),
            ClientMethod::SessionSendInput(p) => ("session.sendInput", serde_json::to_value(p).unwrap()),
            // ... etc
        };
        // Serialize as {"method":"session.initialize","params":{...}}
        // (omitted for brevity)
    }
}
```

**Alternative (simpler)**: Instead of a custom `Serialize`, use a `match` in the transport layer:

```rust
fn method_name_and_params(method: &ClientMethod) -> (&'static str, serde_json::Value) {
    match method {
        ClientMethod::SessionInitialize(p) => ("session.initialize", json!(p)),
        ClientMethod::SessionHeartbeat => ("session.heartbeat", json!({})),
        // ...
    }
}
```

This avoids the complexity of custom `Serialize` impls and is more maintainable. **This is the recommended approach for v1**.

---

## 4. Event Types

### 4.1 Event Enum

```rust
// agent-protocol/src/events.rs

use serde::{Deserialize, Serialize};

/// A daemon-generated event, sent as a JSON-RPC Notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    #[serde(flatten)]
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", content = "data")]
pub enum EventPayload {
    AgentSpawned(AgentSpawnedData),
    AgentStopped(AgentStoppedData),
    AgentStatusChanged(AgentStatusChangedData),
    ItemStarted(ItemStartedData),
    ItemDelta(ItemDeltaData),
    ItemCompleted(ItemCompletedData),
    MailReceived(MailReceivedData),
    Error(ErrorData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpawnedData {
    pub agent_id: String,
    pub codename: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStoppedData {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusChangedData {
    pub agent_id: String,
    pub status: AgentSlotStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStartedData {
    pub item_id: String,
    pub kind: ItemKind,
    pub agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDeltaData {
    pub item_id: String,
    pub delta: ItemDelta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCompletedData {
    pub item_id: String,
    pub item: TranscriptItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailReceivedData {
    pub to: String,
    pub from: String,
    pub subject: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
```

### 4.2 Event Serialization

An `Event` serializes to the wire format defined in IMP-01 §4.1:

```json
{
  "seq": 42,
  "type": "agentSpawned",
  "data": {
    "agentId": "agent-a1b2",
    "codename": "claude-dev",
    "role": "Developer"
  }
}
```

The `#[serde(tag = "type", rename_all = "camelCase", content = "data")]` attribute produces the correct shape.

### 4.3 Server-Initiated Notification Types

These are not `Event`s — they are separate notification methods:

```rust
// agent-protocol/src/methods.rs

/// Notifications sent by the daemon to the client (server-initiated).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum ServerNotification {
    #[serde(rename = "approval.request")]
    ApprovalRequest {
        request_id: String,
        agent_id: String,
        tool: String,
        preview: String,
        timeout_ms: u64,
    },
    #[serde(rename = "decision.request")]
    DecisionRequest {
        request_id: String,
        situation: String,
        options: Vec<String>,
        timeout_ms: u64,
    },
    #[serde(rename = "session.heartbeatAck")]
    HeartbeatAck {
        server_time: String, // ISO 8601
    },
}
```

Note: `event` is also a server notification but uses the `Event` type above.

---

## 5. State Types

### 5.1 SessionState

```rust
// agent-protocol/src/state.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub alias: String,
    pub server_time: String,      // ISO 8601
    pub last_event_seq: u64,
    pub app_state: AppStateSnapshot,
    pub agents: Vec<AgentSnapshot>,
    pub workplace: WorkplaceSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused_agent_id: Option<String>,
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub transcript: Vec<TranscriptItem>,
    pub input: InputState,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputState {
    pub text: String,
    pub multiline: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Idle,
    Running,
    WaitingForApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptItem {
    pub id: String,
    pub kind: ItemKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: String,       // ISO 8601
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    UserInput,
    AssistantOutput,
    ToolCall,
    ToolResult,
    SystemMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub id: String,
    pub codename: String,
    pub role: String,
    pub provider: String,
    pub status: AgentSlotStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task_id: Option<String>,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSlotStatus {
    Idle,
    Running,
    Stopped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkplaceSnapshot {
    pub id: String,
    pub path: String,
    pub backlog: BacklogSnapshot,
    pub skills: Vec<SkillSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklogSnapshot {
    // Placeholder — exact fields TBD when kanban model stabilizes
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    pub name: String,
    pub enabled: bool,
}
```

### 5.2 Design Notes

- **No `agent_core` types**: `AgentSnapshot` duplicates a subset of `AgentSlot` fields. This is intentional — the protocol crate must not depend on core.
- **String timestamps**: Using `String` (ISO 8601) instead of `chrono::DateTime` avoids pulling in `chrono` as a dependency. Clients can parse if needed.
- `serde_json::Value` for metadata: Transcript item metadata is opaque to the protocol. The daemon serializes whatever `agent_core` gives it.

---

## 6. Error Type

```rust
// agent-protocol/src/jsonrpc.rs

use thiserror::Error;

/// Application-specific error, convertible to JsonRpcError.
#[derive(Debug, Error, Clone)]
pub enum ProtocolError {
    #[error("Session not initialized")]
    SessionNotInitialized,
    #[error("Agent not found: {agent_id}")]
    AgentNotFound { agent_id: String },
    #[error("Tool approval request not found: {request_id}")]
    ToolApprovalNotFound { request_id: String },
    #[error("Decision request not found: {request_id}")]
    DecisionNotFound { request_id: String },
    #[error("Workspace not found")]
    WorkspaceNotFound,
    #[error("Session already initialized")]
    SessionAlreadyInitialized,
    #[error("Not supported: {message}")]
    NotSupported { message: String },
}

impl ProtocolError {
    pub fn code(&self) -> i32 {
        match self {
            ProtocolError::SessionNotInitialized => -32100,
            ProtocolError::AgentNotFound { .. } => -32101,
            ProtocolError::ToolApprovalNotFound { .. } => -32102,
            ProtocolError::DecisionNotFound { .. } => -32103,
            ProtocolError::WorkspaceNotFound => -32104,
            ProtocolError::SessionAlreadyInitialized => -32105,
            ProtocolError::NotSupported { .. } => -32106,
        }
    }

    pub fn data(&self) -> Option<serde_json::Value> {
        match self {
            ProtocolError::AgentNotFound { agent_id } => {
                Some(json!({ "agentId": agent_id }))
            }
            ProtocolError::ToolApprovalNotFound { request_id } => {
                Some(json!({ "requestId": request_id }))
            }
            ProtocolError::DecisionNotFound { request_id } => {
                Some(json!({ "requestId": request_id }))
            }
            ProtocolError::NotSupported { message } => {
                Some(json!({ "message": message }))
            }
            _ => None,
        }
    }
}

impl From<ProtocolError> for JsonRpcError {
    fn from(e: ProtocolError) -> Self {
        JsonRpcError {
            code: e.code(),
            message: e.to_string(),
            data: e.data(),
        }
    }
}
```

---

## 7. Version Constant

```rust
// agent-protocol/src/lib.rs

pub const PROTOCOL_VERSION: &str = "1.0.0";
```

This is the version negotiated during `session.initialize` (IMP-01 §8). It is **not** the crate version — crate may be `1.0.0` while protocol is `1.0.0`, but they evolve independently.

---

## 8. What Is NOT in This Crate

| Concern | Where It Lives | Why |
|---------|---------------|-----|
| WebSocket I/O | `agent-daemon`, `agent-tui` | Transport is not part of the contract |
| `tokio` runtime | `agent-daemon`, `agent-tui` | Async runtime is a caller concern |
| `tungstenite` types | `agent-daemon`, `agent-tui` | Same reason |
| `RuntimeSession` | `agent-core` | Domain logic, not wire format |
| `ProviderEvent` | `agent-core` | Internal event type, mapped to `Event` by daemon |
| `TuiState` | `agent-tui` | Presentation state, not protocol |

---

## 9. Future Extensions (v2+)

These are noted but **not implemented** in v1:

- **`agent-protocol/src/client.rs`**: A shared `ProtocolClient` trait with `async fn call(method) -> Result` and `fn subscribe_events() -> Stream<Event>`. This would be used by both TUI and CLI. Deferred because TUI and CLI have different async requirements (TUI uses `crossterm` event loop, CLI uses blocking or simple async).
- **Compression negotiation**: Add `compression: "deflate" | "none"` to `InitializeParams`.
- **Binary frames**: For large transcript attachments, use binary WebSocket frames with MessagePack.
- **Batch requests**: Allow `[request1, request2]` arrays for bulk operations.
