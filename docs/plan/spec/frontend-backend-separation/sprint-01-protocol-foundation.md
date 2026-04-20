# Sprint 1: Protocol Foundation

## Metadata

- Sprint ID: `sprint-fbs-001`
- Title: `Protocol Foundation`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: Architecture Blueprint approval, IMP-01/02 finalized

## Sprint Goal

Establish the `agent-protocol` crate as the single source of truth for all daemon-client communication. All JSON-RPC 2.0 envelope types, method enums, event types, and state snapshot types are defined, serializable, and covered by unit tests. No runtime logic — pure data contracts only.

## Stories

### Story 1.1: JSON-RPC 2.0 Envelope Types

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the generic JSON-RPC 2.0 message types that wrap all protocol communication.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `JsonRpcMessage` enum (Request/Notification/Response/Error) | Todo | - |
| T1.1.2 | Create `JsonRpcRequest` struct with `id: RequestId(String)` | Todo | - |
| T1.1.3 | Create `JsonRpcNotification` struct (no `id`) | Todo | - |
| T1.1.4 | Create `JsonRpcResponse` + `JsonRpcErrorResponse` structs | Todo | - |
| T1.1.5 | Create `JsonRpcError` struct with `code`, `message`, `data` | Todo | - |
| T1.1.6 | Implement `Serialize`/`Deserialize` for all envelope types | Todo | - |
| T1.1.7 | Write round-trip serialization tests | Todo | - |
| T1.1.8 | Write parse-error handling tests (invalid JSON) | Todo | - |

#### Acceptance Criteria

- All JSON-RPC 2.0 envelope types serialize to spec-compliant JSON
- `RequestId` is always a string, never an integer
- Invalid JSON produces a parseable error structure
- Round-trip serialization is lossless for all variants

#### Technical Notes

See IMP-02 §2.2 for type definitions. The envelope must be transport-agnostic — no `tokio`, no `tungstenite`.

---

### Story 1.2: Method Types — Client → Daemon

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define all method parameters and result types for client-to-daemon requests.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Create `ClientMethod` enum with all variants | Todo | - |
| T1.2.2 | Define `InitializeParams`, `SendInputParams`, `SetFocusParams` | Todo | - |
| T1.2.3 | Define `AgentSpawnParams`, `AgentStopParams`, `AgentListParams` | Todo | - |
| T1.2.4 | Define `ToolApproveParams`, `DecisionRespondParams` | Todo | - |
| T1.2.5 | Define all result types (`SessionState`, `AgentSnapshot`, etc.) | Todo | - |
| T1.2.6 | Implement method-name mapping function (`method_name_and_params`) | Todo | - |
| T1.2.7 | Write unit tests for each param type serialization | Todo | - |
| T1.2.8 | Write contract test: every `ClientMethod` variant round-trips | Todo | - |

#### Acceptance Criteria

- Every method name matches IMP-01 §3 specification exactly (`session.initialize`, `agent.spawn`, etc.)
- Param structs use `#[serde(default)]` and `#[serde(skip_serializing_if)]` where appropriate
- `ClientMethod` enum is exhaustive — no stringly-typed dispatch
- All parameter types have corresponding deserialization tests

#### Technical Notes

See IMP-02 §3. Use `agent_types::AgentRole` and `agent_types::ProviderKind` for enum fields. Do **not** create duplicate role/provider enums in the protocol crate.

---

### Story 1.3: Event Types — Daemon → Client

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the event payload types that the daemon broadcasts to all connected clients.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Create `Event` struct with `seq: u64` + `payload: EventPayload` | Todo | - |
| T1.3.2 | Create `EventPayload` enum with all variants | Todo | - |
| T1.3.3 | Define `AgentSpawnedData`, `AgentStoppedData`, `AgentStatusChangedData` | Todo | - |
| T1.3.4 | Define `ItemStartedData`, `ItemDeltaData`, `ItemCompletedData` | Todo | - |
| T1.3.5 | Define `MailReceivedData`, `ErrorData` | Todo | - |
| T1.3.6 | Define server-initiated notifications (`ApprovalRequest`, `DecisionRequest`) | Todo | - |
| T1.3.7 | Implement `Serialize`/`Deserialize` with `#[serde(tag, content)]` | Todo | - |
| T1.3.8 | Write serialization tests: each variant produces correct wire format | Todo | - |

#### Acceptance Criteria

- Every `EventPayload` variant serializes to the wire format defined in IMP-01 §4.1
- `type` field uses camelCase (`agentSpawned`, `itemDelta`, etc.)
- `data` field contains the payload object
- Server-initiated notifications (`approval.request`) are distinct from `event` notifications

#### Technical Notes

See IMP-02 §4. The `#[serde(tag = "type", rename_all = "camelCase", content = "data")]` attribute must produce exactly the shape in IMP-01.

---

### Story 1.4: State Snapshot Types

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define the `SessionState` and all nested snapshot types sent on `session.initialize`.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Create `SessionState` struct with all fields | Todo | - |
| T1.4.2 | Create `AppStateSnapshot`, `InputState`, `SessionStatus` | Todo | - |
| T1.4.3 | Create `TranscriptItem`, `ItemKind` | Todo | - |
| T1.4.4 | Create `AgentSnapshot`, `AgentSlotStatus` | Todo | - |
| T1.4.5 | Create `WorkplaceSnapshot`, `BacklogSnapshot`, `SkillSnapshot` | Todo | - |
| T1.4.6 | Implement serialization with `#[serde(skip_serializing_if)]` for optional fields | Todo | - |
| T1.4.7 | Write unit tests for `SessionState` serialization | Todo | - |
| T1.4.8 | Write deserialization tests with partial/missing fields | Todo | - |

#### Acceptance Criteria

- `SessionState` contains all fields defined in IMP-01 §5.1
- Timestamps are ISO 8601 strings, not `chrono` types
- Optional fields (`focused_agent_id`, `completed_at`) are omitted when null
- `protocol_version` field is present and matches `PROTOCOL_VERSION` constant

#### Technical Notes

See IMP-02 §5. `metadata` fields use `serde_json::Value` for opaque data. The protocol crate does not depend on `chrono`.

---

### Story 1.5: ProtocolError + Version Constant

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Implement application-specific error types and the protocol version constant.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.5.1 | Create `ProtocolError` enum with all variants | Todo | - |
| T1.5.2 | Implement `code()` method returning i32 error codes | Todo | - |
| T1.5.3 | Implement `data()` method returning structured error context | Todo | - |
| T1.5.4 | Implement `From<ProtocolError> for JsonRpcError` | Todo | - |
| T1.5.5 | Add `PROTOCOL_VERSION` constant (`"1.0.0"`) | Todo | - |
| T1.5.6 | Write error serialization tests | Todo | - |

#### Acceptance Criteria

- Every `ProtocolError` variant maps to the correct code (-32100 through -32106)
- Error `data` includes contextual fields (`agent_id`, `request_id`, etc.)
- `JsonRpcError` serialization matches IMP-01 §7.3
- `PROTOCOL_VERSION` is accessible from both daemon and client crates

#### Technical Notes

See IMP-02 §6. Use `thiserror` for `Display` impls. Error codes must not conflict with standard JSON-RPC codes (-32700 to -32603).

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| `serde` attribute misconfiguration | Medium | High | Contract tests verify every variant against expected JSON shape |
| `agent_types` enum changes break protocol | Low | Medium | Protocol crate re-exports from `agent_types`, does not duplicate |
| Over-engineering the type system | Medium | Medium | Keep types flat — no nested generic params, no custom Serialize impls |

## Sprint Deliverables

- `agent/protocol/src/lib.rs` — crate root with re-exports and version constant
- `agent/protocol/src/jsonrpc.rs` — JSON-RPC envelope types
- `agent/protocol/src/methods.rs` — method enums, param/result types
- `agent/protocol/src/events.rs` — event types and payloads
- `agent/protocol/src/state.rs` — state snapshot types
- Unit tests covering all serialization/deserialization paths
- `Cargo.toml` with correct dependencies (no runtime crates)

## Dependencies

None — this is the foundation sprint. All subsequent sprints depend on this one.

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Daemon Skeleton — WebSocket + Router](./sprint-02-daemon-skeleton.md) for the daemon WebSocket server and request routing.
