# Sprint 1: Core Data Model & Parser

## Metadata

- Sprint ID: `launch-config-sprint-01`
- Title: `Core Data Model & Parser`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: None
- Design Reference: `docs/superpowers/specs/2026-04-16-agent-launch-config-and-resume-design.md`

## Sprint Goal

Establish the core data model for launch configuration, including `LaunchInputSpec`, `ResolvedLaunchSpec`, and `AgentLaunchBundle`. Implement input parsers supporting both env-only and command-fragment syntaxes with comprehensive validation logic.

## Stories

### Story 1.1: LaunchInputSpec Data Model

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define the declarative user input representation.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `core/src/launch_config/mod.rs` module entry | Todo | - |
| T1.1.2 | Create `core/src/launch_config/spec.rs` for data models | Todo | - |
| T1.1.3 | Define `LaunchInputSpec` struct with all fields | Todo | - |
| T1.1.4 | Define `LaunchSourceMode` enum (HostDefault, EnvOnly, CommandFragment) | Todo | - |
| T1.1.5 | Define `LaunchSourceOrigin` enum (Manual, Template, HostDefault) | Todo | - |
| T1.1.6 | Implement `Serialize/Deserialize` for all types | Todo | - |
| T1.1.7 | Write unit tests for struct construction | Todo | - |

#### Technical Design

```rust
pub struct LaunchInputSpec {
    pub provider: ProviderKind,
    pub source_mode: LaunchSourceMode,
    pub source_origin: LaunchSourceOrigin,
    pub raw_text: Option<String>,
    pub env_overrides: BTreeMap<String, String>,
    pub requested_executable: Option<String>,
    pub extra_args: Vec<String>,
    pub template_id: Option<String>,
}

pub enum LaunchSourceMode {
    HostDefault,
    EnvOnly,
    CommandFragment,
}

pub enum LaunchSourceOrigin {
    Manual,
    Template,
    HostDefault,
}
```

---

### Story 1.2: ResolvedLaunchSpec Data Model

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define the resolved launch configuration used by provider execution.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Define `ResolvedLaunchSpec` struct | Todo | - |
| T1.2.2 | Add `resolved_executable_path` field | Todo | - |
| T1.2.3 | Add `effective_env` field (BTreeMap<String, String>) | Todo | - |
| T1.2.4 | Add `extra_args`, `resolved_at`, `derived_from` fields | Todo | - |
| T1.2.5 | Add `resolution_notes` for debugging | Todo | - |
| T1.2.6 | Implement `Serialize/Deserialize` | Todo | - |
| T1.2.7 | Write unit tests for struct construction | Todo | - |

#### Technical Design

```rust
pub struct ResolvedLaunchSpec {
    pub provider: ProviderKind,
    pub resolved_executable_path: String,
    pub effective_env: BTreeMap<String, String>,
    pub extra_args: Vec<String>,
    pub resolved_at: String,
    pub derived_from: LaunchSourceMode,
    pub resolution_notes: Vec<String>,
}
```

---

### Story 1.3: AgentLaunchBundle Data Model

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Define the bundle combining work and decision agent configs.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Define `AgentLaunchBundle` struct | Todo | - |
| T1.3.2 | Add `work_input`, `work_resolved` fields | Todo | - |
| T1.3.3 | Add `decision_input`, `decision_resolved` fields | Todo | - |
| T1.3.4 | Implement `Serialize/Deserialize` | Todo | - |
| T1.3.5 | Write unit tests for bundle construction | Todo | - |

#### Technical Design

```rust
pub struct AgentLaunchBundle {
    pub work_input: LaunchInputSpec,
    pub work_resolved: ResolvedLaunchSpec,
    pub decision_input: LaunchInputSpec,
    pub decision_resolved: ResolvedLaunchSpec,
}
```

---

### Story 1.4: Env-Only Input Parser

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Parse pure environment variable syntax (KEY=VALUE per line).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Create `core/src/launch_config/parser.rs` | Todo | - |
| T1.4.2 | Implement `detect_source_mode()` function | Todo | - |
| T1.4.3 | Implement `parse_env_only()` for KEY=VALUE parsing | Todo | - |
| T1.4.4 | Handle empty lines and whitespace | Todo | - |
| T1.4.5 | Validate KEY format (non-empty, valid identifier) | Todo | - |
| T1.4.6 | Handle VALUE with special characters and spaces | Todo | - |
| T1.4.7 | Return `LaunchInputSpec` with `EnvOnly` mode | Todo | - |
| T1.4.8 | Write parser unit tests (TDD) | Todo | - |

#### Input Constraints

- Max line length: 4096 characters
- Max total input: 64KB
- Max env variables: 100

#### Test Cases

```rust
// Success cases
parse_env_only("KEY=value") -> Ok(LaunchInputSpec { env_overrides: {"KEY": "value"} })
parse_env_only("KEY1=val1\nKEY2=val2") -> Ok(...)
parse_env_only("  KEY=value  ") -> Ok(...) // trim whitespace
parse_env_only("KEY=value with spaces") -> Ok(...)

// Failure cases
parse_env_only("") -> Err("empty input")
parse_env_only("=value") -> Err("empty key")
parse_env_only("INVALID KEY=value") -> Err("invalid key format")
```

---

### Story 1.5: Command-Fragment Input Parser

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Parse full command fragment syntax with env prefix, executable, and args.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.5.1 | Implement shell-style tokenization | Todo | - |
| T1.5.2 | Detect env prefix vs executable boundary | Todo | - |
| T1.5.3 | Identify executable token position | Todo | - |
| T1.5.4 | Collect extra args after executable | Todo | - |
| T1.5.5 | Handle quoted values in env assignments | Todo | - |
| T1.5.6 | Return `LaunchInputSpec` with `CommandFragment` mode | Todo | - |
| T1.5.7 | Write parser unit tests (TDD) | Todo | - |

#### Parsing Rules

- Env prefix: tokens matching `KEY=VALUE` pattern
- Executable: first non-env token
- Extra args: all tokens after executable

#### Test Cases

```rust
// Success cases
parse_fragment("claude") -> Ok(LaunchInputSpec { requested_executable: "claude" })
parse_fragment("ANTHROPIC_MODEL=X claude --flag") -> Ok(...)
parse_fragment("KEY=\"quoted value\" claude") -> Ok(...)

// Failure cases
parse_fragment("") -> Err("empty input")
parse_fragment("--flag") -> Err("no executable found")
parse_fragment("KEY=value") -> Err("no executable") // env-only should use other parser
```

---

### Story 1.6: Provider Consistency Validation

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Validate that command fragment executable matches selected provider.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.6.1 | Create `core/src/launch_config/validation.rs` | Todo | - |
| T1.6.2 | Define `validate_provider_consistency()` | Todo | - |
| T1.6.3 | Match executable name to ProviderKind | Todo | - |
| T1.6.4 | Handle executable path variations (e.g., `/usr/local/bin/claude`) | Todo | - |
| T1.6.5 | Return `ValidationError::ProviderMismatch` on failure | Todo | - |
| T1.6.6 | Write validation unit tests | Todo | - |

#### Validation Logic

```rust
fn validate_provider_consistency(
    selected: ProviderKind,
    executable: &str,
) -> Result<(), ValidationError> {
    // Extract basename from path
    let basename = Path::new(executable).file_name()?;
    match (selected, basename) {
        (ProviderKind::Claude, "claude" | "claude.exe") => Ok(()),
        (ProviderKind::Codex, "codex" | "codex.exe") => Ok(()),
        _ => Err(ValidationError::ProviderMismatch { selected, found: basename }),
    }
}
```

---

### Story 1.7: Reserved Arguments Validation

**Priority**: P1
**Effort**: 2 points
**Status**: Backlog

Validate that user-supplied args don't conflict with provider-reserved arguments.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.7.1 | Define `RESERVED_ARGS_CLAUDE` constant list | Todo | - |
| T1.7.2 | Define `RESERVED_ARGS_CODEX` constant list | Todo | - |
| T1.7.3 | Implement `validate_reserved_args()` | Todo | - |
| T1.7.4 | Return `ValidationError::ReservedArgumentConflict` | Todo | - |
| T1.7.5 | Write validation unit tests | Todo | - |

#### Reserved Arguments

```rust
const RESERVED_ARGS_CLAUDE: &[&str] = &[
    "-p", "--bare", "--output-format", "--input-format",
    "--verbose", "--strict-mcp-config", "--permission-mode", "--resume",
];

const RESERVED_ARGS_CODEX: &[&str] = &[
    "exec", "--json", "--full-auto",
];
```

---

### Story 1.8: Mock Provider Exclusion

**Priority**: P1
**Effort**: 1 point
**Status**: Backlog

Explicitly reject launch overrides for Mock provider.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.8.1 | Add `validate_provider_supports_launch_config()` | Todo | - |
| T1.8.2 | Return `ValidationError::MockProviderNoOverrides` | Todo | - |
| T1.8.3 | Write unit test for Mock rejection | Todo | - |

---

### Story 1.9: Parser Unit Tests (TDD)

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Comprehensive test coverage for all parser scenarios.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.9.1 | Test env-only success cases | Todo | - |
| T1.9.2 | Test env-only failure cases (empty key, invalid format) | Todo | - |
| T1.9.3 | Test command-fragment success cases | Todo | - |
| T1.9.4 | Test command-fragment failure cases (no executable) | Todo | - |
| T1.9.5 | Test provider-mismatch validation | Todo | - |
| T1.9.6 | Test reserved-argument conflict detection | Todo | - |
| T1.9.7 | Test mixed input (env + executable + args) | Todo | - |
| T1.9.8 | Test edge cases (whitespace, quotes, empty lines) | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Shell tokenization complexity | Medium | Medium | Use `shell-words` crate or simplified parsing |
| Env variable edge cases | Low | Low | Comprehensive test coverage |
| Cross-platform path handling | Low | Medium | Use std::path::Path abstractions |

## Sprint Deliverables

- `core/src/launch_config/mod.rs` - Module entry point
- `core/src/launch_config/spec.rs` - Data model definitions
- `core/src/launch_config/parser.rs` - Input parsers
- `core/src/launch_config/validation.rs` - Validation logic
- Complete parser unit test coverage

## Dependencies

- None (foundation sprint)

## Module Structure

```
core/src/launch_config/
├── mod.rs          # Module exports
├── spec.rs         # LaunchInputSpec, ResolvedLaunchSpec, AgentLaunchBundle
├── parser.rs       # Env-only and command-fragment parsers
├── validation.rs   # Provider consistency, reserved args, Mock exclusion
└── error.rs        # ParseError, ValidationError types (optional)
```

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Resolver & Persistence](./sprint-2-resolver-persistence.md).
