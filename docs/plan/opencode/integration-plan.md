# OpenCode Provider Integration Plan

## Metadata

- Date: 2026-04-13
- Project: agile-agent
- Target: OpenCode Provider Support
- Status: Draft
- Language: English

## Sprint Overview

| Sprint | Title | Focus | Stories | Est. Days |
|--------|-------|-------|---------|-----------|
| Sprint O1 | Foundation | OpenCode detection & enum | 3 | 1 |
| Sprint O2 | ACP Protocol | Session negotiation | 4 | 2 |
| Sprint O3 | Event Mapping | ProviderEvent translation | 3 | 1 |
| Sprint O4 | Integration | Full provider integration | 3 | 1 |

**Total Estimated**: ~5 days

---

## Sprint O1: Foundation

### Metadata

- Sprint ID: `sprint-o1`
- Title: `Foundation`
- Duration: 1 day
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Add OpenCode to provider system with detection and enum support.

### Story O1.1: ProviderKind Extension

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O1.1.1 | Add `ProviderKind::OpenCode` to enum | Todo | - |
| T-O1.1.2 | Update `label()` to return "opencode" | Todo | - |
| T-O1.1.3 | Update `next()` to include OpenCode cycle | Todo | - |
| T-O1.1.4 | Update `all()` to include OpenCode | Todo | - |
| T-O1.1.5 | Write unit tests for enum extension | Todo | - |

---

### Story O1.2: SessionHandle Extension

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O1.2.1 | Add `SessionHandle::OpenCodeSession { session_id: String }` | Todo | - |
| T-O1.2.2 | Ensure serialization works | Todo | - |
| T-O1.2.3 | Write unit tests for session handle | Todo | - |

---

### Story O1.3: OpenCode Detection

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O1.3.1 | Add `OPENCODE_PATH_ENV` constant | Todo | - |
| T-O1.3.2 | Add `is_provider_available("opencode")` check | Todo | - |
| T-O1.3.3 | Update `default_provider()` to check OpenCode | Todo | - |
| T-O1.3.4 | Write tests for availability detection | Todo | - |

---

## Sprint O2: ACP Protocol Implementation

### Metadata

- Sprint ID: `sprint-o2`
- Title: `ACP Protocol`
- Duration: 2 days
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Implement ACP (Agent Client Protocol) for OpenCode communication.

### Story O2.1: ACP Message Types

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O2.1.1 | Create `ACPRequest` struct | Todo | - |
| T-O2.1.2 | Create `ACPResponse` struct | Todo | - |
| T-O2.1.3 | Create `ACPNotification` struct | Todo | - |
| T-O2.1.4 | Create `InitializeParams` struct | Todo | - |
| T-O2.1.5 | Create `SessionNewParams` struct | Todo | - |
| T-O2.1.6 | Create `SessionLoadParams` struct | Todo | - |
| T-O2.1.7 | Create `SessionPromptParams` struct | Todo | - |
| T-O2.1.8 | Write unit tests for message types | Todo | - |

#### Technical Notes

```rust
#[derive(Debug, Serialize)]
struct ACPRequest {
    jsonrpc: &'static str,  // "2.0"
    id: u64,
    method: String,
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ACPResponse {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<ACPError>,
}

#[derive(Debug, Deserialize)]
struct ACPError {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}
```

---

### Story O2.2: OpenCode Process Lifecycle

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O2.2.1 | Implement `resolve_opencode_executable()` | Todo | - |
| T-O2.2.2 | Implement process spawn with `acp` args | Todo | - |
| T-O2.2.3 | Handle stdin/stdout/stderr pipes | Todo | - |
| T-O2.2.4 | Implement graceful shutdown | Todo | - |
| T-O2.2.5 | Add lifecycle logging | Todo | - |
| T-O2.2.6 | Write tests for process lifecycle | Todo | - |

#### Process Arguments

```bash
opencode acp --cwd /path/to/project
```

---

### Story O2.3: Initialize Handshake

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O2.3.1 | Send `initialize` request | Todo | - |
| T-O2.3.2 | Parse `initialize` response | Todo | - |
| T-O2.3.3 | Extract capabilities | Todo | - |
| T-O2.3.4 | Handle protocol version mismatch | Todo | - |
| T-O2.3.5 | Write tests for handshake | Todo | - |

#### Initialize Request

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}
```

---

### Story O2.4: Session Management

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O2.4.1 | Implement `session/new` request | Todo | - |
| T-O2.4.2 | Parse `session/new` response | Todo | - |
| T-O2.4.3 | Implement `session/load` request | Todo | - |
| T-O2.4.4 | Parse `session/load` response | Todo | - |
| T-O2.4.5 | Emit `SessionHandle` event | Todo | - |
| T-O2.4.6 | Write tests for session management | Todo | - |

#### Session New Request

```json
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/path/to/project"}}
```

#### Session Load Request

```json
{"jsonrpc":"2.0","id":2,"method":"session/load","params":{"sessionId":"sess-123"}}
```

---

## Sprint O3: Event Mapping

### Metadata

- Sprint ID: `sprint-o3`
- Title: `Event Mapping`
- Duration: 1 day
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Translate ACP notifications to ProviderEvent.

### Story O3.1: ACP Notification Parsing

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O3.1.1 | Parse `session/update` notification | Todo | - |
| T-O3.1.2 | Parse text content updates | Todo | - |
| T-O3.1.3 | Parse tool call events | Todo | - |
| T-O3.1.4 | Parse error notifications | Todo | - |
| T-O3.1.5 | Parse permission notifications | Todo | - |
| T-O3.1.6 | Write tests for notification parsing | Todo | - |

---

### Story O3.2: ProviderEvent Translation

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O3.2.1 | Map text to `AssistantChunk` | Todo | - |
| T-O3.2.2 | Map reasoning to `ThinkingChunk` | Todo | - |
| T-O3.2.3 | Map tool_use to `GenericToolCallStarted` | Todo | - |
| T-O3.2.4 | Map tool_result to `GenericToolCallFinished` | Todo | - |
| T-O3.2.5 | Map bash to `ExecCommandStarted/Finished` | Todo | - |
| T-O3.2.6 | Map edit to `PatchApplyStarted/Finished` | Todo | - |
| T-O3.2.7 | Write tests for event translation | Todo | - |

---

### Story O3.3: Prompt Sending

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O3.3.1 | Build `session/prompt` request | Todo | - |
| T-O3.3.2 | Send prompt request | Todo | - |
| T-O3.3.3 | Handle prompt response | Todo | - |
| T-O3.3.4 | Write tests for prompt sending | Todo | - |

#### Prompt Request

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session/prompt",
  "params": {
    "sessionId": "sess-123",
    "content": [{"type": "text", "text": "write tests"}]
  }
}
```

---

## Sprint O4: Integration

### Metadata

- Sprint ID: `sprint-o4`
- Title: `Integration`
- Duration: 1 day
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Complete OpenCode provider integration with tests.

### Story O4.1: Provider Module Integration

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O4.1.1 | Add `opencode` module to `providers/mod.rs` | Todo | - |
| T-O4.1.2 | Update `start_provider()` to dispatch OpenCode | Todo | - |
| T-O4.1.3 | Ensure proper thread naming | Todo | - |
| T-O4.1.4 | Write module integration tests | Todo | - |

---

### Story O4.2: Doctor Command Update

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O4.2.1 | Add OpenCode to `doctor` output | Todo | - |
| T-O4.2.2 | Show OpenCode availability status | Todo | - |
| T-O4.2.3 | Show OpenCode version | Todo | - |
| T-O4.2.4 | Write tests for doctor output | Todo | - |

---

### Story O4.3: Documentation Update

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-O4.3.1 | Update README.md provider section | Todo | - |
| T-O4.3.2 | Add OpenCode configuration docs | Todo | - |
| T-O4.3.3 | Add `AGILE_AGENT_OPENCODE_PATH` docs | Todo | - |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ACP protocol changes | Low | Medium | Use stable ACP v1 |
| OpenCode CLI not available | Medium | Low | Graceful fallback to next provider |
| Session streaming incomplete | Medium | Medium | ACP spec says streaming is future feature - use polling |
| Permission handling differs | Low | Low | OpenCode uses permission.asked, auto-approve for now |

---

## Acceptance Criteria

After Sprint O4, verify:

1. [ ] `opencode` appears in `doctor` output
2. [ ] OpenCode can be selected via provider cycling
3. [ ] `SessionHandle::OpenCodeSession` persists correctly
4. [ ] Multi-turn conversation works via `session/load`
5. [ ] Tool events map to correct `ProviderEvent` variants
6. [ ] All tests pass

---

## Dependencies

- No dependencies on multi-agent sprints
- Can be implemented independently
- Uses same thread-based execution pattern as Codex

---

## References

- [Provider Analysis](./provider-analysis.md)
- OpenCode ACP README: `packages/opencode/src/acp/README.md`
- ACP Specification: https://agentclientprotocol.com/