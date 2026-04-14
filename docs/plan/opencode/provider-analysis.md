# OpenCode Provider Integration Analysis

## Metadata

- Date: 2026-04-13
- Project: agile-agent
- Target: OpenCode Provider Support
- Status: Draft
- Language: English

## 1. OpenCode Overview

OpenCode is an open-source AI coding agent similar to Claude Code and Codex. It provides:

- Terminal UI for interactive coding
- CLI commands for headless execution
- HTTP server mode for API access
- ACP (Agent Client Protocol) for standardized agent communication

### Key URLs

- Website: https://opencode.ai
- GitHub: https://github.com/anomalyco/opencode
- npm: `opencode-ai`
- ACP Spec: https://agentclientprotocol.com/

---

## 2. Protocol Comparison

### 2.1 Three Provider Protocols

| Provider | Protocol | Transport | Session Continuity |
|----------|----------|-----------|-------------------|
| Claude | stream-json | stdin/stdout | `session_id` via `--resume` |
| Codex | JSON-RPC (proprietary) | stdin/stdout (app-server) | `thread_id` via thread operations |
| OpenCode | ACP (standardized) | stdin/stdout or HTTP | `sessionID` via session operations |

### 2.2 Claude Protocol (stream-json)

**Communication Pattern:**

```
stdin: JSON payload (user message)
stdout: Line-delimited JSON events
```

**Key Events:**

- `{"type":"assistant","message":...}` - Assistant response
- `{"type":"system","session_id":"..."}` - Session handle
- `{"type":"result",...}` - Completion result
- `{"type":"log",...}` - Log messages

**Session Handling:**

- `--resume <session_id>` for multi-turn

**Current Implementation:**

- `core/src/providers/claude.rs` - Parses stream-json events
- `SessionHandle::ClaudeSession { session_id }`

---

### 2.3 Codex Protocol (JSON-RPC)

**Communication Pattern:**

```
stdin: JSON-RPC requests (lines)
stdout: JSON-RPC responses + notifications (lines)
```

**Key Methods:**

- `initialize` - Client capability negotiation
- `thread/start` - Create new thread
- `thread/resume` - Resume existing thread
- `turn/start` - Start conversation turn
- `item/agentMessage/delta` - Streaming text
- `item/commandExecution/*` - Tool execution events
- `item/fileChange/*` - Patch events

**Session Handling:**

- `thread_id` from `thread/start` or `thread/resume`
- Thread persisted across turns

**Current Implementation:**

- `core/src/providers/codex.rs` - Full JSON-RPC protocol
- `SessionHandle::CodexThread { thread_id }`
- Approval handling via JSON-RPC responses

---

### 2.4 OpenCode Protocol (ACP)

**Communication Pattern:**

```
stdin: JSON-RPC 2.0 requests (ACP standard)
stdout: JSON-RPC 2.0 responses + notifications
```

**ACP Methods (v1):**

| Method | Description |
|--------|-------------|
| `initialize` | Protocol version, capabilities |
| `session/new` | Create new session |
| `session/load` | Resume existing session |
| `session/prompt` | Send user message |
| `session/update` | Streaming notification (future) |
| `authenticate` | Authentication (optional) |

**Session Handling:**

- `sessionID` from `session/new` or `session/load`
- Working directory (`cwd`) per session
- MCP server configuration support

**OpenCode CLI:**

```bash
# Start ACP server
opencode acp [--cwd /path]

# JSON-RPC example
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}' | opencode acp
```

---

## 3. Current Provider Architecture Analysis

### 3.1 Architecture Overview

```
provider.rs (core)
├── ProviderKind enum: Mock, Claude, Codex
├── ProviderEvent enum: Unified events
├── SessionHandle enum: ClaudeSession, CodexThread
├── start_provider(): Dispatch to specific provider
│
providers/
├── claude.rs
│   ├── Stream-JSON parsing
│   └── stdin/stdout communication
│
├── codex.rs
│   ├── JSON-RPC protocol
│   ├── Thread lifecycle management
│   └── Approval handling
│
└── mod.rs: Module exports
```

### 3.2 Extensibility Assessment

**Strengths:**

1. `ProviderKind` is enum-based - can add new providers
2. `ProviderEvent` is unified - works across protocols
3. `SessionHandle` is extensible - can add `OpenCodeSession`
4. Thread-based execution pattern - reusable

**Weaknesses:**

1. **Protocol-specific parsing hard-coded** - Each provider has custom JSON parsing
2. **No abstraction for protocol negotiation** - Codex's `initialize` is provider-specific
3. **Session handle variants are fragmented** - Adding new provider requires enum modification
4. **Approval handling is Codex-specific** - No generic approval mechanism
5. **Thread state management varies** - Codex tracks thread state, Claude doesn't

### 3.3 What Needs to Change for OpenCode

| Aspect | Current | Required Change |
|--------|---------|-----------------|
| `ProviderKind` | Mock, Claude, Codex | Add `OpenCode` variant |
| `SessionHandle` | ClaudeSession, CodexThread | Add `OpenCodeSession { session_id }` |
| Protocol parsing | Provider-specific | New `providers/opencode.rs` module |
| Session negotiation | Codex-only | Add ACP session/new/load pattern |
| Approval handling | Codex-specific | ACP has permission.asked pattern |
| Streaming | Different patterns | ACP uses session/update (not yet implemented) |

---

## 4. Protocol Abstraction Opportunities

### 4.1 Current Pattern (Provider-Specific)

```rust
pub enum ProviderKind {
    Mock,
    Claude,
    Codex,
}

pub enum SessionHandle {
    ClaudeSession { session_id: String },
    CodexThread { thread_id: String },
}

pub fn start_provider(
    provider: ProviderKind,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) -> Result<()> {
    match provider {
        ProviderKind::Mock => start_mock_provider(prompt, event_tx),
        ProviderKind::Claude => providers::claude::start(...),
        ProviderKind::Codex => providers::codex::start(...),
    }
}
```

### 4.2 Proposed Pattern (Trait-Based)

```rust
/// Provider trait for protocol abstraction
pub trait Provider: Send + Sync {
    /// Provider identifier
    fn kind(&self) -> ProviderKind;
    
    /// Start a provider session
    fn start(
        &self,
        prompt: String,
        cwd: PathBuf,
        session: Option<SessionHandle>,
        event_tx: Sender<ProviderEvent>,
    ) -> Result<()>;
    
    /// Check if provider is available
    fn is_available(&self) -> bool;
    
    /// Resolve executable path
    fn executable_path(&self) -> Result<PathBuf>;
}

/// Session handle trait for unified handling
pub trait SessionContinuity {
    fn id(&self) -> &str;
    fn provider_kind(&self) -> ProviderKind;
}
```

**Benefits:**

1. New providers only implement trait
2. Protocol-specific logic encapsulated
3. Testability improved (mock providers implement trait)
4. Session handling unified

**Costs:**

1. Requires refactoring existing providers
2. Adds complexity to provider.rs
3. Migration effort for existing code

---

## 5. Integration Options

### Option A: Minimal Integration (Recommended)

Add OpenCode as a new provider module without major refactoring.

**Steps:**

1. Add `ProviderKind::OpenCode`
2. Add `SessionHandle::OpenCodeSession { session_id: String }`
3. Create `providers/opencode.rs` with ACP protocol
4. Add `OPENCODE_PATH_ENV` for executable resolution
5. Implement ACP session negotiation
6. Update `probe.rs` for OpenCode detection

**Timeline:** 2-3 days

**Risk:** Low - follows existing pattern

---

### Option B: Trait-Based Refactoring

Abstract provider interface for future extensibility.

**Steps:**

1. Define `Provider` trait
2. Define `SessionContinuity` trait
3. Refactor Claude provider to implement trait
4. Refactor Codex provider to implement trait
5. Add OpenCode provider implementing trait
6. Update all provider usage to trait interface

**Timeline:** 5-7 days

**Risk:** Medium - requires refactoring working code

---

### Option C: Protocol Adapter Pattern

Create protocol-specific adapters that translate to unified events.

**Steps:**

1. Define `ProtocolAdapter` trait
2. Create `StreamJsonAdapter` for Claude
3. Create `JsonRpcAdapter` for Codex
4. Create `ACPAdapter` for OpenCode
5. Unified event emission from adapters

**Timeline:** 4-5 days

**Risk:** Medium - new abstraction layer

---

## 6. Recommendation

**Option A (Minimal Integration)** is recommended for initial OpenCode support.

**Reasons:**

1. Existing architecture handles two providers well
2. Adding third provider follows established pattern
3. Low risk, fast implementation
4. Can refactor later if more providers needed

**Future Consideration:**

If 4+ providers needed, consider Option B (trait-based) for maintainability.

---

## 7. OpenCode ACP Implementation Details

### 7.1 Initialization Flow

```
1. Start opencode acp process
2. Send initialize request
   Request: {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}
   Response: {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":1,"capabilities":{...}}}
3. Receive capabilities
```

### 7.2 Session Creation

```
Request: {"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/path/to/project"}}
Response: {"jsonrpc":"2.0","id":2,"result":{"sessionId":"sess-123"}}
```

### 7.3 Prompt Sending

```
Request: {"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"sess-123","content":[{"type":"text","text":"write tests"}]}}
Response: {"jsonrpc":"2.0","id":3,"result":{"stopReason":"end"}}
```

### 7.4 Session Resume

```
Request: {"jsonrpc":"2.0","id":2,"method":"session/load","params":{"sessionId":"sess-123"}}
Response: {"jsonrpc":"2.0","id":2,"result":{"sessionId":"sess-123"}}
```

### 7.5 Event Notifications

ACP supports notifications for streaming (future):

```
Notification: {"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"sess-123","update":{"type":"text","content":"..."}}}
```

---

## 8. Key Differences from Codex/Claude

| Feature | Claude | Codex | OpenCode |
|---------|--------|-------|----------|
| Protocol | stream-json | JSON-RPC (proprietary) | JSON-RPC (ACP standard) |
| Initialization | No explicit handshake | `initialize` method | `initialize` method |
| Thread/Session | `session_id` | `thread_id` | `sessionID` |
| Multi-turn | `--resume` flag | `thread/resume` method | `session/load` method |
| Approval | No approval system | `requestApproval` methods | `permission.asked` notification |
| Tool streaming | Embedded in message | `item/agentMessage/delta` | `session/update` (future) |
| CWD handling | Implicit | `thread/start` param | `session/new` param |
| MCP support | Via claude config | Via config | `session/new` param |

---

## 9. Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `core/src/providers/opencode.rs` | ACP protocol implementation |
| `docs/plan/opencode/integration-plan.md` | Implementation plan |

### Modified Files

| File | Changes |
|------|---------|
| `core/src/provider.rs` | Add `ProviderKind::OpenCode`, `SessionHandle::OpenCodeSession` |
| `core/src/providers/mod.rs` | Export `opencode` module |
| `core/src/probe.rs` | Add OpenCode availability check |
| `README.md` | Document OpenCode provider |

---

## 10. References

- OpenCode GitHub: https://github.com/anomalyco/opencode
- OpenCode ACP README: `packages/opencode/src/acp/README.md`
- ACP Specification: https://agentclientprotocol.com/
- ACP TypeScript SDK: https://github.com/agentclientprotocol/typescript-sdk