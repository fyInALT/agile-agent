# Unified ACP Provider Integration Plan

## Metadata

- Date: 2026-04-13
- Project: agile-agent
- Target: OpenCode + Kimi-CLI Provider Support
- Status: Draft
- Language: English

## Key Insight

**OpenCode and Kimi-CLI both use ACP (Agent Client Protocol).**

Instead of creating two separate implementations, we create **one unified ACP provider** that works for both.

---

## Sprint Overview (Unified)

| Sprint | Title | Focus | Stories | Est. Days |
|--------|-------|-------|---------|-----------|
| Sprint U1 | Foundation | ProviderKind enum & detection | 4 | 1 |
| Sprint U2 | ACP Protocol | Unified ACP implementation | 5 | 2 |
| Sprint U3 | Event Mapping | ProviderEvent translation | 3 | 1 |
| Sprint U4 | Integration | Full provider integration | 3 | 1 |

**Total Estimated**: ~5 days (covers both OpenCode and Kimi-CLI)

---

## Sprint U1: Foundation

### Metadata

- Sprint ID: `sprint-u1`
- Title: `Foundation`
- Duration: 1 day
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Add OpenCode and Kimi to provider system with detection and unified config.

### Story U1.1: ProviderKind Extension (Both)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U1.1.1 | Add `ProviderKind::OpenCode` to enum | Todo | - |
| T-U1.1.2 | Add `ProviderKind::Kimi` to enum | Todo | - |
| T-U1.1.3 | Update `label()` for both | Todo | - |
| T-U1.1.4 | Update `next()` cycle order | Todo | - |
| T-U1.1.5 | Update `all()` array | Todo | - |
| T-U1.1.6 | Write unit tests | Todo | - |

---

### Story U1.2: SessionHandle Extension (Both)

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U1.2.1 | Add `SessionHandle::OpenCodeSession { session_id }` | Todo | - |
| T-U1.2.2 | Add `SessionHandle::KimiSession { session_id }` | Todo | - |
| T-U1.2.3 | Write unit tests | Todo | - |

---

### Story U1.3: Provider Detection (Both)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U1.3.1 | Add `OPENCODE_PATH_ENV` constant | Todo | - |
| T-U1.3.2 | Add `KIMI_PATH_ENV` constant | Todo | - |
| T-U1.3.3 | Add OpenCode availability check | Todo | - |
| T-U1.3.4 | Add Kimi availability check | Todo | - |
| T-U1.3.5 | Update `default_provider()` order | Todo | - |
| T-U1.3.6 | Write tests for detection | Todo | - |

---

### Story U1.4: ACP Provider Config

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U1.4.1 | Create `ACPProviderConfig` struct | Todo | - |
| T-U1.4.2 | Define config for OpenCode | Todo | - |
| T-U1.4.3 | Define config for Kimi | Todo | - |
| T-U1.4.4 | Write tests for config | Todo | - |

#### Technical Notes

```rust
pub struct ACPProviderConfig {
    executable: String,        // "opencode" or "kimi"
    path_env: String,          // "AGILE_AGENT_OPENCODE_PATH" or "AGILE_AGENT_KIMI_PATH"
    label: String,             // "opencode" or "kimi"
    requires_auth: bool,       // false for OpenCode, true for Kimi
}

pub const OPENCODE_CONFIG: ACPProviderConfig = ACPProviderConfig {
    executable: "opencode",
    path_env: "AGILE_AGENT_OPENCODE_PATH",
    label: "opencode",
    requires_auth: false,
};

pub const KIMI_CONFIG: ACPProviderConfig = ACPProviderConfig {
    executable: "kimi",
    path_env: "AGILE_AGENT_KIMI_PATH",
    label: "kimi",
    requires_auth: true,
};
```

---

## Sprint U2: ACP Protocol Implementation

### Metadata

- Sprint ID: `sprint-u2`
- Title: `ACP Protocol`
- Duration: 2 days
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Implement unified ACP protocol that works for both OpenCode and Kimi-CLI.

### Story U2.1: ACP Message Types

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U2.1.1 | Create `ACPRequest` struct | Todo | - |
| T-U2.1.2 | Create `ACPResponse` struct | Todo | - |
| T-U2.1.3 | Create `ACPNotification` struct | Todo | - |
| T-U2.1.4 | Create `ACPError` struct | Todo | - |
| T-U2.1.5 | Create capability structs | Todo | - |
| T-U2.1.6 | Write unit tests | Todo | - |

---

### Story U2.2: ACP Process Lifecycle (Unified)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U2.2.1 | Create `resolve_acp_executable()` with config param | Todo | - |
| T-U2.2.2 | Spawn process with `{executable} acp` | Todo | - |
| T-U2.2.3 | Handle stdin/stdout/stderr pipes | Todo | - |
| T-U2.2.4 | Implement graceful shutdown | Todo | - |
| T-U2.2.5 | Add lifecycle logging | Todo | - |
| T-U2.2.6 | Write tests for both executables | Todo | - |

---

### Story U2.3: Initialize Handshake (Unified)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U2.3.1 | Send `initialize` request | Todo | - |
| T-U2.3.2 | Parse response capabilities | Todo | - |
| T-U2.3.3 | Handle `AUTH_REQUIRED` error (Kimi) | Todo | - |
| T-U2.3.4 | Handle protocol version negotiation | Todo | - |
| T-U2.3.5 | Write tests for handshake | Todo | - |

---

### Story U2.4: Session Management (Unified)

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U2.4.1 | Implement `session/new` request | Todo | - |
| T-U2.4.2 | Parse `session/new` response | Todo | - |
| T-U2.4.3 | Implement `session/load` request | Todo | - |
| T-U2.4.4 | Emit appropriate `SessionHandle` variant | Todo | - |
| T-U2.4.5 | Store session for multi-turn | Todo | - |
| T-U2.4.6 | Write tests for session management | Todo | - |

---

### Story U2.5: Prompt Sending (Unified)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U2.5.1 | Build `session/prompt` request | Todo | - |
| T-U2.5.2 | Send prompt request | Todo | - |
| T-U2.5.3 | Handle prompt response | Todo | - |
| T-U2.5.4 | Write tests for prompt | Todo | - |

---

## Sprint U3: Event Mapping

### Metadata

- Sprint ID: `sprint-u3`
- Title: `Event Mapping`
- Duration: 1 day
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Translate ACP notifications to ProviderEvent (works for both providers).

### Story U3.1: ACP Notification Parsing (Unified)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U3.1.1 | Parse `session/update` notification | Todo | - |
| T-U3.1.2 | Parse text content | Todo | - |
| T-U3.1.3 | Parse reasoning/thinking | Todo | - |
| T-U3.1.4 | Parse tool events | Todo | - |
| T-U3.1.5 | Parse permission events | Todo | - |
| T-U3.1.6 | Write tests for parsing | Todo | - |

---

### Story U3.2: ProviderEvent Translation (Unified)

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U3.2.1 | Map text to `AssistantChunk` | Todo | - |
| T-U3.2.2 | Map reasoning to `ThinkingChunk` | Todo | - |
| T-U3.2.3 | Map tool_use to `GenericToolCallStarted` | Todo | - |
| T-U3.2.4 | Map tool_result to `GenericToolCallFinished` | Todo | - |
| T-U3.2.5 | Map bash to `ExecCommandStarted/Finished` | Todo | - |
| T-U3.2.6 | Map edit to `PatchApplyStarted/Finished` | Todo | - |
| T-U3.2.7 | Write tests for translation | Todo | - |

---

### Story U3.3: Permission Handling (Kimi-specific)

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U3.3.1 | Handle `permission.asked` notification | Todo | - |
| T-U3.3.2 | Auto-approve permissions (for now) | Todo | - |
| T-U3.3.3 | Write tests for permission handling | Todo | - |

---

## Sprint U4: Integration

### Metadata

- Sprint ID: `sprint-u4`
- Title: `Integration`
- Duration: 1 day
- Priority: P0 (Critical)
- Status: Backlog

### Sprint Goal

Complete unified ACP provider integration.

### Story U4.1: Provider Module Integration

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U4.1.1 | Add `acp` module to `providers/mod.rs` | Todo | - |
| T-U4.1.2 | Update `start_provider()` to dispatch ACP providers | Todo | - |
| T-U4.1.3 | Add thread naming for both providers | Todo | - |
| T-U4.1.4 | Write integration tests | Todo | - |

---

### Story U4.2: Doctor Command Update

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U4.2.1 | Add OpenCode to `doctor` output | Todo | - |
| T-U4.2.2 | Add Kimi to `doctor` output | Todo | - |
| T-U4.2.3 | Show availability status for both | Todo | - |
| T-U4.2.4 | Show version for both | Todo | - |
| T-U4.2.5 | Write tests | Todo | - |

---

### Story U4.3: Documentation Update

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T-U4.3.1 | Update README.md with ACP providers section | Todo | - |
| T-U4.3.2 | Add OpenCode configuration docs | Todo | - |
| T-U4.3.3 | Add Kimi configuration docs (including login) | Todo | - |
| T-U4.3.4 | Add `AGILE_AGENT_OPENCODE_PATH` docs | Todo | - |
| T-U4.3.5 | Add `AGILE_AGENT_KIMI_PATH` docs | Todo | - |

---

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ACP protocol version mismatch | Low | Medium | Version negotiation in initialize |
| Kimi authentication required | High | Low | Prompt user to run `kimi login` |
| Different tool naming | Low | Low | ACP uses standard tool names |
| Different error handling | Medium | Low | Unified error translation |

---

## Acceptance Criteria

After Sprint U4, verify:

1. [ ] `opencode` appears in `doctor` output
2. [ ] `kimi` appears in `doctor` output
3. [ ] OpenCode can be selected via provider cycling
4. [ ] Kimi can be selected via provider cycling
5. [ ] `SessionHandle::OpenCodeSession` persists correctly
6. [ ] `SessionHandle::KimiSession` persists correctly
7. [ ] Multi-turn works via `session/load`
8. [ ] Kimi authentication error gracefully handled
9. [ ] All tests pass

---

## Architecture After Implementation

```
provider.rs
├── ProviderKind enum:
│   ├── Mock
│   ├── Claude  → providers/claude.rs
│   ├── Codex   → providers/codex.rs
│   ├── OpenCode → providers/acp.rs (config=OPENCODE_CONFIG)
│   └── Kimi    → providers/acp.rs (config=KIMI_CONFIG)
│
providers/
├── claude.rs (stream-json)
├── codex.rs (JSON-RPC proprietary)
├── acp.rs (unified ACP implementation)
│   ├── ACPProviderConfig
│   ├── ACPProvider
│   ├── initialize()
│   ├── session_new()
│   ├── session_load()
│   ├── prompt()
│   └── notification parsing
└── mod.rs
```

---

## References

- [Kimi-CLI Provider Analysis](./provider-analysis.md)
- [OpenCode Provider Analysis](../opencode/provider-analysis.md)
- ACP Specification: https://agentclientprotocol.com/