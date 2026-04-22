# Provider Profile Configuration

Provider Profiles allow you to define named, reusable configurations for different LLM backends. Each profile specifies which CLI to use, environment variables, and CLI arguments.

## Profile Storage

Profiles are stored in `~/.agile-agent/profiles.json`. The file is created automatically on first startup if it doesn't exist.

## Configuration Structure

```json
{
  "profiles": {
    "profile-id": {
      "base_cli": "claude",
      "env_overrides": {
        "ANTHROPIC_API_KEY": "${GLM_API_KEY}",
        "ANTHROPIC_BASE_URL": "https://api.minimax.chat/v1"
      },
      "extra_args": [],
      "display_name": "My Profile",
      "description": "Optional description",
      "icon": null
    }
  },
  "default_work_profile": "profile-id",
  "default_decision_profile": "profile-id"
}
```

## CliBaseType

The `base_cli` field specifies which CLI executable to use:

| Value | CLI | Notes |
|-------|-----|-------|
| `mock` | Mock provider | For testing only |
| `claude` | Claude CLI | Primary provider |
| `codex` | Codex CLI | OpenAI Codex |
| `opencode` | OpenCode CLI | Open-source alternative |

## Environment Variable Interpolation

Use `${ENV_VAR}` syntax to reference environment variables from your shell:

```json
{
  "profiles": {
    "claude-by-glm": {
      "base_cli": "claude",
      "env_overrides": {
        "ANTHROPIC_API_KEY": "${GLM_API_KEY}",
        "ANTHROPIC_BASE_URL": "https://open.bigmodel.cn/api/paas/v4"
      },
      "display_name": "Claude via GLM"
    },
    "claude-by-minimax": {
      "base_cli": "claude",
      "env_overrides": {
        "ANTHROPIC_API_KEY": "${MINIMAX_API_KEY}",
        "ANTHROPIC_BASE_URL": "https://api.minimax.chat/v1",
        "ANTHROPIC_MODEL": "MiniMax-Text-01"
      },
      "extra_args": ["--model", "MiniMax-Text-01"],
      "display_name": "Claude via MiniMax"
    }
  }
}
```

### How Interpolation Works

1. At agent startup, `${ENV_VAR}` is replaced with the actual value from your shell environment
2. Only `${VAR}` syntax is supported (no nested `${${VAR}}`)
3. If an env var is not set, the literal string `${VAR}` is kept

### Security Notes

- Actual API keys never stored in profiles.json — only references
- Values always resolved from your current shell environment
- This allows different keys per terminal session

## Profile Examples

### 1. Default Claude Profile

Auto-created if Claude CLI is detected:

```json
{
  "profiles": {
    "claude-default": {
      "base_cli": "claude",
      "env_overrides": {},
      "extra_args": [],
      "display_name": "Claude CLI (Default)",
      "description": "Standard Claude CLI configuration"
    }
  },
  "default_work_profile": "claude-default",
  "default_decision_profile": "claude-default"
}
```

### 2. Custom Backend with GLM

```json
{
  "profiles": {
    "claude-glm-5": {
      "base_cli": "claude",
      "env_overrides": {
        "ANTHROPIC_API_KEY": "${GLM_API_KEY}",
        "ANTHROPIC_BASE_URL": "https://open.bigmodel.cn/api/paas/v4",
        "ANTHROPIC_MODEL": "glm-5"
      },
      "extra_args": [],
      "display_name": "Claude via GLM-5",
      "description": "Use GLM-5 as the backend via Claude CLI"
    }
  },
  "default_work_profile": "claude-glm-5",
  "default_decision_profile": "claude-glm-5"
}
```

### 3. Multi-Provider Setup

```json
{
  "profiles": {
    "claude-default": {
      "base_cli": "claude",
      "env_overrides": {},
      "display_name": "Claude (Default)"
    },
    "claude-glm": {
      "base_cli": "claude",
      "env_overrides": {
        "ANTHROPIC_API_KEY": "${GLM_API_KEY}",
        "ANTHROPIC_BASE_URL": "https://open.bigmodel.cn/api/paas/v4"
      },
      "display_name": "Claude via GLM"
    },
    "claude-minimax": {
      "base_cli": "claude",
      "env_overrides": {
        "ANTHROPIC_API_KEY": "${MINIMAX_API_KEY}",
        "ANTHROPIC_BASE_URL": "https://api.minimax.chat/v1"
      },
      "display_name": "Claude via MiniMax"
    },
    "codex-default": {
      "base_cli": "codex",
      "env_overrides": {},
      "display_name": "Codex (Default)"
    }
  },
  "default_work_profile": "claude-default",
  "default_decision_profile": "claude-default"
}
```

### 4. Extra CLI Arguments

```json
{
  "profiles": {
    "claude-fast": {
      "base_cli": "claude",
      "env_overrides": {},
      "extra_args": ["--no-input", "--verbose"],
      "display_name": "Claude (Fast)"
    }
  }
}
```

## Managing Profiles

### Auto-Detection

On first startup, the system:
1. Checks PATH for available CLI tools (claude, codex, opencode)
2. Creates default profiles for each detected CLI
3. Always includes `mock-default` for testing
4. Sets the first detected CLI as default

### Manual Editing

Edit `~/.agile-agent/profiles.json` directly:

```bash
vim ~/.agile-agent/profiles.json
```

After editing, restart the TUI for changes to take effect.

### Profile Validation

When loading, profiles are validated:
- Profile ID must exist if referenced as default
- Base CLI type must be valid
- Invalid profiles are logged but don't cause errors

## Environment Variables Reference

Common environment variables used by profiles:

| Variable | Description | Example |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | API key for Anthropic-compatible endpoints | `sk-...` |
| `ANTHROPIC_BASE_URL` | Custom API endpoint | `https://api.minimax.chat/v1` |
| `ANTHROPIC_MODEL` | Model to use | `MiniMax-Text-01`, `glm-5` |
| `OPENAI_API_KEY` | API key for OpenAI/Codex | `sk-...` |
| `OPENAI_BASE_URL` | Custom endpoint for Codex | `https://api.openai.com/v1` |

## Profile Selection

Profiles are selected when spawning an agent:

- **Default profiles**: Used when `spawn_agent()` is called without explicit profile
- **Named profiles**: Used when `spawn_agent_with_profile("profile-id")` is called
- **Decision layer**: Uses `default_decision_profile` unless overridden

The TUI integration (Sprint 4) will provide a profile selector UI for interactive selection.
