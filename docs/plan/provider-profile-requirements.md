# Provider Profile System Requirements

## Overview

This document defines the requirements for a Provider Profile System that enables users to configure and use different LLM backends through named, reusable profiles.

## Problem Statement

The current provider selection mechanism is fixed:
- Only three provider types: Mock, Claude, Codex
- Each type maps to a single CLI executable
- Users cannot configure different LLM backends (e.g., Claude with GLM-5, Claude with MiniMax)
- Every agent creation requires manual environment configuration

Users want to:
- Define named profiles like "claude-by-glm-5" or "claude-by-minimax"
- Reuse profiles across multiple agent creations
- Have separate profiles for work agents and decision layer
- Store profiles persistently at both global and workplace levels

## Goals

1. **Profile Definition**: Allow users to define named provider profiles with custom environment variables and CLI arguments
2. **Profile Selection**: Enable selecting profiles when creating agents
3. **Dual-Level Storage**: Support both global and workplace-specific profiles
4. **Decision Layer Support**: Allow independent profile selection for decision agents
5. **Backward Compatibility**: Existing ProviderKind-based agent creation must continue to work
6. **Security**: Only environment variable references (${ENV_VAR}) allowed, no direct value storage

## Non-Goals

- Profile versioning or migration
- Profile sharing between users
- Profile encryption (host environment handles security)
- Dynamic profile switching for running agents

## User Stories

### US-1: Define Provider Profile

**As a** user, **I want to** define a provider profile with a name, CLI type, and environment variables, **so that** I can reuse this configuration when creating agents.

**Acceptance Criteria**:
- Profile has a unique identifier (user-defined string)
- Profile specifies CLI base type (Claude, Codex, etc.)
- Profile contains environment variable overrides (using ${ENV_VAR} syntax)
- Profile can have optional extra CLI arguments
- Profile has display metadata (name, description, icon)

### US-2: Store Profile Globally

**As a** user, **I want to** store profiles globally in ~/.agile-agent/, **so that** they are available across all workplaces.

**Acceptance Criteria**:
- Profiles stored in ~/.agile-agent/profiles.json
- File created automatically if not exists
- Default profiles generated for each CLI type
- Profiles persist across sessions

### US-3: Store Profile at Workplace Level

**As a** user, **I want to** store workplace-specific profiles in .agile-agent/, **so that** project-specific LLM configurations can override global defaults.

**Acceptance Criteria**:
- Workplace profiles stored in workplace's .agile-agent/ directory
- Workplace profiles override global profiles with same ID
- Workplace can define its own default work/decision profiles
- Missing workplace profiles fall back to global

### US-4: Create Agent with Profile

**As a** user, **I want to** create an agent using a specific profile, **so that** the agent uses the configured LLM backend.

**Acceptance Criteria**:
- AgentPool.spawn_agent_with_profile(profile_id) method
- Profile resolved to LaunchInputSpec with env overrides
- Environment variables interpolated from host environment
- Error if profile not found or env var missing

### US-5: Create Worktree Agent with Profile

**As a** user, **I want to** create an worktree-isolated agent with a profile, **so that** parallel agents can use different LLM backends.

**Acceptance Criteria**:
- AgentPool.spawn_agent_with_worktree_and_profile(profile_id, branch, task)
- Each worktree agent can have different profile
- Worktree state stores profile_id used

### US-6: Separate Decision Profile

**As a** user, **I want to** specify a different profile for the decision layer, **so that** decision-making can use a simpler/faster model.

**Acceptance Criteria**:
- DecisionAgentSlot accepts profile_id parameter
- Decision layer has independent default profile setting
- Decision profile resolved separately from work profile
- Decision profile can differ from work profile

### US-7: Backward Compatibility

**As a** existing user, **I want to** continue using spawn_agent(ProviderKind), **so that** my existing code doesn't break.

**Acceptance Criteria**:
- spawn_agent(ProviderKind) still works
- ProviderKind maps to corresponding default profile
- No changes to existing agent creation flow
- Tests for backward compatibility pass

### US-8: List Available Profiles

**As a** user, **I want to** list all available profiles, **so that** I can see my options when creating an agent.

**Acceptance Criteria**:
- CLI --list-profiles command shows all profiles
- TUI profile selector dropdown available
- Both global and workplace profiles shown
- Default profiles marked clearly

## Technical Requirements

### TR-1: CliBaseType Enum

New enum for CLI executable types:
```rust
pub enum CliBaseType {
    Mock,
    Claude,
    Codex,
    OpenCode, // Future support
}
```

### TR-2: ProviderProfile Struct

Profile definition structure:
```rust
pub struct ProviderProfile {
    pub id: ProfileId,
    pub base_cli: CliBaseType,
    pub env_overrides: BTreeMap<String, String>,
    pub extra_args: Vec<String>,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}
```

### TR-3: Environment Variable Interpolation

Syntax: `${ENV_VAR_NAME}`
- Interpolation happens at profile resolution time
- Missing env vars are left as literal `${VAR}` (no error)
- Only ${} syntax supported (not $VAR or ${VAR:-default})

### TR-4: Profile Resolution Flow

```
ProfileId → ProviderProfile → LaunchInputSpec → ResolvedLaunchSpec → ProviderLaunchContext
```

### TR-5: Profile Priority Chain

1. Explicit profile_id in API call
2. Workplace default profile
3. Global default profile
4. ProviderKind-derived default profile

### TR-6: Profile Storage Format

JSON file with schema:
```json
{
  "profiles": {
    "profile-id": { ... }
  },
  "default_work_profile": "profile-id",
  "default_decision_profile": "profile-id"
}
```

## Quality Requirements

### QR-1: Test Coverage

- Unit tests for all new structs and methods
- Integration tests for profile-based agent creation
- Backward compatibility tests
- Minimum 90% coverage for new code

### QR-2: Documentation

- Inline documentation for all public APIs
- README section for profile usage
- Example profiles.json

### QR-3: Error Handling

- Clear error messages for missing profiles
- Clear error messages for missing env vars
- Validation errors for invalid profile definitions

## Implementation Phases

### Phase 1: Core Data Model (Sprint 1)

- CliBaseType enum
- ProviderProfile struct
- ProfileStore struct
- Default profile generation

### Phase 2: Configuration Layer (Sprint 2)

- Profile persistence
- Profile loader/resolver
- Environment interpolation

### Phase 3: Agent Integration (Sprint 3)

- AgentPool profile support
- Decision agent profile support
- Launch context integration

### Phase 4: UI & CLI (Sprint 4)

- TUI profile selector
- CLI profile commands

## Dependencies

- Existing LaunchInputSpec/ResolvedLaunchSpec system
- Existing ProviderLaunchContext
- GlobalConfigStore for file storage

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Env interpolation edge cases | Medium | Comprehensive test cases |
| Backward compatibility break | High | Extensive compatibility tests |
| Profile ID conflicts | Low | Clear merge rules documented |

## Success Metrics

1. Users can create agents with custom profiles
2. Agents use configured LLM backends
3. Decision layer can use different model
4. Existing code continues to work
5. Profile configuration is intuitive