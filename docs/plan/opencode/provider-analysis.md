# OpenCode Provider Analysis

## Metadata

- Date: 2026-04-13
- Project: agile-agent
- Target: OpenCode Provider Support
- Status: Draft
- Language: English

## 1. OpenCode Overview

OpenCode is an open-source AI coding agent similar to Claude Code and Codex.

**Key URLs:**

- Website: https://opencode.ai
- GitHub: https://github.com/anomalyco/opencode
- npm: `opencode-ai`

---

## 2. Protocol Analysis

### 2.1 ACP Support

**OpenCode uses ACP (Agent Client Protocol).**

```bash
opencode acp [--cwd /path/to/project]
```

**ACP Implementation Files:**

| File | Purpose |
|------|---------|
| `packages/opencode/src/acp/agent.ts` | ACP agent implementation |
| `packages/opencode/src/acp/session.ts` | Session management |
| `packages/opencode/src/acp/types.ts` | Type definitions |
| `packages/opencode/src/cli/cmd/acp.ts` | CLI entrypoint |

### 2.2 ACP README Summary

From `packages/opencode/src/acp/README.md`:

- Implements ACP v1
- Uses `@agentclientprotocol/sdk`
- Session lifecycle: `session/new`, `session/load`
- Working directory via `cwd` parameter
- MCP server configuration support
- Permission handling via `permission.asked`

---

## 3. Protocol Comparison

| Provider | Language | Protocol | ACP? |
|----------|----------|----------|------|
| Claude | TypeScript | stream-json | No |
| Codex | TypeScript | JSON-RPC (proprietary) | No |
| OpenCode | TypeScript/Go | **ACP** | **Yes** |
| Kimi-CLI | Python | **ACP** | **Yes** |

**Key Insight: OpenCode and Kimi-CLI share ACP protocol.**

See [Kimi-CLI Provider Analysis](../kimi-cli/provider-analysis.md) for unified implementation strategy.

---

## 4. ACP Protocol Details

### 4.1 Initialization

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}
```

### 4.2 Session Creation

```json
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/path"}}
```

### 4.3 Session Resume

```json
{"jsonrpc":"2.0","id":2,"method":"session/load","params":{"sessionId":"sess-xxx"}}
```

### 4.4 Prompt

```json
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"sess-xxx","content":[{"type":"text","text":"..."}]}}
```

---

## 5. Unified ACP Architecture

**Recommendation: Create single `providers/acp.rs` for both OpenCode and Kimi.**

See [Kimi-CLI Integration Plan](../kimi-cli/integration-plan.md) for unified sprint breakdown.

---

## 6. OpenCode-Specific Considerations

### 6.1 No Authentication Required

OpenCode ACP does not require login (unlike Kimi-CLI).

### 6.2 Binary Distribution

OpenCode is distributed as a compiled binary (Go), not npm package.

### 6.3 Detection

```bash
which opencode
opencode --version
```

---

## 7. Configuration

```rust
pub const OPENCODE_CONFIG: ACPProviderConfig = ACPProviderConfig {
    executable: "opencode",
    path_env: "AGILE_AGENT_OPENCODE_PATH",
    label: "opencode",
    requires_auth: false,
};
```

---

## 8. References

- OpenCode GitHub: https://github.com/anomalyco/opencode
- OpenCode ACP README: `packages/opencode/src/acp/README.md`
- ACP Specification: https://agentclientprotocol.com/
- [Kimi-CLI Provider Analysis](../kimi-cli/provider-analysis.md) (unified strategy)
- [Unified ACP Integration Plan](../kimi-cli/integration-plan.md)