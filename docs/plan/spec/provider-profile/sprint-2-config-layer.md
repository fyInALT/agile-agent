# Sprint 2: Configuration Layer

## Metadata

- Sprint ID: `provider-profile-sprint-02`
- Title: `Configuration Layer`
- Duration: 1 week
- Priority: P0 (Critical)
- Status: `Completed`
- Created: 2026-04-19
- Depends On: `provider-profile-sprint-01`
- Design Reference: `docs/plan/provider-profile-requirements.md`

## Sprint Goal

Implement profile persistence at both global and workplace levels, profile loader/resolver, and integration with existing launch configuration system.

## Stories

### Story 2.1: Global Profile Persistence

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Implement global profile file storage in ~/.agile-agent/.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.1.1 | Define `ProfilePersistence` struct | Todo | - |
| T2.1.2 | Implement `load_global_profiles()` | Todo | - |
| T2.1.3 | Implement `save_global_profiles()` | Todo | - |
| T2.1.4 | Create default profiles.json if missing | Todo | - |
| T2.1.5 | Handle JSON parse errors gracefully | Todo | - |
| T2.1.6 | Integrate with `GlobalConfigStore` | Todo | - |
| T2.1.7 | Write unit tests for persistence | Todo | - |

#### Technical Design

```rust
use std::path::PathBuf;
use std::fs;
use anyhow::Result;

pub struct ProfilePersistence {
    global_path: PathBuf,
}

impl ProfilePersistence {
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("home directory unavailable"))?;
        let global_path = home.join(".agile-agent").join("profiles.json");
        Ok(Self { global_path })
    }
    
    pub fn global_path(&self) -> &PathBuf {
        &self.global_path
    }
    
    pub fn load_global(&self) -> Result<ProfileStore> {
        if !self.global_path.exists() {
            // Create default profiles
            let store = ProfileStore::with_defaults();
            self.save_global(&store)?;
            return Ok(store);
        }
        
        let content = fs::read_to_string(&self.global_path)?;
        let store: ProfileStore = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse profiles.json: {}", e))?;
        Ok(store)
    }
    
    pub fn save_global(&self, store: &ProfileStore) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.global_path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(store)?;
        fs::write(&self.global_path, content)?;
        Ok(())
    }
}
```

---

### Story 2.2: Workplace Profile Persistence

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Implement workplace-level profile storage with override semantics.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.2.1 | Add workplace path to ProfilePersistence | Todo | - |
| T2.2.2 | Implement `load_workplace_profiles()` | Todo | - |
| T2.2.3 | Implement `save_workplace_profiles()` | Todo | - |
| T2.2.4 | Implement merge/override logic | Todo | - |
| T2.2.5 | Handle workplace profile absence | Todo | - |
| T2.2.6 | Write unit tests for workplace persistence | Todo | - |

#### Technical Design

```rust
impl ProfilePersistence {
    pub fn with_workplace(workplace_path: PathBuf) -> Self {
        let global_path = ...; // as before
        Self { global_path, workplace_path: Some(workplace_path) }
    }
    
    pub fn workplace_path(&self) -> Option<PathBuf> {
        self.workplace_path.as_ref()
            .map(|p| p.join(".agile-agent").join("profiles.json"))
    }
    
    /// Load merged profiles: workplace overrides global
    pub fn load_merged(&self) -> Result<ProfileStore> {
        let global = self.load_global()?;
        
        if let Some(workplace_path) = self.workplace_path() {
            if workplace_path.exists() {
                let content = fs::read_to_string(&workplace_path)?;
                let workplace: ProfileStore = serde_json::from_str(&content)?;
                return Ok(self.merge_stores(global, workplace));
            }
        }
        
        Ok(global)
    }
    
    fn merge_stores(&self, global: ProfileStore, workplace: ProfileStore) -> ProfileStore {
        let mut merged = global.clone();
        
        // Workplace profiles override global
        for (id, profile) in workplace.profiles {
            merged.profiles.insert(id, profile);
        }
        
        // Workplace defaults override global
        if workplace.profiles.contains_key(&workplace.default_work_profile) {
            merged.default_work_profile = workplace.default_work_profile;
        }
        if workplace.profiles.contains_key(&workplace.default_decision_profile) {
            merged.default_decision_profile = workplace.default_decision_profile;
        }
        
        merged
    }
}
```

---

### Story 2.3: Profile Resolver

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Implement profile resolution to LaunchInputSpec and ResolvedLaunchSpec.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T2.3.1 | Define `resolve_profile()` function | Todo | - |
| T2.3.2 | Convert ProviderProfile → LaunchInputSpec | Todo | - |
| T2.3.3 | Interpolate env overrides | Todo | - |
| T2.3.4 | Resolve executable path from CliBaseType | Todo | - |
| T2.3.5 | Handle resolution errors | Todo | - |
| T2.3.6 | Integrate with existing resolver | Todo | - |
| T2.3.7 | Write unit tests for resolution | Todo | - |

#### Technical Design

```rust
use crate::launch_config::spec::{LaunchInputSpec, ResolvedLaunchSpec};
use crate::launch_config::resolver::resolve_launch_spec;

/// Resolve a profile to a LaunchInputSpec
pub fn profile_to_launch_input(profile: &ProviderProfile) -> Result<LaunchInputSpec> {
    // Interpolate env values
    let resolved_env = interpolate_profile_env(profile)?;
    
    let provider_kind = profile.base_cli.to_provider_kind()
        .ok_or_else(|| anyhow::anyhow!("CliBaseType {} not supported as ProviderKind", profile.base_cli.label()))?;
    
    Ok(LaunchInputSpec {
        provider: provider_kind,
        source_mode: LaunchSourceMode::EnvOnly,
        source_origin: LaunchSourceOrigin::Template,
        raw_text: None,
        env_overrides: resolved_env,
        requested_executable: None,
        extra_args: profile.extra_args.clone(),
        template_id: Some(profile.id.clone()),
    })
}

/// Resolve a profile to a ResolvedLaunchSpec (ready for provider launch)
pub fn resolve_profile(profile: &ProviderProfile) -> Result<ResolvedLaunchSpec> {
    let input = profile_to_launch_input(profile)?;
    resolve_launch_spec(&input)
}

/// Resolve a profile by ID from store
pub fn resolve_profile_by_id(
    store: &ProfileStore,
    profile_id: &ProfileId,
) -> Result<ResolvedLaunchSpec> {
    let profile = store.get_profile(profile_id)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_id))?;
    resolve_profile(profile)
}

/// Get effective profile with priority chain
pub fn get_effective_profile(
    store: &ProfileStore,
    explicit_id: Option<&ProfileId>,
    agent_type: AgentType, // Work or Decision
) -> Result<&ProviderProfile> {
    // Priority: explicit > workplace default > global default
    if let Some(id) = explicit_id {
        return store.get_profile(id)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", id));
    }
    
    let default_id = match agent_type {
        AgentType::Work => &store.default_work_profile,
        AgentType::Decision => &store.default_decision_profile,
    };
    
    store.get_profile(default_id)
        .ok_or_else(|| anyhow::anyhow!("Default profile '{}' not found", default_id))
}
```

---

## Sprint Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Workplace path detection | Medium | Low | Use existing WorkplaceStore |
| Profile merge complexity | Low | Medium | Clear override rules documented |
| JSON parse errors | Low | Low | Graceful error handling with defaults |

## Sprint Deliverables

- `core/src/provider_profile/persistence.rs` - File storage
- `core/src/provider_profile/resolver.rs` - Profile resolution
- Integration with GlobalConfigStore
- Integration with launch_config resolver
- Complete unit test coverage

## Dependencies

- Sprint 1 deliverables
- Existing `GlobalConfigStore`
- Existing `launch_config` module
- Existing `WorkplaceStore` for workplace path

## Next Sprint

After completing this sprint, proceed to [Sprint 3: Agent Integration](./sprint-3-agent-integration.md).