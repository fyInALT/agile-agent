# Agent Launch Configuration Guide

## Overview

The Agent Launch Configuration system allows each agent to have its own provider startup settings. When creating a new agent with `Ctrl+N`, you can configure:

- **Work Agent**: The main agent that executes tasks
- **Decision Agent**: An auxiliary agent for making decisions (uses independent configuration)

## Input Syntaxes

### 1. Environment-Only Mode

Pure `KEY=VALUE` format. Each line defines one environment variable.

```
ANTHROPIC_BASE_URL=https://api.minimaxi.com/anthropic
ANTHROPIC_AUTH_TOKEN=sk-xxx
ANTHROPIC_MODEL=MiniMax-M2.7
API_TIMEOUT_MS=3000000
```

**Rules:**
- Every non-empty line must match `KEY=VALUE` format
- Keys must start with a letter or underscore, contain only alphanumeric chars and underscores
- Empty values are not allowed

### 2. Command Fragment Mode

Shell-style command with optional environment prefixes and an executable.

```
ANTHROPIC_MODEL=opus-4 claude --verbose --dangerously-skip-permissions
```

**Rules:**
- The first token that doesn't look like `KEY=VALUE` is the executable
- Environment variables use `KEY=VALUE` format before the executable
- Everything after the executable becomes extra arguments

### 3. Host Default Mode

Leave the input empty. The system will use the host's default environment.

## Configuration Fields

### Work Agent Config

| Field | Description |
|-------|-------------|
| Provider | The provider type (locked from selection) |
| Executable | Path to the provider binary |
| Env Overrides | Custom environment variables |
| Extra Args | Additional command-line arguments |

### Decision Agent Config

Independently configured. If empty, uses host default (does NOT inherit from Work agent).

## Provider-Specific Configuration

### Claude

```text
# Environment-only
ANTHROPIC_BASE_URL=https://api.anthropic.com
ANTHROPIC_AUTH_TOKEN=sk-ant-xxx
ANTHROPIC_MODEL=claude-opus-4-5

# Command fragment
ANTHROPIC_MODEL=claude-opus-4-5 claude --verbose
```

### Codex

```text
# Environment-only
CODEX_API_KEY=sk-xxx
CODEX_BASE_URL=https://api.openai.com
CODEX_MODEL=gpt-4o

# Command fragment
CODEX_MODEL=gpt-4o codex --no-cache
```

### Mock

**No configuration required.** Mock provider skips the config overlay entirely and spawns immediately with default settings.

## Creating an Agent

1. Press `Ctrl+N`
2. Select a provider (Claude, Codex, or Mock)
3. For Claude/Codex: Configure work and decision agents
4. Preview shows parsed configuration
5. Confirm to create the agent

## Resume Behavior

When an agent is restored after shutdown:

1. Uses the **resolved** launch configuration (captured at creation time)
2. Does NOT re-read the current host environment
3. If executable is missing, agent enters `Error` state with visible error message

## Validation Rules

| Error | Cause |
|-------|-------|
| Provider mismatch | Executable in config doesn't match selected provider |
| Tokenization failed | Invalid shell syntax in command fragment |
| Empty executable | No executable found in command fragment |
| Reserved argument conflict | Attempting to override provider-owned protocol arguments |

## Reserved Environment Variables

The following are managed by the provider and cannot be overridden:

**Claude:**
- Stream JSON input/output protocol arguments
- Permission-mode arguments

**Codex:**
- `exec --json` protocol entry arguments

## Environment Variable Inheritance

The system automatically inherits these from the host:

**Whitelist (always):**
- `PATH`, `HOME`, `USER`, `SHELL`, `LANG`, `LC_ALL`

**Provider prefixes (if present):**
- `ANTHROPIC_*`
- `CODEX_*`
- `OPENAI_*`
- `API_*`

## File Storage

Launch configuration is persisted in:

```
<workplace>/agents/<agent-id>/launch-config.json
```

This file contains the full `AgentLaunchBundle` including resolved specs.

## Logging Events

| Event | When |
|-------|------|
| `launch_config.parse.start` | Parse begins |
| `launch_config.parse.success` | Parse completes |
| `launch_config.parse.failed` | Parse fails |
| `launch_config.persist` | Config saved to disk |
| `launch_config.restore.failed` | Restore validation fails |

## Troubleshooting

### Agent stuck in Error state after resume

Check if the executable path in `launch-config.json` still exists. If the binary was moved or uninstalled, the agent cannot start.

### Configuration not taking effect

Ensure the provider binary supports the environment variables you're setting. Some variables may only be read at startup, not dynamically.

### Decision agent using wrong model

Remember: Decision agent configuration is independent. It does NOT inherit from Work agent. Set it explicitly if you need a specific model.
