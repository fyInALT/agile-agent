# Sprint 1: Core Data Model

## Metadata

- Sprint ID: `provider-profile-sprint-01`
- Title: `Core Data Model`
- Duration: 1 week
- Priority: P0 (Critical)
- Status: `Backlog`
- Created: 2026-04-19
- Depends On: None
- Design Reference: `docs/plan/provider-profile-requirements.md`

## Sprint Goal

Establish the core data model for provider profiles, including `CliBaseType`, `ProviderProfile`, and `ProfileStore` structs with comprehensive test coverage.

## Stories

### Story 1.1: CliBaseType Enum

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Define the CLI base type enumeration for provider executables.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.1.1 | Create `core/src/provider_profile/mod.rs` module entry | Todo | - |
| T1.1.2 | Define `CliBaseType` enum (Mock, Claude, Codex, OpenCode) | Todo | - |
| T1.1.3 | Implement `Serialize/Deserialize` for CliBaseType | Todo | - |
| T1.1.4 | Implement `label()` and `display_name()` methods | Todo | - |
| T1.1.5 | Implement conversion from/to `ProviderKind` | Todo | - |
| T1.1.6 | Write unit tests for enum construction | Todo | - |

#### Technical Design

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CliBaseType {
    Mock,
    Claude,
    Codex,
    #[serde(rename = "opencode")]
    OpenCode,
}

impl CliBaseType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }
    
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Mock => "Mock Provider",
            Self::Claude => "Claude CLI",
            Self::Codex => "Codex CLI",
            Self::OpenCode => "OpenCode CLI",
        }
    }
    
    pub fn from_provider_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Mock => Self::Mock,
            ProviderKind::Claude => Self::Claude,
            ProviderKind::Codex => Self::Codex,
        }
    }
    
    pub fn to_provider_kind(&self) -> Option<ProviderKind> {
        match self {
            Self::Mock => Some(ProviderKind::Mock),
            Self::Claude => Some(ProviderKind::Claude),
            Self::Codex => Some(ProviderKind::Codex),
            Self::OpenCode => None, // Not yet supported as ProviderKind
        }
    }
}
```

---

### Story 1.2: ProviderProfile Struct

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the named provider profile structure with all configuration fields.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.2.1 | Define `ProfileId` type alias | Todo | - |
| T1.2.2 | Define `ProviderProfile` struct with all fields | Todo | - |
| T1.2.3 | Implement `Serialize/Deserialize` with defaults | Todo | - |
| T1.2.4 | Implement `new()` constructor | Todo | - |
| T1.2.5 | Implement validation methods | Todo | - |
| T1.2.6 | Write unit tests for profile construction | Todo | - |

#### Technical Design

```rust
pub type ProfileId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderProfile {
    /// Unique profile identifier
    pub id: ProfileId,
    /// Base CLI executable type
    pub base_cli: CliBaseType,
    /// Environment variable overrides (supports ${ENV_VAR} interpolation)
    #[serde(default)]
    pub env_overrides: BTreeMap<String, String>,
    /// Extra arguments for the CLI
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Display name for UI
    pub display_name: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Optional icon/emoji for UI
    #[serde(default)]
    pub icon: Option<String>,
}

impl ProviderProfile {
    pub fn new(id: ProfileId, base_cli: CliBaseType) -> Self {
        Self {
            id,
            base_cli,
            env_overrides: BTreeMap::new(),
            extra_args: Vec::new(),
            display_name: format!("{} Profile", base_cli.display_name()),
            description: None,
            icon: None,
        }
    }
    
    pub fn with_env(mut self, key: String, value: String) -> Self {
        self.env_overrides.insert(key, value);
        self
    }
    
    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = name;
        self
    }
    
    /// Check if env value uses interpolation syntax
    pub fn uses_env_reference(value: &str) -> bool {
        value.starts_with("${") && value.ends_with("}")
    }
    
    /// Extract env var name from ${VAR} syntax
    pub fn extract_env_var_name(value: &str) -> Option<String> {
        if Self::uses_env_reference(value) {
            Some(value[2..value.len()-1].to_string())
        } else {
            None
        }
    }
}
```

---

### Story 1.3: ProfileStore Struct

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Define the profile storage structure with default profile management.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.3.1 | Define `ProfileStore` struct | Todo | - |
| T1.3.2 | Implement profile storage map | Todo | - |
| T1.3.3 | Implement default profile fields | Todo | - |
| T1.3.4 | Implement `add_profile()` and `remove_profile()` | Todo | - |
| T1.3.5 | Implement `get_profile()` and `list_profiles()` | Todo | - |
| T1.3.6 | Implement default profile generation | Todo | - |
| T1.3.7 | Write unit tests for store operations | Todo | - |

#### Technical Design

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStore {
    /// All defined profiles
    #[serde(default)]
    profiles: BTreeMap<ProfileId, ProviderProfile>,
    /// Default profile for work agents
    #[serde(default = "default_work_profile")]
    default_work_profile: ProfileId,
    /// Default profile for decision layer
    #[serde(default = "default_decision_profile")]
    default_decision_profile: ProfileId,
}

fn default_work_profile() -> ProfileId {
    "claude-default".to_string()
}

fn default_decision_profile() -> ProfileId {
    "claude-default".to_string()
}

impl ProfileStore {
    pub fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
            default_work_profile: default_work_profile(),
            default_decision_profile: default_decision_profile(),
        }
    }
    
    /// Create store with default profiles for each CLI type
    pub fn with_defaults() -> Self {
        let mut store = Self::new();
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Mock));
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Claude));
        store.add_profile(ProviderProfile::default_for_cli(CliBaseType::Codex));
        store
    }
    
    pub fn add_profile(&mut self, profile: ProviderProfile) {
        self.profiles.insert(profile.id.clone(), profile);
    }
    
    pub fn remove_profile(&mut self, id: &ProfileId) -> Option<ProviderProfile> {
        self.profiles.remove(id)
    }
    
    pub fn get_profile(&self, id: &ProfileId) -> Option<&ProviderProfile> {
        self.profiles.get(id)
    }
    
    pub fn list_profiles(&self) -> Vec<&ProviderProfile> {
        self.profiles.values().collect()
    }
    
    pub fn set_default_work_profile(&mut self, id: ProfileId) -> Result<(), String> {
        if self.profiles.contains_key(&id) {
            self.default_work_profile = id;
            Ok(())
        } else {
            Err(format!("Profile '{}' not found", id))
        }
    }
    
    pub fn set_default_decision_profile(&mut self, id: ProfileId) -> Result<(), String> {
        if self.profiles.contains_key(&id) {
            self.default_decision_profile = id;
            Ok(())
        } else {
            Err(format!("Profile '{}' not found", id))
        }
    }
}
```

---

### Story 1.4: Environment Variable Interpolation

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement environment variable interpolation for ${ENV_VAR} syntax.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T1.4.1 | Define `interpolate_env_value()` function | Todo | - |
| T1.4.2 | Implement regex-based parsing | Todo | - |
| T1.4.3 | Handle missing env var errors | Todo | - |
| T1.4.4 | Support nested interpolation (optional) | Todo | - |
| T1.4.5 | Write unit tests for interpolation | Todo | - |

#### Technical Design

```rust
use regex::Regex;

/// Interpolate ${ENV_VAR} references in a value string
pub fn interpolate_env_value(value: &str) -> Result<String, ProfileError> {
    let re = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
    
    let mut result = value.to_string();
    let mut missing_vars = Vec::new();
    
    for cap in re.captures_iter(value) {
        let var_name = &cap[1];
        match std::env::var(var_name) {
            Ok(var_value) => {
                result = result.replace(&cap[0], &var_value);
            }
            Err(_) => {
                missing_vars.push(var_name.to_string());
            }
        }
    }
    
    if missing_vars.is_empty() {
        Ok(result)
    } else {
        Err(ProfileError::MissingEnvVars(missing_vars))
    }
}

/// Interpolate all env overrides in a profile
pub fn interpolate_profile_env(profile: &ProviderProfile) -> Result<BTreeMap<String, String>, ProfileError> {
    let mut resolved = BTreeMap::new();
    for (key, value) in &profile.env_overrides {
        let interpolated = interpolate_env_value(value)?;
        resolved.insert(key.clone(), interpolated);
    }
    Ok(resolved)
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Regex complexity for interpolation | Low | Low | Use simple regex, limit to ${VAR} syntax |
| Default profile naming conflicts | Low | Low | Use consistent naming convention |
| CliBaseType vs ProviderKind mapping | Medium | Medium | Clear conversion functions with tests |

## Sprint Deliverables

- `core/src/provider_profile/mod.rs` - Module entry point
- `core/src/provider_profile/types.rs` - CliBaseType enum
- `core/src/provider_profile/profile.rs` - ProviderProfile struct
- `core/src/provider_profile/store.rs` - ProfileStore struct
- `core/src/provider_profile/interpolate.rs` - Env interpolation
- Complete unit test coverage

## Dependencies

- Existing `ProviderKind` enum
- regex crate (add to Cargo.toml if needed)

## Module Structure

```
core/src/provider_profile/
├── mod.rs          # Module exports
├── types.rs        # CliBaseType enum, ProfileId type
├── profile.rs      # ProviderProfile struct
├── store.rs        # ProfileStore struct
├── interpolate.rs  # Environment interpolation
└── error.rs        # ProfileError enum
```

## Next Sprint

After completing this sprint, proceed to [Sprint 2: Configuration Layer](./sprint-2-config-layer.md).