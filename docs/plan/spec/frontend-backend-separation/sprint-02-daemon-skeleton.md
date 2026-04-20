# Sprint 2: Daemon Skeleton — WebSocket + Router

## Metadata

- Sprint ID: `sprint-fbs-002`
- Title: `Daemon Skeleton — WebSocket + Router`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-20
- Depends On: [Sprint 1: Protocol Foundation](./sprint-01-protocol-foundation.md)

## Background

With the `agent-protocol` crate complete, we have the shared language — but no process speaks it yet. The daemon process does not exist; there is no WebSocket server, no connection handler, and no request routing. A client attempting to connect would find nothing listening on any port.

This sprint builds the service-layer skeleton: the `agent-daemon` binary that binds to a local WebSocket, accepts connections, parses JSON-RPC, and routes requests. At the end of this sprint, a test client can connect, send `session.initialize`, and receive a response. The daemon has no real state yet — it returns hardcoded stubs — but the full request/response pipeline is operational.

## Sprint Goal

Create the `agent-daemon` binary with a functional WebSocket server that accepts connections, parses JSON-RPC messages, and routes requests to the correct handler. The daemon can start, bind to an ephemeral port, and echo a `session.initialize` response. No real runtime state yet — just the service layer skeleton.

## TDD Approach

This sprint introduces async I/O and concurrency — tests must be deterministic and fast.

1. **Red**: Write tests using in-memory `tokio::io::duplex()` streams to simulate WebSocket connections without real TCP.
2. **Green**: Implement the server, connection handler, and router until tests pass.
3. **Refactor**: Extract shared test harness code; ensure no real ports are needed for unit tests.

Test requirements per story:
- In-memory WebSocket tests (no real TCP, no port conflicts)
- Unit tests for router dispatch (mock handlers)
- Integration tests for full request→response pipeline
- Connection lifecycle tests: connect, initialize, disconnect, cleanup
- All async tests use `tokio::test` with deterministic ordering

## Stories

### Story 2.1: WebSocket Server Binding

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the WebSocket server that binds to localhost on an ephemeral port.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Create `agent/daemon/` crate with `Cargo.toml` | Todo | - |
| T2.1.2 | Implement `WebSocketServer::bind()` — binds to `127.0.0.1:0` | Todo | - |
| T2.1.3 | Implement `WebSocketServer::local_addr()` returning assigned port | Todo | - |
| T2.1.4 | Implement TCP accept loop with `tokio::net::TcpListener` | Todo | - |
| T2.1.5 | Integrate `tokio_tungstenite::accept_async` for WebSocket upgrade | Todo | - |
| T2.1.6 | Reject binary frames with code `1003` | Todo | - |
| T2.1.7 | Write unit test: server binds and returns valid local address | Todo | - |
| T2.1.8 | Write unit test: binary frame rejection | Todo | - |

#### Acceptance Criteria

- Server binds successfully on first attempt (OS assigns ephemeral port)
- `local_addr()` returns `127.0.0.1:<port>`
- Binary WebSocket frames are rejected with `1003`
- Server runs until explicitly shut down
- **Tests**: `server_bind_ephemeral` — binds to `127.0.0.1:0` and returns valid port; `server_reject_binary` — binary frame rejected with `1003`


#### Technical Notes

See IMP-03 §2.1 and §3.1. The server does not handle TLS for v1. Use `tokio-tungstenite` for WebSocket handling.

---

### Story 2.2: Connection Handler + State Machine

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement per-connection state management: read JSON-RPC messages, enforce initialization gate, write responses.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Create `Connection` struct with `id`, `addr`, `state` fields | Todo | - |
| T2.2.2 | Define `ConnectionState` enum (`Connected`, `Initialized`, `Closing`) | Todo | - |
| T2.2.3 | Implement read loop: parse `JsonRpcMessage` from text frames | Todo | - |
| T2.2.4 | Implement initialization gate: reject methods before `session.initialize` | Todo | - |
| T2.2.5 | Implement write loop: serialize responses and send as text frames | Todo | - |
| T2.2.6 | Handle WebSocket close frames (graceful disconnect) | Todo | - |
| T2.2.7 | Write unit test: connection accepts `session.initialize` then blocks other methods | Todo | - |
| T2.2.8 | Write unit test: malformed JSON produces parse error response | Todo | - |

#### Acceptance Criteria

- Connection transitions from `Connected` → `Initialized` on successful `session.initialize`
- Any method call before `session.initialize` returns error `-32100`
- Malformed JSON returns JSON-RPC parse error (`-32700`)
- Connection cleans up on disconnect (no resource leaks)
- **Tests**: `connection_initialize_gate` — methods before `session.initialize` return `-32100`; `connection_malformed_json` — invalid JSON returns `-32700`


#### Technical Notes

See IMP-03 §2.2 and IMP-01 §1.2. Each connection gets its own `tokio::task`. Use `futures::StreamExt::split()` to separate read/write halves.

---

### Story 2.3: JSON-RPC Request/Response Framing

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the two-way JSON-RPC message serialization and deserialization within the connection handler.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Implement request parsing: text → `JsonRpcRequest` | Todo | - |
| T2.3.2 | Implement notification parsing: text → `JsonRpcNotification` | Todo | - |
| T2.3.3 | Implement response serialization: `JsonRpcResponse` → text | Todo | - |
| T2.3.4 | Implement error response serialization: `JsonRpcErrorResponse` → text | Todo | - |
| T2.3.5 | Handle unknown methods with `-32601` (Method not found) | Todo | - |
| T2.3.6 | Handle invalid params with `-32602` (Invalid params) | Todo | - |
| T2.3.7 | Write unit test: request/response round-trip | Todo | - |
| T2.3.8 | Write unit test: unknown method returns correct error | Todo | - |

#### Acceptance Criteria

- Every received text frame is parsed into exactly one `JsonRpcMessage`
- Request `id` is echoed verbatim in the response
- Unknown methods produce `-32601` with the method name in the message
- Invalid JSON produces `-32700`; invalid params produces `-32602`
- **Tests**: `framing_request_response` — request `id` echoed verbatim; `framing_unknown_method` — unknown method returns `-32601`


#### Technical Notes

See IMP-01 §2 and IMP-03 §2.2. Use `serde_json::from_str` for parsing. Errors during parsing must not panic — they produce JSON-RPC Error Responses.

---

### Story 2.4: Method Router + Dispatch

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement the router that maps method names to handler functions.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Define `Handler` trait with `async fn handle(&self, req) -> Result<Response>` | Todo | - |
| T2.4.2 | Create `Router` struct with `HashMap<String, Box<dyn Handler>>` | Todo | - |
| T2.4.3 | Implement `Router::register(method, handler)` | Todo | - |
| T2.4.4 | Implement `Router::dispatch(req)` — looks up handler by method name | Todo | - |
| T2.4.5 | Create stub handlers for all methods (return `not implemented` for now) | Todo | - |
| T2.4.6 | Implement notification dispatch (fire-and-forget) | Todo | - |
| T2.4.7 | Write unit test: registered method routes correctly | Todo | - |
| T2.4.8 | Write unit test: unregistered method returns `-32601` | Todo | - |

#### Acceptance Criteria

- Every method from IMP-01 §3 has a registered handler (even if stub)
- Router dispatches in O(1) via HashMap lookup
- Handler errors are converted to `JsonRpcErrorResponse`, not panics
- Notifications do not expect or wait for responses
- **Tests**: `router_dispatch` — registered method routes to correct handler; `router_not_found` — unregistered method returns `-32601`


#### Technical Notes

See IMP-03 §2.3. Use `async_trait` for the `Handler` trait. Avoid `BoxFuture` — the trait-based approach is cleaner and more testable. Stub handlers return `JsonRpcError` with code `-32106` (Not supported).

---

### Story 2.5: session.initialize Handler (Stub)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement the first real handler: `session.initialize` returning a hardcoded snapshot. This validates the full request→router→handler→response pipeline.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.5.1 | Create `SessionHandler` struct implementing `Handler` | Todo | - |
| T2.5.2 | Implement `session.initialize` parsing `InitializeParams` | Todo | - |
| T2.5.3 | Return hardcoded `SessionState` with valid structure | Todo | - |
| T2.5.4 | Set connection state to `Initialized` on success | Todo | - |
| T2.5.5 | Validate `client_type` enum (reject unknown values) | Todo | - |
| T2.5.6 | Include `protocol_version` in response | Todo | - |
| T2.5.7 | Write integration test: client sends initialize, receives snapshot | Todo | - |
| T2.5.8 | Write integration test: double initialize returns `-32105` | Todo | - |

#### Acceptance Criteria

- `session.initialize` returns a `SessionState` with all required fields
- Connection state transitions to `Initialized` after successful response
- Second `session.initialize` on same connection returns error `-32105`
- Unknown `client_type` values produce `-32602`
- **Tests**: `initialize_returns_snapshot` — response contains valid `SessionState`; `initialize_twice_fails` — second initialize returns `-32105`


#### Technical Notes

See IMP-01 §3.2 and IMP-03 §2.5. The snapshot is hardcoded for this sprint — real data comes in Sprint 4. Use `serde_json::from_value` to parse params into typed structs.

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| `tokio-tungstenite` API changes | Low | Medium | Pin to exact version in `Cargo.toml` |
| Connection leak on rapid connect/disconnect | Medium | High | Add cleanup test, verify `Drop` impls |
| Router performance with many methods | Low | Low | HashMap is O(1); premature optimization |

## Sprint Deliverables

- `agent/daemon/src/main.rs` — binary entry point
- `agent/daemon/src/server.rs` — WebSocket server
- `agent/daemon/src/connection.rs` — per-connection handler
- `agent/daemon/src/router.rs` — method dispatch
- `agent/daemon/src/handler/session.rs` — stub `session.initialize`
- Integration tests: in-memory WebSocket client connects to daemon

## Dependencies

- [Sprint 1: Protocol Foundation](./sprint-01-protocol-foundation.md) — `agent-protocol` crate must exist with all types.

## Next Sprint

After completing this sprint, proceed to [Sprint 3: Auto-Link + Daemon Lifecycle](./sprint-03-auto-link-lifecycle.md) for daemon startup, shutdown, and client-side auto-discovery.
