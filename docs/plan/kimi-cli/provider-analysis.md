# Kimi-CLI Provider Analysis

## Metadata

- Date: 2026-04-13
- Project: agile-agent
- Target: Kimi-CLI Provider Support
- Status: Draft
- Language: English

## 1. Kimi-CLI Overview

Kimi-CLI is Moonshot AI's CLI coding agent, similar to Claude Code, Codex, and OpenCode.

**Key URLs:**

- Website: https://www.kimi.com/code/
- GitHub: https://github.com/MoonshotAI/kimi-cli
- PyPI: `kimi-cli`
- Documentation: https://moonshotai.github.io/kimi-cli/

**Key Features:**

- Shell command mode (Ctrl-X switch)
- VS Code extension integration
- ACP (Agent Client Protocol) support
- MCP tool support
- Zsh integration plugin

---

## 2. Protocol Analysis

### 2.1 ACP Support (Critical Finding!)

**Kimi-CLI uses ACP (Agent Client Protocol) - same as OpenCode!**

```toml
# pyproject.toml
dependencies = [
    "agent-client-protocol==0.8.0",
    ...
]
```

**ACP Command:**

```bash
kimi acp
```

**ACP Version:**

```python
# src/kimi_cli/acp/version.py
CURRENT_VERSION = ACPVersionSpec(
    protocol_version=1,
    spec_tag="v0.10.8",
    sdk_version="0.8.0",
)
```

### 2.2 Key Architecture Files

| File | Purpose |
|------|---------|
| `acp/server.py` | ACP server implementation |
| `acp/session.py` | ACP session management |
| `acp/version.py` | Protocol version negotiation |
| `acp/convert.py` | Content conversion |
| `acp/tools.py` | Tool mapping |
| `cli/__init__.py` | CLI entrypoint with `acp` subcommand |

---

## 3. Protocol Comparison Summary

| Provider | Language | Protocol | Transport | ACP? |
|----------|----------|----------|-----------|------|
| Claude | TypeScript | stream-json | stdin/stdout | No |
| Codex | TypeScript | JSON-RPC (proprietary) | stdin/stdout | No |
| OpenCode | TypeScript/Go | ACP | stdin/stdout | **Yes** |
| Kimi-CLI | Python | ACP | stdin/stdout | **Yes** |

**Key Insight: OpenCode and Kimi-CLI share the same protocol (ACP)!**

---

## 4. ACP Protocol Details (Shared Between OpenCode & Kimi-CLI)

### 4.1 Initialization

```json
// Request
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}

// Response
{
  "jsonrpc":"2.0",
  "id":1,
  "result":{
    "protocolVersion":1,
    "agentCapabilities":{
      "loadSession":true,
      "promptCapabilities":{"embeddedContext":true,"image":true},
      "mcpCapabilities":{"http":true},
      "sessionCapabilities":{"list":true,"resume":true}
    },
    "agentInfo":{"name":"Kimi Code CLI","version":"..."}
  }
}
```

### 4.2 Session Creation

```json
// Request
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/path/to/project"}}

// Response
{"jsonrpc":"2.0","id":2,"result":{"sessionId":"sess-xxx"}}
```

### 4.3 Session Resume

```json
// Request
{"jsonrpc":"2.0","id":2,"method":"session/load","params":{"sessionId":"sess-xxx"}}
```

### 4.4 Prompt

```json
// Request
{
  "jsonrpc":"2.0",
  "id":3,
  "method":"session/prompt",
  "params":{
    "sessionId":"sess-xxx",
    "content":[{"type":"text","text":"write tests"}]
  }
}
```

---

## 5. Unified ACP Provider Architecture

### 5.1 Current Architecture Limitation

```
ProviderKind enum:
├── Mock
├── Claude  → providers/claude.rs (stream-json)
├── Codex   → providers/codex.rs (JSON-RPC proprietary)
└── (OpenCode) → providers/opencode.rs (ACP) [PLANNED]
└── (Kimi)    → providers/kimi.rs (ACP) [PLANNED - REDUNDANT!]
```

**Problem**: Creating two separate ACP implementations is redundant!

### 5.2 Proposed Unified Architecture

```
ProviderKind enum:
├── Mock
├── Claude  → providers/claude.rs (stream-json)
├── Codex   → providers/codex.rs (JSON-RPC proprietary)
├── OpenCode → providers/acp.rs (ACP with executable="opencode")
└── Kimi     → providers/acp.rs (ACP with executable="kimi")
```

**Benefit**: Single ACP implementation, parameterized by executable.

### 5.3 ACP Provider Trait Design

```rust
/// ACP-based provider configuration
pub struct ACPProviderConfig {
    /// Executable name or path
    executable: String,
    
    /// Environment variable for custom path
    path_env: String,
    
    /// Provider label
    label: String,
}

/// Unified ACP provider implementation
pub struct ACPProvider {
    config: ACPProviderConfig,
}

impl ACPProvider {
    pub fn start(
        &self,
        prompt: String,
        cwd: PathBuf,
        session: Option<SessionHandle>,
        event_tx: Sender<ProviderEvent>,
    ) -> Result<()> {
        // 1. Resolve executable path
        // 2. Spawn process: {executable} acp
        // 3. Initialize handshake
        // 4. Create/resume session
        // 5. Send prompt
        // 6. Parse notifications → ProviderEvent
    }
}
```

---

## 6. Implementation Strategy

### Option A: Unified ACP Provider (Recommended)

Create single `providers/acp.rs` that works for both OpenCode and Kimi.

**Steps:**

1. Create `ACPProviderConfig` struct
2. Create `providers/acp.rs` with unified ACP implementation
3. Add `ProviderKind::OpenCode` and `ProviderKind::Kimi`
4. Both dispatch to same `acp::start()` with different configs
5. Add `OPENCODE_PATH_ENV` and `KIMI_PATH_ENV`
6. Update `probe.rs` for both providers

**Benefits:**

- Code reuse (no duplication)
- Easier maintenance
- Future ACP providers trivially supported

**Timeline:** 3-4 days (same as OpenCode alone, but covers both)

---

### Option B: Separate Implementations

Create separate `providers/opencode.rs` and `providers/kimi.rs`.

**Costs:**

- Duplicate code
- Maintenance burden
- Inconsistent behavior risk

**Not recommended.**

---

## 7. Differences Between OpenCode and Kimi-CLI ACP

| Aspect | OpenCode | Kimi-CLI |
|--------|----------|----------|
| Language | TypeScript/Go (binary) | Python |
| ACP Version | v0.10.8 (sdk 0.8.0) | v0.10.8 (sdk 0.8.0) |
| Authentication | Optional | Required (OAuth login first) |
| Tool names | Standard ACP tools | Standard ACP tools |
| MCP support | Via config | Via config + CLI |
| Additional dirs | Not in ACP params | `additional_dirs` support |

**Key Difference: Kimi requires authentication before ACP session**

```python
# kimi-cli: authentication check
def _check_auth(self) -> None:
    """Check if Kimi Code authentication is complete. Raise AUTH_REQUIRED if not."""
    reason = self._check_token_usable()
    if reason:
        raise acp.RequestError.auth_required({"authMethods": auth_methods_data})
```

**Recommendation**: Gracefully handle `AUTH_REQUIRED` error by prompting user to run `kimi login`.

---

## 8. Session Handle Unification

### 8.1 Current Session Handles

```rust
pub enum SessionHandle {
    ClaudeSession { session_id: String },
    CodexThread { thread_id: String },
}
```

### 8.2 Proposed Extension

```rust
pub enum SessionHandle {
    ClaudeSession { session_id: String },
    CodexThread { thread_id: String },
    ACPSession { session_id: String, provider: ACPProviderKind },
}

pub enum ACPProviderKind {
    OpenCode,
    Kimi,
}
```

**Or simpler:**

```rust
pub enum SessionHandle {
    ClaudeSession { session_id: String },
    CodexThread { thread_id: String },
    OpenCodeSession { session_id: String },
    KimiSession { session_id: String },
}
```

---

## 9. Files to Create/Modify

### New Files

| File | Purpose |
|------|---------|
| `core/src/providers/acp.rs` | Unified ACP protocol implementation |
| `docs/plan/kimi-cli/provider-analysis.md` | This document |
| `docs/plan/kimi-cli/integration-plan.md` | Sprint breakdown |

### Modified Files

| File | Changes |
|------|---------|
| `core/src/provider.rs` | Add `ProviderKind::OpenCode`, `ProviderKind::Kimi`, `SessionHandle` variants |
| `core/src/providers/mod.rs` | Export `acp` module |
| `core/src/probe.rs` | Add detection for both providers |
| `README.md` | Document both providers |

---

## 10. Provider Detection

### OpenCode Detection

```bash
# Check if opencode is installed
which opencode || echo "not found"

# Check version
opencode --version
```

### Kimi-CLI Detection

```bash
# Check if kimi is installed
which kimi || echo "not found"

# Check version
kimi --version
```

---

## 11. Authentication Considerations

### Kimi-CLI OAuth Flow

1. Run `kimi login` in terminal
2. Follow OAuth instructions
3. Token stored in keyring
4. ACP sessions work after login

**Integration Note:**

- If `AUTH_REQUIRED` error, prompt user: "Please run `kimi login` first"
- No need to handle OAuth in agile-agent - delegate to kimi-cli

---

## 12. Summary

| Aspect | Status |
|--------|--------|
| Protocol | ACP (same as OpenCode) |
| Implementation Strategy | Unified ACP provider |
| Session Handle | Separate variants (OpenCodeSession, KimiSession) |
| Authentication | Handle `AUTH_REQUIRED` error |
| Timeline | 3-4 days for both |

---

## 13. References

- Kimi-CLI GitHub: https://github.com/MoonshotAI/kimi-cli
- Kimi-CLI Docs: https://moonshotai.github.io/kimi-cli/
- ACP Specification: https://agentclientprotocol.com/
- OpenCode Provider Analysis: `docs/plan/opencode/provider-analysis.md`