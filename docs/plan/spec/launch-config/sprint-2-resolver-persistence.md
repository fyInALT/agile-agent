# Sprint 2: Resolver & Persistence

## Metadata

- Sprint ID: `launch-config-sprint-02`
- Title: `Resolver & Persistence`
- Duration: 2 weeks
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-16
- Depends On: Sprint 1 (Core Data Model & Parser)
- Design Reference: `docs/superpowers/specs/2026-04-16-agent-launch-config-and-resume-design.md`

## Sprint Goal

Implement the environment resolver that captures host default configuration, persist launch bundles to agent directories and TUI snapshots, and handle snapshot version migration for backward compatibility.

## Stories

### Story 2.1: Host Default Environment Resolver

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Resolve host environment variables into captured snapshot for deterministic resume.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Create `core/src/launch_config/resolver.rs` | Todo | - |
| T2.1.2 | Define `HostEnvWhitelist` for inherited variables | Todo | - |
| T2.1.3 | Implement `resolve_host_env()` function | Todo | - |
| T2.1.4 | Capture PATH, HOME, USER, SHELL, LANG, LC_ALL | Todo | - |
| T2.1.5 | Capture provider-specific variables (ANTHROPIC_*, CODEX_*) | Todo | - |
| T2.1.6 | Merge with explicit env_overrides | Todo | - |
| T2.1.7 | Write resolver unit tests | Todo | - |

#### Host Environment Inheritance Strategy

```rust
const HOST_ENV_WHITELIST: &[&str] = &[
    "PATH", "HOME", "USER", "SHELL", "LANG", "LC_ALL",
];

const PROVIDER_ENV_PREFIXES: &[&str] = &[
    "ANTHROPIC_", "CODEX_", "OPENAI_", "API_",
];

fn resolve_host_env(provider: ProviderKind) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    
    // Inherit whitelist variables
    for key in HOST_ENV_WHITELIST {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }
    
    // Inherit provider-specific variables
    for (key, value) in std::env::vars() {
        if PROVIDER_ENV_PREFIXES.iter().any(|p| key.starts_with(p)) {
            env.insert(key, value);
        }
    }
    
    env
}
```

---

### Story 2.2: Executable Path Resolver

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Resolve provider executable path using `which` crate.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Implement `resolve_executable_path()` | Todo | - |
| T2.2.2 | Handle custom executable from LaunchInputSpec | Todo | - |
| T2.2.3 | Handle CLAUDE_PATH_ENV / CODEX_PATH_ENV overrides | Todo | - |
| T2.2.4 | Default to standard executable name if not specified | Todo | - |
| T2.2.5 | Return absolute path as String | Todo | - |
| T2.2.6 | Write resolver unit tests | Todo | - |

#### Resolution Logic

```rust
fn resolve_executable_path(
    provider: ProviderKind,
    requested: Option<&str>,
) -> Result<String> {
    let executable_name = requested
        .or_else(|| default_executable_name(provider))
        .unwrap_or_else(|| provider.label());
    
    // Check environment override first
    let env_override = match provider {
        ProviderKind::Claude => std::env::var("CLAUDE_PATH_ENV").ok(),
        ProviderKind::Codex => std::env::var("CODEX_PATH_ENV").ok(),
        _ => None,
    };
    
    let resolved = if let Some(custom_path) = env_override {
        custom_path
    } else {
        which::which(executable_name)?.display().to_string()
    };
    
    Ok(resolved)
}
```

---

### Story 2.3: ResolvedLaunchSpec Generator

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Generate complete ResolvedLaunchSpec from LaunchInputSpec + host environment.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Implement `resolve_launch_spec()` function | Todo | - |
| T2.3.2 | Merge host env with explicit overrides | Todo | - |
| T2.3.3 | Set resolved_at timestamp | Todo | - |
| T2.3.4 | Track derived_from mode | Todo | - |
| T2.3.5 | Add resolution_notes for debugging | Todo | - |
| T2.3.6 | Write generator unit tests | Todo | - |

#### Generator Implementation

```rust
fn resolve_launch_spec(
    input: &LaunchInputSpec,
) -> Result<ResolvedLaunchSpec> {
    let resolved_executable = resolve_executable_path(
        input.provider,
        input.requested_executable.as_deref(),
    )?;
    
    let host_env = resolve_host_env(input.provider);
    let mut effective_env = host_env;
    
    // Explicit overrides win
    for (key, value) in &input.env_overrides {
        effective_env.insert(key.clone(), value.clone());
    }
    
    Ok(ResolvedLaunchSpec {
        provider: input.provider,
        resolved_executable_path: resolved_executable,
        effective_env,
        extra_args: input.extra_args.clone(),
        resolved_at: Utc::now().to_rfc3339(),
        derived_from: input.source_mode,
        resolution_notes: vec![
            format!("Resolved from {} mode", input.source_mode),
        ],
    })
}
```

---

### Story 2.4: Decision Config Independent Resolution

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Ensure decision config resolves independently from work config (empty = host default, NOT work config inheritance).

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.4.1 | Implement `resolve_decision_launch_spec()` | Todo | - |
| T2.4.2 | Empty decision input resolves to host default | Todo | - |
| T2.4.3 | NEVER inherit from work_resolved | Todo | - |
| T2.4.4 | Add unit test verifying independence | Todo | - |
| T2.4.5 | Add integration test with different work/decision configs | Todo | - |

#### Key Design Decision

```rust
// IMPORTANT: Decision config never inherits from work config
fn resolve_decision_launch_spec(
    decision_input: &LaunchInputSpec,
) -> Result<ResolvedLaunchSpec> {
    // Always resolve from host environment, not work_resolved
    resolve_launch_spec(decision_input)
}

// Example test case:
// work_input: env_overrides = {"ANTHROPIC_MODEL": "claude-opus"}
// decision_input: empty (HostDefault mode)
// Expected:
//   work_resolved.effective_env["ANTHROPIC_MODEL"] = "claude-opus"
//   decision_resolved.effective_env["ANTHROPIC_MODEL"] = <host default, NOT "claude-opus">
```

---

### Story 2.5: launch-config.json Persistence

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Persist AgentLaunchBundle to agent directory as JSON file.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.5.1 | Create `core/src/launch_config/persistence.rs` | Todo | - |
| T2.5.2 | Implement `save_launch_config()` | Todo | - |
| T2.5.3 | Implement `load_launch_config()` | Todo | - |
| T2.5.4 | Define file path: `<workplace>/agents/<agent-id>/launch-config.json` | Todo | - |
| T2.5.5 | Use pretty JSON format for readability | Todo | - |
| T2.5.6 | Handle file not found gracefully | Todo | - |
| T2.5.7 | Write persistence round-trip tests | Todo | - |

#### Persistence Functions

```rust
pub fn launch_config_path(workplace: &WorkplaceStore, agent_id: &AgentId) -> PathBuf {
    workplace.path()
        .join("agents")
        .join(agent_id.as_str())
        .join("launch-config.json")
}

pub fn save_launch_config(
    workplace: &WorkplaceStore,
    agent_id: &AgentId,
    bundle: &AgentLaunchBundle,
) -> Result<PathBuf> {
    let path = launch_config_path(workplace, agent_id);
    std::fs::create_dir_all(path.parent()?)?;
    let json = serde_json::to_string_pretty(bundle)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

pub fn load_launch_config(
    workplace: &WorkplaceStore,
    agent_id: &AgentId,
) -> Result<Option<AgentLaunchBundle>> {
    let path = launch_config_path(workplace, agent_id);
    if !path.exists() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path)?;
    let bundle = serde_json::from_str(&json)?;
    Ok(Some(bundle))
}
```

---

### Story 2.6: Agent Directory Structure Integration

**Priority**: P0
**Effort**: 1 point
**Status**: Backlog

Ensure launch-config.json integrates with existing agent directory structure.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.6.1 | Verify compatibility with existing agent directories | Todo | - |
| T2.6.2 | Add launch-config.json to agent creation flow | Todo | - |
| T2.6.3 | Ensure directory creation before save | Todo | - |
| T2.6.4 | Write integration test with agent_store | Todo | - |

---

### Story 2.7: PersistedAgentSnapshot Extension

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Add launch_bundle field to TuiResumeSnapshot's PersistedAgentSnapshot.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.7.1 | Add `launch_bundle: Option<AgentLaunchBundle>` to `PersistedAgentSnapshot` | Todo | - |
| T2.7.2 | Update `PersistedAgentSnapshot::from_slot()` to capture bundle | Todo | - |
| T2.7.3 | Update `TuiResumeSnapshot` serialization | Todo | - |
| T2.7.4 | Update `TuiResumeSnapshot::from_state()` | Todo | - |
| T2.7.5 | Write snapshot serialization tests | Todo | - |

#### Modified Structure

```rust
pub struct PersistedAgentSnapshot {
    pub agent_id: AgentId,
    pub codename: AgentCodename,
    pub provider_type: ProviderType,
    pub role: AgentRole,
    pub status: PersistedAgentStatus,
    pub provider_session_id: Option<String>,
    pub transcript: Vec<TranscriptEntry>,
    pub assigned_task_id: Option<TaskId>,
    pub launch_bundle: Option<AgentLaunchBundle>,  // NEW
}
```

---

### Story 2.8: Snapshot Version Migration

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Handle migration from old snapshot format without launch_bundle field.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.8.1 | Add `version` field to `TuiResumeSnapshot` | Todo | - |
| T2.8.2 | Define version constants (V1 = no bundle, V2 = with bundle) | Todo | - |
| T2.8.3 | Implement `deserialize_with_migration()` | Todo | - |
| T2.8.4 | Generate empty launch_bundle for V1 snapshots | Todo | - |
| T2.8.5 | Log warning when migrating old snapshot | Todo | - |
| T2.8.6 | Write migration tests | Todo | - |
| T2.8.7 | Write backward compatibility tests | Todo | - |

#### Migration Strategy

```rust
const SNAPSHOT_VERSION_V1: &str = "v1";  // No launch_bundle
const SNAPSHOT_VERSION_V2: &str = "v2";  // With launch_bundle

pub struct TuiResumeSnapshot {
    pub version: String,  // NEW
    // ... existing fields
}

impl TuiResumeSnapshot {
    fn generate_default_launch_bundle(
        provider_type: ProviderType,
    ) -> AgentLaunchBundle {
        // Generate using host default env
        let work_input = LaunchInputSpec {
            provider: provider_type.into(),
            source_mode: LaunchSourceMode::HostDefault,
            source_origin: LaunchSourceOrigin::HostDefault,
            // ... other defaults
        };
        // ...
    }
}

// Custom deserialization for backward compatibility
fn deserialize_tui_resume_snapshot(json: &str) -> Result<TuiResumeSnapshot> {
    let raw: serde_json::Value = serde_json::from_str(json)?;
    
    let version = raw.get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("v1");
    
    if version == "v1" || !raw.get("agents").unwrap().get(0).unwrap().has("launch_bundle") {
        // Migration needed - generate default bundles
        // ...
    }
    
    serde_json::from_str(json)  // Standard deserialize after patching
}
```

---

### Story 2.9: Persistence Round-Trip Tests

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Comprehensive tests for save/load cycles and migration scenarios.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.9.1 | Test launch-config.json save/load round-trip | Todo | - |
| T2.9.2 | Test TuiResumeSnapshot save/load round-trip | Todo | - |
| T2.9.3 | Test snapshot with launch_bundle serialization | Todo | - |
| T2.9.4 | Test old snapshot (v1) migration to v2 | Todo | - |
| T2.9.5 | Test corrupted JSON handling | Todo | - |
| T2.9.6 | Test missing launch-config.json handling | Todo | - |
| T2.9.7 | Test snapshot/agent file consistency warning | Todo | - |

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Old snapshot migration break | Low | High | Backup snapshots, extensive migration tests |
| Host env capture incomplete | Medium | Medium | Review whitelist, test on different platforms |
| File path conflicts | Low | Low | Verify existing agent directory structure |

## Sprint Deliverables

- `core/src/launch_config/resolver.rs` - Environment and executable resolution
- `core/src/launch_config/persistence.rs` - launch-config.json save/load
- Modified `tui/src/tui_snapshot.rs` - PersistedAgentSnapshot with launch_bundle
- Snapshot version migration logic
- Complete persistence test coverage

## Dependencies

- Sprint 1: LaunchInputSpec, ResolvedLaunchSpec, AgentLaunchBundle data models
- Existing: WorkplaceStore, AgentId, agent directory structure

## Module Structure

```
core/src/launch_config/
├── mod.rs
├── spec.rs
├── parser.rs
├── validation.rs
├── resolver.rs      # NEW: Host env + executable resolution
└── persistence.rs   # NEW: JSON file save/load
```

## Next Sprint

After completing this sprint, proceed to [Sprint 3: Provider Integration & UI Overlay](./sprint-3-provider-ui.md).
